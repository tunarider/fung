# 언어 명세·바이트코드 VM 점검 결과와 작업 목록 (2026-11-14)

네 번째 점검(`docs/plans/2026-11-13-full-review.md`, 58개 모두 처리)을 고친 뒤, 언어 명세와 바이트코드 VM 구현을 다시 점검한 결과다.
기준은 사용자가 준 세 지침이다.

1. **엄격함이 우선이다**(매우 중요): 말없이 틀린 값, 말없이 받아 주는 실수, 느슨한 인자 처리는 결함이다.
2. **예외 조항을 줄인다**(중요): "…인 경우만 빼고"가 붙는 규칙은 결함이다.
3. **같은 일을 하는 방법은 하나다**(존중, 실질적인 편의를 위해 조금 희생할 수 있음).

하위 호환, 비용, 난이도는 문제를 덮는 이유가 되지 않는다. 큰 구조 변경도 선택지에 넣었다.
형식은 지난 네 점검과 같다. 새 세션이 이 문서만 읽고 항목 하나(또는 한 묶음)를 이어서 고칠 수 있게 썼다.
항목을 처리하면 `처리:` 줄에 커밋과 바꾼 문서를 적는다.

**결정 (2026-11-14)**: 결정 항목을 사용자와 하나씩 검토해 D108~D135로 모두 정했다(`docs/plans/2026-11-14-full-review-fixes.md`).
검토 중에 확정한 지침(CLAUDE.md "언어 검토·추천 지침")으로 다시 따져, 이 문서의 추천 가운데 열넷이 뒤집혔다. **각 항목의 "고치는 법(결정)"이 아니라 결정 문서의 표가 기준이다.**

**상태 (2026-11-14)**: 41개 항목 모두 처리했다(항목마다 `처리:` 줄, 요약은 `2026-11-14-full-review-fixes.md`의 상태).

## 0. 이 문서를 쓰는 법

- **기준 커밋**: `ce3c1cd5`(`fix: the verifier writes no path for a deep nest of functions; a 30 minute CI timeout`). 줄 번호는 이 커밋 기준이고 함수 이름을 함께 적었다.
- **범위**: 언어 명세(`manual/reference/language/`), 앞단(렉서·파서·desugar·resolve·로더·진단), 컴파일러와 바이트코드(수명 분석·검증기 포함), VM 실행 핵심과 값 층, 언어의 일부인 prelude 표면.
  동시성 런타임의 내부, 표준 모듈(`fs`·`http`·`time` 등), CLI 도구는 범위가 아니다. 걸쳐 있는 것만 적었다.
- **방법**: 명세 15쪽은 직접 읽고 의심 가는 규칙을 릴리스 바이너리로 재현했다. 네 영역(앞단, 컴파일러·바이트코드, VM 코어·값, prelude)은 따로 점검한 뒤 주장을 종합 단계에서 다시 돌렸다.
  - **"(다시 확인)"** 표시는 종합 단계에서 한 번 더 돌려 같은 결과를 본 항목이다. 표시가 없는 항목은 영역 점검의 재현 한 번뿐이다.
  - "재현 안 함(코드 읽기)"이라고 적은 것은 실행으로 확인하지 못했다.
  - 모든 실행은 macOS arm64다. 리눅스는 돌리지 않았다.
  - "사본에서 확인"이라고 적은 고침은 영역 점검이 저장소 밖 사본에 적용해 본 것이다. 저장소에는 아무것도 고치지 않았다.
- **뺀 것**: 지난 네 점검의 항목(모두 처리), 결정 D1~D107, open-questions "남은 일"·"알려진 한계"·보류 목록(평평한 연산자 사슬 300만 항의 Rust 스택 넘침 포함).
  이미 정한 결정을 새 근거로 다시 꺼낸 것은 제목에 "(기존 결정 재검토)"라고 적고 새 근거를 적었다.
  설계 원칙 "짧은 축약 문법을 좋아한다"(`background.md`)에 속하는 것(`then … else`와 블록 `if`, 람다와 익명 `fn`, `{name}` 축약, `|>`)은 "방법이 둘"로 올리지 않았다.
- **재현**: `cargo build --release --locked` 뒤 저장소 루트에서 `F=target/release/fung`. 여러 줄 소스는 `x.fung`로 저장해 `$F run x.fung`.
- **ID**: 지난 점검에 이어 매긴다: SP67~, FE77~, VM48~, VA59~. SP 명세(언어 규칙의 결정) · FE 앞단과 진단 · VM 컴파일러와 바이트코드 · VA 값과 prelude.
- **분류**: 버그(명세가 정답을 말함) · 결정(언어 동작이 바뀜: 선택지 + 추천으로 사용자에게 물음) · 성능 · 정리 · 문서.
- **심각도**: 높음(정상적인 쓰임에서 프로그램이 죽거나 틀린 값이 조용히 나옴) · 중간 · 낮음.
- **고칠 때의 공통 절차**: CLAUDE.md 그대로다(TDD, 커밋 전 검사 전체, CHANGELOG, 매뉴얼 다시 만들기, 기대 출력 bless 뒤 diff 검토).

## 1. 총평

**높음이 둘이다. 둘 다 "구현의 속사정이 보이는 동작을 바꾸는" 버그다.**

- VM48: 매개변수 기본값 안에서 묶은 이름(`match`·`if` 블록의 지역 이름, 숨은 임시 칸)이 **뒤 매개변수의 칸을 덮어쓴다**. `f((1, 2), c: 9)`가 `c`를 2로 받는다. 준 인자가 말없이 바뀐다.
- VA59: 큰 리스트의 슬라이스가 원본 벡터 전체를 붙잡는 "보기"가 되어, 잘려 나간 원소가 살아 있다. 같은 함수가 원소 10개에서는 채널을 닫고 100개에서는 교착(E0702)한다.

둘 다 지난 점검의 생성형 시험(VM41)이 만들지 않는 모양이다(이름을 묶는 기본값, 크기에 따른 값 표현의 갈림과 수명). 이번 점검의 무작위 프로그램 12만 개에서 그 밖의 틀린 값·E0801은 없었다. **수명 분석 자체는 이제 단단하다.** 남은 구멍은 분석 바깥(기본값의 칸 배정, 값 표현)에 있다.

그 밖은 네 갈래다.

**첫째, 검사가 "바로 그 모양"에만 걸린다(지침 1·2).** 지난 점검들에서 넣은 엄격한 검사 여럿이 가장 단순한 모양만 본다.
- E0234(버려지는 효과 없는 식): `len(xs) == 2`처럼 순수 호출이 **든** 식은 통과한다. `assert`를 빠뜨린 시험이 조용히 통과한다(FE77).
- D99(리터럴로 드러난 타입 오류): 스칼라 리터럴만 본다. `not 1s`, `[1](2)`, `-"a"`는 통과한다(FE81). 타입 표기와 어긋나는 리터럴(`fn f() -> int = "x"`)도 통과한다(SP76).
- E0235(돌 수 없는 절): 모두 맞는 절과 맨 리터럴만 본다. `Ok(_)` 뒤의 `Ok(1)`은 통과한다(SP69).
- E0131(`return` 뒤 문장): 직접 쓴 `return`만 본다. 모든 가지가 `return`하는 `if` 뒤의 문장은 통과하고, 그 죽은 코드가 E0239를 가린다(SP72).
- 호출의 인자 개수(D43): `fn f`는 컴파일 때 보는데 `f = (a, b) => …`는 실행할 때 본다(SP74). 내장 함수에 넘긴 콜백의 개수는 처음 불릴 때에야 본다: 빈 리스트면 영영 안 걸리고, `supervise`의 `on_fail`은 실패가 났을 때 진짜 에러를 가린다(SP75).
- `sort`·`min`·`max`의 사전 검사(D87): "첫 원소와 견준다"가 추이적이지 않아, 원소 순서에 따라 되다 말다 한다(VA60).

**둘째, 같은 실수가 자리에 따라 다르게 대접받는다(지침 2).**
- `x in c`에 타입이 다른 값을 찾으면 리스트·범위는 `false`, 문자열은 E0501, 맵은 E0514다(SP67).
- 함수가 든 값의 `==`가 다른 원소의 값과 맵의 넣은 순서에 따라 `false`와 E0501로 갈린다(SP81).
- "보이는 이름은 다시 묶을 수 없다"는 한 규칙에 번호가 여섯이다(SP68).
- 빈 찾을 글 `""`를 `in`·`starts_with`·`ends_with`는 받고 `index_of`·`split`·`replace`는 거절한다(VA66).
- 맵 패턴은 늘 열려 있고(`{}`가 모든 맵에 맞음) 리스트 패턴은 `...`를 써야 열린다(SP70).

**셋째, 말없이 잃거나 틀리는 값(지침 1).**
- `read_lines`가 시간 초과로 끊기면 이미 읽은 줄을 버린다(VA61). 매뉴얼은 "바이트를 잃지 않는다"고 적었다.
- `index_of`가 128 KiB 조각 경계에 걸친 글자 안의 맞음을 놓쳐 `nil`을 준다(VA62). `in`은 찾는다.
- `True`·`False`·`None`은 조용히 태그다. D91 뒤로 `x == None`은 에러 없이 늘 `false`다(SP73).

**넷째, 문자열의 성능 절벽.** 리스트는 머리·꼬리 재귀가 선형인데(L5) 문자열은 `s[1..]` 재귀가 이차다(20만 글자 1.0 s, 리스트 0.04 s; VA69). 한글이 한 글자라도 든 문자열을 뒤에서부터 인덱싱하면 10만 글자에 2.5 s다(ASCII 0.01 s; VA68).

**단단한 곳**: 앞단은 무작위 입력 14,000개와 20만 겹 중첩에서 패닉이 없었다. 상수 접기는 실행과 15,246쌍에서 일치했다. 수명 분석은 기준 해석기와 6.6만 개, 놓는 시점의 탐침과 6만 개에서 SP72의 죽은 코드 말고는 어긋남이 없었다. 값의 표기는 116개 값이 소스로 되읽혔다(SP77의 `sym("Ok")`만 빼고). prelude 전 함수의 타입 행렬·경계·복잡도는 아래 항목 말고는 명세대로다.

## 2. 추천 작업 순서

1. **높음 둘**: VM48(기본값의 칸) → VA59(슬라이스 보기). 둘 다 결정이 필요 없다. VM48은 생성형 시험에 "이름을 묶는 기본값"을, VA59는 "크기 경계를 넘는 리스트와 놓는 시점"을 먼저 더하면 실패하는 시험이 된다.
2. **결정 없이 고칠 버그**: FE77(E0234의 구멍) · VA60(정렬 사전 검사) · VA61(`read_lines`) · VA62(`index_of`) · VA63 · VA64 · VM49 · FE78~FE82.
3. **엄격함을 넓히는 결정 묶음**(사용자 결정 필요): SP73 → SP75 → SP74 → SP72 → SP69 → SP76. 모두 "이미 있는 검사를 같은 뜻의 다른 모양으로 넓힌다"는 한 방향이다.
4. **일관성 결정 묶음**: SP67 · SP81 · SP80 · VA66 · VA65 · VA67 · SP70 · SP71 · SP68 · SP77 · SP78 · SP79.
5. **성능**: VA69(구조 변경: 문자열 보기) → VA68 → VA70 → VM51 → VM52.
6. **정리·문서**: VM50 · VM53 · VM54 · VA71.

## 3. 목록

| ID | 한 줄 | 분류 | 심각도 |
|---|---|---|---|
| SP67 | `x in c`에서 타입이 다른 값을 찾는 일이 그릇마다 다르다(`false`·E0501·E0514) | 결정 | 낮음 |
| SP68 | "보이는 이름은 다시 묶을 수 없다"는 한 규칙에 에러 번호가 여섯이다 | 결정 | 낮음 |
| SP69 | E0235·E0241이 모두 맞는 절과 맨 리터럴만 안다 | 결정 | 낮음 |
| SP70 | 맵 패턴은 늘 열려 있고 리스트 패턴은 `...`로 연다 | 결정 | 낮음 |
| SP71 | 맨 `nil` 문장의 예외가 적힌 까닭보다 넓다 | 결정 | 낮음 |
| SP72 | 모든 길이 `return`하는 문장 뒤의 문장을 받는다 | 결정 | 낮음 |
| SP73 | `True`·`False`·`None`·`Nil`·`Null`이 조용히 태그다 | 결정 | 중간 |
| SP74 | 한 번 묶인 함수 값의 호출은 컴파일 때 검사하지 않는다 | 결정 | 중간 |
| SP75 | 내장 함수에 넘긴 콜백의 인자 개수는 처음 불릴 때에야 본다 | 결정 | 중간 |
| SP76 | 타입 표기와 어긋나는 리터럴이 `check`를 통과한다 | 결정 | 낮음 |
| SP77 | `sym("Ok")`·`sym("Err")`가 쓸 수 없는 값을 만든다 | 결정 | 낮음 |
| SP78 | 표기가 둘인 자리 | 결정 | 낮음 |
| SP79 | 맵 키 자리의 식별자만 NFC로 맞추지 않는다 | 결정 | 낮음 |
| SP80 | 끝을 포함하는 빈 슬라이스가 시작 0에서만 안 된다 | 결정 | 낮음 |
| SP81 | 함수가 든 값의 `==`가 다른 원소의 값과 맵의 넣은 순서에 따라 갈린다 | 결정 | 중간 |
| FE77 | 순수 호출이 든 버려지는 식이 E0234를 비켜 간다 | 버그 | 중간 |
| FE78 | 괄호 안의 줄바꿈이 원소 사이가 아닌 자리에서도 공백이다 | 버그 | 낮음 |
| FE79 | 맵 패턴 `{_}`가 말없이 키 `"_"`다 | 버그 | 낮음 |
| FE80 | import된 파일의 최상위에 이름을 묶지 않는 구조 분해 대입이 온다 | 버그 | 낮음 |
| FE81 | 리터럴로 드러난 타입 오류의 검사가 스칼라 리터럴에서 멈춘다 | 버그 | 낮음 |
| FE82 | 진단의 작은 것들 | 정리(진단) | 낮음 |
| VM48 | 기본값 안에서 묶은 이름이 뒤 매개변수를 덮어쓴다 | 버그 | 높음 |
| VM49 | 반환 타입 있는 함수의 꼬리 `and`·`or`가 E0505 대신 E0404를 낸다 | 버그 | 낮음 |
| VM50 | 가드 붙은 `\|` 패턴만 대안 256개 한도가 있고 코드가 곱으로 는다 | 결정·정리 | 낮음 |
| VM51 | 컴파일 시간의 이차 구간 넷 | 성능 | 낮음 |
| VM52 | `and`·`or`의 꼬리 호출마다 `TAILCHECKBOOL`이 약 11 ns 든다 | 성능 | 낮음 |
| VM53 | 수명 분석과 검증기의 군더더기 | 정리 | 낮음 |
| VM54 | VM 장과 설계 09가 `CALL`의 옛 모양을 적는다 | 문서 | 낮음 |
| VA59 | 큰 리스트의 슬라이스가 잘려 나간 원소를 붙잡는다 | 버그 | 높음 |
| VA60 | `sort`·`sort_by`·`min`·`max`가 원소 순서에 따라 되다 말다 한다 | 버그 | 중간 |
| VA61 | 시간 초과로 끊긴 `read_lines`가 읽은 줄을 잃는다 | 버그 | 중간 |
| VA62 | `index_of`가 조각 경계에 걸친 글자 안의 맞음을 놓친다 | 버그 | 낮음 |
| VA63 | `iterate`·`until`은 문장으로 버려도 E0234가 아니다 | 버그 | 낮음 |
| VA64 | `parse_float`가 정수 범위를 넘는 정수 꼴을 거절한다 | 버그 | 낮음 |
| VA65 | `zip`의 "필요한 만큼만"에 범위 예외가 있고 왼쪽의 넓은 범위는 E0503이다 | 결정·버그 | 낮음 |
| VA66 | 빈 찾을 글을 받는 함수와 거절하는 함수가 갈린다 | 결정 | 낮음 |
| VA67 | `dur / dur`만 2^53 넘는 몫을 말없이 깎는다 | 결정 | 낮음 |
| VA68 | 비ASCII 문자열의 `s[i]`가 앞으로 가는 순회가 아니면 O(i)다 | 성능 | 중간 |
| VA69 | 슬라이스로 문자열을 소비하는 재귀가 이차 시간이다 | 성능 | 중간 |
| VA70 | `index_of`가 같은 찾기의 `in`보다 150배 느리다 | 성능 | 낮음 |
| VA71 | prelude의 작은 어긋남 | 결정·문서 | 낮음 |

## 4. 명세 (SP)

### SP67. `x in c`에서 타입이 다른 값을 찾는 일이 그릇마다 다르다 — 결정, 낮음 (다시 확인, D91에 닿음)
- 재현:
  ```
  print("1" in [1, 2])    → false
  print("a" in 0..3)      → false
  print(1 in "abc")       → E0501 cannot search str and int
  print(1 in {a: 1})      → E0514 cannot use a value of type int as a map key
  ```
- 근거: `operators.md`는 리스트와 범위(`false`), 맵 키(E0514, `values.md`)를 적고 문자열은 적지 않았다. D91의 까닭은 "리스트는 여러 타입을 담는다"였다. 범위·문자열·맵은 원소의 타입이 정해져 있는데(정수·문자열·문자열 키) 범위만 `false`다.
- 원인: `crates/fung-vm/src/value/index.rs:299-325` `contains`.
- 고치는 법(결정):
  - (가) **추천**: 원소 타입이 정해진 그릇(문자열·맵·범위)에 다른 타입을 찾으면 에러다. 범위에 수가 아닌 값은 E0501. `false`는 리스트만의 규칙이고 `==`에서 따라 나온다. `operators.md`의 "범위에 수가 아닌 값을 찾아도 `false`다"를 지우고 문자열의 규칙을 적는다.
  - (나) 넷 다 `false`(D91을 `in` 전체로). 맵의 E0514 "어느 자리에서든"에 예외가 생긴다.
  - (다) 그대로 두고 문자열의 규칙만 적는다.
