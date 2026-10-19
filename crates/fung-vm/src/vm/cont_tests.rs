//! Continuation frames (concurrency v2 R2): natives that call back into
//! fung continue in the task's one run loop instead of nesting another.

use super::cont::{Cont, Flow, Next};
use super::{Stop, Task};
use crate::native::nat;
use crate::testutil::{Opts, exec, probes};
use crate::value::{Native, Value};

/// `twice(f)`: `f(f(1))`.
struct Twice {
    f: Value,
    calls: u8,
}

impl Cont for Twice {
    fn resume(&mut self, t: &mut Task<'_>, v: Value) -> Result<Next, Stop> {
        self.calls += 1;
        if self.calls == 2 {
            return Ok(Next::Done(v));
        }
        t.push_arg(v);
        Ok(Next::Call(self.f.clone(), 1))
    }
}

fn twice(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let f = a[0].clone();
    t.push_arg(Value::Int(1));
    Ok(Flow::Call(
        Box::new(Twice {
            f: f.clone(),
            calls: 0,
        }),
        f,
        1,
    ))
}

/// `call_with(f, x)`: `f(x)`.
struct Once;

impl Cont for Once {
    fn resume(&mut self, _: &mut Task<'_>, v: Value) -> Result<Next, Stop> {
        Ok(Next::Done(v))
    }
}

fn call_with(t: &mut Task<'_>, mut a: Vec<Value>) -> Result<Flow, Stop> {
    let x = a.pop().expect("x");
    let f = a.pop().expect("f");
    t.push_arg(x);
    Ok(Flow::Call(Box::new(Once), f, 1))
}

/// `probe_after(f)`: `f()`, printing `unwound` when a stop unwinds
/// through it.
struct ProbeAfter;

impl Cont for ProbeAfter {
    fn resume(&mut self, _: &mut Task<'_>, v: Value) -> Result<Next, Stop> {
        Ok(Next::Done(v))
    }

    fn unwind(&mut self, t: &mut Task<'_>, stop: Stop) -> Result<Next, Stop> {
        let _ = writeln!(t.stdout(), "unwound");
        Err(stop)
    }
}

fn probe_after(_: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    Ok(Flow::Call(Box::new(ProbeAfter), a[0].clone(), 0))
}

fn natives() -> Vec<Value> {
    let mut v = probes();
    v.push(Value::Native(Native::leak_step(
        "twice",
        vec![nat::param("f")],
        twice,
    )));
    v.push(Value::Native(Native::leak_step(
        "call_with",
        vec![nat::param("f"), nat::param("x")],
        call_with,
    )));
    v.push(Value::Native(Native::leak_step(
        "probe_after",
        vec![nat::param("f")],
        probe_after,
    )));
    v.push(Value::Native(Native::leak(
        "panic_probe",
        vec![],
        |_: &mut Task, _| panic!("probe panicked"),
    )));
    v.push(Value::Native(Native::leak(
        "stack_mark",
        vec![nat::param("x")],
        |_: &mut Task, _| {
            let here = 0u8;
            Ok(Value::Int(std::ptr::addr_of!(here) as i64))
        },
    )));
    v.push(Value::Native(Native::leak(
        "nested_root",
        vec![nat::param("f")],
        |t: &mut Task, a| t.call_root(&a[0], Vec::new()),
    )));
    v
}

fn run(src: &str) -> crate::testutil::Outcome {
    exec(
        src,
        Opts {
            extra: natives,
            ..Opts::default()
        },
    )
}

#[test]
fn a_step_native_calls_back_in_the_same_loop() {
    let o = run("print(twice(x => x + loop_depth()))");
    assert_eq!((o.out.as_str(), o.err.as_str()), ("3\n", ""));
}

#[test]
fn step_natives_nest_at_the_same_depth() {
    let o = run("print(call_with(twice, x => x + loop_depth()))");
    assert_eq!((o.out.as_str(), o.err.as_str()), ("3\n", ""));
}

#[test]
fn a_plain_native_as_the_callback() {
    let o = run("print(twice(str))");
    assert_eq!((o.out.as_str(), o.err.as_str()), ("1\n", ""));
}

#[test]
fn a_step_native_in_tail_position_returns_to_the_caller() {
    let o = run("fn g(n) = twice(x => x + n)\nprint(g(1) + g(2))");
    assert_eq!((o.out.as_str(), o.err.as_str()), ("8\n", ""));
    // A failure below a native in tail position, after 1,000 tail calls:
    // the native keeps its caller's frame (natives never reuse it), so
    // the path shows `loop` with the elided calls, as for any native.
    let o = run(
        "fn h(x) = fail(\"x\")\nfn loop(n) = n == 0 then twice(h) else loop(n - 1)\nloop(1000)",
    );
    let e = o.error.expect("an error");
    let path: Vec<_> = e
        .path
        .iter()
        .map(|c| (c.func.as_str(), c.line, c.elided))
        .collect();
    assert_eq!(path, [("h", 1, 0), ("loop", 2, 1000), ("<main>", 3, 0)]);
}

