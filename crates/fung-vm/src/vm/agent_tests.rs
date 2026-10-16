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

/// The handler waits for the background task's answer: once the
/// handlers end, the background task is cancelled, so without the
/// answer it could be cancelled between its `recv` and its `print`.
#[test]
fn background_tasks_outlive_exit_handlers() {
    let o = exec_plain(
        "import _native.conc as c\nch = channel()\ndone = channel()\nc.spawn_background(fn(): x = recv(ch)!; print(\"bg got\", x); send(done, 1) ;;)\nat_exit(fn(): send(ch, 7); recv(done) ;;)",
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
        "import _native.conc as c\nch = channel()\na = c.agent_handle(ch)\nb = c.agent_handle(ch)\nprint(a, [a], \"${a}\", type_of(a), a == a, a == b, a != b)",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str()),
        ("<agent> [<agent>] <agent> 'agent true false true\n", "")
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
fn try_call_catches_an_exit() {
    // An exit inside `f` still ends the program (the entry is cancelled
    // and the exit code is 3), but `try_call` hands it back as a value,
    // so the background task calling it carries on.
    let o = exec_plain(
        "import _native.conc as c\ndone = channel(1)\nat_exit(fn(): print(\"handler\", recv(done)!) ;;)\nc.spawn_background(fn(): r = c.try_call(fn(x): exit(x) ;;, 3); send(done, r) ;;)\nrecv(channel())\nprint(\"not reached\")",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("handler Err(<error>)\n", "", 3)
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
fn agent_peek_gives_the_state_only_in_the_task_serving_the_agent() {
    // The background task marks itself busy with state 10, peeks, then
    // clears the mark and peeks again. A nil state is still `Ok(nil)`.
    let o = exec_internal(
        "import _native.conc as c\nfn serve(req, a) = match recv(req):\n  Ok(r) => fn():\n    c.agent_busy(req, true, 10)\n    inside = c.agent_peek(a)\n    c.agent_busy(req, false)\n    c.agent_busy(req, true, nil)\n    none = c.agent_peek(a)\n    c.agent_busy(req, false)\n    send(r, (inside, none, c.agent_peek(a)))\n  ;;()\n  _ => nil\n;;\nreq = channel()\na = c.agent_handle(req)\nc.spawn_background(() => serve(req, a))\nr = channel()\nsend(req, r)\nprint(c.agent_peek(a), recv(r)!)\nprint(c.agent_peek(a))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("Err(nil) (Ok(10), Ok(nil), Err(nil))\nErr(nil)\n", "", 0)
    );
}

#[test]
fn agent_peek_is_err_in_another_task_while_the_agent_is_busy() {
    // While the agent's task is busy, the entry peeks: not inside.
    let o = exec_internal(
        "import _native.conc as c\nreq = channel()\ngo = channel()\na = c.agent_handle(req)\nc.spawn_background(fn(): c.agent_busy(req, true, 1); recv(go); c.agent_busy(req, false) ;;)\nblocked(go, 1)\nprint(c.agent_peek(a))\nsend(go, nil)",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("Err(nil)\n", "", 0)
    );
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

// --- agents: `agent`, `swap` and `deref` (`extras/rust/agent.fung`, Task 4) ---

/// Runs user code (not internal) with the probes and `until`, on a
/// thread bounded by `LIMIT`.
fn bounded_user(src: &str) -> Outcome {
    let src = src.to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(crate::testutil::exec_waiting(&src));
    });
    rx.recv_timeout(LIMIT)
        .unwrap_or_else(|_| panic!("the program did not end within {LIMIT:?}"))
}

/// An error in `f` is raised again at the user's `swap(`, with its code
/// and message; the agent's task (its loop, `f`) is not in the call
/// path, only the `swap` the user called and the user's frames.
#[test]
fn an_error_in_f_is_raised_at_the_users_swap() {
    let o = bounded_user("a = agent(1)\nprint(2)\ng = x => [swap(a, y => y / 0)]\nx = g(1)");
    let e = o.error.expect("an error");
    assert_eq!(
        (e.headline.as_str(), e.line, e.col),
        ("error[E0502]: division by zero", 3 + 2, 15),
    );
    let path: Vec<(&str, &str)> = e
        .path
        .iter()
        .map(|c| (c.file.as_str(), c.func.as_str()))
        .collect();
    assert_eq!(
        path,
        [
            ("extras/rust/agent.fung", "swap"),
            ("-e", "<lambda>"),
            ("-e", "<main>")
        ]
    );
    assert_eq!(o.exit, 1);
}

/// The state is kept when `f` fails, and the agent serves on.
#[test]
fn a_failing_f_leaves_the_state() {
    let o = bounded_user(
        "a = agent(5)\nt = spawn(() => swap(a, x => fail(\"no\")))\nprint(await(t) |> is_err, swap(a, x => x + 1), deref(a))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("true 6 6\n", "", 0)
    );
}

/// `swap` and `deref` check their agent like a native does: E0403 at the
/// call's `(`.
#[test]
fn swap_and_deref_reject_what_is_not_an_agent() {
    for (src, headline, col) in [
        (
            "print(1)\nderef(1)",
            "error[E0403]: argument `a` must be agent, got int",
            6,
        ),
        (
            "print(1)\nswap(1, x => x)",
            "error[E0403]: argument `a` must be agent, got int",
            5,
        ),
        (
            "print(1)\nswap(agent(1), 1)",
            "error[E0403]: argument `f` must be fn, got int",
            5,
        ),
    ] {
        let o = bounded_user(src);
        let e = o.error.unwrap_or_else(|| panic!("{src}: no error"));
        assert_eq!(
            (e.headline.as_str(), e.line, e.col),
            (headline, 2 + 2, col),
            "{src}"
        );
    }
}

/// `swap` on the same agent inside its own `f` is E0704, at the inner
/// `swap(` (where Go's atom reported E0703), with the caller's call path.
/// An inner `swap` in tail position has replaced `f`'s frame, so its
/// position is gone, and the error lands on the outer `swap(` like any
/// error of `f` (the tail-call rule of every fung prelude function).
#[test]
fn swap_inside_its_own_f_is_e0704() {
    for (src, col) in [
        ("a = agent(0)\nswap(a, x => 1 + swap(a, y => y))", 22),
        ("a = agent(0)\nswap(a, fn(x): swap(a, y => y); x ;;)", 20),
        ("a = agent(0)\nswap(a, x => swap(a, y => y))", 5),
    ] {
        let o = bounded_user(src);
        let e = o.error.unwrap_or_else(|| panic!("{src}: no error"));
        assert_eq!(
            (e.headline.as_str(), e.line, e.col),
            (
                "error[E0704]: agent called from inside its own swap",
                2 + 2,
                col
            ),
            "{src}"
        );
        let path: Vec<&str> = e.path.iter().map(|c| c.func.as_str()).collect();
        assert_eq!(path, ["swap", "<main>"], "{src}");
    }
    // Another agent inside `f` is fine.
    let o = bounded_user(
        "a = agent(1)\nb = agent(10)\nprint(swap(a, x => swap(b, y => y + x)), deref(a), deref(b))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("11 11 11\n", "", 0)
    );
    // But `deref(a)` inside `b`'s `f`, inside `a`'s: `b`'s task is not
    // `a`'s, so it asks `a`, which waits for `b`. A real deadlock.
    let o = bounded_user("a = agent(1)\nb = agent(10)\nswap(a, x => swap(b, y => y + deref(a)))");
    assert!(o.err.starts_with("error[E0702]: deadlock"), "{}", o.err);
}

/// An agent whose handles are all gone ends while the program runs.
#[test]
fn an_agent_whose_handles_are_dropped_ends_mid_run() {
    let o = bounded_user(
        "a = agent(1)\nprint(swap(a, x => x + 1), background())\na = nil\nuntil(() => background() == 0)\nprint(background())",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("2 1\n0\n", "", 0)
    );
}

/// An agent at a module's top level: the program ends on its own (no
/// E0702, no hang), and an exit handler can still ask the agent.
#[test]
fn a_module_level_agent_ends_cleanly_and_serves_exit_handlers() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let o = crate::testutil::run_files(
            &[
                (
                    "/p/lib.fung",
                    "counter = agent(0)\nbump = () => swap(counter, n => n + 1)\n",
                ),
                (
                    "/p/main.fung",
                    "import \"./lib\"\nlib.bump()\nat_exit(() => print(\"at exit\", deref(lib.counter)))\nprint(lib.bump())",
                ),
            ],
            "/p/main.fung",
        );
        let _ = tx.send(o);
    });
    let o = rx.recv_timeout(LIMIT).expect("the program ended");
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("2\nat exit 2\n", "", 0)
    );
}

