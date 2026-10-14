# M2 변수와 흐름 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 대입과 블록 스코프, `if`/`then`/`else`, 문자열(보간·이스케이프·원시)·심볼·기간·`nil` 값, 스코프 규칙 전부를 검사하는 resolve, 소스 발췌가 붙은 에러 출력, `fail`을 만든다. M1 검토 후속(앞단 파이프라인 추출, 최소 정수 리터럴, 정확한 int/float 비교)도 포함한다.

**Architecture:** 이름은 resolve가 **슬롯 번호**(블록 지역)나 **전역 번호**(파일 최상위)로 바꾸고, 컴파일러와 VM은 이름을 모른다. 블록은 코어 `(do …)`로, 블록 지역 변수는 VM 스택의 프레임 슬롯에, 파일 최상위 변수는 `Runtime`의 전역 표에 산다 (09: M3의 "최상위 fn은 호출 시점에 전역을 찾는다"를 위해 지금부터 분리). 문자열 보간은 렉서가 보관한 코드 조각을 파서가 위치를 이어받아 다시 파싱하고, 코어 `(concat …)` → `CONCAT n` 명령으로 실행한다.

**Tech Stack:** Go 1.27, 표준 라이브러리만.

**Spec:** `docs/design/01-syntax.md` (식/문장, 블록, 줄 규칙, 글자 규칙, EBNF), `02-values-and-types.md` (문자열·심볼·nil·비교 표), `03-functions-and-patterns.md` (블록 스코프), `07-operators-and-types.md` (산술·비교), `08-tooling-and-errors.md` (에러 형식·번호), `09-implementation-plan.md` (resolve, 프레임, 전역).

## Global Constraints

- Go 모듈 `github.com/tunarider/fung`, `go 1.27`, **Go 표준 라이브러리만**.
- 코드·주석·에러 메시지·커밋 메시지는 영어. 문서는 한국어.
- Conventional Commits. 커밋 전 `gofmt -l .`(출력 없음), `go vet ./...`, `go test ./...` 통과.
- 커밋 메시지 끝에 `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Go 패닉·스택 추적은 사용자에게 보이지 않는다. 바이트코드 명령마다 소스 위치. 종료 코드 0/1/2/3.
- **에러 번호는 한 번 붙이면 바꾸지 않는다.** M1 번호(E0101~E0105, E0110·E0111, E0204, E0401, E0501~E0506, E0801·E0802)는 그대로.
- 렉서는 M1에서 닫혔다. 이번에 여는 것은 보간 코드 재파싱을 위한 시작 위치 지정(`LexAt`)과 서로게이트 거부뿐이다.

## Review Focus

1. 깊이 10,000의 중첩 `if` 블록 — 파서 깊이 한도가 블록에도 걸려 E0802가 나야 한다 (Task 6 테스트에 포함).
2. 보간 안의 문법 에러 `"${1 +}"` — 에러 위치가 바깥 파일 기준 줄·칸이어야 한다 (Task 6 테스트에 포함).
3. `x = 1` 뒤에 `if c: x = 2 ;;` — E0205이고 힌트가 붙어야 한다. 반대로 `if c: y = 1 ;;` 뒤 `print(y)` — E0206 (Task 8 테스트에 포함).
4. 전역을 줄 1에서 읽고 줄 2에서 대입 — E0204 (resolve가 위에서 아래로 묶으므로) (Task 8 테스트에 포함).
5. 탭으로 들여쓴 줄의 에러 발췌에서 `^` 표시가 제자리를 가리켜야 한다 (Task 2 테스트에 포함).

## 파일 구조

| 패키지 | 변경 | 작업 |
|------|------|------|
| `internal/frontend` (신설) | 렉서→컴파일러 파이프라인 한 곳. `cmd/fung`과 `vm` 테스트가 쓴다 | Task 1 |
| `internal/diag` | `Error.Len/Help`, 소스 발췌 `Format(e, src)`, 새 번호 | Task 2 |
| `internal/value` | `Str` `Sym` `Dur`, 포맷, 문자열·기간 산술, 정확 비교 | Task 3 |
| `internal/lexer` | `LexAt`, 서로게이트 거부 | Task 4 |
| `internal/tree` | 실수 출력을 `value.Format`으로 | Task 5 |
| `internal/parser` | 대입, `nil`·문자열·심볼·기간 리터럴, 보간 재파싱, `then … else`, `if` 블록, 최소 정수 | Task 6 |
| `internal/desugar` | assign→let, block→do, then→if, interp→concat | Task 7 |
| `internal/resolve` | 스코프, 슬롯·전역, E0205·E0206, 비슷한 이름 힌트, `Result` | Task 8 |
| `internal/bytecode` | LOAD/STORE LOCAL·GLOBAL, CONCAT, `Proto.NumLocals/GlobalNames` | Task 9 |
| `internal/compiler` | 새 코어 형태, `Result` 입력, 조건 위치 | Task 10 |
| `internal/vm`, `internal/native` | 슬롯·전역·CONCAT, E0207, `fail` | Task 11 |
| `cmd/fung`, `tests/lang` | 소스 발췌 출력, `dump ast -e` 메시지, 골든 | Task 12 |
| `docs` | 노트 M2, 03 문서의 가리기 문구 수정, CLAUDE.md | Task 13 |

### M2에서 할당하는 에러 번호

| 번호 | 메시지 | 힌트/라벨 |
|------|------|------|
| E0112 | `expected %s, found %s` | 파서 |
| E0205 | `` cannot reassign `%s` here: it was defined outside this block `` | 힌트: `` a block cannot change an outer variable; build the value with an expression instead, e.g. `x = if … ;;` or `fold` `` |
| E0206 | `` `%s` is not visible here: it was defined inside a block `` | 힌트: `` assign the block's value instead: `x = if … ;;` `` |
| E0207 | `` `%s` is used before it is assigned `` | 런타임(전역) |
| E0803 | `%s` | `fail(msg)` |
| (라벨) | E0204 `not found in this scope` | 발췌의 `^` 옆 |

---

### Task 1: frontend — 앞단 파이프라인 추출

**Files:**
- Create: `internal/frontend/frontend.go`
- Modify: `cmd/fung/pipeline.go` (compileSource 삭제, frontend 사용), `internal/vm/vm_test.go` (compileWith가 frontend 사용)
- Test: `internal/frontend/frontend_test.go`

**Interfaces:**
- Produces: `type Stage int` (`StageAST`, `StageCore`, `StageAll`), `type Build struct{Surface *tree.Node; Core *tree.Node; Proto *bytecode.Proto}`, `func Compile(file, src string, prelude map[string]value.Value, upTo Stage) (Build, []*diag.Error)`
- `compiler_test`, `resolve_test`, `desugar_test`는 자기 패키지를 frontend가 import하므로 쓸 수 없다 (순환). 그대로 둔다.

- [ ] **Step 1: 실패하는 테스트**

```go
// internal/frontend/frontend_test.go
package frontend

import (
	"testing"

	"github.com/tunarider/fung/internal/value"
)

var prelude = map[string]value.Value{"print": &value.Native{Name: "print"}}

func TestCompileStopsAtRequestedStage(t *testing.T) {
	b, errs := Compile("t.fung", "print(1)", prelude, StageAST)
	if len(errs) != 0 || b.Surface == nil || b.Core != nil || b.Proto != nil {
		t.Fatalf("ast: %+v %v", b, errs)
	}
	b, errs = Compile("t.fung", "print(1)", prelude, StageCore)
	if len(errs) != 0 || b.Core == nil || b.Proto != nil {
		t.Fatalf("core: %+v %v", b, errs)
	}
	if got := b.Core.String(); got != "(do (call (prelude print) 1))" {
		t.Fatalf("core = %s", got)
	}
	b, errs = Compile("t.fung", "print(1)", prelude, StageAll)
	if len(errs) != 0 || b.Proto == nil || b.Proto.Name != "<main>" {
		t.Fatalf("all: %+v %v", b, errs)
	}
}

func TestCompileReturnsFirstSyntaxErrorAndAllNameErrors(t *testing.T) {
	_, errs := Compile("t.fung", "1 +", prelude, StageAll)
	if len(errs) != 1 || errs[0].Code != "E0110" {
		t.Fatalf("errs = %v", errs)
	}
	_, errs = Compile("t.fung", "print(a + b)", prelude, StageAll)
	if len(errs) != 2 || errs[0].Code != "E0204" || errs[1].Code != "E0204" {
		t.Fatalf("errs = %v", errs)
	}
}
```

- [ ] **Step 2: 실패 확인** — `go test ./internal/frontend/` → `undefined: Compile`.

- [ ] **Step 3: 구현**

```go
// Package frontend runs the compile pipeline — lex, parse, desugar,
// resolve, compile — in one place, so the command, the REPL (M6), module
// loading (M5) and tests all share it.
package frontend

import (
	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/compiler"
	"github.com/tunarider/fung/internal/desugar"
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/parser"
	"github.com/tunarider/fung/internal/resolve"
	"github.com/tunarider/fung/internal/tree"
	"github.com/tunarider/fung/internal/value"
)

// Stage says how far Compile should go.
type Stage int

const (
	StageAST  Stage = iota // lex and parse only
	StageCore              // through desugar and resolve
	StageAll               // through the compiler
)

// Build holds whatever stages ran. Fields past the requested stage, or
// past the first failing stage, are nil.
type Build struct {
	Surface *tree.Node
	Core    *tree.Node
	Proto   *bytecode.Proto
}

// Compile runs the front end. Syntax errors stop at the first; name
// errors are all collected (08).
func Compile(file, src string, prelude map[string]value.Value, upTo Stage) (Build, []*diag.Error) {
	var b Build
	toks, _, err := lexer.Lex(file, src)
	if err != nil {
		return b, []*diag.Error{err}
	}
	surface, perr := parser.Parse(toks)
	if perr != nil {
		return b, []*diag.Error{perr}
	}
	b.Surface = surface
	if upTo == StageAST {
		return b, nil
	}
	core, derr := desugar.Desugar(surface)
	if derr != nil {
		return b, []*diag.Error{derr}
	}
	resolved, rerrs := resolve.Resolve(core, func(n string) bool { _, ok := prelude[n]; return ok })
	if len(rerrs) > 0 {
		return b, rerrs
	}
	b.Core = resolved
	if upTo == StageCore {
		return b, nil
	}
	proto, cerr := compiler.Compile(resolved, prelude)
	if cerr != nil {
		return b, []*diag.Error{cerr}
	}
	b.Proto = proto
	return b, nil
}
```

`cmd/fung/pipeline.go`: `build` 타입과 `compileSource`를 지우고 `execute`/`dump`가 `frontend.Compile`을 쓴다 (`stage` 문자열 → `frontend.Stage` 변환 함수 `stageOf(s string) frontend.Stage`). `internal/vm/vm_test.go`의 `compileWith`는 본문을 `b, errs := frontend.Compile("t.fung", src, prelude, frontend.StageAll); if len(errs) > 0 { t.Fatal(errs) }; return b.Proto`로 바꾸고 안 쓰는 import를 지운다.

- [ ] **Step 4: 통과 확인** — `gofmt -l . && go vet ./... && go test ./...` 전부 PASS (기존 cmd·vm 테스트가 리팩터링을 검증한다).

- [ ] **Step 5: 커밋**

```bash
git add internal/frontend cmd/fung internal/vm
git commit -m "refactor: extract the front-end pipeline into internal/frontend

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: diag — 소스 발췌, 라벨, 개별 힌트, 새 번호

**Files:**
- Modify: `internal/diag/error.go`, `internal/diag/catalog.go`, `internal/diag/exit.go` (패키지 주석 갱신)
- Test: `internal/diag/error_test.go` (추가)

**Interfaces:**
- Produces: `Error.Len int` (강조할 글자 수, 0이면 1), `Error.Help string` (카탈로그 힌트 대신 쓰는 개별 힌트), `func (e *Error) Span(n int) *Error`, `func (e *Error) WithHelp(h string) *Error`
- Produces: `func Format(e *Error, src string) string` — `src`가 비어 있지 않고 위치가 있으면 발췌를 붙인다. **기존 `Format(e)` 호출은 전부 `Format(e, "")`로 바꾼다.**
- Produces: `func Label(code string) string` (발췌 `^` 옆 짧은 라벨, 없으면 "")

