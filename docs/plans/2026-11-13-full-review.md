# 언어 명세·바이트코드 VM 점검 결과와 작업 목록 (2026-11-13)

세 번째 전체 점검(`docs/plans/2026-11-11-full-review.md`, 115개 모두 처리)을 고친 뒤, 언어 명세와 바이트코드 VM 구현을 다시 점검한 결과다.
이번 점검의 기준은 사용자가 준 세 지침이다.

1. **엄격함이 우선이다**(매우 중요): 말없이 틀린 값, 말없이 받아 주는 실수, 느슨한 인자 처리는 결함이다.
2. **예외 조항을 줄인다**(중요): "…인 경우만 빼고"가 붙는 규칙은 결함이다.
3. **같은 일을 하는 방법은 하나다**(존중, 실질적인 편의를 위해 조금 희생할 수 있음).

하위 호환, 비용, 난이도는 문제를 덮는 이유가 되지 않는다. 큰 구조 변경도 선택지에 넣었다.
형식은 지난 세 점검과 같다. 새 세션이 이 문서만 읽고 항목 하나(또는 한 묶음)를 이어서 고칠 수 있게 썼다.
항목을 처리하면 `처리:` 줄에 커밋과 바꾼 문서를 적는다.

**결정 (2026-11-13)**: SP52·SP53·SP54·SP56과 VA47의 남은 결정은 추천대로, SP55는 바꾸지 않음(D85~D90, `docs/plans/2026-11-13-full-review-fixes.md`).

**상태 (2026-11-13)**: 58개 항목(작업 중 FE76을 더함) 가운데 VM37~VM41, FE63·FE64·FE66, VA41·VA43·VA47을 고쳤고, 결정 항목(SP52~SP65, VA48~VA56)은 D85~D107로 모두 정해 고치거나 닫았다(작업 브랜치, 미릴리스). 결정 없이 고칠 항목(FE65·FE67~FE76, VA42·VA44~VA46·VA54·VA57·VA58, VM42~VM47)도 2026-11-13~14에 모두 처리했다: 58개 항목마다 `처리:` 줄이 있다. 고치지 않고 닫은 것은 FE74의 4번(기본값 안의 `?`도 E0211), VA54(코드 대신 `functions.md`의 규칙을 고침), VA58의 3번(`read_line`의 끝 `\r`은 동작을 두고 문서에 적음)이다. 작업 중 본 것: D87의 "끝까지 비교" 때문에 앞 원소가 다른 튜플 30만 개의 `sort`가 15 → 40 ms다(VA44 작업의 측정, 결정대로 둠).

## 0. 이 문서를 쓰는 법

- **기준 커밋**: `1951b727`(`fix: a closed pipe dies after the other stream's earlier lines (CC15)`). 줄 번호는 이 커밋 기준이고 함수 이름을 함께 적었다.
- **범위**: 언어 명세(`manual/reference/language/`), 앞단(렉서·파서·desugar·resolve·로더·진단), 컴파일러와 바이트코드(수명 분석·검증기 포함), VM 실행 핵심과 값 층, 언어의 일부인 prelude 표면.
  동시성 런타임(스케줄러·이벤트 루프), 표준 모듈(`fs`·`http`·`time` 등)의 내부, CLI 도구는 이번 범위가 아니다(지난 점검에서 봤다). 걸쳐 있는 것만 적었다.
- **방법**: 명세 15쪽은 직접 읽고 의심 가는 규칙을 릴리스 바이너리로 재현했다. 네 영역(앞단, 컴파일러·바이트코드, VM 코어·값, prelude)은 따로 점검한 뒤 주장을 종합 단계에서 다시 돌렸다.
  - **"(다시 확인)"** 표시는 종합 단계에서 한 번 더 돌려 같은 결과를 본 항목이다. 표시가 없는 항목은 영역 점검의 재현 한 번뿐이다.
  - "재현 안 함(코드 읽기)"이라고 적은 것은 실행으로 확인하지 못했다.
  - 모든 실행은 macOS arm64다. 리눅스는 돌리지 않았다.
  - 고치는 법 가운데 "사본에서 확인"이라고 적은 것은 영역 점검이 저장소 밖 사본에 적용해 본 것이다. 저장소에는 아무것도 고치지 않았다.
- **뺀 것**: 지난 세 점검의 항목(모두 처리), 결정 D1~D84, open-questions "남은 일"·"알려진 한계"·보류 목록.
  이미 정한 결정이 세 지침과 부딪친다고 보아 다시 꺼낸 것은 제목에 "(기존 결정 재검토)"라고 적었다.
- **재현**: `cargo build --release --locked` 뒤 저장소 루트에서 `F=target/release/fung`. 여러 줄 소스는 `x.fung`로 저장해 `$F run x.fung`.
- **ID**: 지난 점검에 이어 매긴다: SP52~, FE63~, VM37~, VA41~. SP 명세(언어 규칙의 결정) · FE 앞단과 진단 · VM 컴파일러와 바이트코드 · VA 값과 prelude.
- **분류**: 버그(명세가 정답을 말함) · 결정(언어 동작이 바뀜: 선택지 + 추천으로 사용자에게 물음) · 성능 · 정리(구조·진단 문구) · 문서.
- **심각도**: 높음(정상적인 쓰임에서 프로그램이 죽거나 틀린 값이 조용히 나옴) · 중간 · 낮음.
- **고칠 때의 공통 절차**: CLAUDE.md 그대로다(TDD, 커밋 전 검사 전체, CHANGELOG, 매뉴얼 다시 만들기, 기대 출력 bless 뒤 diff 검토).

## 1. 총평

**이번에는 깨지는 곳이 있다.** 지난 점검의 "0건"과 달리, 올바른 프로그램이 죽는 컴파일러 버그 셋을 찾았다. 셋 다 수명 분석(`compiler/moves.rs`, 11-11 VM19·VM23에서 다시 짠 것)에 있다.

- VM37: 모든 갈래가 `return`하는 `match`의 주제를 일찍 놓아 실행 중 내부 에러 E0801로 죽는다(높음).
- VM38·VM39: `if not (a and b)` 조건과 기본값 식에서 마지막으로 읽힌 값이 놓이지 않아, 채널이 닫히지 않고 교착한다.

VM37은 디버그 빌드의 바이트코드 검증기가 정확히 잡는 버그다. 릴리스는 검증기를 돌리지 않고, 저장소의 퍼징 시험(TL9)은 시험 코드를 변이하는 방식이라 이 모양을 만들지 못했다.
문법을 아는 무작위 프로그램 생성기와 기준 해석기를 붙이자 4,500개 가운데 8개가 걸렸다. **지키는 장치를 한 단계 올릴 때다**(VM41: 릴리스에서도 검증, 생성형 시험).

그 밖의 문제는 네 갈래다.

**첫째, 말없이 틀린 값이 나오는 자리가 아직 있다(지침 1).**
- 괄호 안에서 쉼표를 빠뜨린 `[1⏎ -1]`이 `[0]`이 되고, 블록의 마지막 줄 `-x`가 앞 줄의 호출을 버리고 반환값이 된다(SP53).
- `9007199254740993 / 3`이 틀린 몫을 낸다(VA41). `+ - * // %`는 같은 경우에 E0523인데 `/`만 빠졌다.
- 값 없는 `return` 다음 줄의 `?? 1`이 조용히 죽은 코드가 된다(FE64).
- 같은 내용의 리스트 비교가 `Rc`를 공유하는지에 따라 `true`와 E0501로 갈린다(VA43).

**둘째, 자료에 따라 나타났다 사라지는 에러(지침 1).** `==`·`<`·`sort`·`unique`의 타입 검사는 "실제로 견준 쌍"만 본다.
그래서 `sort([[1, "a"], [2, 2]])`는 통과하고 앞 칸이 같은 두 행이 들어오면 죽는다(SP56). 시험 자료에서는 되고 운영 자료에서 죽는 종류다. 지난 결정(D16·D40)이 남긴 가장 큰 구멍이다.

**셋째, 파일의 역할과 구현 언어에 따라 달라지는 규칙(지침 2).**
- 같은 파일이 import되면 올바르고 진입 파일이면 컴파일 에러라, 라이브러리 파일을 `fung check`·`fung test`로 볼 수 없다(SP52).
- 효과 없는 내장 함수를 문장으로 쓰면 E0234인데, fung으로 쓴 내장 함수(`take`·`zip`·`first` 등)는 조용하다(VA47).
- `then … else` 가지의 판정이 desugar가 만든 모양에 기대어, `else`가 있는데도 "else 없는 if" 에러가 난다(FE63).

**넷째, 방법이 둘인 자리(지침 3).** 표기 수준(`[x, ...]`과 `[x, ..._]`, `"$5"`와 `"\$5"`)과 함수 수준(`int(s)`와 `parse_int(s)!`, `get(m, k, d)`와 `get(m, k) ?? d`)에 남아 있다(SP59, VA50·VA51).
가장 큰 것은 지역 변수의 다시 대입(SP54)과 `??`의 두 역할(SP55)이다. 둘 다 예전 결정이지만, 이름이 한 번만 묶이고 연산자가 한 가지 일만 하면 명세의 규칙 수가 눈에 띄게 준다.

**단단한 곳**: 앞단은 무작위 입력 24,000개와 1만 겹 중첩에서 패닉이 없었다. 상수 접기는 실행과 1,500식에서 일치했다. 영속 벡터·맵은 모델과의 차등 시험을 통과했다. 정수 산술의 경계, 한도(E0804·E0806), 호출 규약, 동시성 API의 모양은 명세대로다.

## 2. 추천 작업 순서

1. **컴파일러 버그 셋과 지키는 장치**: VM37 → VM38 → VM39, 이어서 VM41(릴리스 검증, 생성형 시험)과 VM40(검증기 구멍). 생성형 시험을 먼저 넣으면 VM37~VM39의 실패하는 시험이 된다.
2. **결정 없이 고칠 버그**: FE64(`return` 이어짐), FE63(`then` 가지 판정), VA41·VA42(실수 산술의 조용한 깎기), VA43(`==` 지름길), VA47(순수 함수 표), FE65(부분 적용 검사), FE66·FE67·FE68·FE69·FE70, VA45, VA57.
3. **뜻이 바뀌는 결정 묶음**(사용자 결정 필요): SP53(줄 앞의 `-`) → SP52(파일의 역할) → SP56(자료에 따른 에러) → SP54(다시 대입) → SP55(`??`) → SP58(태그와 심볼). 이어서 VA48~VA56의 prelude 결정.
4. **성능**: VA46(보간 누적), VA44(공유 구조의 비교), FE75(앞단의 이차 시간), VM44.
5. **구조 정리**: VM42·VM43(명령 모양), VM45, VM46.
6. 나머지 낮음(SP57·SP59~SP65)과 문서(SP66, VM47, VA58).

## 3. 목록

