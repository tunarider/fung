# Rust 이식 4c단계: `regex`, 첫 크레이트, SIGPIPE Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 크레이트 허용 목록 장치를 먼저 세우고, `libc`로 SIGPIPE를 Go처럼, `regex` 크레이트 + Go 의미 맞춤 층으로 `_native.regex`를 옮겨 골든 테스트 2개를 비교에 넣는다.

**Architecture:** 허용 목록의 원본은 최상위 `deny.toml`. 표준 라이브러리만 쓰는 저장소 테스트가 `Cargo.lock`을 그 목록과 대조하고, CI가 `cargo-deny`·`cargo-audit`를 돈다.
정규식은 `regex-syntax` AST를 훑어 Go가 거부하는 문법·에러 문구·ASCII Perl 묶음을 맞춘 뒤 `regex` 엔진에 넘긴다.

**Tech Stack:** Rust 1.94, 크레이트 `regex` 1.x(+ `regex-automata`·`regex-syntax`·`aho-corasick`·`memchr`), `libc` 0.2.

**Spec:** `docs/superpowers/specs/2026-10-16-rust-regex-crates-4c-design.md`

## Global Constraints

- 크레이트는 `deny.toml` 허용 목록에 있는 것만. `fung-front`는 의존성 0개. 새 크레이트는 `crates/fung-vm/Cargo.toml`에만. Go 구현(`cmd/`, `internal/`, `extras/`)은 수정하지 않는다.
- 커밋 전: `gofmt -l .`(빈 출력), `go vet ./...`, `go test ./...`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`(디버그와 `--release`),
  `FUNG_REQUIRE_PARITY=1 cargo test`, `FUNG_REQUIRE_PARITY=1 cargo test --release`.
- 코드·주석·에러 문구·커밋 메시지는 영어, 학습 노트는 한국어. Conventional Commits. 커밋 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- git 작성자는 저장소 설정 그대로. 다른 이메일 금지.
- **실행권 규칙**: fung 값(`Rc`를 품은 모든 것)은 실행권을 쥔 스레드만 만지고 해제한다. 프로세스 공용 캐시는 Rust 값만.
- `unsafe` 블록마다 `// SAFETY:` 근거.
- 표시 없는 `tests/lang` 파일은 Go 대조에서 Go와 바이트 단위로 같다. 새 에러 번호를 만들지 않는다.
- Go를 흉내 내려고만 붙이는 코드는 `docs/design/open-questions.md` "Go 동등을 위한 과도기 잔가지"에 기록할 수 있게 보고서에 적고, 코드에 그 목록을 가리키는 주석을 단다.

## Review Focus

1. 한글·여러 바이트 글자가 섞인 입력에서 `\w` `\b` `\d` `\s`와 글자 위치(`start`·`end`)가 Go와 같다 → Task 3, Task 4.
2. 빈 매치를 낼 수 있는 패턴(`a*`, `x*`, `\b`, `^`, `$`)의 `find_all`·`split`·`replace` 결과가 Go와 같다 → Task 4.
3. 잘못된 패턴과 Go·Rust가 받아들이는 범위가 다른 패턴(`(?x)`, `\Z`, `a{1001}`, `\pN` 등)에서 E0515 여부와 문구가 Go와 같다 → Task 3.
4. 받는 쪽이 닫힌 파이프로 출력할 때(표준 출력과 표준 에러 각각, `spawn`한 태스크에서도) Go처럼 신호 13으로 죽고, 파일·자식 프로세스의 EPIPE는 에러 값이다 → Task 2.
5. 허용 목록 밖 크레이트가 `Cargo.lock`에 들어오면(직접이든 따라오든) 저장소 테스트가 실패한다 → Task 1.

---

### Task 1: 크레이트 허용 목록 장치

**Files:** Create `deny.toml`, `crates/fung-cli/tests/deps.rs`(또는 작업 공간에 맞는 자리), Modify `.github/workflows/ci.yml`, `Cargo.lock`(커밋 확인), `.gitignore`(필요하면)

**Interfaces:**
- Produces: `deny.toml`의 `[bans] allow = [...]`가 허용 목록의 유일한 원본. 이 작업에서는 비어 있거나 다음 작업이 넣을 크레이트를 아직 넣지 않는다(각 작업이 크레이트를 들일 때 목록에 함께 더한다). 저장소 테스트가 이 목록을 읽는다.

