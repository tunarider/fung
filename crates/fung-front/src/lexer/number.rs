//! Number literals: integers, floats and durations.

use super::{Kind, Lexer, is_digit, is_ident_part, is_ident_start, is_lower_hex_digit};
use crate::diag::{Arg, Diag};
use crate::source::Pos;

/// Largest value an `i64` nanosecond count can hold, as a float (2^63).
const I64_LIMIT: f64 = 9_223_372_036_854_775_808.0;

/// Duration units: (text, descending rank, nanoseconds).
fn duration_unit(s: &str) -> Option<(u8, i64)> {
    match s {
        "h" => Some((3, 3_600_000_000_000)),
        "m" => Some((2, 60_000_000_000)),
        "s" => Some((1, 1_000_000_000)),
        "ms" => Some((0, 1_000_000)),
        _ => None,
    }
}

/// The help of E0135 for an int literal (FE61).
pub(crate) const INT_RANGE: &str = "an int holds -9223372036854775808 to 9223372036854775807";

/// The help of E0135 for a float literal too large.
const FLOAT_RANGE: &str = "a float holds up to about 1.8e308";

impl Lexer<'_> {
    /// Lexes integers (decimal, 0x, 0o, 0b, with `_` separators), floats
    /// (digits on both sides of the point, optional exponent) and durations
    /// (a number immediately followed by ms/s/m/h, possibly compound: 1h30m).
    pub(super) fn number(&mut self) -> Result<(), Diag> {
        // The error underlines the number it shows (FE61).
        self.number_inner().map_err(|mut d| {
            if let (0, Some(Arg::Str(text))) = (d.len, d.args.first()) {
                d.len = text.chars().count();
            }
            d
        })
    }

    fn number_inner(&mut self) -> Result<(), Diag> {
        let p = self.pos();
        let start = self.off;
        self.number_token(p, start)?;
        // A number has no fields (FE2, FE56): `1.5.foo` and `5s.foo` are
        // refused like `1.foo`.
        if self.peek() == Some('.') && is_ident_start(self.peek_at(1)) {
            self.advance();
            self.ident_text();
            return Err(self.err("E0102", p, &self.src[start..self.off]));
        }
        Ok(())
    }

    fn number_token(&mut self, p: Pos, start: usize) -> Result<(), Diag> {
        if self.peek() == Some('0') && matches!(self.peek_at(1), Some('x' | 'o' | 'b')) {
            return self.prefixed_int(p, start);
        }
        // One way to write a number (SP27): `0XFF` and `0O17` (an `O` next
        // to a `0`) are refused with the lowercase form as help.
        if self.peek() == Some('0') && matches!(self.peek_at(1), Some('X' | 'O' | 'B')) {
            self.advance();
            self.ident_text();
            let raw = &self.src[start..self.off];
            // the digits too, for hexadecimal (D56)
            let fixed = if raw.as_bytes()[1] == b'X' {
                raw.to_ascii_lowercase()
            } else {
                format!("0{}{}", raw[1..2].to_ascii_lowercase(), &raw[2..])
            };
            let help = format!("write the base prefix in lowercase: `{fixed}`");
            return Err(suggest(self.err("E0102", p, raw), &fixed, &help));
        }

        if !self.digits() {
            return Err(self.err("E0102", p, &self.src[start..self.off]));
        }
        // `0755` is not octal (FE1): a leading zero is refused rather than
        // read as decimal, so a shell-style file mode cannot go wrong.
        let whole = &self.src[start..self.off];
        if whole.starts_with('0') && whole.bytes().filter(u8::is_ascii_digit).count() > 1 {
            while is_digit(self.peek()) || is_ident_part(self.peek()) || self.peek() == Some('.') {
                self.advance();
            }
            // Only an integer has an octal form; a float or a duration
            // (`01.5`, `05s`, `01e3`) just loses its zeros.
            let written = &self.src[start..self.off];
            let help = if written.bytes().all(|b| b.is_ascii_digit() || b == b'_') {
                "a decimal number does not start with 0: write `0o755` for octal, or drop the zeros"
            } else {
                "a number does not start with 0: drop the leading zeros"
            };
            return Err(self.err("E0102", p, written).with_help(help));
        }
        // `1.` is not a number (01), and an integer has no fields: a `.`
        // right after the digits that starts neither a fraction nor a
        // range is refused here, so `x = 1.` cannot swallow the next
        // line's name as a field (FE2).
        if self.peek() == Some('.') && !matches!(self.peek_at(1), Some('0'..='9' | '.')) {
            self.advance();
            self.ident_text();
            return Err(self.err("E0102", p, &self.src[start..self.off]));
        }
        let mut is_float = false;
        if self.peek() == Some('.') && is_digit(self.peek_at(1)) {
            self.advance();
            is_float = true;
            if !self.digits() {
                return Err(self.err("E0102", p, &self.src[start..self.off]));
            }
        }
        let mut has_exp = false;
        if self.peek() == Some('E') && self.exponent_ahead() {
            let mant = self.off;
            self.advance();
            if matches!(self.peek(), Some('+' | '-')) {
                self.advance();
            }
            self.digits();
            self.ident_text();
            let raw = &self.src[start..self.off];
            let fixed = format!(
                "{}e{}",
                &self.src[start..mant],
                &self.src[mant + 1..self.off]
            );
            let help = format!("write the exponent with a lowercase `e`: `{fixed}`");
            return Err(suggest(self.err("E0102", p, raw), &fixed, &help));
        }
        if self.peek() == Some('e') && self.exponent_ahead() {
            self.advance();
            if matches!(self.peek(), Some('+' | '-')) {
                self.advance();
            }
            if !self.digits() {
                return Err(self.err("E0102", p, &self.src[start..self.off]));
            }
            is_float = true;
            has_exp = true;
        }

        // A letter right after the number means either a duration unit or junk.
        if is_ident_start(self.peek()) {
            if has_exp {
                let head = &self.src[start..self.off];
                let msg = format!("{}{}", head, self.ident_text());
                return Err(self.err("E0102", p, &msg));
            }
            return self.duration(p, start);
        }

        let raw = &self.src[start..self.off];
        let text: String = raw.chars().filter(|&c| c != '_').collect();
        if is_float {
            return match text.parse::<f64>() {
                // A literal that is not zero but reads as 0.0 lost its
                // value (FE31): refused like an overflow.
                Ok(f) if f == 0.0 && mantissa_is_nonzero(&text) => Err(self
                    .err("E0135", p, raw)
                    .with_help("the number is too small for a float; write `0.0` for zero")),
                // an overflow parses as infinity: out of range (FE61)
                Ok(f) if f.is_finite() => {
                    let raw = raw.to_string();
                    self.emit(Kind::Float, &raw, p).float = f;
                    Ok(())
                }
                _ => Err(self.err("E0135", p, raw).with_help(FLOAT_RANGE)),
            };
        }
        match text.parse::<i64>() {
            Ok(n) => {
                let raw = raw.to_string();
                self.emit(Kind::Int, &raw, p).int = n;
                Ok(())
            }
            Err(_) if text == "9223372036854775808" => {
                // The magnitude of the smallest int. Only the parser can tell
                // whether a unary minus precedes it.
                let raw = raw.to_string();
                self.emit(Kind::Int, &raw, p).int = i64::MIN;
                Ok(())
            }
            // the digits are fine: the value does not fit (FE61)
            Err(_) => Err(self.err("E0135", p, raw).with_help(INT_RANGE)),
        }
    }

    /// Whether the `e`/`E` at the cursor starts an exponent: a digit, or a
    /// sign and a digit, follows.
    fn exponent_ahead(&self) -> bool {
        let next = self.peek_at(1);
        is_digit(next) || (matches!(next, Some('+' | '-')) && is_digit(self.peek_at(2)))
    }

    /// Consumes `digit (digit | _ digit)*` and reports whether at least one
    /// digit was read and no `_` was left dangling or doubled.
    fn digits(&mut self) -> bool {
        if !is_digit(self.peek()) {
            return false;
        }
        loop {
            self.advance();
            if self.peek() == Some('_') {
                if !is_digit(self.peek_at(1)) {
                    self.advance(); // consume so the error text shows it
                    return false;
                }
                self.advance();
                continue;
            }
            if !is_digit(self.peek()) {
                return true;
            }
        }
    }

    fn prefixed_int(&mut self, p: Pos, start: usize) -> Result<(), Diag> {
        self.advance(); // 0
        let base = match self.advance() {
            'o' => 8,
            'b' => 2,
            _ => 16,
        };
        let dig_start = self.off;
        // `_` only between digits (01): not right after the prefix
        while is_lower_hex_digit(self.peek())
            || (self.peek() == Some('_')
                && self.off > dig_start
                && is_lower_hex_digit(self.peek_at(1)))
        {
            self.advance();
        }
        if self.off == dig_start || is_ident_start(self.peek()) || is_digit(self.peek()) {
            self.ident_text();
            let raw = &self.src[start..self.off];
            let d = self.err("E0102", p, raw);
            // `0xFF`: the digits are lowercase too (D56)
            if base == 16 && raw.bytes().any(|b| b.is_ascii_uppercase()) {
                let fixed = raw.to_ascii_lowercase();
                let help = format!("write the hexadecimal digits in lowercase: `{fixed}`");
                return Err(suggest(d, &fixed, &help));
            }
            return Err(d);
        }
        let text: String = self.src[dig_start..self.off]
            .chars()
            .filter(|&c| c != '_')
            .collect();
        // The magnitude of the least int is a token in every base, as in
        // decimal: only a preceding minus makes it valid (FE31).
        match u64::from_str_radix(&text, base) {
            Ok(n) if n <= i64::MIN.unsigned_abs() => {
                let raw = self.src[start..self.off].to_string();
                self.emit(Kind::Int, &raw, p).int = n as i64;
                Ok(())
            }
            _ => Err(self
                .err("E0135", p, &self.src[start..self.off])
                .with_help(INT_RANGE)),
        }
    }

    /// Consumes identifier characters and returns them (used to make error
    /// messages show the whole bad token, e.g. "30abc").
    pub(super) fn ident_text(&mut self) -> String {
        let start = self.off;
        while is_ident_part(self.peek()) {
            self.advance();
        }
        self.src[start..self.off].to_string()
    }

    /// Continues after the first numeric component has been consumed (`off`
    /// points at its unit letters). Components must use descending units:
    /// 1h30m is fine, 30m1h is not. A first component whose suffix is not a
    /// unit at all (30abc, 5x) is a malformed number, not a duration.
    fn duration(&mut self, p: Pos, start: usize) -> Result<(), Diag> {
        let mut total: i64 = 0;
        let mut prev_rank = 4u8;
        let mut comp_start = start;
        let mut first = true;
        loop {
            let num_text: String = self.src[comp_start..self.off]
                .chars()
                .filter(|&c| c != '_')
                .collect();
            // Every part follows the integer rule (FE31): `1h05m` is
            // refused like `05m`.
            let whole = num_text.split('.').next().unwrap_or("");
            if !first && whole.len() > 1 && whole.starts_with('0') {
                return Err(self
                    .bad_number("E0105", p, start)
                    .with_help("a duration part does not start with 0: drop the zeros"));
            }
            let fraction = num_text.contains('.');
            let unit_start = self.off;
            while matches!(self.peek(), Some('a'..='z')) {
                self.advance();
            }
            let unit = duration_unit(&self.src[unit_start..self.off]);
            let Some((rank, unit_ns)) = unit else {
                return Err(self.bad_number(if first { "E0102" } else { "E0105" }, p, start));
            };
            if rank >= prev_rank {
                return Err(self.bad_number("E0105", p, start));
            }
            let Ok(f) = num_text.parse::<f64>() else {
                return Err(self.bad_number("E0105", p, start));
            };
            // Round to the nearest nanosecond (4.35h is exactly 15660s, but
            // the float product lands one ns short) and refuse anything the
            // i64 duration cannot hold: an out-of-range float to int
            // conversion must never be relied on.
            let ns = (f * unit_ns as f64).round();
            // A part that is not zero must not round to 0 ns (FE31).
            if f != 0.0 && ns == 0.0 {
                return Err(self.bad_number("E0105", p, start).with_help(
                    "a duration is counted in whole nanoseconds; this part rounds to 0",
                ));
            }
            if ns >= I64_LIMIT || ns > (i64::MAX - total) as f64 {
                return Err(self.bad_number("E0105", p, start));
            }
            let Some(sum) = total.checked_add(ns as i64) else {
                return Err(self.bad_number("E0105", p, start));
            };
            total = sum;
            prev_rank = rank;
            first = false;
            if !is_digit(self.peek()) {
                break;
            }
            // Only the last part has a fraction (FE31), so a part reads
            // the same wherever it stands: `1.5h30m` is refused.
            if fraction {
                return Err(self
                    .bad_number("E0105", p, start)
                    .with_help("only the last part of a duration has a fraction: write `1h30.5m`, not `1.5h30m`"));
            }
            comp_start = self.off;
            if !self.digits() {
                return Err(self.bad_number("E0105", p, start));
            }
            if self.peek() == Some('.') && is_digit(self.peek_at(1)) {
                self.advance();
                if !self.digits() {
                    return Err(self.bad_number("E0105", p, start));
                }
            }
            if !is_ident_start(self.peek()) {
                return Err(self.bad_number("E0105", p, start));
            }
        }
        if is_ident_part(self.peek()) {
            return Err(self.bad_number("E0105", p, start));
        }
        let raw = self.src[start..self.off].to_string();
        self.emit(Kind::Duration, &raw, p).dur = total;
        Ok(())
    }

    /// Builds a malformed-number error that shows the whole bad token.
    fn bad_number(&mut self, code: &str, p: Pos, start: usize) -> Diag {
        self.ident_text();
        self.err(code, p, &self.src[start..self.off])
    }
}

