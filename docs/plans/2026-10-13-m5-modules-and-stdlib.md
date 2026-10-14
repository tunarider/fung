# M5 모듈과 실사용 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `import`(표준 라이브러리 이름·경로 문자열·`as`·이름 목록), 모듈 값, 모듈별 전역, 숨은 원시 모듈 층(`_native.*`), 실행 파일에 내장된 fung 표준 라이브러리(`fs` `path`, prelude 일부), 내장 함수의 호출 규약 통일(이름 인자·기본값·`...rest`), prelude 확장(컬렉션·문자열·변환·결과값·입출력·프로세스), `exec` `sh` `env` `args` `exit`, 파일 핸들과 `with_open`, 그리고 실제 스크립트 `examples/rename.fung`을 만든다.

**Architecture:** 진입 파일을 컴파일할 때 **로더**(`internal/frontend/load.go`)가 import 그래프 전체를 DFS로 읽고 컴파일한다 (순환·비공개·최상위 제한·없는 이름은 컴파일 에러). 모듈 하나는 Proto 하나이고 **자기 전역 표**를 가진다: 프레임과 클로저가 전역 표를 들고 다니므로 다른 모듈에서 불려도 자기 모듈을 본다. `IMPORT k` 명령은 런타임 모듈 표에서 모듈 값을 꺼내고, 처음이면 그 모듈의 최상위를 현재 태스크에서 한 번 실행해 공개 이름(`_`로 시작하지 않는 전역)으로 **불변 모듈 값**을 만든다. `fs.read`는 모듈 값에 대한 보통 `FIELD`다. 내장 함수는 매개변수 목록을 선언하고 VM의 `bindArgs` **하나**가 fung 함수와 똑같이 인자를 배치한다. `exit`는 VM 안에서 종료 신호(에러와 같은 경로)로 올라가 `fung` 명령이 `at_exit` 뒤에 종료 코드로 바꾼다. fung으로 쓴 표준 라이브러리는 `embed.FS`에 담겨 import 시 컴파일되고, prelude의 fung 부분은 시작 시 컴파일된다.

**Tech Stack:** Go 1.27, 표준 라이브러리만 (`embed`, `os/exec`, `bufio`, `path/filepath`).

**Spec:** `04-modules.md` (import 형태, 공개/비공개, 모듈은 값, 최상위 제한, 순환, 적재 시점과 전역, 가리기), `06-go-interop-and-stdlib.md` (2층 구조, 호출 규약, `exit` 신호, 에러 규칙과 `kind` 어휘, prelude 목록, `fs`·`path`, 파일 핸들, 명령 실행), `02-values-and-types.md` (파일 값, 맵 중복 키, 비교 표), `03-functions-and-patterns.md` (`with_open`, 패턴 중복 이름, `_ = 식`), `08-tooling-and-errors.md` (명령, 종료 코드), `09-implementation-plan.md` (Runtime, embed).

## Global Constraints

- Go 모듈 `github.com/tunarider/fung`, `go 1.27`, **Go 표준 라이브러리만**.
- 코드·주석·에러 메시지·커밋 메시지는 영어. 문서는 한국어.
- Conventional Commits. 커밋 전 `gofmt -l .`(출력 없음), `go vet ./...`, `go test ./...` 통과.
- 커밋 메시지 끝에 `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Go 패닉·스택 추적은 사용자에게 보이지 않는다. 명령마다 소스 위치. 종료 코드 0/1/2/3, `exit(n)`은 n.
- **에러 번호는 한 번 붙이면 바꾸지 않는다.** E0207·E0510·E0805는 쓰지 않는다 (E0805는 번호만 남음).
- 렉서는 닫혀 있다 (`import` `as` 토큰은 이미 있다). 불변 자료구조는 단순 복사.
- 스코프 모델 C (03). 모듈 이름은 가릴 수 없다 (04). prelude 이름은 가릴 수 있다 (04).
- 06의 에러 규칙: 외부 요인 실패는 `Ok`/`Err`(내용은 `{kind, message, …}` 맵), 코드 오류는 런타임 에러.

## 계획 수준의 결정

1. **내장 함수의 매개변수 선언.** `value.Native{Name, Params []NativeParam, Fn}`. `Params`가 nil이면 "매개변수 없음"이다. `bindArgs`가 fung 함수와 내장 함수를 같은 규칙으로 배치하고, 내장 함수의 기본값은 상수 값이라 `bindArgs`가 바로 채운다 (fung 함수는 서두가 채운다). `print(...args)`처럼 가변 인자는 `Rest`로 선언하고 Go 함수는 `args[0].(*List)`로 받는다. 기존 내장 함수의 손 개수 검사는 모두 지운다.
2. **모듈 값은 `*value.Module`**(이름, 경로/키, 공개 멤버의 순서 있는 표). `type_of` → `'module`, 출력 `<module fs>`, `==`는 동일성. `FIELD`는 `value.Index`의 `*Module` 분기로 처리하고 새 명령을 두지 않는다. 비공개(`_x`) 접근은 E0304, 없는 멤버는 E0303 (정적으로 알면 컴파일, 아니면 런타임, 같은 번호).
3. **모듈 키.** 사용자 파일은 정리된 절대 경로, 표준 라이브러리는 이름(`fs`), 원시 모듈은 `_native.fs`. resolve는 로더가 준 `ModuleInfo{Key, Exports}`로 import 문의 spec을 키로 바꿔 `(import key …)`를 내고, 컴파일러는 `IMPORT k`(k = 키 문자열 상수)를 낸다. 런타임 모듈 표도 같은 키를 쓴다.
4. **import는 최상위 문장**(E0311). 묶이는 이름(모듈 이름·별칭·목록의 이름)은 **상수 전역**이고 `binding.module`로 표시한다. 같은 모듈 이름을 다시 묶거나(E0309) 모듈 이름을 어떤 방식으로든 가리면(E0308) 컴파일 에러. 목록으로 가져온 이름은 가릴 수 있다(보통 이름이 된다, 04는 모듈 이름만 막는다).
5. **import된 파일의 최상위 제한**(04)은 resolve가 `Library: true`일 때 검사한다: 허용은 `import`, `fn`, `type`, 대입(구조 분해 포함)뿐, 그 외 문장은 E0306. (`test` 블록은 M6.)
6. **모듈 최상위 실행은 `IMPORT`가 처음 실행될 때** 현재 태스크 위에서 중첩 프레임으로 돈다 (`Task.Call`과 같은 방식). 끝나면 전역 표에서 `_`·`$`로 시작하지 않는 이름을 순서대로 모듈 값에 담는다. 모듈의 최상위 코드는 한 번만 돌고 그 뒤엔 아무도 그 전역에 대입하지 않으므로(모델 C) 스냅샷이 최종값이다.
7. **`exit(n)`은 `diag.Error{Code: "exit"}`**(카탈로그 밖의 고정 코드 `diag.CodeExit`)로 전파된다. 호출 경로·메시지 없이 프레임을 되감고, `fung` 명령이 `at_exit`를 돌린 뒤 n으로 끝낸다. 골든은 `# expect-exit: n`으로 적는다.
8. **표준 입력은 Runtime이 `*bufio.Reader` 하나로** 감싸 둔다 (`read_line`을 여러 번 불러도 버퍼가 이어진다). `Context`에 `Stdin()`·`Stderr()`를 더한다.
9. **prelude의 fung 부분(`until`, `iterate`)**은 `internal/stdlib/prelude.fung`이고, `fung` 명령이 시작할 때 모듈로 적재·실행해 그 공개 멤버를 prelude 표에 합친 뒤 진입 파일을 컴파일한다. `retry`는 `sleep`이 생기는 M7.
10. **`fs`·`path` 모듈은 fung 파일**(`internal/stdlib/fs.fung`, `path.fung`)이 `_native.fs`·`_native.path`를 `import … as _n`으로 가져와 `read = _n.read`처럼 다시 내보내고, `with_open`·`with_temp_dir`는 fung으로 쓴다. `_native.*` import는 내장 파일에서만 허용(E0310).
11. **컬렉션 함수가 받는 "순회 대상"은 리스트와 범위**(02). 문자열·맵은 `chars`·`pairs`·`keys`·`values`로 바꾼다. 그 외 타입은 E0506 `cannot <verb> <type>`.
12. **`take`·`drop`의 n은 길이로 잘라 준다**(안전한 버전, 02의 엄격한 슬라이스와 대비). 음수 n은 0으로 본다.
13. **`min`·`max`·`first`·`last`·`find`는 없으면 `nil`**(02: nil은 "없음"). `sum`의 빈 리스트는 `0`. `all`의 빈 리스트는 `true`.
14. **`join(xs, sep = "")`은 원소를 `print`처럼 문자열로 바꿔** 잇는다 (문자열은 그대로, 나머지는 리터럴 형태). `group_by`의 키는 문자열이어야 한다(E0514).
15. **맵 리터럴의 중복 키는 파서가 E0113**, 패턴 안 중복 이름은 resolve가 E0221, `_ = 식`은 파서가 `(assign (pwild) 식)`으로 만들어 값을 버린다.
16. **골든 실행기 확장:** `# args: a b`(공백으로 나눈 인자), `# stdin:` 줄(각각 표준 입력 한 줄), `# expect-exit: n`. 이름이 `_`로 시작하는 디렉터리(`tests/lang/_lib/`)는 골든으로 돌리지 않고 모듈 fixture로 쓴다.
17. **`fung check`는 M6**에 그대로 둔다 (로더가 생겨 쉬워졌지만 범위 밖).

## Review Focus

1. 두 모듈이 같은 이름의 최상위 `fn`을 가질 때 각자 자기 모듈의 것을 부른다 (Task 5 테스트: `a.fung`·`b.fung` 모두 `fn helper()`를 두고 `b.run()`이 `a.run()`을 부름).
2. 모듈 A를 두 파일이 import해도 최상위가 **한 번만** 실행된다 (Task 5: 모듈 최상위의 `x = print("init")`는 허용된 구멍이므로 이것으로 횟수를 센다).
3. 순환 import의 에러 경로 메시지가 전체 경로(`a → b → a`)를 보여 주고, 자기 자신 import도 순환이다 (Task 4·5).
4. 내장 함수 기본값: `exec(["true"], cwd: nil)`처럼 기본값과 같은 값을 명시해도 되고, 이름 인자를 두 번 주면 E0405, rest 함수에 이름 인자를 주면 E0406 (Task 2).
5. `read_line()`이 표준 입력 끝에서 `Ok(nil)`, 닫힌 핸들에서 `Err({kind: 'closed})`를 돌려주고, `with_open` 안에서 `?`로 나가도 핸들이 닫힌다 (Task 8·9: 나간 뒤 `read_line(h)`가 `'closed`).

## 파일 구조

| 패키지 | 변경 | 작업 |
|------|------|------|
| `internal/value`, `internal/diag`, `internal/bytecode` | `NativeParam`, `Module`, `File`, `Exit`, 새 번호, `OpImport`, `Closure.Globals` | Task 1 |
| `internal/vm`, `internal/native` | 내장 호출 규약, `Stdin`/`Stderr`, 종료 신호, 기존 내장 함수 선언 | Task 2 |
| `internal/parser` | `import` 문, 맵 중복 키, `_ = 식` | Task 3 |
| `internal/desugar`, `internal/resolve`, `internal/compiler` | import 묶기·정적 검사·최상위 제한·가리기, 패턴 중복 이름, `IMPORT` | Task 4 |
| `internal/frontend`, `internal/vm`, `internal/stdlib` | 로더(그래프·순환·embed·원시 모듈), 모듈별 전역, `IMPORT` 실행, 모듈 값 생성 | Task 5 |
| `cmd/fung`, `internal/golden`, `tests` | 인자·표준 입력·종료 코드·여러 파일 에러 출력, 골든 지시어, fixture 디렉터리 | Task 6 |
| `internal/native` | 컬렉션 함수 | Task 7 |
| `internal/native` | 문자열·변환·결과값·입출력·프로세스 | Task 8 |
| `internal/native`, `internal/stdlib` | `_native.fs`, `_native.path`, `fs.fung`, `path.fung`, `prelude.fung` | Task 9 |
| `examples`, `tests/lang`, `docs` | `rename.fung`, 골든, 노트, 상태 | Task 10 |

### M5에서 할당하는 에러 번호

| 번호 | 메시지 | 비고 |
|------|------|------|
| E0113 | `` duplicate key `%s` in map literal `` | 파서 |
| E0221 | `` name `%s` is bound twice in one pattern `` | 힌트: `` use a guard to compare the two positions: `[a, b] when a == b` `` |
| E0301 | `` cannot find module `%s` `` | 힌트: `` standard library modules are imported by name (`import fs`); your own files by path (`import "./lib/x"`) `` |
| E0302 | `circular import: %s` | `%s`는 `a → b → a` |
| E0303 | `` module `%s` has no member `%s` `` | |
| E0304 | `` `%s` is private to module `%s` `` | 힌트: `` names starting with `_` are not exported `` |
| E0305 | `` `%s` is not a valid module name; use `as` `` | 파일명이 식별자가 아닐 때 |
| E0306 | `` only import, fn, type, and assignment are allowed at the top level of an imported module `` | 힌트: `` move the statement into a function, or into the entry file `` |
| E0307 | `cannot import the entry file` | |
| E0308 | `` cannot shadow module `%s` `` | 힌트: `` import it under another name: `import %s as m` `` |
| E0309 | `` module `%s` is imported twice `` | |
| E0310 | `` `%s` is internal; import `%s` instead `` | `_native.fs` → `fs` |
| E0311 | `import must be at the top level` | |
| E0513 | `cannot convert %s to %s` | `int("x")` |
| E0514 | `cannot use a value of type %s as a map key` | `group_by` |

`diag.CodeExit = "exit"`는 번호가 아니다 (카탈로그 밖, `Error()`는 `exit(%d)`).

---

### Task 1: value·diag·bytecode — 내장 매개변수, 모듈·파일 값, 종료 신호, 번호, `IMPORT`

**Files:**
- Modify: `internal/value/value.go`, `internal/value/format.go`, `internal/value/compare.go`, `internal/value/index.go`, `internal/diag/catalog.go`, `internal/diag/error.go`, `internal/bytecode/op.go`, `internal/bytecode/closure.go`, `internal/bytecode/disasm.go`
- Create: `internal/value/module.go`
- Test: `internal/value/module_test.go`, `internal/diag/error_test.go`(추가), `internal/bytecode/m5_test.go`

**Interfaces:**
- Produces:
  ```go
  // value
  type NativeParam struct { Name string; Default Value; Rest bool }   // Default nil = required
  type Native struct { Name string; Params []NativeParam; Fn func(ctx Context, args []Value) (Value, error) }
  type Context interface { Stdout() io.Writer; Stderr() io.Writer; Stdin() *bufio.Reader; Call(fn Value, args []Value) (Value, error); AtExit(fn Value) }
  type Module struct{ Name, Key string; … }
  func NewModule(name, key string) *Module
  func (m *Module) Set(name string, v Value); func (m *Module) Get(name string) (Value, bool); func (m *Module) Names() []string
  type File struct { Path string; Reader *bufio.Reader; Writer *bufio.Writer; Closer io.Closer; Closed bool }
  // Index(*Module, Str): 비공개 → E0304, 없음 → E0303
  // diag
  const CodeExit = "exit"
  func ExitSignal(code int) *Error        // Code: CodeExit, Args: [code]
  func (e *Error) ExitCode() (int, bool)
  // bytecode
  OpImport   // -- module (arg = const index of the Str key)
  type Closure struct { Proto *Proto; Captured []value.Value; Globals []value.Value }
  ```

- [ ] **Step 1: 테스트**

```go
// internal/value/module_test.go
package value

import "testing"

func TestModuleValue(t *testing.T) {
	m := NewModule("fs", "fs")
	m.Set("read", Int(1))
	m.Set("write", Int(2))
	if m.TypeName() != "module" || Format(m) != "<module fs>" {
		t.Fatalf("type %s format %s", m.TypeName(), Format(m))
	}
	if got := m.Names(); len(got) != 2 || got[0] != "read" || got[1] != "write" {
		t.Fatalf("names = %v", got)
	}
	if v, err := Index(m, Str("read")); err != nil || v != Int(1) {
		t.Fatalf("read = %v %v", v, err)
	}
	if _, err := Index(m, Str("nope")); err == nil || err.Code != "E0303" {
		t.Fatalf("missing: %v", err)
	}
	if _, err := Index(m, Str("_hidden")); err == nil || err.Code != "E0304" {
		t.Fatalf("private: %v", err)
	}
	if _, err := Index(m, Int(0)); err == nil || err.Code != "E0501" {
		t.Fatalf("int key: %v", err)
	}
	other := NewModule("fs", "fs")
	if !Equal(m, m) || Equal(m, other) {
		t.Fatal("modules compare by identity")
	}
	if _, err := Compare(m, other); err == nil || err.Code != "E0501" {
		t.Fatal("modules are not ordered")
	}
}

func TestFileValue(t *testing.T) {
	f := &File{Path: "/tmp/x"}
	if f.TypeName() != "file" || Format(f) != `<file "/tmp/x">` {
		t.Fatalf("type %s format %s", f.TypeName(), Format(f))
	}
	if !Equal(f, f) || Equal(f, &File{Path: "/tmp/x"}) {
		t.Fatal("files compare by identity")
	}
}
```

