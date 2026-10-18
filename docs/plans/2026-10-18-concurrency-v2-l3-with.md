# 동시성 v2 L3 (정리 장치 `with`) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `with(acquire, use, release)`를 넣는다. `release`는 정상, `Err` 반환, 런타임 에러, 취소, `exit` 어느 경우에나 돈다. `fs.with_open`·`fs.with_temp_dir`를 그 위에 다시 짠다. `at_exit`를 지운다.

**Architecture:**
- **`with`는 네이티브다.**
  - `acquire()`를 부르고, 그 값으로 `use(r)`를 부른다. 결과가 무엇이든 그다음 `release(r)`를 부른다.
  - `acquire`가 돌아온 뒤 `use` 호출 전에 취소 확인 지점이 있어도 `release`는 돈다. 그래서 획득과 등록 사이에 자원이 새지 않는다.
- **`release`는 차단 노드에서 돈다.**
  - 차단 노드는 새 루트 노드라서 바깥 취소와 기한이 닿지 않는다.
  - 살아 있는 차단 노드는 `World::shields`에 등록한다. 교착 판정은 차단 노드가 하나라도 있으면 앞서 `exit`이나 교착이 있었더라도 판정한다. 교착이면 그 노드들을 취소한다(지금 `at_exit` 핸들러를 다루는 방식과 같음). 그래서 정리 중에 멈춘 `release`가 프로세스를 영원히 붙잡지 않는다.
- **`at_exit`가 하던 일은 되감기가 맡는다.** `exit`·교착·진입 파일 에러는 모두 루트 취소이고, 되감기는 모든 `with`의 `release`를 지난다. 그래서 `at_exit`와 그 장치(핸들러 목록, 핸들러 루트, 핸들러 교착 판정, CLI의 핸들러 실행)를 지운다.

**Tech Stack:** Rust 표준 라이브러리, `fung-vm`(`vm/task.rs`의 `CancelNode`, `vm/deadlock.rs`, `vm/world.rs`, `vm/runtime.rs`), `extras/fs.fung`, `fung-cli`(`run.rs`·`test.rs`·`repl/mod.rs`).

**Spec:** `docs/superpowers/specs/2026-10-18-concurrency-v2-r7-design.md` (5절 정리, 8절 종료, 11절 L3). 앞 단계: `docs/plans/2026-10-18-concurrency-v2-l1-scopes.md`, `…-l2-timeout.md`, 노트 `docs/notes/R7-language.md`.

## Global Constraints

- 사용자 방침(메모리 `plan-execution-style`)을 따른다.
  - 작업마다 TDD로 하고, 그 작업의 테스트만 돌린다. 작업 단위로 커밋하되 전체 커밋 전 검사는 하지 않는다.
  - 모든 작업이 끝난 뒤 전체 검사(fmt, clippy와 `cargo test --locked --no-fail-fast`를 debug·release 둘 다), 브랜치 전체 검토, 수정을 한 번 한다.
- 커밋은 Conventional Commits, 영어, 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- 동시성 테스트는 상태로 순서를 정한다(`testutil::exec_waiting`의 `until`·`blocked`·`queued`·`hold`·`buffered`·`finished`).
- 새 `unsafe`는 없다. 차단 노드 목록은 `World` 안(실행권 아래)에 둔다.
- `with`의 겉모습: `with(acquire, use, release)`. 셋 다 함수가 아니면 E0403(`acquire`·`use`·`release`, `fn`). 결과는 `use(r)`의 반환값이다.
- **에러를 합치는 규칙** (스펙 5.1. 사용자 정의 밖의 경우는 이 계획의 판단):
  1. `acquire`가 런타임 에러나 취소로 끝나면 그대로 올라간다. `release`는 돌지 않는다.
  2. `release`가 실패가 아닌 것(값, 취소, 교착으로 끊김)으로 끝나면 `use`의 결과가 그대로다.
  3. `use`가 진짜 런타임 에러이고 `release`도 진짜 에러면 `use`의 에러가 올라가고, `release`의 머리줄은 `note: also failed in release: …`로 붙는다.
  4. **그 밖에** `release`가 진짜 런타임 에러면 그 에러가 올라간다. `use`가 값, `Err` 값, 취소, `exit`이었던 경우가 해당한다. `release`의 실패는 어떤 경우에도 버리지 않는다는 뜻이다.
- `use(r)` 호출은 꼬리 위치가 아니다(네이티브 안에서 부르므로 자연히 그렇다). 03에 한 줄 적는다.
- 에러 번호는 새로 달지 않는다. E0806(`too many %s`)은 다른 한도가 계속 쓴다.
- 명세·기대 출력 갱신과 사례 수 상수는 앞 단계와 같다.

## Review Focus

