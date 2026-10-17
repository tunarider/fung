//! Expected-output files: what `fung` must print for a test's cases,
//! kept under `crates/fung-cli/tests/expected/`.
//!
//! A suite is a list of entries in order. Each entry has a label (one
//! line, unique in its file) and named fields (standard output, standard
//! error, the exit status, or whatever else the test records). The file
//! is plain text, one line per output line, so a change shows up in a
//! diff as the lines that changed:
//!
//! ```text
//! ==== run tests/lang/arith.fung
//! -- stdout
//! | 7
//! | 9 -4
//! -- stderr
//! -- exit: 0
//! ```
//!
//! - `-- name: value`: a short value of one line, written inline.
//! - `-- name`: the lines that follow, each prefixed with `| ` (`|` alone
//!   for an empty line). `\ no newline` after the last one when the value
//!   does not end with a newline. No lines: the value is empty.
//! - `-- name (escaped)`: the same, for a value that is not plain text
//!   (not UTF-8, or with control characters other than tab and newline):
//!   `\\`, `\t`, `\r` and `\xNN` (a byte) are escapes.
//! - `-- name (N bytes, fnv1a64 H)`: a value longer than the suite's
//!   limit, kept as its length and FNV-1a hash only.
//!
//! The default mode compares the binary under test with the files.
//! `FUNG_EXPECTED=bless` rewrites them from its output instead, after a
//! deliberate change (review the diff before committing it).

use std::collections::HashMap;
use std::path::PathBuf;

/// What a run of a test does with its expected-output files.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Compare with the files (the default).
    Check,
    /// Rewrite the files from the binary under test (`FUNG_EXPECTED=bless`).
    Bless,
}

pub fn mode() -> Mode {
    match std::env::var("FUNG_EXPECTED").as_deref() {
        Err(_) | Ok("") => Mode::Check,
        Ok("bless") => Mode::Bless,
        Ok(other) => panic!("FUNG_EXPECTED={other:?}: want bless"),
    }
}

/// One case's output: a label and named fields, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub label: String,
    pub fields: Vec<(String, Vec<u8>)>,
}

impl Entry {
    pub fn new(label: impl Into<String>) -> Entry {
        Entry {
            label: one_line(&label.into()),
            fields: Vec::new(),
        }
    }

    pub fn field(mut self, name: &str, value: impl Into<Vec<u8>>) -> Entry {
        self.fields.push((name.to_string(), value.into()));
        self
    }

    /// A run's standard output, standard error and exit status, with each
    /// `(from, to)` replacement applied to the two outputs (paths that
    /// change from run to run).
    pub fn run(label: impl Into<String>, run: &super::Run, subs: &[(&str, &str)]) -> Entry {
        Entry::new(label)
            .field("stdout", replace(&run.0, subs))
            .field("stderr", replace(&run.1, subs))
            .field("exit", exit_text(&run.2))
    }

    pub fn get(&self, name: &str) -> Option<&[u8]> {
        self.fields
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_slice())
    }
}

/// `bytes` with every `from` replaced by `to`, in order.
pub fn replace(bytes: &[u8], subs: &[(&str, &str)]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    for (from, to) in subs {
        if from.is_empty() {
            continue;
        }
        let (from, to) = (from.as_bytes(), to.as_bytes());
        let mut next = Vec::with_capacity(out.len());
        let mut i = 0;
        while i < out.len() {
            if out[i..].starts_with(from) {
                next.extend_from_slice(to);
                i += from.len();
            } else {
                next.push(out[i]);
                i += 1;
            }
        }
        out = next;
    }
    out
}

/// `bytes` with each port number replaced by its name (`$PORT0`, ...):
/// only whole numbers, not digits inside a longer one.
pub fn replace_ports(bytes: &[u8], ports: &[(u16, &str)]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        let end = i + bytes[i..].iter().take_while(|b| b.is_ascii_digit()).count();
        let digits = &bytes[i..end];
        match ports
            .iter()
            .find(|(p, _)| p.to_string().as_bytes() == digits)
        {
            Some((_, name)) => out.extend_from_slice(name.as_bytes()),
            None => out.extend_from_slice(digits),
        }
        i = end;
    }
    out
}

