# Rust 이식 3b단계: atom을 agent로 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rust 구현에서 `atom`을 없애고 같은 이름의 `agent`·`swap`·`deref`(상태를 쥔 배경 태스크 + 채널)로 대신하며, 채널에 자기 자신을 담아 보내는 것을 E0705로 막고, Go와 갈라지는 테스트를 `# only:` 표시와 Rust 골든 실행기로 다룬다.

**Architecture:** 갈라지는 테스트는 파일 맨 위 `# only: go|rust`로 표시하고 Go·Rust 골든 실행기와 Go 대조가 그 표시를 따른다. 에이전트는 Rust만 읽는 `extras/agent.fung`(fung 코드)와 Rust 전용 내부 도우미(`_native.conc`)로 만들며, 핸들은 마지막 사본이 사라지면 요청 채널을 닫는 불투명 값(닫개)이다. Go 기준 명세(`spec/builtins.txt`)와 Rust의 차이는 Rust 코드 한 곳의 차이 목록이 맡는다.

**Tech Stack:** Rust 1.94 표준 라이브러리만, Go 1.27(골든 실행기·카탈로그만 고침).

**Spec:** `docs/superpowers/specs/2026-10-15-rust-agent-3b-design.md`

## Global Constraints

- Rust 크레이트 의존성 0개. 모든 크레이트 `edition = "2024"`.
- Go 구현의 **언어 동작**(`cmd/`, `internal/vm`, `internal/prelude`, `extras/`의 기존 파일 등)은 바꾸지 않는다. 고쳐도 되는 Go 코드는 `internal/golden`(`# only:`), `tests/*_test.go`, `internal/diag/catalog.go`(E0704·E0705 추가)뿐이다.
- 커밋 전: `gofmt -l .`(빈 출력), `go vet ./...`, `go test ./...`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`(디버그와 `--release`),
  `FUNG_REQUIRE_PARITY=1 cargo test`, `FUNG_REQUIRE_PARITY=1 cargo test --release`.
- 코드·주석·에러 문구·커밋 메시지는 영어, 학습 노트는 한국어. Conventional Commits. 커밋 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- git 작성자는 저장소 설정 그대로. 다른 이메일 금지.
- **실행권 규칙**(3a): fung 값(`Rc`를 품은 모든 것)을 만들고·읽고·복제하고·해제하는 일은 실행권을 쥔 스레드만 한다. `unsafe`에는 `// SAFETY:` 근거.
- 표시 없는 `tests/lang` 파일은 Go 대조에서 Go와 바이트 단위로 같다. 새 에러 번호는 E0704 `agent called from inside its own swap`, E0705 `channel sent into itself` 둘뿐.
- 동시성 테스트는 고정 `sleep`이 아니라 상태로 순서를 맞춘다(`crates/fung-vm/src/testutil.rs`의 탐침).
- Go를 흉내 내려고만 붙이는 코드, Go와의 의도적 차이는 `docs/design/open-questions.md`에 기록한다.

## Review Focus

1. `f` 안의 에러가 부른 쪽 `swap(` 위치에서 원래 번호·문구로 나고 상태는 그대로다 — 에이전트 태스크의 위치나 호출 경로가 새어 나오면 안 된다 → Task 4.
2. 모듈 최상위·전역에 에이전트가 있는 프로그램이 거짓 E0702 없이, 멈추지 않고 끝나며 `at_exit`에서 `deref`가 된다 → Task 3, Task 4.
3. 일반 태스크가 아무도 보내지 않는 채널에서 막히면 에이전트가 살아 있어도 E0702가 난다(배경 태스크가 교착 감지를 가리면 안 됨) → Task 3.
4. E0705 검사가 아주 깊은 값(백만 단계 중첩 리스트)에서 스택을 넘치지 않는다 → Task 5.
5. 핸들이 모두 사라진 에이전트는 실행 중에 끝난다(태스크가 쌓이지 않음) → Task 3, Task 4.

---

### Task 1: `# only:` 표시, Go 골든 실행기, 새 에러 번호

**Files:**
- Modify: `internal/golden/golden.go`(`Case.Only`, `# only:` 파싱·검증), `internal/golden/*_test.go`, `tests/golden_test.go`(`only == "rust"`면 건너뜀),
  `internal/diag/catalog.go`(E0704·E0705), `spec/errors.txt`(재생성), `crates/fung-cli/tests/run_parity.rs`(`# only:` 파일 건너뜀)
