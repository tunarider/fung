# M7 동시성 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 태스크(`spawn`·`await`·`cancel`, 취소 전파), 채널(`channel`·`send`·`recv`·`close`, 채널 순회), `atom`(`swap`·`deref`), `sleep`, 종료 규칙(남은 태스크 대기, 기다리지 않은 실패는 종료 코드 1, 어느 태스크의 `exit`든 프로그램 종료), 런타임이 직접 세는 교착 감지, 출력 원자성, fung으로 쓴 `fan_in`·`pmap`·`retry`를 만든다.

**Architecture:** 태스크 하나는 고루틴 하나 + `vm.Task` 하나다(05·09). `Task`는 `context.Context`와 원자적 취소 플래그를 갖고, `enter`(모든 호출)와 막히는 원시 연산이 플래그를 본다. 태스크·채널·atom은 `internal/vm`의 값 타입이고 그 원시 연산도 `vm` 패키지의 내장 함수다(`vm.Concurrency()` 표를 prelude에 합친다). Runtime은 **살아 있는 태스크 수 / 막힌 태스크 수 / 깨어날 것(타이머) 수**를 세고, 막히려는 순간 "막힌 == 살아 있는, 깨어날 것 없음"이면 교착(E0702)으로 프로그램을 끝낸다. 채널은 Go 채널을 **닫지 않고** 별도 `done` 신호로 닫힘을 표현해 "닫힌 채널에 send"가 패닉이 아니라 `Err`가 되게 한다. 기다리지 않은 태스크의 실패는 죽는 순간 `task failed:`로 stderr에 찍고 Runtime의 `failed` 플래그를 세워 종료 코드 1을 만든다. 순회 함수들은 `value.Iterable`(채널이 구현)을 받아 닫힐 때까지 받아 처리한다. `fung` 명령은 fung 코드를 별도 고루틴에서 돌리고 메인 스레드는 기다린다.

**Tech Stack:** Go 1.27, 표준 라이브러리만 (`context`, `sync`, `sync/atomic`, `time`).

**Spec:** `05-concurrency.md` 전체, `02-values-and-types.md` (비교 표의 동일성, 순회 대상에 채널), `06-go-interop-and-stdlib.md` (동시성 prelude, `retry`), `03-functions-and-patterns.md` (`at_exit`, 태스크 안 런타임 에러의 한계), `08-tooling-and-errors.md` (E07xx, 종료 코드), `09-implementation-plan.md` (태스크·취소·교착 메모).

## Global Constraints