/// An exit status as `0`, `3` or `signal 9`, from its debug text
/// (`ExitStatus(unix_wait_status(768))`) or a plain code.
pub fn exit_text(status: &str) -> String {
    let raw = status
        .strip_prefix("ExitStatus(unix_wait_status(")
        .and_then(|s| s.strip_suffix("))"));
    match raw.map(str::parse::<i32>) {
        Some(Ok(w)) if w & 0x7f == 0 => ((w >> 8) & 0xff).to_string(),
        Some(Ok(w)) => format!("signal {}", w & 0x7f),
        _ => status.to_string(),
    }
}

/// A label on one line: newlines and other control characters escaped.
pub fn one_line(label: &str) -> String {
    let mut out = String::new();
    for c in label.chars() {
        if c.is_control() {
            out.extend(c.escape_default());
        } else {
            out.push(c);
        }
    }
    out
}

/// A label for a case with long arguments: short ones as they are, long
/// ones cut, with their length and hash so the label stays unique.
pub fn short_label(text: &str) -> String {
    const MAX: usize = 120;
    if text.chars().count() <= MAX {
        return text.to_string();
    }
    let head: String = text.chars().take(60).collect();
    format!(
        "{head}… ({} bytes, {:016x})",
        text.len(),
        fnv1a64(text.as_bytes())
    )
}

/// FNV-1a, 64 bits.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// The directory of the expected-output files.
pub fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/expected")
}

/// A field as a file holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stored {
    Bytes(Vec<u8>),
    Hash { len: usize, hash: u64 },
}

impl Stored {
    fn of(value: &[u8], limit: usize) -> Stored {
        if value.len() > limit {
            Stored::Hash {
                len: value.len(),
                hash: fnv1a64(value),
            }
        } else {
            Stored::Bytes(value.to_vec())
        }
    }

    /// Whether `got` is this value.
    pub fn matches(&self, got: &[u8]) -> bool {
        match self {
            Stored::Bytes(b) => b == got,
            Stored::Hash { len, hash } => *len == got.len() && *hash == fnv1a64(got),
        }
    }
}

/// A file's entries: label, then its fields.
pub type Suite = Vec<(String, Vec<(String, Stored)>)>;

fn plain(value: &[u8]) -> bool {
    std::str::from_utf8(value)
        .is_ok_and(|s| s.chars().all(|c| c == '\t' || c == '\n' || !c.is_control()))
}

fn escape_line(line: &[u8]) -> String {
    let mut out = String::new();
    for chunk in line.utf8_chunks() {
        for c in chunk.valid().chars() {
            match c {
                '\\' => out.push_str("\\\\"),
                '\t' => out.push_str("\\t"),
                '\r' => out.push_str("\\r"),
                c if c.is_control() => {
                    let mut buf = [0; 4];
                    for b in c.encode_utf8(&mut buf).bytes() {
                        out.push_str(&format!("\\x{b:02x}"));
                    }
                }
                c => out.push(c),
            }
        }
        for b in chunk.invalid() {
            out.push_str(&format!("\\x{b:02x}"));
        }
    }
    out
}

fn unescape_line(line: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut cs = line.chars();
    while let Some(c) = cs.next() {
        if c != '\\' {
            let mut buf = [0; 4];
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            continue;
        }
        match cs.next() {
            Some('\\') => out.push(b'\\'),
            Some('t') => out.push(b'\t'),
            Some('r') => out.push(b'\r'),
            Some('x') => {
                let hex: String = cs.by_ref().take(2).collect();
                out.push(u8::from_str_radix(&hex, 16).expect("a \\x escape"));
            }
            e => panic!("bad escape \\{e:?} in an expected-output file"),
        }
    }
    out
}