출력 형식 (08):
```
error[E0204]: undefined name `fiels`
  --> rename.fung:12:12
   |
12 |   for f in fiels:
   |            ^^^^^ not found in this scope
   |
 help: a similar name exists: `files`
```
- 거터 너비 = 줄 번호 자릿수. 탭은 발췌에서 공백 4개로 바꾸고 `^` 위치도 같은 규칙으로 센다.
- 줄 끝의 `\r`은 떼어낸다. 줄 번호가 소스 범위 밖이면 발췌를 생략한다.

- [ ] **Step 1: 실패하는 테스트** (`error_test.go`에 추가; 기존 `Format(e)` 호출은 `Format(e, "")`로 고친다)

```go
func TestFormatWithSourceExcerpt(t *testing.T) {
	src := "x = 1\n  for f in fiels:\n"
	e := Errorf("E0204", Pos{File: "rename.fung", Line: 2, Col: 12}, "fiels").Span(5).WithHelp("a similar name exists: `files`")
	want := "error[E0204]: undefined name `fiels`\n" +
		"  --> rename.fung:2:12\n" +
		"  |\n" +
		"2 |   for f in fiels:\n" +
		"  |            ^^^^^ not found in this scope\n" +
		"  |\n" +
		" help: a similar name exists: `files`\n"
	if got := Format(e, src); got != want {
		t.Fatalf("got:\n%s\nwant:\n%s", got, want)
	}
}

func TestFormatExcerptExpandsTabsAndWidensGutter(t *testing.T) {
	src := strings.Repeat("\n", 9) + "\tprint(1 / 0)\n"
	e := Errorf("E0502", Pos{File: "a.fung", Line: 10, Col: 10})
	want := "error[E0502]: division by zero\n" +
		"  --> a.fung:10:10\n" +
		"   |\n" +
		"10 |     print(1 / 0)\n" +
		"   |             ^\n" +
		"   |\n"
	if got := Format(e, src); got != want {
		t.Fatalf("got:\n%s\nwant:\n%s", got, want)
	}
}

func TestFormatWithoutSourceOrOutOfRangeLineHasNoExcerpt(t *testing.T) {
	e := Errorf("E0502", Pos{File: "a.fung", Line: 3, Col: 1})
	if got := Format(e, ""); got != "error[E0502]: division by zero\n  --> a.fung:3:1\n" {
		t.Fatalf("got %q", got)
	}
	if got := Format(e, "one line\n"); got != "error[E0502]: division by zero\n  --> a.fung:3:1\n" {
		t.Fatalf("got %q", got)
	}
}

func TestHelpOverridesCatalogHint(t *testing.T) {
	e := Errorf("E0111", Pos{}).WithHelp("custom")
	if got := Format(e, ""); got != "error[E0111]: comparison operators cannot be chained\n help: custom\n" {
		t.Fatalf("got %q", got)
	}
}

func TestNewCodesExist(t *testing.T) {
	for _, code := range []string{"E0112", "E0205", "E0206", "E0207", "E0803"} {
		if strings.HasPrefix(Message(code, "a", "b"), "unknown error") {
			t.Errorf("%s missing from catalog", code)
		}
	}
	if Hint("E0205") == "" || Hint("E0206") == "" || Label("E0204") == "" {
		t.Error("expected hints for E0205/E0206 and a label for E0204")
	}
}
```

(`strings` import 추가.)

- [ ] **Step 2: 실패 확인** — 컴파일 에러 (`Span`, `WithHelp`, `Label` 미정의; `Format` 인자 수).

- [ ] **Step 3: 구현**

```go
// internal/diag/error.go — 전체 교체
package diag

import (
	"fmt"
	"strconv"
	"strings"
)

// Error is every error fung reports to the user: a catalog number, where
// it happened, how many characters to highlight, the values that fill the
// message template, and an optional per-error help line.
type Error struct {
	Code string
	Pos  Pos
	Len  int // characters to underline; 0 means 1
	Args []any
	Help string // overrides the catalog hint when set
}

func Errorf(code string, pos Pos, args ...any) *Error {
	return &Error{Code: code, Pos: pos, Args: args}
}

func (e *Error) Error() string {
	return fmt.Sprintf("error[%s]: %s", e.Code, Message(e.Code, e.Args...))
}

// WithPos fills in the position if the error does not have one yet.
func (e *Error) WithPos(p Pos) *Error {
	if e.Pos.IsZero() {
		e.Pos = p
	}
	return e
}

// Span sets how many characters the excerpt underlines.
func (e *Error) Span(n int) *Error {
	e.Len = n
	return e
}

// WithHelp sets a help line specific to this error.
func (e *Error) WithHelp(h string) *Error {
	e.Help = h
	return e
}

// Format renders an error the way the fung command prints it (08). When
// src (the file's text) is given and the position is known, the offending
// line is shown with the span underlined.
func Format(e *Error, src string) string {
	var b strings.Builder
	b.WriteString(e.Error())
	b.WriteByte('\n')
	if !e.Pos.IsZero() {
		fmt.Fprintf(&b, "  --> %s\n", e.Pos)
		if src != "" {
			writeExcerpt(&b, e, src)
		}
	}
	help := e.Help
	if help == "" {
		help = Hint(e.Code)
	}
	if help != "" {
		fmt.Fprintf(&b, " help: %s\n", help)
	}
	return b.String()
}

const tabWidth = 4

// writeExcerpt appends the gutter, the source line, and the caret line.
func writeExcerpt(b *strings.Builder, e *Error, src string) {
	lines := strings.Split(src, "\n")
	if e.Pos.Line < 1 || e.Pos.Line > len(lines) {
		return
	}
	line := strings.TrimSuffix(lines[e.Pos.Line-1], "\r")
	num := strconv.Itoa(e.Pos.Line)
	gutter := strings.Repeat(" ", len(num))

	// Expand tabs so the caret column matches what is printed.
	var shown strings.Builder
	col := 0 // printed column of the caret, computed while expanding
	for i, r := range []rune(line) {
		if i == e.Pos.Col-1 {
			col = shown.Len()
		}
		if r == '\t' {
			shown.WriteString(strings.Repeat(" ", tabWidth))
		} else {
			shown.WriteRune(r)
		}
	}
	if e.Pos.Col-1 >= len([]rune(line)) {
		col = len([]rune(shown.String()))
	}
	n := e.Len
	if n < 1 {
		n = 1
	}
	fmt.Fprintf(b, "%s |\n", gutter)
	fmt.Fprintf(b, "%s | %s\n", num, shown.String())
	fmt.Fprintf(b, "%s | %s%s", gutter, strings.Repeat(" ", col), strings.Repeat("^", n))
	if l := Label(e.Code); l != "" {
		b.WriteString(" " + l)
	}
	b.WriteByte('\n')
	fmt.Fprintf(b, "%s |\n", gutter)
}

func IsCompileError(code string) bool {
	return strings.HasPrefix(code, "E01") || strings.HasPrefix(code, "E02") || strings.HasPrefix(code, "E03")
}
```

`col = shown.Len()`은 바이트 길이다. 탭 확장 결과에 멀티바이트 글자가 있으면 어긋난다. **룬 수**로 세야 한다: `col = len([]rune(shown.String()))`로 쓴다 (구현 시 이 버전).

카탈로그 추가:

```go
	"E0112": "expected %s, found %s",
	"E0205": "cannot reassign `%s` here: it was defined outside this block",
	"E0206": "`%s` is not visible here: it was defined inside a block",
	"E0207": "`%s` is used before it is assigned",
	"E0803": "%s",
```
```go
var hints = map[string]string{
	"E0111": "split it with `and`: a < b and b < c",
	"E0205": "a block cannot change an outer variable; build the value with an expression instead, e.g. `x = if … ;;` or `fold`",
	"E0206": "assign the block's value instead: `x = if … ;;`",
}

// labels are short notes printed next to the underline in excerpts.
var labels = map[string]string{
	"E0204": "not found in this scope",
}

// Label returns the excerpt note for a code, or "".
func Label(code string) string { return labels[code] }
```

`exit.go` 주석의 "for now only exit codes live here"를 "Exit codes are defined here; errors and the catalog are in error.go and catalog.go."로.

`cmd/fung/pipeline.go`의 `reportErrors`가 `diag.Format(e, src)`를 받도록 시그니처에 `src string`을 더한다 (Task 12에서 소스를 넘긴다; 지금은 `""`).

- [ ] **Step 4: 통과 확인** — `go test ./...` PASS.

- [ ] **Step 5: 커밋**

```bash
git add internal/diag cmd/fung
git commit -m "feat(diag): add source excerpts, spans, per-error help, and M2 error numbers

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: value — 문자열, 심볼, 기간, 정확 비교

**Files:**
- Create: `internal/value/sym.go`, `internal/value/dur.go`
- Modify: `internal/value/value.go`, `format.go`, `arith.go`, `compare.go`
- Test: `internal/value/str_test.go`, `internal/value/sym_test.go`, `internal/value/dur_test.go`, `internal/value/compare_test.go` (추가)

**Interfaces:**
- Produces: `type Str string`, `type Sym int32`, `type Dur time.Duration`; `TypeName` "str" "sym" "dur"
- Produces: `func Intern(name string) Sym`, `func (s Sym) Name() string` (전역 인턴 표, `sync.RWMutex`)
- Produces: `Format`: Str → fung 리터럴(따옴표, 이스케이프 `\n \t \r \\ \" \$`(`${` 앞의 `$`만), 출력 불가 글자는 `\u{…}`), Sym → `'name`, Dur → `1h30m` `1s500ms` `0.5ms` `0s` `-5s`. `FormatForPrint`: Str은 그대로.
- Produces: `Add`: Str+Str 이어 붙이기, Dur+Dur. `Sub`: Dur-Dur. `Mul`: Dur×num, num×Dur. `Div`: Dur/num → Dur. `Neg`: Dur. 그 외 조합은 E0501.
- Produces: `Equal`: Str·Sym·Dur 값 비교, int/float **정확** 비교. `Compare`: Str(바이트 순 = 코드 포인트 순), Dur, 숫자 정확 비교.

- [ ] **Step 1: 실패하는 테스트**

```go
// internal/value/str_test.go
package value

import "testing"

func TestStrFormatQuotesAndEscapes(t *testing.T) {
	cases := map[Str]string{
		"a b":         `"a b"`,
		"tab\there":   `"tab\there"`,
		"nl\n":        `"nl\n"`,
		`q"q`:         `"q\"q"`,
		`back\slash`:  `"back\\slash"`,
		"cost $5":     `"cost $5"`,
		"tmpl ${x}":   `"tmpl \${x}"`,
		"참치":          `"참치"`,
		"bell\x07":    `"bell\u{7}"`,
	}
	for s, want := range cases {
		if got := Format(s); got != want {
			t.Errorf("Format(%q) = %s, want %s", string(s), got, want)
		}
	}
	if got := FormatForPrint(Str("a \"b\"")); got != `a "b"` {
		t.Errorf("FormatForPrint = %q", got)
	}
	if Str("x").TypeName() != "str" {
		t.Error("TypeName")
	}
}

func TestStrConcatAndErrors(t *testing.T) {
	ok(t, Add, Str("a"), Str("b"), Str("ab"))
	fails(t, Add, Str("a"), Int(1), "E0501")
	fails(t, Sub, Str("a"), Str("b"), "E0501")
	fails(t, Mul, Str("a"), Int(3), "E0501")
}

func TestStrEqualAndCompare(t *testing.T) {
	if !Equal(Str("a"), Str("a")) || Equal(Str("a"), Str("b")) || Equal(Str("1"), Int(1)) {
		t.Error("Equal")
	}
	if c, err := Compare(Str("abc"), Str("abd")); err != nil || c != -1 {
		t.Errorf("Compare abc abd = %d %v", c, err)
	}
	if c, _ := Compare(Str("가"), Str("나")); c != -1 {
		t.Error("Hangul order")
	}
	if _, err := Compare(Str("a"), Int(1)); err == nil || err.Code != "E0501" {
		t.Error("str vs int")
	}
}
```

```go
// internal/value/sym_test.go
package value

import "testing"

func TestSymbolsInternAndFormat(t *testing.T) {
	a, b := Intern("ok"), Intern("ok")
	if a != b || a.Name() != "ok" || Intern("err") == a {
		t.Fatalf("intern: %v %v", a, b)
	}
	if Format(a) != "'ok" || a.TypeName() != "sym" {
		t.Errorf("Format = %q", Format(a))
	}
	if !Equal(a, b) || Equal(a, Str("ok")) || Equal(a, Intern("err")) {
		t.Error("Equal")
	}
	if _, err := Compare(a, b); err == nil || err.Code != "E0501" {
		t.Error("symbols are not ordered")
	}
	fails(t, Add, a, b, "E0501")
}
```

