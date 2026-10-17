# Rust 이식 6a단계: 릴리스 전환 준비 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rust 구현을 0.2.0으로 내보낼 준비: `conc` 모듈, 예제·매뉴얼을 Rust 기준으로, musl 정적 릴리스 파이프라인, CHANGELOG, 벤치마크 기록. Go는 정답지로 남긴다.

**Architecture:** Go와 Rust의 차이는 지금처럼 `# only:` 표시와 Rust 내장 함수 차이 목록으로 다룬다. 릴리스 스크립트·워크플로를 cargo로 바꾸되 묶음 이름·`install.sh`는 그대로.

**Tech Stack:** Rust 1.94(+ musl 대상), fung, sh, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-10-17-rust-release-prep-6a-design.md`

## Global Constraints

- Go 구현(`cmd/`, `internal/`, Go 테스트)은 수정하지 않는다(6b까지 정답지). 예외: Go 테스트가 예제·매뉴얼을 돌리면 그 대상에서 빼는 최소 수정.
- 크레이트는 `deny.toml` 허용 목록만, `fung-front` 0개, `fung-cli`는 작업 공간 크레이트만.
- 커밋 전: `gofmt -l .`, `go vet ./...`, `go test ./...`, Go `fung fmt --check extras examples tests/lang`(일부러 망가뜨린 `err_*`만 실패), `cargo fmt --check`,
  `cargo clippy --all-targets --locked -- -D warnings`(디버그와 `--release`), `FUNG_REQUIRE_PARITY=1 cargo test --locked`(디버그와 `--release`).
- 코드·주석·커밋 메시지는 영어, 매뉴얼·학습 노트·CHANGELOG·설계 문서는 한국어. Conventional Commits, 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. 다른 이메일 금지.
- 커밋은 자기 경로만 명시적으로(`-A`·`-a` 금지). 원격으로 푸시하지 않는다. 실제 태그를 만들지 않는다.
- 매뉴얼을 바꾸면 매뉴얼 코드 예제가 Rust `check`·`fmt --check`·실행을 통과해야 한다.

## Review Focus

1. 이름을 옮긴 뒤 어디에도 Rust에서 prelude `fan_in`·`pmap`·`retry`를 부르는 곳이 남지 않는다(예제·매뉴얼·`extras/rust`·테스트) → Task 1, 2.
2. 매뉴얼이 0.2.0 Rust 동작과 어긋나는 서술이 없다(`atom`, HTTP "Go 기본", 옛 E 코드, 옛 `fung test`·REPL 규칙) → Task 2.
3. musl 바이너리가 정말 정적이고, `install.sh`가 Rust 묶음을 그대로 설치하며, 버전 문자열이 `fung version`에 나온다 → Task 3.
4. CHANGELOG가 open-questions "0.2.0에서 바뀌는 것"과 5단계 결정을 빠짐없이 담고, 사용자 말로 쓰였다 → Task 4.
5. 벤치마크가 같은 조건(같은 기계, 반복 횟수, 릴리스 빌드)에서 측정됐다 → Task 5.

---

### Task 1: `conc` 모듈

**Files:** Create `extras/rust/conc.fung`, Modify Rust 부팅(`extras/rust/*.fung` 읽는 곳), `crates/fung-front/src/builtins.rs`(Rust 차이 목록: prelude에서 `fan_in`·`pmap`·`retry` 빠짐, `conc` 모듈 더함), `extras/rust/http.fung` 등 세 이름을 쓰는 Rust 전용 코드, `tests/lang`(세 이름을 쓰는 파일을 Go판 `# only: go`와 Rust판 `# only: rust`로 분리)
- 참조: `extras/prelude.fung`의 세 함수(그대로 옮김), `extras/rust/agent.fung`(Rust 전용 fung 파일을 읽는 방식)

- [ ] **Step 1: 실패하는 테스트** — Rust판 골든(`conc.pmap`·`conc.fan_in`·`conc.retry`), prelude에서 `pmap` 등을 부르면 Go와 같은 "정의되지 않은 이름" 에러, `conc` 매개변수 표 확인.
- [ ] **Step 2: 실패 확인** — Run: `cargo test -p fung-cli --test golden` / Expected: FAIL.
- [ ] **Step 3: 구현**
- [ ] **Step 4: 통과 확인** — Run: 커밋 전 검사 전체 / Expected: PASS.
- [ ] **Step 5: 커밋** — `feat(rust): move fan_in, pmap and retry into conc`

### Task 2: 예제와 매뉴얼

**Files:** Modify `examples/*.fung`(`atom` → `agent`, 세 이름 → `conc.`), `docs/index.html`·`syntax.html`·`builtins.html`·`internals.html`(0.2.0 Rust 기준), 매뉴얼 예제 검사(`crates/fung-cli/tests/tools_parity.rs`의 매뉴얼 예제를 Rust 전용 검사로), Go 테스트가 예제·매뉴얼을 돌리면 그 대상 조정
- 참조: open-questions "Rust 첫 릴리스(0.2.0)에서 바뀌는 것", 06의 "fung HTTP 약속", 08의 `fung test`·REPL 규칙, `docs/notes/R3b-agent.md`

- [ ] **Step 1: 실패하는 테스트** — 매뉴얼·예제 검사를 Rust 전용으로 바꾸고(`atom` 예제를 건너뛰던 허용치를 없앰), 예제의 `test` 블록을 Rust `fung test`로.
- [ ] **Step 2: 실패 확인** — Expected: FAIL(아직 `atom`).
- [ ] **Step 3: 구현** — 예제·매뉴얼 수정. `builtins.html`의 함수 목록과 서명은 Rust 실제 매개변수 표에서(지금 Go에서 뽑는 방식이면 Rust로). 사이드바 버전은 그대로(릴리스 때).
- [ ] **Step 4: 통과 확인** — Run: 커밋 전 검사 전체 / Expected: PASS.
- [ ] **Step 5: 커밋** — `docs: update examples and manual for the Rust release`

### Task 3: Rust 릴리스 파이프라인

**Files:** Modify `scripts/build-release.sh`, `.github/workflows/release.yml`, `.github/workflows/ci.yml`(필요하면 musl 빌드 확인 단계), `tests/install_test.go`가 가짜 릴리스를 만드는 방식이 스크립트에 기대면 그에 맞게(Go 테스트 최소 수정 허용 — 이유를 보고서에)
- 참조: 지금 `build-release.sh`·`release.yml`(Go), `install.sh`, `cmd/fung/version.go`와 Rust `FUNG_VERSION`

- [ ] **Step 1: 실패하는 테스트** — 스크립트를 로컬에서 돌리는 테스트(가능한 대상만: 이 기계의 macOS arm64/amd64, musl 대상은 설치돼 있으면): 묶음 이름·내용(`fung`·`LICENSE`·`README.md`)·`SHA256SUMS`·`fung version` 출력. Linux musl 바이너리 정적 확인은 CI 단계로(`file`·`ldd`).
- [ ] **Step 2: 실패 확인**
- [ ] **Step 3: 구현** — `cargo build --release --target …` + `FUNG_VERSION`. 워크플로: 플랫폼별 러너 행렬(ubuntu x86_64·arm64 → musl, macOS arm64·x86_64), 빌드 증명, 릴리스 노트(`scripts/changelog-section.sh`). 수동 실행(`workflow_dispatch`)으로 태그 없이 시험하는 길(릴리스 생성은 건너뜀). 6b 전까지 Go 테스트도 함께 돈다.
- [ ] **Step 4: 통과 확인** — Run: 커밋 전 검사 전체 + 로컬 스크립트 실행 / Expected: PASS.
- [ ] **Step 5: 커밋** — `build: release the Rust binary with static musl builds on Linux`

### Task 4: CHANGELOG

**Files:** Modify `CHANGELOG.md`(`[Unreleased]`), 필요하면 `tests/changelog_test.go`의 규칙 확인(버전 짝은 릴리스 때)
- 참조: open-questions "Rust 첫 릴리스(0.2.0)에서 바뀌는 것", 4a~5단계의 알려진 차이 중 사용자에게 보이는 것, 6a 결정

- [ ] **Step 1~4:** `[Unreleased]`에 "바뀜(호환 깨짐)"·"더함"·"고침"·"성능" 절로 사용자 말로 정리. `tests/changelog_test.go` 통과.
- [ ] **Step 5: 커밋** — `docs: describe the 0.2.0 changes in the changelog`

### Task 5: 벤치마크

**Files:** Modify `bench/README.md`, `docs/design/open-questions.md`(R7 항목에 숫자)
- [ ] **Step 1:** `bench/`의 각 측정을 같은 기계에서 Go 릴리스, Rust 릴리스(glibc 기본·musl 정적 — 이 기계에서 musl을 못 돌리면 Linux에서 돈 결과를 CI로 받거나 그렇다고 적음)로 여러 번 돌려 중앙값 표.
- [ ] **Step 2: 커밋** — `docs: record Go and Rust benchmarks`

### Task 6: 문서

**Files:** Create `docs/notes/R6a-release-prep.md`, Modify `docs/design/open-questions.md`, `CLAUDE.md`
- [ ] **Step 1: 학습 노트**(한국어): 이름 배치 원칙과 결정, 정적 링크와 musl, 릴리스 파이프라인, 벤치마크 읽는 법, 실제로 부딪힌 함정.
- [ ] **Step 2: open-questions·CLAUDE.md** — 6a 완료, `conc` 이동, musl 결정, 6b 할 일.
- [ ] **Step 3: 전체 검사와 커밋** — `docs: record Rust port stage 6a`
