# 동시성 v2 R4b (외부 명령과 REPL) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 외부 명령(`exec`·`sh`)의 기다림을 명령마다 스레드 넷(기다리는 스레드, 출력 읽는 스레드 둘, 입력 넣는 스레드)에서 이벤트 루프 위의 주차로 옮긴다. REPL에서 멈춘 정리 중의 두 번째 Ctrl-C가 그 평가를 버리고 프롬프트로 돌아오게 한다. 이것으로 "기다림마다 스레드 하나"(`Kind::Thread`)가 모두 사라진다.

**Architecture:**
- **작업 1: 주차의 정리를 빠짐없이.** `park_io`의 한 번 보기가 에러나 패닉으로 끝나도, 그리고 태스크가 버려져도(작업 4) 연결의 차례와 셈을 놓는다(R4a 검토에서 미룬 ①).
- **작업 2: 명령의 끝을 루프로.** `Poller::register_exit(pid, token)`: macOS `EVFILT_PROC`+`NOTE_EXIT`, Linux `pidfd_open`의 fd를 읽기로. `ENOSYS`이면 풀의 일이 `waitid(WNOWAIT)`로 기다렸다가 깨운다. 명령 하나를 기다리는 상자 `CommandWait`가 "돈다 → (멈춤) 끝내는 중 → 끝남"을 쥔다.
- **작업 3: 파이프를 루프로.** 명령의 표준 출력·에러는 읽기, 표준 입력은 쓰기로 같은 상자가 등록한다. 10 ms 폴링, 읽는 스레드 둘, 넣는 스레드를 지운다. `Kind::Thread`와 `nat::offload_thread`를 지운다.
- **작업 4: REPL의 두 번째 Ctrl-C.** 평가가 이미 끊겼는데 또 Ctrl-C가 오면 스케줄러가 그 평가의 태스크를 모두 버리고 돌아온다. 버린 기다림은 채널 줄·차례 줄·입출력 표에서 빠지고(`Drop`), 기다리던 명령은 SIGKILL로 끝낸다(스펙 15절). REPL은 한 줄을 남기고 다음 입력으로 간다.
- **작업 5: 문서, 측정, 노트.** `request_stop()`의 REPL 동작(보는 쪽이 없으면 세션을 끝냄)을 매뉴얼과 08에 적는다.

**Tech Stack:** Rust 표준 라이브러리와 `libc`. `fung-vm`(`vm/{poller,poller/kqueue,poller/epoll,block,sched,stop,runtime,channel,world,offload}.rs`, `native/{process,net,nat}.rs`), `fung-cli`(`repl/mod.rs`).

**Spec:** `docs/superpowers/specs/2026-10-18-concurrency-v2-r7-design.md` (10.1, 15절의 "자식 프로세스"·"REPL: 멈춘 release에서 두 번째 Ctrl-C"·"REPL: request_stop()"·"버린 평가의 외부 명령"·"R4b의 모양"). 노트 `docs/notes/R7-scheduler.md`의 R4a 절, open-questions "R4a 검토에서 미룬 것".

## Global Constraints

- 사용자 방침(메모리 `plan-execution-style`)을 따른다.
  - 작업마다 TDD로 하고 그 작업의 시험만 돌린다. 작업 단위로 커밋한다.
  - 끝에 한 번만 한다: 전체 검사(fmt, clippy, `cargo test --locked --no-fail-fast`를 debug·release 둘 다, Docker Linux), 가장 능력 있는 모델의 브랜치 전체 검토, 수정.
- 사용자와의 대화·보고는 한국어. 코드·주석·커밋은 영어. 커밋 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **언어 의미는 하나만 바뀐다**: REPL의 두 번째 Ctrl-C. 외부 명령의 동작은 그대로다. 취소되면 SIGTERM, 2초(`TERM_GRACE`) 뒤 SIGKILL이고, 자기 그룹이면 그룹 전체에 보낸다. 명령이 끝난 뒤에도 뒤에 남은 것이 출력을 쥐고 있으면 닫힐 때까지 읽는다. 기대 출력은 bless하지 않는다.
- **Linux 확인**: epoll·pidfd 쪽을 건드린 작업은 Docker(`scratchpad/linux.sh`, 공식 `rust:1.94-slim`, `kill`을 위해 `procps`를 깖)에서 그 작업의 시험을 돌린다. 컨테이너만의 실패(root라서 권한 시험, VM 메모리, 128 TiB 스택)는 R4a에서 확인한 대로 판단으로 기록한다.
- **모든 확인용 실행은 바깥에 `timeout`을 건다**(셸의 `timeout`과 fung의 `timeout(…)`). R4a에서 시간 제한 없는 확인이 몇 시간 멈췄다.
- 고아 프로세스 금지: 시험이 띄우는 자식은 `common::Reaped`로 쥐고, 런타임이 띄운 명령도 버려지면 끝내고 거둔다.
- 새 크레이트 없음. `unsafe`는 `libc` 호출과 그 `// SAFETY:`뿐.
- 계산 위주와 `chan`·`spawn`·`wait`·`fs` 벤치마크는 이 브랜치 시작점보다 5% 넘게 느려지지 않는다.

