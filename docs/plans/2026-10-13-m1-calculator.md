# M1 계산기 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `fung -e 'print(1 + 2 * 3)'`이 `7`을 출력한다. 렉서 → Pratt 파서 → 균일 트리 → desugar → resolve → 컴파일러 → 스택 VM이 처음으로 한 줄로 이어진다.

**Architecture:** 렉서는 01의 글자 수준 규칙 **전부**를 구현하고(M2·M4에서 다시 열지 않는다), 파서·VM은 숫자·불리언·산술·비교·논리·`print` 호출만 다룬다. 모든 단계는 `internal/` 아래 한 패키지씩이며, 트리는 `tree.Node` 한 형태, 명령은 32비트 고정 폭, 에러는 `diag.Error` 하나로 통일한다.

**Tech Stack:** Go 1.27, 표준 라이브러리만. 테스트는 `go test` (표 기반 단위 테스트 + `tests/lang/*.fung` 골든 테스트).

**Spec:** `docs/design/01-syntax.md` (글자 규칙, EBNF), `02-values-and-types.md` (비교 표), `07-operators-and-types.md` (산술 규칙, 우선순위), `08-tooling-and-errors.md` (에러 번호 분류, 출력 형식), `09-implementation-plan.md` (앞단·뒷단 구조).

## Global Constraints

- Go 모듈 `github.com/tunarider/fung`, `go 1.27`, **Go 표준 라이브러리만** (go.mod에 require 없음).
- 코드·주석·에러 메시지·커밋 메시지는 영어. 문서는 한국어.
- 커밋: Conventional Commits. 커밋 전 `gofmt -l .`(출력 없음), `go vet ./...`, `go test ./...` 통과.
- 커밋 메시지 끝에 `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Go 패닉과 Go 스택 추적은 사용자에게 보이지 않는다. 모두 fung 에러로 변환한다 (00).
- 바이트코드의 모든 명령은 소스 위치(파일, 줄, 칸)를 가진다 (08).
- 종료 코드: 0 정상, 1 런타임 에러, 2 컴파일 에러, 3 사용 오류 (08, `internal/diag`).
- 에러 번호는 한 번 붙이면 바꾸지 않는다 (08). 이 계획에서 붙이는 번호가 최초 할당이다.

## Review Focus

사양이 암시하지만 아래 작업의 테스트가 다루지 않을 뻔한 입력들. 각 항목은 해당 작업의 테스트에 넣었다.

1. `9223372036854775808` (int64 최댓값 + 1)을 리터럴로 쓰면 → 렉서가 E0102로 거부해야 한다 (Task 3 테스트에 포함).
2. `-9223372036854775808 // -1`, `-9223372036854775808 * -1` → 오버플로 E0503 (Task 9 테스트에 포함).
3. `0.1 + 0.2`의 출력 `0.30000000000000004`처럼 가장 짧은 왕복 표현을 그대로 보여 주고, `1e21`은 `1e21`, `1e-7`은 `1e-07`로 (Task 9 테스트에 포함).
4. `print(1 +\n\n 2)`처럼 이어짐 뒤에 빈 줄이 여러 개 → 이어져야 한다 (Task 6 테스트에 포함).
5. 윈도우 줄바꿈 `\r\n` → `\r`은 공백으로 취급되어 위치 계산과 토큰에 영향이 없어야 한다 (Task 2 테스트에 포함).

## 파일 구조

| 패키지 | 책임 | 만드는 작업 |
|------|------|------|
| `internal/diag` | `Pos`, `Error`, 에러 번호 카탈로그, 출력 형식, 종료 코드(M0) | Task 1 |
| `internal/lexer` | 소스 → 토큰 + 주석 trivia | Task 2, 3, 4 |
| `internal/tree` | 균일 트리 노드와 S-식 출력 | Task 5 |
| `internal/parser` | 토큰 → 겉 문법 트리 (Pratt) | Task 6 |
| `internal/desugar` | 겉 문법 트리 → 코어 트리 | Task 7 |
| `internal/resolve` | 이름 분류, 미정의 이름 에러 | Task 8 |
| `internal/value` | 값의 Go 표현, 산술, 비교, 출력 형식 | Task 9 |
| `internal/bytecode` | 명령, 인코딩, `Proto`, 역어셈블러 | Task 10 |
| `internal/compiler` | 코어 트리 → `Proto` | Task 11 |
| `internal/native` | prelude (`print`) | Task 12 |
| `internal/vm` | 실행기 | Task 12 |
| `cmd/fung` | `-e`, `run`, `dump` 연결 | Task 13 |
| `tests/lang` | 골든 테스트 파일 | Task 13 |
| `docs/notes/M1-calculator.md` | 학습 노트 | Task 14 |

### M1에서 할당하는 에러 번호

| 번호 | 메시지 템플릿 | 어디서 |
|------|------|------|
| E0101 | `unexpected character %q` | 렉서 |
| E0102 | `malformed number %q` | 렉서 |
| E0103 | `unknown escape \%s in string` | 렉서 |
| E0104 | `unterminated string` | 렉서 |
| E0105 | `malformed duration %q` | 렉서 |
| E0110 | `unexpected %s` | 파서 |
| E0111 | `comparison operators cannot be chained` (힌트: `split it with and`) | 파서 |
| E0204 | `undefined name %s` | resolve |
| E0401 | `cannot call a value of type %s` | VM |
| E0501 | `cannot %s %s and %s` / 단항은 `cannot %s %s` | value |
| E0502 | `division by zero` | value |
| E0503 | `integer overflow` | value |
| E0504 | `result is not a finite number` | value |
| E0505 | `expected bool, got %s` | VM |
| E0506 | `cannot %s %s` (단항 연산 타입 불일치) | value |
| E0801 | `internal error: %v` | VM (Go 패닉 변환) |

---

### Task 1: diag — 위치, 에러, 카탈로그, 출력 형식

**Files:**
- Create: `internal/diag/pos.go`, `internal/diag/error.go`, `internal/diag/catalog.go`
- Modify: `internal/diag/exit.go` (변경 없음, 같은 패키지)
- Test: `internal/diag/error_test.go`

**Interfaces:**
- Produces: `type Pos struct{File string; Line, Col int}`, `func (Pos) String() string`
- Produces: `type Error struct{Code string; Pos Pos; Args []any}`, `func Errorf(code string, pos Pos, args ...any) *Error`, `func (e *Error) Error() string`, `func (e *Error) WithPos(Pos) *Error`
- Produces: `func Message(code string, args ...any) string`, `func Hint(code string) string`, `func Format(e *Error) string`, `func IsCompileError(code string) bool`

- [ ] **Step 1: 실패하는 테스트 작성**

```go
// internal/diag/error_test.go
package diag

import "testing"

func TestErrorfUsesCatalogMessage(t *testing.T) {
	e := Errorf("E0204", Pos{File: "a.fung", Line: 3, Col: 7}, "fiels")
	want := "error[E0204]: undefined name `fiels`"
	if e.Error() != want {
		t.Fatalf("Error() = %q, want %q", e.Error(), want)
	}
}

func TestFormatShowsPositionAndHint(t *testing.T) {
	e := Errorf("E0111", Pos{File: "a.fung", Line: 1, Col: 9})
	got := Format(e)
	want := "error[E0111]: comparison operators cannot be chained\n  --> a.fung:1:9\n help: split it with `and`: a < b and b < c\n"
	if got != want {
		t.Fatalf("Format() = %q, want %q", got, want)
	}
}

func TestFormatWithoutHintOrPosition(t *testing.T) {
	e := Errorf("E0502", Pos{})
	got := Format(e)
	want := "error[E0502]: division by zero\n"
	if got != want {
		t.Fatalf("Format() = %q, want %q", got, want)
	}
}

func TestUnknownCodeStillFormats(t *testing.T) {
	e := Errorf("E9999", Pos{}, "x")
	if got := e.Error(); got != "error[E9999]: unknown error E9999 [x]" {
		t.Fatalf("Error() = %q", got)
	}
}

func TestWithPosFillsOnlyEmptyPosition(t *testing.T) {
	e := Errorf("E0502", Pos{})
	p := Pos{File: "a.fung", Line: 2, Col: 1}
	if got := e.WithPos(p).Pos; got != p {
		t.Fatalf("WithPos on empty = %v, want %v", got, p)
	}
	q := Pos{File: "b.fung", Line: 9, Col: 9}
	if got := e.WithPos(q).Pos; got != p {
		t.Fatalf("WithPos on filled = %v, want it kept %v", got, p)
	}
}

func TestIsCompileError(t *testing.T) {
	cases := map[string]bool{"E0101": true, "E0204": true, "E0301": true, "E0401": false, "E0502": false, "E0801": false}
	for code, want := range cases {
		if got := IsCompileError(code); got != want {
			t.Errorf("IsCompileError(%s) = %v, want %v", code, got, want)
		}
	}
}

func TestPosString(t *testing.T) {
	if got := (Pos{File: "a.fung", Line: 3, Col: 7}).String(); got != "a.fung:3:7" {
		t.Fatalf("String() = %q", got)
	}
}
```

- [ ] **Step 2: 실패 확인**

Run: `go test ./internal/diag/`
Expected: FAIL, `undefined: Errorf` 등 컴파일 에러.

- [ ] **Step 3: 구현**

```go
// internal/diag/pos.go
package diag

import "fmt"

// Pos is a source position. Line and Col are 1-based; Col counts runes.
// The zero Pos means "unknown", which value operations use before the VM
// fills in the position of the instruction that failed.
type Pos struct {
	File      string
	Line, Col int
}

func (p Pos) String() string { return fmt.Sprintf("%s:%d:%d", p.File, p.Line, p.Col) }

// IsZero reports whether the position is unknown.
func (p Pos) IsZero() bool { return p.Line == 0 }
```

```go
// internal/diag/error.go
package diag

import (
	"fmt"
	"strings"
)

// Error is every error fung reports to the user: a catalog number, where
// it happened, and the values that fill the message template. The message
// text itself lives in the catalog so it can be translated and kept stable.
type Error struct {
	Code string
	Pos  Pos
	Args []any
}

// Errorf builds an Error for a catalog code.
func Errorf(code string, pos Pos, args ...any) *Error {
	return &Error{Code: code, Pos: pos, Args: args}
}

func (e *Error) Error() string {
	return fmt.Sprintf("error[%s]: %s", e.Code, Message(e.Code, e.Args...))
}

// WithPos fills in the position if the error does not have one yet.
// Value operations raise errors without knowing where they were called
// from; the VM adds the position of the current instruction.
func (e *Error) WithPos(p Pos) *Error {
	if e.Pos.IsZero() {
		e.Pos = p
	}
	return e
}

// Format renders an error the way the fung command prints it:
//
//	error[E0204]: undefined name `fiels`
//	  --> rename.fung:12:12
//	 help: ...
//
// Source excerpts and call paths are added in later milestones.
func Format(e *Error) string {
	var b strings.Builder
	b.WriteString(e.Error())
	b.WriteByte('\n')
	if !e.Pos.IsZero() {
		fmt.Fprintf(&b, "  --> %s\n", e.Pos)
	}
	if h := Hint(e.Code); h != "" {
		fmt.Fprintf(&b, " help: %s\n", h)
	}
	return b.String()
}

// IsCompileError reports whether a code belongs to the compile-time
// categories (syntax E01xx, names E02xx, modules E03xx). The fung command
// uses it to pick the exit code.
func IsCompileError(code string) bool {
	return strings.HasPrefix(code, "E01") || strings.HasPrefix(code, "E02") || strings.HasPrefix(code, "E03")
}
```

```go
// internal/diag/catalog.go
package diag

import "fmt"

// catalog maps error numbers to message templates. Numbers never change
// once assigned (golden tests and users depend on them); messages may be
// reworded. Categories: E01 syntax, E02 names/scope, E03 modules,
// E04 calls/annotations, E05 runtime operations, E06 patterns/results,
// E07 concurrency, E08 limits/environment, W01 warnings.
var catalog = map[string]string{
	// E01xx syntax (lexer, parser)
	"E0101": "unexpected character %q",
	"E0102": "malformed number %q",
	"E0103": "unknown escape \\%s in string",
	"E0104": "unterminated string",
	"E0105": "malformed duration %q",
	"E0110": "unexpected %s",
	"E0111": "comparison operators cannot be chained",
	// E02xx names and scope
	"E0204": "undefined name `%s`",
	// E04xx calls
	"E0401": "cannot call a value of type %s",
	// E05xx runtime operations
	"E0501": "cannot %s %s and %s",
	"E0502": "division by zero",
	"E0503": "integer overflow",
	"E0504": "result is not a finite number",
	"E0505": "expected bool, got %s",
	// E08xx limits and environment
	"E0801": "internal error: %v",
}

// hints are optional one-line suggestions shown after the position.
var hints = map[string]string{
	"E0111": "split it with `and`: a < b and b < c",
}

// Message renders the catalog template for a code.
func Message(code string, args ...any) string {
	tmpl, ok := catalog[code]
	if !ok {
		return fmt.Sprintf("unknown error %s %v", code, args)
	}
	return fmt.Sprintf(tmpl, args...)
}

// Hint returns the help line for a code, or "".
func Hint(code string) string { return hints[code] }
```

단항 연산의 타입 불일치(`cannot negate bool`)는 이항용 템플릿에 맞지 않으므로 전용 번호를 쓴다. 카탈로그의 E05xx 블록에 한 줄을 더한다:

```go
	"E0506": "cannot %s %s",
```

- [ ] **Step 4: 통과 확인**

Run: `go test ./internal/diag/`
Expected: PASS

- [ ] **Step 5: 커밋**

```bash
git add internal/diag
git commit -m "feat(diag): add positions, catalog-backed errors, and formatting

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: lexer — 토큰 종류, 위치, 구두점, 식별자, 예약어, 줄바꿈, 주석

**Files:**
- Create: `internal/lexer/token.go`, `internal/lexer/lexer.go`
- Test: `internal/lexer/lexer_test.go`

**Interfaces:**
- Produces: `type Kind int` 과 상수들 (아래 목록), `func (Kind) String() string`
- Produces: `type Token struct{Kind Kind; Text string; Pos diag.Pos; Int int64; Float float64; Dur time.Duration; Parts []Part}`
- Produces: `type Part struct{IsCode bool; Text string; Pos diag.Pos}` (Task 4에서 채움)
- Produces: `type Comment struct{Pos diag.Pos; Text string}`
- Produces: `func Lex(file, src string) ([]Token, []Comment, *diag.Error)` — 마지막 토큰은 항상 `EOF`
- Produces: `func Keywords() map[string]Kind`

- [ ] **Step 1: 실패하는 테스트 작성**

```go
// internal/lexer/lexer_test.go
package lexer

import (
	"testing"

	"github.com/tunarider/fung/internal/diag"
)

// kinds returns the token kinds of a source, dropping the final EOF.
func kinds(t *testing.T, src string) []Kind {
	t.Helper()
	toks, _, err := Lex("t.fung", src)
	if err != nil {
		t.Fatalf("Lex(%q): %v", src, err)
	}
	var ks []Kind
	for _, tok := range toks[:len(toks)-1] {
		ks = append(ks, tok.Kind)
	}
	return ks
}

func equalKinds(a, b []Kind) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}

func TestPunctuationLongestMatch(t *testing.T) {
	cases := map[string][]Kind{
		"( ) [ ] { } , : ;":   {LParen, RParen, LBracket, RBracket, LBrace, RBrace, Comma, Colon, Semi},
		";; ;;; ;;;;":         {DoubleSemi, DoubleSemi, Semi, DoubleSemi, DoubleSemi},
		". .. ..= ...":        {Dot, DotDot, DotDotEq, Ellipsis},
		"= == => -> - <= < >= > !=": {Assign, Eq, FatArrow, Arrow, Minus, Le, Lt, Ge, Gt, Ne},
		"+ * ** / // % |> | ?? ? !": {Plus, Star, StarStar, Slash, SlashSlash, Percent, Pipe, Bar, Coalesce, Question, Bang},
		"a;;b":                {Name, DoubleSemi, Name},
	}
	for src, want := range cases {
		if got := kinds(t, src); !equalKinds(got, want) {
			t.Errorf("%q: got %v, want %v", src, got, want)
		}
	}
}

func TestIdentifiersKeywordsAndTags(t *testing.T) {
	toks, _, err := Lex("t.fung", "fn is_log Ok _x 파일목록 Red true nil x1")
	if err != nil {
		t.Fatal(err)
	}
	want := []struct {
		kind Kind
		text string
	}{
		{KwFn, "fn"}, {Name, "is_log"}, {Tag, "Ok"}, {Name, "_x"}, {Name, "파일목록"},
		{Tag, "Red"}, {KwTrue, "true"}, {KwNil, "nil"}, {Name, "x1"},
	}
	for i, w := range want {
		if toks[i].Kind != w.kind || toks[i].Text != w.text {
			t.Errorf("token %d = %v %q, want %v %q", i, toks[i].Kind, toks[i].Text, w.kind, w.text)
		}
	}
}

func TestAllKeywordsAreRecognized(t *testing.T) {
	for word, kind := range Keywords() {
		toks, _, err := Lex("t.fung", word)
		if err != nil {
			t.Fatal(err)
		}
		if toks[0].Kind != kind {
			t.Errorf("%q lexed as %v, want %v", word, toks[0].Kind, kind)
		}
	}
	if len(Keywords()) != 20 {
		t.Errorf("Keywords() has %d entries, want 20 (01. 예약어)", len(Keywords()))
	}
}

func TestNewlinesCommentsAndPositions(t *testing.T) {
	src := "a # first\r\n  b\n\n# only comment\nc"
	toks, comments, err := Lex("t.fung", src)
	if err != nil {
		t.Fatal(err)
	}
	wantKinds := []Kind{Name, Newline, Name, Newline, Newline, Newline, Name, EOF}
	for i, k := range wantKinds {
		if toks[i].Kind != k {
			t.Fatalf("token %d = %v, want %v (all: %v)", i, toks[i].Kind, k, toks)
		}
	}
	wantPos := []diag.Pos{
		{File: "t.fung", Line: 1, Col: 1}, // a
		{File: "t.fung", Line: 1, Col: 10}, // the \r\n counts as one newline at the \r
		{File: "t.fung", Line: 2, Col: 3}, // b
	}
	for i, p := range wantPos {
		if toks[i].Pos != p {
			t.Errorf("token %d pos = %v, want %v", i, toks[i].Pos, p)
		}
	}
	if len(comments) != 2 || comments[0].Text != "# first" || comments[1].Text != "# only comment" {
		t.Fatalf("comments = %+v", comments)
	}
	if comments[1].Pos != (diag.Pos{File: "t.fung", Line: 4, Col: 1}) {
		t.Errorf("comment pos = %v", comments[1].Pos)
	}
}