- **`exit` 뒤의 정리.** 태스크가 `exit(3)`을 부르면 진입 파일과 다른 태스크의 `with`가 되감기며 `release`를 돈다. 종료 코드는 3이다. → Task 1의 `exit_runs_every_release`.
- **정리 중 교착.** `exit` 뒤의 `release`가 `recv(channel())`에서 멈추면 E0702로 끊기고 프로세스가 끝난다(멈추지 않는다). → Task 1의 `a_release_stuck_after_exit_is_cut_by_the_deadlock_check`.
- **기한이 정리를 끊지 않는다.** `timeout(20ms, () => with(…, () => recv(channel()), r => sleep(50ms)))`는 `release`의 `sleep`을 끝까지 돌고 `Err({kind: 'timeout})`. → Task 1의 `a_deadline_does_not_cut_release`.
- **범위 실패 중의 정리.** 형제의 실패로 취소된 자식의 `with`도 `release`를 돌고, 범위는 그 뒤에 형제의 에러를 낸다. → Task 1의 `a_failing_sibling_still_lets_release_run`.
- **`with_open`의 파일 핸들.** 본문이 `1 / 0`으로 죽어도 파일이 닫힌다(그 뒤 `fs.write_line(h, …)`가 `Err('closed)`). → Task 2.

---

### Task 1: `with`와 차단 노드

**Files:**
- Create: `crates/fung-vm/src/vm/cleanup.rs`(`run_with`), `crates/fung-vm/src/vm/cleanup_tests.rs`, `tests/lang/with.fung`
- Modify: `crates/fung-vm/src/vm/world.rs`(`shields: Vec<Weak<CancelNode>>`), `crates/fung-vm/src/vm/deadlock.rs`(차단 노드가 있으면 판정하고 그것들을 취소), `crates/fung-vm/src/conc.rs` 또는 `prelude.rs`(네이티브 `with`; 둘 중 동시성 표가 있는 `conc.rs`에 둔다), `crates/fung-vm/src/vm.rs`, `crates/fung-vm/src/prelude/tests.rs`·`crates/fung-cli/tests/run_expected.rs`의 `CONCURRENCY` 목록, `spec/builtins.txt`

**Interfaces:**
- Produces:
  - `pub(crate) fn run_with(t: &mut Task<'_>, acquire: Value, use_: Value, release: Value) -> Result<Value, Stop>`
  - `World`:
    - `pub(super) shields: Vec<Weak<CancelNode>>`. `run_with`이 `release` 동안 넣었다가 뺀다.
    - `pub(super) fn live_shields(&mut self) -> Vec<Arc<CancelNode>>`: 죽은 `Weak`를 치우고 살아 있는 것을 돌려준다.
  - 교착 판정(`judge`)의 규칙. 지금의 `handler` 갈래를 일반화한다(Task 3에서 `handler`를 지운다).
    - 차단 노드가 살아 있으면, 앞서 교착이나 `exit`이 있었어도 판정한다. 교착이면 E0702 줄을 쓰고, 차단 노드를 모두 취소하고, 루트도 취소한다.
    - 차단 노드가 없으면 지금과 같다.

