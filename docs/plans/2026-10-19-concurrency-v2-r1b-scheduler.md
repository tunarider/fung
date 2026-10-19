# 동시성 v2 R1b (M:1 스케줄러) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 태스크를 운영체제 스레드에서 떼어 낸다.
- 명령을 부른 스레드 하나가 실행 대기열의 태스크를 차례로 돌린다.
- 태스크는 주차하면 데이터(`Fiber`)로 치워지고, 깨어나면 같은 자리에서 이어 간다.
- `spawn`은 스레드를 만들지 않으므로 실패하지 않고, 기다리는 태스크 수만 개가 돈다.

**Architecture:**
- **작업 1~4: 지금 런타임 위에서 남은 "Rust 위의 기다림"을 없앤다.** 각각 끝나면 전체 시험이 초록이다.
  1. 태스크의 끝과 `supervise`의 `on_fail` 처리를 이어 가기 상자로 바꾼다.
  2. 표준 입력을 읽기 스레드 위의 주차로 바꾼다.
  3. 차례 넘기기를 "실행 루프에서 나와 달라"는 신호로 바꾼다.
  4. offload의 일을 다른 스레드에 맡기고 주차한다.
- **작업 5: 런타임 교체.**
  - `Task` = 달리는 태스크의 모습(`Fiber` + 세계 + 공유 부분), `Fiber` = 치워 둔 태스크다.
  - 스케줄러(`vm/sched.rs`)가 대기열·타이머·교착 판정을 가진다.
  - 다른 스레드는 받은 편지함과 깨우개(`Mutex`+`Condvar`)로 알린다.
  - 태스크 스레드, 실행권 줄(`Gate`), `Census`, `Handoff`, 태스크 스택, `try_spawn`, `Runtime::hold`를 지운다.
- **작업 6: `YIELD_EVERY`를 재서 정하고, 10.4의 측정과 문서.**
- `World`의 잠금은 남긴다. 실행 스레드가 실행 내내 쥔다(스펙 15절, R5에서 지움).

**Tech Stack:** Rust 표준 라이브러리. `fung-vm`(`vm.rs`, `vm/{call,cont,block,task,scope,stdin,stop,deadlock,world,runtime,m6,module,channel}.rs`, 새 `vm/sched.rs`, `native/nat.rs`, `signals.rs`, `stack.rs`, `conc.rs`), `fung-cli`(`run.rs`·`test.rs`·`repl/mod.rs`), `extras/http.fung`.

**Spec:** `docs/superpowers/specs/2026-10-18-concurrency-v2-r7-design.md` (10절, 10.4, 15절의 R1b 네 줄). 앞 단계 계획 `docs/plans/2026-10-18-concurrency-v2-r1a-park.md`, 노트 `docs/notes/R7-scheduler.md`의 R1a 6절("R1b가 바꿀 곳").

## Global Constraints

- 사용자 방침(메모리 `plan-execution-style`)을 따른다.
  - 작업마다 TDD로 하고 그 작업의 시험만 돌린다. 작업 단위로 커밋한다.
  - 끝에 한 번만 한다: 전체 검사(fmt, clippy, `cargo test --locked --no-fail-fast`를 debug·release 둘 다), 가장 능력 있는 모델의 브랜치 전체 검토, 수정.
- 사용자와의 대화·보고는 한국어. 코드·주석·커밋은 영어. 커밋 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **언어 의미를 바꾸지 않는다.** 골든, 기대 출력, 단위 시험이 정답지다. 기대 출력을 bless하지 않는다. 필요해 보이면 멈추고 묻는다.
  - 예외는 하나다. 스레드를 만들지 못해 나던 E0801 `cannot start a task`와 `try_spawn`이 사라진다(스펙 10.1). 그 시험은 지운다.
