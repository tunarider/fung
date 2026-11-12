//! `fs.glob` patterns, over bytes (file names need not be UTF-8): `*`
//! matches any run of characters but `/`, `?` one character, `[...]` a
//! class (`[^...]` its complement, `a-z` a range), and `\` escapes the
//! next character; `!` is an ordinary character. A malformed pattern is
//! an error before any directory is read. A directory the pattern
//! enters that cannot be read is an error with its path, as in `walk`
//! (LB18); one that is not there (or is a file) holds no match. Each
//! directory's matches come in byte order.

use std::io;

/// A malformed pattern ("syntax error in pattern").
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct BadPattern;

/// Why `glob` failed: a malformed pattern, or a path it had to look at
/// (a directory to read, a name to stat) and could not.
#[derive(Debug)]
pub(crate) enum Failure {
    Pattern,
    /// An unescaped `**` outside a class: another glob's "any depth",
    /// which this one does not have (D68; `fs.walk` does).
    DoubleStar,
    Path(Vec<u8>, io::Error),
}

impl From<BadPattern> for Failure {
    fn from(_: BadPattern) -> Failure {
        Failure::Pattern
    }
}

/// Whether a failed look at a path only says that nothing is there: the
/// name is missing, or a part of it is a file.
fn absent(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
    )
}

/// U+FFFD, what an invalid byte reads as.
const RUNE_ERROR: u32 = 0xFFFD;

/// Whether all of `name` matches `pattern`. A malformed part of the
/// pattern is found only when matching reaches it; `validate` checks it
/// all first.
pub(crate) fn matches(mut pattern: &[u8], mut name: &[u8]) -> Result<bool, BadPattern> {
    'pattern: while !pattern.is_empty() {
        let (star, chunk, rest) = scan_chunk(pattern);
        pattern = rest;
        if star && chunk.is_empty() {
            // A trailing `*` matches the rest unless it holds a `/`.
            return Ok(!name.contains(&b'/'));
        }
        // A match here, unless this is the last chunk and the name is
        // not used up: a later start may still match then.
        if let Some(t) = match_chunk(chunk, name)?
            && (t.is_empty() || !pattern.is_empty())
        {
            name = t;
            continue;
        }
        if star {
            // Skip i+1 bytes, never past a `/`.
            let mut i = 0;
            while i < name.len() && name[i] != b'/' {
                if let Some(t) = match_chunk(chunk, &name[i + 1..])? {
                    if pattern.is_empty() && !t.is_empty() {
                        i += 1;
                        continue;
                    }
                    name = t;
                    continue 'pattern;
                }
                i += 1;
            }
        }
        return Ok(false);
    }
    Ok(name.is_empty())
}

/// An optional run of `*`, then everything up to the
/// next `*` outside a character class.
fn scan_chunk(pattern: &[u8]) -> (bool, &[u8], &[u8]) {
    let mut p = pattern;
    let mut star = false;
    while let [b'*', rest @ ..] = p {
        p = rest;
        star = true;
    }
    let mut in_range = false;
    let mut i = 0;
    while i < p.len() {
        match p[i] {
            // A bad trailing `\` is matchChunk's to report.
            b'\\' if i + 1 < p.len() => i += 1,
            b'[' => in_range = true,
            b']' => in_range = false,
            b'*' if !in_range => return (star, &p[..i], &p[i..]),
            _ => {}
        }
        i += 1;
    }
    (star, p, &[])
}

