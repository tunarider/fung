# 동시성 v2 L5 (엄격한 `atom`, `agent` 삭제) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 공유 상태를 `atom(v)`·`deref(a)`·`swap(a, f)`·`reset(a, v)`로 바꾼다. `swap`의 `f`는 순수 계산만 하도록 런타임이 강제하고, atom 안에는 변하는 것을 넣지 못하게 한다. `agent`와 그 장치를 지운다.

**Architecture:**
- **atom은 값 하나를 가리키는 칸이다.** `RefCell<Value>`와 판(version) 번호로 이뤄진다.
  - `swap`은 비교 후 교체다. 옛 값과 판 번호를 읽고, `f`를 돌린 뒤, 판 번호가 그대로면 바꾸고 아니면 다시 돈다.
  - 지금 런타임에서도 `f` 도중에 차례가 넘어갈 수 있으므로(`YIELD_EVERY`) 다시 도는 일이 실제로 생긴다.
- **순수성 검사:**
  - 네이티브에 `effect` 표시를 둔다. 태스크는 `swap`의 `f`를 도는 동안 `pure` 깊이를 센다.
  - VM은 네이티브를 부르기 직전에 `pure > 0 && effect`면 E0707을 낸다. 네이티브를 부르는 두 길(`enter`의 네이티브 갈래와 `call_buf`의 빠른 길)이 모두 이 검사를 지난다.
- **"변하는 것을 품었는지"는 O(1)이다** (스펙 7.2).
  - 영속 벡터(`Pvec`)의 노드마다 표시를 두고, 노드를 만들 때 아이들의 표시로 정한다. 경로 복사로 바뀐 노드만 다시 셈한다.
  - 작은 표현(평평한 리스트 ≤32, 평평한 맵 ≤16)과 튜플, 태그, 클로저는 원소를 훑는다. 원소 하나하나의 검사는 다시 O(1)이다.
  - 모듈 참조는 표시를 전파하지 않는다(스펙 7.2의 판단).

**Tech Stack:** Rust 표준 라이브러리, `fung-vm`(`value/pvec.rs`·`value/list.rs`·`value/map.rs`·`value/omap.rs`, `vm/call.rs`, `conc.rs`), `extras/http.fung`(에이전트 도우미 중 서버가 쓰는 것만 남김).

**Spec:** `docs/superpowers/specs/2026-10-18-concurrency-v2-r7-design.md` 7절(공유 상태), 11절. 순서는 사용자 결정으로 L4보다 먼저다(2026-10-18). 앞 단계 계획: `docs/plans/2026-10-18-concurrency-v2-l{1,2,3}-*.md`.

## Global Constraints

- 사용자 방침(메모리 `plan-execution-style`)을 따른다.
  - 작업마다 TDD로 하고, 그 작업의 테스트만 돌린다. 작업 단위로 커밋하되 전체 커밋 전 검사는 하지 않는다.
  - 끝에 전체 검사(fmt, clippy와 test를 debug·release 둘 다), 브랜치 전체 검토, 수정을 한 번 한다.
  - 사용자가 계획 쓰기와 실행을 함께 요청했다(2026-10-18).
- 커밋은 Conventional Commits, 영어, 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- 새 `unsafe`는 없다.
- 이름과 겉모습:
  - `atom(v)`, `deref(a)`, `swap(a, f)`(새 값을 돌려줌), `reset(a, v)`(새 값을 돌려줌).
  - `type_of`는 `'atom`, 출력은 `<atom>`, 타입 표기는 `atom`, `==`는 같은 칸인지 본다.
  - 인자 에러는 E0403(`a`·`atom`, `f`·`fn`).
- 새 에러 번호:
  - **E0707** `` `%s` has an effect, which a swap's function must not have ``(인자: 네이티브 이름)
  - **E0708** `an atom cannot hold an atom, a channel, a task, a scope, a file or a socket`(인자 없음)
  - E0704(`agent called from inside its own swap`)는 "쓰지 않음"으로 바꾼다. E0703은 그대로 "쓰지 않음"이다. 번호는 다시 쓰지 않는다.
