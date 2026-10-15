# Rust 이식 3a단계: 동시성 런타임 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rust `fung`이 태스크·채널·취소·타임아웃·`atom`·교착 감지·꺼내지 않은 실패 보고·어디서든 `exit`를 Go와 같은 출력·종료 코드로 실행한다.

**Architecture:** 태스크마다 운영체제 스레드 하나, fung 코드는 실행권 잠금(`Mutex`) 하나를 쥔 스레드만 돌린다. 막히는 연산은 모두 한 함수를 거쳐
조건 변수(`Condvar`)로 기다린다. `Runtime`은 공유 부분(잠금 아래)과 태스크별 부분(스택·프레임·취소 표시)으로 나눈다.

**Tech Stack:** Rust 1.94 표준 라이브러리만(`std::sync::{Mutex, Condvar, Arc}`, `std::thread`). Go 구현이 정답지.

**Spec:** `docs/superpowers/specs/2026-10-15-rust-runtime-3a-design.md`

## Global Constraints

- Rust 크레이트 의존성 0개. `rust-toolchain.toml`의 Rust 1.94, 모든 크레이트 `edition = "2024"`.
- 커밋 전: `gofmt -l .`(빈 출력), `go vet ./...`, `go test ./...`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `FUNG_REQUIRE_PARITY=1 cargo test`, `FUNG_REQUIRE_PARITY=1 cargo test --release`.
- 코드·주석·에러 문구·커밋 메시지는 영어, 학습 노트는 한국어. Conventional Commits. 커밋 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- git 작성자는 저장소 설정 그대로. 다른 이메일 금지.
- 비교 대상 출력은 Go와 바이트 단위로 같다. 에러 번호와 문구는 `spec/errors.txt`에서. 새 번호를 만들지 않는다.
- Go 구현(`cmd/`, `internal/`, `extras/`)은 수정하지 않는다.
- **실행권 규칙**: fung 값(`Rc`를 품은 모든 것)을 만들고·읽고·복제하고·해제하는 일은 실행권을 쥔 스레드만 한다. `unsafe impl Send/Sync`는
  이 규칙에 기대는 감싸개에만 쓰고, 쓰는 곳마다 `// SAFETY:` 주석으로 근거를 적는다.
- Go를 흉내 내려고만 붙이는 코드는 `docs/design/open-questions.md` "Go 동등을 위한 과도기 잔가지" 항목에 한 줄씩 더한다.

## 이식 규칙 (모든 작업에 적용)

- Go 소스가 참조 구현이다: `internal/vm/task.go`, `channel.go`, `atom.go`, `conc.go`, `runtime_conc.go`, `cmd/fung/pipeline.go`의 `execute`.
  함수 단위로 읽고 같은 동작을 Rust로 쓴다(줄 단위 번역 금지).
- 작업에 이름으로 나열한 Go 테스트는 **모두** Rust로 옮긴다. Go 전용(GC 시점, `-race`)이면 같은 의도의 Rust 테스트로 바꾸고 보고서에 적는다.
- 정확한 기대 출력은 Go 바이너리로 만든다: `go build -o /tmp/fung-go ./cmd/fung` 후 `/tmp/fung-go -e '<code>'`.
- 시간에 기대는 테스트는 넉넉한 상한을 둔다(예: `sleep(10s)`는 취소되어야 하고, 전체 테스트가 1초 안에 끝나야 한다).
- 동시성 테스트는 결과가 실행 순서에 달리지 않게 쓴다(출력 순서가 정해지지 않는 곳은 정렬하거나 하나의 태스크에서 모아 찍는다).

## Review Focus

1. 실행권 없이 값이 만져지는 경로(태스크 스레드 끝의 해제, 다른 스레드에서 사라지는 핸들, 표준 입력 대기 중의 값) → 디버그 단언이 잡아야 한다 → Task 2, Task 3.
2. 내장 함수가 부른 람다 안에서 막힌 태스크(`each(xs, x => send(ch, x))`)를 취소하거나 `exit`해도 프로그램이 멈추지 않고 끝난다 → Task 3, Task 4, Task 7.
3. 다른 태스크들이 `recv`·`sleep`·표준 입력에서 막힌 상태에서 한 태스크가 `exit(n)`하면 곧 끝나고 `at_exit`는 한 번만 돈다 → Task 7.
4. 태스크 2,000개를 띄워도 스택 예약 때문에 죽지 않고 적당한 시간에 끝난다 → Task 3, Task 8.
5. 계산만 하는 태스크가 있어도 다른 태스크가 진행한다(차례 넘기기) → Task 3.

---

### Task 1: 스레드 사이에서 안전한 내장 함수와 스트림

