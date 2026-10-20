# 언어·VM 검토 결과와 작업 목록 (2026-10-20)

언어 명세(`docs/design/01`~`08`)와 VM·런타임을 "고칠 수 있는 문제" 기준으로 검토한 결과다.
새 세션이 이 문서만 읽고 이어서 작업할 수 있게 썼다. 항목을 처리하면 체크하고 `처리:` 줄에 커밋과 바꾼 문서를 적는다.

## 0. 이 문서를 쓰는 법

- **기준 커밋**: `273fc6a`(0.3.0 뒤 시험 두 개). 줄 번호는 이 커밋 기준이고 함수 이름을 함께 적었다. 줄이 밀리면 함수 이름으로 찾는다.
- **범위**: 언어 명세와 VM·값 층·동시성 런타임·앞단(렉서·파서·이름 해석·포매터).
  표준 라이브러리 모듈(`http`·`json`·`time`·`regex`·`fs`)의 세부 동작은 범위 밖이다.
- **뺀 것**: open-questions "Rust 구현: 정한 것과 남은 일"의 "남은 일"·"알려진 한계"에 이미 있는 것.
- **재현**: `cargo build --release --locked` 뒤 저장소 루트에서 `F=target/release/fung`.
  - 각 항목의 재현은 그대로 붙여 돈다. 여러 줄 소스는 `x.fung`로 저장해 `$F x.fung`.
  - 동시성 재현은 멈출 수 있으니 `perl -e 'alarm 10; exec @ARGV' $F x.fung`로 시간을 묶는다.
  - 모든 항목은 2026-10-20에 릴리스 바이너리에서 재현을 확인했다(시간·메모리 수치는 M2 Max·macOS).
- **분류**
  - **버그**: 명세가 정답을 말한다. 결정 없이 TDD로 고친다.
  - **결정**: 언어 동작이 바뀌거나 명세가 비어 있다. CLAUDE.md 규칙대로 선택지 + 추천 + 이유로 사용자에게 묻고,
    결정 뒤 해당 설계 문서를 고친다. 각 항목에 선택지와 추천을 적어 두었다(추천은 검토 때의 의견이고 결정이 아니다).
  - **성능**: 의미는 맞고 비용이 문제다. 측정 → 고침 → 같은 측정을 기록한다.
  - **문서**: 설계 문서가 구현과 어긋난다.
- **고칠 때의 공통 절차**(CLAUDE.md)
  - 시험을 먼저 쓰고 실패를 확인한다. 언어 동작은 `tests/lang/*.fung`의 `# expect:`·`# expect-error:`, 내부는 해당 모듈의 `*_tests.rs`.
  - 커밋 전 검사 전체(`cargo fmt --check`, clippy 두 번, `cargo test` 두 번, `cargo deny`·`cargo audit`).
  - 사용자에게 보이는 변화는 `CHANGELOG.md`의 `[Unreleased]`, 문법·내장 함수 변화는 매뉴얼(`docs/*.html`).
  - 새 에러 번호는 `crates/fung-front/src/diag/catalog.rs`에서 붙이고 `FUNG_UPDATE_SPEC=1 cargo test spec_file`.
  - `.fung` 파일이나 출력이 바뀌면 기대 출력을 `FUNG_EXPECTED=bless …`로 다시 쓰고 diff를 검토한다.
  - 명령 번호는 고정이다(`crates/fung-front/src/bytecode/op.rs` 머리 주석). 명령을 더하면 `dump bytecode` 기대 출력이 바뀐다.

## 1. 추천 작업 순서

1. **결정 없이 고칠 버그 묶음**(언어 변경 없음). 셋은 파일이 겹치지 않아 따로(병렬로) 할 수 있다.
   - 진행(2026-10-20): 세 묶음 모두 끝나 main에 스쿼시 머지했다(F7의 결정 부분은 E0222, C7도 함께).
     남은 것은 결정 항목 L1~L11·T1~T4·V8(L8에 달림)·V10, 성능 P1~P7, 문서 D1~D9이다. 2026-10-20에 모든 항목을 처리했다(P2는 알려진 한계로 기록).
   - 앞단: F1 F2 F3 F4 F5 F6 F8 F9 F10 F11 F12
   - VM·값: V1 V2 V3 V4 V5 V6 V7 V9
   - 동시성: C1 C2 C3 C6 (C4·C5는 성능이지만 같은 파일이라 함께)
2. **결정 묻기**. 가장 큰 것은 L1(버린 `Err`). 그다음 L3 L4 L5 T1 L2 L6 L7, 나머지.
3. **교착 보고**(C7) — 출력 모양만 정하면 된다.
4. **성능**: P1~P7, F3.
5. **문서**: D1~D9(결정 항목을 고칠 때 해당 문서도 함께 고친다).

## 2. 목록

| ID | 한 줄 | 분류 | 심각도 |
|---|---|---|---|
| L1 | 식 문장의 `Err`를 조용히 버린다 | 결정 | 높음 |
| L2 | `for`의 `?` 금지가 람다에는 없고, 결과값을 모으는 함수가 없다 | 결정 | 중간 |
| L3 | 값 자리의 `else` 없는 `if`가 조용히 `nil` | 결정 | 중간 |
| L4 | 대문자 이름이 패턴 축약·대입에서 조용히 심볼이 된다 | 결정 | 중간 |
| L5 | 기간을 숫자로 바꿀 길이 없다 | 결정 | 중간 |
| L6 | 파이프와 `_` 부분 적용, 파이프 오른쪽의 태그 | 결정 | 낮음 |
| L7 | 채널 자동 닫힘이 자연스러운 코드에서 교착 | 결정 | 중간 |
| L8 | NFC 정규화로 `m.k`와 `m["k"]`가 어긋남 | 결정 | 낮음 |
| L9 | 최상위 `fn`이 최상위 값을 못 봐 모듈 상태·상수를 못 씀 | 결정 | 중간 |
| L10 | 엄격함과 긴장하는 결정들(`??`, 타입이 다른 `==`, 문자열 키) | 결정(논의) | 낮음 |
| L11 | `supervise` 에러 값에 진단 머리가 문자열로 섞임 | 결정 | 낮음 |
| T1 | 인자 없는 태그를 타입 표기로 못 씀 | 결정 | 중간 |
| T2 | `type_of`는 `'file`인데 타입 표기 `file`이 없음 | 결정 | 낮음 |
| T3 | `...rest`의 타입 표기가 모든 호출을 실패시킴 | 결정 | 낮음 |
| T4 | 반환 타입이 있으면 자기 꼬리 재귀도 스택을 씀 | 결정 | 낮음 |
| F1 | prelude 이름이면 E0206이 빠짐 | 버그 | 중간 |
| F2 | 람다·패턴 매개변수의 중복 이름 검사 없음 | 버그 | 중간 |
| F3 | 중첩 인덱스 파싱이 지수 시간 | 버그(성능) | 중간 |
| F4 | 줄 이어짐 규칙 위반 여러 곳 | 버그 | 중간 |
| F5 | `fung fmt`가 올바른 입력에서 내부 에러 | 버그 | 낮음 |
| F6 | 패턴이 부호 없는 `9223372036854775808`을 받음 | 버그 | 낮음 |
| F7 | EBNF보다 느슨하게 받는 것들 | 버그(일부 결정) | 낮음 |
| F8 | 패턴 매개변수 뒤 기본값이 그 이름을 못 봄 | 버그 | 낮음 |
| F9 | `match` 절의 이름을 그 절에서 재대입 못함 | 버그 | 낮음 |
| F10 | `when` 가드 안 블록의 람다 문장이 거절됨 | 버그 | 낮음 |
| F11 | 진단 품질(위치·도움말·연쇄 에러) | 버그 | 낮음 |
| F12 | `assert x in xs` 실패에 양쪽 값이 없음 | 버그 | 낮음 |
| V1 | 실수 `%`·`//`가 틀린 값 | 버그 | 중간 |
| V2 | nil 슬라이스 경계가 열린 끝이 됨 | 버그 | 중간 |
| V3 | 범위의 `in`이 `2.0`을 못 찾음 | 버그 | 낮음 |
| V4 | REPL에서 실패한 구조 분해가 전역 일부를 바꿈 | 버그 | 낮음 |
| V5 | `dur / int`가 f64를 거쳐 정밀도를 잃음 | 버그 | 낮음 |
| V6 | 문자열 1억 글자 한도를 `+`·`replace`가 건너뜀 | 버그 | 낮음 |
| V7 | 아주 넓은 범위에서 `is_empty`·`take`·`last`가 E0503 | 버그 | 낮음 |
| V8 | 출력한 맵 키가 NFC가 아니면 다시 읽을 때 다른 키 | 버그 | 낮음 |
| V9 | 런타임 진단(7.9 MB 메시지, 안쪽 비교 에러, 거꾸로 된 슬라이스) | 버그 | 낮음 |
| V10 | `sym(s)`로 만든 심볼은 영원히 남음 | 결정 | 낮음 |
| C1 | `on_fail`이 받은 실패를 다시 보고하고 종료 코드 1 | 버그 | 중간 |
| C2 | `conc.fan_in`이 소비자가 사라져도 CPU 100%로 영원히 돎 | 버그 | 중간 |
| C3 | 취소된 `await`가 기다림 목록에 남음(서버에서 누적) | 버그 | 중간 |
| C4 | 한 채널의 대기자 N개 취소가 O(N²) | 성능 | 낮음 |
| C5 | 주차한 `release` N개가 O(N²) | 성능 | 낮음 |
| C6 | `swap`의 `f`가 끝없이 돌면 신호가 먹히지 않음 | 버그 | 낮음 |
| C7 | 교착 보고에 위치·기다리는 대상이 없음 | 개선 | 중간 |
| P1 | `unique`가 O(n²) | 성능 | 중간 |
| P2 | 앞에 붙이기 `[x] + acc`가 O(n²) | 성능 | 중간 |
| P3 | 스택처럼 쓰기 `s[..-1] + [x]`가 O(n²) | 성능 | 낮음 |
| P4 | `s[i]`가 접근마다 문자열 전체를 글자 배열로 만듦 | 성능 | 낮음 |
| P5 | `==`에 같은 포인터 지름길이 없음 | 성능 | 낮음 |
| P6 | 타입 검사마다 문자열 할당(호출 2배) | 성능 | 중간 |
| P7 | 상수 리터럴 리스트를 접지 않음 | 성능 | 낮음 |
| D1~D9 | 설계 문서가 구현과 어긋남 | 문서 | 낮음 |

---

## 3. 언어 명세 (L)

### L1. 식 문장의 `Err`를 조용히 버린다 — 결정, 높음

- [x] 처리: 사용자 결정(2026-10-20) 6개 모두 추천대로. ① (가) 버려지는 `Err`는 런타임 에러 E0606 `discarded Err: %s`(위치는 문장의 첫 글자).
  ② `each`의 `f` 결과도. ③ 버려지는 자리의 `for`는 desugar가 `each`로 낮춘다(버려지는 자리는 블록의 마지막이 아닌 식, 그런 자리의 블록·`if`·`match` 가지의 끝, `for` 문장의 본문 끝).
  ④ 파일의 마지막 식도(로더 `keep_value`: REPL과 VM 시험만 값을 남김). ⑤ 일부러 버리기는 `_ = 식`만. ⑥ `await`하지 않은 태스크의 `Err` 결과는 보류(open-questions).
  명령 수는 그대로(`POP`이 늘 검사; 모든 `POP`이 문장 버림이라 표시가 필요 없었다). `extras`의 일부러 버리던 곳 8군데에 `_ =`
  (`conc.fan_in`의 `scope`, `http`의 `_net.close`·`_net.write`·`recv(stop)`·`_serve_on`의 `scope`). 문서 01·02·03·08·09·open-questions, `syntax.html`·`builtins.html`·`internals.html`, CHANGELOG.
  후속(사용자 결정 2026-10-20): `with`의 `release`와 `supervise`의 `on_fail`이 돌려준 `Err`도 E0606(그 콜백의 런타임 에러, `vm::discarded`), `http.serve`의 리스너 해제에 `_ =`.
  `recv(stop)`의 어색함은 써 보며 판단(open-questions).
  시험: 골든 `discard.fung`·`discard_callbacks.fung`·`err_discard_*.fung` 7개, `desugar::tests::discarded_for_becomes_each`, `repl_tests::an_input_prints_its_last_err_and_rejects_a_discarded_one`.
