# 동시성 v2 R5 (옛 런타임 삭제) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 태스크마다 스레드를 쓰던 옛 런타임의 남은 장치를 지운다. 실행권 잠금(`Shared`의 `Mutex<World>`, `Token`, `token.rs`)과 마지막 `unsafe impl Send`가 대상이다. fung 값이 한 스레드에만 산다는 것을 컴파일러가 지키게 한다(`Runtime: !Send`). 스펙이 R5로 미룬 잔가지(E0707의 이름, 효과 표, E0506 도움말)와 남은 정리를 마친다.

**Architecture:**
- **작업 1: `World`를 `Runtime`으로.** `Runtime { world: RefCell<World>, shared: Arc<Shared> }`. `Task<'rt>`는 `Token` 대신 `RefMut<'rt, World>`를 쥔다. `Shared`에는 스레드에 안전한 것만 남는다. `unsafe impl Send for World`, `Token`, `token.rs`, `_not_sync` 표시를 지운다.
- **작업 2: 효과 표와 E0707.** 내장 함수마다 효과·순수를 적은 표, 모든 내장 함수가 분류됐는지 지키는 시험. E0707은 사용자가 부른 함수의 이름을 댄다.
- **작업 3: E0506 도움말.** 보내는 끝을 순회하면 도움말 한 줄.
- **작업 4: 풀의 `Drop`과 신호.** 런타임이 사라지면 풀의 노는 스레드가 곧바로 끝난다(R3 미룬 ①). 신호 처리가 사건을 게시하려고 띄우던 스레드를 지운다.
- **작업 5: 주석·문서·측정·노트.** 옛 런타임(실행권, `block`, 태스크마다 스레드, 교착 집계)을 설명하는 코드 주석과 현재 문서(05·09 설계 문서, `internals.html`)를 고친다.

**Tech Stack:** Rust 표준 라이브러리. `fung-vm`(`vm/{world,runtime,token,sched,block,m6,module,atom,stop,pool}.rs`, `vm.rs`, `signals.rs`, `prelude.rs`, 시험들), `fung-front`(`diag/catalog.rs`의 E0506 도움말), `fung-cli`(필요하면).

**Spec:** `docs/superpowers/specs/2026-10-18-concurrency-v2-r7-design.md` (11절 R단계 5, 15절의 "그 밖"·"`World`의 잠금"과 R5 여섯 줄). 노트 `docs/notes/R7-scheduler.md`의 R4b 절, open-questions의 미룬 목록.

## Global Constraints

- 사용자 방침(메모리 `plan-execution-style`)을 따른다.
  - 작업마다 TDD로 하고 그 작업의 시험만 돌린다. 작업 단위로 커밋한다.
  - 끝에 한 번만 한다: 전체 검사(fmt, clippy, `cargo test --locked --no-fail-fast`를 debug·release 둘 다, Docker Linux), 가장 능력 있는 모델의 브랜치 전체 검토, 수정.
- 사용자와의 대화·보고는 한국어. 코드·주석·커밋은 영어. 커밋 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **언어 의미는 셋만 바뀐다.**
  - E0707이 대는 이름
  - 순수한 내장 도우미가 `swap` 안에서 된다
  - E0506의 도움말
  기대 출력이 이 셋 밖에서 바뀌면 멈추고 묻는다. 셋 때문에 바뀐 것은 bless하고 diff를 검토한다.
- 작업 1 뒤 워크스페이스에 `unsafe impl Send`·`unsafe impl Sync`가 0개다. 이것을 시험이 지킨다.
- **모든 확인용 실행은 바깥에 `timeout`을 건다.**
- 새 크레이트 없음. 계산 위주와 `chan`·`spawn`·`wait`·`fs`·`exec` 벤치마크는 이 브랜치 시작점보다 5% 넘게 느려지지 않는다. `RefCell`의 빌림 확인이 잠금보다 싸므로 같거나 빨라야 한다.

## Review Focus

