//! Port of internal/format/format_test.go, plus the byte-level cases the
//! Rust port adds (bytes that are not UTF-8, a byte order mark) and the
//! token-preservation safeguard.

use super::*;
use crate::lexer::lex;
use crate::source::FileId;

/// Formats `src`, insists the result is `want`, that formatting again
/// changes nothing, and that the token stream survived.
fn check(src: &str, want: &str) {
    let got = format("t.fung", src).unwrap_or_else(|e| panic!("{src:?}: {e:?}"));
    assert_eq!(got, want, "format {src:?}");
    let again = format("t.fung", &got).unwrap_or_else(|e| panic!("{got:?}: {e:?}"));
    assert_eq!(
        again, got,
        "not idempotent: {src:?} -> {got:?} -> {again:?}"
    );
    assert!(same_tokens(src, &got), "tokens changed: {src:?} -> {got:?}");
}

fn same_tokens(a: &str, b: &str) -> bool {
    let (ta, ca) = lex(FileId(0), a).unwrap();
    let (tb, cb) = lex(FileId(0), b).unwrap();
    token_texts(a, &ta) == token_texts(b, &tb) && ca.len() == cb.len()
}

fn token_texts<'a>(src: &'a str, toks: &[crate::lexer::Token]) -> Vec<&'a str> {
    toks.iter()
        .filter(|t| !matches!(t.kind, Kind::Newline | Kind::Eof))
        .map(|t| &src[t.off..t.end])
        .collect()
}

fn check_all(cases: &[(&str, &str)]) {
    for (src, want) in cases {
        check(src, want);
    }
}

#[test]
fn spacing() {
    check_all(&[
        ("a+b*c", "a + b * c\n"),
        ("x=1", "x = 1\n"),
        ("f( a,b )", "f(a, b)\n"),
        ("xs [ 0 ]", "xs[0]\n"),
        ("m [ \"k\" ]", "m[\"k\"]\n"),
        ("f(x)(y)", "f(x)(y)\n"),
        ("{ a : 1 , b:2 }", "{a: 1, b: 2}\n"),
        ("{a, b}", "{a, b}\n"),
        ("[ 1,2 ]", "[1, 2]\n"),
        ("( 1, )", "(1,)\n"),
        ("()", "()\n"),
        ("m . a . b", "m.a.b\n"),
        ("0 .. n", "0..n\n"),
        ("0..=10", "0..=10\n"),
        ("[ a, ... rest ] = xs", "[a, ...rest] = xs\n"),
        ("x=>x+1", "x => x + 1\n"),
        ("(a,b)=>a", "(a, b) => a\n"),
        ("()=>work()", "() => work()\n"),
        ("fs.read(p) !", "fs.read(p)!\n"),
        ("parse(s) ?", "parse(s)?\n"),
        ("x ?? y", "x ?? y\n"),
        ("xs|>map(f)|>sum", "xs |> map(f) |> sum\n"),
        ("- x", "-x\n"),
        ("a - b", "a - b\n"),
        ("a - -b", "a - -b\n"),
        ("f(-1, - 2)", "f(-1, -2)\n"),
        ("x = - 5s", "x = -5s\n"),
        ("return - 1", "return -1\n"),
        ("not  x", "not x\n"),
        ("if(a==b): 1 ;;", "if (a == b): 1 ;;\n"),
        ("fn add(a,b)=a+b", "fn add(a, b) = a + b\n"),
        ("fn f(x:int)->bool=true", "fn f(x: int) -> bool = true\n"),
        ("fn ( x ) : x ;;", "fn(x): x ;;\n"),
        ("each(xs, fn(x): x ;;)", "each(xs, fn(x): x ;;)\n"),
        ("exec(cmd, cwd : d)", "exec(cmd, cwd: d)\n"),
        ("match x: 0=>1;_=>2;;", "match x: 0 => 1; _ => 2 ;;\n"),
        ("match v: 1|2 => 'a ;;", "match v: 1 | 2 => 'a ;;\n"),
        ("Ok( x )", "Ok(x)\n"),
        ("c then a else b", "c then a else b\n"),
        ("x in [1, 2]", "x in [1, 2]\n"),
        ("\"a ${ x+1 } b\"", "\"a ${ x+1 } b\"\n"),
        ("`raw  text`", "`raw  text`\n"),
        ("1_000 + 0xff + 2 ** 10", "1_000 + 0xff + 2 ** 10\n"),
        ("a;b", "a; b\n"),
        ("fn f(): a ;;", "fn f(): a ;;\n"),
        ("x   \n", "x\n"),
        ("", ""),
    ]);
}