/// Whether `chunk` (no `*`) matches the start of `s`,
/// and what of `s` is left. After a mismatch the rest of the chunk is
/// still read, to report a malformed pattern.
fn match_chunk<'a>(mut chunk: &[u8], mut s: &'a [u8]) -> Result<Option<&'a [u8]>, BadPattern> {
    let mut failed = false;
    while let Some(&c) = chunk.first() {
        failed = failed || s.is_empty();
        match c {
            b'[' => {
                let mut r = 0;
                if !failed {
                    let (c, n) = decode_rune(s);
                    r = c;
                    s = &s[n..];
                }
                chunk = &chunk[1..];
                let negated = chunk.first() == Some(&b'^');
                if negated {
                    chunk = &chunk[1..];
                }
                let mut matched = false;
                let mut ranges = 0;
                loop {
                    if chunk.first() == Some(&b']') && ranges > 0 {
                        chunk = &chunk[1..];
                        break;
                    }
                    let (lo, rest) = get_esc(chunk)?;
                    chunk = rest;
                    let mut hi = lo;
                    // `get_esc` leaves at least one byte.
                    if chunk[0] == b'-' {
                        (hi, chunk) = get_esc(&chunk[1..])?;
                    }
                    matched = matched || (lo <= r && r <= hi);
                    ranges += 1;
                }
                failed = failed || matched == negated;
            }
            b'?' => {
                if !failed {
                    failed = s[0] == b'/';
                    s = &s[decode_rune(s).1..];
                }
                chunk = &chunk[1..];
            }
            _ => {
                if c == b'\\' {
                    chunk = &chunk[1..];
                    if chunk.is_empty() {
                        return Err(BadPattern);
                    }
                }
                if !failed {
                    failed = chunk[0] != s[0];
                    s = &s[1..];
                }
                chunk = &chunk[1..];
            }
        }
    }
    Ok(if failed { None } else { Some(s) })
}

/// One possibly escaped character of a class, and the rest
/// of the chunk, which must not be empty.
fn get_esc(mut chunk: &[u8]) -> Result<(u32, &[u8]), BadPattern> {
    match chunk.first() {
        None | Some(b'-') | Some(b']') => return Err(BadPattern),
        Some(b'\\') => {
            chunk = &chunk[1..];
            if chunk.is_empty() {
                return Err(BadPattern);
            }
        }
        _ => {}
    }
    let (r, n) = decode_rune(chunk);
    if (r == RUNE_ERROR && n == 1) || n == chunk.len() {
        return Err(BadPattern);
    }
    Ok((r, &chunk[n..]))
}

/// The first character and its length; a byte
/// that does not start a valid sequence reads as U+FFFD of length 1.
fn decode_rune(s: &[u8]) -> (u32, usize) {
    if s.is_empty() {
        return (RUNE_ERROR, 0);
    }
    let head = &s[..s.len().min(4)];
    let valid = match std::str::from_utf8(head) {
        Ok(t) => t,
        Err(e) => std::str::from_utf8(&head[..e.valid_up_to()]).unwrap_or_default(),
    };
    match valid.chars().next() {
        Some(c) => (c as u32, c.len_utf8()),
        None => (RUNE_ERROR, 1),
    }
}

/// Whether `matches` gives `path` any special meaning.
fn has_meta(path: &[u8]) -> bool {
    path.iter().any(|c| matches!(c, b'*' | b'?' | b'[' | b'\\'))
}

/// Whether `pattern` (a valid one) holds two `*` in a row that are
/// neither escaped nor in a class.
fn double_star(pattern: &[u8]) -> bool {
    let mut i = 0;
    let mut class = false;
    let mut star = false;
    while i < pattern.len() {
        let c = pattern[i];
        match c {
            b'\\' => {
                star = false;
                i += 2;
                continue;
            }
            b'[' if !class => {
                class = true;
                // `]` right after `[` or `[^` is a member, not the end.
                if pattern.get(i + 1) == Some(&b'^') {
                    i += 1;
                }
                if pattern.get(i + 1) == Some(&b']') {
                    i += 1;
                }
            }
            b']' if class => class = false,
            b'*' if !class => {
                if star {
                    return true;
                }
                star = true;
                i += 1;
                continue;
            }
            _ => {}
        }
        star = false;
        i += 1;
    }
    false
}