- Go 참조: `internal/golden/golden.go`의 기존 지시 파싱

**Interfaces:**
- Produces: `# only: go` / `# only: rust` 지시. 파일 어디에 있어도 되지만 한 번만, 값은 `go`·`rust`뿐(다른 값·중복은 파싱 에러).
  `golden.Case.Only string`("", "go", "rust"). 카탈로그 `E0704 agent called from inside its own swap`, `E0705 channel sent into itself`(E07 동시성 묶음).

- [ ] **Step 1: 실패하는 테스트** — `internal/golden`의 파싱 테스트에 다음을 더한다:
```go
func TestOnlyDirective(t *testing.T) {
	c, err := golden.Parse("x.fung", []byte("# only: rust\nprint(1)\n# expect: 1\n"))
	if err != nil || c.Only != "rust" {
		t.Fatalf("got %q, %v", c.Only, err)
	}
	for _, bad := range []string{"# only: java\n", "# only: go\n# only: go\n"} {
		if _, err := golden.Parse("x.fung", []byte(bad)); err == nil {
			t.Fatalf("%q: want an error", bad)
		}
	}
}
```
  `tests/spec_test.go`가 `spec/errors.txt`의 최신 여부를 검사하므로, 카탈로그를 고친 뒤 갱신 전 실패를 확인한다.
- [ ] **Step 2: 실패 확인** — Run: `go test ./internal/golden ./tests` / Expected: FAIL.
- [ ] **Step 3: 구현** — 파싱·검증, `tests/golden_test.go`에서 `c.Only == "rust"`면 `t.Skip`. 카탈로그 두 줄, `go test ./tests -run TestSpecFiles -update`로 재생성.
  `run_parity.rs`는 `tests/lang` 파일에 `# only:`가 있으면 비교하지 않는다(최소 건수 검사는 그만큼 고려).
- [ ] **Step 4: 통과 확인** — Run: 커밋 전 검사 전체 / Expected: PASS. (아직 `# only:`를 단 파일은 없다.)
- [ ] **Step 5: 커밋**
```bash
git add internal/golden tests spec crates/fung-cli/tests/run_parity.rs
git commit -m "feat: mark golden files that only one implementation runs"
```

---

### Task 2: Rust 골든 실행기

**Files:**
- Create: `crates/fung-cli/tests/golden.rs`(+ 필요하면 `crates/fung-cli/tests/common/golden.rs`)
- Go 참조: `internal/golden/golden.go`(`Parse`·`Check`), `tests/golden_test.go`

**Interfaces:**
- Consumes: Task 1의 지시 형식.
- Produces: `cargo test -p fung-cli --test golden` — `tests/lang`의 `*.fung` 중 `# only: go`가 아니고 `run_parity_skip.txt`의 `native-module`에도 없는 파일을 Rust CLI(`fung run <file>`, `# args:`·`# stdin:`)로 돌려 Go `Check`와 같은 규칙으로 판정한다(표준 출력이 `# expect` 줄들과 같음, `# expect-error`면 그 번호가 표준 에러에 있고 종료 코드 1, 없으면 표준 에러가 비고 종료 코드는 `# expect-exit` 또는 0). Go가 없어도 돈다.

- [ ] **Step 1: 실패하는 테스트** — 실행기를 쓰기 전에, 실행기 자신을 검사하는 작은 단위 테스트(파싱한 `Case`와 가짜 결과로 판정)를 먼저 쓴다. 예: 기대 출력이 다르면 실패, `# expect-error: E0702`인데 표준 에러에 없으면 실패, `# only: go`면 건너뜀.
- [ ] **Step 2: 실패 확인** — Run: `cargo test -p fung-cli --test golden` / Expected: 컴파일 실패.
- [ ] **Step 3: 구현** — Go의 판정 규칙을 그대로 옮긴다. 실패하면 파일 이름과 처음 갈라진 줄을 보인다. 최소 건수(지금 `tests/lang`에서 대상이 되는 파일 수보다 하나 적게)를 단언해 빈 통과를 막는다.
- [ ] **Step 4: 통과 확인** — Run: `cargo test -p fung-cli --test golden` / Expected: PASS(지금 Rust는 대상 파일 전부를 통과해야 한다. 실패하면 Rust가 아니라 실행기를 의심하고 Go 실행기와 대조한다).
- [ ] **Step 5: 커밋**
```bash
git add crates/fung-cli/tests
git commit -m "test(rust): check golden files against their expectations without Go"
```