```go
// internal/value/dur_test.go
package value

import (
	"testing"
	"time"
)

func TestDurFormat(t *testing.T) {
	cases := map[Dur]string{
		Dur(90 * time.Minute):                       "1h30m",
		Dur(1500 * time.Millisecond):                "1s500ms",
		Dur(500 * time.Microsecond):                 "0.5ms",
		Dur(0):                                      "0s",
		Dur(-5 * time.Second):                       "-5s",
		Dur(2*time.Hour + 3*time.Second):            "2h3s",
		Dur(time.Millisecond + 500*time.Microsecond): "1.5ms",
	}
	for d, want := range cases {
		if got := Format(d); got != want {
			t.Errorf("Format(%v) = %q, want %q", time.Duration(d), got, want)
		}
	}
}

func TestDurArithmetic(t *testing.T) {
	s, m := Dur(time.Second), Dur(time.Minute)
	ok(t, Add, s, m, Dur(61*time.Second))
	ok(t, Sub, m, s, Dur(59*time.Second))
	ok(t, Mul, s, Int(3), Dur(3*time.Second))
	ok(t, Mul, Float(1.5), s, Dur(1500*time.Millisecond))
	ok(t, Div, m, Int(4), Dur(15*time.Second))
	ok(t, Div, s, Float(0.5), Dur(2*time.Second))
	fails(t, Add, s, Int(1), "E0501")
	fails(t, Div, m, s, "E0501")
	fails(t, Div, s, Int(0), "E0502")
	fails(t, FloorDiv, m, Int(2), "E0501")
	fails(t, Mod, m, Int(2), "E0501")
	if v, _ := Neg(s); v != Dur(-time.Second) {
		t.Error("Neg")
	}
	if c, err := Compare(s, m); err != nil || c != -1 {
		t.Error("Compare")
	}
	if !Equal(s, Dur(time.Second)) || Equal(s, Int(1000000000)) {
		t.Error("Equal")
	}
}

func TestDurOverflowIsAnError(t *testing.T) {
	big := Dur(1 << 62)
	fails(t, Add, big, big, "E0503")
	fails(t, Mul, big, Int(4), "E0503")
	fails(t, Mul, big, Float(4), "E0503")
}
```

`compare_test.go`에 추가:

```go
func TestIntFloatComparisonIsExact(t *testing.T) {
	// 2^53 + 1 is not representable as a float64; float64 conversion would
	// make these equal (02: 정확 비교).
	i := Int(9007199254740993)
	f := Float(9007199254740992)
	if Equal(i, f) || Equal(f, i) {
		t.Error("Equal lost precision")
	}
	if c, _ := Compare(i, f); c != 1 {
		t.Errorf("Compare(i, f) = %d, want 1", c)
	}
	if c, _ := Compare(f, i); c != -1 {
		t.Errorf("Compare(f, i) = %d, want -1", c)
	}
	if c, _ := Compare(Int(-9223372036854775808), Float(-9.3e18)); c != 1 {
		t.Error("below int range")
	}
	if c, _ := Compare(Int(1), Float(1e19)); c != -1 {
		t.Error("above int range")
	}
	if !Equal(Int(3), Float(3)) || Equal(Int(3), Float(3.5)) {
		t.Error("small values")
	}
	if c, _ := Compare(Int(3), Float(3.5)); c != -1 {
		t.Error("fraction")
	}
}
```

- [ ] **Step 2: 실패 확인** — 컴파일 에러.

- [ ] **Step 3: 구현**

`value.go`에 추가:
```go
type Str string

func (Str) TypeName() string { return "str" }
```

```go
// internal/value/sym.go
package value

import "sync"

// Sym is an interned symbol ('ok). Equal symbols share one id, so
// comparison is an integer compare (02). The table is process-wide and
// guarded for the tasks of M7.
type Sym int32

var symbols = struct {
	sync.RWMutex
	ids   map[string]Sym
	names []string
}{ids: map[string]Sym{}}

// Intern returns the symbol for a name, creating it on first use.
func Intern(name string) Sym {
	symbols.RLock()
	id, ok := symbols.ids[name]
	symbols.RUnlock()
	if ok {
		return id
	}
	symbols.Lock()
	defer symbols.Unlock()
	if id, ok := symbols.ids[name]; ok {
		return id
	}
	id = Sym(len(symbols.names))
	symbols.names = append(symbols.names, name)
	symbols.ids[name] = id
	return id
}

func (s Sym) Name() string {
	symbols.RLock()
	defer symbols.RUnlock()
	return symbols.names[s]
}

func (Sym) TypeName() string { return "sym" }
```

```go
// internal/value/dur.go
package value

import (
	"math"
	"strconv"
	"strings"
	"time"

	"github.com/tunarider/fung/internal/diag"
)

// Dur is a duration with nanosecond precision (02).
type Dur time.Duration

func (Dur) TypeName() string { return "dur" }

// formatDur prints the literal form: descending h, m, s, ms components,
// the last one possibly fractional (1h30m, 1s500ms, 0.5ms, 0s).
func formatDur(d Dur) string {
	if d == 0 {
		return "0s"
	}
	var b strings.Builder
	ns := int64(d)
	if ns < 0 {
		b.WriteByte('-')
		ns = -ns
	}
	h, ns := ns/int64(time.Hour), ns%int64(time.Hour)
	m, ns := ns/int64(time.Minute), ns%int64(time.Minute)
	s, ns := ns/int64(time.Second), ns%int64(time.Second)
	if h > 0 {
		b.WriteString(strconv.FormatInt(h, 10) + "h")
	}
	if m > 0 {
		b.WriteString(strconv.FormatInt(m, 10) + "m")
	}
	if s > 0 {
		b.WriteString(strconv.FormatInt(s, 10) + "s")
	}
	if ns > 0 {
		ms := float64(ns) / float64(time.Millisecond)
		b.WriteString(strconv.FormatFloat(ms, 'f', -1, 64) + "ms")
	}
	return b.String()
}

func addDur(a, b Dur) (Value, *diag.Error) {
	c := a + b
	if (b > 0 && c < a) || (b < 0 && c > a) {
		return nil, overflow()
	}
	return c, nil
}

// scaleDur multiplies a duration by a number, rejecting overflow.
func scaleDur(d Dur, n Value) (Value, *diag.Error) {
	var f float64
	switch x := n.(type) {
	case Int:
		c, ok := mulInt(int64(d), int64(x))
		if !ok {
			return nil, overflow()
		}
		return Dur(c), nil
	case Float:
		f = float64(x)
	default:
		return nil, diag.Errorf("E0501", diag.Pos{}, "multiply", d.TypeName(), n.TypeName())
	}
	r := math.Round(float64(d) * f)
	if math.IsNaN(r) || math.Abs(r) >= float64(math.MaxInt64) {
		return nil, overflow()
	}
	return Dur(r), nil
}

func divDur(d Dur, n Value) (Value, *diag.Error) {
	var f float64
	switch x := n.(type) {
	case Int:
		f = float64(x)
	case Float:
		f = float64(x)
	default:
		return nil, diag.Errorf("E0501", diag.Pos{}, "divide", d.TypeName(), n.TypeName())
	}
	if f == 0 {
		return nil, divZero()
	}
	r := math.Round(float64(d) / f)
	if math.IsNaN(r) || math.Abs(r) >= float64(math.MaxInt64) {
		return nil, overflow()
	}
	return Dur(r), nil
}
```

`arith.go`의 각 함수 앞머리에 타입별 분기를 추가한다 (숫자 경로는 그대로):

```go
func Add(a, b Value) (Value, *diag.Error) {
	switch x := a.(type) {
	case Str:
		if y, ok := b.(Str); ok {
			return x + y, nil
		}
		return nil, diag.Errorf("E0501", diag.Pos{}, "add", a.TypeName(), b.TypeName())
	case Dur:
		if y, ok := b.(Dur); ok {
			return addDur(x, y)
		}
		return nil, diag.Errorf("E0501", diag.Pos{}, "add", a.TypeName(), b.TypeName())
	}
	ai, bi, af, bf, isInt, err := numeric("add", a, b)
	... (기존)
```
`Sub`: `Dur - Dur` → `addDur(x, -y)` (y가 MinInt64면 overflow). `Mul`: `a`가 Dur면 `scaleDur(a, b)`, `b`가 Dur면 `scaleDur(b, a)`. `Div`: `a`가 Dur면 `divDur(a, b)`. `Neg`: `Dur` → `-x` (MinInt64 검사). `FloorDiv`·`Mod`·`Pow`는 숫자 경로만 (Dur·Str은 `numeric`이 E0501을 낸다).

`format.go`:
```go
	case Str:
		return quoteStr(string(x))
	case Sym:
		return "'" + x.Name()
	case Dur:
		return formatDur(x)
```
```go
// FormatForPrint is what print() and ${} use: strings bare, the rest literal.
func FormatForPrint(v Value) string {
	if s, ok := v.(Str); ok {
		return string(s)
	}
	return Format(v)
}

// quoteStr writes a string as a fung literal using only the escapes 01 allows.
func quoteStr(s string) string {
	var b strings.Builder
	b.WriteByte('"')
	rs := []rune(s)
	for i, r := range rs {
		switch {
		case r == '\n':
			b.WriteString(`\n`)
		case r == '\t':
			b.WriteString(`\t`)
		case r == '\r':
			b.WriteString(`\r`)
		case r == '\\':
			b.WriteString(`\\`)
		case r == '"':
			b.WriteString(`\"`)
		case r == '$' && i+1 < len(rs) && rs[i+1] == '{':
			b.WriteString(`\$`)
		case !unicode.IsPrint(r) && r != ' ':
			fmt.Fprintf(&b, `\u{%X}`, r)
		default:
			b.WriteRune(r)
		}
	}
	b.WriteByte('"')
	return b.String()
}
```

`compare.go`: `Equal`에 `Str`, `Sym`, `Dur` 케이스(같은 타입이면 `==`), int/float 교차는 `cmpIntFloat(i, f) == 0`. `Compare`: `Str` 둘이면 `strings.Compare`, `Dur` 둘이면 값 비교, 그 외 `numericCompare`:

```go
// cmpIntFloat compares an int and a float exactly, without converting the
// int to float64 (02: 정확 비교). Floats are never NaN in fung.
func cmpIntFloat(i int64, f float64) int {
	if f >= 9223372036854775808.0 { // 2^63
		return -1
	}
	if f < -9223372036854775808.0 {
		return 1
	}
	t := int64(f) // exact: |f| < 2^63 and truncation is exact for floats
	switch {
	case i < t:
		return -1
	case i > t:
		return 1
	}
	frac := f - float64(t)
	switch {
	case frac > 0:
		return -1
	case frac < 0:
		return 1
	}
	return 0
}
```
`Compare`의 숫자 경로: Int/Int → 정수 비교, Float/Float → 실수 비교, Int/Float → `cmpIntFloat(i, f)`, Float/Int → `-cmpIntFloat(i, f)`. 이를 위해 `numeric`을 거치지 않고 타입 스위치를 직접 쓴다.

- [ ] **Step 4: 통과 확인** — `go test ./internal/value/` PASS, 그리고 `go test ./...`.

- [ ] **Step 5: 커밋**

```bash
git add internal/value
git commit -m "feat(value): add str, sym, and dur values with formatting, arithmetic, and exact comparison

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: lexer — `LexAt`와 서로게이트 거부

**Files:**
- Modify: `internal/lexer/lexer.go`, `internal/lexer/string.go`
- Test: `internal/lexer/lexer_test.go`, `internal/lexer/string_test.go` (추가)

**Interfaces:**
- Produces: `func LexAt(file, src string, start diag.Pos) ([]Token, []Comment, *diag.Error)` — 첫 글자의 위치를 `start`로 놓고 렉싱 (보간 코드 재파싱용). `Lex`는 `LexAt(file, src, Pos{File: file, Line: 1, Col: 1})`에 BOM 제거를 더한 것.

- [ ] **Step 1: 실패하는 테스트**