/// Checks the whole pattern, every chunk of it, without matching.
pub(crate) fn validate(mut pattern: &[u8]) -> Result<(), BadPattern> {
    while !pattern.is_empty() {
        let (_, chunk, rest) = scan_chunk(pattern);
        match_chunk(chunk, b"")?;
        pattern = rest;
    }
    Ok(())
}

/// The names of existing files matching `pattern`, each directory's
/// matches in byte order. A malformed pattern fails, and so does a path
/// that cannot be looked at for another reason than its absence (a
/// directory without read permission, say).
///
/// The directory part may hold patterns too: this walks down the chain of
/// directory parts first, then expands it upward.
pub(crate) fn glob(pattern: &[u8]) -> Result<Vec<Vec<u8>>, Failure> {
    validate(pattern)?;
    if double_star(pattern) {
        return Err(Failure::DoubleStar);
    }
    let mut files = Vec::new();
    let mut p = pattern;
    let mut found = loop {
        if !has_meta(p) {
            break match std::fs::symlink_metadata(os(p)) {
                Ok(_) => vec![p.to_vec()],
                Err(e) if absent(&e) => Vec::new(),
                Err(e) => return Err(Failure::Path(p.to_vec(), e)),
            };
        }
        let (dir, file) = split(p);
        let dir = clean_glob_path(dir);
        if !has_meta(dir) {
            break read_matches(dir, file, Vec::new())?;
        }
        // A pattern that is its own directory would never end.
        if dir == p {
            return Err(Failure::Pattern);
        }
        files.push(file);
        p = dir;
    };
    for file in files.iter().rev() {
        let mut next = Vec::new();
        for dir in &found {
            next = read_matches(dir, file, next)?;
        }
        found = next;
    }
    Ok(found)
}

fn os(p: &[u8]) -> &std::ffi::OsStr {
    std::os::unix::ffi::OsStrExt::from_bytes(p)
}

/// Everything up to the last `/`, and the rest.
fn split(p: &[u8]) -> (&[u8], &[u8]) {
    let i = p.iter().rposition(|&c| c == b'/').map_or(0, |i| i + 1);
    p.split_at(i)
}

/// The directory part as `glob` reads it: `.` for nothing, `/` kept, else the trailing `/`
/// cut.
fn clean_glob_path(p: &[u8]) -> &[u8] {
    match p {
        b"" => b".",
        b"/" => p,
        _ => &p[..p.len() - 1],
    }
}

