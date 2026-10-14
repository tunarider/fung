# Rust 이식 2단계: 값과 VM — 설계

2026-10-15. 이식 전체의 결정은 `docs/design/open-questions.md`의 "Rust 이전", 1단계는 `2026-10-14-rust-front-end-design.md`.

## 목표

fung의 값·영속 컬렉션·VM·내장 함수를 Rust로 옮겨, 동시성과 네이티브 모듈을 쓰지 않는 프로그램을 Rust `fung`이 실행한다.
같은 프로그램의 `fung run` 표준 출력·표준 에러·종료 코드가 Go 구현과 **바이트 단위로 같아야** 한다. Go 구현은 계속 정답지이고 릴리스도 Go 버전이다.

공부를 겸한다. VM 설계, 참조 계수와 제자리 수정, 영속 자료구조를 Rust 소유권 아래서 짜는 법을 학습 노트로 남긴다.

## 결정 사항

| 항목 | 결정 |
|---|---|
| 범위 | VM 기능 전부(모듈 적재·실행, `at_exit`, `exit`, 런타임 에러와 호출 경로, `assert`, `test` 블록, `dbg`). 동시성(3단계)과 네이티브 모듈 `fs`·`json`·`time`·`regex`·`path`·`http`·`cli`(4단계)는 제외 |
| 리스트·맵 | Go M10 구조 그대로(작으면 평평한 표현, 크면 영속 벡터와 보기, 항목 벡터와 HAMT) + 참조가 하나일 때 `Rc::make_mut`로 제자리 수정 |
| 프로세스 내장 함수 | `env`·`exec`·`sh`를 2단계에서 구현(`std::env`, `std::process`) |
| 동시성 내장 함수 | 이름은 두고 부르면 `E0805 not supported yet: <name>`. `atom` 제거는 대체 라이브러리와 함께 3단계 |
| 값 | 16바이트 `enum`, 큰 값은 `Rc`(원자적 연산 없음) |
| 의존성 | 크레이트 0개 |

## 1. 구조

```
crates/
  fung-front/   1단계 그대로
  fung-vm/      값, 영속 컬렉션, VM, 내장 함수
  fung-cli/     fung: dump + run, 파일 실행, -e
```
의존 방향은 `fung-cli → fung-vm → fung-front` 하나다. 앞단은 실행을 모른다.

| Rust 모듈 | Go | 하는 일 |
|---|---|---|
| `value` (+ `list`, `pvec`, `hamt`, `omap`) | `internal/value` | 값 `enum`, 산술·비교·인덱스·출력 서식·기간·심볼, 영속 컬렉션 |
| `vm` | `internal/vm`의 비동시성 부분 | 실행 루프, 프레임, 호출 규약, 꼬리 호출, 패턴 매칭, 모듈 실행, `assert`·`test`·`dbg`, `at_exit`·`exit`, 런타임 에러와 호출 경로 |
| `prelude` | `internal/prelude` | 컬렉션·문자열·변환·입출력 내장 함수 |
| `native` | `internal/native/process.go`, `internal/nat` | `env`·`exec`·`sh`, 내장 함수 공용 도우미 |
| `conc` | (자리표) | 동시성 내장 함수와 `_native.conc.spawn_awaited`: 부르면 E0805 |

실행 흐름(Go `bootPrelude`·`execute`와 같음):
1. Rust 내장 함수 + `extras/prelude.fung`를 컴파일·실행해 얻은 공개 값으로 prelude 표(이름 → 값)를 만든다.
2. 앞단 로더가 프로그램 전체를 컴파일한다. 상수 풀의 `Const::Builtin(name)`은 적재할 때 prelude 표의 값으로 바꾼다.
3. 모듈을 의존 순서대로 한 번씩 실행해 모듈 값을 만들고, 진입 파일을 실행한다.
4. `at_exit` 핸들러, 종료 코드, 런타임 에러를 Go `cmd/fung/pipeline.go`의 `execute`와 같은 순서로 처리한다.

`fung-cli`: `fung run <file>`, `fung <file.fung>`, `fung -e '<code>'`를 Go와 같은 인자 처리·종료 코드로 추가한다. `check`·`test`·`fmt`·REPL은 "not implemented yet in the Rust port"(종료 코드 3)로 남긴다(5단계).
파이프라인 전체는 1단계처럼 4 GiB 스택 스레드에서 돈다.

