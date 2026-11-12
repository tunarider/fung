//! `_native.arg`: each check gives nil for a good value and E0403 naming
//! the argument it was told, at the user's call (SP18).

use crate::testutil::{Opts, Outcome, exec};

fn run(src: &str) -> Outcome {
    exec(
        &crate::testutil::imports_first(&format!("import _native.arg as arg\n{src}")),
        Opts {
            internal: true,
            ..Opts::default()
        },
    )
}

#[test]
fn good_values_give_nil() {
    let o = run(
        "print(arg.positive(\"n\", 1), arg.dur(\"d\", 0s), arg.list_of(\"xs\", [], \"str\"), arg.list_of(\"xs\", [\"a\", \"b\"], \"str\"), arg.str_map(\"m\", {a: \"1\"}), arg.token(\"method\", \"propfind\"), arg.headers(\"headers\", {a: \"1\", \"X-B\": \"tab\\tok\"}))",
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str()),
        ("nil nil nil nil nil nil nil\n", "")
    );
}

/// `count` gives the count back: the prelude's `iterate` counts with it
/// (one rule for `http`'s `max_body` and the prelude's counts).
#[test]
fn count_gives_the_count() {
    let o = run("print(arg.count(\"n\", 0), arg.count(\"n\", 3))");
    assert_eq!((o.out.as_str(), o.err.as_str()), ("0 3\n", ""));
}

/// `header_problem(m)`: nil for headers that can be written, else the
/// reason the server gives for a handler's answer (500): the same rule
/// as the client's `headers` check, in one place.
#[test]
fn header_problem_gives_the_servers_reason() {
    for (m, want) in [
        (r#"{}"#, "nil"),
        (r#"{a: "1", "X-B": "tab\tok"}"#, "nil"),
        (r#"{a: 1}"#, "header `a` must be str, got int"),
        (r#"{"a b": "1", c: 2}"#, "header `c` must be str, got int"),
        (r#"{"a b": "1"}"#, "header name `a b` is not a token"),
        (r#"{"": "1"}"#, "header name `` is not a token"),
        (
            r#"{x: "a\nb"}"#,
            "header `x` has a control character in its value",
        ),
        (
            r#"{x: "a\u{7f}"}"#,
            "header `x` has a control character in its value",
        ),
        (
            r#"{"x-a": "1", b: "2", "X-A": "3"}"#,
            "header `X-A` is given twice",
        ),
        (r#"{"X-A": "1", "x-a": "3"}"#, "header `X-A` is given twice"),
    ] {
        let o = run(&format!("print(arg.header_problem({m}))"));
        assert_eq!(
            (o.out.as_str(), o.err.as_str()),
            (format!("{want}\n").as_str(), ""),
            "{m}"
        );
    }
}

#[test]
fn a_bad_value_is_e0403_naming_the_argument() {
    for (src, want) in [
        (
            "arg.count(\"limit\", -1)",
            "argument `limit` must be a non-negative int, got -1",
        ),
        (
            "arg.count(\"limit\", 1.5)",
            "argument `limit` must be int, got float",
        ),
        (
            "arg.positive(\"times\", 0)",
            "argument `times` must be a positive int, got 0",
        ),
        (
            "arg.positive(\"times\", -2)",
            "argument `times` must be a positive int, got -2",
        ),
        (
            "arg.positive(\"times\", \"2\")",
            "argument `times` must be int, got str",
        ),
        (
            "arg.dur(\"delay\", -1s)",
            "argument `delay` must be a non-negative dur, got -1s",
        ),
        (
            "arg.dur(\"delay\", 5)",
            "argument `delay` must be dur, got int",
        ),
        (
            "arg.list_of(\"bools\", [\"a\", 1], \"str\")",
            "argument `bools` must be a list of str, got a list holding int",
        ),
        (
            "arg.list_of(\"rxs\", [1], \"receiver\")",
            "argument `rxs` must be a list of receiver, got a list holding int",
        ),
        (
            "arg.headers(\"headers\", {\"content-length\": \"3\"})",
            "argument `headers` must be a map without Content-Length, Transfer-Encoding or Trailer (the request frames its body itself), got `content-length`",
        ),
        (
            "arg.headers(\"headers\", {a: \"1\", \"Transfer-Encoding\": \"chunked\"})",
            "argument `headers` must be a map without Content-Length, Transfer-Encoding or Trailer (the request frames its body itself), got `Transfer-Encoding`",
        ),
        (
            "arg.headers(\"headers\", {TRAILER: \"x\"})",
            "argument `headers` must be a map without Content-Length, Transfer-Encoding or Trailer (the request frames its body itself), got `TRAILER`",
        ),
        (
            "arg.list_of(\"argv\", \"x\", \"str\")",
            "argument `argv` must be list, got str",
        ),
        (
            "arg.str_map(\"headers\", {a: \"1\", b: 2})",
            "argument `headers` must be a map of strings, got a map holding int",
        ),
        (
            "arg.str_map(\"headers\", 1)",
            "argument `headers` must be map, got int",
        ),
        (
            "arg.token(\"method\", \"\")",
            "argument `method` must be a token (RFC 9110 5.6.2), got \"\"",
        ),
        (
            "arg.token(\"method\", \"G T\")",
            "argument `method` must be a token (RFC 9110 5.6.2), got \"G T\"",
        ),
        (
            "arg.headers(\"headers\", {\"x-a\": \"1\", b: \"2\", \"X-A\": \"3\"})",
            "argument `headers` must be a map without names that differ only in case, got `x-a` and `X-A`",
        ),
        (
            "arg.headers(\"headers\", {\"a b\": \"1\"})",
            "argument `headers` must be a map whose names are tokens (RFC 9110 5.6.2), got the name \"a b\"",
        ),
        (
            "arg.headers(\"headers\", {x: \"a\\nb\"})",
            "argument `headers` must be a map whose values have no control characters but tab, got \"a\\nb\" for `x`",
        ),
        (
            "arg.headers(\"headers\", {x: 1})",
            "argument `headers` must be a map of strings, got a map holding int",
        ),
        ("arg.count(1, 1)", "argument `name` must be str, got int"),
    ] {
        let o = run(src);
        assert!(
            o.err.starts_with(&format!("error[E0403]: {want}\n")),
            "{src}: {}",
            o.err
        );
    }
}

/// `is_token(s)`: the token rule (RFC 9110 5.6.2) that `token` and
/// `headers` check, for `http`'s server.
#[test]
fn is_token_is_the_token_rule() {
    let o = run(
        r#"print(arg.is_token("GET"), arg.is_token("x-a!#$%&'*+.^_`|~1"), arg.is_token(""), arg.is_token("a b"), arg.is_token("é"), arg.is_token("a:"))"#,
    );
    assert_eq!(
        (o.out.as_str(), o.err.as_str()),
        ("true true false false false false\n", "")
    );
}