func TestColumnsCountRunesNotBytes(t *testing.T) {
	toks, _, err := Lex("t.fung", "참치 x")
	if err != nil {
		t.Fatal(err)
	}
	if toks[1].Pos.Col != 4 {
		t.Fatalf("x at col %d, want 4", toks[1].Pos.Col)
	}
}

func TestUnexpectedCharacter(t *testing.T) {
	_, _, err := Lex("t.fung", "a @ b")
	if err == nil || err.Code != "E0101" {
		t.Fatalf("err = %v, want E0101", err)
	}
	if err.Pos != (diag.Pos{File: "t.fung", Line: 1, Col: 3}) {
		t.Fatalf("pos = %v", err.Pos)
	}
}

func TestEmptySourceIsJustEOF(t *testing.T) {
	toks, _, err := Lex("t.fung", "")
	if err != nil || len(toks) != 1 || toks[0].Kind != EOF {
		t.Fatalf("toks = %v, err = %v", toks, err)
	}
}
```

- [ ] **Step 2: 실패 확인**

Run: `go test ./internal/lexer/`
Expected: FAIL (컴파일 에러, `Lex` 미정의).

- [ ] **Step 3: 구현**

```go
// internal/lexer/token.go
package lexer

import (
	"fmt"
	"time"

	"github.com/tunarider/fung/internal/diag"
)

// Kind classifies a token.
type Kind int

const (
	EOF Kind = iota
	Newline

	// Literals and names
	Int
	Float
	String    // "..." with escapes and ${} parts
	RawString // `...`
	Duration
	Symbol // 'name
	Name   // starts with lowercase, _, or a caseless letter
	Tag    // starts with an uppercase letter

	// Keywords (01. 예약어)
	KwFn
	KwIf
	KwElse
	KwThen
	KwMatch
	KwWhen
	KwFor
	KwIn
	KwNot
	KwAnd
	KwOr
	KwReturn
	KwImport
	KwAs
	KwType
	KwTest
	KwAssert
	KwTrue
	KwFalse
	KwNil

	// Punctuation and operators
	LParen
	RParen
	LBracket
	RBracket
	LBrace
	RBrace
	Comma
	Colon
	Semi
	DoubleSemi
	Dot
	DotDot
	DotDotEq
	Ellipsis
	FatArrow // =>
	Arrow    // ->
	Assign   // =
	Eq       // ==
	Ne       // !=
	Lt
	Gt
	Le
	Ge
	Plus
	Minus
	Star
	StarStar
	Slash
	SlashSlash
	Percent
	Pipe     // |>
	Bar      // |
	Coalesce // ??
	Question // ?
	Bang     // !
)

var kindNames = map[Kind]string{
	EOF: "end of file", Newline: "newline",
	Int: "integer", Float: "float", String: "string", RawString: "raw string",
	Duration: "duration", Symbol: "symbol", Name: "name", Tag: "tag",
	KwFn: "fn", KwIf: "if", KwElse: "else", KwThen: "then", KwMatch: "match", KwWhen: "when",
	KwFor: "for", KwIn: "in", KwNot: "not", KwAnd: "and", KwOr: "or", KwReturn: "return",
	KwImport: "import", KwAs: "as", KwType: "type", KwTest: "test", KwAssert: "assert",
	KwTrue: "true", KwFalse: "false", KwNil: "nil",
	LParen: "(", RParen: ")", LBracket: "[", RBracket: "]", LBrace: "{", RBrace: "}",
	Comma: ",", Colon: ":", Semi: ";", DoubleSemi: ";;", Dot: ".", DotDot: "..", DotDotEq: "..=",
	Ellipsis: "...", FatArrow: "=>", Arrow: "->", Assign: "=", Eq: "==", Ne: "!=",
	Lt: "<", Gt: ">", Le: "<=", Ge: ">=", Plus: "+", Minus: "-", Star: "*", StarStar: "**",
	Slash: "/", SlashSlash: "//", Percent: "%", Pipe: "|>", Bar: "|", Coalesce: "??",
	Question: "?", Bang: "!",
}

func (k Kind) String() string {
	if s, ok := kindNames[k]; ok {
		return s
	}
	return fmt.Sprintf("Kind(%d)", int(k))
}

// Keywords returns the reserved words and their kinds.
func Keywords() map[string]Kind {
	return map[string]Kind{
		"fn": KwFn, "if": KwIf, "else": KwElse, "then": KwThen, "match": KwMatch, "when": KwWhen,
		"for": KwFor, "in": KwIn, "not": KwNot, "and": KwAnd, "or": KwOr, "return": KwReturn,
		"import": KwImport, "as": KwAs, "type": KwType, "test": KwTest, "assert": KwAssert,
		"true": KwTrue, "false": KwFalse, "nil": KwNil,
	}
}

// Token is one lexical unit. Text is the source slice (for names, tags,
// symbols it is the identifier without any sigil). Int, Float, Dur and
// Parts carry the decoded value of literals.
type Token struct {
	Kind  Kind
	Text  string
	Pos   diag.Pos
	Int   int64
	Float float64
	Dur   time.Duration
	Parts []Part
}

// Part is one piece of a "..." string: literal text, or the source of an
// interpolated ${...} expression that the parser parses later.
type Part struct {
	IsCode bool
	Text   string
	Pos    diag.Pos
}

// Comment is trivia kept aside for the formatter.
type Comment struct {
	Pos  diag.Pos
	Text string
}

func (t Token) String() string {
	switch t.Kind {
	case Name, Tag, Symbol, Int, Float, Duration, String, RawString:
		return fmt.Sprintf("%v(%s)", t.Kind, t.Text)
	}
	return t.Kind.String()
}
```

```go
// internal/lexer/lexer.go
package lexer

import (
	"unicode"
	"unicode/utf8"

	"github.com/tunarider/fung/internal/diag"
)

type lexer struct {
	file     string
	src      string
	off      int // byte offset of the next rune
	line     int
	col      int // rune column of the next rune, 1-based
	toks     []Token
	comments []Comment
	keywords map[string]Kind
}

// Lex turns source text into tokens. The last token is always EOF. Only
// the first error is reported (08: 문법 에러는 첫 번째 하나만).
func Lex(file, src string) ([]Token, []Comment, *diag.Error) {
	lx := &lexer{file: file, src: src, line: 1, col: 1, keywords: Keywords()}
	if err := lx.run(); err != nil {
		return nil, nil, err
	}
	return lx.toks, lx.comments, nil
}

func (lx *lexer) pos() diag.Pos { return diag.Pos{File: lx.file, Line: lx.line, Col: lx.col} }

// peek returns the next rune without consuming it, or 0 at end of input.
func (lx *lexer) peek() rune { return lx.peekAt(0) }

// peekAt returns the n-th rune ahead (0 = next), or 0 past the end.
func (lx *lexer) peekAt(n int) rune {
	off := lx.off
	for {
		if off >= len(lx.src) {
			return 0
		}
		r, size := utf8.DecodeRuneInString(lx.src[off:])
		if n == 0 {
			return r
		}
		off += size
		n--
	}
}

func (lx *lexer) advance() rune {
	r, size := utf8.DecodeRuneInString(lx.src[lx.off:])
	lx.off += size
	if r == '\n' {
		lx.line++
		lx.col = 1
	} else {
		lx.col++
	}
	return r
}

func (lx *lexer) emit(kind Kind, text string, pos diag.Pos) *Token {
	lx.toks = append(lx.toks, Token{Kind: kind, Text: text, Pos: pos})
	return &lx.toks[len(lx.toks)-1]
}

func (lx *lexer) run() *diag.Error {
	for {
		c := lx.peek()
		switch {
		case c == 0:
			lx.emit(EOF, "", lx.pos())
			return nil
		case c == '\r' && lx.peekAt(1) == '\n':
			// Windows line ending: one newline token at the \r.
			p := lx.pos()
			lx.advance()
			lx.advance()
			lx.emit(Newline, "\n", p)
		case c == ' ' || c == '\t' || c == '\r':
			lx.advance()
		case c == '\n':
			p := lx.pos()
			lx.advance()
			lx.emit(Newline, "\n", p)
		case c == '#':
			lx.comment()
		case isDigit(c):
			if err := lx.number(); err != nil {
				return err
			}
		case c == '"':
			if err := lx.str(); err != nil {
				return err
			}
		case c == '`':
			if err := lx.rawStr(); err != nil {
				return err
			}
		case c == '\'':
			if err := lx.symbol(); err != nil {
				return err
			}
		case isIdentStart(c):
			lx.ident()
		default:
			if err := lx.punct(); err != nil {
				return err
			}
		}
	}
}

func (lx *lexer) comment() {
	p := lx.pos()
	start := lx.off
	for lx.peek() != 0 && lx.peek() != '\n' {
		lx.advance()
	}
	lx.comments = append(lx.comments, Comment{Pos: p, Text: lx.src[start:lx.off]})
}

func (lx *lexer) ident() {
	p := lx.pos()
	start := lx.off
	first := lx.advance()
	for isIdentPart(lx.peek()) {
		lx.advance()
	}
	text := lx.src[start:lx.off]
	kind := Name
	if k, ok := lx.keywords[text]; ok {
		kind = k
	} else if unicode.IsUpper(first) {
		kind = Tag
	}
	lx.emit(kind, text, p)
}

// punct tries the longest operator first so that `..=` wins over `..`
// and `;;` over `;` (01: `;;`는 렉서가 항상 가장 길게 자른다).
func (lx *lexer) punct() *diag.Error {
	p := lx.pos()
	r0, r1, r2 := lx.peek(), lx.peekAt(1), lx.peekAt(2)
	if k, ok := punct3[string([]rune{r0, r1, r2})]; ok {
		lx.advance()
		lx.advance()
		lx.advance()
		lx.emit(k, string([]rune{r0, r1, r2}), p)
		return nil
	}
	if k, ok := punct2[string([]rune{r0, r1})]; ok {
		lx.advance()
		lx.advance()
		lx.emit(k, string([]rune{r0, r1}), p)
		return nil
	}
	if k, ok := punct1[r0]; ok {
		lx.advance()
		lx.emit(k, string(r0), p)
		return nil
	}
	return diag.Errorf("E0101", p, string(r0))
}

var punct3 = map[string]Kind{"..=": DotDotEq, "...": Ellipsis}

var punct2 = map[string]Kind{
	"==": Eq, "!=": Ne, "<=": Le, ">=": Ge, "=>": FatArrow, "->": Arrow,
	"//": SlashSlash, "**": StarStar, "|>": Pipe, "??": Coalesce, "..": DotDot, ";;": DoubleSemi,
}

var punct1 = map[rune]Kind{
	'(': LParen, ')': RParen, '[': LBracket, ']': RBracket, '{': LBrace, '}': RBrace,
	',': Comma, ':': Colon, ';': Semi, '.': Dot, '=': Assign, '<': Lt, '>': Gt,
	'+': Plus, '-': Minus, '*': Star, '/': Slash, '%': Percent, '!': Bang, '?': Question, '|': Bar,
}

func isDigit(c rune) bool      { return c >= '0' && c <= '9' }
func isIdentStart(c rune) bool { return c == '_' || unicode.IsLetter(c) }
func isIdentPart(c rune) bool  { return c == '_' || unicode.IsLetter(c) || unicode.IsDigit(c) }

// number, str, rawStr and symbol are implemented in later tasks. Until
// then they report the character as unexpected so this task compiles.
func (lx *lexer) number() *diag.Error { return diag.Errorf("E0101", lx.pos(), string(lx.peek())) }
func (lx *lexer) str() *diag.Error    { return diag.Errorf("E0101", lx.pos(), string(lx.peek())) }
func (lx *lexer) rawStr() *diag.Error { return diag.Errorf("E0101", lx.pos(), string(lx.peek())) }
func (lx *lexer) symbol() *diag.Error { return diag.Errorf("E0101", lx.pos(), string(lx.peek())) }
```

- [ ] **Step 4: 통과 확인**

Run: `gofmt -l . && go vet ./... && go test ./internal/lexer/`
Expected: PASS (number/str 테스트는 아직 없음).

- [ ] **Step 5: 커밋**

```bash
git add internal/lexer
git commit -m "feat(lexer): tokenize punctuation, identifiers, keywords, newlines, and comments

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: lexer — 숫자와 기간

**Files:**
- Create: `internal/lexer/number.go`
- Modify: `internal/lexer/lexer.go` (임시 `number` 스텁 삭제)
- Test: `internal/lexer/number_test.go`

**Interfaces:**
- Consumes: Task 2의 `lexer` 구조체, `emit`, `peek`, `advance`
- Produces: `Int`/`Float`/`Duration` 토큰 (`Token.Int`, `Token.Float`, `Token.Dur` 채움)

- [ ] **Step 1: 실패하는 테스트 작성**

```go
// internal/lexer/number_test.go
package lexer

import (
	"testing"
	"time"
)

func one(t *testing.T, src string) Token {
	t.Helper()
	toks, _, err := Lex("t.fung", src)
	if err != nil {
		t.Fatalf("Lex(%q): %v", src, err)
	}
	if len(toks) != 2 {
		t.Fatalf("Lex(%q) = %v, want one token + EOF", src, toks)
	}
	return toks[0]
}

func TestIntegerLiterals(t *testing.T) {
	cases := map[string]int64{
		"0": 0, "42": 42, "1_000_000": 1000000, "0xff": 255, "0XFF": 255,
		"0o17": 15, "0b1010": 10, "0b1_0": 2, "9223372036854775807": 9223372036854775807,
	}
	for src, want := range cases {
		tok := one(t, src)
		if tok.Kind != Int || tok.Int != want {
			t.Errorf("%q = %v %d, want Int %d", src, tok.Kind, tok.Int, want)
		}
	}
}

func TestFloatLiterals(t *testing.T) {
	cases := map[string]float64{
		"3.14": 3.14, "1e9": 1e9, "2.5e-3": 2.5e-3, "1E+2": 100, "1_0.5": 10.5, "0.1": 0.1,
	}
	for src, want := range cases {
		tok := one(t, src)
		if tok.Kind != Float || tok.Float != want {
			t.Errorf("%q = %v %g, want Float %g", src, tok.Kind, tok.Float, want)
		}
	}
}

func TestDurationLiterals(t *testing.T) {
	cases := map[string]time.Duration{
		"500ms": 500 * time.Millisecond, "30s": 30 * time.Second, "1.5s": 1500 * time.Millisecond,
		"1h30m": 90 * time.Minute, "2h": 2 * time.Hour, "1m30s500ms": time.Minute + 30*time.Second + 500*time.Millisecond,
	}
	for src, want := range cases {
		tok := one(t, src)
		if tok.Kind != Duration || tok.Dur != want {
			t.Errorf("%q = %v %v, want Duration %v", src, tok.Kind, tok.Dur, want)
		}
	}
}

func TestNumberFollowedByOperatorOrRange(t *testing.T) {
	if got := kinds(t, "1..3"); !equalKinds(got, []Kind{Int, DotDot, Int}) {
		t.Errorf("1..3 = %v", got)
	}
	if got := kinds(t, "1.5.3"); !equalKinds(got, []Kind{Float, Dot, Int}) {
		t.Errorf("1.5.3 = %v", got)
	}
	if got := kinds(t, "30 s"); !equalKinds(got, []Kind{Int, Name}) {
		t.Errorf("30 s = %v", got)
	}
	if got := kinds(t, "1+2"); !equalKinds(got, []Kind{Int, Plus, Int}) {
		t.Errorf("1+2 = %v", got)
	}
}

func TestMalformedNumbers(t *testing.T) {
	cases := map[string]string{
		"30abc":                "E0102",
		"1__0":                 "E0102",
		"1_":                   "E0102",
		"0x":                   "E0102",
		"0xg":                  "E0102",
		"9223372036854775808":  "E0102", // int64 max + 1
		"1e":                   "E0102",
		"1h30":                 "E0105", // trailing component without unit
		"30m1h":                "E0105", // units must descend
		"5x":                   "E0102",
		"1e5s":                 "E0102", // no exponent inside a duration component
	}
	for src, want := range cases {
		_, _, err := Lex("t.fung", src)
		if err == nil || err.Code != want {
			t.Errorf("%q: err = %v, want %s", src, err, want)
		}
	}
}
```

- [ ] **Step 2: 실패 확인**

Run: `go test ./internal/lexer/ -run 'Integer|Float|Duration|Number|Malformed'`
Expected: FAIL (스텁이 E0101을 냄).

- [ ] **Step 3: 구현** (`lexer.go`의 `number` 스텁 줄을 지우고 아래 파일 추가)