## 2. 값과 메모리

```rust
enum Value {
    Int(i64), Float(f64), Bool(bool), Nil,
    Str(Rc<String>), Sym(Sym), Dur(i64),
    List(Rc<List>), Map(Rc<Map>), Tuple(Rc<Vec<Value>>),
    Tag(Rc<Tag>), Range(Rc<Range>),
    Closure(Rc<Closure>), Native(&'static Native), Module(Rc<Module>),
}
```
- 16바이트를 목표로 한다(크기를 테스트로 고정). 정수·실수·불·기간은 칸에 바로 들어간다. 이를 위해 포인터는 모두 얇은 `Rc`다(`Rc<String>`, `Rc<Vec<Value>>`, `Rc<Range>`; `Rc<str>`·`Rc<[T]>`는 두 워드라 쓰지 않는다).
- 심볼은 전역 표에 등록해 번호로 비교한다.
- `Time`·파일 핸들·태스크·채널은 3·4단계에서 이 `enum`에 더한다.
- 회수는 참조 계수만으로 한다. 2단계에는 순환이 생길 길이 없다(`atom`이 동작하지 않음).
- 영속 벡터·HAMT·리스트 보기의 해제는 `Drop`을 직접 구현해 반복으로 한다. 재귀 해제로 스택이 넘치지 않게 한다.
- 영속 컬렉션은 Go M10과 같은 경계값(리스트 32, 맵 16), 1/4 규칙, 죽은 항목 압축을 쓴다. 참조가 하나일 때는 `Rc::make_mut`로 노드를 제자리에서 고친다.
- VM은 스택에서 꺼낸 값을 복제하지 않고 옮겨, 제자리 수정이 실제로 일어나게 한다(정확성이 아니라 속도의 규칙).
- 맵의 해시는 `std::collections::hash_map::RandomState`(실행마다 다른 키)로 한다. 순서는 항목 벡터가 정하므로 출력은 늘 같다.
- 런타임 에러는 1단계 `Diag`에 호출 경로를 더한다. 출력은 Go `diag.Format`과 같다("call path (innermost first)", 앞 8개·뒤 2개만, `(N tail calls elided)`).
- 값 연산의 실패(넘침, 0으로 나누기, 타입 불일치)는 Go와 같은 번호·문구다.
- 사용자 프로그램이 Rust 패닉을 일으키면 안 된다. 구현 실수로 생긴 패닉은 스레드 경계에서 잡아 E0801로 보고한다.

## 3. VM과 내장 함수

- 명령 체계와 의미는 Go와 같다. 상태는 값 스택(`Vec<Value>`)과 프레임 목록(프레임: 원형, 명령 위치, 슬롯 시작, 클로저, 전역 표).
- fung 함수 호출은 프레임만 쌓는다(Rust 재귀 없음). 한도 100,000을 넘으면 E0804. 꼬리 호출은 프레임을 재사용한다.
- 처음부터 넣는 최적화(Go 성능 1·2단계에서 측정으로 찾은 것): 산술은 `match`로 바로 분기, 위치 인자 개수가 맞는 fung 호출은 인자를 프레임에 바로 놓기, 범위는 펼치지 않고 하나씩.
- 클로저는 모델 C: 만들어질 때 보이는 값을 복사해 담는다.
- 내장 함수도 fung 함수와 같은 매개변수 표(이름, 기본값, `...rest`)로 인자를 검사한다(E0402~E0407 한 곳에서).
- 람다를 부르는 내장 함수(`fold`, `map` 등)는 VM 실행 루프를 한 겹 더 연다. Rust 스택이 쌓이는 유일한 경우이고 4 GiB 스레드가 받는다.
- 모듈: 로더가 정한 순서대로 본문을 한 번 실행하고, 그 전역의 공개 값으로 모듈 값을 만든다. `IMPORT`는 그 값을 돌려준다. `_native.conc`(자리표)와 `_native.process`(실제)는 Rust 네이티브 모듈이다.