- **보장하지 않던 순서는 바뀔 수 있다**(스펙 10.2). 형제의 시작 순서 같은 것이다. 그런 순서에 기대는 시험이 깨지면 상태로 순서를 정하게 고치고(`testutil`의 탐침), 판단으로 기록한다. 출력 문구나 에러를 바꿔서 맞추지 않는다.
- **성능**: 계산 위주(`fib`·`loop`·`map`·`str`·`list`)는 이 브랜치 시작점보다 5% 넘게 느려지지 않는다. 10.4의 목표(태스크 하나 5 µs 이하, 버퍼 없는 채널 값 하나 1 µs 이하, 기다리는 태스크 2만 개)는 작업 6에서 잰다.
- 새 크레이트도 새 `unsafe`도 없다. `Handoff`의 `unsafe impl Send`는 지운다.
- **offload 중의 취소는 지금과 같다**(스펙 15절). 일이 끝날 때까지 기다린 뒤, 기한이 지났으면 만료시키고 마무리를 돈다. 소켓 기다림(`Watch`)과 외부 명령은 지금처럼 취소 훅이나 폴링이 일을 끊는다.
- 동시성 시험은 상태로 순서를 정한다. 릴리스의 동시성 50회 반복(`concurrent_runs_are_stable`)이 통과해야 한다.

## Review Focus

- **잃어버린 깨움.** 다른 스레드(표준 입력 읽기 스레드, offload의 일)가 태스크를 깨우는 순간과 그 태스크가 주차하는 순간이 겹친다. 깨움은 사라지지 않아야 한다. → 작업 5의 `a_wake_from_another_thread_before_the_park_is_kept`
- **계산하는 태스크 옆의 다른 태스크.** 호출 없는 콜백의 `map` 1천만 원소, 보통 내장 함수 콜백(`map(xs, str)`), 뒤로 가는 점프만 도는 반복 옆에서도 다른 태스크가 차례를 받는다. → 작업 3의 `every_yield_point_lets_others_run`
- **신호가 실행 중에 온다.** 계산만 하는 진입 파일에 SIGINT가 오면 실행권을 기다리지 않고 처리되어 130으로 끝나고, 두 번째 신호는 곧바로 끝낸다. 실행하지 않을 때 온 신호도 사라지지 않는다. → 작업 5의 `a_signal_during_a_busy_entry_stops_it`, `a_stop_while_idle_is_kept_for_the_next_run`
- **교착 판정.** 주차한 태스크만 남으면 E0702다. 타이머·offload·표준 입력·정지 채널을 기다리는 태스크가 있으면 교착이 아니다. `with`의 `release` 안의 교착(차단 노드)도 지금처럼 판정된다. → 작업 5의 `deadlock_is_judged_by_the_scheduler` 묶음과 기존 `deadlock`·`cleanup` 시험
- **태스크 2만 개.** 기다리는 태스크 2만 개가 스레드 한도 없이 돌고, 그 사이 채널 주고받기와 취소가 맞다. → 작업 5의 `twenty_thousand_waiting_tasks`(릴리스 전용)

---

### Task 1: 태스크의 끝과 `on_fail` 처리기를 이어 가기로

**Files:**
- Modify: `crates/fung-vm/src/vm/task.rs`(`run_spawned`, 새 `TaskEnd` 상자), `crates/fung-vm/src/vm/scope.rs`(`Task::handle`·`end_in_scope`를 상자의 단계로), `crates/fung-vm/src/vm/call.rs`(`call_root_with`)
- Test: `crates/fung-vm/src/vm/cont_tests.rs`(저장소 시험의 `call_root` 목록), `crates/fung-vm/src/vm/scope_tests.rs`

**Interfaces:**
- Produces:
  - `Task::call_root_with(&mut self, f: &Value, args: Vec<Value>, end: Box<dyn Cont>) -> Result<Value, Stop>`: `call_root`과 같되, 시작 전에 `end`를 깊이 0의 상자로 놓는다(`native: None`, `pos`는 `no_pos()`). `f`의 결과·되감기가 그 상자로 간다.
  - `vm/task.rs`의 `TaskEnd` 상자: 지금 `run_spawned`가 `call_root` 다음에 하는 일을 그 순서대로 한다.
    1. 결과 저장(`outcome`)과 기다리는 쪽 깨우기
    2. 범위에 알리기
       - `supervise` 범위의 진짜 실패면 대기열에 넣는다. 처리하는 쪽이 없으면 이 태스크가 처리기가 된다.
       - 처리 노드로 바꿔 끼우고, 대기열이 빌 때까지 `Next::Call(on_fail, 1)`을 하나씩 부른다.
       - `on_fail`의 진짜 에러는 범위를 실패시키고 멈춘다. 취소로 끊긴 실패는 대기열 앞으로 되돌린다.
       - 지금 `Task::handle`의 규칙 그대로다.
    3. `child_done`, 노드 취소, `running -= 1`
  - `Task::handle`과 `end_in_scope`가 그 상자의 단계가 된다. `run_spawned`는 `call_root_with` 하나만 부른다.

- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `cont_tests::no_native_nests_the_run_loop`의 `call_root(` 목록에서 `vm/scope.rs`를 뺀다(처리기가 더는 루프를 열지 않음). 이것이 RED다.
  - `scope_tests`에 `an_on_fail_that_waits_still_handles_in_order`: 자식 셋이 차례로 실패한다. `on_fail`은 채널로 받은 신호를 기다린 뒤 실패를 출력한다. 실패 순서대로 하나씩 출력되고, 범위는 셋 다 처리된 뒤 끝난다. 동작 고정이라 지금도 통과해야 한다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- cont_tests::no_native_nests` → FAIL
- [ ] **Step 3: 구현한다.**
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- cont_tests scope_tests task_tests failure_tests cleanup_tests && cargo test -p fung-cli --test http` → PASS
- [ ] **Step 5: 커밋한다.** `refactor(vm): a task's end and its supervisor's handler continue`

### Task 2: 표준 입력을 읽기 스레드 위의 주차로

**Files:**
- Modify: `crates/fung-vm/src/vm/stdin.rs`(`Reader::wait` 삭제, 기다리는 쪽 등록·해제), `crates/fung-vm/src/vm.rs`(`stdin_next`·`StdinWait` 삭제), `crates/fung-vm/src/prelude/io.rs`(`read_line`·`read_lines`)
- Test: `crates/fung-vm/src/vm/park_tests.rs`

**Interfaces:**
- Produces:
  - `Stdin::poll(&self, me: &Arc<Waiter>) -> Option<Option<Vec<u8>>>`
    - 줄이 있으면 꺼내고 `me`를 기다리는 목록에서 뺀다.
    - 없으면 `me`를 목록에 올리고 읽기 스레드에 수요를 알린 뒤 `None`이다. 읽기 스레드는 지금처럼 누가 원할 때만 4 KiB씩 읽는다.
  - `Stdin::leave(&self, me: &Arc<Waiter>)`: 취소된 기다림이 목록에서 빠진다.
  - 읽기 스레드는 줄을 넣은 뒤 목록 맨 앞의 `Waiter::wake()`를 부른다. 이 깨우기는 다른 스레드에서 온다(지금 `Waiter::wake`는 스레드를 깨우므로 그대로 된다. 작업 5에서 대기열에 넣는 것으로 바뀐다).
  - `read_line(nil)`: `park_until`. 닫개는 `poll`이고, 상자의 `unwind`는 `leave`다. `Wake { until: None, timed: true }`: 바깥이 깨우므로 교착이 아니다. 지금 `outside`에 있는 태스크가 "살아 있고 막히지 않음"이던 것과 같은 판정이다.
  - `read_lines(nil)`: 줄마다 주차하는 상자. 줄이 들어 있으면 주차 없이 모은다.
  - 기한: 노드의 기한은 `wait_parked`가 본다(지금 `Reader::wait`의 `park_timeout`을 대신함).

- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `park_tests::reading_stdin_offloads`를 `reading_stdin_parks`로 바꾼다. 줄이 아직 없으면 `parks() >= 1`이고 `offloads()`는 그대로다.
  - `a_cancelled_stdin_wait_leaves_the_queue`: 두 태스크가 표준 입력을 기다린다(`stdin_waiting` 탐침이 2가 될 때까지). 하나를 취소하면 1이 되고, 남은 쪽이 줄을 받는다.
  - 기존 `crates/fung-cli/tests/stdin.rs`와 `repl` 시험은 그대로 통과해야 한다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- park_tests::reading_stdin` → FAIL
- [ ] **Step 3: 구현한다.**
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- park_tests stdin repl && cargo test -p fung-cli --test stdin --test repl_tty` → PASS
- [ ] **Step 5: 커밋한다.** `refactor(vm): standard input parks on its reader thread`

