# 09. 구현 계획

## 작업 방식

- 구현은 Claude가 한다. 테스트를 먼저 쓰고 실패를 확인한 뒤 구현한다 (TDD).
- 사용자는 이론과 설계 판단(언어를 만들 때 무엇을 고려하는지)에 집중한다.
  마일스톤마다 `docs/notes/M<n>-<주제>.md`에 무엇을 만들었고 어떤 원리로 동작하는지 한국어로 남긴다.
- 커밋은 Conventional Commits, 영어. 커밋 전 검사는 `CLAUDE.md`의 목록(`cargo fmt --check`, `cargo clippy`, `cargo test`, 디버그·릴리스).

## 기본 정보

- Rust, 루트 Cargo 작업 공간(`Cargo.toml`, edition 2024, 최소 Rust 1.94). 크레이트 셋은 `publish = false`.
- 의존성: 기본은 **Rust 표준 라이브러리.** 크레이트는 허용 목록(`deny.toml`)만, 앞단(`fung-front`)은 0개. 이유와 목록은
  open-questions "Rust 구현: 정한 것과 남은 일"의 의존성 정책. REPL 줄 편집 등도 직접 구현한다.

## 구조

```
fung/
  Cargo.toml         작업 공간
  crates/
    fung-front/      앞단 (외부 크레이트 0개)
      src/lexer/     소스 → 토큰 (위치 정보, 주석은 trivia 목록으로 보존)
      src/parser/    토큰 → 균일 트리 (문장 경계에서 재동기화 가능하게)
      src/tree/      균일 트리 노드
      src/desugar.rs 겉 문법 트리 → 코어 언어 트리
      src/resolve/   이름 해석, 스코프 검사, 의미 에러 수집, 경고
      src/compiler/  코어 트리 → 바이트코드
      src/bytecode/  명령 정의, 소스 위치 표, 역어셈블러
      src/loader.rs  import 그래프 적재, builtins.rs 내장 이름 표(spec/builtins.txt), extras.rs 내장 fung 소스
      src/diag/      에러 번호, 메시지 카탈로그(spec/errors.txt를 만드는 표), 출력 형식, 종료 코드
      src/format/    포맷터
    fung-vm/         값과 실행
      src/value/     fung 값의 Rust 표현, 영속 벡터·HAMT
      src/vm/        실행기, 태스크, 스케줄러, 이벤트 루프, 풀, 취소 트리, 채널, 교착 감지
      src/prelude/   core의 값 연산 (print, map, filter, split, int …)
      src/native/    숨은 원시 모듈 `_native.*` (fs, path, process, json, time, regex, net, TLS, range)
      src/conc.rs    동시성 내장 함수 (`spawn`·`await`·`send`·`recv`·`sleep` …)
      src/boot.rs    prelude 부팅, `_native.*` 다리 표(`NATIVE_MODULES`), spec/builtins.txt 생성(테스트)
    fung-cli/        실행 파일 `fung`: 하위 명령(run, check, test, fmt, dump, version, help), REPL, 경로 순회
      tests/         통합 테스트, 골든 실행기, 기대 출력 파일(tests/expected/)
  extras/            fung으로 쓴 내장 extras (include_str!): prelude.fung, fs, path, cli, conc, http … (06)
  spec/              내장 이름 표·에러 카탈로그(Rust에서 생성), 유니코드 표(고정 데이터)
  tests/lang/        언어 기능 골든 테스트 (.fung)
  docs/design/       설계 문서
  docs/notes/        마일스톤·이식 단계별 학습 노트
```

- `desugar`와 `resolve`는 M1부터 배관으로 둔다 (M1에서는 거의 통과).
- VM은 처음부터 태스크별 상태로 분리한다. 동시성 구현은 M7이지만 구조는 M1부터.

## 앞단 설계

### 토큰

- `{종류, 글자, 위치(파일·줄·칸), 값}`.
- 주석은 토큰 흐름에서 빼되 위치와 함께 **별도 목록(trivia)**에 보관한다. 포맷터가 원래 자리에 되돌려 놓기 위해서다.
- 줄바꿈은 `NEWLINE` 토큰으로 내보낸다. 줄 이어짐과 `:`의 역할 구분은 **파서가 문맥으로** 처리한다 (01).

### 균일 트리