| ID | 한 줄 | 분류 | 심각도 |
|---|---|---|---|
| SP52 | 파일이 진입 파일인지 모듈인지에 따라 E0239·E0240이 달라 라이브러리를 `check`·`test`할 수 없다 | 결정 | 중간 |
| SP53 | 줄 앞의 `-`·`(`·`[`가 괄호 안에서는 앞 원소에 붙고, 블록 끝에서는 앞 줄을 버린다 | 결정 | 중간 |
| SP54 | 지역 변수만 다시 대입할 수 있다 | 결정 | 중간 |
| SP55 | `??`가 `nil`의 기본값과 `Err` 삼키기 두 가지 일을 한다 | 결정 | 중간 |
| SP56 | 타입 에러(E0517·E0501)가 자료의 값에 따라 나타났다 사라진다 | 결정 | 중간 |
| SP57 | `match` 절 본문의 마지막 람다가 말없이 새 절이 된다 | 결정 | 낮음 |
| SP58 | 인자 없는 태그가 심볼이라서 생기는 예외 조항들 | 결정 | 낮음 |
| SP59 | 표기가 둘인 자리 | 결정 | 낮음 |
| SP60 | `and`·`or`의 오른쪽 호출만 꼬리 호출이 아니다 | 결정 | 낮음 |
| SP61 | `dbg`가 부르는 꼴에 따라 다른 것을 찍는다 | 결정 | 낮음 |
| SP62 | 리터럴만으로 확정되는 오류를 `check`가 통과시킨다 | 결정 | 낮음 |
| SP63 | 아무 일도 하지 않는 블록과 읽히지 않는 정의가 통과한다 | 결정 | 낮음 |
| SP64 | 타입 표기에서 조용히 통과하는 것 | 결정 | 낮음 |
| SP65 | 보이지 않는 글자와 제어 문자가 이름·주석·문자열에 들어간다 | 결정 | 낮음 |
| SP66 | 꼬리 위치 목록이 문서 세 곳에서 서로 다르고 구현과도 다르다 | 문서 | 낮음 |
| FE63 | `then … else` 가지의 판정이 desugar가 만든 모양에 따라 갈린다 | 버그 | 중간 |
| FE64 | 값 없는 `return` 다음 줄의 이어짐 토큰이 E0132를 비켜 간다 | 버그 | 중간 |
| FE65 | 부분 적용과 지역 `fn` 호출은 인자를 컴파일 때 검사하지 않는다 | 버그 | 중간 |
| FE66 | 줄 이어짐 규칙 1의 `else`·`not`이 일부 자리에서만 이어진다 | 버그 | 낮음 |
| FE67 | 슬라이스 끝의 맨 람다를 받는다 | 버그 | 낮음 |
| FE68 | `import "./lnk/../x"`가 운영체제와 다른 파일을 읽는다 | 버그 | 낮음 |
| FE69 | import된 파일의 최상위에 `_ = 식`이 온다 | 버그 | 낮음 |
| FE70 | E0235가 `match`가 같다고 보는 수 리터럴을 놓친다 | 버그 | 낮음 |
| FE71 | 숫자 리터럴의 진단 번호가 어긋난다 | 버그(진단) | 낮음 |
| FE72 | `...rest`가 묶는 이름의 에러가 `[`를 가리킨다 | 버그(진단) | 낮음 |
| FE73 | 이름 에러가 소스 순서가 아니라 패스 순서로 나온다 | 정리(진단) | 낮음 |
| FE74 | 진단의 작은 것들 | 정리(진단) | 낮음 |
| FE75 | 앞단의 이차 시간 다섯 곳 | 성능 | 낮음 |
| FE76 | 파일 끝의 주석이 이어진 문장의 들여쓰기를 받는다 | 버그(포매터) | 낮음 |
| VM37 | 모든 갈래가 `return`하는 `match`의 주제를 일찍 놓아 E0801로 죽는다 | 버그 | 높음 |
| VM38 | 조건 자리의 `not (a and b)`가 건너뛴 쪽의 값을 놓지 않아 교착한다 | 버그 | 중간 |
| VM39 | 마지막 읽기가 기본값 식에 있는 매개변수는 함수가 끝날 때까지 값을 쥔다 | 버그 | 중간 |
| VM40 | 검증기가 믿고 넘기는 것들 | 정리 | 중간 |
| VM41 | 릴리스 빌드는 검증기를 돌리지 않고 생성형 시험이 없다 | 결정 | 중간 |
| VM42 | 이웃 명령에 기대는 명령 둘 | 정리 | 낮음 |
| VM43 | 인자 4,095개 한도는 `CALL` 피연산자 묶음에서만 나온다 | 정리 | 낮음 |
| VM44 | 반환 타입 있는 함수끼리의 꼬리 호출마다 `TAILCHECK`가 약 12 ns 든다 | 성능 | 낮음 |
| VM45 | `assert`의 쪼갠 비교는 비교 연산자의 둘째 구현이다 | 버그 | 낮음 |
| VM46 | 코드 생성과 주석의 군더더기 | 정리 | 낮음 |
| VM47 | VM 장의 설명이 구현과 다른 곳 | 문서 | 낮음 |
| VA41 | `int / int`가 2^53 넘는 정수를 말없이 깎아 틀린 몫을 낸다 | 버그 | 중간 |
| VA42 | `dur * float`가 104일 넘는 기간을 말없이 깎는다 | 버그 | 낮음 |
| VA43 | `==`의 "같은 객체" 지름길이 함수 검사를 건너뛴다 | 버그 | 중간 |
| VA44 | 공유 구조 둘의 `==`·`<`·해시가 지수 시간이고 기한이 닿지 않는다 | 버그 | 낮음 |
| VA45 | 값의 표기가 `$이름`을 이스케이프하지 않아 소스로 되읽히지 않는다 | 버그 | 낮음 |
| VA46 | 보간으로 쌓는 문자열은 O(n²)이다 | 성능 | 중간 |
| VA47 | fung으로 쓴 prelude 함수는 문장으로 버려도 E0234가 아니다 | 버그 | 중간 |
| VA48 | `is_ok`·`is_err`가 결과값이 아닌 값에 조용히 `false`다 | 결정 | 중간 |
| VA49 | `zip`이 길이가 다른 입력을 말없이 자른다 | 결정 | 중간 |
| VA50 | 문자열을 수로 읽는 길이 둘이다 | 결정 | 낮음 |
| VA51 | `get(m, k, d)`와 `get(m, k) ?? d`가 겹치고 답이 갈린다 | 결정 | 낮음 |
| VA52 | `remove(m, k)`가 없는 키를 말없이 받는다 | 결정 | 낮음 |
| VA53 | 인자 자리의 `nil`이 "표준 입력"·"공백"을 뜻한다 | 결정 | 낮음 |
| VA54 | 틀린 타입의 인자가 E0403·E0506·E0513으로 갈린다 | 정리 | 낮음 |
| VA55 | 부분 문자열을 찾는 함수들이 글자 경계를 다르게 본다 | 결정 | 낮음 |
| VA56 | `sum`의 빈 입력은 정수 `0`이라 기간의 합이 빈 리스트에서만 죽는다 | 결정 | 낮음 |
| VA57 | `pad_left`·`pad_right`가 한도에 꼭 맞는 결과를 거절한다 | 버그 | 낮음 |
| VA58 | prelude의 작은 어긋남 | 결정·문서 | 낮음 |

## 4. 명세 (SP)

### SP52. 파일이 진입 파일인지 모듈인지에 따라 E0239·E0240이 달라 라이브러리를 `check`·`test`할 수 없다 — 결정, 중간 (다시 확인, 기존 결정 D38 재검토, 앞단 점검과 합침)
- 재현: `lib3.fung`
  ```
  max_size = 10
  fn double(n) = n * 2
  test "d": assert double(2) == 4 ;;
  ```
  `fung test lib3.fung` → `FAIL lib3.fung: <load>  error[E0239]: unused variable max_size`.
  `limit = 3` / `fn helper(len) = len + limit`인 파일의 `fung check` → `E0240: len hides a prelude function`.
  두 파일 모두 다른 파일이 import하면 올바르다.
- 근거: `modules.md`가 "같은 파일을 진입 파일로 실행하거나 `fung check`하면 인터페이스가 아니므로 이 둘이 에러다"라고 적어 두었다. 그러나 `test` 블록은 그 파일이 진입 파일일 때만 돈다. 그래서 공개 상수가 있거나 prelude와 같은 이름을 인터페이스로 쓰는 모듈(표준 모듈 `http.get`·`path.join`의 모양)은 시험을 돌릴 방법이 없다. 파일의 옳고 그름이 부르는 방식에 달린 것은 지침 2에 어긋난다.
- 원인 위치: `resolve/names.rs` `interface_name`·`report_unused_file`(`opts.library`), `loader.rs` `compile_after`.
- 고치는 법(결정):
  - (가) **추천**: 규칙을 파일 안에서만 정한다. `_`로 시작하지 않는 최상위 이름은 어느 파일에서나 공개 이름(인터페이스)이라 읽지 않아도 되고, 공개 이름과 공개 `fn`의 매개변수는 어느 파일에서나 prelude와 같은 이름일 수 있다. 진입 파일의 쓰지 않는 최상위 변수 검사는 잃는다(지역 변수 검사는 그대로).
  - (나) `fung test`는 대상을 모듈 규칙으로 컴파일하고 `fung check`에 모듈로 검사하는 옵션을 둔다. 엄격함은 지키지만 "어떻게 부르는가"에 따른 규칙이 도구에 남는다.
  - (다) 인터페이스 예외를 없앤다(공개 이름도 prelude를 가릴 수 없음). 예외가 0개지만 `http.get`·`path.join`·`regex.split`의 이름을 모두 바꿔야 하고, 쓰지 않는 공개 상수 문제는 남는다.
- 처리: 결정 D86((가)). 커밋 `feat!: a file's public top-level names are its interface in any file (D86)`. `resolve/names.rs` `interface_name`이 진입 파일을 가르지 않고, `report_unused_file`은 import만 본다. 재현 둘(`fung test lib3.fung`, `fung check lib.fung`)이 통과한다. 명세 `scope.md` 6·7번 규칙, `modules.md`, `functions.md`, 설계 03·04·08. 시험 `resolve/tests.rs` `unused_and_hiding_names`, `tests/lang/interface_names.fung`(옛 `err_hide_top_assign_prelude`·`err_hide_top_fn_prelude`), `err_unused_top_level.fung`(블록 안의 이름으로). 남는 것: 로더의 `library` 구분은 E0306(모듈 최상위의 문장 모양)에만 쓰인다.

### SP53. 줄 앞의 `-`·`(`·`[`가 괄호 안에서는 앞 원소에 붙고, 블록 끝에서는 앞 줄을 버린다 — 결정, 중간 (다시 확인, 앞단 점검과 합침)
- 재현:
  ```
  xs = [
    1
    -1
  ]
  print(xs)                    → [0]          (쉼표를 빠뜨림)
  [a⏎ [1]]  (a = [10, 20])     → [20]
  {k: 1⏎ -1}                   → {k: 0}

  fn f(a):
    compute(a)
    -a
  ;;
  print(f(5))                  → -5           (compute(a) - a를 뜻했다면 호출이 말없이 버려짐)
  ```
- 근거: `blocks.md`는 줄 앞의 `-`를 "앞 줄의 식이 버려지는 자리에 남아 E0234"라고 설명한다. 그런데 앞 줄이 사용자 함수 호출이면 E0234가 아니고, 괄호 안에서는 규칙 3("줄바꿈은 공백")이 값을 조용히 바꾼다. 이어짐 규칙이 괄호 안팎에서 다른 것도 지침 2에 걸린다.
- 원인 위치: `parser.rs` `peek`(nest > 0이면 줄바꿈을 건너뜀), `parser/expr.rs` `infix_loop`.
- 고치는 법(결정):
  - (가) **추천**: 식이 줄을 넘는 방법은 괄호 안팎 모두 규칙 1(줄 끝의 연산자)과 규칙 2(이어짐 전용 토큰)뿐이다. 괄호 안의 줄바꿈은 원소 사이와 여닫는 괄호 옆에서만 공백이다. 그리고 문장이 `-`로 시작하면 컴파일 에러다(값이면 `(-a)`나 `0 - a`). 이어짐 규칙이 하나가 되고 두 재현이 모두 에러가 된다.
  - (나) 괄호 안만 (가)대로 하고, 문장 앞의 `-`는 지금대로 둔다.
  - (다) 그대로 두고 문서에 적는다.
- 처리: 결정 D85((가)). 커밋 `feat!: one rule for a line that goes on, inside brackets too (D85)`. `parser/expr.rs` `peek_infix`가 괄호 안에서도 줄바꿈 뒤의 토큰을 연산자·호출·인덱스로 보지 않는다(이어짐 전용 토큰만). 한 원소 안의 두 토큰도 같은 줄: 람다의 `=>`(`parser/fn_.rs`·`expr.rs`), 태그의 `(`(`collections.rs` `tag_expr`, `pattern.rs`), 이름 인자의 `:`(11-08 FE6의 "괄호 안 어디서나 공백"을 뒤집음). 앞 문장의 다음 줄을 `-`로 시작하는 문장은 새 번호 E0136(`parser/stmt.rs` `statement`). **결정문을 좁힌 곳**: "문장은 `-`로 시작할 수 없다"를 글자 그대로 하면 `_ => -1`, 블록의 첫 문장, REPL의 `-1`까지 막히므로, 이을 앞 줄이 있는 경우(앞 토큰이 줄바꿈이고 그 앞이 `:`·`=>`·`;`가 아님)만 에러다. E0234의 "줄 앞의 `-`" 도움말과 그 코드(`resolve/value.rs` `unary_minus`)를 지웠다. 저장소에서 걸린 것은 골든 둘뿐(`err_effect_free_minus` → `err_line_starts_minus`, `line_continuations_more`의 이름 인자). 명세 `blocks.md`(규칙 둘, 괄호 안 단락, E0136), `errors.md`, `grammar.md`, 매뉴얼 `vm/04-parser.md`, 설계 01. 시험 `parser/tests.rs` 넷, `tests/lang/err_line_starts_minus.fung`·`err_bracket_missing_comma.fung`·`line_continuation_brackets.fung`.

### SP54. 지역 변수만 다시 대입할 수 있다 — 결정, 중간 (다시 확인, 기존 결정 D53 재검토)
- 재현:
  ```
  fn f(x):
    y = x
    g = () => y
    y = y + 1
    (g(), y)
  ;;
  print(f(1))                             → (1, 2)
  fn f(): fn g() = 1; g = 2; g ;;          → 2      (지역 fn 이름을 변수로 다시 묶음)
  fn f(): g = 2; fn g() = g; g() ;;        → <fn g>
  fn f(): x = len([1]); x = 2; x ;;        → 2      (첫 대입의 값은 읽히지 않음)
  ```
- 근거: `scope.md`의 일곱 규칙 가운데 넷이 다시 대입에서 나온다: 2번(최상위는 한 번만, E0238), 4번의 "나중에 다시 대입해도", 5번의 E0205(바깥 이름에는 대입 불가), 7번의 "다시 대입하기만 하는 것은 읽는 것이 아니다". D53은 최상위만 막았다. 남은 지역 다시 대입은 반복문이 없는 언어에서 힘이 작고(같은 블록 안에서만, 가지 안에서는 안 됨), 최상위·바깥 블록·같은 블록의 규칙이 서로 달라 지침 2에 걸린다. 최상위 `fn` 이름은 상수인데 지역 `fn` 이름은 다시 묶인다.
- 원인 위치: `resolve.rs` `bind_new`(같은 블록의 다시 대입 허용)·`fn_node`(`cur_block().fns`만 봄), `compiler/moves.rs`("대입이 생존을 끊음").
- 고치는 법(결정):
  - (가) **추천**: 이름은 어디서나 한 번만 묶인다. 같은 블록의 두 번째 묶기도 E0238이다. 2·5·6번 규칙이 "보이는 이름은 다시 묶을 수 없다" 하나로 합쳐지고, E0205·E0209가 은퇴하며, 수명 분석에서 "대입이 생존을 끊는" 갈래가 사라진다. 저장소의 `.fung` 파일에서 다시 대입을 쓰는 곳을 모두 새 이름으로 고쳐야 한다.
  - (나) 지역 `fn` 이름만 한 번 묶이게 한다(변수 → fn, fn → 변수 모두 에러). FE65의 지역 `fn` 인자 검사가 가능해진다.
  - (다) 그대로.
- 처리: 결정 D88((가)). 커밋 `feat!: a name is bound once, in a block as at the top level (D88)`. `resolve.rs` `bind_new`가 같은 블록의 두 번째 묶기를 어느 블록에서나 E0238로(REPL의 최상위만 다시 묶음, 노트는 처음 묶은 줄). 지역 `fn`과 변수의 조합도 같다(FE73의 재현 넷). E0209는 은퇴(E0238). **결정문과 다른 곳 둘**: (1) E0205는 남겼다. 규칙으로는 E0240과 하나지만 "반복문 안의 카운터"는 가장 흔한 실수라 맞춤 도움말이 있는 번호를 지켰다(문구만 고침). (2) 수명 분석의 "쓰기가 생존을 끊는" 갈래는 지우지 않았다: 형제 블록의 이름이 같은 칸을 다시 쓴다. 저장소에서 걸린 것은 언어 시험 다섯 파일뿐(`closures`·`http_serve`·`err_reassign_fn`·`err_top_reassign`·`err_unused_written_only` → `err_rebind_local`); 내장 모듈·예제·벤치마크에는 쓰임이 없었다. 매뉴얼의 다시 대입 예제 여섯(배우기 3·5장, VM 6·9·16장, 에러 E0209)과 스코프 명세의 규칙 2·4·5·6·7을 다시 썼다. 설계 03·08. 시험 `resolve/tests.rs` `a_name_is_bound_once`. FE65의 지역 `fn` 인자 검사는 하지 않았다(이제 이름이 한 번만 묶이므로 가능하다).