```go
func TestLexAtStartsFromGivenPosition(t *testing.T) {
	toks, _, err := LexAt("t.fung", "x + 1", diag.Pos{File: "t.fung", Line: 3, Col: 10})
	if err != nil {
		t.Fatal(err)
	}
	if toks[0].Pos != (diag.Pos{File: "t.fung", Line: 3, Col: 10}) || toks[2].Pos.Col != 14 {
		t.Fatalf("toks = %v", toks)
	}
}
```
```go
func TestSurrogateEscapeIsRejected(t *testing.T) {
	for _, src := range []string{`"\u{D800}"`, `"\u{DFFF}"`} {
		if _, _, err := Lex("t.fung", src); err == nil || err.Code != "E0103" {
			t.Errorf("%s: err = %v, want E0103", src, err)
		}
	}
	if _, _, err := Lex("t.fung", `"\u{E000}"`); err != nil {
		t.Errorf("E000 is fine: %v", err)
	}
}
```

- [ ] **Step 2: 실패 확인**.

- [ ] **Step 3: 구현** — `Lex`를 둘로 나눈다:

```go
func Lex(file, src string) ([]Token, []Comment, *diag.Error) {
	src = strings.TrimPrefix(src, "﻿")
	return LexAt(file, src, diag.Pos{File: file, Line: 1, Col: 1})
}

// LexAt lexes text that sits inside a larger file, such as the code of a
// ${...} interpolation, so that positions refer to the enclosing file.
func LexAt(file, src string, start diag.Pos) ([]Token, []Comment, *diag.Error) {
	lx := &lexer{file: file, src: src, line: start.Line, col: start.Col, keywords: Keywords()}
	...
}
```
`escape()`의 `\u` 분기에서 `n > 0x10FFFF` 검사에 `|| (n >= 0xD800 && n <= 0xDFFF)`를 더한다.

- [ ] **Step 4: 통과 확인**, **Step 5: 커밋** `feat(lexer): add LexAt for nested code and reject surrogate escapes`.

---

### Task 5: tree — 리터럴 출력을 value와 통일

**Files:**
- Modify: `internal/tree/node.go`
- Test: `internal/tree/node_test.go` (수정·추가)

**Interfaces:**
- `litString`이 `float64` → `value.Format(value.Float(v))`, `time.Duration` → `value.Format(value.Dur(v))`, `string` 잎 중 머리 `string`은 `value.Format(value.Str(v))`, 머리 `symbol`은 `'name`. 새 잎 머리: `nil`(Lit 없음, `(nil)`로 출력), `string`, `symbol`, `duration`. `bareLeaves`에 `string` `symbol` `duration` `nil`을 더해 값 그대로 출력한다 (`(string "a b")` 대신 `"a b"`). **Task 5의 테스트에서 M1의 `(string "a b")` 기대값을 `"a b"`로 바꾼다.**

- [ ] **Step 1: 테스트** — `TestStringFormatsLeaves`의 `Leaf("string", p, "a b")` 기대를 `"a b"`로 바꾸고 추가:

```go
func TestLeavesUseValueFormatting(t *testing.T) {
	p := diag.Pos{}
	cases := map[*Node]string{
		Leaf("float", p, 1e21):                    "1e21",
		Leaf("float", p, 1e6):                     "1000000.0",
		Leaf("symbol", p, "ok"):                   "'ok",
		Leaf("duration", p, 90*time.Minute):       "1h30m",
		Leaf("string", p, "tab\t${"):              `"tab\t\${"`,
		New("nil", p):                             "(nil)",
		New("assign", p, Leaf("name", p, "x"), Leaf("int", p, int64(1))): "(assign x 1)",
	}
	for n, want := range cases {
		if got := n.String(); got != want {
			t.Errorf("String() = %q, want %q", got, want)
		}
	}
}
```

- [ ] **Step 2: 실패 확인**, **Step 3: 구현** (`tree`가 `value`를 import; `value`는 `tree`를 모르므로 순환 없음):

```go
var bareLeaves = map[string]bool{"int": true, "float": true, "bool": true, "name": true, "string": true, "symbol": true, "duration": true}

func (n *Node) write(b *strings.Builder) {
	if len(n.Kids) == 0 && n.Lit != nil {
		lit := n.litString()
		if bareLeaves[n.Head] {
			b.WriteString(lit)
			return
		}
		b.WriteString("(" + n.Head + " " + lit + ")")
		return
	}
	...
}

func (n *Node) litString() string {
	switch v := n.Lit.(type) {
	case float64:
		return value.Format(value.Float(v))
	case time.Duration:
		return value.Format(value.Dur(v))
	case string:
		switch n.Head {
		case "string":
			return value.Format(value.Str(v))
		case "symbol":
			return "'" + v
		}
		return v
	}
	return fmt.Sprint(n.Lit)
}
```

- [ ] **Step 4: 통과 확인**, **Step 5: 커밋** `refactor(tree): print literals with value.Format`.

---

### Task 6: parser — 대입, 리터럴, 보간, `then … else`, `if` 블록, 최소 정수

**Files:**
- Modify: `internal/parser/parser.go`, `internal/parser/expr.go`, `internal/lexer/number.go` (최소 정수 크기 특례)
- Create: `internal/parser/stmt.go` (대입, 블록, if)
- Test: `internal/parser/parser_test.go` (추가), `internal/lexer/number_test.go` (추가)

**Interfaces (겉 문법 형태, desugar가 소비):**
- `(assign (name x) e)` — 문장. `패턴 =`는 M4; M2는 `NAME =`만.
- `(block stmt…)` — 블록 본문. `(if c (block …) ELSE)`, ELSE는 `(block …)` 또는 중첩 `(if …)` 또는 `(nil)`.
- `(then c a b)`.
- 잎: `(nil)` (Lit 없음), `string` (Lit string, 보간 없는 문자열·원시 문자열), `symbol` (Lit 이름), `duration` (Lit `time.Duration`). `(interp 조각…)`: 조각은 `string` 잎 또는 식.
- 렉서: 10진 `9223372036854775808`(정확히 이 값)은 E0102 대신 `Int` 토큰(값 `math.MinInt64`)으로 내보낸다. 파서는 단항 `-` 바로 뒤에서만 이를 받아 `(int -9223372036854775808)`로 접고, 그 외 자리에서는 E0102를 낸다.
- 파서 에러 E0112 `expected %s, found %s`: `if` 조건 뒤 `:`, 블록 끝 `;;`, `then` 뒤 `else`가 없을 때.

문법 (01 EBNF의 M2 부분):
```
stmt     = assign | expr .            assign = NAME "=" expr .
if_expr  = "if" expr ":" body { "else" "if" expr ":" body } [ "else" ":" body ] ";;" .
body     = { sep } { stmt { sep } } .   (* 블록 안에서는 괄호 깊이와 무관하게 줄바꿈이 구분자 *)
then     = or_expr "then" expr "else" expr .   (* 양쪽 가지는 완전한 expr; 오른쪽 결합 *)
```

- [ ] **Step 1: 실패하는 테스트** (`parser_test.go`에 추가)

```go
func TestAssignmentStatements(t *testing.T) {
	cases := map[string]string{
		"x = 1":              "(program (assign x 1))",
		"x = 1\ny = x + 1":   "(program (assign x 1) (assign y (binary + x 1)))",
		"x =\n  1 + 2":       "(program (assign x (binary + 1 2)))",
		"x = y = 1":          "", // error, tested below
	}
	for src, want := range cases {
		if want == "" {
			continue
		}
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
	if got := parseErr(t, "x = y = 1"); got != "E0110" {
		t.Errorf("chained assignment: %s, want E0110", got)
	}
	if got := parseErr(t, "x == 1 = 2"); got != "E0110" {
		t.Errorf("assignment to expression: %s, want E0110", got)
	}
}

func TestLiterals(t *testing.T) {
	cases := map[string]string{
		"nil":            "(program (nil))",
		`"a b"`:          `(program "a b")`,
		"`raw ${x}`":     `(program "raw \${x}")`,
		"'ok":            "(program 'ok)",
		"1h30m":          "(program 1h30m)",
		`"x=${x + 1}!"`:  `(program (interp "x=" (binary + x 1) "!"))`,
		`"${a}${b}"`:     "(program (interp a b))",
		`"${m}"`:         "(program (interp m))",
		`"${"in" + s}"`:  `(program (interp (binary + "in" s)))`,
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestInterpolationErrorsKeepOuterPositions(t *testing.T) {
	toks, _, _ := lexer.Lex("t.fung", "x = 1\nprint(\"a ${1 +} b\")")
	_, err := Parse(toks)
	if err == nil || err.Code != "E0110" || err.Pos.Line != 2 || err.Pos.Col != 15 {
		t.Fatalf("err = %v, want E0110 at 2:15 (the closing brace)", err)
	}
}

func TestThenElse(t *testing.T) {
	cases := map[string]string{
		"c then 1 else 2":                   "(program (then c 1 2))",
		"a then 1 else b then 2 else 3":     "(program (then a 1 (then b 2 3)))",
		"x = n > 0 then \"pos\" else \"neg\"": `(program (assign x (then (binary > n 0) "pos" "neg")))`,
		"c then 1\n  else 2":                "(program (then c 1 2))",
		"c then a or b else d":              "(program (then c (binary or a b) d))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
	if got := parseErr(t, "c then 1"); got != "E0112" {
		t.Errorf("then without else: %s, want E0112", got)
	}
}

func TestIfBlocks(t *testing.T) {
	cases := map[string]string{
		"if c: 1 ;;":                          "(program (if c (block 1) (nil)))",
		"if c: 1 else: 2 ;;":                  "(program (if c (block 1) (block 2)))",
		"if a: 1 else if b: 2 ;;":             "(program (if a (block 1) (if b (block 2) (nil))))",
		"if a: 1 else if b: 2 else: 3 ;;":     "(program (if a (block 1) (if b (block 2) (block 3))))",
		"if c:\n  x = 1\n  x\nelse:\n  2\n;;": "(program (if c (block (assign x 1) x) (block 2)))",
		"if c: ;;":                            "(program (if c (block) (nil)))",
		"x = if c: 1 else: 2 ;;":              "(program (assign x (if c (block 1) (block 2))))",
		"print(if c:\n  1\nelse:\n  2\n;;)":    "(program (call print (if c (block 1) (block 2))))",
		"if c: if d: 1 ;; else: 2 ;;":          "(program (if c (block (if d (block 1) (nil))) (block 2)))",
		"if c: 1 ;; 2":                        "(program (if c (block 1) (nil)) 2)",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
	errs := map[string]string{
		"if c 1 ;;":        "E0112", // missing colon
		"if c: 1":          "E0112", // missing ;;
		"if c: 1 else 2 ;;": "E0112", // else needs colon or if
		"else: 1 ;;":       "E0110",
		";;":               "E0110",
	}
	for src, want := range errs {
		if got := parseErr(t, src); got != want {
			t.Errorf("%q: %s, want %s", src, got, want)
		}
	}
}

func TestDeeplyNestedBlocksHitTheDepthLimit(t *testing.T) {
	src := strings.Repeat("if true: ", 20000) + "1" + strings.Repeat(" ;;", 20000)
	if got := parseErr(t, src); got != "E0802" {
		t.Fatalf("code = %s, want E0802", got)
	}
}

func TestMinIntLiteral(t *testing.T) {
	if got := parse(t, "-9223372036854775808"); got != "(program -9223372036854775808)" {
		t.Fatalf("got %s", got)
	}
	if got := parse(t, "- 9_223_372_036_854_775_808"); got != "(program -9223372036854775808)" {
		t.Fatalf("with separators: got %s", got)
	}
	for _, src := range []string{"9223372036854775808", "1 + 9223372036854775808", "-(9223372036854775808)"} {
		if got := parseErr(t, src); got != "E0102" {
			t.Errorf("%q: %s, want E0102", src, got)
		}
	}
	if got := parse(t, "-9223372036854775807"); got != "(program (unary - 9223372036854775807))" {
		t.Fatalf("ordinary negative: %s", got)
	}
}
```

`number_test.go`에 추가:
```go
func TestMinIntMagnitudeIsATokenForTheParser(t *testing.T) {
	tok := one(t, "9223372036854775808")
	if tok.Kind != Int || tok.Int != math.MinInt64 {
		t.Fatalf("tok = %+v", tok)
	}
	if _, _, err := Lex("t.fung", "9223372036854775809"); err == nil || err.Code != "E0102" {
		t.Fatalf("one past: %v", err)
	}
}
```
(`math` import; `TestMalformedNumbers`의 `"9223372036854775808": "E0102"` 줄은 `"9223372036854775809"`로 바꾼다.)

- [ ] **Step 2: 실패 확인**.