## Review Focus

- **이미 끝난 명령.** 명령이 등록보다 먼저 끝난다(`EVFILT_PROC`의 `ESRCH`, pidfd가 이미 읽을 수 있음). 기다림이 영영 끝나지 않으면 안 된다. → 작업 2의 `an_exited_child_fires_its_exit_token`, `a_child_that_already_exited_fires_at_once`
- **두 파이프가 다 차는 명령.** 출력과 에러를 한꺼번에 많이 쓰고 입력도 많이 받는 명령이 멈추지 않는다(셋을 모두 루프에서 본다). → 작업 3의 `a_command_with_much_input_and_output_on_both_streams`
- **뒤에 남은 손자.** `sh("sleep 30 &")`처럼 끝난 명령의 손자가 출력을 쥐면 지금처럼 그것이 닫힐 때까지 기다리고, 취소되면 그룹을 끝낸다. → 기존 `a_cancel_after_the_command_itself_ended_ends_what_it_left`와 작업 3의 `a_grandchild_holding_the_output_is_waited_for`
- **버린 평가 뒤의 세션.** 두 번째 Ctrl-C로 버린 평가가 쥐고 있던 채널·연결을 다음 입력이 쓸 수 있다(줄에 죽은 기다림이 남지 않음). 기다리던 명령은 끝나고 거둬진다. → 작업 4의 `a_second_ctrl_c_abandons_a_stuck_release`, `an_abandoned_evaluation_leaves_its_channel_usable`, `an_abandoned_command_is_killed_and_reaped`
- **pidfd가 없는 Linux.** `pidfd_open`이 `ENOSYS`이면 풀이 기다린다. 그 길도 같은 결과다. → 작업 2의 `a_pool_wait_ends_a_command_without_pidfd`(시험용 설정으로 pidfd를 끔)

---

### Task 1: 주차의 정리를 빠짐없이

**Files:**
- Modify: `crates/fung-vm/src/vm/block.rs`(`park_io`, `IoWait`), `crates/fung-vm/src/native/net.rs`(`run_io`의 상태)
- Test: `crates/fung-vm/src/native/net_tests.rs`, `crates/fung-vm/src/vm/sched_tests.rs`

**Interfaces:**
- Produces:
  - `IoWait`은 한 번 보기(`poll` → `after`)가 `Err`이면 `leave`와 `on_stop`을 부른 뒤 그 에러를 돌려준다. 첫 보기(`park_io` 안)도 같다. `poll`의 패닉은 `catch_unwind` 없이도 정리되게, 정리는 `run_io`의 상태 값(`IoState`, `Rc` 안)의 `Drop`이 한다: 차례를 쥐고 있으면 놓고, `WAITING`에 세어져 있으면 뺀다. `end()`는 그 정리를 한 번만 하는 같은 함수다.
  - 시험용: `Poller::fail_next_register()`(`cfg(test)`): 다음 `register`가 `ENOSPC`로 실패한다.
- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `net_tests::a_failed_registration_releases_the_turn`: 연결의 읽기가 막혀 등록하려는 순간 등록이 실패한다(`fail_next_register`). 그 호출은 `error[E0801]`(`cannot wait for a descriptor`)을 `supervise`의 처리기로 보내고, 같은 연결의 다음 `read_line`은 데이터를 받는다(차례가 풀려 있음). `net_waiting()`은 0.
  - `sched_tests::a_dropped_io_wait_releases_its_state`: `run_io`로 만든 상자를 기다리는 중에 그대로 버리면(`drop`), 차례가 풀리고 셈이 돌아온다(시험용 내장 함수로 상자를 만들고 꺼내 버림).
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- a_failed_registration a_dropped_io_wait` → FAIL
- [ ] **Step 3: 구현한다.**
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- native::net sched_tests` → PASS
- [ ] **Step 5: 커밋한다.** `fix(vm): a socket wait releases its turn on every way out`

