# M3 함수 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `fn` 정의(짧은 형·블록 형·익명)와 람다, 호출 규약(위치·이름 인자, 기본값), 프레임과 클로저(값 복사, `LOAD_SELF`), `return`, 재귀와 꼬리 호출 최적화, 타입 표기 검사, `at_exit`, 스택 한도, 그리고 런타임 에러의 **호출 경로** 출력을 만든다. `count(1000000, 0)`이 돈다.

**Architecture:** 함수는 컴파일 시점에 자식 `Proto`가 되고, 실행 시점에 `Closure{Proto, Captured}`가 된다. 클로저는 만들어질 때 붙잡을 값을 **복사**한다 (03: 변수는 바뀌지 않으므로 upvalue 상자가 필요 없다). 지역 이름 찾기는 resolve가 함수 스코프 사슬을 따라 `(local n)` / `(captured n)` / `(self)` / `(global n)`으로 바꾼다. 최상위 `fn`은 전역 상수로 **끌어올려**(hoist) 순서와 무관하게 서로 부른다. 호출은 `CALL argc|named<<12`로 스택의 [함수, 위치 인자…, (이름, 값)…]을 새 프레임의 슬롯에 묶고, 기본값과 타입 검사는 함수 서두(prologue)에 컴파일된다. 꼬리 위치의 호출은 `TAILCALL`로 프레임을 재사용한다. VM 루프는 반복문이라 fung의 깊은 재귀가 Go 스택을 키우지 않으며, 프레임 100,000에서 E0804를 낸다.

**Tech Stack:** Go 1.27, 표준 라이브러리만.

**Spec:** `docs/design/01-syntax.md` (함수·람다·`return` 문법, EBNF), `03-functions-and-patterns.md` (반환, 인자, 평가 순서, 꼬리 호출, 스코프와 클로저), `07-operators-and-types.md` (타입 표기), `08-tooling-and-errors.md` (호출 경로 출력), `09-implementation-plan.md` (Proto, 프레임, 클로저, 호출 규약, 한도).

## Global Constraints

- Go 모듈 `github.com/tunarider/fung`, `go 1.27`, **Go 표준 라이브러리만**.
- 코드·주석·에러 메시지·커밋 메시지는 영어. 문서는 한국어.
- Conventional Commits. 커밋 전 `gofmt -l .`(출력 없음), `go vet ./...`, `go test ./...` 통과.
- 커밋 메시지 끝에 `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Go 패닉·스택 추적은 사용자에게 보이지 않는다. 바이트코드 명령마다 소스 위치. 종료 코드 0/1/2/3.
- **에러 번호는 한 번 붙이면 바꾸지 않는다.** M1·M2 번호 유지.
- 렉서는 닫혀 있다 (이번엔 손대지 않는다).

## 계획 수준의 결정 (사양에 없거나 조정한 것)

1. **가변 인자 `...rest`는 M4로 미룬다.** 리스트 값이 M4에서 생기므로 M3에서는 표현할 수 없다. 문법은 받아들이되 resolve가 E0805 "not supported yet"을 낸다.
2. **런타임 에러의 호출 경로 출력을 M5에서 M3로 당긴다.** 프레임이 M3에서 생기므로 비용이 작고, M3 자체를 디버깅하는 데 필요하다.
3. **`_` 부분 적용은 M4** (09 그대로).
4. **`at_exit` 실행 순서는 등록의 역순**(LIFO). 03에 명시가 없어 C `atexit`과 같게 한다. 정상 종료와 런타임 에러 중단 모두에서 실행하고, 핸들러의 에러는 출력하되 다음 핸들러를 계속 실행한다. `exit()`는 M5.
5. 최상위 `fn` 이름은 **전역 상수**다 (03). 같은 파일에 두 번 정의하면 E0210, `=`로 덮어쓰면 E0209.
6. 타입 이름은 07의 목록(`int float num str bool nil sym dur time list map tuple range fn module task channel atom any`)과 태그 이름(대문자, M4에서 값이 생김)만. 그 외는 E0212. `list[int]`의 원소 타입은 받아들이되 버린다 (07: 바깥 타입만 검사).

## Review Focus

1. 비꼬리 재귀 200,000 깊이 → E0804, Go 스택 추적 없음. 50,000은 동작 (Task 9 테스트에 포함).
2. 클로저가 붙잡은 뒤 바깥 변수가 재할당되어도 클로저의 값은 그대로 (Task 9 테스트: `x = 1; f = () => x; x = 2; f()` → 1).
3. 람다 안의 람다가 두 단계 바깥 함수의 인자를 쓴다 (Task 6·9 테스트: 중첩 캡처).
4. 반환 타입 표기가 있는 함수는 꼬리 호출 최적화에서 제외되어 깊은 재귀에서 E0804가 난다 (Task 8·9 테스트).
5. `at_exit` 핸들러 안의 런타임 에러가 나머지 핸들러 실행을 막지 않고 종료 코드는 1이다 (Task 10 테스트).

## 파일 구조

| 패키지 | 변경 | 작업 |
|------|------|------|
| `internal/diag` | `CallSite`, `Error.CallPath`, 호출 경로 출력, 새 번호 | Task 1 |
| `internal/tree` | `FnInfo` `Param` `Capture` 타입과 출력 | Task 2 |
| `internal/value` | `Fn` 인터페이스, `Context`에 `Call`·`AtExit`, 타입 매칭 | Task 3 |
| `internal/parser` | `fn` 정의, 람다, 익명 함수, `return`, 이름 인자, 타입 표기 | Task 4 |
| `internal/desugar` | 새 형태 통과 | Task 5 |
| `internal/resolve` | 함수 스코프, 캡처, self, 끌어올리기, 상수, 새 에러 | Task 6 |
| `internal/bytecode` | `Closure`, Proto 확장, 새 명령 | Task 7 |
| `internal/compiler` | 함수 컴파일, 서두, 꼬리 호출, 클로저 | Task 8 |
| `internal/vm`, `internal/native` | 프레임·호출·클로저·한도·호출 경로·재진입 `Call`·`at_exit` | Task 9 |
| `cmd/fung`, `tests/lang` | 종료 코드 수정, `at_exit` 실행, 골든 | Task 10 |
| `docs` | 노트 M3, 09의 가변 인자 이동, CLAUDE.md | Task 11 |

### M3에서 할당하는 에러 번호

| 번호 | 메시지 | 비고 |
|------|------|------|
| E0208 | `` local function `%s` is defined later in this block `` | 힌트: `local functions can only call themselves; move both functions to the top level` |
| E0209 | `` cannot reassign function `%s` `` | 최상위 fn은 상수 |
| E0210 | `` function `%s` is defined twice `` | |
| E0211 | `return outside a function` | 힌트: `` use `exit()` to stop a script early `` |
| E0212 | `` unknown type `%s` `` | |
| E0402 | `%s takes %d argument(s), got %d` | 런타임 |
| E0403 | `` argument `%s` must be %s, got %s `` | 런타임 |
| E0404 | `return value must be %s, got %s` | 런타임 |
| E0405 | `` argument `%s` given twice `` | 런타임 |
| E0406 | `` %s has no argument named `%s` `` | 런타임 |
| E0407 | `` missing argument `%s` `` | 런타임 |
| E0804 | `stack overflow: call depth exceeded %d` | 힌트: `` make the recursion a tail call or use `fold` `` |
| E0805 | `not supported yet: %s` | |

---

### Task 1: diag — 호출 경로, 새 번호, 종료 코드 수정

**Files:**
- Modify: `internal/diag/error.go`, `internal/diag/catalog.go`, `cmd/fung/pipeline.go`
- Test: `internal/diag/error_test.go`, `cmd/fung/main_test.go`

**Interfaces:**
- Produces: `type CallSite struct{File string; Line int; Fn string; Elided int}`, `Error.CallPath []CallSite` (안쪽부터), `func (e *Error) WithCallPath(p []CallSite) *Error` (비어 있을 때만 채움).
- `Format`은 발췌 뒤, 힌트 앞에 호출 경로를 찍는다 (08):
```
 call path (innermost first):
   report.fung:7   format_size
   report.fung:15  report
   report.fung:22  <entry>
```
  - `Fn`이 `<main>`이면 `<entry>`로. `Elided > 0`이면 ` (N tail calls elided)`를 덧붙인다. 프레임이 12개를 넘으면 안쪽 8개, `   … N more`, 바깥 2개만.
- `cmd/fung/pipeline.go`의 `execute`: `Run`에서 나온 에러는 항상 `exitRuntimeError` (E0207이 이제 도달 가능).

- [ ] **Step 1: 테스트**

```go
func TestFormatCallPath(t *testing.T) {
	e := Errorf("E0501", Pos{File: "r.fung", Line: 7, Col: 11}, "add", "int", "str").WithCallPath([]CallSite{
		{File: "r.fung", Line: 7, Fn: "format_size"},
		{File: "r.fung", Line: 15, Fn: "report", Elided: 3},
		{File: "r.fung", Line: 22, Fn: "<main>"},
	})
	want := "error[E0501]: cannot add int and str\n" +
		"  --> r.fung:7:11\n" +
		" call path (innermost first):\n" +
		"   r.fung:7   format_size\n" +
		"   r.fung:15  report (3 tail calls elided)\n" +
		"   r.fung:22  <entry>\n"
	if got := Format(e, ""); got != want {
		t.Fatalf("got:\n%s\nwant:\n%s", got, want)
	}
}

func TestFormatCallPathIsTruncated(t *testing.T) {
	var path []CallSite
	for i := 0; i < 30; i++ {
		path = append(path, CallSite{File: "a.fung", Line: i + 1, Fn: "f"})
	}
	got := Format(Errorf("E0804", Pos{File: "a.fung", Line: 1, Col: 1}, 100000).WithCallPath(path), "")
	if !strings.Contains(got, "   … 20 more\n") || strings.Count(got, "\n   a.fung") != 10 {
		t.Fatalf("got:\n%s", got)
	}
}

func TestWithCallPathKeepsExisting(t *testing.T) {
	e := Errorf("E0502", Pos{}).WithCallPath([]CallSite{{Fn: "a"}})
	e.WithCallPath([]CallSite{{Fn: "b"}})
	if len(e.CallPath) != 1 || e.CallPath[0].Fn != "a" {
		t.Fatal("call path was overwritten")
	}
}

func TestM3CodesExist(t *testing.T) {
	for _, code := range []string{"E0208", "E0209", "E0210", "E0211", "E0212", "E0402", "E0403", "E0404", "E0405", "E0406", "E0407", "E0804", "E0805"} {
		if strings.HasPrefix(Message(code, "a", "b", "c"), "unknown error") {
			t.Errorf("%s missing", code)
		}
	}
}
```
`cmd/fung/main_test.go`:
```go
func TestRuntimeErrorFromVMAlwaysExitsOne(t *testing.T) {
	// E0207 (an E02xx code) raised at runtime must map to exit 1, not 2.
	_, stderr, code := invoke(t, "-e", "fn f() = x\nf()\nx = 1")
	if code != exitRuntimeError || !strings.Contains(stderr, "error[E0207]") {
		t.Fatalf("code=%d stderr=%q", code, stderr)
	}
}
```
(이 cmd 테스트는 Task 10까지 실패한다. Task 1에서는 diag 테스트만 통과시키고 cmd 테스트는 Task 10의 확인에 포함한다.)

- [ ] **Step 2: 실패 확인** — `go test ./internal/diag/`.

- [ ] **Step 3: 구현**

```go
// CallSite is one frame of the call path shown with runtime errors (08).
type CallSite struct {
	File   string
	Line   int
	Fn     string // "<main>" for the entry file
	Elided int    // tail calls that reused this frame
}
```
`Error`에 `CallPath []CallSite` 필드. `WithCallPath`는 `len(e.CallPath) == 0`일 때만 설정.
`Format`에서 발췌 뒤:
```go
	if len(e.CallPath) > 0 {
		writeCallPath(&b, e.CallPath)
	}
```
```go
const callPathHead, callPathTail = 8, 2