- [ ] **Step 3: 구현**

`number.go`: 10진 정수 파싱 실패 지점에서
```go
	n, err := strconv.ParseInt(text, 10, 64)
	if err != nil {
		if text == "9223372036854775808" {
			// The magnitude of the smallest int. Only the parser can tell
			// whether a unary minus precedes it (01).
			tok := lx.emit(Int, lx.src[start:lx.off], p)
			tok.Int = math.MinInt64
			return nil
		}
		return diag.Errorf("E0102", p, lx.src[start:lx.off])
	}
```

`parser.go`: `statement`, `expectDesc`, 블록 본문. 새 파일 `stmt.go`:

```go
package parser

import (
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/tree"
)

// statement parses `NAME = expr` or an expression statement.
func (p *parser) statement() (*tree.Node, *diag.Error) {
	t := p.peek()
	if t.Kind == lexer.Name && p.toks[p.i+1].Kind == lexer.Assign {
		p.next()
		p.next()
		p.skipNewlinesAfterOperator()
		rhs, err := p.expr(0)
		if err != nil {
			return nil, err
		}
		return tree.New("assign", t.Pos, tree.Leaf("name", t.Pos, t.Text), rhs), nil
	}
	return p.expr(0)
}

// expectDesc is expect with an E0112 "expected X, found Y" message.
func (p *parser) expectDesc(k lexer.Kind, what string) (lexer.Token, *diag.Error) {
	if !p.at(k) {
		t := p.peek()
		return lexer.Token{}, diag.Errorf("E0112", t.Pos, what, describe(t))
	}
	return p.next(), nil
}

// body parses statements up to (not including) `;;` or `else`. Inside a
// block newlines separate statements even when the block sits inside
// parentheses (01: 괄호 안에서 블록이 열리면 그 ;;까지는 줄바꿈이 구분자).
func (p *parser) body(pos diag.Pos) (*tree.Node, *diag.Error) {
	saved := p.nest
	p.nest = 0
	defer func() { p.nest = saved }()

	blk := tree.New("block", pos)
	p.skipSeps()
	for !p.at(lexer.DoubleSemi) && !p.at(lexer.KwElse) && !p.at(lexer.EOF) {
		stmt, err := p.statement()
		if err != nil {
			return nil, err
		}
		blk.Kids = append(blk.Kids, stmt)
		switch p.peek().Kind {
		case lexer.Newline, lexer.Semi, lexer.DoubleSemi, lexer.KwElse, lexer.EOF:
		default:
			return nil, p.unexpected()
		}
		p.skipSeps()
	}
	return blk, nil
}

// ifExpr parses a whole if/else-if/else chain closed by one `;;`.
func (p *parser) ifExpr() (*tree.Node, *diag.Error) {
	n, err := p.ifChain()
	if err != nil {
		return nil, err
	}
	if _, err := p.expectDesc(lexer.DoubleSemi, "`;;` to close the if"); err != nil {
		return nil, err
	}
	return n, nil
}

// ifChain parses `if cond: body` plus an optional else part, without the
// closing `;;` (the whole chain shares one).
func (p *parser) ifChain() (*tree.Node, *diag.Error) {
	t := p.next() // if
	cond, err := p.expr(0)
	if err != nil {
		return nil, err
	}
	if _, err := p.expectDesc(lexer.Colon, "`:` after the if condition"); err != nil {
		return nil, err
	}
	thenB, err := p.body(t.Pos)
	if err != nil {
		return nil, err
	}
	var elseB *tree.Node
	if p.at(lexer.KwElse) {
		e := p.next()
		switch {
		case p.at(lexer.KwIf):
			elseB, err = p.ifChain()
		case p.at(lexer.Colon):
			p.next()
			elseB, err = p.body(e.Pos)
		default:
			err = diag.Errorf("E0112", p.peek().Pos, "`:` or `if` after else", describe(p.peek()))
		}
		if err != nil {
			return nil, err
		}
	} else {
		elseB = tree.New("nil", t.Pos)
	}
	return tree.New("if", t.Pos, cond, thenB, elseB), nil
}
```

`parser.go`에서 기존 `statement`(expr(0)만 하던 것)를 지운다.

`expr.go`:
- `expr()` 루프 안, 후위 호출 검사 앞에 `then` 처리:
```go
		if t.Kind == lexer.KwThen && bpThen >= minBP {
			p.next()
			p.skipNewlinesAfterOperator()
			a, err := p.expr(0)
			if err != nil {
				return nil, err
			}
			if _, err := p.expectAfterNewlines(lexer.KwElse, "`else` after then"); err != nil {
				return nil, err
			}
			p.skipNewlinesAfterOperator()
			b, err := p.expr(0)
			if err != nil {
				return nil, err
			}
			return tree.New("then", t.Pos, lhs, a, b), nil
		}
```
`expectAfterNewlines`: `else`가 다음 줄 첫 토큰이면 이어짐이므로, 현재 토큰이 Newline이면 `peekInfix()`로 건너뛴 뒤 `expectDesc`.
- `prefix()`에 추가:
```go
	case lexer.KwNil:
		p.next()
		return tree.New("nil", t.Pos), nil
	case lexer.String:
		p.next()
		return p.stringLiteral(t)
	case lexer.RawString:
		p.next()
		return tree.Leaf("string", t.Pos, t.Text), nil
	case lexer.Symbol:
		p.next()
		return tree.Leaf("symbol", t.Pos, t.Text), nil
	case lexer.Duration:
		p.next()
		return tree.Leaf("duration", t.Pos, t.Dur), nil
	case lexer.KwIf:
		return p.ifExpr()
```
- `Int` 케이스: `if isMinIntMagnitude(t) { return nil, diag.Errorf("E0102", t.Pos, t.Text) }`. `Minus` 케이스 첫머리: `if nt := p.toks[p.i+1]; nt.Kind == lexer.Int && isMinIntMagnitude(nt) { p.next(); p.next(); return tree.Leaf("int", t.Pos, int64(math.MinInt64)), nil }` (`p.next()`로 `-`를 먹기 전에 검사). `isMinIntMagnitude(t) = t.Int == math.MinInt64 && strings.ReplaceAll(t.Text, "_", "") == "9223372036854775808"`.
- 문자열:
```go
// stringLiteral turns a "..." token into a string leaf or an (interp ...)
// node whose code parts are parsed with positions in the enclosing file.
func (p *parser) stringLiteral(t lexer.Token) (*tree.Node, *diag.Error) {
	hasCode := false
	for _, part := range t.Parts {
		hasCode = hasCode || part.IsCode
	}
	if !hasCode {
		var text strings.Builder
		for _, part := range t.Parts {
			text.WriteString(part.Text)
		}
		return tree.Leaf("string", t.Pos, text.String()), nil
	}
	n := tree.New("interp", t.Pos)
	for _, part := range t.Parts {
		if !part.IsCode {
			n.Kids = append(n.Kids, tree.Leaf("string", t.Pos, part.Text))
			continue
		}
		toks, _, err := lexer.LexAt(part.Pos.File, part.Text, part.Pos)
		if err != nil {
			return nil, err
		}
		sub := &parser{toks: toks, depth: p.depth}
		e, err := sub.expr(0)
		if err != nil {
			return nil, err
		}
		sub.skipSeps()
		if !sub.at(lexer.EOF) {
			return nil, sub.unexpected()
		}
		n.Kids = append(n.Kids, e)
	}
	return n, nil
}
```
`"${1 +}"`: 하위 파서가 `+` 뒤에서 EOF를 만나 E0110 "unexpected end of file"; EOF 토큰의 위치는 코드 조각 끝 = 닫는 `}`의 위치 (LexAt가 이어서 센 열). 테스트가 기대하는 2:15가 이것이다.

- [ ] **Step 4: 통과 확인** — `go test ./internal/lexer/ ./internal/parser/` PASS. **`desugar`·`resolve`·`compiler`·`vm` 테스트는 새 머리(`assign` 등)를 아직 모르지만 M1 입력만 쓰므로 깨지지 않아야 한다**: `go test ./...` PASS.

- [ ] **Step 5: 커밋** `feat(parser): parse assignment, literals, interpolation, then/else, and if blocks`.

---

### Task 7: desugar — 새 겉 문법 → 코어

**Files:**
- Modify: `internal/desugar/desugar.go`
- Test: `internal/desugar/desugar_test.go` (추가)

**Interfaces (코어 형태):**
- `(let (name x) e)` ← assign. `(do …)` ← block. `(if c a b)` ← then, if. `(concat 조각…)` ← interp. 잎 `nil` `string` `symbol` `duration` 통과.

- [ ] **Step 1: 테스트**

```go
func TestDesugarStatementsAndBlocks(t *testing.T) {
	cases := map[string]string{
		"x = 1":                           "(do (let x 1))",
		"if c: x = 1 else: 2 ;;":          "(do (if c (do (let x 1)) (do 2)))",
		"if c: 1 ;;":                      "(do (if c (do 1) (nil)))",
		"c then 1 else 2":                 "(do (if c 1 2))",
		`"a${x}b"`:                        `(do (concat "a" x "b"))`,
		"nil":                             "(do (nil))",
		"'ok":                             "(do 'ok)",
		"1h":                              "(do 1h)",
		`"plain"`:                         `(do "plain")`,
		"c then a and b else d":           "(do (if c (if a (op bool b) false) d))",
	}
	for src, want := range cases {
		if got := core(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}
```

- [ ] **Step 2: 실패 확인**, **Step 3: 구현** — `Desugar`의 switch에 케이스 추가:

```go
	case "assign":
		rhs, err := Desugar(n.Kids[1])
		if err != nil {
			return nil, err
		}
		return tree.New("let", n.Pos, n.Kids[0], rhs), nil
	case "block":
		return mapKids("do", n)
	case "then", "if":
		return mapKids("if", n)
	case "interp":
		return mapKids("concat", n)
	case "nil", "string", "symbol", "duration":
		return n, nil
```
`mapKids(head, n)`은 자식을 각각 Desugar한 새 노드를 만든다 (`program`·`call` 케이스도 이것을 쓰도록 정리).

- [ ] **Step 4**, **Step 5: 커밋** `feat(desugar): lower assignment, blocks, then/else, and interpolation`.

---

### Task 8: resolve — 스코프, 슬롯·전역, E0205·E0206, 비슷한 이름

**Files:**
- Modify: `internal/resolve/resolve.go`
- Create: `internal/resolve/scope.go`, `internal/resolve/similar.go`
- Test: `internal/resolve/resolve_test.go` (기존 수정 + 추가)

**Interfaces:**
- Produces: `type Result struct{Core *tree.Node; NumLocals int; Globals []string}`
- Produces: `func Resolve(core *tree.Node, prelude []string) (Result, []*diag.Error)` — **시그니처 변경**: prelude는 이름 목록 (비슷한 이름 힌트에도 쓴다). `frontend`, `compiler_test`, `vm_test`(frontend 경유)를 맞춘다.
- 코어 출력: `(local n)`, `(global n)`, `(let (local n) e)`, `(let (global n) e)`, `(prelude name)`.
- 규칙 (03): 루트 `do`는 파일 스코프 → 전역. 중첩 `do`는 블록 스코프 → 슬롯(블록이 끝나면 슬롯 번호 재사용). 같은 스코프의 재대입은 같은 칸. 바깥 스코프의 이름에 대입 → E0205. 끝난 블록에서 만든 이름을 읽음 → E0206. 그 외 미정의 → E0204 (+ 비슷한 이름 힌트, 편집 거리 ≤ 2). `let`은 오른쪽을 먼저 해석하고 묶는다. 에러의 `Span`은 이름 길이.

- [ ] **Step 1: 테스트** (`coreOf`는 그대로. `prelude`를 `[]string{"print"}`로. 기존 세 테스트의 호출을 새 시그니처로 고친 뒤 추가:)

