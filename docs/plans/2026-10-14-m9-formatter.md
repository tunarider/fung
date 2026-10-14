# M9 포맷터 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `fung fmt`가 `.fung` 파일의 공백·들여쓰기·빈 줄·정렬을 단일 스타일로 바로잡는다. 줄바꿈은 사용자가 쓴 대로 둔다.

**Architecture:** `internal/format`가 렉서의 토큰·주석을 받아 줄 단위로 다시 쓴다: (1) 토큰을 줄로 묶고 꼬리 주석을 붙인다, (2) 줄 시작마다 상태 기계(열린 블록·괄호의 "base 들여쓰기" 스택, `if` 체인, `match` 절, 이어지는 줄)가 들여쓰기를 정한다, (3) 토큰 쌍 표가 사이 공백을 정한다, (4) 연속 줄의 꼬리 주석·`=>`를 정렬한다. 파서는 구문 검사에만 쓴다. 출력은 다시 렉싱해 토큰 열이 같을 때만 돌려준다. `cmd/fung/fmt.go`가 경로·`--check`·stdin을 처리한다.

**Tech Stack:** Go 표준 라이브러리만. 기존 `internal/lexer`(`Lex` → 토큰 + `Comment`, 토큰에 `Off/End`), `internal/parser`(`Parse`). 테스트는 표 기반 + 저장소 전체 성질 검사.

**Spec:** `docs/superpowers/specs/2026-10-14-formatter-design.md` (08 "포맷터"에 요약).

## Global Constraints

- Go 1.27, 표준 라이브러리만. Conventional Commits, `gofmt -l .`·`go vet ./...`·`go test ./...` 통과 후 커밋, `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- TDD: 실패하는 테스트를 먼저 보고 구현한다.
- `internal/format`은 `internal/lexer`·`internal/parser`·`internal/diag`만 import한다(`extras`·`vm` 금지).
- 토큰 원문은 `src[tok.Off:tok.End]`로 쓴다(`Token.Text`는 문자열의 따옴표·심볼의 `'`가 빠져 있다).
- 안전장치: `Format`은 출력을 다시 렉싱해 `Newline`을 뺀 (Kind, 원문) 열과 주석 Text 열이 입력과 같지 않으면 E0801(내부 오류)을 낸다.
- 종료 코드: `diag.ExitOK`(0)·`ExitRuntimeError`(1: `--check` 차이, 안전장치)·`ExitCompileError`(2: 구문 에러)·`ExitUsage`(3).
- 디렉터리 순회는 `_`·`.`로 시작하는 디렉터리를 건너뛴다(`.git`·`.claude` 안의 작업 트리 복사본을 밟지 않기 위해; 스펙의 `_` 규칙에 `.`을 더한 결정).

## Review Focus

1. **여러 줄 원시 문자열**: `` `a\n  b` `` 안의 줄은 손대지 않고, 그 뒤 줄의 들여쓰기도 흔들리지 않는다 → Task 1 표.
2. **`then … else`와 `if … else:`의 구분**: `x = c then a\n  else b`는 이어지는 줄(+1), `if c:\n  a\nelse:\n  b\n;;`는 체인(0) → Task 2 표.
3. **`;;)`와 호출 속 여러 줄 람다**: `each(xs, fn(x):\n  body\n;;)` — body +1, `;;)` 0 → Task 2 표.
4. **골든 주석 `# expect:`**: 꼬리 주석이 아니라 한 줄 주석이며 포맷 뒤에도 같은 줄에 있다(골든 러너는 줄 순서만 본다). 저장소 전체 포맷 후 골든이 통과한다 → Task 5.
5. **빈 입력·주석만 있는 파일·shebang**: 빈 파일은 빈 파일, 주석만 있으면 주석 + 줄바꿈, shebang 줄은 그대로 → Task 3 표.

---

### Task 1: `internal/format` 뼈대 — 줄 묶기, 토큰 공백, 안전장치

**Files:**
- Create: `internal/format/format.go`, `internal/format/lines.go`, `internal/format/spacing.go`, `internal/format/format_test.go`

**Interfaces:**
- Produces: `func Format(file, src string) (string, *diag.Error)`. 내부: `type line struct{ toks []lexer.Token; comment string; srcLine int; indent int; arrow int }`, `func splitLines(src string, toks []lexer.Token, comments []lexer.Comment) []line`, `func gap(prev, next lexer.Token, hasPrev bool) int`, `func render(src string, ln *line) string`(토큰 사이 공백만; 들여쓰기·정렬은 Task 2·3).

- [ ] **Step 1: 실패하는 테스트 — 공백 표와 안전장치**

`internal/format/format_test.go`:

```go
package format

import (
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/lexer"
)

// check formats src and insists the result is what we want, that it is
// idempotent, and that the token stream survived.
func check(t *testing.T, src, want string) {
	t.Helper()
	got, err := Format("t.fung", src)
	if err != nil {
		t.Fatalf("%q: %v", src, err)
	}
	if got != want {
		t.Errorf("format %q\n got %q\nwant %q", src, got, want)
	}
	again, err := Format("t.fung", got)
	if err != nil || again != got {
		t.Errorf("not idempotent: %q -> %q -> %q (%v)", src, got, again, err)
	}
	if !sameTokens(src, got) {
		t.Errorf("tokens changed: %q -> %q", src, got)
	}
}

func sameTokens(a, b string) bool {
	ta, ca, _ := lexer.Lex("a", a)
	tb, cb, _ := lexer.Lex("b", b)
	return strings.Join(tokenTexts(a, ta), "\x00") == strings.Join(tokenTexts(b, tb), "\x00") && len(ca) == len(cb)
}

func tokenTexts(src string, toks []lexer.Token) []string {
	var out []string
	for _, t := range toks {
		if t.Kind != lexer.Newline && t.Kind != lexer.EOF {
			out = append(out, src[t.Off:t.End])
		}
	}
	return out
}

func TestSpacing(t *testing.T) {
	cases := map[string]string{
		"a+b*c":                       "a + b * c\n",
		"x=1":                         "x = 1\n",
		"f( a,b )":                    "f(a, b)\n",
		"xs [ 0 ]":                    "xs[0]\n",
		"m [ \"k\" ]":                 "m[\"k\"]\n",
		"f(x)(y)":                     "f(x)(y)\n",
		"{ a : 1 , b:2 }":             "{a: 1, b: 2}\n",
		"{a, b}":                      "{a, b}\n",
		"[ 1,2 ]":                     "[1, 2]\n",
		"( 1, )":                      "(1,)\n",
		"()":                          "()\n",
		"m . a . b":                   "m.a.b\n",
		"0 .. n":                      "0..n\n",
		"0..=10":                      "0..=10\n",
		"[ a, ... rest ]":             "[a, ...rest]\n",
		"x=>x+1":                      "x => x + 1\n",
		"(a,b)=>a":                    "(a, b) => a\n",
		"()=>work()":                  "() => work()\n",
		"fs.read(p) !":                "fs.read(p)!\n",
		"parse(s) ?":                  "parse(s)?\n",
		"x ?? y":                      "x ?? y\n",
		"xs|>map(f)|>sum":             "xs |> map(f) |> sum\n",
		"- x":                         "-x\n",
		"a - b":                       "a - b\n",
		"a - -b":                      "a - -b\n",
		"f(-1, - 2)":                  "f(-1, -2)\n",
		"x = - 5s":                    "x = -5s\n",
		"return - 1":                  "return -1\n",
		"not  x":                      "not x\n",
		"if(a==b): 1 ;;":              "if (a == b): 1 ;;\n",
		"fn add(a,b)=a+b":             "fn add(a, b) = a + b\n",
		"fn f(x:int)->bool=true":      "fn f(x: int) -> bool = true\n",
		"fn ( x ) : x ;;":             "fn(x): x ;;\n",
		"each(xs, fn(x): x ;;)":       "each(xs, fn(x): x ;;)\n",
		"exec(cmd, cwd : d)":          "exec(cmd, cwd: d)\n",
		"match x: 0=>1;_=>2;;":        "match x: 0 => 1; _ => 2 ;;\n",
		"match v: 1|2 => 'a ;;":       "match v: 1 | 2 => 'a ;;\n",
		"Ok( x )":                     "Ok(x)\n",
		"c then a else b":             "c then a else b\n",
		"x in [1, 2]":                 "x in [1, 2]\n",
		"\"a ${ x+1 } b\"":            "\"a ${ x+1 } b\"\n",
		"`raw  text`":                 "`raw  text`\n",
		"1_000 + 0xff + 2 ** 10":      "1_000 + 0xff + 2 ** 10\n",
		"a;b":                         "a; b\n",
		"a ;;":                        "a ;;\n",
		"x   \n":                      "x\n",
		"":                            "",
	}
	for src, want := range cases {
		check(t, src, want)
	}
}

func TestRejectsSyntaxErrors(t *testing.T) {
	if _, err := Format("t.fung", "f(\n"); err == nil || err.Code != "E0110" {
		t.Errorf("a syntax error must come back untouched: %v", err)
	}
	if _, err := Format("t.fung", "\"open"); err == nil || err.Code != "E0104" {
		t.Errorf("a lex error must come back: %v", err)
	}
}
```

- [ ] **Step 2: 실패 확인**

Run: `go test ./internal/format 2>&1 | head -3`
Expected: 패키지 없음 / `undefined: Format`

- [ ] **Step 3: 구현**

`internal/format/lines.go`:

```go
// Package format is the fung formatter (08): it rewrites a file's
// whitespace, indentation, blank lines and alignment from the lexer's
// tokens and comments, keeping the user's line breaks.
package format

import (
	"sort"
	"strings"

	"github.com/tunarider/fung/internal/lexer"
)

// line is one source line: its tokens (never Newline), an optional
// trailing comment, and what layout decides for it. A blank line has
// neither tokens nor comment; a comment line has only the comment.
type line struct {
	toks    []lexer.Token
	comment string // trailing comment text ("" for none), or a comment line's text
	srcLine int
	indent  int // in levels, decided by layout (Task 2)
	arrow   int // rune offset of a top-level `=>` in the rendered code, -1 for none (Task 3)
	opensBlock bool // the last token is a block-opening `:` (blank-line rule, Task 3)
}

func (l *line) blank() bool   { return len(l.toks) == 0 && l.comment == "" }
func (l *line) isComment() bool { return len(l.toks) == 0 && l.comment != "" }

// splitLines groups tokens by the source line they start on and attaches
// comments: a comment on a line that has tokens is its trailing comment,
// otherwise it is a comment line. Lines inside a multi-line token (a raw
// string) are not lines of their own.
func splitLines(src string, toks []lexer.Token, comments []lexer.Comment) []line {
	byLine := map[int]*line{}
	covered := map[int]bool{}
	last := 0
	add := func(n int) *line {
		if l, ok := byLine[n]; ok {
			return l
		}
		l := &line{srcLine: n, arrow: -1}
		byLine[n] = l
		if n > last {
			last = n
		}
		return l
	}
	for _, t := range toks {
		if t.Kind == lexer.Newline || t.Kind == lexer.EOF {
			continue
		}
		l := add(t.Pos.Line)
		l.toks = append(l.toks, t)
		for i := 1; i <= strings.Count(src[t.Off:t.End], "\n"); i++ {
			covered[t.Pos.Line+i] = true
		}
	}
	for _, c := range comments {
		add(c.Pos.Line).comment = c.Text
	}
	// Every source line from 1 to the last one with content is a line;
	// those with nothing are blank lines.
	total := strings.Count(src, "\n") + 1
	if total < last {
		total = last
	}
	var out []line
	for n := 1; n <= total; n++ {
		if covered[n] {
			continue
		}
		if l, ok := byLine[n]; ok {
			out = append(out, *l)
		} else {
			out = append(out, line{srcLine: n, arrow: -1})
		}
	}
	sort.SliceStable(out, func(i, j int) bool { return out[i].srcLine < out[j].srcLine })
	return out
}
```

`internal/format/spacing.go`:

```go
package format

import "github.com/tunarider/fung/internal/lexer"

func isOpener(k lexer.Kind) bool {
	return k == lexer.LParen || k == lexer.LBracket || k == lexer.LBrace
}

func isCloser(k lexer.Kind) bool {
	return k == lexer.RParen || k == lexer.RBracket || k == lexer.RBrace
}

// isValueEnd reports a token a call or index may follow directly: f(x),
// xs[0], Ok(x), "s"[0], f(x)(y).
func isValueEnd(k lexer.Kind) bool {
	switch k {
	case lexer.Name, lexer.Tag, lexer.RParen, lexer.RBracket, lexer.String, lexer.RawString:
		return true
	}
	return false
}

// prefixContext reports a token after which `-` is a sign, not a
// subtraction: operators, openers, separators, arrows, keywords.
func prefixContext(k lexer.Kind) bool {
	if isOpener(k) || k.IsKeyword() {
		return true
	}
	switch k {
	case lexer.Comma, lexer.Colon, lexer.Semi, lexer.DoubleSemi, lexer.FatArrow, lexer.Arrow, lexer.Assign,
		lexer.Eq, lexer.Ne, lexer.Lt, lexer.Gt, lexer.Le, lexer.Ge, lexer.Plus, lexer.Minus, lexer.Star,
		lexer.StarStar, lexer.Slash, lexer.SlashSlash, lexer.Percent, lexer.Pipe, lexer.Bar, lexer.Coalesce,
		lexer.DotDot, lexer.DotDotEq:
		return true
	}
	return false
}

// gap is the number of spaces between two adjacent tokens on a line
// (spec: 토큰 사이 공백). hasPrev is false for the first token.
func gap(prev, next lexer.Token, hasPrev bool) int {
	if !hasPrev {
		return 0
	}
	p, n := prev.Kind, next.Kind
	switch {
	case isOpener(p), isCloser(n):
		return 0
	case n == lexer.Comma, n == lexer.Semi, n == lexer.Colon:
		return 0
	case p == lexer.Dot, n == lexer.Dot, p == lexer.DotDot, n == lexer.DotDot, p == lexer.DotDotEq, n == lexer.DotDotEq:
		return 0
	case p == lexer.Ellipsis:
		return 0
	case n == lexer.Bang, n == lexer.Question:
		return 0
	case n == lexer.LParen && (isValueEnd(p) || p == lexer.KwFn):
		return 0
	case n == lexer.LBracket && isValueEnd(p):
		return 0
	case p == lexer.Minus && prefixContext(prevOfPrev(prev)):
		return 0
	case p == lexer.DoubleSemi && isCloser(n):
		return 0
	}
	return 1
}
```

`prefixContext(prevOfPrev(prev))`는 "`-`가 접두인가"를 그 **앞** 토큰으로 판단해야 하므로, `gap`에 토큰 셋이 필요하다. 간단히 `render`가 접두 여부를 미리 표시한다: `render`는 토큰을 돌며 `prefixMinus[i]`를 계산해 두고, `gap`에 `prev` 대신 "prev가 접두 `-`인가"를 넘긴다. 실제 시그니처는 `gap(prev, next lexer.Token, hasPrev, prevIsPrefixMinus bool) int`로 하고 위의 `p == lexer.Minus && prefixContext(...)` 행을 `prevIsPrefixMinus`로 바꾼다. `prevOfPrev`는 만들지 않는다.

`internal/format/format.go`:

```go
package format

import (
	"strings"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/parser"
)

// Format rewrites src in the single fung style (08). A syntax error comes
// back as is and nothing is formatted; an output whose tokens differ from
// the input is an internal error, never returned as text.
func Format(file, src string) (string, *diag.Error) {
	toks, comments, err := lexer.Lex(file, src)
	if err != nil {
		return "", err
	}
	if _, perr := parser.Parse(toks); perr != nil {
		return "", perr
	}
	lines := splitLines(src, toks, comments)
	layout(src, lines)      // Task 2: indentation
	lines = tidyBlank(lines) // Task 3: blank lines
	out := emit(src, lines)  // Task 3: alignment + assembly (Task 1: spacing only)
	if !same(file, src, toks, comments, out) {
		return "", diag.Errorf("E0801", diag.Pos{}, "formatter changed the code of "+file)
	}
	return out, nil
}

// render joins a line's tokens with the spacing table.
func render(src string, ln *line) string {
	var b strings.Builder
	prefix := false
	for i, t := range ln.toks {
		if i > 0 {
			for n := gap(ln.toks[i-1], t, true, prefix); n > 0; n-- {
				b.WriteByte(' ')
			}
		}
		b.WriteString(src[t.Off:t.End])
		prefix = t.Kind == lexer.Minus && (i == 0 || prefixContext(ln.toks[i-1].Kind))
	}
	return b.String()
}

// same checks the safety property: the output lexes to the same tokens
// and comments as the input, newlines aside.
func same(file, src string, toks []lexer.Token, comments []lexer.Comment, out string) bool {
	toks2, comments2, err := lexer.Lex(file, out)
	if err != nil || len(comments) != len(comments2) {
		return false
	}
	for i := range comments {
		if comments[i].Text != comments2[i].Text {
			return false
		}
	}
	a, b := code(src, toks), code(out, toks2)
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

func code(src string, toks []lexer.Token) []string {
	var out []string
	for _, t := range toks {
		if t.Kind != lexer.Newline && t.Kind != lexer.EOF {
			out = append(out, src[t.Off:t.End])
		}
	}
	return out
}
```

Task 1에서는 `layout`을 "모든 줄 indent 0", `tidyBlank`를 항등, `emit`을 "줄마다 `render` + 꼬리 주석(`"  " + comment`) + `\n`, 빈 파일은 빈 문자열"로 둔다(Task 2·3이 바꾼다):

```go
func layout(src string, lines []line) {}
func tidyBlank(lines []line) []line { return lines }

func emit(src string, lines []line) string {
	var b strings.Builder
	for i := range lines {
		ln := &lines[i]
		b.WriteString(strings.Repeat("  ", ln.indent))
		codeText := render(src, ln)
		b.WriteString(codeText)
		if ln.comment != "" {
			if codeText != "" {
				b.WriteString("  ")
			}
			b.WriteString(ln.comment)
		}
		b.WriteByte('\n')
	}
	return b.String()
}
```

- [ ] **Step 4: 통과 확인**

Run: `go test ./internal/format`
Expected: `ok`. `"x   \n"` → `"x\n"`은 끝 줄바꿈 뒤의 빈 줄(2번째 줄)이 없어야 하므로 `splitLines`의 `total` 계산을 확인한다: 소스가 `\n`으로 끝나면 마지막 "줄"은 비어 있고 내용이 없으므로 만들지 않는다(`total = 줄바꿈 수 + 1`에서 마지막 줄이 빈 줄이고 토큰·주석이 없으면 뺀다). 빈 입력은 줄 0개 → `""`.

- [ ] **Step 5: 커밋**

