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

/// Two tasks each add 1 ten thousand times, their turns interleaved
/// between swaps. No update is lost.
#[test]
fn concurrent_swaps_lose_no_update() {
    assert_eq!(
        run(
            "a = atom(0)\nfn spin(n) = n == 0 then 0 else spin(n - 1)\nscope(fn(s): each(0..2, _ => spawn(s, () => each(0..10000, _ => swap(a, fn(x): spin(20); x + 1 ;;)))) ;;)\nprint(deref(a))"
        ),
        ("20000\n".into(), "".into(), 0)
    );
}

/// A turn that ends inside a swap's function is owed, not dropped: the
/// task gives it at the first yield point after the function. Here the
/// function runs longer than a turn (`YIELD_EVERY` calls), so every turn
/// ends inside a swap; the task used to never give its turn, and the
/// sleeping root (and timers, commands, other tasks) waited for the whole
/// loop. Release builds met it in `fn(): swap(n, x => x + 1); lp(k - 1) ;;()`
/// too, whose 4 calls a round divide `YIELD_EVERY`.
#[test]
fn a_turn_ending_inside_a_swap_is_given_after_it() {
    assert_eq!(
        run(
            "n = atom(0)\nfn spin(k) = k == 0 then 0 else spin(k - 1)\nfn lp(k) = k == 0 then nil else lp(k - 1 + 0 * swap(n, fn(x): spin(1500); x + 1 ;;))\nscope(fn(s):\n  spawn(s, () => lp(300))\n  sleep(1ms)\n  print(deref(n) < 300)\n;;)"
        ),
        ("true\n".into(), "".into(), 0)
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
    let mut want: Vec<String> = crate::vm::EFFECTS
        .iter()
        .flat_map(|(m, names)| names.iter().map(move |n| format!("{m}.{n}")))
        .collect();
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
        assert!(out.starts_with("`recv` has an effect"), "{body}: {out}");
        assert!(out.ends_with("Ok(5) 0\n"), "{body}: {out}");
        assert_eq!(code, 0, "{body}");
    }
}

/// Every builtin is an effect or pure, in exactly one of the two tables
/// (`atom.rs`): a new builtin without its classification fails here.
#[test]
fn every_builtin_is_classified() {
    let mut missing = Vec::new();
    let mut both = Vec::new();
    let mut check = |module: &str, name: &str| {
        let e = crate::vm::is_listed(crate::vm::EFFECTS, module, name);
        let p = crate::vm::is_listed(crate::vm::PURE, module, name);
        match (e, p) {
            (false, false) => missing.push(format!("{module}.{name}")),
            (true, true) => both.push(format!("{module}.{name}")),
            _ => {}
        }
    };
    for (name, v) in crate::prelude::core()
        .into_iter()
        .chain(crate::conc::natives())
    {
        if matches!(v, crate::value::Value::Native(_)) {
            check("prelude", name);
        }
    }
    for (key, make) in crate::boot::NATIVE_MODULES {
        let m = make();
        for name in m.names() {
            if matches!(m.get(name), Some(crate::value::Value::Native(_))) {
                check(key, name);
            }
        }
    }
    assert!(
        missing.is_empty() && both.is_empty(),
        "unclassified {missing:?}, both {both:?}"
    );
}

/// A pure helper of a native module runs inside a swap's function.
#[test]
fn pure_helpers_run_inside_a_swap() {
    let o = exec(
        "import _native.net as net\na = atom(0)\nprint(swap(a, x => x + net.byte_len(\"é\")))",
        crate::testutil::Opts {
            internal: true,
            ..crate::testutil::Opts::default()
        },
    );
    assert_eq!((o.out.as_str(), o.err.as_str()), ("2\n", ""));
}

/// E0707 names what the user called (a function of a built-in module),
/// not the primitive it reached inside.
#[test]
fn e0707_names_what_the_user_called() {
    let (_, err, _) = run("import fs\na = atom(0)\nswap(a, x => fs.with_open(\"/tmp/x\", _ => x))");
    assert!(err.contains("`fs.with_open` has an effect"), "{err}");
}

