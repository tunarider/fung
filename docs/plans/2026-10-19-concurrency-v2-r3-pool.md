# 동시성 v2 R3 (스레드 풀) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** offload의 일을 일마다 새 스레드가 아니라 스레드 풀(최대 64)에서 돌린다. 경로만 읽는 일은 취소·기한에 곧바로 되감는다(L2의 한계를 닫음). 실행 스레드에서 일을 바로 돌리는 `outside`를 지운다.

**Architecture:**
- **작업 1: 풀.** `vm/pool.rs`. 필요할 때 스레드를 만들고 최대 64개, 10초 놀면 끝난다. 꽉 차면 FIFO 줄. 일마다 표(`Ticket`)가 있고, 버려진 표의 일은 꺼낼 때 건너뛴다.
- **작업 2: offload를 풀 위로.** 일의 종류가 셋이 된다.
  - 기다리는 일(`nat::offload`): 풀에서 돈다. 시작했으면 끝까지 기다린다(지금과 같음). 줄에 있을 때 온 취소는 일을 버린다.
  - 버리는 일(`nat::offload_abandoning`): 풀에서 돈다. 취소·기한이 오면 표를 버리고 곧바로 되감는다.
  - 자기 스레드의 일(`nat::offload_thread`·`offload_watched`): 소켓과 외부 명령. R4까지 지금처럼 일마다 스레드 하나(스펙 15절).
  - 스레드를 만들 수 없고 도는 풀 스레드도 없으면 E0801 `cannot start a job: …`. `outside`를 지운다.
- **작업 3: 버리기.** 경로만 읽는 `fs` 함수와 정규식 컴파일을 버리는 일로 옮긴다(스펙 15절의 표).
- **작업 4: R1b 잔가지.** `run_root` 재진입을 릴리스에서도 막고, 옛 런타임을 설명하는 주석을 고친다.
- **작업 5: 측정과 문서.** `bench/fs.fung`, 노트 R3 절, 매뉴얼, CHANGELOG, open-questions.

**Tech Stack:** Rust 표준 라이브러리. `fung-vm`(새 `vm/pool.rs`·`vm/pool_tests.rs`, `vm/offload.rs`, `vm/world.rs`, `vm/block.rs`, `vm/sched.rs`, `vm/runtime.rs`, `native/{nat,fs,regex,process,net}.rs`, `prelude/io.rs`), `bench/`.

**Spec:** `docs/superpowers/specs/2026-10-18-concurrency-v2-r7-design.md` (10.1 스레드 풀, 15절의 "스레드 풀"·"버리는 일"·"풀에 넣지 않는 것"·"스레드를 만들 수 없을 때"·"R3에 함께 넣는 것"). 앞 단계 노트 `docs/notes/R7-scheduler.md`의 R1b 절, open-questions의 "R1b 검토에서 미룬 것".

## Global Constraints

- 사용자 방침(메모리 `plan-execution-style`)을 따른다.
  - 작업마다 TDD로 하고 그 작업의 시험만 돌린다. 작업 단위로 커밋한다.
  - 끝에 한 번만 한다: 전체 검사(fmt, clippy, `cargo test --locked --no-fail-fast`를 debug·release 둘 다), 가장 능력 있는 모델의 브랜치 전체 검토, 수정.
- 사용자와의 대화·보고는 한국어. 코드·주석·커밋은 영어. 커밋 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **언어 의미는 하나만 바뀐다.** 경로만 읽는 일과 정규식 컴파일이 기한·취소에 곧바로 끝난다(지금은 일이 끝날 때까지 기다림). 다른 기대 출력은 bless하지 않는다. 필요해 보이면 멈추고 묻는다.
- 풀의 수치는 고정이다: 최대 `64`, 놀면 `10s` 뒤 끝남, 환경 변수 설정 없음. 시험만 `Shared`의 시험용 설정으로 바꾼다.
- 풀 스레드 이름은 `fung-pool`, 자기 스레드의 일은 지금처럼 `fung-offload`.
- 일과 그 결과는 `Send + 'static`인 평범한 데이터다. fung 값(`Rc`)은 풀로 가지 않는다. 새 `unsafe`가 없다.
- 버려진 일의 결과는 그 일의 스레드에서 버린다. 버려진 일이 나중에 부르는 `Waiter::wake`는 해가 없어야 한다(태스크는 깨어나 조건을 다시 볼 뿐).
- 동시성 시험은 상태로 순서를 정한다(`sleep` 대신 탐침과 걸쇠). 릴리스의 동시성 50회 반복이 통과해야 한다.
- 계산 위주 벤치마크는 이 브랜치 시작점보다 5% 넘게 느려지지 않는다.