- 처리: 2026-11-14, D115(바꾸지 않음: (다)에 가깝다). `in`은 그릇마다 하는 일이 달라서 각각 기존 규칙을 따른다: 리스트·범위는 `==`로 원소 찾기(타입이 다르면 `false`), 문자열은 부분 문자열 연산(E0501), 맵은 키 조회(E0514). 점검 문서의 (가)는 범위의 `false`를 에러로 바꾸려 했으나 범위의 `in`은 `==`로 견주는 리스트와 같은 규칙이고 에러로 막으면 "원소 타입이 정해진 그릇"이라는 예외 목록이 생긴다. 고친 것은 `operators.md`의 `in` 설명뿐이다.

### SP68. "보이는 이름은 다시 묶을 수 없다"는 한 규칙에 에러 번호가 여섯이다 — 결정, 낮음 (다시 확인)
- 재현:
  ```
  x = 1; x = 2                          → E0238
  fn f() = 1; f = 2                     → E0238
  fn f() = 1; fn f() = 2                → E0210   (블록 안의 지역 fn 둘도 E0210)
  import json (parse); parse = 1        → E0238
  import json (parse); fn parse() = 1   → E0210
  for … : total = total + 1 ;;          → E0205
  fn g(): y = 1; f = y => y …           → E0240
  import json; json = 1                 → E0308
  import json; import json as j         → E0309
  ```
- 근거: `scope.md` 2번·6번 규칙이 스스로 "2번 규칙과 뜻은 하나다: 보이는 이름에는 값을 다시 붙일 수 없다"고 적는다. 그런데 번호는 묶는 방법(대입·`fn`·import)과 이름의 출신(같은 블록·바깥·모듈)의 곱으로 갈린다. 같은 실수 `parse`를 다시 묶는 것이 `=`로는 E0238, `fn`으로는 E0210이다. D88은 E0205·E0209를 합치기로 했다가 E0205를 남겼다.
- 원인: `crates/fung-front/src/resolve.rs` `bind_new`·`fn_node`, `resolve/names.rs`.
- 고치는 법(결정):
  - (가) **추천**: 번호 하나(E0238 "cannot bind `x` again")로 합치고, 갈래는 노트와 도움말이 말한다("bound on line N", "a prelude function", "module `json`", 반복문 안이면 "누적은 `fold`"). E0205·E0210·E0240·E0308은 은퇴한다. E0309(같은 모듈을 두 번 import)는 이름이 아니라 모듈의 규칙이라 남긴다. `scope.md`의 2번·6번 규칙이 하나가 된다.
  - (나) 두 갈래만: 같은 블록(E0238)과 가리기(E0240). E0205·E0210·E0308을 접는다.
  - (다) 그대로.
- 처리: 2026-11-14, D122((가)). 커밋 `feat!: rebinding a visible name is one error, E0238 (D122)`. `resolve/names.rs`: `binds_fresh`가 바깥 이름을 E0238 하나로 보고하고 노트가 출신을 말한다(`Outer::note`: "is bound outside this block, on line N"·"is a top-level name bound on line N"·"is a function defined on line N"·"is imported on line N"·"is the module imported on line N"·"is the name of the function it is in"·"is a prelude function"). 도움말: 바깥 변수에 `=`로 대입하면 옛 E0205의 것(식이나 `fold`), 그 밖은 옛 E0240의 것. 같은 블록의 다시 묶기는 `bound_again` 하나가 맡는다(`bind_new`·지역 `fn`·최상위 `fn`·import 이름): 노트가 첫 묶기가 무엇인지(처음 묶은 줄, import, 함수, 모듈), `fn`을 두 번 정의하면 도움말 "fung has no overloading: give each function a name of its own". 모듈 이름은 `module_again`(옛 `shadow_error`)이 노트와 `import … as m` 도움말을 붙인다. 최상위 `fn` 이름도 블록의 `fns`에 적어 같은 판정을 쓴다. 카탈로그의 E0205·E0210·E0240·E0308은 "not raised" 주석과 함께 남기고(E0209·E0517처럼), E0309는 그대로. `scope.md`의 2번과 6번 규칙을 한 규칙(2번 "보이는 이름은 다시 묶을 수 없다")으로 합쳐 일곱이 여섯이 됨(다른 페이지는 번호로 가리키지 않아 고칠 참조가 없었다). `errors.md`: E0238에 옛 네 번호의 예와 고치는 법을 모으고 네 번호는 은퇴 문구. 그 밖에 매뉴얼 25쪽의 번호, `03-functions-and-patterns.md`·`04-modules.md`·`08-tooling-and-errors.md`·`open-questions.md`. 골든 `tests/lang/err_*` 21개는 이름이 번호와 묶여 있지 않아 이름은 그대로 두고 `# expect-error:`만 E0238로. 시험: `resolve/tests.rs`의 `binding_a_visible_name_again_is_one_error`(갈래마다 노트·도움말), 옛 번호를 단언하던 사례는 E0238로(`not_hiding`은 prelude 노트로 거름), 노트 문구 단언 둘(`small_name_diagnostics_of_the_third_review`).

### SP69. E0235·E0241이 모두 맞는 절과 맨 리터럴만 안다 — 결정, 낮음 (다시 확인)
- 재현(모두 통과하고 뒤 절은 결코 돌지 않는다):
  ```
  [] => 1;  [] => 2
  Ok(_) => 1;  Ok(1) => 2
  (_, _) => 1;  (1, 2) => 2
  {a: 1} => "one";  {a: 1, b: 2} => "two"      (f({a: 1, b: 2})는 "one")
  [...] => 1;  [] => 2
  1 | 2 => …;  2 | 3 => …                      (둘째 절의 2는 죽은 대안)
  ```
- 근거: `match.md`는 E0235를 두 모양(모두 맞는 패턴, 앞 절과 같은 리터럴)으로 하나씩 적는다. 까닭("그 뒤의 절은 결코 돌지 않으므로")은 위 재현에도 그대로다.
- 원인: `crates/fung-front/src/resolve/pattern.rs` `repeated_literals`·`literal_key`·`dead_alternatives`.
- 고치는 법(결정):
  - (가) **추천**: 구조적 포섭. `_`·이름은 무엇이든 덮고, 튜플·리스트·태그·맵은 모양을 맞춘 뒤 안쪽으로 내려가고, 리터럴은 값으로 견준다. 규칙은 "가드 없는 앞 절(이나 앞 대안)이 덮는 절·대안은 에러" 하나가 된다. `match.md`의 두 조항이 한 문장이 된다.
  - (나) 그대로.
- 처리: 2026-11-14, D112((가)). 커밋 `feat!: an arm or alternative an earlier one covers is E0235 or E0241 (D112)`. `resolve/pattern.rs`: 새 `covers(a, b)`(`_`·이름은 무엇이든, 리터럴은 `literal_key`로, 튜플·태그는 개수(와 이름)가 같을 때 칸마다, 리스트는 닫힌 것이 같은 길이·연 것이 앞 칸을 덮는 같거나 긴 것, 맵은 D114의 규칙: 닫힌 것은 같은 키의 닫힌 것, 연 것은 그 키를 가진 것, 값마다; `a | b`는 하나가, `c | d`는 모두). `repeated_literals`를 `covered_arms`로 바꿈: guard 없는 앞 절이 덮는 절은 E0235(앞 절의 줄을 적음, 리터럴끼리는 옛 문구), 모든 값에 맞는 앞 절이면 전처럼 그 절에서 한 번, 절의 대안 일부만 덮이면 첫 덮인 대안이 E0241. `dead_alternatives`는 글자 비교 대신 `covers`(같은 것을 다시 쓴 대안은 옛 문구). guard 붙은 절의 대안은 대안마다 guard를 보므로 서로 덮지 않는다("guard가 있으면 덮지 않는다"의 같은 규칙). 같은 줄의 `1 => y => …`가 문장 자리에서 E0235가 될 때 `unreachable_after`가 람다 괄호 도움말을 붙인다(앞 절이 빈 절이고 같은 줄). 저장소에서 고친 것: `tests/lang/rest_wildcard.fung`의 `[a, ...] | [a]`(앞 대안이 덮음 → `[a] | [a, _, ...]`), fung-vm `match_tests`의 `1 | 1.0`(같은 값 → `1 | 1.5`), 생성형 시험 `gen_tests.rs`의 생성기(앞의 guard 없는 절이 덮을 절은 guard 붙은 이름 절로 바꿈; 그대로면 1500개 가운데 699개가 컴파일되지 않아 기준 70%를 밑돌았다). 옛 시험의 `1 => 1; 1 | 2 => 2`는 이제 E0241. 매뉴얼 `match.md`(두 조항을 "덮는다" 한 규칙과 목록으로, 경계 사례 펜스 둘), `errors.md`(E0235·E0241), 설계 `01-syntax.md`. 시험: `resolve/tests.rs`의 `an_arm_covered_by_an_earlier_one_never_runs`(덮는 것 열여덟, 덮지 않는 것 열넷, 위치·줄, 한 `|` 안, 문장 자리의 `1 => y => …`).

### SP70. 맵 패턴은 늘 열려 있고 리스트 패턴은 `...`로 연다 — 결정, 낮음 (다시 확인, 기존 결정 재검토: 11-10 "맵 패턴의 부분 일치"는 문제없음으로 닫음)
- 재현:
  ```
  fn f(x) = match x:
    {} => "empty"
    _  => 0
  ;;
  print(f({a: 1}), f({}))     → empty empty
  ```
  리스트의 `[] => "empty"`는 빈 리스트에만 맞는다.
- 새 근거: 리스트 패턴은 개수가 정확해야 하고 나머지를 버리려면 `...`를 쓴다(D93이 그 표기를 하나로 줄였다). 맵 패턴은 표기 없이 늘 "다른 키는 무시"다. 같은 모양의 두 패턴이 반대로 읽히고, `{}`는 빈 맵처럼 보이지만 모든 맵에 맞는다. SP69를 (가)로 고치면 `{a: 1}` 뒤의 `{a: 1, b: 2}`가 죽은 절이라는 것도 이 차이에서 나온다.
- 원인: `crates/fung-front/src/parser/pattern.rs` `pmap`, `compiler/pattern.rs`.
- 고치는 법(결정):
  - (가) **추천**: 맵 패턴도 `...`로 연다. `{name: n}`은 키가 정확히 `name` 하나인 맵, `{name: n, ...}`은 나머지를 무시, `{...}`는 모든 맵. 리스트와 한 규칙이 된다. 표준 `Err` 내용의 `Err({kind: 'not_found})`는 `Err({kind: 'not_found, ...})`로 써야 한다(저장소·매뉴얼 전체 변경). 닫힌 패턴은 모르는 키를 조용히 넘기지 않는다는 엄격함도 얻는다.
  - (나) `{}`만 컴파일 에러(모든 맵은 `m when type_of(m) == 'map`이나 타입 표기로). 예외 조항이 하나 는다.
  - (다) 그대로 두고 `match.md`에 "`{}`는 모든 맵"을 적는다.
- 판단의 근거: (가)는 편의를 꽤 깎는다(부분 일치가 맵 패턴의 흔한 쓰임). 지침 1·2는 (가), 지침 3의 "실질적인 편의"는 (다) 쪽이다.
- 처리: 2026-11-14, D114((가)). 커밋 `feat!: a map pattern matches exactly the keys written unless opened with ... (D114)`. `parser/pattern.rs`: 리스트 패턴의 `...`·`...rest` 읽기를 `prest`로 빼서 맵 패턴(`pmap`)도 맨 끝에 한 번 받는다(`(pmap (pentry …) (prest rest))`, `..._`는 E0112, 뒤에 항목이 오면 E0110). 패턴 상수(`bytecode::Pattern`)는 리스트와 같은 칸 `rest`를 쓴다: `None`이면 키가 정확히 적은 것, `Wild`면 나머지 무시, `Bind`면 나머지를 맵으로. VM(`vm/matcher.rs`)은 닫힌 패턴에서 길이를 먼저 견주고(키는 E0113으로 서로 달라 길이가 같으면 정확히 그 키들), 나머지 맵은 원래 맵의 사본에서 적은 키를 지워 넣은 순서를 지킨다(`{...rest}`는 맵 그대로). 검증기는 `rest`가 리스트·맵에만, `_`나 이름만 오는지 본다. 역어셈블은 `{a: $0, ...$1}`. 이름 묶기·E0239·E0216·포매터는 리스트의 `...rest`와 같은 길이라 고칠 것이 없었다. 저장소의 맵 패턴: `extras/http.fung` 15곳(`Err({kind: 'limit, ...})` 등), `examples/staticfiles.fung` 2곳, `tests/lang` 8곳, `crates/fung-cli/tests/data` 1곳, 매뉴얼 펜스 30곳과 본문 몇 곳, 설계 문서 02·03·06. 정확한 맵이라 그대로 둔 것도 있다(`{x, y} = {x: 1, y: 2}` 등). `match.md`·`grammar.md`(`map_item`)·`08-patterns.md`·`internals.md`·`03-functions-and-patterns.md`. 시험: 파서 `a_map_pattern_opens_with_dots`, VM `map_patterns_are_exact_unless_opened`(큰 맵의 나머지 순서 포함), 바이트코드 표시·검증기 사례, 포매터 사례, 골든 `map_pattern_open.fung`.

### SP71. 맨 `nil` 문장의 예외가 적힌 까닭보다 넓다 — 결정, 낮음 (다시 확인)
- 재현(모두 통과):
  ```
  x = 1
  nil
  print(x)

  if x == 1: nil ;;
  for _ in [1]: nil ;;
  fn f(): nil; print(1) ;;        (nil 뒤에 문장이 더 있다)
  ```
- 근거: `blocks.md` "예외는 맨 `nil` 하나다: 문장으로 쓴 `match`에서 할 일이 없는 절은 `_ => nil`로 쓰는데, 절에는 본문이 있어야 하므로". 까닭은 `match` 절인데 면제는 어디서나다.
- 원인: `crates/fung-front/src/resolve/value.rs:224` `pointless`가 `Head::Nil`을 모든 자리에서 뺀다.
- 고치는 법(결정):
  - (가) **추천**: 본문 전체가 `nil` 하나인 `match` 절만 면제한다. `if c: … else: nil ;;`의 문장 `if`는 `else`를 빼면 된다.
  - (나) 절의 본문을 비울 수 있게 한다(`_ =>`). 예외가 사라지지만 문법이 바뀌고, 빠뜨린 본문과 구별되지 않는다.
  - (다) 그대로 두고 명세를 "어디서나"로 고친다.
- 처리: 2026-11-14, D121((나)). 커밋 `feat!: a match arm may be empty and a bare nil statement is E0234 (D121)`. `parser/pattern.rs` `match_arms`가 본문이 빈 절을 받는다(`(arm (pwild) (nil) (block))`; E0112 "the arm's body"를 지움). 절 판정은 그대로라 `1 => y => y + 1`은 빈 절 `1`과 절 `y`다. `resolve/value.rs`: `pointless`의 `Head::Nil` 면제를 지우고, `else` 없는 `if`에 파서가 넣는 `nil`은 문장으로 보지 않는다. 값 자리의 빈 절은 빈 블록의 E0224("is empty")이고, 바로 같은 줄에 이름 절이 이어지면 FE25의 람다 도움말을 붙인다. 버려지는 `nil`의 E0234는 "`nil`"이라 적고 도움말이 빈 절·`else` 빼기를 알린다. 포매터(`format/layout.rs` `end_line`): 줄 끝의 절 화살표 `=>`는 열린 연산자가 아니라 다음 줄이 이어짐 줄이 아니다(다음 절은 절의 들여쓰기, 본문은 전처럼 한 단 깊이). 저장소: `extras/http.fung` 5곳(`nil => nil`·`_ => nil` → 빈 절), `tests/lang/effect_statements.fung`·`discard.fung`, 매뉴얼 `unreachable-servers.md`(빈 절)·`concurrency.md`(`for _ in stop: ;;`), Rust 시험 하나(`compile_literals_and_concat`의 `nil` 문장). `match.md`·`blocks.md`·`grammar.md`(`arm`)·`errors.md`(E0224·E0234)·`01-syntax.md`. 시험: 파서 `an_arm_may_have_an_empty_body`, resolve E0234 표(맨 `nil`·`else: nil`·`for` 본문·빈 절)·`values_used_must_not_be_nil`(값 자리 빈 절)·`an_empty_arm_before_a_name_arm_hints_at_a_lambda`·`a_discarded_nil_points_at_an_empty_branch`, 포매터 `an_empty_arm`, VM `an_empty_arm_does_nothing`, 골든 `err_effect_free_nil.fung`.
- 덧붙임(2026-11-14, D121의 남은 부분): 버려지는 자리의 `then … else`는 문장 `if`처럼 두 가지를 버린다. 커밋 `feat!: a discarded then … else discards its branches (D121)`. `resolve/value.rs`의 `check`가 `Then`의 두 가지를 노드의 자리(버려짐·씀·REPL의 값)로 본다(전에는 피연산자라 늘 씀). 그래서 `c then print(1) else nil` 문장은 `nil`이 E0234, `c then 1 else f()`는 `1`이 E0234, 가지의 `else` 없는 `if`는 버려지는 자리라 된다. 저장소에서 고친 것: `tests/lang/err_discard_for.fung`(`x == 2 then fails("two") else Ok(x)` → `if x == 2: fails("two") ;;`, 실행 때의 E0606을 그대로 봄). 매뉴얼 `blocks.md`(버려지는 자리 목록). 시험: `resolve/tests.rs` E0234 표에 다섯, `values_used_must_not_be_nil`의 "가지는 늘 씀" 사례를 새 규칙으로.

