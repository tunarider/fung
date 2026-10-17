# Rust 이식 6b단계: Go 삭제 — 설계

2026-10-17. 앞 단계는 `2026-10-17-rust-release-prep-6a-design.md`(6a). 6a는 main에 머지됐다(`9a3a112`, CI 초록).

## 목표

저장소에서 Go 구현과 Go에 기대는 장치를 모두 걷어내고, Rust 하나로 모든 검사(테스트·명세 파일·릴리스 검사)가 돈다. 지금까지 Go와 대조해 쌓은
사례는 **fung의 기대 출력**으로 얼려 회귀 검사로 남긴다(Go·대조라는 이름표 없이). 동작은 바꾸지 않는다(얼린 기대값이 그대로 맞아야 한다).

**최종 목표(2026-10-17 사용자):** 처음부터 Rust로 짠 것 같은 저장소. 사용자가 없으므로 호환성은 고려하지 않는다. Go 구현은 이제 "제약"일 뿐이고,
Go 때문에 그렇게 했던 동작·구현은 모두 Rust답게 바꾸거나 최적화한다. 6b는 그 첫 단계(삭제와 이름표 떼기), **6c**가 과도기 잔가지 정리(동작 변경,
기대값 갱신)이고, **0.2.0 릴리스는 6c 뒤**다. 6c를 마친 뒤 코드와 현재 문서에서 "Go"가 검색되지 않는 것이 완료 기준이다.

## 결정 사항 (2026-10-17)

| 항목 | 결정 |
|---|---|
| Go 대조 테스트 | 입력마다 출력이 정해지는 것(`dump`·실행 비교·`check`·`test`·`fmt`·`json`·`time`·`regex`)은 Go 출력을 **한 번 기록해 기대값 파일로 얼리고** Rust가 그 파일과 비교. 실제 Go 서버·클라이언트를 맞붙이는 것(`http`·`tls`·`sigpipe`)은 Go 쪽을 떼고 이미 고정한 기대값·요청 원문 검사로 Rust 단독 |
| `# only:` | `# only: go` 파일 삭제, `# only: rust` 표시 제거, 장치(골든 실행기·Go 대조의 처리) 삭제 |
| `extras/` | `extras/rust/*.fung`(agent·conc·http)를 `extras/`로, Go 전용 파일(`extras/http.fung` 얇은 래퍼, `extras.go`, `extras_test.go`) 삭제, `prelude.fung`에서 `fan_in`·`pmap`·`retry` 제거 → `RUST_PRELUDE_FUNG_MOVED`·Rust 전용 차이 목록(`builtins.rs`)을 없애 명세를 하나로 |
| `spec/` | `builtins.txt`·`errors.txt`는 Rust 코드에서 생성(+ 코드와 파일이 같은지 테스트, 갱신 명령). `unicode.txt`·`regex_unicode.txt`·`regex_go_fixtures.rs`는 지금 내용으로 얼린 데이터(다시 만드는 법은 과도기 잔가지로 기록) |
| Go 테스트가 지키던 것 | `tests/install_test.go`(가짜 릴리스로 `install.sh`)·`tests/changelog_test.go`(CHANGELOG ↔ 매뉴얼 사이드바 버전 짝)를 Rust 통합 테스트로 옮김. `fmt_test`·`perf_test`·`golden_test`·`layers_test`는 Rust 쪽 대응(`fmt_repo`·`perf`·`golden`)이 있거나 의미가 없어짐(층 구조는 Rust 크레이트 의존으로) |
| 과도기 잔가지 | 6b에서는 건드리지 않는다(동작 불변). 6c에서 하나씩(Go 흉내 동작·`regex_go.rs`·`GoWriter`·"Go's …" 주석 포함) |
| 기대값의 이름 | 얼린 출력은 Go·parity라는 이름 없이 "기대 출력"(예: `tests/expected/`, 테스트 이름 `*_expected`·`*_snapshot`)으로. 처음부터 Rust로 짰을 때의 골든 테스트와 구별되지 않게 |
| 문서 | 현재 상태를 말하는 문서(설계 00~09, `background.md`, open-questions, `CLAUDE.md`, README, 매뉴얼)는 Rust 기준으로 고친다(6b는 Go 삭제에 직접 걸린 부분, 나머지는 6c). 학습 노트(M·R)와 지난 계획·스펙(`docs/plans`, `docs/superpowers`)은 역사 기록으로 둔다. CHANGELOG의 0.1.0 항목은 공개된 기록이라 둔다 |