**Files:**
- Modify: `crates/fung-vm/src/value.rs`(`NativeParam`), `crates/fung-vm/src/prelude.rs`, `crates/fung-vm/src/conc.rs`, `crates/fung-vm/src/native/process.rs`(스레드 지역 → 프로세스 전체), `crates/fung-vm/src/native/nat.rs`(`opt` 등 도우미), `crates/fung-vm/src/vm/call.rs`(기본값 채우기), `crates/fung-vm/src/vm/runtime.rs`·`crates/fung-cli/src/run.rs`·`crates/fung-vm/src/testutil.rs`(스트림 타입)

**Interfaces:**
- Produces:
  - `pub enum Lit { Nil, Bool(bool), Int(i64), Float(f64), Str(&'static str), Dur(i64) }` + `impl Lit { pub fn to_value(&self) -> Value }`. `NativeParam { name: &'static str, default: Option<Lit>, rest: bool }`(지금 `Option<Value>`를 대신함). 기본값이 필요한 내장 함수의 기본값은 모두 리터럴이다(아니면 보고서에 적고 가장 가까운 리터럴로).
  - `Native`가 `Send + Sync`가 된다. 내장 함수 값 표(`prelude::core()`, `conc::placeholders()`, `conc::module()`의 내용, `_native.process`)는 `std::sync::OnceLock`으로 프로세스에 한 번만 만든다. 이름이 같은 내장 함수는 어느 스레드에서 얻어도 같은 `&'static Native`다.
  - `Runtime::new(stdout: Box<dyn Write + Send>, stderr: Box<dyn Write + Send>, stdin: Box<dyn BufRead + Send>, sources: SourceMap)`.

- [ ] **Step 1: 실패하는 테스트**

```rust
#[test]
fn natives_are_the_same_on_every_thread() {
    let here = crate::prelude::core().into_iter().find(|(n, _)| *n == "print").unwrap().1;
    let there = std::thread::spawn(|| {
        let v = crate::prelude::core().into_iter().find(|(n, _)| *n == "print").unwrap().1;
        match v { crate::value::Value::Native(n) => n as *const _ as usize, _ => 0 }
    }).join().unwrap();
    match here { crate::value::Value::Native(n) => assert_eq!(n as *const _ as usize, there), _ => panic!() }
}

#[test]
fn natives_can_cross_threads() {
    fn assert_sync<T: Send + Sync>() {}
    assert_sync::<crate::value::Native>();
}
```
기본값이 있는 내장 함수(`await`·`recv`의 `timeout=nil`, `channel`의 `n=0`, `retry`가 아닌 Rust 쪽 것들)의 동작 테스트는 지금 것이 그대로 지킨다.

- [ ] **Step 2: 실패 확인** — Run: `cargo test -p fung-vm natives_` / Expected: 컴파일 실패 또는 FAIL.
- [ ] **Step 3: 구현** — `Lit`으로 바꾸고, 바인딩할 때 `to_value()`로 만든다. `thread_local!` 내장 함수 표를 `OnceLock`으로. 스트림 타입에 `+ Send`.
- [ ] **Step 4: 통과 확인** — Run: `FUNG_REQUIRE_PARITY=1 cargo test && cargo clippy --all-targets -- -D warnings` / Expected: PASS(2단계 테스트 전부 그대로).
- [ ] **Step 5: 커밋**
```bash
git add crates
git commit -m "refactor(rust): make natives and streams safe to share across threads"
```

---

### Task 2: 런타임 분할과 실행권 잠금 (태스크 하나)

**Files:**
- Create: `crates/fung-vm/src/vm/world.rs`(공유 부분과 실행권), `crates/fung-vm/src/vm/token.rs`(실행권을 쥐었는지의 디버그 표시)
- Modify: `crates/fung-vm/src/vm.rs`(`Task`), `crates/fung-vm/src/vm/runtime.rs`, `crates/fung-vm/src/vm/module.rs`, `crates/fung-vm/src/vm/m6.rs`, `crates/fung-vm/src/boot.rs`, `crates/fung-vm/src/testutil.rs`, `crates/fung-cli/src/run.rs`