```go
func res(t *testing.T, src string) (Result, []*diag.Error) {
	t.Helper()
	return Resolve(coreOf(t, src), []string{"print", "fail"})
}

func TestTopLevelNamesBecomeGlobals(t *testing.T) {
	r, errs := res(t, "x = 1\ny = x + 1\nx = x * 2\nprint(x, y)")
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	want := "(do (let (global 0) 1) (let (global 1) (op + (global 0) 1)) (let (global 0) (op * (global 0) 2)) (call (prelude print) (global 0) (global 1)))"
	if got := r.Core.String(); got != want {
		t.Fatalf("\n got %s\nwant %s", got, want)
	}
	if r.NumLocals != 0 || len(r.Globals) != 2 || r.Globals[0] != "x" || r.Globals[1] != "y" {
		t.Fatalf("locals=%d globals=%v", r.NumLocals, r.Globals)
	}
}

func TestBlockNamesBecomeSlotsAndAreReused(t *testing.T) {
	r, errs := res(t, "if true: a = 1; b = a ;;\nif true: c = 2 ;;")
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	want := "(do (if true (do (let (local 0) 1) (let (local 1) (local 0))) (nil)) (if true (do (let (local 0) 2)) (nil)))"
	if got := r.Core.String(); got != want {
		t.Fatalf("\n got %s\nwant %s", got, want)
	}
	if r.NumLocals != 2 {
		t.Fatalf("NumLocals = %d, want 2", r.NumLocals)
	}
}

func TestNestedBlocksStackSlots(t *testing.T) {
	r, _ := res(t, "if true: a = 1; if true: b = a ;; ;;")
	if r.NumLocals != 2 || !strings.Contains(r.Core.String(), "(let (local 1) (local 0))") {
		t.Fatalf("locals=%d core=%s", r.NumLocals, r.Core)
	}
}

func TestReassigningOuterNameInBlockIsE0205(t *testing.T) {
	_, errs := res(t, "total = 0\nif true: total = total + 1 ;;")
	if len(errs) != 1 || errs[0].Code != "E0205" || errs[0].Args[0] != "total" || errs[0].Len != 5 {
		t.Fatalf("errs = %v", errs)
	}
	if errs[0].Pos.Line != 2 || errs[0].Pos.Col != 10 {
		t.Fatalf("pos = %v", errs[0].Pos)
	}
	// Block-local names may be reassigned inside their own block.
	if _, errs := res(t, "if true: n = 1; n = n + 1 ;;"); len(errs) != 0 {
		t.Fatalf("same-scope rebinding: %v", errs)
	}
	// And a local may not be reassigned from a nested block either.
	if _, errs := res(t, "if true: n = 1; if true: n = 2 ;; ;;"); len(errs) != 1 || errs[0].Code != "E0205" {
		t.Fatalf("nested: %v", errs)
	}
}

func TestUsingBlockNameOutsideIsE0206(t *testing.T) {
	_, errs := res(t, "if true: y = 1 ;;\nprint(y)")
	if len(errs) != 1 || errs[0].Code != "E0206" || errs[0].Args[0] != "y" {
		t.Fatalf("errs = %v", errs)
	}
}

func TestUseBeforeAssignmentIsUndefined(t *testing.T) {
	_, errs := res(t, "print(x)\nx = 1")
	if len(errs) != 1 || errs[0].Code != "E0204" {
		t.Fatalf("errs = %v", errs)
	}
	// The right-hand side is resolved before the name is bound.
	if _, errs := res(t, "x = x + 1"); len(errs) != 1 || errs[0].Code != "E0204" {
		t.Fatalf("self-reference: %v", errs)
	}
}

func TestUndefinedNameSuggestsSimilarName(t *testing.T) {
	_, errs := res(t, "files = 1\nprint(fiels)")
	if len(errs) != 1 || errs[0].Help != "a similar name exists: `files`" || errs[0].Len != 5 {
		t.Fatalf("errs = %+v", errs)
	}
	_, errs = res(t, "print(prnt(1))")
	if len(errs) != 1 || errs[0].Help != "a similar name exists: `print`" {
		t.Fatalf("prelude suggestion: %+v", errs)
	}
	_, errs = res(t, "print(zzzzzz)")
	if len(errs) != 1 || errs[0].Help != "" {
		t.Fatalf("no suggestion expected: %+v", errs)
	}
}
```
(`strings` import 추가. 기존 `TestResolveCollectsAllUndefinedNames`는 `Resolve(..., []string{"print"})`로.)

- [ ] **Step 2: 실패 확인**.

- [ ] **Step 3: 구현**

```go
// internal/resolve/scope.go
package resolve

// binding says where a name lives.
type binding struct {
	global bool
	index  int // global index or local slot
}

// scope is one lexical scope: the file (globals) or a block (slots).
type scope struct {
	names map[string]binding
	file  bool
	base  int // first slot this block may use; restored on exit
}
```

```go
// internal/resolve/similar.go
package resolve

import "sort"

// similar returns the candidate within edit distance 2 of name, preferring
// the closest and then the alphabetically first, or "".
func similar(name string, candidates []string) string {
	best, bestDist := "", 3
	sorted := append([]string(nil), candidates...)
	sort.Strings(sorted)
	for _, c := range sorted {
		if c == name {
			continue
		}
		if d := editDistance(name, c); d < bestDist {
			best, bestDist = c, d
		}
	}
	return best
}

func editDistance(a, b string) int {
	ra, rb := []rune(a), []rune(b)
	prev := make([]int, len(rb)+1)
	cur := make([]int, len(rb)+1)
	for j := range prev {
		prev[j] = j
	}
	for i := 1; i <= len(ra); i++ {
		cur[0] = i
		for j := 1; j <= len(rb); j++ {
			cost := 1
			if ra[i-1] == rb[j-1] {
				cost = 0
			}
			cur[j] = min(prev[j]+1, cur[j-1]+1, prev[j-1]+cost)
		}
		prev, cur = cur, prev
	}
	return prev[len(rb)]
}
```

```go
// internal/resolve/resolve.go — 전체 교체
package resolve

import (
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/tree"
)

// Result is the resolved tree plus what the compiler needs to lay out
// storage: how many block-local slots the main frame needs and the names
// of the file's globals, in index order.
type Result struct {
	Core      *tree.Node
	NumLocals int
	Globals   []string
}

type resolver struct {
	prelude   map[string]bool
	preludeL  []string
	scopes    []*scope
	nextSlot  int
	maxSlot   int
	globals   []string
	closed    map[string]bool // names defined in blocks that have ended (for E0206)
	errs      []*diag.Error
}

// Resolve classifies every name (09): block locals become (local n), file
// globals (global n), prelude names (prelude name). Scope errors are all
// collected (08). The input tree is not modified.
func Resolve(core *tree.Node, prelude []string) (Result, []*diag.Error) {
	r := &resolver{prelude: map[string]bool{}, preludeL: prelude, closed: map[string]bool{}}
	for _, p := range prelude {
		r.prelude[p] = true
	}
	out := r.walk(core, true)
	return Result{Core: out, NumLocals: r.maxSlot, Globals: r.globals}, r.errs
}

func (r *resolver) walk(n *tree.Node, root bool) *tree.Node {
	switch n.Head {
	case "do":
		return r.block(n, root)
	case "let":
		return r.let(n)
	case "name":
		return r.name(n)
	}
	out := &tree.Node{Head: n.Head, Pos: n.Pos, Lit: n.Lit}
	for _, k := range n.Kids {
		out.Kids = append(out.Kids, r.walk(k, false))
	}
	return out
}

func (r *resolver) block(n *tree.Node, root bool) *tree.Node {
	sc := &scope{names: map[string]binding{}, file: root, base: r.nextSlot}
	r.scopes = append(r.scopes, sc)
	out := &tree.Node{Head: "do", Pos: n.Pos}
	for _, k := range n.Kids {
		out.Kids = append(out.Kids, r.walk(k, false))
	}
	r.scopes = r.scopes[:len(r.scopes)-1]
	if !root {
		for name := range sc.names {
			r.closed[name] = true
		}
		r.nextSlot = sc.base
	}
	return out
}

// let resolves the right-hand side first (x = x + 1 reads the old x),
// then binds the name in the current scope.
func (r *resolver) let(n *tree.Node) *tree.Node {
	target, rhs := n.Kids[0], r.walk(n.Kids[1], false)
	name := target.Lit.(string)
	cur := r.scopes[len(r.scopes)-1]
	if b, ok := cur.names[name]; ok {
		return tree.New("let", n.Pos, ref(target.Pos, b), rhs)
	}
	for i := len(r.scopes) - 2; i >= 0; i-- {
		if _, ok := r.scopes[i].names[name]; ok {
			r.errs = append(r.errs, diag.Errorf("E0205", target.Pos, name).Span(runeLen(name)))
			return tree.New("let", n.Pos, target, rhs)
		}
	}
	var b binding
	if cur.file {
		b = binding{global: true, index: len(r.globals)}
		r.globals = append(r.globals, name)
	} else {
		b = binding{index: r.nextSlot}
		r.nextSlot++
		if r.nextSlot > r.maxSlot {
			r.maxSlot = r.nextSlot
		}
	}
	cur.names[name] = b
	return tree.New("let", n.Pos, ref(target.Pos, b), rhs)
}

func (r *resolver) name(n *tree.Node) *tree.Node {
	name := n.Lit.(string)
	for i := len(r.scopes) - 1; i >= 0; i-- {
		if b, ok := r.scopes[i].names[name]; ok {
			return ref(n.Pos, b)
		}
	}
	if r.prelude[name] {
		return tree.Leaf("prelude", n.Pos, name)
	}
	if r.closed[name] {
		r.errs = append(r.errs, diag.Errorf("E0206", n.Pos, name).Span(runeLen(name)))
		return n
	}
	e := diag.Errorf("E0204", n.Pos, name).Span(runeLen(name))
	if s := similar(name, r.visible()); s != "" {
		e.WithHelp("a similar name exists: `" + s + "`")
	}
	r.errs = append(r.errs, e)
	return n
}

// visible lists every name that could have been meant: all scopes plus the prelude.
func (r *resolver) visible() []string {
	var names []string
	for _, sc := range r.scopes {
		for name := range sc.names {
			names = append(names, name)
		}
	}
	return append(names, r.preludeL...)
}

func ref(pos diag.Pos, b binding) *tree.Node {
	if b.global {
		return tree.Leaf("global", pos, int64(b.index))
	}
	return tree.Leaf("local", pos, int64(b.index))
}

func runeLen(s string) int { return len([]rune(s)) }
```

`frontend.Compile`: `resolve.Resolve(core, preludeNames(prelude))` — 정렬된 키 목록을 만드는 도우미를 frontend에 둔다. `compiler.Compile`의 입력은 Task 10에서 `resolve.Result`로 바뀐다; Task 8 시점에는 `compiler.Compile(res.Core, prelude)`로 잇고 `compiler_test`의 호출도 맞춘다.

- [ ] **Step 4: 통과 확인** — `go test ./...` PASS (compiler/vm 테스트는 M1 입력이라 `local`/`global`을 만나지 않는다).

- [ ] **Step 5: 커밋** `feat(resolve): add block scopes, slots and globals, scope errors, and similar-name hints`.

---

### Task 9: bytecode — 슬롯·전역·CONCAT 명령

**Files:**
- Modify: `internal/bytecode/op.go`, `proto.go`, `disasm.go`
- Test: `internal/bytecode/bytecode_test.go` (추가)

**Interfaces:**
- Produces: `OpLoadLocal`, `OpStoreLocal`, `OpLoadGlobal`, `OpStoreGlobal`, `OpConcat` (모두 `HasArg`). 기존 상수 순서 뒤에 추가 (번호 안정성은 요구하지 않지만 역어셈블 골든을 위해 뒤에 붙인다).
- Produces: `Proto.NumLocals int`, `Proto.GlobalNames []string`.
- 역어셈블러: `LOADGLOBAL 0    ; x`처럼 전역은 이름을 주석으로.

- [ ] **Step 1: 테스트**

```go
func TestNewOpsHaveArgsAndNames(t *testing.T) {
	for _, op := range []Op{OpLoadLocal, OpStoreLocal, OpLoadGlobal, OpStoreGlobal, OpConcat} {
		if !op.HasArg() || strings.HasPrefix(op.String(), "Op(") {
			t.Errorf("%v: HasArg=%v name=%q", op, op.HasArg(), op.String())
		}
	}
}

func TestDisassembleShowsGlobalNames(t *testing.T) {
	p := &Proto{Name: "<main>", GlobalNames: []string{"x"}}
	pos := diag.Pos{Line: 1, Col: 1}
	p.Emit(OpConst, p.Const(value.Int(1)), pos)
	p.Emit(OpStoreGlobal, 0, pos)
	p.Emit(OpLoadGlobal, 0, pos)
	p.Emit(OpLoadLocal, 2, pos)
	want := "== <main> ==\n" +
		"0000  1:1   CONST        0    ; 1\n" +
		"0001  1:1   STOREGLOBAL  0    ; x\n" +
		"0002  1:1   LOADGLOBAL   0    ; x\n" +
		"0003  1:1   LOADLOCAL    2\n"
	if got := Disassemble(p); got != want {
		t.Fatalf("got:\n%s\nwant:\n%s", got, want)
	}
}
```

