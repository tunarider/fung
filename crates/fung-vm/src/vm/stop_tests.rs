//! Stop requests (concurrency v2, L6): `stop_signal()`, `request_stop()`
//! and a signal delivered from outside the program.

use crate::native::nat;
use crate::testutil::{Opts, UNTIL, exec, probes, run};
use crate::value::{Native, Value};
use crate::vm::{StopHandle, Task};

/// `probes` plus natives that deliver a signal from another thread,
/// as the command's signal thread does: `deliver_when_watched(sig)` once
/// a task waits on the stop signal, `deliver_soon(sig)` at the next
/// moment the run thread is free; and two that post the event at once,
/// from the run thread itself: `post_stop(sig)` (a signal's stop
/// request) and `post_interrupt()` (the REPL's Ctrl-C). They have no
/// effect the atom check knows of, so a swap's function may call them.
fn signal_probes() -> Vec<Value> {
    let mut v = probes();
    v.push(Value::Native(Native::leak(
        "post_stop",
        vec![nat::param("sig")],
        |t: &mut Task, a| {
            let Value::Int(sig) = a[0] else {
                panic!("an int")
            };
            StopHandle::of(t).deliver(sig as i32);
            Ok(Value::Nil)
        },
    )));
    v.push(Value::Native(Native::leak(
        "post_interrupt",
        vec![],
        |t: &mut Task, _| {
            StopHandle::of(t).interrupt();
            Ok(Value::Nil)
        },
    )));
    v.push(Value::Native(Native::leak(
        "deliver_when_watched",
        vec![nat::param("sig")],
        |t: &mut Task, a| {
            let Value::Int(sig) = a[0] else {
                panic!("an int")
            };
            let h = StopHandle::of(t);
            std::thread::spawn(move || h.deliver_when_watched(sig as i32));
            Ok(Value::Nil)
        },
    )));
    v.push(Value::Native(Native::leak(
        "deliver_soon",
        vec![nat::param("sig")],
        |t: &mut Task, a| {
            let Value::Int(sig) = a[0] else {
                panic!("an int")
            };
            let h = StopHandle::of(t);
            std::thread::spawn(move || h.deliver(sig as i32));
            Ok(Value::Nil)
        },
    )));
    v
}

fn exec_signals(src: &str) -> (String, String, i32) {
    let o = exec(
        &format!("{UNTIL}{src}"),
        Opts {
            extra: signal_probes,
            ..Opts::default()
        },
    );
    (o.out, o.err, o.exit)
}

#[test]
fn a_watcher_sees_the_stop_request() {
    assert_eq!(
        exec_signals(
            "scope(fn(s):\n  stop = stop_signal()\n  t = spawn(s, () => recv(stop))\n  blocked(stop, 1)\n  request_stop()\n  print(await(t))\n;;)\nprint(\"after\")"
        ),
        (
            "Err({kind: 'closed, message: \"channel closed\"})\nafter\n".into(),
            "".into(),
            0
        )
    );
}

/// Nothing in the program can wake it, but a signal can: not a deadlock.
#[test]
fn waiting_only_for_the_stop_is_not_a_deadlock() {
    assert_eq!(
        exec_signals("deliver_when_watched(15)\nprint(recv(stop_signal()))\nprint(\"after\")"),
        (
            "Err({kind: 'closed, message: \"channel closed\"})\nafter\n".into(),
            "".into(),
            0
        )
    );
}

#[test]
fn without_a_watcher_a_stop_cancels_the_program() {
    assert_eq!(
        run("with(() => nil, _ => request_stop(), _ => print(\"released\"))\nprint(\"not here\")"),
        ("released\n".into(), "".into(), 0)
    );
    assert_eq!(
        exec_signals(
            "with(() => nil, fn(_): deliver_soon(2); sleep(10s) ;;, _ => print(\"released\"))\nprint(\"not here\")"
        ),
        ("released\n".into(), "".into(), 130)
    );
    assert_eq!(exec_signals("deliver_soon(15)\nsleep(10s)").2, 143);
}

/// A watcher that is gone does not count.
#[test]
fn a_dropped_watcher_is_no_watcher() {
    assert_eq!(
        run(
            "fn f() = stop_signal()\nf()\nwith(() => nil, _ => request_stop(), _ => print(\"released\"))\nprint(\"not here\")"
        ),
        ("released\n".into(), "".into(), 0)
    );
}