- 노드 하나의 형태는 `{머리, 위치, 자식들, 리터럴 값}` 뿐이다. `f(a, b)`는 `(call f a b)`, `if c: a else: b ;;`는 `(if c (do a) (do b))`.
- 노드 종류마다 Rust 구조체·`enum` 변형을 따로 만들지 않는다(`Node{head, pos, lit, kids}` 하나). "속은 Lisp"의 실체이며 `dump ast`·desugar·추후 매크로가 모두 이 형태 위에서 동작한다.
- 대가: Rust 컴파일러가 "이 노드에 자식이 셋이어야 한다"를 검사해 주지 않는다. 노드를 다루는 단계(desugar·resolve·컴파일러)가 저마다 기대하는 모양을 가정하고, desugar는 모르는 머리 단어만 E0801로 거른다(2026-10-24 코드에 맞춤; 처음 계획은 desugar가 모양을 모두 검사).

### 코어 언어

desugar의 출력. 컴파일러는 이것만 안다.

| 코어 형태 | 뜻 | 여기로 풀리는 겉 문법 |
|------|----|------|
| `(do e…)` | 순서대로, 마지막 값 | 블록 본문 |
| `(let 패턴 e)` | 대입 | `x = e`, 구조 분해 |
| `(if c a b)` | 조건 | `if`/`else if` 체인, `then … else`, `and`/`or` (단락 평가) |
| `(fn 이름 인자표 본문)` | 함수 | `fn` 정의, 람다, 익명 함수, `_` 부분 적용, `for`의 본문 |
| `(call f 인자… (named 이름 e)…)` | 호출 | 호출, `x \|> f(a)` → `(call f x a)`, `for` → `(call map xs (fn …))`(버려지는 자리면 `each`, 01) |
| `(op 연산자 a b)` | 원시 연산 | 산술·비교·`in`·`..`. 연산자는 값이 아니다 (`fold(xs, 0, +)` 없음, 람다로 쓴다) |
| `(field x "name")` | 필드 | `x.name` (맵·모듈 공통) |
| `(index x i)`, `(slice x a b)` | 인덱스 | `x[i]`, `x[a..b]` |
| `(tag Ok e…)` | 태그 값 | `Ok(e)` |
| `(match e (절 패턴 guard 본문)…)` | 패턴 매칭 | `match` |
| `(unwrap e 모드)` | 결과값 축약 | `e!`, `e?`, `e ?? b` (모드: abort / return / else) |
| `(return e)`, `(quote v)`, `(import …)`, `(test …)`, `(assert e)` | | |

desugar는 겉 문법 트리를 **같은 형태의 더 작은 트리**로 바꾼다.

### 파서

- 문장은 재귀 하강, 식은 **Pratt 파서** (07의 우선순위 표를 숫자로 옮긴 것 하나로 모든 이항·단항·후위 연산 처리).
- 두 곳에서 **되돌리기**를 쓴다: 문장 시작의 `패턴 =`(`try_destructure`), `match` 절의 `패턴 =>`(`arm_head`). 패턴 문법이 작아 비용이 적다.
  `(a, b) =>` 람다는 되돌리지 않고 괄호 너머를 앞질러 본다(`lambda_params_ahead`; 2026-10-24 코드에 맞춤).
- 문법 에러는 첫 번째에서 멈추되, 구조는 문장 경계(`NEWLINE` `;` `;;`)에서 재동기화할 수 있게 둔다 (08의 LSP 대비).

### 이름 해석 (resolve)

- 코어 트리를 한 번 훑어 모든 이름을 "지역 슬롯 n번 / 붙잡은 값 n번 / 자기 자신 / 모듈 전역 / prelude"로 분류한다.
- 02~04의 스코프 규칙 위반(바깥 변수 재할당, 블록 밖 이름, `for` 안 `return`, 최상위 `return`, 모듈 이름 가리기,
  지역 함수 선행 참조, 대안 패턴의 이름 불일치)을 **전부 모아** 보고한다.
- 컴파일러는 이름을 다시 찾지 않는다.

## 뒷단 설계

### 값의 Rust 표현

- `enum Value`(종류 태그 + 값 칸, 16바이트): `Int(i64)`, `Float(f64)`, `Bool`, `Nil`, `Str(Rc<String>)`, `Sym`(전역 인턴 표), `Dur(i64)`, `Time`,
  `List`(작으면 평평한 배열, 크면 영속 벡터 위의 보기), `Map`(작으면 넣은 순서의 쌍 배열, 크면 항목 벡터 + HAMT), `Tuple`, `Tag{이름, 인자들}`, `Range`,
  `Closure{원형, 붙잡은 값}`, `Native`, `Module`, `Task`, `Scope`, `Sender`, `Receiver`, `Atom`, `File`, `Conn`, `Listener`. 상자가 필요한 것은 `Rc`로 공유하고 참조 계수로 회수한다
  (긴 구조의 연쇄 해제는 재귀가 아니라 반복).
