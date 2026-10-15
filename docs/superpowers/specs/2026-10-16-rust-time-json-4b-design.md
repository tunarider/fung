# Rust 이식 4b단계: `time`·`json` — 설계

2026-10-16. 이식 전체의 결정은 `docs/design/open-questions.md`의 "Rust 이전", 앞 단계는 `2026-10-15-rust-fs-4a-design.md`(4a).

## 목표

Go의 `_native.time`(8개)·`_native.json`(2개)과 `Time` 값을 Rust로 옮겨, `extras/time.fung`·`json.fung`가 Rust에서 Go와 같은 출력·종료 코드로 돈다.
골든 테스트 `time_module`·`time_values`·`json`·`err_json_stringify`를 Go 대조와 Rust 골든 실행기에 넣는다. 크레이트는 쓰지 않는다.

## 결정 사항 (2026-10-16)

| 항목 | 결정 |
|---|---|
| 지역 시간대 | Go처럼 TZif 파일을 직접 읽는다: `TZ` → `/etc/localtime` → zoneinfo 디렉터리를 Go(`time` 패키지 `initLocal`·`LoadLocation`)와 같은 순서·규칙으로 찾고, TZif v1~v3과 끝의 POSIX TZ 규칙 문자열을 해석한다 |
| Go를 흉내 내는 부분 | 이식 중에는 맞추고 "과도기 잔가지"에 기록(시간대 탐색 순서, Go 시간 서식 세부, Go json 이스케이프 규칙) |

## 1. 구조

| 무엇 | 어디 | 기준(Go) |
|---|---|---|
| `Time` 값 | `Value`의 새 종류(유닉스 나노초 + 오프셋(초) + 시간대 이름). 16바이트 유지 | `internal/value`의 `Time` |
| 시각 연산 | `==` `<` `± dur` `time - time`, ISO 출력, 정규화 차단 | `internal/value`의 산술·비교·서식 |
| 시간대 | `crates/fung-vm/src/native/tz.rs`. 프로세스당 한 번 읽어 둔다 | Go `time` 패키지(`zoneinfo_unix.go`, `zoneinfo_read.go`, `zoneinfo.go`의 `tzset`) |
| `_native.time` 8개: `now` `unix` `from_unix` `utc` `local` `parts` `format` `parse` | `native/time.rs` | `internal/native/time.go` |
| 서식·파싱(토큰 10개, `Z`, 정규화 차단) | `native/timefmt.rs` | `internal/native/timefmt.go` |
| `_native.json` 2개: `parse` `stringify` | `native/json.rs` | `internal/native/json.go` |
| fung 쪽 | `extras/time.fung`·`json.fung`는 Go와 같은 파일 그대로 | |
| 모듈 등록 | `time`·`json`을 지원 안 함(E0805) 목록에서 빼고 실제 모듈로. `run_parity_skip.txt`의 네 줄 삭제 | |

## 2. 동작

- 의미는 Go와 같다: 출력 문구, 에러 종류, 범위 밖 값, 윤초 없음.
- json 출력: 실수는 Go와 같은 가장 짧은 표현(1단계의 실수 서식을 재사용), 문자열 이스케이프는 Go `encoding/json`과 같다(`<` `>` `&` U+2028 U+2029 포함).
- json 입력: 키 순서 유지, 정수·실수 구분(Go의 토큰 디코더와 같은 기준), 큰 정수·깊은 중첩·잘못된 입력에서 Go와 같은 결과·에러(E0513).
  해석은 **반복으로**(재귀 금지), 깊은 중첩에서 스택이 넘치지 않는다.
- 시각·json 처리는 순수 계산이므로 실행권 안에서 한다. 시간대 파일 읽기는 프로세스당 한 번.

## 3. 검증

- 골든 테스트 4개를 `run_parity_skip.txt`에서 빼서 Go 대조와 Rust 골든 실행기에 넣는다.
- Go 단위 테스트(`internal/native/time_test.go`, `timefmt_test.go`, `json_test.go`, `internal/value`의 시각 테스트)를 옮긴다.
- 시간대: 두 구현 모두 `TZ`를 고정해 Go와 대조한다 — `Asia/Seoul`, `America/New_York`(서머타임 경계 앞뒤), `UTC`, `""`, 존재하지 않는 이름.
  TZif 해석기는 zoneinfo 파일로 단위 테스트(v1, v2+, 끝의 POSIX 규칙 `EST5EDT,M3.2.0,M11.1.0`).
- json: 정수/실수 경계(`1`, `1.0`, `1e2`, 큰 정수), 키 순서, 중복 키, 이스케이프(`<`, U+2028, 제어 문자), 잘못된 입력의 위치, 백만 단계 중첩을 Go와 대조.
- `now()`는 값이 매번 달라 형식·범위만 검사한다.

## 4. 문서

- 학습 노트 `docs/notes/R4b-time-json.md`(한국어): TZif 형식과 시간대 탐색, 시각 값의 표현, 서식 토큰, 순서 유지 json 파서, 정수·실수 구분, 실제로 부딪힌 함정.
- `docs/design/open-questions.md` "Rust 이전": 4b 완료 한 줄, 과도기 잔가지, 알려진 차이.
- `CLAUDE.md` 현재 상태.

## 완료 기준

1. 골든 테스트 4개를 포함한 실행 비교 전체가 Go와 같다.
2. Rust 골든 실행기의 대상에 4개가 들어간다.
3. 커밋 전 검사(`gofmt`·`go vet`·`go test`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`(디버그·릴리스),
   `FUNG_REQUIRE_PARITY=1 cargo test`(디버그·릴리스))가 모두 통과한다.
4. 학습 노트가 있다.

## 범위 밖

`regex`·크레이트·`libc` 신호(4c), `http`·소켓·TLS·스케줄러(4d).