- **빌림이 겹침.** 실행 중(`Task`가 `World`를 빌림)에 `Runtime`의 공개 함수가 불리면 `RefCell`이 패닉한다. 지금 그런 길이 없어야 한다. 테스트 도우미, REPL, `fung test`, 모듈 로드(`import_module`의 `run_root`)도 포함이다. → 작업 1의 전체 시험, 그리고 `no_runtime_call_borrows_the_world_during_a_run`(공개 함수 목록을 실행 중의 내장 함수에서 부르면 E0801이 되는지 확인)
- **다른 스레드에서 오는 것.** 신호 스레드(`StopHandle`), 풀의 일, 표준 입력 읽기 스레드가 `Shared`만으로 일을 마친다. 정지 요청, 두 번째 신호의 곧바로 끝내기, `cancel_all`이 지금처럼 된다. → 기존 `signals`·`stop_tests`·`task_tests`
- **값이 스레드를 넘지 않음.** `Runtime`, `Task`, `World`가 `!Send`이고 `Shared`, `StopHandle`이 `Send + Sync`다. → 작업 1의 컴파일 시간 확인
- **효과 표의 빈틈.** 새로 더한 내장 함수가 분류 없이 들어오면 시험이 실패한다. 순수로 바뀐 도우미가 정말 순수하다(입출력·상태 변경 없음). → 작업 2의 `every_builtin_is_classified`
- **런타임이 사라질 때.** `fung test`가 파일마다 런타임을 만들어도 풀 스레드가 쌓이지 않는다. 도는 일은 끝까지 돌고(버리지 않음), 노는 스레드만 곧바로 끝난다. → 작업 4의 `dropping_the_pool_ends_its_idle_threads`

---

### Task 1: `World`를 `Runtime`으로

**Files:**
- Modify: `crates/fung-vm/src/vm/world.rs`(`Shared`, `Token` 삭제, `unsafe impl Send` 삭제), `crates/fung-vm/src/vm/runtime.rs`, `crates/fung-vm/src/vm.rs`(`Task`의 `world`, `Task::new`), `crates/fung-vm/src/vm/{module,m6,sched,stop,deadlock,channel,task,failure}.rs`(실행권 확인 호출), 시험(`tests.rs`, `world.rs`의 시험, `failure_tests.rs`, `sched_tests.rs`, `mutable_tests.rs`, `testutil.rs`)
- Delete: `crates/fung-vm/src/vm/token.rs`
- Test: `crates/fung-vm/src/vm/tests.rs`, 새 `crates/fung-cli/tests/no_unsafe_send.rs`(또는 `repo_rust_only.rs`에 더함)

**Interfaces:**
- Produces:
  - `pub struct Runtime { world: RefCell<World>, shared: Arc<Shared> }`. `Runtime::new`는 지금과 같은 인자.
  - `Shared`: `ready`, `pool`, `poller`, `next_fiber`, `next_token`, `stop_requested`, `failures`, `root`, 시험 탐침. `world` 필드와 `lock()`이 없다.
  - `Task<'rt>`: `world: RefMut<'rt, World>`, `shared: &'rt Arc<Shared>`. `Task::new(world: &'rt RefCell<World>, shared: &'rt Arc<Shared>, cancel: Arc<CancelNode>) -> Task<'rt>`. 실행 중에 같은 런타임으로 다른 `Task`를 만들면 빌림 패닉이다. 패닉 울타리가 E0801로 바꾼다. 지금 `assert_not_held`가 하던 일이다.
  - `assert_holds_token` 호출(값의 `Drop`, `release_values` 등)은 지운다. 그 불변식("값은 실행 스레드에만 있다")은 이제 타입(`!Send`)이 지킨다.
  - 컴파일 시간 확인: `Runtime: !Send`, `Shared: Send + Sync`, `StopHandle: Send + Sync`. 지금의 `AmbiguousIfSync` 기법과 같은 방식이다.
  - `Runtime`을 다른 스레드로 옮기던 시험은 그 스레드 안에서 런타임을 만든다(`sched_tests`의 `open_when_parked`, `failure_tests` 237행 부근). `the_runtime_can_move_to_another_thread`는 `the_runtime_stays_on_its_thread`(컴파일 시간 확인)로 바뀐다.
- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `repo_rust_only::no_unsafe_send_or_sync`: 워크스페이스의 `.rs`에 `unsafe impl Send`·`unsafe impl Sync`가 없다. 지금은 `world.rs`에 하나 있어 실패한다.
  - `tests::the_runtime_stays_on_its_thread`: 위의 컴파일 시간 확인(지금은 `Runtime: Send`라 컴파일되지 않음).
  - `tests::no_runtime_call_borrows_the_world_during_a_run`: 시험용 내장 함수가 실행 중에 같은 런타임의 `World`를 다시 빌리려 하면(`Task::new` 재진입) E0801(`already borrowed` 문구)이 되고, 그 뒤에도 런타임을 쓸 수 있다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-cli --test repo_rust_only && cargo test -p fung-vm -- tests::` → FAIL
- [ ] **Step 3: 구현한다.** 기계적인 바꾸기가 많다(`lock()` 30여 곳, `assert_holds_token` 11곳). 컴파일러를 따라간다.
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm && cargo test -p fung-cli --test signals --test repl_tty --test run_expected` → PASS. 벤치마크: `scripts/bench.sh /tmp/fung-r5-base target/release/fung`(시작점은 작업 1 전에 만든다).
- [ ] **Step 5: 커밋한다.** `refactor(vm)!: the world lives in the runtime, which stays on its thread; no unsafe Send`