- **값을 훑는 Rust 코드는 값의 모양을 따라 재귀하지 않는다** (L9, 2026-10-28): 출력(`value/format.rs`), `==`·E0517 검사·순서 비교·해시(`value/compare.rs`),
  해제(`release.rs`), 변하는 값 훑기, JSON은 명시적 스택(힙의 `Vec`)으로 돈다. Rust의 스택 넘침은 잡을 수 있는 에러가 아니라 강제 종료(134)이기 때문이다.
  출력은 한 버퍼에 이어 써서 깊이에 선형이다. 컨테이너가 아닌 값은 스택 없이 곧바로 처리한다(흔한 경우의 비용을 지킴).
- 모든 연산이 새 값을 돌려준다. M9까지는 단순 복사, M10부터 리스트·맵은 영속 자료구조.
  - M10(2026-10-14 결정): 측정상 누적과 꼬리 재귀가 n²라 영속 자료구조로 바꾼다. 리스트는 32개 이하면 평평한 배열, 넘으면
    영속 벡터(32갈래 트리 + tail) 위의 보기 `{벡터, 시작, 끝}`(자르기·`...rest`는 상수 시간). 맵은 16개 이하면 지금 표현(측정으로 32에서 낮춤), 넘으면
    넣은 순서의 항목 벡터 + HAMT 색인. 값 층 밖은 `Len`/`At`/`All`/`Slice`/`Append`와 빌더만 쓴다.
    스펙 `docs/superpowers/specs/2026-10-14-persistent-collections-design.md`.
- 스택 슬롯은 `Option<Value>`: `None`은 "슬롯이 아직 비어 있음", fung의 `nil`은 별도 값 `Value::Nil`. 이 구분이 기본값 처리(비어 있으면 채운다)를 공짜로 만든다.

### 함수 원형 (Proto)

- 코드, 상수 풀, 인자 표(이름·기본값 여부·`...rest`·타입 표기), 지역 슬롯 수, 붙잡을 값 목록, **명령마다 소스 위치 하나**(병렬 슬라이스. 압축은 나중에).

### 명령 인코딩

- **고정 폭 32비트 한 단어 = 8비트 명령 + 24비트 피연산자.** 두 단어 명령은 없다. 피연산자가 둘인 명령은 24비트를 나눠 담는다(`CALL`·`TAILCALL`은 위치 인자 수 12비트 + 이름 인자 수 12비트, `encode_call`; 2026-10-24 코드에 맞춤).
- 디코딩과 점프 계산이 단순하고 역어셈블러가 쉽다. 24비트면 상수·슬롯·점프 거리에 충분하다.
- 기각: 가변 길이 바이트 열 (Crafting Interpreters 방식) → 매 명령 길이를 따져야 한다.

### 프레임과 스택

- 태스크마다 피연산자 스택 하나를 공유하고, 프레임은 `{Proto, ip, 슬롯 시작 위치, Closure}`.
- 지역 변수는 resolve가 정해 둔 **슬롯 번호**로 접근하며, 블록이 끝나면 슬롯을 재사용한다. 실행 중 이름 찾기는 없다.
- 프레임 깊이 **100,000**에서 E08xx "stack overflow". 프레임이 작은 구조체라 수십 MB 수준이며, Python(1,000)보다 넉넉하게 두어 적당한 비꼬리 재귀는 허용한다.

### 클로저

- `CLOSURE n`이 원형 n의 "붙잡을 값 목록"(현재 프레임 슬롯 / 현재 클로저의 붙잡은 값)을 **복사**해 `Captured`를 채운다. 03의 결정 덕에 upvalue 상자가 없다.
- 지역 `fn`의 자기 참조는 `LOAD_SELF`(현재 프레임의 클로저를 올림) 한 명령.

### 모듈 전역

- 모듈마다 `globals []Value`, 이름→번호는 컴파일 시점에.
- 최상위 `fn` 칸은 초기화 전에 채우고(끌어올리기), 변수 칸은 초기화가 진행되며 채운다.
  함수는 전역 변수를 조회하지 않는다: 만들어질 때 값을 복사한다 (`CaptureGlobal`). 다른 최상위 `fn`은 상수라 직접 참조한다 (03).
  한 번만 대입되는 최상위 이름도 상수라 최상위 `fn`에서는 직접 참조한다(`LOADGLOBAL`, 비어 있으면 E0809; L9).
