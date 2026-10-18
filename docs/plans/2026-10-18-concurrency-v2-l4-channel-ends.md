# 동시성 v2 L4 (채널 끝 나누기와 자동 닫힘) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `channel(n)`이 `(tx, rx)` 튜플을 준다. 보내는 끝의 마지막 사본이 사라지면 채널이 닫히고, 받는 끝의 마지막 사본이 사라지면 `send`가 `Err({kind: 'closed})`가 된다.

**Architecture:**
- **끝은 값이다.** `Value::Sender(Rc<Sender>)`·`Value::Receiver(Rc<Receiver>)`이고, 둘 다 같은 `Rc<Channel>`을 쥔다.
  - `channel()`은 끝마다 `Rc`를 하나씩 만든다. fung에서의 "사본"은 그 바깥 `Rc`의 사본이다. 그래서 바깥 `Rc`가 0이 되는 순간(`Drop`)이 곧 "마지막 사본이 사라짐"이다.
  - 따로 세는 수를 두지 않는다. 사본이 어디에 있든(지역 변수, 클로저가 붙잡은 값, 리스트, 다른 채널의 버퍼) Rust의 참조 수가 이미 센다.
- **`Sender::drop`**은 `close()`를 부른다. 받는 쪽은 남은 버퍼를 다 받은 뒤 `Closed`를 받는다(지금의 `close`와 같음).
- **`Receiver::drop`**은 닫고 버퍼를 버린다(`abandon`). 막혀 있던 `send`는 깨어나 `closed`를 본다. 받는 끝이 없으면 아무도 받을 수 없으므로, "닫힘" 표시 하나로 두 경우를 나타내도 겉으로 보이는 차이가 없다.
- **풀어 주기는 재귀하지 않는다** (`value/release.rs`).
  - 마지막 사본인 끝은 자기 일(닫기, 버퍼를 스택으로 옮기기)을 한다.
  - 그 끝이 채널의 마지막 주인이었으면 채널을 `spill`한다.
  - 그래서 "채널 버퍼 안의 다른 채널의 받는 끝 안의 …"가 100만 겹이어도 넘치지 않는다.
- **`for x in rx`·`each`·`map` 등**은 받는 끝만 돈다. 보내는 끝은 E0506이다.
- **E0705**는 같은 채널의 어느 끝이든 그 채널로 보내면 낸다.
  - 버퍼가 자기 보내는 끝을 쥐면 영원히 닫히지 않고, 자기 받는 끝을 쥐면 영원히 버려지지 않기 때문이다.
  - `v.holds_mutable()`이 거짓이면 훑지 않는다(스펙 6절, O(1) 표시).
- **`set_asker`·`try_call`의 `reply`**(http 서버)는 어느 끝이든 받는다.

**Tech Stack:** Rust 표준 라이브러리. `fung-vm`(`vm/channel.rs`, `conc.rs`, `value.rs`·`value/{format,compare,release}.rs`, `native/nat.rs`, `vm/catch.rs`, `testutil.rs`)과 `fung-front`(`resolve/fn_.rs`), `extras/{conc,http}.fung`.

**Spec:** `docs/superpowers/specs/2026-10-18-concurrency-v2-r7-design.md` 6절(채널), 11절, 12절. 순서는 사용자 결정으로 L5 다음이다(2026-10-18). 앞 단계 계획: `docs/plans/2026-10-18-concurrency-v2-l{1,2,3,5}-*.md`.

## Global Constraints

- 사용자 방침(메모리 `plan-execution-style`)을 따른다.
  - 작업마다 TDD로 하고, 그 작업의 테스트만 돌린다. 작업 단위로 커밋한다.
  - 끝에 전체 검사(fmt, clippy와 test를 debug·release 둘 다), 브랜치 전체 검토, 수정을 한 번 한다.
  - 사용자가 계획 쓰기와 실행을 함께 요청했다(2026-10-18).