---

### Task 3: 배경 태스크와 에이전트 도우미

**Files:**
- Create: `crates/fung-vm/src/vm/agent.rs`(에이전트 핸들·닫개·도우미), `crates/fung-vm/src/vm/agent_tests.rs`
- Modify: `crates/fung-vm/src/value.rs`(`Value::Agent`), 서식(`<agent>`)·`type_of`(`'agent`)·`==`(같은 에이전트), `value/release.rs`, `crates/fung-vm/src/vm/task.rs`(배경 태스크), `vm/deadlock.rs`(배경 태스크는 집계 밖), `vm/runtime.rs`(종료 순서: 일반 태스크 대기 → `at_exit` → 배경 태스크 취소·대기 → 꺼내지 않은 실패 보고), `crates/fung-cli/src/run.rs`·`testutil.rs`(같은 순서), `crates/fung-vm/src/conc.rs`(`_native.conc`에 내부 도우미 등록)

**Interfaces:**
- Produces (`_native.conc`의 Rust 전용 멤버. `extras/agent.fung`만 쓴다. 이름은 이대로):
  - `spawn_background(f)` — 배경 태스크로 `f`를 띄우고 태스크 핸들을 돌려준다. 배경 태스크는 프로그램이 끝나기를 막지 않고, 교착 집계(살아 있는 수·막힌 수)에 들지 않으며, `at_exit` 뒤에 취소된다. 배경 태스크의 실패는 꺼내지 않은 실패로 보고하지 않는다(에이전트는 실패하지 않게 짠다).
  - `agent_handle(ch)` — 요청 채널 `ch`를 품은 `Value::Agent`를 만든다. 마지막 사본이 해제되면(실행권 아래) `ch`를 닫는다.
  - `agent_channel(a)` — 핸들 안의 요청 채널.
  - `try_call(f, x)` — `f(x)`를 부르고 `Ok(값)` 또는 `Err(잡은 에러)`(내부 값: 번호·문구를 그대로 다시 낼 수 있는 불투명 값)를 돌려준다. `exit`와 취소(E0701)는 잡지 않고 그대로 통과시킨다.
  - `raise(e)` — `try_call`이 잡은 에러를 **부른 쪽의 위치에서** 같은 번호·문구로 낸다(원래 위치·호출 경로는 버린다).
  - `send_unchecked(ch, v)` — E0705 검사 없이 보낸다(Task 5가 검사를 넣은 뒤에도 에이전트 내부 요청용).
  - `agent_busy(a, on)` / `in_agent(a)` — 에이전트 태스크가 지금 `f`를 돌리는 중인지 표시·확인하고, 지금 태스크가 그 에이전트의 태스크인지 확인(E0704·`f` 안의 `deref`용). 모양은 구현이 다듬어도 되지만 보고서에 적는다.
  - 테스트 전용 탐침: 살아 있는 배경 태스크 수.