- Go 모듈 `github.com/tunarider/fung`, `go 1.27`, **Go 표준 라이브러리만**.
- 코드·주석·에러 메시지·커밋 메시지는 영어. 문서는 한국어.
- Conventional Commits. 커밋 전 `gofmt -l .`(출력 없음), `go vet ./...`, `go test ./...` 통과. **M7에서는 `go test -race ./internal/vm/ ./cmd/fung/`도 통과해야 한다.**
- 커밋 메시지 끝에 `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Go 패닉·스택 추적은 사용자에게 보이지 않는다. **Go 런타임의 교착 감지가 뜨면 실패다.** 종료 코드 0/1/2/3.
- **에러 번호는 한 번 붙이면 바꾸지 않는다.** E0207·E0510·E0805는 쓰지 않는다.
- 원시 연산 이름에 Go 용어(`go`, `chan`, `select`, `context`)를 쓰지 않는다 (05).
- 골든 테스트의 출력은 **결정적**이어야 한다: `await` 결과, 정렬한 값, 횟수만 찍는다. 순서가 달라질 수 있는 `print`는 골든에 넣지 않는다.

## 계획 수준의 결정

1. **값 타입과 자리.** `vm.TaskHandle`(`'task`), `vm.Channel`(`'channel`), `vm.Atom`(`'atom`)은 `internal/vm`에 있다(태스크 내부가 필요하다). 출력은 `value.Opaque` 인터페이스(`Display() string`)로 `<task>` `<channel>` `<atom 3>`, `==`는 동일성(02). `value.Format`·`Equal`이 `Opaque`를 처리한다.
2. **내장 함수의 자리.** `spawn` `await` `cancel` `channel` `send` `recv` `close` `atom` `swap` `deref` `sleep`은 `internal/vm/conc.go`의 `vm.Concurrency() map[string]value.Value`. `bootPrelude`(cmd)와 vm 테스트의 `run`이 `native.PreludeWith`에 합친다. 이 내장 함수들은 `ctx.(*Task)`로 자기 태스크를 얻는다.
3. **취소.** `spawn`은 현재 태스크의 ctx에서 `context.WithCancel`로 자식 ctx를 만든다(전파는 공짜). `context.AfterFunc(ctx, …)`가 `Task.cancelled`(atomic.Bool)를 세우고, `enter`가 매 호출마다 읽는다(값 하나 읽기). 막히는 연산은 `ctx.Done()`도 함께 기다린다. 취소된 태스크는 **E0701**로 되감긴다; `await`는 그것을 `Err({kind: 'cancelled})`로 돌려주고 아무것도 출력하지 않는다.
4. **태스크 결과.** `TaskHandle{done chan struct{}, result value.Value, err *diag.Error, awaited atomic.Bool, cancel}`. 끝나면 `done`을 닫는다. `await`: 성공 → 값, E0701 → `Err cancelled`, 종료 신호 → (아래 6) , 그 외 에러 → `Err({kind: 'error, message: err.Error()})`. `await` 전에 `awaited`를 세운다.
5. **기다리지 않은 실패.** 태스크가 보통 런타임 에러로 죽을 때 `awaited`가 거짓이면 `task failed: ` + `diag.Format(err, src)`를 stderr에 쓰고 `rt.failed = true`. 소스 발췌를 위해 Runtime이 `Sources map[string]string`을 받는다(cmd가 `prog.Sources`를 넣는다).
6. **`exit(n)`.** 어느 태스크든 `exit`를 부르면 `rt.requestExit(n)`: 코드를 기록하고(첫 요청만) **루트 ctx를 취소**한다. 그 태스크 자신은 종료 신호로 되감기고, 다른 태스크(진입 포함)는 E0701로 되감긴다. cmd는 `rt.ExitCode()`가 있으면 그것을 쓴다.
7. **진입 파일의 끝.** cmd: 진입이 정상으로 끝나면 `rt.Wait()`(남은 태스크 대기) → `at_exit` → 코드(`exit` 요청 > 실패 플래그 1 > 0). 진입이 런타임 에러면 `rt.CancelAll()` → `rt.Wait()` → 에러 출력 → `at_exit` → 1 (05).
8. **교착 감지.** Runtime이 `live`·`blocked`·`waking`을 뮤텍스 아래에서 센다. `Task.block(timed bool, wait func())`: 잠금 → `blocked++`, `timed`면 `waking++` → **검사**: `blocked == live && waking == 0`이면 교착 → `rt.deadlock()`(stderr에 `error[E0702]: deadlock: all tasks are waiting`, `requestExit(1)`, 루트 취소) 하고 E0702 반환 → 풀고 `wait()` → 잠금 → 카운터 복구. `live`는 진입 태스크를 포함해 실행 중인 태스크 수(`spawn`에서 ++, 끝날 때 --). 표준 입력 읽기 같은 Go 차단은 세지 않는다(그 태스크는 "실행 중"으로 보여 교착으로 판정하지 않는다 — 안전한 쪽).
9. **채널.** `Channel{ch chan value.Value, done chan struct{}, once sync.Once}`. `send`: `done`이 이미 닫혔으면 `Err closed`; 아니면 `select{ch<-v / <-done / <-ctx.Done()}`. `recv`: 먼저 비차단 수신(버퍼에 남은 값은 닫힌 뒤에도 준다) → 없고 닫혔으면 `Err closed` → 아니면 `select{<-ch / <-done(한 번 더 비차단 수신 뒤 Err closed) / 타임아웃 / <-ctx.Done()}`. `close`: `once.Do(close(done))`, 두 번 닫아도 `Ok(nil)`. `channel(n = 0)`의 n이 음수면 E0403.
10. **순회.** `value.Iterable interface{ Value; Drain(ctx Context) ([]Value, error) }`. `native.items(ctx, v, verb)`가 `Iterable`을 처리하고(모든 호출자에 ctx 전달), `Channel.Drain`은 닫힐 때까지 `recv`해 모은다. `for x in ch`는 `map` 호출이므로 그대로 된다.
11. **atom.** `Atom{mu sync.Mutex, v value.Value, holder *Task}`. `swap(a, f)`: `holder == 현재 태스크`면 E0703; `TryLock` 실패면 `block(false, Lock)`; 잠근 뒤 `ctx.Call(f, [v])` → 저장 → 새 값 반환. `deref`: 잠금 아래 읽기. `atom(v)`.
12. **sleep(d).** `block(true, select{timer / ctx.Done()})`. `d`는 `dur`(음수는 0).
13. **출력 원자성.** `Runtime`이 `Stdout`·`Stderr`를 `lockedWriter`로 감싼다(`Task.Stdout()`이 그것을 돌려준다). `print`는 이미 한 번의 `Write`다.
14. **모듈 초기화의 경쟁.** `moduleState`에 `sync.Mutex`: `importModule`은 잠그고 상태를 본다(이미 로드 → 값; 로딩 중인데 **같은 태스크** → E0801 그대로; 다른 태스크 → 잠금이 풀릴 때까지 기다린다). 간단히: 모듈당 뮤텍스를 `importModule` 전체에 건다(초기화는 짧고 한 번뿐이다). 재진입(같은 태스크가 초기화 중 다시 import)은 컴파일 시점에 막혀 있으므로 데드락이 아니다.
15. **메인 스레드.** `cmd/fung/main.go`: `go func() { code <- run(…) }()`, `os.Exit(<-code)`. GUI를 위해 OS 메인 스레드를 비워 둔다(05·09).
16. **fung으로 쓰는 것.** `prelude.fung`에 `fan_in(chs)`, `pmap(xs, f, limit = 0)`, `retry(f, times = 3, delay = 0ms)`.
17. **REPL.** 태스크가 도는 중의 재정의는 정의하지 않는다(08). REPL 세션 끝에 `rt.Wait()`는 하지 않는다(서버 태스크가 있으면 끝나지 않음) — `:quit`은 `CancelAll` 뒤 종료.

## Review Focus

1. 닫힌 채널에 `send`·`recv`가 **절대 Go 패닉이 되지 않는다**: 버퍼에 값이 남은 채널을 닫은 뒤 `recv`는 남은 값을 다 준 다음 `Err closed` (Task 4).
2. 취소가 자식에 전파된다: 부모를 `cancel`하면 손자까지 `Err cancelled`이고 프로그램이 끝난다(고아가 남지 않는다) (Task 3).
3. 교착 감지가 **거짓 양성**을 내지 않는다: `sleep` 중인 태스크가 깨워 줄 수신, 타임아웃 있는 `recv`, 표준 입력을 읽는 태스크 (Task 3·4).
4. `exit(n)`을 자식 태스크에서 불러도 종료 코드 n, `at_exit` 실행, 진입 파일의 남은 코드는 실행되지 않는다 (Task 6).
5. `-race`로 돌려도 데이터 경쟁이 없다: 열 개 태스크의 `swap` 카운터, 같은 모듈 동시 import, 동시 `print` (Task 5·6).