```go
// internal/diag/error_test.go (추가)
func TestExitSignal(t *testing.T) {
	e := ExitSignal(3)
	if code, ok := e.ExitCode(); !ok || code != 3 {
		t.Fatalf("ExitCode = %d %v", code, ok)
	}
	if e.Error() != "exit(3)" {
		t.Fatalf("Error = %q", e.Error())
	}
	if _, ok := Errorf("E0801", Pos{}, "x").ExitCode(); ok {
		t.Fatal("ordinary errors are not exits")
	}
	for _, code := range []string{"E0113", "E0221", "E0301", "E0302", "E0303", "E0304", "E0305", "E0306", "E0307", "E0308", "E0309", "E0310", "E0311", "E0513", "E0514"} {
		if Message(code, "a", "b") == "" || strings.HasPrefix(Message(code, "a", "b"), "unknown error") {
			t.Errorf("%s missing from the catalog", code)
		}
	}
}
```

```go
// internal/bytecode/m5_test.go
package bytecode

import "testing"

func TestImportOpcode(t *testing.T) {
	if !OpImport.HasArg() || OpImport.String() != "IMPORT" {
		t.Fatalf("IMPORT: hasArg=%v name=%s", OpImport.HasArg(), OpImport.String())
	}
	in := Encode(OpImport, 7)
	if in.Op() != OpImport || in.Arg() != 7 {
		t.Fatal("round trip")
	}
}
```

- [ ] **Step 2: 실패 확인** — `go test ./internal/value/ ./internal/diag/ ./internal/bytecode/` → 컴파일 실패(`NewModule`, `File`, `ExitSignal`, `OpImport` 없음).

- [ ] **Step 3: 구현**

```go
// internal/value/value.go — 바뀌는 부분
import ("bufio"; "io")

// Context is what a native function may ask the VM for.
type Context interface {
	Stdout() io.Writer
	Stderr() io.Writer
	Stdin() *bufio.Reader                        // shared across calls so read_line continues where it left off
	Call(fn Value, args []Value) (Value, error) // re-enter the VM (map, at_exit, …)
	AtExit(fn Value)                            // register an exit handler
}

// NativeParam declares one parameter of a native function (06: 내장
// 함수도 fung 함수와 같은 호출 규약). A nil Default means required.
type NativeParam struct {
	Name    string
	Default Value
	Rest    bool
}

// Native is a function implemented in Go. The VM lays out its arguments
// exactly as for a fung function and passes one slot per parameter.
type Native struct {
	Name   string
	Params []NativeParam
	Fn     func(ctx Context, args []Value) (Value, error)
}
```

```go
// internal/value/module.go
package value

import (
	"bufio"
	"io"
)

// Module is an immutable record of a module's public names (04: 모듈은
// 값). Members keep export order for listings and dumps.
type Module struct {
	Name    string // what the user wrote or the file stem: fs, rename
	Key     string // runtime key: stdlib name, _native.x, or the absolute file path
	names   []string
	members map[string]Value
}

func NewModule(name, key string) *Module {
	return &Module{Name: name, Key: key, members: map[string]Value{}}
}

func (m *Module) Set(name string, v Value) {
	if _, ok := m.members[name]; !ok {
		m.names = append(m.names, name)
	}
	m.members[name] = v
}

func (m *Module) Get(name string) (Value, bool) { v, ok := m.members[name]; return v, ok }
func (m *Module) Names() []string             { return m.names }
func (*Module) TypeName() string              { return "module" }

// File is an open file handle (02, 06): a stateful resource. Reader is
// set for 'read, Writer for 'write/'append; Closed makes every operation
// Err({kind: 'closed}).
type File struct {
	Path   string
	Reader *bufio.Reader
	Writer *bufio.Writer
	Closer io.Closer
	Closed bool
}

func (*File) TypeName() string { return "file" }
```

format.go의 `Format`에 두 분기: `case *Module: return "<module " + x.Name + ">"`, `case *File: return "<file " + strconv.Quote(x.Path) + ">"`.
compare.go의 `Equal`에 `case *Module: y, ok := b.(*Module); return ok && x == y`, `case *File: …` 같은 꼴. `Compare`는 기본 분기가 E0501이면 그대로.
index.go의 `Index`에:

```go
	case *Module:
		k, ok := i.(Str)
		if !ok {
			return nil, diag.Errorf("E0501", diag.Pos{}, "index", v.TypeName(), i.TypeName())
		}
		if strings.HasPrefix(string(k), "_") {
			return nil, diag.Errorf("E0304", diag.Pos{}, string(k), x.Name)
		}
		member, found := x.Get(string(k))
		if !found {
			return nil, diag.Errorf("E0303", diag.Pos{}, x.Name, string(k))
		}
		return member, nil
```

diag/error.go:

```go
// CodeExit marks the exit signal that exit(n) raises (06): it unwinds
// like an error but is not one, so it has no catalog entry, no excerpt,
// and no call path in reports.
const CodeExit = "exit"

func ExitSignal(code int) *Error { return &Error{Code: CodeExit, Args: []any{code}} }

// ExitCode reports the code when the error is an exit signal.
func (e *Error) ExitCode() (int, bool) {
	if e.Code != CodeExit {
		return 0, false
	}
	return e.Args[0].(int), true
}
```
`Error()`는 `CodeExit`이면 `fmt.Sprintf("exit(%d)", e.Args[0])`.

catalog.go: 위 표의 번호·메시지·힌트를 추가 (`// E03xx modules` 묶음).

bytecode: `OpImport` 상수(마지막에), `HasArg` 목록, 이름 표 `OpImport: "IMPORT"`, disasm의 주석에 `; key`를 상수로 출력. `Closure`에 `Globals []value.Value` 필드와 주석 (04: 함수 값은 자기 모듈의 전역 표를 들고 다닌다).

- [ ] **Step 4: 통과 확인** — `go test ./internal/value/ ./internal/diag/ ./internal/bytecode/` PASS. `go build ./...`는 `Context` 변경 때문에 `native`·`vm`가 깨진다 → Task 2에서 고친다. 이 작업의 커밋은 **빌드가 깨진 상태**이면 안 되므로 Step 3에서 `vm.Task`에 `Stderr()`·`Stdin()`를 임시로 추가한다 (`Stderr`는 `t.rt.Stderr`가 nil이면 `io.Discard`, `Stdin`은 nil이면 빈 reader). native 테스트의 fake ctx에도 두 메서드를 더한다.

- [ ] **Step 5: 커밋** `feat(value): add module and file values, native parameter specs, exit signal, and the IMPORT opcode`

---

### Task 2: vm·native — 내장 호출 규약, 표준 입출력, 종료 신호

**Files:**
- Modify: `internal/vm/call.go`, `internal/vm/vm.go`, `internal/native/prelude.go`, `internal/native/prelude_test.go`
- Test: `internal/vm/native_test.go`

**Interfaces:**
- Consumes: `value.NativeParam`, `diag.ExitSignal` (Task 1).
- Produces:
  ```go
  // vm
  type Runtime struct { Stdout, Stderr io.Writer; Stdin io.Reader; … }     // nil Stderr → io.Discard, nil Stdin → empty
  func bindArgs(name string, params []argSpec, numSlots int, positional []value.Value, named []namedArg) ([]value.Value, *diag.Error)
  // native
  func param(name string) value.NativeParam                 // required
  func opt(name string, def value.Value) value.NativeParam  // with default
  func rest(name string) value.NativeParam
  func def(name string, fn func(value.Context, []value.Value) (value.Value, error), params ...value.NativeParam) *value.Native
  ```

- [ ] **Step 1: 테스트**

```go
// internal/vm/native_test.go
package vm

import (
	"bytes"
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/native"
	"github.com/tunarider/fung/internal/value"
)

// padNative is a native with a default and a rest parameter, to test the
// shared calling convention without depending on the real prelude.
func padNative() *value.Native {
	return &value.Native{
		Name: "pad",
		Params: []value.NativeParam{
			{Name: "s"}, {Name: "n", Default: value.Int(2)}, {Name: "tail", Rest: true},
		},
		Fn: func(_ value.Context, args []value.Value) (value.Value, error) {
			n := int(args[1].(value.Int))
			s := string(args[0].(value.Str))
			return value.Str(s + strings.Repeat("*", n) + value.Format(args[2])), nil
		},
	}
}

func runWith(t *testing.T, src string, extra map[string]value.Value) (string, string) {
	t.Helper()
	prelude := native.Prelude()
	for k, v := range extra {
		prelude[k] = v
	}
	p := compileWith(t, src, prelude)
	var out bytes.Buffer
	_, err := (&Runtime{Stdout: &out}).Run(p)
	code := ""
	if err != nil {
		code = err.Code
	}
	return out.String(), code
}

func TestNativesShareTheCallingConvention(t *testing.T) {
	extra := map[string]value.Value{"pad": padNative()}
	cases := map[string]string{
		`print(pad("a"))`:                 "a**[]\n",
		`print(pad("a", 3))`:              "a***[]\n",
		`print(pad("a", n: 1))`:           "a*[]\n",
		`print(pad("a", 1, 'x, 'y))`:      "a*['x, 'y]\n",
		`print(pad(n: 0, s: "b"))`:        "b[]\n",
		`print(pad("a", n: 2))`:           "a**[]\n", // same as the default
	}
	for src, want := range cases {
		out, code := runWith(t, src, extra)
		if code != "" || out != want {
			t.Errorf("%s: out=%q code=%s, want %q", src, out, code, want)
		}
	}
	errs := map[string]string{
		`pad()`:                 "E0407",
		`pad("a", n: 1, n: 2)`:  "E0405",
		`pad("a", m: 1)`:        "E0406",
		`print(1, sep: "")`:     "E0406", // rest params cannot be named
		`len()`:                 "E0407",
		`len(1, 2)`:             "E0402",
		`type_of()`:             "E0407",
	}
	for src, want := range errs {
		if _, code := runWith(t, src, extra); code != want {
			t.Errorf("%s: code = %s, want %s", src, code, want)
		}
	}
}

func TestExitUnwindsAsASignal(t *testing.T) {
	p := compileWith(t, "fn f(): print(1); exit(3); print(2) ;;\nf()\nprint(3)", native.Prelude())
	var out bytes.Buffer
	_, err := (&Runtime{Stdout: &out}).Run(p)
	code, ok := err.ExitCode()
	if !ok || code != 3 || out.String() != "1\n" {
		t.Fatalf("err=%v out=%q", err, out.String())
	}
	if _, _, code := run(t, "exit()"); code != "exit" {
		t.Fatalf("exit() code = %s", code)
	}
	if _, _, code := run(t, "exit(\"x\")"); code != "E0403" {
		t.Fatalf("exit(\"x\") code = %s", code)
	}
}

func TestStdinAndStderr(t *testing.T) {
	p := compileWith(t, "eprint(\"warn\", 1)\nprint(read_line(), read_line(), read_line())", native.Prelude())
	var out, errOut bytes.Buffer
	rt := &Runtime{Stdout: &out, Stderr: &errOut, Stdin: strings.NewReader("a\nb\n")}
	if _, err := rt.Run(p); err != nil {
		t.Fatal(err)
	}
	if out.String() != "Ok(\"a\") Ok(\"b\") Ok(nil)\n" || errOut.String() != "warn 1\n" {
		t.Fatalf("out=%q err=%q", out.String(), errOut.String())
	}
	p = compileWith(t, "print(read_lines())", native.Prelude())
	out.Reset()
	rt = &Runtime{Stdout: &out, Stdin: strings.NewReader("x\ny")}
	rt.Run(p)
	if out.String() != "Ok([\"x\", \"y\"])\n" {
		t.Fatalf("read_lines: %q", out.String())
	}
}
```
`eprint`·`read_line`·`read_lines`는 Task 8에 있지만 `Stdin`/`Stderr` 배관의 테스트라 여기서 만든다 (세 함수는 이 작업에서 `io.go`에 넣는다; Task 8은 그 파일에 더 넣기만 한다).

- [ ] **Step 2: 실패 확인** — `go test ./internal/vm/` → `pad()`가 E0406/인자 전달 실패, `exit` 미정의 등으로 FAIL.

- [ ] **Step 3: 구현**

`call.go`의 `bindArgs`를 매개변수 명세 기반으로 일반화한다:

```go
// argSpec is one parameter as bindArgs sees it, for fung functions
// (bytecode.Param: defaults come from the prologue) and natives
// (value.NativeParam: defaults are constants filled here).
type argSpec struct {
	name    string
	rest    bool
	hasDef  bool
	def     value.Value // natives only
}

func protoSpecs(p *bytecode.Proto) []argSpec {
	specs := make([]argSpec, len(p.Params))
	for i, prm := range p.Params {
		specs[i] = argSpec{name: prm.Name, rest: prm.Rest, hasDef: prm.HasDefault}
	}
	return specs
}

func nativeSpecs(n *value.Native) []argSpec {
	specs := make([]argSpec, len(n.Params))
	for i, prm := range n.Params {
		specs[i] = argSpec{name: prm.Name, rest: prm.Rest, hasDef: prm.Default != nil, def: prm.Default}
	}
	return specs
}

// bindArgs lays out a call's arguments in a fresh slot array (09. 호출
// 규약): positionals in order, named by lookup, a trailing rest parameter
// collecting the surplus as a list. A slot left as Go nil means "use the
// default" (a fung prologue fills it; a native default is filled here);
// a required slot left empty is E0407.
func bindArgs(name string, specs []argSpec, numSlots int, positional []value.Value, named []namedArg) ([]value.Value, *diag.Error) {
	fixed := len(specs)
	rest := -1
	if fixed > 0 && specs[fixed-1].rest {
		fixed--
		rest = fixed
	}
	if len(positional) > fixed && rest < 0 {
		return nil, diag.Errorf("E0402", diag.Pos{}, name, fixed, len(positional))
	}
	slots := make([]value.Value, numSlots)
	if len(positional) > fixed {
		copy(slots, positional[:fixed])
		slots[rest] = &value.List{Items: append([]value.Value(nil), positional[fixed:]...)}
	} else {
		copy(slots, positional)
	}
	if rest >= 0 && slots[rest] == nil {
		slots[rest] = &value.List{}
	}
	for _, na := range named {
		idx := -1
		for i := 0; i < fixed; i++ {
			if specs[i].name == na.name.Name() {
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
	for i := 0; i < fixed; i++ {
		if slots[i] != nil {
			continue
		}
		if !specs[i].hasDef {
			return nil, diag.Errorf("E0407", diag.Pos{}, specs[i].name)
		}
		if specs[i].def != nil {
			slots[i] = specs[i].def
		}
	}
	return slots, nil
}
```

`enter`의 `*value.Native` 분기:

```go
	case *value.Native:
		slots, e := bindArgs(fn.Name, nativeSpecs(fn), len(fn.Params), positional, named)
		if e != nil {
			return nil, true, e.WithPos(pos)
		}
		r, e2 := fn.Fn(t, slots)
		if e2 != nil {
			if d, ok := e2.(*diag.Error); ok {
				return nil, true, d.WithPos(pos)
			}
			return nil, true, diag.Errorf("E0801", pos, e2)
		}
		return r, true, nil
	case *bytecode.Closure:
		name := fn.Proto.Name
		if name == "" {
			name = "the function"
		}
		slots, e := bindArgs(name, protoSpecs(fn.Proto), fn.Proto.NumLocals, positional, named)
```
(종료 신호는 `*diag.Error`라 그대로 올라간다. `WithPos`는 신호에도 위치를 붙이지만 출력하지 않는다.)

`vm.go`의 `Runtime`:

```go
type Runtime struct {
	Stdout       io.Writer
	Stderr       io.Writer // nil: discard
	Stdin        io.Reader // nil: empty
	stdin        *bufio.Reader
	exitHandlers []value.Value
	…
}

func (t *Task) Stderr() io.Writer {
	if t.rt.Stderr == nil {
		return io.Discard
	}
	return t.rt.Stderr
}

func (t *Task) Stdin() *bufio.Reader {
	if t.rt.stdin == nil {
		in := t.rt.Stdin
		if in == nil {
			in = strings.NewReader("")
		}
		t.rt.stdin = bufio.NewReader(in)
	}
	return t.rt.stdin
}
```