```bash
git add internal/format
git commit -m "feat(format): token spacing and the safety check

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: 들여쓰기 상태 기계

**Files:**
- Create: `internal/format/layout.go`
- Modify: `internal/format/format.go`(`layout` 호출은 이미 있음), `internal/format/format_test.go`

**Interfaces:**
- Produces: `func layout(src string, lines []line)` — 각 코드 줄의 `indent`·`arrow`·`opensBlock`을 채운다. 주석 줄의 indent는 **다음 코드 줄**의 indent(다음 코드 줄이 닫는 토큰으로 시작하면 +1), 뒤에 코드 줄이 없으면 앞 코드 줄의 indent.

- [ ] **Step 1: 실패하는 테스트**

`format_test.go`에 추가:

```go
func TestIndentation(t *testing.T) {
	cases := map[string]string{
		// blocks
		"fn f(x):\nx + 1\n;;\n":                       "fn f(x):\n  x + 1\n;;\n",
		"fn f(x):\n    if x:\n  1\n  ;;\n;;\n":          "fn f(x):\n  if x:\n    1\n  ;;\n;;\n",
		"if c:\na\nelse:\nb\n;;\n":                     "if c:\n  a\nelse:\n  b\n;;\n",
		"if c:\na\nelse if d:\nb\nelse:\nc\n;;\n":      "if c:\n  a\nelse if d:\n  b\nelse:\n  c\n;;\n",
		"test \"t\":\nassert true\n;;\n":               "test \"t\":\n  assert true\n;;\n",
		"for x in xs:\nprint(x)\n;;\n":                 "for x in xs:\n  print(x)\n;;\n",
		// then/else continuation vs if-chain else
		"x = c then a\nelse b\n":                       "x = c then a\n  else b\n",
		"if c:\n  x = d then 1\n  else 2\nelse:\n  3\n;;\n": "if c:\n  x = d then 1\n    else 2\nelse:\n  3\n;;\n",
		// match arms and bodies
		"match v:\nOk(x) =>\nx\nErr(e) => 0\n;;\n":      "match v:\n  Ok(x) =>\n    x\n  Err(e) => 0\n;;\n",
		"match v:\n  Ok(x) => f(y => y)\n;;\n":          "match v:\n  Ok(x) => f(y => y)\n;;\n",
		// brackets spanning lines; closers go back to the opener's line
		"f(a,\nb,\nc)\n":                               "f(a,\n  b,\n  c)\n",
		"xs = [\n1,\n2,\n]\n":                          "xs = [\n  1,\n  2,\n]\n",
		"m = {\na: 1,\n}\n":                            "m = {\n  a: 1,\n}\n",
		// multi-line lambda inside a call: body +1, `;;)` at the call's line
		"each(xs, fn(x):\nprint(x)\n;;)\n":             "each(xs, fn(x):\n  print(x)\n;;)\n",
		"fn g():\neach(xs, fn(x):\nprint(x)\n;;)\n;;\n": "fn g():\n  each(xs, fn(x):\n    print(x)\n  ;;)\n;;\n",
		// continuation lines
		"xs\n|> map(f)\n|> sum\n":                      "xs\n  |> map(f)\n  |> sum\n",
		"a = b\n.c\n.d\n":                              "a = b\n  .c\n  .d\n",
		"x = a\nand b\nor c\n":                         "x = a\n  and b\n  or c\n",
		"v = f(x) ?? 1\n":                              "v = f(x) ?? 1\n",
		"fn f():\n  xs\n  |> sum\n;;\n":                "fn f():\n  xs\n    |> sum\n;;\n",
		// map colon and type annotation never open a block
		"m = {a: 1}\nx = 2\n":                          "m = {a: 1}\nx = 2\n",
		"fn f(x: int) -> int = x\ny = 1\n":             "fn f(x: int) -> int = x\ny = 1\n",
		"fn f(a) = if a: 1 else: 2 ;;\nmatch x:\n0 => 1\n;;\n": "fn f(a) = if a: 1 else: 2 ;;\nmatch x:\n  0 => 1\n;;\n",
		// raw string spanning lines
		"s = `a\n  b`\nx = 1\n":                        "s = `a\n  b`\nx = 1\n",
		// one-line forms are left alone
		"match x: 0 => 1; _ => 2 ;;\n":                 "match x: 0 => 1; _ => 2 ;;\n",
		"if c: a else: b ;;\n":                         "if c: a else: b ;;\n",
	}
	for src, want := range cases {
		check(t, src, want)
	}
}
```

- [ ] **Step 2: 실패 확인**

Run: `go test ./internal/format -run TestIndentation 2>&1 | head -5`
Expected: 들여쓰기가 전부 0이라 FAIL

- [ ] **Step 3: 구현**

`internal/format/layout.go`:

```go
package format

import "github.com/tunarider/fung/internal/lexer"

// opener is something that indents the lines after it until it is
// closed: a block (fn/if/match/for/test, `:` … `;;`) or a bracket.
type opener struct {
	kind  lexer.Kind // KwIf, KwMatch, KwFn, KwFor, KwTest for blocks; LParen/LBracket/LBrace for brackets
	block bool
	base  int // indent of the line that opened it
}

// pending is a block keyword waiting for its `:` at a bracket depth.
type pending struct {
	kind  lexer.Kind
	depth int // bracket depth where the keyword appeared
}

type layouter struct {
	stack   []opener
	pend    []pending
	thens   []int // unmatched `then` per bracket frame (index = bracket depth)
}

func (l *layouter) brackets() int {
	n := 0
	for _, o := range l.stack {
		if !o.block {
			n++
		}
	}
	return n
}

func (l *layouter) innermost() *opener {
	if len(l.stack) == 0 {
		return nil
	}
	return &l.stack[len(l.stack)-1]
}

func isContinuation(k lexer.Kind) bool {
	switch k {
	case lexer.Pipe, lexer.Dot, lexer.Coalesce, lexer.KwAnd, lexer.KwOr, lexer.KwThen:
		return true
	}
	return false
}

func blockKeyword(k lexer.Kind) bool {
	switch k {
	case lexer.KwFn, lexer.KwIf, lexer.KwMatch, lexer.KwFor, lexer.KwTest, lexer.KwElse:
		return true
	}
	return false
}

// hasTopArrow reports a `=>` outside brackets on the line: the parser's
// rule for "this line starts a match arm".
func hasTopArrow(toks []lexer.Token) (bool, int) {
	depth := 0
	for i, t := range toks {
		switch {
		case isOpener(t.Kind):
			depth++
		case isCloser(t.Kind):
			depth--
		case t.Kind == lexer.FatArrow && depth == 0:
			return true, i
		}
	}
	return false, -1
}

// layout decides every code line's indent, then gives comment lines the
// indent of the code line that follows them.
func layout(src string, lines []line) {
	l := &layouter{thens: []int{0}}
	for i := range lines {
		ln := &lines[i]
		if len(ln.toks) == 0 {
			continue
		}
		ln.indent = l.lineIndent(ln)
		l.consume(ln)
	}
	// Comment lines: like the next code line; before a closer, inside it.
	prev := 0
	for i := len(lines) - 1; i >= 0; i-- {
		if len(lines[i].toks) > 0 {
			prev = lines[i].indent
			if isCloser(lines[i].toks[0].Kind) || lines[i].toks[0].Kind == lexer.DoubleSemi {
				prev++
			}
			continue
		}
		if lines[i].isComment() {
			lines[i].indent = prev
		}
	}
	// A trailing run of comments after the last code line sits at that
	// line's own level, not inside it.
	last := -1
	for i := range lines {
		if len(lines[i].toks) > 0 {
			last = i
		}
	}
	for i := last + 1; i < len(lines); i++ {
		if lines[i].isComment() && last >= 0 {
			lines[i].indent = lines[last].indent
			if isCloser(lines[last].toks[0].Kind) || lines[last].toks[0].Kind == lexer.DoubleSemi {
				// after a closing line the level is the closer's own
			}
		}
	}
}

