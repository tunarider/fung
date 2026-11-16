//! When a local lets go of its value (03 "값의 수명"): the last read hands
//! the value over, and what a path will never read again is let go as
//! the statement that read it last ends, or as a branch starts. Until
//! D136 a channel closing when its last sending end went showed this from
//! fung (`tests/lang/local_lifetime.fung`); now a probe does: `token(n)`
//! makes a value (an atom) whose liveness `alive(n)` tells.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use crate::native::nat;
use crate::testutil::{Opts, exec};
use crate::value::{Native, Value};
use crate::vm::{Stop, Task};

thread_local! {
    static TOKENS: RefCell<HashMap<String, Weak<super::atom::Atom>>> = RefCell::new(HashMap::new());
}

fn name(a: &[Value]) -> String {
    let Value::Str(s) = &a[0] else {
        panic!("a name")
    };
    s.lossy().into_owned()
}

fn token(_: &mut Task<'_>, a: crate::value::Args) -> Result<Value, Stop> {
    let v = super::atom::make(Value::Nil)?;
    let Value::Atom(rc) = &v else { unreachable!() };
    TOKENS.with(|t| t.borrow_mut().insert(name(&a), Rc::downgrade(rc)));
    Ok(v)
}

fn alive(_: &mut Task<'_>, a: crate::value::Args) -> Result<Value, Stop> {
    let n = name(&a);
    Ok(Value::Bool(TOKENS.with(|t| {
        t.borrow().get(&n).is_some_and(|w| w.strong_count() > 0)
    })))
}

fn natives() -> Vec<Value> {
    vec![
        Value::Native(Native::leak("token", vec![nat::param("name")], token)),
        Value::Native(Native::leak("alive", vec![nat::param("name")], alive)),
    ]
}

/// `keep(x)` takes the value and drops it (it is pure: `_ = keep(t)`).
const KEEP: &str = "fn keep(_x) = nil\n";

fn out(src: &str) -> String {
    let o = exec(
        &format!("{KEEP}{src}"),
        Opts {
            extra: natives,
            ..Opts::default()
        },
    );
    assert_eq!(o.code, "", "{src}: {}", o.err);
    o.out
}

#[test]
fn a_local_lets_go_after_its_last_read() {
    for (src, want) in [
        // the last read hands the value over within its statement
        (
            "fn f():\n  t = token(\"a\")\n  _ = keep(t)\n  alive(\"a\")\n;;\nprint(f())",
            "false\n",
        ),
        // a local never read lives on until the function returns
        (
            "fn f():\n  _t = token(\"b\")\n  alive(\"b\")\n;;\nprint(f())",
            "true\n",
        ),
        // on a path that never reads it again it goes, though another
        // branch reads it
        (
            "fn f(flag):\n  t = token(\"c\")\n  if flag: _ = keep(t) ;;\n  alive(\"c\")\n;;\nprint(f(true), f(false))",
            "false false\n",
        ),
        // a branch lets go, as it starts, of what only another reads
        (
            "fn f(flag):\n  t = token(\"d\")\n  flag then keep(t) else alive(\"d\")\n;;\nprint(f(false))",
            "false\n",
        ),
        // sibling blocks reuse their slots: nothing of the first lives on
        (
            "fn f():\n  if true:\n    t = token(\"e\")\n    _ = keep(t)\n  ;;\n  if true:\n    u = token(\"f\")\n    print(alive(\"e\"), alive(\"f\"))\n    _ = keep(u)\n  ;;\n  nil\n;;\nf()",
            "false true\n",
        ),
        // the entry file's blocks follow the same rule
        (
            "if true:\n  t = token(\"g\")\n  _ = keep(t)\n  print(alive(\"g\"))\n;;",
            "false\n",
        ),
        // a capture that is the last read: the function value goes with it
        (
            "fn f():\n  t = token(\"h\")\n  _ = keep(() => t)\n  alive(\"h\")\n;;\nprint(f())",
            "false\n",
        ),
        // `??`'s paths: the one not taken lets go of what only it reads
        (
            "fn f(m):\n  t = token(\"i\")\n  v = m ?? keep(t)\n  [v, alive(\"i\")]\n;;\nprint(f(5))",
            "[5, false]\n",
        ),
        // a parameter read last by a later parameter's default goes as
        // the body starts (VM39)
        (
            "fn f(x = token(\"j\"), _ok = keep(x)) = alive(\"j\")\nprint(f())",
            "false\n",
        ),
        // a slice holds its own items only (VA59)
        (
            "fn f():\n  xs = map(0..100, i => i == 99 then token(\"k\") else i)\n  ys = xs[..10]\n  [alive(\"k\"), len(ys)]\n;;\nprint(f())",
            "[false, 10]\n",
        ),
    ] {
        assert_eq!(out(src), want, "{src}");
    }
}

