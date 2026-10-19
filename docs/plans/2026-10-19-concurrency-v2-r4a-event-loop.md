# 동시성 v2 R4a (이벤트 루프와 소켓) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 소켓과 TLS의 기다림을 기다림마다 스레드 하나(`offload_watched` + `Watch`의 `poll`)에서 kqueue(macOS)·epoll(Linux) 이벤트 루프 위의 주차로 옮긴다. 느린 클라이언트 수천이 붙어도 스레드가 늘지 않는다.

**Architecture:**
- **작업 1: 열린 파일 수 한도.** `fung`이 시작할 때 `RLIMIT_NOFILE`의 소프트 한도를 하드 한도까지 올리고, 자식에게는 원래 값을 넘긴다.
- **작업 2: 루프 핵심.** `vm/poller.rs`(겉 API)와 `vm/poller/kqueue.rs`·`vm/poller/epoll.rs`(두 구현). 준비 상태 기반, one-shot 등록, 시간 제한 있는 기다림, 다른 스레드에서 깨우기(`EVFILT_USER`·`eventfd`).
- **작업 3: 스케줄러가 루프에서 잔다.** `Ready`의 `Condvar` 잠을 루프의 기다림으로 바꾼다. 다른 스레드의 `push`·`post`는 깨우개를 당긴다. 내장 함수는 `park_io`(fd, 관심)로 주차한다. 입출력을 기다리는 태스크가 있으면 차례마다 루프를 시간 제한 0으로 본다(스펙 15절).
- **작업 4: 소켓을 루프 위로.** `net.rs`의 기다리는 내장 함수가 한 번 해 보고 `WouldBlock`이면 `park_io`로 주차한다. 연결의 읽기·쓰기 잠금(`Mutex`를 기다림 내내 쥠)은 주차하는 차례 줄로 바뀐다. 이름 조회는 풀(버리는 일)로. `take_body`도 차례 줄로.
- **작업 5: TLS를 루프 위로.** rustls의 `ClientConnection`을 블로킹하지 않는 소켓 위에서 같은 방식으로 돌린다(rustls 0.23.45, 2026-10-28 이후 버전만 올림).
- **작업 6: `Watch`와 소켓 다리 삭제, 측정, 문서.**

**Tech Stack:** Rust 표준 라이브러리와 `libc`(이미 `fung-vm`의 의존성). `fung-vm`(`vm/{poller,poller/kqueue,poller/epoll,sched,block,world,offload,watch}.rs`, `native/{net,tls,sock,process,nat}.rs`, 새 `rlimit.rs`), `fung-cli/src/main.rs`.

**Spec:** `docs/superpowers/specs/2026-10-18-concurrency-v2-r7-design.md` (10.1 이벤트 루프, 10.4 "HTTP 서버 동시 연결 수천", 15절의 "이벤트 루프"·"YIELD_EVERY"·"교착 판정"·"R4를 둘로"·"Linux 확인"·"열린 파일 수"). 노트 `docs/notes/R7-scheduler.md`의 R1b·R3 절.

## Global Constraints

- 사용자 방침(메모리 `plan-execution-style`)을 따른다.
  - 작업마다 TDD로 하고 그 작업의 시험만 돌린다. 작업 단위로 커밋한다.
  - 끝에 한 번만 한다: 전체 검사(fmt, clippy, `cargo test --locked --no-fail-fast`를 debug·release 둘 다), 가장 능력 있는 모델의 브랜치 전체 검토, 수정.
- 사용자와의 대화·보고는 한국어. 코드·주석·커밋은 영어. 커밋 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **언어 의미를 바꾸지 않는다.** 소켓·TLS·HTTP의 골든, 기대 출력(`http`·`tls`), 단위 시험이 정답지다. 기대 출력을 bless하지 않는다. 필요해 보이면 멈추고 묻는다.
- **Linux 확인**(스펙 15절): epoll 쪽을 건드린 작업은 Docker의 Linux 컨테이너에서 그 작업의 시험을 돌린다.
  - 명령: `docker run --rm -v "$PWD":/src:ro -v fung-cargo-registry:/usr/local/cargo/registry -w /src -e CARGO_TARGET_DIR=/tmp/target rust:1.94-slim sh -c '<그 작업의 cargo test 명령>'`
  - 이미지(`rust:1.94-slim`)를 처음 받기 전에 사용자에게 묻는다. 받을 수 없으면 그 사실을 판단으로 기록하고 끝의 보고에 적는다.
