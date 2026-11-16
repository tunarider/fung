# 언어 명세·바이트코드 VM 점검 결과와 작업 목록 (2026-11-15)

여섯 번째 점검(`2026-11-14-fresh-eyes-review.md`, D136~D158)과 매뉴얼 점검(D159~D161)을 고친 뒤, 언어 명세와 바이트코드 VM 구현을 다시 점검한 결과다.
기준은 CLAUDE.md의 "언어 검토·추천 지침"(아홉 절)이다. 형식은 지난 점검들과 같다. 새 세션이 이 문서만 읽고 항목 하나(또는 한 묶음)를 이어서 고칠 수 있게 썼다.
항목을 처리하면 `처리:` 줄에 커밋과 바꾼 문서를 적는다. 결정 항목은 아직 사용자에게 묻지 않았다(다음 결정 번호는 D162).

## 0. 이 문서를 쓰는 법

- **기준 커밋**: `94ac4ad2`(`fix: a FIFO read before any writer is not the end on Linux`). 줄 번호는 이 커밋 기준이고 함수 이름을 함께 적었다.
- **범위**: 언어 명세(`manual/reference/language/` 15쪽), 앞단(렉서·파서·desugar·resolve·로더·진단), 컴파일러와 바이트코드(수명 분석·검증기), VM 실행 핵심, 값 층과 prelude.
  동시성 런타임의 내부, 표준 모듈, CLI 도구는 범위가 아니다.
- **방법**: 명세 15쪽은 직접 읽고 의심 가는 규칙을 릴리스 바이너리로 돌렸다. 세 영역(앞단, 컴파일러·바이트코드·VM 핵심, 값·prelude)은 따로 점검한 뒤, 주장을 종합 단계에서 다시 돌렸다.
  - 이 문서의 항목은 **모두 종합 단계에서 한 번 더 돌려 같은 결과를 본 것**이다. 예외는 항목에 "다시 돌리지 않음"이나 "재현 안 함(코드 읽기)"이라고 적었다.
  - 모든 실행은 macOS arm64다. 리눅스는 돌리지 않았다.
  - 저장소에는 아무것도 고치지 않았다.
- **뺀 것**: 지난 점검들의 항목(모두 처리), open-questions "남은 일"·"알려진 한계"·보류 목록. 이전 결정에 닿는 항목은 제목에 그 결정을 적고 새 근거를 적었다(지침 8).
- **재현**: `cargo build --release --locked` 뒤 저장소 루트에서 `F=target/release/fung`. 여러 줄 소스는 `x.fung`로 저장해 `$F run x.fung`. "종료 2"는 컴파일 에러, "종료 1"은 실행 중 에러다.
- **ID**: 지난 점검에 이어 매긴다: SP98~, FE83~, VM55~, VA72~. SP 명세(언어 규칙의 결정) · FE 앞단과 진단 · VM 컴파일러·바이트코드·VM 핵심 · VA 값과 prelude.
- **분류**: 버그(명세가 정답을 말함) · 결정(언어 동작이 바뀜: 선택지 + 추천) · 성능 · 정리 · 문서.
- **심각도**: 높음(정상적인 쓰임에서 프로그램이 죽거나 조용히 틀림) · 중간 · 낮음.
- **고칠 때의 공통 절차**: CLAUDE.md 그대로다(TDD, 커밋 전 검사 전체, CHANGELOG, 매뉴얼 다시 만들기, 기대 출력 bless 뒤 diff 검토).

## 1. 총평

항목은 20개다. **높음이 둘이고, 둘 다 "쥐기만 하는 변수"(`_watch = stop_signal()`)의 수명이다.**

- VM55: 블록 안의 `_watch`가, 뒤에 오는 다른 블록이 이름을 하나 묶거나 `match` 절 하나가 실패하면 **조용히 놓인다**. 두 이름이 같은 칸을 쓰기 때문이다. 중간 값에 이름을 붙였느냐에 따라 `request_stop()`이 프로그램을 끝내기도 하고 아니기도 한다.
- VM56: 함수가 꼬리 호출로 끝나면 `_watch`가 그때 놓인다. 그런데 꼬리 자리의 함수가 Rust로 쓴 내장(`map`, `scope`)이면 살고 fung으로 쓴 것(`iterate`, 사용자 함수)이면 죽는다.

명세는 "한 번도 읽지 않는 변수는 함수가 돌아올 때까지 산다"고 적는다(10장, 설계 03). 값이 틀리게 계산되는 코드 생성 버그는 이번에도 없었다(같은 뜻 다른 모양 9,000개 × 3벌, 상수 접기 10,192쌍, 값 공유 6,600개에서 차이 0). 남은 구멍은 지난번처럼 수명 분석의 바깥(칸 배정, 꼬리 호출의 프레임 재사용)에 있다.

그 밖은 세 갈래다.

**첫째, 지난 점검이 넣은 규칙의 귀결(지침 2·5).**
- prelude 이름을 어디서나 가릴 수 있게 한 뒤(D138), 한 블록이 가린 이름을 **옆 블록이나 그 뒤에서** prelude 함수로 쓰면 E0206이다(SP98). `match`의 한 절이 `[min] => min`을 쓰면 다른 절의 `min(xs)`가 컴파일 에러다. 명세에는 없는 동작이다.
- 리터럴이 묶인 이름을 컴파일 때 검사하게 한 뒤(D149), `log_path = nil`을 두고 `if log_path != nil: open_log(log_path) ;;`라고 쓴 **맞는 프로그램이 컴파일 에러**다(SP99). 결정 문서는 "맞는 프로그램은 그대로다"라고 적었다.

**둘째, 컴파일 때 검사가 여전히 모양을 가린다(지침 2의 "가장 단순한 모양에서만").**
- 사용자 함수 `fn wait(d: dur)`의 `wait(1)`은 컴파일 에러인데 내장 `sleep(1)`·`timeout(f, 5)`는 실행해야 걸린다(SP100).
- `1 + "a"`는 컴파일 에러인데 `1.0..3`, `1 in "abc"`, `1["a"]`, `1!`, `assert 1`, `for x in "ab"`는 실행할 때다(SP101). 음수 리터럴은 아예 리터럴로 보지 않는다(FE83).
- `x = a ?? return 0`은 E0132인데 `x = a == nil then return 0 else a`는 된다(SP102).

**셋째, 문자열의 남은 성능 절벽 둘(VA72·VA73).** 지난 점검이 슬라이스를 보기로 바꿨는데(VA69), UTF-8이 아닌 바이트가 하나라도 든 문자열과 `trim`류는 그 길에서 빠져 있다.

**단단한 곳**: 앞단은 약 900개 입력에서 패닉·내부 에러가 없었다. 값 층은 함수 95개 × 값 20가지의 타입 행렬, 표현 문턱(리스트 15가지 길, 문자열 6단위 × 14길이 × 8가지 길), 30만 겹 중첩, 수·기간 경계에서 아래 항목 말고는 명세대로다. 검증기는 큰 프로그램 16가지에서 E0801이 없었다. VM 장(08·09·10·internals)의 명령 표와 설명은 코드와 같다.

## 2. 추천 작업 순서

1. **높음 둘**: VM55(결정 없이 고칠 수 있다) → VM56(결정).
2. **맞는 프로그램을 막는 것**: SP98 → SP99. 둘 다 결정이고, 둘 다 여섯 번째 점검의 결정에 닿는다.
3. **결정 없이 고칠 것**: FE83 · VM57 · VA75 · FE85.
4. **검사를 넓히는 결정**: SP100 → SP101 → SP104(SP99의 결정에 달렸다) → SP105.
5. **작은 결정**: SP102 · SP103 · VA74.
6. **성능**: VA73 → VA72 → FE84 → VM58 → VM59.
7. **문서**: SP106.

## 3. 목록