/// A local never read is held until its function returns, whatever comes
/// after it in the function (VM55): a later block's names do not take
/// its slot, and a failed arm's pattern does not clear it.
#[test]
fn a_held_local_outlives_the_blocks_after_it() {
    for (src, want) in [
        // a later sibling block binds a name
        (
            "fn f(flag):\n  if flag:\n    _t = token(\"a\")\n  ;;\n  if flag:\n    n = alive(\"a\")\n    print(n)\n  ;;\n  alive(\"a\")\n;;\nprint(f(true))",
            "true\ntrue\n",
        ),
        // an arm that fails wrote its pattern's slot
        (
            "fn f(flag, m):\n  if flag: _t = token(\"b\") ;;\n  match m:\n    {port: p} => print(p)\n    _ => print(\"none\")\n  ;;\n  alive(\"b\")\n;;\nprint(f(true, {}))",
            "none\ntrue\n",
        ),
        // held inside a nested block, a sibling of the outer one binds
        (
            "fn f(flag):\n  if flag:\n    if flag: _t = token(\"c\") ;;\n  ;;\n  if flag:\n    (x, y) = (1, 2)\n    print(x + y)\n  ;;\n  alive(\"c\")\n;;\nprint(f(true))",
            "3\ntrue\n",
        ),
        // the file's blocks are the same
        (
            "fn id(v) = v\nif id(true): _t = token(\"d\") ;;\nif id(true):\n  n = id(1)\n  print(n)\n;;\nprint(alive(\"d\"))",
            "1\ntrue\n",
        ),
        // and it goes when the function returns
        (
            "fn f(flag):\n  if flag: _t = token(\"e\") ;;\n  alive(\"e\")\n;;\nprint(f(true), alive(\"e\"))",
            "true false\n",
        ),
    ] {
        assert_eq!(out(src), want, "{src}");
    }
}

/// A tail call hands the frame over, and the values the caller only
/// holds go with it: they are let go when that frame returns, whether
/// the callee is a fung function or a native (D162).
#[test]
fn a_held_local_outlives_a_tail_call() {
    for (src, want) in [
        // a fung function in the tail place
        (
            "fn g() = alive(\"a\")\nfn f():\n  _t = token(\"a\")\n  g()\n;;\nprint(f(), alive(\"a\"))",
            "true false\n",
        ),
        // a fung prelude function and a native one, the same
        (
            "fn w(x) = alive(\"b\") then x else -1\nfn f():\n  _t = token(\"b\")\n  iterate(1, w, 1)\n;;\nprint(f(), alive(\"b\"))",
            "1 false\n",
        ),
        (
            "fn w(x) = alive(\"c\") then x else -1\nfn f():\n  _t = token(\"c\")\n  map([1], w)\n;;\nprint(f(), alive(\"c\"))",
            "[1] false\n",
        ),
        // a chain of tail calls: each caller's holds live to the end
        (
            "fn h() = (alive(\"d\"), alive(\"e\"))\nfn g():\n  _u = token(\"e\")\n  h()\n;;\nfn f():\n  _t = token(\"d\")\n  g()\n;;\nprint(f(), alive(\"d\"), alive(\"e\"))",
            "(true, true) false false\n",
        ),
        // a tail call through `and`, `??` and a branch
        (
            "fn g() = alive(\"f\")\nfn f(c):\n  _t = token(\"f\")\n  c and g()\n;;\nprint(f(true), alive(\"f\"))",
            "true false\n",
        ),
        (
            "fn g() = alive(\"g\")\nfn f(c):\n  _t = token(\"g\")\n  if c: g() else: false ;;\n;;\nprint(f(true), alive(\"g\"))",
            "true false\n",
        ),
        // a local that is read is not held: it goes at its last read
        (
            "fn g() = alive(\"h\")\nfn f():\n  t = token(\"h\")\n  _ = keep(t)\n  g()\n;;\nprint(f())",
            "false\n",
        ),
        // a parameter never read is held like any name (D172)
        (
            "fn g() = alive(\"i\")\nfn f(_t) = g()\nprint(f(token(\"i\")), alive(\"i\"))",
            "true false\n",
        ),
        // so is a pattern's name, and each turn of a loop holds its own
        (
            "fn go(n) = match (n, token(\"j${n}\")):\n  (0, _t) => (alive(\"j2\"), alive(\"j1\"), alive(\"j0\"))\n  (_, _t) => go(n - 1)\n;;\nprint(go(2), alive(\"j2\"), alive(\"j0\"))",
            "(true, true, true) false false\n",
        ),
        // `_` binds nothing: its value goes with the frame's slots
        (
            "fn g() = alive(\"k\")\nfn f(_) = g()\nprint(f(token(\"k\")))",
            "false\n",
        ),
        // a function with nothing held still runs in constant stack
        (
            "fn count(n) = n == 0 then \"done\" else count(n - 1)\nprint(count(200000))",
            "done\n",
        ),
    ] {
        assert_eq!(out(src), want, "{src}");
    }
}