`native/prelude.go`: 선언 도우미와 기존 함수 재선언.

```go
func param(name string) value.NativeParam                   { return value.NativeParam{Name: name} }
func opt(name string, def value.Value) value.NativeParam    { return value.NativeParam{Name: name, Default: def} }
func rest(name string) value.NativeParam                    { return value.NativeParam{Name: name, Rest: true} }

// def declares a native with its parameter list (06: 같은 호출 규약).
func def(name string, fn func(value.Context, []value.Value) (value.Value, error), params ...value.NativeParam) *value.Native {
	return &value.Native{Name: name, Params: params, Fn: fn}
}

func Prelude() map[string]value.Value {
	p := map[string]value.Value{}
	add := func(n *value.Native) { p[n.Name] = n }
	add(def("print", printFn, rest("args")))
	add(def("eprint", eprintFn, rest("args")))
	add(def("fail", failFn, rest("args")))
	add(def("at_exit", atExitFn, param("f")))
	add(def("len", lenFn, param("x")))
	add(def("map", mapFn, param("xs"), param("f")))
	add(def("type_of", typeOfFn, param("x")))
	add(def("exit", exitFn, opt("code", value.Int(0))))
	add(def("read_line", readLineFn, opt("f", value.Nil{})))
	add(def("read_lines", readLinesFn, opt("f", value.Nil{})))
	return p
}
```
`printFn`은 `args[0].(*value.List).Items`를 잇는다. `failFn`도 같다. 모든 손 개수 검사(`len(args) != 1`)를 지운다. `at_exit`·`map`의 타입 검사(E0403)는 남긴다.

```go
// internal/native/io.go
func exitFn(_ value.Context, args []value.Value) (value.Value, error) {
	code, ok := args[0].(value.Int)
	if !ok {
		return nil, diag.Errorf("E0403", diag.Pos{}, "code", "int", args[0].TypeName())
	}
	return nil, diag.ExitSignal(int(code))
}

func eprintFn(ctx value.Context, args []value.Value) (value.Value, error) {
	_, err := ctx.Stderr().Write([]byte(joinForPrint(args[0].(*value.List).Items) + "\n"))
	return value.Nil{}, err
}

// lineReader picks the reader for read_line/read_lines: stdin for nil,
// the file's reader otherwise. A closed file or a write-only file is
// Err({kind: 'closed}) (06).
func lineReader(ctx value.Context, f value.Value) (*bufio.Reader, value.Value) {
	switch x := f.(type) {
	case value.Nil:
		return ctx.Stdin(), nil
	case *value.File:
		if x.Closed || x.Reader == nil {
			return nil, errValue("closed", "file `"+x.Path+"` is closed")
		}
		return x.Reader, nil
	}
	return nil, nil
}

func readLineFn(ctx value.Context, args []value.Value) (value.Value, error) {
	r, errv := lineReader(ctx, args[0])
	if errv != nil {
		return errv, nil
	}
	if r == nil {
		return nil, diag.Errorf("E0403", diag.Pos{}, "f", "file | nil", args[0].TypeName())
	}
	line, err := r.ReadString('\n')
	if err != nil && line == "" {
		return ok(value.Nil{}), nil
	}
	return ok(value.Str(strings.TrimSuffix(strings.TrimSuffix(line, "\n"), "\r"))), nil
}

func readLinesFn(ctx value.Context, args []value.Value) (value.Value, error) {
	r, errv := lineReader(ctx, args[0])
	if errv != nil {
		return errv, nil
	}
	if r == nil {
		return nil, diag.Errorf("E0403", diag.Pos{}, "f", "file | nil", args[0].TypeName())
	}
	var lines []value.Value
	for {
		line, err := r.ReadString('\n')
		if line != "" {
			lines = append(lines, value.Str(strings.TrimSuffix(strings.TrimSuffix(line, "\n"), "\r")))
		}
		if err != nil {
			break
		}
	}
	return ok(&value.List{Items: lines}), nil
}
```
`internal/native/result.go`에 결과값 도우미 (Task 8·9가 다 쓴다):

```go
// ok wraps a value as Ok(v); errValue builds Err({kind, message}) (06).
func ok(v value.Value) value.Value { return &value.Tag{Name: value.Intern("Ok"), Args: []value.Value{v}} }

func errValue(kind, message string, extra ...value.Value) value.Value {
	m := value.NewMap()
	m.Set("kind", value.Intern(kind))
	m.Set("message", value.Str(message))
	for i := 0; i+1 < len(extra); i += 2 {
		m.Set(string(extra[i].(value.Str)), extra[i+1])
	}
	return &value.Tag{Name: value.Intern("Err"), Args: []value.Value{m}}
}
```
`prelude_test.go`: `print.Fn(c, []value.Value{&value.List{Items: …}})`로 바꾸고, fake `ctx`에 `Stderr()`·`Stdin()`을 더한다.

- [ ] **Step 4: 통과 확인** — `go test ./...` 전부 PASS (M1~M4의 `print`·`fail` 골든이 그대로 돌아야 한다).
- [ ] **Step 5: 커밋** `feat(vm): give natives the shared calling convention, stdin/stderr, and the exit signal`

---

### Task 3: parser — `import` 문, 맵 중복 키, `_ = 식`

**Files:**
- Modify: `internal/parser/stmt.go`, `internal/parser/collections.go`
- Test: `internal/parser/m5_test.go`

**Interfaces:**
- Produces: surface 노드 `(import spec alias (name…))` → `tree.WithLit("import", pos, *tree.Import{Spec, IsPath, Alias, Names, NamePos})`로 Lit 하나에 담는다 (`tree/fn.go`에 구조체 추가, `String()`은 `import fs as f (a, b)` 꼴). dump: `(import "./lib/x" as rn (a, b))` / `(import fs)`.
- `_ = e` → `(assign (pwild) e)`.

- [ ] **Step 1: 테스트**

```go
// internal/parser/m5_test.go
package parser

import "testing"

func TestImportStatement(t *testing.T) {
	cases := map[string]string{
		"import fs":                               "(program (import fs))",
		"import \"./lib/rename\"":                  `(program (import "./lib/rename"))`,
		"import \"./lib/rename\" as rn":            `(program (import "./lib/rename" as rn))`,
		"import \"./lib/rename\" (slugify, kebab)": `(program (import "./lib/rename" (slugify, kebab)))`,
		"import \"./x\" as y (a)":                  `(program (import "./x" as y (a)))`,
		"import fs\nprint(fs)":                     "(program (import fs) (call print fs))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
	errs := map[string]string{
		"import":             "E0112",
		"import 1":           "E0112",
		"import fs as":       "E0112",
		"import fs ()":       "E0112",
		"import fs (a,)":     "E0112",
		"import fs.read":     "E0110",
		"import \"x\" as Fs": "E0112", // alias must be a name, not a tag
	}
	for src, want := range errs {
		if got := parseErr(t, src); got != want {
			t.Errorf("%q: %s, want %s", src, got, want)
		}
	}
}

func TestMapLiteralRejectsDuplicateKeys(t *testing.T) {
	if got := parseErr(t, "{a: 1, a: 2}"); got != "E0113" {
		t.Errorf("dup key: %s", got)
	}
	if got := parseErr(t, "{a: 1, \"a\": 2}"); got != "E0113" {
		t.Errorf("dup quoted key: %s", got)
	}
	if got := parse(t, "{a: 1, b: 1}"); got != "(program (map (entry a 1) (entry b 1)))" {
		t.Errorf("distinct keys: %s", got)
	}
}

func TestUnderscoreAssignmentDiscards(t *testing.T) {
	if got := parse(t, "_ = f()"); got != "(program (assign (pwild) (call f)))" {
		t.Errorf("got %s", got)
	}
}
```

- [ ] **Step 2: 실패 확인** — `go test ./internal/parser/` FAIL (`import`가 E0110, 중복 키 통과, `_ = f()`가 `(assign _ …)`).

- [ ] **Step 3: 구현**

`tree/fn.go`:

```go
// Import is the Lit of an (import …) node (04).
type Import struct {
	Spec    string     // "fs" or "./lib/rename"
	IsPath  bool       // written as a string: a file path
	Alias   string     // after `as`, "" if none
	Names   []string   // the (a, b) list, nil if none
	NamePos []diag.Pos // one per Names entry, for errors
}

func (im *Import) String() string {
	var b strings.Builder
	if im.IsPath {
		b.WriteString(strconv.Quote(im.Spec))
	} else {
		b.WriteString(im.Spec)
	}
	if im.Alias != "" {
		b.WriteString(" as " + im.Alias)
	}
	if im.Names != nil {
		b.WriteString(" (" + strings.Join(im.Names, ", ") + ")")
	}
	return b.String()
}
```
dump가 Lit를 먼저 찍으므로 `(import fs as f (a))`가 된다. **`tree/node.go`의 `String`이 `*Import`를 `fmt.Sprint`로 찍는지 확인**하고(`Stringer`면 그대로) 아니면 분기를 더한다.

`stmt.go`의 `statement()` 맨 앞에:

```go
	if t.Kind == lexer.KwImport {
		return p.importStmt()
	}
```

```go
// importStmt parses `import NAME|STRING [as NAME] [(NAME, …)]` (01 EBNF).
func (p *parser) importStmt() (*tree.Node, *diag.Error) {
	t := p.next() // import
	im := &tree.Import{}
	switch p.peek().Kind {
	case lexer.Name:
		im.Spec = p.next().Text
	case lexer.String:
		im.Spec = p.next().Value.(string)
		im.IsPath = true
	default:
		return nil, diag.Errorf("E0112", p.peek().Pos, "a module name or a path string after `import`", describe(p.peek()))
	}
	if p.at(lexer.KwAs) {
		p.next()
		name, err := p.expectDesc(lexer.Name, "a name after `as`")
		if err != nil {
			return nil, err
		}
		im.Alias = name.Text
	}
	if p.at(lexer.LParen) {
		p.next()
		for {
			name, err := p.expectDesc(lexer.Name, "a name to import")
			if err != nil {
				return nil, err
			}
			im.Names = append(im.Names, name.Text)
			im.NamePos = append(im.NamePos, name.Pos)
			if !p.at(lexer.Comma) {
				break
			}
			p.next()
		}
		if _, err := p.expectDesc(lexer.RParen, "`)`"); err != nil {
			return nil, err
		}
	}
	return tree.WithLit("import", t.Pos, im), nil
}
```
(`String` 토큰의 값 필드 이름은 렉서를 보고 맞춘다: 보간 없는 문자열이어야 하며, 보간이 있으면 E0112 "a plain string".)

`collections.go`의 `mapLiteral`: 키를 `seen map[string]bool`에 넣고 중복이면 `diag.Errorf("E0113", keyPos, key)`.

`stmt.go`: `t.Kind == lexer.Name && t.Text == "_" && 다음이 Assign` → `tree.New("assign", t.Pos, tree.New("pwild", t.Pos), rhs)`.

- [ ] **Step 4: 통과 확인** — `go test ./internal/parser/` PASS. 전체도 PASS (`_ = 1`이 resolve에서 E0215를 내던 테스트가 있으면 지운다: 이제 유효한 문장이다. `internal/resolve/m4_test.go`의 `"x = _"`는 그대로 E0215).
- [ ] **Step 5: 커밋** `feat(parser): parse import statements, reject duplicate map keys, and discard with _ =`

---

### Task 4: desugar·resolve·compiler — import 묶기, 정적 검사, 최상위 제한, 가리기, 패턴 중복 이름, `IMPORT`

**Files:**
- Modify: `internal/desugar/desugar.go`(import 통과), `internal/resolve/resolve.go`, `internal/resolve/scope.go`, `internal/resolve/pattern.go`, `internal/resolve/fn.go`, `internal/compiler/compiler.go`
- Create: `internal/resolve/module.go`
- Test: `internal/resolve/m5_test.go`, `internal/compiler/m5_test.go`

**Interfaces:**
- Produces:
  ```go
  // resolve
  type ModuleInfo struct { Key, Name string; Exports []string }
  type Options struct {
      Imports map[string]ModuleInfo // keyed by the spec as written in this file
      Library bool                  // an imported file: top-level restrictions (04)
  }
  func Resolve(core *tree.Node, prelude []string, opts Options) (Result, []*diag.Error)
  // Result.Imports []string — the keys this file imports, in order (loader uses it)
  ```
  resolve 뒤 핵심 형태: `(import "key" (global n)|(nil) (member (global m) name)…)` (Lit = key 문자열). 컴파일: `IMPORT k; STOREGLOBAL n` 그리고 멤버마다 `IMPORT k; FIELD name; STOREGLOBAL m`.
- `binding.module *ModuleInfo` (모듈 이름 바인딩), `binding.constant`.

- [ ] **Step 1: 테스트**

```go
// internal/resolve/m5_test.go
package resolve

import (
	"strings"
	"testing"
)

var fsInfo = ModuleInfo{Key: "fs", Name: "fs", Exports: []string{"read", "write"}}
var libInfo = ModuleInfo{Key: "/abs/lib/x.fung", Name: "x", Exports: []string{"a", "b"}}

func resolveM5(t *testing.T, src string, lib bool) (Result, []string) {
	t.Helper()
	opts := Options{Imports: map[string]ModuleInfo{"fs": fsInfo, "./lib/x": libInfo}, Library: lib}
	r, errs := Resolve(coreOf(t, src), []string{"print", "map"}, opts)
	codes := make([]string, len(errs))
	for i, e := range errs {
		codes[i] = e.Code
	}
	return r, codes
}

func TestImportBindsConstantGlobals(t *testing.T) {
	r, errs := resolveM5(t, "import fs\nimport \"./lib/x\" as y (a)\nprint(fs.read, y.b, a)", false)
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	got := r.Core.String()
	for _, want := range []string{
		`(import "fs" (global 0))`,
		`(import "/abs/lib/x.fung" (global 1) (member (global 2) a))`,
		"(field read (global 0))",
	} {
		if !strings.Contains(got, want) {
			t.Errorf("missing %s in %s", want, got)
		}
	}
	if len(r.Imports) != 2 || r.Imports[0] != "fs" || r.Imports[1] != "/abs/lib/x.fung" {
		t.Errorf("Imports = %v", r.Imports)
	}
	if len(r.Globals) != 3 || r.Globals[0] != "fs" || r.Globals[1] != "y" || r.Globals[2] != "a" {
		t.Errorf("Globals = %v", r.Globals)
	}
}

func TestImportErrors(t *testing.T) {
	cases := map[string]string{
		"import nope":                               "E0301",
		"import fs\nprint(fs.nope)":                 "E0303",
		"import \"./lib/x\" (nope)":                 "E0303",
		"import fs\nprint(fs._secret)":              "E0304",
		"import fs\nimport fs":                      "E0309",
		"import fs\nimport \"./lib/x\" as fs":       "E0309",
		"import fs\nfs = 1":                         "E0308",
		"import fs\nfn fs() = 1":                    "E0308",
		"import fs\nfn f(fs) = fs":                  "E0308",
		"import fs\nfor fs in [1]: fs ;;":           "E0308",
		"import fs\nmatch 1: fs => fs ;;":           "E0308",
		"fn f():\n  import fs\n  1\n;;":             "E0311",
		"import \"./lib/x\" (a)\na = 2":             "",      // listed names are ordinary names
		"import \"./lib/x\" (a)\nprint(a, x)":       "E0204", // the module name is not bound when only names are listed
		"fn map(xs) = xs\nprint(map(1))":            "",      // prelude names may be shadowed (04)
	}
	for src, want := range cases {
		_, errs := resolveM5(t, src, false)
		got := ""
		if len(errs) > 0 {
			got = errs[0]
		}
		if got != want {
			t.Errorf("%q: %v, want %s", src, errs, want)
		}
	}
}

func TestLibraryTopLevelRestriction(t *testing.T) {
	cases := map[string]string{
		"import fs\nfn f() = 1\ntype t = int\nx = print(1)\n(a, b) = (1, 2)": "",
		"print(1)":          "E0306",
		"fn f() = 1\nf()":   "E0306",
		"1 + 1":             "E0306",
		"if true: 1 ;;":     "E0306",
	}
	for src, want := range cases {
		_, errs := resolveM5(t, src, true)
		got := ""
		if len(errs) > 0 {
			got = errs[0]
		}
		if got != want {
			t.Errorf("%q: %v, want %s", src, errs, want)
		}
	}
	if _, errs := resolveM5(t, "print(1)", false); len(errs) != 0 {
		t.Error("the entry file may have statements")
	}
}

func TestDuplicatePatternNames(t *testing.T) {
	cases := map[string]string{
		"[a, a] = [1, 2]":                         "E0221",
		"match 1: (a, a) => a ;;":                 "E0221",
		"match 1: {x: a, y: a} => a ;;":           "E0221",
		"match 1: Ok(a) | Err(a) => a ;;":         "",      // the same name once per alternative is fine
		"match 1: [a, b] when a == b => a ;;":     "",
		"_ = print(1)":                            "",
	}
	for src, want := range cases {
		_, errs := resolveM5(t, src, false)
		got := ""
		if len(errs) > 0 {
			got = errs[0]
		}
		if got != want {
			t.Errorf("%q: %v, want %s", src, errs, want)
		}
	}
}
```
`coreOf`는 기존 테스트 도우미(desugar까지). 기존 `Resolve(core, prelude)` 호출은 모두 `Resolve(core, prelude, Options{})`로 바꾼다 (`frontend`, 테스트).