| ID | 제목 | 분류 | 심각도 |
|---|---|---|---|
| SP98 | 블록이 가린 prelude 이름을 옆 블록이나 그 뒤에서 쓰면 E0206 | 결정 | 중간 |
| SP99 | 리터럴이 묶인 이름의 검사가 맞는 프로그램을 막는다 | 결정 | 중간 |
| SP100 | 내장 함수의 리터럴 인자는 실행할 때에야 걸린다 | 결정 | 중간 |
| SP101 | 리터럴 타입 검사가 적힌 연산 몇 개에서 멈춘다 | 결정 | 낮음 |
| SP102 | 값 자리의 `return`(E0132)이 `then … else`의 가지로는 된다 | 결정 | 낮음 |
| SP103 | 매개변수 기본값에서 prelude 이름의 뜻이 자리마다 다르다 | 결정 | 낮음 |
| SP104 | import한 상수는 리터럴로 따라가지 않는다 | 결정 | 낮음 |
| SP105 | 효과 판정이 람다 안에서 부른 매개변수를 놓친다 | 결정 | 낮음 |
| SP106 | 문법의 `import a.b`와 낡은 주석 | 문서 | 낮음 |
| FE83 | 음수 리터럴은 컴파일 때 검사의 리터럴이 아니다 | 버그 | 낮음 |
| FE84 | `match` 절의 덮임 검사가 절 수의 제곱이다 | 성능 | 낮음 |
| FE85 | 진단의 작은 것들 | 정리 | 낮음 |
| VM55 | 블록 안의 쥐기만 하는 변수가 뒤 블록 때문에 놓인다 | 버그 | 높음 |
| VM56 | 쥐기만 하는 변수가 꼬리 호출에서 놓이고, 내장이 Rust냐 fung이냐에 달렸다 | 결정 | 높음 |
| VM57 | 이어받은 반환 타입 검사의 순서가 호출 횟수에 달렸다 | 버그 | 낮음 |
| VM58 | 수명 분석이 지역 수 × `return` 수에 비례한다 | 성능 | 낮음 |
| VM59 | 가지 시작의 `CLEARLOCAL`이 가지 수 × 지역 수만큼 나온다 | 성능·결정 | 낮음 |
| VA72 | UTF-8이 아닌 바이트가 든 문자열의 슬라이스는 매번 복사다 | 성능 | 중간 |
| VA73 | `trim`류가 남는 부분을 늘 복사한다 | 성능 | 중간 |
| VA74 | 길이가 다른 리스트의 `<`는 짝 없는 원소를 보지 않는다 | 결정 | 낮음 |
| VA75 | `x * 기간`의 타입 에러가 피연산자를 뒤집어 적는다 | 정리 | 낮음 |

## 4. 명세 (SP)

### SP98. 블록이 가린 prelude 이름을 옆 블록이나 그 뒤에서 쓰면 E0206 — 결정, 중간 (D138·D139에 닿음)

쉬운 말로: `match`의 한 절이나 `for`가 `min`·`len`·`first` 같은 prelude 이름을 변수로 쓰면, 같은 함수의 다른 자리에서 그 prelude 함수를 부를 수 없다.

- 재현 (종료 2):
```
fn smallest(xs, fallback) = match xs:
  []    => fallback
  [min] => min
  _     => min(xs)
;;
print(smallest([3, 1], 0))
```
```
error[E0206]: `min` is not visible here: it was defined inside a block
 help: `min` is bound in a `match` arm, only there; use the match's value instead: `min = match … ;;`
```
  `for len in xs: print(len) ;;` 다음 줄의 `len(xs)`도, `if c: len = 1; print(len) else: print(len(xs)) ;;`의 `else` 가지도 같다. 람다(`g = len => len + 1` 뒤의 `len(xs)`)는 된다.
- 순서와 자리에 따라 갈린다: `match r: Ok(lines) => … ;;` **뒤**의 `lines("a\nb")`는 E0206이고 **앞**으로 옮기면 된다. 최상위 `for (first, last) in …` 뒤의 `fn head(xs) = first(xs)`는 되고 같은 자리의 람다 `xs => first(xs)`는 E0206이다(영역 점검의 재현, 다시 돌리지 않음).
- 명세 근거: `scope.md` 2번 "prelude 함수는 … 어느 자리에서나 묶을 수 있고 **묶은 블록에서** prelude 함수를 가린다", 5번 "블록 안에서 만든 이름은 밖으로 새지 않는다". 같은 쪽의 예 `for len in [1, 2]: print(len) ;; # 된다` 다음 줄에 `len(xs)`를 쓰면 에러다. 블록 밖에서 E0206이 된다는 말은 명세와 `errors.md` 어디에도 없다.
- 이전 기록: 여섯 번째 점검 SP83의 `처리:` 줄에 "블록이 끝난 뒤 그 이름을 읽으면 다른 이름처럼 E0206(prelude 함수로 돌아가지 않음)"이라고 적혀 있다. 고치며 정한 것이고 결정 표(D138·D139)에는 없다.
- 원인 위치: `crates/fung-front/src/resolve.rs` `Resolver::name`(약 770~785행)이 `FnScope::closed`를 prelude 판정보다 먼저 본다. 시험 `resolve/tests.rs` `a_closed_name_is_not_read_as_the_prelude`.
- 선택지:
  - (가) 추천. 블록 밖에서는 그 이름이 prelude 함수다. 규칙이 "가린 블록 안에서만 가려진다" 하나가 된다.
  - (나) 그대로 두고 `scope.md`·`errors.md`에 적고, 도움말을 고친다(지금은 prelude 함수를 부르려던 사람에게 "use the match's value"라고 한다).
- 지침: 2(같은 이름이 앞뒤 순서, 블록이냐 람다냐에 따라 다르다)가 (가)를 가리킨다. 5는 (나)를 가리키는 듯하지만 "규칙에 예외를 붙여야만 잡히는 실수"라 까닭이 되지 않는다.
- (가)가 놓치는 것과 그 실제 비용: `if c: lines = read() ;; print(lines)`처럼 블록 안의 변수를 밖에서 읽는 실수가 그 이름이 prelude 이름일 때만 E0206 대신 함수 `<fn lines>`를 읽는다. 대개 다음 연산에서 타입 에러로 곧바로 드러나고, `print(lines)`만 조용하다. 반대로 지금 동작이 막는 것은 `first`·`last`·`min`·`max`·`len`·`lines`·`keys`를 패턴 이름으로 쓴 평범한 함수다. 확신은 중간이다.
- 처리: 결정 D163((가)). 2026-11-15. `resolve.rs` `Resolver::name`이 prelude 이름에는 `closed`(E0206)를 보지 않는다: 열린 블록이 가리지 않으면(`hiding`) prelude 함수다. 시험 `resolve/tests.rs` `a_prelude_name_is_hidden_only_inside_the_block_that_binds_it`(재현의 일곱 모양, 다시 돌리지 않았던 변형 포함), `scope.md`의 실행 예.

### SP99. 리터럴이 묶인 이름의 검사가 맞는 프로그램을 막는다 — 결정, 중간 (D149 재검토)

쉬운 말로: 스크립트 맨 위에 "끄려면 `nil`"인 설정 상수를 두면, 그 값을 `nil`인지 확인하고 쓰는 코드가 컴파일되지 않는다.

- 재현 1 (종료 2, 실행하면 결코 에러가 아닌 프로그램):
```
log_path = nil
fn open_log(p: str) = p
if log_path != nil: print(open_log(log_path)) ;;
print("done")
```
```
error[E0403]: argument `p` must be str, got nil
  note: `log_path` is bound to a literal on line 1
```
- 재현 2 (종료 2): `max_size = nil` 뒤의 `fn too_big(n) = max_size != nil and n > max_size + 1` → `E0501 cannot add nil and int`. 실행하면 단락 평가로 닿지 않는다.
- 재현 3 (반대 방향, 종료 2): `wait = 5s` 뒤의 `print(wait ?? 30s)` → E0243. `wait = nil`이면 통과한다. "nil이거나 값"인 손잡이는 어느 값을 넣든 한쪽 코드가 에러다.
- 명세 근거: 구현은 `conditions.md`("단 하나의 묶기가 리터럴인 이름도 그 리터럴로 본다")대로다. 문제는 그 규칙의 전제다. 지침 5는 "**이미 에러인 것**은 가능한 한 일찍 잡는다"인데 위 셋은 실행하면 에러가 아니다. 결정 문서의 D149는 "맞는 프로그램은 그대로다"라고 적었다.
- 원인 위치: `resolve/lits.rs` `literal_view`·`check_named_literals`, `resolve/call.rs` `call`의 `annotated_argument_error`.
- 선택지:
  - (가) 그대로 두고 `conditions.md`에 이 귀결을 적는다. 쓰는 법: 손잡이를 계산한 값으로 둔다(`log_path = env("LOG_PATH")`).
  - (나) 추천, 확신은 낮다. 이름을 따라가는 타입 검사(D149)를 되돌린다. 직접 쓴 리터럴(`if 1:`, `f("4")`)만 컴파일 때 본다. 함수가 묶인 이름의 호출 검사(D110, 인자 개수·이름)는 값의 타입이 아니라 그대로 둔다.
  - (다) `nil`이 묶인 이름만 뺀다. 새 예외라 추천하지 않는다(재현 3도 못 고친다).