```go
// internal/lexer/number.go
package lexer

import (
	"strconv"
	"strings"
	"time"

	"github.com/tunarider/fung/internal/diag"
)

// number lexes integers (decimal, 0x, 0o, 0b, with _ separators), floats
// (digits on both sides of the point, optional exponent) and durations
// (number immediately followed by ms/s/m/h, possibly compound: 1h30m).
func (lx *lexer) number() *diag.Error {
	p := lx.pos()
	start := lx.off

	if lx.peek() == '0' {
		switch lx.peekAt(1) {
		case 'x', 'X', 'o', 'O', 'b', 'B':
			return lx.prefixedInt(p, start)
		}
	}

	if !lx.digits() {
		return diag.Errorf("E0102", p, lx.src[start:lx.off])
	}
	isFloat := false
	if lx.peek() == '.' && isDigit(lx.peekAt(1)) {
		lx.advance()
		isFloat = true
		if !lx.digits() {
			return diag.Errorf("E0102", p, lx.src[start:lx.off])
		}
	}
	hasExp := false
	if c := lx.peek(); c == 'e' || c == 'E' {
		next := lx.peekAt(1)
		if isDigit(next) || ((next == '+' || next == '-') && isDigit(lx.peekAt(2))) {
			lx.advance()
			if lx.peek() == '+' || lx.peek() == '-' {
				lx.advance()
			}
			if !lx.digits() {
				return diag.Errorf("E0102", p, lx.src[start:lx.off])
			}
			isFloat, hasExp = true, true
		}
	}

	// A letter right after the number means either a duration unit or junk.
	if isIdentStart(lx.peek()) {
		if hasExp {
			return diag.Errorf("E0102", p, lx.src[start:lx.off]+lx.identText())
		}
		return lx.duration(p, start)
	}

	text := strings.ReplaceAll(lx.src[start:lx.off], "_", "")
	if isFloat {
		f, err := strconv.ParseFloat(text, 64)
		if err != nil {
			return diag.Errorf("E0102", p, lx.src[start:lx.off])
		}
		tok := lx.emit(Float, lx.src[start:lx.off], p)
		tok.Float = f
		return nil
	}
	n, err := strconv.ParseInt(text, 10, 64)
	if err != nil {
		return diag.Errorf("E0102", p, lx.src[start:lx.off])
	}
	tok := lx.emit(Int, lx.src[start:lx.off], p)
	tok.Int = n
	return nil
}

// digits consumes `digit (digit | _ digit)*` and reports whether at least
// one digit was read and no `_` was left dangling or doubled.
func (lx *lexer) digits() bool {
	if !isDigit(lx.peek()) {
		return false
	}
	for {
		lx.advance()
		if lx.peek() == '_' {
			if !isDigit(lx.peekAt(1)) {
				lx.advance() // consume so the error text shows it
				return false
			}
			lx.advance()
			continue
		}
		if !isDigit(lx.peek()) {
			return true
		}
	}
}

func (lx *lexer) prefixedInt(p diag.Pos, start int) *diag.Error {
	lx.advance() // 0
	base := 16
	switch lx.advance() {
	case 'o', 'O':
		base = 8
	case 'b', 'B':
		base = 2
	}
	digStart := lx.off
	for isHexDigit(lx.peek()) || (lx.peek() == '_' && isHexDigit(lx.peekAt(1))) {
		lx.advance()
	}
	if lx.off == digStart || isIdentStart(lx.peek()) || isDigit(lx.peek()) {
		lx.identText()
		return diag.Errorf("E0102", p, lx.src[start:lx.off])
	}
	text := strings.ReplaceAll(lx.src[digStart:lx.off], "_", "")
	n, err := strconv.ParseInt(text, base, 64)
	if err != nil {
		return diag.Errorf("E0102", p, lx.src[start:lx.off])
	}
	tok := lx.emit(Int, lx.src[start:lx.off], p)
	tok.Int = n
	return nil
}

func isHexDigit(c rune) bool {
	return isDigit(c) || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F')
}

// identText consumes identifier characters and returns them (used to make
// error messages show the whole bad token, e.g. "30abc").
func (lx *lexer) identText() string {
	start := lx.off
	for isIdentPart(lx.peek()) {
		lx.advance()
	}
	return lx.src[start:lx.off]
}

var durationUnits = map[string]struct {
	rank int
	unit time.Duration
}{
	"h": {3, time.Hour}, "m": {2, time.Minute}, "s": {1, time.Second}, "ms": {0, time.Millisecond},
}

// duration continues after the first numeric component has been consumed
// (lx.off points at its unit letters). Components must use descending
// units: 1h30m is fine, 30m1h is not. A first component whose suffix is
// not a unit at all (30abc, 5x) is a malformed number, not a duration.
func (lx *lexer) duration(p diag.Pos, start int) *diag.Error {
	bad := func(code string) *diag.Error {
		lx.identText()
		return diag.Errorf(code, p, lx.src[start:lx.off])
	}
	var total time.Duration
	prevRank := 4
	compStart := start
	first := true
	for {
		numText := strings.ReplaceAll(lx.src[compStart:lx.off], "_", "")
		unitStart := lx.off
		for c := lx.peek(); c >= 'a' && c <= 'z'; c = lx.peek() {
			lx.advance()
		}
		u, ok := durationUnits[lx.src[unitStart:lx.off]]
		if !ok && first {
			return bad("E0102")
		}
		if !ok || u.rank >= prevRank {
			return bad("E0105")
		}
		f, err := strconv.ParseFloat(numText, 64)
		if err != nil {
			return bad("E0105")
		}
		total += time.Duration(f * float64(u.unit))
		prevRank = u.rank
		first = false
		if !isDigit(lx.peek()) {
			break
		}
		compStart = lx.off
		if !lx.digits() {
			return bad("E0105")
		}
		if lx.peek() == '.' && isDigit(lx.peekAt(1)) {
			lx.advance()
			lx.digits()
		}
		if !isIdentStart(lx.peek()) {
			return bad("E0105")
		}
	}
	if isIdentPart(lx.peek()) {
		return bad("E0105")
	}
	tok := lx.emit(Duration, lx.src[start:lx.off], p)
	tok.Dur = total
	return nil
}
```

- [ ] **Step 4: 통과 확인**

Run: `gofmt -l . && go vet ./... && go test ./internal/lexer/`
Expected: PASS

- [ ] **Step 5: 커밋**