/// Adds `help` to `d` only when `fixed`, the suggested spelling, lexes as
/// one number: `0XG` gets no `0xG` to copy.
fn suggest(d: Diag, fixed: &str, help: &str) -> Diag {
    let lexes = super::lex(crate::source::FileId(0), fixed)
        .is_ok_and(|(toks, _)| toks.len() == 2 && matches!(toks[0].kind, Kind::Int | Kind::Float));
    if lexes { d.with_help(help) } else { d }
}

/// Whether a float literal's digits before any exponent hold a non-zero
/// digit: such a literal is not zero even when it reads as 0.0.
fn mantissa_is_nonzero(text: &str) -> bool {
    text.split(['e', 'E'])
        .next()
        .is_some_and(|m| m.bytes().any(|b| matches!(b, b'1'..=b'9')))
}

#[cfg(test)]
mod tests {
    use crate::lexer::tests::lex_src;
    use crate::lexer::{Kind, Token};

    fn one(src: &str) -> Token {
        let (mut toks, _) = lex_src(src).unwrap_or_else(|d| panic!("lex {src:?}: {}", d.code));
        assert_eq!(toks.len(), 2, "{src:?} should be one token + EOF");
        toks.remove(0)
    }

    fn kinds(src: &str) -> Vec<Kind> {
        let (toks, _) = lex_src(src).unwrap();
        toks[..toks.len() - 1].iter().map(|t| t.kind).collect()
    }

