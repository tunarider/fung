# 동시성 v2 L1 (범위) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 태스크를 범위 안에서만 띄우게 한다. `scope(f)`·`supervise(f, on_fail)`·`spawn(s, g)`·`cancel(s)`를 넣고, 자유로운 `spawn(f)`, 꺼내지 않은 실패 보고, `spawn_awaited`를 지운다. 런타임은 지금 것(운영체제 스레드 + 실행권)을 그대로 쓴다.

**Architecture:** 범위는 새 값 `Value::Scope`다. 범위마다 취소 노드(`CancelNode`) 하나가 있고, 그 노드는 연 태스크의 노드 아래에 달린다. 본문은 연 태스크에서 그 노드를 쓰며 돈다(`run_task`와 같은 방식). 범위는 살아 있는 자식 수, 첫 실패와 덧붙일 실패, 스스로 취소됐는지(`cancel(s)`)를 들고 있다. 자식은 끝날 때 범위에 알린다. 본문이 끝나면 범위는 자식이 0이 될 때까지 기다린다. 이 기다림은 취소되지 않는다. 옮기는 동안 테스트가 계속 통과하도록 `spawn`은 Task 1~5 동안 두 형태를 다 받는다(과도기). Task 6에서 옛 형태와 옛 장치를 지운다.

**Tech Stack:** Rust 표준 라이브러리, 기존 `fung-vm` 런타임(`vm/task.rs`·`block.rs`·`world.rs`).

**Spec:** `docs/superpowers/specs/2026-10-18-concurrency-v2-r7-design.md` (3절 범위, 9절 `conc`, 11절 L1, 12절 문서와 테스트)

## Global Constraints

- CLAUDE.md의 커밋 전 검사를 모두 통과한 뒤 커밋한다: `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`와 `--release`, `cargo test --locked --no-fail-fast`와 `--release`. 커밋은 Conventional Commits, 영어, 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- TDD: 테스트를 먼저 쓰고, 실패를 눈으로 확인한 뒤 구현한다.
- 동시성 테스트는 `sleep`이 아니라 상태로 순서를 정한다(`testutil::exec_waiting`의 `until`·`blocked`·`queued`·`hold`).
- fung 값(`Rc`)은 실행권을 쥔 스레드만 만진다. 새 `unsafe impl Send`는 만들지 않는다. 범위 상태는 기존 `Handoff`에 실어 보낸다.
- 에러 번호는 `crates/fung-front/src/diag/catalog.rs`에서 단다. 새 번호는 **E0706** `spawn into a closed scope` 하나다. 한 번 단 번호는 바꾸지 않는다.
- 명세 파일은 `FUNG_UPDATE_SPEC=1 cargo test spec_file`로, 기대 출력은 `FUNG_EXPECTED=bless …`로 다시 만들고 **diff를 검토한 뒤** 커밋한다. `.fung` 파일을 더하거나 고치면 사례 수 상수(`DUMP_CASES` 등)도 따라 고친다.
- 이름과 겉모습: `type_of(s)`는 `'scope`, 출력은 `<scope>`, 타입 표기는 `scope`, `==`는 같은 범위인지 본다.
- `agent`·`http.serve`·`_native.conc.spawn_background`·`try_call`은 L1에서 건드리지 않는다(L5·L6). 배경 태스크는 범위 밖에 그대로 남는다.
- `await(t, timeout:)`·`recv(…, timeout:)`은 L2에서 지운다. L1에서는 그대로 둔다.

## Review Focus

- **본문이 자식을 기다리는 동안 바깥에서 취소가 온다.** 범위는 자식을 취소하고 그것들이 **다 끝난 뒤에야** 되감기를 이어 가야 한다. 자식이 남은 채 돌아오면 구조적 동시성이 깨진다. → Task 2의 `outer_cancel_drains_children_first`.
- **자식이 실패한 순간 본문이 다른 자식을 `await` 중이다.** 본문은 E0701로 되감기고, `scope(`에서 나는 에러는 E0701이 아니라 자식의 에러여야 한다. → Task 2의 `the_child_error_wins_over_the_cancellation`.
- **범위 핸들이 자기 자식의 클로저에 붙잡힌다**(`scope(s => spawn(s, () => spawn(s, …)))`). 범위 상태가 자식 목록을 강하게 쥐면 `Rc` 순환이다. 범위는 자식 수만 세고, `on_fail`은 범위가 닫힐 때 놓는다. → Task 1·3에 누수 시험(`a_scope_captured_by_its_children_is_freed`).
- **`supervise`에서 처리기가 돌기 전에 범위가 취소된다.** 대기열의 실패를 조용히 버리면 안 된다. stderr에 `task failed:` 형식으로 보고하고 종료 코드를 1로 한다. → Task 3의 `queued_failures_of_a_cancelled_supervise_are_reported`.
- **교착.** 모두가 범위 끝에서 서로를 기다리면 E0702가 나야 한다. 범위의 기다림이 교착 집계(`Census`)에서 막힌 것으로 세어져야 한다. → Task 1의 `a_scope_waiting_on_a_stuck_child_is_a_deadlock`.