- **`effect` 표시 목록** (스펙 7.2의 분류. 기준은 "`f`가 여러 번 돌아도 바깥에서 보이는 차이는 걸린 시간뿐"):
  - **효과 있음:**
    - prelude: `print`, `eprint`, `dbg`, `read_line`, `read_all`(있으면), `exit`, `sh`, `exec`
    - 동시성: `spawn`, `await`, `cancel`, `send`, `recv`, `close`, `sleep`, `swap`, `reset`
    - 모듈: `_native.fs`의 전부, `_native.net`의 전부, `_native.process`의 `exec`·`sh`(같은 이름의 prelude 재내보내기 포함)
    - 내부: `_native.conc`의 전부
  - **효과 없음:** 나머지 모두다. `deref`, `atom`, `channel`, `scope`, `supervise`, `timeout`, `with`, `fail`, 시간·환경 읽기, json, regex, path, 문자열·컬렉션이 해당한다.
    - `scope` 등은 그 안에서 효과 있는 것을 부르면 그때 걸린다.
    - 목록은 Task 2의 표 시험이 지킨다.
- "변하는 것": atom, 채널, 태스크 핸들, 범위 핸들, 파일, 연결(`Conn`), 리스너. 그리고 이것들을 품은 리스트·맵·튜플·태그·클로저(붙잡은 값)다. 모듈과 네이티브는 아니다.

## Review Focus

- **`swap` 경합.**
  - 두 태스크가 같은 atom에 `swap(a, x => x + 1)`을 각 1만 번 하면 결과는 2만이다(갱신이 사라지지 않음).
  - 계산이 긴 `f`(차례 넘기기가 일어나는 크기)로 다시 도는 길을 실제로 지나게 한다.
  - → Task 2의 `concurrent_swaps_lose_no_update`.
- **다시 도는 `f`의 효과가 막힌다.** `f` 안의 `print`는 E0707이다. 첫 시도에서 바로 막히므로 두 번 찍히는 일이 없다. → Task 2.
- **큰 영속 컬렉션의 검사 비용.**
  - 원소 10만 개 맵을 담은 atom에 키 하나를 바꾸는 `swap`을 1,000번 한다. 몇백 ms 안에 끝나야 한다(O(log n) 경로 재계산).
  - 그 맵에 채널을 하나 넣으면 E0708, 그 키를 지우면 다시 된다.
  - → Task 1·2.
- **리스트 조각.** 채널이 든 큰 리스트에서 채널이 없는 조각(`xs[100..]`)은 atom에 넣을 수 있다. → Task 1.
- **HTTP 서버.** 에이전트를 지운 뒤에도 `http.serve`가 지금처럼 돈다. 지금 `http`의 시험 전체가 지킨다. 서버가 쓰는 `_native.conc` 도우미(`spawn_background`·`try_call`·`set_asker`·`describe`·`raise`)는 남긴다. → Task 3.

---

### Task 1: O(1) "변하는 것을 품었는지"

**Files:**
- Modify:
  - `crates/fung-vm/src/value/pvec.rs`: `Node::Leaf(Vec<T>, bool)`, `Node::Branch(Vec<Rc<Node<T>>>, bool)`, 생성은 도우미 `leaf(items)`·`branch(kids)`로만, `Pvec::holds_mutable()`(루트 표시와 꼬리를 훑음)
  - `crates/fung-vm/src/value/list.rs`, `value/map.rs`, `value/omap.rs`
  - `crates/fung-vm/src/value.rs`: `trait HoldsMutable`, `Value::holds_mutable`
- Test: `crates/fung-vm/src/value/tests.rs`(또는 새 `value/mutable_tests.rs`)

**Interfaces:**
- Produces:
  - `pub(crate) trait HoldsMutable { fn holds_mutable(&self) -> bool; }`
    - `Value`: atom·채널·태스크·범위·파일·연결·리스너는 true. 리스트·맵·튜플·태그·클로저는 각자의 규칙. 나머지는 false.
    - `Option<Entry>`(omap): 값의 것.
    - `i64`(시험): false.
  - `impl Value { pub(crate) fn holds_mutable(&self) -> bool }`
  - 리스트 `Big { vec, lo, hi }`: 창이 전체(`lo == 0 && hi == vec.len()`)면 `vec.holds_mutable()`, 아니면 창을 훑는다.