#[test]
fn errors_unwind_through_a_step_native() {
    let o = run("probe_after(() => 1 / 0)");
    assert_eq!(o.out, "unwound\n");
    let e = o.error.expect("an error");
    assert_eq!((e.code.as_str(), e.line, e.col), ("E0502", 1, 21));
    assert_eq!(o.exit, 1);
}

#[test]
fn exit_unwinds_through_a_step_native() {
    let o = run("probe_after(() => exit(4))");
    assert_eq!(o.out, "unwound\n");
    assert_eq!(o.exit, 4);
}

#[test]
fn a_panic_inside_a_callback_is_e0801() {
    let o = run("probe_after(() => twice(x => panic_probe()))");
    assert_eq!(o.out, "unwound\n");
    let e = o.error.expect("an error");
    assert_eq!(e.code, "E0801");
    assert!(o.err.contains("probe panicked"), "{}", o.err);
}

#[test]
fn call_root_refuses_a_nested_loop() {
    let o = run("twice(x => nested_root(() => x))");
    assert!(o.err.contains("nested run loop"), "{}", o.err);
}

#[test]
fn iterating_natives_call_back_in_one_loop() {
    let cases = [
        ("map([1, 2], x => loop_depth())", "[1, 1]"),
        ("filter([1, 2], x => loop_depth() == 1)", "[1, 2]"),
        ("fold(0..3, 0, (a, x) => a + loop_depth())", "3"),
        ("each([1], x => print(loop_depth()))", "nil"),
        ("find([1, 2], x => loop_depth() == 1 and x == 2)", "2"),
        ("any([1], x => loop_depth() == 1)", "true"),
        ("all([1, 2], x => loop_depth() == 1)", "true"),
        ("group_by([1], x => str(loop_depth()))", "{\"1\": [1]}"),
        ("sort_by([2, 1], x => x * loop_depth())", "[1, 2]"),
    ];
    for (src, want) in cases {
        let o = run(&format!("print({src})"));
        let out = o.out.lines().last().unwrap_or("").to_string();
        assert_eq!((out.as_str(), o.err.as_str()), (want, ""), "{src}");
    }
}

/// The reports the release build of the branch's starting point gives
/// (`fung -e`), byte for byte.
#[test]
fn callback_errors_report_as_before() {
    let cases = [
        (
            "print(map([1, 0], x => 10 / x))",
            "error[E0502]: division by zero\n  --> -e:1:27\n  |\n1 | print(map([1, 0], x => 10 / x))\n  |                           ^\n  |\n call path (innermost first):\n   -e:1       <lambda>\n   -e:1       <entry>\n",
        ),
        (
            "print(filter([1], x => 1))",
            "error[E0505]: expected bool, got int\n  --> -e:1:13\n  |\n1 | print(filter([1], x => 1))\n  |             ^\n  |\n call path (innermost first):\n   -e:1       <entry>\n",
        ),
        (
            "print(fold([1], 0, (a, x) => a.nope))",
            "error[E0509]: cannot index a value of type int\n  --> -e:1:31\n  |\n1 | print(fold([1], 0, (a, x) => a.nope))\n  |                               ^\n  |\n call path (innermost first):\n   -e:1       <lambda>\n   -e:1       <entry>\n",
        ),
        (
            "print(sort_by([2, 1], x => x / 0))",
            "error[E0502]: division by zero\n  --> -e:1:30\n  |\n1 | print(sort_by([2, 1], x => x / 0))\n  |                              ^\n  |\n call path (innermost first):\n   -e:1       <lambda>\n   -e:1       <entry>\n",
        ),
        (
            "print(group_by([1], x => 1))",
            "error[E0514]: cannot use a value of type int as a map key\n  --> -e:1:15\n  |\n1 | print(group_by([1], x => 1))\n  |               ^\n  |\n call path (innermost first):\n   -e:1       <entry>\n",
        ),
    ];
    for (src, want) in cases {
        let o = run(src);
        assert_eq!(o.err, want, "{src}");
    }
}

#[test]
fn iterating_a_receiver_still_works() {
    let o = run(
        "(tx, rx) = channel()\nprint(scope(fn(s):\n  spawn(s, fn(): each(0..3, i => send(tx, i)); close(tx) ;;)\n  d = loop_depth()\n  fold(rx, 0, (a, x) => a + x + loop_depth() - d)\n;;))",
    );
    assert_eq!((o.out.as_str(), o.err.as_str()), ("3\n", ""));
}

/// A plain native called back per element (`map(xs, str)`) is called
/// from one Rust frame: the stack does not grow per element. (The spread
/// is not zero: once a turn is due the callback goes through the run
/// loop instead of in place, a fixed difference; four times the elements
/// must not widen it.)
#[test]
fn a_plain_native_callback_runs_in_constant_rust_stack() {
    let spread = |n: usize| -> i64 {
        let o = run(&format!(
            "ms = map(0..{n}, stack_mark)\nhi = fold(ms, ms[0], (a, x) => a > x then a else x)\nlo = fold(ms, ms[0], (a, x) => a < x then a else x)\nprint(hi - lo)"
        ));
        o.out.trim().parse().expect(&o.err)
    };
    let (small, big) = (spread(2000), spread(8000));
    assert!(big <= small + 1024, "{small} then {big} bytes");
    assert!(big < 64 * 1024, "{big} bytes");
}