### Task 2: 효과 표와 E0707

**Files:**
- Modify: `crates/fung-vm/src/vm/atom.rs`(`EFFECT_*`, `is_effect`, `check_effect`), `crates/fung-vm/src/vm/atom_tests.rs`, `docs/builtins.html`(`swap`의 효과 목록), `docs/notes/R7-language.md`(189행의 `conc` 언급)
- Test: `crates/fung-vm/src/vm/atom_tests.rs`

**Interfaces:**
- Produces:
  - `atom.rs`의 표 `EFFECTS: &[(&str, &[&str])]`(모듈 → 효과인 함수)와 `PURE: &[(&str, &[&str])]`(모듈 → 순수한 함수). 모듈째 표시(`EFFECT_MODULES`)는 지운다.
  - 순수: `_native.net`의 `byte_len`·`utf8_bytes`·`utf8_string`·`need_str_map`·`user_agent`·`local_addr`, 그리고 지금 효과가 아닌 prelude 함수 전부.
  - 효과: 소켓 연산, 파일 연산 전부(읽기 포함), `exec`·`sh`, 그리고 지금의 prelude 효과 16개.
  - `check_effect`가 E0707의 이름으로 사용자가 부른 함수를 댄다. 실행 중인 프레임을 안쪽부터 거슬러 올라가 내장(`builtin`)이 아닌 첫 프레임을 찾고, 그 프레임이 부른 함수의 이름(모듈 이름 포함, `fs.with_open`)을 쓴다. 내장 프레임이 없으면(사용자가 원시 함수를 직접 부름) 지금처럼 그 함수 이름이다.
- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `every_builtin_is_classified`: `boot`의 표에 있는 모든 내장 함수(prelude와 `_native.*`)가 `EFFECTS`와 `PURE` 중 정확히 한쪽에 있다.
  - `pure_helpers_run_inside_a_swap`: 내장 파일처럼 컴파일한 시험 프로그램(`Opts::internal`)에서 `import _native.net as net`, `a = atom(0)`, `print(swap(a, x => x + net.byte_len("é")))`가 `2`를 출력한다. 지금은 E0707이다.
  - `e0707_names_what_the_user_called`: `import fs`, `a = atom(0)`, `swap(a, x => fs.with_open("/tmp/x", _ => x))`의 에러 메시지가 `` `fs.with_open` has an effect ``이다(지금은 `` `open` ``).
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- atom_tests` → FAIL
- [ ] **Step 3: 구현한다.**
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- atom_tests && cargo test -p fung-cli --test golden --test run_expected --test manual` → PASS(바뀐 기대 출력은 bless하고 diff 검토)
- [ ] **Step 5: 커밋한다.** `fix(atom): E0707 names what was called; every builtin is classified`

### Task 3: E0506 도움말

**Files:**
- Modify: `crates/fung-vm/src/prelude.rs`(순회의 대상 검사), `crates/fung-front/src/diag/catalog.rs`(필요하면 도움말 문구), `spec/errors.txt`(다시 만듦)
- Test: `crates/fung-vm/src/vm/channel_tests.rs`

**Interfaces:**
- Produces: 보내는 끝(`Value::Sender`)을 순회하려는 E0506에 도움말 `` `for` and `map` go over a receiving end (`rx`) ``이 붙는다. 다른 값의 E0506은 그대로다.
- [ ] **Step 1: 실패하는 시험을 쓴다.** `channel_tests::iterating_a_sender_says_to_use_the_receiving_end`: `(tx, rx) = channel()`, `for x in tx: print(x) ;;`의 에러 문자열에 위의 도움말이 있다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- channel_tests::iterating` → FAIL
- [ ] **Step 3: 구현한다.**
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- channel_tests && cargo test -p fung-front spec_file && cargo test -p fung-cli --test run_expected` → PASS
- [ ] **Step 5: 커밋한다.** `fix(diag): iterating a sending end says to use the receiving end`

### Task 4: 풀의 `Drop`과 신호

**Files:**
- Modify: `crates/fung-vm/src/vm/pool.rs`(`Drop`), `crates/fung-vm/src/signals.rs`(`stop_requests`·`repl`이 스레드를 띄우지 않음)
- Test: `crates/fung-vm/src/vm/pool_tests.rs`, `crates/fung-cli/tests/signals.rs`