## 파일 구조

| 패키지 | 변경 | 작업 |
|------|------|------|
| `internal/value`, `internal/diag` | `Opaque`, `Iterable`, Format/Equal, E0701~E0703 | Task 1 |
| `internal/vm` | Task ctx·취소 플래그, Runtime 카운터·대기·종료 요청·locked writer·교착, `block`, `sleep`, `exit` 전파, 모듈 뮤텍스 | Task 2 |
| `internal/vm` | `TaskHandle`, `spawn`·`await`·`cancel`, 실패 출력 | Task 3 |
| `internal/vm`, `internal/native` | `Channel`, `send`·`recv`·`close`·`channel`, `Iterable` 순회 | Task 4 |
| `internal/vm` | `Atom`, `atom`·`swap`·`deref` | Task 5 |
| `cmd/fung`, `internal/repl` | 고루틴 메인, 태스크 대기, 종료 코드, `Sources`, REPL 종료 | Task 6 |
| `internal/stdlib`, `tests/lang` | `fan_in`·`pmap`·`retry`, 골든 | Task 7 |
| `docs`, `CLAUDE.md` | 노트, 09 행, 상태 | Task 8 |

### M7에서 할당하는 번호

| 번호 | 메시지 | 비고 |
|------|------|------|
| E0701 | `task cancelled` | 내부 되감기용; `await`가 `Err cancelled`로 바꾼다. 사용자에게 보이지 않는다 |
| E0702 | `deadlock: all tasks are waiting` | 프로그램 종료, 코드 1 |
| E0703 | `atom swapped inside its own swap` | |

---

### Task 1: value·diag — `Opaque`, `Iterable`, 번호

**Files:**
- Modify: `internal/value/value.go`, `internal/value/format.go`, `internal/value/compare.go`, `internal/diag/catalog.go`
- Test: `internal/value/opaque_test.go`, `internal/diag/error_test.go`(추가)

**Interfaces:**
```go
// value
type Opaque interface { Value; Display() string }   // <task>, <channel>, <atom 3>: identity-compared, no literal
type Iterable interface { Value; Drain(ctx Context) ([]Value, error) }
```
`Format(Opaque)` → `Display()`; `FormatForPrint`도 같다. `Equal(Opaque, Opaque)` → 포인터 동일성(`a == b`). `Compare` → E0501.

- [ ] **Step 1: 테스트**

```go
// internal/value/opaque_test.go
package value

import "testing"

type fakeOpaque struct{ n int }

func (*fakeOpaque) TypeName() string   { return "widget" }
func (f *fakeOpaque) Display() string  { return "<widget>" }

func TestOpaqueFormatAndIdentity(t *testing.T) {
	a, b := &fakeOpaque{1}, &fakeOpaque{1}
	if Format(a) != "<widget>" || FormatForPrint(a) != "<widget>" {
		t.Fatalf("format %s", Format(a))
	}
	if !Equal(a, a) || Equal(a, b) {
		t.Fatal("identity")
	}
	if _, err := Compare(a, b); err == nil || err.Code != "E0501" {
		t.Fatal("not ordered")
	}
	if Equal(a, Int(1)) {
		t.Fatal("never equal to other types")
	}
}
```
diag 테스트: `E0701` `E0702` `E0703`이 카탈로그에 있다.

- [ ] **Step 2: 실패 확인.** **Step 3: 구현.** **Step 4: 통과.** **Step 5: 커밋** `feat(value): add opaque and iterable value interfaces and the E07xx codes`

---

### Task 2: vm 기반 — 태스크 ctx·취소, Runtime 카운터, `block`, `sleep`, `exit` 전파, locked writer, 모듈 뮤텍스

**Files:**
- Modify: `internal/vm/vm.go`, `internal/vm/call.go`, `internal/vm/module.go`
- Create: `internal/vm/runtime_conc.go`, `internal/vm/conc.go`(`Concurrency()` 표, 이 작업에서는 `sleep`만), `internal/vm/conc_test.go`

**Interfaces:**
```go
type Task struct { …; ctx context.Context; cancel context.CancelFunc; cancelled atomic.Bool; handle *TaskHandle }
func (rt *Runtime) root() context.Context            // lazily made; CancelAll cancels it
func (rt *Runtime) CancelAll()
func (rt *Runtime) Wait()                            // waits for spawned tasks
func (rt *Runtime) ExitCode() (int, bool)            // an exit(n) requested anywhere
func (rt *Runtime) Failed() bool                     // an un-awaited task failed
func (rt *Runtime) requestExit(code int)             // first wins; cancels root
func (t *Task) block(timed bool, wait func()) *diag.Error   // deadlock accounting around a wait; E0702 on deadlock
func Concurrency() map[string]value.Value            // sleep (this task); spawn/await/cancel (Task 3); channel… (Task 4); atom… (Task 5)
```
- `Run`: 진입 태스크의 ctx = root; `live++` 시작, 끝나면 `live--`.
- `enter`: 첫 줄에 `if t.cancelled.Load() { return nil, true, diag.Errorf("E0701", pos) }`.
- 종료 신호: `exitFn`(native)은 그대로 `diag.ExitSignal(n)`을 돌려주고, **VM의 `enter`가 종료 신호를 보면** `t.rt.requestExit(n)`을 부른 뒤 전파한다(어느 태스크든 같은 길).
- `Stdout()`/`Stderr()`는 `lockedWriter`를 돌려준다(Runtime이 처음 쓸 때 감싼다).
- `importModule`: `moduleState.mu`로 전체를 감싼다.
- `Runtime.Sources map[string]string` 필드(실패 출력의 발췌용).