```go
// internal/compiler/m5_test.go
package compiler

import (
	"strings"
	"testing"
)

func TestCompileImport(t *testing.T) {
	// compileSrc is the existing helper that runs the front end with the
	// test prelude; extend it (or add compileSrcWith) to pass resolve
	// Options with Imports {"fs": {Key: "fs", Exports: ["read"]}}.
	p := compileSrcWith(t, "import fs\nprint(fs.read)", fsImports)
	got := disasmMain(p)
	for _, want := range []string{"IMPORT       0", "STOREGLOBAL  0", "FIELD"} {
		if !strings.Contains(got, want) {
			t.Errorf("missing %s in\n%s", want, got)
		}
	}
	if p.Consts[0].TypeName() != "str" {
		t.Errorf("const 0 should be the module key, got %v", p.Consts[0])
	}
	p = compileSrcWith(t, "import fs (read)\nread", fsImports)
	got = disasmMain(p)
	if !strings.Contains(got, "IMPORT       0\n") || !strings.Contains(got, "FIELD") || !strings.Contains(got, "STOREGLOBAL  0") {
		t.Errorf("member import:\n%s", got)
	}
}
```

- [ ] **Step 2: 실패 확인** — 두 패키지 FAIL (Options 없음, import 노드가 E0801 unknown core form).

- [ ] **Step 3: 구현**

desugar: `import` 노드는 그대로 통과 (`walk`의 기본 분기가 kid를 재귀하므로 Lit만 있는 노드는 그대로 나온다. 확인만).

resolve (`module.go`):

```go
// ModuleInfo is what resolve knows about an imported module: enough to
// check member names at compile time (04: 정적으로 알면 컴파일 에러).
type ModuleInfo struct {
	Key     string
	Name    string
	Exports []string
}

// Options configures a file's resolution.
type Options struct {
	Imports map[string]ModuleInfo // spec as written → module
	Library bool                  // imported file: top-level restrictions
}

// importStmt binds an import's names as constant globals (module names
// marked as modules so they cannot be shadowed, 04) and rewrites the
// statement to (import key (global n)|(nil) (member (global m) name)…).
func (r *resolver) importStmt(n *tree.Node, sc *scope) *tree.Node {
	im := n.Lit.(*tree.Import)
	info, ok := r.opts.Imports[im.Spec]
	if !ok {
		r.errs = append(r.errs, diag.Errorf("E0301", n.Pos, im.Spec))
		return tree.New("nil", n.Pos)
	}
	r.imports = append(r.imports, info.Key)
	out := tree.WithLit("import", n.Pos, info.Key)
	modName := im.Alias
	if modName == "" && im.Names == nil {
		modName = info.Name
	}
	if modName != "" {
		if b := r.bindModule(modName, n.Pos, info, sc); b != nil {
			out.Kids = append(out.Kids, ref(n.Pos, *b))
		} else {
			out.Kids = append(out.Kids, tree.New("nil", n.Pos))
		}
	} else {
		out.Kids = append(out.Kids, tree.New("nil", n.Pos))
	}
	for i, name := range im.Names {
		pos := im.NamePos[i]
		if !contains(info.Exports, name) {
			r.errs = append(r.errs, diag.Errorf("E0303", pos, info.Name, name).Span(runeLen(name)))
			continue
		}
		b := r.bindNew(name, pos) // an ordinary global: may be shadowed
		if b == nil {
			continue
		}
		out.Kids = append(out.Kids, tree.WithLit("member", pos, name, ref(pos, *b)))
	}
	return out
}

// bindModule binds a module name: constant, unshadowable, once (E0309).
func (r *resolver) bindModule(name string, pos diag.Pos, info ModuleInfo, sc *scope) *binding {
	if b, ok := sc.names[name]; ok {
		if b.module != nil {
			r.errs = append(r.errs, diag.Errorf("E0309", pos, name).Span(runeLen(name)))
		} else {
			r.errs = append(r.errs, diag.Errorf("E0308", pos, name).Span(runeLen(name)))
		}
		return nil
	}
	mi := info
	b := binding{global: true, index: len(r.globals), constant: true, module: &mi}
	r.globals = append(r.globals, name)
	sc.names[name] = b
	return &b
}

// checkShadow reports E0308 when a new name would hide an imported
// module name (04: import한 모듈 이름을 가리면 에러). It is called from
// every place that creates a name: assignment, fn, params, patterns.
func (r *resolver) checkShadow(name string, pos diag.Pos) bool {
	if b, ok := r.moduleNames[name]; ok && b {
		r.errs = append(r.errs, diag.Errorf("E0308", pos, name).Span(runeLen(name)).
			WithHelp(fmt.Sprintf("import it under another name: `import %s as m`", name)))
		return false
	}
	return true
}
```
`r.moduleNames map[string]bool`은 `bindModule`이 채운다. `bindNew`·`bindFresh`·함수 매개변수·named `fn` 바인딩 앞에 `checkShadow`를 넣는다 (모듈 이름 자신을 다시 `import`하는 경우는 `bindModule`이 먼저 처리). `import fs` 뒤의 `fs = 1`은 `bindNew`가 constant 바인딩을 만나 E0209를 내던 자리에서 **module이면 E0308**로 바꾼다.

`rootBlock`: 최상위 `import`는 `fn` 끌어올리기와 같은 패스에서 **순서대로** 처리한다 — import 바인딩이 먼저 있어야 fn 본문이 모듈을 볼 수 있으므로 "typealias → import → fn 끌어올림 → 나머지" 순으로 `sc.names`를 채우고, 출력은 import 문(순서대로) → fn let → 나머지. `walk`의 `case "import"`는 루트가 아니면 E0311.

최상위 제한 (`Library`): `rootBlock`에서 `rest`의 각 문장이 `let`이 아니면(`assign`은 desugar 뒤 `let`; 구조 분해도 `let`) E0306.

정적 멤버 검사: `walk`의 `case "field"`에서 대상이 `(name x)`이고 `x`가 모듈 바인딩이면 `info.Exports`로 확인 — `_` 시작 → E0304, 없음 → E0303. (대상이 다른 식이면 런타임.)

패턴 중복 이름(E0221): `bindPattern`이 패턴 하나를 처리하는 동안 `seen map[string]diag.Pos`를 들고 `pbind`·`prest`마다 검사한다. `palt`는 대안마다 `seen`을 **처음 대안 상태로 되돌려** 시작한다 (각 대안이 같은 이름을 한 번씩 묶는 것은 정상).

`Result`에 `Imports []string` 추가.

compiler (`compiler.go`의 `block`/`expr`):

```go
	case "import":
		key := c.proto.Const(value.Str(n.Lit.(string)))
		if n.Kids[0].Head != "nil" {
			c.proto.Emit(bytecode.OpImport, key, n.Pos)
			c.storeRef(n.Kids[0], n.Pos) // STOREGLOBAL n
		}
		for _, m := range n.Kids[1:] {
			c.proto.Emit(bytecode.OpImport, key, m.Pos)
			c.proto.Emit(bytecode.OpField, c.proto.Const(value.Str(m.Lit.(string))), m.Pos)
			c.storeRef(m.Kids[0], m.Pos)
		}
		return nil
```
`import`는 문장이므로 `block`에서 `let`처럼 값을 남기지 않는다 (마지막이면 `CONST nil`). `storeRef`는 `let`이 쓰는 저장 코드를 뽑아낸 도우미.

- [ ] **Step 4: 통과 확인** — `go test ./...` PASS.
- [ ] **Step 5: 커밋** `feat(resolve): bind imports, check module members statically, restrict library top levels, and reject duplicate pattern names`

---

### Task 5: 로더와 모듈 런타임 — import 그래프, 순환, embed, 원시 모듈, 모듈별 전역, `IMPORT`

**Files:**
- Create: `internal/frontend/load.go`, `internal/stdlib/stdlib.go`, `internal/stdlib/empty.fung`(embed가 비면 안 되므로 임시, Task 9에서 교체), `internal/vm/module.go`
- Modify: `internal/frontend/frontend.go`, `internal/vm/vm.go`, `internal/vm/call.go`
- Test: `internal/frontend/load_test.go`, `internal/vm/module_test.go`

**Interfaces:**
- Produces:
  ```go
  // frontend
  type Loaded struct { Key, Name string; Proto *bytecode.Proto; Native *value.Module }
  type Program struct { Entry *bytecode.Proto; Modules []Loaded; Sources map[string]string }
  type Loader struct {
      Prelude  map[string]value.Value
      Natives  map[string]*value.Module          // "_native.fs" → module
      Stdlib   func(name string) (string, bool)  // fung source of a stdlib module by name
      ReadFile func(path string) (string, error) // os.ReadFile by default
  }
  func (l *Loader) Load(entryFile, src string) (Program, []*diag.Error)
  // vm
  func (rt *Runtime) Register(key string, p *bytecode.Proto)
  func (rt *Runtime) RegisterNative(key string, m *value.Module)
  // stdlib
  func Source(name string) (string, bool)   // "fs" → contents of fs.fung
  ```
- 모듈 키: 파일은 `filepath.Abs` + `filepath.Clean`; `-e`의 상대 경로는 cwd 기준; stdlib은 이름; 원시는 `_native.x`.
- 모듈 이름: 파일은 확장자 뺀 이름이 식별자(`[a-z_][a-z0-9_]*`)여야 하고 아니면 `as` 없을 때 E0305. stdlib은 이름.

- [ ] **Step 1: 테스트**

```go
// internal/frontend/load_test.go
package frontend

import (
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/native"
	"github.com/tunarider/fung/internal/value"
)

// memLoader serves files from a map so tests need no disk.
func memLoader(files map[string]string, stdlib map[string]string) *Loader {
	nfs := value.NewModule("fs", "_native.fs")
	nfs.Set("read", &value.Native{Name: "read"})
	return &Loader{
		Prelude: native.Prelude(),
		Natives: map[string]*value.Module{"_native.fs": nfs},
		Stdlib:  func(name string) (string, bool) { s, ok := stdlib[name]; return s, ok },
		ReadFile: func(path string) (string, error) {
			if s, ok := files[path]; ok {
				return s, nil
			}
			return "", &fileNotFound{path}
		},
	}
}

func TestLoadGraphInDependencyOrder(t *testing.T) {
	files := map[string]string{
		"/p/main.fung":  "import \"./lib/a\"\nimport \"./lib/b\" as bb\nprint(a.x, bb.y)",
		"/p/lib/a.fung": "import \"../lib/b\"\nx = b.y + 1",
		"/p/lib/b.fung": "y = 1",
	}
	l := memLoader(files, nil)
	prog, errs := l.Load("/p/main.fung", files["/p/main.fung"])
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	if len(prog.Modules) != 2 || prog.Modules[0].Key != "/p/lib/b.fung" || prog.Modules[1].Key != "/p/lib/a.fung" {
		t.Fatalf("modules = %+v", prog.Modules)
	}
	if prog.Entry == nil || prog.Sources["/p/lib/a.fung"] == "" {
		t.Fatal("entry or sources missing")
	}
}

func TestLoadStdlibAndNatives(t *testing.T) {
	stdlib := map[string]string{"fs": "import _native.fs as _n\nread = _n.read"}
	l := memLoader(map[string]string{}, stdlib)
	prog, errs := l.Load("-e", "import fs\nprint(fs.read)")
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	if len(prog.Modules) != 2 || prog.Modules[0].Key != "_native.fs" || prog.Modules[0].Native == nil || prog.Modules[1].Key != "fs" {
		t.Fatalf("modules = %+v", prog.Modules)
	}
	_, errs = l.Load("-e", "import _native.fs")
	if len(errs) != 1 || errs[0].Code != "E0310" {
		t.Fatalf("user code importing _native: %v", errs)
	}
}

func TestLoadErrors(t *testing.T) {
	files := map[string]string{
		"/p/main.fung":    "import \"./cyc_a\"",
		"/p/cyc_a.fung":   "import \"./cyc_b\"",
		"/p/cyc_b.fung":   "import \"./cyc_a\"",
		"/p/self.fung":    "import \"./self\"",
		"/p/entry.fung":   "import \"./uses_entry\"",
		"/p/uses_entry.fung": "import \"./entry\"",
		"/p/bad-name.fung": "x = 1",
		"/p/usebad.fung":  "import \"./bad-name\"",
		"/p/usebad2.fung": "import \"./bad-name\" as ok\nprint(ok.x)",
		"/p/lib_err.fung": "print(1)",
		"/p/uselib.fung":  "import \"./lib_err\"",
		"/p/synerr.fung":  "x = (",
		"/p/usesyn.fung":  "import \"./synerr\"",
	}
	l := memLoader(files, map[string]string{})
	cases := map[string]string{
		"/p/main.fung":    "E0302",
		"/p/self.fung":    "E0302",
		"/p/entry.fung":   "E0307",
		"/p/usebad.fung":  "E0305",
		"/p/usebad2.fung": "",
		"/p/uselib.fung":  "E0306",
		"/p/usesyn.fung":  "E0112",
	}
	for entry, want := range cases {
		_, errs := l.Load(entry, files[entry])
		got := ""
		if len(errs) > 0 {
			got = errs[0].Code
		}
		if got != want {
			t.Errorf("%s: %v, want %s", entry, errs, want)
		}
	}
	_, errs := l.Load("/p/main.fung", files["/p/main.fung"])
	if msg := errs[0].Error(); !strings.Contains(msg, "cyc_a → cyc_b → cyc_a") {
		t.Errorf("cycle message: %s", msg)
	}
	_, errs = l.Load("-e", "import missing")
	if len(errs) != 1 || errs[0].Code != "E0301" {
		t.Errorf("missing stdlib: %v", errs)
	}
	_, errs = l.Load("/p/x.fung", "import \"./nothing\"")
	if len(errs) != 1 || errs[0].Code != "E0301" {
		t.Errorf("missing file: %v", errs)
	}
}
```
```go
// fileNotFound is the error memLoader returns for unknown paths.
type fileNotFound struct{ path string }

func (e *fileNotFound) Error() string { return "no file " + e.path }
```

```go
// internal/vm/module_test.go
package vm

import (
	"bytes"
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/frontend"
	"github.com/tunarider/fung/internal/native"
	"github.com/tunarider/fung/internal/value"
)

func runProgram(t *testing.T, files map[string]string, entry string) (string, string) {
	t.Helper()
	l := &frontend.Loader{
		Prelude: native.Prelude(),
		Natives: map[string]*value.Module{},
		Stdlib:  func(string) (string, bool) { return "", false },
		ReadFile: func(p string) (string, error) {
			if s, ok := files[p]; ok {
				return s, nil
			}
			return "", fmt.Errorf("no file %s", p)
		},
	}
	prog, errs := l.Load(entry, files[entry])
	if len(errs) > 0 {
		t.Fatal(errs)
	}
	var out bytes.Buffer
	rt := &Runtime{Stdout: &out}
	for _, m := range prog.Modules {
		if m.Native != nil {
			rt.RegisterNative(m.Key, m.Native)
		} else {
			rt.Register(m.Key, m.Proto)
		}
	}
	_, err := rt.Run(prog.Entry)
	code := ""
	if err != nil {
		code = err.Code
	}
	return out.String(), code
}

func TestModulesKeepTheirOwnGlobals(t *testing.T) {
	files := map[string]string{
		"/p/a.fung":    "fn helper() = \"a\"\nfn run() = helper()",
		"/p/b.fung":    "import \"./a\"\nfn helper() = \"b\"\nfn run() = helper() + a.run()",
		"/p/main.fung": "import \"./a\"\nimport \"./b\"\nprint(b.run(), a.run())\nf = b.run\nprint(f())",
	}
	out, code := runProgram(t, files, "/p/main.fung")
	if code != "" || out != "ba a\nba\n" {
		t.Fatalf("out=%q code=%s", out, code)
	}
}

func TestModuleTopLevelRunsOnce(t *testing.T) {
	files := map[string]string{
		"/p/m.fung":    "x = print(\"init\")\ny = 1",
		"/p/a.fung":    "import \"./m\"\nv = m.y",
		"/p/main.fung": "import \"./m\"\nimport \"./a\"\nprint(m.y, a.v)",
	}
	out, _ := runProgram(t, files, "/p/main.fung")
	if out != "init\n1 1\n" {
		t.Fatalf("out=%q", out)
	}
}

func TestModuleValueAndErrors(t *testing.T) {
	files := map[string]string{
		"/p/m.fung":    "fn _hidden() = 1\nfn shown() = 2\nz = 3",
		"/p/main.fung": "import \"./m\"\nprint(m, type_of(m), m.shown(), m.z, m == m)\nv = m\nprint(v._hidden)",
	}
	out, code := runProgram(t, files, "/p/main.fung")
	if out != "<module m> 'module 2 3 true\n" || code != "E0304" {
		t.Fatalf("out=%q code=%s", out, code)
	}
	files["/p/main.fung"] = "import \"./m\"\nv = m\nprint(v.nope)"
	if _, code := runProgram(t, files, "/p/main.fung"); code != "E0303" {
		t.Fatalf("dynamic missing member: %s", code)
	}
	files["/p/boom.fung"] = "x = 1 / 0"
	files["/p/main.fung"] = "import \"./boom\""
	if _, code := runProgram(t, files, "/p/main.fung"); code != "E0502" {
		t.Fatalf("error during module init: %s", code)
	}
}

func TestMemberImportAndClosureGlobals(t *testing.T) {
	files := map[string]string{
		"/p/m.fung":    "base = 10\nfn add(x) = x + base",
		"/p/main.fung": "import \"./m\" (add)\nbase = 1\nprint(add(5), map([1, 2], add))",
	}
	out, code := runProgram(t, files, "/p/main.fung")
	if code != "" || out != "15 [11, 12]\n" {
		t.Fatalf("out=%q code=%s", out, code)
	}
}
```