- **증상**: 값을 버리는 자리(식 문장)의 값이 `Err`여도 아무 일 없이 지나간다. `fung check`도 조용하다.
  셸 대용 언어에서 `set -e`를 잊은 셸과 같은 상태다.
  ```
  import fs
  fs.write("/nonexistent/dir/x.txt", "data")
  exec(["false"])
  (tx, rx) = channel(1)
  close(tx)
  send(tx, 1)
  print("reached, exit 0")
  ```
  실행하면 `reached, exit 0`, 종료 코드 0.
- **근거**: `background.md:25`(원칙 1: 조용히 넘어가는 동작은 에러).
- **원인**: 컴파일러가 식 문장 끝에 `POP`을 낸다(`crates/fung-front/src/compiler.rs:157`, `Compiler::block`).
  VM의 `Op::Pop`(`crates/fung-vm/src/vm.rs:765`, `step_loop`)은 값을 보지 않는다.
- **선택지**
  - (가) 버리는 값이 `Err`면 런타임 에러(새 번호 E06xx, 예: "an Err was discarded"; 힌트 `!`·`?`·`match`·`_ = …`).
    `_ = 식`은 명시적 버리기로 그대로 된다. **추천**: 원칙 1에 맞고 비용은 태그 검사 하나다. C2 같은 버그가 바로 드러난다.
  - (나) 경고(W01xx). 정적으로는 어떤 호출이 `Err`를 돌려줄지 몰라 거의 잡지 못한다.
  - (다) 그대로 두고 매뉴얼에 `!` 권장만.
  - 함께 정할 것: `each(xs, f)`가 `f`의 결과를 버리는 것도 같은 규칙으로 볼지(추천: 예, 같은 "버림"이다. `each(xs, x => send(tx, x))`).
- **구현 메모**
  - 명령 수를 늘리지 않으려면 `POP`의 피연산자(지금 0)에 "문장 버림" 표시를 둔다. `Op::has_arg`와 역어셈블러가 바뀐다.
  - 바꾼 뒤 `extras/*.fung`·`examples`·`tests/lang`을 모두 돌려 `Err`를 버리던 곳을 먼저 센다(예: C2의 `extras/conc.fung:16`).
  - 02 "에러 처리와 축약 연산자", 매뉴얼 `syntax.html`의 에러 처리 절, CHANGELOG.

### L2. `for`의 `?` 금지가 람다에는 없고, 결과값을 모으는 함수가 없다 — 결정, 중간

- [x] 처리: 사용자 결정(2026-10-20) 4개 모두 추천대로. 다시 보니 조용한 것은 `map`뿐이었다(`each`는 L1의 E0606, `filter`·`find`·`all`·`any`·`fold`·`sort_by`·`group_by`는 다음 계산에서 에러).
  ① 경고 대신 컴파일 에러 E0226(prelude `map`의 인자 자리 람다 리터럴, 파이프 포함; `FnScope::map_body`). ② `try_map(xs, f)`(첫 `Err`에서 멈춤, 결과값이 아니면 E0605; `prelude.rs` `try_map_fn`).
  ③ `all_ok` 같은 별도 함수는 두지 않음(`try_map(rs, r => r)`). ④ 새 번호 E0226, E0220 힌트에 `try_map`. 문서 03·06, `builtins.html`·`syntax.html`, CHANGELOG.
  시험: `resolve::tests::for_body_return_and_question_outside_fn`, 골든 `try_map.fung`·`err_map_question.fung`·`err_try_map_not_result.fung`.
- **증상**: 03은 `for` 본문의 `?`를 E0220으로 막는다. 이유는 "`Err`가 조용히 리스트 원소가 된다"인데, 같은 일이 `map`에 넘긴 람다에서는 그대로 난다.
  ```
  $F -e 'print(["1", "x", "3"] |> map(s => parse_int(s)?))'
  ```
  결과 `[1, Err({kind: 'parse, message: "not an integer: x"}), 3]`.
  결과값 리스트를 `Ok(리스트)` 또는 첫 `Err`로 모으는 함수도 없어서 이 패턴을 안전하게 쓸 길이 없다.
- **근거**: `03-functions-and-patterns.md:19-23`.
- **원인 자리**: E0220 판정은 `crates/fung-front/src/resolve.rs:397`(`for` 본문만 본다).
- **선택지**
  - (가) prelude에 `map_ok(xs, f)`(`f`가 `Ok`/`Err`를 돌려줌, 첫 `Err`에서 멈춤; 이름은 결정)를 더한다.
    그리고 `map`·`filter`·`each`·`fold`·`sort_by`의 인자 자리에 바로 쓴 람다 안의 `?`에 경고(W01xx)를 낸다. **추천**.
  - (나) 람다 안의 `?`를 모두 금지. `with`의 `use` 같은 정상 용도를 막는다.
  - (다) 그대로.

### L3. 값 자리의 `else` 없는 `if`가 조용히 `nil` — 결정, 중간

- [x] 처리: 사용자 결정(2026-10-20) 6개 모두 추천대로. ① (가) 컴파일 에러 E0223. ② 값을 쓰는 자리는 대입 오른쪽·인자·`return`·연산자·원소·보간·인덱스·`assert`·기본값·`= 식` 본문.
  ③ 람다 `=>` 본문도(부수효과는 `fn(x): … ;;`). ④ 블록 본문 함수의 마지막 `if`는 제외(open-questions). ⑤ W0104는 에러 E0224로(번호 W0104는 쓰지 않음으로 남김). ⑥ `match`는 그대로(E0601).
  `FnInfo::expr_body`(파서가 `=`·`=>` 본문에 켬), 판정은 이름 해석 뒤 한 번 더 도는 `resolve/value.rs`(`for` 본문은 그 `for`가 쓰일 때만, 그래서 REPL 끝의 `for`는 괜찮다).
  저장소 코드에서 걸린 것은 없었다(그 동작을 보이던 시험 파일 셋만 고침). D6(01의 "M6에서 검토" 문장)도 함께 지웠다.
  문서 01·08·open-questions, `syntax.html`·`internals.html`, CHANGELOG. 시험: 골든 `open_if.fung`·`err_open_if_*.fung` 4개·`err_value_ends_in_assign.fung`, `resolve::tests::values_used_must_not_be_nil`, `check/block_value.fung`.
- **증상**
  ```
  $F -e $'x = if false: 1 ;;\nprint(x)'
  ```
  결과 `nil`. `fung check`도 경고하지 않는다(W0104는 대입으로 끝나는 블록만 본다).
  `nil`은 원인에서 먼 다음 연산(`cannot add nil and int`)에서야 터진다.
- **근거**: `01-syntax.md:39`(명세대로 `nil`)와 원칙 1의 충돌. W0104 `08-tooling-and-errors.md:74-75`.
- **원인 자리**: `crates/fung-front/src/resolve/warn.rs:59`(`warn_block_value`, `last_assign`)가 값 자리를 이미 판정한다.
- **선택지**
  - (가) 값 자리(대입 오른쪽·호출 인자·`return`·연산자 피연산자·`= 식` 함수 본문)의 `else` 없는 `if`를 컴파일 에러로. **추천**.
    블록 본문 함수(`fn f(): … ;;`)의 마지막 `if`는 제외한다(부수효과 함수에서 흔하다).
  - (나) W0104를 넓혀 경고만(`fung run`은 경고를 보이지 않으므로 효과가 작다).
  - (다) 그대로.

### L4. 대문자 이름이 패턴 축약·대입에서 조용히 심볼이 된다 — 결정, 중간

- [x] 처리: 사용자 결정(2026-10-20) 4개 모두 추천대로. ① (가) 컴파일 에러 E0225(파서, `parser::tag_as_name`). ② 대입은 왼쪽 전체가 태그 하나일 때만(`Ok(x) = r`·`(Red, x) = p`는 그대로).
  ③ 맵 리터럴 `{Red}`도 E0204 대신 E0225. ④ 번호 하나. (다) 전부 대문자 이름을 변수로 보기는 기각(02에 기록). 01 EBNF의 `pat_entry`·`entry`에서 `TAG` 축약을 지웠다.
  저장소 코드에서 걸린 것은 없었다. 문서 01·02, `syntax.html`, CHANGELOG. 시험: 골든 `tag_keys.fung`·`err_tag_assign.fung`·`err_tag_shorthand_map.fung`·`err_tag_shorthand_pattern.fung`, `parser::tests::map_shorthand`.
- **증상**
  ```
  $F -e $'cfg = {HOME: "/home/me", PORT: 8080}\n{HOME, PORT} = cfg\nprint(HOME, PORT)'
  ```
  결과 `'HOME 'PORT`(값은 사라지고 check도 통과; `dump core`는 `(pentry HOME (pbind (global 1)))`).
  `MAX_RETRIES = 3`은 `fung check`를 통과하고 실행하면 `E0602 value 3 does not match the pattern`. `Red = Red`는 아무 일도 하지 않는다.
  환경 변수·설정 키가 대문자인 스크립트에서 바로 밟는다.
- **근거**
  - `01-syntax.md:303`의 `pat_entry … | TAG`: 축약이 `{Foo: Foo}`라면 값 자리 `Foo`는 심볼 패턴이다(`03:100`).
  - `02-values-and-types.md:56`은 "그 이름의 변수가 있을 수 없으므로" 예약어 축약을 막는데, 대문자도 같은 처지다.
  - `01-syntax.md:115`(대문자 이름은 태그 전용).
- **원인**
  - `crates/fung-front/src/parser/pattern.rs:181`(`pmap`)이 태그 키를 `Pbind`로 바꾼다.
  - `crates/fung-front/src/parser/stmt.rs:56`(`statement`)이 `Tag =` 문장을 `try_destructure`로 보내 심볼 패턴이 된다.
- **선택지**
  - (가) 둘 다 컴파일 에러(힌트: "대문자 이름은 태그다: `{HOME: home}`, `max_retries = 3`"). 01 EBNF의 `pat_entry`에서 `| TAG`를 지운다. **추천**.
    맵 리터럴의 `{Red}` 축약(값이 심볼 `'Red`)도 같은 이유로 막을지 함께 정한다.
  - (나) EBNF대로 심볼 패턴으로 낮춘다(`{HOME}` = `{HOME: 'HOME}`). 쓸모가 없고 함정이 그대로다.

### L5. 기간을 숫자로 바꿀 길이 없다 — 결정, 중간

- [x] 처리: 사용자 결정(2026-10-20) 5개 모두 추천대로. (가) `dur / dur → float`(늘 실수), `dur // dur → int`, `dur % dur → dur`, 정수의 내림 규칙과 오버플로·0 나누기 그대로.
  `/`는 몫의 정수 부분과 나머지 비율을 i128로 따로 구한다(2⁵³ ns를 넘어도 정확). 섞인 `dur // 수`·`dur % 수`·`수 / dur`은 에러 유지. `value/arith.rs`(`dur_ratio`, 정수 `//`·`%`를 `floor_div_i64`·`rem_i64`로 뺌).
  문서 02·07, `syntax.html`, CHANGELOG(`### 더함`). 시험: `value::tests::dur_by_dur_gives_numbers`, 골든 `dur_numbers.fung`·`err_dur_div_zero.fung`·`err_dur_floor_div_int.fung`.
- **증상**: `$F -e 'print(1s / 2s)'` → E0501 `cannot divide dur and dur`. `int(1500ms)`·`float(1500ms)` → E0513. `time.unix(t)`는 초 단위 정수다.
  경과 시간을 ms 숫자로 로그·지표에 남기거나 처리량(바이트/초)을 계산할 수 없다.
- **근거**: `02-values-and-types.md:13`, `07-operators-and-types.md:27-28`(기간끼리는 `+ -`만).
- **선택지**
  - (가) `dur / dur → float`, `dur // dur → int`, `dur % dur → dur`. 07의 내림 규칙 그대로, 0으로 나누면 E0502. **추천**: `d / 1ms`가 ms 수이고 단위가 식에 보인다.
  - (나) `time.ms(d)` 같은 변환 함수.
  - (다) `int(d)`가 나노초. 단위가 숨어 실수를 부른다(비추천).