    const MS: i64 = 1_000_000;
    const S: i64 = 1000 * MS;
    const M: i64 = 60 * S;
    const H: i64 = 60 * M;

    #[test]
    fn integer_literals() {
        let cases: [(&str, i64); 8] = [
            ("0", 0),
            ("42", 42),
            ("1_000_000", 1000000),
            ("0xff", 255),
            ("0o17", 15),
            ("0b1010", 10),
            ("0b1_0", 2),
            ("9223372036854775807", i64::MAX),
        ];
        for (src, want) in cases {
            let t = one(src);
            assert_eq!((t.kind, t.int), (Kind::Int, want), "{src}");
        }
    }

    #[test]
    fn float_literals() {
        let cases: [(&str, f64); 6] = [
            ("2.75", 2.75),
            ("1e9", 1e9),
            ("2.5e-3", 2.5e-3),
            ("1e+2", 100.0),
            ("1_0.5", 10.5),
            ("0.1", 0.1),
        ];
        for (src, want) in cases {
            let t = one(src);
            assert_eq!(t.kind, Kind::Float, "{src}");
            assert_eq!(t.float, want, "{src}");
        }
    }

    #[test]
    fn duration_literals() {
        let cases: [(&str, i64); 6] = [
            ("500ms", 500 * MS),
            ("30s", 30 * S),
            ("1.5s", 1500 * MS),
            ("1h30m", 90 * M),
            ("2h", 2 * H),
            ("1m30s500ms", M + 30 * S + 500 * MS),
        ];
        for (src, want) in cases {
            let t = one(src);
            assert_eq!((t.kind, t.dur), (Kind::Duration, want), "{src}");
        }
    }

