# 동시성 v2 L2 (기한 범위) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `timeout(d, f)`를 넣는다. `f` 안의 모든 것(자식 태스크 포함)을 `d` 뒤에 취소하고 `Err({kind: 'timeout})`을 돌려준다. `await`·`recv`의 `timeout:` 매개변수는 지운다. HTTP 클라이언트의 기한도 이 범위로 다시 짠다.

**Architecture:**
- **기한은 취소 노드에 싣는다.** `CancelNode`에 `deadline: Option<Deadline>`(시각 + 그 기한을 정한 노드)을 두고, `child()`가 부모의 기한을 물려받는다.
  - 타이머 스레드는 두지 않는다. 막힌 태스크는 기한까지만 잔다(`block_until`·`Watch::wait`·표준 입력 대기). 계산하는 태스크는 차례 넘기기 지점(`YIELD_EVERY`마다)에서 시계를 본다.
  - 기한을 처음 본 쪽이 주인 노드를 **만료**시킨다. 만료는 `expired` 표시를 세우고 노드를 취소하는 일이다.
  - 기한이 있는 기다림은 교착 집계에서 "타이머가 깨울 수 있음"으로 세어지므로, 기한 안에서 막힌 태스크를 교착으로 몰지 않는다.
- **`timeout(d, f)`는 범위처럼 동작한다.** 노드 하나를 만들어 본문을 그 아래에서 돌리고, 끝나면 결과를 정한다. 기한 안에서 연 `scope`의 자식은 범위 노드를 거쳐 기한을 물려받는다.

**Tech Stack:** Rust 표준 라이브러리, `fung-vm` 런타임(`vm/task.rs`의 `CancelNode`, `vm/block.rs`, `vm/watch.rs`, `vm/stdin.rs`, `vm/channel.rs`), `extras/http.fung`.

**Spec:** `docs/superpowers/specs/2026-10-18-concurrency-v2-r7-design.md` (4절 기한, 11절 L2). L1 계획과 노트: `docs/plans/2026-10-18-concurrency-v2-l1-scopes.md`, `docs/notes/R7-language.md`.

## Global Constraints

- 사용자 방침(메모리 `plan-execution-style`)을 따른다.
  - 작업마다 TDD로 하고, 그 작업의 테스트만 돌린다. 작업 단위로 커밋하되 CLAUDE.md의 전체 커밋 전 검사는 하지 않는다.
  - 모든 작업이 끝난 뒤 전체 검사(`cargo fmt --check`, clippy와 `cargo test --locked --no-fail-fast`를 debug·release 둘 다)와 브랜치 전체 검토를 한 번 하고, 그때 고친다.
- 커밋은 Conventional Commits, 영어, 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- 동시성 테스트는 `sleep`이 아니라 상태로 순서를 정한다(`testutil::exec_waiting`의 `until`·`blocked`·`queued`·`hold`). 시간을 재는 단언은 "걸린 시간 < 2초" 같은 넉넉한 상한만 둔다.
- 새 `unsafe`는 만들지 않는다. 다른 스레드에서 만지는 것(`Watch` 대기, 표준 입력 대기)은 `Arc<CancelNode>`뿐이다. 노드는 이미 `Send + Sync`다.
- `timeout`의 겉모습:
  - 매개변수는 `timeout(d, f)`다. `d`는 `dur`가 아니면 E0403(`d`, `dur`), `f`는 함수가 아니면 E0403.
  - 결과는 `f()`의 값이거나 `Err({kind: 'timeout})`(`err_kind("timeout")`)다.