### Task 3: 차례 넘기기를 실행 루프 밖으로 나오는 신호로

**Files:**
- Modify: `crates/fung-vm/src/vm/block.rs`(`tick`·`turn`), `crates/fung-vm/src/vm/call.rs`(`Ran::Yielded`, `issue`, `call_plain`), `crates/fung-vm/src/vm.rs`(`step_loop`의 CALL·TAILCALL·뒤로 가는 JUMP, `drive`)
- Test: `crates/fung-vm/src/vm/park_tests.rs`

**Interfaces:**
- Produces:
  - `Ran::Yielded(Option<(Value, usize)>)`: 차례를 넘길 때가 됐다. 안의 값은 이어 가기가 부탁했지만 아직 하지 않은 호출이다.
  - `tick`은 취소 확인과 기한 확인만 하고, 예산이 다 되면 `self.turn_due = true`만 세운다(`yield_turn`을 부르지 않음).
  - 넘기는 자리 셋:
    1. `step_loop`의 CALL·TAILCALL·뒤로 가는 JUMP: `tick` 뒤에 `turn_due`이면 `ip`를 그 명령으로 되돌리고 `Ran::Yielded(None)`.
    2. `issue`의 반복: 호출 직전에 `turn_due`이면 `Ran::Yielded(Some((f, n)))`. 인자는 스택에 그대로 둔다.
    3. `call_plain`: `turn_due`이면 `None`을 돌려 제자리 호출을 하지 않는다. 그러면 순회가 `Next::Call`로 돌아가 2를 지난다.
  - `drive`가 `Yielded`를 받으면 지금 런타임에서는 `turn_due = false`, `world.yield_turn()`, 그다음 `Todo::Start(f, n)`이나 `Todo::Loop`로 이어 간다. 작업 5에서는 이 갈래가 "스케줄러로 돌아간다"가 된다.
  - 시험용 셈 `yields`(`drive`가 넘길 때마다).