- [ ] **Step 1: 실패하는 테스트** (`agent_tests.rs`, `testutil`로 `_native.conc`를 직접 import하는 작은 fung 프로그램)
```rust
#[test]
fn a_background_task_does_not_keep_the_program_alive() {
    // the background task waits forever on a channel nobody sends on
    let o = exec("import _native.conc as c\nch = channel()\nc.spawn_background(() => recv(ch))\nprint(\"done\")", Opts::default());
    assert_eq!((o.stdout.as_str(), o.stderr.as_str(), o.exit), ("done\n", "", 0));
}

#[test]
fn a_blocked_normal_task_is_still_a_deadlock_beside_a_background_one() {
    let o = exec("import _native.conc as c\nc.spawn_background(() => recv(channel()))\nrecv(channel())", Opts::default());
    assert!(o.stderr.starts_with("error[E0702]: deadlock: all tasks are waiting"), "{}", o.stderr);
    assert_eq!(o.exit, 1);
}

#[test]
fn background_tasks_outlive_exit_handlers() {
    let o = exec("import _native.conc as c\nch = channel()\nc.spawn_background(fn(): x = recv(ch)!; print(\"bg got\", x) ;;)\nat_exit(() => send(ch, 7))", Opts::default());
    assert_eq!((o.stdout.as_str(), o.exit), ("bg got 7\n", 0));
}

#[test]
fn dropping_the_last_agent_handle_closes_its_channel() {
    let o = exec("import _native.conc as c\nch = channel()\nt = c.spawn_background(() => recv(ch))\nfn mk() = c.agent_handle(ch)\nmk()\nprint(await(t))", Opts::default());
    assert_eq!(o.stdout, "Err({kind: 'closed})\n");
}
```
  (테스트가 `_native.*`를 import할 수 있게 하는 방법은 기존 `testutil`의 관례를 따른다. 사용자 코드는 `_native.*`를 import할 수 없다 — E0306류 — 그대로 둔다. 필요하면 테스트 전용 진입로를 쓴다.)
  `try_call`·`raise`·`in_agent`의 단위 테스트도 같은 파일에.
- [ ] **Step 2: 실패 확인** — Run: `cargo test -p fung-vm agent_tests` / Expected: FAIL.
- [ ] **Step 3: 구현** — 배경 태스크는 3a의 태스크와 같은 스레드·실행권 구조를 쓰고, 표시 하나로 집계와 종료 순서에서 다르게 다룬다. 닫개의 `Drop`은 실행권 아래에서 채널을 닫는다(3a `Channel::close`와 같은 경로).
- [ ] **Step 4: 통과 확인** — Run: 커밋 전 검사 전체 / Expected: PASS. 디버그 테스트 바이너리 8개 동시 실행 1회도.
- [ ] **Step 5: 커밋**
```bash
git add crates
git commit -m "feat(rust): add background tasks and the helpers agents are built from"
```

---

### Task 4: `agent`·`swap`·`deref`, atom 제거, 테스트 정리

**Files:**
- Create: `extras/agent.fung`(Rust 부팅만 읽음), `tests/lang/agents.fung`·`err_agent_reentry.fung`·`agent_errors.fung`(모두 `# only: rust`), `tests/lang/conc_atoms.fung`(`conc_helpers`에서 떼어 낸 atom 부분, `# only: go`)
- Modify: `crates/fung-front/src/builtins.rs`(Rust의 차이 목록: 빠짐 `atom`·`swap`·`deref` 네이티브, 더함 `extras/agent.fung`의 공개 이름), `crates/fung-vm/src/boot.rs`(agent.fung 컴파일·실행·공개 이름 등록), `crates/fung-vm/src/conc.rs`(atom 네이티브 삭제), 삭제 `crates/fung-vm/src/vm/atom.rs`·`atom_tests.rs`와 `Value::Atom`·서식·비교·`release`·`testutil` 탐침, `tests/lang/atoms.fung`·`err_atom_reentry.fung`(맨 위에 `# only: go`), `tests/lang/conc_helpers.fung`(atom 부분 제거), 스펙 대조 테스트(`prelude_params_match_the_spec` 등)가 차이 목록을 반영하게

**Interfaces:**
- Consumes: Task 3의 `_native.conc` 도우미, Task 1·2의 `# only:`·골든 실행기.
- Produces: prelude `agent(v)`, `swap(a, f)`, `deref(a)` — 스펙 2절의 동작. `swap`·`deref`에 에이전트가 아닌 값을 주면 E0403 문구(`a must be agent`).