- **`d <= 0`이면 노드가 처음부터 만료돼 있다**(Trio의 `move_on_after(0)`와 같음). `f`의 첫 호출 지점에서 되감기므로 결과는 언제나 `Err({kind: 'timeout})`다. "기다리지 않고 한 번 보기"는 이 단계에서 없다. 아래 Task 3의 옮겨 쓰기 규칙을 본다.
- 일반 `outside`로 하는 기다림은 기한에 끊기지 않는다(L2의 알려진 한계). fs, 외부 프로세스, regex 컴파일이 해당하고, 끝날 때까지 기한이 밀린다. 외부 프로세스 죽이기는 L6, 버려두기는 R단계 스레드 풀에서 한다(스펙 4절·11절). 05와 매뉴얼에 적는다.
- HTTP 클라이언트의 `timeout = 30s`는 남긴다(스펙 4절). 안에서 `timeout` 범위로 구현하므로 실제 기한은 바깥 기한과의 이른 쪽이다. `0s`는 지금처럼 "제한 없음"이다(06의 HTTP 약속, 바꾸지 않음). 시간 초과 에러 문구 `request to `<url>` timed out`도 그대로다.
- 명세·기대 출력 갱신 방법, 사례 수 상수 갱신은 L1과 같다(`FUNG_UPDATE_SPEC=1 cargo test spec_file`, `FUNG_EXPECTED=bless …`, diff 검토).

## Review Focus

- **범위 안 손자 태스크의 기한.** `timeout(…)` 안에서 연 `scope`의 자식이 그 아래에서 또 막힌다. 그 자식이 기한에 깨어 주인을 만료시켜야 하고, 본문은 범위의 끊기지 않는 기다림(새 루트 노드, 기한 없음) 안에 있다. → Task 1의 `a_grandchild_in_a_scope_wakes_at_the_deadline`.
- **교착 오판.** 태스크 하나가 `timeout(1s, () => recv(channel()))`에서 막히면 E0702가 아니라 약 1초 뒤 `Err({kind: 'timeout})`여야 한다. 기한 밖에서 아무도 깨울 수 없는 기다림은 여전히 E0702다. → Task 1의 `a_wait_under_a_deadline_is_not_a_deadlock`, `a_wait_outside_any_deadline_is_still_a_deadlock`.
- **기다리지 않는 계산.** `timeout(50ms, () => spin(10^9))`는 계산 중에도 끊겨야 한다. → Task 1의 `a_computation_is_cut_at_the_deadline`.
- **겹친 기한.** 바깥이 먼저면 바깥 `timeout`이 `Err`를 주고 안쪽은 그대로 되감긴다. 안쪽이 먼저면 안쪽이 `Err`를 주고 바깥은 이어 간다. → Task 1의 `nested_timeouts_answer_at_the_right_level`.
- **HTTP.** 응답하지 않는 서버에 `http.get(url, timeout: 200ms)`는 약 200ms에 `request to … timed out`다. 바깥 `timeout(100ms, …)`은 약 100ms에 바깥의 `Err({kind: 'timeout})`다. → Task 4.

---

### Task 1: 노드의 기한과 `timeout(d, f)`

**Files:**
- Create: `crates/fung-vm/src/vm/deadline.rs`(`Deadline`과 `run_timeout`), `crates/fung-vm/src/vm/deadline_tests.rs`, `tests/lang/timeouts.fung`
- Modify:
  - `crates/fung-vm/src/vm/task.rs`(`CancelNode`에 `deadline`·`expired`, `child()`에서 물려받기, `with_deadline`·`expire`·`expire_due`·`deadline_at`)
  - `crates/fung-vm/src/vm/block.rs`(`block_until`이 노드 기한까지만 자고 집계에서 timed로 셈, `tick`의 `turn`에서 `expire_due`)
  - `crates/fung-vm/src/conc.rs`(네이티브 `timeout`)
  - `crates/fung-vm/src/vm.rs`(모듈 등록)
  - `crates/fung-vm/src/prelude/tests.rs`(`CONCURRENCY`에 `"timeout"`), `spec/builtins.txt`