- [ ] **Step 1: 테스트**

```go
// internal/vm/conc_test.go
package vm

import (
	"bytes"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/tunarider/fung/internal/native"
	"github.com/tunarider/fung/internal/value"
)

// concPrelude is the real prelude plus the concurrency natives.
func concPrelude() map[string]value.Value {
	p := native.Prelude()
	for k, v := range Concurrency() {
		p[k] = v
	}
	return p
}

func TestSleepWaitsAndIsCancellable(t *testing.T) {
	p := compileWith(t, "sleep(20ms)\nprint(1)", concPrelude())
	var out bytes.Buffer
	rt := &Runtime{Stdout: &out}
	start := time.Now()
	if _, err := rt.Run(p); err != nil {
		t.Fatal(err)
	}
	if time.Since(start) < 15*time.Millisecond || out.String() != "1\n" {
		t.Fatalf("slept %v, out %q", time.Since(start), out.String())
	}
	p = compileWith(t, "sleep(10s)\nprint(1)", concPrelude())
	rt = &Runtime{Stdout: &out}
	go func() { time.Sleep(10 * time.Millisecond); rt.CancelAll() }()
	_, err := rt.Run(p)
	if err == nil || err.Code != "E0701" {
		t.Fatalf("cancelled sleep: %v", err)
	}
	if _, _, code := run(t, "sleep(1)"); code != "E0403" {
		t.Fatalf("sleep(1): %s", code)
	}
}

func TestLockedWritersKeepLinesWhole(t *testing.T) {
	var out bytes.Buffer
	rt := &Runtime{Stdout: &out}
	task := &Task{rt: rt}
	var wg sync.WaitGroup
	for i := 0; i < 50; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			task.Stdout().Write([]byte(strings.Repeat("x", 100) + "\n"))
		}()
	}
	wg.Wait()
	for _, line := range strings.Split(strings.TrimSpace(out.String()), "\n") {
		if len(line) != 100 {
			t.Fatalf("torn line: %d", len(line))
		}
	}
}

func TestExitFromAnywhereRequestsExit(t *testing.T) {
	p := compileWith(t, "fn f() = exit(4)\nf()", concPrelude())
	rt := &Runtime{Stdout: &bytes.Buffer{}}
	_, err := rt.Run(p)
	if code, ok := rt.ExitCode(); !ok || code != 4 || err == nil {
		t.Fatalf("code=%d ok=%v err=%v", code, ok, err)
	}
	if rt.root().Err() == nil {
		t.Fatal("exit cancels the root context")
	}
}
```
(`-race`로도 돈다: `go test -race ./internal/vm/`.)

- [ ] **Step 2: 실패 확인.** **Step 3: 구현** — `runtime_conc.go`:

```go
type lockedWriter struct { mu sync.Mutex; w io.Writer }
func (l *lockedWriter) Write(b []byte) (int, error) { l.mu.Lock(); defer l.mu.Unlock(); return l.w.Write(b) }

// Runtime additions
type Runtime struct {
	…
	Sources   map[string]string
	outMu     sync.Once; out, errw *lockedWriter
	rootOnce  sync.Once; rootCtx context.Context; rootCancel context.CancelFunc
	wg        sync.WaitGroup
	mu        sync.Mutex   // live, blocked, waking, exitCode
	live, blocked, waking int
	exitCode  int; exitSet bool
	failed    atomic.Bool
}

func (t *Task) block(timed bool, wait func()) *diag.Error {
	rt := t.rt
	rt.mu.Lock()
	rt.blocked++
	if timed { rt.waking++ }
	if rt.blocked == rt.live && rt.waking == 0 {
		rt.blocked--
		rt.mu.Unlock()
		rt.deadlock()
		return diag.Errorf("E0702", diag.Pos{})
	}
	rt.mu.Unlock()
	wait()
	rt.mu.Lock()
	rt.blocked--
	if timed { rt.waking-- }
	rt.mu.Unlock()
	return nil
}
```
`deadlock()`: stderr에 `error[E0702]: deadlock: all tasks are waiting\n`, `requestExit(1)`. `sleepFn`: `d`가 `value.Dur`가 아니면 E0403; `t.block(true, func(){ select { case <-time.After(d): case <-t.ctx.Done(): } })`; 깨어난 뒤 `t.cancelled.Load()`면 E0701. 진입 Task의 ctx: `Run`이 `rt.root()`에서 `WithCancel`로 만들고 `AfterFunc`로 플래그 연결. `Call`(내장 함수의 재진입)은 같은 Task이므로 그대로.

- [ ] **Step 4: 통과** (`go test ./... && go test -race ./internal/vm/`). **Step 5: 커밋** `feat(vm): add task contexts, cancellation checks, deadlock accounting, sleep, exit propagation, and locked output`

---

### Task 3: 태스크 — `spawn`·`await`·`cancel`, 실패 출력

**Files:**
- Create: `internal/vm/task.go`
- Modify: `internal/vm/conc.go`, `internal/vm/conc_test.go`