    #[test]
    fn number_followed_by_operator_or_range() {
        use Kind::*;
        assert_eq!(kinds("1..3"), vec![Int, DotDot, Int]);
        assert_eq!(kinds("1.5.3"), vec![Float, Dot, Int]);
        assert_eq!(kinds("30 s"), vec![Int, Name]);
        assert_eq!(kinds("1+2"), vec![Int, Plus, Int]);
    }

    #[test]
    fn malformed_numbers() {
        let cases = [
            ("30abc", "E0102"),
            ("1__0", "E0102"),
            ("1_", "E0102"),
            ("0x", "E0102"),
            ("0xg", "E0102"),
            ("9223372036854775809", "E0135"),
            ("99999999999999999999", "E0135"),
            ("1e400", "E0135"),
            ("1e", "E0102"),
            ("1h30", "E0105"),
            ("30m1h", "E0105"),
            ("5x", "E0102"),
            ("1e5s", "E0102"),
            // `_` only between digits (01), also right after a base prefix
            // and in a duration's fraction
            ("0x_ff", "E0102"),
            ("0o_7", "E0102"),
            ("0b_1", "E0102"),
            ("0xf__f", "E0102"),
            ("1h30.5_m", "E0105"),
            ("1.5_s", "E0102"),
            // an integer has no fields: `1.` is not a number (01, FE2)
            ("1.", "E0102"),
            ("1.\nprint(x)", "E0102"),
            ("1.e5", "E0102"),
            ("1.foo", "E0102"),
            ("1.5.foo", "E0102"),
            ("1e5.foo", "E0102"),
            ("5s.foo", "E0102"),
            // lowercase prefixes and exponents only (SP27, D32)
            ("0XFF", "E0102"),
            ("0O17", "E0102"),
            ("0B1", "E0102"),
            ("1E5", "E0102"),
            ("2.5E-3", "E0102"),
            // every duration part follows the leading-zero rule, and only
            // the last one has a fraction (FE31, D9)
            ("1h05m", "E0105"),
            ("1h007m", "E0105"),
            ("1m00s", "E0105"),
            ("1.5h30m", "E0105"),
            // a non-zero literal that rounds to zero (FE31, D9)
            ("0.0000004ms", "E0105"),
            ("1e-400", "E0135"),
            ("0.1e-330", "E0135"),
        ];
        for (src, want) in cases {
            match lex_src(src) {
                Err(d) => assert_eq!(d.code, want, "{src}"),
                Ok(_) => panic!("{src}: expected {want}"),
            }
        }
    }