## Review Focus

- **늦은 깨움.** 버려진 일이 끝나며 부르는 `wake`가, 그사이 다른 것을 기다리며 주차한 같은 태스크를 깨운다. 태스크는 조건을 다시 보고 다시 주차해야 하고 값이 섞이면 안 된다. → 작업 3의 `a_late_wake_from_an_abandoned_job_is_harmless`
- **막힌 일로 꽉 찬 풀.** 끝나지 않는 일(FIFO 읽기)이 풀을 다 차지하면 새 일은 줄에서 기다린다. 그 기다림도 기한에 끊겨야 한다. → 작업 3의 `a_queued_job_is_dropped_at_the_deadline`
- **풀 안의 패닉.** 일의 패닉은 실행 스레드에서 E0801이 되고, 풀 스레드는 살아서 다음 일을 받는다. → 작업 1의 `a_panicking_job_does_not_end_its_thread`, 작업 2의 `a_panic_in_a_pool_job_is_E0801`
- **교착 판정.** 줄에서 기다리는 일이 있는 태스크는 교착이 아니다. → 작업 2의 `a_task_waiting_in_the_pool_queue_is_not_deadlocked`
- **끝날 때.** 막힌 풀 스레드가 남아 있어도 프로그램은 곧바로 끝나고 종료 코드가 맞다. → 작업 3의 `crates/fung-cli/tests/cleanup.rs::exit_does_not_wait_for_an_abandoned_job`

---

### Task 1: 풀

**Files:**
- Create: `crates/fung-vm/src/vm/pool.rs`, `crates/fung-vm/src/vm/pool_tests.rs`
- Modify: `crates/fung-vm/src/vm.rs`(모듈 선언), `crates/fung-vm/src/vm/world.rs`(`Shared::pool`)

**Interfaces:**
- Produces:
  - `pub(crate) const POOL_MAX: usize = 64;`, `pub(crate) const POOL_IDLE: Duration = Duration::from_secs(10);`
  - `pub(crate) struct Pool`: `Mutex<PoolState>`와 `Condvar`. `PoolState { queue: VecDeque<(Arc<Ticket>, Work)>, threads: usize, idle: usize, max: usize, idle_for: Duration }`. `type Work = Box<dyn FnOnce() + Send>`.
  - `Pool::new(max: usize, idle_for: Duration) -> Pool`; `Shared::new`는 `Pool::new(POOL_MAX, POOL_IDLE)`.
  - `pub(crate) struct Ticket(AtomicU8)`: 상태 `QUEUED`·`RUNNING`·`ABANDONED`.
    - `Ticket::abandon(&self) -> bool`: 줄에 있었으면 `ABANDONED`로 바꾸고 `true`(일은 돌지 않는다). 이미 도는 중이면 `false`.
    - `Ticket::abandon_running(&self)`: 도는 중이라도 `ABANDONED`로(결과를 버림). 일 쪽이 끝에 이것을 보고 결과를 놓는다.
    - `Ticket::is_abandoned(&self) -> bool`
  - `Pool::submit(&self, work: Work) -> Result<Arc<Ticket>, std::io::Error>`
    - 노는 스레드가 있으면 그것이 받는다. 없고 `threads < max`이면 새 스레드(`fung-pool`)를 만든다. 아니면 줄 끝에 넣는다.
    - 스레드를 만들지 못하면: `threads > 0`이면 줄에 넣고 `Ok`, `threads == 0`이면 그 `io::Error`.
  - 풀 스레드의 반복: 줄 앞에서 꺼내 `QUEUED → RUNNING`을 바꾸지 못하면(버려짐) 건너뛴다. 일은 `catch_unwind` 안에서 돈다(패닉은 일 쪽 클로저가 결과 칸에 담는다. 풀은 스레드가 죽지 않게만 막는다). 줄이 비면 `idle_for`까지 기다리고, 그래도 없으면 끝난다.
  - 시험용: `Pool::threads()`, `Pool::queued()`, `Pool::fail_spawns(bool)`(새 스레드 만들기를 일부러 실패시킴), `Shared::set_pool(Pool)`(시험이 한도를 바꿈).

