# 동시성 v2 R6 (측정과 마무리) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** R단계를 닫는다. 앞 단계 검토에서 미룬 것 중 견고함과 사용자에게 보이는 것을 고치고, 지연을 새로 재고, 10.4의 목표를 모두 재서 표로 남긴다. 나머지 미룬 것은 "알려진 한계"로 옮긴다.

**Architecture:**
- **작업 1: 명령의 끝 기다리기(pidfd 없는 Linux).** 풀 일의 `waitid`가 EINTR이면 다시 부르고, ECHILD이면 끝난 것으로 본다. 풀에 넣지 못하면 짧은 타이머로 `try_wait`를 다시 보는 길로 간다(깨어나지 못하는 일이 없다).
- **작업 2: 버린 입력의 뒷정리.** 버린 태스크의 핸들을 `await`하면 취소된 결과가 온다(`TaskEnd`가 결과 없이 사라지면 취소로 적고 기다리는 쪽을 깨운다). 버린 입력의 아직 시작하지 않은 풀 일은 돌지 않는다(`Job`이 사라지면 `ticket.abandon()`).
- **작업 3: 이벤트 루프 등록 표.** `World`에 fd → 토큰 색인을 두어 `drop_io_waits`가 표 전체를 훑지 않는다. 같은 (fd, 방향)에 둘째 기다림이 오면 디버그 단언이 잡는다.
- **작업 4: Linux의 열린 파일 한도.** 천장을 `/proc/sys/fs/nr_open`에서 읽는다. 하드 한도로 올리기가 실패하면 `nr_open`으로 한 번 더 해 본다.
- **작업 5: 진단과 시험.** 원시 함수를 바로 부른 E0707이 `fs.read`처럼 모듈 이름을 댄다. E0506 도움말이 동사와 무관하다. 효과 표에 없는 내장 함수 이름이 남으면 시험이 실패한다. 공개 API 흐름 시험을 되살린다. 틀린 주석 둘(`rlimit::restore_in_child`, `signal_group`)을 고친다.
- **작업 6: 지연 측정.** 릴리스 전용 perf 시험: 큰 JSON을 파싱하는 요청 옆에서 가벼운 요청의 응답 시간을 재서 출력한다. 한도는 1초.
- **작업 7: 측정, 노트, 문서.** 10.4의 표를 모두 재고, 노트에 "R단계 전체 요약"과 목표 대비 표를 쓴다. 미룬 목록을 닫거나 "알려진 한계"로 옮긴다. CHANGELOG.

**Tech Stack:** Rust 표준 라이브러리와 `libc`. `fung-vm`(`native/process.rs`, `vm/{scope,offload,block,world,sched,atom}.rs`, `rlimit.rs`, `native/nat.rs`, 시험들), `fung-cli`(`tests/perf.rs`, `tests/rlimit.rs`).

**Spec:** `docs/superpowers/specs/2026-10-18-concurrency-v2-r7-design.md` (10.4 성능 목표, 11절 R단계 6, 15절의 R6 세 줄). 노트 `docs/notes/R7-scheduler.md`, open-questions의 R1b·R3·R4a·R4b·R5 미룬 목록.

## Global Constraints

- 사용자 방침(메모리 `plan-execution-style`)을 따른다.
  - 작업마다 TDD로 하고 그 작업의 시험만 돌린다. 작업 단위로 커밋한다.
  - 끝에 한 번만 한다: 전체 검사(fmt, clippy, `cargo test --locked --no-fail-fast`를 debug·release 둘 다, Docker Linux), 가장 능력 있는 모델의 브랜치 전체 검토, 수정.
- 사용자와의 대화·보고는 한국어. 코드·주석·커밋은 영어. 커밋 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **언어 의미는 둘만 바뀐다.**
  - 버린 태스크의 핸들을 `await`하면 끝나지 않던 것이 취소된 결과가 된다(REPL의 두 번째 Ctrl-C 뒤에만 생김).
  - E0707과 E0506이 보여 주는 이름·도움말.
  기대 출력이 이 밖에서 바뀌면 멈추고 묻는다. 이 둘 때문에 바뀐 것은 bless하고 diff를 검토한다.