- [ ] **Step 1: 실패하는 테스트를 쓴다** (Rust에서 `Value`를 만들어 부른다. atom이 아직 없으므로 채널로 시험한다)
  - `ints_hold_nothing_mutable`: 원소 1,000개 정수 리스트 → false
  - `a_channel_anywhere_is_found`: 큰 리스트의 500번째, 맵의 값, 튜플·태그의 인자, 리스트 안 맵 안 튜플에 채널 → 모두 true
  - `removing_the_channel_clears_it`: 큰 맵에서 채널을 담은 키를 지운 맵 → false. 큰 리스트에서 그 자리를 정수로 바꾼 리스트 → false
  - `a_slice_without_the_channel_holds_none`: 채널이 0번에 있는 원소 200개 리스트의 `[100..]` 조각 → false
  - `a_closure_holding_a_channel_is_found`: fung `ch = channel()\nf = () => ch`의 `f` → true(`testutil::exec`의 결과 값을 쓰거나 `setup`으로 전역을 읽는다)
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm holds_mutable` → `holds_mutable`이 없어 컴파일 실패
- [ ] **Step 3: 구현한다.** 노드를 만드는 모든 자리를 `leaf`·`branch`로 바꾼다. 꼬리(tail)는 ≤32라서 훑는다.
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm value::`, `cargo test --release -p fung-cli --test perf` → PASS(컬렉션 누적 0.3초 한도가 지켜짐)
- [ ] **Step 5: 커밋한다.** `feat(vm): know in O(1) whether a value holds a mutable one`

### Task 2: `atom`·`deref`·`swap`·`reset`과 순수성 검사

**Files:**
- Create: `crates/fung-vm/src/vm/atom.rs`, `crates/fung-vm/src/vm/atom_tests.rs`, `tests/lang/atoms.fung`, `tests/lang/err_atom_effect.fung`, `tests/lang/err_atom_holds.fung`
- Modify:
  - `crates/fung-vm/src/value.rs`: `Value::Atom(Rc<Atom>)`, `Native.effect: bool`, `Native::leak_effect`
  - `crates/fung-vm/src/value/{format,compare,release}.rs`
  - `crates/fung-vm/src/native/nat.rs`: `effect_native(name, f, params)`
  - 효과 목록대로 네이티브를 만드는 자리: `prelude.rs`·`prelude/io.rs`·`conc.rs`·`native/fs.rs`·`native/net.rs`·`native/process.rs`
  - `crates/fung-vm/src/vm.rs`·`vm/call.rs`: `Task.pure: u32`, 네이티브 호출 전 검사
  - `crates/fung-front/src/resolve/fn_.rs`: `BUILTIN_TYPES`에 `"atom"`
  - `crates/fung-front/src/diag/catalog.rs`: E0707·E0708, E0704를 "쓰지 않음"으로
  - `spec/`
- 과도기: prelude에 `agent`의 fung `swap`·`deref`가 이미 있다. 이름이 겹치므로 이 작업에서 `extras/agent.fung`의 공개 이름을 prelude에서 뺀다(파일과 네이티브는 Task 3에서 지움). 그사이 `agent(`를 쓰는 시험은 깨지고, Task 3이 옮긴다.

**Interfaces:**
- Consumes: Task 1의 `Value::holds_mutable`
- Produces:
  - `pub struct Atom { cell: RefCell<Value>, version: Cell<u64> }`
  - `pub(crate) fn run_swap(t: &mut Task<'_>, a: &Rc<Atom>, f: Value) -> Result<Value, Stop>`
    - `loop { (old, v) = 읽기; t.pure += 1; r = t.call(&f, vec![old]); t.pure -= 1; new = r?; 검사(E0708); 판이 v면 쓰고 v+1, 끝; 아니면 다시 }`
    - `f`가 에러면 상태는 그대로이고 그 에러가 올라간다.
  - `reset`·`atom`도 E0708을 검사한다.
  - `pure`는 `f`의 호출 안에서 띄운 범위의 자식에게 물려지지 않는다. `spawn` 자체가 막히므로 그럴 일이 없다.