#[test]
fn cleanup_natives_call_back_in_one_loop() {
    let cases = [
        (
            "with(() => loop_depth(), r => r + loop_depth(), r => print(loop_depth()))",
            "1\n2\n",
        ),
        ("timeout(1s, () => loop_depth())", "1\n"),
        ("swap(atom(0), x => x + loop_depth())", "1\n"),
    ];
    for (src, want) in cases {
        let o = run(&format!("print({src})"));
        assert_eq!((o.out.as_str(), o.err.as_str()), (want, ""), "{src}");
    }
}

#[test]
fn a_scope_body_runs_in_the_same_loop() {
    for src in [
        "print(scope(s => loop_depth()))",
        "print(supervise(s => loop_depth(), e => nil))",
    ] {
        let o = run(src);
        assert_eq!((o.out.as_str(), o.err.as_str()), ("1\n", ""), "{src}");
    }
}

#[test]
fn a_scope_as_a_direct_callback() {
    let o = run("print(map([s => 1, s => 2], scope))");
    assert_eq!((o.out.as_str(), o.err.as_str()), ("[1, 2]\n", ""));
}

#[test]
fn a_module_body_runs_in_the_importers_loop() {
    let o = crate::testutil::run_files_with(
        &[
            ("/p/lib.fung", "depth = loop_depth()\n"),
            ("/p/main.fung", "import \"./lib\"\nprint(lib.depth)"),
        ],
        "/p/main.fung",
        natives,
    );
    assert_eq!((o.out.as_str(), o.err.as_str()), ("1\n", ""));
}

/// No native nests a run loop: `call_root` opens one only where no fung
/// code runs yet on the task, and nothing else calls back the old way.
#[test]
fn no_native_nests_the_run_loop() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut stack = vec![src.clone()];
    let mut roots = Vec::new();
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            if name.ends_with("_tests.rs") || name == "tests.rs" || name == "testutil.rs" {
                continue;
            }
            let text = std::fs::read_to_string(&p).unwrap();
            let rel = p.strip_prefix(&src).unwrap().to_string_lossy().into_owned();
            for bad in [".call(&", "call_buf(", "run_task(", ".run(depth"] {
                assert!(!text.contains(bad), "{rel} has `{bad}`");
            }
            if text.contains(".call_root(") || text.contains(".call_root_with(") {
                roots.push(rel);
            }
        }
    }
    roots.sort();
    assert_eq!(roots, ["vm/m6.rs"]);
}

/// A plain native called back in place (`native/walk.rs`) reports and
/// exits as the release build of the branch's starting point does.
#[test]
fn plain_native_callbacks_report_as_before() {
    let cases = [
        (
            "print(map([\"x\"], fail))",
            "error[E0803]: x\n  --> -e:1:10\n  |\n1 | print(map([\"x\"], fail))\n  |          ^\n  |\n call path (innermost first):\n   -e:1       <entry>\n",
            1,
        ),
        (
            "print(swap(atom(0), x => len(map([1], print))))",
            "error[E0707]: `print` has an effect, which a swap's function must not have\n  --> -e:1:33\n  |\n1 | print(swap(atom(0), x => len(map([1], print))))\n  |                                 ^\n  |\n call path (innermost first):\n   -e:1       <lambda>\n   -e:1       <entry>\n",
            1,
        ),
        ("print(map([1, 2], exit))", "", 1),
        (
            "print(each([\"a\", 1], len))",
            "error[E0506]: cannot take the length of int\n  --> -e:1:11\n  |\n1 | print(each([\"a\", 1], len))\n  |           ^\n  |\n call path (innermost first):\n   -e:1       <entry>\n",
            1,
        ),
    ];
    for (src, want, exit) in cases {
        let o = run(src);
        assert_eq!(
            (o.out.as_str(), o.err.as_str(), o.exit),
            ("", want, exit),
            "{src}"
        );
    }
}

/// A step native as the callback of another (`map(fs, twice)`) whose own
/// calls end at once (a plain native): the next element is still called
/// from the same Rust frame, not from inside the finished callback's.
#[test]
fn a_step_native_callback_runs_in_constant_rust_stack() {
    let o = run(
        "fs = map(0..2000, _ => stack_mark)\nms = map(fs, twice)\nhi = fold(ms, ms[0], (a, x) => a > x then a else x)\nlo = fold(ms, ms[0], (a, x) => a < x then a else x)\nprint(hi - lo)",
    );
    let spread: i64 = o.out.trim().parse().expect(&o.err);
    assert!(spread < 16 * 1024, "{spread} bytes");
}