**Interfaces:**
- Consumes: Task 1의 `Send` 스트림과 `Sync` 내장 함수.
- Produces:
  - `World`(공유 부분): 스트림, `at_exit` 목록, 종료 코드, 모듈 상태, 소스 맵, 전역 표 목록, 테스트 블록, (Task 3부터) 태스크 집계. `Mutex<World>` + `Condvar` 하나를 `Arc`로 나눠 갖는다. `World`는 `Rc`를 품으므로 `// SAFETY:` 근거와 함께 `unsafe impl Send for World`.
  - `Task`는 실행권(`MutexGuard<World>`)을 쥔 채 돈다: 태스크별 부분(값 스택, 프레임, 호출 중인 내장 함수)은 `Task`에, 공유 부분은 가드를 거쳐서. 내장 함수 쪽 API(`stdout()`·`stderr()`·`read_line()`·`at_exit`·`args`·`call`·`calling_native`)는 이름과 의미가 그대로다.
  - `pub(crate) fn assert_holds_token()`: 디버그 빌드에서 이 스레드가 실행권을 쥐었는지 단언(스레드 지역 표시, 릴리스에서는 아무것도 안 함). 태스크 시작·끝, 막히는 지점(Task 3), 값 해제 경계(`Runtime`의 `Drop`)에서 부른다.
  - 공개 API는 유지: `Runtime::{new, run, run_exit_handlers, exit_code, sources, add_sources, args}`, `boot`, `register`, `link`, `testutil::*`. 시그니처가 `&mut self`에서 `&self`로 바뀌는 것은 괜찮다(호출부를 함께 고친다).

- [ ] **Step 1: 실패하는 테스트**

```rust
#[test]
fn the_runtime_can_move_to_another_thread() {
    fn assert_send<T: Send>() {}
    assert_send::<crate::vm::Runtime>();
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "without the run token")]
fn touching_the_world_without_the_token_is_caught() {
    crate::vm::token::assert_holds_token();
}
```

- [ ] **Step 2: 실패 확인** — Run: `cargo test -p fung-vm runtime_can_move touching_the_world` / Expected: 컴파일 실패.
- [ ] **Step 3: 구현** — `Runtime`의 필드를 `World`로 옮기고 `Task`가 가드를 쥐게 한다. 메인 태스크는 지금처럼 호출한 스레드에서 돈다.
- [ ] **Step 4: 통과와 성능 확인** — Run: `FUNG_REQUIRE_PARITY=1 cargo test && FUNG_REQUIRE_PARITY=1 cargo test --release && cargo clippy --all-targets -- -D warnings` / Expected: PASS.
  그리고 `cargo build --release`와 `git stash`를 쓰지 말고 이전 커밋을 다른 작업 트리(`git worktree add /tmp/fung-before HEAD~1` 등)에 빌드해, `bench/*.fung` 네 개를 각 5회 재서 가장 빠른 값이 5% 넘게 느려지지 않았는지 확인해 보고서에 표로 적는다(작업 트리는 끝나고 지운다).
- [ ] **Step 5: 커밋**
```bash
git add crates
git commit -m "refactor(rust): split the runtime into shared state behind a run token"
```

---

### Task 3: 태스크 — `spawn`·`await`·`cancel`·`sleep`, 막히는 지점, 차례 넘기기

**Files:**
- Create: `crates/fung-vm/src/vm/task.rs`(태스크 핸들·결과 칸·취소 트리·스레드 시작), `crates/fung-vm/src/vm/block.rs`(막히는 지점 하나), `crates/fung-vm/src/vm/task_tests.rs`
- Modify: `crates/fung-vm/src/value.rs`(`Value::Task`), `value/format.rs`·`value/compare.rs`·`value/types.rs`(서식·같음·`type_of`, Go `task.go`와 같게), `value/release.rs`(필요하면), `crates/fung-vm/src/conc.rs`(`spawn`·`await`·`cancel`·`sleep`·`_native.conc.spawn_awaited`를 자리표에서 실제로), `crates/fung-vm/src/vm.rs`·`vm/call.rs`(호출·뒤로 뛰기 지점의 취소 검사와 차례 넘기기), `crates/fung-vm/src/vm/runtime.rs`(`wait`, `cancel_all`)

**Interfaces:**
- Consumes: Task 2의 `World`·실행권·`assert_holds_token`.
- Produces:
  - `pub(crate) fn block<T>(t: &mut Task, wait: impl FnMut(&mut World) -> Option<T>, timeout: Option<Duration>) -> Result<Option<T>, Stop>` (이름·모양은 구현이 다듬어도 되지만 **막히는 연산은 모두 이것 하나를 거친다**): 조건이 되면 `Some`, 타임아웃이면 `None`, 취소되면 E0701. 기다리는 동안 실행권을 내려놓고(`Condvar::wait_timeout`), 교착 집계(Task 6)를 위한 "막힘" 표시를 한다. 실행권 밖에서 기다리는 표준 입력·외부 프로세스는 `pub(crate) fn outside<T>(t: &mut Task, f: impl FnOnce() -> T) -> T`로 실행권만 놓고 운영체제 호출을 한다(집계에 넣지 않음).
  - 태스크 스레드 스택 상수 `TASK_STACK: usize = 256 << 20`(계획 값. 재서 바꾸면 보고서에 근거).
  - 차례 넘기기 상수 `YIELD_EVERY: u32`(호출·뒤로 뛰기 횟수. 값은 재서 정하고 근거를 보고서에). 기다리는 다른 태스크가 없으면 아무것도 하지 않는다. 있으면 순번표(공정한 줄)로 넘긴다.
  - `Runtime::wait(&self)`(남은 태스크가 모두 끝날 때까지), `Runtime::cancel_all(&self)`.
  - `Value::Task(Rc<TaskHandle>)` — 16바이트 유지(`value_is_16_bytes` 테스트가 지킨다).
