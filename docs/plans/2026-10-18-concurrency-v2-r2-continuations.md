# 동시성 v2 R2 (이어 가는 내장 함수) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 람다를 부르는 내장 함수가 Rust 스택 위에서 VM을 한 번 더 돌리지(중첩 `run`) 않게 한다. 대신 "이 함수를 불러 달라, 끝나면 이 상태로 이어 달라"를 돌려주고, 하나뿐인 실행 루프가 그것을 이어 간다. 그래서 R1(M:1 스케줄러)이 람다 안에서 막힌 태스크를 주차할 수 있게 된다.

**Architecture:**
- **이어 가기 프레임**(Lua 5.2 `lua_callk`와 같은 생각). 람다를 부르는 내장 함수는 *단계 내장 함수*(step native)가 된다.
  - 시작할 때 `Flow`를 돌려준다. 바로 끝나면 `Done(값)`, 함수를 불러야 하면 `Call`과 상태 상자(`Box<dyn Cont>`)다.
  - 상자는 태스크의 **이어 가기 줄**(`Task::conts`)에 놓인다. 그것이 부른 함수가 돌아오면 VM이 `resume`을, 에러·취소·`exit`이 그 자리를 지나 되감기면 `unwind`를 부른다.
  - 각각 `Next::Done(값)`이나 다음 `Next::Call`을 돌려준다.
- **인자는 태스크 스택에 바로 쌓는다**(`Task::push_arg`). 원소마다 `Vec`을 만들지 않는다. fung 함수에 매개변수가 딱 맞으면 그 자리가 곧 새 프레임의 칸이다(지금 `call_buf`의 빠른 길과 같음).
- **실행 루프는 하나다.** 실행 시작점(진입 파일, 태스크 시작, `test` 블록, 태스크 끝의 `on_fail` 처리)만 `Task::call_root`로 루프를 연다. `call_root`는 프레임이 비었을 때만 불린다(디버그 단언). `Task::call`·`call_buf`·`run_task`는 지운다.
- 막히는 원시 연산(`recv`·`send`·`sleep`·`await`, 범위 끝의 기다림)은 이번에는 그대로 스레드째 잔다. 주차는 R1이다.

**Tech Stack:** Rust 표준 라이브러리, `fung-vm`(`vm.rs` 실행 루프, `vm/call.rs`, 새 `vm/cont.rs`, `value.rs`의 `Native`, `native/nat.rs`, `prelude.rs`·`prelude/collections.rs`, `vm/cleanup.rs`·`deadline.rs`·`scope.rs`·`atom.rs`·`m6.rs`·`task.rs`).

**Spec:** `docs/superpowers/specs/2026-10-18-concurrency-v2-r7-design.md` (10.1 그린 스레드, 10.2 의미 불변, 10.4 성능, 11절 R단계, **15절 R단계 결정**). 앞 단계 노트 `docs/notes/R7-language.md`, 지금 런타임 `docs/notes/R3a-runtime.md`.

## R단계 지도

스펙 15절의 순서다. 각 단계의 세부 계획은 그 단계를 시작하기 직전에 `docs/plans/2026-10-18-concurrency-v2-r<n>-*.md`로 쓴다. 단계마다 전체 시험(릴리스의 동시성 50회 반복 포함)이 초록이어야 다음으로 간다.

1. **R2 (이 계획)**: 이어 가는 내장 함수. 지금 런타임 위에서 한다.
2. **R1**: 스케줄러. (2026-10-18에 R1a·R1b로 나눔, 스펙 15절. 아래의 이벤트 루프 핵심은 R4로 옮김)
   - 실행 스레드 하나, 실행 대기열, 주차·재개, 타이머 힙.
   - 이벤트 루프의 핵심(`vm/poll/` kqueue·epoll, 깨우개).
   - 채널·`await`·`sleep`·범위 기다림·`swap`·정지 채널을 그린으로. 막히는 원시 연산도 `Flow`/`Next`에 주차를 더해 같은 통로로.
   - 수신자 순회(`Cursor`의 받는 끝)와 `nat::items`의 받기도 주차로.
   - `Token`·`Gate`·`Waiter`·`Census`·태스크 스레드·`stack.rs` 태스크 스택·`try_spawn`을 지운다. 교착 판정은 스펙 15절.
   - `YIELD_EVERY`를 2,000에서 재서 정한다.
   - 임시 다리: `outside`와 소켓 기다림(`Watch`)은 기다림마다 스레드 하나를 띄우고 깨우개로 끝을 알린다.
   - 신호 스레드는 깨우개로 전하고, 신호를 런타임 상태에 남긴다(`fung test` 다음 파일 컴파일 중의 신호).