#[test]
fn after_a_stop_the_signal_is_closed() {
    assert_eq!(
        run("stop = stop_signal()\nrequest_stop()\nprint(recv(stop), recv(stop_signal()))"),
        (
            "Err({kind: 'closed, message: \"channel closed\"}) Err({kind: 'closed, message: \"channel closed\"})\n".into(),
            "".into(),
            0
        )
    );
}

#[test]
fn every_watcher_wakes() {
    assert_eq!(
        exec_signals(
            "scope(fn(s):\n  ts = map(0..3, _ => spawn(s, () => recv(stop_signal())))\n  blocked(stop_signal(), 3)\n  request_stop()\n  print(map(ts, await))\n;;)"
        ),
        (
            "[Err({kind: 'closed, message: \"channel closed\"}), Err({kind: 'closed, message: \"channel closed\"}), Err({kind: 'closed, message: \"channel closed\"})]\n".into(),
            "".into(),
            0
        )
    );
}

#[test]
fn request_stop_is_an_effect() {
    let (_, err, code) = run("swap(atom(1), fn(x): request_stop(); x ;;)");
    assert!(
        err.contains("error[E0707]: `request_stop` has an effect"),
        "{err}"
    );
    assert_eq!(code, 1);
}

#[test]
fn stop_signal_gives_a_receiver() {
    assert_eq!(
        run("print(type_of(stop_signal()), stop_signal() == stop_signal())"),
        ("'receiver true\n".into(), "".into(), 0)
    );
}

/// A swap's function runs to its end without giving its turn (so that it
/// is not retried forever), and so it used to keep the scheduler from
/// the events: a function that never ended kept a signal from stopping
/// the program (review C6). The events are made inside it now, at the
/// end of each turn's worth of calls. `spin` is long but ends, so the
/// test ends either way.
const SPIN: &str = "fn spin(n) = n == 0 then nil else spin(n - 1)\na = atom(0)\n";

#[test]
fn a_stop_reaches_a_function_running_in_swap() {
    assert_eq!(
        exec_signals(&format!(
            "{SPIN}swap(a, fn(x): post_stop(15); spin(200000); x ;;)\nprint(\"not here\")"
        )),
        ("".into(), "".into(), 143)
    );
}

/// The REPL's Ctrl-C cancels the input even while it runs a swap's
/// function.
#[test]
fn an_interrupt_reaches_a_function_running_in_swap() {
    let src = format!(
        "{UNTIL}{SPIN}swap(a, fn(x): post_interrupt(); spin(200000); x ;;)\nprint(\"not here\")"
    );
    let s = crate::testutil::setup(
        &src,
        Opts {
            extra: signal_probes,
            ..Opts::default()
        },
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    let r = s.rt.run();
    assert!(r.is_err(), "{r:?}");
    assert_eq!(s.out.text(), "");
}

/// L3: `wait_closed(stop)` waits for the stop request as a statement (no
/// discarded Err), and like `recv` it is no deadlock alone.
#[test]
fn wait_closed_waits_for_the_stop() {
    assert_eq!(
        exec_signals(
            "scope(fn(s):\n  stop = stop_signal()\n  t = spawn(s, fn():\n    wait_closed(stop)\n    \"stopped\"\n  ;;)\n  blocked(stop, 1)\n  request_stop()\n  print(await(t), try_recv(stop))\n;;)"
        ),
        (
            "stopped Err({kind: 'closed, message: \"channel closed\"})\n".into(),
            "".into(),
            0
        )
    );
    assert_eq!(
        exec_signals("deliver_when_watched(15)\nwait_closed(stop_signal())\nprint(\"after\")"),
        ("after\n".into(), "".into(), 0)
    );
}

/// L14: two Ctrl-Cs in one batch of events are not a second Ctrl-C after
/// the first: the cancellation they make runs, and the release with it.
/// Only one after the scheduler has gone round since drops the tasks.
#[test]
fn two_interrupts_in_one_batch_still_run_the_release() {
    let (out, _, _) = exec_signals(
        "with(() => nil, fn(_):\n  post_interrupt()\n  post_interrupt()\n  sleep(1s)\n;;, _ => print(\"released\"))",
    );
    assert_eq!(out, "released\n");
}