#[test]
fn rejects_syntax_errors() {
    let e = format("t.fung", "f(\n").unwrap_err();
    assert_eq!(e.code, "E0110", "a syntax error must come back untouched");
    let e = format("t.fung", "\"open").unwrap_err();
    assert_eq!(e.code, "E0104", "a lex error must come back");
}

#[test]
fn indentation() {
    check_all(&[
        // blocks
        ("fn f(x):\nx + 1\n;;\n", "fn f(x):\n  x + 1\n;;\n"),
        (
            "fn f(x):\n    if x:\n  1\n  ;;\n;;\n",
            "fn f(x):\n  if x:\n    1\n  ;;\n;;\n",
        ),
        ("if c:\na\nelse:\nb\n;;\n", "if c:\n  a\nelse:\n  b\n;;\n"),
        (
            "if c:\na\nelse if d:\nb\nelse:\nc\n;;\n",
            "if c:\n  a\nelse if d:\n  b\nelse:\n  c\n;;\n",
        ),
        (
            "test \"t\":\nassert true\n;;\n",
            "test \"t\":\n  assert true\n;;\n",
        ),
        (
            "for x in xs:\nprint(x)\n;;\n",
            "for x in xs:\n  print(x)\n;;\n",
        ),
        // then/else continuation vs if-chain else
        ("x = c then a\nelse b\n", "x = c then a\n  else b\n"),
        (
            "if c:\n  x = d then 1\n  else 2\nelse:\n  3\n;;\n",
            "if c:\n  x = d then 1\n    else 2\nelse:\n  3\n;;\n",
        ),
        // match arms and bodies
        (
            "match v:\nOk(x) =>\nx\nErr(e) => 0\n;;\n",
            "match v:\n  Ok(x)  =>\n    x\n  Err(e) => 0\n;;\n",
        ),
        (
            "match v:\n  Ok(x) => f(y => y)\n;;\n",
            "match v:\n  Ok(x) => f(y => y)\n;;\n",
        ),
        // brackets spanning lines; closers go back to the opener's line
        ("f(a,\nb,\nc)\n", "f(a,\n  b,\n  c)\n"),
        ("xs = [\n1,\n2,\n]\n", "xs = [\n  1,\n  2,\n]\n"),
        ("m = {\na: 1,\n}\n", "m = {\n  a: 1,\n}\n"),
        // multi-line lambda inside a call: body +1, `;;)` at the call's line
        (
            "each(xs, fn(x):\nprint(x)\n;;)\n",
            "each(xs, fn(x):\n  print(x)\n;;)\n",
        ),
        (
            "fn g():\neach(xs, fn(x):\nprint(x)\n;;)\n;;\n",
            "fn g():\n  each(xs, fn(x):\n    print(x)\n  ;;)\n;;\n",
        ),
        // continuation lines
        ("xs\n|> map(f)\n|> sum\n", "xs\n  |> map(f)\n  |> sum\n"),
        ("a = b\n.c\n.d\n", "a = b\n  .c\n  .d\n"),
        ("x = a\nand b\nor c\n", "x = a\n  and b\n  or c\n"),
        ("v = f(x) ?? 1\n", "v = f(x) ?? 1\n"),
        (
            "fn f():\n  xs\n  |> sum\n;;\n",
            "fn f():\n  xs\n    |> sum\n;;\n",
        ),
        // map colon and type annotation never open a block
        ("m = {a: 1}\nx = 2\n", "m = {a: 1}\nx = 2\n"),
        (
            "fn f(x: int) -> int = x\ny = 1\n",
            "fn f(x: int) -> int = x\ny = 1\n",
        ),
        (
            "fn f(a) = if a: 1 else: 2 ;;\nmatch x:\n0 => 1\n;;\n",
            "fn f(a) = if a: 1 else: 2 ;;\nmatch x:\n  0 => 1\n;;\n",
        ),
        // raw string spanning lines
        ("s = `a\n  b`\nx = 1\n", "s = `a\n  b`\nx = 1\n"),
        // one-line forms are left alone
        (
            "match x: 0 => 1; _ => 2 ;;\n",
            "match x: 0 => 1; _ => 2 ;;\n",
        ),
        ("if c: a else: b ;;\n", "if c: a else: b ;;\n"),
    ]);
}