3. **R3**: 스레드 풀.
   - 최대 64, 10초 놀면 끝남, FIFO, 버려진 작업은 건너뜀.
   - `outside` 전부, 이름 조회, 정규식 컴파일이 대상이다. 기한·취소에 곧바로 되감는다(L2 한계를 닫음).
   - R1의 `outside` 다리를 지운다.
4. **R4**: 이벤트 루프 위의 입출력.
   - rustls는 0.23.45로 짜고 2026-10-28 이후 0.23.46으로 버전만 올린다(2026-10-19 고침; 처음에는 "첫 작업, 그 전에는 기다린다"였다).
   - 소켓을 블로킹하지 않는 모드로, rustls를 그 위에. `Watch`와 R1의 소켓 다리를 지운다.
   - `exec`·`sh`: 파이프를 루프에, 끝은 `EVFILT_PROC`/`pidfd_open`(`ENOSYS`이면 풀 스레드가 `waitpid`). 10 ms 폴링과 읽는 스레드 둘을 지운다.
   - 표준 입력: 전용 스레드 + 깨우개.
   - REPL: 두 번째 Ctrl-C가 멈춘 평가를 버림, `request_stop()` 문서화.
5. **R5**: 옛 런타임의 남은 조각 삭제.
   - `unsafe impl Send` 0개가 목표. `CancelNode`를 원자 변수 없는 단일 스레드 구조로(2026-10-19 고침: 그대로 둔다, 스펙 15절 "취소 노드").
   - 잔가지: E0707이 도우미 이름을 댐, 효과 표 시험이 모든 내장 함수를 분류, E0506의 for 문구.
6. **R6**: 벤치마크와 노트.
   - 10.4의 측정: 태스크 2만 개, 지연, HTTP 동시 연결.
   - `docs/notes/R7-scheduler.md` 마무리, `internals.html`, open-questions, CHANGELOG.
   - 그 뒤 0.3.0(사용자 확인 뒤).

## Global Constraints

- 사용자 방침(메모리 `plan-execution-style`)을 따른다.
  - 작업마다 TDD로 하고 그 작업의 시험만 돌린다. 작업 단위로 커밋하되 전체 커밋 전 검사는 하지 않는다.
  - 모든 작업이 끝난 뒤 한 번 한다: 전체 검사(fmt, clippy, `cargo test --locked --no-fail-fast`를 debug·release 둘 다), 가장 능력 있는 모델의 브랜치 전체 검토, 수정.
- 작업 브랜치는 이 워크트리의 브랜치다. main에는 사용자가 요청할 때만 스쿼시 머지한다. PR은 만들지 않고, 푸시는 사용자가 말할 때만 한다.
- 커밋은 Conventional Commits, 영어, 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **언어 의미를 바꾸지 않는다**(스펙 10.2). 골든(`tests/lang`), 기대 출력(`crates/fung-cli/tests/expected/`, `crates/fung-vm/tests/expected/`), fung-vm 단위 시험이 정답지다. 에러의 번호, 문구, 위치, 호출 경로(줄임 수 포함)가 바이트 단위로 같아야 한다. 기대 출력을 bless하지 않는다. bless가 필요해 보이면 멈추고 사용자에게 묻는다.
- **성능**(스펙 10.4): `bench/`의 계산 위주 넷(`fib`·`loop`·`map`·`str`)과 새 `list`가 이 브랜치의 시작점보다 5% 넘게 느려지지 않는다. 방법은 `bench/README.md`(같은 기계, 준비 1회 + 9회 중앙값).
- 새 크레이트는 없다. 새 `unsafe`도 없다.
- 상태 상자는 원소마다 만들지 않는다. 내장 함수 호출 한 번에 `Box` 하나다. 원소마다 하는 할당은 지금과 같아야 한다(결과 리스트에 넣기 정도).
- 동시성 시험은 상태로 순서를 정한다(`testutil::exec_waiting`의 `until`·`blocked`·`queued`·`buffered`·`finished`).
- 코드·주석·커밋은 영어, 노트는 한국어. 학습 노트는 `docs/notes/R7-scheduler.md`(이 계획에서 새로 만든다).

