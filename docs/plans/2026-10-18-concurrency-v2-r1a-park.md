# 동시성 v2 R1a (주차 요청과 offload) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 기다리는 모든 내장 함수가 스레드를 직접 재우지 않게 한다.
- 다른 태스크를 기다리는 원시 연산은 VM에 "주차해 달라"를 돌려준다.
- 운영체제를 기다리는 것은 `offload(일, 마무리)`로 넘긴다.

이번에는 지금 런타임(태스크마다 스레드) 위에서 한다. 주차 요청을 받은 실행 루프는 지금처럼 스레드를 재우고, offload는 지금처럼 실행권 밖에서 일을 돌린다. 의미는 바뀌지 않는다. R1b는 이 API 아래의 런타임만 바꾼다.

**Architecture:**
- **주차는 이어 가기의 한 갈래다**(R2의 `cont.rs`). 단계 내장 함수와 이어 가기는 지금의 `Done`·`Call`에 더해 `Park(Wake)`를 돌려줄 수 있다.
  - 주차한 상자는 줄 맨 위에 그대로 있다. 깨어나면 VM이 `resume(Value::Nil)`을 부르고, 상자가 조건을 다시 본다. 지금 `block`의 `ready`를 다시 부르는 것과 같다.
  - 취소·기한은 상자의 `unwind`로 간다. 그래서 "값이 이미 건너왔으면 취소보다 값"(채널의 `slot.done`) 같은 규칙을 상자가 정한다.
- **주차 요청은 실행 루프(`drive`)까지 올라간다.** 루프는 지금 런타임에서는 `wait_parked`로 스레드를 재운다. 이것은 `block_until`의 반복을 `ready` 없이 떼어 낸 것이다. R1b에서는 이 한 갈래만 "스케줄러로 돌아간다"로 바뀐다.
- **offload**:
  - 내장 함수가 fung 값을 평범한 데이터로 바꿔 `Send + 'static` 일을 만든다. 일이 끝나면 마무리 함수가 결과를 fung 값으로 바꾼다.
  - 지금 런타임에서는 `outside`로 곧바로 돌리고 `Flow::Done`을 준다. R1b에서는 일을 다른 스레드에 맡기고 주차한다(R3에서는 풀).
  - 일이 `'static`이어야 하므로, fung 값에서 빌린 것을 쓰지 않고 필요한 것을 복제해 가진다.

**Tech Stack:** Rust 표준 라이브러리, `fung-vm`(`vm/cont.rs`·`call.rs`·`vm.rs`·`block.rs`·`channel.rs`·`scope.rs`, `conc.rs`, `native/walk.rs`·`nat.rs`·`fs.rs`·`process.rs`·`regex.rs`·`net.rs`·`file.rs`, `vm/watch.rs`, `prelude/io.rs`·`collections.rs`·`strings.rs`).

**Spec:** `docs/superpowers/specs/2026-10-18-concurrency-v2-r7-design.md` (10.1, 10.2, 15절, 특히 "R1을 둘로"). 앞 단계 계획 `docs/plans/2026-10-18-concurrency-v2-r2-continuations.md`, 노트 `docs/notes/R7-scheduler.md`.

## Global Constraints

- 사용자 방침(메모리 `plan-execution-style`)을 따른다.
  - 작업마다 TDD로 하고 그 작업의 시험만 돌린다. 작업 단위로 커밋한다.
  - 끝에 한 번만 한다: 전체 검사(fmt, clippy, `cargo test --locked --no-fail-fast`를 debug·release 둘 다), 가장 능력 있는 모델의 브랜치 전체 검토, 수정.