- [ ] **Step 1: 실패하는 테스트** — 표준 라이브러리만으로 `Cargo.lock`(`[[package]]`의 `name`, 작업 공간 크레이트는 `source`가 없음)과 `deny.toml`의 허용 목록을 읽어 비교하는 함수를 쓰고, (a) 지금 저장소는 통과, (b) 허용 목록 밖 패키지가 든 가짜 `Cargo.lock` 문자열은 실패, (c) `crates/fung-front/Cargo.toml`에 `[dependencies]` 항목이 생기면 실패를 검사한다. 해석기가 없으므로 먼저 실패.
- [ ] **Step 2: 실패 확인** — Run: `cargo test -p fung-cli --test deps` / Expected: FAIL.
- [ ] **Step 3: 구현** — `deny.toml`(`[bans]` 허용 목록·`multiple-versions`, `[licenses]` 필요한 것만, `[sources]` crates.io만, `[advisories]`), 테스트, CI `rust` 작업에 `cargo install --locked cargo-deny@<고정> cargo-audit@<고정>` 후 `cargo deny check`·`cargo audit`(버전은 2주 이상 된 안정판으로 고정, 보고서에 적음).
- [ ] **Step 4: 통과 확인** — Run: 커밋 전 검사 전체 / Expected: PASS.
- [ ] **Step 5: 커밋** — `build(rust): gate crate dependencies with an allowlist`

---

### Task 2: `libc`와 SIGPIPE

**Files:** Modify `crates/fung-vm/Cargo.toml`(`[target.'cfg(unix)'.dependencies] libc = "0.2"`), `deny.toml`(`libc` 추가), 표준 출력·에러에 쓰는 코드(지금 `exit(141)`을 하는 곳을 찾아서), Test `crates/fung-cli/tests/sigpipe.rs`
- Go 참조: Go 런타임 `os/file_unix.go` `epipecheck`, `runtime/signal_unix.go` `sigpipe`, 자식 프로세스 신호 상태(`syscall/exec_unix.go`)

- [ ] **Step 1: 실패하는 테스트** — 받는 쪽을 먼저 닫은 파이프(읽는 끝을 닫은 `Command` stdout/stderr)에 많이 출력하는 fung 프로그램(진입 태스크에서, `spawn` 안에서, 표준 에러로)을 Rust와 Go 바이너리로 돌려 `ExitStatusExt::signal() == Some(13)`인지 비교. 파일에 쓰는 EPIPE류·자식 프로세스 파이프는 에러 값으로 남는지(Go와 같은 출력). 자식 프로세스가 물려받는 SIGPIPE 상태를 Go와 비교(예: `sh -c 'yes | head -1'`을 실행하는 fung 프로그램의 결과).
- [ ] **Step 2: 실패 확인** — Run: `FUNG_REQUIRE_PARITY=1 cargo test -p fung-cli --test sigpipe` / Expected: FAIL(지금은 141 종료).
- [ ] **Step 3: 구현** — EPIPE에서 `libc::signal(SIGPIPE, SIG_DFL)` 후 `libc::raise(SIGPIPE)`, 살아남으면 141. 다른 스레드가 쥔 잠금·실행권과 상관없이 안전한지 보고서에 근거를 적는다.
- [ ] **Step 4: 통과 확인** — Run: 커밋 전 검사 전체 / Expected: PASS.
- [ ] **Step 5: 커밋** — `fix(rust): die by SIGPIPE on closed stdout and stderr like Go`

---

### Task 3: Go 의미 맞춤 층 (`regex_go.rs`)

**Files:** Modify `crates/fung-vm/Cargo.toml`(`regex = "1"`), `deny.toml`(`regex`와 따라오는 크레이트, 필요한 라이선스), Create `crates/fung-vm/src/native/regex_go.rs`(+ 테스트)
- Go 참조: Go 표준 `regexp/syntax`(`parse.go`의 에러 경로와 `ErrorCode` 문구, `perl_groups.go`), `regexp/regexp.go`(`Compile`)

**Interfaces:**
- Produces: `pub(crate) fn compile_go(pat: &str) -> Result<regex::Regex, String>` — `Ok`는 Go와 같은 의미로 맞춘 엔진, `Err`는 Go `regexp.Compile`의 `err.Error()`와 같은 한 줄 문구.

- [ ] **Step 1: 실패하는 테스트** — Go 바이너리 출력으로 기대값을 얻어 하드코딩한다(스크래치패드의 작은 Go 프로그램). 표: (a) Go `ErrorCode`마다 하나 이상의 잘못된 패턴 → 문구가 같다, (b) Go는 거부하고 Rust는 받는 패턴(`(?x)a`, `\Z`, `a{1001}`, Rust 전용 클래스 문법 등) → Go 문구, (c) Go는 받고 Rust는 거부하는 패턴이 있으면 → 받아들여 Go 의미로, (d) `\w` `\d` `\s` `\b` `\B` `[\w]` `[^\d]`가 한글·`é`·숫자 `٣`에서 Go와 같은 매칭, (e) `(?i)`·`(?m)`·`(?s)`·`(?U)`, `\pL`, `\p{Greek}`, 이름 그룹 `(?P<n>)`·`(?<n>)`.
- [ ] **Step 2: 실패 확인** — Run: `cargo test -p fung-vm regex_go` / Expected: FAIL.
- [ ] **Step 3: 구현** — `regex-syntax` AST(`ast::parse::Parser`)로 해석 → 거부 검사·에러 문구 변환 → Perl 묶음을 ASCII로 바꾼 AST(또는 동등한 패턴 문자열)로 엔진 생성. 실패 경로는 패닉 없이.
- [ ] **Step 4: 통과 확인** — Run: 커밋 전 검사 전체 / Expected: PASS.
- [ ] **Step 5: 커밋** — `feat(rust): compile regular expressions with Go semantics`