- **구현 자리**: `crates/fung-vm/src/value/arith.rs`의 `div`·`floor_div`(151)·`rem`(176)에 `(Dur, Dur)`. 02·07 표, 매뉴얼, CHANGELOG. V5와 같은 파일.

### L6. 파이프와 `_` 부분 적용, 파이프 오른쪽의 태그 — 결정, 낮음

- [x] 처리: 사용자 결정(2026-10-20) 3개 모두 추천대로. `_`는 (가) 컴파일 에러 E0227(오른쪽 호출의 바로 그 인자만, 안쪽 호출의 `_`는 그대로). 태그는 (가) desugar(`x |> Ok` → `Ok(x)`, `x |> Pair(2)` → `Pair(x, 2)`).
  `desugar.rs`의 `pipe`. 저장소 코드에서 걸린 것은 없었다. 문서 01·03·07, `syntax.html`, CHANGELOG. 시험: `desugar::tests::pipe_into_tags`·`a_hole_in_a_piped_call_is_an_error`, 골든 `pipe_tags.fung`·`err_pipe_hole.fung`.
- **증상**
  - `$F -e $'fn h(a, b) = a - b\nprint(3 |> h(_, 10))'` → `<fn>`. 파이프 값이 구멍 앞에 들어가 `h(3, _, 10)`의 부분 적용이 된다.
  - `$F -e 'print(1 |> Ok)'` → 실행 중 E0401 `cannot call a value of type sym`.
- **근거**: `07-operators-and-types.md:101`, `03-functions-and-patterns.md:164-177`, `01-syntax.md:312`(`Ok(x)`는 태그의 호출 형태).
- **선택지**
  - `_`: (가) 파이프 오른쪽 호출에 `_`가 있으면 컴파일 에러. **추천**(원칙 7, 규칙을 늘리지 않는다). (나) 파이프 값이 첫 `_` 자리로 간다.
  - 태그: (가) `x |> Ok`를 `Ok(x)`로 푼다(desugar). **추천**: `x |> f`가 `f(x)`이고 `Ok(x)`가 태그의 호출 형태라 일관된다. (나) 컴파일 에러.

### L7. 채널 자동 닫힘이 자연스러운 코드에서 교착한다 — 결정, 중간

- [x] 처리: 사용자 결정(2026-10-20) 4개 모두 추천대로. ① 교착 보고의 힌트(`b1a9d65`): 받으려고 기다리는 태스크가 같은 채널의 보내는 끝을 스스로 쥐면(변수·붙잡은 값·전역) 루트는 `help:`, 다른 태스크는 `, holds its own sending end`.
  ② 지역 변수는 마지막으로 읽힌 문장 뒤에 놓는다: 새 명령 `CLEARLOCAL`(52), 함수 본문 블록의 문장 사이에서만(안쪽 블록은 같은 식의 뒷부분을 몰라 늦게 놓는 쪽으로 안전),
  한 번도 읽지 않는 변수는 끝까지 산다(`let _guard` 관용구), 전역은 그대로(L9 미정). ③ 정적 경고는 하지 않음. ④ `close(tx)`는 그대로.
  문서 03(값의 수명)·05, `syntax.html`·`builtins.html`·`internals.html`, CHANGELOG. 시험: `compiler::tests::locals_are_cleared_after_their_last_read`, `sched_tests::a_deadlock_report_names_a_sending_end_the_receiver_holds`, 골든 `local_lifetime.fung`·`err_local_never_read.fung`.
- **증상**
  ```
  fn main() = scope(fn(s):
    (tx, rx) = channel()
    spawn(s, () => each(0..3, i => send(tx, i)))
    for x in rx: x * 10 ;;
  ;;)
  print(main())
  ```
  결과 `error[E0702]: deadlock: all tasks are waiting`, 종료 코드 1. 함수 지역 변수 `tx`가 살아 있어 `for`가 끝나지 않는다.
  받는 끝만 돌려주는 도우미 함수로 나누면 `[0, 10, 20]`이다. 보고에 아무 정보가 없는 것은 C7.
- **근거**: `05-concurrency.md:44-50`(명세대로다: 지역 변수는 함수가 돌아올 때 사라진다).
- **선택지**
  - (가) 정적 경고: 같은 함수에서 `channel()`의 두 끝을 묶고 `tx`를 쥔 채 그 `rx`를 순회(`for`·`each`·`map` …)하면 W01xx. **추천**(C7과 함께).
  - (나) 지역 이름을 마지막 사용 직후 비운다. 루프가 없는 언어라 블록 단위 생존 분석으로 충분하다.
    닫히는 시점이 빨라지는 언어 변경이라 05:47을 고친다. 큰 값의 메모리도 일찍 풀린다.
  - (다) 그대로 두고 매뉴얼에 안내.

### L8. NFC 정규화로 `m.k`와 `m["k"]`가 어긋난다 — 결정, 낮음

- [x] 처리: 사용자 결정(2026-10-20) 4개 모두 추천대로. (가) 키 자리(`.` 뒤, 맵 리터럴 키, 맵 패턴 키)의 식별자는 쓴 그대로(`Token::raw`·`Token::key`). 모듈 멤버는 NFC로 찾는다
  (`value/index.rs`, `resolve/module.rs` `check_member`). 축약은 키가 쓴 그대로, 변수는 NFC. V8도 함께 풀렸다. 포매터는 원래 소스 바이트를 옮기므로 그대로다.
  문서 01·02, `syntax.html`, CHANGELOG. 시험: 골든 `nfc_keys.fung`, `module_tests::a_module_member_is_found_in_nfc`.
- **증상**: 분해형 `é`(e + U+0301)를 소스에 그대로 쓴 경우
  ```
  printf 'm = {"cafe\xcc\x81": 1}\nprint(m["cafe\xcc\x81"])\nprint(m.cafe\xcc\x81)\n' > n.fung; $F n.fung
  ```
  `m["café"]`는 `1`인데 `m.café`는 E0508 `key not found`. `{café: 1} == {"café": 1}`(둘 다 분해형)도 `false`.
- **근거**: `01-syntax.md:71-72`(이름은 NFC, 문자열은 그대로)와 `02-values-and-types.md:50·57`(`m.k`는 `m["k"]`의 축약, 키는 항상 문자열)의 충돌.
- **원인**: `crates/fung-front/src/lexer.rs:425`(`Lexer::ident`)가 이름 토큰을 정규화하는데, 맵 키 자리의 이름도 이름 토큰이다.
- **선택지**
  - (가) 키 자리(`.` 뒤, 맵 리터럴 키, 맵 패턴 키)의 이름은 정규화하지 않고 쓴 그대로의 문자열로 본다. **추천**: 키는 값이다. V8도 같이 풀린다.
  - (나) 문자열 키도 NFC로 바꾼다. 값을 바꾸므로 01:72에 어긋난다.

### L9. 최상위 `fn`이 최상위 값을 못 봐 모듈 상태·상수를 못 쓴다 — 결정, 중간

- [x] 처리: 사용자 결정(2026-10-20) 6개 모두 추천대로 + 하나 더. (나) 한 번만 대입되는 최상위 이름은 상수이고 최상위 `fn`(과 그 안의 함수)이 호출할 때 읽는다(`LOADGLOBAL`).
  판정은 최상위 문장 수(구조 분해 포함), 초기화 전 읽기는 E0809, REPL은 전역을 늘 호출할 때 읽음, `http.fung` 상수 함수 18개를 상수로(`_http_date`만 함수), 전역 수명은 그대로.
  추가 결정: 최상위 `fn`이 자기 본문에서 묶는 이름은 그 `fn` 전체의 지역이고 같은 이름의 상수를 가린다(E0205로 막는 안은 기존 코드를 깨서 기각).
  `resolve.rs`(`declare_consts`·`consts`·`slots`·`FnScope::top`·`own`; 최상위 이름의 전역 칸을 처음 나온 순서로 미리 잡아 번호는 그대로), VM의 빈 전역 읽기는 E0801 대신 E0809(`global_name`).
  문서 03·08·09, `syntax.html`, CHANGELOG. 시험: 골든 `top_constants.fung`·`err_top_assigned_twice.fung`·`err_top_before_init.fung`·`err_top_lambda_before.fung`,
  `resolve::tests::top_level_fns_see_names_assigned_once` 등, `module_tests::a_module_fn_sees_the_module_constants`, `repl_tests::a_repl_fn_reads_globals_when_called`.
- **증상**
  ```
  # lib.fung
  counter = atom(0)
  fn bump() = swap(counter, x => x + 1)
  ```
  `import "./lib"` 하는 파일을 실행하면 `lib.fung`에서 E0204(help: file-level variable … pass it as an argument).
  모듈의 함수가 자기 모듈의 atom·상수를 쓰지 못한다. 그래서 표준 라이브러리도 상수를 함수로 우회한다
  (`extras/http.fung`의 `fn _redirect_limit() = 10`, `fn _ctls() = [...]` 등 20개 남짓). 이런 함수는 부를 때마다 리스트를 새로 만든다(P7).
- **근거**: `03-functions-and-patterns.md:181-217`(규칙 1~4와 기각안 A). `05-concurrency.md:216`은 "모듈 최상위의 atom은 된다"고만 하고, 그 모듈의 `fn`이 그것을 못 쓴다는 점은 적지 않았다.
- **선택지**
  - (가) 규칙 유지. P7로 비용을 없애고 05:216과 매뉴얼에 "모듈 상태는 인자로 넘기거나 람다 값으로 둔다"를 한 줄 적는다. **추천**: 03의 기각 이유가 여전히 유효하다.
  - (나) 최상위 `fn`이 "한 번만 대입되는 최상위 이름"을 보게 한다(초기화 전에 부르면 E0808류). 기각안 (A)의 변형이라 03을 다시 논의해야 한다.
  - (다) 상수 전용 선언 문법.

### L10. 엄격함과 긴장하는 결정들 — 결정(논의), 낮음

- [x] 처리: 사용자 결정(2026-10-20) 3개 모두 추천대로. `??`는 그대로(모든 `Err`를 받음; 02와 `syntax.html`에 주의와 `match` 예).
  심볼과 문자열의 `==`·`!=`·`in`(·`assert`)은 런타임 에러 E0517(`value::eq`, 컨테이너 안쪽까지; 패턴은 `equal` 그대로). 맵 키는 문자열 그대로, E0514에 `str(…)` 도움말.
  문서 02, `syntax.html`·`builtins.html`, CHANGELOG. 시험: 골든 `sym_str_compare.fung`·`err_sym_str_eq.fung`·`err_sym_str_nested.fung`, `vm::tests`, `cont_tests`, prelude 기대 출력.
- 셋 다 결정된 설계다. 바꿀지는 사용자 판단이고, 검토 의견은 "유지하되 `group_by`에는 힌트"다.
- **`??`가 nil과 모든 `Err`를 삼킨다**(`02-values-and-types.md:145-153`).
  - `fs.read(p) ?? "{}"`는 권한 오류도 기본값으로 바꾼다.
  - 대안: `??`는 nil만, `Err`는 `or_else(r, v)` 같은 함수.
- **타입이 다른 `==`는 `false`**(`02:130`). 가장 흔한 함정은 심볼과 문자열이다.
  - `$F -e $'import json\nback = json.parse(json.stringify({kind: \'not_found}))!\nprint(back.kind == \'not_found)'` → `false`.
  - 대안: 심볼과 문자열의 `==`만 에러로.
- **맵 키가 문자열뿐**(`02:57`). 그래서 prelude `group_by`가 정수 키에서 실패한다.
  - `$F -e 'print(group_by([1, 2, 3, 4], x => x % 2))'` → E0514.
  - 대안: E0514에 `str(…)` 힌트를 붙인다, 또는 `group_by`가 `(키, 원소들)` 리스트를 돌려준다.

### L11. `supervise` 에러 값에 진단 머리가 문자열로 섞인다 — 결정, 낮음

- [x] 처리: 사용자 지시(2026-10-20: 남은 항목은 추천대로). (가) `{kind: 'error, code, message, at}`(`vm/task.rs` `failure_value`, `await`와 `on_fail`이 같은 맵).
  `at`은 파일:줄, 위치가 없으면 없음. `http.serve`의 내부 에러 보고는 `error[${e.code}]: ${e.message}`로 전과 같은 출력. 문서 05, `builtins.html`, CHANGELOG. 시험: 골든 `failure_value.fung` 외.
