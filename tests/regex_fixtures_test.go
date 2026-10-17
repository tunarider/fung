package tests

import (
	"fmt"
	"math/rand/v2"
	"os"
	"regexp"
	"slices"
	"strings"
	"testing"
)

// The Rust port's Go-semantics regular expressions
// (crates/fung-vm/src/native/regex_go.rs) are checked against tables of
// Go's own results. This test writes them with -update and otherwise
// checks that they are current.
const regexFixturesPath = "../crates/fung-vm/src/native/regex_go_fixtures.rs"

// regexCompileCases: patterns whose regexp.Compile result (error text or
// SubexpNames) is recorded. One or more per reachable syntax.ErrorCode,
// and the patterns Go and the Rust regex crate disagree on.
var regexCompileCases = []string{
	`(?x)a`,
	`(?x)`,
	`\Z`,
	`\z`,
	`\A`,
	`\G`,
	`\K`,
	`\e`,
	`\cA`,
	`\x{110000}`,
	`\x{10FFFF}`,
	`\x{D800}`,
	`[[:word:]]`,
	`[[:alpha:]]`,
	`[[:foo:]]`,
	`[a-z&&b]`,
	`[a--b]`,
	`[a~~b]`,
	`[[a]]`,
	`[a[b]]`,
	`[]a]`,
	`[^]a]`,
	`[]`,
	`[^]`,
	`(?<name>x)`,
	`(?P<name>x)`,
	`(?P=name)`,
	`(?'n'x)`,
	`(?<n>x)(?<n>y)`,
	`(?P<n>x)(?P<n>y)`,
	`(?<1a>x)`,
	`(?<a.b>x)`,
	`(?<é>x)`,
	`(?P<>x)`,
	`(?P<n`,
	`\p{Greek}`,
	`\pN`,
	`\p{^Greek}`,
	`\PL`,
	`\P{^L}`,
	`\p{L}`,
	`\p{Letter}`,
	`\p{letter}`,
	`\p{Lc}`,
	`\p{LC}`,
	`\p{Any}`,
	`\p{Assigned}`,
	`\p{ASCII}`,
	`\p{Alphabetic}`,
	`\p{sc=Greek}`,
	`\p{Greek`,
	`\p{}`,
	`\pZ`,
	`\pé`,
	`\p`,
	`\P`,
	`(?i:x)`,
	`(?i)`,
	`(?U)`,
	`(?u)`,
	`(?-u:a)`,
	`(?s-i:a)`,
	`(?i-)`,
	`(?-)`,
	`(?i-i)`,
	`(?`,
	`(?:`,
	`(?)`,
	`(?P`,
	`(?P>`,
	`x{2,1}`,
	`x{1001}`,
	`x{1000}`,
	`x{1000,}`,
	`x{,5}`,
	`x{01}`,
	`x{1}{2}`,
	`x{`,
	`x{a}`,
	`{1}`,
	`a{1}*`,
	`**`,
	`a**`,
	`a*+`,
	`a*?`,
	`a+?+`,
	`a???`,
	`a??`,
	`(?:)`,
	`()`,
	`(|)`,
	`|`,
	`a|*`,
	`(*)`,
	`(?i)*`,
	`^*`,
	`$+`,
	`\b*`,
	`\8`,
	`\0`,
	`\01`,
	`\012`,
	`\0123`,
	`\1`,
	`\08`,
	`\Q..\E`,
	`\Qa*`,
	`\Q`,
	`\b{start}`,
	`\<`,
	`\>`,
	`(?R)`,
	`(?#c)`,
	`(?=a)`,
	`(?!a)`,
	`(?<=a)`,
	`(?<!a)`,
	`(?>a)`,
	`(?|a)`,
	`\`,
	`a\`,
	`[a`,
	`[a-`,
	`[a-]`,
	`[-a]`,
	`[z-a]`,
	`[\d-z]`,
	`[a-\d]`,
	`[\w]`,
	`[^\w]`,
	`[\W]`,
	`\pL{1000}`,
	`(a)`,
	`a)`,
	`)`,
	`(a`,
	`((a)`,
	`\x`,
	`\x1`,
	`\xZZ`,
	`\x{}`,
	`\x{zz}`,
	`\x{41`,
	`A`,
	`\U00000041`,
	`\N{LATIN}`,
	`\C`,
	`\X`,
	`\R`,
	`\h`,
	`\v`,
	`\V`,
	`\H`,
	`\a\f\t\n\r`,
	`\_`,
	`\-`,
	`\:`,
	`\#`,
	`\ `,
	`\é`,
	`\y`,
	`\o{101}`,
	`[\b]`,
	`[\B]`,
	`[\A]`,
	`[\z]`,
	`[\Q]`,
	`[\pL]`,
	`[\p{Greek}a]`,
	`[[:^alpha:]]`,
	`[[=a=]]`,
	`[[.a.]]`,
	`[a-z[:digit:]]`,
	`[:alpha:]`,
	`(?i)k`,
	`.*`,
	`\d+`,
	`\s`,
	`\S`,
	`\w\W\D`,
	`(?m)^a$`,
	`(?sm).`,
	`x{2}{3}`,
	`(x{2}){3}`,
	`(?:x{100}){10}`,
	`(?:x{100}){11}`,
	`((x{10}){10}){10}`,
	`((x{10}){10}){11}`,
	`x{0}{2000}`,
	`(?:x{0}){1001}`,
	"",
	`a**b`,
	`\x{41}`,
	`a{1000}{1}`,
	`ÿ`,
	`é`,
	`(?P<n>`,
	`(?<n`,
	`(?i)K`,
	`[\x{10FFFF}-\x{0}]`,
	`\8\9`,
	`a(?<n`,
	`x(?P<a.b>y)`,
	`ab[c`,
	`ab[^`,
	`ab[]`,
	`a[\`,
	`[\`,
	`[a-\`,
	`(?i`,
	`(?i-s`,
	`a(?z)`,
	`(?im-sU:x)`,
	`(?m:^a$)`,
	`(?s:.)`,
	`(?-i)`,
	`(?--i)`,
	`(?i-:x)`,
	`a)b(`,
	`((a)|b`,
	`a|b)`,
	`(a|b))`,
	`a{2}{`,
	`a{2}?`,
	`a{2}??`,
	`a{2,}?{`,
	`x{1000,1001}`,
	`x{2,1000}`,
	`x{100000000}`,
	`x{99999999}`,
	`x{1,99999999999}`,
	`x{0,1001}`,
	`(?:x{2}){500}`,
	`(?:x{2}){501}`,
	`a|*b`,
	`(?:*)`,
	`(|*)`,
	`^?`,
	`\b+`,
	`\Q\E*`,
	`\Qab\E*`,
	`[[:alpha:]-z]`,
	`[a-[:alpha:]]`,
	`[\pL-z]`,
	`[a-\pL]`,
	`[\x{41}-\x{40}]`,
	`[\0-\1]`,
	`[\08]`,
	`[\x]`,
	`[\x{110000}]`,
	`[\8]`,
	`[a-\8]`,
	`[\Z]`,
	`\pL\p`,
	`[\p{Foo}]`,
	`[\p{Greek]`,
	`[[:alpha:]`,
	`[[:alpha:`,
	`[[:foo:]`,
	`[[:^foo:]]`,
	`[[:]]`,
	`[[::]]`,
	`\p{greek}`,
	`\p{GREEK}`,
	`\p{Old_Italic}`,
	`\p{olditalic}`,
	`\p{Old Italic}`,
	`\p{old-italic}`,
	`\p{ lu }`,
	`\p{L u}`,
	`\p{Uppercase_Letter}`,
	`\p{uppercaseletter}`,
	`\p{cntrl}`,
	`\p{digit}`,
	`\p{punct}`,
	`\p{ascii}`,
	`\p{any}`,
	`\p{assigned}`,
	`\p{^Any}`,
	`\P{^Any}`,
	`\p{Cased_Letter}`,
	`\p{Lu}`,
	`\p{lu}`,
	`\p{LU}`,
	`\pl`,
	`\pC`,
	`\p{Han}`,
	`\p{Hangul}`,
	`\p{Common}`,
	`\p{Inherited}`,
	`\p{Zzzz}`,
	`\p{Unknown}`,
	`\p{Latin1}`,
	`\p{^}`,
	`\p{^^L}`,
	`\x{0}`,
	`\x{000041}`,
	`\x41`,
	`\x4`,
	`\x{FFFFFFFFF}`,
	`\x{10ffff}`,
	`\x{-1}`,
	`\7`,
	`\77`,
	`\777`,
	`\7777`,
	`\18`,
	`\9`,
	`(?P<name>a)(?P<other>b)(c)`,
	`(?P<n1>a)|(?P<n2>b)`,
	`(?P<_>x)`,
	`(?P<__init__>x)`,
	`(?<A1_b2>x)`,
	`(?P<na me>x)`,
	`(?P<n>x`,
	`(?:(?:(?:a)))`,
	`(a(b(c)))`,
	`a{,}`,
	`a{1,2,3}`,
	`a{ 1}`,
	`a{1 }`,
	`{`,
	`}`,
	`]`,
	`a]`,
	`{}`,
	`a{}`,
	`x{2}{,}`,
	`(?i)ǅ`,
	`(?i)[ǅ]`,
	`(?i)[^ǅ]`,
	`[Aa]`,
	`[Kk]`,
	`[kK]`,
}

// regexMatchCases: (pattern, subject) pairs whose
// FindAllStringSubmatchIndex result is recorded.
var regexMatchCases = [][2]string{
	{`\w+`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`\W+`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`\d+`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`\D+`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`[\w]+`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`[^\w]+`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`[\W]+`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`[^\W]+`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`[^\d]+`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`[\d]+`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`\b`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`\B`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`\b\w+\b`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`é\b`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`\bé`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`[[:word:]]+`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`[[:alpha:]]+`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`[[:^alpha:]]+`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`\pL+`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`\PL+`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`\pN+`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`\p{Hangul}+`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`[\pL\d]+`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`[^\pL]+`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`\p{^L}+`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`\P{^L}+`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`.`, `héllo 한글 wörld_1 ٣ x٣y é9`},
	{`\s`, "a\u00a0b\u2003c\td\ne\vf\fg\rh i\u0085j"},
	{`\S+`, "a\u00a0b\u2003c\td\ne\vf\fg\rh i\u0085j"},
	{`[\s]`, "a\u00a0b\u2003c\td\ne\vf\fg\rh i\u0085j"},
	{`[^\s]+`, "a\u00a0b\u2003c\td\ne\vf\fg\rh i\u0085j"},
	{`[[:space:]]`, "a\u00a0b\u2003c\td\ne\vf\fg\rh i\u0085j"},
	{`\pZ`, "a\u00a0b\u2003c\td\ne\vf\fg\rh i\u0085j"},
	{`\v`, "a\u00a0b\u2003c\td\ne\vf\fg\rh i\u0085j"},
	{`[\v]`, "a\u00a0b\u2003c\td\ne\vf\fg\rh i\u0085j"},
	{`\x0b`, "a\u00a0b\u2003c\td\ne\vf\fg\rh i\u0085j"},
	{`(?i)k+`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)K`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)\x{212a}`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)s+`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)ſ`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)σ+`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)ς`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)[a-z]+`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)\w+`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)\W+`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)[^k]+`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)[^\W]+`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)[\W]+`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)[[:upper:]]+`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)[[:^lower:]]+`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)\p{Lu}+`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)\P{Lu}+`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)\p{Greek}+`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)ǅ`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)[ǅ]+`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)\x{3bc}`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)[k-s]+`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)[^a-z\s]+`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)[[:word:]]+`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)\p{Ascii}+`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)\P{Ascii}+`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)[\x{100}-\x{17f}]+`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i:k)K`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`(?i)k(?-i)k`, `kKK sSſ σςΣ ǄǅǆĲĳ µΜμ`},
	{`.+`, "ab\ncd\n\nef\n"},
	{`(?s).+`, "ab\ncd\n\nef\n"},
	{`^`, "ab\ncd\n\nef\n"},
	{`$`, "ab\ncd\n\nef\n"},
	{`(?m)^`, "ab\ncd\n\nef\n"},
	{`(?m)$`, "ab\ncd\n\nef\n"},
	{`^ab$`, "ab\ncd\n\nef\n"},
	{`(?m)^\w+$`, "ab\ncd\n\nef\n"},
	{`\A\w`, "ab\ncd\n\nef\n"},
	{`\w\z`, "ab\ncd\n\nef\n"},
	{`(?m)^$`, "ab\ncd\n\nef\n"},
	{`(?s:.)\n`, "ab\ncd\n\nef\n"},
	{`[^a]`, "ab\ncd\n\nef\n"},
	{`(?m:$)\n`, "ab\ncd\n\nef\n"},
	{`(?-m:^)`, "ab\ncd\n\nef\n"},
	{`a+?`, `aaa`},
	{`(?U)a+`, `aaa`},
	{`(?U)a+?`, `aaa`},
	{`(?U)a{1,3}`, `aaa`},
	{`a{1,3}?`, `aaa`},
	{`a??b`, `ab`},
	{`(?P<first>\w+) (?<last>\w+)`, `john smith, jane doe`},
	{`(?P<n>a)|(?P<n>b)`, `ab`},
	{`(?<1a>x)`, `x`},
	{`\<a\>`, `<a>`},
	{`\b{start}`, `a{start}`},
	{`[[a]]`, `a] [a]`},
	{`[a-z&&b]+`, `a&&b`},
	{`[a~~b]+`, `~a`},
	{`\Qa.b\E+`, `a.bb a.b`},
	{`\Q(a)*`, `(a)*`},
	{`\x{10FFFF}`, `􏿿`},
	{`\x{D800}|a`, `a`},
	{`[\x{D7FF}-\x{E000}]+`, `퟿`},
	{`\0`, "\x00"},
	{`\01`, "\x01"},
	{`\101`, `A`},
	{`\08`, "\x008"},
	{`x{,5}`, `x{,5}`},
	{`x{a}`, `x{a}`},
	{`x{`, `x{`},
	{`a{,}`, `a{,}`},
	{`(?:)`, `ab`},
	{`()`, `ab`},
	{`(|a)`, `a`},
	{`(a|)`, `a`},
	{`a|ab`, `ab`},
	{`ab|a`, `ab`},
	{`(a*)*`, `b`},
	{`(a*)+`, `b`},
	{`(a|b)*`, `ab`},
	{`(a*)+$`, `aab`},
	{`x*`, `xax`},
	{`a*`, `baaab`},
	{`(a){0}(b)`, `ab`},
	{`(a){0,0}`, `a`},
	{`(?:(a)|b){0}(c)`, `c`},
	{`(a)?`, `b`},
	{`(a)|b`, `b`},
	{`(a)(?:(b)|c)`, `ac`},
	{`(?:(a)|(b))+`, `ab`},
	{`(a+)(b+)?`, `aab`},
	{`\a\f\t\n\r\v`, "\a\f\t\n\r\v"},
	{`\_\-\:\#\ `, `_-:# `},
	{`[]a]+`, `]a`},
	{`[^]a]+`, `]ab`},
	{`[a-]+`, `-a`},
	{`[-a]+`, `a-`},
	{`[\d-z]+`, `1-z`},
	{`(?i-i)a`, `aA`},
	{`(?)a`, `a`},
	{`ab|ac|ad`, `ad ac ab`},
	{`abc|abd|aef|bcx|bcy`, `bcy aef abd`},
	{`[ab]c|[ab]d`, `bd ac`},
	{`a|b|c|d`, `dcba`},
	{`a(?:b|c)|a(?:d|e)`, `ae`},
	{`(?:a|a)b|ab`, `ab`},
	{`(?i)ab|ac`, `AB aC`},
	{`(?i:a)b|ac`, `Ab ac aC`},
	{`x|(?:)`, `x`},
	{`(?:|x)+`, `xx`},
	{`(?:a{2}){3}`, `aaaaaa`},
	{`^*a`, `a`},
	{`$+`, `a`},
	{`\b*a`, `a`},
	{`(?:^|a)+`, `aa`},
	{`(?:$|a)+`, `aa`},
	{`\bx\B`, `xa x`},
	{`(\w+)@(\w+)\.com`, `me@x.com you@ä.com`},
	{`(?s)a.c`, "a\nc"},
	{`a.c`, "a\nc abc"},
	{`[^\n]+`, "ab\ncd"},
	{`\pL\p{Greek}\pN`, `aβ1`},
	{`\p{Assigned}+`, `a͸b`},
	{`\p{Any}+`, "a\nb"},
	{`\P{Any}`, `a`},
	{`\p{Lc}+`, `aBǅ`},
	{`\p{Cs}`, `a`},
	{`(?:a+)?`, `xaay`},
	{`(?:a*)?`, `xaay`},
	{`(?:a?)??`, `xaay`},
	{`(?:a{1,2})?`, `x`},
	{`(?:a{1,2})?`, `xaaay`},
	{`(?:(a)*)?`, `xaay`},
	{`(?i:k+)?`, `xkKKy`},
	{`(?:(?:ab){2})?`, `abababx`},
	{`(?:\s+)?`, `a 	 b`},
	{`(?:\d{1,3})?x`, `12345x x`},
}