- 지침: 1·2는 (가)를 가리킨다(직접 쓴 `if false: f(nil) ;;`도 같은 에러이고, 이름이든 리터럴이든 한 규칙이다). 5의 전제는 (나)를 가리킨다.
- 실제 비용: (가)가 막는 것은 타입이 값에 따라 달라지는 설정 상수뿐이다(`debug = false`, `retries = 0`은 문제없다). SRE 스크립트에서 흔한 모양이고, 컴파일 에러라 조용하지는 않지만 상수를 그대로 둔 채 피할 방법이 없다. (나)가 놓치는 것은 `count = "4"` 뒤의 `twice(count)`가 실행할 때 걸리는 것이고, 그 길을 돌리면 곧바로 같은 에러로 드러난다.
- 처리: 결정 D164((나)). 2026-11-15. `resolve/lits.rs`를 지웠다(`literal_view`·`check_named_literals`·`global_lits`·블록의 `lits`). `literal_type_error`와 `annot::argument_error`는 그 자리의 노드만 본다. 효과 판정(`effect.rs` `arg_val`)도 이름을 따라가지 않는다. D149는 미릴리스였으므로 CHANGELOG에서는 그 항목을 지웠다. 시험 `names_bound_to_literals_are_left_to_the_run`(재현 셋 포함), 골든 `err_named_literal.fung`(이제 `ran`을 찍고 실행 때 E0505).

### SP100. 내장 함수의 리터럴 인자는 실행할 때에야 걸린다 — 결정, 중간

쉬운 말로: 내가 쓴 함수에 타입이 틀린 리터럴을 넘기면 실행 전에 잡히는데, 내장 함수에 넘기면 그 줄이 돌 때에야 잡힌다.

- 재현:
```
fn wait(d: dur) = sleep(d)
print("ran")
wait(1)          # 종료 2, "ran"이 찍히지 않는다: E0403 argument `d` must be dur, got int
```
```
print("ran")
sleep(1)         # 종료 1, "ran"이 찍힌 뒤: E0403 argument `d` must be dur, got int
```
  `timeout(() => 1, 5)`, `split("a", 1)`, `take([1], "a")`, `repeat("a", "b")`, `env(1)`, `map([1], 2)`도 실행할 때 같은 번호다.
- 명세 근거: `types.md` "값이 리터럴이라 소스에 드러나면 컴파일 때 같은 번호와 문구로 잡는다: … 대상이 정해진 호출의 리터럴 인자". `functions.md`는 "대상이 정해진 호출"에 prelude 함수를 넣는다. 지침 2의 "내장 함수에만 있고 사용자 함수에는 없는 동작"의 거울상이다.
- 원인 위치: 내장 함수의 표(`spec/builtins.txt`, `fung_vm::boot`)에 매개변수 이름과 기본값 유무만 있고 타입이 없다. Rust 함수가 스스로 인자를 검사한다.
- 선택지:
  - (가) 추천. 내장 함수의 표에 매개변수 타입을 적고(`sleep(d: dur)`), resolve가 사용자 함수와 같은 길로 본다. 함수 레퍼런스의 매개변수 표도 여기서 나온다.
  - (나) 그대로 두고 `types.md`에 "타입 표기가 있는 fung 함수만"이라고 적는다.
- 지침: 2·5가 (가)를 가리킨다. 새 예외는 없다(있던 구분을 없앤다).
- 실제 비용: (나)의 흠은 `sleep(1)`·`timeout(f, 5)`가 실행 중에, 그 길에 닿았을 때에야 드러나는 것이다. 기간 자리에 수를 쓰는 실수는 흔하고, 에러 처리 가지 안이면 그 가지가 처음 도는 날 드러난다. (가)의 비용은 표를 채우는 일과, Rust 쪽 검사와 표가 어긋나지 않게 지키는 시험이다. SP99에서 (나)를 고르면 리터럴을 직접 쓴 호출만 해당한다.
- 처리: 결정 D165((가))와 D173((1b): 인자 타입은 E0403 하나, 표가 곧 검사). 2026-11-16, prelude까지. `NativeParam::types`(`param("d").of("dur")`)와 `Native::check_args`를 `vm/call.rs` `run_native`·`call_plain`이 부른다. `spec/builtins.txt`가 타입을 적고(`sleep(d: dur)`, `cut: str | nil=?`) `Sig::parse`가 읽어 `annot::argument_error`가 리터럴 인자를 본다. prelude의 Rust 함수 매개변수 약 100개와 fung 함수 다섯(`take`·`drop`·`zip`·`first`·`last`)에 타입을 적었다. E0506 `cannot <동사> T`를 내던 약 31개가 E0403이 되었다(`for x in "ab"`도). 맵 키(E0514)와 변환(E0513)은 표에 적지 않았다. 시험 `vm/tests.rs` `a_builtins_parameter_types_are_checked_as_a_fung_functions_are`: 타입 붙은 prelude 매개변수마다 리터럴 13가지를 직접 쓴 것(컴파일 때)과 `id(…)`를 거친 것(실행 때)으로 돌려 같은 E0403인지 본다. 모듈 부분(2026-11-16): `_native.fs`·`path`·`json`·`time`·`regex`·`process`·`net`·`range`의 매개변수 107개에 타입을 적었다(`exec`의 `cmd: list`, `sh`의 `cmd: str`, `fs.open`의 `mode: sym`, 소켓의 `conn`·`listener`는 표에만 있는 이름: 사용자가 볼 수 없는 `_native.net`의 값이다). 문구가 바뀐 것: `exec("ls")`는 `must be list`(전에는 `a list of strings`; 원소가 틀린 것은 그대로), `fs.open(p, "read")`는 `must be sym`(전에는 세 심볼의 나열; 모르는 심볼은 그대로), 타입이 값 검사보다 먼저다(`regex.replace(s, "(", 1)`은 E0515가 아니라 E0403). 다른 함수의 인자를 대신 검사하는 `_native.arg`(`count(name, n)` 등)는 에러가 부른 쪽의 이름을 말하므로 타입을 적지 않았다. 함수 레퍼런스의 시그니처 130개가 타입을 보이고(`:::fn sleep(d: dur)`) `manual.rs`가 표와 같은지 지킨다. 시험은 `fs`·`path`·`json`·`time`·`regex`의 타입 붙은 매개변수까지 같은 길로 본다(`net`은 사용자가 부를 수 없어 같은 검사 코드에 맡긴다).

### SP101. 리터럴 타입 검사가 적힌 연산 몇 개에서 멈춘다 — 결정, 낮음 (D99·D113에 이어)

- 재현: 줄마다 `print("ran")` 다음 줄에 두고 돌렸다.

| 종료 2 (컴파일 때) | 종료 1 (실행할 때) |
|---|---|
| `1 + "a"` · `1 < "a"` · `1s + 1` · `2 ** "a"` (E0501) | `1.0..3` · `"a"..3` (E0501) · `1 in "abc"` (E0501) · `"a" in 5` (E0512) |
| `if 1:` · `true and 1` · `1 then 2 else 3` (E0505) | `assert 1` (E0505) |
| `1(2)` · `1 \|> 2` (E0401) | `1["a"]` · `"a".b` · `[1][true]` · `(1, 2)[0]` (E0509) |
| `-"a"` · `-nil` · `-[1]` (E0506) | `1!` (E0605) · `nil!` (E0604) · `print(...1)` (E0506) · `for x in "ab": … ;;` (E0506) |

