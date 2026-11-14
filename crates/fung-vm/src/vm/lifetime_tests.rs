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