- Go 테스트: `internal/vm/conc_test.go`의 `TestSleepWaitsAndIsCancellable`, `TestSpawnAwaitAndResults`, `TestAwaitTimeoutDoesNotHideALaterFailure`(보고 부분은 Task 6과 함께 완성해도 됨), `internal/vm/context_test.go`의 `TestRunTaskRunsOnAFreshTask`.

- [ ] **Step 1: 실패하는 테스트** (`task_tests.rs`, 기대값은 Go 바이너리로 먼저 확인)

```rust
use crate::testutil::run;

#[test]
fn spawn_and_await_like_go() {
    let (out, err, code) = run("ts = for i in 1..=3: spawn(() => i * 2) ;;\nprint(ts |> map(await), await(spawn(() => Err('e))), await(spawn(() => 1 / 0)) |> is_err)");
    assert_eq!((out.as_str(), err.as_str(), code), ("[2, 4, 6] Err('e) true\n", "", 0));
}

#[test]
fn cancel_reaches_a_sleeping_task_and_its_children() {
    let start = std::time::Instant::now();
    let (out, _, code) = run("t = spawn(fn(): spawn(() => sleep(10s)); sleep(10s) ;;)\ncancel(t)\nprint(await(t), await(spawn(() => sleep(10s)), timeout: 10ms))");
    assert_eq!((out.as_str(), code), ("Err({kind: 'cancelled}) Err({kind: 'timeout})\n", 0));
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
}

#[test]
fn a_busy_task_does_not_starve_the_others() {
    let (out, _, code) = run("fn spin(n) = n == 0 then 0 else spin(n - 1)\nt = spawn(() => spin(50000000))\nprint(\"other ran\")\ncancel(t)");
    assert_eq!((out.as_str(), code), ("other ran\n", 0));
}

#[test]
fn two_thousand_tasks_run() {
    let (out, _, code) = run("ts = for i in 0..2000: spawn(() => i) ;;\nprint(fold(ts, 0, (a, t) => a + await(t)))");
    assert_eq!((out.as_str(), code), ("1999000\n", 0));
}
```
마지막에서 두 번째 테스트의 `spin`은 태스크 하나가 실행권을 놓지 않으면 끝나지 않는다(차례 넘기기 검사). 2단계의 "`spawn`·`await`·`cancel`·`sleep`는 E0805" 단언(`boot/tests.rs`의 `every_placeholder_gives_e0805_without_a_panic`, `prelude/go_cases.rs`의 동시성 케이스 주석)은 이 작업이 구현한 것만큼 실제 동작 단언으로 바꾼다.

- [ ] **Step 2: 실패 확인** — Run: `cargo test -p fung-vm task_tests` / Expected: FAIL(E0805).
- [ ] **Step 3: 구현** — Go `task.go`·`runtime_conc.go`를 옮긴다. 태스크 스레드는 `std::thread::Builder::new().stack_size(TASK_STACK)`로 띄우고, 시작하자마자 실행권을 얻고, 끝날 때 결과를 적고 자기 값을 실행권을 쥔 채 해제한 뒤 놓는다. 취소는 부모→자식 트리, VM의 호출·뒤로 뛰기 지점과 `block`에서 확인하고 내부 E0701로 되감는다. `sleep`은 `block`의 타임아웃으로(2단계의 `std::thread::sleep`을 대신).
- [ ] **Step 4: 통과 확인** — Run: `FUNG_REQUIRE_PARITY=1 cargo test && cargo clippy --all-targets -- -D warnings` / Expected: PASS. 디버그 빌드의 `assert_holds_token` 단언이 하나도 터지지 않아야 한다.
- [ ] **Step 5: 커밋**
```bash
git add crates
git commit -m "feat(rust): run tasks on threads behind the run token"
```

---

### Task 4: 채널