**Interfaces:**
```go
type TaskHandle struct { done chan struct{}; result value.Value; err *diag.Error; awaited atomic.Bool; cancel context.CancelFunc; task *Task }
func (*TaskHandle) TypeName() string { return "task" }; Display() → "<task>"
// natives: spawn(f), await(t, timeout = nil), cancel(t)
```
- `spawn(f)`: `f`가 fn이 아니면 E0403. 새 Task(ctx = 현재 태스크의 자식), `rt.wg.Add(1)`, `live++`, 고루틴: `guarded(Call(f))` → 결과 저장 → 실패 출력 규칙 → `close(done)`, `live--`, `wg.Done`.
- 결과 변환은 결정 4; 종료 신호면 `requestExit`는 이미 `enter`에서 됨 → 핸들의 결과는 `Err cancelled`로 둔다(아무도 보지 않는다).
- `await(t, timeout)`: `awaited.Store(true)`; `block(timeout != nil, select{done / timer / ctx.Done})`; 자기 취소면 E0701; 타임아웃 → `Err({kind: 'timeout})`; 아니면 저장된 결과. 자기 자신을 await → `done`이 닫힐 수 없으니 교착 감지가 잡는다(단일 태스크라면 `blocked == live`).
- `cancel(t)`: `t.cancel()` → `nil`.

- [ ] **Step 1: 테스트**

```go
func TestSpawnAwaitAndResults(t *testing.T) {
	cases := map[string]string{
		"t = spawn(() => 1 + 1)\nprint(await(t), await(t))":                                          "2 2\n",
		"t = spawn(() => Err(\"e\"))\nprint(await(t))":                                               "Err(\"e\")\n",
		"t = spawn(() => 1 / 0)\nprint(await(t))":                                                     "Err({kind: 'error, message: \"error[E0502]: division by zero\"})\n",
		"t = spawn(() => sleep(10s))\ncancel(t)\nprint(await(t))":                                     "Err({kind: 'cancelled})\n",
		"t = spawn(() => sleep(10s))\nprint(await(t, timeout: 10ms))\ncancel(t)":                     "Err({kind: 'timeout})\n",
		"ts = for i in 0..5: spawn(() => i * i) ;;\nprint(ts |> map(await))":                           "[0, 1, 4, 9, 16]\n",
		"print(type_of(spawn(() => 1)), spawn(() => 1) == spawn(() => 1))":                            "'task false\n",
		"outer = spawn(fn():\n  inner = spawn(() => sleep(10s))\n  await(inner)\n;;)\nsleep(10ms)\ncancel(outer)\nprint(await(outer))": "Err({kind: 'cancelled})\n",
	}
	for src, want := range cases {
		_, out, code := runConc(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
	if _, _, code := runConc(t, "spawn(1)"); code != "E0403" {
		t.Errorf("spawn(1): %s", code)
	}
	if _, _, code := runConc(t, "await(1)"); code != "E0403" {
		t.Errorf("await(1): %s", code)
	}
}

func TestUnawaitedFailureIsReportedOnce(t *testing.T) {
	p := compileWith(t, "spawn(() => fail(\"boom\"))\nsleep(20ms)\nprint(\"after\")", concPrelude())
	var out, errOut bytes.Buffer
	rt := &Runtime{Stdout: &out, Stderr: &errOut, Sources: map[string]string{"t.fung": "…"}}
	if _, err := rt.Run(p); err != nil {
		t.Fatal(err)
	}
	rt.Wait()
	if out.String() != "after\n" || !strings.HasPrefix(errOut.String(), "task failed: error[E0803]: boom") || !rt.Failed() {
		t.Fatalf("out=%q err=%q failed=%v", out.String(), errOut.String(), rt.Failed())
	}
	// Awaited failures are not printed.
	p = compileWith(t, "t = spawn(() => fail(\"boom\"))\nprint(await(t) |> is_err)", concPrelude())
	out.Reset(); errOut.Reset()
	rt = &Runtime{Stdout: &out, Stderr: &errOut}
	rt.Run(p)
	rt.Wait()
	if out.String() != "true\n" || errOut.String() != "" || rt.Failed() {
		t.Fatalf("awaited: out=%q err=%q failed=%v", out.String(), errOut.String(), rt.Failed())
	}
}

func TestDeadlockIsDetected(t *testing.T) {
	p := compileWith(t, "t = spawn(() => sleep(10s))\nawait(spawn(() => await(t)))\n", concPrelude())
	// Not a deadlock: a sleeper will wake. Use channels (Task 4) for the real one; here: await on self.
	var errOut bytes.Buffer
	rt := &Runtime{Stdout: &bytes.Buffer{}, Stderr: &errOut}
	p = compileWith(t, "me = atom(nil)\n", concPrelude()) // placeholder until Task 4/5 provide a self-wait
	_ = p
	_ = rt
}
```
(교착의 결정적 재현은 Task 4의 "수신자만 있는 비버퍼 채널"로 한다: 위 `TestDeadlockIsDetected`는 Task 4에서 채운다. 이 작업에서는 `runConc` 도우미(`concPrelude`로 컴파일·`registerModules`·`rt.Run`·`rt.Wait`)를 추가한다.)

- [ ] **Step 2: 실패 확인.** **Step 3: 구현** (`task.go`):