- 커밋은 Conventional Commits, 영어, 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- 새 `unsafe`는 없다. 끝의 `Drop`은 `token::assert_holds_token()`으로 시작한다(채널과 같음).
- 이름과 겉모습:
  - `channel(n = 0)` → `(tx, rx)`. `n`의 뜻은 그대로다.
  - `send(tx, v)`, `recv(rx)`, `close(tx)`. 매개변수 이름이 `tx`·`rx`로 바뀐다.
  - `type_of`는 `'sender`·`'receiver`, 출력은 `<sender>`·`<receiver>`(설계자 판단), 타입 표기는 `sender`·`receiver`. 타입 이름 `channel`은 지운다.
  - `==`는 같은 채널의 같은 쪽 끝인지 본다(바깥 `Rc`의 동일성).
  - 끝을 잘못 주면 E0403이다(`send(rx, 1)` → argument `tx` must be sender). 새 에러 번호는 없다(스펙 12절의 판단).
  - `close(rx)`도 E0403이다. 받는 쪽은 닫지 않고 버린다.
- E0705 문구 `channel sent into itself`는 그대로다.
- 시험의 탐침 `blocked(ch, n)`·`buffered(ch)`(`testutil.rs`)는 어느 끝이든 받는다.

## Review Focus

- **값을 내는 함수가 받는 끝만 돌려주면, 생산 태스크가 끝날 때 채널이 닫힌다.**
  - `fn numbers(s, n):` 안에서 `(tx, rx) = channel()`, `spawn(s, () => each(0..n, i => send(tx, i)))`, `rx`를 돌려준다.
  - 그러면 `for x in numbers(s, 3)`는 0, 1, 2를 받고 끝난다. 함수가 돌아올 때 지역 `tx`가 사라지고, 태스크가 끝날 때 붙잡힌 `tx`가 사라지기 때문이다.
  - → Task 1의 `a_producer_that_ends_closes_its_channel`.
- **최상위 변수는 끝을 살려 둔다.** 진입 파일 최상위의 `(tx, rx) = channel()` 뒤에 `for x in rx`만 있으면, 멈추지 않고 E0702(교착)로 끝난다. → Task 1의 `a_live_sender_at_top_level_is_a_deadlock`.
- **받는 끝이 사라지면 막힌 보내기가 깨어난다.**
  - 버퍼 없는 채널에서 `send`로 막힌 태스크가 있을 때, 받는 끝의 마지막 사본이 사라지면 그 `send`는 `Err({kind: 'closed})`다.
  - 버퍼에 있던 값은 풀린다. `with`의 해제 같은 부작용은 없지만 메모리는 돌아온다.
  - → Task 1의 `dropping_the_receiver_wakes_a_blocked_sender`.
- **깊은 사슬을 풀어도 넘치지 않는다.** 채널 A의 버퍼에 B의 받는 끝이 있고, B의 버퍼에 C의 받는 끝이 있는 식으로 100,000겹이어도 마지막 사본이 사라질 때 스택이 넘치지 않는다. → Task 1의 `a_deep_chain_of_channels_frees_without_recursion`.
- **응답 채널 패턴은 된다.**
  - 다른 채널의 끝을 보내는 것(`send(req_tx, (x, reply_tx))`)은 E0705가 아니다.
  - 같은 채널의 끝은 리스트, 클로저, 튜플 안에 숨겨도 E0705다.
  - → Task 1의 `sending_an_end_into_its_own_channel_is_e0705`, `reply_channels_work`.

---

### Task 1: 끝 두 개와 자동 닫힘