- 사용자와의 대화·보고는 한국어. 코드·주석·커밋은 영어. 커밋 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **언어 의미를 바꾸지 않는다.** 골든, 기대 출력, 단위 시험이 정답지다. 기대 출력을 bless하지 않는다. 필요해 보이면 멈추고 묻는다.
- **성능**: `bench/`의 계산 위주(`fib`·`loop`·`map`·`str`·`list`)는 이 브랜치 시작점(`edcc15b`)보다 5% 넘게 느려지지 않는다. `chan`·`spawn`은 기록만 한다(R1b가 목표를 진다).
- 새 크레이트도 새 `unsafe`도 없다.
- 동시성 시험은 상태로 순서를 정한다(`testutil::exec_waiting`).
- **기계적 변환은 서브에이전트가 병렬로 한다**(스펙 15절). Task 5·6·7은 파일이 겹치지 않는다. 각 에이전트에게 다음을 준다.
  - 이 계획의 해당 작업과 Task 4의 Interfaces 블록
  - "시험·커밋은 그 작업의 파일만" 규칙
  - 이 워크트리 경로. 모두 같은 워크트리에서 서로 다른 파일만 고치고, 커밋은 하지 않는다.

  나는 결과 diff를 검토하고, 작업 시험을 돌리고, 작업마다 커밋한다.

## Review Focus

- **값과 취소가 겹칠 때.** 받는 쪽이 주차한 동안 보내는 쪽이 값을 넘기고, 그 직후 받는 쪽이 취소된다. 이때 받는 쪽은 값을 받고 끝나야 한다(지금 `recv`의 `Err(_) if slot.done`). 보내는 쪽도 같다. → Task 2의 `a_value_handed_over_before_a_cancel_is_kept`
- **기다리는 순회 중의 취소.** `for x in rx`로 기다리는 중에 취소되어도 이미 받은 값은 처리된 뒤 되감기고, 채널에 남은 값은 그대로다. → Task 2의 `a_cancelled_walk_leaves_the_rest_in_the_channel`
- **범위가 자식을 기다리는 동안의 바깥 취소.** 기다림은 끊기지 않고(새 루트 노드), 자식이 되감은 뒤에 범위가 취소를 이어 간다. 기다리는 동안 바뀐 `t.cancel`이 모든 길(정상, 패닉)에서 돌아온다. → Task 3의 `an_outer_cancel_waits_for_the_children_then_unwinds`
- **offload 마무리의 에러.** 마무리가 내는 에러(E0403·E0515 등)는 지금처럼 내장 함수의 호출 위치에 붙는다. → Task 4의 `offload_errors_point_at_the_call`, Task 5~7의 기존 시험
- **교착 판정.** 주차한 태스크만 남으면 E0702다. 타이머가 깨울 주차(`sleep`, 기한 안의 기다림)나 정지 채널의 주차는 교착이 아니다. → Task 1의 `parked_tasks_are_judged_like_blocked_ones`, 기존 `deadlock` 시험

---

### Task 1: 주차 요청과 `sleep`·`await`

**Files:**
- Modify:
  - `crates/fung-vm/src/vm/cont.rs`(`Wake`, `Next::Park`, `Flow::Park`, `Cont::resume`의 패닉 규칙 문서)
  - `crates/fung-vm/src/vm/call.rs`(`Entered::Park`, `Delivered::Park`, `issue`·`finish`·`unwind`의 반환형 `Ran`)
  - `crates/fung-vm/src/vm.rs`(`step_loop`, `drive`의 주차 갈래, 시험용 `parks` 셈)
  - `crates/fung-vm/src/vm/block.rs`(`wait_parked`, `park_until`)
  - `crates/fung-vm/src/conc.rs`(`sleep`·`await`를 단계 내장 함수로)
  - `crates/fung-vm/src/testutil.rs`(탐침 `parks()`)
- Test: `crates/fung-vm/src/vm/park_tests.rs`(새 파일)