```go
func (t *Task) spawn(fn value.Value) *TaskHandle {
	ctx, cancel := context.WithCancel(t.ctx)
	child := &Task{rt: t.rt, ctx: ctx, cancel: cancel}
	context.AfterFunc(ctx, func() { child.cancelled.Store(true) })
	h := &TaskHandle{done: make(chan struct{}), cancel: cancel, task: child}
	child.handle = h
	t.rt.addTask()                       // wg.Add(1); live++
	go func() {
		defer t.rt.taskDone()            // live--; wg.Done()
		defer close(h.done)
		v, err := child.guarded(func() (value.Value, *diag.Error) {
			r, e := child.Call(fn, nil)
			if e != nil { return nil, e.(*diag.Error) }
			return r, nil
		})
		h.finish(t.rt, v, err)
	}()
	return h
}
```
`finish`: err nil → result; E0701 또는 종료 신호 → `Err cancelled`; 그 외 → `Err({kind: 'error, message})` + `if !h.awaited.Load() { rt.reportTaskFailure(err); rt.failed.Store(true) }`. `reportTaskFailure`: `"task failed: " + diag.Format(err, rt.Sources[err.Pos.File])`를 stderr로.

- [ ] **Step 4: 통과** (`-race` 포함). **Step 5: 커밋** `feat(vm): add spawn, await, and cancel with propagated cancellation and failure reporting`

---

### Task 4: 채널 — `channel`·`send`·`recv`·`close`, 순회

**Files:**
- Create: `internal/vm/channel.go`
- Modify: `internal/vm/conc.go`, `internal/vm/conc_test.go`, `internal/native/collections.go`(`items(ctx, …)`), `internal/native/strings.go`(`join`의 `items`), `internal/native/prelude.go`(`map`)

**Interfaces:**
```go
type Channel struct { ch chan value.Value; done chan struct{}; once sync.Once }
func (*Channel) TypeName() string { return "channel" }; Display() → "<channel>"
func (c *Channel) Drain(ctx value.Context) ([]value.Value, error)   // recv until closed (value.Iterable)
// natives: channel(n = 0), send(ch, v), recv(ch, timeout = nil), close(ch)
```
`native.items(ctx value.Context, v value.Value, verb string)`: `*List`, `Range`, `value.Iterable`(→ `Drain(ctx)`), 그 외 E0506.

- [ ] **Step 1: 테스트**

```go
func TestChannels(t *testing.T) {
	cases := map[string]string{
		"ch = channel()\nspawn(() => send(ch, 1))\nprint(recv(ch))":                                          "Ok(1)\n",
		"ch = channel(2)\nsend(ch, 1)\nsend(ch, 2)\nclose(ch)\nprint(recv(ch), recv(ch), recv(ch), send(ch, 3))": "Ok(1) Ok(2) Err({kind: 'closed}) Err({kind: 'closed})\n",
		"ch = channel()\nprint(recv(ch, timeout: 10ms))":                                                      "Err({kind: 'timeout})\n",
		"ch = channel(3)\neach(1..=3, x => send(ch, x))\nclose(ch)\nprint(for x in ch: x * 10 ;;)":           "[10, 20, 30]\n",
		"ch = channel(3)\neach([3, 1, 2], x => send(ch, x))\nclose(ch)\nprint(sort(ch), close(ch))":           "[1, 2, 3] Ok(nil)\n",
		"ch = channel(1)\nt = spawn(fn():\n  each(0..100, i => send(ch, i))\n  close(ch)\n;;)\nprint(fold(ch, 0, (a, x) => a + x), await(t))": "4950 nil\n",
		"print(type_of(channel()))":                                                                          "'channel\n",
	}
	for src, want := range cases {
		_, out, code := runConc(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
	for src, want := range map[string]string{"channel(-1)": "E0403", "send(1, 2)": "E0403", "recv(1)": "E0403", "close(1)": "E0403"} {
		if _, _, code := runConc(t, src); code != want {
			t.Errorf("%q: %s", src, code)
		}
	}
}

func TestDeadlockIsDetected(t *testing.T) {
	for _, src := range []string{
		"ch = channel()\nrecv(ch)",                                   // no sender anywhere
		"ch = channel()\nt = spawn(() => recv(ch))\nawait(t)",        // both waiting on each other
		"t = spawn(() => sleep(10s))\nch = channel()\nrecv(ch)",     // NOT a deadlock: the sleeper will wake — ends by timeout below
	} {
		_ = src
	}
	var errOut bytes.Buffer
	p := compileWith(t, "ch = channel()\nrecv(ch)", concPrelude())
	rt := &Runtime{Stdout: &bytes.Buffer{}, Stderr: &errOut}
	_, err := rt.Run(p)
	if err == nil || err.Code != "E0702" || !strings.Contains(errOut.String(), "error[E0702]: deadlock") {
		t.Fatalf("err=%v stderr=%q", err, errOut.String())
	}
	if code, ok := rt.ExitCode(); !ok || code != 1 {
		t.Fatalf("exit code after deadlock = %d %v", code, ok)
	}
	p = compileWith(t, "ch = channel()\nt = spawn(() => recv(ch))\nawait(t)", concPrelude())
	rt = &Runtime{Stdout: &bytes.Buffer{}, Stderr: &errOut}
	if _, err := rt.Run(p); err == nil || (err.Code != "E0702" && err.Code != "E0701") {
		t.Fatalf("mutual wait: %v", err)
	}
	rt.Wait()
	// A sleeper keeps the program alive: recv with a timeout returns instead of a false deadlock.
	p = compileWith(t, "t = spawn(() => sleep(30ms))\nch = channel()\nprint(recv(ch, timeout: 10ms))\nawait(t)", concPrelude())
	rt = &Runtime{Stdout: &bytes.Buffer{}, Stderr: &bytes.Buffer{}}
	if _, err := rt.Run(p); err != nil {
		t.Fatalf("false deadlock: %v", err)
	}
}
```
(첫 `for` 루프의 자리표시는 지우고 세 경우를 본문처럼 쓴다.)