- 명세 근거: `conditions.md`가 컴파일 때 보는 것을 연산 이름으로 나열한다(조건, 리터럴 호출, 두 리터럴의 뜻 없는 연산, `-`, `??`). 그런데 "두 리터럴에 뜻이 없는 연산(E0501)"에는 `1.0..3`과 `1 in "abc"`도 든다. `assert`의 식도 bool이어야 하는 조건 자리다.
- 선택지:
  - (가) 추천. 규칙을 "피연산자의 타입만으로 정해지는 에러는, 피연산자가 리터럴이면 컴파일 때"로 적고 남은 연산(범위, `in`, 인덱스·필드, `!`·`?`, 펼침, `for`의 대상, `assert`)을 같은 길에 넣는다. 나열이 사라진다.
  - (나) 그대로 두고 범위·`in`·`assert`만 넣는다(명세의 지금 문장에 이미 드는 것).
- 지침: 2(이름을 나열하는 규칙, 가장 단순한 모양에서만)가 (가)를 가리킨다.
- 실제 비용: 오른쪽 칸의 식을 리터럴로 직접 쓰는 일은 드물고, 그 줄이 돌면 곧바로 같은 에러다. 그래서 낮음이다. 리터럴이 묶인 이름(`name = "ab"` 뒤의 `for ch in name`)까지 치면 더 흔하지만 그것은 SP99의 결정에 달렸다.
- 처리: 결정 D166((가)). 2026-11-15. `resolve/value.rs` `literal_type_error`에 범위, `in`, 인덱스, 필드, 슬라이스, `!`·`?`, 펼침, `assert`를 넣었다(실행 때의 순서 그대로: 슬라이스는 대상의 타입이 먼저). `conditions.md`의 나열을 한 문장으로 고쳤다. 시험 `vm/tests.rs` `a_type_error_of_literals_is_the_runs_error_found_early`: 리터럴 16가지 × 연산 15가지(1,680개)를 직접 쓴 것과 `id(…)`를 거친 것으로 돌려, 컴파일 때 에러가 실행 때 에러와 번호·문구·도움말까지 같고 타입이 정하는 에러가 실행 때로 남지 않음을 본다. `for x in "ab"`는 `each`·`map`의 인자라 D165(내장 함수의 표)에서 다룬다.

### SP102. 값 자리의 `return`(E0132)이 `then … else`의 가지로는 된다 — 결정, 낮음

- 재현:
```
fn f(a):
  x = a ?? return 0            # E0132 `return` used as a value
  x
;;
fn g(a):
  x = a == nil then return 0 else a          # 된다: g(nil)은 0, g(2)는 2
  y = 1 + (a == nil then return 0 else a)    # 된다
  z = match a: nil => return 0; v => v ;;    # 된다
  print(a == nil then return 0 else a)       # 된다
  x + y + z
;;
```
  `fn h(a) = a > 0 and return 1`도 E0132다.
- 명세 근거: `functions.md` "`return`은 문장이거나 가지의 끝이다. 연산자의 피연산자, 인자, 리스트·맵의 원소, 대입의 오른쪽에 쓰면 E0132다(`x = a ?? return nil`은 `if a == nil: return nil ;;`로)". 그런데 가지가 그 자리들에 있으면 된다. 같은 쪽의 꼬리 호출 규칙은 `a ?? f()`·`a and f()`의 오른쪽을 가지처럼 꼬리 자리로 친다.
- 선택지:
  - (가) 추천. "가지"를 "필요할 때만 평가하는 자리"로 적는다: `if`·`match` 절, `then … else`의 가지, `??`·`and`·`or`의 오른쪽. 꼬리 호출 규칙과 같은 목록이 된다. `port = env("PORT") ?? return Err("PORT is not set")`가 된다.
  - (나) 그대로 둔다. 명세의 문장만 "가지가 어느 자리에 있든"으로 고친다.
  - (다) 값 자리에 있는 가지의 `return`도 막는다. `x = match a: nil => return 0; v => v ;;`가 안 된다.
- 지침: 2(같은 것이 자리에 따라 다르다: 꼬리 호출은 넷 다 가지로, `return`은 둘만)가 (가)를, 3(같은 일을 하는 방법 하나)은 (나)를 약하게 가리킨다.
- 실제 비용: (나)의 흠은 `x ?? return …`을 `if x == nil: return … ;;`와 새 이름으로 풀어 쓰는 두 줄이다. 조용히 틀리는 것은 없다. `a and return 1`은 쓸 까닭이 없지만 해도 없다(지침 5). 확신은 중간이다.

- 처리: 결정 D167((가)). 2026-11-16. 파서 `parser/expr.rs` `infix_loop`가 `??`·`and`·`or`의 오른쪽을 읽기 전에 `return_ok`를 켠다(`then … else`의 가지와 같은 길). `return`은 뒤따르는 식 전체를 값으로 가져간다(`a ?? return b + 1`). 괄호 바로 안(`a ?? (return 1)`)과 왼쪽(`(return 1) ?? a`)은 그대로 E0132다. 오른쪽의 `return f()`는 꼬리 호출이다. 명세 `functions.md`·`grammar.md`(`or_expr`·`and_expr`·`coalesce`), E0132의 설명과 도움말, 골든 `return_branch.fung`, 파서 시험.

### SP103. 매개변수 기본값에서 prelude 이름의 뜻이 자리마다 다르다 — 결정, 낮음 (D139에 닿음)

- 재현:
```
fn render(rows, keys = keys(rows[0])) = keys      # 된다: ["a"]. 기본값의 keys는 prelude 함수
fn pad(s, width = len(s)):                         # E0204: 본문의 대입이 기본값의 len까지 가린다
  len = width * 2
  repeat(s, len)
;;
```
  `fn f(xs, n = len(xs), len = 3) = n + len`은 4를 찍는다. 보통 이름의 `fn f(a = b, b = 1)`은 E0204다(뒤 둘은 영역 점검의 재현, 다시 돌리지 않음).
- 명세 근거: `scope.md` 1번 "묶은 블록에서는 그 블록 전체 … 대입보다 앞에서 읽어도 E0204(`lines = lines(s)`도)", `functions.md` "매개변수는 본문의 지역 이름이다". 기본값이 어느 쪽인지는 적혀 있지 않고, 구현은 "본문의 대입은 기본값을 가리고 매개변수는 가리지 않는다"다.
- 원인 위치: `resolve/fn_.rs` `function`. `push_block(body)`가 본문 문장의 대입만 `hiding`에 넣고 기본값을 그 블록 안에서 걷는다.
- 선택지:
  - (가) 추천. 매개변수도 본문 블록의 이름이므로 가린다: 뒤에 묶이는 매개변수와 같은 이름을 기본값에서 읽으면 E0204(`fn f(a = b, b = 1)`과 같다). `keys = keys(rows[0])`는 `lines = lines(s)`와 같은 에러가 된다.
  - (나) 기본값은 본문 블록 밖이다. 본문의 대입도 매개변수도 기본값을 가리지 않는다.
- 지침: 2가 (가)를 가리킨다(`lines = lines(s)`와 같은 규칙). 깎이는 것은 `keys = keys(…)` 꼴의 기본값이고 매개변수 이름을 바꾸면 된다.
- 처리: 결정 D168((가)). 2026-11-15. `resolve/fn_.rs` `function`이 prelude 이름인 매개변수를 본문 블록의 `hiding`에 넣는다(묶이면 빠진다). 기본값 자리의 E0204 도움말을 따로 적었다. 시험 `a_default_reads_the_names_of_the_body_block`(`fn f(xs, n = len(xs), len = 3)`도 E0204, 보통 이름의 `fn f(a = b, b = 1)`은 전부터 E0204임을 확인).

### SP104. import한 상수는 리터럴로 따라가지 않는다 — 결정, 낮음 (영역 점검의 재현, 다시 돌리지 않음)