**Files:**
- Create: `crates/fung-vm/src/vm/channel.rs`, `crates/fung-vm/src/vm/channel_tests.rs`
- Modify: `crates/fung-vm/src/value.rs`(`Value::Channel`), 서식·같음·`type_of`(Go `channel.go`), `crates/fung-vm/src/conc.rs`(`channel`·`send`·`recv`·`close`), `crates/fung-vm/src/native/nat.rs`(순회 도우미 `each`·`items`가 채널을 닫힐 때까지 받는다; Go `internal/nat`), `crates/fung-vm/src/vm/m4.rs`(`for x in ch`이 desugar로 `map`이 되므로 보통은 손댈 것 없음 — 확인)

**Interfaces:**
- Consumes: Task 3의 `block`.
- Produces: `Value::Channel(Rc<Channel>)`. 버퍼 없는 채널은 맞교환(받는 쪽이 가져갈 때까지 `send`가 기다림), 버퍼 있는 채널, `close`, 닫힌 뒤 `Err({kind: 'closed})`, `recv(ch, timeout:)`의 `Err({kind: 'timeout})`, `channel(n)`의 인자 검사(Go와 같은 E0403 문구).
- Go 테스트: `conc_test.go`의 `TestChannels`.

- [ ] **Step 1: 실패하는 테스트**

```rust
use crate::testutil::run;

#[test]
fn a_native_blocked_inside_a_lambda_still_works() {
    let (out, err, code) = run("ch = channel()\nspawn(fn(): each(1..=5, x => send(ch, x)); close(ch) ;;)\nprint(fold(ch, 0, (a, x) => a + x))");
    assert_eq!((out.as_str(), err.as_str(), code), ("15\n", "", 0));
}

#[test]
fn closed_and_buffered_channels_like_go() {
    let (out, _, code) = run("buf = channel(2)\nsend(buf, \"a\")\nclose(buf)\nprint(recv(buf), recv(buf), send(buf, \"b\"), recv(channel(), timeout: 5ms))");
    assert_eq!((out.as_str(), code), ("Ok(\"a\") Err({kind: 'closed}) Err({kind: 'closed}) Err({kind: 'timeout})\n", 0));
}

#[test]
fn cancelling_a_task_blocked_in_send_inside_each_ends_it() {
    let (out, _, code) = run("ch = channel()\nt = spawn(() => each(1..=5, x => send(ch, x)))\ncancel(t)\nprint(await(t))");
    assert_eq!((out.as_str(), code), ("Err({kind: 'cancelled})\n", 0));
}
```

- [ ] **Step 2: 실패 확인** — Run: `cargo test -p fung-vm channel_tests` / Expected: FAIL.
- [ ] **Step 3: 구현** — Go `channel.go`를 옮긴다. 채널 상태는 `World` 실행권 아래, 기다림은 `block`으로.
- [ ] **Step 4: 통과 확인** — Run: `FUNG_REQUIRE_PARITY=1 cargo test && cargo clippy --all-targets -- -D warnings` / Expected: PASS.
- [ ] **Step 5: 커밋**
```bash
git add crates
git commit -m "feat(rust): add channels"
```

---

### Task 5: atom

**Files:**
- Create: `crates/fung-vm/src/vm/atom.rs`, `crates/fung-vm/src/vm/atom_tests.rs`
- Modify: `crates/fung-vm/src/value.rs`(`Value::Atom`), 서식(`<atom v>`)·같음·`type_of`(Go `atom.go`), `crates/fung-vm/src/conc.rs`(`atom`·`swap`·`deref`)

**Interfaces:**
- Consumes: Task 3의 `block`.
- Produces: `Value::Atom(Rc<Atom>)`. `swap(a, f)`이 도는 동안 그 atom을 붙잡고(다른 태스크의 `swap`은 `block`으로 기다림), 같은 태스크의 재진입은 E0703, `deref`는 기다리지 않음. 3b에서 지울 코드이므로 파일 하나에 모은다.
- Go 테스트: `conc_test.go`의 `TestAtoms`, `TestDerefAndPrintInsideSwap`.

- [ ] **Step 1: 실패하는 테스트**

```rust
use crate::testutil::run;

#[test]
fn concurrent_swaps_do_not_lose_updates() {
    let (out, _, code) = run("counter = atom(0)\neach(for _ in 0..8: spawn(() => each(0..50, _ => swap(counter, n => n + 1))) ;;, await)\nprint(deref(counter))");
    assert_eq!((out.as_str(), code), ("400\n", 0));
}

#[test]
fn reentrant_swap_is_e0703() {
    let (_, err, code) = run("a = atom(0)\nswap(a, x => swap(a, y => y))");
    assert!(err.starts_with("error[E0703]: atom swapped inside its own swap"), "{err}");
    assert_eq!(code, 1);
}

#[test]
fn a_swap_that_sleeps_makes_the_others_wait() {
    let (out, _, code) = run("a = atom(0)\nt = spawn(() => swap(a, fn(x): sleep(20ms); x + 1 ;;))\nsleep(5ms)\nswap(a, x => x * 10)\nawait(t)\nprint(deref(a))");
    assert_eq!((out.as_str(), code), ("10\n", 0));
}
```
마지막 테스트는 Go 바이너리로 먼저 확인한다.