**Interfaces:**
- Produces:
  - `pub(crate) struct Deadline { pub(crate) at: Instant, owner: Weak<CancelNode> }`(Clone).
  - `CancelNode`:
    - `pub(crate) fn with_deadline(self: &Arc<Self>, at: Option<Instant>) -> Arc<CancelNode>`: 자식 노드. 기한은 부모의 것과 `at`(주인은 새 노드 자신, `Arc::new_cyclic`) 중 이른 쪽. `at`이 `None`이면 부모의 것만 따른다. `at`이 이미 지났으면 만들자마자 만료된다.
    - `pub(crate) fn deadline_at(&self) -> Option<Instant>`
    - `pub(crate) fn expire_due(&self, now: Instant) -> bool`: 기한이 지났으면 주인을 `expire()`하고 true. 주인이 사라졌으면 false.
    - `pub(crate) fn expired(&self) -> bool`: 이 노드가 자기 기한으로 만료됐는지
    - 기존 `child()`는 부모의 `Deadline`을 그대로 복사한다.
  - `pub(crate) fn run_timeout(t: &mut Task<'_>, d: i64, f: Value) -> Result<Value, Stop>`
  - 네이티브 `timeout(d, f)`
- 결과 규칙(`run_timeout`): `node = t.cancel.with_deadline(now + d)`. `d <= 0`이면 지금, `now + d`가 넘치면 `None`. 본문 `r = f()`를 그 노드 아래에서 부른 뒤 다음 순서로 정한다.
  1. `Ok(v)` → `Ok(v)`. 기한 직후에 끝났어도 되감기지 않았으면 값이 이긴다.
  2. `Exit`이나 취소가 아닌 에러 → 그대로.
  3. 취소 → 연 태스크의 노드가 취소됐으면 그대로(바깥 기한·바깥 취소). 아니고 `node.expired()`면 `Ok(err_kind("timeout"))`. 아니면 그대로.
  4. 끝으로 `node.cancel()`(부모 목록에서 빠짐. `expired`는 건드리지 않음).

- [ ] **Step 1: 실패하는 테스트를 쓴다** (`deadline_tests.rs`, `testutil::{run, exec_waiting}`)
  - `timeout_gives_the_value`: `print(timeout(1s, () => 1 + 1))` → `"2\n"`, 코드 0
  - `a_wait_under_a_deadline_is_not_a_deadlock`: `print(timeout(50ms, () => recv(channel())))` → `"Err({kind: 'timeout})\n"`, 코드 0, 2초 미만
  - `a_wait_outside_any_deadline_is_still_a_deadlock`: `timeout(10s, () => 1)\nrecv(channel())` → E0702, 코드 1, 2초 미만
  - `a_computation_is_cut_at_the_deadline`: `fn spin(n) = n == 0 then 0 else spin(n - 1)\nprint(timeout(50ms, () => spin(1000000000)))` → `Err({kind: 'timeout})`, 2초 미만
  - `sleep_is_cut_at_the_deadline`: `print(timeout(20ms, () => sleep(10s)))` → timeout, 2초 미만
  - `a_zero_timeout_always_times_out`: `print(timeout(0s, () => 1), timeout(-1s, () => 1))` → `"Err({kind: 'timeout}) Err({kind: 'timeout})\n"`
  - `nested_timeouts_answer_at_the_right_level`:
    - `print(timeout(1s, () => timeout(20ms, () => recv(channel()))))` → `Err({kind: 'timeout})`(안쪽이 답하고 바깥은 그 값을 돌려줌)
    - `print(timeout(20ms, fn(): r = timeout(10s, () => recv(channel())); print("not here") ;;))` → `"Err({kind: 'timeout})\n"`만(바깥이 답함, 안쪽 다음 줄은 돌지 않음)
  - `a_grandchild_in_a_scope_wakes_at_the_deadline`: `inner = channel()\nprint(timeout(50ms, () => scope(s => spawn(s, () => recv(inner)))), queued(inner))` → `"Err({kind: 'timeout}) 0\n"`, 2초 미만(`exec_waiting`)
  - `errors_and_exit_pass_through`: `timeout(1s, () => 1 / 0)` → E0502, 코드 1. `timeout(1s, () => exit(4))` → 코드 4.
  - `an_outer_cancel_is_not_a_timeout`: `r = scope(fn(s): t = spawn(s, () => timeout(10s, () => recv(channel()))); cancel(t); await(t) ;;)\nprint(r)` → `"Err({kind: 'cancelled})\n"`
  - `timeout_checks_its_arguments`: `timeout(1, () => 1)` → stderr에 ``argument `d` must be dur``. `timeout(1s, 2)` → ``argument `f` must be fn``.
  - golden `tests/lang/timeouts.fung`(앞의 셋 요약)
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm deadline_tests` → `timeout`이 없어 FAIL
- [ ] **Step 3: 구현한다.**
  - `block_until`: 실제 마감은 `min(deadline, t.cancel.deadline_at())`이다. `timed`는 둘 중 하나라도 있으면 참이다. 마감에 깼을 때 노드 기한 쪽이면 `t.cancel.expire_due(now)` 뒤 `Err(t.unwound())`, 아니면 지금처럼 `Ok(None)`.
  - `turn()`: `self.cancel.deadline_at()`이 있을 때만 `Instant::now()`를 보고 `expire_due`를 부른다(다음 `tick`이 취소를 본다).
  - `sleep(d)`는 바꾸지 않아도 위 규칙으로 끊긴다.
  - `FUNG_UPDATE_SPEC=1 cargo test spec_file`
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm deadline_tests`, `cargo test -p fung-vm scope_tests`, `cargo test -p fung-cli --test golden` → PASS
- [ ] **Step 5: 커밋한다.** `feat(vm): deadlines on cancellation nodes and timeout(d, f)`