---

### Task 1: 범위 값, `scope(f)`, `spawn(s, g)`

**Files:**
- Create: `crates/fung-vm/src/vm/scope.rs`, `crates/fung-vm/src/vm/scope_tests.rs`, `tests/lang/scopes.fung`, `tests/lang/err_spawn_closed_scope.fung`
- Modify: `crates/fung-vm/src/vm.rs`(모듈 등록, `pub use`), `crates/fung-vm/src/value.rs`(`Value::Scope`), `value/format.rs`, `value/compare.rs`, `value/release.rs`, `vm/channel.rs`(`contains`가 새 갈래를 훑지 않음), `crates/fung-vm/src/conc.rs`(`scope`·과도기 `spawn`), `vm/task.rs`(`spawn_in`, 끝날 때 범위에 알림), `crates/fung-front/src/resolve/fn_.rs`(`BUILTIN_TYPES`에 `"scope"`), `crates/fung-front/src/diag/catalog.rs`(E0706), `spec/builtins.txt`·`spec/errors.txt`(재생성)

**Interfaces:**
- Produces:
  - `pub struct ScopeHandle { pub(crate) state: Rc<ScopeState> }` (`vm/scope.rs`), `Value::Scope(Rc<ScopeHandle>)`
  - `pub(crate) struct ScopeState`: `node: Arc<CancelNode>`, `open: Cell<bool>`(`scope(`가 돌아오면 false), `live: Cell<usize>`, `waiters: RefCell<WaitList>`, `failure: RefCell<Option<Box<Diag>>>`, `also: RefCell<Vec<String>>`(덧붙일 실패의 머리줄), `self_cancelled: Cell<bool>`, `policy: Policy`
  - `pub(crate) enum Policy { FailFast, Supervise(RefCell<Option<Value>>) }` (Task 3이 채움)
  - `pub(crate) fn run_scope(t: &mut Task<'_>, f: Value, policy: Policy) -> Result<Value, Stop>`: 범위를 열고, `f(s)`를 돌리고, 자식을 기다리고, 결과를 정한다.
  - `impl Task { pub(crate) fn spawn_in(&mut self, s: &Rc<ScopeState>, f: Value) -> Result<Value, Stop> }`: 닫힌 범위면 E0706, 아니면 `s.node.child()` 아래에서 `start`하고 `s.live += 1`.
  - `ScopeState::child_done(&self, r: &Result<Value, Stop>)`: `run_spawned`가 실행권을 쥔 채 부른다. `live -= 1`, 실패 기록(Task 2), 0이 되면 `waiters.wake_all()`.
  - 네이티브 `scope(f)`, 과도기 `spawn(s, f = nil)`: 둘째 인자가 `nil`이면 첫 인자를 함수로 보고 옛 자유 `spawn`을 한다. 아니면 `spawn_in`이다.