- [ ] **Step 2: 실패 확인** — Run: `cargo test -p fung-vm atom_tests` / Expected: FAIL.
- [ ] **Step 3: 구현** — Go `atom.go`를 옮긴다. `open-questions.md` "Go 동등을 위한 과도기 잔가지"에는 넣지 않는다(언어 기능이고, 3b에서 언어 변경으로 지운다).
- [ ] **Step 4: 통과 확인** — Run: `FUNG_REQUIRE_PARITY=1 cargo test && cargo clippy --all-targets -- -D warnings` / Expected: PASS.
- [ ] **Step 5: 커밋**
```bash
git add crates
git commit -m "feat(rust): add atoms as Go has them (removed in stage 3b)"
```

---

### Task 6: 꺼내지 않은 실패와 교착 감지

**Files:**
- Create: `crates/fung-vm/src/vm/failure.rs`, `crates/fung-vm/src/vm/deadlock.rs`, `crates/fung-vm/src/vm/failure_tests.rs`
- Modify: `crates/fung-vm/src/vm/task.rs`(핸들 `Drop`, 결과를 꺼냈는지 표시), `crates/fung-vm/src/vm/block.rs`(막힘·깨어남 집계), `crates/fung-vm/src/vm/runtime.rs`(`report_unretrieved`, `failed`), `crates/fung-vm/src/conc.rs`(`spawn_awaited`는 처음부터 꺼낸 것)

**Interfaces:**
- Consumes: Task 3의 `TaskHandle`·`block`, Task 4·5의 막히는 연산.
- Produces:
  - 꺼내지 않은 실패(05 "아무도 확인하지 않은 태스크의 실패"): 마지막 핸들이 사라질 때 이미 실패했으면 그때, 아직 실행 중이면 실패하는 순간 `task failed:`(E0803, Go와 같은 문구와 호출 경로) 보고. `Runtime::report_unretrieved(&self)`(끝의 훑기), `Runtime::failed(&self) -> bool`. 타임아웃으로 끝난 `await`는 꺼낸 것이 아니다. 핸들의 `Drop`은 실행권을 쥔 스레드에서만 일어난다(규칙). 보고는 실행권 아래에서 `World`의 stderr로.
  - 교착: 살아 있는 태스크 수와 `block`에서 막힌 수, 그중 타이머가 깨울 수를 실행권 아래에서 센다. "전부 막혔고 깨어날 것이 없다"가 되는 순간 E0702 `deadlock: all tasks are waiting`(유예 없음, 스펙 2절). 마지막 실행 태스크가 끝날 때도 판정. `outside`(표준 입력·프로세스) 대기는 세지 않는다. 교착 보고 뒤의 처리(남은 태스크 취소, `at_exit`, 종료 코드 1)는 Go `runtime_conc.go`의 `deadlock`과 같게.
- Go 테스트: `unretrieved_test.go` 전부(`TestDroppedHandleIsReportedByTheGC`는 "핸들을 버린 즉시 보고"로 의도를 옮긴다), `conc_test.go`의 `TestUnawaitedFailureIsReportedOnce`, `TestAwaitOnSelfIsADeadlock`, `TestDeadlockIsDetected`, `TestDeadlockAfterTheLastRunnerFinishes`, `TestAwaitTimeoutDoesNotHideALaterFailure`(Task 3에서 못 끝낸 부분).

- [ ] **Step 1: 실패하는 테스트**

```rust
use crate::testutil::run;

#[test]
fn a_dropped_failed_task_is_reported_once_and_fails_the_run() {
    let (out, err, code) = run("spawn(() => fail(\"x\"))\nsleep(20ms)\nprint(\"entry done\")");
    assert_eq!(out, "entry done\n");
    assert_eq!(err.matches("error[E0803]").count(), 1, "{err}");
    assert_eq!(code, 1);
}

#[test]
fn awaited_and_helper_failures_are_not_reported() {
    let (out, err, code) = run("t = spawn(() => fail(\"x\"))\nr = pmap([1], x => 1 / 0)\nprint(await(t) |> is_err, type_of(r))");
    assert_eq!((out.as_str(), err.as_str(), code), ("true 'list\n", "", 0));
}

#[test]
fn a_channel_nobody_sends_on_is_a_deadlock() {
    let start = std::time::Instant::now();
    let (_, err, code) = run("at_exit(() => print(\"bye\"))\nch = channel()\nrecv(ch)");
    assert!(err.starts_with("error[E0702]: deadlock: all tasks are waiting"), "{err}");
    assert_eq!(code, 1);
    assert!(start.elapsed() < std::time::Duration::from_millis(500));
}
```
`err`의 정확한 전체 문구(호출 경로 포함)는 Go 바이너리 출력과 비교해 같은지 테스트에서 확인한다.