- [ ] **Step 1: 실패하는 테스트를 쓴다** (`atom_tests.rs`)
  - `atoms_hold_swap_and_reset`: `a = atom(1)\nprint(swap(a, x => x + 1), deref(a), reset(a, 10), deref(a), a, type_of(a), a == a)` → `"2 2 10 10 <atom> 'atom true\n"`
  - `concurrent_swaps_lose_no_update`: `a = atom(0)\nfn spin(n) = n == 0 then 0 else spin(n - 1)\nscope(fn(s): each(0..2, _ => spawn(s, () => each(0..10000, _ => swap(a, x => fn(): spin(20); x + 1 ;;())))) ;;)\nprint(deref(a))` → `"20000\n"`
  - `an_effect_in_f_is_e0707`: `swap(atom(1), fn(x): print(x); x ;;)` → stdout 비어 있음, stderr에 ``error[E0707]: `print` has an effect``, 코드 1
  - `blocking_spawning_and_other_atoms_are_effects`: `swap` 안의 `sleep(1ms)`·`recv(…)`·`scope(s => spawn(s, () => 1))`·`swap(b, …)`·`reset(b, 1)`·`exit(0)`·`sh("true")` → 각각 E0707과 그 이름
  - `reads_are_not_effects`: `swap(a, x => x + len(fs_free_string_op))` 같은 순수 계산, `deref(b)`, `time.now()`(`import time`), `env("HOME")` → 모두 된다
  - `an_atom_cannot_hold_mutable_things`: `atom(channel())`, `reset(a, [1, channel()])`, `swap(a, _ => {k: atom(1)})`, `atom(scope(s => s))` → E0708. `atom(() => 1)` → 된다
  - `a_failing_f_leaves_the_state`: `a = atom(1)\nsupervise(s => spawn(s, () => swap(a, x => x / 0)), e => nil)\nprint(deref(a))` → `"1\n"`
  - `big_map_swaps_are_cheap`: 원소 10만 개 맵 atom에 키 하나 바꾸기 `swap` 1,000번 → 2초 미만. 그 맵에 채널 하나 → E0708. 지우면 다시 됨
  - `the_effect_table`: `crate::boot`가 만든 prelude와 `_native.*` 모듈을 훑어 `effect` 표시가 Global Constraints의 목록과 같은지 본다(이름 목록 상수와 비교)
  - golden 셋(`atoms.fung`, `err_atom_effect.fung`, `err_atom_holds.fung`)
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm atom_tests` → `atom`이 없어 FAIL
- [ ] **Step 3: 구현한다.** `FUNG_UPDATE_SPEC=1 cargo test spec_file`
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm atom_tests`, `cargo test -p fung-vm scope_tests`, `cargo test -p fung-cli --test golden` → `atom_tests`·`scope_tests` PASS, golden은 에이전트 파일만 실패(Task 3)
- [ ] **Step 5: 커밋한다.** `feat(vm): strict atoms`

### Task 3: `agent`를 지운다