#[test]
fn blank_lines_and_comments() {
    check_all(&[
        ("\n\nx = 1\n\n\n\ny = 2\n\n", "x = 1\n\ny = 2\n"),
        ("fn f():\n\n  x\n\n;;\n", "fn f():\n  x\n;;\n"),
        (
            "#!/usr/bin/env fung\n# about\nx = 1\n",
            "#!/usr/bin/env fung\n# about\nx = 1\n",
        ),
        ("x = 1 # one\n", "x = 1  # one\n"),
        (
            "x = 1    # one\nlong_name = 2 # two\n",
            "x = 1          # one\nlong_name = 2  # two\n",
        ),
        (
            "x = 1 # one\ny = 2\nlong = 3 # three\n",
            "x = 1  # one\ny = 2\nlong = 3  # three\n",
        ),
        (
            "fn f():\n# inside\nx\n# before end\n;;\n# after\n",
            "fn f():\n  # inside\n  x\n  # before end\n;;\n# after\n",
        ),
        (
            "match v:\nOk(x) => 1\nErr(e) => 2\n;;\n",
            "match v:\n  Ok(x)  => 1\n  Err(e) => 2\n;;\n",
        ),
        (
            "match v:\nOk(text) =>\nlen(text)\nErr(e)   => 0\n;;\n",
            "match v:\n  Ok(text) =>\n    len(text)\n  Err(e)   => 0\n;;\n",
        ),
        (
            "match v:\n  0 => a\n\n  _ => other\n;;\n",
            "match v:\n  0 => a\n\n  _ => other\n;;\n",
        ),
        ("# only a comment", "# only a comment\n"),
        (
            "print(1)\n# expect: 1\nprint(2)\n# expect: 2\n",
            "print(1)\n# expect: 1\nprint(2)\n# expect: 2\n",
        ),
        (
            "f(a, # first\n  b) # second\n",
            "f(a,  # first\n  b)  # second\n",
        ),
    ]);
}

/// Found by formatting the repository: import lists keep their space, and
/// a line ending in an operator continues on the next line.
#[test]
fn repository_findings() {
    check_all(&[
        ("import \"./x\" (a, b)\n", "import \"./x\" (a, b)\n"),
        ("import fs (read)\n", "import fs (read)\n"),
        ("import fs(read)\n", "import fs (read)\n"),
        ("fn f(x) =\nx + 1\n", "fn f(x) =\n  x + 1\n"),
        ("fn f(x) =\n  x + 1\ny = 2\n", "fn f(x) =\n  x + 1\ny = 2\n"),
        ("total = a +\nb\n", "total = a +\n  b\n"),
        ("xs |> map(x =>\nx + 1)\n", "xs |> map(x =>\n  x + 1)\n"),
        (
            "fn g():\n  v = f(x) ??\n  d\n  v\n;;\n",
            "fn g():\n  v = f(x) ??\n    d\n  v\n;;\n",
        ),
        (
            "match v:\n  Ok(t) =>\n    t\n;;\n",
            "match v:\n  Ok(t) =>\n    t\n;;\n",
        ),
    ]);
}