### SP55. `??`가 `nil`의 기본값과 `Err` 삼키기 두 가지 일을 한다 — 결정, 중간 (다시 확인, 기존 결정 재검토)
- 재현:
  ```
  m = {k: Ok(1)}
  print(get(m, "k") ?? 0)          → 1        (Ok(1)이 아니다)
  print(5 ?? 0, Red(1) ?? 2)       → 5 Red(1) (nil도 결과값도 될 수 없는 값도 받음)
  fs.read(p) ?? "{}"               → 권한 오류일 때도 "{}"
  ```
- 근거: `results.md`가 두 함정을 스스로 적는다("`??`는 모든 `Err`를 기본값으로 바꾼다", "맵에 결과값을 담아 두었다면 … `Ok(1)`이 아니라 `1`이다"). 같은 문서의 "`!`·`?`는 결과값만 받는다, 규칙은 하나다"와 달리 `??`는 세 종류의 값을 다 받는다. 실패를 종류와 상관없이 말없이 삼키는 것은 버려지는 `Err`를 E0606으로 막는 언어의 기조와 맞지 않는다.
- 원인 위치: `vm/m4.rs`의 `??` 처리, 명세 `results.md`의 표.
- 고치는 법(결정):
  - (가) **추천**: `??`는 `nil`만 본다(왼쪽이 `nil`이면 오른쪽, 아니면 그대로, `Ok`를 벗기지 않음). 결과값의 기본값은 `match`로 종류를 가려 쓴다. `parse_int(s) ?? 0` 같은 짧은 쓰임을 잃는데, 그것이 바로 "모든 실패를 삼키는" 쓰임이다.
  - (나) (가)에 더해 결과값용 함수 하나를 둔다(`or_else(r, d)`: `Ok(v)`면 `v`, `Err`면 `d`, 그 밖은 E0605). 삼키는 자리가 이름으로 드러난다.
  - (다) 그대로 두되 `nil`도 결과값도 될 수 없는 리터럴 왼쪽만 컴파일 에러로 한다(SP62).
- 처리: 결정 D89(2026-11-13): 바꾸지 않는다. `??`는 `!`·`?`와 한 벌인 결과값 연산자이고 `Ok`를 벗기는 것은 원래 설계다(설계 02의 표, 2026-10-20 L10에서 같은 대안을 기각). 이 항목의 "두 가지 일을 한다"는 틀이 잘못이었고, 새 근거가 없다. 코드·명세 변경 없음.

### SP56. 타입 에러(E0517·E0501)가 자료의 값에 따라 나타났다 사라진다 — 결정, 중간 (다시 확인, 기존 결정 D16·D40 재검토, 세 영역 합침)
- 재현:
  ```
  [1, 'a] == [2, "a"]                       → false     [1, "a"] == [1, 2]  → E0517
  {a: 1, b: "x"} == {a: 2, b: 3}            → false     {b: "x", a: 1} == {a: 2, b: 3} → E0517 (넣은 순서)
  1 in [1, "a"]                             → true      "a" in [1, "a"]     → E0517
  sort([[1, "a"], [2, 2]])                  → 통과       sort([[1, "a"], [1, 2]]) → E0501
  sort([[1, print]])                        → E0501     sort([[1, print], [2, print]]) → 통과
  unique([[1, "a"], [1, 2]])                → 통과
  (1, 2) == (1, 2, 3)                       → false     (1, 2) < (1, 2, 3)  → true
  ```
- 근거: `values.md`는 "다른 타입의 비교는 거의 늘 실수"라며 E0517을 두고, 같은 단락에서 "`==`가 실제로 견주는 쌍만 본다"고 물러선다. 그래서 같은 실수가 앞 원소의 값이나 맵의 삽입 순서에 따라 잡히거나 지나간다. D16은 "원소 하나여도 비교할 수 있어야 한다"로 `sort([nil])`을 막았는데, 원소가 둘이면 느슨해지는 역전이 생겼다. 고정 개수라는 튜플도 길이가 달라도 견준다.
- 원인 위치: `value/compare.rs` `walk`·`compare`(첫 차이에서 반환), `prelude/collections.rs` `sort_fn`·`sort_by_fn`·`min`·`max`·`unique`.
- 고치는 법(결정):
  - (가) **추천**: 비교는 값이 달라도 같은 자리의 타입을 끝까지 본다. `==`·`in`·`unique`는 두 값의 모양(길이와 키가 겹치는 범위)을 따라 타입을 확인하고, `sort`·`min`·`max`는 시작 전에 모든 원소(키)를 첫 원소와 견줄 수 있는지 한 번 훑는다(O(n)). 길이가 다른 튜플의 비교는 E0517이다. "첫 차이에서 멈춤"의 속도 이점을 잃는다(다른 값의 비교도 O(n)).
  - (나) 순서를 쓰는 함수(`sort`·`sort_by`·`min`·`max`)와 `unique`만 미리 훑고, `==`는 지금대로 둔다.
  - (다) 그대로 두고 명세에 "에러는 비교가 실제로 본 쌍에서만 난다"를 더 분명히 적는다.
- 처리: 결정 D87((가)). 커밋 `feat!: a comparison checks types to the end, whatever the values (D87)`. `value/compare.rs`: 엄격한 `walk`가 첫 차이에서 돌아가지 않고 `differs`만 기억한 채 겹치는 모양(`shared_shape`: 짧은 리스트의 끝까지, 두 맵이 함께 가진 키, 이름 같은 태그의 인자)을 끝까지 걷는다. `compare`도 첫 차이를 답으로 쥐고 끝까지 간다. 개수가 다른 튜플은 `tuple_sizes`(E0517·E0501). `value/index.rs` `contains`는 리스트를 끝까지 보고, `unique`는 원소마다 첫 원소와 `eq`, `Sort::run`과 `best`는 모든 키를 첫 키와 `compare`. 패턴의 `equal`은 그대로다. 저장소에서 걸린 것: `extras/http.fung` `_query_pairs`의 `p != ()`(패턴 `_is_pair`로), 골든 `eq_sym_str_pairs`·`eq_types`. 측정(릴리스, 200만 원소): 첫 원소가 다른 리스트의 `==` 0.03 → 12.7 ms, 첫 원소에서 찾는 `in` 0.0005 → 10 ms, `sort` 50.7 → 55.6 ms, `unique` 169 → 182 ms. 명세 `values.md`·`operators.md`, 설계 02. 시험 `vm/tests.rs` `comparisons_check_types_whatever_the_values`, `tests/lang/err_compare_types_later.fung`·`err_sort_mixed_rows.fung`. 남는 것: 길이가 다른 리스트가 섞인 정렬은 첫 원소가 짧으면 뒤 원소끼리의 타입을 미리 보지 못한다(설계 02). VA44(공유 구조의 지수 시간)는 손대지 않았고, `==`가 끝까지 걷게 되어 다른 값끼리도 해당한다.
- 처리(이어서): 결정 D91(2026-11-13)로 같음 부분을 되돌렸다. 커밋 `feat!: values of different types are not equal, with no error (D91)`. `==`·`!=`·`in`·`unique`는 타입이 달라도 에러 없이 `false`이고 E0517은 은퇴했다(`value/compare.rs` `check_fns`만 남김: 함수 둘은 E0501). `walk`는 다시 첫 차이에서 멈추고 `contains`는 찾으면 멈춘다. D87에서 남은 것은 순서 쪽이다: `compare`가 끝까지 가는 것, 튜플 개수의 E0501, `Sort::run`·`best`의 사전 검사. 언어 시험의 E0517 파일 열 개와 `eq_sym_str_pairs`를 지우고 `eq_types.fung` 하나로 모았다. 명세 `values.md`·`operators.md`·`lexical.md`, 에러 E0517(은퇴), 배우기 3·4·13장, 예제 `json-fields`, 레퍼런스 `convert`·`collections`, 설계 02·07·09, open-questions. 시험 `vm/tests.rs` `values_of_different_types_are_not_equal`, `value/tests.rs` `eq_says_false_for_different_types_and_refuses_functions`. 측정은 아래 커밋 메시지와 설계 02.

### SP57. `match` 절 본문의 마지막 람다가 말없이 새 절이 된다 — 결정, 낮음 (다시 확인)
- 재현:
  ```
  fn pick(m) = match m:
    1 =>
      print("one")
      y => y + 1
  ;;
  print(pick(1), pick(5))      → one / nil 6     (람다를 돌려주려던 것)
  ```
- 근거: `match.md`의 세 항목이 이 모호함을 다룬다("파서는 문장마다 먼저 `패턴 =>`인가를 본다", guard의 `이름 =>`, E0235로 드러나는 경우). E0235는 뒤에 절이 더 있을 때만 잡는다. 마지막 절 자리에서는 조용하다.
- 원인 위치: `parser/stmt.rs`의 절 본문 읽기(문장마다 패턴을 먼저 시도).
- 고치는 법(결정):
  - (가) **추천**: 절의 본문은 식 하나이거나 블록이다(`패턴 => 식`, 여러 문장이면 `패턴 =>: … ;;`). 절은 줄바꿈이나 `;`로 나뉜다. "문장마다 패턴을 먼저 본다"는 규칙과 그 설명 두 항목이 사라지고 `1 => y => y + 1`이 람다가 된다. 여러 문장짜리 절을 쓰는 코드를 모두 고쳐야 한다.
  - (나) 절 본문의 둘째 문장부터는 괄호 없는 `이름 =>`를 E0235로 한다(새 절은 본문이 한 문장일 때만 이어 쓸 수 없게 되어 예외가 는다).
  - (다) 그대로.
- 처리: 결정 D92(2026-11-13): 바꾸지 않는다.

### SP58. 인자 없는 태그가 심볼이라서 생기는 예외 조항들 — 결정, 낮음 (다시 확인, open-questions "남은 일"의 두 항목을 하나로)
- 재현:
  ```
  print(type_of(Red), type_of(Red(1)))   → 'sym 'Red
  fn f(c: Red) = c; f(Red); f(Red(1))    → 둘 다 통과
  print(Red)                             → 'Red     (소스에 쓰면 E0128)
  print(Red == Blue, Ok(1) == Red)       → false / E0517
  ```
- 근거: 지금 명세에 붙은 조항: "대문자 심볼은 태그로 쓴다(E0128)", "타입 이름은 인자 없는 태그(심볼)도 받는다", "`Ok`·`Err`만은 값 하나를 담은 태그만", "괄호 없는 태그 패턴 `Red`는 심볼에만 맞는다", "`x |> (Red)`는 심볼을 부르는 E0401". open-questions에 남은 "대문자 심볼의 출력"과 "`type_of`와 태그의 타입"도 같은 뿌리다. 열거형을 쓰는 방법도 둘이다(`'red`와 `Red`).
- 고치는 법(결정):
  - (가) **추천**: 인자 없는 태그는 심볼이 아니라 값 0개인 태그다. `type_of(Red)`는 `Red(1)`과 같은 답을 내고, `Red`로 찍히고, `Red == 'red`는 E0517이다. 심볼은 소문자 이름만이고 `sym("Red")`는 에러다. 위의 다섯 조항이 "태그는 이름과 값 0개 이상"으로 줄고 출력이 소스로 되읽힌다. `type_of`가 태그에 무엇을 돌려줄지(`'tag` 하나, 추천)도 함께 정한다.
  - (나) open-questions의 추천대로 출력만 `Red`로 바꾼다.
  - (다) 그대로.
- 처리: 결정 D103((나)). 커밋 `feat!: the remaining decisions of the fourth review (D93 to D107)`. `fung_front::consts::symbol`이 대문자로 시작하는 심볼을 따옴표 없이 쓰고 `value/format.rs`·상수 표기가 그것을 쓴다. open-questions의 "대문자 심볼의 출력"을 지웠다. `type_of(Circle(1))`도 `Circle`로 찍힌다. 인자 없는 태그와 심볼의 분리((가))는 하지 않았다.

### SP59. 표기가 둘인 자리 — 결정, 낮음 (다시 확인)
- 재현과 추천:
  1. `[x, ...] = a`와 `[x, ..._] = a`가 같다. → 이름 없는 `...` 하나만 남기고 `..._`는 에러(추천), 또는 반대.
  2. `"$5"`와 `"\$5"`가 같다. D47은 `$이름`만 막았다. → 보통 문자열의 맨 `$`는 늘 `\$`로 쓴다(`${`만 보간). "그 밖의 `$`는 글자다"라는 조항이 사라진다(추천, VA45와 함께).
  3. `fn f() = return 1`, 블록 끝의 `return 2`, `c then return 1 else 2`는 `return`이 없어도 같다. → 꼬리 위치의 `return`은 컴파일 에러(추천: `return`은 일찍 돌아갈 때만).
  4. `import fs`와 `import fs as f2`를 한 파일에 함께 쓸 수 있다(같은 이름 두 번만 E0309). → 같은 모듈을 두 번 import하면 E0309(추천). 원인 위치 `resolve/module.rs` `import_stmt`·`bind_module`.
- 일부러 둔 것(바꾸지 않기를 추천): `if … else` 식과 `then … else`, `fn f(x) = e`와 블록 본문, 람다와 익명 `fn`(배경 문서의 "짧은 축약 문법" 원칙), `m.k`와 `m["k"]`, 파이프, `contains`·`is_empty`(11-11 SP51).
- 처리: 결정 D93·D94·D95·D96. 커밋 `feat!: the remaining decisions of the fourth review (D93 to D107)`. (1) `..._`는 E0112(`parser/pattern.rs` `plist`), (4) 같은 모듈의 두 번째 import는 E0309(`resolve/module.rs` `import_stmt`; 이름들은 그대로 묶어 연쇄 에러를 막음). (2) `"$5"`와 (3) 꼬리 위치의 `return`은 바꾸지 않는다.