**Interfaces:**
- Produces:
  - 런타임이 쥐는 풀은 `PoolOwner`(`Shared::pool`, 하나뿐)이고, 그 `Drop`이 `shutdown` 표시를 세우고 모든 노는 스레드를 깨운다. 노는 스레드는 곧바로 끝난다. 줄에 남은 일과 도는 일은 끝까지 돈다(기다리는 일을 버리지 않음). `PoolOwner`가 나눠 주는 `Pool` 사본(`CommandWait`의 `Drop`이 거두기를 맡기는 데 씀)은 `shutdown` 뒤에도 `submit`할 수 있고, 그 일은 새 스레드가 받아 끝낸 뒤 그 스레드도 끝난다.
  - 신호 처리기(`signals::stop_requests`, `signals::repl`)는 `StopHandle::deliver`·`interrupt`를 그 자리에서 부른다(게시만 하므로 실행권을 기다리지 않음). 스레드를 띄우지 않는다.
- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `pool_tests::dropping_the_pool_ends_its_idle_threads`: 노는 스레드 셋이 있는 풀(`Pool::new(4, LONG)`)을 놓으면, 1초 안에 그 스레드들이 끝난다. 끝을 세는 시험용 셈 `Pool::live_threads()`는 전역 `AtomicUsize`로, 놓은 뒤에도 읽을 수 있다.
  - `pool_tests::dropping_the_pool_lets_a_running_job_finish`: 도는 일(걸쇠에서 막힘)이 있는 풀을 놓고 걸쇠를 열면, 그 일이 끝까지 돈다.
  - 기존 `signals` 시험(두 번째 신호, REPL Ctrl-C, 정지 요청)이 그대로 통과한다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- pool_tests` → FAIL
- [ ] **Step 3: 구현한다.**
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- pool_tests && cargo test -p fung-cli --test signals --test tools_expected` → PASS
- [ ] **Step 5: 커밋한다.** `fix(vm): a dropped pool ends its idle threads; signals post without a thread`

### Task 5: 주석·문서·측정·노트

**Files:**
- Modify:
  - 코드 주석: 조사에서 찾은 곳들(`vm.rs` 260~268행·769행, `signals.rs`, `atom.rs` 9행, `sched.rs` 16~17행, `world.rs`·`runtime.rs` 머리, `regex.rs`, `file.rs`, `fs.rs`, `net.rs`, `nat.rs`, `stdin.rs`, `channel.rs`, `scope.rs`, `failure.rs`, `deadline.rs`, `boot.rs`, `prelude/io.rs`, `sigpipe.rs`, `testutil.rs`의 `hold`, 시험의 문서 주석)
  - 문서: `docs/design/05-concurrency.md`(7·65·134~138·162·229~230행), `docs/design/09-implementation-plan.md`(35·141·172~177행), `docs/internals.html`(362·423행)
  - 노트 `docs/notes/R7-scheduler.md`(R5 절), `CHANGELOG.md`, `docs/design/open-questions.md`(R7 진행, R3 미룬 ① 닫기)
- [ ] **Step 1: 고친다.** 기준: 지금 코드에 없는 장치(실행권을 내려놓고 다시 잡기, 태스크마다 스레드, `block`, `outside`, 교착 집계, 게이트)를 현재형으로 설명하는 문장이 없다. 역사 기록(옛 단계 노트, 계획, `R-port-record.md`)은 고치지 않는다.
- [ ] **Step 2: 확인 시험을 더한다.** `repo_rust_only::no_current_text_describes_the_run_token`: 현재 코드와 현재 문서(역사 기록 제외)에 `run token`·`실행권`·`outside(`·`Census`가 없다. 허용 목록은 시험 안에 둔다(용어를 설명하는 역사 문장이 남는 곳).
- [ ] **Step 3: 잰다.** `scripts/bench.sh /tmp/fung-r5-base target/release/fung`. 노트 R5 절에 들어갈 것:
  - `Send`·`Sync`가 무엇을 약속하는지
  - `unsafe impl Send`가 왜 있었고, 이제 왜 타입이 대신 지키는지
  - `RefCell`의 빌림 확인이 잠금을 대신하는 방식
  - 남은 스레드(신호, 표준 입력, 풀)가 `Shared`만 쥐는 이유
  - 측정
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-cli --test repo_rust_only --test manual --test changelog` → PASS
- [ ] **Step 5: 커밋한다.** `docs: the old runtime is gone from code comments and current docs; R5 note`

### 끝에 한 번 (사용자 방침)

- [ ] 전체 검사(macOS debug·release, Docker Linux clippy·test; 알려진 컨테이너 실패만 남는지).
- [ ] 가장 능력 있는 모델로 브랜치 전체 검토(확인 실행에 바깥 시간 제한).
- [ ] 검토 결과를 한 번에 고치고(Critical·Important만, 실패하는 시험 먼저), 전체 검사를 다시 돌려 커밋한다.
- [ ] 사용자에게 한국어로 보고. 다음은 R6(측정과 마무리, R4a·R4b 미룬 목록 정리, rustls 0.23.46은 2026-10-28 이후) 계획.