## Review Focus

- **콜백 안의 에러 보고.** `map([1, 0], x => 10 / x)`, `filter(xs, x => 1)`(E0505), `fold(xs, 0, (a, x) => a.nope)`가 지금과 같은 위치·호출 경로로 보고된다. → Task 3의 `callback_errors_report_as_before`(지금 바이너리의 출력을 먼저 기대값으로 적고 시작한다).
- **`acquire`와 `use` 사이의 취소.** `acquire`가 돌아온 뒤 `use`를 부르기 직전 차례 확인에서 취소되어도 `release`가 돈다. → Task 4의 `a_cancel_before_use_still_releases`.
- **꼬리 위치의 단계 내장 함수.** `fn f(xs) = map(xs, g)`를 깊이 부르고, 그 안에서 에러를 내 호출 경로의 줄임 수를 본다. → Task 2의 `a_step_native_in_tail_position_returns_to_the_caller`.
- **콜백이 내장 함수일 때.** `map(xs, str)`(보통 내장 함수), `map([s => 1], scope)`(단계 내장 함수가 바로 콜백. 같은 깊이에 이어 가기 둘). → Task 2·Task 5.
- **콜백 안의 패닉과 `exit`.** 콜백 안 내장 함수의 패닉은 E0801이지 프로세스 중단이 아니다. `with` 안의 콜백에서 `exit(3)`하면 `release`가 돌고 종료 코드가 3이다. → Task 2의 `a_panic_inside_a_callback_is_e0801`, Task 4의 `exit_inside_a_callback_releases`.

---

### Task 1: 벤치마크 기준선과 노트 뼈대

**Files:**
- Create: `bench/list.fung`, `scripts/bench.sh`, `docs/notes/R7-scheduler.md`
- Modify: `bench/README.md`(표에 `list`, `scripts/bench.sh` 사용법)

**Interfaces:**
- Produces: `scripts/bench.sh <바이너리>...`. 파일마다 바이너리마다 준비 1회 + 9회 실행의 벽시계 중앙값(초, 소수 셋째 자리)을 표로 찍는다. 출력은 버린다. 시간은 `bench/README.md`처럼 `python3 -c` `perf_counter`로 잰다. 파일 목록은 `bench/*.fung`이다.
  - `repo_rust_only.rs`가 스크립트 언어를 막는지 먼저 확인한다. 막으면 같은 일을 `bash`만으로 하고, 정밀도를 노트에 적는다.

- [ ] **Step 1: `bench/list.fung`을 쓴다.** 리스트 10만 개를 만들고 `map`·`filter`·`each`·`find`·`sort_by`를 한 번씩 거친다. 결과 하나를 출력한다. 단계 내장 함수의 리스트 순회를 잰다(`loop`·`map`은 범위를 돈다).
- [ ] **Step 2: `scripts/bench.sh`를 쓰고 이 브랜치의 시작점을 잰다.**
  - Run: `cargo build --release --locked -p fung-cli && cp target/release/fung /tmp/fung-r2-base && scripts/bench.sh /tmp/fung-r2-base`
  - Expected: 7개 파일의 중앙값 표
- [ ] **Step 3: `docs/notes/R7-scheduler.md`의 뼈대를 쓴다.** 머리(스펙·계획 경로, R단계 지도 한 단락), "R2" 절의 1소절 "무엇이 문제였나", 기준선 표(커밋, 기계, 숫자). 나머지 소절은 Task 6에서 채운다.
- [ ] **Step 4: 커밋한다.** `chore(bench): add list benchmark and runner, record the R2 baseline`

### Task 2: 이어 가기 장치

**Files:**
- Create: `crates/fung-vm/src/vm/cont.rs`, `crates/fung-vm/src/vm/cont_tests.rs`
- Modify:
  - `crates/fung-vm/src/value.rs`(`NativeBody`)
  - `crates/fung-vm/src/vm.rs`(`Task` 필드, `exec`의 반환·되감기)
  - `crates/fung-vm/src/vm/call.rs`(`enter`가 단계 내장 함수를 시작, `call_root`, `push_arg`)
  - `crates/fung-vm/src/native/nat.rs`(`def_step`·`native_step`)
  - `crates/fung-vm/src/testutil.rs`(탐침 `loop_depth`)
  - `(n.func)`을 직접 부르는 시험 6곳(`native/*_tests.rs`, `prelude/tests.rs`)
  - `crates/fung-vm/src/vm/tests.rs`의 `natives_call_closures_through_the_task`(시험용 `twice`를 단계 내장 함수로)