### Task 2: 명령의 끝을 루프로

**Files:**
- Modify: `crates/fung-vm/src/vm/poller.rs`, `poller/kqueue.rs`, `poller/epoll.rs`, `crates/fung-vm/src/vm/block.rs`(여러 등록을 쥐는 도우미), `crates/fung-vm/src/native/process.rs`
- Test: `crates/fung-vm/src/vm/poller_tests.rs`, `crates/fung-vm/src/native/process_tests.rs`

**Interfaces:**
- Produces:
  - `Poller::register_exit(&self, pid: libc::pid_t, token: u64) -> io::Result<Exit>`
    - kqueue: `EVFILT_PROC` + `NOTE_EXIT` + `EV_ONESHOT`. 이미 끝난 프로세스(`ESRCH`)는 `Exit::Already`.
    - epoll: `pidfd_open`(`libc::syscall(libc::SYS_pidfd_open, pid, 0)`)의 fd를 `CLOEXEC`로 읽기 등록, `Exit::Fd(OwnedFd)`(기다림이 그 fd를 쥠, 닫히면 등록도 사라짐). `ENOSYS`이면 `Err(Unsupported)`.
    - `deregister_exit(&self, pid, &Exit)`.
  - `vm/block.rs`의 `IoRegs`: 한 상자가 여러 등록을 함께 쥐는 도우미. `add(t, IoKey, token)`(`IoKey::Fd(fd, Interest)` | `IoKey::Exit(pid)`), `leave_all(t)`. 표 `World::io`의 값에 등록 종류를 넣는다.
  - `process.rs`의 `CommandWait` 상자(이어 가기):
    - 상태: `Running` → (멈춤이 오면) `Terminating { until: now + TERM_GRACE }` → `Exited`. 끝내는 중의 기한이 지나면 SIGKILL. 멈춤을 받으면 R3의 기다리는 일처럼 새 루트 노드로 바꿔 끼우고 계속 기다린다. 끝나면 노드를 되돌리고 `check_cancelled`가 되감는다(지금 `finish`와 같음).
    - 끝남 알림을 받으면 `Child::try_wait`로 상태를 거둔다.
    - pidfd가 없으면(`Unsupported`) 풀의 일이 `waitid(P_PID, pid, WEXITED | WNOWAIT)`로 기다렸다가 상자를 깨운다(거두기는 `try_wait`가 함).
  - 이 작업에서 파이프는 아직 지금의 스레드가 읽는다(작업 3에서 옮김). `exec`·`sh`는 `nat::offload_thread` 대신 `CommandWait`로 주차한다.
  - 시험용: `Poller::disable_pidfd()`(`cfg(test)`, Linux에서 `Unsupported`인 척).
- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `poller_tests::an_exited_child_fires_its_exit_token`: `sleep 0.05` 자식의 끝을 등록하면 5초 안에 토큰이 나온다.
  - `poller_tests::a_child_that_already_exited_fires_at_once`: 이미 끝났지만 거두지 않은 자식(좀비)의 끝을 등록하면 곧바로 끝난 것으로 본다(`Exit::Already`이거나 토큰이 곧바로 나옴).
  - `process_tests::exec_parks_on_the_loop`: `exec(["true"])`가 `offloads()`를 늘리지 않는다.
  - `process_tests::a_pool_wait_ends_a_command_without_pidfd`(Linux만): `disable_pidfd()` 뒤에도 `exec(["true"])`의 결과가 같다.
  - 기존 `process_tests`(취소·그룹·SIGTERM 무시·`exit`·손자)는 그대로 통과해야 한다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- poller_tests process` → FAIL
- [ ] **Step 3: 구현한다.**
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- poller_tests process && cargo test -p fung-cli --test signals` → PASS. Docker에서 같은 명령.
- [ ] **Step 5: 커밋한다.** `refactor(process): a command's end comes from the event loop`

### Task 3: 파이프를 루프로

**Files:**
- Modify: `crates/fung-vm/src/native/process.rs`(`read_all`·`feeder`·`end_group`의 폴링 삭제), `crates/fung-vm/src/vm/offload.rs`(`Kind::Thread` 삭제), `crates/fung-vm/src/native/nat.rs`(`offload_thread` 삭제)
- Test: `crates/fung-vm/src/native/process_tests.rs`, `crates/fung-cli/tests/perf.rs`