// lineIndent decides the indent of a code line from the state at its start.
func (l *layouter) lineIndent(ln *line) int {
	first := ln.toks[0]
	in := l.innermost()
	// A closing token drops to the line that opened what it closes.
	if first.Kind == lexer.DoubleSemi || isCloser(first.Kind) {
		if in != nil {
			return in.base
		}
		return 0
	}
	// An if-chain `else` sits with its `if`.
	if first.Kind == lexer.KwElse && in != nil && in.block && in.kind == lexer.KwIf && l.thens[l.brackets()] == 0 {
		return in.base
	}
	indent := 0
	if in != nil {
		indent = in.base + 1
	}
	if isContinuation(first.Kind) || (first.Kind == lexer.KwElse && l.thens[l.brackets()] > 0) {
		return indent + 1
	}
	// Inside a match block a line that is not an arm belongs to the
	// previous arm's body.
	if in != nil && in.block && in.kind == lexer.KwMatch {
		if arm, _ := hasTopArrow(ln.toks); !arm {
			return indent + 1
		}
	}
	return indent
}

// consume updates the state with a line's tokens.
func (l *layouter) consume(ln *line) {
	first := ln.toks[0]
	if !isContinuation(first.Kind) && !(first.Kind == lexer.KwElse && l.thens[l.brackets()] > 0) {
		// A new statement: forget block keywords that never got their `:`
		// (fn f() = e) and unmatched thens at this bracket depth.
		d := l.brackets()
		for len(l.pend) > 0 && l.pend[len(l.pend)-1].depth == d {
			l.pend = l.pend[:len(l.pend)-1]
		}
		l.thens[d] = 0
	}
	if arm, i := hasTopArrow(ln.toks); arm {
		if in := l.innermost(); in != nil && in.block && in.kind == lexer.KwMatch {
			ln.arrow = i
		}
	}
	for i, t := range ln.toks {
		d := l.brackets()
		switch {
		case isOpener(t.Kind):
			l.stack = append(l.stack, opener{kind: t.Kind, base: ln.indent})
			l.thens = append(l.thens, 0)
		case isCloser(t.Kind):
			l.popBracket()
		case t.Kind == lexer.DoubleSemi:
			l.popBlock()
		case t.Kind == lexer.KwThen:
			l.thens[d]++
		case t.Kind == lexer.KwElse:
			if l.thens[d] > 0 {
				l.thens[d]--
			} else {
				// if-chain: the if block ends here and `else …:` reopens one.
				if in := l.innermost(); in != nil && in.block && in.kind == lexer.KwIf {
					l.stack = l.stack[:len(l.stack)-1]
				}
				l.pend = append(l.pend, pending{kind: lexer.KwIf, depth: d})
			}
		case blockKeyword(t.Kind):
			l.pend = append(l.pend, pending{kind: t.Kind, depth: d})
		case t.Kind == lexer.Colon:
			if n := len(l.pend); n > 0 && l.pend[n-1].depth == d {
				p := l.pend[n-1]
				l.pend = l.pend[:n-1]
				// `else if c:` leaves a stale `else` pending at this depth: drop it.
				for len(l.pend) > 0 && l.pend[len(l.pend)-1].depth == d && l.pend[len(l.pend)-1].kind == lexer.KwIf && p.kind == lexer.KwIf {
					l.pend = l.pend[:len(l.pend)-1]
				}
				l.stack = append(l.stack, opener{kind: p.kind, block: true, base: ln.indent})
				if i == len(ln.toks)-1 {
					ln.opensBlock = true
				}
			}
		}
	}
}

func (l *layouter) popBracket() {
	for n := len(l.stack); n > 0; n-- {
		if !l.stack[n-1].block {
			l.stack = l.stack[:n-1]
			if len(l.thens) > 1 {
				l.thens = l.thens[:len(l.thens)-1]
			}
			return
		}
	}
}

func (l *layouter) popBlock() {
	for n := len(l.stack); n > 0; n-- {
		if l.stack[n-1].block {
			l.stack = l.stack[:n-1]
			return
		}
	}
}
```

주의할 점(구현하며 확인):
- `fn(x):` 람다 블록: `fn` 대기 → `(`…`)` → `:`은 `fn`과 같은 괄호 깊이 → 블록. `fn f(x) -> int = e`: `:` 없음, 다음 문장에서 대기 제거.
- `else`가 `if` 블록을 **닫고** `pend`를 올리므로 `else:`의 `:`가 새 `if` 블록을 연다. `else if c:`는 `else`(pend if) + `if`(pend if) + `:`(가장 안쪽 pend 소비) → 남은 stale pend 하나는 위 루프가 지운다.
- `thens` 프레임: 괄호를 열 때 0을 push, 닫을 때 pop. `l.thens[d]`는 항상 존재한다(`d = brackets()`).
- 한 줄 `match x: 0 => 1; _ => 2 ;;`: `match` 블록이 열렸다 같은 줄에서 닫힌다. `arrow`는 **블록 안 줄**에서만 기록한다(한 줄 match는 그 줄에서 블록이 아직 안 열렸을 때 `=>`를 보므로 `innermost`가 match가 아님 → 정렬 대상 아님. 단, `match v:` 다음 줄부터는 맞음).
- 주석 줄의 두 번째 루프: 마지막 코드 줄 뒤의 주석은 "그 줄과 같은 깊이"가 되도록 첫 루프의 결과를 덮어쓴다. 첫 루프는 `prev`가 0에서 시작하므로 파일 끝 주석은 0이 되어 있고, 마지막 코드 줄이 `;;`면 0이 맞다. 위 빈 `if`는 지우고, 단순히 `lines[i].indent = lines[last].indent`로 둔다. 다만 마지막 코드 줄이 블록을 열어 둔 채 끝나는 일은 구문 에러라 없다.

- [ ] **Step 4: 통과 확인**

Run: `go test ./internal/format`
Expected: `ok`. 어긋나는 케이스는 상태 기계를 고친다(기대값은 스펙).

- [ ] **Step 5: 커밋**

```bash
git add internal/format
git commit -m "feat(format): indentation from blocks, brackets, chains, arms and continuations

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: 빈 줄, 주석, 정렬