| 묶음 | 함수 | 2단계 |
|---|---|---|
| 컬렉션 | `map` `filter` `fold` `each` `find` `all` `any` `sort` `sort_by` `group_by` `flatten` `len` `pairs` `put` `remove` `merge` | 구현 |
| 문자열 | `split` `join` `trim` `upper` `lower` `replace` `repeat` `chars` `starts_with` `ends_with` | 구현 |
| 변환 | `str` `int` `float` `sym` `parse_int` `parse_float` `parse_dur` `type_of` | 구현 |
| 입출력·제어 | `print` `eprint` `read_line` `read_lines` `fail` `exit` `at_exit` `dbg` `args` | 구현 |
| 프로세스 | `env` `exec` `sh` | 구현 |
| 동시성 | `spawn` `await` `cancel` `channel` `send` `recv` `close` `sleep` `atom` `swap` `deref` | 부르면 E0805 |
| fung prelude | `spec/builtins.txt`의 `prelude fung` 이름 | `prelude.fung` 실행으로 얻음 |

## 4. 검증과 성능

- **모듈별 TDD**: Go 단위 테스트(값 서식·산술·비교·인덱스·기간, 영속 컬렉션의 참조 모델 대조 테스트, VM의 비동시성 테스트, 내장 함수)를 먼저 옮기고 실패를 확인한 뒤 구현한다. 백만 개짜리 리스트·맵 해제가 스택을 넘지 않는 테스트를 더한다.
- **실행 정답지 비교**(`crates/fung-cli/tests/run_parity.rs`): Go 바이너리와 Rust 바이너리의 `fung run` 표준 출력·표준 에러·종료 코드를 바이트 단위로 비교한다.
  - 골든 파일의 `# args:`·`# stdin:` 지시를 두 바이너리에 똑같이 넘긴다.
  - 대상: `tests/lang`의 모든 `.fung`(제외 목록 밖), `bench/*.fung`, 직접 만든 케이스(런타임 에러 종류별, `at_exit` 순서, `exit` 코드, 백만 번 꼬리 재귀, E0804, 람다 안의 깊은 `fold` 중첩, 긴 체인).
  - 제외 목록 `crates/fung-cli/tests/run_parity_skip.txt`: 한 줄에 파일 하나와 이유(`concurrency` 또는 `native-module`). 목록 밖 파일은 자동으로 대상이다. 3·4단계가 이 목록을 줄인다.
  - 1단계처럼 `FUNG_REQUIRE_PARITY=1`이면 Go가 없을 때 실패한다(CI).
- **성능 고정**: Go `tests/perf_test.go`의 다섯 모양(맵 `put` 누적, 기존 키 갱신, 리스트 덧붙이기, `...rest` 재귀, 꼬리 자르기 재귀)을 2만 개로 돌려 각 0.3초 안. 릴리스 빌드에서만 돌리고(`#[cfg(not(debug_assertions))]`), CI에 `cargo test --release` 단계를 더한다.
- **벤치마크 기록**: `bench/` 네 개를 Go와 같은 기계에서 재서 학습 노트에 남긴다(합격 기준 아님).
- **CI**: Rust 작업에 릴리스 테스트 단계를 더한다.

## 5. 문서

- 학습 노트 `docs/notes/R2-vm.md`: VM 실행 루프와 프레임, `Rc`와 참조 계수, `Rc::make_mut`와 제자리 수정, 영속 자료구조를 Rust 소유권 아래서 짜는 법, `Drop`과 연쇄 해제, 내장 함수가 VM을 다시 부르는 구조, 실제로 부딪힌 함정, 벤치마크 결과.
- CLAUDE.md 현재 상태, open-questions "Rust 이전"의 2단계 완료 줄.

## 완료 기준

1. 실행 정답지 비교 대상 전부에서 Go와 표준 출력·표준 에러·종료 코드가 같다.
2. 1단계 dump 비교가 그대로 통과한다.
3. 성능 고정 테스트가 릴리스 빌드에서 통과한다.
4. Go 테스트, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`(디버그·릴리스), CI가 모두 통과한다.
5. 학습 노트가 있다.

## 범위 밖

동시성(3단계), 네이티브 모듈 `fs`·`json`·`time`·`regex`·`path`·`http`·`cli`(4단계), `check`·`test`·`fmt`·REPL(5단계), `atom` 제거(3단계), 배포.
