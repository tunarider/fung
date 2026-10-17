//! Go `path/filepath.Match` and `Glob` (Unix), ported over bytes: file
//! names need not be UTF-8, and Go's matcher reads bytes and runes the
//! same way.

/// Go `filepath.ErrBadPattern` ("syntax error in pattern").
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct BadPattern;

/// Go `utf8.RuneError`.
const RUNE_ERROR: u32 = 0xFFFD;

/// Go `filepath.Match`: whether all of `name` matches `pattern`. A
/// malformed part of the pattern is found only when matching reaches it.
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

/// Go `scanChunk`: an optional run of `*`, then everything up to the
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

/// Go `matchChunk`: whether `chunk` (no `*`) matches the start of `s`,
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

/// Go `getEsc`: one possibly escaped character of a class, and the rest
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

/// Go `utf8.DecodeRune`: the first character and its length; a byte
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

/// Go `hasMeta` (Unix): whether `Match` gives `path` any special meaning.
fn has_meta(path: &[u8]) -> bool {
    path.iter().any(|c| matches!(c, b'*' | b'?' | b'[' | b'\\'))
}

/// Go `filepath.Glob`: the names of existing files matching `pattern`,
/// each directory's matches in byte order. File system errors are
/// ignored; only a malformed pattern fails.
///
/// Go recurses on the directory part while it has meta characters; this
/// walks down the same chain first (with Go's checks at every level,
/// including its limit of 10,000), then expands it upward.
pub(crate) fn glob(pattern: &[u8]) -> Result<Vec<Vec<u8>>, BadPattern> {
    const DEPTH_LIMIT: usize = 10_000;
    let mut files = Vec::new();
    let mut p = pattern;
    let mut found = loop {
        if files.len() == DEPTH_LIMIT {
            return Err(BadPattern);
        }
        matches(p, b"")?;
        if !has_meta(p) {
            break if std::fs::symlink_metadata(os(p)).is_ok() {
                vec![p.to_vec()]
            } else {
                Vec::new()
            };
        }
        let (dir, file) = split(p);
        let dir = clean_glob_path(dir);
        if !has_meta(dir) {
            break read_matches(dir, file, Vec::new())?;
        }
        // Go issue 15879: a pattern that is its own directory.
        if dir == p {
            return Err(BadPattern);
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

/// Go `filepath.Split`: everything up to the last `/`, and the rest.
fn split(p: &[u8]) -> (&[u8], &[u8]) {
    let i = p.iter().rposition(|&c| c == b'/').map_or(0, |i| i + 1);
    p.split_at(i)
}

/// Go `cleanGlobPath`: `.` for nothing, `/` kept, else the trailing `/`
/// cut.
fn clean_glob_path(p: &[u8]) -> &[u8] {
    match p {
        b"" => b".",
        b"/" => p,
        _ => &p[..p.len() - 1],
    }
}

/// Go's `glob(dir, pattern, matches)`: the names in `dir` matching
/// `pattern`, joined onto `dir`, in byte order. A directory that cannot
/// be read adds nothing; names read before a read error still count.
fn read_matches(
    dir: &[u8],
    pattern: &[u8],
    mut out: Vec<Vec<u8>>,
) -> Result<Vec<Vec<u8>>, BadPattern> {
    if !std::fs::metadata(os(dir)).is_ok_and(|m| m.is_dir()) {
        return Ok(out);
    }
    let Ok(entries) = std::fs::read_dir(os(dir)) else {
        return Ok(out);
    };
    let mut names: Vec<Vec<u8>> = entries
        .map_while(Result::ok)
        .map(|e| std::os::unix::ffi::OsStrExt::as_bytes(e.file_name().as_os_str()).to_vec())
        .collect();
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
    use super::*;

    /// Go `matchTests` (`path/filepath/match_test.go`), plus `!`, which
    /// Go does not treat as negation.
    #[test]
    fn match_matches_go() {
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
            // Not Go's table: `!` is an ordinary character, a later bad
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
        let one = |p: &str| glob(p.as_bytes());
        let at = |rel: &str| Ok(vec![format!("{d}/{rel}").into_bytes()]);
        assert_eq!(one(&format!("{d}/*.toml")), at("Cargo.toml"));
        // A pattern with a wildcard has its directory part cleaned; one
        // without is returned as written.
        assert_eq!(one(&format!("{d}/./*.toml")), at("Cargo.toml"));
        assert_eq!(one(&format!("{d}/Cargo.toml")), at("Cargo.toml"));
        assert_eq!(one(&format!("{d}/./Cargo.toml")), at("./Cargo.toml"));
        assert_eq!(one(&format!("{d}/no-such-file")), Ok(vec![]));
        assert_eq!(
            one(&format!("{d}/src/nat*/glob.r?")),
            at("src/native/glob.rs")
        );
        assert_eq!(one(&format!("{d}/src/*/glob.rs")), at("src/native/glob.rs"));
        assert_eq!(glob(b"[").unwrap_err(), BadPattern);
        assert_eq!(glob(b"src/[/x").unwrap_err(), BadPattern);
        assert_eq!(glob(b"/").unwrap(), vec![b"/".to_vec()]);
    }
}
