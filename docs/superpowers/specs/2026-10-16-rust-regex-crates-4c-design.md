# Rust 이식 4c단계: `regex`, 첫 크레이트, SIGPIPE — 설계

2026-10-16. 이식 전체의 결정은 `docs/design/open-questions.md`의 "Rust 이전", 앞 단계는 `2026-10-16-rust-time-json-4b-design.md`(4b).

## 목표

Go의 `_native.regex`(5개)를 Rust로 옮겨 `extras/regex.fung`가 Rust에서 Go와 같은 출력·종료 코드로 돈다. 이식에서 처음으로 외부 크레이트를
들이고(`regex`, `libc`), 허용 목록을 지키는 장치(저장소 테스트 + CI의 `cargo-deny`·`cargo-audit`)를 만든다. 닫힌 표준 출력·에러 파이프에서
Go처럼 SIGPIPE로 죽는다. 골든 테스트 `regex`·`err_regex`를 Go 대조와 Rust 골든 실행기에 넣는다.

## 결정 사항 (2026-10-16)

| 항목 | 결정 |
|---|---|
| 정규식 엔진 | `regex` 크레이트 + Go 의미로 맞추는 얇은 층. 엔진 직접 구현(Thompson NFA)은 나중의 공부 과제로 남긴다(기존 결정) |
| 에러 문구 | Rust 에러를 Go 에러 종류 문구로 바꿔 Go 형식 한 줄. 흔한 에러는 Go와 대조, 못 맞춘 것은 알려진 차이 |
| 크레이트 점검 | `deny.toml`(허용 목록의 원본) + 표준 라이브러리만 쓰는 저장소 테스트 + CI의 `cargo deny check`·`cargo audit` |
| SIGPIPE | `libc`로. 표준 출력·에러의 EPIPE에서 SIGPIPE를 기본 동작으로 되돌리고 스스로 보낸다 |
| 허용 목록 형식 | 직접 들인 크레이트와 그것이 끌고 오는 크레이트를 함께 적고, 직접 들인 것마다 이유를 적는다 |

## 1. 구조

| 무엇 | 어디 | 기준(Go) |
|---|---|---|
| 크레이트 | `crates/fung-vm/Cargo.toml`: `regex` 1.x, `libc` 0.2(유닉스 대상만). 따라오는 것: `regex-automata`, `regex-syntax`, `aho-corasick`, `memchr`. `fung-front`·`fung-cli`의 직접 의존성은 늘리지 않는다(`fung-front` 0개 유지) | |
| `_native.regex` 5개: `is_match` `find` `find_all` `replace` `split` | `crates/fung-vm/src/native/regex.rs`. 패턴 캐시 256개(차면 통째로 비움, 프로세스 공용이면 실행권과 무관한 Rust 값만), 바이트 → 글자 위치는 앞으로만 가는 커서로 선형 | `internal/native/regex.go` |
| Go 의미 맞춤 층 | `native/regex_go.rs` (2절) | Go `regexp/syntax`(`parse.go`·`perl_groups.go`), `regexp`(`regexp.go`) |
| 바꿈 문자열 | Go `Regexp.Expand`를 직접 옮긴다(`$1`, `${name}`, `$$`, 없는 그룹, `$1a`를 이름으로 읽는 규칙) | `regexp.go`의 `expand`·`extract` |
| SIGPIPE | 표준 출력·에러에 쓰는 곳 (3절) | Go 런타임 `os/file_unix.go` `epipecheck`, `runtime` sigpipe |
| 허용 목록·점검 | 최상위 `deny.toml`, 저장소 테스트, `.github/workflows/ci.yml` (4절) | |
| fung 쪽 | `extras/regex.fung`는 Go와 같은 파일 그대로 | |
| 모듈 등록 | `regex`를 지원 안 함(E0805) 목록에서 빼고 실제 모듈로. `run_parity_skip.txt`의 `regex.fung`·`err_regex.fung` 줄 삭제(남는 것은 `http_serve.fung`) | |

## 2. Go 의미 맞춤 층