### SP60. `and`·`or`의 오른쪽 호출만 꼬리 호출이 아니다 — 결정, 낮음 (다시 확인, 기존 결정 재검토)
- 재현: `fn allpos(i) = i == 0 or (i > 0 and allpos(i - 1)); print(allpos(200000))` → E0804. 같은 뜻의 `then … else`는 `true`.
- 근거: 설계 03의 이유는 "결과가 bool인지 검사해야 하므로"다. D44가 같은 종류의 문제(반환 타입 검사)를 `TAILCHECK`로 풀었는데 `and`·`or`만 예외로 남았다.
- 원인 위치: `compiler.rs` `op`(525~531행).
- 고치는 법(결정): (가) **추천**: 꼬리 자리의 `and`·`or` 오른쪽 호출은 `TAILCHECK`에 bool 검사를 실어 `TAILCALL`로 낸다(실패하면 E0505를 그 연산자 자리로). (나) 그대로.
- 처리: 결정 D97((가)). 커밋 `feat!: the remaining decisions of the fourth review (D93 to D107)`. 새 명령 `TAILCHECKBOOL`(59번째): 꼬리 자리의 `(bool call)`이 그 명령과 `TAILCALL`로 나간다(`compiler.rs` `op`, `compiler/fn_.rs` `call_checked`). 프레임의 `Check`가 반환 타입 대신 "불이어야 한다"를 실을 수 있다(`vm/frame.rs`, `vm/call.rs` `tail_check_bool`·`run_checks`). 검증기는 그 명령 뒤에 `TAILCHECK`나 꼬리 호출이 오는지 본다. 시험 `vm/tests.rs` `the_right_side_of_and_or_is_a_tail_call`(깊이 30만, 반환 타입과 함께, 불이 아닌 값의 E0505).

### SP61. `dbg`가 부르는 꼴에 따라 다른 것을 찍는다 — 결정, 낮음 (다시 확인, 기존 결정 11-08 SP11 재검토)
- 재현: `y = dbg(1 + 2)` → `[x.fung:3] 1 + 2 = 3`. `f = dbg; x = f(1 + 2)` → `dbg = 3`. `map(xs, dbg)`도 뒤쪽이다.
- 근거: 컴파일러의 모양 판정(`compiler/m6.rs` `is_dbg_call`)이 언어의 뜻으로 새어 나온다.
- 고치는 법(결정): (가) **추천**: 직접 호출이 아닌 `dbg`(값으로 쓰기, 펼침, 이름 인자)는 컴파일 에러. `map(xs, x => dbg(x))`로 쓴다. (나) 그대로.
- 처리: 결정 D98(2026-11-13): 바꾸지 않는다.

### SP62. 리터럴만으로 확정되는 오류를 `check`가 통과시킨다 — 결정, 낮음 (일부 다시 확인)
- 재현: `1(2)`, `"a"(2)`, `nil.x`, `1[0]`, `1 + "a"`, `not 1`, `if 1: …`, `nil!`, `"${nil}"`, `for x in 5: …`가 모두 `check`를 통과하고 실행 중에 죽는다. `x = 1 ?? 2`는 실행 중 에러도 없다(오른쪽은 결코 쓰이지 않음). `1.foo`만 렉서가 E0102로 막고 `1 .foo`는 통과한다.
- 원인 위치: `lexer/number.rs` `number_inner`(공백 없는 `.이름`만), resolve·컴파일러에 리터럴 피연산자 검사 없음(`compiler/fold.rs`는 접기만).
- 고치는 법(결정): (가) **추천**: 상수 접기가 실패를 아는 식은 실행 때와 같은 번호의 컴파일 에러로 하고, `1.foo`의 렉서 특례를 그 규칙에 합친다. `nil`도 결과값도 될 수 없는 리터럴 왼쪽의 `??`도 에러. (나) `??`만. (다) 그대로.
- 처리: 결정 D99((가), 범위를 좁힘). 커밋 `feat!: the remaining decisions of the fourth review (D93 to D107)`. `resolve/value.rs` `literal_type_error`: 리터럴을 부름(E0401), 불이 아닌 리터럴 조건·`not`·`and`/`or`의 피연산자(E0505), 두 스칼라 리터럴의 뜻 없는 산술·순서 비교(E0501), 리터럴 뒤의 `??`(새 번호 E0243). **넣지 않은 것**: 값이 정하는 에러(`1 // 0`, 넘침; 매뉴얼과 시험이 실행 중 에러의 예로 쓴다), 리터럴의 필드·인덱스(`nil.x`, `1[0]`), `nil!`, 보간의 `nil`, `for x in 5`. `1.foo`의 렉서 특례는 그대로다.

### SP63. 아무 일도 하지 않는 블록과 읽히지 않는 정의가 통과한다 — 결정, 낮음 (일부 다시 확인)
- 재현(모두 종료 코드 0): `if true: ;;`, `if x > 0: else: print(1) ;;`, `for _ in [1]: ;;`, `test "empty": ;;`(단언 없는 시험이 통과), 진입 파일의 부르지 않는 `fn unused() = 1`.
- 근거: `blocks.md` "버려지는 자리의 식은 무언가를 해야 한다"(E0234), `scope.md` 7번 규칙. "최상위 `fn`은 변수가 아니라 세지 않는다"는 예외 조항이다.
- 원인 위치: `resolve/value.rs` `check`(빈 `Do`는 쓰는 자리에서만 E0224), `resolve/names.rs` `report_unused_file`.
- 고치는 법(결정): (가) **추천**: 버려지는 자리의 빈 블록과 빈 `test`는 E0234. 부르지 않는 최상위 `fn`은 SP52의 결정에 따른다(공개 이름을 인터페이스로 보면 `_`로 시작하는 `fn`만 E0239). (나) 빈 블록만. (다) 그대로.
- 처리: 결정 D100. 커밋 `feat!: the remaining decisions of the fourth review (D93 to D107)`. 본문이 빈 `test`만 새 번호 E0242(`resolve/value.rs`). 빈 `if`·`for`와 부르지 않는 최상위 `fn`은 바꾸지 않는다(D86: 공개 이름은 인터페이스).

### SP64. 타입 표기에서 조용히 통과하는 것 — 결정, 낮음 (다시 확인)
- 재현:
  ```
  fn f(a: int | int, ...r: any) = a + len(r)     → 통과
  fn h(x: Int) = x; h(1)                         → 실행 중 E0403 "must be Int, got int"
  type unused_alias = int                        → 쓰지 않아도 통과
  ```
  `any | int`, `num | int | float`도 통과한다.
- 근거: 패턴의 같은 실수는 E0241이다. `types.md`는 "`...rest`는 `list`만 적을 수 있다(다른 표기는 E0228)"고 한다.
- 원인 위치: `resolve/alias.rs` `expand_types`(중복을 말없이 합침), `resolve/fn_.rs` `check_types`(`any` 허용).
- 고치는 법(결정, 모두 추천): 유니온의 같은 이름과 포함 관계는 컴파일 에러. 내장 타입과 대소문자만 다른 태그 이름(`Int`·`String`·`List`·`Bool`·`Nil`)은 컴파일 에러(도움말 `int`). `...rest: any`는 E0228. 쓰지 않는 `type`은 E0239.
- 처리: 결정 D101. 커밋 `feat!: the remaining decisions of the fourth review (D93 to D107)`. 내장 타입과 대소문자만 다른 태그 이름(`Int`·`String`·`Integer`·`Boolean` 포함)은 E0212에 소문자 이름을 권하는 도움말(`resolve/alias.rs` `expand`). 중복 유니온, `...rest: any`, 쓰지 않는 별칭은 바꾸지 않는다.

### SP65. 보이지 않는 글자와 제어 문자가 이름·주석·문자열에 들어간다 — 결정, 낮음 (다시 확인, D59 확장)
- 재현:
  ```
  a<U+FE0F> = 1; a = 2; print(a<U+FE0F>, a)     → 1 2    (눈으로 같은 두 이름)
  x = 1 # <ESC>[2K hidden                        → 실행됨  (터미널의 cat은 그 줄을 지운다)
  ```
  U+200C·U+200D·U+034F, 주석·문자열 안의 BS·NUL·DEL·U+2028도 같다.
- 근거: `lexical.md`가 같은 까닭("터미널과 편집기가 서로 다르게 그려서")으로 홀로 선 CR과 양방향 제어 문자를 E0101로 한다. 예외 목록이 그 둘뿐이다.
- 원인 위치: `unicode/generate.rs`의 `name_continue = XID_Continue`, `lexer.rs` `ident`·`comment`, `lexer/string.rs` `symbol`·`str`·`raw_str`.
- 고치는 법(결정): (가) **추천**: 이름·심볼의 `Default_Ignorable_Code_Point`는 E0101. 주석과 문자열(원시 포함) 안의 제어 문자(탭과 원시 문자열의 줄 끝 제외)와 U+2028·U+2029도 E0101이고 문자열에는 `\u{…}`로 쓴다. "보이는 것과 읽히는 것이 다른 글자는 받지 않는다" 하나가 된다. (나) 그대로.
- 처리: 결정 D102. 커밋 `feat!: the remaining decisions of the fourth review (D93 to D107)`. `unicode::is_invisible`(Default_Ignorable_Code_Point의 범위를 직접 적음)이 참인 글자는 이름의 시작도 이어짐도 아니다(`lexer.rs`): 그 글자는 낱글자로 남아 E0101이 된다. 주석·문자열 안의 제어 문자는 바꾸지 않는다. 표 생성(`unicode/generate.rs`)에는 넣지 않았다(범위가 짧고 UCD 파일이 필요 없게).

### SP66. 꼬리 위치 목록이 문서 세 곳에서 서로 다르고 구현과도 다르다 — 문서, 낮음 (다시 확인)
- 재현: `fn h(i) = i == 0 then 0 else (nil ?? h(i - 1)); print(h(300000))` → `0`(`??`의 오른쪽은 꼬리 호출이다). `return g(i - 1)`도 꼬리다.
- 근거: `functions.md` "꼬리 호출"은 `f() ?? b`의 왼쪽만 "꼬리가 아닌 것"으로 적고 `??`의 오른쪽과 `return`의 피연산자를 적지 않았다. 설계 03, `manual/vm/09-closures.md`도 서로 다르다.
- 고치는 법: 세 곳을 구현대로 맞추고 `tests/lang`에 깊이 20만 사례를 넣는다.
- 처리: D97과 같은 커밋에서 `functions.md`의 꼬리 위치 목록에 `return f()`, `a ?? f()`의 오른쪽, `and`·`or`의 오른쪽을 더했다. `vm/09-closures.md`에 `TAILCHECKBOOL` 한 문장. 설계 03은 고치지 않았다.

## 5. 앞단과 진단 (FE)

### FE63. `then … else` 가지의 판정이 desugar가 만든 모양에 따라 갈린다 — 버그, 중간 (다시 확인)
- 재현:
  ```
  fn g() = 1
  x = true then match g(): _ => 1 ;; else nil     → E0223 `if` without `else` used as a value (2:10)
  x = true then g2(_, 1) else nil                 → 같은 E0223
  x = true then 1 else nil                        → 정상
  ```
  버려지는 자리에서는 `match`의 대상이 이름인지 호출인지에 따라 E0223·E0234가 갈린다.
- 근거: `conditions.md`의 `then … else`는 `else`가 반드시 있다. 있는데 "else 없는 if"라고 한다.
- 원인 위치: `resolve/value.rs` `check`의 `Head::If` 갈래(140·149행)와 `effect_free`(233행). "블록 if"를 `kids[1].head == Head::Do`로, "else 없음"을 `kids[2].head == Head::Nil`로 알아보는데, desugar가 `Then`도 `If`로 바꾸고 then 가지가 `Do`로 낮춰지는 식과 글자 그대로의 `nil`이 그 모양과 겹친다. open-questions의 "FE62에서 하지 않은 것"(모양 다시 알아보기)의 실제 피해다.
- 고치는 법: desugar가 표시를 남긴다(`Then`은 코어에서도 구별되는 head, 블록 `if`의 빠진 `else`는 전용 표시). `partial_shape`·`hidden`도 같은 표시로 바꾼다.
- 처리: 2026-11-13 커밋 `fix: four silent mistakes (FE63, VA41, VA43, VA47)`. desugar가 `c then a else b`를 코어에서도 `(then c a b)`로 남긴다(`Head::Then`; 컴파일러와 수명 분석은 `If`와 같게 다룸). 그래서 `resolve/value.rs`가 가지의 모양으로 블록 `if`를 추측하지 않는다: `Then`의 가지는 늘 쓰이는 값이고 E0223은 블록 `if`에만 난다. desugar도 `then`의 가지를 버려지는 자리로 걷지 않는다(명세 `blocks.md`의 버려지는 자리 목록대로). `fung dump core`의 표기가 바뀜(저장소 33개 파일의 코어 dump, 바이트코드는 그대로). 매뉴얼 `vm/05-desugar.md`. 시험 `resolve/tests.rs` `values_used_must_not_be_nil`. 하지 않은 것: `partial_shape`·`hidden`의 모양 다시 알아보기(open-questions의 FE62 남은 일 그대로).

### FE64. 값 없는 `return` 다음 줄의 이어짐 토큰이 E0132를 비켜 간다 — 버그, 중간 (다시 확인)
- 재현:
  ```
  fn f():
    return
      ?? 1
  ;;
  print(f())        → nil, 종료 코드 0
  ```
  `return⏎ |> g`, `return⏎ .foo`, `return⏎ and c`도 같다. 같은 줄의 `return ?? 1`은 E0110이다.
- 근거: `functions.md` "`return`을 연산자의 피연산자에 쓰면 E0132".
- 원인 위치: `parser/expr.rs` `return_expr`(406행)가 돌려준 노드로 `expr_loop`(139행)가 `infix_loop`를 계속 돈다.
- 고치는 법: `prefix`가 `Return`을 돌려주면 `infix_loop`에 들어가지 않는다. 다음 줄의 `?? 1`은 E0110이 된다.
- 처리: 2026-11-13 커밋 `fix: a return without a value takes no continuation line (FE64)`. `parser/expr.rs` `expr_loop`가 `prefix`가 돌려준 `Return`으로는 `infix_loop`에 들어가지 않는다. 다음 줄은 E0131(`return` 뒤의 문장), `then` 가지 안에서는 E0112. 시험 `parser/tests.rs` `return_stands_as_a_statement_or_ends_a_branch`.