**Interfaces:**
- Produces:
  - `cont.rs`
    - `pub struct Wake { pub until: Option<Instant>, pub timed: bool }`
      - `until`: 상자 자신의 마감(`sleep`). 노드의 기한은 런타임이 더한다.
      - `timed`: 타이머나 바깥이 깨울 기다림이라 교착이 아님(지금 `block_until`의 `timed`).
    - `Next::Park(Wake)`, `Flow::Park(Box<dyn Cont>, Wake)`
    - 깨어난 상자의 `resume`은 `Value::Nil`을 받는다. 문서에 적는다: "주차 뒤의 `resume`은 조건을 다시 보고 `Done`·`Call`·`Park` 중 하나를 준다"
  - `call.rs`
    - `pub(super) enum Ran { Going, Done(Value), Parked(Wake) }`
      - `Going`: 루프를 계속 돈다(지금의 `None`)
      - `Done`: 루프의 값(지금의 `Some`)
    - `issue`·`finish`·`unwind`가 `Result<Ran, Stop>`을 준다.
    - `Entered::Park(Wake)`: `enter`가 `Flow::Park`의 상자를 놓은 뒤 준다.
    - `Delivered::Park(Wake)`
  - `vm.rs`
    - `step_loop`는 `Result<Ran, Stop>`을 주고, `Going`은 주지 않는다.
    - `drive`의 `Todo`에 `Resume`을 더한다: `self.finish(Value::Nil)`. 맨 위 상자가 주차한 상자다.
    - 시험용 `#[cfg(test)] pub(crate) parks: u32`. `drive`가 주차를 받을 때마다 하나 늘린다.
  - `block.rs`
    - `pub(super) fn wait_parked(&mut self, w: Wake) -> Result<(), Stop>`(`Task`의 메서드): `block_until`의 반복에서 `ready`를 뺀 것이다.
      - 노드 기한을 `until`에 합친다.
      - `self.cancel.watch(&me)` 다음, 취소·기한·마감을 본 뒤 센다(`count`·`judge`).
      - 잔 뒤 깨면 `me.uncount()`, `me.take_woken()`, `watch`를 하고 `Ok(())`. 취소면 `Err(self.unwound())`.
      - 자기 마감이 지났으면 자지 않고 `Ok(())`. 상자가 시간을 본다.
      - 지금 `block_until`의 순서와 주석의 "잃어버린 깨움 없음" 논증을 그대로 옮긴다.
    - `pub(crate) fn park_until(t: &mut Task<'_>, poll: impl FnMut(&mut Task<'_>, &Arc<Waiter>) -> Option<Result<Value, Stop>> + 'static, wake: Wake) -> Result<Flow, Stop>`
      - 조건을 닫개(poll)로 보는 가장 흔한 기다림이다.
      - 처음에 `take_woken` 다음 `poll`. 준비됐으면 `Flow::Done`, 아니면 `Flow::Park(상자, wake)`.
      - 상자의 `resume`은 `take_woken`(이미 `wait_parked`가 했으므로 생략 가능, 둘 다 해도 무해) 다음 `poll`이고, 아니면 `Next::Park(wake)`.
  - 탐침 `parks()`: 이 태스크가 주차한 횟수(`Value::Int`)

- [ ] **Step 1: 실패하는 시험을 쓴다** (`park_tests.rs`, `testutil::exec_waiting`에 탐침)
  - `sleep_parks`: `sleep(1ms)\nprint(parks() >= 1)` → `"true\n"`
  - `await_parks`: `print(scope(fn(s): t = spawn(s, () => sleep(5ms)); await(t); parks() >= 1 ;;))` → `"true\n"`
  - `a_finished_task_is_awaited_without_parking`: 자식이 끝난 뒤(`until(() => finished(t))`) `await` 전후의 `parks()`가 같다.
  - `a_cancelled_sleep_unwinds`: `scope(fn(s): t = spawn(s, () => sleep(10s)); cancel(t); print(await(t)) ;;)` → `"Err({kind: 'cancelled})\n"`, 1초 안에 끝남. 자식이 잠들기 전에 취소돼도 결과가 같으므로 순서를 맞출 필요가 없다.
  - `parked_tasks_are_judged_like_blocked_ones`: `recv` 없이 `await`만으로 서로를 기다리는 두 태스크 → E0702. `sleep(20ms)` 하나만 도는 프로그램은 교착이 아님
  - 기존 `deadline_tests`(기한 안의 `sleep`), `task_tests`가 그대로 통과
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- park_tests` → `parks`가 없어 컴파일 실패
- [ ] **Step 3: 구현한다.**
  - `sleep`은 `park_until`로 한다. `until = now + d`이고, 닫개는 `now >= until`이면 `Ok(Nil)`이다. `d <= 0`은 지금처럼 곧바로 `Nil`.
  - `await`의 닫개는 지금 `ready`와 같다.
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- park_tests task_tests deadline_tests exit_tests` → PASS
- [ ] **Step 5: 커밋한다.** `feat(vm): park requests through continuations; sleep and await park`