- 재현: `lib/util.fung`에 `limit = 3`. `import "./lib/util" (limit)` 뒤의 `limit(1)`은 실행할 때 E0401이고, 같은 파일의 `limit = 3; limit(1)`은 컴파일 때다. 같은 모듈의 **함수**는 컴파일 때 잡힌다(`util.typed("x")` E0403).
- 명세 근거: `conditions.md`의 리터럴 규칙은 "어느 블록의 이름이든"이라고만 하고 import한 이름은 말하지 않는다. 지침 2의 "파일의 역할에 따라 다르다".
- 원인 위치: `resolve.rs` 150~173행이 내보내는 것은 `sigs`(함수)뿐이다. `resolve/lits.rs` `literal_entry`는 파일 안만 본다.
- 고치는 법: SP99에 달렸다. SP99에서 (나)를 고르면 이 항목은 사라진다. (가)면 공개 상수의 리터럴도 내보내 따라간다(그때 SP99의 비용이 설정 모듈로 넓어진다).
- 처리: 결정 D164로 없어졌다(이름을 따라가지 않는다).

### SP105. 효과 판정이 람다 안에서 부른 매개변수를 놓친다 — 결정, 낮음 (D141에 이어)

- 재현 (종료 2, 여섯째 줄에서만):
```
fn total_by(xs, f)  = sum(map(xs, f))
fn total_by2(xs, f) = sum(map(xs, x => f(x)))
orders = [{price: 1}]
total_by2(orders, o => o.price)   # 통과
print("between")
total_by(orders, o => o.price)    # E0234
```
  `sum(for x in xs: f(x) ;;)`로 쓴 것도 통과한다(영역 점검의 재현).
- 명세 근거: `blocks.md` "함수를 받는 함수는 받은 함수가 효과 없을 때만 효과가 없다". 같은 함수가 쓴 모양에 따라 판정이 갈린다. 놓치기만 하고 옳은 코드를 막지는 않는다.
- 원인 위치: `resolve/effect.rs` `param_of`(114행). 지금 함수의 지역 칸만 매개변수로 보고, 안쪽 람다가 붙잡은 바깥 매개변수는 `Callee::Unknown`이 된다.
- 선택지: (가) 추천. 붙잡은 이름이 바깥 함수의 매개변수면 "바깥 매개변수 i에 달림"을 바깥 요약으로 올린다. (나) 명세에 "본문에서 곧바로 부르거나 넘긴 매개변수만"이라고 적는다. 급하지 않다.

- 처리: 결정 D169((가)). 2026-11-16. `resolve/effect.rs`의 요약이 매개변수를 `Dep`(그 매개변수를 가진 함수의 키, 번호)로 적는다. `capture_from`이 붙잡은 이름이 둘러싼 함수의 매개변수인지 `FnScope::cap_params`에 남기고 `param_of`가 `Head::Captured`도 읽는다. `eval`은 부른 함수의 요약 가운데 그 함수 자신의 매개변수만 인자로 풀고, 붙잡은 바깥 매개변수는 그대로 올린다(넘긴 람다도 같다). 다른 파일에 내보내는 요약은 자기 매개변수뿐이다. 시험 `resolve/tests.rs` `effect_free_functions_are_known_from_their_bodies`: 람다, `for` 본문, 두 겹 람다, 지역 `fn`, 그리고 밖으로 나간 람다(`mk(print)(1)`)는 효과 있음.

### SP106. 문법의 `import a.b`와 낡은 주석 — 문서, 낮음

- `grammar.md`의 `import = "import" ( NAME { "." NAME } | PLAIN_STRING ) …`: 점으로 이은 이름은 내부 모듈 `_native.*`뿐이고 사용자는 쓸 수 없다(`import _native.fs`는 E0310, `import fs.x`는 E0301). `modules.md`에는 설명이 없다. 주석으로 "점 이름은 내장 파일의 `_native.*`뿐: E0310"을 적는다.
- `grammar.md` `arm`의 주석 "앞 절의 가드 없는 리터럴과 같은 리터럴 절은 E0235"는 D112 전의 규칙이다. `match.md`의 "덮는 절"로 고친다.

- 처리: 2026-11-16. `grammar.md`의 `import` 주석에 점 이름(`_native.*`뿐, E0310·E0301)을 적고 `arm`의 주석을 "앞의 가드 없는 절이 덮는 절은 E0235"로 고쳤다.

## 5. 앞단과 진단 (FE)

### FE83. 음수 리터럴은 컴파일 때 검사의 리터럴이 아니다 — 버그, 낮음

- 재현 (모두 `print("ran")`이 찍힌 뒤 종료 1; 같은 자리의 양수는 종료 2):
```
if -1: print("a") ;;                 # E0505, 실행할 때
x = -1                               # x = 1이면 컴파일 때
y = not x
fn f(n: int = -1.5) = n              # E0403, 부를 때 (= 1.5는 컴파일 때)
fn g() -> str = -1                   # E0404, 부를 때
```
- 명세 근거: `conditions.md` "리터럴은 문법이 타입을 보이는 모든 값이다: 수 …". `match.md`는 `-1`·`-1s`를 리터럴 패턴으로 친다.
- 원인 위치: 확인하지 않았다. 리터럴의 타입을 정하는 곳(`resolve/lits.rs`와 그것을 쓰는 검사)이 수 리터럴의 단항 `-`를 리터럴로 치지 않는 것으로 보인다.
- 고치는 법: 수·기간 리터럴의 단항 `-`를 같은 타입의 리터럴로 본다. 시험은 위 넷.
- 처리: 2026-11-15. 원인은 `value::known_type`이 `-1`(`(op - 1)`)을 리터럴로 치지 않던 것이다. 수·기간 리터럴의 단항 `-`는 그 리터럴의 타입이다. 시험 `a_negative_literal_is_a_literal`(열두 모양).

### FE84. `match` 절의 덮임 검사가 절 수의 제곱이다 — 성능, 낮음

- 측정 (`fung check`, 문자열 리터럴 절 n개 + `_`): 2,000개 0.54초, 4,000개 2.09초. 정수 절은 4,000개 0.9초, 8,000개 3.7초(영역 점검). 가드 붙은 절 8,000개는 0.08초.
- 원인 위치: `resolve/pattern.rs` `covered_arms`(243행)가 절마다 앞 절 전부와 `covers`를 부르고, `literal_key`(355행)가 매번 `String`을 만든다. `dead_alternatives`(214행)도 같다.
- 고치는 법: 리터럴 절은 키를 한 번 계산해 해시 집합으로 보고, 쌍 비교는 구조 패턴끼리만 한다.
- 실제 비용: 손으로 쓴 `match`에는 보이지 않는다. 생성한 조회 표에서만 드러난다.

- 처리: 2026-11-16. `resolve/pattern.rs` `Seen`: 앞선 패턴 가운데 리터럴은 `literal_key`의 해시 표로 찾고 나머지만 하나씩 `covers`로 묻는다(`covered_arms`, `dead_alternatives`). 문자열 절 8,000개와 대안 8,000개의 `match` 둘이 든 파일이 12초 → 0.03초. 시험 `perf.rs` `many_literal_arms_compile_in_linear_time`.

### FE85. 진단의 작은 것들 — 정리, 낮음 (5만 다시 돌림)

1. 바로 부르는 함수 리터럴의 E0234: `(x => x + 1)(2)` → "the result of \`the function(…)\` is …", 밑줄이 어긋난다(`resolve/effect.rs` `note_call`, `resolve/value.rs` 403행).
2. 한 호출에 E0234와 E0403이 함께 난다: `import cli` 뒤의 `cli.parse([], {})`(`resolve/call.rs` `call` 376~391행).
3. `import "./lib/util" (slug, slug)`의 E0238 도움말이 대입용이다.
4. E0301이 찾은 경로를 제각각 보인다: `lib.fung`, `lib/util`, `./lib/`, `lib/util.fung.fung`(`loader.rs` 304~377행과 495행).
5. `m.a = 2`, `xs[0] = 2`는 도움말 없는 "E0110 unexpected \`=\`"다. fung의 규칙(값은 제자리에서 바뀌지 않는다, `put`)을 알리는 도움말을 붙인다(`habits.rs`).

- 처리: 2026-11-16. (1) 바로 부른 함수 리터럴의 E0234는 "the result of calling this function …"이다. (2) 인자의 리터럴 타입이 틀린 호출은 E0403 하나다(`resolve/call.rs` `call`이 `argument_error`를 `note_call` 앞에서 본다). (3) import 목록에 같은 이름을 두 번 쓴 E0238은 import의 도움말이다. (5) `m.a = 2`·`xs[0] = 2`의 E0110에 "값은 제자리에서 바뀌지 않는다"는 도움말(`habits::IN_PLACE`). (4)는 고치지 않았다: `./lib`·`./nope/util`·`./lib/util.fung`은 모두 찾으려던 파일을 위치를 보이는 꼴로 보이고(`lib.fung`, `nope/util.fung`, `lib/util.fung.fung`, FE29), `./lib/`만 쓴 그대로다(디렉터리라 찾을 파일 이름이 없다, FE82).