### Task 2: 운영체제 기다림도 기한을 지킨다 (소켓, 표준 입력)

**Files:**
- Modify: `crates/fung-vm/src/vm/watch.rs`(`Watch::wait`), `crates/fung-vm/src/vm/stdin.rs`(`Reader::wait`), `crates/fung-vm/src/vm/deadline_tests.rs`, `crates/fung-vm/src/native/net_tests.rs`

**Interfaces:**
- Consumes: Task 1의 `deadline_at`·`expire_due`
- Produces: 동작만 바뀐다. 시그니처는 그대로다.
  - `Watch::wait(fds, deadline)`는 `min(deadline, node.deadline_at())`까지 `poll`한다. 노드 기한으로 끝나면 `node.expire_due(now)` 뒤 `Waited::Cancelled`다(호출한 쪽은 지금처럼 E0701로 되감는다).
  - `Reader::wait(me, cancel)`은 `cancel.deadline_at()`이 있으면 `park_timeout`으로 자고, 지나면 `expire_due` 뒤 `None`이다.

- [ ] **Step 1: 실패하는 테스트를 쓴다**
  - `net_tests::a_read_is_cut_at_the_deadline`: `net_tests`의 실행 도우미(`import _native.net as net` 머리를 붙이는 쪽)로 다음을 돌린다. 받아 주지 않는 리스너라도 연결은 백로그로 성립하고, 읽기는 막힌다.
    ```
    l = net.listen("127.0.0.1:0")!
    p = parse_int(split(net.local_addr(l), ":")[1])!
    c = net.connect("127.0.0.1", p)!
    print(timeout(100ms, () => net.read_line(c, 100)))
    ```
    → `"Err({kind: 'timeout})\n"`, 2초 미만, 끝난 뒤 남은 깨우개 파이프 0(`live_wakes`, `serial()` 아래에서).
  - `deadline_tests::a_stdin_read_is_cut_at_the_deadline`: 닫히지 않는 표준 입력(`exec_with_stdin`에 영원히 막히는 `BufRead`)으로 `print(timeout(50ms, () => read_line()))` → `Err({kind: 'timeout})`, 2초 미만, `stdin_waiting` 0
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm a_read_is_cut_at_the_deadline a_stdin_read_is_cut` → 2초 넘게 막히다가 시험 시간 제한(`within`)으로 FAIL
- [ ] **Step 3: 구현한다.** 위 Produces대로 한다.
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm deadline_tests net_tests` → PASS
- [ ] **Step 5: 커밋한다.** `feat(vm): socket and stdin waits keep the deadline`

### Task 3: `await`·`recv`의 `timeout:`을 지운다