### Task 2: 채널

**Files:**
- Modify:
  - `crates/fung-vm/src/vm/channel.rs`(`send`·`recv`를 기다림 상태로)
  - `crates/fung-vm/src/conc.rs`(`send`·`recv` 내장 함수를 단계 내장 함수로)
  - `crates/fung-vm/src/native/walk.rs`(`Cursor::Receiver`가 주차, `Drain` 상자)
  - `crates/fung-vm/src/native/nat.rs`(`items`·`each`·`each_until`이 받는 끝을 직접 받지 않게)
  - `crates/fung-vm/src/prelude/collections.rs`(`sort`·`sort_by`·`flatten`), `crates/fung-vm/src/prelude/strings.rs`(`join`)
- Test: `crates/fung-vm/src/vm/park_tests.rs`, 기존 `channel_tests.rs`

**Interfaces:**
- Consumes: Task 1의 `Wake`·`Next::Park`·`Flow::Park`·`park_until`
- Produces:
  - `channel.rs`
    - `pub(crate) enum Try<T, W> { Ready(T), Wait(W) }`
    - `pub(crate) struct RecvWait { ch: Rc<Channel>, slot: Rc<Slot> }`, `pub(crate) struct SendWait { ch: Rc<Channel>, slot: Rc<Slot> }`
      - `RecvWait::poll(&self) -> Option<Received>`, `SendWait::poll(&self) -> Option<bool>`: 지금 `block_until`에 준 닫개와 같다.
        - 칸이 채워졌으면 그 결과다.
        - 닫혔으면 받는 쪽은 `leave` 다음 버퍼를 다시 보고, 보내는 쪽은 `leave` 다음 `false`다.
        - 아니면 `None`이다.
      - `RecvWait::cancel(self, e: Stop) -> Result<Received, Stop>`, `SendWait::cancel(self, e: Stop) -> Result<bool, Stop>`: 지금 `Err(_) if slot.done`의 규칙이다. 값이 이미 건너왔으면 그 결과, 아니면 `ch.leave(&slot)` 다음 `Err(e)`.
      - `RecvWait::wake(&self) -> Wake`는 `timed = ch.external`, `SendWait::wake`는 `timed = false`.
    - `pub(crate) fn try_recv(t: &mut Task<'_>, ch: &Rc<Channel>) -> Result<Try<Received, RecvWait>, Stop>`: E0707(`pure > 0`) 검사, 버퍼, 닫힘, 아니면 칸을 줄에 세우고 `Wait`.
    - `pub(crate) fn try_send(t: &mut Task<'_>, ch: &Rc<Channel>, v: Value) -> Result<Try<bool, SendWait>, Stop>`
    - 지금의 `recv`·`send`·`drain`(스레드를 재우는 것)은 지운다.
  - `conc.rs`의 `send`·`recv`: `Try::Wait`이면 `Flow::Park(Box<ChanWait>, pending.wake())`. 상자의 `resume`은 `poll`이고, `unwind`는 `cancel`이다.
  - `walk.rs`
    - `Cursor::next`는 `Result<Step, Stop>`을 준다. `enum Step { Item(Value), End, Wait(Wake) }`이고, 받는 끝이 기다리면 `RecvWait`을 커서 안에 둔다(`Cursor::Receiver { rx, wait: Option<RecvWait> }`).
    - `Walker::advance`가 `Wait`를 받으면 `Next::Park`를 준다. `resume(Nil)`은 커서의 `RecvWait`을 다시 본다.
    - `Walker`의 `unwind`는 커서에 `RecvWait`이 있으면 `cancel`한다. 값을 받았으면 그 원소로 `Next::Call`을 이어 가고, 다음 차례 확인이 다시 되감는다(지금의 순서).
    - `pub(crate) fn drain_with<P: Collect + 'static>(t: &mut Task<'_>, cur: Cursor, p: P) -> Result<Flow, Stop>`: 콜백이 없는 순회(`join`·`flatten`)다.
      - `trait Collect { fn item(&mut self, x: Value) -> Result<bool, Stop>; fn result(&mut self) -> Result<Value, Stop>; }`
      - 원소마다 Rust에서 처리하고, 받는 끝이면 주차한다. 첫 원소에서 실패하면 지금처럼 남은 값은 채널에 남는다.
    - `sort`·`sort_by`는 지금처럼 다 받은 뒤 정렬한다. 받는 끝이면 `drain_with`로 모은 뒤 정렬한다(`sort_by`는 모은 다음 키 순회로 이어 간다).
  - `nat::items`·`nat::each`·`nat::each_until`은 받는 끝을 받지 않는다(받는 끝이 오면 내부 에러). 받는 끝을 받는 내장 함수는 모두 `walk.rs`를 지난다.