## 6. 컴파일러·바이트코드·VM 핵심 (VM)

### VM55. 블록 안의 쥐기만 하는 변수가 뒤 블록 때문에 놓인다 — 버그, 높음

쉬운 말로: `_watch = stop_signal()`을 `if` 안에 두면, 뒤에 오는 다른 블록이 변수를 하나 만드는 순간 `_watch`가 사라진다.

- 재현 1: 아래는 `watching`, `n 1`만 찍고 종료 0으로 끝난다(`still running`이 없다). `n = id(1)` 두 줄을 `print("n", id(1))` 한 줄로 바꾸면 `still running`까지 찍는다.
```
fn id(v) = v
if id(true):
  _watch = stop_signal()
  print("watching")
;;
if id(true):
  n = id(1)
  print("n", n)
;;
request_stop()
print("still running")
```
- 재현 2 (함수 안, 실패한 `match` 절만으로): `watching`, `default port`만 찍고 끝난다. `match`를 빼면 마지막 줄까지 찍는다.
```
fn serve(graceful, opts):
  if graceful:
    _watch = stop_signal()
    print("watching")
  ;;
  match opts:
    {port: p} => print("port", p)
    _ => print("default port")
  ;;
  request_stop()
  print("still running: drain and finish")
;;
serve(true, {})
```
  `fung dump bytecode`의 `serve`: `STORELOCAL 2`(`_watch`), `MATCH … ; {port: $2}`, `CLEARLOCAL 2`. `_watch`와 `p`가 같은 칸이다.
- 명세 근거: `manual/vm/10-values-and-memory.md` 245행 "한 번도 읽지 않는 변수는 함수가 돌아올 때까지 산다. `_watch = stop_signal()`처럼 쥐고 있는 것 자체가 뜻인 값을 위해서다", 설계 03 69행, `scope.md` "변수가 값을 언제 놓는지는 프로그램의 결과를 바꾸지 않는다". 지침 4.
- 원인 위치:
  - `crates/fung-front/src/resolve.rs:530` `pop_block`의 `f.next_slot = sc.base`: 블록이 끝나면 칸을 돌려주어 옆 블록의 이름이 다시 쓴다.
  - `crates/fung-front/src/compiler/moves.rs` `Fwd::match_expr`(706~728행, `reached.extend(bound)`): 실패한 절의 패턴이 쓰는 칸을 다음 절 시작에서 비운다.
- 고치는 법: 읽지 않은 이름(`_` 이름)이 받은 칸은 블록이 끝나도 돌려주지 않는다. `pop_block`은 어느 이름이 읽히지 않았는지 이미 안다(E0239 판정). 규칙 "함수가 돌아올 때까지"가 그대로 참이 되고 `moves.rs`는 손대지 않는다. 비용은 그런 이름 하나당 칸 하나.
  규칙을 "블록이 끝날 때까지"로 바꾸는 길도 있지만 명세를 바꾸는 결정이다(그때는 블록 끝에 `CLEARLOCAL`).
- 시험: 위 두 재현, `vm/lifetime_tests.rs`의 탐침으로 "뒤 블록이 이름을 묶어도 살아 있음". 생성형 시험(`gen_tests.rs`)에 "블록 안의 읽지 않는 이름 + 뒤 블록의 이름"을 더한다.
- 처리: 2026-11-15. `resolve.rs` `pop_block`이 읽지 않은 이름이 있는 블록(과 그것을 품은 블록)의 칸을 돌려주지 않고, `bind_here`가 `_` 이름에 다른 이름이 쓴 적 없는 칸(`max_slot`)을 준다. 그런 칸의 목록이 `FnInfo::held` → `Proto::held`로 간다(검증기가 범위를 봄). 시험 `vm/lifetime_tests.rs` `a_held_local_outlives_the_blocks_after_it`(다섯 모양), `resolve/tests.rs` `a_held_name_keeps_a_slot_of_its_own`. 재현 둘 모두 끝까지 찍는다. 생성형 시험에는 더하지 않았다.

### VM56. 쥐기만 하는 변수가 꼬리 호출에서 놓이고, 내장이 Rust냐 fung이냐에 달렸다 — 결정, 높음

쉬운 말로: `_watch`를 쥔 함수가 다른 함수 호출로 끝나면, 그 호출이 도는 동안 `_watch`는 이미 없다. 다만 그 함수가 Rust로 쓴 내장이면 있다.

- 재현 1: 아래는 아무것도 찍지 않고 종료 0이다. `work()`를 `r = work()`와 다음 줄 `r`로 바꾸면 `work: still running, finishing up`과 `done`을 찍는다.
```
fn work():
  request_stop()
  print("work: still running, finishing up")
  "done"
;;
fn a():
  _watch = stop_signal()
  work()
;;
print(a())
```
- 재현 2: `via_map`은 살고 `via_iterate`에서 프로그램이 끝난다(`end`가 찍히지 않는다). `map`은 Rust, `iterate`는 `extras/prelude.fung`의 fung 함수다.
```
fn work(_x):
  request_stop()
  print("work: still running, finishing up")
  "done"
;;
fn via_map():
  _watch = stop_signal()
  map([1], work)
;;
fn via_iterate():
  _watch = stop_signal()
  iterate(1, work, 1)
;;
print(via_map())
print(via_iterate())
print("end")
```
- 명세 근거: VM55와 같은 문장("함수가 돌아올 때까지 … Rust의 `let _guard`와 같은 관용구"). 꼬리 호출에서 놓인다는 말은 명세에 없다. 어느 내장이 fung이고 어느 것이 Rust인지는 사용자가 알 일이 아니다(지침 2·4).
- 원인 위치: `crates/fung-vm/src/vm/call.rs` `call_op`의 꼬리 갈래(`self.fiber.stack.drain(base..=callee_at); self.reuse_frame(c, start)`)는 프레임의 칸을 모두 버린다. 네이티브 갈래(`run_native(…, tail)`, `vm.rs` 1081행 `Entered::Value(r) if tail => self.ret_with(r)`)는 네이티브가 끝날 때까지 칸이 남는다. `compiler/fn_.rs` `call_checked`는 쥐기만 하는 이름이 살아 있어도 `TAILCALL`을 낸다.
- 선택지:
  - (가) 프레임이 반환 검사를 이어받듯(D44) 쥐기만 하는 값을 프레임이 이어받아 프레임이 돌아올 때 놓는다. 꼬리 호출 규칙에 조건이 붙지 않는다. 프레임 필드(또는 명령)가 하나 더 필요하고, 가드를 묶으며 도는 꼬리 재귀는 가드가 쌓인다(뜻대로다: 돌아올 때까지 쥔다).
  - (나) 쥐기만 하는 이름이 묶여 있는 길의 꼬리 자리 호출은 `CALL` + `RETURN`으로 낸다. 구현이 가장 작다. 꼬리 호출 규칙에 "쥐기만 하는 이름이 있으면 아니다"가 붙고, 그런 함수의 깊은 재귀는 E0804다.
  - (다) 명세를 "꼬리 호출은 돌아가는 것이라 쥐기만 하는 값도 놓인다"로 고치고 네이티브 꼬리 호출도 칸을 먼저 버린다. 예외는 없어지지만 `_watch = stop_signal()` 뒤 `scope(…)`로 끝나는, 지금 되는 가장 자연스러운 모양이 조용히 깨진다.