- **증상**
  ```
  x = supervise(fn(s):
    t = spawn(s, () => 1 / 0)
    await(t)
  ;;, e => nil)
  print(x)
  ```
  결과 `Err({kind: 'error, message: "error[E0502]: division by zero"})`.
  에러 번호와 위치가 따로 없어, 서버 로그에서 종류별로 나누려면 문자열을 잘라야 한다.
- **근거**: `05-concurrency.md:107`(`e = {kind: 'error, message}`).
- **선택지**
  - (가) `{kind: 'error, code: "E0502", message: "division by zero", at: "x.fung:3"}`. `message`에서 진단 머리를 뗀다. **추천**.
  - (나) 유지.
  - 같은 값이 `await`의 `Err`, `on_fail`, `http.serve`의 핸들러 실패 보고에 어떻게 쓰이는지 함께 확인한다.

---

## 4. 타입 표기 (T)

### T1. 인자 없는 태그를 타입 표기로 못 쓴다 — 결정, 중간

- [x] 처리: 사용자 결정(2026-10-20) 3개 모두 추천대로. (가) 타입의 태그 이름은 같은 이름의 심볼도 받는다(대문자로 시작하는 타입 이름은 태그). P6을 함께(아래). `type_of(Red)`는 `'sym` 그대로(07에 적음).
  `value/types.rs`의 `TypeSet`(값 종류 비트 + 태그 심볼 목록, `link`가 적재 때 만든다). 문서 07, `syntax.html`, CHANGELOG. 시험: `value::tests::matches_type_cases`, 골든 `types_enum.fung`·`err_types_enum.fung`.
- **증상**: `$F -e $'fn f(c: Red | Green) = c\nprint(f(Red))'` → E0403 `argument c must be Red | Green, got sym`.
  인자 없는 태그는 심볼이고(`02:112`), 타입 검사는 태그 값의 이름만 본다. 열거형을 타입으로 쓸 수 없고, `type color = Red | Green`도 같다.
- **근거**: `07-operators-and-types.md:109`(태그 이름도 타입). 인자 없는 태그의 경우를 명세가 말하지 않는다.
- **원인**: `crates/fung-vm/src/value/types.rs:6`(`matches_type`)이 `type_name`(태그면 이름, 심볼이면 `"sym"`)과 문자열로 비교한다.
- **선택지**
  - (가) 타입의 태그 이름은 같은 이름의 심볼도 받는다. **추천**: 패턴 `Red`가 심볼 `'Red`에 맞는 것(`03:100`)과 같은 규칙이다.
  - (나) 타입에 심볼 표기(`c: 'Red | 'Green`)를 들인다.
  - (다) 그대로 두고 문서에 한계로.
- P6(검사 비용)과 같은 자리를 고치므로 함께 한다.

### T2. 타입 표기 `file`이 없다 — 결정, 낮음

- [x] 처리: 사용자 지시(2026-10-20: 남은 항목은 추천대로). `file`을 더함(`resolve/fn_.rs` `BUILTIN_TYPES`; 실행 검사는 `TypeSet`이 이미 앎). `conn`·`listener`는 더하지 않음. 문서 07(D3), `syntax.html`.
- **증상**: `type_of(f)`는 `'file`(`02:16`)인데 `$F -e 'fn f(x: file) = 1'` → E0212 `unknown type file`.
- **원인**: `crates/fung-front/src/resolve/fn_.rs:9`(`BUILTIN_TYPES`)와 07의 타입 이름 목록(`07:107`)에 `file`이 없다.
  `type_of`가 내는 이름은 `crates/fung-vm/src/value/format.rs`의 `type_name`이고, `conn`·`listener`도 있다.
- **추천**: `file`을 더한다. `conn`·`listener`는 `_native.net` 내부 값이라 더하지 않는다.

### T3. `...rest`의 타입 표기가 모든 호출을 실패시킨다 — 결정, 낮음

- [x] 처리: 사용자 지시(2026-10-20: 남은 항목은 추천대로). (가) `list`·`any`만 허용, 다른 표기는 컴파일 에러 E0228. 문서 07, `syntax.html`. 시험: 골든 `err_rest_type.fung`.
- **증상**: `$F -e $'fn f(...xs: int) = xs\nprint(f())'` → E0403 `argument xs must be int, got list`(인자 없는 호출도). `fung check`는 통과한다.
- **선택지**: (가) `...rest`에는 `list`(또는 표기 없음)만 허용하고 나머지는 컴파일 에러. **추천**. (나) 표기를 원소 타입으로 해석해 원소마다 검사.

### T4. 반환 타입이 있으면 자기 꼬리 재귀도 스택을 쓴다 — 결정, 낮음

- [x] 처리: 사용자 지시(2026-10-20: 남은 항목은 추천대로). (가) 자기 자신(`SelfFn`)으로의 꼬리 호출은 `TAILCALL`(`compiler/fn_.rs` `call`). 다른 함수로의 꼬리 호출은 그대로. 문서 03, `syntax.html`. 시험: 골든 `types_more.fung`·`err_tail_return_type.fung`, `compile_tail_calls`, `non_tail_recursion_hits_the_frame_limit`.
- **증상**: `$F -e $'fn count(n, acc) -> int = n == 0 then acc else count(n - 1, acc + 1)\nprint(count(200000, 0))'` → E0804.
- **근거**: `03-functions-and-patterns.md:159`(명세대로 제외). 대가는 "안전을 위해 타입을 붙이면 재귀가 깨지는 것"이다.
- **선택지**
  - (가) 같은 함수의 자기 꼬리 호출은 프레임을 다시 쓴다. 반환 검사는 마지막에 한 번 하면 된다(같은 검사가 겹칠 뿐). **추천**: 가장 흔한 경우를 살린다.
  - 다른 함수로의 꼬리 호출은 그대로 둔다(검사가 쌓인다).
  - (나) 명세대로 둔다.

---

## 5. 앞단 (F) — 렉서·파서·이름 해석·포매터

### F1. prelude 이름이면 E0206이 빠진다 — 버그, 중간

- [x] 처리: `08243a4`(2026-10-20).
- **재현**: `$F -e $'if true: lines = ["a"] ;;\nprint(lines)'` → `<fn lines>`. 함수 안에서 `max`도 같다.
  `x`처럼 prelude에 없는 이름이면 E0206이 난다. 블록 밖으로 새면 안 되는 이름이 조용히 내장 함수로 바뀐다.
- **근거**: `03-functions-and-patterns.md:186`(규칙 4), 예시 `03:230-232`.
- **원인**: `crates/fung-front/src/resolve.rs:546`(`Resolver::name`)이 prelude를 `closed` 검사(549)보다 먼저 본다.
- **고침**: `closed`(그리고 E0208의 `later`)를 prelude보다 먼저 본다.

### F2. 람다·패턴 매개변수의 중복 이름 검사가 없다 — 버그, 중간

- [x] 처리: `42af93f`(2026-10-20).
- **재현**
  - `$F -e $'f = (a, a) => a\nprint(f(1, 2))'` → `2`.
  - `$F -e $'fn g(a, (a, b)) = a\nprint(g(1, (2, 3)))'` → `2`.
  - `fn g((a, b), (a, c))`도 통과한다.
  - 일반 이름 둘인 `fn g(a, a)`는 E0110 "duplicate parameter"다.
- **원인**
  - `crates/fung-front/src/parser/fn_.rs:259`(`lambda_params`)에 검사가 없다.
  - 패턴 매개변수는 본문 앞의 `let`으로 풀린다. `crates/fung-front/src/resolve/fn_.rs:58`(`Resolver::function`)이 같은 슬롯을 조용히 덮는다.
- **고침**: resolve에서 일반·패턴 매개변수의 이름을 모두 모아 중복이면 에러.

### F3. 중첩 인덱스 파싱이 지수 시간 — 버그(성능), 중간

- [x] 처리: `36132ae`(2026-10-20). 의심으로 적었던 `match` 절 머리의 지수 증가도 재현돼(가드 중첩 22단 2.3초) 같은 커밋에서 고쳤다: 절 머리를 한 번만 읽는다.
- **재현**
  ```
  python3 -c "print('x=[0]'); print('y = ' + 'x['*22 + '0' + ']'*22)" > i.fung
  time $F check i.fung
  ```
  22단 0.38초, 24단 1.5초, 26단 6.2초(두 단마다 4배). `check`·`fmt`·`run`과 나중의 LSP가 모두 영향을 받는다.
- **원인**: `crates/fung-front/src/parser/expr.rs:484`(`index_or_slice_inner`). 열린 끝 슬라이스 `[a..]`를 위해 `expr(BP_RANGE+1)`로 추측해 읽고 `expr(0)`으로 다시 읽는다. 중첩마다 일이 두 배가 된다.
- **고침**: 한 번만 읽고 이미 읽은 시작에서 Pratt 루프를 이어 가거나, 다시 읽지 않고 `..]`만 확인한다.
- **의심(미확인)**: `match` 절 머리의 추측 읽기(`arm_head_ahead`)도 가드를 다시 읽는다. 괄호 안에 가드 있는 `match`를 겹치면 같은 지수 증가가 있을 수 있다.

### F4. 줄 이어짐 규칙 위반 여러 곳 — 버그, 중간

- [x] 처리: `08b7137`(2026-10-20). 포매터 변형 시험에서 더 드러난 것도 함께: 패턴 괄호 안의 줄바꿈(`Ok(\n y\n) =>`, `[a,\n b] = xs`), `import m (\n a\n)`. 이제 "여는 괄호·쉼표 뒤마다 줄바꿈" 변형이 저장소의 올바른 파일 전부에서 파싱된다.
- **근거**: `01-syntax.md:46-48`(줄이 연산자·`.`·`->`·`=`로 끝나면 이어짐), `01:51`(괄호 안의 줄바꿈은 공백).
- **(a) 여러 줄 람다 매개변수**
  - `$F -e $'print(fold([1, 2], 0, (acc,\n  x) => acc + x))'` → E0110 `unexpected =>`.
  - `(\n) => 1`, `([a,\n b]) => …`, `map(xs, x\n=> x)`도 같다.
  - 원인: `crates/fung-front/src/parser/fn_.rs:220`(`lambda_params_ahead`)이 줄바꿈에서 멈추고, `parser/expr.rs`의 `prefix`가 `x =>`를 줄바꿈을 건너뛰지 않는 `kind_after()`로 본다.
  - 고침: depth·nest가 0보다 크면 줄바꿈을 건너뛴다.
- **(b) `.`·`->`·`type … =` 뒤의 줄바꿈**
  - `$F -e $'m = {a: 1}\nx = m.\n  a'` → E0112.
  - `$F -e $'fn f() ->\n  int = 1'` → E0110.
  - `$F -e $'type t =\n  int'` → E0110.
  - 원인: `parser/expr.rs:215`(`field`), `parser/fn_.rs:50`(`fn_def`), `parser/stmt.rs:271`(`type_alias`)이 `skip_newlines_after_operator`를 부르지 않는다.
- **(c) 괄호 안에서 태그와 `(` 사이의 줄바꿈**
  - `$F -e $'x = [Ok\n  (1)]\nprint(x)'` → 실행 중 E0401 `cannot call a value of type sym`. `[print\n("x")]`는 정상 호출이다.
  - 패턴 `[Ok\n(x)]`는 문법 에러.
  - 원인: `parser/collections.rs:125`(`tag_expr`)와 `parser/pattern.rs:104`(`pat_term_inner`)의 같은 줄 검사.
  - 고침: nest == 0일 때만 같은 줄을 요구한다.

### F5. `fung fmt`가 올바른 입력에서 내부 에러 — 버그, 낮음

- [x] 처리: `9ac6ab8`(2026-10-20).
- **재현**
  - `printf 'print(1); ; print(2)\n' | $F fmt -` (빈 문장은 `01-syntax.md:8`이 허용)
  - `printf 'fn f() = Ok(Ok(1))? ?\nprint(f())\n' | $F fmt -`
  - 둘 다 `error: formatter changed the code of <stdin>; nothing was written`, 종료 코드 1. 안전장치가 막아서 파일은 깨지지 않는다.