### FE65. 부분 적용과 지역 `fn` 호출은 인자를 컴파일 때 검사하지 않는다 — 버그, 중간 (다시 확인)
- 재현(`"ran"`이 먼저 찍힌다):
  ```
  fn add(a, b) = a + b
  print("ran"); g = add(1, 2, _)                       → ran 뒤 실행 중 E0402
  fn f(): fn g(a) = a; print("ran"); g(1, 2) ;;        → ran 뒤 실행 중 E0402
  ```
  `add(_, zz: 1)`, `len(_, 1, 2)`, `map(xs, str(_, 2))`도 `check`를 통과한다.
- 근거: `functions.md` "호출 대상이 컴파일 때 정해지면 컴파일 때 같은 번호와 문구로 잡고"(D43).
- 원인 위치: `desugar.rs` `partial_with`(398행)가 호출 대상을 숨은 변수에 담아 `resolve/call.rs` `known_callee`(232행)가 알아보지 못한다. 지역 `fn`은 `global_sigs`에 없다.
- 고치는 법: 부분 적용은 원래 호출 대상으로 검사한다(구멍은 위치 인자 하나). 지역 `fn`은 SP54의 (가)나 (나)로 이름이 한 번만 묶이면 같은 표에 넣는다.
- 처리: 고침(2026-11-13). `resolve/scope.rs` `Scope::sigs`·`FnScope::self_sig`, `resolve/call.rs` `block_sig`: 블록 안 `fn`(자기 호출 포함)과 부분 적용이 숨은 변수에 담는 호출 대상을 그 이름을 묶은 블록에서 찾아 같은 검사를 한다(D88로 이름은 한 번만 묶이므로 묶은 블록이 답을 안다). `functions.md` 고침.

### FE66. 줄 이어짐 규칙 1의 `else`·`not`이 일부 자리에서만 이어진다 — 버그, 낮음 (다시 확인)
- 재현: `x = if true: 1 else⏎: 2 ;;` → E0112. `x = 1 not⏎ in [1]` → E0110. `x = true then 1 else⏎ 2`는 된다.
- 근거: `blocks.md` 규칙 1의 목록에 `not`·`else`가 자리 구분 없이 있다.
- 원인 위치: `parser/stmt.rs` `if_chain`(166~179행), `parser/expr.rs` 195행 `not_in`.
- 고치는 법: 두 자리에서 줄바꿈을 건넌다.
- 처리: D85와 같은 커밋. `parser/stmt.rs` `if_chain`이 `else` 뒤의 줄바꿈을 건너고, `parser/expr.rs`의 `not in` 판정과 소비가 괄호 밖에서도 줄바꿈을 건넌다. 시험 `parser/tests.rs` `a_line_ending_in_else_or_not_continues`.

### FE67. 슬라이스 끝의 맨 람다를 받는다 — 버그, 낮음 (다시 확인)
- 재현: `xs[x => x..2]` → 실행 중 `E0501 cannot slice with int and fn`.
- 근거: `access.md` "그보다 약한 연산자가 끝의 맨 위에 오면 E0125".
- 원인 위치: `parser/expr.rs` `looser_than_a_slice`(88행)에 람다가 없다.
- 고치는 법: 괄호 없는 람다가 슬라이스 끝이면 E0125.
- 처리: 고침(2026-11-13). `parser/expr.rs` `looser_than_a_slice`가 괄호 없는 람다를 E0125로 본다.

### FE68. `import "./lnk/../x"`가 운영체제와 다른 파일을 읽는다 — 버그, 낮음
- 재현: `lnk -> d1/sub`, `./x.fung`과 `d1/x.fung`이 따로 있을 때 `import "./lnk/../x"`는 `./x.fung`을, `fs.read("./lnk/../x.fung")`은 `d1/x.fung`을 읽는다.
- 원인 위치: `loader.rs` `load_file`(362·375행) → `abs_key` → `clean`(488행)이 `..`를 글자로 지운다.
- 고치는 법: 여는 경로는 `..`를 지우지 않고 운영체제에 맡긴다. 같은 파일인지는 이미 `file_id`가 가린다.
- 처리: 고침(2026-11-13). `loader.rs` `same_file_clean`: 글자로 정리한 경로가 쓴 그대로의 경로와 같은 파일일 때만 정리한 경로를 쓰고, 다르면(`..` 앞에 링크) 쓴 그대로 연다. 시험 `a_parent_past_a_link_is_the_systems`.

### FE69. import된 파일의 최상위에 `_ = 식`이 온다 — 버그, 낮음 (다시 확인)
- 재현: `_ = print("side effect")`가 든 파일을 import하면 `side effect`가 찍힌다. 구조 분해 대입 `(a, b) = (1, 2)`도 된다.
- 근거: `modules.md` "막는 것은 값을 남기지 않는 맨 문장이 모듈을 가져올 때 몰래 도는 일".
- 원인 위치: `resolve.rs` `root_block` 413행(`Head::Let`이면 통과).
- 고치는 법: 모듈 최상위의 `_ = 식`은 E0306. 구조 분해 대입은 이름을 남기므로 두고 명세 문장을 "이름이나 패턴 `= 식`"으로 고친다.
- 처리: 고침(2026-11-13). `resolve.rs` `root_block`이 라이브러리 최상위의 `_ = 식`을 E0306으로 본다. 구조 분해 대입은 그대로 되고 `modules.md`에 적었다.

### FE70. E0235가 `match`가 같다고 보는 수 리터럴을 놓친다 — 버그, 낮음 (다시 확인)
- 재현: `1 => 1; 1.0 => 2; _ => 0`에서 `f(1.0)`은 `1`이다(둘째 절은 결코 돌지 않는데 통과). `-0.0`과 `0.0`도 같다.
- 근거: `values.md` "리터럴 패턴 `1.0`은 `1`에도 맞고". `0x10`과 `16`은 이미 잡는다.
- 원인 위치: `resolve/pattern.rs` `repeated_literals`(213행, `to_string` 비교).
- 고치는 법: 수 리터럴은 값으로 견준다. 명세의 "쓴 그대로 같은"을 "값이 같은"으로 고친다.
- 처리: 고침(2026-11-13). `resolve/pattern.rs` `literal_key`: 수 리터럴은 값으로 견준다(`1`과 `1.0`, `0.0`과 `-0.0`). `match.md`·`errors.md` 문장 고침.

### FE71. 숫자 리터럴의 진단 번호가 어긋난다 — 버그(진단), 낮음 (다시 확인)
- 재현: `0b12` → `E0135 out of range`(진법에 없는 숫자). `9223372036854775808` → `E0102 malformed number`(범위 밖). `9999999999h` → `E0105 malformed duration`. `lexical.md`의 표는 `1e-400`을 E0102라고 하는데 실제는 E0135다.
- 원인 위치: `lexer/number.rs` `prefixed_int`(217~226행)·`duration`, `parser/expr.rs` 311행, `parser/pattern.rs` 89행.
- 고치는 법: 진법에 없는 숫자는 E0102, 범위 밖은 모두 E0135. 명세 표를 고친다.
- 처리: 고침(2026-11-13). `lexer/number.rs`: 진법에 없는 숫자는 E0102, 기간의 범위 밖은 E0135(도움말에 최댓값). `parser/expr.rs`·`pattern.rs`: 부호 없는 최소 정수의 크기는 E0135. `lexical.md` 표 고침.

### FE72. `...rest`가 묶는 이름의 에러가 `[`를 가리킨다 — 버그(진단), 낮음
- 재현: `[head, ...tail] = v`의 `E0239 unused variable tail`이 `[hea` 아래에 밑줄을 긋는다. `[a, ...a]`의 E0221도 `[`를 가리킨다.
- 원인 위치: `parser/pattern.rs` `plist` 233행(`Prest` 노드에 리스트의 위치를 씀).
- 고치는 법: 이름 토큰의 위치를 쓴다.
- 처리: 고침(2026-11-13). `parser/pattern.rs` `plist`: `Prest`의 위치는 이름 토큰(이름이 없으면 `...`).

### FE73. 이름 에러가 소스 순서가 아니라 패스 순서로 나온다 — 정리(진단), 낮음
- 재현: 여섯 줄에 에러가 하나씩 있는 파일에서 2행, 5행, 1행, 4행, 3행, 6행 순으로 나온다(최상위 `fn`을 먼저 걷고 값 자리 검사는 맨 뒤).
- 원인 위치: `resolve.rs` `resolve`(125~129행: E0239만 있을 때만 정렬).
- 고치는 법: 돌려주기 전에 늘 (줄, 칸)으로 안정 정렬한다.
- 처리: 고침(2026-11-13). `resolve.rs`: 돌려주기 전에 늘 (줄, 칸)으로 정렬한다. 첫 에러만 보던 시험 셋의 소스를 고쳤다.

### FE74. 진단의 작은 것들 — 정리(진단), 낮음
1. 대문자 이름을 묶는 자리의 번호가 갈린다. `Count = 1`은 E0225인데 `fn Foo() = 1`·`fn f(X)`·`for X in`은 E0112, `f = X => 1`은 E0110이다. → 모두 E0225.
2. `import _native._native.fs`의 도움말이 내부 모듈 `_native.fs`를 권한다(`loader.rs` `load_import`).
3. 진입 파일이 자기를 import하면 E0307이 아니라 `E0302 circular import: p.fung → p.fung`이다.
4. 최상위의 `return`은 E0211, `?`는 E0214인데 기본값 안에서는 둘 다 E0211이다.
5. 맵 리터럴은 UTF-8이 아닌 키(`{"\xff": 1}`)를 받는데 맵 패턴은 E0112라 그 키를 패턴으로 꺼낼 수 없다(`parser/pattern.rs` `pmap`).
- 처리: 고침(2026-11-13). 1 대문자 이름을 묶는 자리는 모두 E0225(`destructure_term`, `fn` 이름, `Tag =>`). 2 내장 표에 없는 `_native.*`는 E0301. 3 진입 파일의 자기 import는 E0307. 4 바꾸지 않음: E0211은 "기본값 안"이라는 자리의 규칙이고 `return`·`?` 둘에 같은 번호라 일관되다(E0214는 최상위 `?`의 `!` 도움말 때문에 따로 있다). 5 맵 패턴이 UTF-8이 아닌 키를 받는다(`Pattern::keys`가 바이트 열, dump는 따옴표로).

### FE75. 앞단의 이차 시간 다섯 곳 — 성능, 낮음 (첫 줄 다시 확인)
| 입력 | 시간 | 원인 위치 |
|---|---|---|
| `1 \| 2 \| … \| n` 대안 패턴 | 5천 2.9초, 2만 48.6초 | `resolve/pattern.rs` `dead_alternatives`(194행) |
| 대안 n개가 든 튜플 패턴 | 2만 10.5초 | `resolve/pattern.rs` `bind_pat` 109행 `seen.clone()` |
| 이름 n개의 패턴 매개변수 | 5만 2.2초, 20만 22.9초 | `parser/fn_.rs` `note_pattern` 48행 |
| 공개 최상위 `fn` n개 | 4만 1.3초, 8만 5.9초 | `resolve.rs` 144행 `exports.contains`(Vec) |
| 한 줄(40만 글자)에 에러 5만 개 | 15.3초 | `diag.rs` `write_excerpt` 240행 |
- 고치는 법: 앞의 넷은 해시 집합(둘째는 `seen`을 길이 표시로 되돌리기), 다섯째는 줄의 글자 색인을 줄마다 한 번만 만든다.
- 참고: `x ?? 0 ?? 1 ?? …` 사슬은 `check`가 10만 항 1.7초, 20만 항 7.1초로 이차다(컴파일러 단계, 원인 위치는 찾지 않음).
- 처리: 고침(2026-11-13). 다섯 곳 모두 고침: `dead_alternatives`는 한 번 훑기, `bind_pat`은 `seen`을 복사하지 않고 대안이 더한 이름을 빼기, `note_pattern`·`exports`는 해시 집합, `write_excerpt`는 보여 줄 창만 떼기(ASCII 줄은 줄 길이와 무관; 비ASCII 긴 줄은 줄마다 글자 수를 세는 비용이 남는다: 2만 에러 1.8초). 측정(릴리스): 대안 2만 0.01초, 이름 10만 패턴 매개변수 + 에러 10만 개 3.8초, `fn` 8만 0.3초, 한 줄 에러 5만 1.3초. 참고의 `??` 사슬은 보지 않았다.

### FE76. 파일 끝의 주석이 이어진 문장의 들여쓰기를 받는다 — 버그(포매터), 낮음 (D85 작업 중 찾음, 다시 확인)
- 재현: `printf 'x = 1 +\n  2\n# c\n' | fung fmt -` → 마지막 줄이 `  # c`로 들여쓰인다. 주석 뒤에 문장이 하나라도 있으면 들여쓰지 않는다. `1951b727`의 바이너리도 같다.
- 근거: 주석은 이어진 문장의 일부가 아니다. 골든 파일의 끝에 `# expect:`를 두면 포매터가 그 줄을 옮긴다.
- 원인 위치: 찾지 않음(`crates/fung-front/src/format/`의 이어짐 들여쓰기가 파일 끝에서 풀리지 않는 것으로 보인다).
- 고치는 법: 이어짐 들여쓰기를 문장이 끝난 줄에서 푼다(다음 토큰이 없어도).
- 처리: 고침(2026-11-13). `format/layout.rs` `layout`: 마지막 코드 줄 뒤의 주석은 들여쓰지 않는다(파일 끝에서는 모든 블록과 문장이 끝났다).

## 6. 컴파일러와 바이트코드 (VM)

### VM37. 모든 갈래가 `return`하는 `match`의 주제를 일찍 놓아 E0801로 죽는다 — 버그, 높음 (다시 확인)
- 재현:
  ```
  fn handle(kind, x):
    n = if x > 0:
      x
    else:
      match kind:
        'a => return 0
        _ => return -1
      ;;
    ;;
    (kind, n)
  ;;
  print(handle('b, 1))      → ('b, 1)
  print(handle('b, 0))      → error[E0801]: internal error: VM read an unassigned slot (6:7)
  ```
  `dump bytecode`의 `handle`: else 가지 시작의 `CLEARLOCAL 0`(`kind`를 놓음) 바로 뒤에 `LOADLOCAL 0; MATCH`가 온다.
