# Rust 이식 4b단계: `time`·`json` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Go의 `_native.time`·`_native.json`과 `Time` 값을 Rust로 옮겨 `time`·`json` 모듈이 Go와 같은 출력으로 돌고, 골든 테스트 4개를 비교에 넣는다.

**Architecture:** `Time`은 `Value`의 새 종류(유닉스 나노초 + 오프셋 + 시간대 이름). 지역 시간대는 Go처럼 TZif 파일을 직접 읽는다(`native/tz.rs`). 서식·파싱(`timefmt.rs`)과 json(`json.rs`)은 Go 코드를 함수 단위로 옮기고, json 해석은 반복으로 한다.

**Tech Stack:** Rust 1.94 표준 라이브러리만.

**Spec:** `docs/superpowers/specs/2026-10-16-rust-time-json-4b-design.md`

## Global Constraints

- Rust 크레이트 의존성 0개. Go 구현(`cmd/`, `internal/`, `extras/`)은 수정하지 않는다.
- 커밋 전: `gofmt -l .`(빈 출력), `go vet ./...`, `go test ./...`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`(디버그와 `--release`),
  `FUNG_REQUIRE_PARITY=1 cargo test`, `FUNG_REQUIRE_PARITY=1 cargo test --release`.
- 코드·주석·에러 문구·커밋 메시지는 영어, 학습 노트는 한국어. Conventional Commits. 커밋 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- git 작성자는 저장소 설정 그대로. 다른 이메일 금지.
- **실행권 규칙**: fung 값(`Rc`를 품은 모든 것)은 실행권을 쥔 스레드만 만지고 해제한다.
- 표시 없는 `tests/lang` 파일은 Go 대조에서 Go와 바이트 단위로 같다. 새 에러 번호를 만들지 않는다.
- Go를 흉내 내려고만 붙이는 코드는 `docs/design/open-questions.md` "Go 동등을 위한 과도기 잔가지"에 한 줄씩 더한다.
- 시간대에 기대는 테스트는 `TZ`를 고정해 두 구현을 같은 조건으로 돌린다.

## Review Focus

1. 서머타임 경계(존재하지 않는 시각, 두 번 있는 시각)에서 `parse`·`local`·`parts`가 Go와 같다 → Task 1, Task 3.
2. `TZ`가 없음/빈 문자열/없는 이름/`:`로 시작/절대 경로일 때 Go와 같은 지역 시간대가 고른다 → Task 1.
3. json의 아주 큰 정수(`9223372036854775808`, `1e400`), `-0`, 중복 키, 깊은 중첩(백만 단계)에서 Go와 같은 결과·에러이고 스택이 넘치지 않는다 → Task 4.
4. `stringify`가 실수·이스케이프·순환 없는 깊은 값·Go가 거부하는 값(함수, 채널 등)에서 Go와 같다 → Task 4.
5. 시각 산술의 넘침(`from_unix`의 극단값, `t + 큰 dur`)이 Go와 같은 에러다 → Task 2.

---

### Task 1: TZif 시간대

**Files:** Create `crates/fung-vm/src/native/tz.rs`(+ 테스트)
- Go 참조: Go 표준 `time` 패키지 `zoneinfo_unix.go`(`initLocal`: `TZ` 처리, `/etc/localtime`, `platformZoneSources`), `zoneinfo_read.go`(`LoadLocationFromTZData`: TZif v1/v2+, 끝의 POSIX 규칙), `zoneinfo.go`(`tzset`, `lookup`). `go env GOROOT`로 소스를 찾는다.

**Interfaces:**
- Produces: `pub(crate) struct Zone { name: String, ... }`, `pub(crate) fn local() -> &'static Zone`(프로세스당 한 번), `pub(crate) fn load(name: &str) -> Option<Zone>`, `impl Zone { fn lookup(&self, unix_sec: i64) -> (abbrev: &str, offset_sec: i32) }`, 지역 시각을 유닉스 시각으로 되돌리는 Go `Date`의 정규화 규칙에 필요한 함수(존재하지 않는/두 번 있는 시각의 Go 선택 규칙 포함).