- [ ] **Step 2: 실패 확인** — Run: `cargo test -p fung-vm failure_tests` / Expected: FAIL.
- [ ] **Step 3: 구현** — Go `runtime_conc.go`의 `trackFailure`·`reportFailure`·`ReportUnretrieved`·`deadlock`을 옮긴다. GC 대신 핸들의 `Drop`.
- [ ] **Step 4: 통과 확인** — Run: `FUNG_REQUIRE_PARITY=1 cargo test && cargo clippy --all-targets -- -D warnings` / Expected: PASS.
- [ ] **Step 5: 커밋**
```bash
git add crates
git commit -m "feat(rust): report unretrieved task failures and detect deadlocks"
```

---

### Task 7: 종료 규칙과 CLI, 정답지 비교

**Files:**
- Modify: `crates/fung-vm/src/vm/runtime.rs`(어디서든 `exit`, `at_exit` 핸들러도 살아 있는 태스크, 진입 에러 시 나머지 취소), `crates/fung-vm/src/native/process.rs`·`crates/fung-vm/src/prelude/io.rs`(외부 프로세스·표준 입력 대기를 `outside`로), `crates/fung-vm/src/testutil.rs`(Go `execute` 순서), `crates/fung-cli/src/run.rs`(Go `cmd/fung/pipeline.go`의 `execute`와 같은 순서: `run` → 성공이면 `wait`, 실패면 `cancel_all`+`wait`+보고 → 종료 코드 → `run_exit_handlers`(E0701·E0702 핸들러 에러는 건너뜀) → `wait` → `report_unretrieved` → 종료 코드 규칙), `crates/fung-cli/tests/run_parity_skip.txt`(`concurrency` 9줄 삭제), `crates/fung-cli/tests/run_parity.rs`(최소 건수 올리기, `-e` 사례 추가, E0805 `spawn` 단언 사례를 실제 동작 비교로)

**Interfaces:**
- Consumes: Task 3~6 전부.
- Produces: `fung run`이 동시성 프로그램을 Go와 같은 출력·종료 코드로 실행. `Runtime`의 공개 API: `run`, `wait`, `cancel_all`, `exit_code`, `run_exit_handlers`, `report_unretrieved`, `failed`.
- Go 테스트: `conc_test.go`의 `TestExitFromAnywhereRequestsExit`, `TestLockedWritersKeepLinesWhole`, `TestAtExitAndTestsAreThreadSafe`, `TestStdinIsSharedSafely`.

- [ ] **Step 1: 실패하는 비교와 테스트**

`run_parity_skip.txt`에서 `concurrency` 9줄을 지우고, `run_parity.rs`의 `-e` 사례에 다음을 더한다(모두 Go와 비교):

```rust
"at_exit(() => print(\"bye\"))\nspawn(fn(): sleep(10ms); exit(5) ;;)\nsleep(10s)\nprint(\"never\")",
"at_exit(() => print(\"bye\"))\nch = channel()\nspawn(() => recv(ch))\nspawn(fn(): sleep(10ms); exit(3) ;;)\nrecv(channel())",
"t = spawn(() => each(1..=3, x => sleep(10s)))\nspawn(fn(): sleep(10ms); cancel(t) ;;)\nprint(await(t))",
"spawn(() => fail(\"boom\"))\nfail(\"entry\")",
"print(retry(() => Err(1), times: 2, delay: 1ms), pmap([1, 2, 3], x => x * x, limit: 2), fan_in([]) |> map(x => x))",
"ts = for i in 0..200: spawn(() => i) ;;\nprint(fold(ts, 0, (a, t) => a + await(t)))",
```
그리고 실행권을 놓고 표준 입력을 기다리는 테스트(`fung-vm` 단위 테스트, Go `TestStdinIsSharedSafely`의 의도): 표준 입력을 막아 둔 채 다른 태스크가 출력하고 `exit`할 수 있어야 한다.