func writeCallPath(b *strings.Builder, path []CallSite) {
	b.WriteString(" call path (innermost first):\n")
	write := func(c CallSite) {
		fn := c.Fn
		if fn == "<main>" {
			fn = "<entry>"
		}
		loc := fmt.Sprintf("%s:%d", c.File, c.Line)
		fmt.Fprintf(b, "   %-10s %s", loc, fn)
		if c.Elided > 0 {
			fmt.Fprintf(b, " (%d tail calls elided)", c.Elided)
		}
		b.WriteByte('\n')
	}
	if len(path) <= callPathHead+callPathTail+2 {
		for _, c := range path {
			write(c)
		}
		return
	}
	for _, c := range path[:callPathHead] {
		write(c)
	}
	fmt.Fprintf(b, "   … %d more\n", len(path)-callPathHead-callPathTail)
	for _, c := range path[len(path)-callPathTail:] {
		write(c)
	}
}
```
`%-10s`는 `r.fung:7` 뒤에 공백을 채워 `r.fung:7   format_size`가 되게 한다 (테스트 기대값과 맞춘다: `r.fung:7` 8자 + 2칸 + 구분 1칸 = 3칸, `r.fung:15` 9자 + 1 + 1 = 2칸).

카탈로그에 번호 표의 메시지·힌트 추가. `execute`:
```go
	if _, err := rt.Run(b.Proto); err != nil {
		reportErrors([]*diag.Error{err}, src, stderr)
		return exitRuntimeError
	}
```

- [ ] **Step 4**: `go test ./internal/diag/ ./cmd/fung/` — diag PASS, cmd의 새 테스트는 아직 실패(기록). 나머지 PASS.
- [ ] **Step 5: 커밋** `feat(diag): add call paths and M3 error numbers; runtime errors always exit 1`.

---

### Task 2: tree — 함수 메타데이터

**Files:**
- Modify: `internal/tree/node.go`
- Test: `internal/tree/node_test.go`

**Interfaces:**
```go
// Param is one declared parameter.
type Param struct {
	Name       string
	Type       []string // union alternatives, nil = unchecked
	HasDefault bool
	Rest       bool     // ...name (rejected until M4)
}

type CaptureKind int
const (
	CaptureLocal    CaptureKind = iota // copy the creating frame's slot Index
	CaptureCaptured                    // copy the creating closure's Captured[Index]
	CaptureSelf                        // copy the creating frame's own closure
)
type Capture struct { Kind CaptureKind; Index int }

// FnInfo is the Lit of an (fn …) node. The parser fills the declaration;
// resolve fills NumLocals, Captures and UsesSelf.
type FnInfo struct {
	Name      string   // "" for lambdas and anonymous fns
	Params    []Param
	Return    []string // nil = unchecked
	NumLocals int
	Captures  []Capture
}
func (f *FnInfo) String() string   // add(a, b=?)->int ; lambdas: (a, b)
```
- `(fn <FnInfo> default… body)`: 자식은 기본값 식들(HasDefault인 인자 순서대로) 뒤에 본문 `(do …)`.
- 출력: `(fn add(a, b=?) (do …))`, `(fn (x) (do …))`, 타입은 `a:int|nil`, 반환 `->str`.

- [ ] **Step 1: 테스트**
```go
func TestFnInfoString(t *testing.T) {
	cases := map[*FnInfo]string{
		{Name: "add", Params: []Param{{Name: "a"}, {Name: "b", HasDefault: true}}}:              "add(a, b=?)",
		{Params: []Param{{Name: "x"}}}:                                                           "(x)",
		{Name: "f", Params: []Param{{Name: "p", Type: []string{"int", "nil"}}}, Return: []string{"str"}}: "f(p:int|nil)->str",
		{Name: "g", Params: []Param{{Name: "xs", Rest: true}}}:                                   "g(...xs)",
		{Name: "h"}: "h()",
	}
	for info, want := range cases {
		if got := info.String(); got != want {
			t.Errorf("got %q, want %q", got, want)
		}
	}
	n := WithLit("fn", diag.Pos{}, &FnInfo{Name: "id", Params: []Param{{Name: "x"}}}, New("do", diag.Pos{}, Leaf("name", diag.Pos{}, "x")))
	if got := n.String(); got != "(fn id(x) (do x))" {
		t.Errorf("node = %s", got)
	}
}
```
- [ ] **Step 2: 실패 확인**, **Step 3: 구현** — 위 타입들과 `String()`; `litString`에 `case fmt.Stringer: return v.String()`을 `string` 케이스 **앞**에 둔다 (`*FnInfo`는 string이 아니므로 순서는 무관하지만 명시).
- [ ] **Step 4**, **Step 5: 커밋** `feat(tree): add function metadata types`.

---

### Task 3: value — 함수 인터페이스, Context 확장, 타입 매칭

**Files:**
- Modify: `internal/value/value.go`, `internal/value/format.go`, `internal/value/compare.go`
- Create: `internal/value/types.go`
- Test: `internal/value/types_test.go`, `internal/native/prelude_test.go`의 `ctx` 스텁에 메서드 추가

**Interfaces:**
```go
// Fn is any callable value: natives and closures.
type Fn interface {
	Value
	FnName() string // "" for anonymous
}
func (n *Native) FnName() string { return n.Name }

type Context interface {
	Stdout() io.Writer
	Call(fn Value, args []Value) (Value, error) // re-enter the VM (map, at_exit, …)
	AtExit(fn Value)                            // register an exit handler
}

// MatchesType reports whether v satisfies a union of type names (07):
// "any" always, "num" for int|float, a tag name for a tag value (M4),
// otherwise v.TypeName() == name.
func MatchesType(v Value, alts []string) bool
func TypeText(alts []string) string // "int | nil"
```
`Format`: `case Fn: "<fn " + name + ">"` (name 비면 `<fn>`). `Equal`: 두 `Fn`은 동일성(`a == b`).

- [ ] **Step 1: 테스트**
```go
func TestMatchesType(t *testing.T) {
	cases := []struct {
		v    Value
		alts []string
		want bool
	}{
		{Int(1), []string{"int"}, true}, {Int(1), []string{"num"}, true}, {Float(1), []string{"num"}, true},
		{Str("a"), []string{"num"}, false}, {Nil{}, []string{"any"}, true}, {Nil{}, []string{"str", "nil"}, true},
		{Bool(true), []string{"str", "nil"}, false}, {&Native{Name: "p"}, []string{"fn"}, true},
		{Int(1), []string{"Circle"}, false}, {Int(1), nil, true},
	}
	for _, c := range cases {
		if got := MatchesType(c.v, c.alts); got != c.want {
			t.Errorf("MatchesType(%v, %v) = %v", c.v, c.alts, got)
		}
	}
	if TypeText([]string{"int", "nil"}) != "int | nil" {
		t.Error("TypeText")
	}
}