**Files:**
- Modify:
  - `crates/fung-vm/src/conc.rs`: `await`는 `[param("t")]`, `recv`는 `[param("ch")]`. `timeout_arg`를 지운다.
  - `crates/fung-vm/src/vm/channel.rs`: `recv`의 `timeout` 인자와 `Received::Timeout`, 받는 쪽 자리의 마감을 지운다. 기한은 Task 1의 노드 기한이 맡는다.
  - `extras/http.fung` `serve`: `recv(failed, timeout: 0s)` 대신, 받기 태스크(`_acceptor`)가 끝날 때 언제나 `close(failed)`를 하고 `serve`는 `recv(failed)`에서 `Ok(e)`면 `raise`, `Err(_)`면 `nil`로 한다.
  - 옮겨 쓰기: `git grep -nE "(await|recv)\([^)]*timeout:"`이 찾는 모든 곳(약 32곳, `.fung`·`.rs`·`.html`)
  - `spec/builtins.txt`, 기대 출력
- Test: 기존 테스트 전부(옮겨 쓴 뒤), `deadline_tests.rs`

**Interfaces:**
- Consumes: `timeout(d, f)`
- Produces: `await(t)`, `recv(ch)`만 있다. `await(t, timeout: 1s)`는 E0405(알 수 없는 이름 인자) 계열의 기존 에러다.
- 옮겨 쓰기 규칙:
  - `recv(X, timeout: D)` → `timeout(D, () => recv(X))`, `await(X, timeout: D)` → `timeout(D, () => await(X))`. 결과 값의 모양이 같다(`Err({kind: 'timeout})`).
  - **`D`가 `0s`이고 값이 이미 있기를 기대하는 곳**(`recv(ch, timeout: 0s)`로 "이미 보냈는지" 단언)은 `recv(X)`로 바꾼다. 값이 없으면 교착 보고로 실패하므로 단언의 뜻이 유지된다.
  - `D`가 `0s`이고 `Err timeout`을 기대하는 곳은 `timeout(0s, …)`로 둔다(언제나 timeout).