- **모든 확인용 실행은 바깥에 `timeout`을 건다.** 시험 하나는 `timeout 300`, 전체 시험은 `timeout 1800`.
- pidfd 없는 길과 `nr_open`은 Linux에서만 돈다. Docker 실행기(`scratchpad/linux.sh '<명령>'`, rust:1.94-slim)로 확인한다. 컨테이너에서만 나는 알려진 실패는 그대로 둔다:
  - root라 통과하는 권한 시험(`listings_keep_gos_order`, 걷기 시험, `with_temp_dir_reports_a_failed_removal`)
  - 128 TiB 스택 시험(`no_stack_size_fits_is_one_line_and_exit_1`, `every_size_refused…`)
  - 깊은 dump의 signal 9
- 새 크레이트 없음. 0.3.0 준비(CHANGELOG 판 번호, 매뉴얼 사이드바), rustls 0.23.46, 태그, main 스쿼시 머지는 이 계획에 없다.
- 계산 위주와 `chan`·`spawn`·`wait`·`fs`·`exec` 벤치마크는 R5 끝(`2f611dd`)보다 5% 넘게 느려지지 않는다.

## Review Focus

- **취소로 적는 때.** `TaskEnd`가 결과 없이 사라지는 길은 버림뿐이어야 한다. 정상 끝, 실패, 취소, 패닉 울타리(E0801)에서 결과가 두 번 적히거나 기다리는 쪽이 두 번 깨면 안 된다. → 작업 2의 `an_abandoned_tasks_handle_awaits_as_cancelled`와 기존 `scope_tests`·`task_tests`·`signals`
- **버린 풀 일이 무엇이었나.** 시작 전의 일만 버린다. 이미 도는 바꾸는 fs 일은 끝까지 돈다(15절). 평소의 취소(버림이 아닌)는 지금처럼 `Kind`를 따른다. → 작업 2의 `an_abandoned_inputs_queued_pool_jobs_do_not_run`과 기존 `offload_tests`
- **색인과 표가 어긋남.** 토큰이 표에서 빠지는 길(깨움, `drop_io_waits`, `IoWait`의 `Drop`, `Sched::abandon`)마다 색인에서도 빠져야 한다. 남으면 나중에 같은 fd 번호를 닫을 때 남의 기다림을 깨운다. → 작업 3의 `the_fd_index_empties_with_the_table`
- **pidfd 없는 길의 끝.** `waitid`가 ECHILD(누가 먼저 거뒀음)여도 태스크가 깬다. 풀이 가득해 넣지 못해도 명령의 끝을 본다. 취소와 기한이 이 길에서도 명령을 끝낸다. → 작업 1의 시험들(Docker)
- **지연 시험이 흔들림.** 무거운 요청이 정말 가벼운 요청보다 먼저 파싱을 시작했는지 상태로 확인한 뒤 잰다(`sleep`으로 순서를 정하지 않음). → 작업 6

---

### Task 1: 명령의 끝 기다리기 (pidfd 없는 Linux)

**Files:**
- Modify: `crates/fung-vm/src/native/process.rs`(`wait_on_the_pool` 355행 부근, `exited` 플래그를 보는 곳)
- Modify: `crates/fung-vm/src/vm/pool.rs`(시험 갈고리 `fail_next_submit`, `cfg(test)`)
- Test: `crates/fung-vm/src/native/process_tests.rs`