- [ ] **Step 1: 실패하는 테스트를 쓴다** (`scope_tests.rs`, `testutil::run`·`exec_waiting`)
  - `scope_returns_the_body_value`: `print(scope(s => 1 + 1))` → `"2\n"`, 코드 0
  - `scope_waits_for_its_children`: `ch = channel(1)\nscope(fn(s): spawn(s, fn(): hold(20); send(ch, 'done) ;;) ;;)\nprint(recv(ch, timeout: 0s))` → `"Ok('done)\n"`. 범위가 돌아온 순간 이미 보냈어야 한다.
  - `await_inside_a_scope`: `print(scope(fn(s): a = spawn(s, () => 2); b = spawn(s, () => Err('e)); [await(a), await(b)] ;;))` → `"[2, Err('e)]\n"`
  - `spawn_into_a_closed_scope_is_e0706`: `s2 = scope(s => s)\nspawn(s2, () => 1)` → stderr에 `E0706`, 코드 1
  - `a_child_may_spawn_while_the_scope_drains`: 본문은 곧바로 끝나고, 자식이 같은 `s`에 손자를 띄워 채널로 값을 보낸다. 손자가 보낸 값을 범위 뒤에 받는다.
  - `scope_values`: `scope(fn(s): print(s, type_of(s), s == s) ;;)` → `"<scope> 'scope true\n"`, `fn f(x: scope) = 1`이 컴파일된다.
  - `a_scope_waiting_on_a_stuck_child_is_a_deadlock`: `scope(s => spawn(s, () => recv(channel())))` → E0702, 코드 1
  - `a_scope_captured_by_its_children_is_freed`: 범위 1,000개를 연달아 열고 닫으며 자식이 `s`를 붙잡는다. 끝난 뒤 살아 있는 취소 노드 수(`CancelNodeForTests` / `descendant_count`)가 처음과 같다.
  - golden `tests/lang/scopes.fung`(첫 넷의 요약 `# expect:`), `tests/lang/err_spawn_closed_scope.fung`(`# expect-error: E0706`)
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm scope_tests` → `scope`가 없어 E0201 계열 이름 에러로 FAIL
- [ ] **Step 3: 구현한다.**
  - `run_scope`의 순서:
    1. `node = t.cancel.child()`, 상태를 만든다.
    2. `t.cancel`을 `node`로 바꾸고 `t.call(&f, vec![Value::Scope(..)])`를 부른다. 끝나면 되돌린다.
    3. 본문이 `Err`로 끝났으면 `node.cancel()`.
    4. **취소되지 않는 기다림**: `live == 0`까지 `block`한다. 이때 `t.cancel`을 새 `CancelNode::root()`로 잠시 바꾼다(`Runtime::wait_until`과 같은 수법). 교착 집계에는 그대로 들어간다.
       자식은 범위 노드 아래라서 바깥 취소와 교착 보고(실행 루트 취소)는 자식에게 닿는다. 자식이 끝나면 이 기다림도 풀린다. `Task::unlisted`는 쓰지 않는다. 기다리는 태스크는 살아 있는 태스크로 세어져야 교착이 판정된다.
    5. `open = false`, `policy`의 `on_fail`을 놓는다.
    6. 결과를 정한다(Task 2에서 채움. 지금은 본문 결과).
  - E0706 문구는 `spawn into a closed scope`, 인자는 없다.
  - `Handoff`의 짐에 `Option<Rc<ScopeState>>`를 더해 `run_spawned`가 끝날 때 `child_done`을 부르게 한다. 범위 자식의 `TaskState.awaited`는 true다(옛 실패 보고에서 빠짐).
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm scope_tests`, `FUNG_UPDATE_SPEC=1 cargo test spec_file`, `cargo test -p fung-cli --test golden`, 그다음 전체 검사 → PASS. 새 `.fung` 두 개 때문에 dump·fmt·check 기대 출력과 사례 수가 바뀐다. bless하고 diff를 검토한다.
- [ ] **Step 5: 커밋한다.** `feat(vm): scopes and spawning into them`

### Task 2: `scope`의 실패와 취소

**Files:**
- Modify: `vm/scope.rs`, `vm/scope_tests.rs`, `conc.rs`(`cancel`이 범위도 받음), `tests/lang/scopes.fung`
- Create: `tests/lang/err_scope_failed.fung`

**Interfaces:**
- Consumes: Task 1의 `ScopeState`, `child_done`, `run_scope`
- Produces: `cancel(x)`: `x`가 태스크면 지금처럼, 범위면 `self_cancelled = true` 뒤 `node.cancel()`. 결과 규칙(스펙 3.2):
  - 첫 실패가 있으면 그 `Diag`를 `Stop::Error`로 낸다. 위치·호출 경로는 자식의 것을 그대로 둔다. `notes`에 `scope opened at <file:line:col>` 한 줄, 덧붙인 실패마다 `also failed: <머리줄>` 한 줄씩 넣는다.
  - 실패가 없고 본문이 `Stop::Exit`이나 실패가 아닌 에러로 끝났으면 그대로 낸다.
  - 본문이 E0701로 끝났는데 연 태스크의 노드는 취소되지 않았고 `self_cancelled`라면 `Ok(Err({kind: 'cancelled}))`다. 연 태스크가 취소됐으면 E0701을 그대로 낸다.
  - 아니면 본문의 값.