- [ ] **Step 1: 실패하는 시험을 쓴다** (`park_tests.rs`)
  - `recv_and_send_park`: 버퍼 없는 채널에서 받는 쪽이 먼저 기다리면 `parks() >= 1`, 보내는 쪽이 먼저 기다려도 그쪽 `parks() >= 1`
  - `a_walk_over_a_receiver_parks`: `fold(rx, 0, (a, x) => a + x)`를 기다리는 태스크의 `parks()`, 그리고 `join(rx, ",")`·`sort(rx)`도 같다.
  - `a_value_handed_over_before_a_cancel_is_kept`: Rust 시험이다(L1 노트 5절처럼 fung으로 순서를 만들 수 없음). 받는 쪽 상자를 주차시킨다. 보내는 쪽이 값을 넘긴 뒤 같은 차례에 받는 쪽 노드를 취소한다. `unwind`가 값을 준다. 지금 `channel_tests`에 같은 꼴의 시험이 있으면 새 API로 옮긴다.
  - `a_cancelled_walk_leaves_the_rest_in_the_channel`: 버퍼 3인 채널에 `1, 2, 3`을 넣는다. `each(rx, x => …)`가 첫 원소에서 자기 태스크를 취소한다. `buffered(rx)`가 2이고 `await`는 `Err({kind: 'cancelled})`.
  - `join_stops_at_the_first_bad_item_without_draining`: `join(rx, ",")`에서 둘째 값이 int이면, 셋째 값은 채널에 남고 에러는 지금 문구 그대로다(시작점 바이너리의 출력을 기대값으로).
  - 기존 `channel_tests`가 그대로 통과
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- park_tests` → `parks` 단언 FAIL
- [ ] **Step 3: 구현한다.**
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- park_tests channel_tests cont_tests prelude stop_tests && cargo test -p fung-cli --test golden` → PASS
- [ ] **Step 5: 커밋한다.** `refactor(vm): channel waits park instead of sleeping the thread`

### Task 3: 범위가 자식을 기다리는 것

**Files:**
- Modify: `crates/fung-vm/src/vm/scope.rs`(`Scope`의 단계: 본문 → 기다림 → 정리)
- Test: `crates/fung-vm/src/vm/park_tests.rs`, 기존 `scope_tests.rs`

**Interfaces:**
- Consumes: Task 1
- Produces:
  - `Scope`가 단계를 가진다: `Body`, `Draining { r }`.
    - `resume`·`unwind`가 본문의 결과를 받으면 지금 `close`의 앞부분(노드 되돌리기, 실패면 `node.cancel()`)을 한다.
    - 그다음 `Draining`으로 바꾸고 `t.cancel`을 새 루트 노드로 바꿔 끼운 뒤(지금 `drain`과 같음), `live == 0`이 될 때까지 `Next::Park(Wake { until: None, timed: false })`를 준다.
    - 깨어나면 `live`를 본다. 0이면 원래 노드로 되돌리고 지금 `close`의 뒷부분(`open = false`, 미처리 실패 보고, `node.cancel()`, `settle`)을 한다.
    - `Draining` 중의 `unwind`는 지금 `drain`과 같은 가정을 둔다. 그 루트 노드는 `shields`에 없으므로 교착 판정도 취소하지 않는다. 그러니 `unwind`는 `debug_assert!(false, "a scope's wait ended early")`를 하고 노드를 되돌린 뒤 stop을 그대로 올린다.
  - `drain` 함수를 지운다.

- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `a_scope_waiting_for_children_parks`: 본문이 곧 끝나고 자식이 `sleep(5ms)`하면, `scope(` 다음 줄의 `parks() >= 1`
  - `an_outer_cancel_waits_for_the_children_then_unwinds`: 바깥 `timeout(20ms, …)` 안의 범위에서 자식이 `with(…, r => sleep(50ms))`의 `release`를 돈다. `release`의 출력이 `Err({kind: 'timeout})`보다 먼저 나온다. 지금도 통과해야 하는 동작 고정이다.
  - 기존 `scope_tests`가 그대로 통과
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- park_tests::a_scope` → FAIL
- [ ] **Step 3: 구현한다.**
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- park_tests scope_tests cleanup_tests failure_tests` → PASS
- [ ] **Step 5: 커밋한다.** `refactor(vm): a scope parks while it waits for its children`

### Task 4: offload와 작은 자리들(표준 입력, 파일 줄 읽기, 정규식)

**Files:**
- Modify:
  - `crates/fung-vm/src/native/nat.rs`(`offload`, `offload_watched`)
  - `crates/fung-vm/src/vm.rs`(`Task::read_line`이 내장 함수가 쓰는 offload 꼴로)
  - `crates/fung-vm/src/prelude/io.rs`(`read_line`·`read_lines`·`lines`류)
  - `crates/fung-vm/src/native/regex.rs`(`compile_pattern`을 부르는 내장 함수)
  - `crates/fung-vm/src/testutil.rs`(탐침 `offloads()`)
- Test: `crates/fung-vm/src/vm/park_tests.rs`, 기존 `regex_tests.rs`·`stdin` 관련 시험

**Interfaces:**
- Consumes: Task 1
- Produces (Task 5·6·7의 에이전트가 쓰는 꼴):
  - `pub(crate) fn offload<R: Send + 'static>(t: &mut Task<'_>, job: impl FnOnce() -> R + Send + 'static, finish: impl FnOnce(&mut Task<'_>, R) -> Result<Value, Stop> + 'static) -> Result<Flow, Stop>`
    - R1a: `let r = outside(t, job); finish(t, r).map(Flow::Done)`. 마무리는 지금 `outside` 다음 줄들이다.
    - 마무리의 에러에 위치가 없으면 VM이 내장 함수의 호출 위치를 단다(지금과 같음).
  - `pub(crate) fn offload_watched<R: Send + 'static>(t, job: impl FnOnce(&mut Watch) -> R + Send + 'static, finish: …) -> Result<Flow, Stop>`: 지금 `outside_watched(t, job)?` 다음 `finish`.
  - `pub(crate) fn offload_then<R: Send + 'static>(t, job, finish: impl FnOnce(&mut Task<'_>, R) -> Result<Flow, Stop> + 'static) -> Result<Flow, Stop>`: 마무리가 다음 offload나 호출을 이어 갈 수 있는 꼴. R1a에서는 `outside` 다음 `finish`.
  - 시험용 셈 `offloads`(`Task`의 `#[cfg(test)]` 필드, 탐침 `offloads()`)
  - 변환 규칙(에이전트 공통)
    1. 내장 함수를 `def_step`/`native_step`으로 바꾸고 반환형을 `Result<Flow, Stop>`으로.
    2. `outside(t, || …)`의 닫개가 빌린 것(`&Conn`, `&File`, `&str`)은 소유한 것으로 바꾼다.
       - 문자열은 `to_string()`, 핸들은 그 안의 운영체제 부분을 `Arc`로 복제한다.
       - 그 타입에 `Arc`가 없으면 운영체제 부분을 `Arc`로 감싸는 것까지 그 작업에서 한다.
    3. `outside` 다음 줄들은 마무리 닫개로 옮긴다. 마무리는 fung 값을 만들어도 된다(실행권 안).
    4. 한 내장 함수에 `outside`가 둘 이상이면 하나의 일로 합친다. 합칠 수 없으면(첫 결과를 fung 값으로 본 뒤에야 둘째 일이 정해지는 경우) `offload_then`을 쓴다.
    5. 동작, 에러 문구, 순서는 그대로다.