**Files:**
- Create: `internal/format/align.go`
- Modify: `internal/format/format.go`(`tidyBlank`, `emit`), `internal/format/format_test.go`

**Interfaces:**
- Produces: `func tidyBlank(lines []line) []line`(연속 빈 줄 하나로, 파일·블록 시작과 `;;` 직전 빈 줄 제거), `func align(rendered []rendered)`(꼬리 주석·`=>` 열 맞춤), `emit`이 둘을 쓴다. `type rendered struct{ indent int; code string; comment string; arrow int; group key }`.

- [ ] **Step 1: 실패하는 테스트**

```go
func TestBlankLinesAndComments(t *testing.T) {
	cases := map[string]string{
		"\n\nx = 1\n\n\n\ny = 2\n\n":                     "x = 1\n\ny = 2\n",
		"fn f():\n\n  x\n\n;;\n":                          "fn f():\n  x\n;;\n",
		"#!/usr/bin/env fung\n# about\nx = 1\n":          "#!/usr/bin/env fung\n# about\nx = 1\n",
		"x = 1 # one\n":                                  "x = 1  # one\n",
		"x = 1    # one\nlong_name = 2 # two\n":          "x = 1          # one\nlong_name = 2  # two\n",
		"x = 1 # one\ny = 2\nlong = 3 # three\n":         "x = 1  # one\ny = 2\nlong = 3  # three\n",
		"fn f():\n# inside\nx\n# before end\n;;\n# after\n": "fn f():\n  # inside\n  x\n  # before end\n;;\n# after\n",
		"match v:\nOk(x) => 1\nErr(e) => 2\n;;\n":        "match v:\n  Ok(x)  => 1\n  Err(e) => 2\n;;\n",
		"match v:\nOk(text) =>\nlen(text)\nErr(e)   => 0\n;;\n": "match v:\n  Ok(text) =>\n    len(text)\n  Err(e)   => 0\n;;\n",
		"match v:\n  0 => a\n\n  _ => other\n;;\n":        "match v:\n  0 => a\n\n  _ => other\n;;\n",
		"# only a comment":                               "# only a comment\n",
		"print(1)\n# expect: 1\nprint(2)\n# expect: 2\n": "print(1)\n# expect: 1\nprint(2)\n# expect: 2\n",
		"f(a, # first\n  b) # second\n":                  "f(a,  # first\n  b)  # second\n",
	}
	for src, want := range cases {
		check(t, src, want)
	}
}
```

- [ ] **Step 2: 실패 확인**

Run: `go test ./internal/format -run TestBlankLinesAndComments 2>&1 | head -5`
Expected: FAIL (빈 줄 유지, 주석 들여쓰기·정렬 없음)

- [ ] **Step 3: 구현**

`format.go`의 `tidyBlank`:

```go
// tidyBlank collapses runs of blank lines to one and removes them at the
// start of the file, right after a block-opening `:` and right before a
// `;;` (spec: 빈 줄과 파일 끝).
func tidyBlank(lines []line) []line {
	var out []line
	for i := range lines {
		ln := lines[i]
		if ln.blank() {
			if len(out) == 0 || out[len(out)-1].blank() || out[len(out)-1].opensBlock {
				continue
			}
			// drop a blank right before `;;`
			next := nextCode(lines, i)
			if next != nil && next.toks[0].Kind == lexer.DoubleSemi {
				continue
			}
		}
		out = append(out, ln)
	}
	for len(out) > 0 && out[len(out)-1].blank() {
		out = out[:len(out)-1]
	}
	return out
}

func nextCode(lines []line, from int) *line {
	for j := from + 1; j < len(lines); j++ {
		if len(lines[j].toks) > 0 {
			return &lines[j]
		}
		if !lines[j].blank() {
			return nil
		}
	}
	return nil
}
```

`align.go`:

```go
package format

import "unicode/utf8"

// rendered is a line ready for assembly.
type rendered struct {
	indent  int
	code    string
	comment string
	arrow   int // rune offset of a top-level `=>` to align, -1 for none
	blank   bool
}

// align pads consecutive lines so trailing comments start in one column
// (code width max + 2) and consecutive match arms share the `=>` column
// (pattern width max + 1). A run is broken by a blank line, a comment
// line, a line without the feature, or a different indent (spec: 정렬).
func align(rs []rendered) {
	alignRuns(rs, func(r *rendered) bool { return r.code != "" && r.comment != "" },
		func(r *rendered) int { return utf8.RuneCountInString(r.code) },
		func(r *rendered, width int) {
			r.code += spaces(width - utf8.RuneCountInString(r.code))
		})
	alignRuns(rs, func(r *rendered) bool { return r.arrow >= 0 },
		func(r *rendered) int { return r.arrow },
		func(r *rendered, width int) {
			head, tail := r.code[:byteAt(r.code, r.arrow)], r.code[byteAt(r.code, r.arrow):]
			pad := width - r.arrow
			r.code = head + spaces(pad) + tail
			r.arrow += pad
		})
}

func alignRuns(rs []rendered, has func(*rendered) bool, width func(*rendered) int, pad func(*rendered, int)) {
	for i := 0; i < len(rs); {
		if !has(&rs[i]) {
			i++
			continue
		}
		j, max := i, 0
		for j < len(rs) && has(&rs[j]) && rs[j].indent == rs[i].indent {
			if w := width(&rs[j]); w > max {
				max = w
			}
			j++
		}
		for k := i; k < j; k++ {
			pad(&rs[k], max)
		}
		i = j
	}
}

func spaces(n int) string {
	if n <= 0 {
		return ""
	}
	return string(make([]byte, n, n))[:0] + repeatSpace(n)
}
```

`spaces`는 그냥 `strings.Repeat(" ", n)`으로 쓴다(위의 꼼수 줄은 지운다). `byteAt(s, runeIdx)`는 룬 인덱스를 바이트 오프셋으로 바꾼다(`utf8` 순회). `arrow`는 `render`가 `=>` 토큰 **앞**의 룬 수를 계산해 둔다: `render`를 `(code string, arrowRunes int)`를 돌려주도록 바꾸고, `ln.arrow`(토큰 인덱스)가 `>= 0`이면 그 토큰을 쓰기 직전의 `utf8.RuneCountInString(b.String())`을 기록한다. 꼬리 주석 정렬이 먼저이고 `=>` 정렬이 그 뒤면 주석 열이 틀어지므로 **`=>` 정렬을 먼저, 꼬리 주석 정렬을 나중에** 한다(`align` 안의 순서를 그렇게).

`emit`:

```go
func emit(src string, lines []line) string {
	rs := make([]rendered, len(lines))
	for i := range lines {
		ln := &lines[i]
		code, arrow := render(src, ln)
		rs[i] = rendered{indent: ln.indent, code: code, comment: ln.comment, arrow: arrow, blank: ln.blank()}
	}
	align(rs)
	var b strings.Builder
	for _, r := range rs {
		if r.blank {
			b.WriteByte('\n')
			continue
		}
		b.WriteString(strings.Repeat("  ", r.indent))
		b.WriteString(r.code)
		if r.comment != "" {
			if r.code != "" {
				b.WriteString("  ")
			}
			b.WriteString(r.comment)
		}
		b.WriteByte('\n')
	}
	return b.String()
}
```

꼬리 주석 정렬에서 `code`에 덧붙인 공백 뒤에 `"  "`가 더 붙으면 "최대 너비 + 2"가 된다 — `pad`에서 너비까지만 채우고 `emit`이 두 칸을 더한다(위 코드가 그렇다).

- [ ] **Step 4: 통과 확인**

Run: `go test ./internal/format`
Expected: `ok`

- [ ] **Step 5: 커밋**

```bash
git add internal/format
git commit -m "feat(format): blank-line rules, comment placement, and alignment

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: `fung fmt` 명령

**Files:**
- Create: `cmd/fung/fmt.go`
- Modify: `cmd/fung/main.go:72-73`(`notImplemented` 제거), `cmd/fung/main_test.go`(usage 문구 검사가 있으면 맞춤)

**Interfaces:**
- Consumes: `format.Format`.
- Produces: `func runFmt(args []string, stdin io.Reader, stdout, stderr io.Writer) int`. `fung fmt <path>...`(제자리, 바뀐 파일 이름을 stdout에), `fung fmt --check <path>...`(바뀔 파일 이름, 있으면 1), `fung fmt -`(stdin → stdout). 구문 에러: 에러를 stderr에, 그 파일 건너뜀, 마지막에 2. 안전장치 실패: `error: formatter changed the code of <file>; nothing was written` + 1.

- [ ] **Step 1: 실패하는 테스트**

`cmd/fung/main_test.go`에 추가:

```go
func TestFmtCommand(t *testing.T) {
	dir := t.TempDir()
	messy := filepath.Join(dir, "messy.fung")
	clean := filepath.Join(dir, "clean.fung")
	broken := filepath.Join(dir, "broken.fung")
	os.WriteFile(messy, []byte("fn f(x):\nx+1\n;;\n"), 0o644)
	os.WriteFile(clean, []byte("x = 1\n"), 0o644)
	os.WriteFile(broken, []byte("f(\n"), 0o644)
	os.MkdirAll(filepath.Join(dir, "_fixtures"), 0o755)
	os.WriteFile(filepath.Join(dir, "_fixtures", "skip.fung"), []byte("y=2\n"), 0o644)

	out, errOut, code := invoke(t, "fmt", "--check", dir)
	if code != exitCompileError || out != messy+"\n" || !strings.Contains(errOut, "error[E0110]") {
		t.Fatalf("check: code=%d out=%q err=%q", code, out, errOut)
	}
	out, _, code = invoke(t, "fmt", "--check", clean)
	if code != exitOK || out != "" {
		t.Fatalf("check clean: code=%d out=%q", code, out)
	}
	out, _, code = invoke(t, "fmt", messy, clean)
	if code != exitOK || out != messy+"\n" {
		t.Fatalf("fmt: code=%d out=%q", code, out)
	}
	if b, _ := os.ReadFile(messy); string(b) != "fn f(x):\n  x + 1\n;;\n" {
		t.Errorf("messy not formatted: %q", b)
	}
	if b, _ := os.ReadFile(filepath.Join(dir, "_fixtures", "skip.fung")); string(b) != "y=2\n" {
		t.Error("_ directories must be skipped")
	}
	if b, _ := os.ReadFile(broken); string(b) != "f(\n" {
		t.Error("a broken file must be left alone")
	}
	var outBuf, errBuf bytes.Buffer
	code = run([]string{"fmt", "-"}, strings.NewReader("a=1\n"), &outBuf, &errBuf)
	if code != exitOK || outBuf.String() != "a = 1\n" {
		t.Errorf("stdin: code=%d out=%q err=%q", code, outBuf.String(), errBuf.String())
	}
	_, errOut, code = invoke(t, "fmt")
	if code != exitUsage || !strings.Contains(errOut, "fmt needs") {
		t.Errorf("no args: code=%d err=%q", code, errOut)
	}
}
```

`--check`의 출력 순서: 파일은 경로 정렬 순으로 처리하므로 `broken.fung`(에러, stderr) → `clean.fung`(변화 없음) → `messy.fung`(stdout). 종료 코드는 구문 에러가 있으면 2가 1보다 우선.

- [ ] **Step 2: 실패 확인**

Run: `go test ./cmd/fung -run TestFmtCommand 2>&1 | head -3`
Expected: FAIL (`fmt` is not implemented yet, 종료 코드 3)

- [ ] **Step 3: 구현**

`cmd/fung/fmt.go`:

```go
package main

import (
	"bytes"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/format"
)

// runFmt formats files in place (08): `fung fmt <path>...` rewrites and
// names the changed files, `--check` only names them (exit 1 if any),
// `-` formats standard input to standard output. A file with a syntax
// error is reported and left alone (exit 2).
func runFmt(args []string, stdin io.Reader, stdout, stderr io.Writer) int {
	check := false
	var paths []string
	for _, a := range args {
		switch {
		case a == "--check":
			check = true
		case strings.HasPrefix(a, "-") && a != "-":
			fmt.Fprintf(stderr, "error: unknown option %s\n\n%s", a, usage)
			return exitUsage
		default:
			paths = append(paths, a)
		}
	}
	if len(paths) == 0 {
		fmt.Fprintf(stderr, "error: fmt needs a path, or - for standard input\n\n%s", usage)
		return exitUsage
	}
	if len(paths) == 1 && paths[0] == "-" {
		src, err := io.ReadAll(stdin)
		if err != nil {
			fmt.Fprintf(stderr, "error: %v\n", err)
			return exitRuntimeError
		}
		out, ferr := format.Format("<stdin>", string(src))
		if ferr != nil {
			io.WriteString(stderr, diag.Format(ferr, string(src)))
			return exitCompileError
		}
		io.WriteString(stdout, out)
		return exitOK
	}
	files, err := fungFiles(paths)
	if err != nil {
		fmt.Fprintf(stderr, "error: %v\n", err)
		return exitUsage
	}
	code := exitOK
	worse := func(c int) {
		if c > code {
			code = c
		}
	}
	for _, path := range files {
		src, rerr := os.ReadFile(path)
		if rerr != nil {
			fmt.Fprintf(stderr, "error: cannot read %s: %v\n", path, rerr)
			worse(exitRuntimeError)
			continue
		}
		out, ferr := format.Format(path, string(src))
		if ferr != nil {
			if ferr.Code == "E0801" {
				fmt.Fprintf(stderr, "error: formatter changed the code of %s; nothing was written\n", path)
				worse(exitRuntimeError)
			} else {
				io.WriteString(stderr, diag.Format(ferr, string(src)))
				worse(exitCompileError)
			}
			continue
		}
		if bytes.Equal([]byte(out), src) {
			continue
		}
		fmt.Fprintln(stdout, path)
		if check {
			worse(exitRuntimeError)
			continue
		}
		info, _ := os.Stat(path)
		if werr := os.WriteFile(path, []byte(out), info.Mode().Perm()); werr != nil {
			fmt.Fprintf(stderr, "error: cannot write %s: %v\n", path, werr)
			worse(exitRuntimeError)
		}
	}
	return code
}