// regexFuzzAlphabet: pieces the random patterns are made of.
var regexFuzzAlphabet = append(strings.Split("()[]{}|*+?\\^$.-:<>Pp01279aAkKsz,!=#&~_ é", ""),
	"(?", "(?i", "(?:", "(?P<", `\p{`, `\pL`, "[:alpha:]", "[[:", `\d`, `\w`, `\s`, `\b`, `\B`, `\Q`, `\E`, `\x{`, "}",
	"{2}", "{1,3}", "{0}", "(?s)", "(?m)", "(?U)", `\z`, `\A`, "Greek}", "ſ", "K")

// Pieces of the nested-repetition patterns: group, atom, repetition,
// close, repetition (a repetition directly inside another).
var (
	regexNestOpen  = []string{"(?:", "(", "(?i:", "(?s:", "(?P<g>", "(?U:"}
	regexNestAtom  = []string{"a", "ab", "k", `\d`, `\s`, "[a-c]", ".", "(a)", `\pL`, "é", "a|b", "", `\b`}
	regexNestRep   = []string{"*", "+", "?", "{2}", "{1,2}", "{0,}", "{0}", "*?", "+?", "??", "{1,2}?", "{2,}?"}
	regexSubjects  = []string{"aAkKK sſS_01 é\n{2}[:]", "zz\n\nPp-é9_", "<a>|b*c+?", ""}
	regexNestSubjs = []string{"xaaay ab abab1 kKK", "a\n\né1 2", ""}
)