**Interfaces:**
- Produces:
  - `CommandWait`이 우리 쪽 파이프 끝 셋을 `O_NONBLOCK`으로 놓고 등록한다: 표준 출력·에러는 읽기, 표준 입력은 쓰기. 준비되면 읽은 바이트를 모으고 입력을 넣는다. 입력을 다 넣으면 그 끝을 닫는다. `EPIPE`는 에러가 아니다(명령이 일찍 그만 읽음).
  - 끝남 뒤: 두 출력이 끝(EOF)에 이를 때까지 계속 읽는다(뒤에 남은 손자). 그사이 멈춤이 오면 그룹에 SIGTERM, `TERM_GRACE` 뒤 SIGKILL, 출력은 버린다(지금 `cut`과 같음).
  - `read_all`, 넣는 스레드, 10 ms 폴링(`run_process`의 반복, `terminate`·`end_group`의 `sleep`)을 지운다. 기다림은 모두 루프의 등록과 상자의 기한(`Wake::until`)이다.
  - `Kind::Thread`, `nat::offload_thread`를 지운다(쓰는 곳이 없음).
- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `process_tests::a_command_with_much_input_and_output_on_both_streams`: `sh("cat; cat >&2", stdin: s)`에 1 MB의 `s`를 넣으면, 출력과 에러가 둘 다 `s`이고 5초 안에 끝난다.
  - `process_tests::a_grandchild_holding_the_output_is_waited_for`: `sh("(sleep 0.2; echo late) & echo now")`의 출력은 `"now\nlate\n"`(손자가 닫을 때까지 읽음).
  - `perf::fifty_commands_at_once`(릴리스): 태스크 50개가 동시에 `sh("sleep 1")`을 기다리는 동안 프로세스의 스레드가 20개 미만이다(`threads_of`). 지금은 명령마다 넷이라 200개를 넘는다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- process && cargo test --release -p fung-cli --test perf fifty` → `fifty_commands_at_once` FAIL
- [ ] **Step 3: 구현한다.**
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- process native:: && cargo test -p fung-cli --test signals --test run_expected && cargo test --release -p fung-cli --test perf` → PASS. Docker에서 같은 명령.
- [ ] **Step 5: 커밋한다.** `refactor(process): a command's pipes run on the event loop; no thread waits for one`

### Task 4: REPL의 두 번째 Ctrl-C

**Files:**
- Modify: `crates/fung-vm/src/vm/stop.rs`(`Event::Interrupt`), `crates/fung-vm/src/vm/world.rs`(`World::abandon`), `crates/fung-vm/src/vm/sched.rs`(버리기), `crates/fung-vm/src/vm/runtime.rs`(`recover_interrupt`가 버렸는지 알려 줌), `crates/fung-vm/src/vm/channel.rs`(`RecvWait`·`SendWait`의 `Drop`), `crates/fung-vm/src/native/process.rs`(`CommandWait`의 `Drop`), `crates/fung-cli/src/repl/mod.rs`
- Test: `crates/fung-vm/src/vm/repl_tests.rs`, `crates/fung-vm/src/vm/channel_tests.rs`, `crates/fung-cli/tests/signals.rs`

**Interfaces:**
- Produces:
  - `Event::Interrupt`가 이미 끊긴 평가(`World::interrupted`)에 또 오면 `World::abandon = true`.
  - `Sched::run`은 사건을 처리한 뒤 `abandon`이면 모든 칸(뿌리 빼고)과 타이머를 버리고, 입출력 표를 비우고(등록 해제), 뿌리의 상태도 비운 뒤 `Err(Stop::Error(E0701))`로 돌아온다. 버린 태스크의 값은 실행권을 쥔 채 놓인다.
  - 버린 기다림은 자기가 선 줄에서 빠진다(`Drop`): `RecvWait`·`SendWait`는 채널의 줄(`Channel::leave`), 연결의 차례(작업 1의 `IoState`), 표준 입력의 기다리는 목록(`Stdin::leave`).
  - `CommandWait`의 `Drop`: 명령이 아직 돌면 SIGKILL(자기 그룹이면 그룹 전체)하고, `Child`를 풀의 일로 넘겨 거둔다(스펙 15절).
  - `Runtime::recover_interrupt() -> Option<Interrupted>`(`Interrupted::Cancelled` | `Interrupted::Abandoned`). REPL은 `Abandoned`이면 표준 에러에 한 줄을 쓴다: `interrupted again: the cleanup did not finish; the input's tasks were dropped`(에러 번호 없는 알림, 08의 REPL 절에 적음).
- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `signals::a_second_ctrl_c_abandons_a_stuck_release`: REPL에 `with(() => nil, _ => sleep(60s), _ => fn(): print("releasing"); sleep(60s) ;;())`를 넣고 `ready`를 본 뒤(본문 첫 줄에서 출력) SIGINT → `releasing`을 본 뒤 SIGINT → 다음 입력 `print("after")`의 출력이 나오고, 표준 에러에 위의 한 줄이 있고, 종료 코드 0.
  - `channel_tests::an_abandoned_evaluation_leaves_its_channel_usable`(런타임 수준): REPL 세션 시험 도우미로 첫 입력이 전역 채널 `(tx, rx) = channel()`을 만들고, 둘째 입력이 `recv(rx)`를 기다리는 정리에서 버려진다. 셋째 입력의 `spawn` + `send(tx, 1)` / `recv(rx)`가 `1`을 받는다(죽은 기다림이 값을 가져가지 않음).
  - `signals::an_abandoned_command_is_killed_and_reaped`: 정리 안의 `sh("sleep 60")`을 두 번째 Ctrl-C로 버리면, 그 `sleep`의 프로세스가 5초 안에 사라진다(`kill -0` 실패, 좀비도 아님).
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-cli --test signals a_second_ctrl_c an_abandoned && cargo test -p fung-vm -- channel_tests::an_abandoned` → FAIL(첫 시험은 둘째 SIGINT 뒤에도 끝나지 않음: 시간 제한으로 실패)
- [ ] **Step 3: 구현한다.**
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- repl_tests channel_tests stop_tests && cargo test -p fung-cli --test signals --test repl_tty` → PASS
- [ ] **Step 5: 커밋한다.** `feat(repl): a second Ctrl-C abandons an input stuck in its cleanup`

### Task 5: 문서, 측정, 노트

**Files:**
- Modify:
  - `docs/builtins.html`(`request_stop`: REPL에서 보는 쪽이 없으면 `exit(0)`처럼 세션을 끝냄), `docs/design/08-tooling-and-errors.md`(REPL의 Ctrl-C 두 번, `request_stop`)
  - `docs/internals.html`(동시성 절: 외부 명령도 루프 위, 스레드를 쓰는 기다림은 표준 입력 읽기와 풀뿐)
  - `docs/notes/R7-scheduler.md`(R4b 절), `CHANGELOG.md`, `docs/design/open-questions.md`(R4a 미룬 ① 닫기, R7 진행)
  - `bench/README.md`와 새 `bench/exec.fung`(`exec(["true"])` 500번; `run_expected`·`dump_expected`의 사례 수와 bless한 새 항목, 다른 항목이 바뀌면 멈춤)
- [ ] **Step 1: 잰다.** 작업 1 전의 릴리스 빌드(`/tmp/fung-r4b-base`)와 `scripts/bench.sh`. `exec.fung`은 명령마다 스레드 넷을 만들던 시작점보다 빠르거나 같다.
- [ ] **Step 2: 문서를 쓴다.** 노트 R4b 절: 프로세스의 끝을 기다리는 세 길(`EVFILT_PROC`, pidfd, `waitid(WNOWAIT)`)과 좀비, 파이프 셋을 한 상자가 쥐는 이유(둘 중 하나만 읽으면 명령이 멈춤), 손자, 버리기와 `Drop`이 지켜야 하는 것, 측정.
- [ ] **Step 3: 통과를 확인한다.** Run: `cargo test -p fung-cli --test run_expected --test dump_expected --test manual --test changelog --test repo_rust_only` → PASS
- [ ] **Step 4: 커밋한다.** `perf(process): commands wait on the event loop; R4b note, manual and changelog`

### 끝에 한 번 (사용자 방침)

- [ ] 전체 검사: `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`(debug·`--release`), `cargo test --locked --no-fail-fast`(debug·`--release`), Docker에서 clippy와 `cargo test --locked --no-fail-fast`(알려진 컨테이너 실패만 남는지).
- [ ] 가장 능력 있는 모델로 브랜치 전체 검토. 이 계획, 스펙 10.1·15절, Review Focus, 노트 R4a 절. 검토자의 확인 실행에도 바깥 시간 제한을 요구한다.
- [ ] 검토 결과를 한 번에 고치고(Critical·Important만, 고칠 때마다 실패하는 시험 먼저), 전체 검사를 다시 돌려 커밋한다.
- [ ] 사용자에게 한국어로 보고. 다음은 R5(옛 런타임 삭제: `World` 잠금, `unsafe impl Send`, 잔가지)와 rustls 0.23.46(2026-10-28 이후).