/// From the branch review: inputs the first version got wrong.
#[test]
fn review_findings() {
    check_all(&[
        // tokens or a comment after a multi-line raw string stay on its last line
        ("x = f(`a\nb`)\n", "x = f(`a\nb`)\n"),
        ("x = `a\nb` + \"c\"\n", "x = `a\nb` + \"c\"\n"),
        ("x = `a\nb` # note\n", "x = `a\nb`  # note\n"),
        ("xs = [`a\nb`,\n  `c`]\n", "xs = [`a\nb`,\n  `c`]\n"),
        // a lambda statement inside an arm body is not an arm (01: 절은 패턴 [when] =>)
        (
            "match a:\n  Some(v) =>\n    y = 2\n    g = x => x + v\n    g(y)\n  _ => 0\n;;\n",
            "match a:\n  Some(v) =>\n    y = 2\n    g = x => x + v\n    g(y)\n  _       => 0\n;;\n",
        ),
        (
            "match a:\n  Some(v) =>\n    return x => x\n  _ => 0\n;;\n",
            "match a:\n  Some(v) =>\n    return x => x\n  _       => 0\n;;\n",
        ),
        // an inner match aligns on its own even when an outer arm follows
        (
            "match a:\n  Some(x) =>\n    match x:\n      1 => 2\n      100 => 3\n    ;;\n  _ => 0\n;;\n",
            "match a:\n  Some(x) =>\n    match x:\n      1   => 2\n      100 => 3\n    ;;\n  _       => 0\n;;\n",
        ),
        // a body that ends with `;;` at the arm indent does not break the run
        (
            "match a:\n  Some(x) => each(xs, fn(y):\n    y\n  ;;)\n  _ => 0\n;;\n",
            "match a:\n  Some(x) => each(xs, fn(y):\n    y\n  ;;)\n  _       => 0\n;;\n",
        ),
        // `-` after a keyword used as a field name is a subtraction
        ("x = m.type - 1\n", "x = m.type - 1\n"),
        ("x = {type: - 1}\n", "x = {type: -1}\n"),
        // trailing whitespace inside a comment goes
        ("x = 1 # c   \n", "x = 1  # c\n"),
        ("# top   \n", "# top\n"),
    ]);
}

/// A `fn` type annotation inside a parameter list is not a block keyword
/// waiting for a `:` after the list closes: the body stays indented.
#[test]
fn fn_typed_parameters() {
    check_all(&[
        (
            "fn f(h: fn, x = 1):\n  y = x\n  h(y)\n;;\n",
            "fn f(h: fn, x = 1):\n  y = x\n  h(y)\n;;\n",
        ),
        ("fn f(h: fn):\nh()\n;;\n", "fn f(h: fn):\n  h()\n;;\n"),
        (
            "fn f(a: fn, b: fn, c: str):\na(b(c))\n;;\n",
            "fn f(a: fn, b: fn, c: str):\n  a(b(c))\n;;\n",
        ),
        (
            "fn f(h: fn = print):\nh(1)\n;;\n",
            "fn f(h: fn = print):\n  h(1)\n;;\n",
        ),
        (
            "fn f(h: fn, xs):\nfor x in xs:\nh(x)\n;;\n;;\n",
            "fn f(h: fn, xs):\n  for x in xs:\n    h(x)\n  ;;\n;;\n",
        ),
        (
            "fn f(h: fn):\nif h:\n1\nelse:\n2\n;;\n;;\nx=1\n",
            "fn f(h: fn):\n  if h:\n    1\n  else:\n    2\n  ;;\n;;\nx = 1\n",
        ),
        (
            "fn g(h: fn):\neach(xs, fn(x):\nh(x)\n;;)\n;;\n",
            "fn g(h: fn):\n  each(xs, fn(x):\n    h(x)\n  ;;)\n;;\n",
        ),
        (
            "fn f(h: fn, x: int) = h(x)\ny = 1\n",
            "fn f(h: fn, x: int) = h(x)\ny = 1\n",
        ),
        ("fn f() -> fn:\nprint\n;;\n", "fn f() -> fn:\n  print\n;;\n"),
        (
            "fn f(h: fn | nil, g: list[fn]):\nh\n;;\n",
            "fn f(h: fn | nil, g: list[fn]):\n  h\n;;\n",
        ),
        (
            "apply(fn(x): x ;;, fn(y):\ny\n;;)\nz = 1\n",
            "apply(fn(x): x ;;, fn(y):\n  y\n;;)\nz = 1\n",
        ),
        (
            "fn g():\nh = fn(x: fn):\nx()\n;;\nh\n;;\n",
            "fn g():\n  h = fn(x: fn):\n    x()\n  ;;\n  h\n;;\n",
        ),
    ]);
}

// ---- Rust port additions ----