/// E0707 names the function the user called even when it tail-calls a
/// helper of its module, and a prelude function by its plain name.
#[test]
fn e0707_names_the_called_function_through_tail_calls() {
    for (src, name) in [
        (
            "import http\na = atom(0)\nswap(a, x => http.get(\"http://127.0.0.1:1/\"))",
            "`http.get`",
        ),
        (
            "import conc\na = atom(0)\nswap(a, x => conc.pmap([1], y => y))",
            "`conc.pmap`",
        ),
        (
            "(tx, rx) = channel(1)\na = atom(0)\nswap(a, x => take(rx, 1))",
            "`take`",
        ),
        (
            "(tx, rx) = channel(1)\na = atom(0)\nswap(a, x => first(rx))",
            "`first`",
        ),
    ] {
        let (_, err, _) = run(src);
        assert!(
            err.contains(&format!("{name} has an effect")),
            "{src}\n{err}"
        );
    }
}

/// A regex compiled inside a swap's function runs in place: the function
/// never parks (another task cannot change the atom under it).
#[test]
fn a_regex_inside_a_swap_compiles_in_place() {
    let o = exec(
        "import regex\na = atom(0)\nn = offloads()\nprint(swap(a, x => if regex.is_match(\"q12\", \"q[0-9]+z?\"): 1 else: 2 ;;), offloads() - n)",
        crate::testutil::Opts {
            extra: crate::testutil::probes,
            ..crate::testutil::Opts::default()
        },
    );
    assert_eq!((o.out.as_str(), o.err.as_str()), ("1 0\n", ""));
}

/// A module member that is a primitive itself (`fs.read` is
/// `_native.fs.read`) is named with its module too.
#[test]
fn e0707_names_a_direct_native_with_its_module() {
    let (_, err, _) = run("import fs\na = atom(0)\nswap(a, x => fs.read(\"/nope\"))");
    assert!(err.contains("`fs.read` has an effect"), "{err}");
}

/// Every name the effect and pure tables list is a built-in that exists:
/// a function removed or renamed leaves no stale entry behind.
#[test]
fn every_listed_name_exists() {
    let mut have = std::collections::HashSet::new();
    for (name, v) in crate::prelude::core()
        .into_iter()
        .chain(crate::conc::natives())
    {
        if matches!(v, crate::value::Value::Native(_)) {
            have.insert(format!("prelude.{name}"));
        }
    }
    for (key, make) in crate::boot::NATIVE_MODULES {
        let m = make();
        for name in m.names() {
            if matches!(m.get(name), Some(crate::value::Value::Native(_))) {
                have.insert(format!("{key}.{name}"));
            }
        }
    }
    let mut stale = Vec::new();
    for table in [crate::vm::EFFECTS, crate::vm::PURE] {
        for (module, names) in table {
            for name in *names {
                let full = format!("{module}.{name}");
                if !have.contains(&full) {
                    stale.push(full);
                }
            }
        }
    }
    assert!(stale.is_empty(), "listed but not built in: {stale:?}");
}

/// The help for iterating a sending end does not name a verb: it reads
/// the same under `for`, `map` or `fold`.
#[test]
fn e0506_help_does_not_name_a_verb() {
    let (_, err, _) = run("(tx, rx) = channel()\nfold(tx, 0, (a, x) => a)");
    assert!(
        err.contains("a sending end (`tx`) cannot be read; go over the receiving end (`rx`)"),
        "{err}"
    );
}

/// A user module that binds a primitive under a name of its own does
/// not lend that name (or its path) to E0707: the standard module's name
/// is given.
#[test]
fn e0707_names_a_direct_native_by_its_standard_module() {
    let o = crate::testutil::run_files(
        &[
            ("/p/util.fung", "import fs\nr = fs.read\n"),
            (
                "/p/main.fung",
                "import \"./util\"\nimport fs\na = atom(0)\nswap(a, x => fs.read(\"/nope\"))",
            ),
        ],
        "/p/main.fung",
    );
    assert!(o.err.contains("`fs.read` has an effect"), "{}", o.err);
}