**Interfaces:**
- Produces:
  - `wait_on_the_pool`의 풀 일: `waitid(P_PID, pid, WEXITED | WNOWAIT)`를 EINTR이면 다시 부른다. 다른 에러(ECHILD 포함)도 끝난 것으로 보고 플래그를 세운다. `try_wait`가 진짜 결과를 정한다.
  - 풀에 넣지 못하면 `self.exited = None`인 채로 두지 않고, 명령의 보기가 `IoStep::Wait`의 기한을 `now + 10ms`로 잡아 다시 본다(`POLL_EXIT = 10ms`). 기다림이 루프의 타이머로 깨어나 `try_wait`를 부른다.
  - `Pool::fail_next_submit()`(`cfg(test)`): 다음 `submit_with` 하나가 `Err`를 낸다.
- [ ] **Step 1: 실패하는 시험을 쓴다** (`process_tests.rs`, `cfg(target_os = "linux")`, `disable_pidfd`와 함께)
  - `without_pidfd_a_full_pool_still_sees_the_end`: `fail_next_submit` 뒤 `sh("sleep 0.1")`이 `timeout(5s, …)` 안에서 `code == 0`으로 끝난다. 지금은 깨어나지 못해 기한에 끊긴다.
  - `without_pidfd_an_already_reaped_child_wakes_the_task`: 풀 일이 `waitid` 하기 전에 명령이 끝나고 거둬진 경우를 흉내 낸다. `cfg(test)` 갈고리 `reap_before_waitid`로 일이 `waitid` 앞에서 `waitpid(pid, 0)`을 먼저 부른다. 명령이 `timeout(5s, …)` 안에 결과를 낸다(종료 코드는 알 수 없으니 에러가 아닌 결과면 됨, 지금 동작에 맞춤).
- [ ] **Step 2: 실패를 확인한다.** Run: `scratchpad/linux.sh 'timeout 300 cargo test -p fung-vm without_pidfd'` → 두 시험 FAIL(기한)
- [ ] **Step 3: 구현한다.** 위의 Produces대로. EINTR 반복은 시험으로 만들기 어렵다. 검토에서 본다(이 계획의 판단).
- [ ] **Step 4: 통과를 확인한다.** Run: 같은 명령 → PASS. macOS에서 `timeout 300 cargo test -p fung-vm process_tests` → PASS(이 시험들은 Linux 전용이라 건너뜀)
- [ ] **Step 5: 커밋한다.** `fix(vm): a command's end is seen without pidfd when the pool is full or the child was reaped`

### Task 2: 버린 입력의 뒷정리

**Files:**
- Modify: `crates/fung-vm/src/vm/scope.rs`(`TaskEnd`에 `Drop`), `crates/fung-vm/src/vm/offload.rs`(`Job`에 `Drop`), `crates/fung-vm/src/vm/task.rs`(결과가 이미 있는지 보는 함수가 없으면)
- Test: `crates/fung-vm/src/vm/sched_tests.rs`, `crates/fung-cli/tests/signals.rs`

**Interfaces:**
- Consumes: `Runtime::take_abandoned`, `Sched::abandon`(R4b), `Ticket::abandon() -> bool`(`pool.rs` 49행)
- Produces:
  - `impl Drop for TaskEnd`: `state`에 결과가 없으면 `Err(cancelled)`를 적고 기다리는 쪽(`state.waiters`)을 깨운다. 정상 길은 결과를 적은 뒤 `TaskEnd`를 소비하므로 `Drop`은 아무것도 하지 않는다. 범위(`scope`)의 셈은 건드리지 않는다(범위도 함께 버려졌다).
  - `impl<R> Drop for Job<R>`: 일이 아직 끝나지 않았으면 `ticket.abandon()`. 시작한 일은 `abandon`이 `false`를 돌려주고 그대로 끝까지 돈다.
- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `sched_tests::an_abandoned_tasks_handle_awaits_as_cancelled`: 실행 하나가 태스크를 띄워 핸들을 전역 칸(시험 도우미의 `Rc`)에 두고 `recv`로 영원히 기다린다. 버린다(`World::abandon`, R4b 시험과 같은 도우미). 다음 실행이 그 핸들을 `await`하면 취소된 결과를 받는다(지금은 E0702).
  - `sched_tests::an_abandoned_inputs_queued_pool_jobs_do_not_run`: 풀 하나짜리 런타임에서 첫 일이 걸쇠(`Latch`)로 풀을 붙잡고, 둘째 일(바꾸는 fs 일 대신 플래그를 세우는 `offload`, `Kind::Wait`)이 줄에 선다. 입력을 버리고 걸쇠를 푼 뒤 풀이 빈 것을 상태로 확인하면 플래그는 서지 않았다.
  - `signals::an_abandoned_tasks_handle_in_a_later_input_is_cancelled`: REPL에서 `h = nil` 전역에 태스크 핸들을 담는 입력을 두 번 Ctrl-C로 버린 뒤, 다음 입력 `await(h)`가 취소로 보고된다(출력 문구는 지금의 취소 보고 그대로). 이 시험이 REPL 전역에 핸들을 둘 수 없으면(입력이 버려지며 전역 대입도 사라지면) 이 시험은 빼고 `Ruling:`으로 적는다.
- [ ] **Step 2: 실패를 확인한다.** Run: `timeout 300 cargo test -p fung-vm abandoned` → FAIL
- [ ] **Step 3: 구현한다.** 위의 Produces대로.
- [ ] **Step 4: 통과를 확인한다.** Run: `timeout 300 cargo test -p fung-vm abandoned`, `timeout 300 cargo test -p fung-vm scope_tests task_tests offload`, `timeout 300 cargo test -p fung-cli --test signals` → PASS
- [ ] **Step 5: 커밋한다.** `fix(vm): an abandoned task awaits as cancelled; an abandoned input's queued pool jobs do not run`

### Task 3: 이벤트 루프 등록 표

**Files:**
- Modify: `crates/fung-vm/src/vm/world.rs`(`io_fds`), `crates/fung-vm/src/vm/block.rs`(`drop_io_waits` 216행, 등록·해제 길), `crates/fung-vm/src/vm/sched.rs`(깨움, `abandon`)
- Test: `crates/fung-vm/src/native/net_tests.rs`

**Interfaces:**
- Produces:
  - `World::io_fds: HashMap<RawFd, Vec<u64>>`. 토큰을 `World::io`에 넣고 빼는 곳을 두 함수로 모은다: `World::add_io(token, waiter, key)`, `World::take_io(token) -> Option<(Waiter, IoKey)>`. `IoKey::Exit`는 색인에 넣지 않는다.
  - `drop_io_waits(t, fd)`는 `io_fds.remove(&fd)`의 토큰만 본다.
  - `add_io`의 `debug_assert!`: 같은 `IoKey::Fd(fd, interest)`인 토큰이 이미 있지 않다. 메시지는 `two waits on one fd and interest`.
- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `net_tests::the_fd_index_empties_with_the_table`: 연결 여럿을 열어 읽기로 기다리게 한 뒤, 하나는 데이터로 깨우고, 하나는 닫고(`drop_io_waits`), 하나는 취소하고, 남은 것은 범위를 끝내 정리한다. 끝에 `world.io`와 `world.io_fds`가 비었다. 지금은 `io_fds`가 없어 컴파일되지 않는다.
  - `net_tests::two_accepts_on_one_listener_trip_the_assert`(`cfg(debug_assertions)`): `_native.net.accept`를 같은 리스너에 두 태스크가 부르면 E0801(패닉 울타리)로 보고된다.
- [ ] **Step 2: 실패를 확인한다.** Run: `timeout 300 cargo test -p fung-vm net_tests` → 컴파일 실패 또는 FAIL
- [ ] **Step 3: 구현한다.** 위의 Produces대로. `World::io`에 직접 넣고 빼던 곳을 모두 두 함수로 바꾼다(`grep -n "\.io\." crates/fung-vm/src`로 찾음).
- [ ] **Step 4: 통과를 확인한다.** Run: `timeout 300 cargo test -p fung-vm net_tests process_tests sched_tests poller` → PASS. `http.serve`·`tls` CLI 시험도: `timeout 300 cargo test -p fung-cli --test http --test tls` → PASS(서버가 같은 (fd, 방향)을 두 번 기다리지 않는다는 확인)
- [ ] **Step 5: 커밋한다.** `perf(vm): closing a socket looks only at its own waits; assert one wait per fd and interest`