- 다른 태스크도 읽지만 fung 값은 실행 스레드 하나만 만지므로 칸에 따로 잠금이 없다.

### 호출 규약

- `CALL 위치인자수 이름인자수`. 스택에 [함수, 위치 인자…, (이름 심볼, 값)…].
- VM이 원형의 인자 표를 보고 새 프레임 슬롯에 묶는다: 위치 → 차례로, 이름 → 표에서 찾아, 남는 위치 인자 → `...rest` 리스트, 없으면 E04xx.
- **기본값은 함수 서두(prologue)에 컴파일**된다: "슬롯 i가 비었으면 기본값 식을 실행해 채운다".
- 타입 표기는 서두의 `CHECK_TYPE`, 반환 타입은 `RETURN` 직전 `CHECK_RETURN` (그래서 TCO 제외).
- `TAIL_CALL`은 인자를 임시로 받아 현재 프레임 슬롯에 다시 묶고 `ip`를 0으로.
- Rust 내장 함수(`Native`)는 현재 태스크를 받는다. 람다를 부르는 내장 함수(단계 내장 함수, `NativeBody::Step`)는 직접 부르지 않고
  "이 함수를 불러 달라, 끝나면 이 상태로 이어 달라"(`Flow`·`Next`, 상태 상자 `Cont`)를 돌려주고, VM이 같은 실행 루프에서 불러 결과와 되감기를 상자에 넘긴다
  (`vm/cont.rs`, R2 2026-10-18, Lua 5.2 `lua_callk`와 같은 생각). 태스크마다 실행 루프는 하나다.

### 결과값 축약

- `UNWRAP_ABORT`(`!`), `UNWRAP_RETURN`(`?`).
- `COALESCE L`(`??`): 위를 꺼내 nil/Err면 L로 점프해 오른쪽을 평가, Ok(v)면 v를, 그 외는 그대로 올리고 L 너머로.

### 패턴 매칭

- 패턴을 상수 풀의 **"패턴 트리"**로 두고 `MATCH n` 한 명령이 실행 중에 해석해 슬롯에 묶는다.
- 패턴 의미가 Rust 함수 하나(`vm/matcher.rs`)에 모여 이해하고 고치기 쉽다. 성능이 문제 되면 같은 `MATCH` 인터페이스 뒤에서 명령열 컴파일로 바꾼다.

### 에러

- VM 에러는 구조체 `{번호, 인자, 위치, 호출 경로}`.
- 내장 함수의 패닉은 태스크 경계에서 `catch_unwind`로 잡아 E08xx 내부 에러로 바꾼다. Rust 스택 추적은 절대 사용자에게 가지 않는다.
- 런타임 에러는 태스크 전체를 되감는다 (잡는 구문 없음).

### 태스크

- 태스크는 운영체제 스레드가 아니라 데이터다(`Fiber`: 스택, 프레임들, 이어 가기 상자, 취소 노드, 깨움 표시). 실행 스레드 하나가 대기열의 태스크를
  차례로 돌린다(M:1, `vm/sched.rs`). `spawn`은 `Fiber` 하나를 만들 뿐이다.
- 태스크들이 쓰는 것은 `World{모듈 표(경로 → 모듈 값과 전역 표), 출력 writer, 종료 코드, …}`이고 `Runtime`이 `RefCell`로 쥔다. 다른 스레드(신호, 풀, 표준 입력)는
  값이 없는 `Shared`(대기열, 이벤트 루프, 풀, 정지 표시, 취소 루트)만 본다. 전역은 모듈마다 따로 있고 함수 값이 자기 모듈의 전역 표를 들고 다닌다 (04).
- 취소 확인은 `CALL`·`TAIL_CALL`·뒤로 가는 점프와, 주차한 태스크가 깨어날 때.
- 교착은 스케줄러가 판정한다 (05): 대기열이 비었는데 주차한 태스크 중 타이머나 바깥이 깨울 것이 없으면 바로.
- fung 값(`Rc`)이 스레드를 넘지 않는 것은 타입이 지킨다(`Runtime: !Send`, 손으로 쓴 `unsafe impl Send`는 없음). 동시성 테스트는 `sleep` 대신 상태로 순서를 정한다(`testutil`의 탐침).
- 진입 코드도 별도 스레드에서 돌고 메인 스레드는 기다린다 (05). 끝날 때: 범위가 태스크를 기다리고 되감기가 `with`의 `release`를 지난 뒤 종료 코드 규칙 (05).