- 새 크레이트 없음. `unsafe`는 `libc` 호출과 그 `// SAFETY:` 근거뿐이다. 새 `unsafe impl Send`는 없다.
- fung 값(`Rc`)은 실행 스레드만 만진다. 루프의 깨우개만 다른 스레드에서 불린다.
- 루프는 one-shot 등록이다: 이벤트 하나가 온 등록은 다시 등록하기 전까지 다시 오지 않는다. 등록은 토큰(`u64`, 주차마다 새것)으로 찾고, 지난 토큰의 이벤트는 버린다.
- 동시성 시험은 상태로 순서를 정한다. 테스트가 띄우는 자식 프로세스는 `common::Reaped`로 쥔다.
- 계산 위주 벤치마크는 이 브랜치 시작점보다 5% 넘게 느려지지 않는다. `chan`·`spawn`·`wait`도 그렇다(입출력이 없으면 루프를 보지 않는다).

## Review Focus

- **잠든 스케줄러를 깨우기.** 실행 스레드가 루프에서 자려는 순간 다른 스레드(풀의 일, 표준 입력, 신호)가 `push`한다. 깨움이 사라지지 않아야 한다. → 작업 3의 `a_push_while_the_scheduler_sleeps_wakes_it`
- **닫힌 fd와 다시 쓰인 번호.** 다른 태스크가 등록된 소켓을 닫고, 같은 번호로 새 소켓이 열린다. 기다리던 태스크는 "닫힘"을 보고, 새 소켓에 대한 이벤트가 지난 토큰으로 엉뚱한 태스크를 깨우지 않는다. → 작업 3의 `a_stale_token_is_ignored`, 작업 4의 기존 `close_wakes_a_task_blocked_on_the_same_handle`
- **계산하는 태스크 옆의 입출력.** 한 태스크가 계산만 하는 동안에도 서버는 요청에 답한다(차례마다 시간 제한 0으로 루프를 봄). → 작업 3의 `io_is_seen_between_turns_of_a_computing_task`
- **교착 판정.** 소켓을 기다리는 태스크만 남았으면 교착이 아니다(`accept` 중인 서버를 기다림). → 기존 `awaiting_a_task_in_accept_is_no_deadlock`과 작업 3의 `a_task_parked_on_io_is_not_deadlocked`
- **연결 수천.** 연결 2,000개가 동시에 처리 중이어도 스레드 수가 늘지 않고, 파일 수 한도에 걸리지 않는다. → 작업 6의 `two_thousand_connections_at_once`

---

### Task 1: 열린 파일 수 한도

**Files:**
- Create: `crates/fung-vm/src/rlimit.rs`
- Modify: `crates/fung-vm/src/lib.rs`(모듈), `crates/fung-cli/src/main.rs`(`signals::init` 옆에서 부름), `crates/fung-vm/src/native/process.rs`(`pre_exec`에서 되돌림)
- Test: `crates/fung-vm/src/rlimit.rs`의 `#[cfg(test)]`, 새 `crates/fung-cli/tests/rlimit.rs`

**Interfaces:**
- Produces:
  - `pub fn raise_open_files()`: 한 번만 부른다. 지금 소프트 한도를 `ORIGINAL`(`OnceLock<libc::rlimit>`)에 적고, 소프트를 `min(하드, 플랫폼 한계)`로 올린다. macOS의 플랫폼 한계는 `sysctl kern.maxfilesperproc`(실패하면 `libc::OPEN_MAX`, 10240). 실패는 조용히 무시한다(올리지 못해도 지금과 같음).
  - `pub(crate) fn restore_in_child()`: `pre_exec` 안에서 `ORIGINAL`이 있으면 그 값으로 `setrlimit`. 비동기 신호 안전 함수만 부른다.
- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `rlimit::tests::raising_keeps_within_the_hard_limit`: 부른 뒤 소프트 ≤ 하드, 소프트 ≥ 부르기 전 값.
  - CLI `rlimit::fung_raises_its_soft_limit_and_children_get_the_original`: `sh -c 'ulimit -Sn 256; exec fung -e ...'`로 띄운다. fung 안에서 `sh("ulimit -Sn")`의 출력은 `256`이고(자식은 원래 값), 같은 프로그램이 임시 파일 하나를 `fs.open`으로 300번 열어 핸들을 모두 쥔 채 전부 `Ok`다(fung 자신의 한도는 256보다 큼).
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm rlimit && cargo test -p fung-cli --test rlimit` → FAIL
- [ ] **Step 3: 구현한다.**
- [ ] **Step 4: 통과를 확인한다.** 같은 명령 → PASS. Docker에서 같은 명령(Linux의 하드 한도 경로).
- [ ] **Step 5: 커밋한다.** `feat(cli): raise the open file limit at start; children get the original`

### Task 2: 루프 핵심

**Files:**
- Create: `crates/fung-vm/src/vm/poller.rs`, `crates/fung-vm/src/vm/poller/kqueue.rs`(`#[cfg(any(target_os = "macos", target_os = "freebsd"))]`), `crates/fung-vm/src/vm/poller/epoll.rs`(`#[cfg(target_os = "linux")]`), `crates/fung-vm/src/vm/poller_tests.rs`
- Modify: `crates/fung-vm/src/vm.rs`(모듈)

**Interfaces:**
- Produces (`vm/poller.rs`, 두 구현이 같은 모양):
  - `pub(crate) enum Interest { Read, Write }` (지금 `watch.rs`의 것을 옮김)
  - `pub(crate) struct Poller`: `Poller::new() -> io::Result<Poller>`. fd는 `CLOEXEC`.
  - `register(&self, fd: RawFd, interest: Interest, token: u64) -> io::Result<()>`: one-shot. 같은 fd·관심의 새 등록은 앞의 것을 바꾼다(`EV_ADD|EV_ONESHOT`, `EPOLL_CTL_MOD` 뒤 `ENOENT`면 `ADD`).
  - `deregister(&self, fd: RawFd, interest: Interest)`: 없으면 조용히 넘어간다.
  - `wait(&self, out: &mut Vec<u64>, timeout: Option<Duration>) -> io::Result<()>`: 준비된 등록의 토큰을 `out`에 더한다. `EINTR`이면 빈 채로 돌아온다. 깨우개의 이벤트는 토큰으로 내놓지 않고 소비한다.
  - `waker(&self) -> Waker`, `Waker: Send + Sync + Clone`, `Waker::wake(&self)`: 다른 스레드에서 `wait`를 깨운다(`EVFILT_USER`+`NOTE_TRIGGER`, `eventfd` 쓰기). 여러 번 당겨도 한 번 깨운다.
  - 오류 처리: `EV_ERROR`·`EPOLLERR`·`EPOLLHUP`도 그 토큰을 준비로 내놓는다(내장 함수가 다음 시스템 호출에서 오류를 본다).
- [ ] **Step 1: 실패하는 시험을 쓴다** (`poller_tests.rs`, 실제 `pipe`·`socketpair`)
  - `a_registered_read_fires_once`: 파이프에 쓰면 토큰이 한 번 나오고, 다시 등록하기 전에는 다시 나오지 않는다.
  - `write_interest_fires_on_a_writable_socket`
  - `the_timeout_ends_an_empty_wait`: 20 ms 시간 제한, 아무것도 없음 → 빈 `out`, 걸린 시간 ≥ 20 ms.
  - `a_waker_from_another_thread_ends_the_wait`: 시간 제한 없이 기다리는 동안 다른 스레드가 `wake` → 돌아온다. 기다리기 **전에** 당긴 깨움도 다음 `wait`를 곧바로 끝낸다.
  - `a_deregistered_fd_does_not_fire`
  - `a_hang_up_fires_the_read_token`: 파이프의 쓰는 끝을 닫으면 읽기 토큰이 나온다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- poller_tests` → FAIL
- [ ] **Step 3: 구현한다.** kqueue와 epoll 각 파일은 이 겉 API만 구현한다. `unsafe` 블록마다 `// SAFETY:`.
- [ ] **Step 4: 통과를 확인한다.** macOS에서 같은 명령, 그리고 Docker에서 같은 명령 → PASS
- [ ] **Step 5: 커밋한다.** `feat(vm): an event loop core over kqueue and epoll`

### Task 3: 스케줄러가 루프에서 잔다

**Files:**
- Modify: `crates/fung-vm/src/vm/sched.rs`(`Ready`, `Sched::run`), `crates/fung-vm/src/vm/world.rs`(`Shared::poller`, 입출력 등록표), `crates/fung-vm/src/vm/block.rs`(`park_io`)
- Test: `crates/fung-vm/src/vm/sched_tests.rs`