**Interfaces:**
- Produces:
  - `value.rs`
    - `pub type StepFn = fn(&mut Task<'_>, Vec<Value>) -> Result<Flow, Stop>;`
    - `pub enum NativeBody { Plain(NativeFn), Step(StepFn) }`
    - `Native { name, params, body: NativeBody }`. `Native::leak(name, params, func: NativeFn)`은 그대로 `Plain`을 만든다.
    - `Native::leak_step(name, params, step: StepFn) -> &'static Native`
    - `Native::plain(&self) -> NativeFn`: 시험용. `Step`이면 패닉.
  - `vm/cont.rs`
    - `pub(crate) trait Cont { fn resume(&mut self, t: &mut Task<'_>, v: Value) -> Result<Next, Stop>; fn unwind(&mut self, t: &mut Task<'_>, stop: Stop) -> Result<Next, Stop> { Err(stop) } }`
    - `pub(crate) enum Next { Done(Value), Call(Value, usize) }`: `Call(f, n)`은 "`f`를, 내가 `push_arg`로 쌓은 인자 `n`개로 불러 달라"다.
    - `pub enum Flow { Done(Value), Call(Box<dyn Cont>, Value, usize) }`: 시작할 때의 같은 두 갈래. 상자는 `Call`일 때만 만든다.
    - `pub(super) struct ContFrame { depth: usize, height: usize, pos: Pos, native: &'static Native, tail: bool, k: Box<dyn Cont> }`
      - `depth`: 놓일 때의 `frames.len()`
      - `height`: 인자를 뺀 스택 높이
  - `Task`
    - `conts: Vec<ContFrame>`
    - `pub(crate) fn push_arg(&mut self, v: Value)`
    - `pub(crate) fn call_root(&mut self, f: &Value, args: Vec<Value>) -> Result<Value, Stop>`: 지금 `call`의 몸통이다. 시작에 `debug_assert!(self.frames.is_empty() && self.conts.is_empty())`.
    - 시험용(`#[cfg(test)]`) `loops: u32`: 지금 돌고 있는 `exec`의 수
  - `nat::def_step(name, StepFn, params) -> Value`, `nat::native_step(name, StepFn, params) -> &'static Native`
  - 탐침 `loop_depth()`: 이 태스크에서 돌고 있는 실행 루프 수(`Value::Int`)

**알고리즘** (`exec` 안. 지금의 `Return`·`TailCall`·에러 반환을 바꾼다):
- **시작.** `enter`가 `NativeBody::Step`을 만나면 `effect` 검사와 인자 묶기를 지금처럼 하고 `step`을 부른다.
  - `Flow::Done(v)`는 보통 내장 함수의 결과처럼 다룬다.
  - `Flow::Call(k, f, n)`이면 `ContFrame`을 `depth = frames.len()`, `height = stack.len() - n`, `tail`(TAILCALL이었나), `pos`(호출 위치)로 **먼저 놓은 뒤** `issue(f, n)`한다.
- **`issue(f, n)`**: `tick(cont.pos)`, 그다음 불린 것에 따라 나뉜다.
  - 매개변수가 딱 맞는 fung 함수: 쌓인 인자 자리에 프레임을 바로 올린다.
  - 보통 내장 함수: 인자를 꺼내 바로 부르고, 결과를 `deliver`한다.
  - 단계 내장 함수: 시작으로 간다(같은 깊이에 이어 가기가 하나 더 놓인다).
  - 나머지: 인자를 꺼내 `enter`로 간다.
  - `tick`이나 `enter`의 에러는 아래 되감기로 간다. 이어 가기가 이미 놓여 있으므로 그것의 `unwind`가 본다.
