# Rust 이식 1단계: 앞단 — 설계

2026-10-14. 배경과 이식 전체의 결정은 `docs/design/open-questions.md`의 "Rust 이전" 절.

## 목표

fung의 앞단(렉서, 파서, 균일 트리, desugar, 이름 해석, 컴파일러, 바이트코드, diag, 모듈 로더)을 Rust로 옮긴다.
같은 소스에 대해 `fung dump ast`·`core`·`bytecode`의 출력, 컴파일 에러 메시지, 종료 코드가 Go 구현과 **글자 단위로 같아야** 한다.
Go 구현은 이식 기간의 정답지이고, 릴리스는 계속 Go 버전이다. Rust 실행 파일은 아직 배포하지 않는다.

이 단계는 공부를 겸한다. Rust 고유의 개념(소유권, `enum`과 `match`, `Result`, 빌림)을 학습 노트로 남긴다.

## 결정 사항

| 항목 | 결정 |
|---|---|
| 완료 범위 | dump 세 단계의 출력과 컴파일 에러(소스 발췌·밑줄·도움말·종료 코드)까지. 경고, `fmt`, REPL, 실행은 범위 밖 |
| 내장 함수 목록 | Go가 `spec/builtins.txt`를 생성해 저장소에 둔다. Rust는 빌드할 때 읽는다. Go 테스트가 최신인지 검사한다 |
| 에러 카탈로그 | 같은 방식으로 `spec/errors.txt` |
| 저장소 배치 | 루트를 Cargo 워크스페이스로, `crates/fung-front`(라이브러리)와 `crates/fung-cli`(실행 파일 `fung`). Go 코드는 그대로 |
| 검증 | Rust 테스트가 Go 버전을 빌드해 두 바이너리의 출력을 직접 비교. 산출물을 저장하지 않는다 |
| 옮기는 태도 | 단계 구성·균일 트리·명령 체계·에러 번호는 그대로, 각 모듈 안은 Rust 관용대로. 줄 단위 번역은 하지 않는다 |
| 의존성 | 크레이트 0개. Rust 표준 라이브러리만 |
| 툴체인 | `rust-toolchain.toml`로 Rust 1.94 고정, edition 2024 |

## 1. 구조

```
Cargo.toml                워크스페이스
rust-toolchain.toml
crates/
  fung-front/             라이브러리: 앞단 전체
  fung-cli/               실행 파일 fung (1단계는 dump만)
spec/
  builtins.txt            내장 함수 명세 (Go가 생성)
  errors.txt              에러·경고 카탈로그 (Go가 생성)
cmd/ internal/ go.mod     Go 구현 (정답지)
```

`fung-front`의 모듈과 Go 패키지의 대응:

| Rust 모듈 | Go | 하는 일 |
|---|---|---|
| `source` | (새로) | 파일 번호, 이름과 내용을 모은 소스 맵, 위치(파일 번호·줄·열)와 바이트 구간 |
| `diag` | `internal/diag` | `spec/errors.txt`의 문구, 에러 값, 소스 발췌 출력, 종료 코드 |
| `lexer` | `internal/lexer` | 글자 → 토큰, 주석 trivia |
| `tree` | `internal/tree` | 균일 트리 노드, S-식 출력, 붙잡을 값(`Capture`) |
| `parser` | `internal/parser` | Pratt 파서 |
| `desugar` | `internal/desugar` | 파이프·`for`·축약 문법 풀기 |
| `resolve` | `internal/resolve` | 이름 → 지역 슬롯·전역 칸·prelude, 붙잡을 값(모델 C) |
| `consts` | `internal/value`의 일부 | 상수 풀의 값: 정수, 실수, 문자열, 심볼, 기간, 불, nil, 이름만 가진 내장 함수 자리표 |
| `bytecode` | `internal/bytecode` | 명령, 원형(Proto), 패턴 트리, 역어셈블러 |
| `compiler` | `internal/compiler` | 트리 → 바이트코드 |
| `builtins` | (새로) | `spec/builtins.txt`를 읽어 prelude 이름과 `_native.*` 멤버를 제공 |
| `loader` | `internal/frontend` | 단계 실행, import 그래프, `extras/` 소스 내장(`include_str!`), 모듈 멤버 검사(E0303) |

경계:
- `fung-front`는 실행을 모른다. 이후 생길 VM 크레이트는 앞단에 의존하고, 반대 방향은 크레이트 경계가 막는다.
- `consts`는 1단계에 필요한 최소한이다. VM의 값 타입과의 관계는 2단계 설계에서 정한다.
- prelude의 fung 쪽 이름(`extras/prelude.fung`의 공개 이름)은 Go처럼 실행해서 얻지 않고, 그 파일을 Rust 앞단으로 컴파일해 정적으로 얻는다.
  `spec/builtins.txt`가 이 이름들도 출처와 함께 나열하므로, Rust 테스트가 둘이 같은지 검사한다.

`fung-cli`:
- `fung dump ast|core|bytecode <file>`과 `fung dump <stage> -e '<code>'`. 출력·에러·종료 코드·사용법 에러 문구가 Go와 같다.
- 다른 하위 명령은 "not implemented yet in the Rust port"를 표준 에러에 쓰고 종료 코드 2.

## 2. 핵심 타입과 판단

