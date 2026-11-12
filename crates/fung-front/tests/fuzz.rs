//! Mutation tests of the front end (TL9): every `.fung` file of the
//! repository is a seed, and a fixed-seed xorshift generator turns seeds
//! into broken and half-broken programs (bytes flipped, tokens deleted,
//! repeated or swapped, cut short, punctuation and stray bytes put in,
//! pieces of another file spliced in, spaces, comments, newlines and
//! trailing commas added). No mutant runs; for each one:
//!
//! 1. lexing, parsing, desugar, resolve, the compiler and the formatter
//!    never panic;
//! 2. the formatter's output formats to itself, parses to the same tree
//!    and, when the mutant compiles, compiles to the same core tree;
//! 3. what compiles passes the bytecode verifier (VM22);
//! 4. compiling the same source twice gives the same bytecode;
//! 5. what compiles with its test blocks (`fung check`) compiles without
//!    them (`fung run`).
//!
//! `FUNG_FUZZ_CASES` sets how many mutants to try (default
//! [`DEFAULT_CASES`]) and `FUNG_FUZZ_SEED` the generator's seed (default
//! [`DEFAULT_SEED`]), so a longer run looks further:
//!
//!   FUNG_FUZZ_CASES=200000 FUNG_FUZZ_SEED=7 cargo test --release -p fung-front --test fuzz
//!
//! A failure names the seed, the case and the file, and prints the
//! mutant after shrinking it (chunks removed while it still fails).

use std::cell::{Cell, RefCell};
use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use fung_front::bytecode::{Proto, verify};
use fung_front::format::format;
use fung_front::loader::{Loader, Stage};

const DEFAULT_CASES: usize = 2000;
const DEFAULT_SEED: u64 = 0x5eed_f00d_1234_abcd;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn env_num(name: &str, default: u64) -> u64 {
    match std::env::var(name) {
        Ok(v) => v
            .parse()
            .unwrap_or_else(|_| panic!("{name}={v}: not a number")),
        Err(_) => default,
    }
}

/// xorshift64*: small, fast, and the same sequence everywhere.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        // splitmix64 of the seed, so nearby seeds start far apart and 0 is
        // never the state.
        let mut z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        Rng((z ^ (z >> 31)) | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// A number below `n` (`n` > 0).
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }
}

/// One seed: a file and how its compile is set up.
struct Seed {
    path: String,
    src: String,
    /// An embedded module (`extras/`): compiled as the build compiles it.
    internal: bool,
}

fn fung_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            fung_files(&p, out);
        } else if p.extension().is_some_and(|e| e == "fung") {
            out.push(p);
        }
    }
}

fn seeds() -> Vec<Seed> {
    let root = repo();
    let mut out = Vec::new();
    for dir in [
        "tests/lang",
        "examples",
        "extras",
        "bench",
        "crates/fung-cli/tests/data",
    ] {
        let mut files = Vec::new();
        fung_files(&root.join(dir), &mut files);
        for f in files {
            // A few test inputs are not UTF-8 on purpose; the front end
            // only ever sees text.
            let Ok(src) = std::fs::read_to_string(&f) else {
                continue;
            };
            out.push(Seed {
                path: f.display().to_string(),
                src,
                internal: dir == "extras",
            });
        }
    }
    out
}

/// Pieces of source: runs of name characters, of digits, of spaces, and
/// single other characters. Close enough to tokens to delete, repeat or
/// swap them, and independent of the lexer under test.
fn pieces(src: &str) -> Vec<(usize, usize)> {
    let class = |c: char| {
        if c.is_alphanumeric() || c == '_' {
            0
        } else if c == ' ' || c == '\t' {
            1
        } else {
            2
        }
    };
    let mut out = Vec::new();
    let mut it = src.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        let k = class(c);
        let mut end = i + c.len_utf8();
        if k != 2 {
            while let Some(&(j, d)) = it.peek() {
                if class(d) != k {
                    break;
                }
                end = j + d.len_utf8();
                it.next();
            }
        }
        out.push((i, end));
    }
    out
}

const PUNCT: &[&str] = &[
    "(", ")", "[", "]", "{", "}", ",", ":", ";;", ".", "..", "|>", "??", "?", "!", "=", "==", "=>",
    "->", "+", "-", "*", "/", "%", "**", "<", ">=", "\"", "'", "`", "$", "${", "\\", "#", "@", "_",
    "...", "&", "|", "~", "^",
];