/// `f` may spawn tasks (here through `pmap`); they run as the agent's
/// background subtasks.
#[test]
fn f_may_spawn_tasks() {
    let o =
        bounded_user("a = agent([1, 2, 3])\nprint(swap(a, xs => pmap(xs, x => x * 10)), deref(a))");
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("[10, 20, 30] [10, 20, 30]\n", "", 0)
    );
}

/// An `f` that holds its own agent is sent as a request without the
/// check that a channel is not sent into itself (`send_unchecked`).
#[test]
fn f_may_hold_its_own_agent() {
    let o = bounded_user("a = agent(1)\nprint(swap(a, x => x + len([a])), deref(a))");
    assert_eq!((o.out.as_str(), o.err.as_str(), o.exit), ("2 2\n", "", 0));
}

/// The agent's loop is a tail call: it serves more requests than a task
/// has frames (`MAX_FRAMES`). Not time-bounded (it takes seconds in a
/// loaded debug build): a loop that ran out of frames would end the
/// agent, and the waiting entry would be reported as a deadlock.
#[test]
fn an_agent_serves_more_requests_than_the_frame_limit() {
    let n = crate::vm::MAX_FRAMES + 1;
    let o = crate::testutil::exec_waiting(&format!(
        "a = agent(0)\neach(0..{n}, _ => deref(a))\nprint(swap(a, x => x + 1))"
    ));
    assert_eq!((o.out.as_str(), o.err.as_str(), o.exit), ("1\n", "", 0));
}