### Task 4: Linux의 열린 파일 한도

**Files:**
- Modify: `crates/fung-vm/src/rlimit.rs`(Linux `ceiling`, `raise_open_files`, `restore_in_child`의 주석)
- Test: `crates/fung-vm/src/rlimit.rs`의 시험 모듈(없으면 만듦), `crates/fung-cli/tests/rlimit.rs`

**Interfaces:**
- Produces:
  - Linux `ceiling()`: `/proc/sys/fs/nr_open`을 읽어 수로 바꾼 값. 읽지 못하면 `u64::MAX`(지금과 같음).
  - `raise_open_files`: `setrlimit`가 실패하고 `want > ceiling()`이 아니었으면 그대로 둔다. 하드 한도가 무한대(`RLIM_INFINITY`)이면 `want`는 `ceiling()`이다.
  - `restore_in_child`의 주석: `getrlimit`·`setrlimit`는 POSIX의 async-signal-safe 목록에 없지만 glibc·musl·macOS에서 시스템 호출 하나라 `fork` 뒤에 써도 된다고 적는다.
- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `rlimit::tests::the_linux_ceiling_is_nr_open`(`cfg(target_os = "linux")`): `ceiling()`이 `/proc/sys/fs/nr_open`의 값과 같다. 지금은 `u64::MAX`라 실패한다.
- [ ] **Step 2: 실패를 확인한다.** Run: `scratchpad/linux.sh 'timeout 300 cargo test -p fung-vm rlimit'` → FAIL
- [ ] **Step 3: 구현한다.**
- [ ] **Step 4: 통과를 확인한다.** Run: `scratchpad/linux.sh 'timeout 300 cargo test -p fung-vm rlimit && timeout 300 cargo test -p fung-cli --test rlimit'` → PASS. macOS에서 `timeout 300 cargo test -p fung-cli --test rlimit` → PASS
- [ ] **Step 5: 커밋한다.** `fix(vm): on Linux the open file limit is raised up to fs.nr_open`

### Task 5: 진단과 시험

**Files:**
- Modify: `crates/fung-vm/src/vm.rs`(`Task::user_callee` 643행), E0707을 내는 곳(`vm/atom.rs`), `crates/fung-vm/src/native/nat.rs`(`not_iterable` 156행), `crates/fung-vm/src/native/process.rs`(`signal_group`의 SAFETY 396행)
- Test: `crates/fung-vm/src/vm/atom_tests.rs`, `crates/fung-vm/src/vm/tests.rs`, `crates/fung-vm/src/vm/channel_tests.rs`(지금의 도움말 문장을 확인하는 시험), 도움말이 나오는 기대 출력·골든이 있으면 그것도

**Interfaces:**
- Produces:
  - E0707: 사용자 코드가 원시 함수를 바로 부르면(내장 함수 프레임이 없음) 그 원시 함수가 든 모듈의 이름을 붙여 `fs.read`로 댄다. prelude의 원시 함수는 지금처럼 이름만. 원시 함수의 모듈 이름은 원시 함수 표(`boot`)에서 찾는다.
  - E0506 도움말(보내는 끝): ``a sending end (`tx`) cannot be read; go over the receiving end (`rx`)``. `for`·`map`·`fold` 어느 동사든 같은 문장이다. 주석도 맞춘다.
  - `signal_group`의 SAFETY 주석: 이끄는 프로세스를 거둔 뒤에도 그룹에 남은 구성원이 있으면 그룹 번호는 재사용되지 않는다. 구성원이 모두 끝났으면 번호가 재사용될 수 있지만 `killpg`가 ESRCH가 되거나(빈 그룹) 같은 번호의 새 그룹에 닿을 수 있다는 점과, 그래서 거둔 직후에만 보낸다는 점을 적는다.
- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `atom_tests::e0707_names_a_direct_native_with_its_module`: `swap(a, x => fs.read("nope"))`(또는 `swap` 안에서 막힌다고 분류된 원시 함수 아무것)가 E0707의 이름 자리에 `fs.read`를 댄다. 지금은 `read`.
  - `atom_tests::every_listed_name_exists`: `EFFECTS`·`PURE` 표의 모든 (모듈, 이름)이 실제 내장 함수다(`boot`의 표에서 찾음). 지금 없는 이름이 있으면 실패, 없으면 일부러 없는 이름을 넣어 실패를 한 번 본 뒤 뺀다.
  - `atom_tests::e0506_help_does_not_name_a_verb`: `fold(tx, 0, (a, x) => a)`의 E0506 도움말이 위 문장이다.
  - `tests::the_public_api_runs_a_program_without_handing_out_values`: `Runtime::new`, 소스 실행, 출력 확인, 실패 보고까지 공개 API만으로 한 바퀴. R5에서 지운 것을 실행권 없이 다시 쓴다. 새 동작이 없으므로 처음부터 통과한다(되살리는 시험이라는 것을 `Ruling:`으로 적음).
- [ ] **Step 2: 실패를 확인한다.** Run: `timeout 300 cargo test -p fung-vm atom_tests` → 앞의 셋 FAIL
- [ ] **Step 3: 구현한다.** 위의 Produces대로. 기대 출력·골든의 E0506 도움말을 bless하고 diff가 도움말 줄만인지 본다(`FUNG_EXPECTED=bless timeout 600 cargo test -p fung-cli`, `timeout 300 cargo test -p fung-cli --test golden`).
- [ ] **Step 4: 통과를 확인한다.** Run: `timeout 300 cargo test -p fung-vm atom_tests tests::the_public_api`, `timeout 600 cargo test -p fung-cli --test golden --test run_expected --test manual` → PASS
- [ ] **Step 5: 커밋한다.** `fix(vm): E0707 names a direct native with its module; a verb-neutral E0506 help; check the effect table`

### Task 6: 지연 측정

**Files:**
- Modify: `crates/fung-cli/tests/perf.rs`

**Interfaces:**
- Produces: `perf::a_light_request_beside_a_heavy_parse`(릴리스 전용, 이 파일의 다른 시험과 같은 `cfg`).
  - fung 프로그램: `http.serve`의 처리기가 경로로 나눈다. `/heavy`는 처리기 안에서 큰 JSON 문자열(약 5 MB, 프로그램이 만든 배열 10만 개짜리)을 `json.parse`하고, 시작했다는 것을 채널로 알린다. `/light`는 `"ok"`를 돌려준다.
  - 클라이언트(같은 프로그램): `/heavy`를 띄우고, 시작 알림을 받은 뒤 `/light`의 시작부터 응답까지를 `time.now`로 재서 `light <ms>`와 `heavy <ms>`를 출력한다.
  - Rust 쪽: 출력에서 두 값을 읽어 `eprintln!`으로 남기고, `light < 1000`을 단언한다.
- [ ] **Step 1: 시험을 쓴다.** 새 동작이 없는 측정이므로 처음부터 통과할 수 있다. 시험이 정말 무엇을 재는지 한 번 확인한다: 무거운 요청을 빼면 `light`가 거의 0이고, 넣으면 늘어나는지 본다(`Ruling:`으로 적음).
- [ ] **Step 2: 돌린다.** Run: `timeout 300 cargo test --release -p fung-cli --test perf a_light_request -- --nocapture` → PASS, `light`·`heavy` 값을 기록
- [ ] **Step 3: 커밋한다.** `test(cli): measure a light request's latency beside a heavy JSON parse`