- [ ] **Step 1: 실패하는 시험을 쓴다**
  - `reading_stdin_offloads`: 표준 입력이 `"a\n"`일 때 `read_line()` 뒤의 `offloads() >= 1`. 파일 핸들의 `read_line`·`lines`도 같다.
  - `regex_compile_offloads_once`: 같은 패턴을 두 번 쓰면 `offloads()`가 1 늘어난다(캐시).
  - `offload_errors_point_at_the_call`: 잘못된 정규식의 E0515와 `read_line(1)`의 E0403이 시작점 바이너리와 같은 위치·문구다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- park_tests` → `offloads` 단언 FAIL
- [ ] **Step 3: 구현한다.**
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- park_tests regex prelude repl && cargo test -p fung-cli --test stdin` → PASS
- [ ] **Step 5: 커밋한다.** `refactor(vm): offload for waits on the operating system; stdin, file lines and regex use it`

### Task 5: `fs.rs` offload (서브에이전트)

**Files:**
- Modify: `crates/fung-vm/src/native/fs.rs`, 필요하면 `crates/fung-vm/src/native/file.rs`(핸들의 운영체제 부분)
- Test: 기존 `crates/fung-vm/src/native/fs_tests.rs`, 새 사례는 같은 파일

**Interfaces:**
- Consumes: Task 4의 `offload`와 변환 규칙
- Produces: `fs.rs`에 `outside(`가 남지 않는다. 모든 fs 내장 함수가 단계 내장 함수다.

- [ ] **Step 1: 실패하는 시험을 쓴다** (`fs_tests.rs`): `fs_natives_offload`. 대표 다섯(`read`, `write`, `glob`, `open`+`write_line`+`close`, `with_temp_dir`)을 돌린 뒤 `offloads()`가 그 횟수만큼 늘었다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- fs_tests::fs_natives_offload` → FAIL
- [ ] **Step 3: 변환한다.**
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- fs_tests file && cargo test -p fung-cli --test file_modes --test path_abs` → PASS
- [ ] **Step 5: 커밋한다**(검토 뒤 내가). `refactor(fs): fs natives offload their system calls`

### Task 6: `process.rs` offload (서브에이전트)

**Files:**
- Modify: `crates/fung-vm/src/native/process.rs`
- Test: 기존 `process.rs`의 시험 모듈

**Interfaces:**
- Consumes: Task 4
- Produces: `exec`·`sh`가 단계 내장 함수다.
  - 10 ms 폴링과 출력 읽는 스레드는 **그대로 둔다**(R4에서 루프로).
  - 마무리의 `t.check_cancelled()?`(기한이 지난 뒤 끝난 명령의 결과가 새지 않게, L6)를 그대로 지킨다.

- [ ] **Step 1: 실패하는 시험을 쓴다**: `exec_offloads`. `exec(["true"])` 뒤 `offloads()`가 1 늘었다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- process::tests::exec_offloads` → FAIL
- [ ] **Step 3: 변환한다.**
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- process && cargo test -p fung-cli --test signals` → PASS
- [ ] **Step 5: 커밋한다**(검토 뒤 내가). `refactor(process): exec and sh offload the wait for the command`

### Task 7: `net.rs` offload (서브에이전트)

**Files:**
- Modify: `crates/fung-vm/src/native/net.rs`, 필요하면 `crates/fung-vm/src/native/sock.rs`·`tls.rs`(`Conn`·`Listener`의 운영체제 부분을 `Arc`로)
- Test: 기존 `crates/fung-vm/src/native/net_tests.rs`·`tls_tests.rs`

**Interfaces:**
- Consumes: Task 4의 `offload_watched`
- Produces: `net.rs`에 `outside(`·`outside_watched(`가 남지 않는다.
  - 일은 `Arc`로 복제한 연결을 가진다.
  - 한 연결에서 읽기와 쓰기가 동시에 오는 경우의 잠금 규칙은 지금 그대로다(`take_body`의 "읽기가 끝나기를 기다림").