- [ ] **Step 1: 실패하는 테스트를 쓴다**
  - `a_child_failure_is_raised_at_the_scope`: `scope(s => spawn(s, () => 1 / 0))\nprint("not here")` → 출력 없음, stderr에 0으로 나눈 에러(자식의 위치 `1 / 0`)와 `scope opened at` note, 코드 1
  - `the_child_error_wins_over_the_cancellation`: 본문이 막힌 자식 `b`를 `await`하는 중에 형제 `a`가 실패한다. stderr의 에러 번호가 E0701이 아니라 `a`의 것이다.
  - `a_failure_cancels_the_siblings`: 형제가 `recv(channel())`에서 막혀 있다. 실패한 범위가 형제를 취소하고 돌아온다(교착이 아님, E0702 아님).
  - `two_failures_are_both_shown`: `hold`로 두 자식이 실행권 아래에서 차례로 실패한다(`fail("a")`, `fail("b")`). stderr에 `a`의 에러와 `also failed:` + `b`.
  - `an_err_value_is_not_a_failure`: `print(scope(s => await(spawn(s, () => Err('x)))))` → `"Err('x)\n"`, 코드 0
  - `a_body_error_cancels_and_waits_for_the_children`: 본문이 자식을 띄우고 `1 / 0`. 자식은 `with` 없이 막혀 있으므로 취소만 된다. 에러는 본문 것, 코드 1, 걸린 시간이 2초 미만이다.
  - `outer_cancel_drains_children_first`: 바깥 태스크 `t`(과도기 자유 `spawn`) 안에서 범위가 자식을 띄우고, 자식은 `hold(30)`(실행권을 쥔 채 잠듦) 뒤 채널에 보낸다. `cancel(t)`를 하면 `await(t)`는 `Err({kind: 'cancelled})`이고, 그때 자식은 이미 끝났다(`queued`·`background` 탐침이나 채널 상태로 확인).
  - `cancel_of_the_scope_returns_err_cancelled`: `r = scope(fn(s): spawn(s, () => cancel(s)); recv(channel()) ;;)\nprint(r, "after")` → `"Err({kind: 'cancelled}) after\n"`, 코드 0
  - `cancelling_one_child_is_not_a_failure`: `scope(fn(s): t = spawn(s, () => recv(channel())); cancel(t); print(await(t)) ;;)` → `"Err({kind: 'cancelled})\n"`, 코드 0
  - `exit_in_a_child_ends_the_program`: 자식이 `exit(3)` → 코드 3, 범위 뒤 줄은 돌지 않는다.
  - golden `tests/lang/err_scope_failed.fung`(`# expect-error: E0803`, `fail("x")`를 자식에서)
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm scope_tests` → 새 사례 FAIL
- [ ] **Step 3: 구현한다.**
  - `child_done`은 `Err(Stop::Error(d))`이고 `!task::is_cancel(d)`일 때만 실패로 친다. FailFast에서 첫 실패는 `failure`에 넣고 `node.cancel()`한다. 이미 있으면 `also`에 머리줄(`diag::headline`)을 넣는다. 자식의 `Stop::Exit`은 실패가 아니다(`exit` 쪽이 이미 루트를 취소한다).
  - `cancel` 네이티브의 매개변수 이름은 `t`를 유지하고, 타입 에러 문구는 `task | scope`로 한다.
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm scope_tests`, 전체 검사 → PASS, bless와 diff 검토
- [ ] **Step 5: 커밋한다.** `feat(vm): a failing child fails its scope`

### Task 3: `supervise(f, on_fail)`

**Files:**
- Modify: `vm/scope.rs`, `vm/scope_tests.rs`, `conc.rs`(`supervise`), `vm/failure.rs`(`report_now`·`failed`만 쓰임)
- Create: `tests/lang/supervise.fung`