- **원인**: `crates/fung-front/src/format/spacing.rs:74·78`(`gap`)이 `Semi`·`Question` 앞을 늘 붙인다. `; ;`가 `;;`로, `? ?`가 `??`로 다시 잘린다.
- **고침**: 두 토큰을 붙였을 때 한 토큰으로 다시 잘리면 한 칸을 둔다.

### F6. 패턴이 부호 없는 `9223372036854775808`을 받는다 — 버그, 낮음

- [x] 처리: `33cdbbb`(2026-10-20).
- **재현**: `$F -e 'match -9223372036854775807 - 1: 9223372036854775808 => print("matched MIN") ;;'` → `matched MIN`.
- **근거**: `01-syntax.md:88-90`(`-` 바로 뒤에서만 허용). 식에서는 E0102다.
- **원인**: `crates/fung-front/src/parser/pattern.rs:46`(`pat_term_inner`)에 `parser/expr.rs`의 `prefix`가 하는 `is_min_int_magnitude` 검사가 없다.
- 관련(미확인 의심): `-0x8000000000000000`은 E0102인데 `-9223372036854775808`은 된다. 진법 리터럴에도 특례를 줄지 정한다.

### F7. EBNF보다 느슨하게 받는 것들 — 버그(일부 결정), 낮음

- [x] 처리: 결정이 필요 없는 셋(`not` 피연산자, 진법 접두사·소수 뒤 `_`, 보간 안 줄바꿈)은 `dd4e94c`(2026-10-20).
  내장 타입 이름 재정의는 사용자 결정(2026-10-20, 추천안 1)대로 컴파일 에러 E0222로 막았다(07 "타입 별칭").
- **`not`이 이항 연산의 피연산자로 온다**(`01-syntax.md:264-266`은 허용하지 않음).
  - `$F -e 'print(1 == not 2 == 3)'` → `false`. 비교 연쇄 금지(E0111)를 우회한다.
- **진법 접두사·소수 뒤의 `_`**(`01:84` "숫자 사이에만")
  - `$F -e 'print(0x_ff)'` → `255`. `0o_7`·`0b_1`·`1h30.5_m`도 받는다.
  - 원인: `crates/fung-front/src/lexer/number.rs:169`(`duration`)이 소수 부분에서 `digits()`(104)의 결과를 무시한다.
- **보간 안의 줄바꿈**(`01:94`): `$F -e $'print("a${1 +\n2}b")'` → `a3b`.
- **내장 타입 이름 재정의**(결정, 작음): `type int = str` 뒤 `fn f(x: int)`가 문자열을 받는다(`07:107-110`). 추천: 내장 타입 이름의 별칭 정의를 에러로.

### F8. 패턴 매개변수 뒤의 기본값이 그 이름을 못 본다 — 버그, 낮음

- [x] 처리: `89eb02f`(2026-10-20). 기본값이 읽는 패턴 이름만 그 기본값 블록에서 다시 맞춰 묶고, 몸통 쪽 사본에서는 기본값에서만 쓰인 이름을 `_`로 바꿔 W0102가 잘못 나지 않게 했다.
- **재현**: `$F -e $'fn f((a, b), c = a) = c\nprint(f((1, 2)))'` → E0204 `undefined name a`.
- **근거**: `03-functions-and-patterns.md:64·68`(기본값은 앞의 인자를 참조할 수 있다).
- **원인**: `crates/fung-front/src/desugar.rs:337-352`(`fn_node`)가 패턴 매개변수를 본문 앞에서 푼다. 기본값을 채운 뒤라 기본값에서는 보이지 않는다.
- **고침**: 패턴 풀기를 뒤의 기본값보다 먼저 한다. 어렵다면 03에 한계로 적는다.

### F9. `match` 절이 묶은 이름을 그 절에서 재대입하지 못한다 — 버그, 낮음

- [x] 처리: `a0325c7`(2026-10-20).
- **재현**: `$F -e 'print(match 1: v => v = v + 1; v ;;)'` → E0205. 반면 `for x in [1, 2]: x = x * 10; x ;;`는 된다.
- **근거**: `03-functions-and-patterns.md:112-113`(절의 이름은 새 이름).
- **원인**: `crates/fung-front/src/resolve/pattern.rs:122`(`match_expr`)가 절 본문을 안쪽 `Do` 블록으로 본다.
- **고침**: 절 본문의 문장을 절 자신의 블록에서 해석한다.

### F10. `when` 가드 안의 블록에 있는 람다 문장이 거절된다 — 버그, 낮음

- [x] 처리: `36132ae`(2026-10-20, F3과 같은 커밋: 블록·`match` 본문에서 가드 표시를 끈다).
- **재현**
  ```
  r = match 1:
    x when any([1], fn(z):
      f = y => y == 1
      f(z)
    ;;) => "ok"
    _ => "no"
  ;;
  print(r)
  ```
  결과 E0110 `unexpected name x`. 같은 `fn` 블록은 가드 밖에서 된다.
- **근거**: `01-syntax.md:185-186`(가드 안의 람다는 괄호 안에 쓰면 된다).
- **원인**: `crates/fung-front/src/parser/stmt.rs`의 `body()`가 `nest`는 0으로 되돌리지만 `guard = true`를 그대로 둔다.
- **고침**: `body()`와 `match_expr`에서 `guard`를 저장하고 끈다.

### F11. 진단 품질 — 버그, 낮음

- [x] 처리: 첫 절 위치는 `36132ae`(가드가 있는 절 머리는 `when`에서 절로 확정해 가드의 에러를 그대로 보고), E0308 도움말과 E0216 연쇄는 `c6a9919`(2026-10-20).
- **첫 절 머리의 문법 에러 위치가 틀린다**
  - `$F -e $'x = match 3:\n  n when n >> 2 => 1\n  _ => 2\n;;'` → `unexpected name n`(2:3). 실제 문제는 `>>`다.
  - 원인: `crates/fung-front/src/parser/pattern.rs:274`(`match_arms`)가 추측 읽기의 에러를 버린다.
- **E0308 도움말이 별칭·경로 import에서 틀린다**
  - `$F -e $'import json as j\nj = 1'` → help `import j as m`(맞는 것은 `import json as m`).
  - 원인: `crates/fung-front/src/resolve/module.rs:11`(`shadow_error`)이 묶인 이름을 쓴다.
- **연쇄 에러가 억제되지 않는다**
  - `$F -e 'match [1]: [a] | [a, ...rest] => print(a) ;;'` → E0216 뒤에 `a`의 E0204가 또 나온다.
  - 근거: `08-tooling-and-errors.md:64-66`(같은 원인의 연쇄 에러는 억제).

### F12. `assert x in xs` 실패에 양쪽 값이 없다 — 버그, 낮음

- [x] 처리: `8516b3f`(2026-10-20). VM의 ASSERT도 `in`·`not_in`을 안다(`crates/fung-vm/src/vm/m6.rs`).
- **재현**: `$F -e $'x = 2\nxs = [1]\nassert x in xs'` → `assertion failed: x in xs`만 나온다.
- **근거**: `08-tooling-and-errors.md:236-238`(맨 바깥이 비교면 양쪽 값). `in`·`not in`은 비교 연산자다(`07:83`, `01:266`).
- **원인**: `crates/fung-front/src/compiler/m6.rs:9`(`is_comparison`)에 `in`·`not_in`이 없다.

---

## 6. VM·값 층 (V)

### V1. 실수 `%`·`//`가 틀린 값을 낸다 — 버그, 중간

- [x] 처리: `48bcdae`. CPython `float_divmod` 방식(`value/arith.rs` `float_divmod`). 나머지가 0이면 나누는 수의 부호(`6.0 % -3.0`은 `-0.0`). 문서 07, `syntax.html`. 시험 `float_floor_div_and_rem_use_the_exact_remainder`, 골든 `arith.fung`.
- **재현**: `$F -e 'print(7.7e40 % 0.1, 123456789012345678.0 % 3.0, 1e300 % 7.0, 1 // 0.1)'`
  - 실제: `-9.671406556917033e24 16.0 0.0 10.0`
  - 기대(Python과 같은 정확한 값): `0.04850743287925943 2.0 1.0 9.0`
  - 결과가 나누는 수보다 크고 부호도 반대다.
- **근거**: `07-operators-and-types.md:18-19`(내림 기준, `%`는 나누는 수의 부호).
- **원인**: `crates/fung-vm/src/value/arith.rs:157`(`floor_div`, `(x/y).floor()`)와 `:182`(`rem`, `x - y*(x/y).floor()`). 반올림된 몫을 쓴다.
- **고침**: CPython `float_divmod` 방식.
  - `m = x % y`(IEEE fmod는 정확)로 나머지를 구하고, `m != 0`이고 부호가 다르면 `m += y`.
  - 몫은 `(x - m) / y`를 내림·반올림 보정한다.

### V2. nil 슬라이스 경계가 열린 끝이 된다 — 버그, 중간

- [x] 처리: `858f80a`. `bound`가 nil을 E0501로(`cannot slice with int and nil`). 문서 02, `syntax.html`. 시험 `nil_slice_bound_is_a_type_error`.
- **재현**: `$F -e $'j = find([1], x => x > 5)\nprint([10, 20, 30][1..j], "hello"[j..2])'` → `[20, 30] he`.
- **기대**: E0501 타입 에러. `[1, 2][nil]`, `1..nil`은 이미 에러다.
- **근거**: `02-values-and-types.md:35`(nil의 인덱스는 즉시 에러, 조용히 퍼지지 않는다).
- **원인**: `crates/fung-vm/src/value/index.rs:121`(`bound`)의 `None | Some(Value::Nil) => Ok(None)`.
  열린 끝은 `SLICE` 명령의 비트로 따로 표시된다(`crates/fung-front/src/compiler/m4.rs:51-65`). VM이 유일한 호출자다.
- **고침**: `Some(Value::Nil)`을 타입 에러로.

### V3. 범위의 `in`이 `2.0`을 못 찾는다 — 버그, 낮음

- [x] 처리: `858f80a`. 소수 부분이 0이고 i64 안인 `Float`을 정수로 바꿔 찾는다(`index.rs` `whole_int`). 시험 `contains_cases`, 골든 `ranges.fung`.
- **재현**: `$F -e 'print(2.0 in 0..5, 2.0 in [0, 1, 2, 3, 4], contains(0..5, 2.0))'` → `false true false`(기대 `true true true`).
- **근거**: `02-values-and-types.md:120·132`(`1 == 1.0`, `in`·`contains`는 `==` 표를 따른다).
- **원인**: `crates/fung-vm/src/value/index.rs:204-205`(`contains`)가 `Value::Int`만 본다.
- **고침**: 소수 부분이 0이고 i64 범위 안인 `Float`도 받는다.

### V4. REPL에서 실패한 구조 분해가 전역 일부를 바꾼다 — 버그, 낮음

- [x] 처리: `de7c280`. `Binder`가 전역 쓰기를 `pending`에 모았다가 패턴 전체가 맞으면 `commit`(실패한 대안은 자기 몫을 버림). 칸(slot) 쓰기는 그대로. 시험 `repl_tests::failed_destructuring_changes_no_global`.
- **재현**: `printf 'a = 5\nb = 6\n{x: a, y: [b]} = {x: 1, y: 2}\nprint(a, b)\n' | $F`
  → E0602 뒤 `1 6`(기대 `5 6`).
- **원인**: `crates/fung-vm/src/vm/matcher.rs:80-90`(`Binder`)가 맞춰 보는 도중에 묶는다. "실패한 매치는 관찰되지 않는다"는 주석의 전제를 REPL이 깬다(파일 실행은 에러로 끝나므로 보이지 않음).
- **고침**: `MATCHORFAIL`은 임시 칸에 묶었다가 성공하면 한꺼번에 쓴다.

### V5. `dur / int`가 f64를 거쳐 정밀도를 잃는다 — 버그, 낮음

- [x] 처리: `48bcdae`. 정수로 나누면 i128 정수 나눗셈, 반은 0에서 먼 쪽(`f64::round`와 같은 기간 리터럴 규칙). `i64::MIN / -1`은 E0503. 문서 07. 시험 `dur_div_by_int_is_exact`, 골든 `durations.fung`.
- **재현**: `$F -e $'d = 2562047h + 1ms\nprint(d / 1 == d, d / 1)'` → `false 2562047h1.000448ms`. `43800h7.000001ms / 1`도 같다.
- **원인**: `crates/fung-vm/src/value/dur.rs:32-41`(`div_dur`).
- **고침**: 나누는 수가 `Int`면 i128 정수 나눗셈. 반올림은 기간 리터럴의 "가장 가까운 나노초"와 같게 한다. 실수일 때만 f64.