**Interfaces:**
- Consumes: 작업 2의 `Poller`, `Waker`, `Interest`.
- Produces:
  - `Shared::poller: Poller`. `Ready`는 `Waker`를 쥔다. `Ready::push`·`post`는 실행 스레드가 루프에서 자는 중(`ReadyState::sleeping`)이면 `wake()`.
  - `Ready::take_ids(.., wait: true, until)`: 대기열·편지함이 비었으면 `sleeping = true`로 놓고 잠금을 풀고 `poller.wait(가장 이른 타이머까지)`. 깨어나면 `sleeping = false`. 나온 토큰은 등록표에서 `Waiter`를 찾아 깨운다. 잠금을 푼 뒤 `wait` 전에 온 `push`는 깨우개가 남아 있으므로 `wait`가 곧바로 끝난다.
  - 등록표: `World::io: HashMap<u64, Arc<Waiter>>`(실행 스레드만), 토큰은 `Shared::next_token`(`AtomicU64`). `World::io_parked: usize`.
  - `pub(crate) fn park_io(t, fd: RawFd, interest: Interest, poll: impl FnMut(&mut Task, &Arc<Waiter>) -> Option<Result<Value, Stop>> + 'static, until: Option<Instant>) -> Result<Flow, Stop>`
    - `park_until_then` 위에 짠다. `poll`이 `None`(`WouldBlock`)이면 새 토큰으로 fd를 등록하고 등록표에 넣고 주차한다(`Wake { until, timed: true }`: 바깥이 깨우므로 교착 아님).
    - 깨어나면(또는 멈춤이면) 그 토큰을 등록표에서 빼고 `deregister`한다. 다시 `None`이면 새 토큰으로 다시 등록한다.
  - `Sched::run`: 등록표가 비어 있지 않으면, 대기열이 비지 않았어도 한 바퀴마다 `poller.wait(Some(0))`으로 준비된 것을 깨운다(스펙 15절: 입출력을 기다리는 태스크가 있을 때만).