`extras/agent.fung`이 지켜야 할 것(코드는 Task 3 도우미의 실제 모양으로 쓴다):
- `agent(v)`: 요청 채널(버퍼 0)을 만들고, `c.spawn_background`로 `_serve(ch, v)`를 띄운 뒤 `c.agent_handle(ch)`를 돌려준다. 에이전트 태스크는 핸들을 붙잡지 않는다(붙잡으면 닫개가 영영 해제되지 않는다) — 채널만 쥔다.
- 요청은 `('swap, f, reply)` 또는 `('deref, reply)`이고 `c.send_unchecked`로 보낸다. `reply`는 요청마다 새로 만든 버퍼 1 채널.
- `_serve(ch, state)`는 꼬리 재귀: `recv(ch)`가 `Err(_)`(닫힘)면 끝, `'swap`이면 "지금 `f`를 돈다" 표시 아래에서 `c.try_call(f, state)` → `Ok(n)`이면 `Ok(n)` 답장 후 `_serve(ch, n)`, `Err(e)`면 `Err(e)` 답장 후 `_serve(ch, state)`. `'deref`면 `state` 답장.
- `swap(a, f)`: 지금 태스크가 `a`의 에이전트 태스크면(그 `f` 안) E0704. 아니면 요청을 보내고 답을 받아 `Ok(n)`이면 `n`, `Err(e)`면 `c.raise(e)`.
- `deref(a)`: 지금 태스크가 `a`의 에이전트 태스크면 바꾸기 전 상태(그 에이전트가 표시해 둔 값)를 바로 돌려준다. 아니면 요청을 보내고 답을 받는다.
- 에이전트가 아닌 값: `swap`·`deref`의 `a`에 대해 E0403 `a must be agent`(Rust 쪽 검사 또는 fung `fail`이 아닌 E0403 문구가 되도록).

- [ ] **Step 1: 실패하는 골든 테스트** — `tests/lang/agents.fung`:
```
# only: rust
counter = agent(0)
each(for _ in 0..8: spawn(() => each(0..50, _ => swap(counter, n => n + 1))) ;;, await)
print(deref(counter))
# expect: 400
a = agent(1)
print(swap(a, x => x + deref(a)), type_of(a), a, a == a, a == agent(1))
# expect: 2 'agent <agent> true false
t = spawn(() => swap(a, x => x / 0))
print(await(t) |> is_err, deref(a))
# expect: true 2
print(swap(a, x => x + len([a])))
# expect: 3
at_exit(() => print("at exit", deref(counter)))
# expect: at exit 400
```
  `tests/lang/err_agent_reentry.fung`(`# only: rust`, `a = agent(0)\nswap(a, x => swap(a, y => y))`, `# expect-error: E0704`),
  `tests/lang/agent_errors.fung`(`# only: rust`, `a = agent(1)\nswap(a, x => x / 0)`, `# expect-error: E0502`) — 그리고 `fung-vm` 단위 테스트로 그 에러의 위치가 사용자 `swap(`(줄·열)이고 호출 경로에 에이전트 태스크 줄이 없는지, 핸들을 모두 버린 에이전트가 끝나는지(배경 태스크 수 탐침), `deref`·`swap`에 에이전트가 아닌 값을 준 E0403.
- [ ] **Step 2: 실패 확인** — Run: `cargo test -p fung-cli --test golden` / Expected: FAIL(`agent` 없음).
- [ ] **Step 3: 구현** — agent.fung, 부팅, 차이 목록, atom 삭제, 테스트 파일 정리. `fung fmt --check tests/lang`(Go 바이너리)이 통과해야 한다.
- [ ] **Step 4: 통과 확인** — Run: 커밋 전 검사 전체 / Expected: PASS(Go 골든은 `# only: rust`를 건너뛰고 `atoms`·`conc_atoms`를 그대로 통과, Rust 골든은 `# only: go`를 건너뜀, Go 대조는 표시 없는 파일 전부 같음).
- [ ] **Step 5: 커밋**
```bash
git add extras/agent.fung tests crates
git commit -m "feat(rust): replace atoms with agents"
```

---

### Task 5: E0705 — 채널에 자기 자신 보내기

**Files:**
- Modify: `crates/fung-vm/src/vm/channel.rs`(보내기 전 깊이 훑기), 새 `tests/lang/err_channel_self.fung`(`# only: rust`)
- Test: `crates/fung-vm/src/vm/channel_tests.rs`

**Interfaces:**
- Consumes: Task 3의 `send_unchecked`(에이전트 내부 요청은 검사 안 함).
- Produces: `send(ch, v)`에서 `v` 안에 `ch`가 있으면 E0705(위치는 `send(`). 훑는 곳: 리스트(영속 벡터 포함), 맵, 튜플, 태그, 클로저가 붙잡은 값, 에이전트 핸들의 요청 채널. 문자열·숫자·범위 등은 건너뜀. **반복으로** 훑는다(재귀 금지 — Review Focus 4). 같은 노드를 두 번 훑지 않는다(공유된 하위 구조).