- 근거: 주제가 이름인 `match`이고, 그 이름이 `match` 뒤에서 읽히며, 갈래가 모두 `return`으로 끝나면 난다. 올바른 프로그램이 실행 중 내부 에러로 죽는다. 디버그 빌드의 검증기는 잡는다(`LOADLOCAL reads slot 0, which may be unassigned`).
- 원인 위치: `compiler/moves.rs` `Back::join`(359~363행). `return`은 살아 있는 칸을 모두 죽인다. 갈래가 모두 그러면 "선택 앞" 값이 false가 되고, 주제 읽기(`reads`)는 "선택 뒤에 죽어 있을 때"만 다시 살리므로 뒤에서 살아 있던 주제가 죽은 채로 남는다.
- 고치는 법: `reads`를 갈래 결과보다 우선하게 한다(주제는 선택 앞에서 늘 살아 있다). 영역 점검이 사본에서 확인: 재현이 `('b, 1)`과 `-1`을 찍고, 무작위 프로그램 9,304개에서 기준 해석기와 불일치 0.
- 처리: 2026-11-13 커밋 `fix: three lifetime bugs (VM37, VM38, VM39)`. `moves.rs` `Back::join`이 선택 앞에서 읽히는 칸(`reads`, `match`의 주제)을 갈래 결과와 상관없이 살아 있게 둔다. 시험 `tests/lang/local_lifetime.fung`(`handle`). 저장소의 다른 `.fung` 파일의 바이트코드는 바뀌지 않았다(dump 기대 출력의 diff가 그 파일뿐).

### VM38. 조건 자리의 `not (a and b)`가 건너뛴 쪽의 값을 놓지 않아 교착한다 — 버그, 중간 (다시 확인, 11-11 VM19 고침의 누락)
- 재현:
  ```
  fn ready(tx):
    send(tx, 1)!
    true
  ;;
  fn go(flag):
    (tx, rx) = channel(1)
    if not (flag and ready(tx)):
      for x in rx: x ;;
    else:
      []
    ;;
  ;;
  print(go(false))          → error[E0702]: deadlock: all tasks are waiting (8:5)
  ```
  같은 뜻의 `if flag and ready(tx): [] else: for x in rx: x ;; ;;`는 `[]`를 찍는다.
- 근거: `scope.md` "값의 수명"(가지가 시작할 때 다른 가지에서만 읽히는 변수를 놓는다).
- 원인 위치: `compiler/moves.rs` `Fwd::cond`(577~602행)에 `is_not_of_bool` 갈래가 없다. `Compiler::cond`(`compiler.rs:615`)에는 있다. 두 곳이 같은 모양 판정을 따로 구현한다.
- 고치는 법: `Fwd::cond`에 같은 갈래를 더한다(사본에서 확인). 근본적으로는 판정 함수 하나를 둘이 같이 쓴다.
- 처리: 같은 커밋. `moves.rs` `Fwd::cond`에 `is_not_of_bool` 갈래(`Compiler::cond`와 같은 모양). 판정 함수를 둘이 같이 쓰게 하는 정리는 하지 않았다(VM46과 함께). 시험 `local_lifetime.fung`(`negated`).

### VM39. 마지막 읽기가 기본값 식에 있는 매개변수는 함수가 끝날 때까지 값을 쥔다 — 버그, 중간 (다시 확인)
- 재현(`ready`는 VM38과 같음):
  ```
  fn go(tx, rx, ok = ready(tx)):
    print(ok)
    for x in rx: x ;;
  ;;
  fn start():
    (tx, rx) = channel(1)
    go(tx, rx)
  ;;
  print(start())            → true 뒤 error[E0702]: deadlock (7:3)
  ```
  `ok = ready(tx)`를 본문 첫 문장으로 옮기면 교착하지 않는다.
- 원인 위치: `compiler/fn_.rs:120`이 `moves::lives`에 본문만 준다. 서두(130~141행)의 기본값 식은 분석 밖이다.
- 고치는 법: 본문 시작(`Proto::body` 뒤)에 "기본값 식이 읽었고 본문 시작에서 죽어 있는 매개변수"의 `CLEARLOCAL`을 낸다. 인자를 모두 준 호출은 서두를 건너뛰므로 서두 안이 아니라 `body` 뒤여야 한다.
- 처리: 같은 커밋. `Lives::live_at_start`(본문 시작에서 읽히는 칸)를 더하고, `fn_.rs`가 타입 검사(`CHECKTYPE`) 뒤에 "기본값 식이 읽었고 본문 시작에서 죽은 매개변수"의 `CLEARLOCAL`을 낸다. 명세 `scope.md` "값의 수명", 설계 03. 시험 `local_lifetime.fung`(`drained`).

### VM40. 검증기가 믿고 넘기는 것들 — 정리, 중간
- 재현: 영역 점검이 사본의 `bytecode/tests.rs`에 원형을 손으로 만들어 넣었다. 아래가 모두 검증을 통과한다.
  1. `MAP 2`의 키 목록 상수가 1개짜리(VM은 `zip`으로 조용히 자른다, `vm/m4.rs:340`).
  2. `MAP`·`TAG`·`FIELD`·`IMPORT`·`TEST`의 상수 피연산자가 다른 종류의 상수(VM은 패닉하거나 조용히 다른 일을 한다).
  3. 패턴의 `Bind`가 `num_locals` 밖의 칸을 씀(남의 칸에 쓴다. 칸 번호가 64를 넘으면 검증기 자신이 패닉). 맵 패턴의 `keys`와 `kids` 길이 불일치.
  4. `Proto::body`가 코드 밖.
  5. 위치 인자 0개인 `CALLSPREAD`.
  6. `SLICE`·`RANGE`의 정의되지 않은 플래그 비트.
- 더 본 것(코드 읽기): `verify.rs:214~219`의 `Operand::Call` 검사는 `& 0xFFF` 뒤에 `> 0xFFF`를 물어 절대 걸리지 않는다.
- 원인 위치: `crates/fung-front/src/bytecode/verify.rs` `one`.
- 고치는 법: 상수의 종류를 `Operand`에 넣는다. 패턴 트리를 걸으며 칸 범위와 길이를 본다. `body`의 범위와 그 지점의 상태를 확인한다. 플래그에 상한을 둔다.
- 처리: 2026-11-13 커밋 `fix: the verifier checks what operands mean (VM40)`. `verify.rs`: `TAG`의 이름은 심볼, `MAP`의 키 목록은 값의 수만큼의 문자열 리스트, `FIELD`·`IMPORT`·`TEST`의 상수는 문자열. 패턴 트리를 걸어 묶는 칸·전역의 범위, 맵 패턴의 키와 패턴 수, 리터럴 유무를 본다(범위 밖 칸에서 검증기가 패닉하던 것도 사라짐). `Proto::body`는 코드 안이어야 하고, 인자를 모두 준 호출의 길(빈 스택, 매개변수만 찬 칸)을 본문으로 들어오는 길 하나로 더해 스택 높이와 칸을 함께 본다. 펼침 호출은 위치 인자 1개 이상, `SLICE`는 7, `RANGE`는 1까지. 빈 칸을 읽어도 되는 `LOADLOCAL; JUMPIFSET`은 기본값 있는 매개변수뿐이고 `JUMPIFSET`은 그 뒤에만 온다. 절대 걸리지 않던 `Operand::Call` 검사는 지웠다. 시험 `bytecode/tests.rs` `the_verifier_checks_what_the_operands_mean`. 매뉴얼 `vm/08-bytecode.md`(검사 여섯, `TAILCALLSPREAD`). 하지 않은 것: `Lit`의 리터럴 종류, 전역 패턴이 아닌 자리의 그 밖의 의미 검사.

### VM41. 릴리스 빌드는 검증기를 돌리지 않고 생성형 시험이 없다 — 결정, 중간
- 재현(영역 점검의 측정): `compiler.rs:111`의 `cfg!(debug_assertions)`를 늘 참으로 바꾼 릴리스 사본과 원래 사본. 지역 10만 개 함수의 `dump bytecode`는 둘 다 0.38초, 표준 모듈 여섯을 import하는 프로그램 200번은 2.74~2.98초 대 2.99~3.04초(잡음 안).
- 근거: VM37은 검증기가 정확히 잡는 버그인데 릴리스 사용자는 부작용이 난 뒤 실행 중에 E0801을 본다. 저장소의 `tests/fuzz.rs`(TL9)는 시험 코드를 변이하는 방식이라 VM37을 놓쳤다.
- 고치는 법(결정):
  - (가) **추천**: 릴리스에서도 늘 검증한다(VM40의 보강과 함께). 그리고 문법을 아는 무작위 프로그램 생성기와 기준 해석기를 고정 씨앗으로 `cargo test`에 넣는다(가지, `match` 가드·대안, `??`, `and`·`or`·`not`, 람다 붙잡기, `return`, 꼬리 호출, 반환 타입). 영역 점검의 Python 생성기는 4,500개에서 8번 걸렸다.
  - (나) 생성형 시험만 넣는다.
  - (다) 그대로.
- 처리: 2026-11-13 커밋 `feat: verify bytecode in release builds, test generated programs (VM41)`, 사용자 결정 (가). `compiler.rs`의 `compile`이 빌드 종류와 상관없이 `bytecode::verify`를 돌린다(걸리면 실행 전 E0801). 생성형 시험 `crates/fung-vm/src/vm/gen_tests.rs`(고정 씨앗 1,500개, `FUNG_GEN_CASES`·`FUNG_GEN_SEED`): 영역 점검의 생성기와 기준 해석기를 Rust로 옮겼다. VM37의 고침을 되돌리면 255번째 프로그램에서 실패하는 것을 확인했다. 측정(릴리스, macOS arm64): 표준 모듈 여섯을 import하는 프로그램 200번이 3.07초 → 3.15초, 지역 10만 개 함수의 `check`가 0.38초 → 0.38초. 매뉴얼 `vm/08-bytecode.md`, 설계 09. VM40(검증기 구멍)은 하지 않았다: 릴리스에서도 도는 지금은 검증기 자신이 패닉하지 않는 것이 더 중요해졌다.

### VM42. 이웃 명령에 기대는 명령 둘 — 정리, 낮음
- 재현(코드 읽기와 dump): 기본값 서두는 `LOADLOCAL i; JUMPIFSET L`이다. 이 때문에 `LOADLOCAL`이 "빈 칸도 올린다"는 둘째 뜻을 갖고(`vm.rs:1113`), 검증기가 두 곳에서 이 쌍을 특수 처리한다(`verify.rs:309`, `:353`). `TAILCHECK`는 다음 명령의 피연산자를 읽어 피호출자 칸을 찾는다(`call.rs:280`).
- 고치는 법: `JUMPIFSET L, slot`으로 칸을 직접 본다(11-11 VM21의 (나)안, 기본값 하나당 명령 하나가 준다). `TAILCHECK`의 "자기 클로저면 건너뛰기"를 뺀다(컴파일러가 이미 내지 않는다).
- 처리: 고침(2026-11-14). `JUMPIFSET L, slot`: 둘째 피연산자가 매개변수 칸이고 스택을 건드리지 않는다(기본값 하나당 명령 하나가 줆). `LOADLOCAL`의 "빈 칸도 올린다"는 뜻과 검증기의 두 특수 처리가 사라졌다(검증기는 `JUMPIFSET`의 칸이 기본값 있는 매개변수인지 본다). `TAILCHECK`는 다음 명령을 엿보지 않는다. dump 주석 없음(둘째 피연산자가 칸 번호).

### VM43. 인자 4,095개 한도는 `CALL` 피연산자 묶음에서만 나온다 — 정리, 낮음 (기존 결정 재검토: 11-11 VM32)
- 재현: 인자·매개변수 4,096개는 E0230. 펼침 인자 10만 개, 원소 10만 개 리스트 리터럴, 지역 10만 개는 정상이다(다른 한도는 모두 16,777,215).
- 원인 위치: `bytecode/op.rs:312~326`(`encode_call`·`decode_call`·`MAX_ARGS`), `compiler/fn_.rs`.
- 고치는 법: `CALL`류는 위치 인자 수와 이름 인자 수를 두 피연산자로 나눈다. `MAX_ARGS`와 묶음 함수가 사라지고 개수 한도가 하나가 된다.
- 처리: 고침(2026-11-14). `CALL`·`TAILCALL`·`CALLSPREAD`·`TAILCALLSPREAD`의 둘째 피연산자가 이름 인자 수, `CHECKTYPE`의 둘째가 타입 번호. `encode_call`·`decode_call`·`decode_slot_type`·`MAX_ARGS`·`Operand::Call`·`Operand::SlotType` 삭제. 인자 10만 개의 호출과 매개변수 10만 개의 함수가 돈다. 함께 찾은 것: `resolve/call.rs` `Sig::check`의 이름 인자 검사가 세제곱 시간이었다(표를 한 번 만들게 고침).

### VM44. 반환 타입 있는 함수끼리의 꼬리 호출마다 `TAILCHECK`가 약 12 ns 든다 — 성능, 낮음
- 재현(영역 점검의 측정, 2,000만 번): 서로 부르는 `a`·`b`가 타입 없음 0.90초, 둘 다 `-> int` 1.15초(+27%). 자기 꼬리 호출은 차이 없음.
- 원인 위치: `vm/call.rs` `tail_check`(278행), `vm/frame.rs` `add_check`(123행). 호출마다 `Check`를 만들고 선형으로 찾아 지우고 다시 넣는다.
- 고치는 법: 만들기 전에 (원형 포인터, 타입)으로 먼저 찾고, 맨 뒤에 있으면 아무것도 하지 않는다.
- 처리: 고침(2026-11-14). `Frame::has_check`: 프레임이 (원형, 타입)이 같은 검사를 이미 들었으면 `Check`를 만들지 않는다. 측정은 하지 않았다(영역 점검의 +27% 수치를 다시 재지 않음).

### VM45. `assert`의 쪼갠 비교는 비교 연산자의 둘째 구현이다 — 버그, 낮음 (다시 확인)
- 재현: `x = parse_int("1") < 1`은 E0501이 연산자를 가리키고 도움말(`unwrap it with ! or ? first`)이 붙는다. `assert parse_int("1") < 1`은 같은 E0501이 `assert`(1:1)를 가리키고 도움말이 없다.
- 원인 위치: `vm/m6.rs:50` `compare_by`(연산자 이름을 문자열로 받아 따로 계산), `compiler/m6.rs:53` `assert_stmt`.
- 고치는 법: `Assert`에 연산자의 위치와 열거형을 담고, 실행 루프의 비교와 같은 도우미를 부른다.
- 처리: 고침(2026-11-14). `bytecode::Assert`가 연산자 열거형 `Cmp`와 그 위치를 담고(`cmp: Option<(Cmp, Pos)>`), `vm/m6.rs` `compare_by`가 실행 루프와 같은 `vm.rs` `ordered`(도움말 포함)·`value::eq`·`index::contains`를 부른다. 시험 `an_asserts_comparison_is_the_operators`.