/// Review Focus 1: what must survive formatting, token for token —
/// comments (own line and trailing, `#!`), blank lines between
/// statements, interpolated strings with their inner spacing, multi-line
/// raw strings with their inner indentation, and `;;` blocks.
#[test]
fn keeps_what_the_code_means() {
    check_all(&[
        (
            "#!/usr/bin/env fung\n# head\n\nx=1   # one\n\n\n# two\ny = \"a ${ x  +  1 } ${\"in ${x}\"}\"\n",
            "#!/usr/bin/env fung\n# head\n\nx = 1  # one\n\n# two\ny = \"a ${ x  +  1 } ${\"in ${x}\"}\"\n",
        ),
        (
            "s = `\n    keep   this\n\tand this  \n`\nfn f():\nif x:\n`a\n b`\n;;\n;;\n",
            "s = `\n    keep   this\n\tand this  \n`\nfn f():\n  if x:\n    `a\n b`\n  ;;\n;;\n",
        ),
        (
            "fn f(): ;;\ng = fn(): 1 ;;\nh(fn(x):\n;;)\n",
            "fn f(): ;;\ng = fn(): 1 ;;\nh(fn(x):\n;;)\n",
        ),
    ]);
}

/// The safeguard itself: an output whose tokens or comments differ from
/// the input is refused with E0801 and never returned.
#[test]
fn safeguard_refuses_a_changed_token_stream() {
    let src = b"x = 1 # c\ny = [1,\n2]\n";
    let lexed = lex_bytes(FileId(0), src).unwrap();
    let good = b"x = 1  # c\ny = [1,\n  2]\n";
    assert!(same(FileId(0), src, &lexed, good));
    for bad in [
        &b"x = 1  # d\ny = [1,\n  2]\n"[..], // a comment changed
        b"x = 1\ny = [1,\n  2]\n",           // a comment lost
        b"x = 1  # c\ny = [1, 2, 3]\n",      // a token added
        b"x = 1  # c\ny = (1,\n  2)\n",      // a token changed
        b"x = 1  # c\ny = [1,\n  2]\n\"",    // the output does not lex
    ] {
        assert!(
            !same(FileId(0), src, &lexed, bad),
            "{:?}",
            String::from_utf8_lossy(bad)
        );
    }
    // Trailing blanks of a comment and newlines are not part of the code.
    assert!(same(
        FileId(0),
        b"x # c  \n\n",
        &lex_bytes(FileId(0), b"x # c  \n\n").unwrap(),
        b"x # c\n"
    ));
}

/// Go formats bytes: a byte that is not UTF-8 inside a string or a
/// comment is written back unchanged (not as U+FFFD), and counts as one
/// character when comments are aligned.
#[test]
fn keeps_bytes_that_are_not_utf8() {
    let got = format_bytes("t.fung", b"x=\"a\xffb\" # c\xfe\nlong_name = 1 # d\n").unwrap();
    assert_eq!(
        got,
        b"x = \"a\xffb\"      # c\xfe\nlong_name = 1  # d\n".to_vec()
    );
    let got = format_bytes("t.fung", b"match v:\n\"\xff\" => 1\n\"ab\" => 2\n;;\n").unwrap();
    assert_eq!(
        got,
        b"match v:\n  \"\xff\"  => 1\n  \"ab\" => 2\n;;\n".to_vec()
    );
}

/// Go's lexer skips a byte order mark but the formatter slices the
/// untrimmed source with the trimmed offsets, so a file with a BOM and
/// code is refused by the safeguard (E0801); one with only comments loses
/// the mark.
#[test]
fn byte_order_mark_like_go() {
    let e = format("t.fung", "\u{feff}x = 1\n").unwrap_err();
    assert_eq!(e.code, "E0801");
    assert_eq!(
        crate::diag::message(&e.code, &e.args),
        "internal error: formatter changed the code of t.fung"
    );
    assert_eq!(format("t.fung", "\u{feff}# c\n").unwrap(), "# c\n");
    assert_eq!(format("t.fung", "\u{feff}").unwrap(), "");
}

/// Windows line endings become `\n`; a `\r` inside a raw string stays.
#[test]
fn windows_line_endings() {
    check(
        "x=1\r\nfn f():\r\ny # c\r\n;;\r\n",
        "x = 1\nfn f():\n  y  # c\n;;\n",
    );
}