    #[test]
    fn malformed_number_error_shows_the_whole_token() {
        use crate::diag::Arg;
        let d = lex_src("30abc").unwrap_err();
        assert_eq!(d.args, vec![Arg::Str("30abc".to_string())]);
        let d = lex_src("1.e5").unwrap_err();
        assert_eq!(d.args, vec![Arg::Str("1.e5".to_string())]);
        let d = lex_src("1h30").unwrap_err();
        assert_eq!(d.args, vec![Arg::Str("1h30".to_string())]);
    }

    /// FE61: a number too large for its type is out of range, not
    /// malformed, and the whole number is underlined.
    #[test]
    fn a_number_out_of_range_says_so() {
        use crate::diag::{Arg, message};
        for (src, help) in [
            ("99999999999999999999", "an int holds"),
            ("0xffffffffffffffffff", "an int holds"),
            ("1e400", "a float holds"),
            ("1e-400", "too small"),
        ] {
            let d = lex_src(src).unwrap_err();
            assert_eq!(d.code, "E0135", "{src}");
            assert_eq!(d.args, vec![Arg::Str(src.to_string())]);
            assert_eq!(d.len, src.len(), "{src}");
            assert!(
                d.help.clone().unwrap_or_default().contains(help),
                "{src}: {:?}",
                d.help
            );
            assert_eq!(
                message(&d.code, &d.args),
                format!("number \"{src}\" is out of range")
            );
        }
        // a malformed number is underlined whole too
        assert_eq!(lex_src("30abc").unwrap_err().len, 5);
    }