- 추천: (가), 확신은 중간. 지침 2는 (가)·(다)를(규칙에 조건 없음), 5는 (가)·(나)를(조용한 실패 없음) 가리키고 둘을 다 채우는 것이 (가)다. (나)의 흠의 실제 비용은 작다(가드를 쥔 함수가 십만 번 도는 일은 드물고 E0804로 드러난다). 구현 비용이 걸리면 (나)가 다음이다.
- 처리: 결정 D162((가))와 D172(읽지 않는 이름은 모두, 매개변수도; 반복에서는 쌓인다). 2026-11-15. `vm/call.rs` `call_op`의 꼬리 갈래가 `RtProto::held`의 칸에 든 값을 `Frame::hold`로 `Extra::holds`에 옮긴 뒤 칸을 버린다. 프레임이 돌아올 때 `Extra`와 함께 놓인다. 쥔 것이 없는 함수는 빈 목록 검사 하나뿐이다(릴리스, 꼬리 호출 3천만 번 1.35초 그대로, `fib(32)` 0.40초 그대로). 읽지 않는 `_x`를 묶으며 300만 번 도는 반복은 10 MB → 60 MB(규칙대로). 시험 `a_held_local_outlives_a_tail_call`(열한 모양). 재현 둘 모두 `still running`·`end`까지 찍는다.

### VM57. 이어받은 반환 타입 검사의 순서가 호출 횟수에 달렸다 — 버그, 낮음

- 재현:
```
fn c(_n) = nil
fn a(n) -> int = n == 0 then c(n) else b(n - 1)
fn b(n) -> str = n == 0 then c(n) else a(n - 1)
print(a(parse_int(args[0])!))
```
  인자 0: `E0404 return value must be int`, 2행(`a`). 인자 2(a→b→a→c, `nil`을 돌려준 것은 똑같이 `a`의 꼬리 호출): `must be str`, 3행(`b`).
- 명세 근거: `call.rs` `run_checks`와 `frame.rs` `Extra::checks`의 주석 "the latest first". 프레임을 재사용하지 않으면 가장 안쪽 `a`의 검사가 먼저 실패한다(지침 4).
- 원인 위치: `crates/fung-vm/src/vm/call.rs:278` `tail_check`. `has_check`가 참이면 순서를 갱신하지 않고 돌아간다. `tail_check_bool`은 `add_check`가 옛 것을 빼고 끝에 다시 넣는다.
- 고치는 법: `tail_check`도 "이미 있고 마지막이 아니면 끝으로 옮긴다"(마지막이면 그대로라 고리는 여전히 일정 공간). 시험은 위 재현의 짝수 인자.
- 처리: 2026-11-15. `tail_check`가 `Frame::take_check`로 같은 함수·타입의 옛 검사를 빼고 끝에 다시 넣는다(이미 마지막이면 그대로, 고리는 여전히 일정 공간). 시험 `vm/tests.rs` `an_inherited_return_check_is_the_latest_callers_first`(인자 0~7).

### VM58. 수명 분석이 지역 수 × `return` 수에 비례한다 — 성능, 낮음 (다시 돌리지 않음)

- 측정 (영역 점검): 지역 N개 뒤에 `if a == i: return x_i ;;` N줄. N=2,000 0.47초, 4,000 1.8초, 8,000 7.2초. `dump core`는 0.03초다. 절마다 다른 지역을 읽는 `match` 8,000절은 1.5초.
- 원인 위치: `compiler/moves.rs` 381행 `Head::Return`의 `for s in 0..self.live.len() { self.set(s, false) }`, 246행 `delta`와 419행 `join`.
- 고치는 법: `return`은 "전부 죽음"을 표지 하나로 기록하고, `join`의 `changed`는 가지들의 키를 한 번 모아 계산한다(VM24가 문장 쪽에서 한 일).

- 처리: 2026-11-16. `compiler/moves.rs`: `return`은 살아 있는 칸 집합을 통째로 비우고 기록에 옛 집합을 넣는다(`Change::Kill`, 되돌리기도 O(1)). 가지의 변화(`Delta`)는 "모든 길이 `return`으로 끝남"(`killed`)과 그 뒤에 읽은 칸만 적는다. `join`은 가지들의 키를 한 번 모아 합치고, 부르지 않고 `return`하는 가지(`Fwd::quiet`가 늘 참)는 죽는 칸을 나열하지 않는다(`match`의 대상만 본다). 지역 8,000개 + `return` 8,000줄 3.79초 → 0.06초, 절마다 다른 지역을 읽는 `match` 8,000절 5.2초 → 0.75초(남은 것은 VM59의 `CLEARLOCAL` 수). 바이트코드 dump 기대 출력 1,604개는 그대로다. 시험 `perf.rs` `many_locals_and_returns_compile_in_linear_time`.
- VM59의 처리: 결정 D170((나)). 2026-11-16. open-questions "알려진 한계"에 적었다.

### VM59. 가지 시작의 `CLEARLOCAL`이 가지 수 × 지역 수만큼 나온다 — 성능·결정, 낮음

- 재현: 지역 N개 뒤에 `r = if a == 0: x0 else if a == 1: x1 …`(가지마다 다른 지역 하나를 읽음). N=2,000이면 `fung dump bytecode`에 `CLEARLOCAL`이 2,002,999개다(다시 셈). N=8,000이면 `E0230 too many instructions in a function`으로 올바른 프로그램이 거절된다(영역 점검). 실행 때도 가지에 들어갈 때마다 죽은 지역 수만큼 명령을 돈다.
- 원인 위치: `moves.rs` `Fwd::enter`(541행)와 `quiet`.
- 선택지: (가) 함수를 부르지 않는 가지는 죽은 칸을 문장 끝에서 한 번만 놓는다. (나) 그대로 두고 알려진 한계에 적는다. (가)의 흠(코드 읽기): 호출 없는 가지 안의 `+`가 홀로 쥔 리스트를 제자리에서 늘릴 때 죽은 칸이 같은 값을 쥐고 있으면 복사가 한 번 생긴다. 생성한 코드에서나 닿는 크기라 급하지 않다.

## 7. 값과 prelude (VA)

### VA72. UTF-8이 아닌 바이트가 든 문자열의 슬라이스는 매번 복사다 — 성능, 중간 (VA69의 남은 부분)

- 재현:
```
fn eat(s, n) = s == "" then n else eat(s[index_of(s, "\n") + 1..], n + 1)
text = repeat("2026-11-15 GET /index.html 200 ok\n", parse_int(args[0])!) + args[1] + "\n"
print(eat(text, 0), is_utf8(text), byte_len(text))
```
  1만 줄(340 KB): 끝 줄이 `café`(UTF-8)면 0.01초, `caf\xe9`(Latin-1 한 바이트)면 0.74초. 영역 점검의 측정으로 2만 줄 2.9초, 4만 줄 11.7초다.
- 명세 근거: 지침 4(값은 같고 시간만 내부 표현에 달렸다). `for.md`는 "`s[1..]`로 문자열의 앞을 떼는 재귀도 선형"이라고 적는다. 로그·명령 출력에 잘못된 바이트 하나가 드는 것은 흔하다.
- 원인 위치: `crates/fung-vm/src/value/text.rs:183` `Text::slice`의 `Repr::Bytes(b) => return Rc::new(Text::new(b[from..to].to_vec()))`. 보기(`Repr::View`)는 뿌리가 UTF-8일 때만 만든다.
- 고치는 법: 보기의 뿌리로 `Repr::Bytes`도 허용한다. 조각이 UTF-8인지는 세 값(맞음·아님·모름)으로 두고 처음 물을 때 본다. 시험은 `perf.rs` "eat a string by slices"에 바이트 문자열 사례.

- 처리: 2026-11-16. `Text::piece`(`slice`의 몸): 뿌리가 UTF-8이 아니어도, 크고 뿌리의 첫·끝 잘못된 바이트(`Seek::bad`, 처음 물을 때 한 번 찾음)를 품은 조각은 보기다(UTF-8이 아님이 확실하다). 그 둘을 품지 않은 조각은 전처럼 복사해 다시 본다: UTF-8이면 그 뒤의 슬라이스는 그것의 보기다. 세 값 표시는 두지 않았다(UTF-8인 보기에서 `&str`을 내려면 다시 검사하거나 `unsafe`가 든다). 4만 줄 12.7초 → 0.02초. 시험 `value/tests.rs` `a_slice_of_bytes_is_what_a_copy_is`(무작위 바이트의 겹친 슬라이스가 복사본과 같음), `perf.rs` "eat a string that is not UTF-8 by slices"(잘못된 바이트가 끝에, 앞에). 남은 것: 잘못된 바이트가 셋 넘게 흩어진 문자열을 가운데에서 바깥으로 자르는 조각은 여전히 복사다.