- [ ] **Step 2: 실패 확인** — 두 패키지 컴파일 실패.

- [ ] **Step 3: 구현**

`internal/stdlib/stdlib.go`:

```go
// Package stdlib embeds the standard library written in fung (06). Each
// file is a module named after the file; it is compiled when imported.
package stdlib

import "embed"

//go:embed *.fung
var files embed.FS

// Source returns the fung source of a standard library module.
func Source(name string) (string, bool) {
	b, err := files.ReadFile(name + ".fung")
	if err != nil {
		return "", false
	}
	return string(b), true
}
```
(`empty.fung`은 `# placeholder until Task 9` 한 줄. Task 9에서 지운다.)

`frontend/load.go` 핵심:

```go
type loader struct {
	*Loader
	entryKey string
	done     map[string]*Loaded          // key → loaded (nil while loading)
	stack    []string                     // names on the DFS path, for E0302
	order    []Loaded
	sources  map[string]string
}

func (l *Loader) Load(entryFile, src string) (Program, []*diag.Error) {
	ld := &loader{Loader: l, done: map[string]*Loaded{}, sources: map[string]string{}}
	if ld.ReadFile == nil { ld.ReadFile = func(p string) (string, error) { b, err := os.ReadFile(p); return string(b), err } }
	key := entryFile
	if entryFile != "-e" { key = absKey(entryFile) }
	ld.entryKey = key
	ld.sources[key] = src
	proto, errs := ld.compileFile(key, entryFile, src, false, false)
	if len(errs) > 0 { return Program{}, errs }
	return Program{Entry: proto, Modules: ld.order, Sources: ld.sources}, nil
}

// compileFile parses one file, loads what it imports (depth first), then
// resolves and compiles it with the exports of its imports in hand.
func (ld *loader) compileFile(key, file, src string, library, stdlib bool) (*bytecode.Proto, []*diag.Error) {
	toks, _, err := lexer.Lex(file, src)
	if err != nil { return nil, []*diag.Error{err} }
	surface, perr := parser.Parse(toks)
	if perr != nil { return nil, []*diag.Error{perr} }
	imports := map[string]resolve.ModuleInfo{}
	for _, k := range surface.Kids {
		if k.Head != "import" { continue }          // non-root imports are E0311 in resolve
		im := k.Lit.(*tree.Import)
		info, errs := ld.loadImport(im, k.Pos, key, stdlib)
		if len(errs) > 0 { return nil, errs }
		imports[im.Spec] = info
	}
	core, derr := desugar.Desugar(surface)
	if derr != nil { return nil, []*diag.Error{derr} }
	res, rerrs := resolve.Resolve(core, preludeNames(ld.Prelude), resolve.Options{Imports: imports, Library: library})
	if len(rerrs) > 0 { return nil, rerrs }
	proto, cerr := compiler.Compile(res, ld.Prelude)
	if cerr != nil { return nil, []*diag.Error{cerr} }
	return proto, nil
}

// loadImport resolves one import spec to a module key, loads it if it
// is new, and returns what resolve needs. Cycles (E0302), the entry file
// (E0307), internal modules from user code (E0310), bad file names
// (E0305) and missing modules (E0301) are reported at the import.
func (ld *loader) loadImport(im *tree.Import, pos diag.Pos, fromKey string, fromStdlib bool) (resolve.ModuleInfo, []*diag.Error) {
	switch {
	case im.IsPath && (strings.HasPrefix(im.Spec, "./") || strings.HasPrefix(im.Spec, "../") || strings.HasPrefix(im.Spec, "/")):
		return ld.loadFile(im, pos, fromKey)
	case im.IsPath:
		return resolve.ModuleInfo{}, []*diag.Error{diag.Errorf("E0301", pos, im.Spec)} // package paths: later
	case strings.HasPrefix(im.Spec, "_native."):
		if !fromStdlib {
			return resolve.ModuleInfo{}, []*diag.Error{diag.Errorf("E0310", pos, im.Spec, strings.TrimPrefix(im.Spec, "_native."))}
		}
		m, ok := ld.Natives[im.Spec]
		if !ok { return resolve.ModuleInfo{}, []*diag.Error{diag.Errorf("E0301", pos, im.Spec)} }
		if ld.done[im.Spec] == nil {
			l := &Loaded{Key: im.Spec, Name: m.Name, Native: m}
			ld.done[im.Spec] = l
			ld.order = append(ld.order, *l)
		}
		return resolve.ModuleInfo{Key: im.Spec, Name: m.Name, Exports: m.Names()}, nil
	default:
		return ld.loadStdlib(im, pos)
	}
}
```
`loadFile`: `dir = filepath.Dir(fromKey)`(fromKey가 `-e`면 cwd); `path = filepath.Clean(filepath.Join(dir, spec))`, `.fung`이 없으면 붙인다; `key = path`; `key == ld.entryKey` → E0307; 이름 = stem; 이름이 식별자가 아니고 `im.Alias == ""` → E0305; `ld.load(key, name, library=true, stdlib=false)`.
`loadStdlib`: `Stdlib(name)` 없으면 E0301; `ld.load(name, name, true, true)` (`ReadFile` 대신 그 소스).
`load(key, name, …)`: `done[key]`가 있고 `Loaded`면 그 exports; `done[key]`가 "loading" 표시(`stack`에 있음)면 E0302 with `strings.Join(append(stack[i:], name), " → ")`; 아니면 stack push → 소스 읽기(없으면 E0301) → `sources[key] = src` → `compileFile(key, file=key or name, src, library=true, stdlib)` → pop → `Loaded{Key, Name, Proto}` 기록·`order`에 추가 → exports = `exportsOf(proto)` (`GlobalNames` 중 `_`·`$`로 시작하지 않는 것).

에러 위치: 가져온 파일 안의 에러는 그 파일의 `Pos.File`이 그 파일 경로다 (렉서에 넘긴 file). 순환·없음 등은 import 문 위치.

`frontend.Compile`(단일 파일, 테스트·dump용)은 `(&Loader{Prelude: prelude, Stdlib: stdlib.Source}).Load(file, src)`의 얇은 포장으로 바꾸고 `Build.Proto = prog.Entry`를 돌려준다. (`Build.Program`도 노출.) dump는 진입 파일만 찍는다.

`vm/module.go`:

```go
// moduleState is one registered module: its compiled body, the globals
// it owns, and its value once the body has run (04: 처음 import될 때 한 번).
type moduleState struct {
	proto   *bytecode.Proto
	native  *value.Module
	globals []value.Value
	value   *value.Module
	loading bool
}

func (rt *Runtime) Register(key string, p *bytecode.Proto) {
	if rt.modules == nil { rt.modules = map[string]*moduleState{} }
	rt.modules[key] = &moduleState{proto: p}
}

func (rt *Runtime) RegisterNative(key string, m *value.Module) {
	if rt.modules == nil { rt.modules = map[string]*moduleState{} }
	rt.modules[key] = &moduleState{native: m, value: m}
}

// importModule returns a module's value, running its top level first
// when this is the first import. The body runs as a nested frame on this
// task, like a call, with its own globals.
func (t *Task) importModule(key string, pos diag.Pos) (value.Value, *diag.Error) {
	st, ok := t.rt.modules[key]
	if !ok {
		return nil, diag.Errorf("E0801", pos, "module `"+key+"` was not registered")
	}
	if st.value != nil {
		return st.value, nil
	}
	if st.loading {
		return nil, diag.Errorf("E0801", pos, "module `"+key+"` imports itself at run time")
	}
	st.loading = true
	st.globals = make([]value.Value, len(st.proto.GlobalNames))
	depth := len(t.frames)
	t.frames = append(t.frames, frame{proto: st.proto, base: len(t.stack), globals: st.globals})
	t.stack = append(t.stack, make([]value.Value, st.proto.NumLocals)...)
	if _, err := t.run(depth); err != nil {
		return nil, err
	}
	m := value.NewModule(moduleName(st.proto), key)
	for i, name := range st.proto.GlobalNames {
		if strings.HasPrefix(name, "_") || strings.HasPrefix(name, "$") || st.globals[i] == nil {
			continue
		}
		m.Set(name, st.globals[i])
	}
	st.value, st.loading = m, false
	return m, nil
}
```
모듈 Proto의 `Name`은 컴파일러가 `<module name>`으로 짓는다 — `compiler.Compile`에 이름을 넘기도록 `resolve.Result`에 `Name`을 두거나 로더가 `proto.Name`을 덮어쓴다 (로더가 덮어쓰는 쪽이 간단: `proto.Name = "<module " + name + ">"`). `moduleName`은 그 접두사를 벗긴다. 호출 경로에는 `<module fs>`로 보인다.

`vm.go`: `frame.globals []value.Value`; `Run`은 `globals := make([]value.Value, len(p.GlobalNames))`로 진입 프레임을 만든다 (Runtime의 `globals`/`globalNames` 필드 삭제, E0801 메시지의 이름은 `f.proto.GlobalNames[arg]`); `OpLoadGlobal`/`OpStoreGlobal`/`CaptureGlobal`은 `f.globals`; `OpClosure`는 `&bytecode.Closure{Proto, Captured, Globals: f.globals}`; `enter`의 클로저 프레임은 `globals: fn.Globals`; `OpImport`: `v, err := t.importModule(string(f.proto.Consts[in.Arg()].(value.Str)), pos)` → push. **주의:** `importModule`이 돌아온 뒤 `f`는 stale이다 — 루프는 매 명령마다 `f`를 다시 읽으므로 push 전에 `f = &t.frames[len(t.frames)-1]`를 다시 잡을 필요는 없지만, `execM4` 스타일로 분리하지 말고 `loop`의 switch 안에서 처리한다.
`RunExitHandlers`의 새 Task·`Call`은 그대로.

- [ ] **Step 4: 통과 확인** — `go test ./...` PASS.
- [ ] **Step 5: 커밋** `feat(frontend): load the import graph, embed the fung stdlib, and run modules with their own globals`

---

### Task 6: `fung` 명령과 골든 실행기 — 인자, 표준 입력, 종료 코드, 여러 파일의 에러, 지시어

**Files:**
- Modify: `cmd/fung/main.go`, `cmd/fung/pipeline.go`, `cmd/fung/main_test.go`, `internal/golden/golden.go`, `internal/golden/golden_test.go`, `tests/golden_test.go`
- Create: `tests/lang/_lib/greet.fung`, `tests/lang/import_basic.fung`, `tests/lang/import_names.fung`, `tests/lang/args_and_exit.fung`, `tests/lang/stdin.fung`, `tests/lang/err_import_missing.fung`, `tests/lang/err_import_private.fung`, `tests/lang/err_import_cycle.fung`(+ `_lib/cyc_a.fung`, `_lib/cyc_b.fung`), `tests/lang/err_import_top.fung`(+ `_lib/noisy.fung`)

**Interfaces:**
- `run(args, stdin, stdout, stderr)`: `fung <script> [args…]`, `fung run <script> [args…]`, `fung -e CODE [args…]`. 스크립트 인자는 prelude 값 `args`.
- `native.PreludeWith(args []string) map[string]value.Value`; `Prelude()`는 `PreludeWith(nil)`.
- 종료: 컴파일 실패 2, 런타임 에러 1, `exit(n)` n, `at_exit` 핸들러 실패는 1 (단, `exit(n)`의 n이 0이 아니면 n 유지).
- 에러 출력: `reportErrors(errs, sources map[string]string, stderr)` — `e.Pos.File`의 소스로 발췌. 종료 신호는 출력하지 않는다.
- 골든: `# args: a b c`, `# stdin: line`(여러 줄 가능, 순서대로, 각각 `\n` 붙임), `# expect-exit: n`. `_`로 시작하는 디렉터리는 건너뛴다.

- [ ] **Step 1: 테스트**

```go
// internal/golden/golden_test.go (추가)
func TestParseDirectives(t *testing.T) {
	c, err := Parse("t.fung", []byte("# args: a b\n# stdin: one\n# stdin: two\nprint(1)\n# expect: 1\n# expect-exit: 3\n"))
	if err != nil {
		t.Fatal(err)
	}
	if len(c.Args) != 2 || c.Args[0] != "a" || c.Args[1] != "b" || c.Stdin != "one\ntwo\n" || c.ExitCode != 3 {
		t.Fatalf("case = %+v", c)
	}
	if err := Check(c, Result{Stdout: "1\n", ExitCode: 3}); err != nil {
		t.Fatal(err)
	}
	if err := Check(c, Result{Stdout: "1\n", ExitCode: 0}); err == nil {
		t.Fatal("wrong exit code must fail")
	}
	if _, err := Parse("t.fung", []byte("# expect-exit: x\n")); err == nil {
		t.Fatal("malformed exit code")
	}
}
```

```go
// cmd/fung/main_test.go (추가)
func TestScriptArgsAndExit(t *testing.T) {
	out, errOut, code := invoke(t, "-e", "print(args)\nat_exit(() => print(\"bye\"))\nexit(3)\nprint(\"no\")", "x", "y")
	if code != 3 || out != "[\"x\", \"y\"]\nbye\n" || errOut != "" {
		t.Fatalf("code=%d out=%q err=%q", code, out, errOut)
	}
	_, _, code = invoke(t, "-e", "exit()")
	if code != 0 {
		t.Fatalf("exit() code = %d", code)
	}
	_, errOut, code = invoke(t, "-e", "at_exit(() => fail(\"boom\"))\nexit(0)")
	if code != 1 || !strings.Contains(errOut, "E0803") {
		t.Fatalf("failing handler after exit(0): code=%d err=%q", code, errOut)
	}
}

func TestStdinFlowsToReadLine(t *testing.T) {
	var out, errOut bytes.Buffer
	code := run([]string{"-e", "print(read_line())"}, strings.NewReader("hi\n"), &out, &errOut)
	if code != 0 || out.String() != "Ok(\"hi\")\n" {
		t.Fatalf("code=%d out=%q", code, out.String())
	}
}

func TestErrorsInImportedFilesShowTheirSource(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "lib.fung"), []byte("fn f() = 1 +\n"), 0o644)
	os.WriteFile(filepath.Join(dir, "main.fung"), []byte("import \"./lib\"\n"), 0o644)
	_, errOut, code := invoke(t, filepath.Join(dir, "main.fung"))
	if code != 2 || !strings.Contains(errOut, "lib.fung:1") || !strings.Contains(errOut, "fn f() = 1 +") {
		t.Fatalf("code=%d err=%q", code, errOut)
	}
}
```

골든 파일:

```
# tests/lang/_lib/greet.fung
_prefix = "hi"
fn greet(name) = "${_prefix}, ${name}"
fn _secret() = 1
version = 2
```
```
# tests/lang/import_basic.fung
import "./_lib/greet"
import "./_lib/greet" as g
print(greet.greet("a"), g.version, type_of(greet), greet == g)
# expect: hi, a 2 'module true
```
```
# tests/lang/import_names.fung
import "./_lib/greet" (greet, version)
print(greet("b"), version)
# expect: hi, b 2
```
```
# tests/lang/args_and_exit.fung
# args: one two
print(args, len(args))
# expect: ["one", "two"] 2
at_exit(() => print("bye"))
exit(4)
print("unreachable")
# expect: bye
# expect-exit: 4
```
(`# expect:` 줄은 파일 순서대로 모이므로 `bye`가 두 번째 줄.)
```
# tests/lang/stdin.fung
# stdin: first
# stdin: second
print(read_line(), read_lines(), read_line())
# expect: Ok("first") Ok(["second"]) Ok(nil)
```
```
# tests/lang/err_import_missing.fung
import "./_lib/nothing"
# expect-error: E0301
```
```
# tests/lang/err_import_private.fung
import "./_lib/greet"
print(greet._secret())
# expect-error: E0304
```
```
# tests/lang/_lib/cyc_a.fung
import "./cyc_b"
# tests/lang/_lib/cyc_b.fung
import "./cyc_a"
# tests/lang/err_import_cycle.fung
import "./_lib/cyc_a"
# expect-error: E0302
```
```
# tests/lang/_lib/noisy.fung
print("side effect")
# tests/lang/err_import_top.fung
import "./_lib/noisy"
# expect-error: E0306
```

- [ ] **Step 2: 실패 확인** — golden 패키지 컴파일 실패(`Args`), cmd 테스트 FAIL, 골든 FAIL.

- [ ] **Step 3: 구현**

golden: `Case`에 `Args []string`, `Stdin string`, `ExitCode int`; `Parse`가 `# args:`(`strings.Fields`), `# stdin:`(접두사 뒤 한 칸 제거, `\n` 추가), `# expect-exit:`(정수, `ErrorCode`와 함께 쓰면 에러). `Check`: 성공 케이스에서 `r.ExitCode != c.ExitCode`면 실패.
tests/golden_test.go: `cmd := exec.Command(fungBinary, append([]string{path}, c.Args...)...)`, `cmd.Stdin = strings.NewReader(c.Stdin)`; `findGoldenFiles`는 `d.IsDir() && strings.HasPrefix(d.Name(), "_")`이면 `fs.SkipDir`.

cmd:
- `run`: `case "run": return runFile(args[1:], stdin, stdout, stderr)`; `-e`: `execute("-e", args[1], args[2:], stdin, stdout, stderr)`; 스크립트 경로: `runFile(args, …)` → 경로 뒤 인자.
- `pipeline.go`의 `execute(file, src string, scriptArgs []string, stdin io.Reader, stdout, stderr io.Writer) int`:

```go
	prelude, rt, errs := buildRuntime(scriptArgs, stdin, stdout, stderr)   // Task 9 adds prelude.fung here; for now natives only
	loader := &frontend.Loader{Prelude: prelude, Natives: native.Modules(), Stdlib: stdlib.Source}
	prog, errs := loader.Load(file, src)
	if len(errs) > 0 {
		reportErrors(errs, mergeSources(prog.Sources, file, src), stderr)
		return exitCompileError
	}
	for _, m := range prog.Modules { register }
	code := exitOK
	if _, err := rt.Run(prog.Entry); err != nil {
		if n, ok := err.ExitCode(); ok {
			code = n
		} else {
			reportErrors([]*diag.Error{err}, prog.Sources, stderr)
			code = exitRuntimeError
		}
	}
	if errs := rt.RunExitHandlers(); len(errs) > 0 {
		reportErrors(errs, prog.Sources, stderr)
		if code == exitOK { code = exitRuntimeError }
	}
	return code
```
(`Program.Sources`는 실패해도 그때까지 읽은 소스를 담도록 `Load`가 항상 채워서 돌려준다.) `native.Modules()`는 Task 9 전까지 빈 맵을 돌려주는 자리.
- `reportErrors(errs, sources map[string]string, stderr)`: `diag.Format(e, sources[e.Pos.File])`.
- `at_exit` 핸들러 안의 `exit(n)`: `RunExitHandlers`가 돌려준 에러 중 종료 신호는 보고하지 않고 코드만 바꾼다 (`exit(0)` → 0).

- [ ] **Step 4: 통과 확인** — `go test ./...` PASS. 골든 84 + 9 = 93.
- [ ] **Step 5: 커밋** `feat(cmd): pass script arguments and stdin, honor exit codes, report errors from imported files, and extend golden directives`

---

### Task 7: native — 컬렉션 함수

**Files:**
- Create: `internal/native/collections.go`, `internal/native/collections_test.go`
- Modify: `internal/native/prelude.go` (등록)

**Interfaces:** 모두 prelude. 순회 대상은 리스트·범위 (`items(v, verb)` 도우미: `*List` → Items, `Range` → `Items()`, 그 외 E0506 `cannot <verb> <type>`). 람다 결과가 bool이어야 하는 곳은 E0505.

| 이름 | 시그니처 | 뜻 |
|------|------|------|
| `is_empty` | `(x)` | `len(x) == 0` (str/list/tuple/map/range) |
| `filter` | `(xs, f)` | `f(x)`가 true인 원소 |
| `fold` | `(xs, init, f)` | `f(acc, x)` 누적 |
| `each` | `(xs, f)` | 부수효과, `nil` |
| `sum` | `(xs)` | `+`로 합, 빈 리스트 `0` |
| `min` `max` | `(xs)` | `Compare`, 빈 리스트 `nil` |
| `sort` | `(xs)` | 안정 정렬, `Compare` 에러 전파 |
| `sort_by` | `(xs, f)` | `f(x)` 키로 안정 정렬 |
| `reverse` | `(xs)` | 리스트 |
| `range` | `(a, b = nil)` | `b`가 nil이면 `0..a`, 아니면 `a..b` |
| `zip` | `(xs, ys)` | 짧은 쪽 길이의 튜플 리스트 |
| `first` `last` | `(xs)` | 없으면 `nil` |
| `take` `drop` | `(xs, n)` | n을 `[0, len]`으로 자른다 |
| `find` | `(xs, f)` | 첫 원소 또는 `nil` |
| `any` `all` | `(xs, f)` | `all([])`은 true |
| `contains` | `(c, x)` | `value.Contains` |
| `unique` | `(xs)` | 처음 나온 것만, `Equal` |
| `flatten` | `(xs)` | 한 단계 |
| `group_by` | `(xs, f)` | 키는 str(E0514), 맵 (삽입 순서) |
| `keys` `values` `pairs` | `(m)` | 맵 → 리스트 (`pairs`는 `(k, v)` 튜플) |
| `get` | `(m, k, default = nil)` | 없으면 default |
| `put` | `(m, k, v)` | 사본 |
| `merge` | `(a, b)` | b가 덮어씀, 사본 |
| `remove` | `(m, k)` | 사본, 없어도 OK |
| `chars` | `(s)` | 한 글자 문자열의 리스트 |

- [ ] **Step 1: 테스트** — `internal/native/collections_test.go`는 **실제 VM**이 필요한 함수(람다를 부르는 것)가 많으므로 `internal/vm`의 `run` 도우미를 쓰는 테이블 테스트를 `internal/vm/prelude_test.go`에 둔다:

```go
// internal/vm/prelude_test.go
package vm

import "testing"

func TestCollectionFunctions(t *testing.T) {
	cases := map[string]string{
		"print(is_empty([]), is_empty(\"a\"), is_empty({}), is_empty(0..0))":              "true false true true\n",
		"print(filter([1, 2, 3, 4], x => x % 2 == 0), filter(0..5, x => x > 2))":          "[2, 4] [3, 4]\n",
		"print(fold([1, 2, 3], 0, (acc, x) => acc + x), fold([], 1, (a, x) => a))":        "6 1\n",
		"print(each([1, 2], x => print(x)))":                                                "1\n2\nnil\n",
		"print(sum([1, 2.5]), sum([]), min([3, 1, 2]), max([\"b\", \"a\"]), min([]))":      "3.5 0 1 b nil\n",
		"print(sort([3, 1, 2]), sort([\"b\", \"a\"]), sort_by([\"bb\", \"a\"], len))":      "[1, 2, 3] [\"a\", \"b\"] [\"a\", \"bb\"]\n",
		"xs = [1, 2]\nprint(reverse(xs), xs, range(3), range(1, 3))":                       "[2, 1] [1, 2] 0..3 1..3\n",
		"print(zip([1, 2, 3], [\"a\", \"b\"]), first([]), first([1]), last([1, 2]))":      "[(1, \"a\"), (2, \"b\")] nil 1 2\n",
		"print(take([1, 2, 3], 2), take([1], 5), drop([1, 2, 3], 1), drop([1], 5), take([1], -1))": "[1, 2] [1] [2, 3] [] []\n",
		"print(find([1, 2, 3], x => x > 1), find([], x => true), any([1, 2], x => x > 1), all([], x => false), all([1], x => x > 1))": "2 nil true true false\n",
		"print(contains([1, 2], 2), contains(\"abc\", \"b\"), contains({a: 1}, \"a\"), contains(0..3, 3))": "true true true false\n",
		"print(unique([1, 1.0, 2, 1]), flatten([[1], [2, [3]], 4]))":                      "[1, 2] [1, 2, [3], 4]\n",
		"print(group_by([\"ab\", \"c\", \"de\"], s => s[..1]))":                            "{a: [\"ab\"], c: [\"c\"], d: [\"de\"]}\n",
		"m = {b: 1, a: 2}\nprint(keys(m), values(m), pairs(m))":                           "[\"b\", \"a\"] [1, 2] [(\"b\", 1), (\"a\", 2)]\n",
		"m = {a: 1}\nprint(get(m, \"a\"), get(m, \"z\"), get(m, \"z\", 0), put(m, \"b\", 2), m)": "1 nil 0 {a: 1, b: 2} {a: 1}\n",
		"print(merge({a: 1, b: 1}, {b: 2, c: 3}), remove({a: 1, b: 2}, \"a\"), remove({}, \"x\"))": "{a: 1, b: 2, c: 3} {b: 2} {}\n",
		"print(chars(\"참치\"), map(0..2, x => x * x))":                                   "[\"참\", \"치\"] [0, 1]\n",
		"print([3, 1, 2] |> sort |> reverse |> first)":                                     "3\n",
	}
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
	errs := map[string]string{
		"filter(1, x => x)":          "E0506",
		"filter([1], x => 1)":        "E0505",
		"sort([1, \"a\"])":           "E0501",
		"min([1, \"a\"])":            "E0501",
		"fold(\"ab\", 0, (a, x) => a)": "E0506",
		"group_by([1], x => x)":      "E0514",
		"keys([])":                   "E0506",
		"put(1, \"a\", 2)":           "E0506",
		"get({}, 1)":                 "E0501",
		"chars(1)":                   "E0506",
		"sum([1, \"a\"])":            "E0501",
		"each([1], 1)":               "E0403",
	}
	for src, want := range errs {
		if _, _, code := run(t, src); code != want {
			t.Errorf("%q: code = %s, want %s", src, code, want)
		}
	}
}
```
(`group_by`의 키가 식별자가 아니면(`"2"`) 따옴표로 출력된다 — 02의 맵 출력 규칙(`isPlainKey`). Task 10의 골든이 그 경우를 덮는다.)

`internal/native/collections_test.go`에는 VM이 필요 없는 것만: `take`/`drop`의 잘라내기, `unique`, `flatten`, `merge`/`remove`가 사본을 만드는지(원본 불변).

- [ ] **Step 2: 실패 확인** — 미정의 이름 E0204로 FAIL.

- [ ] **Step 3: 구현** — `collections.go`. 도우미:

```go
// items lists what a collection function iterates: lists and ranges (02).
func items(v value.Value, verb string) ([]value.Value, *diag.Error) {
	switch x := v.(type) {
	case *value.List:
		return x.Items, nil
	case value.Range:
		return x.Items(), nil
	}
	return nil, diag.Errorf("E0506", diag.Pos{}, verb, v.TypeName())
}

// callBool runs a predicate and insists on a bool result (E0505).
func callBool(ctx value.Context, f, x value.Value) (bool, error) {
	r, err := ctx.Call(f, []value.Value{x})
	if err != nil {
		return false, err
	}
	b, ok := r.(value.Bool)
	if !ok {
		return false, diag.Errorf("E0505", diag.Pos{}, r.TypeName())
	}
	return bool(b), nil
}

func needFn(name string, v value.Value) error {
	if _, ok := v.(value.Fn); !ok {
		return diag.Errorf("E0403", diag.Pos{}, name, "fn", v.TypeName())
	}
	return nil
}

func needMap(verb string, v value.Value) (*value.Map, error) {
	m, ok := v.(*value.Map)
	if !ok {
		return nil, diag.Errorf("E0506", diag.Pos{}, verb, v.TypeName())
	}
	return m, nil
}
```
`sort`: `sort.SliceStable`의 less 안에서 `value.Compare`가 에러를 내면 첫 에러를 변수에 담고 false를 돌려준 뒤 끝나고 나서 에러를 반환. `sort_by`: 키를 먼저 전부 계산(`ctx.Call`)한 뒤 같은 방식. `put`/`merge`/`remove`: `value.NewMap()`에 복사(`Keys()` 순서). `range`: `value.Range{Start: 0, End: a}` 또는 `{a, b}`, int가 아니면 E0403. 등록: `add(def("filter", filterFn, param("xs"), param("f")))` 등 표대로.

- [ ] **Step 4: 통과 확인** — `go test ./...` PASS.
- [ ] **Step 5: 커밋** `feat(native): add the collection functions of the prelude`

---

### Task 8: native — 문자열·변환·결과값·프로세스

**Files:**
- Create: `internal/native/strings.go`, `internal/native/convert.go`, `internal/native/process.go`, `internal/native/strings_test.go`, `internal/native/convert_test.go`
- Modify: `internal/native/prelude.go`(등록, `PreludeWith(args)`), `internal/native/io.go`(없음 — Task 2에서 끝)
- Test: `internal/vm/prelude_test.go`(추가)

**Interfaces:**

| 이름 | 시그니처 | 뜻 |
|------|------|------|
| `split` | `(s, sep)` | `strings.Split`; `sep == ""`는 글자 단위 |
| `join` | `(xs, sep = "")` | 원소는 `FormatForPrint` |
| `trim` | `(s, cut = nil)` | nil → 공백, 아니면 그 글자들 (`strings.Trim`) |
| `upper` `lower` | `(s)` | |
| `replace` | `(s, old, new)` | 전부 |
| `starts_with` `ends_with` | `(s, p)` | |
| `pad_left` `pad_right` | `(s, n, pad = " ")` | 룬 길이 n까지 |
| `repeat` | `(s, n)` | n < 0 → E0506 `cannot repeat -1 times`? → 0으로 본다 |
| `int` | `(x)` | int 그대로, float 버림(Inf/NaN은 E0513), str 파싱(`_` 허용, 앞뒤 공백 불가), 그 외 E0513 |
| `float` | `(x)` | int→float, float, str 파싱, 그 외 E0513 |
| `str` | `(x)` | str 그대로, 나머지 `Format` |
| `sym` | `(s)` | str → `Intern`; sym 그대로; 그 외 E0513 |
| `parse_int` `parse_float` | `(s)` | `Ok(n)` / `Err({kind: 'parse, message: "not an integer: ${s}"})`; str이 아니면 E0403 |
| `is_ok` `is_err` | `(x)` | `Ok(…)`/`Err(…)` 태그인가 |
| `exec` | `(cmd, cwd = nil, env = nil, stdin = nil)` | 아래 |
| `sh` | `(cmd, cwd = nil, env = nil, stdin = nil)` | `exec(["/bin/sh", "-c", cmd], …)` |
| `env` | `(name = nil)` | nil → 전체 맵, 아니면 str 또는 nil |
| `args` | 값 | `PreludeWith(args)`가 만드는 문자열 리스트 |

`exec`: `cmd`는 비어 있지 않은 str 리스트(아니면 E0403 `argument cmd must be a list of strings`… 메시지 형식은 E0403 `argument `cmd` must be list, got int`). `cwd` str|nil, `env` map(값 str)|nil → `os.Environ()` 위에 덮어씀, `stdin` str|nil. 결과: 종료 코드 0 → `Ok({out, err, code})`; 0이 아니면 `Err({kind: 'exit, code, out, err, message: "`mv` exited with code 1"})`; 실행 자체 실패(없는 명령) → `Err({kind: 'not_found, message: "cannot run `nope`: …"})`. `out`·`err`는 문자열 그대로(끝 줄바꿈 유지).

- [ ] **Step 1: 테스트**