### V6. 문자열 1억 글자 한도를 `+`·`replace`가 건너뛴다 — 버그, 낮음

- [x] 처리: `3c6a074`. `+`·`replace`는 만들기 전에(바이트로 먼저 거르고 넘을 때만 글자 수), 같은 구멍이 있던 보간(`CONCAT`, `vm.rs`)은 만든 뒤 검사. 문서 02·07, `syntax.html`·`builtins.html`. 시험 `str_concat_past_the_limit_is_an_error`, `string_size_limits`, `interpolation_past_the_string_limit_is_an_error`.
- **재현**
  - `$F -e $'s = repeat("a", 60000000)\nprint(len(s + s))'` → `120000000`. `join([s, s])`·`repeat`는 E0806이다.
  - `replace(repeat("a", 20000), "a", repeat("b", 10000))`는 2억 글자를 만든다. 길이 × 바꿀 글자 수로 커지므로, 한도 에러가 아니라 메모리 고갈로 프로세스가 죽을 수 있다(기계 보호를 위해 더 크게는 돌리지 않았다).
- **근거**: `02-values-and-types.md:96`.
- **원인**: `crates/fung-vm/src/value/arith.rs:68`(`add`의 `Str`), `crates/fung-vm/src/prelude/strings.rs:136`(`replace_fn`).
- **고침**: 만들기 전에 `MAX_CHARS`(`prelude/strings.rs:152`)로 검사한다. 바이트 길이로 먼저 거르고 넘을 때만 글자 수를 센다(`native/json.rs:409`와 같은 방식).

### V7. 아주 넓은 범위에서 `is_empty`·`take`·`last`가 E0503 — 버그, 낮음

- [x] 처리: `c330f18`. 숨은 다리 `_native.range`(`last(r)`, `slice(r, from, to)`, i128로 셈)를 두고 prelude의 `is_empty`·`last`·`take`·`drop`이 그것을 쓴다(`drop`도 같은 문제였다). 결과가 1억 개를 넘는 `take`·`drop`은 E0806. `atom.rs` `PURE`에 둘을 더함. 문서 06·09, `internals.html`. 시험 `boot::tests::wide_ranges_need_no_length`, 골든 `ranges.fung`.
- **재현**: `$F -e $'r = -9223372036854775808..=9223372036854775807\nprint(is_empty(r))'` → E0503(`extras/prelude.fung:15`). `take(r, 2)`(85)·`last`(70)도 같다.
- **근거**: `02-values-and-types.md:15`(int 폭보다 넓은 범위도 비지 않는다; E0503은 `len()`만). `take`는 개수 때문에 실패하지 않는다(06).
- **원인**: 셋 다 `len`을 거친다. fung에서는 범위의 시작·끝을 꺼낼 길이 없다(`_start(r) = find(r, x => true)`가 우회다).
- **고침**: 범위 경로를 Rust로 옮기거나, 범위의 경계를 주는 내부 원시를 둔다.

### V8. 출력한 맵 키가 NFC가 아니면 다시 읽을 때 다른 키가 된다 — 버그, 낮음

- [x] 처리: L8과 함께(2026-10-20). 키 자리가 쓴 그대로라 출력한 `{é: 1}`(분해형)을 다시 읽으면 같은 키다(`nfc_keys.fung`).
- **재현**: `$F -e 'print({"e\u{301}": 1})'` → `{é: 1}`(분해형을 따옴표 없이). 이 출력을 소스에 붙이면 키가 NFC `é`(한 글자)로 읽혀 다른 키가 된다.
- **근거**: `08-tooling-and-errors.md:151`(출력을 그대로 코드에 붙여도 동작한다).
- **원인**: `crates/fung-vm/src/value/format.rs:70`(`is_plain_key`).
- **고침**: `nfc(k) == k`일 때만 따옴표 없이. L8을 (가)로 정하면 키는 정규화되지 않으므로 이 항목은 저절로 풀린다.

### V9. 런타임 진단 — 버그, 낮음

- [x] 처리: 세 갈래 모두. ① `de7c280`: E0601·E0602·E0603은 값의 처음 200글자 + `…`(`m4.rs` `SHOWN_CHARS`; 200은 이 작업에서 고른 값). 문서 08. 시험 `errors_cut_long_values_in_the_headline`. ② `858f80a`: `compare_items`가 안쪽 에러를 그대로 올린다. 시험 `ordering_error_names_the_elements`. ③ `858f80a`: 새 에러 E0516 `slice start %d is after its end %d (length %d)`(두 끝을 쓴 대로). 문서 02. 시험 `reversed_slice_names_both_ends`.
- **E0601·E0602·E0603이 값 전체를 찍는다**
  - `$F -e '[a] = map(0..1000000, x => x)' 2>&1 | wc -c` → 7,889,051바이트.
  - 원인: `crates/fung-vm/src/vm/m4.rs:187·192`의 `format(&v)`.
  - 고침: 정해진 길이에서 자르고 `…`.
- **안쪽 원소의 비교 에러가 바깥 타입으로 보고된다**
  - `$F -e 'print(sort([[1, "a"], [1, 2]]))'` → `cannot compare list and list`(실제는 str과 int).
  - 원인: `crates/fung-vm/src/value/compare.rs:95`(`compare_items`)의 `.ok()?`. 고침: 안쪽 에러를 그대로 올린다.
- **거꾸로 된 슬라이스의 문구**
  - `$F -e 'print([1, 2, 3][2..1])'` → `index 2 is out of range (length 3)`(범위 밖이 아니다).
  - 원인: `crates/fung-vm/src/value/index.rs:138`(`slice_bounds`)의 `lo > hi` 가지. 고침: 시작이 끝보다 뒤라는 자기 문구.

### V10. `sym(s)`로 만든 심볼은 영원히 남는다 — 결정, 낮음

- [x] 처리: 사용자 지시(2026-10-20: 남은 항목은 추천대로). (가) 06과 `builtins.html`의 `sym`에 "해제되지 않는다, 외부 입력에 쓰지 않는다"를 적었다.
- **내용**
  - 심볼은 전역 인턴 표에 `Box::leak`으로 들어가 해제되지 않는다(`crates/fung-vm/src/value/sym.rs:24`, `intern`).
  - 서버가 요청 값을 `sym`으로 바꾸면 메모리가 계속 는다(Erlang의 atom 고갈과 같은 모양).
  - 덧붙여 `Sym::name()`은 부를 때마다 전역 `Mutex`를 잡는다(태그 값의 `type_name` 등).
- **선택지**
  - (가) 06의 `sym` 설명에 "만든 심볼은 해제되지 않는다. 외부 입력에 쓰지 않는다"를 적는다. **추천**(작고 정직하다).
  - (나) 실행 중에 만든 심볼은 참조 계수 표로 따로 둔다.

---

## 7. 동시성 런타임 (C)

### C1. `on_fail`이 이미 받은 실패를 다시 보고하고 종료 코드를 1로 만든다 — 버그, 중간

- [x] 처리: `02aaa86`(`vm/scope.rs`의 `handled`가 `on_fail`에 넘긴 실패를 되돌리지 않음). 문서 `05-concurrency.md`(범위 절에 한 줄), CHANGELOG.
- **재현**(`on_fail` 도중 바깥 기한이 지남)
  ```
  r = timeout(30ms, () => supervise(fn(s):
    spawn(s, fn(): sleep(10ms); fail("child failed") ;;)
    sleep(1s)
  ;;, fn(e):
    print("on_fail got:", e.message)
    sleep(50ms)
    print("on_fail done")
  ;;))
  print("timeout result:", r)
  ```
  - 실제: `on_fail got: …` 뒤에 `task failed: error[E0803]: child failed …` 보고 전체, `timeout result: Err({kind: 'timeout})`, 종료 코드 1.
  - 기대: 실패는 이미 `on_fail`에 전달됐고 프로그램은 기한을 값으로 처리했다. 두 번째 보고가 없고 종료 코드 0.
- **같은 모순의 다른 모양**
  - `on_fail` 안에서 `cancel(s)`. 범위 핸들은 채널로 받아 온다.
    ```
    (stx, srx) = channel(1)
    r = supervise(fn(s):
      send(stx, s)
      spawn(s, fn(): sleep(10ms); fail("child failed") ;;)
      sleep(1s)
      "body"
    ;;, fn(e):
      s = recv(srx)!
      print("handling:", e.message)
      cancel(s)
      print("after cancel in handler")
    ;;)
    print("r", r)
    ```
    중복 보고에 종료 코드 1이다. 그런데 `cancel(s)` 뒤의 `print` 줄을 지우면 `r Err({kind: 'cancelled})`, 종료 코드 0이다. 취소 지점 뒤에 문장이 있느냐가 결과를 바꾼다.
  - `on_fail`이 `exit(7)`을 불러도 `task failed:`가 한 번 더 찍힌다(종료 코드는 7).
- **근거**: `05-concurrency.md:107-110`. 실패는 `on_fail`로 간다. "범위가 취소되어 **처리하지 못한** 실패"만 `task failed:`로 보고한다.
- **원인**
  - `crates/fung-vm/src/vm/scope.rs:311`(`handled`)이 `on_fail`의 에러가 아닌 멈춤(취소·exit)마다 지금 실패를 줄 앞에 되돌린다(`push_front(current)`).
  - 그러면 `closed`(467)가 그것을 `report_now`로 보고하고 실패 표시를 세운다.
- **고침**: `on_fail`을 부르기 시작한 실패는 전달된 것으로 본다. 처리기를 시작하지 못한 실패만 되돌린다. `on_fail` 자신의 런타임 에러는 지금처럼 범위 실패로 둔다.

### C2. `conc.fan_in`이 소비자가 사라져도 CPU 100%로 영원히 돈다 — 버그, 중간

- [x] 처리: `4cf06a8`(`extras/conc.fung`: `_forward` 재귀, `send`가 `Err`면 전달 범위를 취소해 쉬는 입력도 놓음). 문서 `builtins.html`(`conc.fan_in`), CHANGELOG.
- **재현**
  ```
  import conc
  fn ticker(s, name):
    (tx, rx) = channel()
    fn go(i) = match send(tx, "${name}${i}"):
      Ok(_) => go(i + 1)
      Err(_) => print("producer ${name} stopped")
    ;;
    spawn(s, () => go(0))
    rx
  ;;
  r = scope(fn(s):
    rx = ticker(s, "d")
    take(rx, 2)
  ;;)
  print("direct:", r)
  r2 = scope(fn(s):
    out = conc.fan_in(s, [ticker(s, "a"), ticker(s, "b")])
    take(out, 2)
  ;;)
  print("fan_in:", r2)
  ```
  - 직접 채널은 `producer d stopped`로 끝난다.
  - `fan_in` 쪽은 시간 제한까지 멈추고 CPU를 99.7% 쓴다. 바쁜 상태라 교착 판정도 잡지 못한다.
- **근거**: `05-concurrency.md:44-46`(받는 끝이 모두 사라지면 `send`는 `Err`). 합친 끝이 사라지면 그 사실이 생산자까지 전해져야 한다.
- **원인**: `extras/conc.fung:16`의 `each(r, v => send(tx, v))`가 `send`의 `Err(closed)`를 무시하고 입력을 계속 비운다.
- **고침**: `fn _fwd(r, tx) = match recv(r): Ok(v) => is_ok(send(tx, v)) then _fwd(r, tx) else nil; _ => nil ;;`로 전달한다(검토 때 끝나는 것을 확인함).
  L1의 (가)가 있었으면 바로 드러났을 버그다.

### C3. 취소된 `await`가 기다림 목록에 남는다 — 버그, 중간

