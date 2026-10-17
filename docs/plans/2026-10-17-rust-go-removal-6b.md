# Rust 이식 6b단계: Go 삭제 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Go 구현과 Go에 기대는 장치를 모두 걷어내고, Go와 대조해 쌓은 사례는 Go 이름표 없는 "기대 출력"으로 남겨 Rust 하나로 모든 검사가 돈다. 동작은 바꾸지 않는다.

**Architecture:** Go가 있는 동안(Task 1~4) 출력을 기록하고 Go가 지키던 검사를 옮긴 뒤, Go를 지우고(Task 5), `extras`·내장 함수 명세를 하나로 합치고(Task 6), 현재 상태 문서를 고친다(Task 7).

**Tech Stack:** Rust 1.94, fung, sh, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-10-17-rust-go-removal-6b-design.md`

## Global Constraints

- **동작 불변**: Rust의 출력·종료 코드는 바뀌지 않는다(얼린 기대값이 그대로 맞아야 한다). Go 흉내 동작 정리는 6c.
- **이름표**: 새로 만드는 테스트·파일·디렉터리·주석에 Go·parity라는 말을 쓰지 않는다(기대 출력은 "expected"). 기존 Go 흉내 코드의 주석·이름은 6c.
- 크레이트 허용 목록·`fung-front` 0개·`fung-cli`는 작업 공간 크레이트만(그대로).
- 커밋 전(Task 5 전까지): `gofmt -l .`, `go vet ./...`, `go test ./...`, `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`(디버그·`--release`), `FUNG_REQUIRE_PARITY=1 cargo test --locked --no-fail-fast`(디버그·`--release`).
  Task 5부터: Go 줄과 `FUNG_REQUIRE_PARITY`를 뺀 같은 목록.
- 코드·주석·커밋 메시지는 영어, 문서는 한국어. Conventional Commits, 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. 다른 이메일 금지. 자기 경로만 명시적으로 `git add`. 푸시하지 않는다.
- 학습 노트(M·R)·`docs/plans`·`docs/superpowers`·CHANGELOG 0.1.0 항목은 역사 기록이라 고치지 않는다.

## Review Focus

1. 얼린 기대값의 사례 수가 얼리기 전 Go 대조의 사례 수와 같다(사례가 조용히 빠지지 않음) → Task 1, 2.
2. 기대값이 Rust 출력이 아니라 Go가 낸 출력으로 기록됐다(기록 시점에 둘이 같음을 확인한 뒤 Go 쪽을 기록) → Task 1.
3. Go를 지운 뒤 저장소 어디에도 `.go`·`go.mod`·Go 도구 호출(`go run`·`go test`·`gofmt`·`setup-go`)이 남지 않는다(역사 문서 제외) → Task 5, 테스트로.
4. `install.sh`·CHANGELOG↔사이드바 버전 짝·명세 파일 일치가 Go 없이도 검사된다 → Task 3, 4.
5. 하나로 합친 내장 함수 명세(`spec/builtins.txt`)가 Rust 실제 표와 같고, `conc`·`agent`·`http`가 `extras/`에서 바로 읽힌다 → Task 6.

---

### Task 1: 결정적 출력을 기대값으로 얼리기

**Files:** Modify `crates/fung-cli/tests/{parity,run_parity,tools_parity,json_parity,time_parity,regex_parity}.rs`(+ `common/mod.rs`), Create `crates/fung-cli/tests/expected/…`

- [ ] **Step 1:** 각 테스트의 Go 대조 사례를 "기록 → 파일" 형태로 바꿀 수 있게: 일회용 기록 경로(Go를 돌려 출력·종료 코드를 파일로; 기록하면서 Rust 출력과 같은지 단언)와 기본 경로(파일과 Rust 비교). 사례 수를 테스트가 단언(Review Focus 1).
- [ ] **Step 2:** 기록을 실행해 파일을 만들고, 기본 경로가 통과하는지 확인. 실행마다 달라지는 값(임시 경로·포트·시간)은 지금 하는 치환을 그대로.
- [ ] **Step 3:** 테스트·파일 이름은 Go·parity 없이(예: `dump_expected.rs`, `run_expected.rs`, `tests/expected/dump/…`). 기록 경로는 Task 5에서 지운다는 주석.
- [ ] **Step 4:** 커밋 전 검사 전체 / Expected: PASS.
- [ ] **Step 5: 커밋** — `test(rust): freeze expected outputs for the comparison suites`

### Task 2: 네트워크·신호 검사를 Rust 단독으로

**Files:** Modify `crates/fung-cli/tests/{http_parity,tls,sigpipe}.rs`(이름도 Go 없이), 필요하면 `crates/fung-vm`의 테스트 도우미

- [ ] **Step 1:** Go 프로그램을 띄우던 부분을 떼고, 이미 고정한 기대값(요청 원문·응답·에러 문구·신호 13·헤더)으로 Rust만 검사. Go 서버를 상대로 하던 서버 쪽 비교는 Go 서버의 응답을 기대값으로 얼려 Rust 서버 응답과 비교(`Date`는 가림). 사례 수 유지를 단언.
- [ ] **Step 2:** 안정성: 부하를 걸고 15회.
- [ ] **Step 3: 커밋** — `test(rust): check networking and signals against fixed expectations`

### Task 3: 명세 파일을 Rust에서

**Files:** Create `crates/fung-front/tests/spec_files.rs`(또는 기존 테스트에), Modify `spec/builtins.txt`·`errors.txt`의 머리 주석(생성 명령)

- [ ] **Step 1:** Rust 내장 함수 표·에러 카탈로그에서 두 파일을 만드는 코드, 파일과 같은지 검사하는 테스트, 갱신 방법(`FUNG_UPDATE_SPEC=1 cargo test …` 또는 작은 바이너리). 지금은 Go 기준 파일과 Rust 차이 목록이 공존하므로, Rust가 만드는 내용 = 지금 Go 파일 + Rust 차이 목록 적용 결과가 되는지 먼저 확인(차이는 Task 6에서 정리).
- [ ] **Step 2:** `unicode.txt`·`regex_unicode.txt`·`regex_go_fixtures.rs`는 고정 데이터로 두고 머리 주석에서 Go 생성 문구를 "고정 데이터, 다시 만드는 법은 open-questions"로(이름 `regex_go_fixtures`는 6c).
- [ ] **Step 3: 커밋** — `build(rust): generate the spec files from Rust`

### Task 4: Go 테스트가 지키던 검사 옮기기

**Files:** Create `crates/fung-cli/tests/{install_script,changelog}.rs`
- 참조: `tests/install_test.go`(가짜 릴리스로 `install.sh`), `tests/changelog_test.go`(CHANGELOG 절·매뉴얼 사이드바 버전 짝)

- [ ] **Step 1:** 두 Go 테스트를 Rust 통합 테스트로 옮긴다(같은 사례). `install.sh`는 셸로 실행, 가짜 릴리스는 테스트가 만든다.
- [ ] **Step 2:** Go 테스트와 Rust 테스트가 둘 다 통과.
- [ ] **Step 3: 커밋** — `test(rust): check install.sh and the changelog from Rust`

### Task 5: Go 삭제

**Files:** Delete `cmd/`, `internal/`, `go.mod`(·`go.sum`), `tests/*.go`, `extras/*.go`, `tests/lang`의 `# only: go` 파일; Modify Task 1·2의 기록 경로 삭제, `common/mod.rs`(`go_fung`·`FUNG_REQUIRE_PARITY` 삭제), `.github/workflows/ci.yml`·`release.yml`(Go 작업·`setup-go`·Go 테스트 삭제), `scripts/*`, `bench/README.md`(Go 빌드 줄), 골든 실행기·`tests/lang` 처리에서 Go 관련 분기, Create 저장소 검사 테스트(Review Focus 3: `.go`·`go.mod`·Go 도구 호출이 역사 문서 밖에 없음)

- [ ] **Step 1:** 저장소 검사 테스트를 먼저(실패 확인).
- [ ] **Step 2:** 삭제·수정.
- [ ] **Step 3:** 커밋 전 검사(Go 없는 목록) / Expected: PASS. 골든·기대값 사례 수가 그대로.
- [ ] **Step 4: 커밋** — `build: remove the Go implementation`

### Task 6: `extras`·명세 하나로, `# only:` 제거

**Files:** Move `extras/rust/{agent,conc,http}.fung` → `extras/`(Go의 `extras/http.fung` 대체), Modify `extras/prelude.fung`(`fan_in`·`pmap`·`retry` 삭제), `crates/fung-front/src/{builtins,extras,loader}.rs`(`RUST_PRELUDE_FUNG_MOVED`·`RUST_MODULE_MEMBERS` 등 Rust 차이 목록 삭제, 경로), `crates/fung-vm/src/boot*`, `spec/builtins.txt`(재생성: Rust가 유일한 기준), `tests/lang`(`# only: rust` 표시 제거), 골든 실행기(`# only:` 장치 삭제 — 알 수 없는 지시어는 에러 유지), `manual.rs`·예제 검사의 관련 분기

- [ ] **Step 1~3:** 이동·삭제·재생성, 매뉴얼 검사·골든·기대값 통과(동작 불변).
- [ ] **Step 4: 커밋** — `refactor(rust): fold the Rust-only extras and builtin lists into the single spec`

### Task 7: 문서

**Files:** Modify `CLAUDE.md`(Rust 단일 구현: 구조·커밋 전 검사·저장소 규칙; Go 줄 삭제), `docs/design/09-implementation-plan.md`·`background.md`(원칙 4)·`README.md`·`docs/design/open-questions.md`(6b 완료, 대조가 기대값으로, 6c 입력 목록), 설계 00~08에서 Go 삭제에 직접 걸린 서술(나머지 Go 흔적은 6c), Create `docs/notes/R6b-go-removal.md`

- [ ] **Step 1:** 학습 노트(한국어): 무엇을 지웠나, 대조를 기대값으로 얼린 방법과 이유, 명세 파일의 출처 이동, 남은 Go 흔적과 6c.
- [ ] **Step 2:** 문서 수정, 커밋 전 검사.
- [ ] **Step 3: 커밋** — `docs: record Rust port stage 6b`