- [ ] **Step 1: 실패하는 테스트를 쓴다.** `deadline_tests::the_timeout_parameters_are_gone`: `recv(channel(), timeout: 1s)`와 `await(scope(s => spawn(s, () => 1)), timeout: 1s)`가 둘 다 이름 인자 에러다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm the_timeout_parameters_are_gone` → FAIL(지금은 받음)
- [ ] **Step 3: 구현하고 옮겨 쓴다.** 묶음마다 그 묶음의 테스트를 돌리고, bless와 diff 검토를 한다.
- [ ] **Step 4: 확인한다.** `git grep -nE "(await|recv)\([^)]*timeout:"` → 없음. Run: `cargo test -p fung-vm`, `cargo test -p fung-cli --test golden --test run_expected` → PASS
- [ ] **Step 5: 커밋한다.** `feat!: await and recv lose their timeout parameter`

### Task 4: HTTP 클라이언트를 기한 범위 위에

**Files:**
- Modify:
  - `extras/http.fung`
    - 모듈 맨 위에 `_within = timeout`을 둔다. 매개변수 `timeout`이 prelude의 `timeout`을 가리므로 별칭을 쓴다.
    - `_request`는 `timeout > 0s`면 `_within(timeout, () => _attempt(…))`, 아니면 `_attempt(…)`를 부른다. 범위가 준 `Err({kind: 'timeout})`(`message`가 없는 맵)은 `Err({kind: 'timeout, message: "request to `${url}` timed out"})`으로 바꾼다.
    - `start`·`_left`·`_timed_out`·`_failed`와, `_net.connect`·`_net.start_tls`에 넘기던 `timeout:`, 클라이언트 쪽 `_net.set_deadline(c, left)` 호출을 지운다. 서버 쪽 `set_deadline`(머리 읽기 한도 등)은 그대로 둔다.
  - `crates/fung-vm/src/native/net.rs`: `connect(host, port, tls = false)`, `start_tls(c, host)`. `timeout` 매개변수와 그 마감 처리를 지운다. 이름 조회와 연결은 노드 기한(Task 2)으로 끊긴다.
  - `crates/fung-vm/src/native/net_tests.rs`, `tls_tests.rs`: `connect`·`start_tls`의 `timeout:` 사례를 `timeout(…)` 범위로 옮긴다.
  - `crates/fung-cli/tests/http.rs`, `spec/builtins.txt`, 기대 출력(`http/client.txt`는 출력이 바뀌지 않아야 한다)

**Interfaces:**
- Consumes: `timeout(d, f)`, Task 2의 소켓 기한
- Produces: `http.get/post/request`의 겉모습은 그대로다(`timeout = 30s`, `0s`는 제한 없음, 같은 에러 문구).

- [ ] **Step 1: 실패하는 테스트를 쓴다** (`crates/fung-cli/tests/http.rs`, 기존의 응답하지 않는 서버 도우미 사용)
  - `an_outer_timeout_is_shorter_than_the_requests`: `print(timeout(100ms, () => http.get(url, timeout: 10s)))` → `Err({kind: 'timeout})`, 2초 미만
  - `a_request_timeout_still_reads_as_before`: `http.get(url, timeout: 200ms)` → `Err({kind: 'timeout, message: "request to `…` timed out"})`, 2초 미만
  - `a_zero_request_timeout_is_no_limit`: 300ms 뒤 답하는 서버에 `timeout: 0s` → 응답을 받는다
- [ ] **Step 2: 지금 동작을 확인한다.** 이 작업은 리팩터링이다. Task 2 뒤라 세 사례는 고치기 전에도 통과할 수 있다(소켓 기다림이 이미 노드 기한을 지킴). Run: `cargo test -p fung-cli --test http an_outer_timeout a_request_timeout_still a_zero_request_timeout` → 결과를 ledger에 적는다. 통과하면 리팩터링의 회귀 방지 시험이다. 실패해야 하는 시험은 `net_tests::connect_has_no_timeout_parameter`(`net.connect("127.0.0.1", 1, timeout: 1s)`가 이름 인자 에러)다. 이것이 FAIL하는 것을 확인한다.
- [ ] **Step 3: 구현한다.** Files대로 한다.
- [ ] **Step 4: 통과를 확인한다.** Run: `FUNG_UPDATE_SPEC=1 cargo test spec_file`, `cargo test -p fung-cli --test http`, `cargo test -p fung-vm net_tests tls_tests` → PASS, `http/client.txt` diff 없음
- [ ] **Step 5: 커밋한다.** `refactor(http): the client deadline is a timeout scope`

### Task 5: 문서

**Files:**
- Modify:
  - `docs/design/05-concurrency.md`
    - 원시 연산 표: `await(t)`, `recv(ch)`, 새 행 `timeout(d, f)`
    - "타임아웃·취소" 절: 기한 범위, 노드 기한, `0s`의 뜻, 알려진 한계(일반 `outside`)
    - 기간 리터럴 예제의 `await(t, timeout: 30s)`
  - `docs/design/06-native-and-stdlib.md`: HTTP `timeout`이 바깥 기한과 겹치는 규칙, `_native.net` 시그니처
  - `docs/design/open-questions.md`: L2 완료, `try_recv` 같은 "기다리지 않고 보기"는 보류 항목으로
  - 매뉴얼
    - `docs/builtins.html`: `timeout` 항목, `await`·`recv` 시그니처와 예제, 함수 수
    - `docs/internals.html`: 노드 기한
    - `docs/syntax.html`의 `timeout:` 예제(있으면)
  - `CHANGELOG.md` `[Unreleased]`: 호환을 깨는 변경과 옮겨 쓰기(`recv(ch, timeout: d)` → `timeout(d, () => recv(ch))`), 더함(`timeout`)
  - `docs/notes/R7-language.md`: L2 절. 다룰 것:
    - 연산마다의 기한이 합쳐지지 않는 이유
    - 노드에 기한을 실은 설계와 타이머 스레드를 두지 않은 이유
    - 교착 집계와의 관계
    - `0s`(Trio) 규칙
    - 알려진 한계
- Test: `cargo test -p fung-cli --test manual --test changelog --test repo_rust_only`

- [ ] **Step 1: 고친다.** 매뉴얼 예제는 `fung check`·`fmt --check`·실행을 통과해야 한다.
- [ ] **Step 2: 확인한다.** 위 Test 명령 → PASS
- [ ] **Step 3: 커밋한다.** `docs: timeout scopes in the design, the manual and the changelog`