**Interfaces:**
- Consumes: Task 1·2의 `run_scope`, `Policy`, `child_done`
- Produces: 네이티브 `supervise(f, on_fail)`(둘 다 `need_fn`). `Policy::Supervise`에 `on_fail`, 대기열 `RefCell<VecDeque<Value>>`, `handling: Cell<bool>`를 둔다.
  - 처리기 태스크는 따로 띄우지 않는다. **처음 실패한 자식의 태스크가 처리기가 된다.** 그 태스크는 결과를 저장하고 기다리는 쪽을 깨운 뒤, `handling`이 거짓이면 그것을 참으로 하고 대기열이 빌 때까지 `on_fail(e)`를 차례로 부른다.
  - 이 동안 그 태스크는 `live`에 남는다. `e`는 그 태스크의 `outcome`(`Err({kind: 'error, message})`) 안의 맵이다.
  - 스펙의 "범위 안의 처리기 태스크"를 이렇게 구현한다. 순서는 실패 순서이고, 한 번에 하나만 돈다.

- [ ] **Step 1: 실패하는 테스트를 쓴다**
  - `a_supervised_failure_goes_to_on_fail_and_the_rest_run_on`: 자식 셋 중 가운데가 `fail("b")`. `on_fail`이 메시지를 출력하고, 나머지 둘의 값도 출력되고, 범위는 본문 값을 돌려준다. 코드 0
  - `await_sees_the_failure_too`: `supervise(fn(s): t = spawn(s, () => 1 / 0); print(await(t) |> is_err) ;;, e => print("handled"))` → `"handled"`와 `"true"`(순서는 고정하지 않고 둘 다 있는지 본다)
  - `on_fail_runs_one_at_a_time_in_order`: 실패 셋. `on_fail`이 `sleep(1ms)`로 막히는 동안 다음 실패가 온다. 출력 순서가 실패 순서이고 겹치지 않는다(시작·끝 표시가 번갈아 나온다).
  - `an_on_fail_error_fails_the_supervise`: `on_fail`이 `1 / 0` → `supervise(` 위치에서 그 에러, 코드 1
  - `queued_failures_of_a_cancelled_supervise_are_reported`: 첫 `on_fail`이 막힌 동안 두 번째 실패가 대기열에 있고, 범위가 `cancel(s)`된다. stderr에 `task failed:` 한 번, 코드 1
  - `an_on_fail_capturing_its_scope_is_freed`: `on_fail`이 `s`를 붙잡는 `supervise`를 1,000번 열고 닫는다. 끝난 뒤 살아 있는 취소 노드 수가 처음과 같다(Task 1의 누수 시험과 같은 방법).
  - golden `tests/lang/supervise.fung`
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm scope_tests` → FAIL
- [ ] **Step 3: 구현한다.**
  - `on_fail`이 런타임 에러를 내면 FailFast처럼 `failure`에 넣고 `node.cancel()`한다.
  - 범위가 닫힐 때 대기열에 남은 실패는 `Failures::report_now("task failed: …")`로 쓴다. 문구는 지금 보고와 같다. `failed()`가 참이 되어 종료 코드가 1이 된다. `on_fail`은 닫힐 때 놓는다(`take`).
- [ ] **Step 4: 통과를 확인한다.** 전체 검사 → PASS
- [ ] **Step 5: 커밋한다.** `feat(vm): supervise isolates its children's failures`

### Task 4: `conc`를 범위 위에

**Files:**
- Modify: `extras/conc.fung`, `crates/fung-vm/src/conc.rs`(`spawn_awaited`와 `MODULE_ONLY` 항목 삭제), `tests/lang/conc_helpers.fung`, `spec/builtins.txt`

**Interfaces:**
- Consumes: `scope`, `spawn(s, g)`
- Produces:
  - `conc.pmap(xs, f, limit = 0)`: 안에서 `scope`를 연다. `f`의 런타임 에러는 `pmap`의 실패로 그 위치에서 난다. `f`가 돌려준 `Err`는 값이다. 결과는 입력 순서. `limit` 규칙은 지금과 같다.
  - `conc.fan_in(s, chs)`: 전달 태스크와 닫는 태스크를 `s`에 띄우고 출력 채널을 돌려준다. 모든 입력이 닫히면 출력도 닫힌다.