### Task 7: 측정, 노트, 문서

**Files:**
- Modify: `docs/notes/R7-scheduler.md`(R6 절, "R단계 전체 요약", 10.4 목표 대비 표), `docs/design/open-questions.md`(미룬 목록 닫기, "알려진 한계"로 옮기기, R7 진행 줄), `CHANGELOG.md`(`[Unreleased]`), `bench/README.md`(새 측정이 있으면)
- [ ] **Step 1: 잰다.** 기준선은 open-questions의 2026-10-17 값(옛 런타임)과 R5 끝(`2f611dd`)이다.
  - `scripts/bench.sh`로 `fib`·`loop`·`map`·`str`·`list`·`spawn`·`chan`·`wait`·`fs`·`exec`(9회 중앙값).
  - 기다리는 태스크 2만 개의 최대 메모리(`/usr/bin/time -l`), 연결 2,000개의 스레드 수·메모리(`two_thousand_connections_at_once`의 출력), 지연(작업 6).
  - 모든 실행에 `timeout`.
- [ ] **Step 2: 노트를 쓴다.** R6 절과 "R단계 전체 요약":
  - 처음 런타임(태스크마다 스레드 + 실행권)과 지금(M:1 + 이벤트 루프 + 풀)의 그림을 단계별로(R2 이어 가기, R1 스케줄러, R3 풀, R4 루프와 명령, R5 타입이 지키는 단일 스레드)
  - 무엇이 무엇을 대신했는지(스레드 → 파이버, 잠금 → `RefCell`, 깨우개 파이프 → 루프 하나, 교착 집계 → 대기열 판정)
  - 10.4 목표 대비 표(측정, 목표, 처음, 지금, 됐는지)
  - 남은 한계와 그 이유
- [ ] **Step 3: open-questions를 고친다.**
  - 닫는 것: R1b ⑤(이미), R4a ②③④⑤, R4b ②③⑥⑦, R5 ①③④⑤. 줄 머리에 "(닫힘, R6 2026-10-19)"와 무엇으로 닫았는지.
  - "알려진 한계"로 옮기는 것: R1b ⑥(오래된 깨움의 공정성), R4a ⑥(차례 줄의 공정성), R4b ①(출력 읽기 한도), R4b ④(한 묶음의 Ctrl-C 둘), R4b ⑤(버릴 때 해제 순서), R5 ②(`fung test`의 런타임이 사라지는 때), R3 ③(시험 도우미).
  - R7 진행 줄: "R1~R6 끝. 0.3.0 준비는 rustls 0.23.46 뒤".
- [ ] **Step 4: CHANGELOG `[Unreleased]`.** 고침: 버린 태스크의 `await`, pidfd 없는 Linux의 명령 끝, Linux의 열린 파일 한도. 바꿈: E0707의 모듈 이름, E0506 도움말.
- [ ] **Step 5: 통과를 확인한다.** Run: `timeout 300 cargo test -p fung-cli --test changelog --test manual --test repo_rust_only` → PASS
- [ ] **Step 6: 커밋한다.** `docs: R6 measurement and the R stages summary`

### 끝에 한 번 (사용자 방침)

- [ ] 전체 검사(macOS debug·release, Docker Linux clippy·test; 알려진 컨테이너 실패만 남는지).
- [ ] 가장 능력 있는 모델로 브랜치 전체 검토(확인 실행에 바깥 시간 제한).
- [ ] 검토 결과를 한 번에 고치고(Critical·Important만, 실패하는 시험 먼저), 전체 검사를 다시 돌려 커밋한다.
- [ ] 사용자에게 한국어로 보고: R단계 전체 요약, 10.4 표, 알려진 한계. 다음은 rustls 0.23.46(2026-10-28 이후)과 0.3.0 준비, 태그와 main 스쿼시 머지는 사용자가 말할 때.
