//! Background tasks and the helpers agents are built from (stage 3b,
//! Task 3). The programs import `_native.conc` directly, which user code
//! cannot do (E0310): `Opts::internal` compiles them as an embedded file
//! would be. Concurrent orderings use the probes of `testutil` (state,
//! not sleeps).

use crate::testutil::{Opts, Outcome, UNTIL, exec, probes};

/// Runs `src` as internal code (it may import `_native.*`), with the
/// probes and `until` / `blocked` in front of it.
fn exec_internal(src: &str) -> Outcome {
    exec(
        &format!("{UNTIL}{src}"),
        Opts {
            internal: true,
            extra: probes,
            ..Opts::default()
        },
    )
}

/// `exec_internal` without the helpers, for the brief's programs as they
/// are written.
fn exec_plain(src: &str) -> Outcome {
    exec(
        src,
        Opts {
            internal: true,
            ..Opts::default()
        },
    )
}

#[test]
fn a_background_task_does_not_keep_the_program_alive() {
    // the background task waits forever on a channel nobody sends on
    let o = exec_plain(
        "import _native.conc as c\nch = channel()\nc.spawn_background(() => recv(ch))\nprint(\"done\")",
    );
    assert_eq!((o.out.as_str(), o.err.as_str(), o.exit), ("done\n", "", 0));
}

#[test]
fn a_blocked_normal_task_is_still_a_deadlock_beside_a_background_one() {
    let o = exec_plain(
        "import _native.conc as c\nc.spawn_background(() => recv(channel()))\nrecv(channel())",
    );
    assert!(
        o.err
            .starts_with("error[E0702]: deadlock: all tasks are waiting"),
        "{}",
        o.err
    );
    assert_eq!(o.exit, 1);
}

#[test]
fn background_tasks_outlive_exit_handlers() {
    let o = exec_plain(
        "import _native.conc as c\nch = channel()\nc.spawn_background(fn(): x = recv(ch)!; print(\"bg got\", x) ;;)\nat_exit(() => send(ch, 7))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("bg got 7\n", "", 0)
    );
}

#[test]
fn dropping_the_last_agent_handle_closes_its_channel() {
    let o = exec_plain(
        "import _native.conc as c\nch = channel()\nt = c.spawn_background(() => recv(ch))\nmk = () => c.agent_handle(ch)\nmk()\nprint(await(t))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str()),
        ("Err({kind: 'closed})\n", "")
    );
}

#[test]
fn user_code_still_cannot_import_native_modules() {
    let o = exec("import _native.conc as c\nprint(1)", Opts::default());
    assert!(o.err.starts_with("error[E0310]"), "{}", o.err);
}

#[test]
fn an_agent_handle_prints_types_and_compares_by_identity() {
    let o = exec_plain(
        "import _native.conc as c\nch = channel()\na = c.agent_handle(ch)\nb = c.agent_handle(ch)\nprint(a, [a], \"${a}\", type_of(a), a == a, a == b, a != b, c.agent_channel(a) == ch)",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str()),
        (
            "<agent> [<agent>] <agent> 'agent true false true true\n",
            ""
        )
    );
}

#[test]
fn a_handle_closes_its_channel_only_with_its_last_copy() {
    let o = exec_internal(
        "import _native.conc as c\nch = channel()\nt = c.spawn_background(() => recv(ch))\nmk = () => c.agent_handle(ch)\nxs = [mk()]\nys = xs + [1]\nprint(await(t, timeout: 20ms))\nxs = nil\nprint(await(t, timeout: 20ms))\nys = nil\nprint(await(t))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str()),
        (
            "Err({kind: 'timeout})\nErr({kind: 'timeout})\nErr({kind: 'closed})\n",
            ""
        )
    );
}

#[test]
fn an_agent_whose_handles_are_gone_ends_while_the_program_runs() {
    let o = exec_internal(
        "import _native.conc as c\nfn start() = fn(): ch = channel(); c.spawn_background(() => recv(ch)); c.agent_handle(ch) ;;()\nstart()\nstart()\nuntil(() => background() == 0)\nprint(\"gone\", background())",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("gone 0\n", "", 0)
    );
}

#[test]
fn a_background_failure_is_never_reported() {
    let o = exec_internal(
        "import _native.conc as c\nc.spawn_background(() => 1 / 0)\nuntil(() => background() == 0)\nprint(\"ok\")",
    );
    assert_eq!((o.out.as_str(), o.err.as_str(), o.exit), ("ok\n", "", 0));
}

#[test]
fn a_background_task_spawned_by_a_handler_is_cancelled_after_the_handlers() {
    let o = exec_internal(
        "import _native.conc as c\nat_exit(fn(): c.spawn_background(() => recv(channel())); print(\"handler\") ;;)\nprint(\"main\")",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("main\nhandler\n", "", 0)
    );
}