- [x] 처리: `d0f7d07`(`WaitList`를 순서 있는 칸 + 구멍 압축 + 16개 넘으면 `FiberId` 색인으로, `await`가 끊길 때 빠짐, `vm/stdin.rs`도 같은 목록). 릴리스 perf 시험 `many_awaits_on_one_task`(2.1초 → 0.22초). CHANGELOG.
- **재현**
  ```
  import time
  scope(fn(s):
    bg = spawn(s, () => sleep(1h))
    each(1..=6, fn(b):
      t0 = time.now()
      scope(fn(c):
        each(1..=10000, i => spawn(c, () => timeout(1ms, () => await(bg))))
      ;;)
      print("batch", b, "of 10000 short awaits took", time.now() - t0)
    ;;)
    cancel(bg)
  ;;)
  ```
  - 같은 묶음이 35 → 66 → 97 → 129 → 157 → 185 ms로 늘고, 메모리도 는다.
  - 한 태스크를 그냥 `await`하는 8만 개는 실행 스레드 시간을 약 1초 쓴다(넣을 때마다 목록 전체를 훑는 O(N²)).
  - 요청마다 `timeout(d, () => await(공유_태스크))`를 하는 서버는 갈수록 느려진다.
- **원인**
  - `crates/fung-vm/src/conc.rs:204`(`await_fn`)가 `park_until`을 쓰고, 기다림이 끝날 때(`on_stop`) 목록에서 빼지 않는다.
  - `crates/fung-vm/src/vm/block.rs:102`(`WaitList::add`)는 중복 검사를 `iter().any(ptr_eq)`로 O(n)에 한다.
- **고침**: `park_until_then`에 기다림을 빼는 `on_stop`을 단다. 기다리는 쪽은 `FiberId` 열쇠의 집합으로 둔다.
  같은 모양이 `vm/stdin.rs`의 `poll`·`leave`에도 있다(측정 안 함).

### C4. 한 채널에서 기다리는 태스크 N개의 취소가 O(N²) — 성능, 낮음

- [x] 처리: `d0f7d07`(`vm/channel.rs`: 줄을 `Line`으로, `leave`는 떠남 표시와 값 버리기만, 떠난 칸이 절반이면 압축). perf 시험 `many_cancelled_receivers_on_one_channel`(3.6초 → 0.25초). CHANGELOG.
- **재현**
  ```
  import time
  (tx, rx) = channel()
  t0 = time.now()
  r = scope(fn(s):
    each(1..=80000, i => spawn(s, () => recv(rx)))
    sleep(1ms)
    cancel(s)
  ;;)
  print("total:", time.now() - t0)
  ```
  - 8만 개 3.5초(1만 70 ms, 4만 0.94초). 같은 수를 `timeout(50ms, () => recv(rx))`로 기한에 끊으면 2.4초다.
  - 비교: `close(tx)`는 0.15초, `sleep` 중인 8만 개의 취소는 0.2초.
- **원인**: `crates/fung-vm/src/vm/channel.rs:192`(`Channel::leave`)가 되감기는 칸마다 보내는 쪽·받는 쪽 줄 전체를 `retain`한다.
- **고침**: 짝 맞추기는 이미 끝난 칸을 건너뛴다(`first_live`). `leave`는 칸에 죽음 표시만 하고, 죽은 칸이 절반을 넘으면 줄을 압축한다.

### C5. 주차한 `release` N개가 O(N²) — 성능, 낮음

- [x] 처리: `d0f7d07`(`World::shields`를 노드 주소 열쇠의 맵으로). perf 시험 `many_releases_that_park`(2.2초 → 0.43초). CHANGELOG.
- **재현**
  ```
  import time
  t0 = time.now()
  r = scope(fn(s):
    each(1..=80000, i => spawn(s, () => with(() => i, r => sleep(10s), r => sleep(1ms))))
    sleep(1ms)
    cancel(s)
  ;;)
  print("total:", time.now() - t0)
  ```
  - 8만 개 1.8초(2만 개 0.17초). `release`가 주차하지 않으면(`r => nil`) 0.22초다.
  - HTTP 서버의 drain·취소가 연결 수천 개에서 이 모양이다.
- **원인**: `crates/fung-vm/src/vm/cleanup.rs:100`(`released`)의 `world.shields.retain(...)`이 매번 전체를 훑는다.
- **고침**: shields를 포인터나 번호를 열쇠로 하는 맵·슬랩으로.

### C6. `swap`의 `f`가 끝없이 돌면 신호가 먹히지 않는다 — 버그, 낮음

- [x] 처리: `19fd78f`(고침의 둘째 안. 원자 표시 대신 `end_turn`이 `swap` 안에서는 차례 몫마다 게시된 사건을 그 자리에서 처리, `vm/stop.rs`의 `apply_in_swap`). CLI 시험 `signals.rs` 둘. CHANGELOG.
- **재현**
  ```
  fn spin(n) = spin(n + 1)
  a = atom(0)
  print("start")
  swap(a, x => spin(x))
  ```
  - `$F x.fung &` 뒤에 `kill -TERM`을 한 번 보내도 살아 있고, 두 번째 TERM에서야 끝난다(143).
  - REPL에서는 같은 `swap`에 Ctrl-C를 세 번 눌러도 멈추지 않는다(kill -9만). 그냥 `spin(0)`은 정상적으로 끊긴다.
- **근거**: `05-concurrency.md:58-62`(첫 신호는 정지 요청, 보는 쪽이 없으면 루트 취소).
- **원인**
  - `crates/fung-vm/src/vm/block.rs:507`(`end_turn`)은 `pure > 0`(swap의 `f` 안)이면 `turn_due`를 세우지 않는다. 그래서 스케줄러가 사건(`events()`)을 처리하지 못한다.
  - `crates/fung-vm/src/vm/stop.rs:142`(`StopHandle::interrupt`)는 사건을 게시만 한다.
- **고침**: 두 가지 중 하나.
  - 신호 스레드가 루트 취소 노드를 직접 취소한다(`CancelNode`는 스레드 안전). 보는 쪽이 없는 정지 경로도 같게.
  - 또는 원자적 "사건 있음" 표시를 두고 `end_turn`이 swap 안에서도 그것은 본다.

### C7. 교착 보고에 위치와 기다리는 대상이 없다 — 개선, 중간

- [x] 처리: `a3a6f41`(사용자 결정 2026-10-20: 실행 루트는 위치·소스 줄·호출 경로, 나머지는 종류·위치·함수가 같은 것끼리 묶어 `×N`, 10줄까지, 대상은 종류만. `vm/deadlock.rs`·`sched.rs`가 주차한 태스크의 프레임과 상자를 읽음, REPL은 E0702를 다시 찍지 않음). 문서 `05-concurrency.md`(종료 절)·`08-tooling-and-errors.md`(REPL)·`internals.html`, CHANGELOG `### 바뀜`.
- **재현**: L7의 코드, 또는
  ```
  fn worker(rx) = recv(rx)
  fn main() = scope(fn(s):
    (tx, rx) = channel()
    (tx2, rx2) = channel()
    a = spawn(s, () => worker(rx))
    b = spawn(s, () => worker(rx2))
    await(a)
  ;;)
  main()
  ```
  출력은 `error[E0702]: deadlock: all tasks are waiting` 한 줄뿐이다. 위치도, 호출 경로도, 어느 태스크가 무엇을 기다리는지도 없다.
- **근거**: `08-tooling-and-errors.md:19`(런타임 에러는 호출 경로를 붙인다), `05-concurrency.md:134-138`.
- **원인**: `crates/fung-vm/src/vm/deadlock.rs:40·55`가 `error("E0702", no_pos(), …)`로 만든다.
- **고침**: 주차한 태스크마다(앞의 N개까지) 기다리는 종류(`recv`·`send`·`await`·`release` 안 등)와 그 태스크의 현재 위치·호출 경로를 붙인다. 스케줄러는 주차 목록을 이미 갖고 있다.
  출력 모양(줄 수 한도, 순서)만 정하면 된다. 기대 출력·골든의 E0702 사례가 바뀐다.

---

## 8. 성능 (P)

측정은 `/usr/bin/time -p`, 릴리스 빌드, 같은 기계.

### P1. `unique`가 O(n²) — 성능, 중간

- [x] 처리: 사용자 지시(2026-10-20: 남은 항목은 추천대로). 내장 함수로 옮김(`prelude/collections.rs` `unique_fn`, `value/compare.rs` `hash_value`). 10만 개 21.7초 → 0.02초(L10 뒤의 같은 측정). 심볼은 이름 문자열과 같은 해시라 E0517 유지. perf 시험 `unique`.
- **측정**: `$F -e 'print(len(unique(map(0..20000, x => x))))'`. 8천 0.09초, 2만 0.40초, 10만 약 12초. 서로 다른 문자열 10만 개 20.7초.
- **원인**: `extras/prelude.fung:98`의 `fold(xs, [], (acc, x) => x in acc then acc else acc + [x])`. `docs/notes/M7b-structure.md`에 언급만 있고 open-questions에는 없다.
- **고침**: Rust로 옮기고 `==`와 맞는 해시를 쓴다. 정수와 같은 값의 실수는 같은 해시, 맵은 순서와 무관한 해시다.

### P2. 앞에 붙이기 `[x] + acc`가 O(n²) — 성능, 중간

- [x] 처리: 사용자 지시(2026-10-20: 남은 항목은 추천대로). 구조 변경(앞쪽 버퍼·RRB)은 하지 않고 최소안: 매뉴얼에 이미 "뒤에 붙이고 `reverse`" 안내가 있어 open-questions "알려진 한계"에 기록.
- **측정**: `$F -e 'print(len(fold(0..20000, [], (acc, i) => [i] + acc)))'`. 5천 0.08초, 2만 1.13초.
  함수형 언어에서 가장 흔한 누적 모양인데, open-questions의 "누적은 선형"은 뒤에 붙이기만 해당한다.
- **고침**
  - 영속 벡터에 앞쪽 버퍼를 두거나, 이어 붙이기를 RRB 트리로 바꾼다.
  - 최소한 매뉴얼에 "뒤에 붙이고 마지막에 `reverse`"를 적는다.
  - 자리: `crates/fung-vm/src/value/list.rs`, `value/pvec.rs`.

### P3. 스택처럼 쓰기 `s[..-1] + [x]`가 O(n²) — 성능, 낮음

- [x] 처리: 사용자 지시(2026-10-20: 남은 항목은 추천대로). `Pvec::truncate`(O(log n), 앞 잎은 공유)와 `List::push`가 대부분을 덮는 앞쪽 보기면 잘라서 키움. 2만 번 2.2초 → 0.02초. 시험 `pvec_truncate_matches_the_model`, perf `a list used as a stack`.
- **측정**: `$F -e $'base = map(0..20000, x => x)\nprint(len(fold(0..20000, base, (s, i) => s[..-1] + [i])))'`. 1만 0.58초, 2만 2.25초, 4만 9.8초.
- **원인**: `crates/fung-vm/src/value/list.rs:151-168`(`push`). 앞쪽만 남긴 보기(`hi < len`)에 더하면 리스트 전체를 `to_vec`으로 복사한다.
  `slice`가 `n*4 >= len`이면 보기로 남기므로 pop + push마다 다시 복사한다.
- **고침**: `Pvec`에 O(log n) 자르기·pop(Clojure `pop`처럼 경로 복사)을 두고, 자른 복제에 더한다.

### P4. `s[i]`가 접근마다 문자열 전체를 글자 배열로 만든다 — 성능, 낮음

- [x] 처리: 사용자 지시(2026-10-20: 남은 항목은 추천대로). `index.rs`의 `char_len`·`byte_at`(ASCII는 바이트 위치). 1억 글자 최대 메모리 1.3 GB → 0.11 GB, 비ASCII 1천만 글자 인덱스 200번 2.4초 → 0.17초. perf `index a long string`. 글자 수·ASCII 표시를 문자열에 두는 구조 변경은 하지 않음.
- **측정**
  - `/usr/bin/time -l $F -e $'s = repeat("a", 100000000)\nprint(s[0], s[5..7])'` → 최대 상주 메모리 909 MB(문자열만이면 약 109 MB).
  - 1천만 글자에서 `s[0]` 하나에 약 8 ms.
  - 02:23은 시간이 O(n)이라고 적어 두었지만, 메모리까지 O(n)일 이유는 없다.
- **원인**: `crates/fung-vm/src/value/index.rs:78`(`index`)과 `:183`(`slice`)이 `Vec<char>`를 만든다.
- **고침**
  - `chars().nth(i)`(음수는 `.rev()`)와 `char_indices`의 바이트 위치를 쓴다. 할당이 없고 `s[0]`은 O(1)이다.
  - 더 나아가 `Str(Rc<String>)`을 글자 수·ASCII 표시를 가진 구조로 바꾸면 ASCII 문자열의 `s[i]`가 O(1)이 된다(이중 간접도 사라짐).