### VA73. `trim`류가 남는 부분을 늘 복사한다 — 성능, 중간

- 재현 (손으로 쓴 토큰 나누기, 셋째 인자가 `trim`이면 `trim_left`, 아니면 슬라이스로 쓴 `skip`):
```
fn words(s, acc):
  t = args[2] == "trim" then trim_left(s) else skip(s)
  if t == "": return acc ;;
  i = index_of(t, " ") ?? len(t)
  words(t[i..], acc + [t[..i]])
;;
fn skip(s) = s != "" and s[0] == " " then skip(s[1..]) else s
text = repeat(args[0] + "  ", parse_int(args[1])!)
print(len(words(text, [])), byte_len(text))
```
  `GET` 5만 낱말(250 KB): `trim_left` 0.60초, `skip` 0.06초. 영역 점검의 측정으로 10만 낱말 2.26초 대 0.12초, 비ASCII 낱말이면 16초다.
- 원인 위치: `crates/fung-vm/src/prelude/strings.rs` `trim_ends`의 `Trim::step` 351행 `string(s[self.at..].to_vec())`, 372행. 지울 것이 없어도 전체를 복사하고, 복사본은 글자 수를 모르는 새 문자열이라 다음 슬라이스가 다시 센다.
- 고치는 법: 남는 부분을 `Text::slice`로 만든다(큰 조각은 O(1) 보기, 아무것도 안 지우면 원본 그대로). VA72와 함께 고치면 바이트 문자열에서도 선형이다.

- 처리: 2026-11-16. `Text::cut`(자른 글자 수를 받아 글자 수를 이어받음, 자른 것이 없으면 그 문자열)을 `trim_ends`가 쓴다. 10만 낱말 2.27초 → 0.08초, 한글 5만 낱말 7.2초 → 0.04초. 시험 `prelude/tests.rs` `a_trim_keeps_a_piece_of_the_string`, `perf.rs` "split words with trim_left".

### VA74. 길이가 다른 리스트의 `<`는 짝 없는 원소를 보지 않는다 — 결정, 낮음 (D87에 이어)

- 재현:
```
print([1, nil] < [2])          # true
print([1, nil] < [2, nil])     # E0501 cannot compare nil and nil
print(sort([[nil], []]))       # E0501
print(min([[nil], []]))        # E0501
```
- 명세 근거: `values.md` "`sort`, `sort_by`, `min`, `max`는 `<`를 따른다", "이 검사는 값과 상관없이 끝까지 간다". `[1, nil] < ys`가 에러인지는 `ys`의 길이에 달렸고, `a < b`는 되는데 `sort([a, b])`는 에러다.
- 원인 위치: `crates/fung-vm/src/value/compare.rs:270-275` `compare`가 한쪽이 끝나면 길이만 견준다. `Shapes::leaf`(496행)·`finish`(401행)는 짝이 없어도 순서 없는 값을 거절한다.
- 선택지:
  - (가) 추천. `<`도 짝 없는 꼬리의 원소가 순서를 가진 모양인지 본다. "`a < b`가 된다 ⇔ `sort([a, b])`가 된다"가 예외 없이 선다.
  - (나) `sort`·`min`·`max`를 `<`에 맞춰 느슨하게 한다(`sort([[nil]])`이 통과한다).
  - (다) 그대로 두고 `values.md`에 적는다.
- 지침: 4(에러가 상대 값의 길이에 달림)와 2(`sort`에만 있는 검사)가 (가)를 가리킨다. (가)가 막는 코드는 순서 없는 값이 꼬리에 든, 길이 다른 리스트의 비교뿐이고 길이가 같아지는 날 어차피 에러다. 드문 모양이라 확신은 중간이다.

- 처리: 결정 D171((가)). 2026-11-16. `compare`가 짝 없는 꼬리의 원소를 자기 자신과 견준다(`x < x`와 같은 에러). 수·문자열·기간·시각은 건너뛴다(`has_order`). 잰 것(릴리스, 고치기 전 → 뒤): 길이가 1~20인 정수 줄 20만 개의 `sort` 1.05초 → 1.13초(+8%), 접두사를 나누는 줄들의 `sort` 0.44초 → 0.46초(+5%). 원소 10개인 접두사와 10만 개인 리스트를 2,000번 견주는 것은 0.01초 → 0.56초다(전에는 짧은 쪽 길이, 이제 긴 쪽 길이에 비례). 시험 `value/tests.rs` `an_unpaired_tail_must_have_an_order`(`Shapes`와 같은 답).

### VA75. `x * 기간`의 타입 에러가 피연산자를 뒤집어 적는다 — 정리, 낮음

- 재현: `"a" * 1s` → `E0501 cannot multiply dur and str`. `2 / 1s`는 `cannot divide int and dur`로 쓴 순서대로다.
- 원인 위치: `crates/fung-vm/src/value/arith.rs:238` `mul`의 `scale_dur(*d, a)`.
- 고치는 법: `mul`에서 타입 에러를 쓴 순서로 만든다.

- 처리: 2026-11-16. `arith.rs` `mul`이 오른쪽이 기간이고 왼쪽이 수가 아니면 쓴 순서로 에러를 만든다. 시험 `time_tests.rs`.

## 8. 시험했지만 문제가 없던 것

- **명세 직접 점검**: 평가 순서(이름 인자, 맵 리터럴, 인덱스·슬라이스의 끝, 파이프, 보간, 범위, 기본값, 펼침)는 모두 왼쪽부터다. 패턴(`-0.0`, 기간, `(x,)`, `()`, 원시 문자열, 중첩과 가드, 대입·`for`·람다 매개변수의 `|` 대안), 맵의 중복 키(E0113, 원시 문자열 키·패턴 포함), 타입 별칭(E0212·E0218), 비교 연쇄(E0111), 최상위 상수가 prelude 이름을 가릴 때 최상위 `fn`의 읽기, D139의 E0204(람다·안쪽 블록)는 명세대로다.
- **제기하지 않은 것(규칙대로다)**: 함수 본문이 `for`로 끝나면 그 `for`는 값 자리라 `map`이고(`[nil, nil]`을 돌려준다), 본문 끝의 `else` 없는 `if`·대입·`assert`는 E0223·E0224다. 절차처럼 쓴 함수가 고리로 끝나면 마지막 줄에 `nil`을 써야 한다. 번거롭지만 "함수의 마지막 식은 값 자리"라는 한 규칙의 귀결이고 도움말이 고치는 법을 말한다. `xs |> len + 1`은 `+`가 먼저 묶여 E0501이다(우선순위 표대로).
- **앞단**(약 900개 입력): 어휘 전부, 줄 이어짐(D85) 60여 모양, `return`의 자리, desugar(`x |> Ok()`, 부분 적용, E0227·E0229·E0127), E0238의 모든 묶는 방법, 덮임 검사 40여 모양, 컴파일 때 호출 검사, 효과 판정의 건전성(옳은 코드를 막은 사례 없음), 로더(순환, 마름모, E0309). 패닉·E08xx 없음.
- **컴파일러·VM**: 상수 접기 대 실행 10,192쌍 + 62쌍, 같은 뜻 다른 모양 약 9,000개 × 3벌, 값 공유와 제자리 고침 6,600개(Python 모형과 견줌), 수명 탐침 36개(VM55·VM56 말고는 규칙대로), 꼬리 호출(1,600만 번이 일정 공간), 검증기와 한도 16가지 큰 프로그램. 08·09·10장과 `internals.md`의 명령 표·설명은 `op.rs`·`vm/link.rs`와 같다. `verify.rs`의 구멍은 따로 뒤지지 않았다.
- **값·prelude**: 타입 행렬(함수 약 95개 × 값 20가지), 30만 겹 중첩(`==`·`<`·`str`·해시·`sort`·해제), 리스트·문자열·맵의 표현 문턱, 수(넘침 E0503, NaN·Inf E0504, 2^53 너머, `-0.0`, 되읽기 2만 개), 기간(D135), 범위의 양 끝과 한도, 문자열 함수, 성능(리스트·맵·문자열의 n·2n·4n 선형 확인).