#[test]
fn background_tasks_are_cancelled_after_every_handler() {
    // Handlers run in reverse: the second registered runs first and the
    // background task still serves both.
    let o = exec_internal(
        "import _native.conc as c\nfn serve(req, n) = match recv(req):\n  Ok(r) => fn(): send(r, n); serve(req, n + 1) ;;()\n  _ => nil\n;;\nfn ask(req) = fn(): r = channel(); send(req, r); recv(r)! ;;()\nreq = channel()\nc.spawn_background(() => serve(req, 1))\nat_exit(() => print(\"first\", ask(req)))\nat_exit(() => print(\"second\", ask(req)))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("second 1\nfirst 2\n", "", 0)
    );
}

/// The census window: a normal task waits for a reply that only a
/// background task, running and not blocked, will send. That is not a
/// deadlock. The background task spins (no timer, never blocked) until
/// the entry is queued on the reply channel, so the entry counts itself
/// blocked while the background task is runnable.
#[test]
fn waiting_on_a_running_background_task_is_not_a_deadlock() {
    let o = exec_internal(
        "import _native.conc as c\nfn spin(ch) = queued(ch) >= 1 then nil else spin(ch)\nreq = channel()\nreply = channel()\nc.spawn_background(fn(): recv(req); spin(reply); send(reply, 42) ;;)\nsend(req, 1)\nprint(recv(reply))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("Ok(42)\n", "", 0)
    );
}

/// A background task that ends can leave every normal task blocked with
/// nothing to wake them: its end is judged.
#[test]
fn a_background_task_ending_can_complete_a_deadlock() {
    let o = exec_internal(
        "import _native.conc as c\nfn spin(ch) = queued(ch) >= 1 then nil else spin(ch)\nreply = channel()\nc.spawn_background(() => spin(reply))\nrecv(reply)\nprint(\"not reached\")",
    );
    assert!(
        o.err
            .starts_with("error[E0702]: deadlock: all tasks are waiting"),
        "{}",
        o.err
    );
    assert_eq!((o.out.as_str(), o.exit), ("", 1));
}

/// A background task blocking can complete a deadlock too: the entry is
/// blocked first, then the background task blocks on a channel nobody
/// sends on.
#[test]
fn a_background_task_blocking_can_complete_a_deadlock() {
    let o = exec_internal(
        "import _native.conc as c\nfn spin(ch) = queued(ch) >= 1 then nil else spin(ch)\nreply = channel()\nc.spawn_background(fn(): spin(reply); recv(channel()) ;;)\nrecv(reply)",
    );
    assert!(
        o.err
            .starts_with("error[E0702]: deadlock: all tasks are waiting"),
        "{}",
        o.err
    );
    assert_eq!(o.exit, 1);
}