- [ ] **Step 1: 실패하는 테스트를 쓴다** (`cleanup_tests.rs`, `testutil::{run, exec_waiting}`)
  - `with_gives_the_use_value_and_releases`: `print(with(() => "r", r => r + "!", r => print("released", r)))` → `"released r\nr!\n"`
  - `release_runs_after_an_err_value`: `use`가 `Err('x)`를 돌려준다 → `release` 출력 뒤 `Err('x)`
  - `release_runs_after_a_runtime_error`: `with(() => 1, r => 1 / 0, r => print("released"))` → stdout `"released\n"`, E0502, 코드 1
  - `release_runs_after_a_cancel`: `scope(fn(s): t = spawn(s, () => with(() => 1, r => recv(channel()), r => print("released"))); blocked(…); cancel(t); print(await(t)) ;;)` → `"released\nErr({kind: 'cancelled})\n"`(`exec_waiting`, 막힌 채널은 이름 붙여 `blocked`로 기다림)
  - `exit_runs_every_release`: 진입 파일의 `with` 안에서 범위를 열고, 자식 둘이 각각 `with` 안에서 막혀 있다가, 셋째 자식이 `exit(3)`을 부른다 → 세 `release`의 출력(순서는 단언하지 않고 셋 다 있는지)이 나오고, 코드 3
  - `a_failing_sibling_still_lets_release_run`: `scope(fn(s): spawn(s, () => with(() => 1, r => recv(gate), r => print("released"))); spawn(s, fn(): blocked(gate, 1); fail("boom") ;;) ;;)` → stdout `"released\n"`, stderr에 `boom`, 코드 1
  - `a_deadline_does_not_cut_release`: `print(timeout(20ms, () => with(() => 1, r => recv(channel()), fn(r): sleep(50ms); print("released") ;;)))` → `"released\nErr({kind: 'timeout})\n"`
  - `acquire_failing_skips_release`: `with(() => 1 / 0, r => 1, r => print("released"))` → stdout 비어 있음, E0502
  - `a_release_error_replaces_a_value`: `with(() => 1, r => 2, r => fail("rel"))` → E0803 `rel`, 코드 1
  - `a_use_error_keeps_the_release_error_as_a_note`: `with(() => 1, r => fail("use"), r => fail("rel"))` → `error[E0803]: use`, `note: also failed in release: error[E0803]: rel`
  - `a_release_stuck_after_exit_is_cut_by_the_deadlock_check`: `with(() => 1, r => exit(3), r => recv(channel()))` → stderr에 E0702, 2초 안에 끝남, 코드 3(먼저 요청된 `exit`)
  - `a_release_stuck_without_exit_is_a_deadlock`: `with(() => 1, r => 2, r => recv(channel()))` → E0702, 코드 1
  - `with_checks_its_arguments`: `with(1, r => r, r => r)` → ``argument `acquire` must be fn``
  - golden `tests/lang/with.fung`(앞의 셋 요약)
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm cleanup_tests` → `with`가 없어 FAIL
- [ ] **Step 3: 구현한다.**
  - `run_with`:
    - `r_acq = t.call(&acquire, vec![])?` → `res`
    - `r_use = t.call(&use_, vec![res.clone()])`
    - 차단 노드 `shield = CancelNode::root()`를 `t.world.shields`에 넣고 `t.cancel`과 바꿔 `r_rel = t.call(&release, vec![res])`를 부른 뒤 되돌리고 목록에서 뺀다.
    - 위 Global Constraints의 규칙으로 합친다.
  - `judge`: `let shields = w.live_shields();`를 두고, `handler.is_none() && shields.is_empty() && (deadlocked || exit)`일 때만 판정을 건너뛴다. 보고 뒤 `shields`의 노드를 모두 취소한다. 기존 `handler` 갈래는 Task 3까지 둔다.
  - `FUNG_UPDATE_SPEC=1 cargo test spec_file`
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm cleanup_tests`, `cargo test -p fung-vm scope_tests`, `cargo test -p fung-vm deadline_tests`, `cargo test -p fung-cli --test golden` → PASS
- [ ] **Step 5: 커밋한다.** `feat(vm): with(acquire, use, release)`

### Task 2: `fs.with_open`·`fs.with_temp_dir`를 `with` 위에

**Files:**
- Modify: `extras/fs.fung`, `crates/fung-vm/src/native/fs_tests.rs`(또는 `tests/lang`의 fs 골든)

**Interfaces:**
- Consumes: `with`
- Produces: 겉모습은 그대로다(`with_open(path, f, mode = 'read)`, `with_temp_dir(f)`). 바뀌는 것은 런타임 에러와 취소에도 닫거나 지운다는 점이다.
  - `with_open`: 여는 데 실패하면 `Err`를 돌려준다(지금처럼 `?`). 열었으면 `with(() => h, f, h => _n.close(h))`. 닫기의 `Err` 값은 무시한다(이미 닫힌 핸들 등). 런타임 에러만 규칙 4로 올라간다.
  - `with_temp_dir`: `with(() => dir, f, d => _n.remove_all(d))`.

- [ ] **Step 1: 실패하는 테스트를 쓴다**
  - `with_open_closes_after_a_runtime_error`: `p`에 파일을 만들고 `supervise(fn(s): spawn(s, () => fs.with_open(p, fn(h): send(out, h); 1 / 0 ;;, mode: 'write)) ;;, e => nil)` 뒤 `print(fs.write_line(recv(out)!, "x"))` → `Err({kind: 'closed, …})` 형태(지금 메시지 그대로)
  - `with_temp_dir_removes_after_a_runtime_error`: 같은 방식으로 경로를 내보내고 `1 / 0` 뒤 `print(fs.exists(dir))` → `false`
  - `with_temp_dir_removes_after_a_cancel`: 본문이 `recv(channel())`에서 막힌 태스크를 `cancel` → `fs.exists(dir)`가 `false`
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm with_open_closes with_temp_dir_removes`(필터는 하나씩) → 지금은 닫히거나 지워지지 않아 FAIL
- [ ] **Step 3: 구현한다.** `extras/fs.fung`의 두 함수와 머리 주석(03의 한계가 사라짐)
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm fs_tests`, `cargo test -p fung-cli --test golden` → PASS
- [ ] **Step 5: 커밋한다.** `fix(fs): with_open and with_temp_dir clean up on errors and cancels`

### Task 3: `at_exit`를 지운다