- [ ] **Step 1: 실패하는 시험을 쓴다** (`sched_tests.rs`, 시험용 내장 함수 `pipe_pair()`·`pipe_read(fd)`·`pipe_write(fd, s)`가 `park_io`로 읽고 씀)
  - `a_task_parked_on_io_wakes_when_ready`: 한 태스크가 빈 파이프를 읽으며 주차하고, 다른 태스크가 쓴다. 읽은 값이 맞고 `offloads()`는 0이다.
  - `a_push_while_the_scheduler_sleeps_wakes_it`: 진입이 시험용 offload 일(풀)을 기다리며 주차하고 실행 스레드가 루프에서 잔다(탐침 `parked_probe == 1`을 본 시험 스레드가 걸쇠를 엶). 일이 끝나면 곧바로 깨어난다(5초 경계).
  - `a_stale_token_is_ignored`: 등록한 뒤 기다림이 취소되고(토큰 제거), 같은 fd가 준비된다. 다른 태스크가 깨지 않고 출력이 정확하다.
  - `io_is_seen_between_turns_of_a_computing_task`: 한 태스크가 `fib(27)`을 계산하는 동안 다른 태스크가 파이프에서 읽어 출력하고, 그 출력이 계산 결과보다 먼저 나온다(시험 스레드가 계산 태스크가 돌기 시작한 뒤 파이프에 씀: 계산 태스크가 첫 줄에서 `settle_mark`).
  - `a_task_parked_on_io_is_not_deadlocked`: 파이프를 기다리는 태스크만 남아도 E0702가 아니다. 시험 스레드가 쓰면 끝난다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- sched_tests` → FAIL
- [ ] **Step 3: 구현한다.**
- [ ] **Step 4: 통과를 확인한다.**
  - Run: `cargo test -p fung-vm -- sched_tests park_tests pool_tests stdin` → PASS. Docker에서 같은 명령.
  - Run: `cargo build --release --locked -p fung-cli && scripts/bench.sh /tmp/fung-r4a-base target/release/fung` → 계산 위주·`chan`·`spawn`·`wait` +5% 이하(시작점 바이너리는 작업 1 전에 만들어 둔다).
- [ ] **Step 5: 커밋한다.** `refactor(vm): the scheduler sleeps in the event loop; natives park on file descriptors`

### Task 4: 소켓을 루프 위로

**Files:**
- Modify: `crates/fung-vm/src/native/net.rs`, `crates/fung-vm/src/native/sock.rs`(필요하면)
- Test: `crates/fung-vm/src/native/net_tests.rs`

**Interfaces:**
- Consumes: 작업 3의 `park_io`.
- Produces:
  - `connect`·`listen`·`accept`·`read_line`·`read_raw_line`·`read_bytes`·`read_all`·`read_bytes_into`·`write`가 `nat::offload_watched` 대신 `park_io`로 기다린다. 각 내장 함수의 상태(읽던 버퍼, 쓴 바이트 수, 연결 단계)는 `poll` 클로저나 상자가 쥔다.
  - **차례 줄**: `Conn`의 `rd`·`wr` 잠금은 데이터만 잠깐 지키고, "지금 읽는 태스크"는 `Turnstile`(`Mutex<bool>`로 차지 여부 + 주차한 태스크의 `WaitList`)로 바뀐다. 차지는 실행 스레드에서만 바뀌므로 잠금은 짧다. 두 번째 읽는 태스크는 주차해서 기다리고, 그 기다림은 취소된다(지금은 취소되지 않음). `take_body`도 이 줄을 기다린다(풀의 일이 아님).
  - `set_deadline`의 기한은 `park_io`의 `until`로 준다. 지나면 `Err({kind: 'timeout})`(지금과 같음).
  - `close`(연결·리스너): 소켓을 꺼내고, 그 핸들에 주차한 태스크를 모두 깨운다(핸들마다 `WaitList`). 깨어난 쪽은 "닫힘"을 본다. 리스너의 `closer` 파이프는 지운다.
  - **이름 조회**: `resolve`의 `fung-lookup` 스레드와 `LookupSlot` 파이프 대신 `nat::offload_abandoning`(풀, 버리는 일). `lookups` 탐침은 그대로 쓴다.
  - 시험용 셈 `waiting`·`read_waits`·`write_waits`는 `park_io` 주차를 센다.
- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `net_natives_offload`를 `net_natives_park_on_the_loop`로 바꾼다: 소켓 내장 함수는 `offloads()`를 늘리지 않고 `parks()`를 늘린다. 이름 조회만 offload 하나.
  - `a_second_reader_can_be_cancelled_while_it_waits`: 두 번째 읽는 태스크를 `timeout(20ms, …)`로 감싸면 `Err({kind: 'timeout})`(지금은 첫 읽기가 끝날 때까지 끊기지 않음).
  - 기존 시험은 모두 그대로 통과해야 한다(취소·기한·닫기·서버·`reuse_port`).
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- native::net` → 바꾼 둘이 FAIL
- [ ] **Step 3: 구현한다.**
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- native::net && cargo test -p fung-cli --test http` → PASS. Docker에서 같은 명령.
- [ ] **Step 5: 커밋한다.** `refactor(net): sockets park on the event loop; name lookups run on the pool`

### Task 5: TLS를 루프 위로

**Files:**
- Modify: `crates/fung-vm/src/native/tls.rs`, `crates/fung-vm/src/native/net.rs`(`connect`의 TLS, `start_tls`)
- Test: `crates/fung-vm/src/native/tls_tests.rs`

**Interfaces:**
- Consumes: 작업 3의 `park_io`, 작업 4의 차례 줄.
- Produces: `handshake`·`fill`·`write`가 `wait_io` 대신, `WouldBlock`이면 rustls가 원하는 방향(`wants_write`이면 쓰기, 아니면 읽기)으로 `park_io`. `Session` 잠금은 주차 동안 쥐지 않는다(지금과 같음). 지금의 `Records` 추적(“TLS가 아님”, 레코드 중간의 끝)은 그대로다.
- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `tls_natives_park_on_the_loop`: TLS 연결의 `read_line`·`write`가 `offloads()`를 늘리지 않는다.
  - 기존 `tls_tests`와 `crates/fung-cli/tests/tls.rs`는 그대로 통과해야 한다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- native::tls` → FAIL
- [ ] **Step 3: 구현한다.**
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- native::tls native::net && cargo test -p fung-cli --test tls --test http` → PASS. Docker에서 같은 명령.
- [ ] **Step 5: 커밋한다.** `refactor(tls): TLS runs over the event loop`

### Task 6: `Watch` 삭제, 측정, 문서

**Files:**
- Delete: `crates/fung-vm/src/vm/watch.rs`(시험용 `live_wakes`·`serial` 도우미는 쓰는 곳이 남으면 옮김)
- Modify: `crates/fung-vm/src/vm/offload.rs`(`offload_watched` 삭제; `Kind::Thread`는 외부 명령이 R4b까지 씀), `crates/fung-vm/src/native/nat.rs`, `crates/fung-cli/tests/perf.rs`, `docs/notes/R7-scheduler.md`(R4a 절), `docs/internals.html`(동시성 절), `docs/design/open-questions.md`, `CHANGELOG.md`, `bench/`(필요하면)

**Interfaces:**
- Produces: `two_thousand_connections_at_once`(릴리스 전용 `perf.rs`)
  - fung 프로그램 하나가 `http.serve`를 띄우고, 처리기는 채널에서 신호를 기다린다. 클라이언트 태스크 2,000개가 동시에 `http.get`한다.
  - 처리기 2,000개가 모두 기다리는 중(처리기가 세는 `atom`이 2,000)일 때 프로그램이 `ready`를 출력한다. 시험은 그때 fung 프로세스의 스레드 수를 센다(macOS `ps -M -p <pid>`, Linux `/proc/<pid>/task`). 100 미만이어야 한다.
  - 그다음 신호를 보내면 모든 응답이 10초 안에 끝나고 출력이 맞다.
  - 셸에서 `ulimit -Sn 256`으로 띄워 작업 1의 한도 올리기도 함께 본다.
- [ ] **Step 1: 실패하는 시험을 쓴다.** `two_thousand_connections_at_once`. 작업 4·5 전의 바이너리로는 스레드 수가 2,000을 넘어 실패한다(그 바이너리로 한 번 확인해 기록한다).
- [ ] **Step 2: `Watch`와 `offload_watched`를 지운다.** Run: `cargo test -p fung-vm && cargo test -p fung-cli --test http --test tls --test signals` → PASS
- [ ] **Step 3: 잰다.** `scripts/bench.sh /tmp/fung-r4a-base target/release/fung`. 연결 2,000개의 시간과 최대 RSS(`/usr/bin/time -l`).
- [ ] **Step 4: 문서를 쓴다.** 노트 R4a 절에 들어갈 것:
  - 준비 상태 기반과 완료 기반(io_uring·IOCP)의 차이, one-shot을 고른 이유
  - 깨우개와 "자려는 순간의 깨움"이 사라지지 않는 이유(`sleeping` 표시와 남는 깨움)
  - 토큰으로 지난 이벤트를 거르는 이유(fd 번호는 다시 쓰인다)
  - 계산하는 태스크 옆에서 입출력을 보는 때(시간 제한 0)
  - 연결의 차례 줄과, 그래서 생긴 "두 번째 읽기도 취소된다"
  - 측정 표
  - CHANGELOG: 소켓·TLS가 스레드를 쓰지 않음, 두 번째 읽기의 기다림도 기한·취소에 끊김, 열린 파일 수 한도를 올림. open-questions: 서버의 "느린 클라이언트가 스레드 하나를 붙잡음" 닫기, R3 미룬 것 ②(`take_body`) 닫기.
- [ ] **Step 5: 통과를 확인한다.** Run: `cargo test --release -p fung-cli --test perf && cargo test -p fung-cli --test manual --test changelog --test repo_rust_only` → PASS. Docker에서 `cargo test --release -p fung-cli --test perf two_thousand`.
- [ ] **Step 6: 커밋한다.** `perf(net): thousands of connections without threads; R4a note, manual and changelog`

### 끝에 한 번 (사용자 방침)

- [ ] 전체 검사: `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`(debug·`--release`), `cargo test --locked --no-fail-fast`(debug·`--release`), `cargo deny check`·`cargo audit`(설치되어 있으면). Docker에서 `cargo test --locked --no-fail-fast`와 `cargo clippy --all-targets --locked -- -D warnings`.
- [ ] 가장 능력 있는 모델로 브랜치 전체 검토. 이 계획, 스펙 10.1·10.4·15절, Review Focus, 노트 R3 절을 준다.
- [ ] 검토 결과를 한 번에 고치고(Critical·Important만, 고칠 때마다 실패하는 시험 먼저), 전체 검사를 다시 돌려 커밋한다.
- [ ] 사용자에게 한국어로 보고: 무엇을 만들었고 어떤 원리인지, 측정, 내가 정한 것, 미룬 Minor. 다음은 R4b(외부 명령, REPL 두 번째 Ctrl-C).