- **`deliver(v)`**: 프레임이 돌아와 `frames.len()`이 맨 위 이어 가기의 `depth`와 같아질 때, 또는 보통 내장 함수의 결과가 나왔을 때 부른다. 다음을 되풀이한다.
  - 맨 위 이어 가기의 `depth == frames.len()`이면, `t.native = cont.native`로 두고 `resume(v)`를 부른다.
    - `Next::Call`이면 `issue`하고 멈춘다.
    - `Next::Done(w)`이면 그 이어 가기를 빼고 스택을 `height`로 맞춘다. `tail`이었으면 `w`로 `ret`하고, 아니면 `w`를 다음 이어 가기에 넘긴다.
  - 이어 가기가 없으면 지금처럼 호출자의 스택에 `push`한다.
  - `ret`의 반환 경로는 `frames.len() == 맨 위 이어 가기 depth`일 때만 `deliver`로 간다. 이어 가기가 없으면 `usize::MAX`와 비교하는 한 번의 비교만 늘어야 한다(뜨거운 길).
- **되감기.** 명령, `issue`, `resume`, `unwind`가 에러(`Stop`)를 내면 다음을 한다.
  1. `Stop::Error`이면 지금의 `attach`(호출 경로, extras 위치 옮기기)를 그 자리의 프레임으로 **한 번** 한다. `resume`·`unwind`가 낸 위치 없는 에러는 `cont.pos`로 위치를 단다(지금 `enter`의 `at(d, pos)`와 같음). `Stop::Exit`이면 지금처럼 `request_exit`한다.
  2. 맨 위 이어 가기가 이 루프의 `stop`보다 깊으면, `frames`를 그 `depth`로, `stack`을 `height`로 자른다. 그다음 이어 가기를 꺼내 `unwind(stop)`를 부른다.
     - `Next::Call`이면 다시 놓고 `issue`한다.
     - `Done(w)`이면 `deliver(w)` 규칙으로 간다.
     - `Err(s)`이면 `s`로 1부터 되풀이한다.
  3. 없으면 지금처럼 `Err`로 루프를 나간다(`call_root`가 프레임을 정리한다).
- 패닉 울타리(`guarded`)는 `call_root`와 `run_main`에만 남는다. 콜백 안의 패닉도 같은 울타리에 닿는다(중첩 루프가 없으므로).

- [ ] **Step 1: 실패하는 시험을 쓴다** (`cont_tests.rs`. 시험용 단계 내장 함수 `twice(f)`(`f(1)`의 결과로 `f`를 한 번 더), `call_with(f, x)`(`f(x)` 한 번), `probe_after(f)`(되감기를 지나면 표준 출력에 `unwound`)를 이 파일에서 만든다)
  - `a_step_native_calls_back_in_the_same_loop`: `print(twice(x => x + loop_depth()))` → `"3\n"`(두 번 다 루프 깊이 1)
  - `step_natives_nest_at_the_same_depth`: `print(call_with(twice, x => x + 1))` → `"3\n"`. 단계 내장 함수 `twice`가 바로 콜백이라 같은 깊이에 이어 가기 둘이 쌓인다.
  - `a_plain_native_as_the_callback`: `print(twice(abs))`(보통 내장 함수가 바로 콜백) → `"1\n"`
  - `a_step_native_in_tail_position_returns_to_the_caller`: `fn g(n) = twice(x => x + n)` 다음 `print(g(1) + g(2))` → `"8\n"`. 꼬리 재귀 1,000단계 안에서 `twice`의 콜백이 `fail("x")`를 내면, 호출 경로의 줄임 수가 같은 프로그램을 보통 함수로 쓴 것과 같다.
  - `errors_unwind_through_a_step_native`: `probe_after(() => 1 / 0)` → stdout `"unwound\n"`, E0502, 위치는 `1 / 0`, 코드 1
  - `exit_unwinds_through_a_step_native`: `probe_after(() => exit(4))` → `"unwound\n"`, 코드 4
  - `a_panic_inside_a_callback_is_e0801`: `twice(x => panic_probe())`(시험 전용 패닉 내장 함수) → E0801, 프로세스 중단 아님
  - `call_root_refuses_a_nested_loop`(debug 빌드만, `#[should_panic]`): 콜백 안에서 `call_root`를 부르는 시험 내장 함수
  - 바꾼 `natives_call_closures_through_the_task`가 지금과 같은 결과를 낸다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm cont_tests` → `NativeBody`·`loop_depth`가 없어 컴파일 실패
- [ ] **Step 3: 구현한다.** 위 Interfaces와 알고리즘대로. `Task::call`은 아직 지우지 않는다(Task 5에서 지운다). 이 작업에서는 `call_root`가 `call`의 새 이름이고, `call`은 Task 3~5가 옮기는 동안만 남는 옛 길이다.
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- cont_tests vm::tests` → PASS
- [ ] **Step 5: 커밋한다.** `feat(vm): continuation frames for natives that call back into fung`