### SP72. 모든 길이 `return`하는 문장 뒤의 문장을 받는다 — 결정, 낮음 (다시 확인)
- 재현:
  ```
  fn f(c):
    x = c then 1 else 2
    if c: return 1 else: return 2 ;;
    print("never", x)
  ;;
  ```
  `fung check` 통과. 죽은 `print` 줄을 지우면 `E0239 unused variable x`가 나온다: 죽은 줄이 다른 에러를 가린다. 모든 절이 `return`하는 `match` 뒤도 같다.
- 근거: `functions.md` "같은 블록에서 `return` 문장 뒤의 문장은 결코 실행되지 않으므로 E0131이다". 까닭은 같은데 직접 쓴 `return`만 잡는다. 수명에도 닿는다: 죽은 코드에서만 읽히는 값은 프레임이 끝날 때까지 놓이지 않는다(`scope.md` "값의 수명"과 어긋남; 이번 점검의 놓는 시점 퍼징에서 나온 유일한 어긋남).
- 원인: E0131 판정(`crates/fung-front/src/resolve`)이 `Head::Return` 문장만 본다. 컴파일러는 이미 안다(`Proto::ends_closed`).
- 고치는 법(결정):
  - (가) **추천**: 끝까지 갈 수 없는 문장(모든 가지가 `return`으로 끝나는 `if`·`match`·`then … else`) 뒤의 문장은 E0131. 규칙이 "직접 쓴 `return`만"에서 "돌아오지 않는 문장"으로 하나가 된다. `fail(…)`·`exit(…)` 호출은 넣지 않는다(함수 값이라 정적으로 정해지지 않는다).
  - (나) 그대로 두고 문서에 적는다.
- 처리: 2026-11-14, D111((가)). 커밋 `feat!: a statement after one that cannot complete is E0131 (D111)`. E0131은 파서가 판정한다(`parser/stmt.rs` `after_return`, 블록과 `match` 절의 본문): 새 `never_completes`가 괄호 없는 `return`, `else` 있는 `if`(else-if 사슬 포함)·`match`(모든 절)·`then … else`의 모든 가지가 그런 문장으로 끝나는지 본다. 호출은 무엇을 부르든 들지 않는다(`fail`·`exit`). 파서 에러라 죽은 문장은 이름 해석(E0239의 읽기)·수명 분석에 닿지 않는다: 재현의 죽은 줄을 지우면 `E0239 unused variable x`가 나온다(시험). 카탈로그 문구("comes after `return`")는 모든 길이 `return`이라 그대로 둠. 저장소·매뉴얼에서 새로 걸린 코드는 없었다. 매뉴얼 `functions.md`·`errors.md`(E0131, `if`의 예)·`grammar.md`(주석), 설계 `01-syntax.md`·`03-functions-and-patterns.md`. 시험: 파서 `nothing_follows_a_statement_that_cannot_complete`(가지가 끝나는 모양 여섯, 끝날 수 있는 모양은 통과: `else` 없는 `if`, 빈 절, `fail` 가지), fung-vm `a_dead_statement_is_the_one_error`.

### SP73. `True`·`False`·`None`·`Nil`·`Null`이 조용히 태그다 — 결정, 중간 (다시 확인)
- 재현:
  ```
  x = True
  print(x == true, type_of(x))          → false 'sym
  match v: True => 1; False => 0 ;;     → v가 true면 실행 중 E0601
  get(m, "b") == None                   → 늘 false, 에러 없음
  fn f(a: None) = a                     → 통과
  ```
- 근거: D101이 타입 표기의 같은 실수(`Bool`·`Int`·`Nil`)를 E0212로 막았다. D91 뒤로 타입이 다른 `==`는 에러 없이 `false`라, 다른 언어의 습관(Python `True`·`None`)이 어디서도 드러나지 않는다.
- 원인: 렉서·resolve 어디도 태그의 이름을 보지 않는다.
- 고치는 법(결정):
  - (가) **추천**: 대소문자만 다른 `true`·`false`·`nil`과 `None`·`Null`은 식·패턴·타입 표기 어디서나 컴파일 에러(도움말 "write `true`"). D101과 한 규칙이 된다("내장 이름과 대소문자만 다른 태그는 에러").
  - (나) 그대로.
- 처리: 2026-11-14, D108(바꾸지 않음). D101 재검토(D134): D101을 되돌림. 커밋 `feat!: an uppercase name in a type annotation is always a tag name (D134)`. `resolve/alias.rs`의 `expand`에서 내장 타입과 대소문자만 다른 대문자 이름을 E0212로 막던 갈래와 도움말을 지움: 대문자 이름은 예외 없이 태그 이름이다(`fn f(a: Int)`는 태그 `Int`의 값과 심볼 `Int`를 받는다). E0212는 소문자의 모르는 이름에만 남고 도움말은 붙이지 않는다. `types.md`의 "내장 타입 이름은 소문자다 … E0212" 조항, `errors.md`의 E0212 흔한 원인을 고치고 `07-operators-and-types.md`에 기록. 시험: `resolve/tests.rs`의 E0212·도움말 단언을 "에러 없음"으로 바꿈(`one_import_a_test_with_a_body_and_uppercase_tags`), fung-vm `uppercase_annotations_are_tag_names`(`type token = Int | Str`, `fn show(t: Int | Str)`에 `Int(3)`·`Str("a")`).

### SP74. 한 번 묶인 함수 값의 호출은 컴파일 때 검사하지 않는다 — 결정, 중간 (다시 확인)
- 재현(모두 "ran"을 찍은 뒤 실행 중에 죽는다):
  ```
  f = (a, b) => a + b;  print("ran"); print(f(1))       → 실행 중 E0407
  fn g(a) = a;  f = g;  print("ran"); print(f(1, 2))    → 실행 중 E0402
  x = len;  print("ran"); print(x(1, 2))                → 실행 중 E0402
  ```
  최상위의 `f = fs.read; f()`만은 컴파일 때 잡힌다. 순수 함수 표도 같다: 최상위 `j = path.join; j("a", "b")` 문장은 E0234인데 함수 안의 같은 두 줄이나 `s = sort; s([2, 1])`은 통과한다. 한 매개변수 함수에 `g(1, 2, ...xs)`도 개수가 이미 넘는데 실행 때로 미룬다.
- 근거: `functions.md`는 "변수에 든 함수 값"을 실행 때로 남긴다. 이 문장은 D88 전에 쓰였다. 지금은 모든 이름이 한 번만 묶이므로 `f = x => …`는 지역 `fn`(FE65에서 검사하게 됨)만큼 정적이다. "`fn f`는 검사하고 `f = fn`은 안 한다"는 예외만 남았다.
- 원인: `crates/fung-front/src/resolve/call.rs` `note_aliases`(약 190-212행)가 최상위 `이름 = 모듈.멤버`만 적고, `known_callee`(약 262행)는 그 밖을 모른다.
- 고치는 법(결정):
  - (가) **추천**: 단 하나의 묶기가 람다·익명 `fn`·알려진 함수 이름·모듈 멤버·그 부분 적용인 이름은 어느 블록에서든 그 서명과 순수 표시를 가진다. 바로 부르는 함수 리터럴 `(x => x)(1, 2)`도 같다.
  - (나) 최상위 별칭의 경우를 지워 "`fn` 정의와 import만"으로 맞춘다.
  - (다) 그대로.
- 처리: 2026-11-14, D110((가)). 커밋 `feat!: calls through a name bound to a function are checked at compile time (D110)`. `resolve/call.rs`: `function_value`가 값이 소스에 드러난 함수인지 본다(람다·익명 `fn`은 `Sig::of`와 이름 "the function", 부분 적용은 람다의 서명에 부르는 함수의 `pure`, 그 밖은 `known_callee`). `let_`이 대입마다 `note_bound_functions`로 묶은 이름(이름 하나, 또는 같은 모양의 튜플·리스트 리터럴을 구조 분해한 자리)의 서명을 블록의 `sigs`에, 최상위면 `global_sigs`에도 적는다(REPL 최상위는 빼고). 최상위 `fn`이 먼저 해석되므로 `note_aliases`를 `note_functions`로 넓혀 최상위 대입을 미리 훑는다(최상위 `fn`, import, 앞서 적은 상수, prelude, 모듈 멤버). `call`이 `function_value`로 대상을 찾으므로 바로 부르는 함수 리터럴도 검사된다. 펼친 인자 앞의 위치 인자만으로 넘치면 E0402(카탈로그의 셋째 칸을 `%s`로: ``got at least N``). 순수 호출 표(`value::PureCall`)는 보이는 이름(`s`)과 함수 자신의 이름(`sort`, 도움말)을 따로 둔다. 모듈이 다른 이름으로 내보낸 함수는 `Sig::name`에 자신의 이름을 남겨 에러가 실행 때와 같다. 매뉴얼 `functions.md`(검사 범위 문장, 경계 사례 펜스)·`blocks.md`(E0234)·`errors.md`(E0402, E04xx 머리말)·`vm/09-closures.md`(실행 때 잡히는 예를 매개변수로), 설계 `03-functions-and-patterns.md`·`01-syntax.md`. 시험: `resolve/tests.rs`의 `names_bound_to_a_function_are_checked`, fung-vm `calls_of_known_functions_are_the_runs_errors`(컴파일 때의 문구가 매개변수로 넘긴 같은 호출의 실행 때 문구와 같음)·`module_members_bound_to_names_are_checked`. 옛 시험이 런타임 에러를 보려고 쓰던 `g = f`는 매개변수로 넘기게 고침(`calling_convention_messages`·`natives_share_the_calling_convention`·`hidden_parameters_are_called_by_place`).

### SP75. 내장 함수에 넘긴 콜백의 인자 개수는 처음 불릴 때에야 본다 — 결정, 중간 (다시 확인)
- 재현 1(진짜 실패가 가려진다):
  ```
  r = supervise(fn(s):
    _ = spawn(s, () => fail("child failed"))
    sleep(10ms)
    "body done"
  ;;, () => print("handler"))
  ```
  → `E0402 the function takes 0 argument(s), got 1`(`supervise(`를 가리킴). 자식의 `child failed`는 어디에도 나오지 않는다. 실패가 없는 동안은 틀린 핸들러가 드러나지 않는다.
- 재현 2(조용히 통과):
  ```
  print(map([], () => 1), fold([], 0, () => 1), timeout(x => x, 0s))
  → [] 0 Err({kind: 'timeout, …})
  ```
  `map([], 5)`는 빈 리스트여도 E0403이다. `f`의 타입은 넘길 때 보는데 개수는 안 본다.
- 근거: 지침 1. D43이 대상이 정해진 호출의 개수를 컴파일 때 본다. 내장 함수에 넘긴 람다는 한 걸음 떨어진 같은 실수이고, 지금은 자료(빈 리스트, 실패 없음, 0번 반복)에 따라 나타난다.
- 원인: `crates/fung-vm/src/native/nat.rs:335` `need_fn`이 "함수인가"만 본다.
- 고치는 법(결정):
  - (가) **추천**: `need_fn(name, v, n)`이 위치 인자 `n`개를 받을 수 있는지(필수 ≤ n ≤ 전체, 또는 나머지 매개변수)도 보고, 내장 함수의 호출 자리에서 매개변수 이름과 함께 E0402·E0407을 낸다. 대상: `map`·`filter`·`each`·`find`·`any`·`all`·`sort_by`·`group_by`·`try_map`(1), `fold`(2), `swap`(1), `timeout`(0), `scope`·`supervise`의 본문(1), `on_fail`(1), `with`(0·1·1), `spawn`(0), `iterate`·`until`. 인자가 람다 리터럴이면 컴파일 때도 잡는다(SP74와 같은 표).
  - (나) 실패 길에서만 도는 둘(`supervise`의 `on_fail`, `with`의 `release`)만.
  - (다) 그대로.
- 처리: 2026-11-14, D109(바꾸지 않음: 인자 개수는 부를 때 본다)의 버그만 고침. 커밋 `fix: a failing on_fail names the failure it was handling (D109)`. `vm/scope.rs`: `on_fail`을 부를 때 넘긴 실패 값을 기억하고(자식 처리자 `Handling::failure`, 여는 쪽 `Scope::failure`), `on_fail`이 실패하면 그 에러에 ``note: `on_fail` failed while handling error[코드]: 메시지 (at 파일:줄)``을 붙인다(`name_handled`). 재현 1은 이제 E0402 아래에 자식의 `child failed`를 보인다. `with`의 `release`는 이미 `use`의 에러에 `note: also failed in release`로 붙었고(부를 수 없는 `release`도 같음), 시험으로 묶어 둠. `concurrency.md`(`supervise`)·`program.md`(`with`), 시험 `an_on_fail_error_names_the_failure_it_was_handling`·`a_use_error_keeps_the_release_error_as_a_note`.

### SP76. 타입 표기와 어긋나는 리터럴이 `check`를 통과한다 — 결정, 낮음 (다시 확인)
- 재현(모두 `fung check` 통과):
  ```
  fn f() -> int = "x"
  fn f(a: int = "x") = a
  fn g(a: int) = a;  fn h() = g("x")
  fn f() -> int: return nil ;;
  ```
- 근거: D99가 리터럴로 드러난 연산자 오류를, D43이 호출의 개수·이름을 컴파일 에러로 만들었다. 같은 부류다. `types.md`는 "맞지 않으면 런타임 에러(E0403, E0404)"라고만 적는다.
- 고치는 법(결정):
  - (가) **추천**: 값이 리터럴이면(기본값, 돌려주는 리터럴, 대상이 정해진 호출의 리터럴 인자) 컴파일 때 같은 번호 E0403·E0404. FE81의 "문법으로 드러난 종류" 함수 하나를 함께 쓴다.
  - (나) 기본값과 반환만.
  - (다) 그대로.
- 처리: 2026-11-14, D113((가)). 커밋 `feat!: a literal that a type annotation refuses is a compile error (D113)`. 새 `resolve/annot.rs`: 리터럴의 타입은 `value::known_type`, 표기는 펼친 이름들(`TypeSet::matches`와 같은 판정: `any`, `num`, 태그 이름은 그 태그의 값과 같은 이름의 심볼). 기본값은 `function`에서 `check_types` 뒤에(E0403), 반환은 본문의 끝·그 끝의 `if`(블록, `else` 있는 것)·`match`·`then … else` 가지 끝·`return`의 값(안쪽 함수는 빼고)(E0404), 호출은 D110의 표로 대상이 정해지면 `Sig`의 매개변수 타입(`SigParam::types`, `Resolver::sig_of`가 별칭을 펼쳐 담음; 모듈이 내보내는 서명도 그 모듈에서 펼친 것)으로 위치 인자(펼침 앞)와 이름 인자를 매개변수 순서로 보고 첫째 하나(E0403, 개수 에러가 먼저면 그것만). 문구는 실행 때와 같다(유니온은 ` | `로). 매뉴얼 `types.md`(검사 시점, 경계 사례 펜스)·`errors.md`(읽는 법의 예를 이름에 든 값으로, E0403·E0404), 설계 `07-operators-and-types.md`. 시험: `resolve/tests.rs`의 `literals_against_annotations_are_checked`, fung-vm `literal_annotation_errors_are_the_runs`(여덟 꼴 × 열 리터럴: 컴파일 때의 머리 줄이 `id(…)`로 넘긴 실행 때의 것과 같고, 받는 것은 둘 다 통과).

### SP77. `sym("Ok")`·`sym("Err")`가 쓸 수 없는 값을 만든다 — 결정, 낮음 (다시 확인, D103의 남은 부분)
- 재현:
  ```
  print(sym("Ok"), [sym("Err")])   → Ok [Err]      ([Err]를 소스에 쓰면 E0236)
  r = sym("Ok")
  print(r ?? 1, is_ok(r))          → Ok false
  ```
- 근거: `lexical.md` "맨 `Ok`·`Err`는 결과값의 이름이라 심볼로 쓸 수 없다". 이 값은 표기가 소스로 되읽히지 않는 유일한 값이다(116개 값의 되읽기 시험에서 이것만 실패). 예외 조항도 낳는다: `types.md` "결과값의 이름 `Ok`·`Err`만은 값 하나를 담은 태그만 받는다(`sym("Ok")`를 돌려주면 E0404)"와 `crates/fung-vm/src/value/types.rs:109`의 특례.
- 고치는 법(결정): **추천** `sym("Ok")`·`sym("Err")`는 `sym("a b")`처럼 E0513. `TypeSet::matches`의 특례와 `types.md`·`results.md`의 그 문장을 지운다. D103의 (가)(인자 없는 태그와 심볼의 분리)를 다시 꺼내는 것은 아니다.
- 처리: 2026-11-14, D123(추천과 다름: 막지 않고 푼다). 커밋 `feat!: a bare Ok or Err is a symbol like any other tag name (D123)`. 11-10(D2의 타입 표기 예외)·11-11(D41의 맨 `Ok`·`Err` E0236)의 "맨 `Ok`·`Err` 금지"를 되돌림. `desugar.rs`의 `Head::BareTag` E0236 갈래와 `BARE_RESULT_HELP`, `parser/pattern.rs`의 맨 `Ok`·`Err` 패턴 E0236, `value/types.rs` `TypeSet::matches`의 `Ok`·`Err` 심볼 특례를 지움. E0236은 값을 담은 `Ok`·`Err`의 개수(`Ok(1, 2)`, `x |> Ok(2)`)에만 남는다. `Ok()`는 E0117, `x |> (Ok)`는 E0401, `'Ok`는 E0128, `Ok ?? 1`은 E0243으로 `Red`와 같다. 런타임은 이미 이름이 아니라 태그 값으로 결과값을 판정해(`result_kind`) 바뀐 것이 없다: 심볼 `Ok`의 `!`·`?`는 E0605, `is_ok`는 `false`, 보간은 됨. `lexical.md`·`types.md`·`results.md`·`operators.md`·`match.md`·`grammar.md`·`convert.md`(`type_of`)·`errors.md`(E0128·E0236), `01-syntax.md`·`02-values-and-types.md`·`03-functions-and-patterns.md`. 시험: desugar `a_bare_result_tag_is_its_symbol`(전 `a_bare_result_tag_is_an_error`), 파서의 괄호 안 줄바꿈 사례 `[Ok⏎ (v)]`는 `[Red⏎ (1)]`처럼 E0112, `types.rs` `result_annotations_take_the_tag_and_its_symbol`, fung-vm `bare_result_tags_are_symbols`, 골든 `err_bare_ok`·`err_bare_err_pattern`·`err_result_type_symbol`을 지우고 `result_symbols.fung`을 더함.