// regexFuzzCases: seeded random patterns, then the nested-repetition
// ones, each with a subject.
func regexFuzzCases() [][2]string {
	rng := rand.New(rand.NewPCG(2026, 1016))
	pick := func(xs []string) string { return xs[rng.IntN(len(xs))] }
	seen := map[string]bool{}
	var random []string
	for len(random) < 2000 {
		var b strings.Builder
		for n := 1 + rng.IntN(10); n > 0; n-- {
			b.WriteString(pick(regexFuzzAlphabet))
		}
		if p := b.String(); !seen[p] {
			seen[p] = true
			random = append(random, p)
		}
	}
	var nested []string
	for len(nested) < 600 {
		p := pick(regexNestOpen) + pick(regexNestAtom) + pick(regexNestRep) + ")" + pick(regexNestRep)
		if !seen[p] {
			seen[p] = true
			nested = append(nested, p)
		}
	}
	slices.Sort(random)
	slices.Sort(nested)
	var out [][2]string
	for i, p := range random {
		out = append(out, [2]string{p, regexSubjects[i%len(regexSubjects)]})
	}
	for i, p := range nested {
		out = append(out, [2]string{p, regexNestSubjs[i%len(regexNestSubjs)]})
	}
	return out
}

// rustString spells s as a Rust string literal.
func rustString(s string) string {
	var b strings.Builder
	b.WriteByte('"')
	for _, r := range s {
		switch {
		case r == '"' || r == '\\':
			b.WriteByte('\\')
			b.WriteRune(r)
		case r >= 0x20 && r < 0x7f:
			b.WriteRune(r)
		default:
			fmt.Fprintf(&b, "\\u{%x}", r)
		}
	}
	b.WriteByte('"')
	return b.String()
}