/// The loop does not keep the agent's first state: once a `swap`
/// replaced it, an agent handle inside it can close.
#[test]
fn the_first_state_is_not_kept_by_the_loop() {
    let o = bounded_user(
        "b = agent(1)\na = agent([b])\nswap(a, _ => nil)\nb = nil\nuntil(() => background() == 1)\nprint(background())",
    );
    assert_eq!((o.out.as_str(), o.err.as_str(), o.exit), ("1\n", "", 0));
}

/// The last handle passed to `deref`/`swap` stays alive until the
/// answer, also beside another live task (no hang, no E0702).
#[test]
fn deref_of_the_last_handle_beside_a_live_task() {
    let o = bounded_user(
        "ch = channel()\nt = spawn(() => recv(ch))\nprint(deref(agent(5)), swap(agent(1), x => x + 1))\nsend(ch, 1)\nawait(t)",
    );
    assert_eq!((o.out.as_str(), o.err.as_str(), o.exit), ("5 2\n", "", 0));
}

/// A task spawned (at any depth) inside an agent's `f` counts as inside
/// the agent: `deref` gives the state before the update, `swap` is E0704.
#[test]
fn tasks_spawned_inside_f_are_inside_the_agent() {
    let o = bounded_user(
        "a = agent(1)\nprint(swap(a, x => await(spawn(() => await(spawn(() => deref(a))))) + x), deref(a))",
    );
    assert_eq!((o.out.as_str(), o.err.as_str(), o.exit), ("2 2\n", "", 0));
    let o = bounded_user("a = agent(1)\nprint(swap(a, x => await(spawn(() => swap(a, y => y)))))");
    assert!(
        o.out
            .starts_with("Err({kind: 'error, message: \"error[E0704]"),
        "{} {}",
        o.out,
        o.err
    );
}

/// A task `f` spawned that outlives that request is not inside the
/// agent any more: its `swap` waits its turn like anyone's.
#[test]
fn a_task_outliving_its_request_is_outside_the_agent() {
    let o = bounded_user(
        "a = agent(1)\ngo = channel(1)\ndone = channel(1)\nswap(a, fn(x): spawn(fn(): recv(go); send(done, swap(a, y => y + 10)) ;;); x + 1 ;;)\nsend(go, 1)\nprint(recv(done)!, deref(a))",
    );
    assert_eq!((o.out.as_str(), o.err.as_str(), o.exit), ("12 12\n", "", 0));
}

/// An agent made inside another agent's `f` is not inside that agent.
#[test]
fn an_agent_made_inside_f_is_its_own() {
    let o = bounded_user(
        "a = agent(1)\nprint(swap(a, fn(x): b = agent(x); swap(b, y => y + 1) ;;), deref(a))",
    );
    assert_eq!((o.out.as_str(), o.err.as_str(), o.exit), ("2 2\n", "", 0));
}

/// `agent` is a type name (where Go has `atom`).
#[test]
fn agent_is_a_type_name() {
    let o = bounded_user("fn f(a: agent) = deref(a)\nprint(f(agent(3)))");
    assert_eq!((o.out.as_str(), o.err.as_str(), o.exit), ("3\n", "", 0));
    let o = bounded_user("fn f(a: agent) = a\nf(1)");
    assert_eq!(o.error.expect("an error").code, "E0403", "{}", o.err);
    let o = crate::testutil::exec("fn f(a: atom) = a", crate::testutil::Opts::default());
    assert_eq!(o.code, "E0212", "{}", o.err);
}