`regex-syntax`(이미 따라오는 크레이트)의 구문 트리(AST)로 패턴을 한 번 해석하고, 트리를 훑어:

- **Perl 글자 묶음을 ASCII로**: Go에서 `\w` `\d` `\s` `\b` `\B`(대괄호 안의 `[\w]` 등 포함)는 ASCII만 뜻한다. Rust는 기본이 유니코드이므로
  ASCII 판으로 바꿔 넘긴다. 유니코드 속성(`\pL` 등)과 대소문자 무시는 그대로 넘긴다(유니코드 표 버전 차이는 알려진 차이).
- **Go가 거부하는 문법 거부**: Go가 받지 않는 플래그(`x` 등), 이스케이프(`\Z` 등), 반복 횟수 1000 초과, Rust 전용 문법. Go와 같은 에러로.
- **Rust가 거부하지만 Go가 받는 문법**: 찾으면 Go 의미로 바꿔 넘기거나, 못 하면 알려진 차이로 기록한다.
- **에러 문구**: Rust 에러 종류를 Go `syntax.ErrorCode` 문구로 바꿔 `error parsing regexp: <종류>: ` + "`<문제 부분>`" 한 줄. 문제 부분은 Go가
  고르는 부분 문자열을 따른다(Rust 에러 구간으로 계산). Go의 에러 종류마다 하나 이상 Go와 대조한다. 정확히 못 맞춘 경우는 알려진 차이.
- **매칭 의미**: 가장 왼쪽 우선(leftmost-first), 빈 매치를 다루는 방식(`find_all`·`split`·`replace`에서 직전 매치 바로 뒤의 빈 매치), `.`와 줄바꿈,
  `^` `$`(플래그 없을 때 텍스트 끝만)를 Go와 대조해 맞춘다. 다르면 이 층이나 반복 코드에서 Go 규칙으로 처리한다.

이 층 전체는 open-questions "Go 동등을 위한 과도기 잔가지"에 등록한다.

**2026-10-16 구현 중 개정(사용자 결정).** AST를 훑는 얇은 층으로는 Go가 먼저 내는 에러·에러 문구·`\<`·`[[a]]` 같은 패턴의 뜻을 맞출 수 없어,
Go `regexp/syntax` 파서를 Rust로 옮겼다(`native/regex_go.rs`). 해석 결과는 `regex_syntax` HIR로 바꿔 `regex` 엔진에 넘긴다(매칭은 크레이트).
- 이 파서는 **fung 정규식 문법의 정의**로 본다. 크레이트를 올려도 받아들이는 문법이 바뀌지 않는다. Go 삭제 뒤에도 남기고 6단계 뒤에 다시 판단한다.
- 유니코드 표(`\pL`, 대소문자 무시)는 Go가 생성하는 `spec/regex_unicode.txt`를 쓴다(앞단의 `spec/unicode.txt`와 같은 방식). 과도기 잔가지로 기록한다.
- 무작위 패턴 2,000개의 Go 기대값은 저장소에 테스트 데이터로 둔다(Go 삭제 뒤의 회귀 검사).
- Go는 받지만 Rust 엔진의 크기 한도를 넘는 아주 큰 패턴은 Go의 "expression too large" 문구로 실패한다(알려진 차이).


## 3. SIGPIPE

- Rust 런타임은 시작할 때 SIGPIPE를 무시로 둔다. 그래서 닫힌 파이프에 쓰면 EPIPE 에러가 오고, 지금은 `exit(141)`로 흉내 낸다.
- 표준 출력·에러에 쓰다 EPIPE가 나면: `libc::signal(SIGPIPE, SIG_DFL)` 뒤 `libc::raise(SIGPIPE)`. 살아남으면(막힌 신호 등) 지금처럼 141.
  `unsafe` 블록마다 `// SAFETY:` 근거.
- 다른 파일·소켓·자식 프로세스 파이프에 쓰는 EPIPE는 지금처럼 에러 값(Go와 같음).
- 자식 프로세스가 물려받는 SIGPIPE 상태가 Go로 띄운 자식과 같은지 확인하고, 다르면 맞춘다.
- open-questions "2단계에서 알고 남긴 차이"의 닫힌 파이프 141 항목을 해결로 표시.