### Task 3: 순회하는 내장 함수

**Files:**
- Modify: `crates/fung-vm/src/native/nat.rs`(`Cursor`), `crates/fung-vm/src/prelude.rs`(`map`), `crates/fung-vm/src/prelude/collections.rs`(`filter`·`fold`·`each`·`find`·`any`·`all`·`group_by`·`sort_by`)
- Test: `crates/fung-vm/src/vm/cont_tests.rs`, `crates/fung-vm/src/prelude/tests.rs`

**Interfaces:**
- Consumes: Task 2의 `StepFn`·`Cont`·`Next`·`Flow`·`push_arg`·`nat::def_step`
- Produces:
  - `pub(crate) enum Cursor`: 리스트(인덱스), 범위(시작·길이·인덱스), 받는 끝
    - `Cursor::new(v: &Value, verb: &str) -> Result<Cursor, Stop>`: 지금 `check_items`·`not_iterable`과 같은 에러
    - `fn next(&mut self, t: &mut Task<'_>) -> Result<Option<Value>, Stop>`
    - 리스트는 원소마다 상각 O(1)이어야 한다. `List::flat()`이 있으면 그 조각을 쓰고, 없으면 잎 단위로 넘긴다. `at(i)`를 원소마다 부르지 않는다.
    - 받는 끝은 지금처럼 `channel::recv`로 받는다(R1에서 주차로 바뀐다).
  - `nat::each`·`each_until`은 콜백이 Rust 클로저인 보통 내장 함수(`join`, `flatten`)를 위해 남기고, 안을 `Cursor`로 바꾼다.
  - 내장 함수마다 하나의 `Cont` 구조체(예: `MapK { cur: Cursor, f: Value, out: ListBuilder }`). `fold`는 누산기를 상자에 두고 원소마다 `push_arg(acc)`·`push_arg(x)`로 옮긴다. 복사하지 않는다.