- [ ] **Step 1: 실패하는 테스트** — 저장소 밖의 zoneinfo에 기대지 않도록, 테스트에 필요한 TZif 파일(`Asia/Seoul`, `America/New_York`, `UTC`)은 실행 중인 시스템의 `/usr/share/zoneinfo`에서 읽되 없으면 건너뛴다(macOS·Linux CI 모두 있음). Go 바이너리로 각 시각의 `(약어, 오프셋)`을 확인해 기대값으로 쓴다: 서머타임 시작·끝 앞뒤, 1900년대 과거, 2100년(끝의 POSIX 규칙). `TZ` 처리 표(없음, `""`, `UTC`, `:Asia/Seoul`, `/usr/share/zoneinfo/Asia/Seoul`, 없는 이름)는 Go `initLocal`의 결과와 같아야 한다.
- [ ] **Step 2: 실패 확인** — Run: `cargo test -p fung-vm tz` / Expected: FAIL.
- [ ] **Step 3: 구현** — Go 알고리즘을 옮긴다. 파일 읽기는 프로세스당 한 번(`OnceLock`), 실행권과 무관한 Rust 값만 쓴다.
- [ ] **Step 4: 통과 확인** — Run: 커밋 전 검사 전체 / Expected: PASS.
- [ ] **Step 5: 커밋** — `feat(rust): read time zones from TZif files like Go`

---

### Task 2: `Time` 값과 시각 연산

**Files:** Modify `crates/fung-vm/src/value.rs`(`Value::Time`), `value/format.rs`·`compare.rs`·`arith.rs`·`types.rs`, Test `value/time_tests.rs`
- Go 참조: `internal/value`의 `Time`(서식, `==` `<`, `± dur`, `time - time`, 정규화 차단), 그 테스트.

**Interfaces:**
- Consumes: Task 1 `Zone`.
- Produces: `Value::Time(Rc<Time>)`(16바이트 유지), `Time { unix_nanos: i128 또는 Go와 같은 표현, offset: i32, zone: …}`; ISO 출력과 `type_of`(`'time`)가 Go와 같다.

- [ ] **Step 1: 실패하는 테스트** — Go `internal/value`의 시각 테스트를 옮기고, 넘침(Go가 에러를 내는 `t + 큰 dur`, 극단적 유닉스 값)을 Go 바이너리 출력과 대조.
- [ ] **Step 2: 실패 확인** — Run: `cargo test -p fung-vm time` / Expected: FAIL.
- [ ] **Step 3: 구현**
- [ ] **Step 4: 통과 확인** — Run: 커밋 전 검사 전체 / Expected: PASS.
- [ ] **Step 5: 커밋** — `feat(rust): add time values`

---

### Task 3: `_native.time`과 서식·파싱

**Files:** Create `crates/fung-vm/src/native/time.rs`, `native/timefmt.rs`(+ 테스트), Modify `boot.rs`(`time`을 지원 안 함 목록에서 빼고 등록), `crates/fung-cli/tests/run_parity_skip.txt`(`time_module`·`time_values` 줄 삭제), 최소 건수
- Go 참조: `internal/native/time.go`, `timefmt.go`, `time_test.go`, `timefmt_test.go`

**Interfaces:**
- Consumes: Task 1·2.
- Produces: `_native.time`의 `now` `unix` `from_unix` `utc` `local` `parts` `format` `parse` — 매개변수 표는 `spec/builtins.txt`와 같다.