- **균일 트리**: `Node { head: Head, pos: Pos, lit: Lit, kids: Vec<Node>, span: Span }`.
  - `Head`는 Go의 문자열 대신 `enum`이고, 출력할 때만 Go와 같은 이름(`call`, `plist` …)으로 바꾼다. 출력 이름 표는 하나의 함수에 모은다.
  - 자식은 직접 소유한다. 단계는 트리를 받아 새 트리를 돌려준다. `dump ast`처럼 앞 단계의 트리도 필요하면 출력을 먼저 하거나 복제한다.
  - `Lit`: 없음, 정수(`i64`), 실수(`f64`), 불, 문자열, 기간(나노초 `i64`).
- **위치**: 파일은 번호로 가리키고, 이름과 내용은 소스 맵에 둔다. 열은 문자 단위, 구간은 바이트 단위다(Go와 같음).
- **에러**: `Diag { code, pos, len, args, help, notes }`, 함수는 `Result<T, Vec<Diag>>`를 돌려준다. 문법 에러는 첫 번째에서 멈추고 이름 에러는
  모두 모은다(08). 문구는 `spec/errors.txt`의 템플릿으로 만든다. 템플릿이 쓰는 서식은 `%s`·`%d`·`%q`·`%v` 네 가지이고, Rust 쪽에서 같은 결과를 내는
  작은 서식기를 둔다.
- **바이트코드**: 명령은 32비트(상위 8비트 명령, 하위 24비트 피연산자), 명령 목록은 Go와 같은 순서의 `enum`. 원형은 Go와 같은 필드를 갖고 안쪽 원형을 소유한다.
- **결정성**: 이름에 번호를 매기는 곳(prelude 이름은 정렬, 전역은 등장 순서)은 Go와 같은 순서를 쓴다. 순서가 출력에 드러나는 곳에 `HashMap`의 순회를 쓰지 않는다.

글자 단위 비교에서 함정이 될 곳:

| 항목 | 규칙 |
|---|---|
| 실수 | 가장 짧은 왕복 표현, 정수 꼴이면 `.0`, 지수는 21 이상이거나 -4 미만일 때만(M1). 예: `15000000000.0`, `1e21`, `1e-07` |
| 기간 | fung의 출력 규칙(`90m` → `1h30m`) |
| 문자열 | dump와 에러 문구의 `%q`는 Go `strconv.Quote`와 같은 이스케이프 |
| 열 번호·밑줄 길이 | 문자 단위. 넓은 글자의 화면 폭은 지금처럼 고려하지 않는다 |

## 3. 명세 파일

- `spec/builtins.txt`: 한 줄에 한 항목. prelude의 Go 내장 함수, `vm.Concurrency()`의 함수, `extras/prelude.fung`의 공개 이름, `_native.*` 모듈별 멤버를
  출처와 함께 나열하고, 함수는 매개변수 표(이름, 기본값 여부, `...rest`)를 붙인다. 정확한 문법은 구현 계획에서 정하되, 사람이 diff로 리뷰할 수 있는 텍스트여야 한다.
- `spec/errors.txt`: 한 줄에 한 번호. 번호, 문구 템플릿, (있으면) 도움말.
- 생성은 Go가 한다(`internal/spec` 패키지 + `go test`의 갱신 모드). Go 테스트가 파일이 최신인지 검사하고, 다르면 다시 만드는 명령을 알려 주며 실패한다.
- 이 두 파일은 Go를 지운 뒤에도 남는 언어 명세다. 4단계에서 Rust가 내장 함수를 직접 정의하게 되면 검사 방향을 뒤집는다(Rust 구현이 명세와 같은지).

## 4. 검증

- **모듈별 TDD**: 해당 Go 단위 테스트를 먼저 Rust로 옮기고 실패를 확인한 뒤 구현한다.
- **정답지 비교**(`crates/fung-cli/tests/parity.rs`):
  - `go build ./cmd/fung`로 Go 버전을 임시 폴더에 만든다. `go`가 없으면 이 테스트만 건너뛰고 이유를 출력한다.
  - 저장소 루트에서 `tests/lang/*.fung`(에러를 내는 `err_*` 포함), `examples/`·`extras/`·`bench/`의 `.fung` 전부에 대해 세 단계를 실행하고
    표준 출력·표준 에러·종료 코드를 비교한다. 같은 상대 경로로 실행하므로 에러 위치 표시까지 같아야 한다.
  - 다르면 파일, 단계, 처음 갈라진 줄을 보여 준다.
- **CI**: `ci.yml`에 Rust 작업(Linux·macOS): `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`(Go 설치 포함).
- **커밋 전 검사**: 지금의 Go 검사에 위 세 cargo 명령을 더한다(CLAUDE.md).

## 5. 문서

- 학습 노트 `docs/notes/R1-front-end.md`: 크레이트와 워크스페이스, 소유권과 트리, `enum`과 `match`, `Result`와 `?`, `&str`과 `String`, 반복자,
  이식 중 실제로 부딪힌 함정.
- CLAUDE.md: 저장소 규칙에 Rust 쪽 규칙(워크스페이스 구조, 크레이트 0개, 커밋 전 cargo 검사)과 현재 상태 한 줄.
- open-questions "Rust 이전"의 옮기는 방식 항목에 1단계 진행 상황.

## 완료 기준

1. 정답지 비교 대상 전부, 세 단계 모두에서 표준 출력·표준 에러·종료 코드가 Go와 같다.
2. `spec/builtins.txt`·`spec/errors.txt`가 최신이고, Go 테스트 전체가 그대로 통과한다.
3. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, CI가 모두 통과한다.
4. 학습 노트가 있다.

## 범위 밖

코드 실행(2단계부터), `fung check`의 경고(W0101~W0104), `fung fmt`, REPL, 런타임 에러, 배포.