### SP78. 표기가 둘인 자리 — 결정, 낮음 (1·4·5 다시 확인)
1. `x |> f()`는 `x |> f`와 같고, `x |> Ok()`는 `Tag()`가 허락되는 유일한 자리다(E0117에 예외 문장). 추천: 파이프 오른쪽의 빈 괄호 호출·태그는 에러.
   처리(1): 2026-11-14, D124(바꾸지 않음). `x |> f()`·`x |> Ok()`는 파이프 규칙의 평범한 경우다(파이프가 값을 채운다). `errors.md`의 E0117 설명만 "값을 담지 않은 태그의 빈 괄호"로 고쳐 썼다.
2. `import "./a.fung"`은 `import "./a"`와 같다(`modules.md` ".fung 생략"). 추천: 확장자를 쓰면 에러.
3. `import fs as fs`를 받는다. 추천: 모듈 이름과 같은 별칭은 에러.
   처리(3): 2026-11-14, D126(바꾸지 않음). 쓸 까닭은 없지만 해가 없는 코드라 막지 않는다(지침 5).
4. 16진 대소문자: `0xFF`는 E0102인데 `"\xFF"`·`"\u{FF}"`는 받고, `lexical.md`의 예가 `\u{1F600}`이며 값은 소문자로 찍힌다. 추천: 이스케이프도 소문자만(`lexer/string.rs`), 예를 고친다.
   처리(2): 2026-11-14, D125. 커밋 `feat!: the loader always adds .fung to an import path (D125)`. `loader.rs` `load_file`이 조건 없이 `.fung`을 붙인다. 별칭이 없으면 `x.fung`이 모듈 이름이 아니라 E0305가 먼저이므로 E0305에 "확장자를 빼라"는 도움말을 달았다(별칭을 주면 E0301). `modules.md`·`04-modules.md`. 시험: `an_import_path_always_gets_the_extension`, 골든 `err_import_extension.fung`(E0301)·`err_import_extension_name.fung`(E0305).
5. 기간: `1m60s`(= `2m`), `1h90m`(= `2h30m`), `1m0s`, `0h5m`을 받는다. 추천: 첫 성분 뒤의 성분은 제 단위보다 작고(60s·60m·1000ms) 0이 아니어야 한다(`lexer/number.rs` `duration`). 한 기간의 표기가 하나가 되고 출력과 같아진다.
   처리(5): 2026-11-14, D128(바꾸지 않음). 성분에 제한을 더하면 "…만 빼고" 규칙이 하나 느는데 `1h90m`은 값이 하나로 정해져 틀린 값이 남지 않는다. `lexical.md`의 기간 절에 "성분의 크기에는 제한이 없다"를 적었다.
6. 원시 문자열: `match` 패턴은 받는데(`` `a` => … ``) 맵 키·`import` 경로·`test` 이름은 E0112다. `grammar.md`의 `pat_term`은 `PLAIN_STRING`만 적었다(`parser/pattern.rs:132`가 `Kind::RawString`을 받음). 추천: "보간 없는 문자열"은 한 규칙이다. 네 자리 모두 원시 문자열을 받고 문법에 적는다(원시 문자열은 보간이 없다). 패턴에서 거절하는 쪽도 일관되다.
- 처리(4): 2026-11-14, D127(D56을 되돌림). 커밋 `feat!: hex digits take either case in number literals (D127)`. `lexer/number.rs`의 `prefixed_int`가 이스케이프와 같은 `is_hex_digit`(대소문자 무관)를 쓰고 `is_lower_hex_digit`와 "16진 숫자를 소문자로" 도움말을 지움. 접두사·지수·이스케이프 글자는 문법 글자라 소문자 그대로이고, `0XFF`의 도움말은 접두사만 고친 `0xFF`(전에는 숫자까지 소문자로). `parse_int`·`int(s)`는 렉서를 쓰므로 함께 따라온다. E0235의 같은 리터럴은 값으로 견주므로 `0xFF`·`0xff`·`255`가 하나다(시험만 더함). 포매터는 리터럴을 쓴 그대로 둔다(정규화 없음). `lexical.md`(진법 행, 이스케이프, 경계 사례의 펜스를 `0XFF`로), `errors.md`(E0102), `manual/vm/03-lexer.md`(연습 문제를 `0X1F`로), `01-syntax.md`. 시험: `integer_literals`(`0xFF`·`0xCafe`·`0xaB_cD`), `uppercase_prefixes_suggest_the_lowercase_form`(도움말 `0xFF`·`0xaB`), `int_literal_syntax`(`0x1F`·`-0xCafe`), `arms_after_a_catch_all_are_errors`, 골든 `err_hex_upper.fung`를 `hex_digits.fung`로 바꿈(리터럴·이스케이프·`parse_int`·`int`의 대소문자; `0XFF`의 E0102는 이미 `err_number_upper_prefix.fung`).
   처리(6): 2026-11-14, D129. 커밋 `feat!: a raw string stands where a plain string is required (D129)`. 렉서의 `Token::is_string`·`Token::plain`이 원시 문자열을 보간 없는 문자열로 보고, 파서의 맵 키(리터럴·패턴)·`import` 경로·`test` 이름이 그것을 쓴다(문자열 패턴의 별도 갈래는 합침). `grammar.md`의 `PLAIN_STRING`, `lexical.md`, `testing.md`, `01-syntax.md`, `02-values-and-types.md`. 시험: `raw_strings_fill_the_plain_string_positions`, 골든 `raw_string_positions.fung`.

### SP79. 맵 키 자리의 식별자만 NFC로 맞추지 않는다 — 결정, 낮음 (기존 결정 재검토: 11-10에 "맵 키는 쓴 그대로"를 문제없음으로 닫음)
- 재현(둘째 표기는 분해형 é): `m = {café: 1}; m.café` → 실행 중 E0508. `café = 1; m = {café}`의 키는 분해형인데 변수는 NFC다. 두 표기의 `{café: 1, café: 2}`는 키 둘이고 E0113이 아니다.
- 새 근거: D102가 이름 속의 보이지 않는 글자를 "눈으로 같은 두 이름이 다른 변수가 되기 때문"에 E0101로 만들었다. 키 식별자가 "눈으로 같은 두 프로그램이 다르게 도는" 마지막 자리이고, `lexical.md`에 예외 문장이 필요하다.
- 원인: `crates/fung-front/src/lexer.rs` `ident`가 `raw`를 남기고 `Token::key()`가 그것을 준다.
- 고치는 법(결정):
  - (가) **추천**: NFC가 아닌 식별자는 어디서나 E0101. 정규화 규칙도 키의 예외도 사라지고 `m.k`는 그대로 정확히 `m["k"]`다.
  - (나) 식별자 모양의 키를 NFC로 맞춘다(`m.k`가 `m["k"]`와 달라질 수 있다).
  - (다) 그대로.
- 처리: 2026-11-14, D130(점검 문서의 추천 (가)가 아니라 (나)). 커밋 `feat!: an identifier-shaped map key is read as NFC (D130)`. 렉서의 `Token::key()`(쓴 그대로)를 없애고 맵 리터럴 키·축약·`m.k`·맵 패턴 키가 모두 NFC인 `text`를 쓴다(`raw`는 에러 밑줄 길이에만 남음). 한 길이라 `m.café`·`{café: v}`·`{café}`가 모두 같은 키이고 `{café: 1, café: 2}`는 E0113이다. `01-syntax.md`(L8을 뒤집는 까닭), `02-values-and-types.md`, `lexical.md`. 값의 표기: NFC가 아닌 키는 따옴표로 찍는다(`consts::is_plain_key`가 NFC도 봄; 맨 이름으로 찍으면 다시 읽을 때 NFC 키가 되어 "보이는 값은 소스로 되읽힌다"가 깨지므로. 결정 문서에는 없는 따름 결과). 시험: `identifier_shaped_keys_are_nfc`, `format_map_keys_that_are_not_nfc_are_quoted`, 골든 `nfc_keys.fung`(옛 "쓴 그대로" 시험을 새 규칙으로 고침).

### SP80. 끝을 포함하는 빈 슬라이스가 시작 0에서만 안 된다 — 결정, 낮음 (다시 확인)
- 재현:
  ```
  xs = [1, 2, 3]
  print(xs[3..=2], xs[1..=0])     → [] []
  print(xs[0..=-1])               → E0507 slice end -1 is out of range
  r = 0..=-1;  xs[r]              → E0507 (올바른 빈 범위 값인데 어떤 리스트도 색인하지 못함)
  ```
- 근거: `access.md` "시작이 끝보다 뒤면 에러"와 "`..=`의 끝은 실제 원소의 인덱스여야". 첫 문장대로면 `xs[3..=2]`는 E0516인데 `[]`가 나온다. `operators.md`는 범위 값의 빈 것을 `n..n`과 `n..=n-1`로 정했다.
- 원인: `crates/fung-vm/src/value/index.rs:214-252` `slice_bounds`.
- 고치는 법(결정):
  - (가) **추천**: 끝을 포함하는 슬라이스는 비지 않는다: `lo > end`는 E0516. 빈 슬라이스는 `a..a` 하나로 쓴다. 범위 값 `n..=n-1`로 하는 색인도 같은 에러다(`access.md`의 "`xs[r]`은 같은 끝의 슬라이스와 같다").
  - (나) `0..=len` 안의 모든 `a`에 `a..=a-1`을 허락한다(`0..=-1` 포함).
- 처리: 2026-11-14, 결정 D117((나)를 넓힘: `xs[a..=b]`는 정확히 `xs[a..b+1]`), 커밋 `feat!: an inclusive slice is the half-open one past its end (D117)`. `value/index.rs` `slice_bounds`가 끝을 포함하면 i128로 1을 더할 뿐 따로 검사하지 않는다(`i64::MAX`도 넘치지 않고 E0507). 에러 문구의 끝은 전처럼 쓴 그대로다(`..=5`는 `slice end 5`).
  `access.md`(조항 삭제, 예제 펜스 추가)·`02-values-and-types.md`, 시험 `an_inclusive_slice_is_the_half_open_one_past_its_end`(모든 끝 쌍을 `a..b+1`과 견줌, 범위 값 색인)·`slice_inclusive_cases`·`slice_errors_name_the_index_as_written`·`tests/lang/slices_inclusive.fung`.

### SP81. 함수가 든 값의 `==`가 다른 원소의 값과 맵의 넣은 순서에 따라 갈린다 — 결정, 중간 (다시 확인, D91·D62에 닿음)
- 재현:
  ```
  f = x => x
  print([1, f] == [2, f])                     → false
  print([1, f] == [1, f])                     → E0501
  print({a: 1, cb: f} == {a: 2, cb: f})       → false
  print({cb: f, a: 1} == {cb: f, a: 2})       → E0501   (같은 맵, 넣은 순서만 다름)
  print(unique([[1, f], [2, f]]))             → E0501   ([1, f] == [2, f]는 false인데)
  ```
- 근거: `values.md` "map: 키 집합과 값이 같음(순서 무시)", "`unique`…은 이 표의 `==`를 따른다". 둘 다 여기서 깨진다.
- 새 근거(D91에 견줘): D91은 타입 불일치가 에러가 아니게 되었으므로 첫 차이에서 멈춰도 된다고 보았다. 함수의 같음은 `==`에 남은 유일한 에러인데, 그것이 값과 넣은 순서에 따라 나타난다. 지난 점검의 SP56("자료에 따라 나타났다 사라지는 에러")이 이 한 곳에 남았다.
- 원인: `crates/fung-vm/src/value/compare.rs:27-72` `walk`가 첫 차이에서 멈춘다. `hash_leaf`(624행)는 함수면 늘 에러다.
- 고치는 법(결정):
  - (가) **추천**: 두 쪽이 다 함수를 품으면(`holds_fn()`, 큰 리스트는 노드 기억으로 O(1)) `walk`가 첫 차이에서 멈추지 않고 맞붙는 쌍을 끝까지 보아, 함수끼리 맞붙으면 E0501. 함수가 없는 값은 지금처럼 일찍 끝나므로 평범한 자료에는 비용이 없다. 에러가 모양에만 달린다.
  - (나) 두 쪽이 다 함수를 품은 `==`는 자리와 상관없이 E0501. 규칙은 더 짧고 조금 넓다.
  - (다) 그대로 두고 문서에 적는다.
- 처리: 2026-11-14, 결정 D116((나)), 커밋 `feat!: == on two values that both hold a function is E0501 (D116)`. `value/compare.rs`의 `eq`가 걷기 전에 두 쪽의 `holds_fn()`을 보고(둘 다면 E0501), 걷기 `walk`는 에러 없이 `equal`과 같은 것이 되었다(`strict` 갈래와 같은 저장소 지름길의 함수 예외가 사라짐).
  `in`(`index::contains`)은 찾는 값이 함수를 품지 않으면 `equal`로 곧장 훑는다. 메시지는 바깥 두 값의 타입(``cannot compare list and list``)과 "both values hold a function" 도움말. `unique`·해시는 그대로(함수는 해시가 없다).
  `values.md`·`operators.md`·`errors.md` E0501·`collections.md` `unique`(D91 뒤 낡은 "타입이 같아야" 문장도 함께)·`02-values-and-types.md`, 시험 `eq_says_false_for_different_types_and_refuses_functions`·`closures_print_and_compare`·`tests/lang/err_eq_fn_held.fung`.

## 5. 앞단과 진단 (FE)

### FE77. 순수 호출이 든 버려지는 식이 E0234를 비켜 간다 — 버그, 중간 (다시 확인)
- 재현:
  ```
  test "t":
    len([1]) == 2
  ;;
  ```
  `fung test` → `ok t.fung: t / 1 passed, 0 failed`. `1 == 2`였다면 E0234와 "did you mean `assert`" 도움말이 나온다.
  문장으로 통과하는 것: `count == len(xs)`(`=` 대신 `==`), `upper("a") + "b"`, `len(sort(x)) + 1`, `[sort(x)]`, `sort(x)[0]`, `Ok(sort(x))`, `"${sort(x)}"`, `nil ?? sort(x)`. 맨 `sort(x)`와 `x + 1`은 E0234다.
- 근거: `blocks.md` "효과가 없다고 알려진 내장 함수…의 호출도 E0234다: 결과를 버리면 아무 일도 없었던 것이다", 경계 사례 "`=` 대신 쓴 `==`가 흔한 경우다". "호출이 든 식은 된다"는 문장이 구멍이다.
- 원인: `crates/fung-front/src/resolve/value.rs:234` `effect_free`가 모든 `Head::Call`에 `false`다. 순수 표는 버려지는 노드 자신이 호출일 때만 본다(`check`, 약 60행).
- 고치는 법: `effect_free`에서 `cx.pure`에 있고 인자가 모두 효과 없는 호출은 효과 없음. `blocks.md`의 문장을 "효과가 있을 수 있는 호출이 든 식"으로 고친다. SP74 (가)를 하면 별칭을 거친 호출도 같은 표를 쓴다.
- 처리: 2026-11-14, 커밋 `fix: a pure call on effect-free arguments is itself effect-free (FE77)`. `resolve/value.rs`의 `effect_free`가 순수 표(`Pure`)를 받아, 표에 있고 인자가 모두 효과 없는 호출을 효과 없음으로 본다. 맨 문장의 순수 호출은 전처럼 인자와 상관없이 E0234(도움말은 `pure_call` 한 곳). `blocks.md`·`errors.md`·`01-syntax.md`를 고치고 `tests/lang/err_effect_free_pure_call.fung`을 더함. 저장소 소스는 걸린 것이 없었고, REPL 시험 하나가 `Err(len([1]))`를 실행 중 E0606의 예로 쓰던 것을 사용자 함수로 바꿈.

### FE78. 괄호 안의 줄바꿈이 원소 사이가 아닌 자리에서도 공백이다 — 버그, 낮음 (다시 확인, D85의 누락)
- 재현(괄호 안에서 통과, 밖에서는 E0112·E0110):
  - `{a⏎: 1}`, 맵 패턴 `{a⏎: b}`
  - `xs[1⏎..]`, `xs[0⏎..=⏎1]`(`[1⏎..3]`은 E0112)
  - `[if true⏎: 1 else: 2 ;;]`, `[for x⏎in [1]: x ;;]`, `[fn(x)⏎: x ;;]`, `fn(a⏎: int)`, `fn(a⏎= 1)`
  - 고르지도 않다: `f(a⏎: 1)`은 E0112인데 `{a⏎: 1}`은 통과.
