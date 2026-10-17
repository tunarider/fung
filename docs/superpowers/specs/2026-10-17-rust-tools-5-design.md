# Rust 이식 5단계: 도구(check·test·fmt·REPL) — 설계

2026-10-17. 이식 전체의 결정은 `docs/design/open-questions.md`의 "Rust 이전", 앞 단계는 `2026-10-16-rust-net-http-4d-design.md`(4d).

## 목표

Go의 `fung check`·`test`·`fmt`·REPL·`version`·`help`를 Rust로 옮겨 Rust CLI가 Go CLI와 같은 명령 전체를 같은 출력·종료 코드로 제공한다.
경고 W0101~W0104를 Rust 앞단에 옮긴다. 먼저 Go 포매터의 들여쓰기 버그를 고친다.

## 결정 사항 (2026-10-17)

| 항목 | 결정 |
|---|---|
| 나누기 | 설계 하나, 작업 다섯(+ 앞선 Go 포매터 수정): 경고·`check`·`version`·`help` / `test` / `fmt` / REPL / 문서 |
| Go 포매터 버그 | **Go를 먼저 고친다**(테스트 포함). "이식 중 Go 구현은 수정하지 않는다"의 예외: 정답지의 명백한 버그는 고친다. 고친 뒤 `extras/rust/http.fung`의 `_serve` 감싸개를 걷어낸다 |
| REPL 터미널 | `libc` termios로 raw 모드(허용 목록에 이미 있음), Go 편집기를 그대로 옮긴다. 크레이트(`rustyline` 등)는 쓰지 않는다 |
| Go 대조 범위 | `check`·`test`·`fmt --check`·`fmt` 출력을 `tests/lang`·`examples`·`extras`·매뉴얼 예제 전부로, REPL은 파이프 입력 시나리오로 |

## 1. 구조

| 무엇 | 어디 | 기준(Go) |
|---|---|---|
| Go 포매터 버그 수정 | `internal/format` + 테스트. 최소 재현 `fn f(h: fn, x = 1):\n  y = x\n  h(y)\n;;`가 본문을 내어 쓴다(타입 표기 `fn` 뒤 블록 본문). 고친 뒤 저장소 `.fung` 재포맷 확인, `_serve` 감싸개 제거, open-questions 항목 해결 표시 | `internal/format` |
| 경고 W0101~W0104 | `crates/fung-front` 해석 단계. `check`·`test`에서만 낸다(`run`·`-e`·`dump`는 내지 않음) | `internal/resolve/warn.go`, `m6_test.go` |
| `fung check <path>\|-e <code>` | `fung-cli`. Go처럼 경로 하나 또는 `-e`만 받는다(디렉터리는 읽기 에러, 경로 둘은 사용법 에러; 구현 중 확인) | `cmd/fung/check.go`, `pipeline.go` |
| `version`·`help` | `fung-cli`. 버전 문자열은 빌드 시 주입한 `FUNG_VERSION` → 없으면 `0.0.0-dev`(Task 1에서 정함: 크레이트 버전은 모두 `0.0.0`이라 쓰지 않는다). 6단계 릴리스 스크립트는 `FUNG_VERSION=$version cargo build --release` | `cmd/fung/version.go`, `main.go` |
| `test` 블록·`assert`(E0807)·`fung test [path]` | 앞단: `test` 블록을 함수로 컴파일(`run`은 버림). CLI: 경로 순회(`_`로 시작하는 이름 건너뜀 등 Go 규칙), 파일마다 런타임, 핸들러 → `end_background` → 꺼내지 않은 실패 보고, 테스트 없는 파일 건너뜀, `<load>` 집계, 에러 중복 제거 | `cmd/fung/test.go`, Go 컴파일러의 test 블록 처리 |
| `fung fmt [--check] <path>…\|-` | `crates/fung-front/src/format/`: 줄 묶기·들여쓰기 상태 기계·공백 표·정렬, 출력 토큰 열 보존 안전장치 | `internal/format`, `cmd/fung/fmt.go` |
| REPL | `crates/fung-cli/src/repl/`: raw 모드 줄 편집기(`libc` termios, 유닉스), 기록 파일, 파이프 입력 대체 경로, 재정의 허용, **세션 끝에** `end_background`(2026-10-17 사용자 결정: 입력 사이에 정의가 남으므로 전역에 담은 에이전트도 남아야 한다. 입력마다 부르면 배경 루트가 취소되어 다음 입력부터 에이전트가 일하지 않는다) | `internal/repl`, `cmd/fung/repl.go` |

## 2. 동작

- 출력 문구·종료 코드·경로 순회 순서는 Go와 같다.
- `fung test`·REPL은 3b의 종료 순서를 지킨다: 핸들러 → `end_background` → 꺼내지 않은 실패 보고. 에이전트가 다음 파일로 넘어가지 않는다. REPL은 세션 끝에 한 번(위 표; 아무도 붙잡지 않은 에이전트는 핸들이 사라질 때 끝난다).
- 포매터는 Go와 바이트 단위로 같은 출력을 낸다(고친 버그 포함). 출력 토큰 열이 입력과 다르면 포맷을 거부한다(Go 안전장치와 같음).
- REPL 편집 키·기록 파일 위치·형식은 Go와 같다.

## 3. 검증

- Go 대조(`crates/fung-cli/tests/tools_parity.rs` 등): `check`·`test`·`fmt --check`·`fmt` 출력을 `tests/lang`·`examples`·`extras`(`extras/rust` 포함)·매뉴얼 예제에 대해 두 구현으로 돌려 바이트 비교. 일부러 망가뜨린 입력(잘린 파일, 섞인 들여쓰기 등)도.
  `fmt`의 멱등성(두 번 포맷해도 같음).
- 경고: Go `m6_test.go` 등 경고 테스트를 옮기고 Go 대조.
- REPL: 파이프 입력 시나리오(정의·재정의·에러·여러 줄 입력·`exit`·에이전트가 입력 사이에 남지 않음)를 Go와 대조. 편집기·기록 파일은 Go 단위 테스트를 옮긴다.
- 저장소 `.fung` 포맷 검사를 Rust 쪽에도 둔다(Go `tests/fmt_test.go`가 하던 것).
- 커밋 전 검사에 Rust `fung fmt --check extras examples tests/lang`를 더할지는 계획에서 정한다(Go 검사는 그대로 유지).

## 4. 문서

- 학습 노트 `docs/notes/R5-tools.md`(한국어): 포매터가 토큰을 재배치하는 원리와 고친 버그, 경고 분석(사용·가림), 테스트 실행기의 런타임 경계와 `end_background`, raw 모드 터미널, 실제로 부딪힌 함정.
- `docs/design/open-questions.md` "Rust 이전": 5단계 완료, 포매터 버그 해결, 과도기 잔가지·알려진 차이.
- `CLAUDE.md` 현재 상태(다음은 6단계).

## 완료 기준

1. Rust CLI가 Go CLI의 명령 전체를 제공하고, Go 대조가 같다.
2. Go 포매터 버그가 고쳐지고 `_serve` 감싸개가 없다.
3. 커밋 전 검사 전체가 통과한다.
4. 학습 노트가 있다.

## 범위 밖

6단계(릴리스 전환·Go 삭제·예제와 매뉴얼의 `atom` → `agent`·CHANGELOG·릴리스 스크립트), 언어 서버·tree-sitter, R7 스케줄러.