### 표준 라이브러리 내장

- `.fung` 파일은 `include_str!`로 실행 파일에 넣고 import 시 컴파일 (06).
- Rust 원시 모듈은 `boot::NATIVE_MODULES` 표로 등록하고 `_native.*` 이름은 사용자 import 불가.

## 마일스톤

| 단계 | 내용 | 확인 방법 |
|------|------|------|
| M0 준비 | 프로젝트, `fung` 명령 뼈대, 종료 코드, 골든 테스트 실행기 | 완료 |
| M1 계산기 | 렉서(01의 글자 수준 규칙 **전부**), Pratt 식 파서, 균일 트리, desugar·resolve 통과, 컴파일러, 스택 VM, int·float·bool, 산술·비교·`not`·`and`·`or`, 오버플로·0 나누기·Inf/NaN 에러, `print`, `fung -e`, `fung run`, `fung dump ast/core/bytecode` | `fung -e 'print(1 + 2 * 3)'` → `7`. 골든 테스트 시작 — 완료 |
| M2 변수와 흐름 | 대입, 줄 이어짐, 블록과 블록 스코프, `if`/`then`/`else`, 문자열·보간·이스케이프, 심볼, 기간, resolve 본격(스코프 규칙 전부), 에러 형식·카탈로그·번호, `fail` | 스코프 함정들이 올바른 에러 번호와 힌트를 낸다 — 완료 |
| M3 함수 | `fn`, 호출 규약, 프레임, 클로저(값 복사), `LOAD_SELF`, `return`, 재귀, TCO, 기본값·이름 인자, 타입 표기 검사, `at_exit`, 스택 한도, 런타임 에러의 호출 경로 출력 | `count(1000000, 0)`이 돈다 — 완료 |
| M4 자료구조와 패턴 | 리스트·맵·튜플·태그·범위, 인덱스·슬라이스, `match`·`MATCH` 명령, 구조 분해, guard, `!` `?` `??`, `\|>`, `_`, `for`, `in`, `type` 별칭, 비교 표(02), 가변 인자 `...rest`(리스트가 생기므로 여기서) | `match`·구조 분해·`?`가 골든으로 돈다 — 완료 |
| M5 모듈과 실사용 | `import`, 원시 모듈 층, 내장 stdlib(embed, import 시 컴파일), prelude, `fs` `path` `exec` `sh` `env` `args` `exit`, `with_open`, `parse_int`, 반복 패턴 함수 | `examples/rename.fung`이 돈다 — 완료 |
| M6 개발 도구 | `test`/`assert`, `fung test`, `fung check`, 경고, REPL(줄 편집 직접 구현), `dbg` | `fung test examples/`가 돈다 — 완료 |
| **알파** | M0~M6 + 실제 스크립트 2~3개 | M0~M6 완료 (2026-10-13); 실제 스크립트는 진행 중 |
| M7 동시성 | 태스크, `spawn` `await` `cancel`(전파), 채널(`recv` 타임아웃, 닫힘 Ok/Err), `atom`, `sleep`, `fan_in`·`pmap`(fung), 종료 코드 규칙, 교착 감지 변환 | `-race` 통과, 골든 9개 — 완료 |
| M7.5 구조 | core / native / extras 네 층(06), 조합 함수를 fung으로 | 층 사이 의존 방향 테스트 — 완료 |
| M8 웹 | `json`, `time`(core 타입 포함), `regex`, `http` — 전부 extras(`_native.*` 다리 + 얇은 fung; `json`도 성능 때문에 네이티브 다리, 06). 스펙 `docs/superpowers/specs/2026-10-14-m8-web-design.md` | 간단한 웹서버 (`examples/serve.fung`) — 완료 (2026-10-14, 골든 8개) |
| M9 포맷터 | `fung fmt`: 토큰 재배치(공백·들여쓰기·빈 줄·정렬, 줄바꿈 유지), `--check`, stdin, 토큰 열 보존 안전장치. 스펙 `docs/superpowers/specs/2026-10-14-formatter-design.md` | 저장소의 모든 `.fung`이 `--check`를 통과(`crates/fung-cli/tests/fmt_repo.rs`) — 완료 (2026-10-14) |
| M10 영속 컬렉션 | 리스트: 영속 벡터 + 보기, 맵: 순서 벡터 + HAMT, 리스트 32개·맵 16개 이하는 평평한 표현, 값 층 밖은 메서드·빌더만. 의미 불변 | 맵 누적·리스트 누적·`rest`/슬라이스 재귀가 2만 개에 0.3초 이하(`crates/fung-cli/tests/perf.rs`, 릴리스), 작은 컬렉션 ±3% — 완료 (2026-10-14) |
| Rust 이식 | M0~M10의 첫 구현을 Rust로 옮겼다: 1 앞단, 2 값·VM, 3a·3b 런타임·에이전트, 4a~4d 네이티브 원시 기능, 5 도구, 6a 릴리스 준비, 6b 첫 구현 삭제, 6c 이식 잔가지 정리. 노트 `docs/notes/R*.md`(기록 `R-port-record.md`) | 6c 완료 (2026-10-18). 다음은 0.2.0 릴리스, R7 스케줄러 |
| 이후 | tree-sitter, LSP(파서 복구), 패키지 관리 — 별도 세션에서 필요한 시점에 | |