- [ ] **Step 2: 실패 확인** — Run: `FUNG_REQUIRE_PARITY=1 cargo test -p fung-cli --test run_parity` / Expected: FAIL.
- [ ] **Step 3: 구현** — Go `execute`의 순서를 옮기고, 차이가 나면 원인 모듈에서 실패하는 단위 테스트를 먼저 더해 고친다. 비교를 약하게 만들지 않는다.
- [ ] **Step 4: 흔들림 검사** — 동시성 골든 테스트와 위 `-e` 사례를 릴리스 빌드로 50번 반복해 결과가 매번 Go와 같은지 확인하는 테스트를 `crates/fung-cli/tests/run_parity.rs`에 `#[ignore]` 없이, 단 `#[cfg(not(debug_assertions))]`로 둔다(반복 횟수 상수 `REPEAT = 50`, 전체 30초 안).
  Run: `FUNG_REQUIRE_PARITY=1 cargo test && FUNG_REQUIRE_PARITY=1 cargo test --release && cargo clippy --all-targets -- -D warnings` / Expected: PASS.
- [ ] **Step 5: 커밋**
```bash
git add crates
git commit -m "feat(rust): run concurrent programs like Go and compare them"
```

---

### Task 8: 벤치마크, 성능 확인, 문서

**Files:**
- Create: `bench/chan.fung`(채널로 값 10만 개 주고받기), `bench/spawn.fung`(태스크 2,000개 띄우고 기다리기), `docs/notes/R3a-runtime.md`
- Modify: `docs/design/open-questions.md`, `docs/design/05-concurrency.md`, `CLAUDE.md`, `crates/fung-cli/tests/run_parity.rs`(새 bench 파일이 비교에 들어가므로 최소 건수 확인)

**Interfaces:**
- Consumes: 완성된 런타임.

- [ ] **Step 1: 벤치마크 파일** — `bench/`의 기존 파일 형식(첫 줄 주석으로 무엇을 재는지)을 따른다. 출력은 결정적이어야 한다(합계 등). `fung fmt --check bench`와 Go `go test ./tests`(포맷·골든 검사)가 통과해야 한다.
- [ ] **Step 2: 측정** — Go(`go build -o /tmp/fung-go ./cmd/fung`)와 Rust 릴리스로 `bench/*.fung` 여섯 개를 각 5회, 가장 빠른 값. 기존 네 개는 2단계 노트(`docs/notes/R2-vm.md` 9절)의 Rust 값보다 5% 넘게 느리면 원인을 찾아 고친다(따로 `perf(rust):` 커밋, 단위 테스트 먼저). 기계 정보(`uname -m`, OS)를 적는다.
- [ ] **Step 3: 학습 노트** `docs/notes/R3a-runtime.md`(한국어, `R2-vm.md`의 문체): 1. 무엇을 만들었나 2. 운영체제 스레드와 그린 스레드 3. 실행권 잠금과 `unsafe`의 근거(지키는 법: 디버그 단언) 4. 조건 변수와 막히는 지점 하나 5. 취소 트리 6. 교착 감지(Go의 50ms와 무엇이 다른가) 7. 참조 계수로 꺼내지 않은 실패 보고하기 8. 실제로 부딪힌 함정(진행 기록과 보고서에 있는 것만) 9. 벤치마크(Go와 비교, 태스크가 많을 때의 비용) 10. 다음(3b, 그린 스레드 재검토).
- [ ] **Step 4: 설계 문서** —
  - `open-questions.md` "Rust 이전": 동시성과 메모리 모델 항목의 스케줄러 문장 옆에 "3a는 운영체제 스레드 + 실행권 잠금(2026-10-15), 그린 스레드는 4단계 이벤트 루프와 함께 재검토", 옮기는 방식 항목에 3a 완료 한 줄(계획·노트 경로, 비교 건수), "2단계에서 알고 남긴 차이"의 `at_exit` 안 `exit` 항목과 동시성 E0805 항목 갱신, 3b 할 일 한 줄. Task 1~7이 더한 과도기 잔가지가 있으면 확인.
  - `05-concurrency.md` "Rust 구현에서 바뀌는 것": 3a 방식 한 줄(꺼내지 않은 실패는 핸들이 사라지는 순간, 교착은 유예 없이).
  - `CLAUDE.md` 현재 상태에 3a 한 줄.
- [ ] **Step 5: 전체 검사와 커밋**

Run: `gofmt -l . ; go vet ./... && go test ./... && cargo fmt --check && cargo clippy --all-targets -- -D warnings && FUNG_REQUIRE_PARITY=1 cargo test && FUNG_REQUIRE_PARITY=1 cargo test --release`
Expected: 모두 PASS.
```bash
git add bench docs CLAUDE.md crates
git commit -m "docs: record Rust port stage 3a with concurrency benchmarks"
```