- [ ] **Step 2: 실패 확인.** **Step 3: 구현** — 결정 9·10대로. `recv`의 차단: `t.block(timeout != nil, func(){ select … })`. `Drain`: `ctx.(*Task)`로 태스크를 얻어 `recv`를 반복.
- [ ] **Step 4: 통과** (`-race`). **Step 5: 커밋** `feat(vm): add channels with Err on close, timeouts, and iteration`

---

### Task 5: atom — `atom`·`swap`·`deref`

**Files:**
- Create: `internal/vm/atom.go`
- Modify: `internal/vm/conc.go`, `internal/vm/conc_test.go`

- [ ] **Step 1: 테스트**

```go
func TestAtoms(t *testing.T) {
	cases := map[string]string{
		"a = atom(0)\nprint(swap(a, x => x + 1), deref(a), a)":                                                  "1 1 <atom 1>\n",
		"a = atom(0)\nts = for _ in 0..10: spawn(() => each(0..100, _ => swap(a, x => x + 1))) ;;\neach(ts, await)\nprint(deref(a))": "1000\n",
		"a = atom([])\nswap(a, xs => xs + [1])\nprint(deref(a), type_of(a))":                                   "[1] 'atom\n",
	}
	for src, want := range cases {
		_, out, code := runConc(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
	if _, _, code := runConc(t, "a = atom(0)\nswap(a, x => swap(a, y => y))"); code != "E0703" {
		t.Errorf("re-entrant swap: %s", code)
	}
	if _, _, code := runConc(t, "swap(1, x => x)"); code != "E0403" {
		t.Errorf("swap(1): %s", code)
	}
	if _, _, code := runConc(t, "a = atom(0)\nswap(a, 1)"); code != "E0403" {
		t.Errorf("swap with a non-fn: %s", code)
	}
}
```

- [ ] **Step 2–4.** **Step 5: 커밋** `feat(vm): add atoms with swap and deref`

---

### Task 6: cmd·repl — 고루틴 메인, 태스크 대기, 종료 코드, `Sources`, REPL 종료

**Files:**
- Modify: `cmd/fung/main.go`, `cmd/fung/pipeline.go`, `cmd/fung/test.go`, `cmd/fung/repl.go`, `internal/repl/repl.go`, `cmd/fung/main_test.go`, `internal/vm/vm_test.go`(`run`이 `Concurrency`를 합침), `internal/golden`(불필요하면 없음)

**Interfaces:**
- `bootPrelude`가 `vm.Concurrency()`를 합친다. `rt.Sources = prog.Sources`.
- `execute`: 진입 정상 → `rt.Wait()`; 런타임 에러 → `rt.CancelAll(); rt.Wait()`; 종료 코드: `rt.ExitCode()` > 진입 에러 1 > `rt.Failed()` 1 > 0; `at_exit` 뒤 핸들러 실패 1.
- `main`: `go func() { codeCh <- run(…) }()`; `os.Exit(<-codeCh)`.
- `fung test`: 파일마다 `rt.Wait()` 뒤 결과; 기다리지 않은 실패는 그 파일의 실패로 센다(`rt.Failed()` → `FAIL path: <tasks>` 한 줄).
- REPL: `:quit`/EOF → `rt.CancelAll()` 뒤 `at_exit`.

- [ ] **Step 1: 테스트**

```go
// cmd/fung/main_test.go (추가)
func TestConcurrencyExitRules(t *testing.T) {
	out, errOut, code := invoke(t, "-e", "t = spawn(() => 1)\nprint(await(t))")
	if code != 0 || out != "1\n" || errOut != "" {
		t.Fatalf("basic: code=%d out=%q err=%q", code, out, errOut)
	}
	out, errOut, code = invoke(t, "-e", "spawn(() => fail(\"x\"))\nsleep(20ms)\nprint(\"done\")")
	if code != 1 || out != "done\n" || !strings.Contains(errOut, "task failed: error[E0803]: x") {
		t.Fatalf("unawaited failure: code=%d out=%q err=%q", code, out, errOut)
	}
	out, _, code = invoke(t, "-e", "spawn(() => fail(\"x\"))\nsleep(20ms)\nexit(0)")
	if code != 0 {
		t.Fatalf("exit(0) wins: %d", code)
	}
	out, _, code = invoke(t, "-e", "at_exit(() => print(\"bye\"))\nspawn(fn(): sleep(10ms); exit(3) ;;)\nsleep(10s)\nprint(\"never\")")
	if code != 3 || out != "bye\n" {
		t.Fatalf("exit from a task: code=%d out=%q", code, out)
	}
	out, _, code = invoke(t, "-e", "spawn(fn(): sleep(20ms); print(\"late\") ;;)\nprint(\"entry done\")")
	if code != 0 || out != "entry done\nlate\n" {
		t.Fatalf("waits for tasks: code=%d out=%q", code, out)
	}
	_, errOut, code = invoke(t, "-e", "spawn(() => sleep(10s))\n1 / 0")
	if code != 1 || !strings.Contains(errOut, "E0502") {
		t.Fatalf("entry error cancels tasks and exits 1: code=%d err=%q", code, errOut)
	}
	_, errOut, code = invoke(t, "-e", "ch = channel()\nrecv(ch)")
	if code != 1 || !strings.Contains(errOut, "deadlock") {
		t.Fatalf("deadlock: code=%d err=%q", code, errOut)
	}
}
```
(`cmd/fung` 테스트도 `-race`로 한 번 돌린다.)

- [ ] **Step 2–4.** **Step 5: 커밋** `feat(cmd): run fung on a worker goroutine, wait for tasks, and apply the concurrency exit rules`

---

### Task 7: fung 코드 — `fan_in`·`pmap`·`retry`, 골든