- [ ] **Step 1: 실패하는 시험을 쓴다** (`pool_tests.rs`. 걸쇠는 `Arc<(Mutex<bool>, Condvar)>`, 상태는 `threads()`·`queued()`를 기한 5초로 기다리는 도우미)
  - `at_most_max_threads_run_and_the_rest_wait_in_order`: `Pool::new(2, 10s)`. 걸쇠에서 막히는 일 다섯을 넣으면 `threads() == 2`, `queued() == 3`. 걸쇠를 풀면 다섯 모두 끝나고, 넣은 순서대로 시작했다(시작 순서를 `Vec`에 적음).
  - `an_idle_thread_takes_the_next_job`: 일 하나가 끝난 뒤 다음 일을 넣으면 `threads()`는 1 그대로다.
  - `an_idle_thread_ends`: `Pool::new(4, 50ms)`. 일 하나가 끝나고 `threads()`가 0이 된다.
  - `an_abandoned_ticket_in_the_queue_is_skipped`: `Pool::new(1, 10s)`. 막히는 일 뒤에 넣은 일의 표를 `abandon()`하면 `true`이고, 걸쇠를 풀어도 그 일은 돌지 않는다. 도는 일의 표에 `abandon()`은 `false`.
  - `a_panicking_job_does_not_end_its_thread`: `Pool::new(1, 10s)`. 패닉하는 일 다음의 일도 돌고 `threads()`는 1이다.
  - `no_thread_and_none_can_start_is_an_error`: `fail_spawns(true)`인 빈 풀에 `submit`은 `Err`. 스레드가 하나 도는 풀에서는 `Ok`이고 일은 줄에서 그 스레드가 받는다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- pool_tests` → FAIL(모듈이 없음)
- [ ] **Step 3: 구현한다.** 잃어버리는 일이 없게, 노는 수(`idle`)와 줄은 같은 잠금 아래에서 바꾼다. `notify_one`은 잠금을 쥐고 넣은 뒤에.
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- pool_tests` → PASS
- [ ] **Step 5: 커밋한다.** `feat(vm): a thread pool for offload jobs`

### Task 2: offload를 풀 위로

**Files:**
- Modify: `crates/fung-vm/src/vm/offload.rs`, `crates/fung-vm/src/native/nat.rs`, `crates/fung-vm/src/native/process.rs`(`offload_thread`로), `crates/fung-vm/src/vm/block.rs`(`outside` 삭제), `crates/fung-vm/src/vm.rs`(내보내기)
- Test: `crates/fung-vm/src/vm/park_tests.rs`, `crates/fung-vm/src/vm/sched_tests.rs`