- [ ] **Step 2: 실패 확인**, **Step 3: 구현** — 상수·이름·`HasArg` 추가, `Proto`에 두 필드, 역어셈블러에서 `OpLoadGlobal`/`OpStoreGlobal`이면 `    ; <name>`.

- [ ] **Step 4**, **Step 5: 커밋** `feat(bytecode): add local/global load-store and concat instructions`.

---

### Task 10: compiler — 새 코어 형태와 `Result` 입력

**Files:**
- Modify: `internal/compiler/compiler.go`, `internal/frontend/frontend.go`
- Test: `internal/compiler/compiler_test.go` (수정·추가)

**Interfaces:**
- Produces: `func Compile(res resolve.Result, prelude map[string]value.Value) (*bytecode.Proto, *diag.Error)` — **시그니처 변경**. `Proto.NumLocals`, `Proto.GlobalNames`를 채운다.
- 코어 → 명령: `(let (local n) e)` → e; `STORELOCAL n`. `(let (global n) e)` → e; `STOREGLOBAL n`. `(local n)` → `LOADLOCAL n`. `(global n)` → `LOADGLOBAL n`. `(nil)` → `CONST nil`. `string`/`symbol`/`duration` → `CONST`. `(concat k…)` → k…; `CONCAT n`.
- 블록 값: `do`의 마지막 항목이 `let`이면 저장 뒤 `CONST nil`을 올린다 (01: 블록이 문장으로 끝나면 nil). `let`이 마지막이 아니면 값을 남기지 않으므로 `POP`도 하지 않는다.
- `if`의 `JUMPIFFALSE`는 **조건 노드의 위치**를 쓴다 (E0505 위치 대칭).

- [ ] **Step 1: 테스트** (`compile` 도우미를 `resolve.Resolve(c, []string{"print"})`의 `Result`로 `Compile(res, prelude)` 하도록 고친 뒤 추가)

```go
func TestCompileLetAndLoads(t *testing.T) {
	p := compile(t, "x = 1\ny = x\nif true: a = y; a ;;")
	want := "CONST(0) STOREGLOBAL(0) CONST(1) STOREGLOBAL(1) CONST(2) JUMPIFFALSE(10) LOADGLOBAL(1) STORELOCAL(0) LOADLOCAL(0) JUMP(11) CONST(1) RETURN"
	if got := ops(p); got != want {
		t.Fatalf("\n got %s\nwant %s", got, want)
	}
	if p.NumLocals != 1 || len(p.GlobalNames) != 2 || p.GlobalNames[1] != "y" {
		t.Fatalf("locals=%d globals=%v", p.NumLocals, p.GlobalNames)
	}
	if p.Consts[1] != (value.Nil{}) {
		t.Fatalf("expected nil const for the statement-valued program: %v", p.Consts)
	}
}

func TestCompileBlockEndingInStatementYieldsNil(t *testing.T) {
	p := compile(t, "x = 1")
	if got, want := ops(p), "CONST(0) STOREGLOBAL(0) CONST(1) RETURN"; got != want {
		t.Fatalf("got %s, want %s", got, want)
	}
}

func TestCompileLiteralsAndConcat(t *testing.T) {
	p := compile(t, `"a${1}b" + 'ok`)
	want := "CONST(0) CONST(1) CONST(2) CONCAT(3) CONST(3) ADD RETURN"
	if got := ops(p); got != want {
		t.Fatalf("got %s, want %s", got, want)
	}
	if p.Consts[0] != value.Str("a") || p.Consts[3] != value.Intern("ok") {
		t.Fatalf("consts = %v", p.Consts)
	}
	p = compile(t, "nil\n1h")
	if got, want := ops(p), "CONST(0) POP CONST(1) RETURN"; got != want || p.Consts[1] != value.Dur(time.Hour) {
		t.Fatalf("got %s consts %v", got, p.Consts)
	}
}

func TestJumpIfFalseCarriesConditionPosition(t *testing.T) {
	p := compile(t, "1 and true")
	// CONST 1 at col 1, JUMPIFFALSE should point at the condition (col 1), not the operator (col 3)
	if p.Pos[1].Col != 1 {
		t.Fatalf("JUMPIFFALSE pos = %v, want col 1", p.Pos[1])
	}
}
```
(`time` import.)

- [ ] **Step 2: 실패 확인**, **Step 3: 구현**

```go
func Compile(res resolve.Result, prelude map[string]value.Value) (*bytecode.Proto, *diag.Error) {
	c := &compiler{proto: &bytecode.Proto{Name: "<main>", NumLocals: res.NumLocals, GlobalNames: res.Globals}, prelude: prelude}
	if err := c.block(res.Core); err != nil {
		return nil, err
	}
	c.proto.Emit(bytecode.OpReturn, 0, res.Core.Pos)
	return c.proto, nil
}

func (c *compiler) block(n *tree.Node) *diag.Error {
	if len(n.Kids) == 0 {
		c.proto.Emit(bytecode.OpConst, c.proto.Const(value.Nil{}), n.Pos)
		return nil
	}
	for i, k := range n.Kids {
		last := i == len(n.Kids)-1
		if k.Head == "let" {
			if err := c.let(k); err != nil {
				return err
			}
			if last {
				c.proto.Emit(bytecode.OpConst, c.proto.Const(value.Nil{}), k.Pos)
			}
			continue
		}
		if err := c.expr(k); err != nil {
			return err
		}
		if !last {
			c.proto.Emit(bytecode.OpPop, 0, k.Pos)
		}
	}
	return nil
}

func (c *compiler) let(n *tree.Node) *diag.Error {
	if err := c.expr(n.Kids[1]); err != nil {
		return err
	}
	target := n.Kids[0]
	switch target.Head {
	case "local":
		c.proto.Emit(bytecode.OpStoreLocal, int(target.Lit.(int64)), target.Pos)
	case "global":
		c.proto.Emit(bytecode.OpStoreGlobal, int(target.Lit.(int64)), target.Pos)
	default:
		return diag.Errorf("E0801", n.Pos, "compiler: unresolved assignment target "+target.Head)
	}
	return nil
}
```
`expr`에 케이스 추가:
```go
	case "local":
		p.Emit(bytecode.OpLoadLocal, int(n.Lit.(int64)), n.Pos)
	case "global":
		p.Emit(bytecode.OpLoadGlobal, int(n.Lit.(int64)), n.Pos)
	case "nil":
		p.Emit(bytecode.OpConst, p.Const(value.Nil{}), n.Pos)
	case "string":
		p.Emit(bytecode.OpConst, p.Const(value.Str(n.Lit.(string))), n.Pos)
	case "symbol":
		p.Emit(bytecode.OpConst, p.Const(value.Intern(n.Lit.(string))), n.Pos)
	case "duration":
		p.Emit(bytecode.OpConst, p.Const(value.Dur(n.Lit.(time.Duration))), n.Pos)
	case "concat":
		for _, k := range n.Kids {
			if err := c.expr(k); err != nil {
				return err
			}
		}
		p.Emit(bytecode.OpConcat, len(n.Kids), n.Pos)
	case "let":
		// A let in expression position cannot happen (assignment is a statement).
		return diag.Errorf("E0801", n.Pos, "compiler: let outside a block")
```
`if` 케이스의 `JUMPIFFALSE`를 `p.Emit(bytecode.OpJumpIfFalse, 0, n.Kids[0].Pos)`로. `frontend.Compile`은 `compiler.Compile(res, prelude)`로.

- [ ] **Step 4**, **Step 5: 커밋** `feat(compiler): compile assignments, locals, globals, literals, and concat`.

---

### Task 11: vm·native — 슬롯, 전역, CONCAT, E0207, `fail`

**Files:**
- Modify: `internal/vm/vm.go`, `internal/native/prelude.go`
- Test: `internal/vm/vm_test.go` (추가), `internal/native/prelude_test.go` (추가)

**Interfaces:**
- `Run(p)`: 스택을 `p.NumLocals`개의 빈 칸(Go nil)으로 시작하고 프레임 `base = 0`. `Task.globals = make([]value.Value, len(p.GlobalNames))`.
- `LOADLOCAL n` → `stack[base+n]`; `STORELOCAL n` → `stack[base+n] = pop`. `LOADGLOBAL n` → 빈 칸이면 `E0207`(이름은 `p.GlobalNames[n]`); `STOREGLOBAL n`. `CONCAT n` → n개 꺼내 `FormatForPrint`로 이어 `Str`.
- `native.Prelude()`에 `"fail"`: 인자들을 `print`처럼 공백으로 이어 `diag.Errorf("E0803", diag.Pos{}, msg)`를 **error로 반환** (VM이 위치를 채운다).

- [ ] **Step 1: 테스트**

```go
// vm_test.go 추가
func TestVariablesBlocksAndLiterals(t *testing.T) {
	cases := map[string]string{
		"x = 1\ny = x + 1\nx = x * 10\nprint(x, y)":                "10 2\n",
		"if true: z = 5; print(z) ;;":                             "5\n",
		"w = if 7 > 5: \"big\" else: \"small\" ;;\nprint(w)":      "big\n",
		"print(if false: 1 ;;)":                                   "nil\n",
		"print(1 > 0 then \"a\" else \"b\")":                      "a\n",
		"x = 1\nprint(\"x is ${x}, next ${x + 1}\")":              "x is 1, next 2\n",
		"print(\"a\" + \"b\", \"a\" == \"a\", \"a\" < \"b\")":      "ab true true\n",
		"print('ok, 'ok == 'ok, 'ok == \"ok\")":                   "'ok true false\n",
		"print(1h30m, 500ms + 1.5s, 2 * 30s, 1m / 4, 30s < 1m)":   "1h30m 2s 1m 15s true\n",
		"print(nil, nil == nil, nil == 0)":                        "nil true false\n",
		"print(-9223372036854775808)":                             "-9223372036854775808\n",
		"print(9007199254740993 == 9007199254740992.0)":           "false\n",
		"print(\"${\"q\"}\", \"${'s}\", \"${1.0}\")":              "q 's 1.0\n",
		"x = 1\nx = 2\nprint(x)":                                  "2\n",
	}
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
}

func TestBlockValueAndStatementValue(t *testing.T) {
	v, _, code := run(t, "x = 1")
	if code != "" || v != (value.Nil{}) {
		t.Fatalf("program ending in assignment = %v (%s), want nil", v, code)
	}
	v, _, _ = run(t, "if true: a = 1; a + 1 ;;")
	if v != value.Int(2) {
		t.Fatalf("block value = %v", v)
	}
}

func TestFailRaisesE0803WithPosition(t *testing.T) {
	_, out, code := run(t, "print(1)\nfail(\"boom\", 2)")
	if code != "E0803" || out != "1\n" {
		t.Fatalf("code=%s out=%q", code, out)
	}
	p := compileWith(t, "x = 1\nfail(\"boom\")", native.Prelude())
	_, err := (&Runtime{Stdout: &bytes.Buffer{}}).Run(p)
	if err == nil || err.Error() != "error[E0803]: boom" || err.Pos.Line != 2 || err.Pos.Col != 5 {
		t.Fatalf("err = %v at %v", err, err.Pos)
	}
}