## 1. 작업

| 작업 | 내용 |
|---|---|
| 1. 기대값 얼리기 | Go가 있는 지금, 각 대조 테스트에 일회용 기록 단계를 더해 Go 출력을 `crates/fung-cli/tests/expected/<주제>/…`에 쓰고, 테스트는 그 파일과 Rust를 비교(기록 단계는 작업 6에서 함께 지움; 이후 갱신은 Rust 출력으로 `--bless`류). 기록 직후 Rust 비교가 그대로 통과하는지 확인. 정규화가 필요한 값(임시 경로, 포트)은 지금 대조에서 이미 하는 치환을 그대로 쓴다 |
| 2. 네트워크·신호 대조를 Rust 단독으로 | `http_parity`·`tls`·`sigpipe`에서 Go 프로그램을 띄우는 부분을 떼고, 이미 고정한 기대값(요청 원문, 응답, 신호 13, 에러 문구)으로 Rust만 검사. 사례 수가 줄지 않게 |
| 3. 명세 파일을 Rust에서 | `spec/builtins.txt`·`errors.txt`를 Rust(`fung-front` 테스트의 `-- --update` 또는 작은 도구)에서 생성, 코드와 파일 일치 테스트. 다른 `spec`·고정 데이터는 그대로 두고 생성하던 Go 테스트를 지움 |
| 4. Go 테스트가 지키던 것 옮기기 | `install_test`·`changelog_test`를 Rust 통합 테스트로 |
| 5. `extras` 정리와 `# only:` 제거 | 위 결정대로. `tests/lang`의 `# only: go` 파일 삭제, `# only: rust` 표시 제거, 골든 실행기에서 장치 삭제, 매뉴얼·예제 검사의 Go 관련 분기 삭제 |
| 6. Go 삭제 | `cmd/`, `internal/`, `go.mod`(·`go.sum`), `tests/*.go`, `extras/*.go`, Go 쪽 스크립트·문서 조각. `FUNG_REQUIRE_PARITY`·`common::go_fung` 등 Go 대조 도구 삭제. CI·릴리스 워크플로에서 Go 작업·`setup-go` 삭제. `bench/README.md`의 Go 빌드 줄 |
| 7. 문서 | `CLAUDE.md`를 Rust 단일 구현 기준으로(커밋 전 검사 목록에서 Go 줄 삭제, 저장소 규칙·구조), `docs/design/09`·`background.md` 원칙 4, `README.md`, open-questions(6b 완료, Go 대조가 얼린 기대값으로 바뀜, 남은 과도기 잔가지 목록이 다음 단계 입력), 학습 노트 `docs/notes/R6b-go-removal.md` |

순서: 1·2·3·4는 Go가 있어야 하므로 먼저, 5·6은 그 뒤, 7은 마지막.

## 2. 검증

- 각 작업 뒤 커밋 전 검사 전체(이 단계 동안은 Go가 사라지는 작업 6 전까지 Go 검사도).
- 작업 6 뒤의 커밋 전 검사: `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`(디버그·릴리스), `cargo test --locked --no-fail-fast`(디버그·릴리스).
- 얼린 기대값의 사례 수가 얼리기 전 Go 대조 사례 수와 같다(각 테스트가 개수를 단언).
- 저장소에 `.go` 파일·`go.mod`가 없고, 워크플로·스크립트·`CLAUDE.md`에 Go 도구 호출이 없다(테스트로).

## 3. 범위 밖

과도기 잔가지 정리(6c), R7 스케줄러, 0.2.0 릴리스(6c 뒤, 태그·시험 실행은 사용자 확인), 학습 노트·계획·스펙의 과거 기록(역사로 남김).