**Files:**
- Modify:
  - `crates/fung-vm/src/prelude.rs`: `at_exit` 네이티브
  - `crates/fung-vm/src/vm.rs`: `Task::at_exit`, `request_exit`의 `handler` 갈래
  - `crates/fung-vm/src/vm/world.rs`: `exit_handlers`, `handler`, `Shared::handlers`
  - `crates/fung-vm/src/vm/runtime.rs`: `run_exit_handlers`, `MAX_EXIT_HANDLERS`, `Drop`의 `handlers.cancel()`
  - `crates/fung-vm/src/vm/deadlock.rs`: `handler` 갈래와 그 머리 주석
  - `crates/fung-cli/src/run.rs`·`test.rs`·`repl/mod.rs`, `crates/fung-vm/src/testutil.rs`: 핸들러 실행과 그 에러 보고. `Outcome.handler_errors`를 지운다.
  - 옮겨 쓰기: `git grep -n "at_exit"`이 찾는 모든 fung 코드(약 90곳; `tests/lang/at_exit.fung`, `crates/fung-cli/tests/data/test/handlers.fung`, Rust 테스트의 fung 조각, `docs/*.html` 예제)
- Test: 기존 테스트 전부, `cleanup_tests.rs`

**Interfaces:**
- Consumes: `with`
- Produces: `at_exit`는 없다(E0204 이름 없음).
- 옮겨 쓰기 규칙:
  - 프로그램 끝의 정리가 목적인 사례(`at_exit(() => print("bye"))` 뒤 무언가) → 그 뒤의 코드를 `with(() => nil, _ => …, _ => print("bye"))`로 감싼다. 출력 순서가 같은지 본다.
  - 핸들러 자체의 성질을 시험하던 사례는 **지운다.** 등록 역순, 핸들러 안의 `exit`·교착, 한도 E0806, 핸들러가 띄운 태스크, 핸들러 안의 에러 보고가 해당한다. 같은 뜻을 `with`로 시험할 수 있으면(정리 중 교착, 정리 중 에러) Task 1의 사례가 이미 덮는지 확인하고, 덮지 않으면 `cleanup_tests.rs`에 하나 더한다.
  - 지운 사례와 이유는 커밋 메시지 본문에 적는다.

- [ ] **Step 1: 실패하는 테스트를 쓴다.** `cleanup_tests::at_exit_is_gone`: `at_exit(() => 1)` → E0204
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm at_exit_is_gone` → FAIL
- [ ] **Step 3: 지우고 옮겨 쓴다.** 묶음마다 그 묶음의 테스트를 돌리고, bless와 diff 검토를 한다. 기계적인 옮겨 쓰기는 서브에이전트에 나눠 맡겨도 된다(L1처럼 crate별로 파일이 겹치지 않게).
- [ ] **Step 4: 확인한다.** `git grep -n "at_exit" -- '*.rs' '*.fung' '*.html'` → 역사 문서 밖에서는 없음(또는 "지웠다"는 주석만). Run: `cargo test -p fung-vm`, `cargo test -p fung-cli` → PASS
- [ ] **Step 5: 커밋한다.** `feat!: at_exit is gone; with is the one cleanup`

### Task 4: 문서

**Files:**
- Modify:
  - `docs/design/03-functions-and-patterns.md`: "자원 정리" 절을 다시 쓴다(`with`, `at_exit` 삭제, 한계였던 줄 삭제, `defer` 기각 이유는 둠, 꼬리 위치 규칙 한 줄).
  - `docs/design/05-concurrency.md`: 종료 절(`at_exit` 언급), `with`가 취소·`exit`과 맞물리는 방식
  - `docs/design/06-native-and-stdlib.md`: prelude 목록, `fs.with_open`
  - `docs/design/01-syntax.md`·`background.md`·`09-implementation-plan.md`의 `at_exit` 언급
  - `docs/design/open-questions.md`: L3 완료
  - 매뉴얼: `docs/builtins.html`(`with` 항목, `at_exit` 삭제, 함수 수), `docs/syntax.html`, `docs/internals.html`(끝내기 순서, 차단 노드)
  - `CHANGELOG.md` `[Unreleased]`: 호환을 깨는 변경(`at_exit` 삭제, 옮겨 쓰기)과 더함(`with`), `with_open`·`with_temp_dir`의 고쳐진 동작
  - `docs/notes/R7-language.md`: L3 절. 다룰 것:
    - `defer`를 기각했던 이유와 `with`가 그것을 피하는 방식
    - 차단 노드와 교착 판정
    - 에러를 합치는 규칙
    - `at_exit`를 지운 이유
- Test: `cargo test -p fung-cli --test manual --test changelog --test repo_rust_only`

- [ ] **Step 1: 고친다.** 매뉴얼 예제는 `fung check`·`fmt --check`·실행을 통과해야 한다.
- [ ] **Step 2: 확인한다.** 위 Test 명령 → PASS
- [ ] **Step 3: 커밋한다.** `docs: with in the design, the manual and the changelog`