- [ ] **Step 1: 실패하는 테스트**
```rust
#[test]
fn a_channel_inside_what_is_sent_is_e0705() {
    for src in [
        "ch = channel(1)\nsend(ch, ch)",
        "ch = channel(1)\nsend(ch, [1, {k: (2, ch)}])",
        "ch = channel(1)\nf = () => ch\nsend(ch, f)",
    ] {
        let o = exec(src, Opts::default());
        assert!(o.stderr.starts_with("error[E0705]: channel sent into itself"), "{src}: {}", o.stderr);
    }
}

#[test]
fn other_channels_and_deep_values_are_fine() {
    let o = exec("a = channel(1)\nb = channel(1)\nsend(a, [b])\nfn nest(x, n) = n == 0 then x else nest([x], n - 1)\nsend(b, nest(1, 1000000))\nprint(\"ok\")", Opts::default());
    assert_eq!((o.stdout.as_str(), o.exit), ("ok\n", 0));
}
```
  골든 `tests/lang/err_channel_self.fung`(`# only: rust`, `ch = channel(1)\nsend(ch, [ch])`, `# expect-error: E0705`). 에이전트의 `f`가 에이전트를 붙잡은 `swap`이 E0705가 아닌지는 Task 4의 `agents.fung`가 이미 지킨다.
- [ ] **Step 2: 실패 확인** — Run: `cargo test -p fung-vm channel` / Expected: FAIL.
- [ ] **Step 3: 구현** — 명시적 스택으로 훑는다. 비용 측정: 원소 100만 개 리스트를 보내는 시간(보고서에).
- [ ] **Step 4: 통과 확인** — Run: 커밋 전 검사 전체 / Expected: PASS.
- [ ] **Step 5: 커밋**
```bash
git add crates tests/lang
git commit -m "feat(rust): refuse to send a channel into itself"
```

---

### Task 6: 문서

**Files:**
- Create: `docs/notes/R3b-agent.md`
- Modify: `docs/design/open-questions.md`, `docs/design/05-concurrency.md`, `CLAUDE.md`

- [ ] **Step 1: 학습 노트** `docs/notes/R3b-agent.md`(한국어, `R3a-runtime.md`의 문체): 1. 무엇을 바꿨나 2. atom을 없앤 이유(공유 변경 칸, 순환) 3. Erlang의 원칙과 채널로 짠 에이전트(메일박스와의 차이) 4. 배경 태스크와 종료 순서 5. 닫개 — 참조 계수로 자원 수명을 정확히 잡기 6. `f`의 에러를 부른 쪽에서 다시 내기 7. E0705와 깊이 훑기(반복, 비용) 8. Go와 갈라지는 테스트: `# only:`와 Rust 골든 실행기 9. 실제로 부딪힌 함정(진행 기록과 보고서에 있는 것만).
- [ ] **Step 2: open-questions "Rust 이전"** — `atom` 항목 완료 표시, 채널 순환 항목을 "에이전트에 한해, 닫개로 자동 닫힘; 일반 채널은 `close`"로, **"Rust 첫 릴리스(0.2.0)에서 바뀌는 것"** 목록 신설(atom → agent와 옮겨 쓰기, `<agent>` 출력, E0704·E0705, `swap`이 다른 태스크에서 도는 점), 과도기 잔가지 목록에 Rust의 내장 함수 차이 목록(`crates/fung-front/src/builtins.rs`), 옮기는 방식 항목에 3b 완료 한 줄(계획·노트 경로, Rust 골든 실행기 대상 수).
- [ ] **Step 3: 05·CLAUDE.md** — 05 "Rust 구현에서 바뀌는 것"의 atom 줄을 결과로, CLAUDE.md 현재 상태에 3b 한 줄, Rust 저장소 규칙에 `# only:`와 Rust 골든 실행기 한 줄.
- [ ] **Step 4: 전체 검사와 커밋**

Run: 커밋 전 검사 전체 / Expected: PASS.
```bash
git add docs CLAUDE.md
git commit -m "docs: record Rust port stage 3b"
```
