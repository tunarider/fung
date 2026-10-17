# Rust 이식 6a단계: 릴리스 전환 준비 — 설계

2026-10-17. 이식 전체의 결정은 `docs/design/open-questions.md`의 "Rust 이전", 앞 단계는 `2026-10-17-rust-tools-5-design.md`(5단계).
6단계는 6a(전환 준비, Go는 정답지로 남김)와 6b(Go 삭제, 대조를 얼린 기대값으로)로 나눈다(2026-10-17 결정).

## 목표

Rust 구현을 0.2.0으로 내보낼 준비를 끝낸다: 내장 함수 배치 확정, 예제·매뉴얼을 Rust 기준으로, Rust 릴리스 파이프라인, CHANGELOG, 벤치마크 기록.
Go 구현은 이 단계 끝까지 정답지로 남는다(삭제는 6b).

## 결정 사항 (2026-10-17)

| 항목 | 결정 |
|---|---|
| 6단계 나누기 | 6a 전환 준비 / 6b Go 삭제 |
| 내장 함수 배치 | 원칙 "prelude에는 확실한 것만". `fan_in`·`pmap`·`retry`를 Rust에서 prelude → **`conc` 모듈**로. 나머지는 유지(`until`·`iterate`는 반복의 대체라 핵심, `exec`·`sh`는 셸 대체 언어의 핵심). `remove`·`close`의 모듈 이름 겹침은 유지하고 매뉴얼에 한 줄 |
| `spec` 파일(6b에서) | `builtins.txt`·`errors.txt`는 Rust 코드에서 생성(+동기 테스트), `unicode.txt`·`regex_unicode.txt`·정규식 대조 데이터는 얼린 데이터 |
| Go 대조 테스트(6b에서) | Go 출력을 한 번 기록해 기대값 파일로 얼리고 계속 돌린다. 시간에 기대는 것은 Rust 전용으로 |
| 벤치마크 | Go·Rust 기록만(기준치 없음), R7의 출발점 |
| Linux 바이너리 | **musl 정적 빌드**(Go의 "아무 서버에서나 돈다"를 지킴). `libutil` 링크와 musl 할당 속도를 확인 |
| 0.2.0 릴리스 시점 | 6b 뒤: 브랜치를 main에 스쿼시 머지한 다음 태그. 머지·태그 푸시는 그때 사용자 확인 |

## 1. 작업

| 작업 | 내용 |
|---|---|
| 1. `conc` 모듈 | `extras/rust/conc.fung`(Rust 부팅만 읽음)에 `fan_in`·`pmap`·`retry`. Rust prelude에서 세 이름 제거(Rust 내장 함수 차이 목록 `crates/fung-front/src/builtins.rs`). 이 이름을 쓰는 `tests/lang` 파일은 Go판(`# only: go`, 그대로)과 Rust판(`# only: rust`, `conc.`)으로 나눈다 |
| 2. 예제·매뉴얼 | `examples/`의 `atom` → `agent`, 세 이름 → `conc.`. 매뉴얼(`docs/*.html`)을 0.2.0(Rust) 기준으로: `atom` 절 → `agent`, `conc` 모듈, HTTP 약속, 64MiB 상한, E0704·E0705, `fung test`·REPL 규칙, `remove`·`close` 구분 한 줄. 매뉴얼 코드 예제는 Rust `check`·`fmt --check`·실행 통과(지금 `tools_parity`의 매뉴얼 예제 검사를 Rust 전용으로). Go 쪽 검사가 예제·매뉴얼을 돌리면 그 대상에서 뺀다 |
| 3. Rust 릴리스 | `scripts/build-release.sh`를 `cargo` 기반으로: Linux는 musl 정적(`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`), macOS는 기본(arm64·amd64). 플랫폼별 CI 러너. 묶음 이름 `fung_<ver>_<os>_<arch>.tar.gz`·`SHA256SUMS`·빌드 증명은 지금과 같게, `FUNG_VERSION`으로 버전 주입. `install.sh` 불변(`tests/install_test.go`가 계속 검사). `release.yml`을 Rust로(6b 전까지 Go 테스트도 함께). 실제 태그 없이 워크플로를 시험할 길(수동 실행 등) |
| 4. CHANGELOG | `[Unreleased]`에 0.2.0 변화: open-questions "0.2.0에서 바뀌는 것" 목록 전부 + `conc` 이동 + 5단계 결정. 사이드바 버전은 릴리스 때 |
| 5. 벤치마크 | `bench/`를 Go·Rust(glibc·musl)로 돌려 표로 기록(`bench/README.md`, open-questions R7) |
| 6. 문서 | 학습 노트 `docs/notes/R6a-release-prep.md`, open-questions, CLAUDE.md |

## 2. 검증

- 골든: Go 실행기는 `# only: rust`를 건너뛰고, Rust 실행기는 Rust판을 돈다. Go 대조(`run_parity`)는 표시된 파일을 건너뛴다(지금 규칙).
- 매뉴얼·예제: Rust로 `check`·`fmt --check`·실행(`test` 블록 포함) 통과.
- 릴리스: musl 바이너리가 정적인지(`file`/`ldd`), 묶음 구조, `install.sh`가 Rust 묶음으로 동작(가짜 릴리스 테스트), 버전 문자열.
- 커밋 전 검사 전체.

## 3. 범위 밖

Go 삭제·대조 얼리기·`spec` 생성 이전(6b), 실제 릴리스·머지·태그(6b 뒤, 사용자 확인), 과도기 잔가지 정리(6단계 뒤), R7 스케줄러.