    #[test]
    fn uppercase_prefixes_suggest_the_lowercase_form() {
        for (src, help) in [
            ("0XFF", "`0xff`"),
            ("0xFF", "`0xff`"),
            ("0xaB_cD", "`0xab_cd`"),
            ("0O17", "`0o17`"),
            ("0B1", "`0b1`"),
            ("1E5", "`1e5`"),
        ] {
            let d = lex_src(src).unwrap_err();
            let h = d.help.clone().unwrap_or_default();
            assert!(h.contains(help), "{src}: {h}");
        }
    }

    #[test]
    fn a_lowercase_form_that_would_not_lex_is_not_suggested() {
        for src in ["0XG", "0Bz", "1E5x"] {
            let d = lex_src(src).unwrap_err();
            assert_eq!(d.code, "E0102", "{src}");
            assert_eq!(d.help, None, "{src}");
        }
    }

    #[test]
    fn zero_literals_and_last_part_fractions_are_fine() {
        assert_eq!(one("0.0ms").dur, 0);
        assert_eq!(one("1h0m").dur, H);
        assert_eq!(one("1h30.5m").dur, 90 * M + 30 * S);
        assert_eq!(one("0.000001ms").dur, 1);
        assert_eq!(one("0e5").float, 0.0);
        assert_eq!(one("5e-324").float, 5e-324);
    }

    #[test]
    fn duration_rounds_to_nearest_nanosecond() {
        // 4.35h = 15660s exactly; naive float multiplication lands one ns short.
        assert_eq!(one("4.35h").dur, 4 * H + 21 * M);
    }

    #[test]
    fn duration_overflow_is_an_error() {
        for src in [
            "9999999999h",
            "2562048h",
            "9223372036854775807ms",
            "2562047h47m17s",
        ] {
            match lex_src(src) {
                Err(d) => assert_eq!(d.code, "E0105", "{src}"),
                Ok(_) => panic!("{src}: expected E0105"),
            }
        }
        assert_eq!(one("2562047h").dur, 2562047 * H);
    }

    #[test]
    fn min_int_magnitude_is_a_token_for_the_parser() {
        let t = one("9223372036854775808");
        assert_eq!((t.kind, t.int), (Kind::Int, i64::MIN));
        assert_eq!(lex_src("9223372036854775809").unwrap_err().code, "E0135");
        // in any base (FE31, D9)
        for src in [
            "0x8000000000000000",
            "0o1000000000000000000000",
            "0x8000_0000_0000_0000",
        ] {
            let t = one(src);
            assert_eq!((t.kind, t.int), (Kind::Int, i64::MIN), "{src}");
        }
        assert_eq!(lex_src("0x8000000000000001").unwrap_err().code, "E0135");
    }
}