**Interfaces:**
- Consumes: 작업 1의 `Pool::submit`, `Ticket`.
- Produces:
  - `vm/offload.rs`의 `enum Kind { Wait, Abandon, Thread }`와 `offload_kind<R>(t, kind, job, finish) -> Result<Flow, Stop>`. `offload_then`(지금 이름)은 `Kind::Wait`.
  - `nat::offload`(기다리는 일, 풀), `nat::offload_abandoning`(버리는 일, 풀; 작업 3이 씀), `nat::offload_thread`(자기 스레드), `nat::offload_watched`(자기 스레드, 지금과 같음).
  - `Job` 상자의 모양이 바뀐다.
    - 주차할 때 노드를 바꿔 끼우지 않는다. 그래서 취소·기한이 오면 `after_park`가 상자의 `unwind`로 보낸다.
    - `Job::unwind(stop)`
      - `Kind::Abandon`: `ticket.abandon_running()`, 곧바로 `Err(stop)`.
      - `Kind::Wait`: `ticket.abandon()`이 `true`(줄에 있었음)면 `Err(stop)`. `false`(도는 중)면 R1b처럼 노드를 새 루트로 바꿔 끼우고(`own`에 쥠) `Ok(Next::Park(JOB_WAKE))`로 계속 기다린다. 일이 끝나면 노드를 되돌리고 기한을 만료시킨 뒤 마무리를 돈다(지금 `ended`와 같음).
      - `Kind::Thread`: 지금처럼 처음부터 노드를 바꿔 끼운다(취소 훅이 일을 끊으므로). 동작이 R1b와 같아야 한다.
    - 일의 클로저: `catch_unwind`로 돌려 칸에 넣고, 표가 버려졌으면 칸에 넣지 않고 결과를 그 자리에서 놓는다. 그다음 `wake()`.
  - 스레드를 만들 수 없을 때: `Kind::Wait`·`Abandon`은 `submit`의 `Err(e)`를 `E0801`(`cannot start a job: {e}`)로, `Kind::Thread`는 스레드 생성 실패를 같은 E0801로. `outside`와 그 되돌아가는 길을 지운다.
  - 시험용 셈은 `t.offloads`를 그대로 쓴다.

- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `park_tests::an_offload_runs_on_another_thread`를 고친다: 일 안의 스레드 이름이 `fung-pool`이다.
  - `park_tests::offloads_reuse_pool_threads`: 한 태스크가 시험용 offload 내장 함수를 200번 차례로 부른 뒤 `shared.pool.threads()`가 1이다(지금은 풀이 없어 컴파일되지 않음).
  - `park_tests::socket_and_process_waits_keep_a_thread_of_their_own`: 시험용 `offload_thread` 내장 함수의 일 안 스레드 이름이 `fung-offload`이다.
  - `park_tests::no_thread_gives_E0801`: `fail_spawns(true)`인 빈 풀에서 offload 내장 함수가 `error[E0801]`과 `cannot start a job`을 낸다.
  - `park_tests::a_panic_in_a_pool_job_is_E0801`: 패닉하는 일은 E0801이고, 같은 프로그램의 다음 offload도 돈다.
  - `sched_tests::a_task_waiting_in_the_pool_queue_is_not_deadlocked`: 풀 한도 1, 첫 일은 걸쇠에서 막힘(시험 스레드가 `queued() == 1`을 본 뒤 풂). 둘째 태스크는 줄에서 기다린다. 결과는 E0702 없이 둘 다 끝남.
  - `sched_tests::a_queued_wait_job_is_dropped_on_cancel`: 풀 한도 1, 첫 일이 막힘. 기다리는 일(셈을 하나 올림)을 `timeout(20ms, …)` 안에서 부르면 `Err({kind: 'timeout})`가 걸쇠를 풀기 전에 나오고, 걸쇠를 푼 뒤에도 셈은 0이다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- park_tests sched_tests` → FAIL
- [ ] **Step 3: 구현한다.** `process.rs`의 `run_command`는 `nat::offload_thread`로, `nat::offload_watched`는 `Kind::Thread`로.
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- park_tests sched_tests native:: process deadline_tests && cargo test -p fung-cli --test http --test tls --test signals` → PASS
- [ ] **Step 5: 커밋한다.** `refactor(vm): offload jobs run on the pool; sockets and commands keep a thread`

### Task 3: 경로만 읽는 일은 버린다

**Files:**
- Modify: `crates/fung-vm/src/native/fs.rs`(`read`·`read_lines`(경로)·`exists`·`stat`·`glob`·`walk`), `crates/fung-vm/src/native/regex.rs`(컴파일)
- Test: `crates/fung-vm/src/native/fs_tests.rs`, `crates/fung-vm/src/vm/park_tests.rs`, `crates/fung-cli/tests/cleanup.rs`