/// A sleeping background task will run again: no deadlock is judged
/// while its timer is pending.
#[test]
fn a_sleeping_background_task_is_not_part_of_a_deadlock() {
    let o = exec_internal(
        "import _native.conc as c\nreply = channel()\nc.spawn_background(fn(): blocked(reply, 1); sleep(5ms); send(reply, 'late) ;;)\nprint(recv(reply))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("Ok('late)\n", "", 0)
    );
}

#[test]
fn try_call_returns_ok_or_the_caught_error() {
    let o = exec_plain(
        "import _native.conc as c\nfn inner(r) = match r:\n  Err(e) => type_of(e)\n  _ => nil\n;;\nprint(c.try_call(x => x + 1, 1))\nr = c.try_call(x => x / 0, 1)\nprint(r, type_of(r), inner(r))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("Ok(2)\nErr(<error>) 'Err 'error\n", "", 0)
    );
}

#[test]
fn raise_reports_the_caught_error_at_the_callers_position() {
    let o = exec_plain(
        "import _native.conc as c\nfn bad(x) = x / 0\nr = c.try_call(bad, 1)\nmatch r:\n  Err(e) => c.raise(e)\n  _ => nil\n;;",
    );
    // The division's own line (2) and the call path through `bad` are
    // gone: the error is at the `raise(` call, line 5.
    assert_eq!(o.exit, 1, "{}", o.err);
    let e = o.error.expect("an error");
    assert_eq!((e.code.as_str(), e.line), ("E0502", 5), "{}", o.err);
    assert!(
        e.path.iter().all(|s| s.func != "bad"),
        "the old call path leaked: {:?}",
        e.path
    );
    assert!(
        o.err.starts_with("error[E0502]: division by zero"),
        "{}",
        o.err
    );
}

/// The Task 4 shape: an embedded (builtin) function catches the error in
/// one task and raises it in another; it lands on the user's call.
#[test]
fn raise_inside_a_builtin_function_lands_on_the_users_call() {
    fn mark_relay(p: &mut fung_front::bytecode::Proto) {
        if p.name == "relay" {
            p.builtin = true;
        }
        for c in &mut p.protos {
            mark_relay(c);
        }
    }
    let src = "import _native.conc as c\nfn serve(req) = match recv(req):\n  Ok((f, r)) => send(r, c.try_call(f, 0))\n  _ => nil\n;;\nfn relay(req, f):\n  r = channel()\n  send(req, (f, r))\n  match recv(r)!:\n    Ok(v) => v\n    Err(e) => c.raise(e)\n  ;;\n;;\nreq = channel()\nc.spawn_background(() => serve(req))\nprint(\"before\")\nx = relay(req, n => fail(\"boom ${n}\"))\nprint(\"not reached\", x)";
    let o = exec(
        src,
        Opts {
            internal: true,
            patch: mark_relay,
            ..Opts::default()
        },
    );
    assert_eq!(o.out, "before\n");
    let e = o.error.expect("an error");
    assert_eq!(
        (e.code.as_str(), e.headline.as_str(), e.line, e.col),
        ("E0803", "error[E0803]: boom 0", 17, 10),
        "{}",
        o.err
    );
    // The call path is the caller's, not the background task's (no
    // `serve`, no lambda).
    let path: Vec<&str> = e.path.iter().map(|s| s.func.as_str()).collect();
    assert_eq!(path, ["relay", "<main>"]);
    assert_eq!(o.exit, 1);
}

#[test]
fn try_call_lets_exit_through() {
    let o = exec_plain(
        "import _native.conc as c\nat_exit(() => print(\"handler\"))\nr = c.try_call(fn(x): exit(x) ;;, 3)\nprint(\"not reached\", r)",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("handler\n", "", 3)
    );
}

#[test]
fn try_call_lets_cancellation_through() {
    // The task is cancelled while `f` waits; try_call must not turn that
    // into an Err value the task goes on with.
    let o = exec_internal(
        "import _native.conc as c\nch = channel()\nt = spawn(fn(): r = c.try_call(_ => recv(ch), 0); print(\"caught\", r) ;;)\nblocked(ch, 1)\ncancel(t)\nprint(await(t))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("Err({kind: 'cancelled})\n", "", 0)
    );
}

#[test]
fn send_unchecked_sends_like_send() {
    let o = exec_plain(
        "import _native.conc as c\nch = channel(1)\nprint(c.send_unchecked(ch, 5), recv(ch))\nclose(ch)\nprint(c.send_unchecked(ch, 6))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str()),
        ("Ok(nil) Ok(5)\nErr({kind: 'closed})\n", "")
    );
}

#[test]
fn in_agent_is_true_only_in_the_task_serving_the_agent() {
    // The background task marks itself busy with state 10, answers
    // whether it is inside the agent and with what state, then clears it.
    let o = exec_internal(
        "import _native.conc as c\nfn serve(req, a) = match recv(req):\n  Ok(r) => fn():\n    c.agent_busy(req, true, 10)\n    inside = (c.in_agent(a), c.agent_state(a))\n    c.agent_busy(req, false)\n    send(r, (inside, c.in_agent(a)))\n  ;;()\n  _ => nil\n;;\nreq = channel()\na = c.agent_handle(req)\nc.spawn_background(() => serve(req, a))\nr = channel()\nsend(req, r)\nprint(c.in_agent(a), recv(r)!)\nprint(c.in_agent(a))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("false ((true, 10), false)\nfalse\n", "", 0)
    );
}

#[test]
fn in_agent_is_false_in_another_task_while_the_agent_is_busy() {
    // While the agent's task is busy, the entry asks: not inside.
    let o = exec_internal(
        "import _native.conc as c\nreq = channel()\ngo = channel()\na = c.agent_handle(req)\nc.spawn_background(fn(): c.agent_busy(req, true, 1); recv(go); c.agent_busy(req, false) ;;)\nblocked(go, 1)\nprint(c.in_agent(a))\nsend(go, nil)",
    );
    assert_eq!((o.out.as_str(), o.err.as_str(), o.exit), ("false\n", "", 0));
}

#[test]
fn agent_state_outside_the_agent_is_an_internal_error() {
    let o = exec_plain("import _native.conc as c\nc.agent_state(c.agent_handle(channel()))");
    assert!(o.err.starts_with("error[E0801]"), "{}", o.err);
}

#[test]
fn deeply_nested_agent_handles_drop_in_constant_stack() {
    // Each channel's buffer holds the last handle of the next agent.
    let o = exec_plain(
        "import _native.conc as c\nfn chain(n, acc) = n == 0 then acc else fn(): ch = channel(1); send(ch, acc); chain(n - 1, c.agent_handle(ch)) ;;()\nx = chain(200000, nil)\nx = nil\nprint(\"freed\")",
    );
    assert_eq!((o.out.as_str(), o.err.as_str()), ("freed\n", ""));
}

/// `exec_internal` bounded in time: a hang fails the test instead of
/// stalling the run (the runaway thread is left behind).
fn bounded(src: &str, limit: std::time::Duration) -> (Outcome, std::time::Duration) {
    let src = src.to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let start = std::time::Instant::now();
        let o = exec_internal(&src);
        let _ = tx.send((o, start.elapsed()));
    });
    rx.recv_timeout(limit)
        .unwrap_or_else(|_| panic!("the program did not end within {limit:?}"))
}

const LIMIT: std::time::Duration = std::time::Duration::from_secs(10);

/// A task the background task spawned and left blocked must not keep
/// the program from ending on `exit`.
#[test]
fn exit_ends_a_program_whose_background_task_spawned_a_blocked_task() {
    let (o, _) = bounded(
        "import _native.conc as c\nready = channel()\nc.spawn_background(fn(): spawn(() => recv(channel())); send(ready, 1); recv(channel()) ;;)\nrecv(ready)\nexit(3)",
        LIMIT,
    );
    assert_eq!((o.out.as_str(), o.err.as_str(), o.exit), ("", "", 3));
}

#[test]
fn an_entry_error_ends_a_program_whose_background_task_spawned_a_blocked_task() {
    let (o, _) = bounded(
        "import _native.conc as c\nready = channel()\nc.spawn_background(fn(): spawn(() => recv(channel())); send(ready, 1); recv(channel()) ;;)\nrecv(ready)\n1 / 0",
        LIMIT,
    );
    assert!(o.err.starts_with("error[E0502]"), "{}", o.err);
    assert_eq!(o.exit, 1);
}

#[test]
fn a_deadlock_ends_a_program_whose_background_task_spawned_a_blocked_task() {
    let (o, _) = bounded(
        "import _native.conc as c\nready = channel()\nc.spawn_background(fn(): spawn(() => recv(channel())); send(ready, 1); recv(channel()) ;;)\nrecv(ready)\nrecv(channel())",
        LIMIT,
    );
    assert!(
        o.err
            .starts_with("error[E0702]: deadlock: all tasks are waiting"),
        "{}",
        o.err
    );
    assert_eq!(o.exit, 1);
}

/// The background task's sleeping subtask is cancelled with it: the
/// exit does not wait out the sleep.
#[test]
fn exit_does_not_wait_for_a_background_tasks_sleeping_subtask() {
    let (o, took) = bounded(
        "import _native.conc as c\nready = channel()\nc.spawn_background(fn(): t = spawn(() => sleep(3s)); send(ready, 1); await(t) ;;)\nrecv(ready)\nexit(3)",
        LIMIT,
    );
    assert_eq!((o.out.as_str(), o.err.as_str(), o.exit), ("", "", 3));
    assert!(took < std::time::Duration::from_secs(1), "took {took:?}");
}

/// A real deadlock through an agent's subtask: the agent awaits a task
/// that waits on a silent channel while the entry waits for the agent's
/// reply. Everything is blocked, nothing has a timer.
#[test]
fn a_deadlock_through_a_background_tasks_subtask_is_reported() {
    let (o, _) = bounded(
        "import _native.conc as c\nreq = channel()\nreply = channel()\nc.spawn_background(fn(): recv(req); t = spawn(() => recv(channel())); await(t); send(reply, 1) ;;)\nsend(req, 1)\nrecv(reply)",
        LIMIT,
    );
    assert!(
        o.err
            .starts_with("error[E0702]: deadlock: all tasks are waiting"),
        "{}",
        o.err
    );
    assert_eq!(o.exit, 1);
}

/// Only the task `spawn_background` starts is exempt from failure
/// reports: a task it spawns is user code, and its failure is reported.
#[test]
fn a_failure_of_a_background_tasks_subtask_is_reported() {
    let (o, _) = bounded(
        "import _native.conc as c\nready = channel()\nc.spawn_background(fn(): spawn(() => 1 / 0); send(ready, 1); recv(channel()) ;;)\nrecv(ready)\nuntil(() => background() == 1)\nprint(\"end\")",
        LIMIT,
    );
    assert_eq!(o.out, "end\n");
    assert!(o.err.contains("task failed: error[E0502]"), "{}", o.err);
    assert_eq!(o.exit, 1);
}