- [ ] **Step 1: 실패하는 시험을 쓴다** (`park_tests.rs`)
  - `every_yield_point_lets_others_run`: 다음 셋에서 `yields() >= 2`이고 결과가 맞다.
    - `n = len(map(0..30000, x => x + 1))`(호출 없는 콜백: 2번 자리)
    - `len(map(0..30000, str))`(제자리 호출: 3번 자리)
    - 뒤로 가는 점프만 도는 반복이 있으면 그것(1번 자리)
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- park_tests::every_yield` → `yields` 탐침이 없어 FAIL
- [ ] **Step 3: 구현한다.**
- [ ] **Step 4: 통과를 확인한다.**
  - Run: `cargo test -p fung-vm -- park_tests task_tests deadline_tests cont_tests` → PASS
  - Run: `cargo build --release --locked -p fung-cli && scripts/bench.sh /tmp/fung-r1b-base target/release/fung` → 계산 위주 +5% 이하
  - 시작점 바이너리는 작업 1 전에 만들어 둔다.
- [ ] **Step 5: 커밋한다.** `refactor(vm): turn handoff leaves the run loop`

### Task 4: offload의 일을 다른 스레드에 맡기고 주차

**Files:**
- Modify: `crates/fung-vm/src/native/nat.rs`(`offload`·`offload_then`·`offload_watched`), 필요하면 `crates/fung-vm/src/vm/block.rs`(일을 기다리는 상자)
- Test: `crates/fung-vm/src/vm/park_tests.rs`

**Interfaces:**
- Produces:
  - offload가 일을 새 스레드(`std::thread::Builder`, 이름 `fung-offload`)에서 돌린다.
    - 결과는 `Arc<Mutex<Option<std::thread::Result<R>>>>` 칸에 넣고 태스크의 `Waiter::wake()`를 부른다.
    - 태스크는 `Wake { until: None, timed: true }`로 주차한다. 바깥이 깨우므로 교착이 아니다.
  - **기다리는 동안은 끊기지 않는다**(스펙 15절). 범위가 자식을 기다릴 때처럼 `t.cancel`을 새 루트 노드로 바꿔 끼운다.
    - `Watch`와 외부 명령의 `stopped`는 바꾸기 전의 노드를 이미 쥐고 있으므로 취소가 일을 끊는다.
    - 일이 끝나면 노드를 되돌린다. 그다음 지금 `outside`의 끝처럼 기한을 확인해 만료시키고, `offload_watched`면 `w.stopped`를 확인한 뒤 마무리를 돈다.
  - 일의 패닉은 칸에 담아 와서 실행 스레드에서 `resume_unwind`한다(지금 `outside`와 같음, E0801).
  - 스레드를 만들 수 없으면(드묾) 지금처럼 그 자리에서 `outside`로 돈다(되돌아가는 길). 이것은 R3의 풀이 대신한다.

- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `an_offload_runs_on_another_thread`: 시험용 내장 함수가 offload의 일 안에서 `std::thread::current().id()`를 문자열로 돌려준다. 그것이 태스크 쪽의 스레드 아이디와 다르다.
  - `an_offload_wait_is_not_cut_by_a_cancel`: 시험용 내장 함수 `slow_job()`의 일은 50 ms 잔다(`std::thread::sleep`). `timeout(10ms, () => slow_job())`은 `Err({kind: 'timeout})`이고 걸린 시간은 50 ms 이상이다. 일이 끝난 뒤에 되감긴다는 뜻이다. 지금 런타임에서도 통과하는 동작 고정이다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- park_tests::an_offload` → FAIL
- [ ] **Step 3: 구현한다.**
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- park_tests native:: process && cargo test -p fung-cli --test http --test tls --test signals` → PASS
- [ ] **Step 5: 커밋한다.** `refactor(vm): offload jobs run on a thread of their own while the task parks`

### Task 5: 런타임 교체

이 작업은 크다. 아래 순서로 커밋 없이 진행하고, 전체가 초록이 된 뒤 한 번 커밋한다(중간 상태는 빌드되지 않는다).

**Files:**
- Create: `crates/fung-vm/src/vm/sched.rs`, `crates/fung-vm/src/vm/sched_tests.rs`
- Modify:
  - `vm.rs`(`Task`/`Fiber`)
  - `vm/block.rs`(`Waiter`, `wait_parked` 삭제, `turn`)
  - `vm/world.rs`(`Gate`·`Token`의 넘기기 삭제, 받은 편지함)
  - `vm/task.rs`(`spawn`이 `Fiber`를 만듦, `Handoff` 삭제)
  - `vm/deadlock.rs`(판정을 스케줄러 상태로)
  - `vm/runtime.rs`(실행 시작점이 스케줄러를 돌림, `wait`·`hold`·`release` 정리)
  - `vm/stop.rs`(`StopHandle`이 받은 편지함으로)
  - `vm/m6.rs`, `vm/module.rs`(시작점)
  - `signals.rs`, `stack.rs`(태스크 스택 삭제)
  - `conc.rs`(`try_spawn` 삭제), `extras/http.fung`(`try_spawn` → `spawn`)
  - `fung-cli/src/{run,test}.rs`·`repl/mod.rs`(`hold`·`release`·`wait`)
  - `crates/fung-cli/tests/perf.rs`(`twenty_thousand_waiting_tasks`)
  - `spec/builtins.txt`(다시 만듦), 지워지는 장치의 시험들(`world.rs`의 `Gate` 시험, `task_tests`의 스레드 스택 시험, `deadlock`의 `Census` 시험)

**Interfaces:**
- Produces:
  - `vm.rs`
    - `pub(crate) struct Fiber`: 지금 `Task`의 태스크별 필드(`stack`, `frames`, `conts`, `stop`, `floor`, `native`, `cancel`, `budget`, `pure`, `waiter`, `turn_due`, 시험 셈들)와 이어 갈 곳(`resume: Todo`)
    - `Task<'a>`: `Fiber`의 필드를 그대로 가진 달리는 모습. 그리고 `world: &'a mut World`(스케줄러가 쥔 `Token`에서 빌림), `shared: &'a Arc<Shared>`
    - `Task::from_fiber(f: Fiber, world, shared) -> Task<'a>`, `Task::into_fiber(self) -> Fiber`: 필드를 옮길 뿐 할당이 없다.
    - `drive`는 `Done`이면 결과를, `Parked(Wake)`·`Yielded(_)`이면 `Turn::Parked(Wake)`·`Turn::Yielded`를 스케줄러에 돌려준다. 이어 갈 곳은 `Fiber::resume`에 남긴다(`Todo::Resume`, `Todo::Start(f, n)`, `Todo::Loop`).
  - `vm/sched.rs`
    - `pub(crate) struct Sched`
      - `fibers`: 칸의 목록과 빈 칸 목록
      - `ready: Arc<Ready>`
      - `timers`: 깨울 시각, 태스크, 주차 번호의 최소 힙
      - 주차한 수와 그중 바깥·타이머가 깨울 수, 살아 있는 수
    - `pub(crate) struct Ready { state: Mutex<ReadyState>, wake: Condvar }`
      - `ReadyState { ids: VecDeque<FiberId>, inbox: Vec<Event> }`
      - 다른 스레드에서도 넣을 수 있다.
    - `pub(crate) enum Event { Stop(i32), Interrupt }`
    - `Sched::run_root(&mut self, world: &mut Token, shared, root: Fiber) -> Result<Value, Stop>`
      - 뿌리 태스크와 그것이 남긴 모든 태스크가 끝날 때까지 돈다.
      - 뿌리가 에러로 끝나면 지금 `run_with`처럼 루트를 취소하고 나머지가 되감기를 마치게 둔다.
    - 한 바퀴의 순서
      1. 받은 편지함을 처리한다(`Stop`은 지금 `StopHandle::deliver`의 몸통, `Interrupt`는 `interrupt`의 몸통).
      2. 대기열에서 하나를 꺼내 돌린다. 주차하면 칸에 치워 두고(마감이 있으면 타이머에), 넘기면 대기열 끝에 넣는다.
      3. 대기열이 비면 교착을 판정한다(아래). 아니면 가장 이른 타이머까지, 또는 편지함·대기열에 무엇이 올 때까지 `Condvar`로 잔다.
  - `Waiter { fiber: FiberId, woken: AtomicBool, ready: Arc<Ready> }`
    - `wake()`: `woken`을 처음 세운 쪽만 대기열에 넣고 `Condvar`를 깨운다. 스레드를 깨우던 것을 대신한다.
    - 주차 번호로 오래된 타이머와 거짓 깨움을 거른다.
  - **교착 판정**: 대기열이 비었고, 주차한 수가 살아 있는 수와 같고, 그중 바깥·타이머가 깨울 것이 없을 때다.
    - 차단 노드(`shields`)와 `exit`·`deadlocked`의 규칙은 지금 `judge` 그대로다.
    - `Census`와 그 원자 변수는 지운다.
  - **신호**
    - `StopHandle(Arc<Shared>)`의 `deliver`·`interrupt`는 실행권을 잡지 않고 받은 편지함에 넣고 깨운다.
    - 두 번째 신호의 판정에 쓰는 "정지가 요청됨"은 `Shared`의 `AtomicBool`로 옮긴다. `request_stop()`도 세운다.
    - 실행 중이 아닐 때 온 편지는 다음 시작점(`run_root`)이나 상태를 읽는 공개 함수(`exit_code`, `recover_interrupt` 등)가 먼저 처리한다. 그래서 `fung test`에서 파일을 컴파일하는 동안 온 신호가 사라지지 않는다(R2 계획에서 넘어온 잔가지).
    - `signals::stop_requests`는 따로 스레드를 띄우지 않고 바로 `deliver`한다.
  - **시작점**: `Runtime::run`, `run_repl`, `call_test`, `boot`(prelude 모듈 본문), `import_module`이 모두 뿌리 태스크를 만들어 `run_root`로 돈다. `Task::driven`·`spawned`·`unlisted`는 지운다.
  - **지우는 것**
    - `Gate`, `Token::sleep`·`yield_turn`·`release`·`reacquire`
    - `Census`, `Handoff`
    - `stack::TASK_STACKS`·`spawn_sized`의 태스크 쪽(진입 스레드의 `ENTRY_STACKS`는 남김)
    - `Shared::task_stacks`, `World::running`·`idle`·`held`·`hold_taken`
    - `Runtime::hold`·`release`(CLI의 호출 포함)
    - `_native.conc.try_spawn`(`extras/http.fung`은 `spawn`으로)
    - `Runtime::wait`는 남은 태스크가 없음을 단언하는 함수로 남기고, CLI의 호출도 남긴다.

- [ ] **Step 1: 실패하는 시험을 쓴다** (`sched_tests.rs`. 지금 런타임에서는 컴파일되지 않거나 실패한다)
  - `tasks_run_on_one_thread`: 태스크 셋이 각각 시험용 내장 함수로 자기 스레드 아이디를 돌려준다. 셋 다 진입과 같다.
  - `spawn_does_not_make_a_thread`: 시험용 셈 `Shared::threads_started`(런타임이 스레드를 만드는 자리마다 하나: offload의 일, 표준 입력 읽기 스레드)가 태스크 1,000개를 띄워 모두 `sleep(1ms)`한 뒤에도 그대로다.
  - `twenty_thousand_waiting_tasks`(릴리스 전용, `crates/fung-cli/tests/perf.rs`): 태스크 2만 개가 각각 버퍼 없는 채널에서 기다리다가, 진입이 차례로 보내면 모두 끝난다. 3초 안에 끝나고 출력이 맞다. 지금 런타임은 macOS의 스레드 한도(8,192)를 넘어 E0801로 실패한다.
  - `a_wake_from_another_thread_before_the_park_is_kept`: 시험용 단계 내장 함수가 깨움 표시를 지우고, `Arc<AtomicBool>` 표시를 닫개로 등록한다. 그다음 다른 스레드를 띄워 표시를 세우고 `Waiter::wake()`를 부르게 하고, 그 스레드를 `join`한 **뒤에** `Flow::Park`를 돌려준다. 깨움이 주차 결정보다 확실히 먼저 왔는데도 태스크는 멈추지 않고 끝난다.
  - `deadlock_is_judged_by_the_scheduler` 묶음
    - `recv(channel())`만 남으면 E0702
    - `sleep` 중인 태스크가 있으면 교착 아님
    - offload를 기다리는 태스크(시험용 느린 일)가 있으면 교착 아님
    - 정지 채널만 기다리면 교착 아님
    - `release` 안의 교착은 지금처럼 판정
  - `a_signal_during_a_busy_entry_stops_it`(`fung-cli/tests/signals.rs`): 끝없이 계산하는 진입 파일에 SIGINT → 130, 1초 안에 끝남. 지금은 신호가 실행권을 기다리는 스레드로 전해지므로 통과하지만, 작업 5의 중간에 깨지기 쉬운 자리라 먼저 고정한다.
  - `a_stop_while_idle_is_kept_for_the_next_run`(`sched_tests.rs`): 런타임이 아무것도 돌리지 않을 때 `StopHandle::deliver(SIGINT)`를 부른다. 다음 `Runtime::run`은 진입 파일의 첫 호출에서 취소되고 `exit_code()`는 130이다. `fung test`에서 파일을 컴파일하는 동안 온 신호가 사라지던 잔가지가 이것으로 닫힌다. 순서를 시간이 아니라 호출 순서로 정하므로 흔들리지 않는다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- sched_tests` → FAIL(컴파일되지 않거나 단언 실패). Run: `cargo test --release -p fung-cli --test perf twenty_thousand` → FAIL(E0801)
- [ ] **Step 3: 구현한다.** 순서:
  1. `Fiber`와 `Task`의 나눔(필드 옮기기만)
  2. `Ready`·`Waiter`
  3. `Sched`와 `run_root`
  4. 시작점을 `run_root`로
  5. `spawn`이 `Fiber`를 만듦
  6. `drive`의 주차·넘김이 스케줄러로
  7. 교착 판정
  8. 받은 편지함과 신호
  9. offload의 깨움(작업 4의 칸을 그대로 씀)
  10. 지우는 것
  11. CLI
  12. 시험 정리

  `cargo build`가 되는 지점마다 `cargo test -p fung-vm -- sched_tests task_tests`로 방향을 확인한다.
- [ ] **Step 4: 통과를 확인한다.**
  - Run: `cargo test -p fung-vm && cargo test -p fung-cli` → PASS
  - Run: `cargo test --release -p fung-cli --test run_expected concurrent_runs_are_stable` → PASS
  - 순서에 기대던 시험이 깨지면 상태로 순서를 정하게 고치고 판단으로 기록한다.
- [ ] **Step 5: 커밋한다.** `feat(vm)!: an M:1 scheduler runs every task on one thread`

### Task 6: `YIELD_EVERY`, 10.4의 측정, 문서

**Files:**
- Modify:
  - `crates/fung-vm/src/vm/block.rs`(`YIELD_EVERY`와 그 주석의 측정)
  - `bench/`(새 `wait.fung`: 기다리는 태스크 2만 개. `run_expected`·`dump_expected`의 사례 수와 기대 출력에 그 파일의 항목이 더해진다. 다른 항목이 바뀌면 멈춘다)
  - `docs/notes/R7-scheduler.md`(R1b 절)
  - `docs/internals.html`(동시성 절: 태스크 = 스레드 → 스케줄러)
  - `docs/design/open-questions.md`(R7 스케줄러 항목, 스레드 한도 한계 닫기)
  - `CHANGELOG.md` `[Unreleased]`(태스크가 스레드를 쓰지 않음, 스레드 한도와 E0801 `cannot start a task` 없어짐)

**Interfaces:**
- Produces: `YIELD_EVERY`의 값과 그 근거. 스펙 15절의 기준은 "계산 위주가 1% 넘게 느려지지 않는 가장 작은 값"이고, 후보는 2,000부터 재 본다.

- [ ] **Step 1: 시작점을 잰다.** 작업 5까지의 릴리스 빌드를 `/tmp/fung-r1b-swap`으로 복사해 두고, `YIELD_EVERY` 10,000(지금)에서 `scripts/bench.sh /tmp/fung-r1b-base /tmp/fung-r1b-swap`을 잰다.
- [ ] **Step 2: 후보를 잰다.**
  - `YIELD_EVERY` 후보 10,000·5,000·2,000·1,000마다 `scripts/bench.sh`와 지연 확인(계산하는 태스크 옆에서 `sleep(1ms)` 200번의 늦음)
  - 10.4의 태스크 생성(`spawn.fung`의 태스크당 시간), 채널(`chan.fung`의 값당 시간), 2만 개의 최대 RSS(`/usr/bin/time -l`)
- [ ] **Step 3: 값을 정하고 문서를 쓴다.**
  - 노트 R1b 절에 들어갈 것:
    - `Fiber`와 `Task`로 나눈 이유
    - 다른 스레드의 깨움이 대기열을 지나는 이유와 잃어버린 깨움이 없는 근거
    - 교착 판정이 원자 변수 없이 정확한 이유
    - 받은 편지함과 신호
    - 측정 표
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test --release -p fung-cli --test perf && cargo test -p fung-cli --test manual --test changelog --test repo_rust_only` → PASS
- [ ] **Step 5: 커밋한다.** `perf(vm): measure the turn length; R1b note, manual and changelog`

### 끝에 한 번 (사용자 방침)

- [ ] 전체 검사: `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`(debug·`--release`), `cargo test --locked --no-fail-fast`(debug·`--release`), `cargo deny check`·`cargo audit`(설치되어 있으면)
- [ ] 벤치마크: 계산 위주 5% 안. 10.4의 목표와 비교한 표.
- [ ] 가장 능력 있는 모델로 브랜치 전체 검토. 이 계획, 스펙 10·15절, Review Focus, 노트 R1a 6절을 준다.
- [ ] 검토 결과를 한 번에 고치고(Critical·Important만, 고칠 때마다 실패하는 시험 먼저), 전체 검사를 다시 돌려 커밋한다.
- [ ] 사용자에게 한국어로 보고: 무엇을 만들었고 어떤 원리인지, 측정과 10.4 목표, 내가 정한 것, 미룬 Minor. 다음은 R3(스레드 풀) 계획.