```go
// internal/native/strings_test.go — fake ctx로 직접 호출 (VM 불필요)
func call(t *testing.T, name string, args ...value.Value) (value.Value, error) {
	t.Helper()
	n := Prelude()[name].(*value.Native)
	return n.Fn(&ctx{}, args)
}

func TestStringFunctions(t *testing.T) {
	cases := []struct {
		name string
		args []value.Value
		want string
	}{
		{"split", []value.Value{value.Str("a,b,,c"), value.Str(",")}, `["a", "b", "", "c"]`},
		{"split", []value.Value{value.Str("참치"), value.Str("")}, `["참", "치"]`},
		{"join", []value.Value{list(value.Str("a"), value.Int(1)), value.Str("-")}, `"a-1"`},
		{"trim", []value.Value{value.Str("  a \n"), value.Nil{}}, `"a"`},
		{"trim", []value.Value{value.Str("xxaxx"), value.Str("x")}, `"a"`},
		{"upper", []value.Value{value.Str("aé")}, `"AÉ"`},
		{"lower", []value.Value{value.Str("AÉ")}, `"aé"`},
		{"replace", []value.Value{value.Str("a-b-c"), value.Str("-"), value.Str("+")}, `"a+b+c"`},
		{"starts_with", []value.Value{value.Str("abc"), value.Str("ab")}, "true"},
		{"ends_with", []value.Value{value.Str("abc"), value.Str("ab")}, "false"},
		{"pad_left", []value.Value{value.Str("7"), value.Int(3), value.Str("0")}, `"007"`},
		{"pad_right", []value.Value{value.Str("참"), value.Int(3), value.Str(" ")}, `"참  "`},
		{"pad_left", []value.Value{value.Str("long"), value.Int(2), value.Str(" ")}, `"long"`},
		{"repeat", []value.Value{value.Str("ab"), value.Int(2)}, `"abab"`},
		{"repeat", []value.Value{value.Str("ab"), value.Int(-1)}, `""`},
	}
	for _, c := range cases {
		got, err := call(t, c.name, c.args...)
		if err != nil || value.Format(got) != c.want {
			t.Errorf("%s(%v) = %v %v, want %s", c.name, c.args, got, err, c.want)
		}
	}
	if _, err := call(t, "upper", value.Int(1)); err == nil || err.(*diag.Error).Code != "E0403" {
		t.Errorf("upper(1): %v", err)
	}
}
```
(`list(...)` 도우미는 테스트 파일 안에서 정의.)

```go
// internal/native/convert_test.go
func TestConversions(t *testing.T) {
	cases := []struct {
		name string
		arg  value.Value
		want string
	}{
		{"int", value.Int(3), "3"}, {"int", value.Float(3.9), "3"}, {"int", value.Float(-3.9), "-3"},
		{"int", value.Str("42"), "42"}, {"int", value.Str("-1_000"), "-1000"},
		{"float", value.Int(2), "2.0"}, {"float", value.Str("2.5"), "2.5"}, {"float", value.Str("1e3"), "1000.0"},
		{"str", value.Str("a"), `"a"`}, {"str", value.Int(1), `"1"`}, {"str", value.Float(1), `"1.0"`}, {"str", value.Intern("ok"), `"'ok"`}, {"str", list(value.Int(1)), `"[1]"`},
		{"sym", value.Str("ok"), "'ok"}, {"sym", value.Intern("x"), "'x"},
		{"parse_int", value.Str("12"), "Ok(12)"}, {"parse_int", value.Str("x"), `Err({kind: 'parse, message: "not an integer: x"})`},
		{"parse_float", value.Str("1.5"), "Ok(1.5)"}, {"parse_float", value.Str("x"), `Err({kind: 'parse, message: "not a number: x"})`},
		{"is_ok", ok(value.Int(1)), "true"}, {"is_ok", errValue("x", "m"), "false"}, {"is_ok", value.Int(1), "false"},
		{"is_err", errValue("x", "m"), "true"},
	}
	for _, c := range cases {
		got, err := call(t, c.name, c.arg)
		if err != nil || value.Format(got) != c.want {
			t.Errorf("%s(%s) = %v %v, want %s", c.name, value.Format(c.arg), got, err, c.want)
		}
	}
	errs := []struct {
		name string
		arg  value.Value
	}{
		{"int", value.Str("x")}, {"int", value.Str(" 1")}, {"int", value.Bool(true)}, {"int", value.Float(math.Inf(1))},
		{"float", value.Str("abc")}, {"sym", value.Int(1)},
	}
	for _, c := range errs {
		if _, err := call(t, c.name, c.arg); err == nil || err.(*diag.Error).Code != "E0513" {
			t.Errorf("%s(%s): %v, want E0513", c.name, value.Format(c.arg), err)
		}
	}
	if _, err := call(t, "parse_int", value.Int(1)); err == nil || err.(*diag.Error).Code != "E0403" {
		t.Errorf("parse_int(1): %v", err)
	}
}
```

```go
// internal/vm/prelude_test.go (추가)
func TestProcessFunctions(t *testing.T) {
	cases := map[string]string{
		"print(exec([\"echo\", \"hi there\"]))":                                   "Ok({out: \"hi there\\n\", err: \"\", code: 0})\n",
		"print(sh(\"echo a; echo b >&2; exit 3\"))":                              "Err({kind: 'exit, code: 3, out: \"a\\n\", err: \"b\\n\", message: \"`/bin/sh` exited with code 3\"})\n",
		"print(exec([\"cat\"], stdin: \"x\")!.out)":                              "x\n",
		"print(exec([\"sh\", \"-c\", \"echo $FOO\"], env: {FOO: \"bar\"})!.out)": "bar\n",
		"print(exec([\"pwd\"], cwd: \"/\")!.out)":                                "/\n",
		"match exec([\"definitely-not-a-command-xyz\"]): Err({kind: k}) => print(k); _ => print(\"?\") ;;": "'not_found\n",
		"print(env(\"FUNG_TEST_UNSET_VAR\"), type_of(env()))":                   "nil 'map\n",
		"print(args)":                                                           "[]\n",
	}
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
	errs := map[string]string{
		"exec([])":           "E0403",
		"exec([1])":          "E0403",
		"exec(\"ls\")":       "E0403",
		"exec([\"ls\"], cwd: 1)": "E0403",
	}
	for src, want := range errs {
		if _, _, code := run(t, src); code != want {
			t.Errorf("%q: code = %s, want %s", src, code, want)
		}
	}
}
```
(이 테스트는 `echo`·`sh`·`cat`·`pwd`가 있는 유닉스 환경을 전제한다. `runtime.GOOS == "windows"`면 `t.Skip`.)

- [ ] **Step 2: 실패 확인.**
- [ ] **Step 3: 구현** — 표대로. `exec`의 핵심:

```go
func execFn(_ value.Context, args []value.Value) (value.Value, error) {
	argv, err := strList("cmd", args[0])
	if err != nil {
		return nil, err
	}
	if len(argv) == 0 {
		return nil, diag.Errorf("E0403", diag.Pos{}, "cmd", "a non-empty list of strings", "an empty list")
	}
	cmd := exec.Command(argv[0], argv[1:]...)
	if s, ok := args[1].(value.Str); ok {
		cmd.Dir = string(s)
	} else if _, isNil := args[1].(value.Nil); !isNil {
		return nil, diag.Errorf("E0403", diag.Pos{}, "cwd", "str | nil", args[1].TypeName())
	}
	// env: map of str → str layered over the process environment
	// stdin: str | nil
	var out, errOut bytes.Buffer
	cmd.Stdout, cmd.Stderr = &out, &errOut
	runErr := cmd.Run()
	result := func(code int) *value.Map { m := value.NewMap(); m.Set("out", value.Str(out.String())); m.Set("err", value.Str(errOut.String())); m.Set("code", value.Int(code)); return m }
	var exitErr *exec.ExitError
	switch {
	case runErr == nil:
		return ok(result(0)), nil
	case errors.As(runErr, &exitErr):
		code := exitErr.ExitCode()
		m := result(code)
		m.Set("message", value.Str(fmt.Sprintf("`%s` exited with code %d", argv[0], code)))
		e := value.NewMap(); e.Set("kind", value.Intern("exit")); for _, k := range m.Keys() { v, _ := m.Get(k); e.Set(k, v) }
		return &value.Tag{Name: value.Intern("Err"), Args: []value.Value{e}}, nil
	default:
		return errValue("not_found", fmt.Sprintf("cannot run `%s`: %s", argv[0], reason(runErr))), nil
	}
}
```
`Err` 맵의 키 순서는 테스트가 기대하는 `kind, code, out, err, message`로 맞춘다 (`e.Set` 순서). `reason(err)`은 `*exec.Error`/`*os.PathError`의 안쪽 에러 문장만 꺼낸다 (Go 접두사 제거).

- [ ] **Step 4: 통과 확인** — `go test ./...` PASS.
- [ ] **Step 5: 커밋** `feat(native): add string, conversion, result, and process functions`

---

### Task 9: `_native.fs`·`_native.path`, fung 표준 라이브러리, prelude.fung

**Files:**
- Create: `internal/native/fs.go`, `internal/native/path.go`, `internal/native/modules.go`, `internal/native/fs_test.go`, `internal/stdlib/fs.fung`, `internal/stdlib/path.fung`, `internal/stdlib/prelude.fung`
- Delete: `internal/stdlib/empty.fung`
- Modify: `cmd/fung/pipeline.go`(prelude.fung 적재), `internal/vm/prelude_test.go`(추가)

**Interfaces:**
- `native.Modules() map[string]*value.Module` — `_native.fs`, `_native.path`.
- `_native.fs` (전부 `Ok`/`Err`, Err는 `{kind, message}`; kind: `'not_found` `'exists` `'permission` `'io` `'closed`):
  `read(p)`→`Ok(str)`, `write(p, text)`→`Ok(nil)`, `append(p, text)`, `list(dir)`→`Ok([이름…])` 정렬, `glob(pattern)`→`Ok([경로…])`, `walk(dir)`→`Ok([파일 경로…])` 파일만·정렬·`dir`에 상대가 아닌 `dir`를 붙인 경로, `exists(p)`→bool, `stat(p)`→`Ok({name, size, is_dir})`, `mkdir(p)`(MkdirAll), `remove(p)`(os.Remove), `remove_all(p)`, `move(a, b)`, `copy(a, b)`(파일), `open(p, mode)`→`Ok(file)` (mode `'read` `'write` `'append`, 그 외 E0403), `close(f)`→`Ok(nil)` (닫힌 핸들에 다시 → `Err 'closed`), `write_line(f, s)`→`Ok(nil)`, `temp_dir()`→`Ok(path)`.
- `_native.path`: `join(...parts)`, `base(p)`, `dir(p)`, `ext(p)`(점 포함, 없으면 `""`), `stem(p)`, `abs(p)`. 모두 str 반환, str 아닌 인자 E0403.
- `fs.fung`:

```
import _native.fs as _n

read = _n.read
write = _n.write
append = _n.append
list = _n.list
glob = _n.glob
walk = _n.walk
exists = _n.exists
stat = _n.stat
mkdir = _n.mkdir
remove = _n.remove
remove_all = _n.remove_all
move = _n.move
copy = _n.copy
open = _n.open
close = _n.close
write_line = _n.write_line

# Opens path, calls f with the handle, and always closes it (03: defer 대신
# 감싸는 함수). f's result — a value or the Err that `?` returned — is
# passed through.
fn with_open(path, f, mode = 'read):
  h = _n.open(path, mode)?
  result = f(h)
  _n.close(h)!
  result
;;

fn with_temp_dir(f):
  dir = _n.temp_dir()?
  result = f(dir)
  _n.remove_all(dir)!
  result
;;
```
- `path.fung`: `import _native.path as _n` 뒤 `join = _n.join` … 여섯 줄.
- `prelude.fung`:

```
# Iteration patterns written in fung (06). retry comes with sleep in M7.
fn until(x, f, pred) = pred(x) then x else until(f(x), f, pred)
fn iterate(x, f, n) = n <= 0 then x else iterate(f(x), f, n - 1)
```
- `cmd/fung/pipeline.go`의 `buildRuntime`: `prelude := native.PreludeWith(args)`; `rt := &vm.Runtime{…}`; `Loader{Prelude: prelude, Natives: native.Modules(), Stdlib: stdlib.Source}.Load("prelude", stdlib.Source("prelude"))`로 컴파일하고 `rt.Register` 뒤 `rt.ImportModule("prelude")`(새 공개 메서드: 새 Task에서 `importModule`)로 실행해 모듈 값의 멤버를 `prelude` 표에 넣는다. 그 뒤 진입 파일을 같은 `prelude`로 적재. (`frontend.Compile`도 같은 일을 하도록 `frontend.DefaultPrelude(args)`에 모아 두면 `dump`·테스트가 공유한다.)

- [ ] **Step 1: 테스트**

```go
// internal/native/fs_test.go — _native.fs를 fake ctx로 직접
func fsFn(t *testing.T, name string) *value.Native {
	t.Helper()
	v, ok := Modules()["_native.fs"].Get(name)
	if !ok {
		t.Fatalf("no %s", name)
	}
	return v.(*value.Native)
}

func TestFsReadWriteListStat(t *testing.T) {
	dir := t.TempDir()
	p := filepath.Join(dir, "a.txt")
	c := &ctx{}
	if r, _ := fsFn(t, "write").Fn(c, []value.Value{value.Str(p), value.Str("hi\n")}); value.Format(r) != "Ok(nil)" {
		t.Fatalf("write: %s", value.Format(r))
	}
	fsFn(t, "append").Fn(c, []value.Value{value.Str(p), value.Str("there\n")})
	if r, _ := fsFn(t, "read").Fn(c, []value.Value{value.Str(p)}); value.Format(r) != `Ok("hi\nthere\n")` {
		t.Fatalf("read: %s", value.Format(r))
	}
	r, _ := fsFn(t, "read").Fn(c, []value.Value{value.Str(filepath.Join(dir, "none"))})
	if !strings.HasPrefix(value.Format(r), "Err({kind: 'not_found, message: \"cannot read `") {
		t.Fatalf("missing: %s", value.Format(r))
	}
	fsFn(t, "mkdir").Fn(c, []value.Value{value.Str(filepath.Join(dir, "sub/deep"))})
	fsFn(t, "write").Fn(c, []value.Value{value.Str(filepath.Join(dir, "sub/deep/b.txt")), value.Str("")})
	if r, _ := fsFn(t, "list").Fn(c, []value.Value{value.Str(dir)}); value.Format(r) != `Ok(["a.txt", "sub"])` {
		t.Fatalf("list: %s", value.Format(r))
	}
	if r, _ := fsFn(t, "walk").Fn(c, []value.Value{value.Str(dir)}); value.Format(r) != `Ok(["`+p+`", "`+filepath.Join(dir, "sub/deep/b.txt")+`"])` {
		t.Fatalf("walk: %s", value.Format(r))
	}
	if r, _ := fsFn(t, "glob").Fn(c, []value.Value{value.Str(filepath.Join(dir, "*.txt"))}); value.Format(r) != `Ok(["`+p+`"])` {
		t.Fatalf("glob: %s", value.Format(r))
	}
	if r, _ := fsFn(t, "stat").Fn(c, []value.Value{value.Str(p)}); value.Format(r) != `Ok({name: "a.txt", size: 9, is_dir: false})` {
		t.Fatalf("stat: %s", value.Format(r))
	}
	if r, _ := fsFn(t, "exists").Fn(c, []value.Value{value.Str(p)}); r != value.Bool(true) {
		t.Fatal("exists")
	}
	fsFn(t, "copy").Fn(c, []value.Value{value.Str(p), value.Str(filepath.Join(dir, "c.txt"))})
	fsFn(t, "move").Fn(c, []value.Value{value.Str(filepath.Join(dir, "c.txt")), value.Str(filepath.Join(dir, "d.txt"))})
	if r, _ := fsFn(t, "read").Fn(c, []value.Value{value.Str(filepath.Join(dir, "d.txt"))}); value.Format(r) != `Ok("hi\nthere\n")` {
		t.Fatalf("copy+move: %s", value.Format(r))
	}
	if r, _ := fsFn(t, "remove").Fn(c, []value.Value{value.Str(filepath.Join(dir, "sub"))}); !strings.HasPrefix(value.Format(r), "Err(") {
		t.Fatal("remove on a non-empty dir must fail")
	}
	fsFn(t, "remove_all").Fn(c, []value.Value{value.Str(filepath.Join(dir, "sub"))})
	if r, _ := fsFn(t, "exists").Fn(c, []value.Value{value.Str(filepath.Join(dir, "sub"))}); r != value.Bool(false) {
		t.Fatal("remove_all")
	}
}