```bash
git add internal/lexer
git commit -m "feat(lexer): lex integer, float, and duration literals

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: lexer — 문자열, 원시 문자열, 심볼

**Files:**
- Create: `internal/lexer/string.go`
- Modify: `internal/lexer/lexer.go` (`str`, `rawStr`, `symbol` 스텁 삭제)
- Test: `internal/lexer/string_test.go`

**Interfaces:**
- Produces: `String` 토큰의 `Parts` (텍스트 조각과 `${}` 코드 조각, 코드 조각의 `Pos`는 `${` 바로 뒤 글자의 위치), `RawString` 토큰의 `Text`(내용만), `Symbol` 토큰의 `Text`(따옴표 제외)

- [ ] **Step 1: 실패하는 테스트 작성**

```go
// internal/lexer/string_test.go
package lexer

import (
	"testing"

	"github.com/tunarider/fung/internal/diag"
)

func TestPlainStringAndEscapes(t *testing.T) {
	tok := one(t, `"a\tb\n\\\"\$\u{1F600}"`)
	if tok.Kind != String || len(tok.Parts) != 1 || tok.Parts[0].IsCode {
		t.Fatalf("tok = %+v", tok)
	}
	if want := "a\tb\n\\\"$\U0001F600"; tok.Parts[0].Text != want {
		t.Fatalf("text = %q, want %q", tok.Parts[0].Text, want)
	}
}

func TestInterpolationParts(t *testing.T) {
	tok := one(t, `"x = ${x + 1}, m = ${m["k"]}!"`)
	want := []Part{
		{Text: "x = "},
		{IsCode: true, Text: "x + 1", Pos: diag.Pos{File: "t.fung", Line: 1, Col: 8}},
		{Text: ", m = "},
		{IsCode: true, Text: `m["k"]`, Pos: diag.Pos{File: "t.fung", Line: 1, Col: 22}},
		{Text: "!"},
	}
	if len(tok.Parts) != len(want) {
		t.Fatalf("parts = %+v", tok.Parts)
	}
	for i, w := range want {
		got := tok.Parts[i]
		if got.IsCode != w.IsCode || got.Text != w.Text || (w.IsCode && got.Pos != w.Pos) {
			t.Errorf("part %d = %+v, want %+v", i, got, w)
		}
	}
}

func TestNestedBracesInsideInterpolation(t *testing.T) {
	tok := one(t, `"${ {a: 1}.a }"`)
	if len(tok.Parts) != 1 || tok.Parts[0].Text != " {a: 1}.a " {
		t.Fatalf("parts = %+v", tok.Parts)
	}
}

func TestEmptyString(t *testing.T) {
	tok := one(t, `""`)
	if tok.Kind != String || len(tok.Parts) != 0 {
		t.Fatalf("tok = %+v", tok)
	}
}

func TestRawString(t *testing.T) {
	tok := one(t, "`C:\\new\nline ${x}`")
	if tok.Kind != RawString || tok.Text != "C:\\new\nline ${x}" {
		t.Fatalf("tok = %+v", tok)
	}
	toks, _, _ := Lex("t.fung", "`a\nb` c")
	if toks[1].Pos != (diag.Pos{File: "t.fung", Line: 2, Col: 4}) {
		t.Fatalf("token after multi-line raw string at %v", toks[1].Pos)
	}
}

func TestSymbols(t *testing.T) {
	toks, _, err := Lex("t.fung", "'ok 'not_found 'Red")
	if err != nil {
		t.Fatal(err)
	}
	for i, want := range []string{"ok", "not_found", "Red"} {
		if toks[i].Kind != Symbol || toks[i].Text != want {
			t.Errorf("token %d = %v, want Symbol %q", i, toks[i], want)
		}
	}
}

func TestStringErrors(t *testing.T) {
	cases := map[string]string{
		`"abc`:        "E0104",
		"\"a\nb\"":    "E0104", // literal newline
		`"\q"`:        "E0103",
		`"\u{zz}"`:    "E0103",
		`"${x"`:       "E0104",
		"`abc":        "E0104",
		"'":           "E0101",
		"' x":         "E0101",
	}
	for src, want := range cases {
		_, _, err := Lex("t.fung", src)
		if err == nil || err.Code != want {
			t.Errorf("%q: err = %v, want %s", src, err, want)
		}
	}
}
```

- [ ] **Step 2: 실패 확인**

Run: `go test ./internal/lexer/ -run 'String|Interpolation|Nested|Raw|Symbol'`
Expected: FAIL

- [ ] **Step 3: 구현** (`lexer.go`의 `str`/`rawStr`/`symbol` 스텁 줄 삭제)

```go
// internal/lexer/string.go
package lexer

import (
	"strconv"
	"strings"

	"github.com/tunarider/fung/internal/diag"
)

// str lexes a "..." string into text and ${code} parts. Escapes are the
// closed list from 01; anything else is E0103. A literal newline inside
// the quotes is E0104 (use a raw string or \n).
func (lx *lexer) str() *diag.Error {
	p := lx.pos()
	start := lx.off
	lx.advance() // opening quote
	tok := Token{Kind: String, Pos: p}
	var text strings.Builder
	flush := func() {
		if text.Len() > 0 {
			tok.Parts = append(tok.Parts, Part{Text: text.String()})
			text.Reset()
		}
	}
	for {
		c := lx.peek()
		switch {
		case c == 0 || c == '\n':
			return diag.Errorf("E0104", p)
		case c == '"':
			lx.advance()
			flush()
			tok.Text = lx.src[start:lx.off]
			lx.toks = append(lx.toks, tok)
			return nil
		case c == '\\':
			r, err := lx.escape()
			if err != nil {
				return err
			}
			text.WriteRune(r)
		case c == '$' && lx.peekAt(1) == '{':
			lx.advance()
			lx.advance()
			flush()
			codePos := lx.pos()
			codeStart := lx.off
			if err := lx.skipInterpolation(p); err != nil {
				return err
			}
			tok.Parts = append(tok.Parts, Part{IsCode: true, Text: lx.src[codeStart : lx.off-1], Pos: codePos})
		default:
			text.WriteRune(lx.advance())
		}
	}
}

// escape decodes one backslash escape; lx.off is at the backslash.
func (lx *lexer) escape() (rune, *diag.Error) {
	p := lx.pos()
	lx.advance() // backslash
	c := lx.advance()
	switch c {
	case 'n':
		return '\n', nil
	case 't':
		return '\t', nil
	case 'r':
		return '\r', nil
	case '\\':
		return '\\', nil
	case '"':
		return '"', nil
	case '$':
		return '$', nil
	case 'u':
		if lx.peek() != '{' {
			return 0, diag.Errorf("E0103", p, "u")
		}
		lx.advance()
		hexStart := lx.off
		for isHexDigit(lx.peek()) {
			lx.advance()
		}
		hex := lx.src[hexStart:lx.off]
		if lx.peek() != '}' || len(hex) == 0 || len(hex) > 6 {
			return 0, diag.Errorf("E0103", p, "u{"+hex)
		}
		lx.advance()
		n, err := strconv.ParseUint(hex, 16, 32)
		if err != nil || n > 0x10FFFF {
			return 0, diag.Errorf("E0103", p, "u{"+hex+"}")
		}
		return rune(n), nil
	}
	return 0, diag.Errorf("E0103", p, string(c))
}

// skipInterpolation consumes the code of a ${...} up to and including its
// closing brace, tracking nested braces and skipping over nested string
// literals so a quote inside them does not end the outer string.
func (lx *lexer) skipInterpolation(strPos diag.Pos) *diag.Error {
	depth := 1
	for {
		switch c := lx.peek(); c {
		case 0:
			return diag.Errorf("E0104", strPos)
		case '{':
			depth++
			lx.advance()
		case '}':
			depth--
			lx.advance()
			if depth == 0 {
				return nil
			}
		case '"':
			if err := lx.skipQuoted(strPos); err != nil {
				return err
			}
		case '`':
			lx.advance()
			for lx.peek() != '`' {
				if lx.peek() == 0 {
					return diag.Errorf("E0104", strPos)
				}
				lx.advance()
			}
			lx.advance()
		default:
			lx.advance()
		}
	}
}

// skipQuoted skips a nested "..." literal inside an interpolation,
// honoring escapes and its own ${} parts.
func (lx *lexer) skipQuoted(strPos diag.Pos) *diag.Error {
	lx.advance() // opening quote
	for {
		switch c := lx.peek(); c {
		case 0, '\n':
			return diag.Errorf("E0104", strPos)
		case '\\':
			lx.advance()
			lx.advance()
		case '"':
			lx.advance()
			return nil
		case '$':
			lx.advance()
			if lx.peek() == '{' {
				lx.advance()
				if err := lx.skipInterpolation(strPos); err != nil {
					return err
				}
			}
		default:
			lx.advance()
		}
	}
}

// rawStr lexes a `...` string: no escapes, no interpolation, newlines allowed.
func (lx *lexer) rawStr() *diag.Error {
	p := lx.pos()
	lx.advance()
	start := lx.off
	for lx.peek() != '`' {
		if lx.peek() == 0 {
			return diag.Errorf("E0104", p)
		}
		lx.advance()
	}
	text := lx.src[start:lx.off]
	lx.advance()
	lx.emit(RawString, text, p)
	return nil
}

// symbol lexes 'name.
func (lx *lexer) symbol() *diag.Error {
	p := lx.pos()
	lx.advance()
	if !isIdentStart(lx.peek()) {
		return diag.Errorf("E0101", p, "'")
	}
	start := lx.off
	for isIdentPart(lx.peek()) {
		lx.advance()
	}
	lx.emit(Symbol, lx.src[start:lx.off], p)
	return nil
}
```

- [ ] **Step 4: 통과 확인**

Run: `gofmt -l . && go vet ./... && go test ./internal/lexer/`
Expected: PASS

- [ ] **Step 5: 커밋**

```bash
git add internal/lexer
git commit -m "feat(lexer): lex strings with escapes and interpolation, raw strings, and symbols

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: tree — 균일 트리와 S-식 출력

**Files:**
- Create: `internal/tree/node.go`
- Test: `internal/tree/node_test.go`

**Interfaces:**
- Produces: `type Node struct{Head string; Pos diag.Pos; Lit any; Kids []*Node}`
- Produces: `func New(head string, pos diag.Pos, kids ...*Node) *Node`, `func Leaf(head string, pos diag.Pos, lit any) *Node`, `func WithLit(head string, pos diag.Pos, lit any, kids ...*Node) *Node`
- Produces: `func (n *Node) String() string` — S-식. 잎 중 머리가 `int` `float` `bool` `name`이면 값만, 그 외 잎은 `(head lit)`, 가지는 `(head [lit] kids…)`.

- [ ] **Step 1: 실패하는 테스트 작성**

```go
// internal/tree/node_test.go
package tree

import (
	"testing"

	"github.com/tunarider/fung/internal/diag"
)

func TestStringFormatsSExpressions(t *testing.T) {
	p := diag.Pos{}
	one := Leaf("int", p, int64(1))
	two := Leaf("int", p, int64(2))
	three := Leaf("int", p, int64(3))
	mul := WithLit("binary", p, "*", two, three)
	add := WithLit("binary", p, "+", one, mul)
	call := New("call", p, Leaf("name", p, "print"), add)
	if got, want := call.String(), "(call print (binary + 1 (binary * 2 3)))"; got != want {
		t.Fatalf("String() = %q, want %q", got, want)
	}
}

func TestStringFormatsLeaves(t *testing.T) {
	p := diag.Pos{}
	cases := map[*Node]string{
		Leaf("float", p, 2.5):            "2.5",
		Leaf("float", p, 1.0):            "1.0",
		Leaf("bool", p, true):            "true",
		Leaf("prelude", p, "print"):      "(prelude print)",
		Leaf("string", p, "a b"):         `(string "a b")`,
		New("do", p):                     "(do)",
		New("call", p, Leaf("name", p, "f")): "(call f)",
	}
	for n, want := range cases {
		if got := n.String(); got != want {
			t.Errorf("%+v: String() = %q, want %q", n, got, want)
		}
	}
}
```

- [ ] **Step 2: 실패 확인**

Run: `go test ./internal/tree/`
Expected: FAIL

- [ ] **Step 3: 구현**

```go
// Package tree is the uniform tree every front-end stage works on. The
// parser produces surface forms, desugar rewrites them into core forms,
// and the compiler reads core forms. One node shape covers all of them
// ("겉은 Go, 속은 Lisp"): a head word, a position, an optional literal
// payload, and children.
package tree

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/tunarider/fung/internal/diag"
)

type Node struct {
	Head string
	Pos  diag.Pos
	Lit  any // int64, float64, bool, string, time.Duration, or nil
	Kids []*Node
}

func New(head string, pos diag.Pos, kids ...*Node) *Node {
	return &Node{Head: head, Pos: pos, Kids: kids}
}

func Leaf(head string, pos diag.Pos, lit any) *Node {
	return &Node{Head: head, Pos: pos, Lit: lit}
}

func WithLit(head string, pos diag.Pos, lit any, kids ...*Node) *Node {
	return &Node{Head: head, Pos: pos, Lit: lit, Kids: kids}
}

// bareLeaves print as their value alone; other leaves print as (head lit).
var bareLeaves = map[string]bool{"int": true, "float": true, "bool": true, "name": true}

// String renders the node as an S-expression, the format of `fung dump`.
func (n *Node) String() string {
	var b strings.Builder
	n.write(&b)
	return b.String()
}

func (n *Node) write(b *strings.Builder) {
	if len(n.Kids) == 0 && n.Lit != nil {
		if bareLeaves[n.Head] {
			b.WriteString(litString(n.Lit))
			return
		}
		lit := litString(n.Lit)
		if n.Head == "string" {
			lit = strconv.Quote(n.Lit.(string))
		}
		b.WriteString("(" + n.Head + " " + lit + ")")
		return
	}
	b.WriteString("(" + n.Head)
	if n.Lit != nil {
		b.WriteString(" " + litString(n.Lit))
	}
	for _, k := range n.Kids {
		b.WriteByte(' ')
		k.write(b)
	}
	b.WriteByte(')')
}

func litString(lit any) string {
	switch v := lit.(type) {
	case float64:
		s := strconv.FormatFloat(v, 'g', -1, 64)
		if !strings.ContainsAny(s, ".eIN") {
			s += ".0"
		}
		return s
	case string:
		return v
	}
	return fmt.Sprint(lit)
}
```

- [ ] **Step 4: 통과 확인**

Run: `gofmt -l . && go vet ./... && go test ./internal/tree/`
Expected: PASS

- [ ] **Step 5: 커밋**

```bash
git add internal/tree
git commit -m "feat(tree): add the uniform tree node and S-expression printer

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: parser — Pratt 식 파서와 문장

**Files:**
- Create: `internal/parser/parser.go`, `internal/parser/expr.go`
- Test: `internal/parser/parser_test.go`

**Interfaces:**
- Consumes: `lexer.Token`, `lexer.Kind`, `tree.New/Leaf/WithLit`, `diag.Errorf`
- Produces: `func Parse(toks []lexer.Token) (*tree.Node, *diag.Error)` — 루트는 `(program stmt…)`.
- Produces (겉 문법 트리 형태, desugar가 소비):
  - `int`/`float`/`bool` 잎 (`Lit`: int64/float64/bool), `name` 잎 (`Lit`: string)
  - `(binary <op> a b)`: `Lit`은 연산자 글자 `"+" "-" "*" "/" "//" "%" "**" "==" "!=" "<" ">" "<=" ">=" "and" "or"`
  - `(unary <op> a)`: `Lit`은 `"-"` 또는 `"not"`
  - `(call f args…)`
- Produces: 우선순위 표 `bindingPower(kind) (left, right int, ok bool)`

**규칙 (01·07에서)**: 비교는 연쇄 불가(E0111). `**`는 오른쪽 결합이고 단항 `-`보다 세다. 괄호 안에서는 줄바꿈 무시. 줄이 이항 연산자로 끝나거나 다음 줄이 `and`/`or`(M1에서 존재하는 이어짐 토큰; 표에는 `|>` `.` `??` `then` `else`도 넣는다)로 시작하면 이어진다. 호출 `(`는 같은 줄에서만 후위로 인정한다.

- [ ] **Step 1: 실패하는 테스트 작성**

```go
// internal/parser/parser_test.go
package parser

import (
	"testing"

	"github.com/tunarider/fung/internal/lexer"
)

func parse(t *testing.T, src string) string {
	t.Helper()
	toks, _, err := lexer.Lex("t.fung", src)
	if err != nil {
		t.Fatalf("lex %q: %v", src, err)
	}
	n, perr := Parse(toks)
	if perr != nil {
		t.Fatalf("parse %q: %v", src, perr)
	}
	return n.String()
}

func parseErr(t *testing.T, src string) string {
	t.Helper()
	toks, _, err := lexer.Lex("t.fung", src)
	if err != nil {
		t.Fatalf("lex %q: %v", src, err)
	}
	_, perr := Parse(toks)
	if perr == nil {
		t.Fatalf("parse %q: want error", src)
	}
	return perr.Code
}

func TestPrecedenceAndAssociativity(t *testing.T) {
	cases := map[string]string{
		"1 + 2 * 3":       "(program (binary + 1 (binary * 2 3)))",
		"(1 + 2) * 3":     "(program (binary * (binary + 1 2) 3))",
		"1 - 2 - 3":       "(program (binary - (binary - 1 2) 3))",
		"2 ** 3 ** 2":     "(program (binary ** 2 (binary ** 3 2)))",
		"-2 ** 2":         "(program (unary - (binary ** 2 2)))",
		"2 ** -1":         "(program (binary ** 2 (unary - 1)))",
		"-7 // 2":         "(program (binary // (unary - 7) 2))",
		"7 % 3 / 2":       "(program (binary / (binary % 7 3) 2))",
		"1 < 2 and 2 < 3": "(program (binary and (binary < 1 2) (binary < 2 3)))",
		"not a or b":      "(program (binary or (unary not a) b))",
		"not a == b":      "(program (unary not (binary == a b)))",
		"a or b and c":    "(program (binary or a (binary and b c)))",
		"1 == 1.0":        "(program (binary == 1 1.0))",
		"true != false":   "(program (binary != true false))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestCalls(t *testing.T) {
	cases := map[string]string{
		"print(1 + 2 * 3)":  "(program (call print (binary + 1 (binary * 2 3))))",
		"print()":           "(program (call print))",
		"print(1, 2,)":      "(program (call print 1 2))",
		"f(1)(2)":           "(program (call (call f 1) 2))",
		"print(\n  1,\n  2\n)": "(program (call print 1 2))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestStatementsAndNewlines(t *testing.T) {
	cases := map[string]string{
		"":                          "(program)",
		"\n\n":                      "(program)",
		"1\n2":                      "(program 1 2)",
		"1; 2;":                     "(program 1 2)",
		"1 +\n\n  2":                "(program (binary + 1 2))",
		"true\n  and false":         "(program (binary and true false))",
		"1\n-2":                     "(program 1 (unary - 2))",
		"f\n(1)":                    "(program f 1)",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestSyntaxErrors(t *testing.T) {
	cases := map[string]string{
		"1 < 2 < 3":   "E0111",
		"a == b == c": "E0111",
		"1 +":         "E0110",
		"(1":          "E0110",
		"1 2":         "E0110",
		")":           "E0110",
		"print(1,,2)": "E0110",
		"1 ;; 2":      "E0110",
		"\"s\"":       "E0110", // strings arrive in M2
	}
	for src, want := range cases {
		if got := parseErr(t, src); got != want {
			t.Errorf("%q: code = %s, want %s", src, got, want)
		}
	}
}

func TestErrorPositionPointsAtOffendingToken(t *testing.T) {
	toks, _, _ := lexer.Lex("t.fung", "print(1 2)")
	_, err := Parse(toks)
	if err == nil || err.Pos.Line != 1 || err.Pos.Col != 9 {
		t.Fatalf("err = %v", err)
	}
}
```

- [ ] **Step 2: 실패 확인**

Run: `go test ./internal/parser/`
Expected: FAIL

- [ ] **Step 3: 구현**

```go
// internal/parser/parser.go
package parser

import (
	"fmt"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/tree"
)

// Parser is a recursive-descent parser for statements with a Pratt loop
// for expressions. Newlines are tokens; the parser decides from context
// whether one ends a statement (01. 줄 이어짐 규칙).
type parser struct {
	toks []lexer.Token
	i    int
	nest int // depth of ( [ { — newlines are whitespace inside
}

func Parse(toks []lexer.Token) (*tree.Node, *diag.Error) {
	p := &parser{toks: toks}
	prog, err := p.program()
	if err != nil {
		return nil, err
	}
	return prog, nil
}

// peek returns the next significant token. Inside brackets, newlines are
// skipped here so every caller sees them as whitespace.
func (p *parser) peek() lexer.Token {
	for p.nest > 0 && p.toks[p.i].Kind == lexer.Newline {
		p.i++
	}
	return p.toks[p.i]
}

func (p *parser) next() lexer.Token {
	t := p.peek()
	if t.Kind != lexer.EOF {
		p.i++
	}
	return t
}

func (p *parser) at(k lexer.Kind) bool { return p.peek().Kind == k }

func (p *parser) expect(k lexer.Kind) (lexer.Token, *diag.Error) {
	if !p.at(k) {
		return lexer.Token{}, p.unexpected()
	}
	return p.next(), nil
}

// unexpected builds E0110 for the current token.
func (p *parser) unexpected() *diag.Error {
	t := p.peek()
	return diag.Errorf("E0110", t.Pos, describe(t))
}

func describe(t lexer.Token) string {
	switch t.Kind {
	case lexer.EOF, lexer.Newline:
		return t.Kind.String()
	case lexer.Int, lexer.Float, lexer.Duration, lexer.String, lexer.RawString:
		return fmt.Sprintf("%v `%s`", t.Kind, t.Text)
	case lexer.Name, lexer.Tag:
		return fmt.Sprintf("%v `%s`", t.Kind, t.Text)
	case lexer.Symbol:
		return fmt.Sprintf("symbol `'%s`", t.Text)
	}
	return "`" + t.Kind.String() + "`"
}

func (p *parser) skipSeps() {
	for p.at(lexer.Newline) || p.at(lexer.Semi) {
		p.next()
	}
}

// program = body at nesting 0, terminated by EOF.
func (p *parser) program() (*tree.Node, *diag.Error) {
	prog := tree.New("program", p.peek().Pos)
	p.skipSeps()
	for !p.at(lexer.EOF) {
		stmt, err := p.statement()
		if err != nil {
			return nil, err
		}
		prog.Kids = append(prog.Kids, stmt)
		if !p.at(lexer.EOF) && !p.at(lexer.Newline) && !p.at(lexer.Semi) {
			return nil, p.unexpected()
		}
		p.skipSeps()
	}
	return prog, nil
}

// statement: in M1 every statement is an expression.
func (p *parser) statement() (*tree.Node, *diag.Error) {
	return p.expr(0)
}
```

```go
// internal/parser/expr.go
package parser

import (
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/tree"
)

// Binding powers from 07. 우선순위 (낮음 → 높음), scaled by 10 so that
// right-associative and non-associative operators can be expressed by
// giving the right side a different power than the left.
const (
	bpThen     = 10
	bpOr       = 20
	bpAnd      = 30
	bpNot      = 40
	bpCompare  = 50
	bpPipe     = 60
	bpCoalesce = 70
	bpRange    = 80
	bpAdd      = 90
	bpMul      = 100
	bpNeg      = 110
	bpPow      = 120
	bpPostfix  = 130
)

type infixInfo struct {
	left, right int
	text        string
	compare     bool
}

// infix lists the binary operators M1 understands. Later milestones add
// |>, ??, .., ..=, in, then here.
var infix = map[lexer.Kind]infixInfo{
	lexer.KwOr:       {bpOr, bpOr + 1, "or", false},
	lexer.KwAnd:      {bpAnd, bpAnd + 1, "and", false},
	lexer.Eq:         {bpCompare, bpCompare + 1, "==", true},
	lexer.Ne:         {bpCompare, bpCompare + 1, "!=", true},
	lexer.Lt:         {bpCompare, bpCompare + 1, "<", true},
	lexer.Gt:         {bpCompare, bpCompare + 1, ">", true},
	lexer.Le:         {bpCompare, bpCompare + 1, "<=", true},
	lexer.Ge:         {bpCompare, bpCompare + 1, ">=", true},
	lexer.Plus:       {bpAdd, bpAdd + 1, "+", false},
	lexer.Minus:      {bpAdd, bpAdd + 1, "-", false},
	lexer.Star:       {bpMul, bpMul + 1, "*", false},
	lexer.Slash:      {bpMul, bpMul + 1, "/", false},
	lexer.SlashSlash: {bpMul, bpMul + 1, "//", false},
	lexer.Percent:    {bpMul, bpMul + 1, "%", false},
	lexer.StarStar:   {bpPow, bpPow, "**", false}, // right-assoc: same power on the right
}

// continuesLine lists tokens that may start a continuation line (01).
var continuesLine = map[lexer.Kind]bool{
	lexer.Pipe: true, lexer.Dot: true, lexer.Coalesce: true,
	lexer.KwAnd: true, lexer.KwOr: true, lexer.KwThen: true, lexer.KwElse: true,
}

// peekInfix looks past a newline when the next line starts with a
// continuation token, without consuming anything otherwise.
func (p *parser) peekInfix() lexer.Token {
	t := p.peek()
	if t.Kind != lexer.Newline {
		return t
	}
	j := p.i
	for p.toks[j].Kind == lexer.Newline {
		j++
	}
	if continuesLine[p.toks[j].Kind] {
		p.i = j
		return p.toks[j]
	}
	return t
}

// expr parses an expression whose operators bind at least as tightly as minBP.
func (p *parser) expr(minBP int) (*tree.Node, *diag.Error) {
	lhs, err := p.prefix()
	if err != nil {
		return nil, err
	}
	lastWasCompare := false
	for {
		t := p.peekInfix()
		// Postfix call: only on the same line.
		if t.Kind == lexer.LParen && bpPostfix >= minBP {
			lhs, err = p.call(lhs)
			if err != nil {
				return nil, err
			}
			continue
		}
		info, ok := infix[t.Kind]
		if !ok || info.left < minBP {
			return lhs, nil
		}
		if info.compare && lastWasCompare {
			return nil, diag.Errorf("E0111", t.Pos)
		}
		p.next()
		p.skipNewlinesAfterOperator()
		rhs, err := p.expr(info.right)
		if err != nil {
			return nil, err
		}
		lhs = tree.WithLit("binary", t.Pos, info.text, lhs, rhs)
		lastWasCompare = info.compare
	}
}

// skipNewlinesAfterOperator implements "a line ending in an operator continues".
func (p *parser) skipNewlinesAfterOperator() {
	for p.toks[p.i].Kind == lexer.Newline {
		p.i++
	}
}

func (p *parser) prefix() (*tree.Node, *diag.Error) {
	t := p.peek()
	switch t.Kind {
	case lexer.Int:
		p.next()
		return tree.Leaf("int", t.Pos, t.Int), nil
	case lexer.Float:
		p.next()
		return tree.Leaf("float", t.Pos, t.Float), nil
	case lexer.KwTrue:
		p.next()
		return tree.Leaf("bool", t.Pos, true), nil
	case lexer.KwFalse:
		p.next()
		return tree.Leaf("bool", t.Pos, false), nil
	case lexer.Name:
		p.next()
		return tree.Leaf("name", t.Pos, t.Text), nil
	case lexer.Minus:
		p.next()
		p.skipNewlinesAfterOperator()
		operand, err := p.expr(bpNeg)
		if err != nil {
			return nil, err
		}
		return tree.WithLit("unary", t.Pos, "-", operand), nil
	case lexer.KwNot:
		p.next()
		p.skipNewlinesAfterOperator()
		operand, err := p.expr(bpNot)
		if err != nil {
			return nil, err
		}
		return tree.WithLit("unary", t.Pos, "not", operand), nil
	case lexer.LParen:
		p.next()
		p.nest++
		inner, err := p.expr(0)
		if err != nil {
			return nil, err
		}
		if _, err := p.expect(lexer.RParen); err != nil {
			return nil, err
		}
		p.nest--
		return inner, nil
	}
	return nil, p.unexpected()
}

// call parses `(args)` after a callee. Inside the parentheses newlines
// are whitespace and a trailing comma is allowed.
func (p *parser) call(callee *tree.Node) (*tree.Node, *diag.Error) {
	open := p.next() // (
	p.nest++
	node := tree.New("call", open.Pos, callee)
	for !p.at(lexer.RParen) {
		arg, err := p.expr(0)
		if err != nil {
			return nil, err
		}
		node.Kids = append(node.Kids, arg)
		if p.at(lexer.Comma) {
			p.next()
			continue
		}
		if !p.at(lexer.RParen) {
			return nil, p.unexpected()
		}
	}
	p.next() // )
	p.nest--
	return node, nil
}
```

주의할 점 둘. 첫째, `(1` 같은 미완성 괄호에서 `expect(RParen)` 실패 시 `nest`가 줄지 않지만 에러로 끝나므로 상관없다. 둘째, `-2 ** 2`: `prefix`의 `-`가 `expr(bpNeg=110)`을 부르고, `**`의 left 120 ≥ 110이므로 `2 ** 2`가 먼저 묶인다. `2 ** -1`: `**`의 right 120으로 `expr(120)` → `prefix`가 `-`를 처리. 테스트 기대값과 일치한다.

- [ ] **Step 4: 통과 확인**

Run: `gofmt -l . && go vet ./... && go test ./internal/parser/`
Expected: PASS

- [ ] **Step 5: 커밋**

```bash
git add internal/parser
git commit -m "feat(parser): add Pratt expression parser with calls and line continuation

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: desugar — 겉 문법 트리 → 코어 트리

**Files:**
- Create: `internal/desugar/desugar.go`
- Test: `internal/desugar/desugar_test.go`

**Interfaces:**
- Consumes: Task 6의 겉 문법 형태
- Produces: `func Desugar(n *tree.Node) (*tree.Node, *diag.Error)`
- Produces (코어 형태, 09의 표):
  - `(do stmt…)` ← `(program …)`
  - `(op <sym> a b)` ← `(binary …)` (and/or 제외), `(op - a)`/`(op not a)` ← `(unary …)`, `(op bool a)` (새 단항 코어 연산: bool이 아니면 E0505)
  - `(if c a b)` ← `a and b` → `(if a (op bool b) false)`, `a or b` → `(if a true (op bool b))`
  - `(call f args…)` 그대로, 잎 그대로

- [ ] **Step 1: 실패하는 테스트 작성**

```go
// internal/desugar/desugar_test.go
package desugar

import (
	"testing"

	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/parser"
)

func core(t *testing.T, src string) string {
	t.Helper()
	toks, _, err := lexer.Lex("t.fung", src)
	if err != nil {
		t.Fatal(err)
	}
	surface, perr := parser.Parse(toks)
	if perr != nil {
		t.Fatal(perr)
	}
	c, derr := Desugar(surface)
	if derr != nil {
		t.Fatal(derr)
	}
	return c.String()
}

func TestDesugarArithmeticToOps(t *testing.T) {
	cases := map[string]string{
		"1 + 2 * 3":  "(do (op + 1 (op * 2 3)))",
		"-x":         "(do (op - x))",
		"not x":      "(do (op not x))",
		"print(1)":   "(do (call print 1))",
		"1\n2":       "(do 1 2)",
		"":           "(do)",
		"a == b":     "(do (op == a b))",
	}
	for src, want := range cases {
		if got := core(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestDesugarShortCircuitToIf(t *testing.T) {
	cases := map[string]string{
		"a and b":        "(do (if a (op bool b) false))",
		"a or b":         "(do (if a true (op bool b)))",
		"a and b or c":   "(do (if (if a (op bool b) false) true (op bool c)))",
	}
	for src, want := range cases {
		if got := core(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestDesugarKeepsPositions(t *testing.T) {
	toks, _, _ := lexer.Lex("t.fung", "1 + 2")
	surface, _ := parser.Parse(toks)
	c, _ := Desugar(surface)
	if got := c.Kids[0].Pos.Col; got != 3 {
		t.Fatalf("op node col = %d, want 3 (the operator)", got)
	}
}
```

- [ ] **Step 2: 실패 확인**

Run: `go test ./internal/desugar/`
Expected: FAIL

- [ ] **Step 3: 구현**

```go
// Package desugar rewrites the surface tree the parser builds into the
// smaller core language the compiler understands (09. 코어 언어). Both
// are tree.Node trees; this pass only changes heads and shapes.
package desugar

import (
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/tree"
)

func Desugar(n *tree.Node) (*tree.Node, *diag.Error) {
	switch n.Head {
	case "program":
		out := tree.New("do", n.Pos)
		for _, k := range n.Kids {
			c, err := Desugar(k)
			if err != nil {
				return nil, err
			}
			out.Kids = append(out.Kids, c)
		}
		return out, nil

	case "binary":
		a, err := Desugar(n.Kids[0])
		if err != nil {
			return nil, err
		}
		b, err := Desugar(n.Kids[1])
		if err != nil {
			return nil, err
		}
		switch n.Lit {
		case "and":
			// a and b  →  if a then (b must be bool) else false
			return tree.New("if", n.Pos, a, checkBool(b), tree.Leaf("bool", n.Pos, false)), nil
		case "or":
			// a or b  →  if a then true else (b must be bool)
			return tree.New("if", n.Pos, a, tree.Leaf("bool", n.Pos, true), checkBool(b)), nil
		}
		return tree.WithLit("op", n.Pos, n.Lit, a, b), nil

	case "unary":
		a, err := Desugar(n.Kids[0])
		if err != nil {
			return nil, err
		}
		return tree.WithLit("op", n.Pos, n.Lit, a), nil

	case "call":
		out := tree.New("call", n.Pos)
		for _, k := range n.Kids {
			c, err := Desugar(k)
			if err != nil {
				return nil, err
			}
			out.Kids = append(out.Kids, c)
		}
		return out, nil

	case "int", "float", "bool", "name":
		return n, nil
	}
	return nil, diag.Errorf("E0801", n.Pos, "desugar: unknown node "+n.Head)
}

func checkBool(e *tree.Node) *tree.Node {
	return tree.WithLit("op", e.Pos, "bool", e)
}
```

- [ ] **Step 4: 통과 확인**

Run: `gofmt -l . && go vet ./... && go test ./internal/desugar/`
Expected: PASS

- [ ] **Step 5: 커밋**

```bash
git add internal/desugar
git commit -m "feat(desugar): lower surface operators and short-circuit logic to core forms

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: resolve — 이름 분류와 미정의 이름

**Files:**
- Create: `internal/resolve/resolve.go`
- Test: `internal/resolve/resolve_test.go`

**Interfaces:**
- Consumes: 코어 트리, `name` 잎
- Produces: `func Resolve(core *tree.Node, isPrelude func(string) bool) (*tree.Node, []*diag.Error)` — 새 트리를 돌려준다 (입력은 바꾸지 않음). `name` 잎은 `(prelude <name>)` 잎으로 바뀌거나 E0204가 모인다. 에러가 있어도 끝까지 훑어 **모두** 모은다 (08).

- [ ] **Step 1: 실패하는 테스트 작성**

```go
// internal/resolve/resolve_test.go
package resolve

import (
	"testing"

	"github.com/tunarider/fung/internal/desugar"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/parser"
	"github.com/tunarider/fung/internal/tree"
)

func coreOf(t *testing.T, src string) *tree.Node {
	t.Helper()
	toks, _, err := lexer.Lex("t.fung", src)
	if err != nil {
		t.Fatal(err)
	}
	s, perr := parser.Parse(toks)
	if perr != nil {
		t.Fatal(perr)
	}
	c, derr := desugar.Desugar(s)
	if derr != nil {
		t.Fatal(derr)
	}
	return c
}

var prelude = func(name string) bool { return name == "print" }

func TestResolveMarksPreludeNames(t *testing.T) {
	out, errs := Resolve(coreOf(t, "print(1)"), prelude)
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	if got := out.String(); got != "(do (call (prelude print) 1))" {
		t.Fatalf("got %s", got)
	}
}

func TestResolveCollectsAllUndefinedNames(t *testing.T) {
	_, errs := Resolve(coreOf(t, "print(fiels)\nprint(x + y)"), prelude)
	if len(errs) != 3 {
		t.Fatalf("errs = %v, want 3", errs)
	}
	for i, want := range []string{"fiels", "x", "y"} {
		if errs[i].Code != "E0204" || errs[i].Args[0] != want {
			t.Errorf("errs[%d] = %v, want E0204 %s", i, errs[i], want)
		}
	}
	if errs[1].Pos.Line != 2 || errs[1].Pos.Col != 7 {
		t.Errorf("errs[1].Pos = %v", errs[1].Pos)
	}
}

func TestResolveDoesNotMutateInput(t *testing.T) {
	in := coreOf(t, "print(1)")
	before := in.String()
	Resolve(in, prelude)
	if in.String() != before {
		t.Fatalf("input mutated: %s", in.String())
	}
}
```

- [ ] **Step 2: 실패 확인**

Run: `go test ./internal/resolve/`
Expected: FAIL

- [ ] **Step 3: 구현**

```go
// Package resolve classifies every name in a core tree and reports scope
// errors. In M1 the only binding site is the prelude; M2 adds locals and
// M3 captured values, but the shape stays: walk once, rewrite name leaves
// into what the compiler needs, collect every error (08: 의미 에러는 모두).
package resolve

import (
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/tree"
)

type resolver struct {
	isPrelude func(string) bool
	errs      []*diag.Error
}

func Resolve(core *tree.Node, isPrelude func(string) bool) (*tree.Node, []*diag.Error) {
	r := &resolver{isPrelude: isPrelude}
	out := r.walk(core)
	return out, r.errs
}

func (r *resolver) walk(n *tree.Node) *tree.Node {
	if n.Head == "name" {
		name := n.Lit.(string)
		if r.isPrelude(name) {
			return tree.Leaf("prelude", n.Pos, name)
		}
		r.errs = append(r.errs, diag.Errorf("E0204", n.Pos, name))
		return n
	}
	out := &tree.Node{Head: n.Head, Pos: n.Pos, Lit: n.Lit}
	for _, k := range n.Kids {
		out.Kids = append(out.Kids, r.walk(k))
	}
	return out
}
```

- [ ] **Step 4: 통과 확인**

Run: `gofmt -l . && go vet ./... && go test ./internal/resolve/`
Expected: PASS

- [ ] **Step 5: 커밋**

```bash
git add internal/resolve
git commit -m "feat(resolve): classify prelude names and collect undefined-name errors

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 9: value — 값 표현, 산술, 비교, 출력 형식

**Files:**
- Create: `internal/value/value.go`, `internal/value/format.go`, `internal/value/arith.go`, `internal/value/compare.go`
- Modify: `internal/diag/catalog.go` (E0506 추가)
- Test: `internal/value/format_test.go`, `internal/value/arith_test.go`, `internal/value/compare_test.go`

**Interfaces:**
- Produces: `type Value interface{ TypeName() string }`, `type Int int64`, `type Float float64`, `type Bool bool`, `type Nil struct{}`, `type Native struct{Name string; Fn func(ctx Context, args []Value) (Value, error)}`, `type Context interface{ Stdout() io.Writer }`
- Produces: `func Format(v Value) string` (리터럴 형태), `func FormatForPrint(v Value) string` (M1에선 Format과 같음; M2에서 문자열만 따옴표 없이)
- Produces: `func Add, Sub, Mul, Div, FloorDiv, Mod, Pow (a, b Value) (Value, *diag.Error)`, `func Neg(a Value) (Value, *diag.Error)`, `func Not(a Value) (Value, *diag.Error)`
- Produces: `func Equal(a, b Value) bool`, `func Compare(a, b Value) (int, *diag.Error)`
- 모든 에러의 `Pos`는 비어 있다. VM이 채운다.

- [ ] **Step 1: 실패하는 테스트 작성**

```go
// internal/value/format_test.go
package value

import "testing"

func TestFormatLiterals(t *testing.T) {
	cases := map[Value]string{
		Int(0): "0", Int(-42): "-42", Int(9223372036854775807): "9223372036854775807",
		Float(5): "5.0", Float(2.5): "2.5", Float(-0.25): "-0.25",
		Float(0.1 + 0.2): "0.30000000000000004",
		Float(1e21): "1e21", Float(1e-7): "1e-07", Float(123456789.0): "123456789.0", Float(1e6): "1000000.0",
		Bool(true): "true", Bool(false): "false", Nil{}: "nil",
		&Native{Name: "print"}: "<fn print>",
	}
	for v, want := range cases {
		if got := Format(v); got != want {
			t.Errorf("Format(%#v) = %q, want %q", v, got, want)
		}
	}
}

func TestTypeNames(t *testing.T) {
	cases := map[Value]string{Int(1): "int", Float(1): "float", Bool(true): "bool", Nil{}: "nil", &Native{}: "fn"}
	for v, want := range cases {
		if got := v.TypeName(); got != want {
			t.Errorf("%#v.TypeName() = %q, want %q", v, got, want)
		}
	}
}
```

```go
// internal/value/arith_test.go
package value

import (
	"math"
	"testing"

	"github.com/tunarider/fung/internal/diag"
)

type binop func(a, b Value) (Value, *diag.Error)

func ok(t *testing.T, f binop, a, b Value, want Value) {
	t.Helper()
	got, err := f(a, b)
	if err != nil {
		t.Fatalf("%v op %v: %v", a, b, err)
	}
	if got != want {
		t.Fatalf("%v op %v = %#v, want %#v", a, b, got, want)
	}
}

func fails(t *testing.T, f binop, a, b Value, code string) {
	t.Helper()
	_, err := f(a, b)
	if err == nil || err.Code != code {
		t.Fatalf("%v op %v: err = %v, want %s", a, b, err, code)
	}
}

func TestAddSubMul(t *testing.T) {
	ok(t, Add, Int(1), Int(2), Int(3))
	ok(t, Add, Int(1), Float(2.5), Float(3.5))
	ok(t, Sub, Float(1), Int(3), Float(-2))
	ok(t, Mul, Int(6), Int(7), Int(42))
	fails(t, Add, Int(1), Bool(true), "E0501")
	fails(t, Mul, Nil{}, Int(1), "E0501")
}

func TestIntegerOverflow(t *testing.T) {
	max, min := Int(math.MaxInt64), Int(math.MinInt64)
	fails(t, Add, max, Int(1), "E0503")
	fails(t, Sub, min, Int(1), "E0503")
	fails(t, Mul, max, Int(2), "E0503")
	fails(t, Mul, min, Int(-1), "E0503")
	fails(t, FloorDiv, min, Int(-1), "E0503")
	fails(t, Pow, Int(2), Int(63), "E0503")
	ok(t, Pow, Int(2), Int(62), Int(1<<62))
	ok(t, Add, max, Int(-1), Int(math.MaxInt64-1))
}

func TestDivision(t *testing.T) {
	ok(t, Div, Int(10), Int(4), Float(2.5))
	ok(t, Div, Int(10), Int(2), Float(5))
	ok(t, FloorDiv, Int(7), Int(2), Int(3))
	ok(t, FloorDiv, Int(-7), Int(2), Int(-4))
	ok(t, FloorDiv, Float(7.5), Int(2), Float(3))
	ok(t, Mod, Int(7), Int(3), Int(1))
	ok(t, Mod, Int(-7), Int(3), Int(2))
	ok(t, Mod, Float(-7.5), Int(2), Float(0.5))
	for _, f := range []binop{Div, FloorDiv, Mod} {
		fails(t, f, Int(1), Int(0), "E0502")
		fails(t, f, Float(1), Float(0), "E0502")
		fails(t, f, Int(1), Float(0), "E0502")
	}
}

func TestPow(t *testing.T) {
	ok(t, Pow, Int(2), Int(10), Int(1024))
	ok(t, Pow, Int(2), Int(0), Int(1))
	ok(t, Pow, Int(-2), Int(3), Int(-8))
	ok(t, Pow, Int(2), Int(-1), Float(0.5))
	ok(t, Pow, Float(2), Int(2), Float(4))
	ok(t, Pow, Int(4), Float(0.5), Float(2))
	fails(t, Pow, Int(2), Bool(true), "E0501")
}

func TestNonFiniteResultsAreErrors(t *testing.T) {
	fails(t, Mul, Float(1e308), Float(10), "E0504")
	fails(t, Pow, Float(10), Int(400), "E0504")
	fails(t, Sub, Float(math.Inf(1)), Float(math.Inf(1)), "E0504") // defensive: Inf never exists, but if it did
}

func TestUnary(t *testing.T) {
	if v, _ := Neg(Int(3)); v != Int(-3) {
		t.Fatalf("Neg(3) = %v", v)
	}
	if v, _ := Neg(Float(2.5)); v != Float(-2.5) {
		t.Fatalf("Neg(2.5) = %v", v)
	}
	if _, err := Neg(Int(math.MinInt64)); err == nil || err.Code != "E0503" {
		t.Fatalf("Neg(min) err = %v", err)
	}
	if _, err := Neg(Bool(true)); err == nil || err.Code != "E0506" {
		t.Fatalf("Neg(true) err = %v", err)
	}
	if v, _ := Not(Bool(true)); v != Bool(false) {
		t.Fatalf("Not(true) = %v", v)
	}
	if _, err := Not(Int(1)); err == nil || err.Code != "E0505" {
		t.Fatalf("Not(1) err = %v", err)
	}
}

func TestErrorMessagesNameTheOperation(t *testing.T) {
	_, err := Add(Int(1), Bool(true))
	if got := err.Error(); got != "error[E0501]: cannot add int and bool" {
		t.Fatalf("got %q", got)
	}
	_, err = Neg(Bool(true))
	if got := err.Error(); got != "error[E0506]: cannot negate bool" {
		t.Fatalf("got %q", got)
	}
}
```

```go
// internal/value/compare_test.go
package value

import "testing"

func TestEqual(t *testing.T) {
	cases := []struct {
		a, b Value
		want bool
	}{
		{Int(1), Int(1), true}, {Int(1), Float(1), true}, {Float(1.5), Int(1), false},
		{Bool(true), Bool(true), true}, {Nil{}, Nil{}, true},
		{Int(1), Bool(true), false}, {Nil{}, Int(0), false}, {Int(0), Bool(false), false},
	}
	for _, c := range cases {
		if got := Equal(c.a, c.b); got != c.want {
			t.Errorf("Equal(%v, %v) = %v, want %v", c.a, c.b, got, c.want)
		}
	}
	n := &Native{Name: "print"}
	if !Equal(n, n) || Equal(n, &Native{Name: "print"}) {
		t.Error("natives compare by identity")
	}
}

func TestCompare(t *testing.T) {
	cases := []struct {
		a, b Value
		want int
	}{
		{Int(1), Int(2), -1}, {Int(2), Int(2), 0}, {Float(2.5), Int(2), 1}, {Int(1), Float(1), 0},
	}
	for _, c := range cases {
		got, err := Compare(c.a, c.b)
		if err != nil || got != c.want {
			t.Errorf("Compare(%v, %v) = %d, %v; want %d", c.a, c.b, got, err, c.want)
		}
	}
	for _, pair := range [][2]Value{{Bool(true), Bool(false)}, {Int(1), Bool(true)}, {Nil{}, Nil{}}} {
		if _, err := Compare(pair[0], pair[1]); err == nil || err.Code != "E0501" {
			t.Errorf("Compare(%v, %v) err = %v, want E0501", pair[0], pair[1], err)
		}
	}
}
```

- [ ] **Step 2: 실패 확인**

Run: `go test ./internal/value/`
Expected: FAIL

- [ ] **Step 3: 구현** (카탈로그에 `"E0506": "cannot %s %s"` 추가 후)

```go
// Package value holds the Go representation of fung values and the
// primitive operations on them. Values are immutable by convention: every
// operation returns a new value. Errors carry no position; the VM fills
// in the position of the failing instruction.
package value

import "io"

type Value interface {
	TypeName() string
}

type Int int64
type Float float64
type Bool bool

// Nil is fung's nil. It is distinct from a Go nil interface, which the VM
// uses to mean "slot not yet assigned".
type Nil struct{}

// Context is what a native function may ask the VM for. Later milestones
// add calling back into fung code.
type Context interface {
	Stdout() io.Writer
}

// Native is a function implemented in Go.
type Native struct {
	Name string
	Fn   func(ctx Context, args []Value) (Value, error)
}

func (Int) TypeName() string     { return "int" }
func (Float) TypeName() string   { return "float" }
func (Bool) TypeName() string    { return "bool" }
func (Nil) TypeName() string     { return "nil" }
func (*Native) TypeName() string { return "fn" }
```

```go
// internal/value/format.go
package value

import (
	"strconv"
	"strings"
)

// Format renders a value in fung literal form (08: 출력을 그대로 코드에
// 붙여도 동작한다). Floats always show a decimal point or exponent so
// they read back as floats; Go's "1e+21" becomes "1e21".
func Format(v Value) string {
	switch x := v.(type) {
	case Int:
		return strconv.FormatInt(int64(x), 10)
	case Float:
		return formatFloat(float64(x))
	case Bool:
		if x {
			return "true"
		}
		return "false"
	case Nil:
		return "nil"
	case *Native:
		return "<fn " + x.Name + ">"
	}
	return "<unknown>"
}

// FormatForPrint is what print() uses. In M1 it equals Format; M2 makes
// strings print without quotes.
func FormatForPrint(v Value) string { return Format(v) }

// formatFloat uses plain notation for exponents in [-4, 21) and exponent
// notation outside, like JavaScript, so 1000000.0 prints as itself and
// 1e21 as 1e21. Go's 'g' format would switch to exponents at 1e6.
func formatFloat(f float64) string {
	e := strconv.FormatFloat(f, 'e', -1, 64) // d.ddde±XX
	exp, _ := strconv.Atoi(e[strings.IndexByte(e, 'e')+1:])
	if exp < -4 || exp >= 21 {
		return strings.Replace(e, "e+", "e", 1)
	}
	s := strconv.FormatFloat(f, 'f', -1, 64)
	if !strings.Contains(s, ".") {
		s += ".0"
	}
	return s
}
```

```go
// internal/value/arith.go
package value

import (
	"math"

	"github.com/tunarider/fung/internal/diag"
)

// numeric coerces int/float pairs: both ints stay ints, otherwise both
// become floats (07: 정수와 실수를 섞으면 실수 결과).
func numeric(verb string, a, b Value) (ai, bi int64, af, bf float64, isInt bool, err *diag.Error) {
	switch x := a.(type) {
	case Int:
		switch y := b.(type) {
		case Int:
			return int64(x), int64(y), 0, 0, true, nil
		case Float:
			return 0, 0, float64(x), float64(y), false, nil
		}
	case Float:
		switch y := b.(type) {
		case Int:
			return 0, 0, float64(x), float64(y), false, nil
		case Float:
			return 0, 0, float64(x), float64(y), false, nil
		}
	}
	return 0, 0, 0, 0, false, diag.Errorf("E0501", diag.Pos{}, verb, a.TypeName(), b.TypeName())
}

// finite rejects Inf and NaN (07: fung 값에는 Inf·NaN이 없다).
func finite(f float64) (Value, *diag.Error) {
	if math.IsInf(f, 0) || math.IsNaN(f) {
		return nil, diag.Errorf("E0504", diag.Pos{})
	}
	return Float(f), nil
}

var overflow = func() *diag.Error { return diag.Errorf("E0503", diag.Pos{}) }
var divZero = func() *diag.Error { return diag.Errorf("E0502", diag.Pos{}) }

func Add(a, b Value) (Value, *diag.Error) {
	ai, bi, af, bf, isInt, err := numeric("add", a, b)
	if err != nil {
		return nil, err
	}
	if !isInt {
		return finite(af + bf)
	}
	c := ai + bi
	if (bi > 0 && c < ai) || (bi < 0 && c > ai) {
		return nil, overflow()
	}
	return Int(c), nil
}

func Sub(a, b Value) (Value, *diag.Error) {
	ai, bi, af, bf, isInt, err := numeric("subtract", a, b)
	if err != nil {
		return nil, err
	}
	if !isInt {
		return finite(af - bf)
	}
	c := ai - bi
	if (bi > 0 && c > ai) || (bi < 0 && c < ai) {
		return nil, overflow()
	}
	return Int(c), nil
}

func mulInt(a, b int64) (int64, bool) {
	if a == 0 || b == 0 {
		return 0, true
	}
	c := a * b
	if c/b != a || (a == -1 && b == math.MinInt64) || (b == -1 && a == math.MinInt64) {
		return 0, false
	}
	return c, true
}

func Mul(a, b Value) (Value, *diag.Error) {
	ai, bi, af, bf, isInt, err := numeric("multiply", a, b)
	if err != nil {
		return nil, err
	}
	if !isInt {
		return finite(af * bf)
	}
	c, ok := mulInt(ai, bi)
	if !ok {
		return nil, overflow()
	}
	return Int(c), nil
}

// Div is `/`: always a float result.
func Div(a, b Value) (Value, *diag.Error) {
	ai, bi, af, bf, isInt, err := numeric("divide", a, b)
	if err != nil {
		return nil, err
	}
	if isInt {
		af, bf = float64(ai), float64(bi)
	}
	if bf == 0 {
		return nil, divZero()
	}
	return finite(af / bf)
}

// FloorDiv is `//`: floor division, ints stay ints.
func FloorDiv(a, b Value) (Value, *diag.Error) {
	ai, bi, af, bf, isInt, err := numeric("divide", a, b)
	if err != nil {
		return nil, err
	}
	if !isInt {
		if bf == 0 {
			return nil, divZero()
		}
		return finite(math.Floor(af / bf))
	}
	if bi == 0 {
		return nil, divZero()
	}
	if ai == math.MinInt64 && bi == -1 {
		return nil, overflow()
	}
	q := ai / bi
	if ai%bi != 0 && (ai < 0) != (bi < 0) {
		q--
	}
	return Int(q), nil
}

// Mod is `%`, paired with FloorDiv: the result has the sign of the divisor.
func Mod(a, b Value) (Value, *diag.Error) {
	ai, bi, af, bf, isInt, err := numeric("take the remainder of", a, b)
	if err != nil {
		return nil, err
	}
	if !isInt {
		if bf == 0 {
			return nil, divZero()
		}
		return finite(af - bf*math.Floor(af/bf))
	}
	if bi == 0 {
		return nil, divZero()
	}
	if ai == math.MinInt64 && bi == -1 {
		return Int(0), nil
	}
	r := ai % bi
	if r != 0 && (r < 0) != (bi < 0) {
		r += bi
	}
	return Int(r), nil
}

// Pow is `**`: int ** non-negative int stays int (checked), otherwise float.
func Pow(a, b Value) (Value, *diag.Error) {
	ai, bi, af, bf, isInt, err := numeric("raise", a, b)
	if err != nil {
		return nil, err
	}
	if !isInt || bi < 0 {
		if isInt {
			af, bf = float64(ai), float64(bi)
		}
		return finite(math.Pow(af, bf))
	}
	result, base, exp := int64(1), ai, bi
	for exp > 0 {
		if exp&1 == 1 {
			r, ok := mulInt(result, base)
			if !ok {
				return nil, overflow()
			}
			result = r
		}
		exp >>= 1
		if exp > 0 {
			sq, ok := mulInt(base, base)
			if !ok {
				return nil, overflow()
			}
			base = sq
		}
	}
	return Int(result), nil
}

func Neg(a Value) (Value, *diag.Error) {
	switch x := a.(type) {
	case Int:
		if x == math.MinInt64 {
			return nil, overflow()
		}
		return -x, nil
	case Float:
		return -x, nil
	}
	return nil, diag.Errorf("E0506", diag.Pos{}, "negate", a.TypeName())
}

// Not requires a bool (07: truthiness 없음).
func Not(a Value) (Value, *diag.Error) {
	if b, ok := a.(Bool); ok {
		return !b, nil
	}
	return nil, diag.Errorf("E0505", diag.Pos{}, a.TypeName())
}
```

```go
// internal/value/compare.go
package value

import "github.com/tunarider/fung/internal/diag"

// Equal implements == from the table in 02: numbers compare by value
// across int/float, same types by value, reference types by identity,
// different types are simply not equal.
func Equal(a, b Value) bool {
	switch x := a.(type) {
	case Int:
		switch y := b.(type) {
		case Int:
			return x == y
		case Float:
			return float64(x) == float64(y)
		}
		return false
	case Float:
		switch y := b.(type) {
		case Int:
			return float64(x) == float64(y)
		case Float:
			return x == y
		}
		return false
	case Bool:
		y, ok := b.(Bool)
		return ok && x == y
	case Nil:
		_, ok := b.(Nil)
		return ok
	case *Native:
		y, ok := b.(*Native)
		return ok && x == y
	}
	return false
}

// Compare implements < > <= >= for numbers (M1). Strings, durations,
// lists and tuples join in later milestones. Anything else is E0501.
func Compare(a, b Value) (int, *diag.Error) {
	ai, bi, af, bf, isInt, err := numeric("compare", a, b)
	if err != nil {
		return 0, err
	}
	if isInt {
		switch {
		case ai < bi:
			return -1, nil
		case ai > bi:
			return 1, nil
		}
		return 0, nil
	}
	switch {
	case af < bf:
		return -1, nil
	case af > bf:
		return 1, nil
	}
	return 0, nil
}
```

- [ ] **Step 4: 통과 확인**

Run: `gofmt -l . && go vet ./... && go test ./internal/value/ ./internal/diag/`
Expected: PASS

- [ ] **Step 5: 커밋**

```bash
git add internal/value internal/diag
git commit -m "feat(value): add int/float/bool/nil values with checked arithmetic and comparison

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 10: bytecode — 명령, 인코딩, Proto, 역어셈블러

**Files:**
- Create: `internal/bytecode/op.go`, `internal/bytecode/proto.go`, `internal/bytecode/disasm.go`
- Test: `internal/bytecode/bytecode_test.go`

**Interfaces:**
- Produces: `type Op uint8` 상수: `OpConst OpPop OpAdd OpSub OpMul OpDiv OpFloorDiv OpMod OpPow OpNeg OpNot OpEq OpNe OpLt OpGt OpLe OpGe OpCheckBool OpJump OpJumpIfFalse OpCall OpReturn`, `func (Op) String() string`, `func (Op) HasArg() bool`
- Produces: `type Instr uint32`, `func Encode(op Op, arg int) Instr`, `func (Instr) Op() Op`, `func (Instr) Arg() int` (24비트)
- Produces: `type Proto struct{Name string; Code []Instr; Consts []value.Value; Pos []diag.Pos}`, `func (p *Proto) Emit(op Op, arg int, pos diag.Pos) int` (명령 번호 반환), `func (p *Proto) Patch(at int, arg int)`, `func (p *Proto) Const(v value.Value) int` (같은 값이면 재사용)
- Produces: `func Disassemble(p *Proto) string`

- [ ] **Step 1: 실패하는 테스트 작성**

```go
// internal/bytecode/bytecode_test.go
package bytecode

import (
	"testing"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

func TestEncodeRoundTrip(t *testing.T) {
	for _, c := range []struct {
		op  Op
		arg int
	}{{OpConst, 0}, {OpJump, 1 << 24 - 1}, {OpCall, 3}, {OpAdd, 0}} {
		in := Encode(c.op, c.arg)
		if in.Op() != c.op || in.Arg() != c.arg {
			t.Errorf("Encode(%v, %d) round-trips to (%v, %d)", c.op, c.arg, in.Op(), in.Arg())
		}
	}
}

func TestProtoEmitPatchAndConstDedup(t *testing.T) {
	p := &Proto{Name: "<main>"}
	pos := diag.Pos{File: "t.fung", Line: 1, Col: 1}
	i0 := p.Const(value.Int(1))
	i1 := p.Const(value.Int(1))
	i2 := p.Const(value.Float(1))
	if i0 != 0 || i1 != 0 || i2 != 1 {
		t.Fatalf("Const indexes = %d %d %d", i0, i1, i2)
	}
	at := p.Emit(OpJumpIfFalse, 0, pos)
	p.Emit(OpConst, i0, pos)
	p.Patch(at, 2)
	if p.Code[0].Arg() != 2 || len(p.Pos) != 2 || p.Pos[1] != pos {
		t.Fatalf("code = %v, pos = %v", p.Code, p.Pos)
	}
}

func TestDisassemble(t *testing.T) {
	p := &Proto{Name: "<main>"}
	pos := diag.Pos{File: "t.fung", Line: 1, Col: 7}
	p.Emit(OpConst, p.Const(value.Int(1)), pos)
	p.Emit(OpConst, p.Const(value.Int(2)), diag.Pos{File: "t.fung", Line: 1, Col: 11})
	p.Emit(OpAdd, 0, diag.Pos{File: "t.fung", Line: 1, Col: 9})
	p.Emit(OpReturn, 0, pos)
	want := "== <main> ==\n" +
		"0000  1:7   CONST        0    ; 1\n" +
		"0001  1:11  CONST        1    ; 2\n" +
		"0002  1:9   ADD\n" +
		"0003  1:7   RETURN\n"
	if got := Disassemble(p); got != want {
		t.Fatalf("got:\n%s\nwant:\n%s", got, want)
	}
}
```

- [ ] **Step 2: 실패 확인**

Run: `go test ./internal/bytecode/`
Expected: FAIL

- [ ] **Step 3: 구현**

```go
// Package bytecode defines the instruction set of the fung VM. Every
// instruction is one 32-bit word: 8 bits of opcode and 24 bits of
// operand (09. 명령 인코딩). Each instruction also has a source position
// in the parallel Proto.Pos slice (08: 모든 명령은 소스 위치를 가진다).
package bytecode

import "fmt"

type Op uint8

const (
	OpConst       Op = iota // push Consts[arg]
	OpPop                   // discard top
	OpAdd                   // a b -- a+b
	OpSub
	OpMul
	OpDiv      // always float
	OpFloorDiv // //
	OpMod
	OpPow
	OpNeg // a -- -a
	OpNot // a -- not a (a must be bool)
	OpEq
	OpNe
	OpLt
	OpGt
	OpLe
	OpGe
	OpCheckBool   // a -- a, error unless a is bool
	OpJump        // ip = arg
	OpJumpIfFalse // c -- ; if c is false ip = arg; c must be bool
	OpCall        // f a1..an -- result (arg = n)
	OpReturn      // v -- ; return v from the current frame
)

var opNames = [...]string{
	OpConst: "CONST", OpPop: "POP", OpAdd: "ADD", OpSub: "SUB", OpMul: "MUL", OpDiv: "DIV",
	OpFloorDiv: "FLOORDIV", OpMod: "MOD", OpPow: "POW", OpNeg: "NEG", OpNot: "NOT",
	OpEq: "EQ", OpNe: "NE", OpLt: "LT", OpGt: "GT", OpLe: "LE", OpGe: "GE",
	OpCheckBool: "CHECKBOOL", OpJump: "JUMP", OpJumpIfFalse: "JUMPIFFALSE",
	OpCall: "CALL", OpReturn: "RETURN",
}

func (o Op) String() string {
	if int(o) < len(opNames) && opNames[o] != "" {
		return opNames[o]
	}
	return fmt.Sprintf("Op(%d)", uint8(o))
}

// HasArg reports whether the operand is meaningful for this opcode.
func (o Op) HasArg() bool {
	switch o {
	case OpConst, OpJump, OpJumpIfFalse, OpCall:
		return true
	}
	return false
}

type Instr uint32

const argMask = 1<<24 - 1

func Encode(op Op, arg int) Instr { return Instr(uint32(op)<<24 | uint32(arg)&argMask) }
func (i Instr) Op() Op            { return Op(i >> 24) }
func (i Instr) Arg() int          { return int(i & argMask) }
```

```go
// internal/bytecode/proto.go
package bytecode

import (
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

// Proto is a compiled function body ("prototype"): code, its constant
// pool, and one source position per instruction. M3 adds the parameter
// table and captured-value list.
type Proto struct {
	Name   string
	Code   []Instr
	Consts []value.Value
	Pos    []diag.Pos
}

// Emit appends an instruction and returns its index (for Patch).
func (p *Proto) Emit(op Op, arg int, pos diag.Pos) int {
	p.Code = append(p.Code, Encode(op, arg))
	p.Pos = append(p.Pos, pos)
	return len(p.Code) - 1
}

// Patch rewrites the operand of an earlier instruction (jump targets).
func (p *Proto) Patch(at int, arg int) {
	p.Code[at] = Encode(p.Code[at].Op(), arg)
}

// Const adds a constant, reusing an existing equal one of the same type.
func (p *Proto) Const(v value.Value) int {
	for i, c := range p.Consts {
		if c.TypeName() == v.TypeName() && value.Equal(c, v) {
			return i
		}
	}
	p.Consts = append(p.Consts, v)
	return len(p.Consts) - 1
}
```

```go
// internal/bytecode/disasm.go
package bytecode

import (
	"fmt"
	"strings"

	"github.com/tunarider/fung/internal/value"
)

// Disassemble renders a Proto for `fung dump bytecode`:
//
//	0000  1:7   CONST        0    ; 1
func Disassemble(p *Proto) string {
	var b strings.Builder
	fmt.Fprintf(&b, "== %s ==\n", p.Name)
	for i, in := range p.Code {
		pos := fmt.Sprintf("%d:%d", p.Pos[i].Line, p.Pos[i].Col)
		if !in.Op().HasArg() {
			fmt.Fprintf(&b, "%04d  %-5s %s\n", i, pos, in.Op())
			continue
		}
		fmt.Fprintf(&b, "%04d  %-5s %-12s %d", i, pos, in.Op(), in.Arg())
		if in.Op() == OpConst {
			fmt.Fprintf(&b, "    ; %s", value.Format(p.Consts[in.Arg()]))
		}
		b.WriteByte('\n')
	}
	return b.String()
}
```

- [ ] **Step 4: 통과 확인**

Run: `gofmt -l . && go vet ./... && go test ./internal/bytecode/`
Expected: PASS

- [ ] **Step 5: 커밋**

```bash
git add internal/bytecode
git commit -m "feat(bytecode): define fixed-width instructions, protos, and a disassembler

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 11: compiler — 코어 트리 → Proto

**Files:**
- Create: `internal/compiler/compiler.go`
- Test: `internal/compiler/compiler_test.go`

**Interfaces:**
- Consumes: 코어 트리 (`do`, `op`, `if`, `call`, `prelude`, `int`, `float`, `bool`), `bytecode.Proto`
- Produces: `func Compile(core *tree.Node, prelude map[string]value.Value) (*bytecode.Proto, *diag.Error)` — `Name`은 `"<main>"`. `(do …)`는 마지막 식의 값을 `RETURN`한다. 빈 `do`는 `nil`을 반환.

- [ ] **Step 1: 실패하는 테스트 작성**

```go
// internal/compiler/compiler_test.go
package compiler

import (
	"strconv"
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/desugar"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/parser"
	"github.com/tunarider/fung/internal/resolve"
	"github.com/tunarider/fung/internal/value"
)

var prelude = map[string]value.Value{"print": &value.Native{Name: "print"}}

func compile(t *testing.T, src string) *bytecode.Proto {
	t.Helper()
	toks, _, err := lexer.Lex("t.fung", src)
	if err != nil {
		t.Fatal(err)
	}
	s, perr := parser.Parse(toks)
	if perr != nil {
		t.Fatal(perr)
	}
	c, derr := desugar.Desugar(s)
	if derr != nil {
		t.Fatal(derr)
	}
	r, errs := resolve.Resolve(c, func(n string) bool { _, ok := prelude[n]; return ok })
	if len(errs) > 0 {
		t.Fatal(errs)
	}
	p, cerr := Compile(r, prelude)
	if cerr != nil {
		t.Fatal(cerr)
	}
	return p
}

// ops strips positions for shape comparisons: "CONST(0) ADD RETURN".
func ops(p *bytecode.Proto) string {
	var parts []string
	for _, in := range p.Code {
		s := in.Op().String()
		if in.Op().HasArg() {
			s += "(" + strconv.Itoa(in.Arg()) + ")"
		}
		parts = append(parts, s)
	}
	return strings.Join(parts, " ")
}

func TestCompileArithmetic(t *testing.T) {
	p := compile(t, "1 + 2 * 3")
	if got, want := ops(p), "CONST(0) CONST(1) CONST(2) MUL ADD RETURN"; got != want {
		t.Fatalf("got %s, want %s", got, want)
	}
	if len(p.Consts) != 3 || p.Consts[2] != value.Int(3) {
		t.Fatalf("consts = %v", p.Consts)
	}
}

func TestCompileStatementsPopAllButLast(t *testing.T) {
	p := compile(t, "1\n2")
	if got, want := ops(p), "CONST(0) POP CONST(1) RETURN"; got != want {
		t.Fatalf("got %s, want %s", got, want)
	}
}

func TestCompileEmptyProgramReturnsNil(t *testing.T) {
	p := compile(t, "")
	if got, want := ops(p), "CONST(0) RETURN"; got != want || p.Consts[0] != (value.Nil{}) {
		t.Fatalf("got %s consts %v", got, p.Consts)
	}
}

func TestCompileCallAndPrelude(t *testing.T) {
	p := compile(t, "print(1, 2)")
	if got, want := ops(p), "CONST(0) CONST(1) CONST(2) CALL(2) RETURN"; got != want {
		t.Fatalf("got %s, want %s", got, want)
	}
	if p.Consts[0] != prelude["print"] {
		t.Fatalf("callee const = %v", p.Consts[0])
	}
}

func TestCompileIfWithJumps(t *testing.T) {
	p := compile(t, "true and false")
	// (if true (op bool false) false)
	want := "CONST(0) JUMPIFFALSE(5) CONST(1) CHECKBOOL JUMP(6) CONST(1) RETURN"
	if got := ops(p); got != want {
		t.Fatalf("got %s, want %s", got, want)
	}
}

func TestCompileUnaryAndComparison(t *testing.T) {
	p := compile(t, "not (1 < 2)")
	if got, want := ops(p), "CONST(0) CONST(1) LT NOT RETURN"; got != want {
		t.Fatalf("got %s, want %s", got, want)
	}
	p = compile(t, "-1")
	if got, want := ops(p), "CONST(0) NEG RETURN"; got != want {
		t.Fatalf("got %s, want %s", got, want)
	}
}

func TestCompileRecordsPositions(t *testing.T) {
	p := compile(t, "1 + 2")
	// CONST 1 at col 1, CONST 2 at col 5, ADD at col 3 (the operator)
	if p.Pos[0].Col != 1 || p.Pos[1].Col != 5 || p.Pos[2].Col != 3 {
		t.Fatalf("pos = %v", p.Pos)
	}
}
```

- [ ] **Step 2: 실패 확인**

Run: `go test ./internal/compiler/`
Expected: FAIL

- [ ] **Step 3: 구현**

```go
// Package compiler turns a resolved core tree into bytecode. It is a
// single recursive walk: each core form knows how to emit itself, and
// every emitted instruction carries the position of the node it came from.
package compiler

import (
	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/tree"
	"github.com/tunarider/fung/internal/value"
)

type compiler struct {
	proto   *bytecode.Proto
	prelude map[string]value.Value
}

func Compile(core *tree.Node, prelude map[string]value.Value) (*bytecode.Proto, *diag.Error) {
	c := &compiler{proto: &bytecode.Proto{Name: "<main>"}, prelude: prelude}
	if err := c.block(core); err != nil {
		return nil, err
	}
	c.proto.Emit(bytecode.OpReturn, 0, core.Pos)
	return c.proto, nil
}

// block compiles (do e...) leaving the last value on the stack.
func (c *compiler) block(n *tree.Node) *diag.Error {
	if len(n.Kids) == 0 {
		c.proto.Emit(bytecode.OpConst, c.proto.Const(value.Nil{}), n.Pos)
		return nil
	}
	for i, k := range n.Kids {
		if err := c.expr(k); err != nil {
			return err
		}
		if i < len(n.Kids)-1 {
			c.proto.Emit(bytecode.OpPop, 0, k.Pos)
		}
	}
	return nil
}

var binaryOps = map[string]bytecode.Op{
	"+": bytecode.OpAdd, "-": bytecode.OpSub, "*": bytecode.OpMul, "/": bytecode.OpDiv,
	"//": bytecode.OpFloorDiv, "%": bytecode.OpMod, "**": bytecode.OpPow,
	"==": bytecode.OpEq, "!=": bytecode.OpNe, "<": bytecode.OpLt, ">": bytecode.OpGt,
	"<=": bytecode.OpLe, ">=": bytecode.OpGe,
}

var unaryOps = map[string]bytecode.Op{
	"-": bytecode.OpNeg, "not": bytecode.OpNot, "bool": bytecode.OpCheckBool,
}

func (c *compiler) expr(n *tree.Node) *diag.Error {
	p := c.proto
	switch n.Head {
	case "do":
		return c.block(n)
	case "int":
		p.Emit(bytecode.OpConst, p.Const(value.Int(n.Lit.(int64))), n.Pos)
	case "float":
		p.Emit(bytecode.OpConst, p.Const(value.Float(n.Lit.(float64))), n.Pos)
	case "bool":
		p.Emit(bytecode.OpConst, p.Const(value.Bool(n.Lit.(bool))), n.Pos)
	case "prelude":
		p.Emit(bytecode.OpConst, p.Const(c.prelude[n.Lit.(string)]), n.Pos)
	case "op":
		for _, k := range n.Kids {
			if err := c.expr(k); err != nil {
				return err
			}
		}
		if len(n.Kids) == 2 {
			p.Emit(binaryOps[n.Lit.(string)], 0, n.Pos)
		} else {
			p.Emit(unaryOps[n.Lit.(string)], 0, n.Pos)
		}
	case "if":
		if err := c.expr(n.Kids[0]); err != nil {
			return err
		}
		toElse := p.Emit(bytecode.OpJumpIfFalse, 0, n.Pos)
		if err := c.expr(n.Kids[1]); err != nil {
			return err
		}
		toEnd := p.Emit(bytecode.OpJump, 0, n.Pos)
		p.Patch(toElse, len(p.Code))
		if err := c.expr(n.Kids[2]); err != nil {
			return err
		}
		p.Patch(toEnd, len(p.Code))
	case "call":
		for _, k := range n.Kids {
			if err := c.expr(k); err != nil {
				return err
			}
		}
		p.Emit(bytecode.OpCall, len(n.Kids)-1, n.Pos)
	default:
		return diag.Errorf("E0801", n.Pos, "compiler: unknown core form "+n.Head)
	}
	return nil
}
```

- [ ] **Step 4: 통과 확인**

Run: `gofmt -l . && go vet ./... && go test ./internal/compiler/`
Expected: PASS

- [ ] **Step 5: 커밋**

```bash
git add internal/compiler
git commit -m "feat(compiler): compile core forms to bytecode with positions

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 12: native `print`와 VM

**Files:**
- Create: `internal/native/prelude.go`, `internal/vm/vm.go`
- Test: `internal/native/prelude_test.go`, `internal/vm/vm_test.go`

**Interfaces:**
- Produces: `func native.Prelude() map[string]value.Value` — `"print"`
- Produces: `type vm.Runtime struct{Stdout io.Writer}`, `func (rt *Runtime) Run(p *bytecode.Proto) (value.Value, *diag.Error)`
- `Task`는 `value.Context`를 구현한다 (`Stdout()`).
- VM 에러: value 연산 에러에 `WithPos(proto.Pos[ip])`. `OpCheckBool`/`OpJumpIfFalse`/`OpNot`의 비-bool은 E0505. 호출 불가는 E0401. Go 패닉은 recover해 E0801.

- [ ] **Step 1: 실패하는 테스트 작성**

```go
// internal/native/prelude_test.go
package native

import (
	"bytes"
	"io"
	"testing"

	"github.com/tunarider/fung/internal/value"
)

type ctx struct{ out bytes.Buffer }

func (c *ctx) Stdout() io.Writer { return &c.out }

func TestPrintJoinsArgsWithSpaces(t *testing.T) {
	c := &ctx{}
	print := Prelude()["print"].(*value.Native)
	v, err := print.Fn(c, []value.Value{value.Int(1), value.Float(2), value.Bool(true)})
	if err != nil || v != (value.Nil{}) {
		t.Fatalf("v = %v, err = %v", v, err)
	}
	if got := c.out.String(); got != "1 2.0 true\n" {
		t.Fatalf("out = %q", got)
	}
	c.out.Reset()
	print.Fn(c, nil)
	if got := c.out.String(); got != "\n" {
		t.Fatalf("print() out = %q", got)
	}
}
```

```go
// internal/vm/vm_test.go
package vm

import (
	"bytes"
	"testing"

	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/compiler"
	"github.com/tunarider/fung/internal/desugar"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/native"
	"github.com/tunarider/fung/internal/parser"
	"github.com/tunarider/fung/internal/resolve"
	"github.com/tunarider/fung/internal/value"
)

// compileWith runs the front end with the given prelude table.
func compileWith(t *testing.T, src string, prelude map[string]value.Value) *bytecode.Proto {
	t.Helper()
	toks, _, err := lexer.Lex("t.fung", src)
	if err != nil {
		t.Fatal(err)
	}
	s, perr := parser.Parse(toks)
	if perr != nil {
		t.Fatal(perr)
	}
	c, derr := desugar.Desugar(s)
	if derr != nil {
		t.Fatal(derr)
	}
	r, errs := resolve.Resolve(c, func(n string) bool { _, ok := prelude[n]; return ok })
	if len(errs) > 0 {
		t.Fatal(errs)
	}
	p, cerr := compiler.Compile(r, prelude)
	if cerr != nil {
		t.Fatal(cerr)
	}
	return p
}

// run executes a source with the real prelude and returns the result
// value, what print wrote, and the error code ("" on success).
func run(t *testing.T, src string) (value.Value, string, string) {
	t.Helper()
	p := compileWith(t, src, native.Prelude())
	var out bytes.Buffer
	v, rerr := (&Runtime{Stdout: &out}).Run(p)
	code := ""
	if rerr != nil {
		code = rerr.Code
	}
	return v, out.String(), code
}

func TestEvaluatesExpressions(t *testing.T) {
	cases := map[string]value.Value{
		"1 + 2 * 3":         value.Int(7),
		"(1 + 2) * 3":       value.Int(9),
		"10 / 4":            value.Float(2.5),
		"7 // 2":            value.Int(3),
		"-7 % 3":            value.Int(2),
		"2 ** 10":           value.Int(1024),
		"-2 ** 2":           value.Int(-4),
		"1 == 1.0":          value.Bool(true),
		"1 != true":         value.Bool(true),
		"2 >= 3":            value.Bool(false),
		"not true":          value.Bool(false),
		"true and false":    value.Bool(false),
		"false or true":     value.Bool(true),
		"1\n2":              value.Int(2),
		"":                  value.Nil{},
	}
	for src, want := range cases {
		v, _, code := run(t, src)
		if code != "" || v != want {
			t.Errorf("%q = %v (%s), want %v", src, v, code, want)
		}
	}
}

func TestShortCircuitSkipsRightSide(t *testing.T) {
	// 1 / 0 would be E0502 if evaluated.
	if _, _, code := run(t, "false and (1 / 0 == 1)"); code != "" {
		t.Errorf("and did not short-circuit: %s", code)
	}
	if _, _, code := run(t, "true or (1 / 0 == 1)"); code != "" {
		t.Errorf("or did not short-circuit: %s", code)
	}
}

func TestPrintWritesToRuntimeStdout(t *testing.T) {
	_, out, code := run(t, "print(1 + 2 * 3)\nprint(1, 2.0, true)\nprint()")
	if code != "" || out != "7\n1 2.0 true\n\n" {
		t.Fatalf("out = %q, code = %s", out, code)
	}
}

func TestRuntimeErrorsCarryCodeAndPosition(t *testing.T) {
	cases := map[string]string{
		"1 / 0":            "E0502",
		"9223372036854775807 + 1": "E0503",
		"1 + true":         "E0501",
		"1e308 * 10":       "E0504",
		"not 1":            "E0505",
		"1 and true":       "E0505",
		"true and 1":       "E0505",
		"true or 1":        "E0505",
		"1(2)":             "E0401",
		"-true":            "E0506",
	}
	for src, want := range cases {
		if _, _, code := run(t, src); code != want {
			t.Errorf("%q: code = %s, want %s", src, code, want)
		}
	}
	p := compileWith(t, "print(1 +\n 2 / 0)", native.Prelude())
	_, err := (&Runtime{Stdout: &bytes.Buffer{}}).Run(p)
	if err == nil || err.Pos.Line != 2 || err.Pos.Col != 4 {
		t.Fatalf("err = %v, want position of `/` (2:4)", err)
	}
}

func TestGoPanicBecomesInternalError(t *testing.T) {
	boom := &value.Native{Name: "boom", Fn: func(value.Context, []value.Value) (value.Value, error) { panic("x") }}
	p := compileWith(t, "boom()", map[string]value.Value{"boom": boom})
	_, err := (&Runtime{Stdout: &bytes.Buffer{}}).Run(p)
	if err == nil || err.Code != "E0801" {
		t.Fatalf("err = %v, want E0801", err)
	}
}
```

- [ ] **Step 2: 실패 확인**

Run: `go test ./internal/native/ ./internal/vm/`
Expected: FAIL

- [ ] **Step 3: 구현**

```go
// Package native holds the prelude: functions implemented in Go that
// every fung program can call without importing anything (06).
package native

import (
	"strings"

	"github.com/tunarider/fung/internal/value"
)

// Prelude returns a fresh name → function table.
func Prelude() map[string]value.Value {
	return map[string]value.Value{
		"print": &value.Native{Name: "print", Fn: printFn},
	}
}

// printFn joins its arguments with spaces and writes one line (06).
func printFn(ctx value.Context, args []value.Value) (value.Value, error) {
	parts := make([]string, len(args))
	for i, a := range args {
		parts[i] = value.FormatForPrint(a)
	}
	_, err := ctx.Stdout().Write([]byte(strings.Join(parts, " ") + "\n"))
	return value.Nil{}, err
}
```

```go
// Package vm executes bytecode. All state lives in a Task (one per fung
// task, M7) so nothing is global; the Runtime holds what tasks share.
package vm

import (
	"fmt"
	"io"

	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

type Runtime struct {
	Stdout io.Writer
}

type frame struct {
	proto *bytecode.Proto
	ip    int
	base  int // index into the task stack where this frame's slots start (M3)
}

type Task struct {
	rt     *Runtime
	stack  []value.Value
	frames []frame
}

func (t *Task) Stdout() io.Writer { return t.rt.Stdout }

// Run executes a proto on a fresh task and returns the value it returned.
func (rt *Runtime) Run(p *bytecode.Proto) (v value.Value, err *diag.Error) {
	t := &Task{rt: rt}
	t.frames = append(t.frames, frame{proto: p})
	defer func() {
		// Go panics (bugs in natives or the VM) must never reach the user
		// as a Go stack trace (00).
		if r := recover(); r != nil {
			f := t.frames[len(t.frames)-1]
			pos := diag.Pos{}
			if f.ip-1 >= 0 && f.ip-1 < len(f.proto.Pos) {
				pos = f.proto.Pos[f.ip-1]
			}
			v, err = nil, diag.Errorf("E0801", pos, fmt.Sprint(r))
		}
	}()
	return t.run()
}

func (t *Task) push(v value.Value) { t.stack = append(t.stack, v) }

func (t *Task) pop() value.Value {
	v := t.stack[len(t.stack)-1]
	t.stack = t.stack[:len(t.stack)-1]
	return v
}

type binop func(a, b value.Value) (value.Value, *diag.Error)

var arith = map[bytecode.Op]binop{
	bytecode.OpAdd: value.Add, bytecode.OpSub: value.Sub, bytecode.OpMul: value.Mul,
	bytecode.OpDiv: value.Div, bytecode.OpFloorDiv: value.FloorDiv, bytecode.OpMod: value.Mod,
	bytecode.OpPow: value.Pow,
}

func (t *Task) run() (value.Value, *diag.Error) {
	for {
		f := &t.frames[len(t.frames)-1]
		in := f.proto.Code[f.ip]
		pos := f.proto.Pos[f.ip]
		f.ip++
		op := in.Op()

		if fn, ok := arith[op]; ok {
			b, a := t.pop(), t.pop()
			r, err := fn(a, b)
			if err != nil {
				return nil, err.WithPos(pos)
			}
			t.push(r)
			continue
		}

		switch op {
		case bytecode.OpConst:
			t.push(f.proto.Consts[in.Arg()])
		case bytecode.OpPop:
			t.pop()
		case bytecode.OpNeg:
			r, err := value.Neg(t.pop())
			if err != nil {
				return nil, err.WithPos(pos)
			}
			t.push(r)
		case bytecode.OpNot:
			r, err := value.Not(t.pop())
			if err != nil {
				return nil, err.WithPos(pos)
			}
			t.push(r)
		case bytecode.OpEq, bytecode.OpNe:
			b, a := t.pop(), t.pop()
			eq := value.Equal(a, b)
			t.push(value.Bool(eq == (op == bytecode.OpEq)))
		case bytecode.OpLt, bytecode.OpGt, bytecode.OpLe, bytecode.OpGe:
			b, a := t.pop(), t.pop()
			c, err := value.Compare(a, b)
			if err != nil {
				return nil, err.WithPos(pos)
			}
			var r bool
			switch op {
			case bytecode.OpLt:
				r = c < 0
			case bytecode.OpGt:
				r = c > 0
			case bytecode.OpLe:
				r = c <= 0
			case bytecode.OpGe:
				r = c >= 0
			}
			t.push(value.Bool(r))
		case bytecode.OpCheckBool:
			if _, ok := t.stack[len(t.stack)-1].(value.Bool); !ok {
				return nil, diag.Errorf("E0505", pos, t.stack[len(t.stack)-1].TypeName())
			}
		case bytecode.OpJump:
			f.ip = in.Arg()
		case bytecode.OpJumpIfFalse:
			v := t.pop()
			c, ok := v.(value.Bool)
			if !ok {
				return nil, diag.Errorf("E0505", pos, v.TypeName())
			}
			if !c {
				f.ip = in.Arg()
			}
		case bytecode.OpCall:
			argc := in.Arg()
			calleeIdx := len(t.stack) - argc - 1
			callee := t.stack[calleeIdx]
			native, ok := callee.(*value.Native)
			if !ok {
				return nil, diag.Errorf("E0401", pos, callee.TypeName())
			}
			args := make([]value.Value, argc)
			copy(args, t.stack[calleeIdx+1:])
			t.stack = t.stack[:calleeIdx]
			r, err := native.Fn(t, args)
			if err != nil {
				if d, ok := err.(*diag.Error); ok {
					return nil, d.WithPos(pos)
				}
				return nil, diag.Errorf("E0801", pos, err)
			}
			t.push(r)
		case bytecode.OpReturn:
			return t.pop(), nil
		default:
			return nil, diag.Errorf("E0801", pos, fmt.Sprintf("unknown opcode %v", op))
		}
	}
}
```

- [ ] **Step 4: 통과 확인**

Run: `gofmt -l . && go vet ./... && go test ./...`
Expected: PASS (골든 테스트는 여전히 skip)

- [ ] **Step 5: 커밋**

```bash
git add internal/native internal/vm
git commit -m "feat(vm): add the stack VM with native calls and the print prelude

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 13: `fung -e`, `fung run`, `fung dump` 연결과 골든 테스트

**Files:**
- Create: `cmd/fung/pipeline.go`, `tests/lang/arith.fung`, `tests/lang/compare_logic.fung`, `tests/lang/literals.fung`, `tests/lang/continuation.fung`, `tests/lang/err_*.fung` (아래 목록)
- Modify: `cmd/fung/main.go` (`run`, `-e`, `dump` 분기가 `notImplemented` 대신 pipeline 호출), `cmd/fung/main_test.go` (해당 기대 변경)

**Interfaces:**
- Produces: `func execute(file, src string, stdout, stderr io.Writer) int` — 컴파일 에러들은 전부 `diag.Format`으로 stderr에, 종료 코드 2; 런타임 에러는 1; 정상 0.
- Produces: `func dump(stage, file, src string, stdout, stderr io.Writer) int` — `ast`: 겉 트리 S-식, `core`: resolve까지 거친 코어 트리 S-식, `bytecode`: 역어셈블.
- `fung -e CODE`의 파일 이름은 `-e`. `fung dump STAGE -e CODE`와 `fung dump STAGE FILE` 둘 다 지원.

- [ ] **Step 1: CLI 테스트 수정과 골든 파일 작성**

`cmd/fung/main_test.go`의 `TestUnimplementedSubcommandsReportThemselves`에서 `{"run", "x.fung"}`, `{"x.fung"}`, `{"-e", "print(1)"}`, `{"dump", "ast", "x.fung"}` 네 줄을 지우고, 아래 테스트를 추가한다.

```go
func TestEvalPrintsResultOfPrint(t *testing.T) {
	stdout, stderr, code := invoke(t, "-e", "print(1 + 2 * 3)")
	if code != exitOK || stdout != "7\n" || stderr != "" {
		t.Fatalf("code=%d stdout=%q stderr=%q", code, stdout, stderr)
	}
}

func TestEvalCompileErrorExitsTwo(t *testing.T) {
	_, stderr, code := invoke(t, "-e", "print(fiels)")
	if code != exitCompileError || !strings.Contains(stderr, "error[E0204]: undefined name `fiels`") || !strings.Contains(stderr, "--> -e:1:7") {
		t.Fatalf("code=%d stderr=%q", code, stderr)
	}
}

func TestEvalReportsAllResolveErrors(t *testing.T) {
	_, stderr, code := invoke(t, "-e", "print(a + b)")
	if code != exitCompileError || strings.Count(stderr, "error[E0204]") != 2 {
		t.Fatalf("code=%d stderr=%q", code, stderr)
	}
}

func TestEvalRuntimeErrorExitsOne(t *testing.T) {
	stdout, stderr, code := invoke(t, "-e", "print(1)\nprint(1 / 0)")
	if code != exitRuntimeError || stdout != "1\n" || !strings.Contains(stderr, "error[E0502]: division by zero") {
		t.Fatalf("code=%d stdout=%q stderr=%q", code, stdout, stderr)
	}
}

func TestRunMissingFileIsUsageError(t *testing.T) {
	_, stderr, code := invoke(t, "run", "/nonexistent/x.fung")
	if code != exitUsage || !strings.Contains(stderr, "x.fung") {
		t.Fatalf("code=%d stderr=%q", code, stderr)
	}
}

func TestDumpStages(t *testing.T) {
	stdout, _, code := invoke(t, "dump", "ast", "-e", "1 + 2 * 3")
	if code != exitOK || stdout != "(program (binary + 1 (binary * 2 3)))\n" {
		t.Fatalf("ast: code=%d stdout=%q", code, stdout)
	}
	stdout, _, code = invoke(t, "dump", "core", "-e", "print(1 and 2)")
	if code != exitOK || stdout != "(do (call (prelude print) (if 1 (op bool 2) false)))\n" {
		t.Fatalf("core: code=%d stdout=%q", code, stdout)
	}
	stdout, _, code = invoke(t, "dump", "bytecode", "-e", "1 + 2")
	if code != exitOK || !strings.HasPrefix(stdout, "== <main> ==\n0000  1:1   CONST        0    ; 1\n") {
		t.Fatalf("bytecode: code=%d stdout=%q", code, stdout)
	}
}

func TestDumpNeedsSource(t *testing.T) {
	_, _, code := invoke(t, "dump", "ast")
	if code != exitUsage {
		t.Fatalf("code=%d", code)
	}
}
```

골든 파일:

```
# tests/lang/arith.fung
print(1 + 2 * 3)
# expect: 7
print((1 + 2) * 3, 1 - 2 - 3)
# expect: 9 -4
print(7 // 2, 7 % 2, -7 // 2, -7 % 2, 7.5 // 2, -7.5 % 2)
# expect: 3 1 -4 1 3.0 0.5
print(10 / 4, 10 / 2, 1 + 2.5)
# expect: 2.5 5.0 3.5
print(2 ** 10, 2 ** -1, 2.0 ** 2, -2 ** 2, 2 ** 3 ** 2)
# expect: 1024 0.5 4.0 -4 512
print(0.1 + 0.2)
# expect: 0.30000000000000004
```

```
# tests/lang/compare_logic.fung
print(1 == 1.0, 1 != true, 2 >= 3, 1 < 2, 2.5 > 2)
# expect: true true false true true
print(not true, true and false, false or true, not (1 < 2))
# expect: false false true false
print(false and (1 / 0 == 1), true or (1 / 0 == 1))
# expect: false true
```

```
# tests/lang/literals.fung
print(1_000 + 0xff + 0b11 + 0o7)
# expect: 1265
print(1e3, 2.5e-3, 1e21, 123456789.0)
# expect: 1000.0 0.0025 1e21 123456789.0
print(9223372036854775807, -9223372036854775807 - 1)
# expect: 9223372036854775807 -9223372036854775808
print()
# expect:
```

```
# tests/lang/continuation.fung
print(1 +

  2)
# expect: 3
print(true
  and false)
# expect: false
print(
  1,
  2,
)
# expect: 1 2
print(1); print(2)
# expect: 1
# expect: 2
```

에러 골든 (파일 하나에 에러 하나):

| 파일 | 내용 | expect-error |
|------|------|------|
| `err_undefined.fung` | `print(fiels)` | E0204 |
| `err_div_zero.fung` | `print(1)` 다음 줄 `print(1 / 0)`, `# expect: 1` 포함 | E0502 |
| `err_overflow.fung` | `print(9223372036854775807 + 1)` | E0503 |
| `err_type.fung` | `print(1 + true)` | E0501 |
| `err_nonfinite.fung` | `print(1e308 * 10)` | E0504 |
| `err_bool.fung` | `print(1 and true)` | E0505 |
| `err_chain.fung` | `print(1 < 2 < 3)` | E0111 |
| `err_number.fung` | `print(30abc)` | E0102 |
| `err_unexpected.fung` | `print(1 2)` | E0110 |

- [ ] **Step 2: 실패 확인**

Run: `go test ./cmd/fung/ ./tests/`
Expected: FAIL (not implemented, 골든은 exit 3)

- [ ] **Step 3: 구현**

```go
// cmd/fung/pipeline.go
package main

import (
	"fmt"
	"io"

	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/compiler"
	"github.com/tunarider/fung/internal/desugar"
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/native"
	"github.com/tunarider/fung/internal/parser"
	"github.com/tunarider/fung/internal/resolve"
	"github.com/tunarider/fung/internal/tree"
	"github.com/tunarider/fung/internal/vm"
)

// build runs the front end up to and including compilation. It returns
// the surface tree, the resolved core tree and the proto so dump can show
// any stage; errs holds every compile error found (syntax: the first only,
// names: all).
type build struct {
	surface *tree.Node
	core    *tree.Node
	proto   *bytecode.Proto
	errs    []*diag.Error
}

func compileSource(file, src string, upTo string) build {
	var b build
	prelude := native.Prelude()
	toks, _, err := lexer.Lex(file, src)
	if err != nil {
		return build{errs: []*diag.Error{err}}
	}
	surface, perr := parser.Parse(toks)
	if perr != nil {
		return build{errs: []*diag.Error{perr}}
	}
	b.surface = surface
	if upTo == "ast" {
		return b
	}
	core, derr := desugar.Desugar(surface)
	if derr != nil {
		b.errs = []*diag.Error{derr}
		return b
	}
	resolved, rerrs := resolve.Resolve(core, func(n string) bool { _, ok := prelude[n]; return ok })
	if len(rerrs) > 0 {
		b.errs = rerrs
		return b
	}
	b.core = resolved
	if upTo == "core" {
		return b
	}
	proto, cerr := compiler.Compile(resolved, prelude)
	if cerr != nil {
		b.errs = []*diag.Error{cerr}
		return b
	}
	b.proto = proto
	return b
}

func reportErrors(errs []*diag.Error, stderr io.Writer) {
	for _, e := range errs {
		io.WriteString(stderr, diag.Format(e))
	}
}

// execute compiles and runs a source text, mapping errors to exit codes.
func execute(file, src string, stdout, stderr io.Writer) int {
	b := compileSource(file, src, "")
	if len(b.errs) > 0 {
		reportErrors(b.errs, stderr)
		return exitCompileError
	}
	rt := &vm.Runtime{Stdout: stdout}
	if _, err := rt.Run(b.proto); err != nil {
		reportErrors([]*diag.Error{err}, stderr)
		if diag.IsCompileError(err.Code) {
			return exitCompileError
		}
		return exitRuntimeError
	}
	return exitOK
}

// dump prints one internal stage of a source text.
func dump(stage, file, src string, stdout, stderr io.Writer) int {
	b := compileSource(file, src, stage)
	if len(b.errs) > 0 {
		reportErrors(b.errs, stderr)
		return exitCompileError
	}
	switch stage {
	case "ast":
		fmt.Fprintln(stdout, b.surface.String())
	case "core":
		fmt.Fprintln(stdout, b.core.String())
	case "bytecode":
		io.WriteString(stdout, bytecode.Disassemble(b.proto))
	}
	return exitOK
}
```

`main.go`의 변경:

```go
	case "run":
		return runFile(args[1:], stdout, stderr)
	case "-e":
		if len(args) < 2 {
			fmt.Fprintf(stderr, "error: -e needs code\n\n%s", usage)
			return exitUsage
		}
		return execute("-e", args[1], stdout, stderr)
	...
	if isScriptPath(args[0]) {
		return runFile(args, stdout, stderr)
	}
```

```go
// runFile reads a script and executes it. args[0] is the path; the rest
// become the script's arguments in M5.
func runFile(args []string, stdout, stderr io.Writer) int {
	if len(args) == 0 {
		fmt.Fprintf(stderr, "error: run needs a script path\n\n%s", usage)
		return exitUsage
	}
	src, err := os.ReadFile(args[0])
	if err != nil {
		fmt.Fprintf(stderr, "error: cannot read %s: %v\n", args[0], err)
		return exitUsage
	}
	return execute(args[0], string(src), stdout, stderr)
}

// runDump: `fung dump STAGE FILE` or `fung dump STAGE -e CODE`.
func runDump(args []string, stdout, stderr io.Writer) int {
	if len(args) == 0 {
		fmt.Fprintf(stderr, "error: dump needs a stage: ast, core or bytecode\n\n%s", usage)
		return exitUsage
	}
	stage := args[0]
	switch stage {
	case "ast", "core", "bytecode":
	default:
		fmt.Fprintf(stderr, "error: unknown dump stage `%s` (want ast, core or bytecode)\n\n%s", stage, usage)
		return exitUsage
	}
	rest := args[1:]
	switch {
	case len(rest) >= 2 && rest[0] == "-e":
		return dump(stage, "-e", rest[1], stdout, stderr)
	case len(rest) == 1:
		src, err := os.ReadFile(rest[0])
		if err != nil {
			fmt.Fprintf(stderr, "error: cannot read %s: %v\n", rest[0], err)
			return exitUsage
		}
		return dump(stage, rest[0], string(src), stdout, stderr)
	}
	fmt.Fprintf(stderr, "error: dump needs a script path or -e code\n\n%s", usage)
	return exitUsage
}
```

`runDump`의 시그니처가 `(args, stderr)`에서 `(args, stdout, stderr)`로 바뀌므로 호출부도 고친다. `TestDumpRejectsUnknownStage`는 그대로 통과해야 한다.

- [ ] **Step 4: 통과 확인**

Run: `gofmt -l . && go vet ./... && go test ./...`
Expected: 전부 PASS. `go test -v ./tests/ 2>&1 | grep -c '^    --- PASS'`가 골든 파일 수(13)와 같다.

실제 바이너리로도 확인:

```bash
go build -o /tmp/fung-m1 ./cmd/fung && /tmp/fung-m1 -e 'print(1 + 2 * 3)'; echo "exit=$?"; /tmp/fung-m1 dump core -e '1 and 2'; rm /tmp/fung-m1
```
Expected: `7`, `exit=0`, `(do (if 1 (op bool 2) false))`.

- [ ] **Step 5: 커밋**

```bash
git add cmd/fung tests/lang
git commit -m "feat(cli): wire -e, run, and dump through the pipeline; add M1 golden tests

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 14: 학습 노트와 상태 갱신

**Files:**
- Create: `docs/notes/M1-calculator.md`
- Modify: `CLAUDE.md` (현재 상태: M1 완료, 다음 M2), `docs/design/09-implementation-plan.md` (M1 행 "확인 방법"에 완료 표시)

- [ ] **Step 1: 노트 작성** — 아래 절을 모두 담는다. 각 절은 "무엇을 만들었나 → 왜 그렇게 → 어디서 볼 수 있나(`fung dump …` 명령)" 순서로 쓴다.

1. **파이프라인 한눈에**: `print(1 + 2 * 3)`이 토큰 → `(program (call print (binary + 1 (binary * 2 3))))` → `(do (call (prelude print) (op + 1 (op * 2 3))))` → `CONST CONST CONST CONST MUL ADD CALL RETURN` → `7`로 변하는 과정을 각 `fung dump` 출력과 함께 보여 준다.
2. **렉서**: 글자를 토큰으로 자를 때의 세 가지 문제 — 가장 긴 매칭(`;;` vs `;`, `..=` vs `..`), 숫자 뒤에 바로 붙는 글자(기간 vs 오류), 문자열 보간 안의 중첩 따옴표. 각각을 어떻게 풀었는지 (`punct3/2/1` 표, `duration`의 내림차순 검사, `skipInterpolation`의 깊이 추적). 주석을 trivia로 따로 보관하는 이유(포맷터). 줄바꿈을 토큰으로 내보내고 파서가 판단하게 한 이유(01의 결정).
3. **Pratt 파서**: "결합력(binding power)" 개념을 `1 + 2 * 3`과 `-2 ** 2`, `2 ** 3 ** 2`로 설명. 왼쪽/오른쪽 결합력을 다르게 주면 결합 방향이 정해지는 원리. 비교 연쇄 금지를 `lastWasCompare` 한 변수로 잡은 방법. 줄 이어짐을 `peekInfix`가 처리하는 방식.
4. **균일 트리와 desugar**: 노드 형태가 하나뿐일 때 얻는 것(dump, 변환의 단순함)과 잃는 것(Go 타입 검사). `and`/`or`가 `if`로 풀리면서 단락 평가가 공짜로 나오는 원리와 `(op bool …)`이 필요한 이유.
5. **resolve**: 실행 중에 이름을 찾지 않는다는 원칙. M1에서는 prelude뿐이지만 M2에서 슬롯 번호가 여기서 정해진다는 예고.
6. **값과 산술**: Go `int64`를 그대로 쓰되 오버플로를 매 연산 검사하는 방법(덧셈의 부호 비교, 곱셈의 역나눗셈), 내림 나눗셈과 나머지의 짝(`-7 // 2 == -4`, `-7 % 2 == 1`), `Inf`/`NaN`을 값에서 추방한 이유, `5.0`과 `1e21` 출력 형식.
7. **바이트코드와 VM**: 스택 머신의 동작을 `1 + 2 * 3`의 스택 변화 표로. 32비트 고정 폭 인코딩. `if`의 점프 두 개와 패치. 명령마다 위치를 두어 에러가 `2:4`를 가리키는 과정. Go 패닉을 recover해 E0801로 바꾸는 울타리.
8. **종료 코드와 테스트**: 골든 파일 13개가 무엇을 덮는지. 컴파일 에러(2)와 런타임 에러(1)가 갈리는 지점.
9. **다음: M2**: 대입과 슬롯, 블록 스코프, 문자열, 에러 출력의 소스 발췌.

- [ ] **Step 2: CLAUDE.md와 09 갱신**

CLAUDE.md "현재 상태"의 M0 줄 아래에 `- M1 계산기 완료: ...` 한 줄, "다음 작업"을 M2로. 09의 M1 행 확인 방법 칸 끝에 ` — 완료` 추가.

- [ ] **Step 3: 커밋**

```bash
git add docs/notes/M1-calculator.md CLAUDE.md docs/design/09-implementation-plan.md
git commit -m "docs: add M1 learning notes and update status

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```