M1 렉서를 전부 만드는 이유: 글자 수준 규칙이 닫혀 있고, 렉서는 표 기반 단위 테스트로 한 번에 검증하기 가장 쉬운 부품이다.
M2·M4에서 렉서를 다시 열지 않는다. 파서·VM은 M1에서 숫자·연산만 다룬다.

## 알파 기준

- M0 ~ M6 완료.
- 언어 기능 테스트 전부 통과.
- 실제로 쓰는 스크립트 2~3개를 fung으로 다시 써서 일상에서 사용한다.
  (이 경험으로 prelude와 모듈 목록을 조정한다.)

## 테스트

- 렉서·파서: 표 기반 단위 테스트(크레이트 안 `#[cfg(test)]`).
- 언어 기능: `tests/lang/` 아래 골든 테스트. 기대 출력·에러를 주석으로 적는다.
- VM 명령: `fung dump bytecode` 결과를 비교하는 골든 테스트.
- 표준 라이브러리: M6 이후 fung `test` 블록.

```
# tests/lang/pipe.fung
[1, 2, 3] |> map(x => x * 2) |> print
# expect: [2, 4, 6]
```

```
print(fiels)
# expect-error: E0204
```

- 골든 테스트 실행기(`crates/fung-cli/tests/golden.rs`)는 cargo가 빌드한 `fung` 실행 파일로 각 파일을 **서브프로세스로** 실행하고,
  표준 출력·표준 에러·종료 코드를 비교한다. 사용자가 실제로 겪는 동작 그대로를 검사하기 위해서다(`FUNG_GOLDEN_BIN`으로 다른 바이너리, CI는 musl 릴리스 바이너리).
  - `# expect: 줄`은 순서대로 기대 표준 출력 한 줄. `# expect:`만 있으면 빈 줄.
  - `# expect-error: E0204`는 파일에 하나만. 종료 코드가 1 또는 2이고 표준 에러에 `error[E0204]`가 있어야 한다.
  - 에러를 기대하지 않으면 종료 코드 0, 표준 에러는 비어 있어야 한다.
- **기대 출력 파일** (`crates/*/tests/expected/<주제>/*.txt`): 골든 주석으로 적기 어려운 것(`dump` 세 단계, 실행 전체, `check`·`test`·`fmt`·REPL,
  `json`·`time`·`regex`·`http`·`tls`의 사례 수백 개)은 사례마다 표준 출력·에러·종료 코드를 한 파일에 줄 단위로 적고 테스트가 비교한다.
  각 테스트는 사례 수도 단언한다. 일부러 동작을 바꾼 뒤에는 `FUNG_EXPECTED=bless cargo test -p fung-cli --test <이름>`(fung-vm의 기대 출력은 라이브러리 단위 테스트라 이름으로 거른다: `FUNG_EXPECTED=bless cargo test -p fung-vm runs_match_the_expected_output`)으로 다시 쓰고 diff를 검토한다.
  형식은 `crates/fung-cli/tests/common/expected.rs` 머리 주석.
- 명세 파일 `spec/builtins.txt`·`spec/errors.txt`는 Rust 표에서 생성하고 테스트가 코드와 같은지 지킨다(`FUNG_UPDATE_SPEC=1 cargo test spec_file`로 갱신).

## 참고 자료

- Crafting Interpreters 3부 (바이트코드 VM, 클로저, 호출 프레임) — M1 ~ M3
- Writing A Compiler In Go
- matklad, "Simple but Powerful Pratt Parsing" — M1