- [ ] **Step 1: 실패하는 테스트** — Go 테스트를 옮기고, `TZ`를 고정한 Go 대조 사례(`crates/fung-vm/src/prelude/go_cases.rs` 또는 고정 `TZ`로 두 바이너리를 도는 테스트)를 더한다: 모든 서식 토큰, 시간대 토큰 `Z`, 정규화 차단(`2026-02-30`), 서머타임 경계의 `parse`·`local`·`parts`(Review Focus 1). `now()`는 형식·범위만.
  골든 테스트 `time_module`·`time_values`가 두 실행기에서 통과(골든 실행기의 `TZ`는 Go 골든 실행기와 같은 조건 — 둘 다 무엇을 쓰는지 확인).
- [ ] **Step 2: 실패 확인** — Run: `cargo test -p fung-vm time && FUNG_REQUIRE_PARITY=1 cargo test -p fung-cli` / Expected: FAIL.
- [ ] **Step 3: 구현**
- [ ] **Step 4: 통과 확인** — Run: 커밋 전 검사 전체 / Expected: PASS.
- [ ] **Step 5: 커밋** — `feat(rust): add the time module`

---

### Task 4: `_native.json`

**Files:** Create `crates/fung-vm/src/native/json.rs`(+ 테스트), Modify `boot.rs`(`json` 등록), `run_parity_skip.txt`(`json`·`err_json_stringify` 줄 삭제), 최소 건수
- Go 참조: `internal/native/json.go`(순서 유지 토큰 디코더, 정수·실수 구분, E0513, `stringify`의 들여쓰기 인자), `json_test.go`, Go `encoding/json`의 문자열 이스케이프(`encodeState.string`)

**Interfaces:**
- Produces: `_native.json`의 `parse(s)`, `stringify(v, …)` — 매개변수 표는 `spec/builtins.txt`와 같다.

- [ ] **Step 1: 실패하는 테스트** — Go 테스트를 옮기고, Go 바이너리 대조 사례를 더한다: 정수/실수 경계(`1`, `1.0`, `1e2`, `9223372036854775807`, `9223372036854775808`, `1e400`, `-0`), 키 순서, 중복 키, 이스케이프(`<` `>` `&` U+2028 제어 문자, 잘못된 UTF-8), 잘못된 입력의 에러 문구·위치, `stringify`가 거부하는 값(함수, 채널, 에이전트는 Rust 전용이라 Rust 쪽만), 깊은 중첩 백만 단계(1 MB 스레드에서 스택 넘침 없음 — Review Focus 3).
  골든 테스트 `json`·`err_json_stringify`가 두 실행기에서 통과.
- [ ] **Step 2: 실패 확인** — Run: `cargo test -p fung-vm json && FUNG_REQUIRE_PARITY=1 cargo test -p fung-cli` / Expected: FAIL.
- [ ] **Step 3: 구현** — 해석·출력 모두 명시적 스택으로(재귀 금지).
- [ ] **Step 4: 통과 확인** — Run: 커밋 전 검사 전체 / Expected: PASS.
- [ ] **Step 5: 커밋** — `feat(rust): add the json module`

---

### Task 5: 문서

**Files:** Create `docs/notes/R4b-time-json.md`, Modify `docs/design/open-questions.md`, `CLAUDE.md`

- [ ] **Step 1: 학습 노트**(한국어, `R4a-fs.md`의 문체): 1. 무엇을 옮겼나 2. TZif 형식과 시간대 탐색 순서 3. 시각 값의 표현과 연산 4. 서식 토큰 5. 순서를 지키는 json 파서와 정수·실수 구분 6. 반복 해석 7. 실제로 부딪힌 함정(진행 기록·보고서·리뷰에 있는 것만).
- [ ] **Step 2: open-questions "Rust 이전"** — 4b 완료 한 줄(계획·노트 경로, 비교 건수), 과도기 잔가지(시간대 탐색 순서, Go 시간 서식 세부, Go json 이스케이프), 알려진 차이.
- [ ] **Step 3: CLAUDE.md** — 현재 상태에 4b 한 줄.
- [ ] **Step 4: 전체 검사와 커밋** — `docs: record Rust port stage 4b`