/// A task spawned inside `f` reads the state with `deref` while `f`
/// ends: the "inside?" check and the read are one step, so the agent
/// finishing `f` between them (the turn can pass at any call) cannot
/// make the read fail. The loop's wrapper depth moves where the turn
/// passes (every `YIELD_EVERY` ticks), so one of them lands between two
/// calls inside `deref`.
#[test]
fn deref_from_a_subtask_while_f_ends() {
    for depth in 0..=6 {
        let wrap = |e: &str| format!("{}{e}{}", "id(".repeat(depth), ")".repeat(depth));
        let src = format!(
            "fn id(x) = x\nfn signal(half, n) = n == 10000 then send(half, 1) else nil\nfn work(a, half, n, acc) = n == 0 then acc else fn(): signal(half, n); work(a, half, n - 1, acc + {}) ;;()\na = agent(1)\nhalf = channel(1)\ntc = channel(1)\nswap(a, fn(x): send(tc, spawn(() => work(a, half, 20000, 0))); recv(half); x ;;)\nprint(await(recv(tc)!))",
            wrap("deref(a)")
        );
        let o = bounded_user(&src);
        assert_eq!(
            (o.out.as_str(), o.err.as_str(), o.exit),
            ("20000\n", "", 0),
            "depth {depth}"
        );
    }
}

// --- `f` follows the task that asked for it (final fix wave) ---
//
// An agent runs `f` on its own task, but under a cancellation node that
// is also below the requester's: cancelling the requester (an `exit`, a
// deadlock report, `cancel(t)`) cancels its `f`, the state stays, and the
// agent serves the next request. Go's atom ran `f` on the caller's task;
// the expected outputs come from Go with `atom` for `agent`.

/// `bounded_user`, also giving how long the program took.
fn timed_user(src: &str) -> (Outcome, std::time::Duration) {
    let start = std::time::Instant::now();
    let o = bounded_user(src);
    (o, start.elapsed())
}

const SPIN: &str = "fn spin(n) = n == 0 then 0 else spin(n - 1)\n";

/// An `exit` while another task's `f` runs (a loop that would take
/// hours): `f` is cancelled, the handler reads the old state at once.
#[test]
fn an_exit_cancels_the_f_of_a_task_it_cancels() {
    let (o, took) = timed_user(&format!(
        "{SPIN}a = agent(0)\ngo = channel(1)\nat_exit(() => print(\"handler\", deref(a)))\nspawn(() => swap(a, fn(x): send(go, 1); spin(100000000000) + 1 ;;))\nrecv(go)\nexit(2)"
    ));
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("handler 0\n", "", 2)
    );
    assert!(took < std::time::Duration::from_secs(5), "took {took:?}");
}

/// `cancel(t)` while `t`'s `f` runs: the update is not applied, and the
/// agent goes on to serve the next request (the entry's `deref`).
#[test]
fn cancelling_the_requester_cancels_its_f() {
    let o = bounded_user(&format!(
        "{SPIN}a = agent(0)\ngo = channel(1)\nat_exit(() => print(\"handler\", deref(a)))\nt = spawn(() => swap(a, fn(x): send(go, 1); spin(100000000000) + 1 ;;))\nrecv(go)\ncancel(t)\nprint(await(t), deref(a))"
    ));
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("Err({kind: 'cancelled}) 0\nhandler 0\n", "", 0)
    );
    let o = bounded_user(
        "a = agent(0)\ngo = channel(1)\nt = spawn(() => swap(a, fn(x): send(go, 1); sleep(1s); x + 1 ;;))\nrecv(go)\ncancel(t)\nprint(deref(a))",
    );
    assert_eq!((o.out.as_str(), o.err.as_str(), o.exit), ("0\n", "", 0));
}

/// An `f` asleep when the program exits is woken and cancelled: the exit
/// does not wait out the sleep.
#[test]
fn an_exit_wakes_a_sleeping_f() {
    let (o, took) = timed_user(
        "a = agent(0)\ngo = channel(1)\nat_exit(() => print(\"handler\", deref(a)))\nspawn(() => swap(a, fn(x): send(go, 1); sleep(3s); x + 1 ;;))\nrecv(go)\nexit(2)",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("handler 0\n", "", 2)
    );
    assert!(took < std::time::Duration::from_secs(2), "took {took:?}");
}

/// A deadlock inside `f` is reported once: the report cancels the
/// program, so the entry's `f` with it, and the handler's `deref` is
/// served.
#[test]
fn a_deadlock_inside_f_is_reported_once_and_the_agent_serves_on() {
    let o = bounded_user(
        "a = agent(0)\nc = channel()\nat_exit(() => print(\"handler\", deref(a)))\nswap(a, x => recv(c)!)",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        (
            "handler 0\n",
            "error[E0702]: deadlock: all tasks are waiting\n",
            1
        )
    );
}