- [ ] **Step 1: 실패하는 시험을 쓴다** (`cont_tests.rs`)
  - `iterating_natives_call_back_in_one_loop`: 아홉 함수 각각에서 콜백이 `loop_depth()`를 보고 1인지 확인한다. 예: `print(map([1, 2], x => loop_depth()))` → `"[1, 1]\n"`, `print(fold(0..3, 0, (a, x) => a + loop_depth()))` → `"3\n"`, `sort_by`·`group_by`·`find`·`any`·`all`·`filter`·`each` 같은 꼴
  - `callback_errors_report_as_before`: Review Focus의 세 프로그램. 기대값은 **바꾸기 전 바이너리**로 먼저 돌려 그 stderr를 그대로 적는다.
  - `iterating_a_receiver_still_works`: `scope`에서 생산자가 `0..3`을 보내고, `print(fold(rx, 0, (a, x) => a + x))` → `"3\n"`
  - `a_large_list_is_walked_in_linear_time`: 원소 20만 개 리스트(조각 `xs[1..]` 포함)를 `each`로 돈다. 릴리스 전용 `perf` 시험의 꼴로 0.3초 안이다. 기존 `crates/fung-cli/tests/perf.rs`에 사례를 더한다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm cont_tests` → `loop_depth`가 2인 사례들이 FAIL
- [ ] **Step 3: 구현한다.** 아홉 함수를 단계 내장 함수로 바꾼다. 결과·에러·검사 순서(인자 검사 → 원소 한도 → 순회)는 지금과 같다. `call_bool`의 E0505 검사는 `resume` 안에서 한다.
- [ ] **Step 4: 통과를 확인한다.**
  - Run: `cargo test -p fung-vm -- cont_tests prelude && cargo test -p fung-cli --test golden --test run_expected` → PASS
  - Run: `cargo test --release -p fung-cli --test perf` → PASS
- [ ] **Step 5: 커밋한다.** `refactor(prelude): iterating natives continue instead of nesting the run loop`

### Task 4: `with`·`timeout`·`swap`

**Files:**
- Modify: `crates/fung-vm/src/vm/cleanup.rs`, `crates/fung-vm/src/vm/deadline.rs`, `crates/fung-vm/src/vm/atom.rs`, `crates/fung-vm/src/conc.rs`(정의를 `def_step`으로), 그리고 `swap`을 정의하는 파일
- Test: `crates/fung-vm/src/vm/cleanup_tests.rs`, `deadline_tests.rs`, `atom_tests.rs`, `cont_tests.rs`

**Interfaces:**
- Consumes: Task 2의 장치
- Produces: 세 단계 내장 함수. 모듈 머리 주석의 규칙(에러를 합치는 순서, 차단 노드, 기한의 결과)은 그대로이고, 다음이 단계로 나뉜다.
  - `with`: `Acquire` → `Use(r)` → `Release(r, use의 결과)`
    - `Use`에서 `unwind`가 오면 그 `Stop`을 결과로 들고 `Release`로 간다.
    - `Release`에 들어갈 때 차단 노드로 바꿔 끼우고 `World::shields`에 올린다. `resume`·`unwind`에서 되돌린다.
    - `Acquire`의 `unwind`는 그대로 올린다.
  - `timeout`: 시작할 때 노드를 바꿔 끼운다. `resume`·`unwind` 둘 다 노드를 되돌리고 지금의 판정(`timed_out && !outer_cancelled` → `Err({kind: 'timeout})`)을 한다.
  - `swap`: 시작과 재시도마다 `pure += 1`. `resume`·`unwind`는 `pure -= 1`부터 한다. 판 번호가 바뀌었으면 `Next::Call(f, 1)`로 다시 돈다.

- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `cont_tests.rs`의 `cleanup_natives_call_back_in_one_loop`: `with`의 세 함수, `timeout`의 본문, `swap`의 `f`에서 `loop_depth()`가 1이다.
  - `cleanup_tests.rs`의 `a_cancel_before_use_still_releases`: `acquire`가 자기 태스크를 취소한다(`spawn`된 태스크가 자기 핸들을 채널로 받아 `cancel`). 다음 차례 확인이 `use`의 호출 직전에 걸린다 → `release`의 출력이 있고 `await`는 `Err({kind: 'cancelled})`
  - `cleanup_tests.rs`의 `exit_inside_a_callback_releases`: `with(() => 1, r => each([1], x => exit(3)), r => print("released"))` → `"released\n"`, 코드 3
  - 기존 `cleanup_tests`·`deadline_tests`·`atom_tests`(특히 `concurrent_swaps_lose_no_update`)는 그대로 통과해야 한다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- cont_tests cleanup_tests` → `loop_depth` 사례 FAIL
- [ ] **Step 3: 구현한다.**
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- cleanup_tests deadline_tests atom_tests cont_tests && cargo test -p fung-cli --test golden` → PASS
- [ ] **Step 5: 커밋한다.** `refactor(vm): with, timeout and swap continue instead of nesting the run loop`

### Task 5: 범위와 실행 시작점, 옛 길 지우기

**Files:**
- Modify:
  - `crates/fung-vm/src/vm/scope.rs`(`run_scope`의 본문 호출. `handle`은 시작점으로 남음)
  - `crates/fung-vm/src/vm/task.rs`(`run_spawned`가 `call_root`, `run_task` 삭제)
  - `crates/fung-vm/src/vm/m6.rs`(`call_test`가 `call_root`)
  - `crates/fung-vm/src/vm/call.rs`(`call`·`call_buf` 삭제)
  - `crates/fung-vm/src/vm.rs`(`run`의 `#[inline(always)]` 주석을 새 사정에 맞게)
- Test: `crates/fung-vm/src/vm/scope_tests.rs`, `cont_tests.rs`

**Interfaces:**
- Consumes: Task 2~4
- Produces:
  - `scope`·`supervise`: 단계 내장 함수다. 본문 호출 뒤(`resume`·`unwind` 둘 다)에 지금의 `run_scope` 뒷부분을 그대로 한다: 노드 되돌리기, 실패 시 `node.cancel()`, `drain`(아직 스레드째 기다림), `open = false`, 미처리 실패 보고, `settle`.
  - `supervise`의 `on_fail` 처리(`Task::handle`)는 태스크가 끝난 뒤 프레임이 빈 상태에서 도는 시작점이라 `call_root`를 쓴다. 주차는 R1에서 한다.
  - `Task::call`·`call_buf`·`run_task`가 없어진다. `t.call(`·`call_buf(`를 부르는 곳이 저장소에 남지 않는다.

- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `cont_tests.rs`의 `a_scope_body_runs_in_the_same_loop`: `print(scope(s => loop_depth()))` → `"1\n"`, `supervise`도 같다.
  - `cont_tests.rs`의 `a_scope_as_a_direct_callback`: `print(map([s => 1, s => 2], scope))` → `"[1, 2]\n"`
  - `cont_tests.rs`의 `no_native_nests_the_run_loop`: 저장소 시험. `crates/fung-vm/src` 아래(시험 파일 제외)에 `.call(&`·`call_buf(`·`run_task(`가 없고, `call_root(`는 `vm.rs`·`task.rs`·`m6.rs`·`scope.rs`의 시작점에만 있다. 파일을 읽어 문자열로 본다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm cont_tests` → FAIL
- [ ] **Step 3: 구현한다.**
- [ ] **Step 4: 통과를 확인한다.**
  - Run: `cargo test -p fung-vm && cargo test -p fung-cli --test golden --test run_expected --test tools_expected` → PASS
  - Run: `cargo test --release -p fung-cli --test run_expected concurrent_runs_are_stable` → PASS(동시성 50회 반복)
- [ ] **Step 5: 커밋한다.** `refactor(vm): scopes continue too; only entry points open a run loop`

### Task 6: 측정과 문서

**Files:**
- Modify:
  - `docs/notes/R7-scheduler.md`(R2 절을 채움)
  - `docs/internals.html`(VM 절의 "네이티브가 fung을 부르는 길", 동시성 절의 스택 설명)
  - `docs/notes/R3a-runtime.md`는 고치지 않는다(기록)
  - `docs/design/open-questions.md`("구현 언어 지침"의 고차 함수 줄: R7에서 Rust에 이어 가기로 남김, 2026-10-18)
  - `docs/design/09-implementation-plan.md`(호출 규약에 이어 가기 프레임 한 단락, 해당 절이 있으면)

- [ ] **Step 1: 잰다.** Run: `cargo build --release --locked -p fung-cli && scripts/bench.sh /tmp/fung-r2-base target/release/fung`
  - Expected: `fib`·`loop`·`map`·`str`·`list`가 기준선 대비 +5% 이하. 넘으면 멈춘다.
  - 원인을 프로파일러(`sample`)로 찾는다. 인라인 판단(R3a 9절의 `Task::run` 사례), `deliver`의 비교, `Cursor`의 리스트 순회를 의심한다.
  - 고쳐도 넘으면 사용자에게 숫자와 함께 묻는다.
- [ ] **Step 2: 노트를 채운다.** R2 절에 들어갈 것:
  - 중첩 `run`이 왜 그린 스레드를 막는가: Rust 스택에 VM 프레임이 섞여 저장할 수 없음
  - `lua_callk`와 이어 가기 프레임, 되감기가 이어 가기를 지나는 순서
  - 인자를 스택에 바로 쌓는 이유(원소마다 할당 없음)
  - fung으로 옮기지 않은 이유(`for` = `map`, 5% 목표)
  - 덤: 네이티브를 거친 깊은 재귀가 Rust 스택을 쓰지 않음
  - 측정 표(전후)
  - 매뉴얼은 `internals.html`의 해당 문단만 고친다(`manual.rs`가 지키는 예제는 바뀌지 않음).
- [ ] **Step 3: 확인한다.** Run: `cargo test -p fung-cli --test manual --test repo_rust_only` → PASS
- [ ] **Step 4: 커밋한다.** `docs: R2 continuations note, internals and measurements`

### 끝에 한 번 (사용자 방침)

- [ ] 전체 검사: `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`(debug·`--release`), `cargo test --locked --no-fail-fast`(debug·`--release`), `cargo deny check`·`cargo audit`(설치되어 있으면)
- [ ] 가장 능력 있는 모델로 브랜치 전체 검토(`superpowers:requesting-code-review`). 검토자에게 이 계획, 스펙 10·15절, Review Focus를 준다.
- [ ] 검토 결과를 한 번에 고치고, 고친 부분의 시험과 전체 검사를 다시 돌려 커밋한다.
- [ ] 사용자에게 보고: 무엇을 만들었고 어떤 원리인지, 측정 숫자, 다음은 R1 계획. 스쿼시 머지는 사용자가 요청할 때만.