**Interfaces:**
- Consumes: 작업 2의 `nat::offload_abandoning`.
- Produces: 스펙 15절 "버리는 일"의 분류. 핸들 읽기(`read_line(f)`·`read_lines(f)`, `prelude/io.rs`)와 바꾸는 일은 `nat::offload` 그대로다. 각 함수의 `///` 주석에 어느 쪽인지 한 줄.

- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `fs_tests::a_deadline_ends_a_read_of_a_fifo`: 임시 디렉터리에 `libc::mkfifo`로 FIFO를 만든다. `timeout(50ms, () => fs.read(fifo))`는 `Err({kind: 'timeout})`이고 1초 안에 끝난다. 시험 끝에 FIFO를 쓰기로 열었다 닫아 막힌 풀 스레드를 풀어 준다. 지금은 열기에서 영영 막혀 실패한다.
  - `fs_tests::a_cancel_ends_a_read_of_a_fifo`: 범위 안에서 FIFO를 읽는 태스크를 띄우고, 탐침으로 그 태스크가 주차한 것을 본 뒤 `cancel(s)`. 범위가 곧바로 끝난다.
  - `fs_tests::a_running_write_is_waited_for`: 바꾸는 일은 지금과 같다. 시험용 기다리는 일(걸쇠에서 막히고 끝에 셈을 올림)을 `timeout(10ms, …)` 안에서 부르고, 걸쇠는 다른 스레드가 `pool.threads()`가 1이고 `queued()`가 0인 것을 본 뒤 100 ms 뒤에 푼다. 결과는 R1b와 같고(`nil`), 셈은 1, 걸린 시간 100 ms 이상.
  - `park_tests::a_late_wake_from_an_abandoned_job_is_harmless`: 버리는 시험용 일(걸쇠에서 막힘)을 `timeout(10ms, …)`으로 버린 뒤, 같은 태스크가 채널 `recv`에서 주차한다. 시험 스레드가 걸쇠를 풀어 늦은 깨움이 온 뒤(`wakeups` 탐침), 다른 태스크가 보낸 값을 받는다. 출력이 정확히 그 값이다.
  - `park_tests::a_queued_job_is_dropped_at_the_deadline`: 풀 한도 1을 막힌 일이 차지한 채로, 버리는 일을 `timeout(20ms, …)` 안에서 부르면 걸쇠를 풀기 전에 `Err({kind: 'timeout})`.
  - `cleanup::exit_does_not_wait_for_an_abandoned_job`: FIFO를 만든 뒤 `fung -e`로 `print(timeout(50ms, () => fs.read(fifo)))`를 돌린다. 2초 안에 끝나고, 출력은 `Err({kind: 'timeout})`, 종료 코드 0. 자식은 `common::Reaped`로 쥔다. FIFO는 시험 끝에 열어 풀어 준다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- fs_tests park_tests::a_late park_tests::a_queued` → FIFO 시험이 FAIL(시간 초과 단언)
- [ ] **Step 3: 구현한다.**
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- fs regex park_tests && cargo test -p fung-cli --test run_expected --test golden --test cleanup` → PASS(기대 출력이 바뀌면 멈추고 묻는다)
- [ ] **Step 5: 커밋한다.** `feat(fs): reads of a path and regex compiles end at a deadline or a cancel`

### Task 4: R1b 잔가지

**Files:**
- Modify: `crates/fung-vm/src/vm/sched.rs`(`run_root`), `crates/fung-vm/src/vm/world.rs`(`World::in_run` 또는 같은 표시, SAFETY 주석), `crates/fung-vm/src/vm/block.rs`(`tick` 주석), `crates/fung-vm/src/vm.rs`(`Task` 주석), `crates/fung-vm/src/vm/call.rs`(`call_root`의 겹친 주석), `crates/fung-vm/src/vm/module.rs`(디버그 단언을 새 표시로)
- Test: `crates/fung-vm/src/vm/sched_tests.rs`

**Interfaces:**
- Produces: `run_root`가 실행 중에 다시 불리면 릴리스에서도 패닉한다(`"run_root during a run"`). 패닉 울타리가 E0801로 바꾼다. 표시는 `run_root`가 들어갈 때 세우고 나갈 때(패닉에도) 내린다.

- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `sched_tests::a_nested_run_root_is_refused`: 시험용 내장 함수가 `t.run_root(Todo::Start(..))`를 부른다. 결과는 `error[E0801]`이고 `run_root during a run`을 담는다. 그다음 같은 런타임으로 다른 진입을 돌려도 된다(표시가 내려갔음). 지금은 디버그 빌드에서 다른 단언이나 엉뚱한 결과로 실패한다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- sched_tests::a_nested` → FAIL
- [ ] **Step 3: 구현하고 주석을 고친다.** 고칠 주석: `world.rs`의 SAFETY(스레드 위의 태스크), `block.rs`의 `tick`("토큰을 기다리는 태스크"), `vm.rs`의 `Task`(`block`·`tick`의 실행권), `call.rs`의 `call_root` 두 문단. `outside`는 작업 2에서 지웠다.
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- sched_tests module_tests` → PASS
- [ ] **Step 5: 커밋한다.** `fix(vm): a nested run is refused in release builds; comments on the old runtime`

### Task 5: 측정과 문서

**Files:**
- Create: `bench/fs.fung`(`fs.exists`와 `fs.stat` 5만 번)
- Modify:
  - `bench/README.md`(fs 행), `crates/fung-cli/tests/run_expected.rs`·`dump_expected.rs`의 사례 수와 bless한 새 파일 항목(다른 항목이 바뀌면 멈춤)
  - `docs/notes/R7-scheduler.md`(R3 절)
  - `docs/internals.html`(동시성 절: offload의 세 종류, 풀)
  - `docs/design/open-questions.md`(L2 알려진 한계 닫기, "R1b 검토에서 미룬 것"의 ①②③④ 닫기, R7 스케줄러 진행)
  - `CHANGELOG.md` `[Unreleased]`(파일 경로 읽기와 정규식 컴파일이 기한·취소에 곧바로 끝남, 파일 시스템 호출이 빨라짐)

- [ ] **Step 1: 잰다.** 작업 1 전의 릴리스 빌드를 `/tmp/fung-r3-base`로 복사해 둔다. `scripts/bench.sh /tmp/fung-r3-base target/release/fung`. 계산 위주 5% 안, `fs.fung`은 R1b의 일마다 스레드(호출당 약 22 µs)보다 빠르다.
- [ ] **Step 2: 문서를 쓴다.** 노트 R3 절에 들어갈 것:
  - 풀이 필요한 이유(스레드 생성 비용, 한도)
  - 왜 소켓과 외부 명령은 풀에 넣지 않았나(꽉 찬 풀 뒤의 `accept`)
  - 운영체제 스레드는 멈출 수 없으므로 "버리기"는 태스크만 놓아주는 것, 그래서 읽기만 버린다(늦은 쓰기의 경쟁)
  - 표의 세 상태와 줄에 있는 일은 언제나 버릴 수 있는 이유
  - 늦은 깨움이 해가 없는 이유
  - 측정 표
- [ ] **Step 3: 통과를 확인한다.** Run: `cargo test -p fung-cli --test run_expected --test dump_expected --test manual --test changelog --test repo_rust_only` → PASS
- [ ] **Step 4: 커밋한다.** `perf(vm): measure the pool; R3 note, manual and changelog`

### 끝에 한 번 (사용자 방침)

- [ ] 전체 검사: `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`(debug·`--release`), `cargo test --locked --no-fail-fast`(debug·`--release`), `cargo deny check`·`cargo audit`(설치되어 있으면)
- [ ] 가장 능력 있는 모델로 브랜치 전체 검토. 이 계획, 스펙 10.1·15절, Review Focus, 노트 R1b 절을 준다.
- [ ] 검토 결과를 한 번에 고치고(Critical·Important만, 고칠 때마다 실패하는 시험 먼저), 전체 검사를 다시 돌려 커밋한다.
- [ ] 사용자에게 한국어로 보고: 무엇을 만들었고 어떤 원리인지, 측정, 내가 정한 것, 미룬 Minor. 다음은 R4(이벤트 루프, rustls 0.23.46은 2026-10-28 이후) 계획.