func TestRuntimeErrorsForNewTypes(t *testing.T) {
	cases := map[string]string{
		"nil + 1":        "E0501",
		"\"a\" + 1":      "E0501",
		"'a < 'b":        "E0501",
		"1s * 1s":        "E0501",
		"1s / 0":         "E0502",
		"if 1: 2 ;;":     "E0505",
		"1 then 2 else 3": "E0505",
	}
	for src, want := range cases {
		if _, _, code := run(t, src); code != want {
			t.Errorf("%q: code = %s, want %s", src, code, want)
		}
	}
}
```
E0207은 M2에서는 도달할 수 없다(최상위가 순서대로 실행되고 resolve가 선행 참조를 E0204로 잡는다). 명령 수준에서 직접 검사한다:

```go
func TestLoadGlobalBeforeStoreIsE0207(t *testing.T) {
	p := &bytecode.Proto{Name: "<main>", GlobalNames: []string{"cfg"}}
	pos := diag.Pos{File: "t.fung", Line: 1, Col: 1}
	p.Emit(bytecode.OpLoadGlobal, 0, pos)
	p.Emit(bytecode.OpReturn, 0, pos)
	_, err := (&Runtime{Stdout: &bytes.Buffer{}}).Run(p)
	if err == nil || err.Code != "E0207" || err.Args[0] != "cfg" {
		t.Fatalf("err = %v", err)
	}
}
```
(`diag` import.)

`prelude_test.go` 추가:
```go
func TestFailReturnsE0803(t *testing.T) {
	fail := Prelude()["fail"].(*value.Native)
	_, err := fail.Fn(&ctx{}, []value.Value{value.Str("port"), value.Int(1)})
	d, ok := err.(*diag.Error)
	if !ok || d.Code != "E0803" || d.Args[0] != "port 1" {
		t.Fatalf("err = %v", err)
	}
	if _, err := fail.Fn(&ctx{}, nil); err == nil || err.(*diag.Error).Args[0] != "fail" {
		t.Fatalf("fail() = %v", err)
	}
}
```

- [ ] **Step 2: 실패 확인**, **Step 3: 구현**

`vm.go`:
```go
type Task struct {
	rt      *Runtime
	stack   []value.Value
	frames  []frame
	globals []value.Value
}

func (rt *Runtime) Run(p *bytecode.Proto) (v value.Value, err *diag.Error) {
	t := &Task{rt: rt, globals: make([]value.Value, len(p.GlobalNames))}
	t.stack = make([]value.Value, p.NumLocals, p.NumLocals+16) // slots start empty (Go nil)
	t.frames = append(t.frames, frame{proto: p, base: 0})
	...
```
`run`의 switch에 추가:
```go
		case bytecode.OpLoadLocal:
			t.push(t.stack[f.base+in.Arg()])
		case bytecode.OpStoreLocal:
			t.stack[f.base+in.Arg()] = t.pop()
		case bytecode.OpLoadGlobal:
			v := t.globals[in.Arg()]
			if v == nil {
				return nil, diag.Errorf("E0207", pos, f.proto.GlobalNames[in.Arg()])
			}
			t.push(v)
		case bytecode.OpStoreGlobal:
			t.globals[in.Arg()] = t.pop()
		case bytecode.OpConcat:
			n := in.Arg()
			parts := t.stack[len(t.stack)-n:]
			var b strings.Builder
			for _, part := range parts {
				b.WriteString(value.FormatForPrint(part))
			}
			t.stack = t.stack[:len(t.stack)-n]
			t.push(value.Str(b.String()))
```
`prelude.go`:
```go
		"fail":  &value.Native{Name: "fail", Fn: failFn},
...
// failFn raises a runtime error with the joined arguments as the message
// (06: `x ?? fail("…")`). The VM fills in the position.
func failFn(_ value.Context, args []value.Value) (value.Value, error) {
	msg := "fail"
	if len(args) > 0 {
		parts := make([]string, len(args))
		for i, a := range args {
			parts[i] = value.FormatForPrint(a)
		}
		msg = strings.Join(parts, " ")
	}
	return nil, diag.Errorf("E0803", diag.Pos{}, msg)
}
```

- [ ] **Step 4**, **Step 5: 커밋** `feat(vm): run locals, globals, and concat; add fail to the prelude`.

---

### Task 12: cmd — 소스 발췌 출력, `dump ast -e` 메시지, 골든 테스트

**Files:**
- Modify: `cmd/fung/pipeline.go`, `cmd/fung/main.go`, `cmd/fung/main_test.go`
- Create: `tests/lang/*.fung` (아래 목록)

- [ ] **Step 1: 테스트** (`main_test.go` 추가)

```go
func TestEvalErrorShowsSourceExcerpt(t *testing.T) {
	_, stderr, code := invoke(t, "-e", "x = 1\nprint(fiels)")
	want := "error[E0204]: undefined name `fiels`\n" +
		"  --> -e:2:7\n" +
		"  |\n" +
		"2 | print(fiels)\n" +
		"  |       ^^^^^ not found in this scope\n" +
		"  |\n"
	if code != exitCompileError || stderr != want {
		t.Fatalf("code=%d stderr=%q", code, stderr)
	}
}

func TestRuntimeErrorShowsExcerptFromFile(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "a.fung")
	os.WriteFile(path, []byte("x = 2\nprint(x / 0)\n"), 0o644)
	stdout, stderr, code := invoke(t, path)
	if code != exitRuntimeError || stdout != "" || !strings.Contains(stderr, "2 | print(x / 0)\n  |         ^\n") {
		t.Fatalf("code=%d stdout=%q stderr=%q", code, stdout, stderr)
	}
}

func TestDumpAstWithEButNoCodeIsUsageError(t *testing.T) {
	_, stderr, code := invoke(t, "dump", "ast", "-e")
	if code != exitUsage || !strings.Contains(stderr, "-e needs code") {
		t.Fatalf("code=%d stderr=%q", code, stderr)
	}
}
```
(`os`, `path/filepath` import.)

골든 파일 (`tests/lang/`):

```
# assign.fung
x = 1
y = x + 1
x = x * 10
print(x, y)
# expect: 10 2
if true: z = 5; print(z) ;;
# expect: 5
w = if x > 5: "big" else: "small" ;;
print(w)
# expect: big
print(if false: 1 ;;)
# expect: nil
```
```
# if_blocks.fung
n = 0
label = if n > 0:
  "pos"
else if n < 0:
  "neg"
else:
  "zero"
;;
print(label)
# expect: zero
print(if true: if false: 1 else: 2 ;; ;;)
# expect: 2
print(
  if 1 < 2:
    "yes"
  ;;
)
# expect: yes
```
```
# then_else.fung
n = 5
print(n > 0 then "pos" else "neg")
# expect: pos
print(n > 10 then "big" else n > 3 then "mid" else "small")
# expect: mid
label = n == 5 then "five"
  else "other"
print(label)
# expect: five
```
```
# strings.fung
name = "fung"
print("hello, ${name}!", "tab\tsep", "quote\"d", "\$notvar ${1 + 1}")
# expect: hello, fung!	tab	sep	quote"d	$notvar 2
```
위 줄은 탭이 들어가 골든에 쓰기 불편하다. 실제 파일은 다음으로 한다:
```
# strings.fung
name = "fung"
print("hello, ${name}!")
# expect: hello, fung!
print("quote\"d", "\$notvar ${1 + 1}", "${"nested ${name}"}")
# expect: quote"d $notvar 2 nested fung
print(`raw \n ${x}`)
# expect: raw \n ${x}
print("a" + "b" == "ab", "abc" < "abd", "가" < "나")
# expect: true true true
print("${'sym} ${1.5} ${nil} ${true}")
# expect: 'sym 1.5 nil true
```
```
# symbols.fung
s = 'ok
print(s, s == 'ok, s == "ok", 'a == 'b)
# expect: 'ok true false false
```
```
# durations.fung
print(1h30m, 500ms + 1.5s, 2 * 30s, 1m / 4, 30s < 1m, 1.5ms, -5s)
# expect: 1h30m 2s 1m 15s true 1.5ms -5s
print(1h - 1h, 0.5 * 1m)
# expect: 0s 30s
```
```
# nil.fung
print(nil, nil == nil, nil == 0, nil == false)
# expect: nil true false false
```
```
# minint.fung
print(-9223372036854775808, -9223372036854775807 - 1 == -9223372036854775808)
# expect: -9223372036854775808 true
```
```
# compare_exact.fung
print(9007199254740993 == 9007199254740992.0, 9007199254740993 > 9007199254740992.0)
# expect: false true
```
에러 골든 (파일마다 하나):

| 파일 | 내용 | expect |
|------|------|------|
| `err_reassign_outer.fung` | `total = 0` / `if true: total = total + 1 ;;` | E0205 |
| `err_block_leak.fung` | `if true: y = 1 ;;` / `print(y)` | E0206 |
| `err_use_before_assign.fung` | `print(x)` / `x = 1` | E0204 |
| `err_fail.fung` | `print(1)` + `# expect: 1` / `fail("boom")` | E0803 |
| `err_expected_colon.fung` | `if true 1 ;;` | E0112 |
| `err_missing_end.fung` | `if true: 1` | E0112 |
| `err_escape.fung` | `print("C:\new")` | E0103 |
| `err_interp.fung` | `print("${1 +}")` | E0110 |
| `err_nil_arith.fung` | `print(nil + 1)` | E0501 |
| `err_str_int.fung` | `print("a" + 1)` | E0501 |
| `err_cond_not_bool.fung` | `if 1: 2 ;;` | E0505 |

- [ ] **Step 2: 실패 확인** (`go test ./cmd/fung/ ./tests/`).

- [ ] **Step 3: 구현** — `pipeline.go`의 `execute`/`dump`가 `reportErrors(errs, src, stderr)`로 소스를 넘긴다 (`-e`는 코드 자체가 소스). `runDump`에서 `case len(rest) >= 1 && rest[0] == "-e":`를 먼저 두고 `len(rest) < 2`면 `error: -e needs code`.

- [ ] **Step 4: 통과 확인** — `go test ./...` 전부 PASS, 골든 수 13 + 20 = 33. 바이너리 확인:

```bash
go build -o /tmp/fung-m2 ./cmd/fung && /tmp/fung-m2 -e 'name = "fung"
print("hello, ${name}!")
total = 0
if true: total = total + 1 ;;'; echo "exit=$?"; rm /tmp/fung-m2
```
Expected: `hello, fung!`은 출력되지 않고(컴파일 에러라 실행 전에 멈춤) E0205 발췌와 힌트, `exit=2`.

- [ ] **Step 5: 커밋** `feat(cli): show source excerpts in errors; add M2 golden tests`.

---

### Task 13: 문서 — 노트, 03 가리기 문구, 상태

**Files:**
- Create: `docs/notes/M2-variables-and-flow.md`
- Modify: `docs/design/03-functions-and-patterns.md` (가리기 문구), `docs/design/09-implementation-plan.md` (M2 완료), `CLAUDE.md`

- [ ] **Step 1: 03 문구 수정** — "블록 스코프" 절의
  `- 새 이름(인자, `for` 변수, 패턴, 블록 안의 새 대입)이 바깥 이름을 가리는 것은 허용. …`
  를
  `- 새 이름(인자, `for` 변수, 패턴)이 바깥 이름을 가리는 것은 허용. **블록 안에서 바깥 이름에 `=`로 대입하는 것은 가리기가 아니라 재할당으로 보아 에러**(E0205). …`
  로. "블록 안의 새 대입"은 01의 재할당 금지 규칙과 모순이었다.

- [ ] **Step 2: 노트 작성** — 절 구성:
  1. 한눈에: `x = 1; if c: y = x ;;`가 `(let (global 0) 1)`, `(let (local 0) (global 0))`, `STOREGLOBAL`/`STORELOCAL`로 변하는 과정 (`dump core`, `dump bytecode` 출력 포함).
  2. 이름 해석: 왜 실행 중에 이름을 찾지 않는가. 전역(파일 최상위)과 슬롯(블록)의 차이와 M3에서 전역이 필요한 이유. 슬롯 재사용. `let`이 오른쪽을 먼저 해석하는 이유(`x = x + 1`).
  3. 스코프 규칙의 구현: E0205·E0206·E0204의 판정 순서와 "끝난 블록 이름" 기억. 비슷한 이름 힌트(편집 거리).
  4. 블록과 값: 블록이 문장으로 끝나면 nil을 올리는 컴파일, `if` 없는 else, 괄호 안의 블록에서 줄바꿈 처리(`nest` 저장·복원).
  5. 문자열: 보간 조각의 재파싱과 `LexAt`, 위치가 바깥 파일 기준인 이유, `CONCAT`과 `FormatForPrint`, 리터럴 출력 시 `\$` 규칙.
  6. 심볼 인턴 표, 기간 산술과 출력 형식, 정확한 int/float 비교 알고리즘(2^53 문제).
  7. 에러 발췌 출력: 탭 확장, 거터, 라벨·힌트, `Span`.
  8. `then … else`의 파싱(우선순위 최하, 오른쪽 결합), `if` 체인 하나의 `;;`.
  9. 테스트 요약과 다음 M3 예고(함수·프레임·클로저가 슬롯 위에 올라감).

- [ ] **Step 3: 상태** — 09의 M2 행 끝 ` — 완료`, CLAUDE.md "현재 상태"에 M2 완료 줄과 다음 M3.

- [ ] **Step 4: 커밋** `docs: add M2 learning notes, fix shadowing wording, update status`.