### VM46. 코드 생성과 주석의 군더더기 — 정리, 낮음
1. `if c: return e ;;` 문장의 `RETURN`·`TAILCALL` 뒤에 닿지 않는 `POP`이 남는다(`compiler.rs` `if_stmt` 585행).
2. `Compiler::has_ret`의 주석 "A return type annotation disables tail calls"는 D44 뒤로 틀렸다. 필드를 없애고 `return_type >= 0`으로 묻는다.
3. `"in"`만 `binary_op` 표 밖에서 따로 낸다(`compiler.rs:533`).
4. `moves.rs` 머리 주석이 `Fwd::expr`의 `Head::Fn` 동작과 다르다.
- 처리: 고침(2026-11-14). 1 `if_stmt`가 프레임을 끝내는 본문 뒤에 `POP`을 내지 않는다. 2 `has_ret` 필드 삭제(`return_type >= 0`). 3 `"in"`을 `binary_op` 표에 넣음. 4 `moves.rs` 머리 주석과 `Fwd::expr`의 `Head::Fn`(꼬리의 함수 값은 놓지 않음, 그 밖은 CLOSURE 뒤에 놓음)을 다시 읽어 지금은 같음을 확인(고칠 것 없음).

### VM47. VM 장의 설명이 구현과 다른 곳 — 문서, 낮음 (1~2 다시 확인)
1. `manual/vm/10-values-and-memory.md` 42행의 `Str(Rc<String>)`: 실제는 `Rc<Text>`(바이트 열).
2. 같은 쪽 203행 "남은 n²": `acc + s`는 11-11 VA32 뒤 상각 O(1)이고 `[x] + acc`도 O(1)이다. 지금 남은 n²는 보간 누적과 문자열 앞에 붙이기다(VA46). `for.md` 19행 "피할 것은 문자열을 `acc + s`로 쌓는 것"과 `strings.md`의 같은 문장도 옛말이다.
3. "조각이 바탕의 1/4보다 작으면 복사해서 떼어 낸다"(`10-values-and-memory.md`, `internals.md:236·266`): 11-11 VA33 뒤로 큰 조각은 트리를 잘라 낸다.
4. `08-bytecode.md` 305~307행: `Task::enter`는 없고, `stack`·`frames`는 `Fiber`의 필드다. 실행 루프 의사 코드의 `f.proto.code`는 `f.closure.code`다. `internals.md` 141행의 프레임 필드도 실제(`closure, ip, base, floor, tail_calls, entry, extra`)와 다르다.
5. 검증기 설명의 "모든 길이 `RETURN`·`TAILCALL`·`NOMATCH`로 끝난다"에 `TAILCALLSPREAD`가 빠졌다. (VM40과 함께 고침)
6. 재현 안 함(코드 읽기): 매뉴얼과 `vm/world.rs:311`은 "런타임이 끝날 때 전역 표를 비워 고리를 끊는다"고 하는데, `value.rs`의 `Closure.globals` 주석과 open-questions "알려진 한계"(VM36)는 "고리라 해제되지 않는다"고 한다. 어느 쪽이 맞는지 확인해 하나로 맞춘다.
- 처리: 고침(2026-11-14). 1 `Rc<Text>`. 2 "남은 n²"를 문자열 앞에 붙이기로 고치고 `for.md`·`strings.md`의 옛 문장도 고침. 3 자르기 규칙(1/4보다 작으면 떼어 냄: 32개 이하는 복사, 더 크면 트리 자르기). 4 `Fiber`의 `stack`·`frames`, `f.closure.code`, 프레임 필드, 없는 `Task::enter`·`encode_call` 삭제. 5 VM40에서 고침. 6 확인: 매뉴얼과 `world.rs`가 맞다. 모든 전역 표는 `World::new_globals`가 쥐고 `release_values`가 비우므로 고리는 런타임이 끝날 때 해제된다(시험 `the_globals_ring_is_freed_with_the_runtime`). `value.rs`의 주석을 고치고 open-questions "알려진 한계"의 해당 항목을 지웠다.

## 7. 값과 prelude (VA)

### VA41. `int / int`가 2^53 넘는 정수를 말없이 깎아 틀린 몫을 낸다 — 버그, 중간 (다시 확인)
- 재현: `print(9007199254740993 / 3, 9007199254740993 // 3, 9007199254740993 / 1)` → `3002399751580330.5 3002399751580331 9007199254740992.0`.
- 근거: `operators.md` "정수가 실수로 정확히 옮겨지지 않으면 말없이 깎지 않고 에러 E0523"(D48). `+ - * // % **`는 지키고 `/`만 피연산자를 조용히 깎는다.
- 원인 위치: `value/arith.rs` `div` 255~258행(`x as f64`, `exact_float`를 거치지 않음).
- 고치는 법: `/`의 정수 피연산자도 `exact_float`를 거친다(E0523). 규칙이 "실수 산술에 들어가는 정수는 정확해야 한다" 하나로 남는다.
- 처리: 2026-11-13 커밋 `fix: four silent mistakes (FE63, VA41, VA43, VA47)`. `value/arith.rs` `div`와 상수 접기 `compiler/fold.rs` `ints`의 `/`가 정수 피연산자를 `exact_float`·`exact`로 옮긴다(E0523). 명세 `operators.md`. 시험 `tests/lang/err_div_inexact.fung`·`err_div_inexact_const.fung`. VA42(기간 × 실수)는 하지 않았다(E0523의 문구가 정수 전용).

### VA42. `dur * float`가 104일 넘는 기간을 말없이 깎는다 — 버그, 낮음 (다시 확인)
- 재현: `x = 2600h + 0.000001ms; print(x * 1.0 == x, x * 1 == x)` → `false true`.
- 원인 위치: `value/dur.rs` `scale_dur` 28행(`d as f64 * f`), `div_dur` 44행.
- 고치는 법: VA41과 같이, 기간이 f64로 정확하지 않으면 E0523.
- 처리: 고침(2026-11-13). `value/dur.rs` `scale_dur`·`div_dur`가 `exact_float`를 거친다(E0523). 정수 곱·나눗셈은 그대로 정확.

### VA43. `==`의 "같은 객체" 지름길이 함수 검사를 건너뛴다 — 버그, 중간 (다시 확인)
- 재현:
  ```
  f = x => x
  a = [f]
  b = a
  print(a == b, a in [b])     → true true
  print([f] == [f])           → E0501 cannot compare fn and fn
  ```
- 근거: `values.md` "함수는 같음이 없다"(D62). 같은 내용의 두 리스트가 `Rc`를 공유하는지에 따라 답이 갈린다. 공유 여부는 언어에서 보이지 않아야 하는 구현 사정이다.
- 원인 위치: `value/compare.rs` `walk` 35~37행의 `!same_object(a, b)`.
- 고치는 법: 지름길을 "함수를 품지 않은 값"에만 쓴다. 영속 벡터 노드의 기억(`Probe`)에 "함수를 품음"을 더하면 큰 리스트도 O(1)로 답한다.
- 처리: 2026-11-13 커밋 `fix: four silent mistakes (FE63, VA41, VA43, VA47)`. `Probe::Fn`(함수를 품었는가, 영속 벡터 노드 기억의 셋째 2비트)과 `Value::holds_fn`을 더하고, `value/compare.rs` `walk`의 같은 객체 지름길을 엄격한 비교에서는 함수를 품지 않은 값에만 쓴다(패턴의 `equal`은 그대로). 매뉴얼 `vm/internals.md`. 시험 `vm/tests.rs` `closures_print_and_compare`(작은 값, 5,000개 리스트, 맵·튜플·태그 안).

### VA44. 공유 구조 둘의 `==`·`<`·해시가 지수 시간이고 기한이 닿지 않는다 — 버그, 낮음 (다시 확인)
- 재현:
  ```
  import time
  x = fold(0..26, [1], (acc, _i) => [acc, acc])
  y = fold(0..26, [1], (acc, _i) => [acc, acc])
  t0 = time.monotonic()
  r = timeout(() => x == y, 100ms)
  print(r, time.monotonic() - t0)       → Ok(true) 800ms    (기한 100 ms가 무시됨, 두 겹마다 4배)
  ```
- 근거: 값은 노드 수십 개인데 연산이 2^26 경로를 걷는다. 출력은 11-11 VA28에서 한도로 막았고 `<`는 자기 자신만 막았다(VA36). 한 번에 도는 연산이라 취소·기한·다른 태스크가 모두 멈춘다.
- 원인 위치: `value/compare.rs` `walk`(28행)·`compare`(252행)·`hash_value`(374행).
- 고치는 법: 둘 다 공유된 그릇 쌍의 주소를 집합에 넣어 다 걸은 쌍을 건너뛴다. 해시는 공유 노드의 부분 해시를 기억한다. 최소한 셋을 `Work` 조각으로 나눠 기한이 닿게 한다.
- 처리: 고침(2026-11-14). `value/compare.rs`: `walk`(`==`)와 `compare`(`<`)가 둘 다 공유된(`Rc` 참조 둘 이상) 컬렉션 쌍을 기억해 다시 걷지 않는다(`Walked`; 작은 비교가 값을 치르지 않게 64걸음 뒤부터 기억). `hash_value`는 세 겹 이상 깊은 컬렉션을 자기 해시로 따로 계산해 공유된 것의 해시를 주소로 기억하고, 얕은 값(흔한 레코드)은 전과 같은 길로 간다. 측정(릴리스, 30만 개): 26겹 DAG의 `==` 814 → 0.006 ms, `<` 1,196 → 0.004 ms, `unique` 5,516 → 0.02 ms; 공유 없는 튜플의 `==` 16 → 15 ms, `unique` 58~61 → 60 ms. `Work` 조각으로 나누지는 않았다(선형이 되어 큰 값의 다른 연산과 같아짐).

### VA45. 값의 표기가 `$이름`을 이스케이프하지 않아 소스로 되읽히지 않는다 — 버그, 낮음 (다시 확인, E0129 뒤 회귀)
- 재현: `print(["a\$b", "\${a}"])` → `["a$b", "\${a}"]`. 찍힌 `"a$b"`를 소스에 붙이면 E0129다. `dbg`, 에러 문구, `dump`의 상수 주석도 같은 함수를 쓴다.
- 원인 위치: `crates/fung-front/src/consts.rs` `write_quoted` 225행(`${`만 이스케이프). 같은 함수가 `\x80`은 소문자, `\u{1B}`는 대문자로 쓴다(227행).
- 고치는 법: `$`를 늘 `\$`로 쓴다(SP59-2와 같은 선). `\u{…}`의 16진 숫자를 소문자로 맞춘다.
- 처리: 고침(2026-11-13). `consts.rs` `write_quoted`가 `{`나 이름의 첫 글자(`_` 포함) 앞의 `$`를 `\$`로 쓴다(소스의 규칙과 같은 조건: SP59-2를 바꾸지 않기로 해서 `"$5"`·끝의 `$`는 소스에서도 글자이므로 그대로 둔다). `\u{…}`는 소문자. 기대 출력 다시 씀.

### VA46. 보간으로 쌓는 문자열은 O(n²)이다 — 성능, 중간 (다시 확인)
- 재현(10만 번, `fold(0..n, "", …)`): `acc + "line ${i};"` 14 ms, `"${acc}line ${i};"` 918 ms. 영역 점검의 측정으로 40만 번은 0.06초 대 23초다.
- 근거: 11-11 VA32는 `+`만 제자리 덧붙이기로 고쳤다. 보간은 `+`의 타입 에러 도움말이 권하는 방법인데 수백 배 느리다. 같은 일을 하는 두 방법의 비용이 다르다.
- 원인 위치: `vm.rs` `concat` 802~866행(늘 새 문자열에 전부 복사).
- 고치는 법: 첫 조각이 홀로 쥔 문자열이면 거기에 나머지를 덧붙인다(바이트코드는 이미 첫 조각을 `MOVELOCAL`로 넘긴다).
- 처리: 고침(2026-11-13). `vm.rs` `concat`이 첫 조각이 문자열이면 그 값을 쥐고 나머지를 `value::add_text`로 덧붙인다(홀로 쥐었으면 제자리). 릴리스 시험 `perf.rs` `interpolation_builds_text_in_linear_time`.

### VA47. fung으로 쓴 prelude 함수는 문장으로 버려도 E0234가 아니다 — 버그, 중간 (다시 확인)
- 재현:
  ```
  len([1])                    → E0234
  take([1], 1)                → 조용함
  zip([1], [2])               → 조용함
  filter([1], x => x > 0)     → 조용함
  ```
  `first`·`last`·`drop`·`contains`·`is_empty`·`is_ok`·`is_err`·`pad_left`·`pad_right`도 조용하다.