/// The same in an exit handler: the deadlock cancels that handler and
/// its `f`, and the next handler is served.
#[test]
fn a_deadlock_inside_a_handlers_f_leaves_the_next_handler_served() {
    let o = bounded_user(
        "a = agent(0)\nc = channel()\nat_exit(() => print(\"second\", deref(a)))\nat_exit(() => print(\"first\", swap(a, x => recv(c)!)))\nprint(\"main\", deref(a))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        (
            "main 0\nsecond 0\n",
            "error[E0702]: deadlock: all tasks are waiting\n",
            1
        )
    );
}

/// The tasks `f` spawned end with the task that asked, as when Go's
/// atom ran `f` on that task: they do not outlive it.
#[test]
fn tasks_spawned_inside_f_end_with_the_requester() {
    let (o, took) = timed_user(
        "a = agent(0)\nsc = channel(1)\nt = spawn(() => swap(a, fn(x): send(sc, spawn(() => sleep(5s))); x + 1 ;;))\nprint(await(t))\nprint(await(recv(sc)!), background())",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("1\nErr({kind: 'cancelled}) 1\n", "", 0)
    );
    assert!(took < std::time::Duration::from_secs(3), "took {took:?}");
}

/// A request whose requester was cancelled before the agent took it up
/// is skipped: its `f` does not run. (`agent_ask` over a buffered
/// channel leaves the request queued while the requester waits.)
#[test]
fn a_request_whose_requester_is_cancelled_is_skipped() {
    let o = exec_internal(
        "import _native.conc as c\nreq = channel(1)\na = c.agent_handle(req)\nran = channel(1)\nreply = channel(1)\nt = spawn(() => c.agent_ask(a, 1, reply))\nblocked(reply, 1)\ncancel(t)\nawait(t)\nrecv(req)\nprint(c.try_call(fn(x): send(ran, x); x ;;, 7, reply), recv(ran, timeout: 0s))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("Err(<error>) Err({kind: 'timeout})\n", "", 0)
    );
}

/// `set_asker(ch)` (stage 4d Task 5, the `http` server) records the
/// current task on `ch` for every later `try_call` with it, not just one:
/// each `f` runs below the marking task, so cancelling that task stops a
/// running `f` (caught, E0701) and skips a later one.
#[test]
fn set_asker_lets_every_try_call_follow_the_marking_task() {
    let o = exec_internal(
        "import _native.conc as c\nscope = channel()\nmarked = channel(1)\nt = spawn(fn(): c.set_asker(scope); send(marked, 1); recv(channel()) ;;)\nrecv(marked)!\nout = channel(5)\nwait = channel()\nfn code(r) = match r:\n  Err(e) => c.describe(e).code\n  v => v\n;;\nc.spawn_background(fn():\n  send(out, c.try_call(x => x + 1, 1, scope))\n  send(out, code(c.try_call(fn(x): recv(wait) ;;, 0, scope)))\n  send(out, code(c.try_call(x => x, 0, scope)))\n;;)\nprint(recv(out)!)\nblocked(wait, 1)\ncancel(t)\nprint(recv(out)!, recv(out)!)",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("Ok(2)\nE0701 E0701\n", "", 0)
    );
}

/// `describe(e)`: what a caught stop was, for the `http` server's report:
/// `{code, text}` with the error's code and headline, or `exit` and Go's
/// `exit(n)`.
#[test]
fn describe_tells_what_stopped_a_call() {
    let o = exec_internal(
        "import _native.conc as c\nfn d(f) = match c.try_call(f, 3):\n  Err(e) => c.describe(e)\n  v => v\n;;\nprint(d(x => fail(\"boom ${x}\")))\nprint(d(x => 1 / 0))\nout = channel(1)\nc.spawn_background(() => send(out, d(fn(x): exit(x) ;;)))\nat_exit(() => print(recv(out)!))\nrecv(channel())",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        (
            "{code: \"E0803\", text: \"error[E0803]: boom 3\"}\n{code: \"E0502\", text: \"error[E0502]: division by zero\"}\n{code: \"exit\", text: \"exit(3)\"}\n",
            "",
            3
        )
    );
    let o = exec_internal("import _native.conc as c\nc.describe(1)");
    assert!(
        o.err
            .starts_with("error[E0403]: argument `e` must be error, got int"),
        "{}",
        o.err
    );
}