## 4. 크레이트 점검

- **`deny.toml`(원본)**: `[bans]`의 허용 목록(직접 + 따라오는 크레이트), `[licenses]`는 실제로 필요한 라이선스만, `[sources]`는 crates.io만,
  `[advisories]` 기본.
- **저장소 테스트**(표준 라이브러리만, 도구 없이 로컬에서도 돈다): `Cargo.lock`의 모든 패키지(작업 공간 크레이트 제외)가 `deny.toml` 허용 목록에
  있는지, `crates/fung-front/Cargo.toml`에 `[dependencies]` 항목이 없는지 검사한다. `Cargo.lock`은 커밋한다.
- **CI**: `rust` 작업에 `cargo-deny`·`cargo-audit`를 버전 고정으로 설치(`cargo install --locked`)하고 `cargo deny check`, `cargo audit`.
- **open-questions "Rust 이전"**: 허용 목록 표 — 크레이트마다 "왜 직접 짜지 않는가"와 따라오는 크레이트.

## 5. 검증

- `regex`·`err_regex` 골든 테스트를 Go 대조와 Rust 골든 실행기에 넣는다.
- Go 단위 테스트(`internal/native/regex_test.go`)를 옮긴다.
- Go 대조(`crates/fung-cli/tests/regex_parity.rs`): 한글에 `\w`·`\b`, 빈 매치(`a*`·`x*`·`\b`)의 `find_all`·`split`·`replace`, 이름 그룹·없는 그룹,
  바꿈 문자열의 `$` 규칙, 대소문자 무시, 여러 줄 플래그, 글자 위치(여러 바이트 글자), Go 에러 종류마다 하나 이상, Go가 거부하고 Rust는 받는 패턴.
  커밋하지 않는 무작위 대조도 돌려 보고 결과를 보고서에 적는다.
- SIGPIPE: 받는 쪽을 먼저 닫은 파이프에 많이 출력하는 프로그램을 두 구현으로 돌려, 둘 다 신호 13으로 끝나는지(`ExitStatusExt::signal`) 비교.
- 크레이트 점검 테스트: 허용 목록 밖의 패키지를 넣은 가짜 `Cargo.lock`으로 실패하는지.

## 6. 문서

- 학습 노트 `docs/notes/R4c-regex-crates.md`(한국어): 정규식 엔진의 원리(유한 오토마타, 선형 시간, 역참조가 없는 이유), RE2 계열 안의 차이,
  의존성을 고르는 기준과 공급망 점검, 신호와 종료 코드(141과 "신호 13으로 죽음"의 차이), 실제로 부딪힌 함정.
- `docs/design/open-questions.md` "Rust 이전": 4c 완료 한 줄, 허용 목록 표, 과도기 잔가지(Go 의미 맞춤 층, 에러 문구 바꾸기), 알려진 차이,
  닫힌 파이프 항목 해결.
- `CLAUDE.md`: 현재 상태, 크레이트 규칙 문구("앞단·VM은 0개" → 앞단 0개, VM은 허용 목록).

## 완료 기준

1. 골든 테스트 `regex`·`err_regex`를 포함한 실행 비교 전체가 Go와 같다(남는 제외는 `http_serve`).
2. Rust 골든 실행기의 대상에 두 파일이 들어간다.
3. 커밋 전 검사(`gofmt`·`go vet`·`go test`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`(디버그·릴리스),
   `FUNG_REQUIRE_PARITY=1 cargo test`(디버그·릴리스))가 모두 통과한다. 허용 목록 테스트도 그 안에서 돈다.
4. 학습 노트가 있다.

## 범위 밖

`http`·소켓·TLS·`mio`·`rustls`·스케줄러 재판단(4d), 정규식 엔진 직접 구현, 외부 입력 패턴을 `Err`로 검사하는 함수(`regex.compile`류, 기존 보류 항목).
