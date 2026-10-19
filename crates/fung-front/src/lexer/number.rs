//! Number literals: integers, floats and durations.

use super::{Kind, Lexer, is_digit, is_hex_digit, is_ident_part, is_ident_start};
use crate::diag::Diag;
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

impl Lexer<'_> {
    /// Lexes integers (decimal, 0x, 0o, 0b, with `_` separators), floats
    /// (digits on both sides of the point, optional exponent) and durations
    /// (a number immediately followed by ms/s/m/h, possibly compound: 1h30m).
    pub(super) fn number(&mut self) -> Result<(), Diag> {
        let p = self.pos();
        let start = self.off;

        if self.peek() == Some('0')
            && matches!(self.peek_at(1), Some('x' | 'X' | 'o' | 'O' | 'b' | 'B'))
        {
            return self.prefixed_int(p, start);
        }

        if !self.digits() {
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
        if matches!(self.peek(), Some('e' | 'E')) {
            let next = self.peek_at(1);
            if is_digit(next) || (matches!(next, Some('+' | '-')) && is_digit(self.peek_at(2))) {
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
                // an overflow parses as infinity, which is E0102 below
                Ok(f) if f.is_finite() => {
                    let raw = raw.to_string();
                    self.emit(Kind::Float, &raw, p).float = f;
                    Ok(())
                }
                _ => Err(self.err("E0102", p, raw)),
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
            Err(_) => Err(self.err("E0102", p, raw)),
        }
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
            'o' | 'O' => 8,
            'b' | 'B' => 2,
            _ => 16,
        };
        let dig_start = self.off;
        // `_` only between digits (01): not right after the prefix
        while is_hex_digit(self.peek())
            || (self.peek() == Some('_') && self.off > dig_start && is_hex_digit(self.peek_at(1)))
        {
            self.advance();
        }
        if self.off == dig_start || is_ident_start(self.peek()) || is_digit(self.peek()) {
            self.ident_text();
            return Err(self.err("E0102", p, &self.src[start..self.off]));
        }
        let text: String = self.src[dig_start..self.off]
            .chars()
            .filter(|&c| c != '_')
            .collect();
        match i64::from_str_radix(&text, base) {
            Ok(n) => {
                let raw = self.src[start..self.off].to_string();
                self.emit(Kind::Int, &raw, p).int = n;
                Ok(())
            }
            Err(_) => Err(self.err("E0102", p, &self.src[start..self.off])),
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
        let cases: [(&str, i64); 9] = [
            ("0", 0),
            ("42", 42),
            ("1_000_000", 1000000),
            ("0xff", 255),
            ("0XFF", 255),
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
            ("1E+2", 100.0),
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
            ("9223372036854775809", "E0102"),
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
        let d = lex_src("1h30").unwrap_err();
        assert_eq!(d.args, vec![Arg::Str("1h30".to_string())]);
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
        assert_eq!(lex_src("9223372036854775809").unwrap_err().code, "E0102");
    }
}