- 근거: `blocks.md` "괄호 안에서도 밖에서도 같다", "괄호 안의 줄바꿈은 원소 사이와 여닫는 괄호 옆에서 공백이다 … 두 줄을 한 식으로 잇지는 않는다".
- 원인: `crates/fung-front/src/parser.rs:225` `Parser::peek`가 `nest > 0`이면 모든 줄바꿈을 건너뛴다. D85는 연산자 고리(`parser/expr.rs` `infix_loop`)에 `peek_infix`만 더했다.
- 고치는 법: `peek`의 건너뛰기를 없애고, 여는 괄호 뒤·닫는 괄호 앞·`,` 둘레에서만 드러내어 건너뛴다(규칙 1·2는 그대로).
- 처리: 2026-11-14, 커밋 `fix: inside brackets a newline is space only next to the brackets and around commas (FE78)`. `Parser::peek`가 줄바꿈을 건너뛰지 않고, 괄호마다 여는 괄호 뒤·닫는 괄호 앞·`,` 둘레에서 `gap`으로, 맵의 축약 판정은 `item_ends`로 건너뛴다. 괄호 안에만 있는 규칙 1의 토큰(맵 키·이름 인자·매개변수 타입 뒤의 `:`, 기본값의 `=`, 슬라이스의 `..`)은 줄 끝에서 이어진다: 규칙 1의 목록이 "블록 시작 `:`"만 적었던 것을 모든 `:`로 고쳐 씀(전에도 괄호 안이라 받던 것, `fn f(h:⏎int)`). `blocks.md`·`01-syntax.md`, 시험 `bracket_newlines.fung`·`err_bracket_item_newline.fung`. 저장소 소스는 걸린 것이 없었다.

### FE79. 맵 패턴 `{_}`가 말없이 키 `"_"`다 — 버그, 낮음 (다시 확인)
- 재현: `fn f(v) = match v: {_} => 1; _ => "z" ;;`, `print(f({a: 1}), f({_: 2}))` → `z 1`. `{_} = {a: 1}`은 실행 중 E0602.
- 근거: `grammar.md` `pat_entry = … | NAME`이고 `_`는 `pat_term`에서 NAME과 따로다. 맵 리터럴 `{_}`는 E0215다.
- 원인: `crates/fung-front/src/parser/pattern.rs` `pmap`(약 303행)의 축약 갈래가 `_`를 포함한 모든 `Kind::Name`을 받는다.
- 고치는 법: `_` 축약은 E0112("`{_}` binds nothing; write `{key: _}`"). SP70의 결정에 따라 도움말을 맞춘다.
- 처리: 2026-11-14, 커밋 `fix: the map pattern {_} and a top-level pattern that binds nothing are errors (FE79, FE80)`. `parser/pattern.rs` `pmap`의 축약 갈래가 `_`에 E0112("expected a name to bind, found `_`", 도움말 "`{_}` binds nothing; write `{key: _}`"). D114(`...`)는 아직 하지 않아 도움말에 `{...}`는 넣지 않았다: D114를 고칠 때 "or `{...}` for any map"을 더한다. `match.md`·`grammar.md`.
- 처리(D114 뒤): 2026-11-14, `{_}`의 도움말에 "or `{...}` for any map"을 더함(커밋 `feat!: a map pattern matches exactly the keys written unless opened with ... (D114)`).

### FE80. import된 파일의 최상위에 이름을 묶지 않는 구조 분해 대입이 온다 — 버그, 낮음 (다시 확인, FE69 고침의 누락)
- 재현: `lib5.fung`에 `(_, _) = (print("side5"), 2)`를 두고 `import "./lib5"` → `side5`가 찍힌다. `_ = print(…)`는 E0306이다.
- 근거: `modules.md` "아무 이름도 묶지 않는 `_ = 식`도 맨 문장으로 보아 E0306이다(구조 분해 대입 `(a, b) = 식`은 이름을 남기므로 된다)".
- 원인: `crates/fung-front/src/resolve.rs:416` `discards`가 `kids[0].head == Head::Pwild`만 본다.
- 고치는 법: `discards` = 패턴이 이름을 하나도 묶지 않는다.
- 처리: 2026-11-14, 커밋 `fix: the map pattern {_} and a top-level pattern that binds nothing are errors (FE79, FE80)`. `resolve.rs`의 `discards`가 `Node::bound_names`로 "이름을 하나도 묶지 않음"을 본다. `modules.md`의 문장을 고침.

### FE81. 리터럴로 드러난 타입 오류의 검사가 스칼라 리터럴에서 멈춘다 — 버그, 낮음 (다시 확인: `not 1s`, `-"a"`)
- 재현(모두 `fung check` 통과):
  - 기간: `not 1s`, `if 1s: … ;;`, `1s(2)`, `1s + "a"`, `1s ?? 2`
  - 보간 문자열: `"a${1}"(2)`, `not "a${1}"`
  - 컬렉션: `[1](2)`, `{a: 1}(2)`, `if [1]: … ;;`, `not [1]`, `[1] - [1]`
  - 단항: `-"a"`, `-true`, `-nil`
  - 람다·범위: `(x => x) + 1`, `1..2 ?? 3`
  견줄 것: `1(2)`, `not 1`, `1 + "a"`, `[1] ?? 2`는 컴파일 에러다.
- 근거: `conditions.md` "리터럴을 바로 쓴 … 타입이 이미 드러나 있어 컴파일할 때 같은 번호로 멈춘다"(D99).
- 원인: `crates/fung-front/src/resolve/value.rs:363` `literal_type`에 기간·보간·리스트·맵·튜플·태그·함수·범위가 없고(`??` 갈래만 컬렉션을 안다) 단항 갈래가 없다.
- 고치는 법: "문법으로 드러난 종류" 함수 하나를 네 검사가 함께 쓰고 단항 `-`를 더한다. D99가 뺀 것(리터럴의 인덱스·필드·`!`·`for` 대상)은 다시 꺼내지 않는다.
- 처리: 2026-11-14, 커밋 `fix: the literal type checks cover every kind the syntax shows (FE81)`. `resolve/value.rs`의 `known_type`(실행의 `type_name`과 같은 이름: 수·`str`(보간 포함)·`bool`·`nil`·`sym`·`dur`·`list`·`map`·`tuple`·`range`·태그 이름·`fn`(람다·익명 `fn`·부분 적용))을 호출·조건·연산·`??`·단항 `-` 검사가 함께 쓴다. 연산의 뜻은 `operator_types`가 `value::arith`·`compare`와 같게 판정(`"a" * 1s`는 실행처럼 "dur and str", 두 리스트·튜플의 비교는 실행에 맡김), 도움말도 실행과 같다(문자열 `+`, 결과값의 "unwrap it"). fung-vm 시험 `literal_type_errors_are_the_runs`가 리터럴 꼴의 컴파일 에러와 함수를 거친 같은 값의 실행 에러가 머리 줄·도움말까지 같은지 본다. D113은 `known_type`을 타입 표기의 이름과 견주면 된다. `conditions.md`·`errors.md`.

### FE82. 진단의 작은 것들 — 정리(진단), 낮음 (1·2·3 다시 확인)
1. `x += 1`·`x -= 1`·`x++`의 도움말이 "`n = n + …` (a function's local name may be bound again)"을 권한다. D88 뒤로 그것은 E0238이다. `"a" ++ "b"`도 같은 증가 도움말을 받는다. `crates/fung-front/src/habits.rs:110`·`:122`. → "bind a new name (`m = n + 1`) or compute it in one expression", 이항 `++`에는 "strings and lists join with `+`".
2. 슬라이스 끝의 타입 에러가 어느 끝이 틀렸든 `int`를 먼저 적는다: `xs["a"..2]`도 `xs[.."a"]`도 "cannot slice with int and str", 끝이 하나뿐인 `xs[y..]`도 "int and float". 같은 실수가 인덱스에서는 E0509("cannot index list with float"), 슬라이스에서는 E0501이다. `crates/fung-vm/src/value/index.rs:197-211` `bound`. → 한 번호(E0509)와 "slice bound must be int, got str".
3. `"${atom(1)}"`·`join([atom(1)])`이 "a atom handle"이라고 적는다. `crates/fung-vm/src/vm/m4.rs:65`.
4. `channel(1.0)`·`channel("a")`가 "must be a non-negative int, got 1.0"이다. 다른 개수 인자(`take`·`repeat`)는 타입이 틀리면 "must be int, got float"이고 "non-negative"는 음수에만 쓴다. `crates/fung-vm/src/conc.rs:243-253` `channel_fn` → `nat::need_count`.
5. E0208의 도움말 "local functions can only call themselves": 앞에서 정의한 지역 `fn`도 부를 수 있다(`scope.md`). `diag/catalog.rs:169`.
6. `\u{}`, 닫지 않은 `\u{1F600`, 8자리 `\u{00000041}`이 모두 "E0103 unknown escape"다. 아는 이스케이프이고 모양이 틀렸다. 6자리 한도는 `lexical.md`에 없다.
7. `import "./sub/"` → "E0305 `` is not a valid module name; use `as`"(빈 이름, `as`로 고쳐지지 않음). E0301이어야 한다.
8. 홀로 선 `end` 문장이 같은 토큰에 에러 둘(E0204 + E0234). `if` 사슬 중간의 `elif`는 습관 도움말이 없다(문장 시작에서만). `match … default => 2`는 "unused variable `default`".
9. `x.a`를 nil·int에 쓰면 "cannot index nil with str"이다. 필드 접근의 문구("nil has no fields")가 낫다.
10. 공개 `fn`의 나머지 매개변수 `fn f(...len)`을 인터페이스 예외로 받는다. 나머지 매개변수는 이름으로 줄 수 없으므로(E0406) 예외의 까닭이 없다. 패턴 매개변수 `fn f((len, b))`는 E0240이다. `resolve/names.rs` `interface_name`.
- 처리: 2026-11-14, 커밋 `fix: small diagnostics of the fifth review (FE82)`. 1 `habits.rs`: `+=`·`++`는 "bind a new name (`m = n + 1`) or compute it in one expression", 두 값 사이의 `++`는 `habits::JOIN`("strings and lists join with `+`"). 2 `value/index.rs` `bound`가 인덱스의 `index_error`(E0509 "cannot index list with str": 한 번호·한 문구로 틀린 끝의 타입을 적음; 카탈로그는 번호마다 문구 하나라 "slice bound must be int"가 아니라 인덱스의 문구). 3 `m4.rs`의 관사(an atom; 필드 도움말도). 4 `channel_fn`이 `nat::need_count`. 5 E0208 도움말. 6 E0103의 문구를 "invalid escape"로 하고 `\x`·`\u` 모양 도움말, `lexical.md`에 1~6자리. 7 `loader.rs`: `/`로 끝나는 경로는 E0301. 8 `end` 문장은 같은 자리의 E0204가 있으면 E0234를 빼고(`resolve.rs`), 문장 안의 습관 낱말(`elif`)도 E0134(`parser.rs` `habit`, 값 낱말 `null` 등은 제외), 절 전체가 이름인 패턴의 E0239에 `_ =>` 도움말(이름 목록이 아니라 모든 이름 절). 9 맵·모듈이 아닌 값의 필드는 E0506 "cannot read the field `a` of nil"(`index::field_error`). → 2026-11-14 뒤 커밋 `fix: a field of a value without fields is E0509 again (FE82)`에서 되돌림: `m.name`은 `m["name"]`(access.md)이고 `x["a"]`가 E0509이므로, 같은 연산을 표기에 따라 두 번호로 알리지 않는다(지침 2 "한 규칙은 한 에러 번호"). 문구는 인덱스의 것(카탈로그는 번호마다 문구 하나), 필드였다는 것은 도움말 "a nil has no fields: only maps and modules do"가 말하고 `xs.len`의 도움말은 그대로. 10 나머지 매개변수는 인터페이스가 아님(`resolve/fn_.rs`; 번호는 지금의 E0240, D122가 E0238로 합침).

## 6. 컴파일러와 바이트코드 (VM)

### VM48. 기본값 안에서 묶은 이름이 뒤 매개변수를 덮어쓴다 — 버그, 높음 (다시 확인)
- 재현:
  ```
  fn f(a, b = (match a: (t, k) => t + k ;;), c = 5):
    print(a, b, c)
  ;;
  f((1, 2))          → (1, 2) 3 2     기대 (1, 2) 3 5
  f((1, 2), c: 9)    → (1, 2) 3 2     기대 (1, 2) 3 9   (준 인자가 덮어써진다)
  f((1, 2), 7, 9)    → (1, 2) 7 9     (맞음: 기본값을 건너뜀)
  ```
  같은 원인의 다른 모양:
  - `fn q(a, b = (if a > 0: y = a + 1; z = y + 1; z else: 0 ;;), c = 7)`: `q(2, c: 1)`이 `2 4 4`.
  - `fn w(a, b = (match (a, a): (x, y) => x + y ;;), ...rest) = print(a, b, rest)`: `w(1)`이 `1 2 1`. 나머지 매개변수가 정수를 쥔다("늘 리스트"가 깨짐).
- 근거: `functions.md` "기본값은 … 앞 인자를 참조할 수 있다", 이름 붙인 인자는 이름으로 묶인다.
- 원인: `crates/fung-front/src/resolve/fn_.rs:63-80` `Resolver::function`. 기본값 `i`를 걷는 동안 `next_slot`이 `i`라서, 기본값이 묶는 이름과 숨은 임시 칸이 `i, i+1, …`, 곧 뒤 매개변수의 칸을 받는다. dump: `JUMPIFSET 10 1; LOADLOCAL 0; MATCH … ($1, $2)`. 뒤의 `JUMPIFSET 13 2`는 2번 칸이 찬 것을 보고 `c`의 기본값을 건너뛴다. 검증기는 보지 못한다(칸은 존재한다).
- 고치는 법(사본에서 확인): 고리 앞에서 `next_slot = decl.params.len()`으로 두고 `next_slot = i + 1`을 지운다. 사본에서 모든 재현이 맞게 나오고, fung-front 시험·골든·dump·run 기대 출력이 통과하며(기대 dump 변화 없음), 이름을 묶는 기본값을 넣은 무작위 프로그램 5,696개가 통과했다.
- 함께 할 것:
  - 기본값이 묶은 칸은 수명 분석 밖이라, 고친 뒤에는 프레임이 끝날 때까지 값을 쥔다(지금은 덮어써져서 우연히 놓인다). VM39와 같은 부류다. 본문 시작에서 그 칸들에 `CLEARLOCAL`을 내거나, **추천**: 기본값을 본문 머리의 분석되는 문장으로 컴파일해 `compiler/fn_.rs:150-161`의 VM39 특례를 없앤다.
  - 검증기 규칙: `[0, 본문)`에서 저장·패턴 묶기는 지금 기본값을 받는 매개변수의 칸과 매개변수가 아닌 칸에만 쓴다.
  - `crates/fung-vm/src/vm/gen_tests.rs`가 이름을 묶는 기본값을 만들게 한다.
- 처리: 2026-11-14, 커밋 `fix: names bound in a parameter default no longer take a later parameter's slot (VM48)`. `resolve/fn_.rs`가 고리 앞에서 `next_slot = 매개변수 수`로 둔다.
  수명은 **추천안**(기본값을 분석되는 가지로)으로: `compiler/moves.rs` `fn_lives`가 기본값을 `else` 없는 `if`처럼 "그 매개변수가 비었을 때만 지나는 가지"로 걷는다(뒤로 걷기에서 타입 검사는 기본값 뒤의 읽기).
  기본값 안의 마지막 읽기는 `MOVELOCAL`, 기본값의 가지가 시작할 때 놓을 것은 `CLEARLOCAL`, 건너뛴 길에서 죽은 값과 기본값이 마지막으로 읽은 타입 붙은 매개변수는 타입 검사 뒤 `Lives::at_body`.
  VM39의 특례(`compiler/fn_.rs`의 `local_reads` 훑기, `Lives::live_at_start`)가 사라졌다. 기대 dump는 기본값이 마지막으로 읽는 매개변수의 `LOADLOCAL` → `MOVELOCAL` 두 곳(`drained`, `unused_exempt`의 `f`)만 바뀌었다.
  검증기 7번 규칙(`bytecode/verify.rs`, 서두의 `STORELOCAL`·`MATCH`·`MATCHORFAIL`은 기본값을 받는 매개변수와 매개변수 뒤의 칸에만), 생성형 시험이 이름을 묶는 기본값과 건너뛰기·위치·이름 인자를 만든다(고치기 전 코드에서 검증기와 기준 해석기 둘 다 잡음, 1만 개 통과).
  시험 `tests/lang/args.fung`(재현 셋), `tests/lang/local_lifetime.fung`(기본값 안의 보내는 끝이 놓여 채널이 닫힘), `bytecode/tests.rs` `the_verifier_keeps_a_default_off_the_other_parameters`.
  문서 `scope.md` "값의 수명", 설계 03, `manual/vm/08-bytecode.md`(검증기 일곱), `09-closures.md`(기본값 절).

### VM49. 반환 타입 있는 함수의 꼬리 `and`·`or`가 E0505 대신 E0404를 낸다 — 버그, 낮음 (다시 확인)
- 재현:
  ```
  fn not_bool(x) = x
  fn h(x) -> bool = x > 0 and not_bool(x)
  print(h(2))     → E0404 return value must be bool, got int (본문의 `=`를 가리킴)
  ```
  같은 함수를 `r = x > 0 and not_bool(x); r`로 쓰면 호출 자리에서 `E0505 expected bool, got int`다.