/// The names in `dir` matching
/// `pattern`, joined onto `dir`, in byte order. A `dir` that is not there
/// or is no directory adds nothing; one that cannot be read is an error
/// with its path.
fn read_matches(
    dir: &[u8],
    pattern: &[u8],
    mut out: Vec<Vec<u8>>,
) -> Result<Vec<Vec<u8>>, Failure> {
    let failed = |e: io::Error| Failure::Path(dir.to_vec(), e);
    match std::fs::metadata(os(dir)) {
        Ok(m) if m.is_dir() => {}
        Ok(_) => return Ok(out),
        Err(e) if absent(&e) => return Ok(out),
        Err(e) => return Err(failed(e)),
    }
    let mut names: Vec<Vec<u8>> = Vec::new();
    for e in std::fs::read_dir(os(dir)).map_err(failed)? {
        let name = e.map_err(failed)?.file_name();
        names.push(std::os::unix::ffi::OsStrExt::as_bytes(name.as_os_str()).to_vec());
    }
    names.sort_unstable();
    for name in names {
        if matches(pattern, &name)? {
            out.push(super::path::join_bytes(dir, &name));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{BadPattern, Failure, glob, validate};

    #[test]
    fn validate_checks_the_whole_pattern() {
        for bad in [&b"a[b-"[..], b"\\", b"[]", b"x*y[", b"*[a"] {
            assert_eq!(
                validate(bad),
                Err(BadPattern),
                "{:?}",
                String::from_utf8_lossy(bad)
            );
        }
        for good in [&b"a\\*b"[..], b"[!a]", b"*.txt", b"[^a-c]?", b""] {
            assert_eq!(
                validate(good),
                Ok(()),
                "{:?}",
                String::from_utf8_lossy(good)
            );
        }
    }

    /// `**` is no "any depth" here, so it is refused rather than read as
    /// `*` (D68); escaped or in a class it is two ordinary stars.
    #[test]
    fn a_double_star_is_refused() {
        for bad in [&b"**/*.txt"[..], b"a/**", b"x**y", b"***"] {
            assert!(
                matches!(glob(bad), Err(Failure::DoubleStar)),
                "{:?}",
                String::from_utf8_lossy(bad)
            );
        }
        for good in [&b"\\**"[..], b"*\\*", b"[*]*", b"[]*]*", b"*/*"] {
            assert!(
                !super::double_star(good),
                "{:?}",
                String::from_utf8_lossy(good)
            );
        }
    }

    /// A malformed pattern fails whatever the directory holds.
    #[test]
    fn bad_pattern_fails_without_matching_files() {
        let dir = std::env::temp_dir().join(format!("fung-glob-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pattern = format!("{}/a*[", dir.display());
        let got = glob(pattern.as_bytes());
        let _ = std::fs::remove_dir_all(&dir);
        assert!(matches!(got, Err(Failure::Pattern)), "{got:?}");
    }

    use super::*;

    /// The matcher's cases, `!` as an ordinary character included.
    #[test]
    fn match_cases() {
        let cases: &[(&str, &str, Result<bool, BadPattern>)] = &[
            ("abc", "abc", Ok(true)),
            ("*", "abc", Ok(true)),
            ("*c", "abc", Ok(true)),
            ("a*", "a", Ok(true)),
            ("a*", "abc", Ok(true)),
            ("a*", "ab/c", Ok(false)),
            ("a*/b", "abc/b", Ok(true)),
            ("a*/b", "a/c/b", Ok(false)),
            ("a*b*c*d*e*/f", "axbxcxdxe/f", Ok(true)),
            ("a*b*c*d*e*/f", "axbxcxdxexxx/f", Ok(true)),
            ("a*b*c*d*e*/f", "axbxcxdxe/xxx/f", Ok(false)),
            ("a*b*c*d*e*/f", "axbxcxdxexxx/fff", Ok(false)),
            ("a*b?c*x", "abxbbxdbxebxczzx", Ok(true)),
            ("a*b?c*x", "abxbbxdbxebxczzy", Ok(false)),
            ("ab[c]", "abc", Ok(true)),
            ("ab[b-d]", "abc", Ok(true)),
            ("ab[e-g]", "abc", Ok(false)),
            ("ab[^c]", "abc", Ok(false)),
            ("ab[^b-d]", "abc", Ok(false)),
            ("ab[^e-g]", "abc", Ok(true)),
            ("a\\*b", "a*b", Ok(true)),
            ("a\\*b", "ab", Ok(false)),
            ("a?b", "a☺b", Ok(true)),
            ("a[^a]b", "a☺b", Ok(true)),
            ("a???b", "a☺b", Ok(false)),
            ("a[^a][^a][^a]b", "a☺b", Ok(false)),
            ("[a-ζ]*", "α", Ok(true)),
            ("*[a-ζ]", "A", Ok(false)),
            ("a?b", "a/b", Ok(false)),
            ("a*b", "a/b", Ok(false)),
            ("[\\]a]", "]", Ok(true)),
            ("[\\-]", "-", Ok(true)),
            ("[x\\-]", "x", Ok(true)),
            ("[x\\-]", "-", Ok(true)),
            ("[x\\-]", "z", Ok(false)),
            ("[\\-x]", "x", Ok(true)),
            ("[\\-x]", "-", Ok(true)),
            ("[\\-x]", "a", Ok(false)),
            ("[]a]", "]", Err(BadPattern)),
            ("[-]", "-", Err(BadPattern)),
            ("[x-]", "x", Err(BadPattern)),
            ("[x-]", "-", Err(BadPattern)),
            ("[x-]", "z", Err(BadPattern)),
            ("[-x]", "x", Err(BadPattern)),
            ("[-x]", "-", Err(BadPattern)),
            ("[-x]", "a", Err(BadPattern)),
            ("\\", "a", Err(BadPattern)),
            ("[a-b-c]", "a", Err(BadPattern)),
            ("[", "a", Err(BadPattern)),
            ("[^", "a", Err(BadPattern)),
            ("[^bc", "a", Err(BadPattern)),
            ("a[", "a", Err(BadPattern)),
            ("a[", "ab", Err(BadPattern)),
            ("a[", "x", Err(BadPattern)),
            ("a/b[", "x", Err(BadPattern)),
            ("*x", "xxx", Ok(true)),
            // `!` is an ordinary character, a later bad
            // chunk is found only when the name reaches it, and a name
            // that is not UTF-8 reads one U+FFFD per bad byte.
            ("[!a]", "!", Ok(true)),
            ("[!a]", "b", Ok(false)),
            ("a*[", "", Ok(false)),
            ("a*[", "ab", Err(BadPattern)),
            ("*", "", Ok(true)),
            ("", "", Ok(true)),
            ("", "a", Ok(false)),
            ("a\\", "a", Err(BadPattern)),
            ("\\a", "a", Ok(true)),
            ("[\\a-\\c]", "b", Ok(true)),
            (".*", ".hidden", Ok(true)),
            ("*", ".hidden", Ok(true)),
            ("[A-Z]*", "a", Ok(false)),
        ];
        for (pattern, name, want) in cases {
            assert_eq!(
                &matches(pattern.as_bytes(), name.as_bytes()),
                want,
                "Match({pattern:?}, {name:?})"
            );
        }
        assert_eq!(matches(b"a?b", b"a\xffb"), Ok(true));
        assert_eq!(matches(b"a??b", b"a\xff\xfeb"), Ok(true));
        assert_eq!(matches("a[\u{fffd}]b".as_bytes(), b"a\xffb"), Ok(true));
        assert_eq!(matches(b"a*", b"a\xff/"), Ok(false));
    }

    #[test]
    fn glob_reads_the_file_system() {
        // Absolute patterns: independent of the working directory.
        let d = env!("CARGO_MANIFEST_DIR");
        let one = |p: &str| glob(p.as_bytes()).ok();
        let at = |rel: &str| Some(vec![format!("{d}/{rel}").into_bytes()]);
        assert_eq!(one(&format!("{d}/*.toml")), at("Cargo.toml"));
        // A pattern with a wildcard has its directory part cleaned; one
        // without is returned as written.
        assert_eq!(one(&format!("{d}/./*.toml")), at("Cargo.toml"));
        assert_eq!(one(&format!("{d}/Cargo.toml")), at("Cargo.toml"));
        assert_eq!(one(&format!("{d}/./Cargo.toml")), at("./Cargo.toml"));
        assert_eq!(one(&format!("{d}/no-such-file")), Some(vec![]));
        assert_eq!(one(&format!("{d}/Cargo.toml/*")), Some(vec![]));
        assert_eq!(
            one(&format!("{d}/src/nat*/glob.r?")),
            at("src/native/glob.rs")
        );
        assert_eq!(one(&format!("{d}/src/*/glob.rs")), at("src/native/glob.rs"));
        assert!(matches!(glob(b"["), Err(Failure::Pattern)));
        assert!(matches!(glob(b"src/[/x"), Err(Failure::Pattern)));
        assert_eq!(glob(b"/").unwrap(), vec![b"/".to_vec()]);
    }
}