**Files:**
- Delete: `extras/agent.fung`
- Modify:
  - `crates/fung-vm/src/vm/agent.rs`
    - 서버가 쓰는 `spawn_background`·`try_call`·`raise`·`set_asker`·`describe`와 `Caught`는 `vm/catch.rs`(새 파일)로 옮긴다.
    - 나머지(`Agent`, `agent_handle`, `check_swap`, `agent_ask`, `agent_busy`, `agent_peek`, `Mark`, `send_unchecked`)는 지운다.
  - `Task.serving`과 그 전파를 지운다.
  - `Value::Agent`와 그 갈래를 지운다(format·compare·release·channel의 `contains`).
  - `crates/fung-vm/src/conc.rs`의 `MODULE_ONLY`·표
  - `crates/fung-front/src/extras.rs`, `crates/fung-vm/src/boot.rs`(에이전트 파일 부팅), `crates/fung-front/src/resolve/fn_.rs`(`"agent"` 타입 이름 삭제)
  - 옮겨 쓰기:
    - `git grep -n "agent("`가 찾는 모든 fung 코드(시험, 골든, 예제, 매뉴얼 예제)를 `atom(`으로 바꾼다.
    - `swap`의 `f`가 효과를 내던 곳(`f` 안의 `print`, `spawn`, 다른 에이전트 `swap`)은 효과를 `swap` 밖으로 옮긴다.
    - 에이전트에만 있던 동작(배경 태스크로서의 에이전트, E0704, `swap` 안의 `deref`, 요청 취소, 핸들 닫개)을 시험하던 사례는 지우고 목록을 커밋 메시지에 적는다.
    - 기계적인 부분은 서브에이전트에 나눠 맡겨도 된다(파일이 겹치지 않게).
- Test: 기존 시험 전부

**Interfaces:**
- Consumes: Task 2의 atom
- Produces: `agent`는 없다(E0204). `_native.conc`는 `spawn_background`·`try_call`·`raise`·`set_asker`·`describe`만 남는다.

- [ ] **Step 1: 실패하는 테스트를 쓴다.** `atom_tests::agent_is_gone`: `agent(1)` → E0204
- [ ] **Step 2: 실패를 확인한다.** → FAIL(`agent`가 아직 있음)
- [ ] **Step 3: 지우고 옮겨 쓴다.** 묶음마다 테스트, bless, diff 검토
- [ ] **Step 4: 확인한다.** `git grep -n "agent(\|Value::Agent\|agent_" -- '*.rs' '*.fung' '*.html'` → 역사 문서 밖에서 없음. Run: `cargo test -p fung-vm`, `cargo test -p fung-cli` → PASS(`http` 포함)
- [ ] **Step 5: 커밋한다.** `feat!: agent is gone; atom is the shared state`

### Task 4: 문서

**Files:**
- Modify:
  - `docs/design/05-concurrency.md`: 원시 연산 표의 공유 상태 행, "에이전트와 구현" 절을 atom으로 다시 쓴다(E0704·배경 태스크 서술 정리, 서버가 쓰는 배경 태스크는 남음)
  - `docs/design/06-native-and-stdlib.md`: prelude 목록, extras 표의 `agent`
  - `docs/design/02-values-and-types.md`·`07-operators-and-types.md`: 타입 이름
  - `docs/design/open-questions.md`: L5 완료. 3b의 에이전트 한계 항목들을 닫는다.
  - 매뉴얼
    - `docs/builtins.html`: `atom`·`deref`·`swap`·`reset` 항목, `agent` 삭제, 함수 수
    - `docs/internals.html`: 에이전트 절을 atom으로 바꾼다(비교 후 교체, 순수성 검사, O(1) 표시)
    - `docs/syntax.html`
  - `CHANGELOG.md` `[Unreleased]`
    - 호환을 깨는 변경: `agent` → `atom`, `f`의 순수성, 담을 수 없는 값
    - 옮겨 쓰기: `agent(` → `atom(`, `f` 안의 효과는 밖으로
    - 더함: `reset`, E0707·E0708
  - `docs/notes/R7-language.md`: L5 절. 다룰 것:
    - 0.1 atom → 3b agent → L5 atom으로 돌아온 경위
    - 비교 후 교체와 다시 돌기
    - 효과 표시
    - O(1) 표시를 영속 벡터 노드에 둔 이유
    - 갱신 태스크 하나가 `reset`하는 패턴(토큰 갱신)
- Test: `cargo test -p fung-cli --test manual --test changelog --test repo_rust_only`

- [ ] **Step 1: 고친다.** 매뉴얼 예제는 `fung check`·`fmt --check`·실행을 통과해야 한다.
- [ ] **Step 2: 확인한다.** 위 Test 명령 → PASS
- [ ] **Step 3: 커밋한다.** `docs: atoms in the design, the manual and the changelog`
