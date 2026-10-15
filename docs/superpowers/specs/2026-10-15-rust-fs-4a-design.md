# Rust 이식 4a단계: `fs`·`path` — 설계

2026-10-15. 이식 전체의 결정은 `docs/design/open-questions.md`의 "Rust 이전", 앞 단계는 `2026-10-15-rust-agent-3b-design.md`(3b).
4단계는 4a(`fs`·`path`) → 4b(`time`·`json`) → 4c(`regex`, 첫 크레이트, `libc` 신호) → 4d(`http`, 소켓·TLS, 스케줄러 재판단)로 나눈다(2026-10-15 결정).

## 목표

Go의 `_native.fs`(17개)·`_native.path`(6개)를 Rust로 옮겨, 그 위의 `extras/fs.fung`·`path.fung`과 prelude의 파일 함수(`with_open` 등)가
Rust에서 Go와 같은 출력·종료 코드로 돈다. 골든 테스트 `fs_and_path`를 Go 대조와 Rust 골든 실행기에 넣는다.
3b에서 남긴 취소 노드 누수를 첫 작업으로 고친다. 크레이트는 쓰지 않는다(`std::fs`, `std::path`).

Go를 흉내 내려고 붙이는 것(Go `filepath` 규칙의 직접 구현, Go 문구에 맞춘 운영체제 에러)은 이식 중에는 맞추고 open-questions "과도기 잔가지"에
기록한다. 나중에 더 나은 방법으로 바꿀 수 있다(기존 지침).

## 1. 구조

| 무엇 | 어디 |
|---|---|
| `_native.fs` 17개: `read` `write` `append` `open` `close` `write_line` `exists` `stat` `mkdir` `remove` `remove_all` `copy` `move` `list` `glob` `walk` `temp_dir` | `crates/fung-vm/src/native/fs.rs` |
| `_native.path` 6개: `join` `base` `dir` `ext` `stem` `abs` | `crates/fung-vm/src/native/path.rs` |
| 파일 핸들 | `Value`의 새 종류. 출력 형식·`type_of`·같음은 Go(`internal/value`의 파일 값)와 같다 |
| fung 쪽 | `extras/fs.fung`·`path.fung`·prelude는 Go와 같은 파일을 그대로 쓴다 |
| 모듈 등록 | 3b까지 E0805(지원 안 함)를 내던 `fs`·`path`를 실제 모듈로. `crates/fung-cli/tests/run_parity_skip.txt`의 `fs_and_path` 줄 삭제 |

기준은 `internal/native/fs.go`·`path.go`와 그 테스트(`fs_test.go` 등)다.

**직접 구현(표준 라이브러리로 Go와 같은 결과가 안 나오는 것)**
- `glob`: Go `filepath.Glob`의 패턴 규칙(`*`, `?`, `[...]`, 범위, `^` 부정, 이스케이프)과 결과 순서.
- `walk`·`list`: Go의 순서(사전순)와 결과 모양.
- 경로 함수: Go `filepath`의 `Clean`·`Join`·`Base`·`Dir`·`Ext` 규칙(`..` 정리, 빈 경로는 `.`, 끝의 `/`).
- 운영체제 에러: Go가 `os` 에러를 `Err({kind: …, message: …})`로 바꾸는 표를 그대로 옮긴다. `message`에 들어가는 운영체제 문구는
  Go(`*PathError`의 `op path: err`) 형식에 맞춘다.

## 2. 동작

- 파일 입출력은 실행권을 내려놓고(`outside`) 한다. 큰 파일을 읽는 동안 다른 태스크가 돈다.
- 파일 핸들마다 잠금을 둔다(Go의 핸들 뮤텍스와 같음). 실행권 밖에서 쓰므로 실행권만으로는 보호되지 않는다. 한 줄 쓰기가 섞이지 않는다.
- 핸들의 마지막 사본이 사라지면 파일을 닫는다(참조 계수라 정확하다). Go는 GC가 거둘 때까지 열려 있을 수 있었다 — Rust 쪽이 더 정확한 차이로 기록한다.
  닫힌 핸들에 쓰기·`close` 두 번 등은 Go와 같은 결과.
- 그 밖의 의미(인자 검사, 에러 종류, 반환 모양)는 Go와 같다.

## 3. 검증

- `fs_and_path.fung`를 Go 대조(run_parity)와 Rust 골든 실행기에 넣는다.
- Go 단위 테스트(`internal/native/fs_test.go`, path 테스트)를 옮긴다. Go 전용(GC 시점 등)은 의도를 옮긴다.
- 직접 만든 `-e` 비교: 없는 파일, 권한 없음, 디렉터리를 파일처럼 읽기, `glob`의 대괄호, 중첩 `walk` 순서, 여러 태스크가 한 핸들에 동시에 쓰기,
  닫은 뒤 쓰기. 임시 디렉터리 경로는 실행마다 다르므로 두 구현에 같은 디렉터리를 주거나 출력의 경로를 치환해 비교한다.
- 누수 수정: 3b 리뷰의 중첩 `swap` 반복(`swap(a, x => x + swap(b, fn(y): spawn(() => 1); y + 1 ;;))`)이 일정한 메모리로 끝나는지
  (취소 노드 수 탐침 또는 메모리 상한) 테스트한다.

## 4. 문서

- 학습 노트 `docs/notes/R4a-fs.md`(한국어, 짧게): 운영체제 에러를 값으로, 파일 핸들의 수명과 참조 계수, 실행권 밖의 입출력, `glob`을 직접 구현한 이유,
  실제로 부딪힌 함정.
- `docs/design/open-questions.md` "Rust 이전": 4a 완료 한 줄, 과도기 잔가지(`glob`·`walk`·경로 규칙, Go 문구에 맞춘 운영체제 에러), 핸들 수명 차이,
  3b 누수 항목 해결 표시.
- `CLAUDE.md` 현재 상태.

## 완료 기준

1. `fs_and_path`를 포함한 실행 비교 전체가 Go와 같다.
2. Rust 골든 실행기의 대상에 `fs_and_path`가 들어간다.
3. 커밋 전 검사(`gofmt`·`go vet`·`go test`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`(디버그·릴리스),
   `FUNG_REQUIRE_PARITY=1 cargo test`(디버그·릴리스))가 모두 통과한다.
4. 학습 노트가 있다.

## 범위 밖

`time`·`json`(4b), `regex`·크레이트·`libc` 신호(4c), `http`·소켓·TLS·스케줄러(4d), 내장 함수 배치·구현 언어 재검토(6단계 전, 지침은 open-questions).