- 근거: `functions.md` "꼬리 호출"(프레임이 bool 검사를 이어받는다). `verify.rs:462-464`의 주석은 bool 검사가 반환 타입 검사보다 먼저라고 적는다.
- 원인: `crates/fung-front/src/compiler/fn_.rs:51-57`이 `TAILCHECKBOOL` 다음에 `TAILCHECK`를 내고, `vm/call.rs` `run_checks`는 나중 것부터 돈다.
- 고치는 법: `TAILCHECK`를 먼저 내고 `verify.rs:465-478`의 이웃 규칙을 맞춘다. 함께(코드 읽기): `Frame::add_check`(`vm/frame.rs:139`)가 검사를 `(pos, proto)`로만 가리고 `ty`를 보지 않아, 같은 자리의 bool 검사와 타입 검사가 서로를 갈아 치울 수 있다. `ty`를 열쇠에 넣는다.
- 처리: 2026-11-14, 커밋 `fix: a tail and/or checks its bool before the return type (VM49)`. `compiler/fn_.rs`가 `TAILCHECK`를 먼저, `TAILCHECKBOOL`을 바로 앞(꼬리 호출 직전)에 낸다: 검사는 최근 것부터 돌므로 bool 검사가 먼저다. `verify.rs`의 이웃 규칙: `TAILCHECK` 다음은 `TAILCHECKBOOL`이나 꼬리 호출, `TAILCHECKBOOL` 다음은 꼬리 호출만. `Frame::add_check`가 `(pos, ty, proto)`로 가린다. 서두(기본값·`CHECKTYPE`)는 건드리지 않아 `moves.rs`는 그대로. `09-closures.md`, 시험 `the_right_side_of_and_or_is_a_tail_call`(이름에 담은 꼴과 같은 E0505)·검증기 사례.

### VM50. 가드 붙은 `|` 패턴만 대안 256개 한도가 있고 코드가 곱으로 는다 — 결정·정리, 낮음 (다시 확인)
- 재현:
  ```
  fn f(v, k) = match v:
    (1 | 2, 1 | 2, 1 | 2, 1 | 2, 1 | 2, 1 | 2, 1 | 2, 1 | 2, 1 | 2) when k > 0 => "hit"
    _ => "miss"
  ;;
  → E0230 too many alternatives in a guarded pattern (limit 256)
  ```
  `when`을 빼면 컴파일된다. 자리 8개면 `MATCH` 256개와 가드 사본 256개가 된다. `match.md`는 한도를 적지 않았다. 9,000겹의 가드 붙은 패턴은 2.5 s다(가드 없이 0.01 s).
- 원인: `crates/fung-front/src/compiler/pattern.rs:93-135` `alternatives`가 이름을 묶지 않는 `|`까지 모두 곱하고 단계마다 하위 트리를 복제한다.
- 고치는 법(결정):
  - (가) **추천**: 이름을 묶는 `|`만 펼친다. 묶지 않는 대안은 가드의 결과를 바꿀 수 없으므로 한 패턴 안의 `Alt`로 남긴다. 흔한 리터럴 대안에서 한도와 코드 불어남이 사라진다.
  - (나) VM에 이어 가는 `MATCH`(대안 커서)를 두어 곱을 아예 없앤다. 한도 조항이 사라진다.
  - (다) 그대로 두고 `match.md`에 한도를 적는다.
- 처리: 2026-11-14, D131(추천 (가)). 커밋 `feat!: a guarded pattern spreads only the alternatives that bind names (D131)`. `compiler/pattern.rs`의 `alternatives`가 이름을 묶지 않는 부분 트리(`binds`: `Pbind`와 이름 있는 `Prest`가 없음)를 펼치지 않고 그대로 둔다(`PatKind::Alt`는 가드 없는 패턴에서 이미 쓰던 길). 한도 256과 E0230은 이름을 묶는 대안에만 남고 메시지가 "alternatives that bind names in a guarded pattern"이다. 남는 한도를 `match.md`·`errors.md`·`03-functions-and-patterns.md`에 적었다(문서만 고치는 부분). 시험: `a_failed_guard_tries_the_next_alternative`(묶는 대안 9개 E0230, 8개 됨, 묶지 않는 대안 9·200개와 섞인 경우), 골든 `match_guard_plain_alternatives.fung`. 9,000겹의 느림(2.5 s)은 이름을 묶는 겹이 아니면 사라지는지 측정하지 않았다.
  측정(2026-11-14, 릴리스, `fung check`): 9,000겹 `[[[…1…]]] when k > 0`(묶는 이름 없음)은 0.017 s로 사라졌다(전에는 2.5 s). 9,000겹 `[[[…x…]]] when x > 0 and k > 0`(이름을 묶음)은 여전히 2.1 s다: 이름을 묶는 겹은 D131대로 펼쳐지고 하위 트리를 복제하는 이차 구간이 남는다. 이런 모양은 현실에 없다고 보아 더 고치지 않는다.

### VM51. 컴파일 시간의 이차 구간 넷 — 성능, 낮음 (1·2·3 다시 확인)
1. 상수 접기가 중간 상수를 모두 복제해 둔다. `"a" + "a" + …` 10만 항의 `check`가 1.0 s에 **최대 메모리 6.0 GB**다. `{a: {a: …}}` 5,000겹은 1.0 s. `crates/fung-front/src/compiler/fold.rs:95-101` `walk`의 `out.insert(n.id, c.clone())`. 컴파일러는 가장 바깥 것만 찾는다. → 부모가 접히지 않을 때만 적고, 복제 대신 옮긴다.
2. `x ?? x ?? …` 10만 항의 `check`가 1.05 s다(지난 점검 FE75가 원인을 못 찾고 남긴 것). `crates/fung-front/src/compiler.rs:351-354` `enter`가 `lives.entry`를 보기 전에 `start_pos(n)`(줄 처음부터 글자 세기)를 부른다. → 놓을 것이 없으면 먼저 돌아간다(사본에서 1.08 → 0.14 s). `let_go`(344행)도 같은 모양.
3. `else if` 5만 개의 `check`가 2.0 s다(10만 6.6 s, 20만 20.9 s). `crates/fung-front/src/bytecode.rs:242` `Proto::thread_jumps`가 가지 끝마다 `Jump` 사슬 전체를 다시 따라간다. → 뒤에서 앞으로 돌거나 마지막 목적지를 기억한다.
4. 한 줄에 에러가 많으면 이차다(FE75의 나머지): `print(u0, …, uN)` 한 줄 16만 이름에 11.2 s. `crates/fung-front/src/diag.rs:250`의 `line.is_ascii()`가 에러마다 줄 전체를 훑는다. → 줄마다 한 번.
- 처리: 2026-11-14, 커밋 `perf: four compile-time quadratics go linear (VM51)`. 언어가 보이는 것(dump·에러·위치)은 그대로다(dump·골든·매뉴얼 시험).
  1. `compiler/fold.rs`: `walk`가 자식의 상수를 부모가 접히지 않을 때만 표에 옮겨 적는다(`record`, 맵 항목은 값의 번호로). 컴파일러는 접히는 노드를 자식보다 먼저 찾고 자식에 내려가지 않으므로 바깥 것만 있으면 된다. 접기는 자식 상수를 접힐 때만 꺼내 쓰고(`fold(n, &mut cs)`), 문자열끼리의 `+`는 왼쪽에 제자리로 덧붙인다. 10만 항 0.98 s·6.0 GB → 0.15 s·0.34 GB, 맵 5,000겹 1.0 → 0.014 s.
  2. `compiler.rs` `let_go`·`enter`: 놓을 칸이 있을 때만 `start_pos`를 부른다. `??` 10만 항 1.11 → 0.15 s.
  3. `bytecode.rs` `thread_jumps`: 앞으로 가는 `JUMP` 사슬의 끝을 뒤에서 앞으로 한 번에 구한다(같은 결과인지 옛 방식과 견주는 생성형 시험 `threading_jumps_in_one_pass_changes_nothing_else`, 3,000가지). 점검의 `else if` 5만 개 2.0 s는 512fd966에서 재현되지 않았다(0.21 s, 20만 개 0.83 s로 선형). 사슬이 실제로 생기는 4,900겹의 `then … else`는 0.057 → 0.043 s.
  4. `source.rs` `line_chars`: 파일의 줄마다 글자 수를 처음 물을 때 한 번 센다(`OnceLock`). `diag.rs`의 발췌는 그 수로 ASCII인지 알고 글자 수를 다시 세지 않는다. 에러 16만 개 9.5 → 1.3 s(에러 수에 선형).
  시험 `perf.rs` `three_long_shapes_compile_in_linear_time`(문자열 10만·`??` 10만은 0.5 s, 에러 12만은 2 s 안). 측정 `docs/notes/bench-languages.md` 4.13절.

### VM52. `and`·`or`의 꼬리 호출마다 `TAILCHECKBOOL`이 약 11 ns 든다 — 성능, 낮음
- 측정(2천만 번): `fn loop(n) = n == 0 or loop(n - 1)` 1.02 s, 같은 고리를 `if … else`로 0.79 s(+27%).
- 원인: `crates/fung-vm/src/vm/call.rs` `tail_check_bool`이 번번이 `Check`를 만들고(proto `Rc` 복제, 위치 찾기) `Frame::add_check`가 훑고 지우고 넣는다. VM44가 `TAILCHECK`에 준 `has_check` 지름길이 없다.
- 고치는 법: 프레임의 마지막 검사가 같은 `(proto, pos, None)`이면 건너뛴다.
- 처리: 2026-11-14, 커밋 `perf: TAILCHECKBOOL skips the check the frame took last (VM52)`. `vm/frame.rs` `last_check_is`와 `vm/call.rs` `tail_check_bool`: 위치를 먼저 구하고, 프레임의 마지막 검사가 같은 원형·같은 자리의 bool 검사면 돌아간다. `add_check`가 그 검사를 지우고 다시 넣어도 목록이 같으므로 결과는 그대로다. 명령은 그대로 59가지, dump도 그대로. 시험 `vm/tests.rs` `the_right_side_of_and_or_is_a_tail_call`에 같은 검사를 거듭 넣는 고리와 두 검사를 번갈아 넣는 고리의 E0505 위치. 측정(릴리스, 7번 중앙값): 2천만 번 0.93 → 0.80 s, `if … else` 0.70 s(`docs/notes/bench-languages.md` 4.13절).

### VM53. 수명 분석과 검증기의 군더더기 — 정리, 낮음
1. 앞으로 걷는 분석이 `(bool …)`을 지나며 꼬리 위치를 넘기지 않는다. `fn a(x) = x > 0 and (x > 1 or b(x))`의 dump에 곧바로 돌아가는 가지의 `CLEARLOCAL 0; CONST true; RETURN`이 남는다(틀린 값은 아니고 명령 하나 낭비). `crates/fung-front/src/compiler/moves.rs:567-571` `Fwd::expr`. 모양 판정이 `Compiler`·`Back`·`Fwd` 세 곳에 따로 적힌 탓이다(VM38과 같은 부류, 그때의 "한 함수로" 정리가 안 됨). → 꼬리·가지 판정을 한 함수로 모은다.
2. 수명 분석의 `tests` 플래그는 죽은 코드다. `test` 블록은 최상위뿐이라(E0114) 칸을 붙잡지 못한다. 사본에서 지워도 모든 시험이 통과한다. `moves.rs:83`·`:249`·`:514`.
3. `Verified::max_stack`을 계산하고 쓰지 않는다(읽는 곳은 시험 하나). `crates/fung-front/src/bytecode/verify.rs:47-51`·`:482`. 쓰거나(프레임마다 스택 예약) 지운다. 재현 안 함(코드 읽기).
- 처리: 2026-11-14, 커밋 `refactor: the tail and/or analysis and the verifier lose their leftovers (VM53)`. 1) `compiler/moves.rs` `Fwd::expr`가 `and`/`or`의 검사된 오른쪽(`BOOL_CHECK`)에 꼬리 표시를 그대로 넘긴다: 곧바로 돌아가는 가지의 `CLEARLOCAL`이 사라졌다(`fn a(x) = x > 0 and (x > 1 or b(x))`, 기대 dump 한 곳이 짧아짐). 모양 판정 세 곳을 한 함수로 모으는 큰 정리는 하지 않았다(틀린 값이 아니고 이 분석이 VM37~39에서 여러 번 고쳐진 곳이라 위험이 이익보다 크다). 시험 `a_tail_and_or_nested_lets_go_of_nothing_on_the_quick_path`. 2) 죽은 `tests` 플래그를 지웠다(`lives`·`fn_lives`·`Back`·`Fwd`의 인자; `test` 블록은 최상위뿐이라 이 프레임의 칸을 붙잡지 못하므로 분석이 건너뜀). 모든 시험이 그대로 통과한다. 3) `Verified`·`max_stack`을 지웠다(`verify`는 `Result<(), Violation>`).

### VM54. VM 장과 설계 09가 `CALL`의 옛 모양을 적는다 — 문서, 낮음
- `manual/vm/08-bytecode.md:48`·`:139-140`, `manual/vm/09-closures.md:36`·`:39`의 손으로 쓴 표가 `CALL 1`이다. dump는 VM43 뒤로 `CALL 1 0`을 찍는다.
- `docs/design/09-implementation-plan.md:127`이 "두 단어 명령은 없다", `CALL`의 12 + 12비트 나눔, `encode_call`을 적는다. 셋 다 VM43에서 사라졌다.
- 처리: 2026-11-14, 커밋 `docs: the VM chapters and design 09 show CALL as it is now (VM54)`. `manual/vm/08-bytecode.md`·`09-closures.md`의 표와 설명이 `CALL 1 0`(`CALL n m`)이고, `09-implementation-plan.md`의 명령 인코딩 절이 둘째 24비트 피연산자(VM43)를 적는다.

## 7. 값과 prelude (VA)

### VA59. 큰 리스트의 슬라이스가 잘려 나간 원소를 붙잡는다 — 버그, 높음 (다시 확인)
- 재현:
  ```
  fn go(n):
    (tx, rx) = channel()
    xs = [tx] + map(0..n, i => i)
    ys = xs[1..]
    r = recv(rx)
    (r, len(ys))
  ;;
  print(go(10))     → (Err({kind: 'closed, …}), 10)
  print(go(100))    → E0702 deadlock: all tasks are waiting
  ```
  `xs[..n]`(보내는 끝이 맨 뒤), `drop(xs, 1)`, `take(xs, n)`, `reverse(xs)[1..]`, 나머지 패턴 `[_, ...ys] = …`도 같다. 자르는 비율에 달렸다: 101개 리스트의 `xs[80..]`은 닫히고(복사) `xs[50..]`은 교착한다.
- 근거: `scope.md` "값의 수명": 변수는 마지막 읽기 뒤에 값을 놓고, "함수 안에서 만든 채널의 보내는 끝을 … 채널이 닫힌다". `xs`는 슬라이스 뒤에 죽었으므로 `tx`는 놓여야 한다. 리스트의 크기는 구현의 속사정이다. 창 밖의 파일 핸들·큰 문자열도 슬라이스가 사는 동안 산다.
- 원인: `crates/fung-vm/src/value/list.rs:154` `List::slice`. `n > SMALL_MAX && n * 4 >= vec.len()`이면 영속 벡터 전체에 대한 보기 `{vec: vec.clone(), lo, hi}`를 돌려준다. 다른 갈래(164행, `truncate` + `drop_front`)는 맞게 놓는다(경계 잎을 포함한 8자리에서 확인). 탐침(`holds_in`)은 창을 지키므로 E0606·E0708·`==`는 맞다. 수명만 틀렸다.
- 고치는 법: 늘 자른다(`truncate(hi)` + `drop_front(lo)`, O(log n)). 보기 갈래를 없앤다. 그런 뒤 옛 보기에 대한 `push`·`concat`(`list.rs:187-195`)이 숨은 원소를 남기지 않는지 확인한다. 특례를 더하는 것이 아니라 지우는 고침이다. 시험: 크기 경계(32·1024·32768)를 넘는 리스트에서 슬라이스·`take`·`drop`·나머지 패턴 뒤에 채널이 닫히는지(생성형 시험에도).
- 처리: 2026-11-14, 커밋 `fix: a slice of a big list no longer keeps the items outside it alive (VA59)`. `value/list.rs`의 큰 리스트가 `Big(Box<Pvec>)` 하나다: 창 `{lo, hi}`를 없애고 `slice`는 늘 트리 사본을 자른다(32개 이하는 복사).
  옛 보기를 위한 `push`의 두 갈래와 `extendable_front`가 사라졌고, 쓰는 곳이 없어진 `Pvec::holds_in`·`range_holds`도 지웠다(`each_holding`은 맵이 써서 둠).
  시험 `tests/lang/local_lifetime.fung`(`xs[1..]`·`xs[..n]`·`drop`·`take`·`reverse(xs)[1..]`·`[_, ...rest]`를 10~40,000개, 32·1024·32768 경계에서: 모두 `Err(closed)`, 고치기 전에는 33개부터 E0702),
  `value/tests.rs` `a_slice_lets_go_of_the_items_outside_it`(잘린 자리마다 원본을 버린 뒤 `Rc` 셈이 1), `pvec_both_ends_match_the_model`은 `holds_in` 대신 자른 사본의 `holds`.
  성능: `perf` 시험(릴리스) 통과. 머리·꼬리 재귀 20만 개는 선형 그대로지만 상수가 커졌다(릴리스, `xs[1..]` 0.04 → 0.07 s, `[x, ...ys]` 0.04 → 0.06 s, `xs[..n]` 0.05 → 0.08 s; 160만 개에서 0.36 → 0.60 s). 자를 때마다 머리 잎(32칸 이하)을 복사하기 때문이다.
  문서 `manual/vm/10-values-and-memory.md`, 설계 02·09.
  뒤따름(2026-11-14, 커밋 `perf: a big list held by nothing else is cut in place`): 위 성능 저하를 고쳤다. SLICE는 꺼낸 리스트를 아무도 쥐지 않고 잘려 나가는 것이 `RELEASE_PIECE`(8,192, 한 걸음 약 1 ms) 이하면 제자리에서 자른다(`index::slice_owned`·`List::cut`, 잘린 원소는 곧바로 놓임: 시험 `a_list_held_once_is_cut_in_place`). 늘어난 시간의 대부분은 걸음마다의 `holds`가 메모 없는 새 머리 잎을 훑는 몫이어서, 머리 잎을 제자리에서 덜고 메모의 "없다"를 지키며(`Node::drop_vals_front`, 시험 `pvec_holds_follows_a_vector_eaten_in_place`) `Pvec::holds`가 머리·꼬리도 메모로 답한다. `xs[1..]` 20만 0.091 → 0.037 s(VA59 앞 0.040), `xs[..n]` 0.083 → 0.042 s, 나머지 패턴은 0.070 → 0.066 s(주제를 쥔 채 잘라 제자리로 못 함). `perf.rs` "eat a long list from either end". `docs/notes/bench-languages.md` 4.13절.