func regexCompileRow(pat string) string {
	re, err := regexp.Compile(pat)
	if err != nil {
		return fmt.Sprintf("    (%s, Err(%s)),\n", rustString(pat), rustString(err.Error()))
	}
	var names []string
	for _, n := range re.SubexpNames() {
		names = append(names, rustString(n))
	}
	return fmt.Sprintf("    (%s, Ok(&[%s])),\n", rustString(pat), strings.Join(names, ", "))
}

// regexMatchRow is "" for a pattern Go rejects.
func regexMatchRow(pat, s string) string {
	re, err := regexp.Compile(pat)
	if err != nil {
		return ""
	}
	var ms []string
	for _, m := range re.FindAllStringSubmatchIndex(s, -1) {
		var xs []string
		for _, x := range m {
			xs = append(xs, fmt.Sprint(x))
		}
		ms = append(ms, "&["+strings.Join(xs, ", ")+"]")
	}
	return fmt.Sprintf("    (%s, %s, &[%s]),\n", rustString(pat), rustString(s), strings.Join(ms, ", "))
}

func regexFixtures() string {
	var b strings.Builder
	b.WriteString("// Go regexp results for the Rust port's tests (regex_go_tests.rs).\n")
	b.WriteString("// Generated by `go test ./tests -run TestRegexFixtures -update`\n")
	b.WriteString("// (tests/regex_fixtures_test.go); do not edit.\n\n")
	b.WriteString("/// `regexp.Compile`: `Ok(SubexpNames())` or `Err(err.Error())`.\n")
	b.WriteString("pub(super) const COMPILE: &[(&str, Result<&[&str], &str>)] = &[\n")
	for _, p := range regexCompileCases {
		b.WriteString(regexCompileRow(p))
	}
	b.WriteString("];\n\n")
	b.WriteString("/// `FindAllStringSubmatchIndex(s, -1)`: byte offsets, -1 for a group\n")
	b.WriteString("/// that did not take part.\n")
	b.WriteString("pub(super) const MATCHES: &[(&str, &str, &[&[isize]])] = &[\n")
	for _, c := range regexMatchCases {
		b.WriteString(regexMatchRow(c[0], c[1]))
	}
	b.WriteString("];\n\n")
	fuzz := regexFuzzCases()
	b.WriteString("/// Seeded random patterns and nested repetitions: `regexp.Compile`.\n")
	b.WriteString("pub(super) const FUZZ_COMPILE: &[(&str, Result<&[&str], &str>)] = &[\n")
	for _, c := range fuzz {
		b.WriteString(regexCompileRow(c[0]))
	}
	b.WriteString("];\n\n")
	b.WriteString("/// The same patterns Go accepts: `FindAllStringSubmatchIndex`.\n")
	b.WriteString("pub(super) const FUZZ_MATCH: &[(&str, &str, &[&[isize]])] = &[\n")
	for _, c := range fuzz {
		b.WriteString(regexMatchRow(c[0], c[1]))
	}
	b.WriteString("];\n")
	return b.String()
}

func TestRegexFixtures(t *testing.T) {
	want := regexFixtures()
	if *update {
		if err := os.WriteFile(regexFixturesPath, []byte(want), 0o644); err != nil {
			t.Fatal(err)
		}
		return
	}
	got, err := os.ReadFile(regexFixturesPath)
	if err != nil || string(got) != want {
		t.Errorf("%s is out of date: run `go test ./tests -run TestRegexFixtures -update`", regexFixturesPath)
	}
}