**Files:**
- Modify: `internal/stdlib/prelude.fung`
- Create: `tests/lang/tasks.fung`, `tests/lang/channels.fung`, `tests/lang/atoms.fung`, `tests/lang/cancel.fung`, `tests/lang/conc_helpers.fung`, `tests/lang/exit_from_task.fung`, `tests/lang/err_task_failed.fung`, `tests/lang/err_deadlock.fung`, `tests/lang/err_atom_reentry.fung`

`prelude.fung` 추가:
```
# Merges channels into one: a forwarding task per input; the output
# closes when every input has closed (05).
fn fan_in(chs):
  out = channel(len(chs))
  tasks = for ch in chs: spawn(() => each(ch, v => send(out, v))) ;;
  spawn(fn():
    each(tasks, await)
    close(out)
  ;;)
  out
;;

# Parallel map: at most `limit` tasks at once (0 = no limit). Results keep
# the input order; a failing f shows up as its Err like await would.
fn pmap(xs, f, limit = 0):
  items = xs |> map(x => x)
  limit <= 0 or limit >= len(items)
    then items |> map(x => spawn(() => f(x))) |> map(await)
    else _pmap_chunks(items, f, limit, [])
;;

fn _pmap_chunks(items, f, limit, acc) = is_empty(items) then acc
  else _pmap_chunks(drop(items, limit), f, limit, acc + (take(items, limit) |> map(x => spawn(() => f(x))) |> map(await)))

# Calls f until it returns a non-Err value, at most `times` tries,
# sleeping `delay` between tries; the last Err is returned (06).
fn retry(f, times = 3, delay = 0ms):
  r = f()
  times <= 1 or not is_err(r) then r
  else (sleep(delay); retry(f, times: times - 1, delay: delay))
;;
```
(`(sleep(delay); retry(…))`가 식 자리에서 안 되면 `fn _retry_after(...)`로 나눈다. `pmap`의 `limit` 분할은 단순 청크 방식이다: 알파에서 워커 풀로 바꿀 수 있다.)

골든 예:
```
# tasks.fung
ts = for i in 1..=3: spawn(() => i * 2) ;;
print(ts |> map(await), await(spawn(() => Err('e))), await(spawn(() => 1 / 0)) |> is_err)
# expect: [2, 4, 6] Err('e) true
# channels.fung
ch = channel()
spawn(fn(): each(1..=5, x => send(ch, x)); close(ch) ;;)
print(fold(ch, 0, (a, x) => a + x))
# expect: 15
# atoms.fung
counter = atom(0)
each(for _ in 0..8: spawn(() => each(0..50, _ => swap(counter, n => n + 1))) ;;, await)
print(deref(counter))
# expect: 400
# cancel.fung
t = spawn(() => sleep(10s))
cancel(t)
print(await(t), await(spawn(() => sleep(10s)), timeout: 10ms))
# expect: Err({kind: 'cancelled}) Err({kind: 'timeout})
# conc_helpers.fung
a = channel(2); b = channel(2)
each([1, 2], x => send(a, x)); close(a); send(b, 3); close(b)
print(fan_in([a, b]) |> map(x => x) |> sort, pmap([1, 2, 3], x => x * x, limit: 2))
# expect: [1, 2, 3] [1, 4, 9]
n = atom(0)
print(retry(fn(): swap(n, x => x + 1) < 3 then Err('again) else Ok("done") ;;, times: 5), deref(n))
# expect: Ok("done") 3
# exit_from_task.fung  (# expect-exit: 5)
# err_task_failed.fung  spawn(() => fail("x")); sleep(20ms)   # expect-error: E0803
# err_deadlock.fung     ch = channel(); recv(ch)             # expect-error: E0702
# err_atom_reentry.fung a = atom(0); swap(a, x => swap(a, y => y))   # expect-error: E0703
```

- [ ] **Step 1: 골든·prelude 작성 → `go test ./...` 통과.** **Step 2: 커밋** `feat(stdlib): add fan_in, pmap, and retry, with concurrency golden tests`

---

### Task 8: 노트·문서

- `docs/notes/M7-concurrency.md`: (1) 한눈에: `spawn`/`await` 바이트코드는 보통 호출이고 특별한 명령이 없다 — 동시성이 값과 내장 함수만으로 되는 이유; (2) 태스크 = 고루틴 + Task, 왜 스택·프레임을 태스크별로 두는가; (3) 취소: context 트리와 원자적 플래그, 협조적 취소의 뜻과 한계(정리 안 됨, 03); (4) 결과의 네 가지와 "기다리지 않은 실패"를 죽는 순간 찍는 이유; (5) 교착을 직접 세는 이유와 카운터 셋(live/blocked/waking), 거짓 양성을 피하는 조건; (6) 채널을 닫지 않는 구현 — `done` 신호로 패닉을 없애는 트릭, 닫힌 뒤 버퍼를 비우는 순서; (7) atom과 재진입 감지; (8) 출력 원자성과 모듈 초기화 뮤텍스 — 어디에 경쟁이 있었나; (9) `exit`가 어디서든 통하는 길(종료 신호 → requestExit → 루트 취소 → 모두 E0701); (10) fung으로 쓴 `fan_in`·`pmap`·`retry`; (11) `-race` 테스트; (12) 다음 M8.
- 09의 M7 행 ` — 완료`, 테스트 절에 `-race`; CLAUDE.md 상태(M7 완료, 다음 M8 웹: `json`, `time`, `regex`, `http`); README에 동시성 한 줄.
- 커밋 `docs: add M7 learning notes and update status`.
