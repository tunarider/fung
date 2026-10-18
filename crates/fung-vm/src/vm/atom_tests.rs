//! Atoms (concurrency v2, L5): `atom`, `deref`, `swap`, `reset`, the
//! purity of a swap's function and what an atom may hold.

use std::time::{Duration, Instant};

use crate::testutil::{Opts, exec, run, run_files};

#[test]
fn atoms_hold_swap_and_reset() {
    assert_eq!(
        run(
            "a = atom(1)\nprint(swap(a, x => x + 1), deref(a), reset(a, 10), deref(a), a, type_of(a), a == a)"
        ),
        ("2 2 10 10 <atom> 'atom true\n".into(), "".into(), 0)
    );
}

/// Two tasks each add 1 ten thousand times; `f` is long enough that the
/// run token changes hands in the middle of some calls, so some swaps
/// see their atom changed and run `f` again. No update is lost.
#[test]
fn concurrent_swaps_lose_no_update() {
    assert_eq!(
        run(
            "a = atom(0)\nfn spin(n) = n == 0 then 0 else spin(n - 1)\nscope(fn(s): each(0..2, _ => spawn(s, () => each(0..10000, _ => swap(a, fn(x): spin(20); x + 1 ;;)))) ;;)\nprint(deref(a))"
        ),
        ("20000\n".into(), "".into(), 0)
    );
}

#[test]
fn an_effect_in_f_is_e0707() {
    let (out, err, code) = run("swap(atom(1), fn(x): print(x); x ;;)");
    assert_eq!(out, "");
    assert!(
        err.contains("error[E0707]: `print` has an effect, which a swap's function must not have"),
        "{err}"
    );
    assert_eq!(code, 1);
}

#[test]
fn blocking_spawning_and_other_atoms_are_effects() {
    for (body, name) in [
        ("sleep(1ms)", "sleep"),
        ("recv(rx)", "recv"),
        ("scope(s => spawn(s, () => 1))", "spawn"),
        ("swap(b, y => y)", "swap"),
        ("reset(b, 1)", "reset"),
        ("exit(0)", "exit"),
        ("sh(\"true\")", "sh"),
        ("eprint(1)", "eprint"),
    ] {
        let src = format!(
            "(_tx, rx) = channel()\na = atom(1)\nb = atom(2)\nswap(a, fn(x): {body}; x ;;)"
        );
        let (_, err, code) = run(&src);
        assert!(
            err.contains(&format!("error[E0707]: `{name}` has an effect")),
            "{body}: {err}"
        );
        assert_eq!(code, 1, "{body}");
    }
}

#[test]
fn reads_are_not_effects() {
    assert_eq!(
        run(
            "import time\na = atom(1)\nb = atom(5)\nprint(swap(a, fn(x): t = time.now(); h = env(\"HOME\"); x + deref(b) + len(upper(\"ab\")) ;;))"
        ),
        ("8\n".into(), "".into(), 0)
    );
}

#[test]
fn an_atom_cannot_hold_mutable_things() {
    for src in [
        "atom(channel())",
        "a = atom(1)\nreset(a, [1, channel()])",
        "a = atom(1)\nswap(a, _ => {k: atom(1)})",
        "atom(scope(s => s))",
        "(_tx, ch) = channel()\natom(() => ch)",
    ] {
        let (_, err, code) = run(src);
        assert!(
            err.contains("error[E0708]: an atom cannot hold"),
            "{src}: {err}"
        );
        assert_eq!(code, 1, "{src}");
    }
    let (_, err, _) = run("(tx, _rx) = channel()\natom(tx)");
    assert!(
        err.contains("error[E0708]: an atom cannot hold an atom, a channel end, a task, a scope, a file or a socket"),
        "{err}"
    );
    assert_eq!(
        run("print(deref(atom(() => 1))())"),
        ("1\n".into(), "".into(), 0)
    );
}

#[test]
fn a_failing_f_leaves_the_state() {
    assert_eq!(
        run(
            "a = atom(1)\nsupervise(s => spawn(s, () => swap(a, x => x / 0)), e => nil)\nprint(deref(a))"
        ),
        ("1\n".into(), "".into(), 0)
    );
}

/// A swap on an atom holding a map of 100,000 keys looks at the changed
/// path only (the nodes remember what they hold).
#[test]
fn big_map_swaps_are_cheap() {
    let start = Instant::now();
    assert_eq!(
        run(
            "m = fold(0..100000, {}, (acc, i) => put(acc, \"k${i}\", i))\na = atom(m)\neach(0..1000, i => swap(a, x => put(x, \"k${i % 100}\", i)))\nprint(len(deref(a)))"
        ),
        ("100000\n".into(), "".into(), 0)
    );
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "{:?}",
        start.elapsed()
    );
    let (_, err, _) = run(
        "m = fold(0..100000, {}, (acc, i) => put(acc, \"k${i}\", i))\na = atom(m)\nswap(a, x => put(x, \"k5\", channel()))",
    );
    assert!(err.contains("E0708"), "{err}");
    assert_eq!(
        run(
            "m = put(fold(0..100000, {}, (acc, i) => put(acc, \"k${i}\", i)), \"c\", channel())\nprint(len(deref(atom(remove(m, \"c\")))))"
        ),
        ("100000\n".into(), "".into(), 0)
    );
}