const WORDS: &[&str] = &[
    "fn", "if", "else", "then", "match", "for", "in", "return", "import", "test", "and", "or",
    "not", "nil", "true", "false", "Ok", "Err", "x", "_", "0", "1.5", "0xff", "1s", "\"s\"",
    "r\"raw\"", "'sym", "print", "len", "map",
];

const STRAY: &[&str] = &[
    "\0",
    "\r",
    "\t",
    "\u{feff}",
    "\u{a0}",
    "é",
    "한",
    "\u{200b}",
    "\u{1f600}",
    "\u{301}",
    "\x7f",
];

/// One mutant of `src`, `other` being another seed to splice from.
fn mutate(rng: &mut Rng, src: &str, other: &str) -> String {
    let mut s = src.to_string();
    // One to three edits on top of each other.
    for _ in 0..1 + rng.below(3) {
        s = mutate_once(rng, &s, other);
    }
    s
}

fn char_at(rng: &mut Rng, s: &str) -> usize {
    if s.is_empty() {
        return 0;
    }
    let mut i = rng.below(s.len() + 1);
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn mutate_once(rng: &mut Rng, s: &str, other: &str) -> String {
    let ps = pieces(s);
    let ins = |at: usize, what: &str| format!("{}{what}{}", &s[..at], &s[at..]);
    match rng.below(12) {
        // flip one bit of one byte (lossy: the front end gets text)
        0 if !s.is_empty() => {
            let mut b = s.as_bytes().to_vec();
            let i = rng.below(b.len());
            b[i] ^= 1 << rng.below(8);
            String::from_utf8_lossy(&b).into_owned()
        }
        // delete a piece
        1 if !ps.is_empty() => {
            let (a, b) = *rng.pick(&ps);
            format!("{}{}", &s[..a], &s[b..])
        }
        // repeat a piece
        2 if !ps.is_empty() => {
            let (a, b) = *rng.pick(&ps);
            format!("{}{}", &s[..b], &s[a..])
        }
        // swap two pieces
        3 if ps.len() > 1 => {
            let (mut x, mut y) = (*rng.pick(&ps), *rng.pick(&ps));
            if x.0 > y.0 {
                std::mem::swap(&mut x, &mut y);
            }
            if x.1 > y.0 {
                return s.to_string();
            }
            format!(
                "{}{}{}{}{}",
                &s[..x.0],
                &s[y.0..y.1],
                &s[x.1..y.0],
                &s[x.0..x.1],
                &s[y.1..]
            )
        }
        // cut short
        4 => s[..char_at(rng, s)].to_string(),
        // punctuation
        5 => {
            let at = char_at(rng, s);
            ins(at, rng.pick::<&str>(PUNCT))
        }
        // a keyword or a small literal
        6 => {
            let at = char_at(rng, s);
            ins(at, &format!(" {} ", rng.pick(WORDS)))
        }
        // a stray character or random ASCII byte
        7 => {
            let at = char_at(rng, s);
            if rng.below(2) == 0 {
                ins(at, rng.pick::<&str>(STRAY))
            } else {
                ins(at, &char::from(rng.below(128) as u8).to_string())
            }
        }
        // a piece of another file
        8 if !other.is_empty() => {
            let a = char_at(rng, other);
            let mut b = (a + 1 + rng.below(200)).min(other.len());
            while !other.is_char_boundary(b) {
                b -= 1;
            }
            let at = char_at(rng, s);
            ins(at, &other[a..b])
        }
        // spaces, a comment, newlines
        9 => {
            let at = char_at(rng, s);
            let what = *rng.pick(&[
                " ",
                "  ",
                "\n",
                "\n\n",
                "\n\n\n",
                " # note\n",
                "# c\n",
                "\t",
            ]);
            ins(at, what)
        }
        // a trailing comma before a closing bracket
        10 => {
            let closers: Vec<usize> = s
                .char_indices()
                .filter(|&(_, c)| matches!(c, ')' | ']' | '}'))
                .map(|(i, _)| i)
                .collect();
            if closers.is_empty() {
                return s.to_string();
            }
            let at = *rng.pick(&closers);
            ins(at, rng.pick::<&str>(&[",", ", ", ",\n"]))
        }
        // a whole line deleted or repeated
        _ => {
            let lines: Vec<&str> = s.split_inclusive('\n').collect();
            if lines.is_empty() {
                return s.to_string();
            }
            let i = rng.below(lines.len());
            let mut out = String::new();
            for (j, l) in lines.iter().enumerate() {
                if j != i || rng.below(2) == 0 {
                    out.push_str(l);
                }
                if j == i {
                    out.push_str(l);
                }
            }
            out
        }
    }
}

/// The public parts of a proto, children included (the hash index of
/// constants is left out: its `Debug` order changes from run to run).
fn fingerprint(p: &Proto, out: &mut String) {
    let _ = writeln!(
        out,
        "{} {:?} {:?} {:?} {} {:?} {:?} {:?} {:?} {} {:?} {:?} {:?} {} {}",
        p.name,
        p.code,
        p.consts,
        p.pos,
        p.num_locals,
        p.global_names,
        p.params,
        p.captures,
        p.types,
        p.return_type,
        p.patterns,
        p.exports,
        p.asserts,
        p.builtin,
        p.body,
    );
    for c in &p.protos {
        fingerprint(c, out);
    }
}

/// The loaders of one kind of seed: as `fung check` and `fung test`
/// compile (test blocks kept) and as `fung run` does (left out).
struct Loaders {
    check: Loader,
    run: Loader,
}

fn loaders(internal: bool) -> Loaders {
    Loaders {
        check: loader(internal, true),
        run: loader(internal, false),
    }
}

fn loader(internal: bool, tests: bool) -> Loader {
    Loader {
        // Only regular files: a mutated import path must never open a
        // device or a pipe and wait.
        read_file: |p| {
            if std::fs::metadata(p).is_ok_and(|m| m.is_file()) {
                std::fs::read_to_string(p)
            } else {
                Err(std::io::Error::other("not a regular file"))
            }
        },
        tests,
        internal,
        library: internal,
        ..Loader::default()
    }
}

/// What compiling gives, as text to compare: the bytecode, or the error
/// codes.
struct Compiled {
    code: Option<String>,
    errors: Vec<String>,
}

fn compile(l: &Loader, file: &str, src: &str) -> Result<Compiled, String> {
    let (b, errs) = l.compile(file, src, Stage::All);
    if !errs.is_empty() {
        return Ok(Compiled {
            code: None,
            errors: errs.iter().map(|e| e.code.to_string()).collect(),
        });
    }
    let Some(p) = &b.proto else {
        return Err("no errors and no bytecode".into());
    };
    if let Err(v) = verify(p) {
        return Err(format!("verifier: {v}"));
    }
    let mut code = String::new();
    fingerprint(p, &mut code);
    Ok(Compiled {
        code: Some(code),
        errors: Vec::new(),
    })
}

/// Checks the properties on one source; `Err` says which failed, `Ok`
/// whether it compiled.
fn check(ls: &Loaders, file: &str, src: &str) -> Result<bool, String> {
    let l = &ls.check;
    let first = compile(l, file, src)?;
    let again = compile(l, file, src)?;
    if first.code != again.code || first.errors != again.errors {
        return Err("compiling twice gave different results".into());
    }
    // What `check` passes, `run` compiles.
    if first.code.is_some() {
        let run = compile(&ls.run, file, src)?;
        if !run.errors.is_empty() {
            return Err(format!(
                "compiles with its tests but not without: {:?}",
                run.errors
            ));
        }
    }
    let Ok(formatted) = format(file, src) else {
        // A syntax error: the formatter refuses what the parser refuses.
        if first.errors.is_empty() {
            return Err("the formatter refused a program that compiles".into());
        }
        return Ok(false);
    };
    match format(file, &formatted) {
        Ok(twice) if twice == formatted => {}
        Ok(twice) => {
            return Err(format!(
                "formatting is not idempotent; once:\n{formatted}\ntwice:\n{twice}"
            ));
        }
        Err(e) => {
            return Err(format!(
                "the formatted text does not format again: {}",
                e.code
            ));
        }
    }
    let tree = |s: &str| {
        let (b, errs) = l.compile(file, s, Stage::Ast);
        (b.surface.map(|n| n.to_string()), errs.len())
    };
    let (before, after) = (tree(src), tree(&formatted));
    if before.0.is_none() || before != after {
        return Err(format!(
            "formatting changed the parse; formatted:\n{formatted}"
        ));
    }
    if first.code.is_some() {
        let core = |s: &str| {
            let (b, _) = l.compile(file, s, Stage::Core);
            b.core.map(|n| n.to_string())
        };
        if core(&formatted) != core(src) {
            return Err(format!(
                "formatting changed the core tree; formatted:\n{formatted}"
            ));
        }
    }
    Ok(first.code.is_some())
}

thread_local! {
    /// The message of the last panic on this thread.
    static PANIC: RefCell<Option<String>> = const { RefCell::new(None) };
    /// Inside [`guarded`]: a panic is kept, not printed.
    static GUARDED: Cell<bool> = const { Cell::new(false) };
}

/// `check`, with a panic as a failure too.
fn guarded(l: &Loaders, file: &str, src: &str) -> Result<bool, String> {
    GUARDED.with(|g| g.set(true));
    let r = catch_unwind(AssertUnwindSafe(|| check(l, file, src)));
    GUARDED.with(|g| g.set(false));
    match r {
        Ok(r) => r,
        Err(_) => Err(format!(
            "panic: {}",
            PANIC.with(|p| p.borrow_mut().take()).unwrap_or_default()
        )),
    }
}

/// Removes chunks of `src`, halves first, while it still fails. Bounded,
/// so a slow failure still reports.
fn shrink(l: &Loaders, file: &str, mut src: String) -> String {
    let mut tries = 0;
    let mut chunk = src.len() / 2;
    while chunk > 0 && tries < 2000 {
        let mut at = 0;
        while at < src.len() && tries < 2000 {
            let mut end = (at + chunk).min(src.len());
            while !src.is_char_boundary(end) {
                end += 1;
            }
            let cand = format!("{}{}", &src[..at], &src[end..]);
            tries += 1;
            if guarded(l, file, &cand).is_err() {
                src = cand;
            } else {
                at = end;
            }
        }
        chunk /= 2;
    }
    src
}

fn run(cases: usize, seed: u64) {
    let seeds = seeds();
    assert!(seeds.len() > 400, "only {} seed files", seeds.len());
    let user = loaders(false);
    let internal = loaders(true);
    let mut rng = Rng::new(seed);
    let mut compiled = 0;
    for case in 0..cases {
        let s = &seeds[case % seeds.len()];
        let other = &seeds[rng.below(seeds.len())].src;
        let mutant = mutate(&mut rng, &s.src, other);
        let l = if s.internal { &internal } else { &user };
        match guarded(l, &s.path, &mutant) {
            Ok(ok) => compiled += usize::from(ok),
            Err(why) => {
                let small = shrink(l, &s.path, mutant.clone());
                let again = guarded(l, &s.path, &small).err().unwrap_or(why.clone());
                panic!(
                    "case {case}, a mutant of {} (again: FUNG_FUZZ_SEED={seed} FUNG_FUZZ_CASES={})\n\
                     {why}\n--- shrunk mutant ({again}) ---\n{small}\n--- end ---",
                    s.path,
                    case + 1,
                );
            }
        }
    }
    println!("{cases} mutants, {compiled} compiled (seed {seed})");
    // The mutants must not all be syntax errors: some compile, so the
    // properties past the parser are tried too.
    assert!(
        cases < 100 || compiled * 20 > cases,
        "only {compiled} of {cases} mutants compiled"
    );
}

#[test]
fn front_end_mutants_keep_the_properties() {
    let cases = env_num("FUNG_FUZZ_CASES", DEFAULT_CASES as u64) as usize;
    let seed = env_num("FUNG_FUZZ_SEED", DEFAULT_SEED);
    std::panic::set_hook(Box::new(|info| {
        let place = info
            .location()
            .map(|l| format!(" at {}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        if GUARDED.with(Cell::get) {
            PANIC.with(|p| *p.borrow_mut() = Some(format!("{msg}{place}")));
        } else {
            eprintln!("panicked{place}:\n{msg}");
        }
    }));
    // The command line runs the front end on a big stack (ENTRY_STACKS,
    // at least 64 MiB); so does this.
    let r = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || run(cases, seed))
        .unwrap()
        .join();
    let _ = std::panic::take_hook();
    if let Err(e) = r {
        std::panic::resume_unwind(e);
    }
}

#[test]
fn the_generator_is_fixed() {
    // The same seed gives the same numbers on every machine.
    let mut r = Rng::new(DEFAULT_SEED);
    let a: Vec<u64> = (0..3).map(|_| r.next()).collect();
    let mut r = Rng::new(DEFAULT_SEED);
    let b: Vec<u64> = (0..3).map(|_| r.next()).collect();
    assert_eq!(a, b);
    let mut r = Rng::new(1);
    let s = mutate(&mut r, "x = f(1, 2)\nprint(x)\n", "y = 3\n");
    let mut r = Rng::new(1);
    assert_eq!(s, mutate(&mut r, "x = f(1, 2)\nprint(x)\n", "y = 3\n"));
}