---

### Task 4: `_native.regex`

**Files:** Create `crates/fung-vm/src/native/regex.rs`(+ 테스트), Create `crates/fung-cli/tests/regex_parity.rs`, Modify `boot.rs`(`regex` 등록, 지원 안 함 목록에서 삭제), `boot/tests.rs`(`native_modules_match_the_spec`에 `_native.regex`), `run_parity_skip.txt`(`regex.fung`·`err_regex.fung` 줄 삭제), 최소 건수, E0805 테스트(지금 `regex`·`http`를 쓰는 곳은 `http`만)
- Go 참조: `internal/native/regex.go`(캐시 256·통째로 비움, `runeCursor`, 결과 맵 모양 `text`·`start`·`end`·`groups`·`named`), `regex_test.go`, Go `regexp.go`(`expand`, `FindAllStringSubmatchIndex`의 빈 매치 규칙, `Split`)

**Interfaces:**
- Consumes: Task 3 `compile_go`.
- Produces: `_native.regex`의 `is_match(s, pat)` `find(s, pat)` `find_all(s, pat)` `replace(s, pat, repl)` `split(s, pat)` — 매개변수 표는 `spec/builtins.txt`와 같다.

- [ ] **Step 1: 실패하는 테스트** — Go 테스트를 옮기고, Go 대조(`regex_parity.rs`, `time_parity.rs`·`json_parity.rs`의 형식)를 더한다: Review Focus 1·2의 입력, 바꿈 문자열 `$1` `${1}` `$name` `${name}` `$$` `$9`(없는 그룹) `$1a` `$`, 인자 타입 에러, 잘못된 패턴의 E0515 출력 전문. 골든 `regex`·`err_regex`가 두 실행기에서 통과. 큰 입력에서 `find_all`이 선형(글자 수 10만에 매치 1만 개가 1초 안).
- [ ] **Step 2: 실패 확인** — Run: `cargo test -p fung-vm regex && FUNG_REQUIRE_PARITY=1 cargo test -p fung-cli` / Expected: FAIL.
- [ ] **Step 3: 구현** — Go `expand`를 직접 옮긴다. 빈 매치 규칙은 Go 대조 결과를 따른다.
- [ ] **Step 4: 통과 확인** — Run: 커밋 전 검사 전체 / Expected: PASS. 커밋하지 않는 무작위 대조(패턴·입력 조합 수천 개)를 돌려 결과를 보고서에 적는다.
- [ ] **Step 5: 커밋** — `feat(rust): add the regex module`

---

### Task 5: 문서

**Files:** Create `docs/notes/R4c-regex-crates.md`, Modify `docs/design/open-questions.md`, `CLAUDE.md`

- [ ] **Step 1: 학습 노트**(한국어, `R4b-time-json.md`의 문체): 1. 무엇을 옮겼나 2. 정규식 엔진의 원리(유한 오토마타, 선형 시간, 역참조가 없는 이유) 3. RE2 계열 안의 차이와 맞춤 층 4. 의존성을 고르는 기준과 공급망 점검(허용 목록, `cargo-deny`·`cargo-audit`, 따라오는 크레이트) 5. 신호와 종료 코드(141과 "신호 13으로 죽음") 6. 실제로 부딪힌 함정(진행 기록·보고서·리뷰에 있는 것만).
- [ ] **Step 2: open-questions "Rust 이전"** — 4c 완료 한 줄, 허용 목록 표(크레이트마다 이유·따라오는 것), 과도기 잔가지(Go 의미 맞춤 층, 에러 문구 변환, Go `expand`), 알려진 차이, 닫힌 파이프 141 항목 해결 표시.
- [ ] **Step 3: CLAUDE.md** — 현재 상태에 4c 한 줄(다음은 4d), 크레이트 규칙 문구(앞단 0개, VM은 `deny.toml` 허용 목록, 저장소 테스트).
- [ ] **Step 4: 전체 검사와 커밋** — `docs: record Rust port stage 4c`