### VA60. `sort`·`sort_by`·`min`·`max`가 원소 순서에 따라 되다 말다 한다 — 버그, 중간 (다시 확인, D87 구현의 구멍)
- 재현:
  ```
  print(sort([[], [1, "a"], [2], [3, 0]]))   → [[], [1, "a"], [2], [3, 0]]
  print(sort([[], [3, 0], [1, "a"], [2]]))   → E0501 cannot compare str and int
  print(max([[], [1, "a"], [2], [3, 0]]))    → [3, 0]
  print(max([[], [3, 0], [2], [1, "a"]]))    → E0501
  ```
- 근거: `values.md` "`sort`·`sort_by`·`min`·`max`는 시작하기 전에 모든 원소를 첫 원소와 견준다. 그래서 정렬이 자료에 따라 되다 말다 하지 않는다."
- 원인: "첫 원소와 견줄 수 있다"는 추이적이지 않다. 길이가 다른 리스트는 앞부분만 견주므로 `[]`나 짧은 첫 원소는 무엇과도 견줄 수 있다. 사전 검사는 `crates/fung-vm/src/prelude/collections.rs:238-247`(sort)과 `:883-897`(`Best`).
- 고치는 법: 사전 검사를 모양 합치기로 바꾼다. 모든 원소(키)를 하나의 누적 모양(수 / 문자열 / 기간 / 시각 / 튜플(n, 모양들) / 리스트(자리별 모양, 긴 리스트가 늘림))에 접어 넣고 첫 충돌에서 E0501. 전체 크기에 선형이고 값과 무관하다. 그 뒤의 비교는 실패할 수 없다. `values.md`의 문장을 "모든 원소의 모양이 하나로 합쳐져야 한다"로 고친다. `<` 자체(두 값)는 D87대로 맞다.
- 처리: 2026-11-14, 커밋 `fix: sort, sort_by, min and max unify the shapes of all elements (VA60)`. `value/compare.rs`에 `Shapes`(바뀌지 않는 마디를 나눠 쓰는 모양, 한 원소 안에서 공유된 부분은 (마디, 부분)마다 한 번), `Sort`의 사전 검사와 `min`·`max`의 `Best`가 원소(키)마다 `add`, 끝에 `finish`(순서 없는 값이 하나뿐이어도 `x < x`처럼 E0501). 측정(릴리스, 30만 튜플): `sort` 0.22초 그대로, `max` 24 ms → 16 ms(첫 원소와의 비교가 빠짐). 부분을 60겹 공유하는 값의 `sort`도 즉시 끝남. `max([1, "a"])`의 문구가 `sort`처럼 앞 원소부터(`int and str`). `values.md`·`collections.md`(`sort`)·`02-values-and-types.md`, 시험 `ordering_functions_unify_the_shapes_of_all_elements`(24가지 순서 × 넷), 골든 `err_sort_mixed_rows.fung`.

### VA61. 시간 초과로 끊긴 `read_lines`가 읽은 줄을 잃는다 — 버그, 중간 (다시 확인)
- 재현(FIFO):
  ```
  mkfifo ff; (printf 'a\nb\n'; sleep 0.4; printf 'c\n') > ff &
  fung -e 'import fs
  f = fs.open("ff")!
  print(timeout(() => read_lines(f), 100ms))
  print(read_lines(f))'
  → Err({kind: 'timeout, …})
    Ok(["c"])
  ```
  `a`·`b`가 사라졌다. 표준 입력도 같다.
- 근거: `manual/reference/prelude/concurrency.md`(`timeout`) "FIFO는 다르다: 이벤트 루프에서 기다리므로 읽기·쓰기 모두 곧바로 끊기고 바이트를 잃지 않는다". `crates/fung-vm/src/prelude/io.rs:138-140`의 주석도 같은 약속을 한다.
- 원인: `prelude/io.rs` `pipe_lines`(141-160)와 `read_lines_fn`(271-292)이 읽은 줄을 클로저 지역 `got`에 두고, 멈추면 버린다. `read_line`은 괜찮다.
- 고치는 법: 멈출 때 모은 줄을 핸들의 버퍼(표준 입력 대기열) 앞에 되돌린다(`stopped` 콜백 `turn.end()`·`stdin_leave`가 이미 있다). 또는 `read_lines`가 엿보기만 하다가 끝에서 소비한다.
- 함께(문서): 받는 끝에서 모으는 함수(`take(rx, n)`·`map(rx, f)`·`sort(rx)`·`zip`)도 기한에 끊기면 받은 것을 버린다(`timeout(() => take(rx, 2), 20ms)` 뒤 `recv_now(rx)`는 `'empty`). 모으기의 성질이지만 `timeout`에도 `take`에도 적혀 있지 않다. 적거나, 같은 방식으로 되돌린다(결정).
  - 처리: 2026-11-14, 사용자 결정 "그대로 둔다". 되돌리지 않는다. `timeout` 매뉴얼이 받은 값을 잃는다고 적고 있다(결정 문서 3절).
- 처리: 2026-11-14, 커밋 `fix: a read_lines cut by a deadline gives its lines back (VA61)`. `prelude/io.rs`: 모은 줄을 두 클로저가 나눠 쥐고(`Rc<RefCell<…>>`), 멈춤 콜백이 FIFO는 핸들 버퍼 앞에(`File::pipe_unread` → `Pipe::unread`, 읽기 차례를 넘기기 전에), 표준 입력은 대기열 앞에(`Task::stdin_unread` → `Stdin::unread`, 기다리던 태스크를 깨움) 되돌린다. 취소도 같은 길이다. 시험 `a_cut_read_lines_of_a_fifo_keeps_its_lines`(`fs_tests.rs`)·`a_cut_read_lines_of_stdin_keeps_its_lines`(`exit_tests.rs`): 표식 파일로 순서를 정한다. "함께"는 구현을 보고 되돌리기의 선택지를 사용자에게 묻기로 했고(1.1절), 그동안 `concurrency.md`의 `timeout`에 "받는 끝에서 모으는 호출이 끊기면 받은 값은 사라진다"를 적었다.

### VA62. `index_of`가 조각 경계에 걸친 글자 안의 맞음을 놓친다 — 버그, 낮음 (다시 확인, 조용히 틀린 값)
- 재현:
  ```
  s = repeat("a", 131071) + "é" + "zz"
  print("\xa9" in s, index_of(s, "\xa9"), index_of(s, "\xa9z"))   → true nil nil
  ```
  `repeat("a", 65535)`면 `true 65535 65535`. 길이 131072·k − 1마다 실패한다.
- 근거: `manual/reference/prelude/strings.md`(`index_of`) "`sub in s`가 참이면 `index_of(s, sub)`는 `nil`이 아니다"(D107).
- 원인: `crates/fung-vm/src/prelude/strings.rs` `index_of_fn`의 `Find::step`(411-433). 맞음이 없는 조각 뒤에 세는 고리가 `self.at`을 글자 경계로 옮기며 `stop`을 최대 3바이트 넘고, 다음 찾기가 `self.at`에서 시작한다.
- 고치는 법: 찾는 자리(`stop`)와 센 자리(`at`)를 따로 둔다. `m < at`에서 맞으면 `chars - 1`. 393-398행의 주석이 D107 전의 규칙을 적고 있으니 고친다. VA70과 함께 한다.
- 처리: 2026-11-14, 커밋 `fix: index_of finds a match inside a character on a piece boundary (VA62, VA70)`. `prelude/strings.rs` `Find`가 찾은 자리(`from`, 아무 바이트)와 센 자리(`at`, 글자 경계)를 따로 둔다. 조각마다 찾기만 하고, 맞음이 없으면 조각 끝을 품은 글자의 시작까지(`char_start`: 이어짐 바이트가 아닌 바이트는 글자를 시작하고, 올바른 글자의 이어짐 바이트는 셋까지) 묶음 세기(`text::char_count`)로 센다. 맞으면 맞음이 시작하는 글자의 시작까지 세어 그 인덱스를 돌려준다(D107). 함수 주석을 D107대로 고침. 시험 `index_of_finds_across_pieces`(경계 131071·262143 등, 잘못된 바이트 13만 개 뒤의 맞음).

### VA63. `iterate`·`until`은 문장으로 버려도 E0234가 아니다 — 버그, 낮음 (다시 확인, D90의 누락)
- 재현: `iterate(0, x => x + 1, 3)`과 `until(0, x => x + 1, x => x > 3)`을 문장으로 써도 통과한다. `fold(0..3, 0, (a, _) => a + 1)` 문장은 E0234다.
- 근거: `blocks.md` "함수를 받아 값을 계산하는 내장 함수(`map`·`filter`·`fold`·…)도 같다". `iterate(x, f, n)`은 `fold(0..n, x, (a, _) => f(a))`다.
- 원인: `crates/fung-vm/src/vm/atom.rs:401` `CALLING_FUNG = ["exec", "sh", "iterate", "until"]`가 둘을 효과 있는 쪽에 둔다.
- 고치는 법: 둘을 `PURE_FUNG`로 옮기고 `blocks.md`의 목록에 더한다. 골든 `tests/lang/err_discard_map.fung` 옆에 사례.
- 처리: 2026-11-14, 커밋 `fix: iterate and until as statements are E0234 (VA63)`. `vm/atom.rs`의 `PURE_FUNG`로 옮겨 명세가 `pure`로 표시하고(`spec/builtins.txt`), E0234의 도움말이 `map`처럼 "함수로 값을 계산한다"를 적는다(`resolve/value.rs` `RUNS_A_FUNCTION`). `CALLING_FUNG`에는 명령을 돌리는 `exec`·`sh`만 남는다. `swap` 안의 E0707은 실행 중 효과 내장 함수로 판정하므로 영향이 없다. `blocks.md`·`01-syntax.md`, 시험 `err_discard_iterate.fung`·`err_discard_until.fung`.

### VA64. `parse_float`가 정수 범위를 넘는 정수 꼴을 거절한다 — 버그, 낮음 (다시 확인)
- 재현:
  ```
  print(parse_float("10000000000000000000"), parse_float("1e19"))
  → Err({kind: 'parse, message: "not a number: … (an int holds -9223372036854775808 to 9223372036854775807)"}) Ok(10000000000000000000.0)
  ```
  `float("10000000000000000000")`은 E0513. u64 크기의 계수(cgroup 파일의 `18446744073709551615`)가 걸린다.
- 근거: `manual/reference/prelude/convert.md`(`parse_float`) "정수 꼴(`"3"`)은 `Ok(3.0)`이다". 실수로 읽히는지가 정수에 담기는지에 달려서는 안 되고, 실수를 물은 사람에게 정수의 범위를 말한다.
- 원인: `crates/fung-vm/src/prelude/convert.rs:88-98` `parse_float`의 정수 꼴 갈래가 `literal(s, Kind::Int)`를 거친다.
- 고치는 법: 몸통이 숫자와 `_`뿐이면 정수 렉서를 거치지 않고 바로 `f64`로 바꾼다(유한 검사는 실수 리터럴과 같게). 정확히 옮겨지지 않는 정수 꼴(2^53 초과)을 E0523의 뜻에 맞춰 거절할지는 함께 정한다(추천: 실수 리터럴 `10000000000000000001.0`과 같게 받는다).
- 처리: 2026-11-14, 커밋 `fix: parse_float reads integer-shaped text as a float literal (VA64)`. 1.1절대로: `prelude/convert.rs` `parse_float`이 부호 뒤가 숫자와 `_`뿐이면 `.0`을 붙여 실수 리터럴로 렉싱한다(앞의 0·`_` 규칙과 범위·유한 검사가 리터럴과 같고, 도움말도 실수의 것: `"007"`은 "a number does not start with 0", `"1"+0×400`은 "a float holds up to about 1.8e308"). 2^53 너머는 가장 가까운 실수. 리터럴을 따르므로 `"-0"`은 `-0.0`이 된다(전에는 정수 0을 거쳐 `0.0`). `convert.md`(`float`·`parse_float`)·`06-native-and-stdlib.md`, 시험 `integer_shaped_text_is_a_float_literal`.

### VA65. `zip`의 "필요한 만큼만"에 범위 예외가 있고 왼쪽의 넓은 범위는 E0503이다 — 결정·버그, 낮음 (2 다시 확인, D105의 남은 예외)
- 재현 1(3, 1, 2를 담고 닫힌 채널 `rx`):
  ```
  zip(rx, [1])     → [(3, 1)], 그 뒤 recv(rx) = Ok(1)          (나머지가 남는다)
  zip(rx, 0..1)    → [(3, 0)], 그 뒤 recv(rx) = Err('closed)   (1과 2를 버린다)
  ```
  매뉴얼이 예외로 적는다(`collections.md` `zip`: "다른 쪽이 범위나 받는 끝이면 닫힐 때까지 받는다"). 바로 옆에서 권하는 "번호를 붙일 때는 범위와 짝짓는다"가 버리는 쪽이다. 열린 채널이면 `zip(0..3, rx)`는 돌아오지 않는다.
- 재현 2:
  ```
  zip([1], -9223372036854775807..9223372036854775807)   → [(1, -9223372036854775807)]
  zip(-9223372036854775807..9223372036854775807, [1])   → E0503 integer overflow
  ```
  매뉴얼: "아주 큰 범위와 짝지어도 된다".
- 원인: `extras/prelude.fung` `_zip_side`(42-46)·`zip`(49-55). `left`가 범위인 채로 `len(left)`를 부른다.
- 고치는 법(결정):
  - (가) **추천**: 받는 끝은 다른 쪽이 무엇이든 짝지을 수 있는 만큼만 받는다. 범위는 개수가 있다(리스트 한도로 자른 개수, `_r.slice`가 이미 i128로 자른다). 받는 끝 둘이면 번갈아 받고 처음 닫힌 데서 멈춘다. 예외 문장이 사라지고 재현 2도 함께 고쳐진다.
  - (나) 재현 2만 고친다.
- 처리: 2026-11-14, 결정 D119((가)), 커밋 `feat!: zip takes from a receiving end only what it pairs (D119)`. `extras/prelude.fung`의 `zip`을 `_zip_count`(리스트는 길이, 범위는 `_r.slice(r, 0, 정수 최댓값)`의 길이, 받는 끝은 `nil`; 다른 타입은 받기 전에 E0506)·`_zip_take`·`_zip_ends`(받는 끝 둘을 번갈아)로 다시 썼다(`_zip_side`·`_upto` 삭제).
  `collections.md`(`zip`, 첫머리의 "일찍 멈추는 함수")·`06-native-and-stdlib.md`, 시험 `tests/lang/zip_receiver.fung`(범위 옆, 열린 채널, 받는 끝 둘, 넓은 범위 양쪽)·`prelude_in_fung_checks_like_a_native`.

### VA66. 빈 찾을 글을 받는 함수와 거절하는 함수가 갈린다 — 결정, 낮음 (다시 확인)
- 재현: `print("" in "abc", starts_with("abc", ""), ends_with("abc", ""))` → `true true true`. `index_of("abc", "")` → `E0403 argument sub must be a non-empty str`.
- 근거: D18·D74가 "어디에나 맞는" 빈 문자열을 `split`·`replace`·`trim`에서 에러로 만들었고 `index_of`가 따랐다. 세 술어는 여전히 "예"다. 흔한 원인은 비어 있는 변수다(`starts_with(line, prefix)`의 `prefix == ""`가 모든 줄에 맞음). `strings.md`가 `index_of`에는 "빈 `sub`는 어디에나 있으므로 에러다", `starts_with`에는 "빈 접두사는 언제나 참이다"라고 반대로 적는다.
- 고치는 법(결정):
  - (가) **추천**: `starts_with`·`ends_with`·문자열의 `in`도 `""`를 거절한다. 규칙 하나: "빈 찾을 글은 E0403".
  - (나) `index_of(s, "")`를 `0`으로(찾기는 모두 받고, 결과가 없는 `split`·`replace`·`trim`만 에러).
  - (다) 그대로 두고 갈린 규칙을 `strings.md`에 한 번 적는다.
- 처리: 2026-11-14, 결정 D118((가)), 커밋 `feat!: an empty search text is E0403 for starts_with, ends_with and in (D118)`. `prelude/strings.rs`의 `starts_with`·`ends_with`와 `value/index.rs` `contains`(문자열 갈래, 이름 `x`)가 `index_of`·`split`과 같은 `not_nonempty`(이제 `value::index`에 하나, `nat`이 감쌈)로 거절한다.
  `strings.md`(`starts_with`·`ends_with`, 예제의 `starts_with("a", "")`)·`collections.md` `contains`·`operators.md`·`06-native-and-stdlib.md`, 시험 `an_empty_search_text_is_refused_everywhere`·`run_cases.rs`·`tests/lang/err_empty_search.fung`.

### VA67. `dur / dur`만 2^53 넘는 몫을 말없이 깎는다 — 결정, 낮음 (다시 확인)
- 재현:
  ```
  ns = 1ms / 1000000
  d = 5000h + ns
  print(d / ns, d // ns, d / ns == d // ns)   → 18000000000000000.0 18000000000000001 false
  ```
  `d * 1.0`과 `18000000000000001 / 1`은 E0523이다.
- 근거: `operators.md`의 E0523 규칙(D48, VA41·VA42): 실수로 정확히 옮겨지지 않는 정수는 말없이 깎지 않는다.
- 원인: `crates/fung-vm/src/value/arith.rs:269-275` `dur_ratio`.
- 고치는 법(결정):
  - (가) **추천**: 몫의 정수부가 정확히 옮겨지지 않으면 E0523(정수 개수는 `//`).
  - (나) 그대로 두고(결과는 참값의 올바른 반올림) 명세에 적는다.