/// Writes a file's text.
pub fn render(header: &str, entries: &[Entry], limit: usize) -> String {
    let mut out = String::new();
    for l in header.lines() {
        out.push_str(&format!("# {l}\n").replace("# \n", "#\n"));
    }
    for e in entries {
        out.push_str(&format!("==== {}\n", e.label));
        for (name, value) in &e.fields {
            match Stored::of(value, limit) {
                Stored::Hash { len, hash } => {
                    out.push_str(&format!("-- {name} ({len} bytes, fnv1a64 {hash:016x})\n"));
                }
                Stored::Bytes(v) => {
                    let text = std::str::from_utf8(&v).ok().filter(|_| plain(&v));
                    match text {
                        Some(t)
                            if !t.is_empty()
                                && !t.contains('\n')
                                && t.len() <= 100
                                && t.trim() == t =>
                        {
                            out.push_str(&format!("-- {name}: {t}\n"));
                        }
                        _ => {
                            let escaped = text.is_none();
                            out.push_str(&format!(
                                "-- {name}{}\n",
                                if escaped { " (escaped)" } else { "" }
                            ));
                            let body = v.strip_suffix(b"\n").unwrap_or(&v);
                            if !v.is_empty() {
                                for line in body.split(|b| *b == b'\n') {
                                    let line = if escaped {
                                        escape_line(line)
                                    } else {
                                        String::from_utf8(line.to_vec()).unwrap()
                                    };
                                    if line.is_empty() {
                                        out.push_str("|\n");
                                    } else {
                                        out.push_str(&format!("| {line}\n"));
                                    }
                                }
                                if !v.ends_with(b"\n") {
                                    out.push_str("\\ no newline\n");
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

/// Reads a file's text.
pub fn parse(text: &str) -> Suite {
    let mut suite: Suite = Vec::new();
    // The field being read: its value so far and whether it is escaped.
    let mut open: Option<(Vec<u8>, bool)> = None;
    let close = |suite: &mut Suite, open: &mut Option<(Vec<u8>, bool)>| {
        if let Some((v, _)) = open.take() {
            let field = suite.last_mut().unwrap().1.last_mut().unwrap();
            field.1 = Stored::Bytes(v);
        }
    };
    for (n, line) in text.lines().enumerate() {
        let n = n + 1;
        if let Some(label) = line.strip_prefix("==== ") {
            close(&mut suite, &mut open);
            suite.push((label.to_string(), Vec::new()));
        } else if let Some(field) = line.strip_prefix("-- ") {
            close(&mut suite, &mut open);
            let entry = &mut suite
                .last_mut()
                .unwrap_or_else(|| panic!("line {n}: a field before any entry"))
                .1;
            if let Some((name, value)) = field.split_once(": ") {
                entry.push((name.to_string(), Stored::Bytes(value.as_bytes().to_vec())));
            } else if let Some(name) = field.strip_suffix(" (escaped)") {
                entry.push((name.to_string(), Stored::Bytes(Vec::new())));
                open = Some((Vec::new(), true));
            } else if let Some((name, rest)) = field.split_once(" (") {
                let rest = rest.strip_suffix(')').unwrap_or_else(|| panic!("line {n}"));
                let (len, hash) = rest
                    .split_once(" bytes, fnv1a64 ")
                    .unwrap_or_else(|| panic!("line {n}: {rest}"));
                entry.push((
                    name.to_string(),
                    Stored::Hash {
                        len: len.parse().unwrap(),
                        hash: u64::from_str_radix(hash, 16).unwrap(),
                    },
                ));
            } else {
                entry.push((field.to_string(), Stored::Bytes(Vec::new())));
                open = Some((Vec::new(), false));
            }
        } else if line == "|" || line.starts_with("| ") {
            let (v, escaped) = open
                .as_mut()
                .unwrap_or_else(|| panic!("line {n}: a value line outside a field"));
            let body = line.get(2..).unwrap_or("");
            if *escaped {
                v.extend(unescape_line(body));
            } else {
                v.extend_from_slice(body.as_bytes());
            }
            v.push(b'\n');
        } else if line == "\\ no newline" {
            let (v, _) = open
                .as_mut()
                .unwrap_or_else(|| panic!("line {n}: `no newline` outside a field"));
            assert_eq!(v.pop(), Some(b'\n'), "line {n}");
        } else if line.starts_with('#') && suite.is_empty() {
            // The header.
        } else {
            panic!("line {n}: unexpected {line:?}");
        }
    }
    close(&mut suite, &mut open);
    suite
}

/// Reads an expected-output file (relative to [`dir`]).
pub fn load(name: &str) -> Suite {
    let path = dir().join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e} (FUNG_EXPECTED=bless writes it)", path.display()));
    parse(&text)
}

/// The fields of `label` in a loaded suite.
pub fn find<'a>(suite: &'a Suite, label: &str) -> Option<&'a [(String, Stored)]> {
    suite
        .iter()
        .find(|(l, _)| l == label)
        .map(|(_, f)| f.as_slice())
}

/// Where `got` differs from `want`, or `None`.
pub fn mismatch(want: &[(String, Stored)], got: &Entry) -> Option<String> {
    let names = |fs: &mut dyn Iterator<Item = &String>| fs.cloned().collect::<Vec<_>>();
    let (wn, gn) = (
        names(&mut want.iter().map(|(n, _)| n)),
        names(&mut got.fields.iter().map(|(n, _)| n)),
    );
    if wn != gn {
        return Some(format!("fields {wn:?}, got {gn:?}"));
    }
    let mut out = Vec::new();
    for ((name, w), (_, g)) in want.iter().zip(&got.fields) {
        if w.matches(g) {
            continue;
        }
        out.push(match w {
            Stored::Bytes(w) => format!("{name} {}", first_difference(w, g)),
            Stored::Hash { len, .. } => format!(
                "{name}: {len} bytes expected (hashed), got {} bytes starting {:?}",
                g.len(),
                String::from_utf8_lossy(&g[..g.len().min(200)])
            ),
        });
    }
    (!out.is_empty()).then(|| out.join("\n  "))
}

/// The first differing line of an expected and an actual value.
pub fn first_difference(want: &[u8], got: &[u8]) -> String {
    let (w, g) = (String::from_utf8_lossy(want), String::from_utf8_lossy(got));
    if w == g {
        return "same text, different bytes (invalid UTF-8)".into();
    }
    for (i, (x, y)) in w.lines().zip(g.lines()).enumerate() {
        if x != y {
            return format!("line {}:\n    want: {x}\n    got:  {y}", i + 1);
        }
    }
    format!(
        "lengths differ: want {} lines ({} bytes), got {} lines ({} bytes)",
        w.lines().count(),
        want.len(),
        g.lines().count(),
        got.len()
    )
}

/// Every entry of `got` against the file `name`: the same labels in the
/// same order and the same fields. In [`Mode::Bless`] the file is
/// rewritten from `got`. Panics listing every difference.
///
/// `limit`: a field longer than this is kept as a hash. `header`: the
/// file's opening comment (what the cases are, how to update them).
pub fn verify(name: &str, header: &str, limit: usize, got: &[Entry]) {
    let mut labels = HashMap::new();
    for (i, e) in got.iter().enumerate() {
        if let Some(j) = labels.insert(e.label.as_str(), i) {
            panic!("{name}: cases {j} and {i} share the label {:?}", e.label);
        }
    }
    if mode() == Mode::Bless {
        return write(name, header, limit, got);
    }
    let suite = load(name);
    let mut bad = Vec::new();
    let wanted: Vec<&String> = suite.iter().map(|(l, _)| l).collect();
    let have: Vec<&String> = got.iter().map(|e| &e.label).collect();
    if wanted != have {
        let missing: Vec<_> = have.iter().filter(|l| !wanted.contains(l)).collect();
        let stale: Vec<_> = wanted.iter().filter(|l| !have.contains(l)).collect();
        bad.push(format!(
            "the cases are not the file's (in order): no expected output for {missing:?}; \
             no case for {stale:?}"
        ));
    }
    for e in got {
        if let Some(want) = find(&suite, &e.label)
            && let Some(d) = mismatch(want, e)
        {
            bad.push(format!("{}: {d}", e.label));
        }
    }
    assert!(
        bad.is_empty(),
        "{name}: {} of {} cases differ from the expected output \
         (FUNG_EXPECTED=bless rewrites it after a deliberate change):\n{}",
        bad.len(),
        got.len(),
        bad.join("\n")
    );
}

fn write(name: &str, header: &str, limit: usize, entries: &[Entry]) {
    let path = dir().join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let text = render(header, entries, limit);
    // Written whole, then renamed, so a test reading it meanwhile sees
    // the old file or the new one.
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&tmp, &text).unwrap();
    std::fs::rename(&tmp, &path).unwrap();
    assert_eq!(
        parse(&text)
            .iter()
            .map(|(l, _)| l.as_str())
            .collect::<Vec<_>>(),
        entries.iter().map(|e| e.label.as_str()).collect::<Vec<_>>(),
        "{name}: the written file reads back"
    );
    for ((_, fields), e) in parse(&text).iter().zip(entries) {
        assert!(
            mismatch(fields, e).is_none(),
            "{name}: {} reads back",
            e.label
        );
    }
}