#[test]
fn atom_checks_its_arguments() {
    let (_, err, _) = run("deref(1)");
    assert!(err.contains("argument `a` must be atom"), "{err}");
    let (_, err, _) = run("swap(atom(1), 2)");
    assert!(err.contains("argument `f` must be fn"), "{err}");
}

/// The natives marked as effects, by where they live: what a swap's
/// function may not call (the plan's table).
#[test]
fn the_effect_table() {
    let mut got = crate::vm::effect_names();
    got.sort();
    let mut want: Vec<String> = [
        "prelude.print",
        "prelude.eprint",
        "prelude.dbg",
        "prelude.read_line",
        "prelude.read_lines",
        "prelude.exit",
        "prelude.spawn",
        "prelude.await",
        "prelude.cancel",
        "prelude.send",
        "prelude.recv",
        "prelude.close",
        "prelude.sleep",
        "prelude.swap",
        "prelude.reset",
        "prelude.request_stop",
        "_native.process.exec",
        "_native.process.sh",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    for m in ["_native.fs", "_native.net", "_native.conc"] {
        want.push(format!("{m}.*"));
    }
    want.sort();
    assert_eq!(got, want);
}

/// An error in `f` is reported where it happened, inside `f`, with the
/// user's call path (`swap` runs `f` on the caller's task; the agents
/// raised it again at the `swap(` call).
#[test]
fn an_error_in_f_is_reported_inside_f() {
    let o = exec(
        "a = atom(1)\nprint(2)\ng = x => [swap(a, y => y / 0)]\nx = g(1)",
        Opts::default(),
    );
    assert_eq!(o.out, "2\n");
    let e = o.error.expect("an error");
    assert_eq!(
        (e.headline.as_str(), e.line, e.col),
        ("error[E0502]: division by zero", 3, 26),
        "{}",
        o.err
    );
    let path: Vec<&str> = e.path.iter().map(|c| c.func.as_str()).collect();
    assert_eq!(path, ["<lambda>", "<lambda>", "<main>"], "{}", o.err);
    assert_eq!(o.exit, 1);
}

/// An atom at a module's top level: the program ends on its own, and a
/// release still reads it.
#[test]
fn a_module_level_atom_serves_a_release() {
    let o = run_files(
        &[
            (
                "/p/lib.fung",
                "counter = atom(0)\nbump = () => swap(counter, n => n + 1)\n",
            ),
            (
                "/p/main.fung",
                "import \"./lib\"\nlib.bump()\nwith(() => nil, _ => print(lib.bump()), _ => print(\"at exit\", deref(lib.counter)))",
            ),
        ],
        "/p/main.fung",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str(), o.exit),
        ("2\nat exit 2\n", "", 0)
    );
}

/// `f` may hold its own atom (only what the atom holds is checked), and
/// a fresh atom works beside a live task.
#[test]
fn f_may_hold_its_own_atom() {
    assert_eq!(
        run("a = atom(1)\nprint(swap(a, x => x + len([a])), deref(a))"),
        ("2 2\n".into(), "".into(), 0)
    );
    assert_eq!(
        run(
            "(tx, ch) = channel()\nscope(fn(s):\n  t = spawn(s, () => recv(ch))\n  print(deref(atom(5)), swap(atom(1), x => x + 1))\n  send(tx, 1)\n  await(t)\n;;)"
        ),
        ("5 2\n".into(), "".into(), 0)
    );
}

/// `atom` is a type name.
#[test]
fn atom_is_a_type_name() {
    assert_eq!(
        run("fn f(a: atom) = deref(a)\nprint(f(atom(3)))"),
        ("3\n".into(), "".into(), 0)
    );
    let o = exec("fn f(a: atom) = a\nf(1)", Opts::default());
    assert_eq!(o.error.expect("an error").code, "E0403", "{}", o.err);
}

/// Receiving from a channel inside `f` is an effect however it is
/// reached: by `recv`, by a `for` over the channel or by a native that
/// iterates it (`map`, `fold`). Nothing is taken from the channel.
#[test]
fn iterating_a_channel_in_f_is_an_effect() {
    for body in [
        "for y in ch:\n    y\n  ;;",
        "map(ch, y => y)",
        "fold(ch, 0, (acc, y) => acc + y)",
    ] {
        let src = format!(
            "(tx, ch) = channel(3)\nsend(tx, 5)\nclose(tx)\na = atom(0)\nsupervise(s => spawn(s, () => swap(a, fn(x):\n  {body}\n  x + 1\n;;)), e => print(e.message))\nprint(recv(ch), deref(a))"
        );
        let (out, _, code) = run(&src);
        assert!(
            out.starts_with("error[E0707]: `recv` has an effect"),
            "{body}: {out}"
        );
        assert!(out.ends_with("Ok(5) 0\n"), "{body}: {out}");
        assert_eq!(code, 0, "{body}");
    }
}