### P5. `==`에 같은 포인터 지름길이 없다 — 성능, 낮음

- [x] 처리: 사용자 지시(2026-10-20: 남은 항목은 추천대로). `compare.rs` `same_object`(List·Map·Tuple·Tag·Str), `eq`의 E0517 검사도 같은 지름길. 0.4초 → 0초. perf `a shared list equals itself`.
- **측정**: `$F -e $'x = fold(0..24, [1], (a, _) => [a, a])\nprint(x == x)'`. 0.22초이고 한 단마다 두 배(22단 0.06초). 큰 리스트를 자기 자신과 비교해도 O(n)이다.
- **원인**: `crates/fung-vm/src/value/compare.rs:11`(`equal`).
- **고침**: List·Map·Tuple·Tag가 같은 `Rc`면 바로 참. fung에는 NaN이 없으므로 안전하다.

### P6. 타입 검사마다 문자열을 할당한다(호출 2배) — 성능, 중간

- [x] 처리: T1과 함께(2026-10-20). 같은 측정(M2 Max, 3회 중 최소): `int` 둘 0.55초 → 0.36초, 매개변수 셋 + 반환 타입 0.71초 → 0.38초, 표기 없음 0.33~0.34초.
- **측정**: `fn add(a: int, b: int) = a + b`를 300만 번 부르면 0.43초, 표기가 없으면 0.23초. 매개변수 셋이면 0.59초 대 0.31초.
- **원인**: `crates/fung-vm/src/value/types.rs:6`(`matches_type`)이 검사마다 `type_name`(`String`)을 만들고 타입 이름 문자열과 비교한다.
  호출 쪽은 `crates/fung-vm/src/vm.rs`의 `Op::CheckType`·`Op::CheckReturn`.
- **고침**: 원형(`Proto`)의 타입 표를 적재 때 "값 종류 비트 집합 + 태그 심볼 목록"으로 바꿔 할당 없이 검사한다. T1과 함께.

### P7. 상수 리터럴 리스트를 접지 않는다 — 성능, 낮음

- [x] 처리: 사용자 지시(2026-10-20: 남은 항목은 추천대로). 원소가 모두 상수인 비지 않은 리스트 리터럴을 `CONST` 하나로(`compiler/m4.rs` `constant_list`; 빈 리스트를 품으면 제외, `args` 때문). 0.39초 → 0.19초. 맵·튜플은 남김(open-questions).
- **측정**: 문자열 16개짜리 리스트를 `fn _c() = [...]`로 두고 100만 번 `len(_c())`하면 0.36초. 같은 리스트를 변수로 두면 0.13초.
- **원인**: 컴파일러가 원소가 모두 상수인 리스트·맵·튜플 리터럴도 매번 `CONST`들과 `LIST`로 만든다.
- **고침**: 원소가 모두 상수인 리터럴은 컴파일 때 값 하나로 만들어 `CONST` 하나로 낸다. 값이 불변이라 공유해도 안전하다.
  `extras/http.fung`의 상수 함수 20개 남짓(L9)이 바로 이득을 본다. `dump bytecode` 기대 출력이 바뀐다.

---

## 9. 설계 문서 불일치 (D)

모두 문서만 고친다. 결정 항목(L·T)을 처리할 때 해당 문서를 같이 고치는 것과 별개다.

- [x] D1(2026-10-20). `05-concurrency.md:164-165` "취소… 감싸는 함수의 정리는 실행되지 않는다(03의 한계와 같음)". 지금은 `with`의 `release`가 돈다(03 "자원 정리: with").
- [x] D2(2026-10-20). 지운 것이 남아 있다.
  - `06-native-and-stdlib.md:19`(core 예에 `at_exit`)
  - `06:20`과 `05-concurrency.md:211`(`_native.conc`; R1b에서 지움)
  - `06:206-207`(`_native.conc.try_spawn`, "R7에서 … 지운다" — 이미 지움)
- [x] D3(T2와 함께, 2026-10-20). `07-operators-and-types.md:107` 타입 이름 목록에 `file`이 없다(T2 결정과 함께).
- [x] D4(2026-10-20). `08-tooling-and-errors.md:248` "종료 핸들러의 실패는 `FAIL path: <exit handler>`". 종료 핸들러(`at_exit`)가 없고 코드에도 그 문구가 없다.
- [x] D5(2026-10-20). `08:97`의 `fung dump core -e 'files |> filter(is_log) |> map(size)'` 예. 지금은 이름 해석까지 해서 E0204로 실패하고, 출력 꼴도 `(do (call (prelude map) …))`다.
- [x] D6(L3과 함께, 2026-10-20). `01-syntax.md:40` "(… 경고를 낼지는 M6에서 검토.)". W0104로 끝났다.
- [x] D7(2026-10-20). `09-implementation-plan.md:106` 값 목록의 `Channel`·`Agent`. 지금은 `Sender`·`Receiver`·`Atom`·`Scope`.
- [x] D8(2026-10-20). `05-concurrency.md:192` "모듈 초기화는 모듈당 뮤텍스로". M:1 뒤로 뮤텍스가 없다(`vm/module.rs`의 실행 중 표시).
  `crates/fung-front/src/bytecode/op.rs:40` 주석의 "E0207 if unassigned"도 낡았다(E0207은 폐기, REPL은 E0808).
- [x] D9(2026-10-20). open-questions "알려진 한계"
  - "형제 태스크가 시작하는 순서… 실행권 줄은 FIFO지만 스레드가 줄에 서는 시점에 달림"은 M:1 전의 설명이다.
  - "이름 조회(`getaddrinfo`)는 끊을 수 없어 스레드에서 돈다 … 개수로 묶이지 않음"은 같은 목록 위쪽의 "이름 조회는 풀(최대 64)"과 어긋난다.

---

## 10. 구조상 어쩔 수 없는 부분 (참고, 작업 아님)

다시 제기하지 않도록 적어 둔다.

- **동적 타입**: 태그·맵 키 오타, `match`에서 빠진 경우, 잘못된 인자 타입은 실행해야 드러난다. 타입 표기도 그 가지가 돌 때만 잡는다.
- **참조 계수(`Rc`)**: 순환을 회수하지 못한다. 그래서 E0705·E0708처럼 순환이 생기는 길을 하나씩 막는 규칙이 늘어나고, 원칙 7과 구조적으로 부딪힌다. 추적 GC가 아니면 남는다.
- **M:1 스케줄러**: 한 프로세스는 코어 하나만 쓴다. Rust로 쓴 내장 함수 하나가 도는 동안 다른 태스크가 멈춘다. 여러 코어는 여러 프로세스(`reuse_port`)뿐이다.
- **불변 값 + 스택 기반 인터프리터**: 속도 천장이 Python 근처다. 자료 구조를 바꾸지 않으면 남는 O(n)·O(n²)가 있다(문자열 누적, 글자 인덱스).
- **협조적 취소**: 취소·기한은 호출·주차 지점에서만 본다. 끊을 수 없는 시스템 호출은 기다릴 수밖에 없다.
- **UTF-8 전용 문자열**: 바이너리와 잘못된 바이트를 보존하지 못한다(알려진 한계와 같은 뿌리).

## 11. 확인했고 문제없던 것 (다시 검사하지 않아도 됨)

- **호출 핵심**
  - 꼬리 호출(모든 꼬리 위치, 상호 재귀, `x |> f(a)`)과 E0804 깊이 한도(100,000, Rust 스택 넘침 아님).
  - 콜백 내장 함수를 거치는 재귀, 기본값(호출마다, 왼쪽부터), 이름 인자(내장·람다), `_` 부분 적용.
  - `?`·`!`·`??`, 타입 검사 시점, 클로저 복사(쓰는 이름만 붙잡음).
  - 피연산자 한도: 인자·매개변수 4096개 이상은 E0806.
- **값**
  - 정수 넘침 검사, 정확한 int/float 비교, `int(실수)`는 버림.
  - 영속 맵: 순서 유지·덮어쓰기 위치·삭제 뒤 다시 넣기·작은→큰 전환의 `==`. 5천 개 넣고 빼기와 넣고 빼기 반복(200만 번)에도 메모리 일정.
  - 영속 리스트: 뒤에 붙이기 선형, `[_, ...rest]` 상수 시간.
  - `sort_by`는 키를 원소마다 한 번 계산하고 안정 정렬이다.
- **동시성**
  - 범위: 본문이 돌아온 뒤 자식 실패, 자식·본문의 `cancel(s)`, 끝난 태스크의 `cancel`, 닫힌 범위 태스크의 `await`, 자기 자신 `await`는 E0702.
  - 기한: 겹친 기한, `d <= 0`, 바깥 범위에 띄운 태스크는 안쪽 기한에 살아남음(Trio와 같음).
  - `with`: `release`의 에러·`exit`, `use`의 `Err`·취소, `acquire` 취소, 멈춘 `release`를 E0702가 끊음, `use`의 스택 넘침.
  - `supervise`의 `on_fail` 순서와 그 에러.
  - atom: E0707·E0708(`map`·`each`·기본값 경유 포함), 경합 `swap` 10만 번에 갱신 유실 없음.
  - 채널: 닫기·자동 닫힘, 취소된 상대와의 직접 전달, 다른 채널로 넘긴 끝, 받는 쪽 8개에 유실·중복 없음.
  - 정지·종료 코드, `release` 중의 SIGTERM, 표준 입력·풀·외부 명령 기다림의 교착 판정.
  - 부하: 바쁜 형제 옆의 타이머 오차 약 0.06 ms, `sleep` 2만 개, 주차한 태스크 10만 개(개당 약 1.7 KB), 채널 값 100만 개 0.49초.
- **문법 무작위 시험**: 약 6,000개 프로그램을 포매터에 돌려 멱등성과 의미 변화를 봤고 문제가 없었다(F5의 두 경우 제외).

## 12. 확인하지 못한 의심

- ~~앞단: `match` 절 머리 추측 읽기의 지수 증가~~ → 재현돼 F3에서 고쳤다.
- 앞단: 같은 이름 인자를 두 번 주는 `f(a: 1, a: 2)`가 실행 때만 잡힌다(E0405). 맵 리터럴의 중복 키처럼 컴파일 때 잡을 수 있다.
- 앞단: `xs[a ?? b..]`·`xs[x |> f..]`를 받지 않는다. 열린 끝 슬라이스의 시작을 범위 우선순위로 읽기 때문인데, EBNF `[ expr ] ".."`는 받을 것처럼 보인다.
- 앞단: `Ok()`(인자 0개 태그 값)와 `Ok`(심볼)의 관계를 명세가 말하지 않는다.
- 앞단: 포매터의 절 판정(`format/layout.rs`의 `top_arrow`)과 파서의 절 판정이 다를 수 있다. 들여쓰기만 틀어지고 토큰은 바뀌지 않는다.
- VM: `replace`가 키운 문자열(V6)이 메모리를 다 쓰면 E0806이 아니라 할당 실패로 프로세스가 끝날 수 있다(돌리지 않음).
- VM: `(a, _) | (_, a) when a > 1`에 `(0, 5)`를 주면 맞지 않는다. 가드가 두 번째 대안을 다시 시도하지 않는다(Python도 같다; 명세에 없음).
- VM: `merge(작은 맵, 큰 맵)`이 매번 O(|큰 맵|)이다. 넣은 순서를 지키려면 피할 수 없어 보인다.
- 동시성
  - 교착 보고 뒤에 멈춘 `release`가 `error[E0702]`를 한 줄 더 찍는다(겉모양).
  - 교착 보고 뒤의 `Task::tick`이 `unwound()`가 아니라 E0701로 되감는다(겉모양).
  - `vm/stdin.rs`의 O(n) 중복 검사·제거(C3과 같은 모양).
  - `conc.pmap(limit: n)`이 묶음 단위로 돌아 느린 원소 하나가 묶음 전체를 붙잡는다(처리량 문제).
  - 주차 직전에 세운 `turn_due`가 주차 뒤까지 남아, 깨어나자마자 차례를 한 번 더 넘긴다.