- [ ] **Step 1: 실패하는 테스트를 쓴다.** `conc_helpers.fung`의 사례를 새 형태로 고친다. `pmap`에서 `f`가 `1 / 0`이면 `# expect-error`가 그 에러인 `err_pmap_failed.fung`, `fan_in(s, [a, b])` 사례.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-cli --test golden` → FAIL
- [ ] **Step 3: 구현한다.** `extras/conc.fung`를 고치고 `spawn_awaited`를 지운다. 머리 주석도 고친다.
- [ ] **Step 4: 통과를 확인한다.** `FUNG_UPDATE_SPEC=1 cargo test spec_file`, 전체 검사 → PASS
- [ ] **Step 5: 커밋한다.** `refactor(conc): pmap and fan_in run in scopes`

### Task 5: 자유 `spawn`을 쓰는 코드를 옮긴다

기계적인 옮겨 쓰기다. 옛 의미(꺼내지 않은 실패 보고, 진입 파일이 남은 태스크를 기다림, 핸들이 사라질 때 보고)를 시험하던 사례는 **지운다.** 같은 뜻을 범위로 시험할 수 있으면(실패 전파, 교착, 취소) 바꿔 쓴다. 지운 사례와 그 이유는 커밋 메시지 본문에 적는다.

**Files:**
- Modify (fung):
  - `tests/lang/`: `tasks`·`cancel`·`channels`·`channel_stream`·`agents`·`exit_from_task`·`http_serve`
  - `tests/lang/err_task_failed.fung`: 지우거나 범위 실패로 바꾼다.
  - `examples/`: `api`·`urlcheck`
  - `bench/`: `spawn`·`chan`
  - `crates/fung-cli/tests/data/test/`: `top_task`·`blocked_top`·`unretrieved`·`exit_in_task`
  - `crates/fung-cli/tests/data/test_ends/`: `agents`·`rest_skipped`·`task_exit`·`top_pending`
- Modify (Rust 안의 fung 조각):
  - `crates/fung-vm/src/vm/`: `task_tests`·`channel_tests`·`agent_tests`·`exit_tests`·`repl_tests`·`m6_tests`, 그리고 Task 2가 과도기 자유 `spawn`으로 쓴 `scope_tests`(`outer_cancel_drains_children_first`는 바깥 태스크를 바깥 범위의 자식으로)
  - `crates/fung-vm/src/vm/failure_tests.rs`: 꺼내지 않은 실패 사례는 지우고, 교착 사례는 범위로 옮긴다.
  - `crates/fung-vm/src/native/`: `net_tests`·`tls_tests`·`fs_tests`·`regex_tests`·`json_tests`
  - `crates/fung-vm/src/prelude/`: `run_cases`·`tests`
  - `crates/fung-cli/tests/`: `run_expected`·`tools_expected`·`http`·`sigpipe`·`task_stack`·`json_expected`
  - `crates/fung-front/src/parser/tests.rs`: 파싱만 하므로 이름만 맞으면 둔다.

**Interfaces:**
- Consumes: `scope`, `spawn(s, g)`, `supervise`, `conc.fan_in(s, chs)`
- Produces: 저장소 안에 인자 하나짜리 `spawn(` 호출이 남지 않는다. Task 6의 검사가 이것을 지킨다.

- [ ] **Step 1: 옮겨 쓴다.** 묶음마다(golden·examples·bench → cli data → fung-vm 단위 → cli 통합) 고치고 그 묶음의 테스트를 돌린다. 기대 출력이 바뀐 곳(`run`·`test`·`repl`·`dump`·`fmt`·`check`)은 bless하고 diff를 검토한다. 바뀐 출력이 옮겨 쓰기로 설명되는지 하나씩 본다.
  - `task_stack.rs`(깊은 재귀를 태스크 안에서)와 `two_thousand_tasks_run`은 같은 크기로 범위 안에서 돈다.
- [ ] **Step 2: 남은 곳이 없는지 확인한다.** Run: `git grep -nE "\bspawn\((\(\)|fn\b|[a-z_]+ =>)" -- '*.fung' '*.rs' '*.html'` → 결과 없음(범위 인자가 먼저 오는 형태만 남음)
- [ ] **Step 3: 전체 검사를 통과시킨다.**
- [ ] **Step 4: 커밋한다.** `test: spawn only inside scopes`. 본문에 지운 사례 목록을 적는다.

### Task 6: 옛 형태와 옛 장치를 지운다

**Files:**
- Modify: `conc.rs`(`spawn`의 매개변수 표를 `[param("s"), param("f")]`로; 첫 인자가 범위가 아니면 `wrong_arg("s", "scope", …)`), `vm/task.rs`(`Task::spawn`의 자유 경로, `TaskHandle`의 `Drop` 보고, `TaskState.failure`·`orphan`, `Kind.reported`), `vm/failure.rs`(`track`·`untrack`·`report`·`report_all` 삭제; Task 3의 `report_now`·`failed`만 남김), `vm/runtime.rs`(`report_unretrieved` 삭제, `wait` 설명 고침), `crates/fung-cli/src/run.rs`·`test.rs`·`repl/mod.rs`·`crates/fung-vm/src/testutil.rs`(`report_unretrieved` 호출 삭제, 진입 성공 뒤의 `rt.wait()`는 배경 태스크와 무관하게 0개를 기다리므로 지우고 `debug_assert`로 바꿈), `spec/builtins.txt`
- Test: `scope_tests.rs`, `crates/fung-cli/tests/golden.rs`

**Interfaces:**
- Consumes: Task 5가 옮긴 코드
- Produces: `spawn(s, f)`만 있다. `Failures`는 `report_now`와 `failed`뿐이다.

- [ ] **Step 1: 실패하는 테스트를 쓴다.**
  - `spawn_needs_a_scope`: `spawn(() => 1)` → E0403(`s`, `scope`), 코드 1
  - `spawn_needs_a_function`: `scope(s => spawn(s))` → 인자 수 에러
  - golden `tests/lang/err_spawn_without_scope.fung`
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm scope_tests` → 옛 경로가 받아 FAIL
- [ ] **Step 3: 지운다.** 지운 뒤 `rg "spawn_awaited|report_unretrieved|orphan|untrack"` → 없음. 모듈 머리 주석(`task.rs`·`failure.rs`·`conc.rs`·`runtime.rs`)을 새 규칙에 맞게 고친다.
- [ ] **Step 4: 통과를 확인한다.** `FUNG_UPDATE_SPEC=1 cargo test spec_file`, 전체 검사(`--release` 포함, 동시성 50회 반복) → PASS
- [ ] **Step 5: 커밋한다.** `feat!: spawn needs a scope`

### Task 7: 문서

**Files:**
- Modify:
  - `docs/design/05-concurrency.md`: 원시 연산 표, "태스크의 결과와 에러", "종료", "타임아웃·취소"의 전파 문장, 구현 메모
  - `docs/design/06-native-and-stdlib.md`: prelude 목록, `conc`
  - `docs/design/07-operators-and-types.md`: 타입 이름 `scope`
  - `docs/design/00-concept.md`: 동시성 한 줄
  - `docs/design/open-questions.md`: "태스크 실패 후속" 세 항목을 닫고, "언어 층 후보"의 구조적 동시성을 닫는다.
  - `docs/builtins.html`(`scope`·`supervise`·`spawn`·`cancel` 표와 예제), `docs/syntax.html`(동시성 예제), `docs/internals.html`(태스크와 범위, 실패 보고 절)
  - `CHANGELOG.md` `[Unreleased]`: 호환을 깨는 변경과 옮겨 쓰기(`spawn(f)` → `scope(s => spawn(s, f))`, `pmap` 실패, `fan_in(s, chs)`)
- Create: `docs/notes/R7-language.md`. 학습용 설명으로, L1 절을 쓴다. 다룰 것:
  - 구조적 동시성이 무엇이고 왜 이것 하나로 가는가
  - 범위의 상태 기계와 취소되지 않는 기다림
  - 처리기를 실패한 자식이 맡는 이유
  - Trio·Kotlin·Java `StructuredTaskScope`와의 비교
  - 과도기 `spawn`을 둔 이유
- Test: `crates/fung-cli/tests/manual.rs`, `changelog.rs`, `repo_rust_only.rs`

- [ ] **Step 1: 고친다.** 매뉴얼 예제는 `fung check`·`fmt --check`·실행을 통과해야 하고, `builtins.html`의 매개변수 표는 `spec/builtins.txt`와 같아야 한다.
- [ ] **Step 2: 확인한다.** Run: `cargo test -p fung-cli --test manual --test changelog --test repo_rust_only`, 그다음 전체 검사 → PASS
- [ ] **Step 3: 커밋한다.** `docs: scopes in the design, the manual and the changelog`