- 근거: `blocks.md` "효과가 없다고 알려진 내장 함수의 호출도 E0234다". 걸리는지가 구현 언어(Rust인가 fung인가)에 달려 있다.
- 원인 위치: `vm/atom.rs`의 `PURE` 표에는 네이티브만 있다. `resolve/call.rs`의 `Sig.pure`는 네이티브의 별칭일 때만 참이다.
- 고치는 법: `extras/prelude.fung`의 함수를 받지 않는 11개에 순수 표시를 한다(resolve가 "본문이 순수 호출뿐인 최상위 `fn`"을 추론하거나, 명시 목록을 두고 spec 시험이 지킨다). fung으로 쓴 모듈 함수(`cli.parse`, `fs.read_lines`)도 같다.
- 함께 정할 것(결정): 함수를 받는 `map`·`filter`·`sort_by`·`group_by`·`find`·`any`·`all`·`fold`·`try_map`을 문장으로 쓰는 것은 지금 일부러 예외다. 부수효과 순회는 `each`와 문장 `for`가 맡으므로 (가) **추천**: 이들도 E0234. (나) 그대로.
- 처리: 2026-11-13 커밋 `fix: four silent mistakes (FE63, VA41, VA43, VA47)`. 명시 목록 `vm/atom.rs` `PURE_FUNG`(11개)·`CALLING_FUNG`(`exec`·`sh`·`iterate`·`until`)을 두고 `boot/spec_file.rs`가 `spec/builtins.txt`에 `pure`를 찍는다. 분류되지 않았거나 두 번 분류된 prelude 함수가 있으면 명세 생성 시험이 실패한다. 받는 끝을 읽는 `first`·`take`·`zip`도 넣었다(네이티브 `sort(rx)`·`len(rx)`가 이미 E0234인 것과 같은 선). 저장소의 골든 둘(`err_drop_count_first.fung`·`err_pad_huge.fung`)이 걸려 `_ = …`로 고쳤다. 시험 `tests/lang/err_discard_fung_builtin.fung`, `builtins.rs` `pure_functions_are_marked`. 하지 않은 것: fung으로 쓴 모듈 함수(`cli.parse`, `fs.read_lines`)의 표시, "함께 정할 것"의 결정(함수를 받는 `map`·`filter` 등). "함께 정할 것"은 결정 D90((가)): 커밋 `feat!: a builtin that computes with a function is no statement (D90)`. `vm/atom.rs` `CALLS_OR_RAISES`에서 `map`·`try_map`·`filter`·`fold`·`sort_by`·`find`·`any`·`all`·`group_by`를 뺐다(남은 것: `fail`·`each`·`scope`·`timeout`·`with`·`supervise`). E0234의 도움말이 `each`·`for`를 권한다(`resolve/value.rs` `RUNS_A_FUNCTION`). 함께 고친 것: desugar가 `test` 블록의 본문을 끝까지 버려지는 자리로 걷는다(끝의 `for`가 `map`으로 풀려 이 검사에 걸렸다). 저장소의 시험 소스 아홉 곳을 고쳤다. 명세 `blocks.md`, 설계 01, 시험 `tests/lang/err_discard_map.fung`.

### VA48. `is_ok`·`is_err`가 결과값이 아닌 값에 조용히 `false`다 — 결정, 중간 (다시 확인, 기존 결정 재검토)
- 재현: `print(is_ok(5), is_err(nil))` → `false false`.
- 근거: 결과값을 묻는 다른 자리(`5!`, `5?`, `try_map`, `conc.retry`)는 모두 E0605다. `if not is_err(r): …`가 결과값을 깜빡 벗긴 값에도 통과한다.
- 원인 위치: `extras/prelude.fung` `is_ok`·`is_err`의 `_ => false` 절.
- 고치는 법(결정): (가) **추천**: 결과값이 아니면 E0605(`nil`은 E0604). (나) 그대로.
- 처리: 결정 D104(2026-11-13): 바꾸지 않는다(D91과 같은 판단).

### VA49. `zip`이 길이가 다른 입력을 말없이 자른다 — 결정, 중간 (다시 확인)
- 재현: `zip([1, 2, 3], ["a"])` → `[(1, "a")]`. 영역 점검: 받는 끝을 넘기면 끝까지 받은 뒤 남는 값을 버린다(`zip(rx, [1, 2])` 뒤 `recv(rx)`가 닫힘).
- 근거: 슬라이스는 범위를 넘으면 E0507인 언어에서 두 리스트의 길이가 다른 것은 대개 자료가 어긋난 버그다. `take`·`first`·`find`는 받는 끝에서 필요한 만큼만 받는다.
- 원인 위치: `extras/prelude.fung` `zip`.
- 고치는 법(결정): (가) **추천**: 리스트끼리 길이가 다르면 E0403. 범위는 번호 붙이기용으로 긴 쪽이어도 된다. 받는 끝은 필요한 만큼만 받는다. (나) 자르기는 두고 받는 끝만 고친다. (다) 그대로.
- 처리: 결정 D105((나)). 커밋 `feat!: the remaining decisions of the fourth review (D93 to D107)`. `extras/prelude.fung` `_zip_side`: 받는 끝은 다른 쪽이 리스트면 그 길이만큼만 받는다. 시험 `tests/lang/zip_receiver.fung`.

### VA50. 문자열을 수로 읽는 길이 둘이다 — 결정, 낮음 (다시 확인)
- 재현: `print(int("5"), int("0x1f"), int(3.9))` → `5 31 3`. `parse_int("5")!`도 5다.
- 근거: 지침 3. `int`가 두 일(실수 버림, 글 해석)을 맡아 `int(get(row, "n"))`이 무엇을 하는지 호출 자리에서 알 수 없다.
- 원인 위치: `prelude/convert.rs` `int_fn`·`float_fn`의 문자열 갈래.
- 고치는 법(결정): (가) **추천**: `int`·`float`는 수 사이의 변환만(문자열은 E0513, 도움말 `parse_int(s)!`). (나) 그대로.
- 처리: 결정 D106(2026-11-13): 바꾸지 않는다.

### VA51. `get(m, k, d)`와 `get(m, k) ?? d`가 겹치고 답이 갈린다 — 결정, 낮음 (다시 확인)
- 재현: `m = {a: nil}; print(get(m, "a", 0), get(m, "a") ?? 0)` → `nil 0`.
- 고치는 법(결정): (가) **추천**: `default` 인자를 없앤다(기본값은 `??`). SP55와 함께 정한다. (나) 그대로.
- 처리: 결정 D106(2026-11-13): 바꾸지 않는다. 두 답이 다르다는 것이 둘 다 두는 이유다.

### VA52. `remove(m, k)`가 없는 키를 말없이 받는다 — 결정, 낮음 (다시 확인)
- 재현: `remove({a: 1}, "zz")` → `{a: 1}`.
- 근거: `m["zz"]`는 E0508이다. 오타 키(`remove(cfg, "pasword")`)가 조용히 지나간다. 반대로 멱등 지우기는 편하다.
- 고치는 법(결정): (가) 없는 키면 E0508(지침 1대로). (나) 그대로. 지침대로면 (가)지만 편의 손실이 있어 사용자 판단이 필요하다.
- 처리: 결정 D106(2026-11-13): 바꾸지 않는다.

### VA53. 인자 자리의 `nil`이 "표준 입력"·"공백"을 뜻한다 — 결정, 낮음 (일부 다시 확인)
- 재현: `trim(" a ", nil)` → `a`. `read_line(nil)`은 표준 입력에서 읽는다. `join(xs, nil)`·`pad_left(s, n, nil)`은 E0403이다.
- 근거: 실패한 조회에서 온 `nil`(`read_line(get(handles, name))`, `trim(s, get(cfg, "strip"))`)이 에러가 아니라 다른 동작이 된다. 또 한 줄 읽기는 prelude(`read_line(file)`), 한 줄 쓰기는 `fs.write_line(file, line)`에 있고, `read_lines`는 prelude(핸들)와 `fs`(경로)에서 인자가 다르다.
- 원인 위치: `prelude/io.rs` `read_line_fn`·`read_lines_fn`, `prelude/strings.rs` `trim_ends`, `extras/fs.fung` `read_lines`.
- 고치는 법(결정): (가) **추천**: prelude의 `read_line()`·`read_lines()`는 인자 없이 표준 입력만 읽고, 핸들용은 `fs.read_line(file)`로 옮겨 `fs.write_line`과 짝을 맞춘다. `trim`류의 `nil`은 E0403. (나) 그대로.
- 처리: 결정 D106(2026-11-13): 바꾸지 않는다.

### VA54. 틀린 타입의 인자가 E0403·E0506·E0513으로 갈린다 — 정리, 낮음 (일부 다시 확인)
- 재현: `sum(5)` → E0506. `from_bytes(5)`·`close(5)`·`cancel(5)` → E0403. `int(nil)`·`sym(5)` → E0513. `sym_name(5)` → E0403.
- 근거: `functions.md` "읽는 법"의 규칙("여러 타입을 받는 함수가 받지 못하는 값이면 E0506")이 실제와 다르다. 여러 타입을 받는 `close`·`cancel`은 E0403이다.
- 원인 위치: `native/nat.rs`의 E0506 생성과 `wrong_arg`, 컬렉션 함수들의 입력 검사.
- 고치는 법: 함수 인자의 타입 오류는 E0403 하나로 한다. E0506은 연산자와 `for`에만 남기고, E0513은 "값을 바꿀 수 없음"(`int(1e300)`, `sym("a b")`)에만 쓴다.
- 처리: 고침(2026-11-14). 코드는 바꾸지 않고 `functions.md`의 규칙을 실제대로 고쳤다: 매개변수 타입은 E0403, 값을 훑거나 들여다보는 함수가 그럴 수 없는 값을 받으면 그 일을 말하는 E0506(11-10의 결정 D33이 함수마다 동사를 정함), 변환 함수가 바꿀 수 없는 것은 E0513. 셋을 E0403 하나로 합치는 것은 D33을 되돌리는 결정이라 하지 않았다.

### VA55. 부분 문자열을 찾는 함수들이 글자 경계를 다르게 본다 — 결정, 낮음 (다시 확인)
- 재현: `print("\xa9" in "é", index_of("é", "\xa9"), is_utf8(replace("é", "\xa9", "")))` → `true nil false`(유효한 글에서 깨진 글이 나온다).
- 근거: `split`·`replace`·`in`·`starts_with`는 바이트로 맞추고 `index_of`·`trim`은 글자 경계에서만 맞춘다. "`sub in s`면 `index_of(s, sub)`는 `nil`이 아니다"가 깨진다.
- 원인 위치: `prelude/strings.rs` `index_of_fn`·`trim_ends`, 반대쪽은 `value/index.rs` `contains`와 `split_fn`·`replace_fn`.
- 고치는 법(결정): (가) **추천**: 모두 글자 경계에서만 맞춘다(UTF-8이 아닌 바이트는 하나가 한 글자라 바이트 찾기는 그대로 된다). (나) 그대로 두고 문서에 적는다.
- 처리: 결정 D107. 커밋 `feat!: the remaining decisions of the fourth review (D93 to D107)`. `prelude/strings.rs` `index_of_fn`: 글자 안에서 시작하는 맞춤도 돌려준다(그 글자의 인덱스). `trim`은 손대지 않았다.

### VA56. `sum`의 빈 입력은 정수 `0`이라 기간의 합이 빈 리스트에서만 죽는다 — 결정, 낮음 (다시 확인, 기존 결정 D82 재검토)
- 재현: `print(sum([]) > 5s)` → `E0501 cannot compare int and dur`. `sum([1s, 2s]) > 5s`는 `false`.
- 고치는 법(결정): (가) **추천**: `sum`은 수만 받는다. 기간의 합은 `fold(xs, 0s, …)`. (나) `sum(xs, zero = 0)`. (다) 그대로.
- 처리: 결정 D106(2026-11-13): 바꾸지 않는다.

### VA57. `pad_left`·`pad_right`가 한도에 꼭 맞는 결과를 거절한다 — 버그, 낮음 (다시 확인)
- 재현: `len(pad_left("", 100000000))` → E0806. `len(repeat(" ", 100000000))`은 된다.
- 원인 위치: `extras/prelude.fung` `_padding`의 `repeat(pad, missing // len(pad) + 1)[..missing]`(늘 한 벌을 더 만든다).
- 고치는 법: `repeat(pad, missing // len(pad)) + pad[..missing % len(pad)]`.
- 처리: 고침(2026-11-13). `_padding`이 `repeat(pad, missing // width) + pad[..missing % width]`로 꼭 필요한 만큼만 만든다. 시험 `vm/tests.rs` `small_value_fixes`.

### VA58. prelude의 작은 어긋남 — 결정·문서, 낮음 (1 다시 확인)
1. `exec([""])`는 `Err('invalid)`인데 같은 종류의 빈 값(`exec([])`, `cwd: ""`, `env("")`)은 E0403이다. → 빈 명령 이름도 E0403(추천).
2. `collections.md`의 `map`: "인자 수가 맞지 않으면 E0407"만 적혀 있는데 `map([1, 2], () => 1)`은 E0402다.
3. `io.md`의 `read_line`: `\n` 없이 끝난 마지막 줄 `"y\r"`도 `Ok("y")`가 된다. `lines("y\r")`도 같다. 문서에 적거나 `\r\n` 쌍만 뗀다.
4. `from_bytes(65..70)`은 E0403이다(다른 컬렉션 함수는 범위를 받는다).
5. 튜플에서 되는 연산은 `len`·`is_empty`·`==`·`<`·패턴뿐이라는 한 줄이 `values.md`에 없다.
- 처리: 1 고침(2026-11-13): `native/process.rs` `exec_fn`이 빈 명령 이름을 E0403으로. 2~5 문서(2026-11-14): `collections.md`의 `map`에 E0402, `io.md`의 `read_line`에 줄바꿈 없는 마지막 줄의 `\r`(동작은 그대로 두고 적음), `strings.md`의 `from_bytes`에 범위를 받지 않음, `values.md`에 튜플에 되는 연산.

## 8. 시험했지만 문제가 없던 것

- **앞단**: `fung check`에 무작위 입력 24,000개(저장소 `.fung` 변형과 토큰 나열 반반)에서 패닉·E0801·시간 초과 0건. 괄호·블록·패턴·보간 등 14가지를 1만 겹으로 중첩해 E0115이거나 정상. 문장 10만, 절 7만, 리스트 30만 원소, 500만 글자 문자열은 선형. `grammar.md`의 모든 생성 규칙과 우선순위 표를 구현과 대조.
- **컴파일러**: 상수 접기와 실행을 무작위 식 1,500개로 대조해 불일치 0. 지역 10만 개, `if … return` 6만 개 함수 정상. 기본값·이름 인자·클로저 붙잡기·패턴·꼬리 호출 보장(깊이 20만)은 명세대로.
- **VM·값**: 정수 산술의 경계, 실수의 `-0.0`·NaN·Inf 차단, 문자열 색인(무작위 5씨앗 × 3,000걸음), 영속 벡터(40씨앗)와 맵(11씨앗)의 모델 대조, 100만 겹 값의 `==`·`str`·해제, 한도 E0804·E0806. `size_of::<Value>()`는 16바이트.
- **prelude**: 컬렉션·문자열·변환 함수 약 60개를 값 15종에 넣은 타입 행렬, 남는 인자·모르는 이름 인자의 거절, 정렬의 안정성, 비UTF-8 문자열, 동시성 API의 틀린 인자와 반환 모양.