func TestFsHandles(t *testing.T) {
	dir := t.TempDir()
	p := filepath.Join(dir, "h.txt")
	c := &ctx{}
	r, _ := fsFn(t, "open").Fn(c, []value.Value{value.Str(p), value.Intern("write")})
	h := r.(*value.Tag).Args[0].(*value.File)
	fsFn(t, "write_line").Fn(c, []value.Value{h, value.Str("one")})
	fsFn(t, "write_line").Fn(c, []value.Value{h, value.Str("two")})
	if r, _ := fsFn(t, "close").Fn(c, []value.Value{h}); value.Format(r) != "Ok(nil)" {
		t.Fatal("close")
	}
	if r, _ := fsFn(t, "write_line").Fn(c, []value.Value{h, value.Str("x")}); !strings.Contains(value.Format(r), "'closed") {
		t.Fatalf("write after close: %s", value.Format(r))
	}
	if r, _ := fsFn(t, "close").Fn(c, []value.Value{h}); !strings.Contains(value.Format(r), "'closed") {
		t.Fatalf("double close: %s", value.Format(r))
	}
	r, _ = fsFn(t, "open").Fn(c, []value.Value{value.Str(p), value.Intern("read")})
	h = r.(*value.Tag).Args[0].(*value.File)
	rl := Prelude()["read_line"].(*value.Native)
	if r, _ := rl.Fn(c, []value.Value{h}); value.Format(r) != `Ok("one")` {
		t.Fatalf("read_line: %s", value.Format(r))
	}
	if r, _ := Prelude()["read_lines"].(*value.Native).Fn(c, []value.Value{h}); value.Format(r) != `Ok(["two"])` {
		t.Fatalf("read_lines: %s", value.Format(r))
	}
	fsFn(t, "close").Fn(c, []value.Value{h})
	if r, _ := rl.Fn(c, []value.Value{h}); !strings.Contains(value.Format(r), "'closed") {
		t.Fatalf("read after close: %s", value.Format(r))
	}
	if _, err := fsFn(t, "open").Fn(c, []value.Value{value.Str(p), value.Intern("rw")}); err == nil {
		t.Fatal("bad mode is E0403")
	}
	if r, _ := fsFn(t, "open").Fn(c, []value.Value{value.Str(filepath.Join(dir, "nope/x")), value.Intern("read")}); !strings.Contains(value.Format(r), "'not_found") {
		t.Fatalf("open missing: %s", value.Format(r))
	}
}

func TestPathFunctions(t *testing.T) {
	m := Modules()["_native.path"]
	cases := []struct{ name string; args []value.Value; want string }{
		{"join", []value.Value{list(value.Str("a"), value.Str("b/c"))}, `"a/b/c"`},
		{"base", []value.Value{value.Str("/x/y.txt")}, `"y.txt"`},
		{"dir", []value.Value{value.Str("/x/y.txt")}, `"/x"`},
		{"ext", []value.Value{value.Str("a/b.tar.gz")}, `".gz"`},
		{"ext", []value.Value{value.Str("a/b")}, `""`},
		{"stem", []value.Value{value.Str("a/b.tar.gz")}, `"b.tar"`},
	}
	for _, c := range cases {
		v, _ := m.Get(c.name)
		got, err := v.(*value.Native).Fn(&ctx{}, c.args)
		if err != nil || value.Format(got) != c.want {
			t.Errorf("%s = %v %v, want %s", c.name, got, err, c.want)
		}
	}
}
```
(`join`은 `...parts` rest라 Go 쪽은 `args[0].(*List)`를 받는다.)

```go
// internal/vm/prelude_test.go (추가) — fung 층: with_open, with_temp_dir, until, iterate
func TestFungStdlib(t *testing.T) {
	cases := map[string]string{
		"import fs\nprint(fs.with_temp_dir(d => fs.exists(d)))":                                                   "true\n",
		"import fs\nimport path\nfs.with_temp_dir(fn(d):\n  p = path.join(d, \"x.txt\")\n  fs.write(p, \"a\\nb\\n\")!\n  r = fs.with_open(p, fn(h):\n    first = read_line(h)?\n    first\n  ;;)\n  print(r)\n  nil\n;;)": "Ok(\"a\")\n",
		"import fs\nprint(fs.with_open(\"/nonexistent/x\", h => 1))":                                               "", // see below: Err passes through
		"import fs\nkept = fs.with_temp_dir(fn(d):\n  p = d + \"/f\"\n  fs.write(p, \"\")!\n  fs.with_open(p, h => h)\n;;)\nprint(read_line(kept!))": "Err({kind: 'closed, message: \"file `\" + \"…\"})\n",
		"print(until(1, x => x * 2, x => x > 10), iterate(1, x => x * 2, 3))":                                      "16 8\n",
	}
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" {
			t.Errorf("%q: code = %s", src, code)
			continue
		}
		switch {
		case strings.Contains(src, "/nonexistent/"):
			if !strings.HasPrefix(out, "Err({kind: 'not_found") {
				t.Errorf("%q: out = %q", src, out)
			}
		case strings.Contains(src, "kept"):
			if !strings.HasPrefix(out, "Err({kind: 'closed") {
				t.Errorf("%q: out = %q", src, out)
			}
		default:
			if out != want {
				t.Errorf("%q: out=%q, want %q", src, out, want)
			}
		}
	}
}
```
세 번째 케이스는 `Err({kind: 'not_found, message: …})`가 그대로 출력되므로 `strings.HasPrefix(out, "Err({kind: 'not_found")`로 검사하고, 네 번째는 메시지에 임시 경로가 들어가므로 `strings.HasPrefix(out, "Err({kind: 'closed")`로 검사한다 (표의 문자열은 자리표시). 이 테스트의 `run` 도우미는 **stdlib이 있는 로더**를 써야 한다 → `compileWith`를 `frontend.Compile`(이미 stdlib을 아는 로더 포장)로 두고, `Runtime`에 `prog.Modules`를 등록하는 `runProgram` 경로를 쓴다. `until`·`iterate`는 prelude.fung이므로 `frontend.DefaultPrelude(nil)`를 쓰는 `run`이어야 한다 — `vm_test.go`의 `run`을 그렇게 바꾼다 (prelude.fung을 실행할 Runtime이 필요하므로 `DefaultPrelude(args, rt)`가 그 Runtime에 모듈을 등록·실행).

- [ ] **Step 2: 실패 확인.**
- [ ] **Step 3: 구현** — `fs.go`의 에러 변환:

```go
// fsErr turns an OS error into Err({kind, message}) (06: 외부 요인 실패).
func fsErr(verb, path string, err error) value.Value {
	kind := "io"
	switch {
	case errors.Is(err, fs.ErrNotExist):
		kind = "not_found"
	case errors.Is(err, fs.ErrExist):
		kind = "exists"
	case errors.Is(err, fs.ErrPermission):
		kind = "permission"
	}
	return errValue(kind, fmt.Sprintf("cannot %s `%s`: %s", verb, path, reason(err)))
}
```
`reason`은 `*os.PathError`/`*os.LinkError`의 `.Err.Error()` (`no such file or directory`), 그 외 `err.Error()`. `open`: `'read` → `os.Open` + `bufio.NewReader`; `'write` → `os.Create` + `bufio.NewWriter`; `'append` → `OpenFile(O_APPEND|O_CREATE|O_WRONLY)`. `close`: `Writer != nil`이면 `Flush` 먼저; 에러는 `fsErr("close", …)`; 닫힌 뒤 → `errValue("closed", "file `p` is closed")`. `write_line`: `Writer == nil`(읽기 전용)이면 `'closed`가 아니라 `Err({kind: 'invalid, message: "file `p` is not open for writing"})`.
`modules.go`: `Modules()`가 두 `*value.Module`을 만들어 `Set`으로 natives를 넣는다 (매 호출 새로 — Runtime마다 독립).

- [ ] **Step 4: 통과 확인** — `go test ./...` PASS.
- [ ] **Step 5: 커밋** `feat(stdlib): add fs and path modules, file handles with with_open, and the fung part of the prelude`

---

### Task 10: 예제 스크립트, 골든, 노트, 문서

**Files:**
- Create: `examples/rename.fung`, `tests/lang/collections.fung`, `tests/lang/strings.fung`, `tests/lang/convert.fung`, `tests/lang/results_fns.fung`, `tests/lang/fs_and_path.fung`, `tests/lang/process.fung`, `tests/lang/stdlib_fns.fung`, `tests/lang/underscore_assign.fung`, `tests/lang/err_dup_key.fung`, `tests/lang/err_dup_pattern_name.fung`, `tests/lang/err_convert.fung`, `tests/lang/err_group_key.fung`, `tests/lang/err_shadow_module.fung`, `tests/lang/err_import_twice.fung`, `tests/lang/err_import_in_fn.fung`, `tests/lang/err_native_import.fung`, `docs/notes/M5-modules-and-stdlib.md`
- Modify: `docs/design/09-implementation-plan.md`(M5 행 ` — 완료`), `CLAUDE.md`(상태: M5 완료, 다음 M6), `README.md`가 있으면 사용법 한 줄

`examples/rename.fung`:

```
#!/usr/bin/env fung
# Renames every file in a directory to kebab-case:
#   "My Report (final).TXT" -> "my-report-final.txt"
# usage: fung examples/rename.fung <dir> [--dry-run]
import fs
import path

# Lowercase, keep letters and digits, turn everything else into "-",
# squeeze repeats, trim the ends.
fn kebab(name):
  kept = for c in chars(lower(name)): c in "abcdefghijklmnopqrstuvwxyz0123456789" then c else "-" ;;
  join(kept) |> split("-") |> filter(p => p != "") |> join("-")
;;

# The new name for one entry; names that would vanish (no ASCII letters at
# all) are kept as they are.
fn target(file):
  stem = kebab(path.stem(file))
  stem == "" then file else stem + lower(path.ext(file))
;;

fn rename_all(dir, dry):
  files = fs.list(dir)!
  each(files, fn(file):
    to = target(file)
    if to != file:
      print(file, "->", to)
      if not dry: fs.move(path.join(dir, file), path.join(dir, to))! ;;
    ;;
  ;;)
;;

dir = first(args) ?? fail("usage: rename.fung <dir> [--dry-run]")
rename_all(dir, "--dry-run" in args)
```

골든 (요점만; 각 파일은 `# expect:`로 결과를 고정한다):

```
# tests/lang/collections.fung
print([3, 1, 2] |> sort, [1, 2, 3] |> filter(x => x > 1) |> map(x => x * 10), fold(1..=4, 0, (a, x) => a + x))
# expect: [1, 2, 3] [20, 30] 10
print(take([1, 2, 3], 2), drop([1, 2, 3], 2), first([]), zip([1], ["a"]), unique([1, 1]), flatten([[1], [2]]))
# expect: [1, 2] [3] nil [(1, "a")] [1] [1, 2]
print(group_by(["aa", "b", "cc"], s => str(len(s))), pairs({a: 1}), get({}, "x", 0), put({}, "k", 1))
# expect: {"2": ["aa", "cc"], "1": ["b"]} [("a", 1)] 0 {k: 1}
each([1, 2], x => print(x))
# expect: 1
# expect: 2
```
```
# tests/lang/strings.fung
print(split("a,b", ","), join([1, "x"], "-"), trim("  a "), upper("ab"), replace("aXa", "X", "-"))
# expect: ["a", "b"] 1-x a AB a-a
print(pad_left("7", 3, "0"), repeat("ab", 2), starts_with("abc", "a"), chars("참치") |> reverse |> join)
# expect: 007 abab true 치참
```
```
# tests/lang/convert.fung
print(int("42") + 1, int(3.9), float(2), str(1.5), sym("ok"), parse_int("x"), parse_int("7")!)
# expect: 43 3 2.0 1.5 'ok Err({kind: 'parse, message: "not an integer: x"}) 7
print(is_ok(Ok(1)), is_err(Ok(1)), is_err(Err(1)))
# expect: true false true
```
```
# tests/lang/results_fns.fung
fn parse_port(s) = parse_int(s)? |> (n => n > 0 and n < 65536 then Ok(n) else Err({kind: 'invalid, message: "port out of range: ${n}"}))
print(parse_port("80"), parse_port("x") |> is_err, parse_port("70000"))
# expect: Ok(80) true Err({kind: 'invalid, message: "port out of range: 70000"})
```
(`parse_int(s)? |> (…)`: `?`가 파이프보다 먼저 묶인다 — 07의 우선순위. 안 되면 `n = parse_int(s)?` 두 줄로 쓴다.)
```
# tests/lang/fs_and_path.fung
import fs
import path
fs.with_temp_dir(fn(d):
  p = path.join(d, "notes.txt")
  fs.write(p, "one\ntwo\n")!
  print(fs.read(p)!, fs.exists(p), path.base(p), path.ext(p), path.stem(p))
  lines = fs.with_open(p, h => read_lines(h)!)
  print(lines, fs.list(d)!)
  print(fs.read(path.join(d, "missing")) |> is_err)
  nil
;;)
# expect: one
# expect: two
# expect:  true notes.txt .txt notes
# expect: ["one", "two"] ["notes.txt"]
# expect: true
```
(첫 `print`의 첫 인자가 두 줄짜리 문자열이라 기대 출력이 세 줄로 갈라진다: `one` / `two` / ` true …`(빈 문자열 뒤 공백으로 시작). 골든 `# expect:`는 접두사 뒤 **한 칸만** 떼므로 ` true`는 `# expect:  true`로 쓴다.)
```
# tests/lang/process.fung
print(exec(["echo", "hi"])!.out, sh("exit 2") |> is_err, env("FUNG_SURELY_UNSET_VAR_1"))
# expect: hi
# expect:  true nil
```
```
# tests/lang/stdlib_fns.fung
print(until(1, x => x * 3, x => x > 20), iterate("a", s => s + "b", 2))
# expect: 27 abb
```
```
# tests/lang/underscore_assign.fung
_ = print("side")
print(1)
# expect: side
# expect: 1
```
에러 골든: `err_dup_key.fung` `{a: 1, a: 2}` → E0113; `err_dup_pattern_name.fung` `[a, a] = [1, 2]` → E0221; `err_convert.fung` `int("x")` → E0513; `err_group_key.fung` `group_by([1], x => x)` → E0514; `err_shadow_module.fung` `import fs` / `fs = 1` → E0308; `err_import_twice.fung` `import fs` ×2 → E0309; `err_import_in_fn.fung` `fn f(): import fs ;;` → E0311; `err_native_import.fung` `import _native.fs` → E0310.

- [ ] **Step 1: 예제와 골든 작성 → `go test ./...` 통과** (골든 93 + 17 = 110). `examples/rename.fung`은 손으로 한 번 돌려 본다: `mkdir -p /tmp/rn && touch "/tmp/rn/My File (1).TXT" && go run ./cmd/fung examples/rename.fung /tmp/rn --dry-run` → `My File (1).TXT -> my-file-1.txt`.
- [ ] **Step 2: 노트** `docs/notes/M5-modules-and-stdlib.md` — 절: (1) 한눈에: `import fs` 한 줄의 바이트코드(`IMPORT`·`STOREGLOBAL`)와 로더가 그래프를 먼저 컴파일하는 흐름; (2) 모듈이 값인 것의 뜻과 비용(불변 레코드, 동일성, 스냅샷); (3) 모듈별 전역: 왜 함수 값이 전역 표를 들고 다니는가(다른 모듈에서 불려도 자기 모듈을 봄, M7의 태스크 공유); (4) 적재 시점: 컴파일 시점 그래프(순환·비공개·최상위 제한이 종료 코드 2인 이유)와 실행 시점 한 번(`IMPORT`가 처음 돌 때); (5) 내장 함수의 호출 규약 통일 — `bindArgs` 하나와 "예외 조항 없음"(원칙 7), rest의 `*List`; (6) `exit`가 신호인 이유(`os.Exit` 직접 호출이 깨뜨리는 것들); (7) 2층 표준 라이브러리 — fung 파일이 `_native`를 다시 내보내는 방식, `with_open`이 fung 코드인 이유(03의 `defer` 대체가 실제로 동작함을 보임), embed; (8) 파일 핸들 — 상태 있는 값, 닫힘 Err; (9) prelude 설계 — 06의 에러 규칙(코드 오류 vs 외부 실패), `int` vs `parse_int`, `take`/`drop` vs 슬라이스, nil 반환 함수들; (10) `exec`가 쉘을 거치지 않는 이유와 `sh`; (11) `rename.fung`을 쓰며 느낀 것(prelude 조정 후보); (12) 테스트와 다음 M6.
- [ ] **Step 3: 문서** — 09의 M5 행 ` — 완료`(확인 방법 칸: `examples/rename.fung`이 돈다), CLAUDE.md 상태(M5 완료 요약, 다음 M6 개발 도구), open-questions의 "알파에서 조정" 항목에 rename.fung에서 얻은 후보를 적는다.
- [ ] **Step 4: 커밋** `feat(examples): add the rename script and M5 golden tests` / `docs: add M5 learning notes and update status`.