// fungFiles expands paths: a file as is, a directory recursively, skipping
// directories whose name starts with `_` or `.`.
func fungFiles(paths []string) ([]string, error) {
	var files []string
	for _, p := range paths {
		info, err := os.Stat(p)
		if err != nil {
			return nil, err
		}
		if !info.IsDir() {
			files = append(files, p)
			continue
		}
		err = filepath.WalkDir(p, func(path string, d fs.DirEntry, err error) error {
			if err != nil {
				return err
			}
			if d.IsDir() && path != p && (strings.HasPrefix(d.Name(), "_") || strings.HasPrefix(d.Name(), ".")) {
				return fs.SkipDir
			}
			if !d.IsDir() && strings.HasSuffix(d.Name(), ".fung") {
				files = append(files, path)
			}
			return nil
		})
		if err != nil {
			return nil, err
		}
	}
	sort.Strings(files)
	return files, nil
}
```

`cmd/fung/main.go`의 `case "fmt": return notImplemented(stderr, "fmt")`를 `return runFmt(args[1:], stdin, stdout, stderr)`로 바꾼다(`run`의 인자 이름은 파일에서 확인). usage의 `fung fmt <script.fung>` 줄을 `fung fmt [--check] <path>...|-` 로 고친다. `notImplemented`가 더 안 쓰이면 지운다.

- [ ] **Step 4: 통과 확인**

Run: `go test ./cmd/fung && go vet ./...`
Expected: `ok`

- [ ] **Step 5: 커밋**

```bash
git add cmd/fung
git commit -m "feat(cmd): add fung fmt with --check and stdin

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: 저장소 전체 포맷, 회귀 테스트, 문서

**Files:**
- Create: `tests/fmt_test.go`, `docs/notes/M9-formatter.md`
- Modify: 저장소의 `.fung` 전부(`extras/`, `examples/`, `tests/lang/`), `CLAUDE.md`, `docs/design/09-implementation-plan.md`(M9 행 추가), `docs/design/README.md`?(없음 — 09만)

- [ ] **Step 1: 실패하는 회귀 테스트**

`tests/fmt_test.go`:

```go
package tests

import (
	"bytes"
	"os/exec"
	"testing"
)

// TestRepositoryIsFormatted keeps every .fung file in the repository in
// the fmt style (08): `fung fmt --check` over the tree reports nothing.
func TestRepositoryIsFormatted(t *testing.T) {
	cmd := exec.Command(fungBinary, "fmt", "--check", "../extras", "../examples", "lang")
	var out, errOut bytes.Buffer
	cmd.Stdout, cmd.Stderr = &out, &errOut
	if err := cmd.Run(); err != nil || out.Len() > 0 {
		t.Fatalf("files need formatting (run `fung fmt extras examples tests/lang`):\n%s%s", out.String(), errOut.String())
	}
}
```

- [ ] **Step 2: 실패 확인 — 그리고 차이 검토**

Run: `go test ./tests -run TestRepositoryIsFormatted 2>&1 | head -20`
Expected: FAIL, 바뀔 파일 목록.

그다음 실제로 돌리기 전에 **차이를 눈으로 본다**:

```bash
go build -o /tmp/fung-fmt ./cmd/fung && /tmp/fung-fmt fmt extras examples tests/lang && git diff --stat && git diff | head -300
```

확인할 것: (a) 골든의 `# expect:` 줄이 제자리인가, (b) 예제의 정렬된 꼬리 주석이 규칙(최대 너비 + 2)대로인가, (c) 뜻밖의 들여쓰기가 있으면 **포맷터 버그**다 — 파일을 손대지 말고 `internal/format`에 그 조각을 테스트로 추가해 고친 뒤 다시 돌린다(`git checkout -- extras examples tests/lang`으로 되돌리고 반복).

- [ ] **Step 3: 전체 검사**

Run: `gofmt -l . && go vet ./... && go test ./...`
Expected: 골든 전부 통과(주석 위치 보존), `TestRepositoryIsFormatted` 통과, `fung test examples/` 10 passed.

- [ ] **Step 4: 커밋 (두 개)**

```bash
git add tests/fmt_test.go
git commit -m "test: keep the repository's fung files formatted

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
git add extras examples tests/lang
git commit -m "style: format all fung files with fung fmt

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

(순서가 중요: 포맷 커밋이 먼저여야 테스트 커밋이 단독으로도 녹색이다 — 실제로는 `style:`을 먼저 커밋하고 `test:`를 뒤에 둔다.)

- [ ] **Step 5: 노트와 상태**

`docs/notes/M9-formatter.md`(한국어): 왜 토큰 재배치인가(AST 재조판과의 비교, 주석 붙이기 문제), 들여쓰기 상태 기계(블록 대기와 `:`, base 스택이 `;;)`를 자연스럽게 처리하는 이유, `else`의 두 뜻, `match` 절 판정이 파서 규칙과 같은 이유), 공백 표의 접두 `-` 판정, 정렬 메커니즘 하나로 두 가지를 처리한 것, 안전장치(토큰 열 비교)가 주는 확신, 저장소를 포맷하며 발견한 버그(있었다면), 보류(표시 폭).
`CLAUDE.md` 현재 상태에 M9 완료 줄과 "다음 작업" 갱신(tree-sitter·LSP·패키지 관리는 별도 세션). `09`의 마일스톤 표에 `M9 포맷터` 행(완료).

```bash
git add docs/notes/M9-formatter.md CLAUDE.md docs/design/09-implementation-plan.md
git commit -m "docs: add M9 formatter notes and update status

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```