func TestFnFormatAndEquality(t *testing.T) {
	a, b := &Native{Name: "print"}, &Native{Name: "print"}
	if Format(a) != "<fn print>" || Format(&Native{}) != "<fn>" {
		t.Error("Format")
	}
	if !Equal(a, a) || Equal(a, b) {
		t.Error("identity")
	}
}
```
- [ ] **Step 2**, **Step 3: 구현** (`types.go`에 `MatchesType`/`TypeText`; `compare.go`의 `*Native` 케이스를 `Fn` 케이스로 일반화; `native/prelude_test.go`의 `ctx`에 `Call`(`return nil, nil`)과 `AtExit`(no-op) 추가).
- [ ] **Step 4**, **Step 5: 커밋** `feat(value): add the Fn interface, VM re-entry in Context, and type matching`.

---

### Task 4: parser — `fn`, 람다, 익명 함수, `return`, 이름 인자, 타입 표기

**Files:**
- Create: `internal/parser/fn.go`
- Modify: `internal/parser/stmt.go` (`fn NAME` 문장), `internal/parser/expr.go` (`fn(`, 람다, `return`, 이름 인자)
- Test: `internal/parser/fn_test.go`

**Interfaces (겉 문법 형태):**
- `(fn <FnInfo> default… body)`: `fn NAME(params) [-> type] = expr` → body `(block expr)`; `fn NAME(params) [-> type]: … ;;` → body `(block …)`; 익명 `fn(params) [-> type]: … ;;`; 람다 `x => e`, `(a, b) => e`, `() => e` → Name "", body `(block e)`.
- `(return e)`; `return` 뒤에 줄바꿈·`;`·`;;`·`)`·`else`·EOF면 `(return (nil))`.
- 호출 인자 `name: e` → `(named e)` (Lit 이름). 위치 인자 뒤에만 올 수 있다 (앞에 오면 E0110).
- 타입: `type = term { | term }`, `term = NAME [ "[" type "]" ] | TAG`. 원소 타입은 파싱만 하고 버린다.
- `...rest`: 파싱해서 `Param.Rest = true`.
- `fn NAME` 문장은 `statement()`에서, `fn(`는 `prefix()`에서. 람다는 `prefix()`에서 `NAME =>` 또는 `( [NAME {, NAME}] ) =>` 룩어헤드.

문법 (01 EBNF):
```
fn_def   = "fn" NAME "(" [ params ] ")" [ "->" type ] fn_body .
fn_body  = "=" expr | block .
params   = param { "," param } [ "," ] .
param    = [ "..." ] NAME [ ":" type ] [ "=" expr ] .
lambda   = ( NAME | "(" [ NAME { "," NAME } ] ")" ) "=>" expr .
arg      = [ NAME ":" ] expr .
```

- [ ] **Step 1: 테스트**

```go
// internal/parser/fn_test.go
package parser

import "testing"

func TestFnDefinitions(t *testing.T) {
	cases := map[string]string{
		"fn add(a, b) = a + b":                       "(program (fn add(a, b) (block (binary + a b))))",
		"fn f():\n  1\n  2\n;;":                      "(program (fn f() (block 1 2)))",
		"fn g(a, b = 1, c = a + b) = c":               "(program (fn g(a, b=?, c=?) 1 (binary + a b) (block c)))",
		"fn h(n: int, s: str | nil = nil) -> bool = true": "(program (fn h(n:int, s:str|nil=?)->bool (nil) (block true)))",
		"fn k(xs: list[int]) = xs":                   "(program (fn k(xs:list) (block xs)))",
		"fn v(...rest) = rest":                       "(program (fn v(...rest) (block rest)))",
		"fn t(s: Circle) = s":                        "(program (fn t(s:Circle) (block s)))",
		"fn f(a,) = a":                               "(program (fn f(a) (block a)))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestLambdasAndAnonymousFns(t *testing.T) {
	cases := map[string]string{
		"f = x => x * 2":                    "(program (assign f (fn (x) (block (binary * x 2)))))",
		"f = (a, b) => a + b":               "(program (assign f (fn (a, b) (block (binary + a b)))))",
		"f = () => 1":                       "(program (assign f (fn () (block 1))))",
		"x => y => x + y":                   "(program (fn (x) (block (fn (y) (block (binary + x y))))))",
		"x => c then 1 else 2":              "(program (fn (x) (block (then c 1 2))))",
		"g(xs, x => x + 1, 3)":              "(program (call g xs (fn (x) (block (binary + x 1))) 3))",
		"f = fn(a):\n  a\n;;":               "(program (assign f (fn (a) (block a))))",
		"f = fn(a) -> int:\n  a\n;;":        "(program (assign f (fn (a)->int (block a))))",
		"(1 + 2) * 3":                       "(program (binary * (binary + 1 2) 3))", // still a grouping
		"(x)":                               "(program x)",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestReturnAndNamedArgs(t *testing.T) {
	cases := map[string]string{
		"fn f() = return 1":                 "(program (fn f() (block (return 1))))",
		"fn f():\n  if c: return ;;\n  2\n;;": "(program (fn f() (block (if c (block (return (nil))) (nil)) 2)))",
		"serve(8080, dir: \"./pub\")":       `(program (call serve 8080 (named "./pub")))`,
		"f(a: 1, b: 2)":                     "(program (call f (named 1) (named 2)))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
	errs := map[string]string{
		"f(a: 1, 2)":          "E0110", // positional after named
		"fn (a) = a":          "E0110", // anonymous fn only has the block form
		"fn f(a = 1, b) = a":  "E0110", // required after optional
		"fn f(a: ) = a":       "E0110",
		"fn f(a, a) = a":      "E0110", // duplicate parameter
		"x =>":                "E0110",
		"fn f() -> = 1":       "E0110",
		"fn f(...a, b) = a":   "E0110", // rest must be last
	}
	for src, want := range errs {
		if got := parseErr(t, src); got != want {
			t.Errorf("%q: %s, want %s", src, got, want)
		}
	}
}
```
`(named "./pub")`의 출력: `named` 노드는 Lit에 이름을 담는다. `tree.write`는 가지 노드의 Lit을 머리 뒤에 찍으므로 실제 출력은 `(named dir "./pub")`이다. **테스트 기대값을 `(named dir "./pub")`, `(named a 1) (named b 2)`로 쓴다.**

- [ ] **Step 2: 실패 확인**.

- [ ] **Step 3: 구현**

```go
// internal/parser/fn.go
package parser

import (
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/tree"
)

// fnDef parses `fn NAME(params) [-> type] (= expr | block)` after `fn`
// has been seen (statement) or `fn(` (anonymous expression, name "").
func (p *parser) fnDef(fnTok lexer.Token, name string) (*tree.Node, *diag.Error) {
	info := &tree.FnInfo{Name: name}
	if _, err := p.expectDesc(lexer.LParen, "`(` after fn"); err != nil {
		return nil, err
	}
	defaults, err := p.params(info)
	if err != nil {
		return nil, err
	}
	if p.at(lexer.Arrow) {
		p.next()
		info.Return, err = p.typeSpec()
		if err != nil {
			return nil, err
		}
	}
	var body *tree.Node
	switch {
	case p.at(lexer.Assign) && name != "":
		eq := p.next()
		p.skipNewlinesAfterOperator()
		e, err := p.expr(0)
		if err != nil {
			return nil, err
		}
		body = tree.New("block", eq.Pos, e)
	case p.at(lexer.Colon):
		p.next()
		body, err = p.body(fnTok.Pos)
		if err != nil {
			return nil, err
		}
		if _, err := p.expectDesc(lexer.DoubleSemi, "`;;` to close the function"); err != nil {
			return nil, err
		}
	default:
		return nil, p.unexpected()
	}
	kids := append(defaults, body)
	return tree.WithLit("fn", fnTok.Pos, info, kids...), nil
}

// params parses the parameter list after `(` up to and including `)`.
// It returns the default expressions in parameter order.
func (p *parser) params(info *tree.FnInfo) ([]*tree.Node, *diag.Error) {
	p.nest++
	defer func() { p.nest-- }()
	var defaults []*tree.Node
	seen := map[string]bool{}
	sawDefault, sawRest := false, false
	for !p.at(lexer.RParen) {
		if sawRest {
			return nil, p.unexpected()
		}
		var prm tree.Param
		if p.at(lexer.Ellipsis) {
			p.next()
			prm.Rest = true
			sawRest = true
		}
		nameTok, err := p.expectDesc(lexer.Name, "a parameter name")
		if err != nil {
			return nil, err
		}
		if seen[nameTok.Text] {
			return nil, diag.Errorf("E0110", nameTok.Pos, "duplicate parameter `"+nameTok.Text+"`")
		}
		seen[nameTok.Text] = true
		prm.Name = nameTok.Text
		if p.at(lexer.Colon) {
			p.next()
			prm.Type, err = p.typeSpec()
			if err != nil {
				return nil, err
			}
		}
		if p.at(lexer.Assign) {
			if prm.Rest {
				return nil, p.unexpected()
			}
			p.next()
			d, err := p.expr(0)
			if err != nil {
				return nil, err
			}
			defaults = append(defaults, d)
			prm.HasDefault = true
			sawDefault = true
		} else if sawDefault && !prm.Rest {
			t := p.peek()
			return nil, diag.Errorf("E0110", nameTok.Pos, "required parameter `"+nameTok.Text+"` after an optional one")
			_ = t
		}
		info.Params = append(info.Params, prm)
		if p.at(lexer.Comma) {
			p.next()
			continue
		}
		if !p.at(lexer.RParen) {
			return nil, p.unexpected()
		}
	}
	p.next() // )
	return defaults, nil
}

// typeSpec parses `term { | term }` where term = NAME [ "[" type "]" ] | TAG.
// Element types are parsed and dropped (07: 런타임은 바깥 타입만 검사).
func (p *parser) typeSpec() ([]string, *diag.Error) {
	var alts []string
	for {
		t := p.peek()
		if t.Kind != lexer.Name && t.Kind != lexer.Tag {
			return nil, diag.Errorf("E0110", t.Pos, describe(t))
		}
		p.next()
		alts = append(alts, t.Text)
		if p.at(lexer.LBracket) {
			p.next()
			if _, err := p.typeSpec(); err != nil {
				return nil, err
			}
			if _, err := p.expectDesc(lexer.RBracket, "`]`"); err != nil {
				return nil, err
			}
		}
		if !p.at(lexer.Bar) {
			return alts, nil
		}
		p.next()
	}
}

// lambdaParamsAhead reports whether the tokens at p.i form
// `( [NAME {, NAME}] ) =>`, without consuming anything.
func (p *parser) lambdaParamsAhead() bool {
	j := p.i
	if p.toks[j].Kind != lexer.LParen {
		return false
	}
	j++
	for p.toks[j].Kind == lexer.Name {
		j++
		if p.toks[j].Kind == lexer.Comma {
			j++
		}
	}
	return p.toks[j].Kind == lexer.RParen && p.toks[j+1].Kind == lexer.FatArrow
}

// lambda parses `NAME => expr` or `( names ) => expr`.
func (p *parser) lambda() (*tree.Node, *diag.Error) {
	start := p.peek()
	info := &tree.FnInfo{}
	if start.Kind == lexer.Name {
		p.next()
		info.Params = []tree.Param{{Name: start.Text}}
	} else {
		p.next() // (
		for p.at(lexer.Name) {
			info.Params = append(info.Params, tree.Param{Name: p.next().Text})
			if p.at(lexer.Comma) {
				p.next()
			}
		}
		p.next() // )
	}
	arrow := p.next() // =>
	p.skipNewlinesAfterOperator()
	body, err := p.expr(0)
	if err != nil {
		return nil, err
	}
	return tree.WithLit("fn", start.Pos, info, tree.New("block", arrow.Pos, body)), nil
}
```
`params()`의 `t := p.peek(); … _ = t` 두 줄은 지운다 (초안의 찌꺼기).

`stmt.go`의 `statement()` 첫머리에:
```go
	if t.Kind == lexer.KwFn && p.toks[p.i+1].Kind == lexer.Name {
		p.next()
		name := p.next()
		return p.fnDef(t, name.Text)
	}
```
`expr.go`의 `prefix()`에:
```go
	case lexer.KwFn:
		p.next()
		return p.fnDef(t, "")
	case lexer.KwReturn:
		p.next()
		switch p.peek().Kind {
		case lexer.Newline, lexer.Semi, lexer.DoubleSemi, lexer.RParen, lexer.KwElse, lexer.EOF, lexer.Comma:
			return tree.New("return", t.Pos, tree.New("nil", t.Pos)), nil
		}
		e, err := p.expr(0)
		if err != nil {
			return nil, err
		}
		return tree.New("return", t.Pos, e), nil
```
`Name` 케이스 앞에 `if t.Kind == lexer.Name && p.toks[p.i+1].Kind == lexer.FatArrow { return p.lambda() }`, `LParen` 케이스 첫머리에 `if p.lambdaParamsAhead() { return p.lambda() }`. 주의: `fn(` 익명 함수는 `=` 본문을 허용하지 않으므로 `fnDef`의 `case p.at(lexer.Assign) && name != ""`.

`call()`의 인자 루프: 
```go
		if p.at(lexer.Name) && p.toks[p.i+1].Kind == lexer.Colon {
			nameTok := p.next()
			p.next()
			e, err := p.expr(0)
			if err != nil {
				return nil, err
			}
			node.Kids = append(node.Kids, tree.WithLit("named", nameTok.Pos, nameTok.Text, e))
			sawNamed = true
		} else {
			if sawNamed {
				return nil, p.unexpected()
			}
			arg, err := p.expr(0)
			...
		}
```
`p.toks[p.i+1]`은 `peek()`이 괄호 안 줄바꿈을 건너뛴 뒤의 인덱스이므로 바로 다음 토큰이 맞다.

- [ ] **Step 4**, **Step 5: 커밋** `feat(parser): parse functions, lambdas, return, named arguments, and type annotations`.

---

### Task 5: desugar — 통과

**Files:**
- Modify: `internal/desugar/desugar.go`
- Test: `internal/desugar/desugar_test.go`

- `(fn info defaults… (block …))` → `(fn info defaults'… (do …))` (자식 모두 desugar, Lit 유지). `(return e)` → `(return e')`. `(named e)` → `(named e')`.

- [ ] **Step 1: 테스트**
```go
func TestDesugarFunctions(t *testing.T) {
	cases := map[string]string{
		"fn add(a, b) = a + b":        "(do (fn add(a, b) (do (op + a b))))",
		"f = x => x":                  "(do (let f (fn (x) (do x))))",
		"fn g(a = 1 + 1) = return a":  "(do (fn g(a=?) (op + 1 1) (do (return a))))",
		"f(1, k: 2 * 3)":              "(do (call f 1 (named k (op * 2 3))))",
	}
	for src, want := range cases {
		if got := core(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}
```
- [ ] **Step 2**, **Step 3: 구현** — switch에 `case "fn", "return", "named": out := mapKids(n.Head, n); out.Lit = n.Lit`. (`mapKids`가 Lit을 복사하도록 바꾸면 더 간단하다: `out := tree.WithLit(head, n.Pos, n.Lit)`로.)
- [ ] **Step 4**, **Step 5: 커밋** `feat(desugar): pass functions, return, and named arguments through`.

---

### Task 6: resolve — 함수 스코프, 캡처, self, 끌어올리기, 상수

**Files:**
- Modify: `internal/resolve/resolve.go`, `internal/resolve/scope.go`
- Create: `internal/resolve/fn.go`
- Test: `internal/resolve/fn_test.go`

**Interfaces (코어 출력):**
- `(fn <FnInfo resolved> defaults… (do …))`: FnInfo 복사본에 `NumLocals`, `Captures`를 채운다. 인자는 슬롯 0..n-1.
- 이름 잎: `(local n)`, `(captured n)`, `(self)`, `(global n)`, `(prelude name)`.
- 최상위 `fn` 문장은 `(let (global n) (fn …))`으로 바뀌고 **루트 `do`의 맨 앞**으로 옮겨진다 (끌어올리기). 지역 `fn` 문장은 제자리에서 `(let (local n) (fn …))`.
- `Result.Globals`에 끌어올린 함수 이름 포함. 상수 여부는 resolve 내부에서만 쓴다.
- 규칙:
  - 함수 본문은 자기 스코프. 인자 → 슬롯. 기본값 식은 **앞의 인자가 묶인 뒤** 차례로 해석.
  - 이름 찾기: 현재 함수의 블록들 → `self` → 바깥 함수들(지역이면 캡처 사슬, 전역이면 `(global n)`) → prelude → (끝난 블록 이름이면 E0206) → (같은 블록의 뒤에 정의된 지역 fn이면 E0208) → E0204.
  - 대입: 현재 블록에 있으면 재대입. 바깥(블록·바깥 함수·전역·self)에 있으면 E0205. 끌어올린 fn 전역이면 E0209. 없으면 새 이름.
  - `return`이 함수 밖이면 E0211. `...rest`는 E0805. 모르는 타입 이름은 E0212 (대문자 태그 이름은 허용).
  - 최상위 `fn` 중복 → E0210. `closed`는 함수마다.

- [ ] **Step 1: 테스트**

```go
// internal/resolve/fn_test.go
package resolve

import (
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/tree"
)

func fnInfo(t *testing.T, core *tree.Node, path ...int) *tree.FnInfo {
	t.Helper()
	n := core
	for _, i := range path {
		n = n.Kids[i]
	}
	info, ok := n.Lit.(*tree.FnInfo)
	if !ok {
		t.Fatalf("node %s is not a fn", n)
	}
	return info
}

func TestTopLevelFnsAreHoistedGlobals(t *testing.T) {
	r, errs := res(t, "print(f(1))\nfn f(n) = g(n)\nfn g(n) = n + 1")
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	want := "(do (let (global 0) (fn f(n) (do (call (global 1) (local 0))))) (let (global 1) (fn g(n) (do (op + (local 0) 1)))) (call (prelude print) (call (global 0) 1)))"
	if got := r.Core.String(); got != want {
		t.Fatalf("\n got %s\nwant %s", got, want)
	}
	if r.Globals[0] != "f" || r.Globals[1] != "g" || r.NumLocals != 0 {
		t.Fatalf("globals=%v locals=%d", r.Globals, r.NumLocals)
	}
	if info := fnInfo(t, r.Core, 0, 1); info.NumLocals != 1 || len(info.Captures) != 0 {
		t.Fatalf("f info = %+v", info)
	}
}

func TestParamsLocalsAndDefaults(t *testing.T) {
	r, errs := res(t, "fn f(a, b = a + 1):\n  c = a * b\n  if c > 0: d = c ;;\n  c\n;;")
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	want := "(do (let (global 0) (fn f(a, b=?) (op + (local 0) 1) (do (let (local 2) (op * (local 0) (local 1))) (if (op > (local 2) 0) (do (let (local 3) (local 2))) (nil)) (local 2)))))"
	if got := r.Core.String(); got != want {
		t.Fatalf("\n got %s\nwant %s", got, want)
	}
	if info := fnInfo(t, r.Core, 0, 1); info.NumLocals != 4 {
		t.Fatalf("NumLocals = %d", info.NumLocals)
	}
}

func TestClosuresCaptureByChain(t *testing.T) {
	// y is a local of the outer fn; the middle lambda captures it from a
	// slot, the inner lambda captures it from the middle closure.
	r, errs := res(t, "fn outer(y) = x => z => x + y + z")
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	want := "(do (let (global 0) (fn outer(y) (do (fn (x) (do (fn (z) (do (op + (op + (captured 0) (captured 1)) (local 0))))))))))"
	if got := r.Core.String(); got != want {
		t.Fatalf("\n got %s\nwant %s", got, want)
	}
	middle := fnInfo(t, r.Core, 0, 1, 0, 0)
	if len(middle.Captures) != 1 || middle.Captures[0] != (tree.Capture{Kind: tree.CaptureLocal, Index: 0}) {
		t.Fatalf("middle captures = %+v", middle.Captures)
	}
	inner := fnInfo(t, r.Core, 0, 1, 0, 0, 0, 0)
	if len(inner.Captures) != 2 || inner.Captures[0] != (tree.Capture{Kind: tree.CaptureLocal, Index: 0}) || inner.Captures[1] != (tree.Capture{Kind: tree.CaptureCaptured, Index: 0}) {
		t.Fatalf("inner captures = %+v", inner.Captures)
	}
}

func TestGlobalsAreNotCaptured(t *testing.T) {
	r, errs := res(t, "x = 1\nfn f() = x\ng = () => x")
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	if got := r.Core.String(); !strings.Contains(got, "(fn f() (do (global 0)))") || !strings.Contains(got, "(fn () (do (global 0)))") {
		t.Fatalf("got %s", got)
	}
}

func TestLocalNamedFnUsesSelfAndRejectsLaterSiblings(t *testing.T) {
	r, errs := res(t, "fn outer():\n  fn loop(n) = n == 0 then 0 else loop(n - 1)\n  loop(3)\n;;")
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	if got := r.Core.String(); !strings.Contains(got, "(call (self) (op - (local 0) 1))") || !strings.Contains(got, "(call (local 0) 3)") {
		t.Fatalf("got %s", got)
	}
	// A lambda inside the named fn reaches self through a capture.
	r, _ = res(t, "fn outer():\n  fn f(n) = (() => f(n))()\n  f(1)\n;;")
	if got := r.Core.String(); !strings.Contains(got, "(fn () (do (call (captured 0) (captured 1))))") {
		t.Fatalf("got %s", got)
	}
	_, errs = res(t, "fn outer():\n  fn a() = b()\n  fn b() = 1\n  a()\n;;")
	if len(errs) != 1 || errs[0].Code != "E0208" || errs[0].Args[0] != "b" {
		t.Fatalf("errs = %v", errs)
	}
}

func TestFnScopeErrors(t *testing.T) {
	cases := map[string]string{
		"fn f() = 1\nfn f() = 2":           "E0210",
		"fn f() = 1\nf = 2":                "E0209",
		"x = 1\nfn f(): x = 2 ;;":          "E0205",
		"fn f(a): g = () => a; a = 2 ;;":   "", // same scope rebinding is fine
		"fn f(a) = (() => a = 1)()":        "E0110", // assignment is a statement
		"return 1":                         "E0211",
		"if true: return 1 ;;":             "E0211",
		"fn f(...xs) = xs":                 "E0805",
		"fn f(a: integer) = a":             "E0212",
		"fn f(a) -> strng = a":             "E0212",
		"fn f(a: Circle | int) = a":        "",
		"fn f() = y\ny = 1":                "", // globals are late-bound: resolve ok, runtime E0207
		"f = () => f()":                    "E0204", // only named fns see themselves
		"fn f():\n  if true: v = 1 ;;\n  v\n;;": "E0206",
		"fn f(): fn g() = 1 ;;\nfn h() = g()":   "E0204", // local fn of f is not visible in h
	}
	for src, want := range cases {
		_, errs := res(t, src)
		got := ""
		if len(errs) > 0 {
			got = errs[0].Code
		}
		if want == "" && len(errs) != 0 {
			t.Errorf("%q: unexpected %v", src, errs)
		} else if want != "" && got != want {
			t.Errorf("%q: got %s (%v), want %s", src, got, errs, want)
		}
	}
}

func TestPerFunctionClosedSets(t *testing.T) {
	// A name closed in f's block must not turn an unrelated typo in g into E0206.
	_, errs := res(t, "fn f(): if true: tmp = 1 ;; ;;\nfn g() = tmp")
	if len(errs) != 1 || errs[0].Code != "E0204" {
		t.Fatalf("errs = %v", errs)
	}
}
```
`"fn f(a) = (() => a = 1)()"`은 파서 단계에서 E0110이므로 `res`가 `t.Fatal`한다. **이 줄은 테이블에서 빼고** 파서 테스트에 있는 것으로 충분하다.

- [ ] **Step 2: 실패 확인**.

- [ ] **Step 3: 구현**

`scope.go`에 추가:
```go
// fnScope is one function being resolved: its block scopes, slot counter,
// the values it captures from enclosing functions, and its own name when
// it is a named local fn (reachable as (self)).
type fnScope struct {
	parent   *fnScope
	blocks   []*scope
	nextSlot int
	maxSlot  int
	captures []tree.Capture
	capIndex map[string]int
	self     string
	file     bool // the root pseudo-function whose first block holds globals
	closed   map[string]bool
	later    []map[string]bool // per block: fn names declared later in that block
}

// binding gains: constant (hoisted fn global, cannot be reassigned).
```
`binding`에 `constant bool` 추가.

`resolve.go`를 함수 단위로 다시 쓴다 (M2의 블록 로직을 `fnScope` 안으로 옮긴다):

```go
type resolver struct {
	prelude     map[string]bool
	preludeList []string
	fn          *fnScope // current function
	globals     []string
	errs        []*diag.Error
}

func Resolve(core *tree.Node, prelude []string) (Result, []*diag.Error) {
	r := &resolver{prelude: map[string]bool{}, preludeList: prelude}
	for _, p := range prelude {
		r.prelude[p] = true
	}
	root := r.newFn(nil, "", true)
	r.fn = root
	out := r.rootBlock(core)
	return Result{Core: out, NumLocals: root.maxSlot, Globals: r.globals}, r.errs
}

func (r *resolver) newFn(parent *fnScope, self string, file bool) *fnScope {
	return &fnScope{parent: parent, self: self, file: file, capIndex: map[string]int{}, closed: map[string]bool{}}
}

// rootBlock resolves the file scope: top-level fn statements are bound
// first as constant globals (03: 최상위 fn은 순서와 무관하게 서로 참조),
// then emitted first, then the remaining statements in order.
func (r *resolver) rootBlock(n *tree.Node) *tree.Node {
	sc := r.pushBlock(n, true)
	var fns, rest []*tree.Node
	for _, k := range n.Kids {
		if info, ok := fnStmt(k); ok {
			if _, dup := sc.names[info.Name]; dup {
				r.errs = append(r.errs, diag.Errorf("E0210", k.Pos, info.Name).Span(runeLen(info.Name)))
				continue
			}
			sc.names[info.Name] = binding{global: true, index: len(r.globals), constant: true}
			r.globals = append(r.globals, info.Name)
			fns = append(fns, k)
		} else {
			rest = append(rest, k)
		}
	}
	out := &tree.Node{Head: "do", Pos: n.Pos}
	for _, k := range fns {
		b := sc.names[k.Lit.(*tree.FnInfo).Name]
		out.Kids = append(out.Kids, tree.New("let", k.Pos, ref(k.Pos, b), r.function(k)))
	}
	for _, k := range rest {
		out.Kids = append(out.Kids, r.walk(k))
	}
	r.popBlock(true)
	return out
}

// fnStmt reports whether a node is a named fn definition statement.
func fnStmt(n *tree.Node) (*tree.FnInfo, bool) {
	if n.Head != "fn" {
		return nil, false
	}
	info := n.Lit.(*tree.FnInfo)
	return info, info.Name != ""
}

func (r *resolver) pushBlock(n *tree.Node, file bool) *scope {
	sc := &scope{names: map[string]binding{}, file: file, base: r.fn.nextSlot}
	r.fn.blocks = append(r.fn.blocks, sc)
	later := map[string]bool{}
	for _, k := range n.Kids {
		if info, ok := fnStmt(k); ok && !file {
			later[info.Name] = true
		}
	}
	r.fn.later = append(r.fn.later, later)
	return sc
}

func (r *resolver) popBlock(file bool) {
	f := r.fn
	sc := f.blocks[len(f.blocks)-1]
	f.blocks = f.blocks[:len(f.blocks)-1]
	f.later = f.later[:len(f.later)-1]
	if !file {
		for name := range sc.names {
			f.closed[name] = true
		}
		f.nextSlot = sc.base
	}
}

func (r *resolver) walk(n *tree.Node) *tree.Node {
	switch n.Head {
	case "do":
		return r.block(n)
	case "let":
		return r.let(n)
	case "name":
		return r.name(n)
	case "fn":
		if info, ok := fnStmt(n); ok {
			// A named fn inside a block: bind it as a local after resolving
			// its body (inside, the name is (self)).
			fnNode := r.function(n)
			b := r.bindNew(info.Name, n.Pos)
			if b == nil {
				return fnNode
			}
			return tree.New("let", n.Pos, ref(n.Pos, *b), fnNode)
		}
		return r.function(n)
	case "return":
		if r.fn.parent == nil {
			r.errs = append(r.errs, diag.Errorf("E0211", n.Pos))
		}
	}
	out := &tree.Node{Head: n.Head, Pos: n.Pos, Lit: n.Lit}
	for _, k := range n.Kids {
		out.Kids = append(out.Kids, r.walk(k))
	}
	return out
}

func (r *resolver) block(n *tree.Node) *tree.Node {
	r.pushBlock(n, false)
	out := &tree.Node{Head: "do", Pos: n.Pos}
	for _, k := range n.Kids {
		// Declare the later-fn set entry as bound once we pass its statement.
		if info, ok := fnStmt(k); ok {
			delete(r.fn.later[len(r.fn.later)-1], info.Name)
		}
		out.Kids = append(out.Kids, r.walk(k))
	}
	r.popBlock(false)
	return out
}
```
`later` 처리: 블록 진입 시 그 블록의 지역 fn 이름을 모아 두고, 해당 문장을 지나면 지운다. `name()`에서 못 찾았을 때 `later` 중 어느 블록에든 남아 있으면 E0208.

```go
// bindNew binds a fresh name in the current block (nil if the name
// belongs to an enclosing scope and the assignment is therefore E0205).
func (r *resolver) bindNew(name string, pos diag.Pos) *binding {
	f := r.fn
	cur := f.blocks[len(f.blocks)-1]
	if b, ok := cur.names[name]; ok {
		if b.constant {
			r.errs = append(r.errs, diag.Errorf("E0209", pos, name).Span(runeLen(name)))
			return nil
		}
		return &b
	}
	if r.definedOutside(name) {
		r.errs = append(r.errs, diag.Errorf("E0205", pos, name).Span(runeLen(name)))
		return nil
	}
	var b binding
	if cur.file {
		b = binding{global: true, index: len(r.globals)}
		r.globals = append(r.globals, name)
	} else {
		b = binding{index: f.nextSlot}
		f.nextSlot++
		if f.nextSlot > f.maxSlot {
			f.maxSlot = f.nextSlot
		}
	}
	cur.names[name] = b
	return &b
}

// definedOutside reports whether name is bound in an enclosing block of
// this function, in any enclosing function, or as a function's own name.
func (r *resolver) definedOutside(name string) bool {
	for f := r.fn; f != nil; f = f.parent {
		start := len(f.blocks) - 1
		if f == r.fn {
			start = len(f.blocks) - 2
		}
		for i := start; i >= 0; i-- {
			if _, ok := f.blocks[i].names[name]; ok {
				return true
			}
		}
		if f.self == name {
			return true
		}
	}
	return false
}

func (r *resolver) let(n *tree.Node) *tree.Node {
	target, rhs := n.Kids[0], r.walk(n.Kids[1])
	name := target.Lit.(string)
	b := r.bindNew(name, target.Pos)
	if b == nil {
		return tree.New("let", n.Pos, target, rhs)
	}
	return tree.New("let", n.Pos, ref(target.Pos, *b), rhs)
}
```
`bindNew`의 "현재 블록에 이미 있으면 재대입" 분기에서 `E0209`는 최상위 fn 이름(상수)에만 해당한다.

```go
// name resolves a read.
func (r *resolver) name(n *tree.Node) *tree.Node {
	name := n.Lit.(string)
	if node := r.lookup(name, r.fn, n.Pos); node != nil {
		return node
	}
	if r.prelude[name] {
		return tree.Leaf("prelude", n.Pos, name)
	}
	if r.fn.closed[name] {
		r.errs = append(r.errs, diag.Errorf("E0206", n.Pos, name).Span(runeLen(name)).WithHelp("assign the block's value instead: `"+name+" = if … ;;`"))
		return n
	}
	for _, later := range r.fn.later {
		if later[name] {
			r.errs = append(r.errs, diag.Errorf("E0208", n.Pos, name).Span(runeLen(name)))
			return n
		}
	}
	e := diag.Errorf("E0204", n.Pos, name).Span(runeLen(name))
	if s := similar(name, r.visible()); s != "" {
		e.WithHelp("a similar name exists: `" + s + "`")
	}
	r.errs = append(r.errs, e)
	return n
}

// lookup finds name in function f: its blocks, its own name, then the
// enclosing functions (captured through every function in between), or
// the file's globals (never captured: they are late-bound, 03).
func (r *resolver) lookup(name string, f *fnScope, pos diag.Pos) *tree.Node {
	for i := len(f.blocks) - 1; i >= 0; i-- {
		if b, ok := f.blocks[i].names[name]; ok {
			return ref(pos, b)
		}
	}
	if f.self == name {
		return tree.New("self", pos)
	}
	if f.parent == nil {
		return nil
	}
	cap, ok := r.captureFrom(name, f)
	if !ok {
		return nil
	}
	if cap.Kind == captureGlobal {
		return tree.Leaf("global", pos, int64(cap.Index))
	}
	return tree.Leaf("captured", pos, int64(cap.Index))
}

// captureGlobal marks "found as a file global": not a real capture.
const captureGlobal tree.CaptureKind = -1

// captureFrom makes name available in f by capturing it from f's parent,
// recursively. It returns the capture index in f, or the global index.
func (r *resolver) captureFrom(name string, f *fnScope) (tree.Capture, bool) {
	if idx, ok := f.capIndex[name]; ok {
		return tree.Capture{Kind: tree.CaptureCaptured, Index: idx}, true
	}
	p := f.parent
	var src tree.Capture
	found := false
	for i := len(p.blocks) - 1; i >= 0 && !found; i-- {
		if b, ok := p.blocks[i].names[name]; ok {
			if b.global {
				return tree.Capture{Kind: captureGlobal, Index: b.index}, true
			}
			src, found = tree.Capture{Kind: tree.CaptureLocal, Index: b.index}, true
		}
	}
	if !found && p.self == name {
		src, found = tree.Capture{Kind: tree.CaptureSelf}, true
	}
	if !found && p.parent != nil {
		c, ok := r.captureFrom(name, p)
		if !ok {
			return tree.Capture{}, false
		}
		if c.Kind == captureGlobal {
			return c, true
		}
		src, found = tree.Capture{Kind: tree.CaptureCaptured, Index: c.Index}, true
	}
	if !found {
		return tree.Capture{}, false
	}
	f.captures = append(f.captures, src)
	f.capIndex[name] = len(f.captures) - 1
	return tree.Capture{Kind: tree.CaptureCaptured, Index: len(f.captures) - 1}, true
}
```
`captureFrom`의 반환값에서 "f 안에서의 캡처 인덱스"와 "부모 쪽 소스"를 구분해야 한다. 위 코드는 f에 캡처를 추가하고 **f 안에서의 인덱스**(`CaptureCaptured`)를 돌려준다. `lookup`은 그것을 `(captured idx)`로 쓴다. 재귀 호출에서 돌아온 `c`는 부모 안에서의 인덱스이므로 `src = {CaptureCaptured, c.Index}`가 맞다.

```go
// internal/resolve/fn.go
package resolve

import (
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/tree"
)

var builtinTypes = map[string]bool{
	"int": true, "float": true, "num": true, "str": true, "bool": true, "nil": true, "sym": true,
	"dur": true, "time": true, "list": true, "map": true, "tuple": true, "range": true, "fn": true,
	"module": true, "task": true, "channel": true, "atom": true, "any": true,
}

// function resolves an (fn …) node in a fresh function scope: params are
// slots 0..n-1, each default sees the parameters before it, the body is
// the function's first block.
func (r *resolver) function(n *tree.Node) *tree.Node {
	decl := n.Lit.(*tree.FnInfo)
	info := &tree.FnInfo{Name: decl.Name, Params: append([]tree.Param(nil), decl.Params...), Return: decl.Return}
	f := r.newFn(r.fn, decl.Name, false)
	r.fn = f
	defer func() { r.fn = f.parent }()

	r.checkTypes(decl, n.Pos)
	body := n.Kids[len(n.Kids)-1]
	sc := r.pushBlock(body, false)
	var kids []*tree.Node
	di := 0
	for i, prm := range decl.Params {
		if prm.Rest {
			r.errs = append(r.errs, diag.Errorf("E0805", n.Pos, "variadic parameters (...rest) arrive with lists in M4"))
		}
		if prm.HasDefault {
			kids = append(kids, r.walk(n.Kids[di]))
			di++
		}
		sc.names[prm.Name] = binding{index: i}
	}
	f.nextSlot, f.maxSlot = len(decl.Params), len(decl.Params)
	out := &tree.Node{Head: "do", Pos: body.Pos}
	for _, k := range body.Kids {
		if fi, ok := fnStmt(k); ok {
			delete(f.later[len(f.later)-1], fi.Name)
		}
		out.Kids = append(out.Kids, r.walk(k))
	}
	r.popBlock(false)
	info.NumLocals = f.maxSlot
	info.Captures = f.captures
	return tree.WithLit("fn", n.Pos, info, append(kids, out)...)
}

func (r *resolver) checkTypes(decl *tree.FnInfo, pos diag.Pos) {
	check := func(alts []string) {
		for _, a := range alts {
			if !builtinTypes[a] && !isTagName(a) {
				r.errs = append(r.errs, diag.Errorf("E0212", pos, a))
			}
		}
	}
	for _, p := range decl.Params {
		check(p.Type)
	}
	check(decl.Return)
}

func isTagName(s string) bool {
	for _, r := range s {
		return r >= 'A' && r <= 'Z' || (r > 127 && unicode.IsUpper(r))
	}
	return false
}
```
(`unicode` import.) 주의 두 가지. 첫째, 함수 본문 블록은 `pushBlock(body)`로 **인자와 같은 블록**에 둔다 (03: 인자는 함수 본문 스코프의 이름). 둘째, 기본값 식은 인자 `i`를 묶기 **전에** 해석하므로 `fn f(a, b = a)`에서 `b`의 기본값은 `a`만 본다. 기본값 안에서 아직 안 묶인 인자를 쓰면 E0204.

`visible()`은 현재 함수의 블록 이름 + 바깥 함수들의 블록 이름 + `closed` + prelude를 모은다 (M2 검토 지적 반영: 끝난 블록 이름도 힌트 후보).

- [ ] **Step 4: 통과 확인** — `go test ./...` 전부 PASS (M2 테스트의 전역/슬롯 기대값이 그대로 나와야 한다).
- [ ] **Step 5: 커밋** `feat(resolve): add function scopes, captures, self, hoisting, and function errors`.

---

### Task 7: bytecode — 클로저, Proto 확장, 새 명령

**Files:**
- Create: `internal/bytecode/closure.go`
- Modify: `internal/bytecode/op.go`, `proto.go`, `disasm.go`
- Test: `internal/bytecode/bytecode_test.go`

**Interfaces:**
```go
// 새 명령 (기존 뒤에 추가)
OpClosure      // -- closure            (arg = index into Protos)
OpLoadCaptured // -- closure.Captured[arg]
OpLoadSelf     // -- the running closure
OpTailCall     // like CALL, reusing the frame
OpJumpIfSet    // v -- ; if v is assigned (not Go nil) ip = arg
OpCheckType    // -- ; slot arg&0xFFF must match Types[arg>>12]  (E0403)
OpCheckReturn  // v -- v ; v must match Types[arg]                (E0404)

// CALL / TAILCALL operand packing: positional count in the low 12 bits,
// named count in the next 12.
func EncodeCall(positional, named int) int
func DecodeCall(arg int) (positional, named int)

type Param struct { Name string; HasDefault bool; Type int } // Type: index into Types, -1 none

type Proto struct {
	Name        string
	Code        []Instr
	Consts      []value.Value
	Pos         []diag.Pos
	NumLocals   int
	GlobalNames []string
	Params      []Param
	Protos      []*Proto       // child functions
	Captures    []tree.Capture // what CLOSURE copies from the creating frame
	Types       [][]string     // type specs referenced by CHECKTYPE/CHECKRETURN
	ReturnType  int            // index into Types, -1 none
}
func (p *Proto) AddType(alts []string) int

// Closure is a function value: a proto plus the values it captured.
type Closure struct { Proto *Proto; Captured []value.Value }
func (*Closure) TypeName() string { return "fn" }
func (c *Closure) FnName() string  { return c.Proto.Name }
```
- `HasArg`: 새 명령 모두 true (`OpLoadSelf`는 false).
- 역어셈블러: `CALL`/`TAILCALL`은 `; 2 positional, 1 named` 주석; `CLOSURE n`은 `; <child name>`; `CHECKTYPE`은 `; a: int | nil`; 자식 Proto들을 뒤에 재귀 출력 (`== add ==`).

- [ ] **Step 1: 테스트**
```go
func TestCallEncoding(t *testing.T) {
	for _, c := range [][2]int{{0, 0}, {3, 0}, {2, 1}, {4095, 4095}} {
		p, n := DecodeCall(EncodeCall(c[0], c[1]))
		if p != c[0] || n != c[1] {
			t.Errorf("EncodeCall(%d, %d) → (%d, %d)", c[0], c[1], p, n)
		}
	}
}

func TestClosureIsAFnValue(t *testing.T) {
	c := &Closure{Proto: &Proto{Name: "add"}}
	var v value.Value = c
	if v.TypeName() != "fn" || value.Format(v) != "<fn add>" || !value.Equal(v, v) || value.Equal(v, &Closure{Proto: c.Proto}) {
		t.Fatalf("closure value behaviour: %s", value.Format(v))
	}
	if value.Format(&Closure{Proto: &Proto{}}) != "<fn>" {
		t.Error("anonymous")
	}
}

func TestDisassembleFunctions(t *testing.T) {
	child := &Proto{Name: "add", NumLocals: 2, Params: []Param{{Name: "a", Type: 0}, {Name: "b", Type: -1}}, Types: [][]string{{"int", "nil"}}, ReturnType: -1}
	pos := diag.Pos{Line: 1, Col: 1}
	child.Emit(OpCheckType, 0|0<<12, pos)
	child.Emit(OpLoadLocal, 0, pos)
	child.Emit(OpReturn, 0, pos)
	p := &Proto{Name: "<main>", Protos: []*Proto{child}, ReturnType: -1}
	p.Emit(OpClosure, 0, pos)
	p.Emit(OpConst, p.Const(value.Int(1)), pos)
	p.Emit(OpCall, EncodeCall(1, 0), pos)
	p.Emit(OpTailCall, EncodeCall(0, 2), pos)
	p.Emit(OpLoadSelf, 0, pos)
	want := "== <main> ==\n" +
		"0000  1:1   CLOSURE      0    ; add\n" +
		"0001  1:1   CONST        0    ; 1\n" +
		"0002  1:1   CALL         1    ; 1 positional, 0 named\n" +
		"0003  1:1   TAILCALL     " + strconv.Itoa(EncodeCall(0, 2)) + "    ; 0 positional, 2 named\n" +
		"0004  1:1   LOADSELF\n" +
		"\n== add ==\n" +
		"0000  1:1   CHECKTYPE    0    ; a: int | nil\n" +
		"0001  1:1   LOADLOCAL    0\n" +
		"0002  1:1   RETURN\n"
	if got := Disassemble(p); got != want {
		t.Fatalf("got:\n%s\nwant:\n%s", got, want)
	}
}
```
(`strconv` import.)
- [ ] **Step 2**, **Step 3: 구현** — 위 인터페이스대로. `Disassemble`은 내부 `disasm(p, &b)`로 나누고, 자식마다 빈 줄 하나 뒤 `== name ==`. 이름이 비면 `<lambda>`.
- [ ] **Step 4**, **Step 5: 커밋** `feat(bytecode): add closures, function protos, and call instructions`.

---

### Task 8: compiler — 함수, 서두, 꼬리 호출, 클로저

**Files:**
- Modify: `internal/compiler/compiler.go`
- Create: `internal/compiler/fn.go`
- Test: `internal/compiler/fn_test.go`

**Interfaces:**
- `expr(n, tail bool)`: 꼬리 위치 전파. `block`의 마지막 식, `if`의 두 가지, `return`의 피연산자(반환 타입이 없을 때)는 꼬리; `op`·`concat`·`call` 인자·`let`의 오른쪽은 꼬리 아님. 꼬리 위치의 `call`은 `TAILCALL`.
- 함수 Proto: `Name`, `NumLocals`, `Params`(Type 인덱스), `Captures`, `Types`, `ReturnType`. 서두: 기본값 있는 인자마다 `LOADLOCAL i; JUMPIFSET L; <기본값>; STORELOCAL i; L:`, 그다음 타입 표기 있는 인자마다 `CHECKTYPE i|t<<12`. 본문 `block(body, tail = ReturnType < 0)`; 반환 타입이 있으면 `CHECKRETURN t`; `RETURN`.
- `(return e)`: `expr(e, tail = ReturnType < 0)`; 반환 타입이 있으면 `CHECKRETURN`; `RETURN`.
- `(fn …)` 식: 자식 Proto를 컴파일해 `Protos`에 넣고 `CLOSURE idx`. `(captured n)` → `LOADCAPTURED n`, `(self)` → `LOADSELF`.
- `(call f pos… (named k e)…)`: f, 위치 인자, 그리고 이름 인자마다 `CONST 'k` + 값; `CALL EncodeCall(pos, named)`.
- `<main>`은 꼬리 위치를 쓰지 않는다 (tail=false).

- [ ] **Step 1: 테스트**
```go
// internal/compiler/fn_test.go
package compiler

import (
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/value"
)

func TestCompileFunctionProtoAndClosure(t *testing.T) {
	p := compile(t, "fn add(a, b) = a + b\nprint(add(1, 2))")
	if got, want := ops(p), "CLOSURE(0) STOREGLOBAL(0) CONST(0) LOADGLOBAL(0) CONST(1) CONST(2) CALL(2) CALL(1) RETURN"; got != want {
		t.Fatalf("main:\n got %s\nwant %s", got, want)
	}
	add := p.Protos[0]
	if add.Name != "add" || add.NumLocals != 2 || len(add.Params) != 2 || add.ReturnType != -1 {
		t.Fatalf("add proto = %+v", add)
	}
	if got, want := ops(add), "LOADLOCAL(0) LOADLOCAL(1) ADD RETURN"; got != want {
		t.Fatalf("add:\n got %s\nwant %s", got, want)
	}
}

func TestCompileDefaultsAndTypeChecks(t *testing.T) {
	p := compile(t, "fn f(a: int, b = a * 2) -> num = a + b")
	f := p.Protos[0]
	want := "LOADLOCAL(1) JUMPIFSET(5) LOADLOCAL(0) CONST(0) MUL STORELOCAL(1) CHECKTYPE(0) LOADLOCAL(0) LOADLOCAL(1) ADD CHECKRETURN(1) RETURN"
	if got := ops(f); got != want {
		t.Fatalf("\n got %s\nwant %s", got, want)
	}
	if f.Types[0][0] != "int" || f.Types[1][0] != "num" || f.ReturnType != 1 || f.Params[0].Type != 0 || f.Params[1].Type != -1 {
		t.Fatalf("types = %v return = %d params = %+v", f.Types, f.ReturnType, f.Params)
	}
}

func TestCompileTailCalls(t *testing.T) {
	p := compile(t, "fn count(n, acc) = n == 0 then acc else count(n - 1, acc + 1)")
	if got := ops(p.Protos[0]); !strings.Contains(got, "TAILCALL(2)") || strings.Contains(got, " CALL(") {
		t.Fatalf("expected a tail call: %s", got)
	}
	// Not tail: operand of +, argument of a call, with a return type.
	for _, src := range []string{"fn f(n) = 1 + f(n)", "fn f(n) = print(f(n))", "fn f(n) -> int = f(n)"} {
		p := compile(t, src)
		if got := ops(p.Protos[0]); strings.Contains(got, "TAILCALL") {
			t.Errorf("%q compiled a tail call: %s", src, got)
		}
	}
	// return of a call is a tail call; the branches of if are tail positions.
	p = compile(t, "fn f(n):\n  if n > 0: return f(n - 1) ;;\n  if n < 0: f(0) else: 0 ;;\n;;")
	if got := ops(p.Protos[0]); strings.Count(got, "TAILCALL(1)") != 2 {
		t.Fatalf("got %s", got)
	}
}

func TestCompileCapturesSelfAndNamedArgs(t *testing.T) {
	p := compile(t, "fn outer(y):\n  fn loop(n) = n == 0 then y else loop(n - 1)\n  loop(2)\n;;\nouter(1)")
	outer := p.Protos[0]
	loop := outer.Protos[0]
	if len(loop.Captures) != 1 || loop.Captures[0].Index != 0 {
		t.Fatalf("loop captures = %+v", loop.Captures)
	}
	if got := ops(loop); !strings.Contains(got, "LOADCAPTURED(0)") || !strings.Contains(got, "LOADSELF") {
		t.Fatalf("loop = %s", got)
	}
	p = compile(t, "fn f(a, b = 2) = a\nf(1, b: 3)")
	if got := ops(p); !strings.Contains(got, "CONST(2) CALL("+itoa(bytecode.EncodeCall(1, 1))+")") || p.Consts[2] != value.Intern("b") {
		t.Fatalf("named call: %s consts %v", got, p.Consts)
	}
}
```
(`itoa` = `strconv.Itoa`를 쓴다.)
- [ ] **Step 2**, **Step 3: 구현** — `compiler`에 `proto`, `prelude`, `hasRet bool` 필드. `expr(n, tail)`로 시그니처 변경 (호출부 전부). `fn.go`에 `function(n) int`와 `prologue`, `callExpr`.
- [ ] **Step 4**, **Step 5: 커밋** `feat(compiler): compile functions, prologues, tail calls, and closures`.

---

### Task 9: vm·native — 프레임, 호출, 클로저, 한도, 호출 경로, 재진입, `at_exit`

**Files:**
- Modify: `internal/vm/vm.go`, `internal/native/prelude.go`
- Create: `internal/vm/call.go`
- Test: `internal/vm/fn_test.go`, `internal/native/prelude_test.go`

**Interfaces:**
- `frame{proto, ip, base, closure *bytecode.Closure, tailCalls int}`.
- `run(stopDepth int)`: `RETURN`으로 프레임 수가 `stopDepth`가 되면 값을 돌려준다. `Run`은 `run(0)`.
- `Task.Call(fn, args)`: 네이티브면 직접, 클로저면 프레임을 밀고 `run(len(frames)-1)`. `Task.AtExit(fn)`: `Runtime.exitHandlers`에 추가. `Runtime.RunExitHandlers() []*diag.Error`: 역순으로 새 `Task`에서 실행, 에러는 모아 반환.
- 호출 바인딩 `bindArgs(p *Proto, pos []Value, named []namedArg) ([]Value, *diag.Error)`: E0402/E0405/E0406/E0407. 네이티브에 이름 인자 → E0406.
- `maxFrames = 100000` → E0804 (힌트는 카탈로그).
- 모든 런타임 에러에 `WithCallPath` (안쪽부터; `<main>`은 그대로 두고 Format이 `<entry>`로; 이름 없는 함수는 `<lambda>`; `Elided = tailCalls`).
- `CHECKTYPE` E0403 인자: 인자 이름, `TypeText`, 실제 타입. `CHECKRETURN` E0404.
- native: `at_exit(f)`: `f`가 `value.Fn`이 아니면 E0403("f", "fn", 타입).

- [ ] **Step 1: 테스트**

```go
// internal/vm/fn_test.go
package vm

import (
	"bytes"
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/native"
	"github.com/tunarider/fung/internal/value"
)

func TestFunctionsRun(t *testing.T) {
	cases := map[string]string{
		"fn add(a, b) = a + b\nprint(add(1, 2))":                                   "3\n",
		"fn fib(n) = n < 2 then n else fib(n - 1) + fib(n - 2)\nprint(fib(20))":    "6765\n",
		"print(even(10), odd(10))\nfn even(n) = n == 0 then true else odd(n - 1)\nfn odd(n) = n == 0 then false else even(n - 1)": "true false\n",
		"f = x => x * 2\nprint(f(21))":                                             "42\n",
		"print((fn(a):\n  a + 1\n;;)(1))":                                           "2\n",
		"fn f():\n  x = 1\n  y = 2\n  x + y\n;;\nprint(f())":                        "3\n",
		"fn f() = return 1\nprint(f())":                                             "1\n",
		"fn f(n):\n  if n > 0: return \"pos\" ;;\n  \"other\"\n;;\nprint(f(1), f(0))": "pos other\n",
		"fn f() = return\nprint(f())":                                               "nil\n",
		"fn f(): x = 1 ;;\nprint(f())":                                              "nil\n",
		"fn add(a, b) = a + b\nprint(add, x => x)":                                  "<fn add> <fn>\n",
	}
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
}

func TestClosuresCaptureByValue(t *testing.T) {
	cases := map[string]string{
		"x = 1\nf = () => x\nx = 2\nprint(f())":                                                  "2\n", // x is a global: late-bound
		"fn mk():\n  x = 1\n  f = () => x\n  x = 2\n  f()\n;;\nprint(mk())":                       "1\n", // a local: copied when f was made
		"fn adder(n) = x => x + n\nadd5 = adder(5)\nprint(add5(1), adder(10)(1))":                 "6 11\n",
		"fn outer(a) = b => c => a + b + c\nprint(outer(1)(2)(3))":                               "6\n",
		"fn outer():\n  fn loop(n, acc) = n == 0 then acc else loop(n - 1, acc + n)\n  loop(4, 0)\n;;\nprint(outer())": "10\n",
		"fn outer(y):\n  fn f(n) = (() => f(n - 1))\n  f(y)()(0)\n;;\nprint(outer(3))":           "", // see below
	}
	delete(cases, "fn outer(y):\n  fn f(n) = (() => f(n - 1))\n  f(y)()(0)\n;;\nprint(outer(3))")
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
}

func TestArgumentsDefaultsAndNames(t *testing.T) {
	cases := map[string]string{
		"fn f(a, b = a * 2) = a + b\nprint(f(1), f(1, 5))":                       "3 6\n",
		"fn f(a, b = 1, c = 2) = a + b * 10 + c * 100\nprint(f(1, c: 3), f(c: 3, a: 2))": "211 213\n",
		"fn f(greeting = \"hi\", name = \"you\") = greeting + \", \" + name\nprint(f(name: \"fung\"))": "hi, fung\n",
		"f = (a, b) => a - b\nprint(f(b: 1, a: 3))":                              "2\n",
		"calls = 0\nfn f(a = print(\"eval\")) = a\nf()\nf(1)":                     "eval\n", // default evaluated only when needed, per call
	}
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
}

func TestTailCallsDoNotGrowTheStack(t *testing.T) {
	_, out, code := run(t, "fn count(n, acc) = n == 0 then acc else count(n - 1, acc + 1)\nprint(count(1000000, 0))")
	if code != "" || out != "1000000\n" {
		t.Fatalf("out=%q code=%s", out, code)
	}
	_, out, code = run(t, "fn even(n) = n == 0 then true else odd(n - 1)\nfn odd(n) = n == 0 then false else even(n - 1)\nprint(even(300001))")
	if code != "" || out != "false\n" {
		t.Fatalf("mutual: out=%q code=%s", out, code)
	}
}

func TestNonTailRecursionHitsTheFrameLimit(t *testing.T) {
	_, _, code := run(t, "fn f(n) = n == 0 then 0 else 1 + f(n - 1)\nprint(f(50000))")
	if code != "" {
		t.Fatalf("50000 deep: %s", code)
	}
	_, _, code = run(t, "fn f(n) = n == 0 then 0 else 1 + f(n - 1)\nprint(f(200000))")
	if code != "E0804" {
		t.Fatalf("200000 deep: %s, want E0804", code)
	}
	// A return type annotation disables TCO (03), so this one overflows too.
	_, _, code = run(t, "fn count(n, acc) -> int = n == 0 then acc else count(n - 1, acc + 1)\nprint(count(200000, 0))")
	if code != "E0804" {
		t.Fatalf("annotated: %s, want E0804", code)
	}
}

func TestTypeAnnotationsAreChecked(t *testing.T) {
	cases := map[string]string{
		"fn f(n: int, s: str = \"x\") -> str = s\nprint(f(1), f(2, \"y\"))":  "x y\n",
		"fn f(n: num) = n\nprint(f(1), f(1.5))":                              "1 1.5\n",
		"fn f(v: str | nil) = v\nprint(f(nil), f(\"a\"))":                    "nil a\n",
		"fn f(v: any) = v\nprint(f('s))":                                     "'s\n",
	}
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
	errs := map[string]string{
		"fn f(n: int) = n\nf(\"a\")":              "E0403",
		"fn f(n: int, s: str = 1) = n\nf(1)":      "E0403", // default is checked too
		"fn f() -> str = 1\nf()":                  "E0404",
		"fn f(a, b) = a\nf(1, 2, 3)":              "E0402",
		"fn f(a) = a\nf(1, a: 2)":                 "E0405",
		"fn f(a) = a\nf(b: 1)":                    "E0406",
		"fn f(a, b) = a\nf(1)":                    "E0407",
		"print(x: 1)":                             "E0406",
		"fn f() = x\nf()\nx = 1":                  "E0207",
		"f = 1\nf()":                              "E0401",
	}
	for src, want := range errs {
		if _, _, code := run(t, src); code != want {
			t.Errorf("%q: code = %s, want %s", src, code, want)
		}
	}
}

func TestRuntimeErrorsCarryTheCallPath(t *testing.T) {
	p := compileWith(t, "fn f(x) = x / 0\nfn g() = f(1)\ng()", native.Prelude())
	_, err := (&Runtime{Stdout: &bytes.Buffer{}}).Run(p)
	if err == nil || err.Code != "E0502" {
		t.Fatalf("err = %v", err)
	}
	names := []string{}
	for _, c := range err.CallPath {
		names = append(names, c.Fn)
	}
	if got := strings.Join(names, " "); got != "f g <main>" {
		t.Fatalf("call path = %v", err.CallPath)
	}
	if err.CallPath[0].Line != 1 || err.CallPath[1].Line != 2 || err.CallPath[2].Line != 3 {
		t.Fatalf("lines = %+v", err.CallPath)
	}
	p = compileWith(t, "fn loop(n) = n == 0 then 1 / 0 else loop(n - 1)\nloop(5)", native.Prelude())
	_, err = (&Runtime{Stdout: &bytes.Buffer{}}).Run(p)
	if err == nil || err.CallPath[0].Elided != 5 {
		t.Fatalf("elided = %+v", err.CallPath)
	}
}

func TestAtExitRunsHandlersInReverseAfterTheProgram(t *testing.T) {
	p := compileWith(t, "at_exit(() => print(\"first registered\"))\nat_exit(() => print(\"second registered\"))\nprint(\"body\")", native.Prelude())
	var out bytes.Buffer
	rt := &Runtime{Stdout: &out}
	if _, err := rt.Run(p); err != nil {
		t.Fatal(err)
	}
	if errs := rt.RunExitHandlers(); len(errs) != 0 {
		t.Fatal(errs)
	}
	if out.String() != "body\nsecond registered\nfirst registered\n" {
		t.Fatalf("out = %q", out.String())
	}
}

func TestAtExitHandlerErrorsAreCollectedAndOthersStillRun(t *testing.T) {
	p := compileWith(t, "at_exit(() => print(\"ok\"))\nat_exit(() => fail(\"boom\"))", native.Prelude())
	var out bytes.Buffer
	rt := &Runtime{Stdout: &out}
	rt.Run(p)
	errs := rt.RunExitHandlers()
	if len(errs) != 1 || errs[0].Code != "E0803" || out.String() != "ok\n" {
		t.Fatalf("errs = %v out = %q", errs, out.String())
	}
	if _, _, code := run(t, "at_exit(1)"); code != "E0403" {
		t.Fatalf("at_exit(1) = %s", code)
	}
}
```
`TestClosuresCaptureByValue`의 마지막 케이스(삭제 줄)는 지저분하다. 실제 파일에서는 그 항목과 `delete` 줄을 쓰지 않는다.

- [ ] **Step 2: 실패 확인**.

- [ ] **Step 3: 구현** (`call.go`)

```go
package vm

import (
	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

// maxFrames bounds non-tail recursion (09: 깊이 한도 100,000).
const maxFrames = 100000

type namedArg struct {
	name  value.Sym
	value value.Value
}

// bindArgs lays out a call's arguments in a fresh slot array for proto p
// (09. 호출 규약): positionals in order, named by lookup; defaults are
// filled in by the prologue, so a slot left as Go nil means "use the default".
func bindArgs(p *bytecode.Proto, positional []value.Value, named []namedArg) ([]value.Value, *diag.Error) {
	name := p.Name
	if name == "" {
		name = "the function"
	}
	if len(positional) > len(p.Params) {
		return nil, diag.Errorf("E0402", diag.Pos{}, name, len(p.Params), len(positional))
	}
	slots := make([]value.Value, p.NumLocals)
	copy(slots, positional)
	for _, na := range named {
		idx := -1
		for i, prm := range p.Params {
			if prm.Name == na.name.Name() {
				idx = i
				break
			}
		}
		if idx < 0 {
			return nil, diag.Errorf("E0406", diag.Pos{}, name, na.name.Name())
		}
		if slots[idx] != nil {
			return nil, diag.Errorf("E0405", diag.Pos{}, na.name.Name())
		}
		slots[idx] = na.value
	}
	for i, prm := range p.Params {
		if slots[i] == nil && !prm.HasDefault {
			return nil, diag.Errorf("E0407", diag.Pos{}, prm.Name)
		}
	}
	return slots, nil
}

// splitCall pops a call's operands off the stack: the callee, positional
// arguments, and (name, value) pairs for named arguments.
func (t *Task) splitCall(arg int) (value.Value, []value.Value, []namedArg) {
	npos, nnamed := bytecode.DecodeCall(arg)
	top := len(t.stack)
	namedStart := top - 2*nnamed
	named := make([]namedArg, nnamed)
	for i := range named {
		named[i] = namedArg{name: t.stack[namedStart+2*i].(value.Sym), value: t.stack[namedStart+2*i+1]}
	}
	posStart := namedStart - npos
	positional := make([]value.Value, npos)
	copy(positional, t.stack[posStart:namedStart])
	callee := t.stack[posStart-1]
	t.stack = t.stack[:posStart-1]
	return callee, positional, named
}

// enter starts running callee. A native runs immediately and its result
// is returned with done=true; a closure gets a new frame (or reuses the
// current one when tail is set) and done=false.
func (t *Task) enter(callee value.Value, positional []value.Value, named []namedArg, pos diag.Pos, tail bool) (result value.Value, done bool, err *diag.Error) {
	switch fn := callee.(type) {
	case *value.Native:
		if len(named) > 0 {
			return nil, true, diag.Errorf("E0406", pos, fn.Name, named[0].name.Name())
		}
		r, e := fn.Fn(t, positional)
		if e != nil {
			if d, ok := e.(*diag.Error); ok {
				return nil, true, d.WithPos(pos)
			}
			return nil, true, diag.Errorf("E0801", pos, e)
		}
		return r, true, nil
	case *bytecode.Closure:
		slots, e := bindArgs(fn.Proto, positional, named)
		if e != nil {
			return nil, true, e.WithPos(pos)
		}
		if tail {
			f := &t.frames[len(t.frames)-1]
			t.stack = append(t.stack[:f.base], slots...)
			f.proto, f.closure, f.ip = fn.Proto, fn, 0
			f.tailCalls++
			return nil, false, nil
		}
		if len(t.frames) >= maxFrames {
			return nil, true, diag.Errorf("E0804", pos, maxFrames)
		}
		t.frames = append(t.frames, frame{proto: fn.Proto, closure: fn, base: len(t.stack)})
		t.stack = append(t.stack, slots...)
		return nil, false, nil
	}
	return nil, true, diag.Errorf("E0401", pos, callee.TypeName())
}

// Call runs a fung function from Go (natives such as at_exit handlers and,
// later, map/filter). It returns when that function's frame returns.
func (t *Task) Call(fn value.Value, args []value.Value) (value.Value, error) {
	depth := len(t.frames)
	r, done, err := t.enter(fn, args, nil, diag.Pos{}, false)
	if err != nil {
		return nil, err
	}
	if done {
		return r, nil
	}
	v, err := t.run(depth)
	if err != nil {
		return nil, err
	}
	return v, nil
}

// AtExit registers an exit handler (03).
func (t *Task) AtExit(fn value.Value) { t.rt.exitHandlers = append(t.rt.exitHandlers, fn) }

// RunExitHandlers runs the at_exit handlers in reverse order on fresh
// tasks and returns every error they raised.
func (rt *Runtime) RunExitHandlers() []*diag.Error {
	var errs []*diag.Error
	for i := len(rt.exitHandlers) - 1; i >= 0; i-- {
		t := &Task{rt: rt}
		if _, err := t.Call(rt.exitHandlers[i], nil); err != nil {
			errs = append(errs, err.(*diag.Error))
		}
	}
	rt.exitHandlers = nil
	return errs
}

// callPath lists the frames innermost first for error reports (08).
func (t *Task) callPath() []diag.CallSite {
	path := make([]diag.CallSite, 0, len(t.frames))
	for i := len(t.frames) - 1; i >= 0; i-- {
		f := t.frames[i]
		pos := diag.Pos{}
		if f.ip-1 >= 0 && f.ip-1 < len(f.proto.Pos) {
			pos = f.proto.Pos[f.ip-1]
		}
		name := f.proto.Name
		if name == "" {
			name = "<lambda>"
		}
		path = append(path, diag.CallSite{File: pos.File, Line: pos.Line, Fn: name, Elided: f.tailCalls})
	}
	return path
}
```
`Call` 안에서 `t.enter`가 Go 에러(`*diag.Error`)를 `error`로 돌려주는 타입 변환에 주의: `enter`는 `*diag.Error`를 반환하므로 `if err != nil { return nil, err }`는 nil 인터페이스 함정이 있다 — `*diag.Error`가 nil이어도 `error`로 변환되면 non-nil이 된다. **반드시** `if err != nil` 검사를 `*diag.Error` 타입에서 하고 그 뒤에 변환한다 (위 코드가 그렇게 되어 있다).

`vm.go`의 변경:
- `Runtime`에 `exitHandlers []value.Value`.
- `frame`에 `closure *bytecode.Closure`, `tailCalls int`.
- `Run`: `t.run(0)`. `run(stopDepth)`은 `loop(stopDepth)`를 감싸 에러에 `WithCallPath(t.callPath())`.
- `loop`의 switch에 새 명령:
```go
		case bytecode.OpCall, bytecode.OpTailCall:
			callee, positional, named := t.splitCall(in.Arg())
			r, done, err := t.enter(callee, positional, named, pos, op == bytecode.OpTailCall)
			if err != nil {
				return nil, err
			}
			if done {
				t.push(r)
				if op == bytecode.OpTailCall {
					if v, stop := t.ret(stopDepth); stop {
						return v, nil
					}
				}
			}
		case bytecode.OpReturn:
			if v, stop := t.ret(stopDepth); stop {
				return v, nil
			}
		case bytecode.OpClosure:
			child := f.proto.Protos[in.Arg()]
			captured := make([]value.Value, len(child.Captures))
			for i, c := range child.Captures {
				switch c.Kind {
				case tree.CaptureLocal:
					captured[i] = t.stack[f.base+c.Index]
				case tree.CaptureCaptured:
					captured[i] = f.closure.Captured[c.Index]
				case tree.CaptureSelf:
					captured[i] = f.closure
				}
			}
			t.push(&bytecode.Closure{Proto: child, Captured: captured})
		case bytecode.OpLoadCaptured:
			t.push(f.closure.Captured[in.Arg()])
		case bytecode.OpLoadSelf:
			t.push(f.closure)
		case bytecode.OpJumpIfSet:
			if t.pop() != nil {
				f.ip = in.Arg()
			}
		case bytecode.OpCheckType:
			slot, ti := in.Arg()&0xFFF, in.Arg()>>12
			v := t.stack[f.base+slot]
			if !value.MatchesType(v, f.proto.Types[ti]) {
				return nil, diag.Errorf("E0403", pos, f.proto.Params[slot].Name, value.TypeText(f.proto.Types[ti]), v.TypeName())
			}
		case bytecode.OpCheckReturn:
			v := t.stack[len(t.stack)-1]
			if !value.MatchesType(v, f.proto.Types[in.Arg()]) {
				return nil, diag.Errorf("E0404", pos, value.TypeText(f.proto.Types[in.Arg()]), v.TypeName())
			}
```
```go
// ret pops the current frame, leaving its result on the caller's stack.
// It reports stop=true when the frame count has dropped to stopDepth.
func (t *Task) ret(stopDepth int) (value.Value, bool) {
	v := t.pop()
	f := t.frames[len(t.frames)-1]
	t.stack = t.stack[:f.base]
	t.frames = t.frames[:len(t.frames)-1]
	if len(t.frames) == stopDepth {
		return v, true
	}
	t.push(v)
	return nil, false
}
```
주의: `loop`의 각 반복은 `f := &t.frames[len(t.frames)-1]`로 **현재 프레임을 다시 읽는다** (호출·반환으로 프레임이 바뀌므로). `enter`가 `t.frames`에 append하면 기존 `f` 포인터는 무효가 될 수 있으므로, 명령 처리 뒤에는 `f`를 다시 쓰지 않는다.

`native/prelude.go`에 `at_exit`:
```go
func atExitFn(ctx value.Context, args []value.Value) (value.Value, error) {
	if len(args) != 1 {
		return nil, diag.Errorf("E0402", diag.Pos{}, "at_exit", 1, len(args))
	}
	if _, ok := args[0].(value.Fn); !ok {
		return nil, diag.Errorf("E0403", diag.Pos{}, "f", "fn", args[0].TypeName())
	}
	ctx.AtExit(args[0])
	return value.Nil{}, nil
}
```
- [ ] **Step 4**, **Step 5: 커밋** `feat(vm): add frames, calls, closures, tail calls, call paths, and at_exit`.

---

### Task 10: cmd — `at_exit` 실행, 골든 테스트

**Files:**
- Modify: `cmd/fung/pipeline.go`, `cmd/fung/main_test.go`
- Create: `tests/lang/*.fung`

- `execute`: `Run` 뒤 `rt.RunExitHandlers()`; 핸들러 에러는 모두 출력; 종료 코드는 `Run` 실패면 1, 아니면 핸들러 에러가 있으면 1, 없으면 0.

- [ ] **Step 1: 테스트** (`main_test.go` 추가; Task 1의 `TestRuntimeErrorFromVMAlwaysExitsOne`도 여기서 통과)
```go
func TestCallPathIsPrinted(t *testing.T) {
	_, stderr, code := invoke(t, "-e", "fn f(x) = x / 0\nfn g() = f(1)\ng()")
	want := " call path (innermost first):\n   -e:1       f\n   -e:2       g\n   -e:3       <entry>\n"
	if code != exitRuntimeError || !strings.Contains(stderr, want) {
		t.Fatalf("code=%d stderr=%q", code, stderr)
	}
}

func TestAtExitRunsAndFailuresSetExitCode(t *testing.T) {
	stdout, stderr, code := invoke(t, "-e", "at_exit(() => print(\"bye\"))\nprint(\"hi\")")
	if code != exitOK || stdout != "hi\nbye\n" || stderr != "" {
		t.Fatalf("code=%d stdout=%q stderr=%q", code, stdout, stderr)
	}
	stdout, stderr, code = invoke(t, "-e", "at_exit(() => print(\"cleanup\"))\nat_exit(() => fail(\"boom\"))\nprint(1 / 0)")
	if code != exitRuntimeError || stdout != "cleanup\n" || strings.Count(stderr, "error[") != 2 {
		t.Fatalf("code=%d stdout=%q stderr=%q", code, stdout, stderr)
	}
}
```
골든 (`tests/lang/`):
```
# fn_basics.fung
fn add(a, b) = a + b
fn fib(n) = n < 2 then n else fib(n - 1) + fib(n - 2)
print(add(1, 2), fib(20))
# expect: 3 6765
print(even(10), odd(7))
# expect: true true
fn even(n) = n == 0 then true else odd(n - 1)
fn odd(n) = n == 0 then false else even(n - 1)
fn stats(xs_len):
  avg = xs_len / 2
  fn half(v) = v / 2
  half(avg)
;;
print(stats(8))
# expect: 2.0
print((fn(a):
  a + 1
;;)(1), (x => x * 2)(21))
# expect: 2 42
print(add, x => x)
# expect: <fn add> <fn>
```
```
# closures.fung
fn mk():
  x = 1
  f = () => x
  x = 2
  f()
;;
print(mk())
# expect: 1
fn adder(n) = x => x + n
add5 = adder(5)
print(add5(1), adder(10)(1))
# expect: 6 11
fn outer(a) = b => c => a + b + c
print(outer(1)(2)(3))
# expect: 6
fn sum_to(n):
  fn loop(i, acc) = i == 0 then acc else loop(i - 1, acc + i)
  loop(n, 0)
;;
print(sum_to(100))
# expect: 5050
g = 1
fn read_g() = g
g = 2
print(read_g())
# expect: 2
```
```
# args.fung
fn f(a, b = a * 2) = a + b
print(f(1), f(1, 5), f(b: 1, a: 1))
# expect: 3 6 2
fn greet(greeting = "hi", name = "you") = greeting + ", " + name
print(greet(), greet(name: "fung"), greet("yo"))
# expect: hi, you hi, fung yo, you
fn log(a = print("default evaluated")) = a
log(1)
log()
# expect: default evaluated
```
```
# return.fung
fn classify(n):
  if n > 0: return "pos" ;;
  if n < 0: return "neg" ;;
  "zero"
;;
print(classify(1), classify(-1), classify(0))
# expect: pos neg zero
fn nothing() = return
print(nothing())
# expect: nil
```
```
# tco.fung
fn count(n, acc) = n == 0 then acc else count(n - 1, acc + 1)
print(count(1000000, 0))
# expect: 1000000
fn even(n) = n == 0 then true else odd(n - 1)
fn odd(n) = n == 0 then false else even(n - 1)
print(even(300000))
# expect: true
```
```
# types.fung
fn f(n: int, s: str = "x") -> str = s
fn g(n: num) = n
fn h(v: str | nil) = v
fn k(v: any) = v
print(f(1), f(2, "y"), g(1), g(1.5), h(nil), h("a"), k('s))
# expect: x y 1 1.5 nil a 's
```
```
# at_exit.fung
at_exit(() => print("first"))
at_exit(() => print("second"))
print("body")
# expect: body
# expect: second
# expect: first
```
에러 골든:

| 파일 | 내용 | expect |
|------|------|------|
| `err_too_many_args.fung` | `fn f(a) = a` / `f(1, 2)` | E0402 |
| `err_arg_type.fung` | `fn f(n: int) = n` / `f("a")` | E0403 |
| `err_return_type.fung` | `fn f() -> str = 1` / `f()` | E0404 |
| `err_arg_twice.fung` | `fn f(a) = a` / `f(1, a: 2)` | E0405 |
| `err_unknown_named.fung` | `fn f(a) = a` / `f(b: 1)` | E0406 |
| `err_missing_arg.fung` | `fn f(a, b) = a` / `f(1)` | E0407 |
| `err_stack_overflow.fung` | `fn f(n) = n == 0 then 0 else 1 + f(n - 1)` / `print(f(200000))` | E0804 |
| `err_local_fn_later.fung` | `fn outer():` / `  fn a() = b()` / `  fn b() = 1` / `  a()` / `;;` | E0208 |
| `err_reassign_fn.fung` | `fn f() = 1` / `f = 2` | E0209 |
| `err_dup_fn.fung` | `fn f() = 1` / `fn f() = 2` | E0210 |
| `err_top_return.fung` | `return 1` | E0211 |
| `err_unknown_type.fung` | `fn f(a: integer) = a` | E0212 |
| `err_rest.fung` | `fn f(...xs) = xs` | E0805 |
| `err_global_from_fn.fung` | `x = 1` / `fn f(): x = 2 ;;` | E0205 |
| `err_global_before_assign.fung` | `fn f() = x` / `f()` / `x = 1` | E0207 |

- [ ] **Step 2: 실패 확인**, **Step 3: 구현**:
```go
	rt := &vm.Runtime{Stdout: stdout}
	code := exitOK
	if _, err := rt.Run(b.Proto); err != nil {
		reportErrors([]*diag.Error{err}, src, stderr)
		code = exitRuntimeError
	}
	if errs := rt.RunExitHandlers(); len(errs) > 0 {
		reportErrors(errs, src, stderr)
		code = exitRuntimeError
	}
	return code
```
- [ ] **Step 4: 통과 확인** — `go test ./...` 전부, 골든 33 + 21 = 54. 바이너리: `fung -e 'fn count(n, acc) = n == 0 then acc else count(n - 1, acc + 1)
print(count(1000000, 0))'` → `1000000`.
- [ ] **Step 5: 커밋** `feat(cli): run at_exit handlers; add M3 golden tests`.

---

### Task 11: 문서

**Files:**
- Create: `docs/notes/M3-functions.md`
- Modify: `docs/design/09-implementation-plan.md`, `docs/design/03-functions-and-patterns.md`, `CLAUDE.md`

- [ ] **Step 1: 09·03 갱신** — 09의 M3 행에서 "기본값·이름·가변 인자"를 "기본값·이름 인자"로 바꾸고 M4 행에 "가변 인자 `...rest`"를 추가; M3 행 끝 ` — 완료`; M5 행에서 "호출 경로 출력" 삭제(M3로 옮김). 03의 "자원 정리" 절 `at_exit` 설명에 "등록의 역순으로 실행된다"를 추가.
- [ ] **Step 2: 노트** — 절 구성: (1) 한눈에: `fn add` 한 줄이 `CLOSURE`·자식 Proto·`CALL`로 되는 과정과 `dump bytecode`의 `== add ==` 절; (2) 프레임과 슬롯: 호출 시 스택 모양 그림(표), `base`, `RETURN`이 스택을 자르는 방식; (3) 호출 규약: `CALL argc|named<<12`, 바인딩 순서, 기본값이 서두에 컴파일되는 이유(`JUMPIFSET`과 Go nil); (4) 클로저: 값 복사가 가능한 이유(변수 불변), 캡처 사슬(`local`→`captured`→`captured`), `LOADSELF`가 필요한 이유(슬롯이 아직 비어 있음), 전역은 캡처하지 않고 호출 시점에 읽음; (5) 끌어올리기와 상수; (6) 꼬리 호출: 꼬리 위치 전파, `TAILCALL`이 프레임을 재사용하는 방법, 반환 타입이 TCO를 막는 이유, 호출 경로의 "elided"; (7) 스택 한도와 E0804, VM 루프가 반복문이라 Go 스택이 안 자라는 점; (8) 타입 검사 시점; (9) `at_exit`와 재진입 `Call`(네이티브가 fung 함수를 부르는 길, M4 `map`의 토대); (10) 테스트와 다음 M4.
- [ ] **Step 3: CLAUDE.md** — M3 완료 줄, 다음 M4(리스트·맵·튜플·태그·범위, 패턴, `match`, `! ? ??`, `|>`, `_`, `for`, `in`, `type` 별칭, 가변 인자).
- [ ] **Step 4: 커밋** `docs: add M3 learning notes and update status`.