- [ ] **Step 1: 실패하는 시험을 쓴다** (`net_tests.rs`): `net_natives_offload`. 같은 프로세스 안의 `listen`·`connect`·`accept`·`write`·`read_line`이 각각 offload를 지난다(`offloads()`의 늘어난 수).
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm -- net_tests::net_natives_offload` → FAIL
- [ ] **Step 3: 변환한다.**
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- net tls && cargo test -p fung-cli --test http --test tls` → PASS
- [ ] **Step 5: 커밋한다**(검토 뒤 내가). `refactor(net): socket natives offload their waits`

### Task 8: 막는 자리 하나로, R2의 Minor 셋, 문서

**Files:**
- Modify:
  - `crates/fung-vm/src/vm/block.rs`(`block`·`block_until`은 `Runtime::wait`용 하나만 남김, 모듈 문서)
  - `crates/fung-vm/src/vm/call.rs`(`call_root`의 검사를 `assert!`로, 실패 시 들어올 때의 깊이로 자르기)
  - `crates/fung-vm/src/vm/cont.rs`(`floor`의 이유를 바로잡음: 지금은 늘 0이고 방어용임을 적음)
  - `docs/notes/R7-scheduler.md`(R1a 절), `docs/internals.html`(동시성 절의 기다림 설명)
- Test: `crates/fung-vm/src/vm/park_tests.rs`

**Interfaces:**
- Produces: 저장소 시험 `nothing_sleeps_a_thread_but_the_parking_point`
  - `block(`·`block_until(`은 `vm/block.rs`(정의와 `wait_parked`)와 `vm/runtime.rs`(`Runtime::wait`)에만 있다.
  - `outside(`·`outside_watched(`은 `vm/block.rs`·`vm/watch.rs`(정의)와 `native/nat.rs`(`offload`)에만 있다.
  - 파일을 읽어 문자열로 보고, 시험 파일은 뺀다.

- [ ] **Step 1: 실패하는 시험을 쓴다**: 위 저장소 시험, 그리고 `call_root_refuses_a_nested_loop`를 릴리스에서도 돌게 바꾼다(`#[cfg(debug_assertions)]` 제거).
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test --release -p fung-vm -- park_tests::nothing_sleeps cont_tests::call_root` → `call_root`의 검사가 릴리스에서 없어 FAIL. 남은 `block(` 자리가 있으면 그것도 FAIL.
- [ ] **Step 3: 구현하고 문서를 쓴다.**
  - 노트 R1a 절에 들어갈 것: 주차가 이어 가기의 한 갈래인 이유, `wait_parked`가 `block_until`에서 무엇을 떼어 냈나, offload의 두 반쪽과 `'static`이 필요한 이유, 서브에이전트로 나눈 변환과 검토에서 본 것, 측정.
  - 측정: `scripts/bench.sh /tmp/fung-r1a-base target/release/fung`. 시작점 바이너리는 Task 1 전에 `edcc15b`로 만들어 둔다.
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm -- park_tests cont_tests && cargo test --release -p fung-vm -- cont_tests::call_root && cargo test -p fung-cli --test manual --test repo_rust_only` → PASS
- [ ] **Step 5: 커밋한다.** `refactor(vm): one parking point; R2 review follow-ups; R1a note`

### 끝에 한 번 (사용자 방침)

- [ ] 전체 검사: `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`(debug·`--release`), `cargo test --locked --no-fail-fast`(debug·`--release`), `cargo deny check`·`cargo audit`(설치되어 있으면)
- [ ] 벤치마크: 계산 위주 5% 안. 넘으면 원인을 찾고, 고쳐도 넘으면 숫자와 함께 묻는다.
- [ ] 가장 능력 있는 모델로 브랜치 전체 검토. 이 계획, 스펙 10·15절, Review Focus를 준다.
- [ ] 검토 결과를 한 번에 고치고(Critical·Important만, 고칠 때마다 실패하는 시험 먼저), 전체 검사를 다시 돌려 커밋한다.
- [ ] 사용자에게 한국어로 보고: 무엇을 만들었고 어떤 원리인지, 측정, 내가 정한 것, 미룬 Minor. 다음은 R1b 계획.