**Files:**
- Modify:
  - `crates/fung-vm/src/vm/channel.rs`
    - `pub struct Sender(Rc<Channel>)`·`pub struct Receiver(Rc<Channel>)`와 `Drop`, `Channel::abandon(&self, stack: &mut Vec<Value>)`(닫고 버퍼를 옮김)
    - `contains`가 두 끝을 본다
    - 모듈 머리 주석
  - `crates/fung-vm/src/value.rs`: `Value::Channel` → `Value::Sender(Rc<Sender>)`·`Value::Receiver(Rc<Receiver>)`. `holds_mutable`은 둘 다 true
  - `crates/fung-vm/src/value/{format,compare,release}.rs`
  - `crates/fung-vm/src/conc.rs`: `channel_fn`이 튜플을 주고, `need_sender`·`need_receiver`, 매개변수 `tx`·`rx`. E0705 검사 앞에 `holds_mutable` 지름길
  - `crates/fung-vm/src/native/nat.rs`: `check_items`·`each_until`·`items`는 `Receiver`만 받는다. `not_iterable`의 문구는 그대로
  - `crates/fung-vm/src/vm/catch.rs`: `need_channel_named`가 어느 끝이든 받는다
  - `crates/fung-vm/src/testutil.rs`: 탐침이 어느 끝이든 받는다
  - `crates/fung-vm/src/vm.rs`: 다시 내보내기
  - `crates/fung-front/src/resolve/fn_.rs`: `BUILTIN_TYPES`에서 `"channel"`을 빼고 `"sender"`·`"receiver"`를 넣는다
  - `spec/builtins.txt`: `FUNG_UPDATE_SPEC=1 cargo test spec_file`
- Test: `crates/fung-vm/src/vm/channel_tests.rs`(새 시험은 파일 끝의 `// --- L4: ends ---` 아래)

**Interfaces:**
- Produces:
  - `pub struct Sender(pub(crate) Rc<Channel>)`, `pub struct Receiver(pub(crate) Rc<Channel>)`, 둘 다 `Debug`는 `<sender>`·`<receiver>`
  - `impl Value { pub(crate) fn channel_end(&self) -> Option<&Rc<Channel>> }`: 어느 끝이든 그 채널. `catch.rs`·탐침이 쓴다
  - `channel::send(t, ch: &Rc<Channel>, v)`·`recv(t, ch)`·`drain(t, ch)`의 모양은 그대로다(끝을 벗긴 채널을 받음)