- 함께 볼 것: `1s * 0.0000000001`은 `0s`다(나노초로 반올림). 리터럴은 "0이 아닌 성분이 0 ns로 반올림되면 E0105"인데 산술은 조용하다. 실수 곱의 반올림은 실수의 성질이라 그대로가 추천이지만, `lexical.md`에 "기간의 산술은 나노초로 반올림한다"를 적는다.
- 처리(D135, VA42 재검토; VA67 자체는 D120으로 문서만 고친다): 2026-11-14, 커밋 `feat!: a duration times or over a float is exact and rounded once (D135)`. `value/dur.rs`의 `mul_float`·`div_float`가 실수를 `m × 2^e`로 풀어 나노초 정수와 128비트로 곱하거나(자리씩 나누기로) 나눈 뒤 한 번 반올림한다(반은 0에서 먼 쪽). VA42의 E0523이 사라졌다. 넘침 E0503, `0.0`·`-0.0`으로 나누기 E0502.
  `lexical.md`에 "기간의 산술은 정확히 계산하고 결과를 나노초로 한 번 반올림한다"와 예제, `07-operators-and-types.md`, 시험 `duration_and_float_arithmetic_is_exact`(경계와 i128 기준값 2만 개)·`small_value_fixes`·`tests/lang/dur_numbers.fung`.
- 처리(VA67 자체): 2026-11-14, D120(바꾸지 않음). `기간 / 기간`은 나노초 정수로 정확히 나눈 뒤 한 번 반올림한 실수다. 피연산자를 실수로 바꾸지 않으므로 E0523이 없다(D135의 기간 산술 규칙과 한 규칙). `lexical.md`에 적었다.

### VA68. 비ASCII 문자열의 `s[i]`가 앞으로 가는 순회가 아니면 O(i)다 — 성능, 중간 (다시 확인)
- 측정(`repeat("한", n)`):

  | 접근 | n | 시간 |
  |---|---|---|
  | 앞으로 `s[i]` | 40만 | 63 ms |
  | 뒤로 `s[n-1-i]` | 10만 | 2.5 s (ASCII는 0.01 s) |
  | 뒤로 `s[n-1-i]` | 40만 | 40.4 s |
  | 회문 검사 `all(0..n//2, i => s[i] == s[n-1-i])` | 20만 | 7.3 s |

  큰 글에 한글 한 글자가 들면 밀리초가 수십 초가 된다.
- 원인: `crates/fung-vm/src/value/text.rs:185` `offset_of`. 커서(11-11 VA35)는 `n >= k`일 때만 돕고, 아니면 0번 바이트부터 다시 센다.
- 고치는 법: 게으르게 만드는 표본 오프셋 표(64글자마다의 바이트 자리, 메모리 약 2%, 처음으로 순서를 벗어난 접근에서 만든다). 적어도 커서에서 뒤로 걷는다. VA69의 구조 변경과 함께 설계한다.
- 처리: 2026-11-14, 커밋 `perf: a non-ASCII string finds a character from the nearest known place (VA68)`. `value/text.rs`의 `offset_of`가 커서에서 앞으로(64글자 안쪽이면 곧바로), 커서에서 뒤로·끝에서 거꾸로(유효한 UTF-8), 처음부터 가운데 가장 가까운 곳에서 걷는다. 가장 가까운 곳도 64글자(`STRIDE`)보다 멀고 커서보다 뒤인 접근이 처음 오면 64글자마다의 바이트 자리 표(`marks`, 한글이면 바이트의 약 4%)를 한 번 만들고, 그 뒤로는 표에서 걷는다. 앞으로 읽기만 하는 고리는 표를 만들지 않는다. 덧붙이면(`append`) 표를 버린다. 자르기의 끝 자리도 같은 길이라 끝에서 먹는 고리(`s[..len(s) - 1]`)가 선형이다.
  `index_of`가 `in`의 20배이던 것(VA70 뒤 남은 것)은 원인이 달랐다: 맞음이 없어도 조각마다 글자를 `char_count`(UTF-8 검사 + 디코딩)로 셌다. 싼 고침이라 함께: 유효한 UTF-8이면 `str::chars().count()`로 센다(`prelude/strings.rs` `count_to`). 1.5 MB 한글에서 0.90 → 0.10 ms(`in` 0.044 ms).
  시험: `value/text.rs` `lookups_in_any_order_agree_with_a_walk_from_the_start`(UTF-8·보기·UTF-8이 아닌 바이트에서 앞·뒤·건너뛰기·지그재그 순서의 자리가 처음부터 걸은 것과 같고, 표가 만들어짐), `perf.rs` "eat a string from its end"·"index a non-ASCII string backwards and both ways"·"search non-ASCII text that has no match". 측정(릴리스, 중앙값): 거꾸로 10만 2.33 → 0.023 s, 40만 0.072 s, 회문 20만 6.9 → 0.039 s, 앞으로 40만 0.066 → 0.071 s(`docs/notes/bench-languages.md` 4.13절). 매뉴얼 VM 10장, 설계 02.

### VA69. 슬라이스로 문자열을 소비하는 재귀가 이차 시간이다 — 성능, 중간 (다시 확인)
- 측정: `fn eat(s, acc) = s == "" then acc else eat(s[1..], acc + 1)`

  | 입력 | n | 시간 |
  |---|---|---|
  | ASCII | 5만 | 26 ms |
  | ASCII | 20만 | 1.0 s |
  | `"한"` 반복 | 20만 | 13.3 s |
  | 리스트, 같은 고리 | 20만 | 0.04 s |

  앞에 붙이기 `"x" + acc`도 20만에 820 ms다(리스트 `[x] + acc`는 L5 뒤로 상각 O(1)). `for.md`의 곁글은 리스트의 꼬리 떼기를 선형이라고 적고 문자열은 "앞에 붙여 쌓기"만 경고한다.
- 원인: `crates/fung-vm/src/value/index.rs:268-286` `slice`가 바이트를 복사하고, `Text::from`이 ASCII인지 다시 훑고, 다음 `slice_bounds`가 `char_len()`으로 글자를 다시 센다(비ASCII의 13배가 이 다시 세기다).
- 고치는 법:
  - 싼 부분: 새 글의 글자 수를 `hi - lo`로, ASCII 표시를 부모에서 물려준다. 다시 세기는 없어지고 복사는 남는다(여전히 이차).
  - **추천(구조)**: `Text`를 보기(`Rc` 버퍼 + 바이트 범위)로 만들어 `s[a..b]`를 O(1)로. 머리·꼬리 재귀는 이 언어의 관용적인 고리이고 문자열만 벌을 받는다. 단, VA59와 같은 함정(작은 조각이 큰 버퍼를 붙잡음)이 생긴다: 문자열은 핸들을 품지 않으므로 수명의 뜻은 깨지지 않지만 메모리는 붙잡힌다. 조각이 버퍼의 일정 비율 아래면 복사하는 규칙을 함께 정한다.
- 처리: 2026-11-14, 커밋 `perf: a big slice of a string is a view of its bytes (VA69)`. 추천(구조)대로. `value/text.rs`의 `Repr::View { root, lo, hi }`(뿌리는 늘 자기 바이트를 가진 UTF-8, 보기의 보기는 뿌리를 바로 가리킴)와 `Text::slice`: 64바이트(`SMALL_PIECE`) 이하이거나 뿌리 바이트의 4분의 1(`KEEP`)보다 작은 조각은 복사하고, 나머지는 O(1) 보기. 조각의 글자 수는 `hi - lo`, ASCII 여부는 세 값(ASCII·아님·모름)으로 원본이 ASCII면 물려받고 아니면 `is_ascii`가 처음 물을 때 한 번 본다. 보기에 제자리로 덧붙이면(`append`) 먼저 자기 바이트를 갖는다. 바이트를 보는 곳(해시·비교·`as_bytes`·`into_bytes`·`as_str`)은 모두 `as_bytes`를 거치므로 그대로이고, 문자열은 핸들을 품지 않아 `send`·해제 조각·수명 검사와 무관하다. `index.rs`의 자르기는 끝 자리를 `offset_of(hi)`로(끝이면 길이) 찾는다.
  시험: `value/text.rs`의 `a_big_piece_is_a_view_and_a_small_one_a_copy`·`a_view_grows_as_a_string_of_its_own`, `perf.rs` "eat a string by slices"(ASCII 20만·한글 10만, 고치기 전 4.1 s). 측정(릴리스, 5번 중앙값): ASCII 20만 0.95 → 0.03 s, 한글 20만 12.8 → 0.03 s(`docs/notes/bench-languages.md` 4.13절). 앞에 붙이기 `"x" + acc`는 그대로(0.87 s): 보기로는 풀리지 않아 이번 범위 밖이다. 매뉴얼 VM 10장.

### VA70. `index_of`가 같은 찾기의 `in`보다 150배 느리다 — 성능, 낮음
- 측정(맞음 없음, ASCII): 590 KB에서 `index_of(big, "zz")` 3.29 ms, `"zz" in big` 0.022 ms. 2.36 MB에서 14.7 ms 대 0.080 ms.
- 원인: `crates/fung-vm/src/prelude/strings.rs` `Find::step` 421-425가 맞음이 없는 조각에서도 글자마다 `text::first`로 풀어 센다.
- 고치는 법: 묶음 세기(`char_count`, `join`·`repeat`가 쓰는 것)로 세거나 맞음을 찾은 뒤에만 센다. VA62와 함께.
- 처리: 2026-11-14, VA62와 같은 커밋. 측정(릴리스, 맞음 없음, 20번 평균): 590 KB ASCII 3.29 ms → 0.041 ms(`in` 0.017 ms), 2.36 MB 12.3 ms → 0.16 ms(`in` 0.069 ms), 1.5 MB 한글 4.15 ms → 0.87 ms(`in` 0.041 ms; 한글은 `str::chars().count()`의 셈이 남는다).

### VA71. prelude의 작은 어긋남 — 결정·문서, 낮음 (2 다시 확인)
1. 환경 변수 이름의 NUL: `env("A\x00")`는 E0403인데 `exec(["true"], env: {"A\x00": "x"})`는 `Err({kind: 'invalid, …})`다. D76은 두 이름 검사가 같다고 했다. `crates/fung-vm/src/native/process.rs:101`은 NUL을 보고 `:168`은 안 본다. 추천: 이름은 둘 다 E0403, 값은 `'invalid` 그대로.
   처리: 2026-11-14, 결정 D132, 커밋 `feat!: a NUL in an env name is E0403 for exec and sh too (D132)`. `native/process.rs`에 검사 하나(`var_name`)와 조건 문구 하나(`VAR_NAME`, `ENV_WANT`가 같은 조건)를 두고 `env`와 `exec`·`sh`의 `env`가 쓴다. `process.md`·`06-native-and-stdlib.md`, 시험 `variable_names_are_checked_alike`·`run_cases.rs`·`tests/lang/err_exec_env_name.fung`.
2. `sh("")`·`sh("   ")`가 `Ok({out: "", code: 0, …})`다. `exec([])`·`exec([""])`는 E0403이다(VA58-1). 추천: 비었거나 공백뿐인 `cmd`는 E0403.
   처리: 2026-11-14, D133(바꾸지 않음). `sh`는 문자열을 셸에 넘기고 뜻은 셸이 정한다(빈 명령은 셸에서 아무 일도 아니다). `process.md`의 `sh`에 적었다.
3. `split`이 원소 100,000,001개의 리스트를 만들 수 있다(한도보다 하나 많음). `strings.rs` `Parts::step`(213-221)이 `nat::check_len` 없이 넣는다. `collections.md:37`은 리스트를 만드는 모든 길이 한도를 지킨다고 적는다. 재현 안 함(코드 읽기, 수 GB 필요).
   처리: 2026-11-14, 커밋 `fix: split, lines and chars check the list limit (VA71)`. `prelude/strings.rs`의 `Parts`(`split`·`lines`)와 `chars`가 한 걸음마다 `nat::check_len`으로 길이를 본다(리스트는 늘기만 하므로 한 걸음 끝에서 넘었으면 끝내 넘는다). 시험은 원소 1억 개가 필요해(수 GB) 만들지 못했다: 코드와 `nat::check_len` 시험으로만 지킨다. `collections.md`의 한도 문장에 `split`·`lines`·`chars`를 더했다.
4. `first(rx)`·`take(rx, n)`·`zip(rx, …)`이 문장으로는 E0234("computed and discarded"), `swap` 안에서는 E0707("has an effect")이다. VA47에서 일부러 정한 것이지만, 메시지 둘을 건너뛰려면 `_ = take(rx, 2)`가 필요한 까닭을 `blocks.md`나 `collections.md`에 한 줄 적는다.
   처리: 2026-11-14, 문서만. `blocks.md`의 버려지는 자리 조항에 "받는 끝에서 값을 꺼내는 `first`·`take`·`zip`도 문장으로는 E0234, 일부러 버리려면 `_ = take(rx, 2)`, `swap` 안에서는 E0707"을 적었다(실행으로 확인).
5. 기간과 수의 연산이 `*`·`/`만 되고 `//`·`%`는 안 된다(`1s // 2`는 E0501, `1s / 2`는 `500ms`). `values.md`의 표대로이고 기간의 `/` 수는 이미 나노초로 반올림하므로 `//`가 따로 할 일이 없다. 바꾸지 않되 `lexical.md`의 기간 절에 "수로 나누는 것은 `/`뿐"을 적는다.
   처리: 2026-11-14, 문서만(D 결정 1.1절). `lexical.md`의 기간 절에 "기간을 수로 나누는 것은 `/`뿐"과 `1s // 2`·`1s % 2`의 E0501을 적었다(실행으로 확인).

## 8. 시험했지만 문제가 없던 것

- **앞단의 견고함**: 무작위·변이 입력 14,000개(토큰 뒤섞기, `tests/lang`·`examples`·`extras`의 변이)를 `check`·`fmt`로: 패닉·E0801·시간 초과 없음. 중첩 31가지 모양을 2천·9천·2만·20만 겹으로: E0115이거나 정상. 폭 사례 약 45개(2만·8만 항)는 VM51 말고 선형.
- **문법**: `grammar.md`의 모든 생성 규칙(`fn`·매개변수·나머지·기본값, 람다, 익명 `fn`, `return` 자리, `then … else`, 비교 연쇄, 범위와 슬라이스, 파이프, 부분 적용, 태그, `match` 절과 가드, 구조 분해, 타입 표기와 별칭, import, `test`). 괄호 밖의 줄 이어짐 규칙 1·2와 E0136.
- **렉서**: BOM, CR, 원시 문자열의 CRLF, 양방향 제어 문자, 보이지 않는 글자, NFC 이름, Lt·Nl 이름, 비UTF-8 입력, 숫자의 모든 꼴과 64비트 경계, 기간, 이스케이프, `$` 규칙, 겹친 보간, 에러의 칸 위치.
- **resolve**: E0204·E0205·E0206·E0208·E0238·E0239·E0240·E0308·E0309를 모든 묶는 방법에서. 인터페이스 이름. 대상이 정해진 호출의 개수·이름 검사(최상위 `fn`, 지역 `fn`, prelude, 모듈 멤버, 파이프, 부분 적용). E0223·E0224의 값 자리.
- **수명 분석**: Python 기준 해석기를 붙인 무작위 프로그램 66,000개(`?`·`!`·패턴·가드 대안·기본값과 이름 인자·보간·파이프·`for`·지역 `fn`·바로 부르는 람다): 틀린 값·E0801 없음. 보내는 끝·별칭·클로저와 `recv_now` 탐침으로 놓는 시점을 본 약 60,000개: SP72의 죽은 코드 말고 어긋남 없음.
- **상수 접기와 실행**: 연산자·리터럴 15,246쌍(i64 한계, 2^53, `-0.0`, `1e308`, 바이트, 기간, 섞인 타입): 값과 에러 번호가 같다.
- **꼬리 호출**: `then`·가드 있는 `match` 절·`return f()`·`??`·`and`·`or`·펼침·서로 부르는 `-> int`로 30만 깊이.
- **명령 표와 검증기**: 명령 59가지가 매뉴얼의 묶음과 맞는다. 검증기의 여섯 검사를 `vm.rs`·`m4.rs`·`call.rs`가 믿는 것과 대조.
- **산술**: 모든 연산자의 정수 넘침(`MIN // -1`, `-MIN`, `(-2) ** 63`), E0523·E0524, NaN·Inf 막기, `-0.0`, 실수 `//`·`%`의 부호, 2^53 너머의 정수·실수 `==`·`<`, 기간과 시각의 넘침.
- **같음·해시·순서**: `a == b`면 `unique`의 한 칸(정수·실수, 범위, 시간대가 다른 시각, 넣은 순서가 다른 맵, 공유 구조). `<` 자체의 D87.
- **표현 독립**: 여섯 방법으로 만든 리스트를 23개 크기(32·1024·32768 경계)에서 `==`·`<`·`str`·해시·인덱스·슬라이스·`in`으로 견줌: 같다(수명만 VA59). 맵 18개 크기(16칸 경계)의 넣은 순서.
- **값의 표기**: 116개 값(`$`·제어 문자·보이지 않는 글자·비UTF-8 바이트·실수·기간·맵 키·태그·범위)이 소스로 되읽힌다(SP77만 빼고).
- **prelude**: 모든 함수의 틀린 타입 인자(E0403·E0506·E0513·E0514의 갈래가 문서대로), 경계 값, 전 범위의 정수 범위, 한도(E0806), 콜백 중의 에러·이른 `return`·겹친 호출, 단락 평가, 받는 끝을 입력으로, `swap`의 순수성(E0707·E0708), 결과값의 모양(D65), 10만 대 40만 원소의 선형성(VA70만 예외), 기한이 약 1 ms 안에 닿는 것.