- [ ] **Step 1: 실패하는 테스트를 쓴다** (`channel_tests.rs`, `run`/`exec_waiting`)
  - `channel_gives_two_ends`: `(tx, rx) = channel(1)\nprint(tx, rx, type_of(tx), type_of(rx), tx == tx, send(tx, 5), recv(rx))` → `"<sender> <receiver> 'sender 'receiver true Ok(nil) Ok(5)\n"`
  - `the_wrong_end_is_a_type_error`: `send(rx, 1)` → `argument \`tx\` must be sender`; `recv(tx)` → `argument \`rx\` must be receiver`; `close(rx)` → `argument \`tx\` must be sender`; `for x in tx: x ;;` → E0506
  - `end_types_in_annotations`: `fn f(r: receiver) = recv(r)\nfn g(t: sender) = send(t, 1)\n(tx, rx) = channel(1)\ng(tx)\nprint(f(rx))` → `"Ok(1)\n"`. `fn h(c: channel) = c` → 알 수 없는 타입 에러(지금 알 수 없는 타입 이름에 내는 번호)
  - `a_producer_that_ends_closes_its_channel`: Review Focus의 `numbers` → `"0\n1\n2\ndone\n"`(`for` 뒤 `print("done")`), 코드 0
  - `a_live_sender_at_top_level_is_a_deadlock`: `(tx, rx) = channel()\nfor x in rx: print(x) ;;` → stderr에 E0702, 코드 1
  - `close_closes_for_every_copy`: `(tx, rx) = channel(2)\nt2 = tx\nsend(tx, 1)\nclose(t2)\nprint(send(tx, 2), recv(rx), recv(rx))` → `"Err({kind: 'closed}) Ok(1) Err({kind: 'closed})\n"`
  - `dropping_the_receiver_wakes_a_blocked_sender`
    - 범위 안에서 `spawn(s, () => send(tx, 1))`를 띄우고, `blocked`로 막혔는지 확인한다.
    - 그다음 받는 끝을 쥔 유일한 값을 없앤다. 받는 끝을 함수 안의 지역 변수로만 두고 그 함수가 돌아오게 한다.
    - `await`의 결과 → `Err({kind: 'closed})`.
  - `dropping_the_receiver_fails_later_sends`: `fn mk() = channel(1)` 뒤 `(tx, _) = mk()` → `print(send(tx, 1))` → `"Err({kind: 'closed})\n"`
  - `many_workers_share_a_receiver`: 일꾼 3개가 같은 `rx`에서 받아 결과 채널로 보내고, 6개 값의 합 → `"21\n"`
  - `sending_an_end_into_its_own_channel_is_e0705`: `send(tx, tx)`, `send(tx, rx)`, `send(tx, [1, rx])`, `send(tx, () => tx)` → 각각 E0705
  - `reply_channels_work`: 일꾼이 `(x, reply)`를 받아 `send(reply, x * 2)` → 요청한 쪽이 `Ok(42)`를 받는다
  - `a_deep_chain_of_channels_frees_without_recursion`: Rust에서 `with_token`으로 100,000겹 사슬을 만들고 떨어뜨린다(`mutable_tests.rs`의 `with_token` 사용) → 넘치지 않고 끝남
  - `ends_hold_mutable_and_cannot_go_into_atoms`: `atom(channel()[0])`처럼 끝 하나 → E0708. 튜플 인덱싱이 없으면 `(tx, _) = channel()\natom(tx)`로 쓴다
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-vm channel_tests` → FAIL(`channel()`이 끝이 아니라 채널 하나를 줌)
- [ ] **Step 3: 구현한다.**
  - `send`·`recv`·`close`의 매개변수 이름을 바꾸고 `spec_file`을 다시 만든다.
  - 이 단계에서 `channel(`을 쓰는 기존 시험은 깨진다(Task 2·3이 옮긴다).
- [ ] **Step 4: 통과를 확인한다.** Run: `cargo test -p fung-vm channel_tests` → 새 시험 PASS
- [ ] **Step 5: 커밋한다.** `feat(vm)!: channels have a sending and a receiving end`

### Task 2: fung-vm의 시험을 옮긴다

**Files:**
- Modify: `crates/fung-vm/src/**/*_tests.rs`·`tests.rs`·`run_cases.rs`(`channel(`을 쓰는 것 전부. `channel_tests` 61, `catch_tests` 20, `run_cases` 18, `failure_tests` 15, `cleanup_tests` 14, `scope_tests` 12, `atom_tests` 8, `deadline_tests` 7, `repl_tests` 6, `net_tests` 6, `fs_tests` 5, `exit_tests` 4, `boot/tests` 4, `prelude/tests` 3, `task_tests` 2, `m6_tests` 2, `mutable_tests`), `crates/fung-vm/tests/expected/`
- Test: `cargo test -p fung-vm`

**Interfaces:**
- Consumes: Task 1의 끝과 탐침

옮겨 쓰는 규칙:
- `ch = channel(n)` → `(tx, ch) = channel(n)`. 한 시험에 채널이 여럿이면 `gate`→`gate_tx`처럼 짝을 짓는다.
  - `send(ch, …)`·`close(ch)` → 보내는 끝.
  - `recv`·`for`·`each`·탐침 → 받는 끝.
- **영원히 막히는 받기**(`recv(channel())`)는 보내는 끝을 살려 둬야 막힌다. `(_tx, rx) = channel()` 뒤 `recv(rx)`로 쓴다(`_` 접두사로 W0102를 피함). 임시 튜플만 쓰면 보내는 끝이 곧바로 사라져 즉시 `Err({kind: 'closed})`가 된다.
- 시험이 지키던 의미가 자동 닫힘 때문에 바뀌는 사례는 의미를 지키도록 고친다(보통 보내는 끝을 변수로 살려 둠). 지킬 수 없는 사례(채널 하나로 양쪽을 하던 동작)는 지우고 목록을 커밋 메시지에 적는다.
- 기대 출력(`<channel>` 출력 등)은 `FUNG_EXPECTED=bless cargo test -p fung-vm runs_match_the_expected_output`으로 다시 쓰고 diff를 검토한다.

- [ ] **Step 1: 옮겨 쓴다.** 파일 묶음마다 그 시험을 돌린다.
- [ ] **Step 2: 확인한다.** Run: `cargo test -p fung-vm` → PASS
- [ ] **Step 3: 커밋한다.** `test(vm): move the tests to channel ends`

### Task 3: 표준 라이브러리, 골든, 예제, 명령 시험

**Files:**
- Modify:
  - `extras/conc.fung`: `fan_in(s, rxs)`가 받는 끝들을 받아 받는 끝을 돌려준다
  - `extras/http.fung`: `serve`의 표시용 채널(`(asker, _) = channel()`, 이름을 `scope`에서 `asker`로)과 `failed` 채널(`(failed_tx, failed) = channel(1)`, `_acceptor`는 보내는 끝을 받음)
  - `tests/lang/*.fung`(`channel_stream`·`conc_helpers`·`channels`·`timeouts`·`scopes`·`http_serve`·`err_deadlock`·`err_channel_self`·`err_atom_holds`·`cancel`)
  - `examples/{urlcheck,api}.fung`
  - `crates/fung-cli/tests/{run_expected,tools_expected,http,json_expected,cleanup}.rs`
  - `crates/fung-cli/tests/expected/{fmt/stdin.txt,json/programs.txt,run/runs.txt}`와 사례 수 상수
- Test: `cargo test -p fung-cli`

**Interfaces:**
- Consumes: Task 1의 끝. 옮겨 쓰는 규칙은 Task 2와 같다.

- [ ] **Step 1: 실패하는 테스트를 쓴다.** `tests/lang/channels.fung`에 Review Focus의 `numbers` 패턴과 `conc.fan_in`이 받는 끝으로 닫히는 사례를 골든으로 더한다.
- [ ] **Step 2: 실패를 확인한다.** Run: `cargo test -p fung-cli --test golden` → FAIL
- [ ] **Step 3: 옮겨 쓴다.**
  - 기대 출력은 `FUNG_EXPECTED=bless cargo test -p fung-cli --test <이름>`으로 다시 쓰고 diff를 검토한다.
  - Task 2와 파일이 겹치지 않으므로 서브에이전트에 나눠 맡겨도 된다.
- [ ] **Step 4: 확인한다.** Run: `cargo test -p fung-cli` → PASS(`http` 포함)
- [ ] **Step 5: 커밋한다.** `feat!: the standard library, goldens and examples use channel ends`

### Task 4: 문서

**Files:**
- Modify:
  - `docs/design/05-concurrency.md`: 채널 절(끝, 자동 닫힘, `close(tx)`, 순회는 받는 끝, E0705, 원시 연산 표)
  - `docs/design/02-values-and-types.md`·`07-operators-and-types.md`: 타입 이름 `sender`·`receiver`
  - `docs/design/06-native-and-stdlib.md`: `conc.fan_in`
  - `docs/design/open-questions.md`: L4 완료
  - 매뉴얼
    - `docs/builtins.html`: `channel`·`send`·`recv`·`close` 항목과 매개변수 표, `conc.fan_in`
    - `docs/syntax.html`: 채널 예제와 타입 이름
    - `docs/internals.html`: 채널 절(끝과 `Drop`, 풀어 주기)
  - `CHANGELOG.md` `[Unreleased]`: 호환을 깨는 변경(`channel` → `(tx, rx)`, 타입 이름), 옮겨 쓰기 안내, 자동 닫힘
  - `docs/notes/R7-language.md`: L4 절. 다룰 것:
    - 끝을 나누는 이유(누가 닫는지, 닫지 않은 채널의 교착)
    - 참조 수가 곧 사본 수인 점
    - 지역 변수, 클로저, 최상위 변수가 끝을 살려 두는 방식과 그래서 `close(tx)`를 남긴 이유
    - 받는 끝이 사라질 때 버퍼를 버리는 이유
    - E0705가 두 끝을 보는 이유
    - Go·Rust의 채널과의 비교
- Test: `cargo test -p fung-cli --test manual --test changelog --test repo_rust_only`

- [ ] **Step 1: 고친다.** 매뉴얼 예제는 `fung check`·`fmt --check`·실행을 통과해야 한다.
- [ ] **Step 2: 확인한다.** 위 Test 명령 → PASS
- [ ] **Step 3: 커밋한다.** `docs: channel ends in the design, the manual and the changelog`
