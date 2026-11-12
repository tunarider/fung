# 언어·VM 전체 점검 결과와 작업 목록 (2026-11-11)

두 번째 전체 점검(`docs/plans/2026-11-10-full-review.md`, 74개 모두 처리)을 고친 직후, 언어 명세와 구현 전체를 다시 점검한 결과다.
이번 점검의 기준은 둘이다: **언어 명세가 기조(일관성 중시·예외 최소화, 엄격함 우선)를 따르는가**, **바이트코드 VM 구현의 문제와 비효율적인 구조**.
여기에 언어 개발자가 함께 봐야 하는 것(진단 품질, 도구, 시험 전략, 자원 한도, CI)을 더했다.
형식은 지난 두 점검과 같다. 새 세션이 이 문서만 읽고 항목 하나(또는 한 묶음)를 이어서 고칠 수 있게 썼다.
항목을 처리하면 `처리:` 줄에 커밋과 바꾼 문서를 적는다.

**상태 (2026-11-12)**: 115개 항목을 모두 처리했다(작업 브랜치 `claude/language-bytecode-vm-review-4cf7c7`의 W1~W12 커밋, main에 스쿼시하지 않음, 미릴리스). 결정은 `docs/plans/2026-11-11-full-review-fixes.md`의 D35~D84, 항목마다 `처리:` 줄. 처리하며 남긴 것은 open-questions "남은 일"의 "세 번째 전체 점검을 고치며 남은 것".

## 0. 이 문서를 쓰는 법

- **기준 커밋**: `276b0a0f`(`feat!: fix the second full review`). 줄 번호는 이 커밋 기준이고 함수 이름을 함께 적었다.
- **방법**: 일곱 영역(명세, 앞단, 컴파일러·바이트코드, VM 코어·값, 동시성 런타임, 표준 라이브러리, 도구·진단·시험)을 따로 점검하고 주장을 릴리스 바이너리로 재현했다.
  영역 사이에 겹친 항목은 하나로 합쳤다(제목 끝에 "… 합침"). 영역별 점검이 낸 133개가 합쳐서 115개가 되었다.
  - **"(다시 확인)"** 표시는 영역 점검과 별도로 종합 단계에서 한 번 더 돌려 같은 결과를 본 항목이다. 표시가 없는 항목은 영역 점검의 재현 한 번뿐이다.
  - "재현 안 함(코드 읽기)"이라고 적은 것은 실행으로 확인하지 못했다.
  - 모든 실행은 macOS arm64(Apple Silicon)다. **리눅스(epoll 경로)는 한 번도 돌리지 않았다.**
  - 성능 제안의 이득은 빌드를 하지 않아 재지 못했다. 표본 비율이나 "같은 일을 하는 두 fung 프로그램의 차이"로 추정한 값이다.
  - 점검 중 기계에 다른 작업이 돌아 부하가 높았다. 시간은 user 시간 위주로 적었다.
- **뺀 것**: 2026-11-08·2026-11-10 점검의 항목(모두 처리), 결정 D1~D34, open-questions "남은 일"·"알려진 한계"·보류 목록.
  이미 정한 결정이 기조와 부딪친다고 보아 다시 꺼낸 것은 제목에 "(기존 결정 재검토)"라고 적었다.
- **재현**: `cargo build --release --locked` 뒤 저장소 루트에서 `F=target/release/fung`. 여러 줄 소스는 `x.fung`로 저장해 `$F run x.fung`.
  멈출 수 있는 재현은 `perl -e 'alarm 10; exec @ARGV' $F run x.fung`로 시간을 묶는다. 메모리는 `/usr/bin/time -l`의 최대 상주 크기다.
  점검에 쓴 퍼징·검증 스크립트는 세션의 임시 디렉터리에 있었고 저장소에 넣지 않았다(TL9가 그것을 시험으로 옮기자는 항목이다).
- **ID**: 지난 점검에 이어 매긴다: SP28~, FE40~, VM19~, VA28~, CC14~, LB41~, TL9~. 그보다 작은 번호는 지난 점검의 항목이다("11-10 VM18"처럼 적었다).
  SP 명세 · FE 앞단과 진단 · VM 컴파일러와 VM 실행 핵심 · VA 값과 prelude · CC 동시성 · LB 표준 라이브러리 · TL CLI 도구와 시험·CI.
- **분류**: 버그(명세가 정답을 말함) · 결정(언어 동작이 바뀜: 선택지 + 추천으로 사용자에게 물음) · 성능 · 정리(구조·진단 문구) · 문서. 뜻은 지난 문서 0절과 같다.
- **심각도**: 높음(정상적인 쓰임에서 프로그램이 죽거나 틀린 값이 조용히 나오거나 자원이 터짐) · 중간 · 낮음.
- **고칠 때의 공통 절차**: 지난 문서 0절과 CLAUDE.md 그대로다(TDD, 커밋 전 검사 전체, CHANGELOG, 매뉴얼 다시 만들기, 기대 출력 bless 뒤 diff 검토).

## 1. 총평

**깨지는 곳은 없다.** 일곱 영역 어디에서도 Rust 패닉, 세그폴트, 틀린 계산 결과를 만들지 못했다. 돌린 것:

- 앞단: 적대적 프로그램 약 400개, 무작위 토큰 열 6,000개, 저장소 `.fung` 변형 4,800개, 9,000겹 중첩과 300만 항 사슬.
- 컴파일러: 무작위 프로그램 21,300개를 Python 참조 평가기와 견줌(불일치 0), 상수 접기 4,000식, dump에 대한 검증기를 함수 15,535개·명령 61만 개에.
- VM·값: 내장 함수 약 80개에 극단값 65,830회, 영속 벡터·맵을 Python 모델과 차등 시험 600개, 100만 겹 값, `Rc` 고리 만들기 7가지.
- 동시성: 무작위 스트레스 190여 회, 취소 폭풍, 깊이 5만 중첩, 외부 명령·소켓 취소. 멈춤·고아 프로세스·fd 누수 0.
- 라이브러리: 공개 함수 전부를 값 39종으로 97,513회 조합 호출.
- 도구: 변이 퍼징 28,300건, 공백·주석 변이 3,000건. 포매터 멱등성 위반 0, 포맷 전후 `dump core` 차이 0.

두 번의 점검이 흔한 함정을 대부분 에러로 만들었고, 그 결과가 단단하다. 남은 문제는 네 갈래다.

**첫째, 기조를 끝까지 밀지 않은 자리(명세).** 틀린 규칙은 거의 없고, 엄격함이 빠진 자리와 예외 조항이 남은 자리가 있다.
말없이 틀린 값을 내는 것은 `x |> f ?? d` 하나(SP28)이고, 나머지는 여덟 뿌리로 묶인다:
① `??`와 `|>`·`..`의 우선순위(SP28·SP36) ② 음수 인덱스(SP29) ③ 경고 등급과 가리기(SP30·SP45) ④ 버려지는 값의 범위(SP31·SP43·SP44)
⑤ 다른 타입의 `==`와 글에 넣는 값(SP32·SP39·SP40) ⑥ 인자 없는 태그(SP33) ⑦ 람다의 `return`·`?`(SP34·FE41) ⑧ 호출의 정적 검사와 펼치기(SP35·SP38).
하위 호환을 따지지 않는 지금이 프로그램의 뜻을 바꾸는 이 결정들을 내리기 가장 싼 때다.

**둘째, 자원 한도의 구멍.** 한도(E0806·E0115)는 있는데 닿지 않는 길이 있다: 타입 별칭 펼치기(FE40, 26줄로 5 GB), 값 출력(VA28, 200자 에러 문구를 위해 전체를 만듦),
리스트 `+`(VA29), `fs.read`의 64 KB 용량(LB41), 겹친 `fn`(FE60). 넷은 한 줄~한 함수 크기의 고침이다.

**셋째, 실행 스레드 하나라는 구조의 가장자리(동시성).** 핵심(스케줄러·취소 트리·채널·기한)은 단단하고, 문제는 경계다:
FIFO 핸들이 이벤트 루프의 "fd 하나에 기다림 하나"를 지키지 않고(CC14), `print`가 실행 스레드를 막고(CC15), 보호막에서 돌아온 자리가 지난 기한을 보지 않는다(CC16).

**넷째, 값 층과 네이티브 다리의 숨은 비용(VM).** 실행 루프는 이미 다듬어져 있어 더 줄일 여지가 작다. 비용은 그 둘레에 있다:
순회 네이티브의 원소마다 시계 읽기(VA30, `fold` 시간의 37%), 문자열 `+`의 세 번 훑기(VA32), 아톰의 `holds_mutable` 재검사(VA31), 작은 조각의 통째 복사(VA33),
캡처 없는 람다의 할당(VM20), 기본값 매개변수의 머리말(VM21).
구조 쪽은 **바이트코드 검증기가 없다**(VM22)와 **수명 상태를 세 갈래 코드가 손으로 이어 간다**(VM23)가 핵심이다. 지난 VM1·VM14가 모두 뒤쪽의 누락이었다.

**함께 볼 것: 지금의 견고함을 지키는 장치가 없다(TL9).** 이번 점검의 0건은 사람이 스크립트를 돌려 얻었다. 저장소에는 퍼징·속성·차등 시험이 없어, 새 문법을 넣으며 중첩 세기를 빠뜨려도(FE60이 그 예) 실패하는 시험이 없다.

**Python과의 속도**(VM 영역 측정, 3회 중 최소, 초, Python 3.14.3):

| 작업 | fung | python | fung/py |
|---|---|---|---|
| `fib(35)` | 1.60 | 1.11 | 1.44 |
| 합 1e7 (꼬리 재귀 + `fold`) | 0.94 | 1.27 | 0.74 |
| 문자열 (`join` 50만, `split`/`upper`/`join`, `s + "x"` 10만) | 0.59 | 0.31 | 1.9 |
| 맵 30만 (넣기·찾기·지우기) | 0.49 | 0.22 | 2.2 |
| 리스트 100만 (`map`·`filter`·`sum`·`sort_by`) | 0.34 | 0.30 | 1.13 |
| 이진 트리 2^20 (튜플 + `match` 재귀) | 0.35 | 0.20 | 1.75 |

## 2. 추천 작업 순서

1. **결정 없이 고칠 버그와 한도**: FE40(별칭 펼치기), LB41(`fs.read` 용량), VA28(출력 예산), CC16(지난 기한), VA30·VA34(시계 읽기), VA40(prelude 버그 둘), FE42(머리 줄 제어 문자), FE46·FE51·FE57, CC18.
2. **지키는 장치**: VM22(검증기), TL9(고정 씨앗 변이 시험). 뒤의 컴파일러 변경을 싸게 만든다.
3. **뜻이 바뀌는 결정 묶음**(사용자 결정 필요, 1절의 여덟 뿌리): SP28 → SP33·SP34·FE41·SP36(예외 조항이 줄고 고치기 쉬움) → SP29·SP30·SP32·SP39 → SP31·SP35·SP37·SP38. 함께 CC17(통과 함수의 반환 꼴), VM19(가지에 들어설 때 놓기), VA29(리스트 한도).
4. **동시성 경계**: CC14(FIFO 차례 줄; 리눅스에서 먼저 재현), CC15(`print` 비차단).
5. **수명 분석 다시 짜기**: VM23 + VM19 + VM24를 한 작업으로.
6. **성능**: VA32 → VA31 → VA33 → VM20 → VM21, 그다음 작은 코드 생성 개선(VM26~VM30, dump 기대 출력을 한 번에 bless).
7. **라이브러리의 조용한 오동작**: LB42·LB43·LB44, `kind` 정리(LB45·LB48), LB46, 결정 LB47·LB58·LB59·LB60.
8. **진단**: FE43·FE44(매일 만나는 것), 그다음 FE45·FE52·FE53·FE61·VA39.
9. 나머지 낮음과 문서.

## 3. 목록

| ID | 한 줄 | 분류 | 심각도 |
|---|---|---|---|
| SP28 | `x \|> f ?? d`에서 `??`가 먼저 묶여 기본값이 말없이 사라진다 | 결정 | 높음 |
| SP29 | 음수 인덱스가 `xs[i - 1]`을 말없이 맨 끝으로 바꾼다 | 결정 | 중간 |
| SP30 | 쓰지 않는 변수·import, prelude 가리기가 `check`에서만 경고다 | 결정 | 중간 |
| SP31 | 순수 함수 호출을 문장으로 써도 조용하다 | 결정 | 중간 |
| SP32 | 다른 타입의 `==`가 조용히 `false`다 | 결정 | 중간 |
| SP33 | "인자 없는 태그 = 심볼"에서 예외 조항 여섯이 나온다 | 결정 | 중간 |
| SP34 | 람다 안의 `return`·`?` 규칙이 세 갈래다 | 결정 | 중간 |
| SP35 | 컴파일 때 알 수 있는 호출 실수를 `check`가 통과시킨다 | 결정 | 중간 |
| SP36 | 슬라이스 끝에서만 `??`·`\|>`가 `..`보다 먼저 묶인다 | 결정 | 중간 |
| SP37 | 반환 타입을 적으면 꼬리 호출 보장이 달라진다 | 결정 | 중간 |
| SP38 | `...rest`로 받을 수는 있지만 펼쳐 넘길 수 없다 | 결정 | 중간 |
| SP39 | `nil`과 함수 같은 값이 보간·`join`에 말없이 들어간다 | 결정 | 중간 |
| SP40 | `"$name"`은 글자 그대로다 | 결정 | 낮음 |
| SP41 | 정수·실수 혼합 산술이 2^53 넘는 정수를 말없이 깎는다 | 결정 | 낮음 |
| SP42 | `**`의 결과 타입이 지수의 값에 따라 바뀐다 | 결정 | 낮음 |
| SP43 | `return` 뒤의 죽은 코드와 피연산자 자리의 `return` | 결정 | 낮음 |
| SP44 | `match`의 같은 리터럴 절 중복, 마지막 절 자리의 람다 문장 | 결정 | 낮음 |
| SP45 | 가리기 규칙이 묶는 방법마다 다르다 | 결정 | 낮음 |
| SP46 | 타입 표기의 남은 구멍(원소 타입은 검사하지 않음) | 결정 | 낮음 |
| SP47 | 다시 대입 + 복사 포착 + "한 번만 대입" 규칙의 무게 | 결정 | 낮음 |
| SP48 | 범위가 리스트인 자리와 아닌 자리 | 문서·결정 | 낮음 |
| SP49 | 남아 있는 작은 예외 조항들 | 결정·문서 | 낮음 |
| SP50 | 명세가 말하지 않는 것(실수 출력, 리터럴 패턴의 같음 등) | 문서 | 낮음 |
| SP51 | `contains` ≡ `in`, `is_empty` ≡ `len == 0` | 문서 | 낮음 |
| FE40 | 타입 별칭 펼치기가 지수 시간·메모리다 | 버그 | 높음 |
| FE41 | 연산자의 피연산자 자리에서 괄호 없는 람다를 받는다 | 버그 | 중간 |
| FE42 | 에러 머리 줄이 소스의 제어 문자를 그대로 찍는다 | 버그 | 중간 |
| FE43 | 다른 언어 습관의 문법 실수가 모두 맨 E0110이고 한 토큰 뒤를 가리킨다 | 정리(진단) | 중간 |
| FE44 | 닫지 않은 블록의 에러가 여는 자리를 말하지 않는다 | 정리(진단) | 중간 |
| FE45 | 문법 에러는 첫 하나만 보이는데 문서는 "모든 에러" | 문서·결정 | 중간 |
| FE46 | 상수와 같은 이름의 지역 `fn`이 맨 바깥 블록에서 E0205다 | 버그 | 낮음 |
| FE47 | 최상위 `fn` 안에서 가릴 수 있는 바깥 이름이 종류마다 다르다 | 결정 | 낮음 |
| FE48 | 파이프의 평가 순서가 오른쪽 모양에 따라 다르다 | 결정 | 낮음 |
| FE49 | 지역이 상수를 가릴 때 E0204 도움말이 틀린 까닭을 말한다 | 버그(진단) | 낮음 |
| FE50 | 타입의 `[ ]` 안에서 줄바꿈이 공백이 아니다 | 버그 | 낮음 |
| FE51 | `return`의 "값 없음" 판정이 닫는 토큰 일부만 안다 | 버그 | 낮음 |
| FE52 | 구조 분해 대입의 패턴 에러가 버려진다 | 정리(진단) | 낮음 |
| FE53 | 발췌가 줄 전체를 찍는다 | 정리(진단) | 낮음 |
| FE54 | 이름 목록만 가져오는 import도 `as`를 요구한다 | 버그 | 낮음 |
| FE55 | 숨은 이름이 사용자에게 보이고 이름 인자로 통한다 | 버그 | 낮음 |
| FE56 | `1.foo`는 E0102인데 `1.5.foo`는 받는다 | 버그 | 낮음 |
| FE57 | 보간 안의 끝 구분자를 받는다 | 버그 | 낮음 |
| FE58 | CRLF 파일의 원시 문자열에 `\r`이 들어간다 | 결정 | 낮음 |
| FE59 | 주석·문자열 안의 양방향 제어 문자 | 결정 | 낮음 |
| FE60 | 겹친 `fn` 정의는 E0115에 세지 않고 검사가 제곱이다 | 버그·성능 | 낮음 |
| FE61 | 진단의 작은 것들 | 정리(진단) | 낮음 |
| FE62 | 앞단의 구조 | 정리 | 낮음 |
| VM19 | 문장이 없는 자리의 마지막 읽기는 값을 놓지 못한다 | 결정 | 중간 |
| VM20 | 캡처 없는 람다도 평가마다 클로저를 할당한다 | 성능·결정 | 중간 |
| VM21 | 기본값 매개변수마다 호출 때 명령 둘이 돈다 | 성능 | 중간 |
| VM22 | 바이트코드 검증기가 없다 | 정리 | 중간 |
| VM23 | 수명 상태 `held`를 세 갈래 코드가 손으로 이어 간다 | 정리 | 중간 |
| VM24 | 수명 분석이 "문장 수 × 살아 있는 지역 수"다 | 성능 | 낮음 |
| VM25 | 상수가 아닌 중첩 리터럴은 깊이마다 처음부터 다시 걷는다 | 성능 | 낮음 |
| VM26 | `a ?? b`의 성공 길에 `JUMP`가 하나 더 있다 | 성능 | 낮음 |
| VM27 | `JUMPIFTRUE`가 없다 | 성능 | 낮음 |
| VM28 | 점프가 점프·`RETURN`으로 간다 | 성능 | 낮음 |
| VM29 | `match`가 갈래마다 명령 셋이고 패턴을 처음부터 해석한다 | 성능 | 낮음 |
| VM30 | 태그·맵·이름 인자의 이름을 `CONST`로 스택에 올린다 | 성능 | 낮음 |
| VM31 | 접히지 않는 상수식 | 성능 | 낮음 |
| VM32 | 32비트 `Instr`는 한도만 만드는 중간 표현이다 | 정리 | 낮음 |
| VM33 | 명령 하나를 더하려면 표 일곱 곳을 고친다 | 정리 | 낮음 |
| VM34 | 이름 인자가 많은 호출의 묶기가 O(n³)이다 | 성능 | 낮음 |
| VM35 | 명령 해석의 남은 고정 비용 | 성능 | 낮음 |
| VM36 | 숨은 불변식과 전역 표의 `Rc` 고리 | 정리 | 낮음 |
| VA28 | 값 출력이 문자열 한도를 다 만든 뒤에 본다 | 버그 | 높음 |
| VA29 | 리스트 한도가 `+`에는 걸리지 않는다 | 결정 | 중간 |
| VA30 | 순회 네이티브가 람다 원소마다 시계를 읽는다 | 성능 | 중간 |
| VA31 | `holds_mutable`이 작은 컨테이너마다 `Vec`과 `HashSet`을 만든다 | 성능 | 중간 |
| VA32 | 문자열 `+`와 보간이 결과 전체를 세 번 훑는다 | 성능 | 중간 |
| VA33 | 부모의 1/4보다 작은 조각은 통째로 복사된다 | 성능 | 중간 |
| VA34 | `Work` 네이티브가 작은 입력에서도 시계를 읽는다 | 성능 | 낮음 |
| VA35 | 비ASCII 문자열 색인 루프가 O(n²)이다 | 성능 | 낮음 |
| VA36 | `<`·정렬·해시가 공유 구조에서 지수 시간이다 | 버그 | 낮음 |
| VA37 | 작은 리스트·맵의 머리가 큰 변종 크기로 잡힌다 | 성능 | 낮음 |
| VA38 | `Text::new`를 아는 호출자도 탄다 | 정리 | 낮음 |
| VA39 | 런타임 진단 문구의 어긋남 | 정리(진단) | 낮음 |
| VA40 | prelude 버그 둘: `drop(rx, -1)`, `pad_left`의 큰 `n` | 버그 | 낮음 |
| CC14 | FIFO 핸들 하나를 두 태스크가 쓰면 이벤트 루프 불변식이 깨진다 | 버그 | 높음 |
| CC15 | `print`·`eprint`가 실행 스레드에서 막힌다 | 버그·결정 | 중간 |
| CC16 | 보호막·범위 대기에서 돌아온 자리가 지난 기한을 보지 않는다 | 버그 | 중간 |
| CC17 | 통과 함수는 "맨 값 또는 `Err`", `recv`는 `Ok`로 감싼다 | 결정 | 중간 |
| CC18 | REPL: 교착 보고 중 Ctrl-C 한 번에 세션이 끝난다 | 버그 | 낮음 |
| CC19 | "취소·시간 초과의 `Err`는 세지 않는다"가 실제와 다르다 | 문서 | 낮음 |
| CC20 | 레퍼런스의 취소 지점·차례 넘기기 설명이 틀렸다 | 문서 | 낮음 |
| CC21 | 스케줄러의 바퀴마다 드는 작은 비용 | 성능 | 낮음 |
| CC22 | `scope`·`supervise`의 핸들이 본문 밖으로 나온다 | 결정 | 낮음 |
| LB41 | `fs.read`가 문자열마다 64 KB 용량을 쥔 채 돌려준다 | 버그 | 높음 |
| LB42 | `http.serve` 핸들러 응답 맵의 모르는 키를 무시한다 | 결정 | 중간 |
| LB43 | `fs.glob`의 `**`가 말없이 `*`다 | 결정 | 중간 |
| LB44 | `cli.parse`는 모르는 옵션을 거절할 길이 없다 | 결정 | 중간 |
| LB45 | http의 실패 `kind`가 전부 `'network`다 | 결정 | 중간 |
| LB46 | `time.parse`의 `Z` 토큰이 `"Z"`를 읽지 못한다 | 결정 | 중간 |
| LB47 | `close(x)`가 보내는 끝과 파일에서 다르게 군다 | 결정 | 중간 |
| LB48 | `exec`·`sh`와 `fs`의 `kind`가 같은 원인에 다르다 | 결정 | 중간 |
| LB49 | 용도(SRE 스크립트)에 비춰 빠진 함수들 | 결정 | 중간 |
| LB50 | 빈 문자열 인자의 답이 함수마다 다르다 | 결정 | 낮음 |
| LB51 | http 클라이언트가 URL을 말없이 고친다 | 결정·문서 | 낮음 |
| LB52 | 흐름의 끝이 `read_line`은 `Ok(nil)`, `recv`는 `Err('closed)`다 | 문서 | 낮음 |
| LB53 | 바깥 값을 받는데 `Err` 길이 없는 변환 | 문서 | 낮음 |
| LB54 | `env("A=B")`·`env("")`가 `nil`이다 | 결정 | 낮음 |
| LB55 | `flatten`이 리스트가 아닌 원소를 통과시킨다 | 결정 | 낮음 |
| LB56 | `fs`의 같은 경로·파일 인자 | 결정 | 낮음 |
| LB57 | `fs.write_line`이 줄바꿈이 든 `line`을 받는다 | 결정 | 낮음 |
| LB58 | `json.stringify`의 말없는 변환 (기존 결정 재검토) | 결정 | 낮음 |
| LB59 | 서머타임 빈틈의 시각을 밀어서 받는다 (기존 결정 재검토) | 결정 | 낮음 |
| LB60 | `sum`이 문자열·리스트도 잇는다 | 결정 | 낮음 |
| LB61 | 문서와 이름의 작은 어긋남 | 문서 | 낮음 |
| TL9 | 저장소에 퍼징·속성·차등 시험이 없다 | 정리(시험) | 중간 |
| TL10 | CI의 빈 곳 | 정리 | 낮음 |
| TL11 | `fmt`의 자잘한 것 | 버그·문서 | 낮음 |
| TL12 | `fung test`의 자잘한 것 | 결정 | 낮음 |
| TL13 | REPL의 앞 참조 | 문서 | 낮음 |
| TL14 | 앞단의 패닉은 메시지 없는 한 줄이 된다 | 정리 | 낮음 |
| TL15 | E0229·E0518은 매뉴얼 예제만이 지킨다 | 정리(시험) | 낮음 |

합계 115개: 높음 5, 중간 38, 낮음 72.

## 4. 언어 명세 (SP)

원칙은 `docs/design/background.md`의 1(엄격함 우선)·6(같은 일은 한 방법)·7(예외 조항 최소화)이다.

### SP28. `x |> f ?? d`에서 `??`가 `|>`보다 먼저 묶여 기본값이 말없이 사라진다 — 결정, 높음 (다시 확인)
- 재현:
  ```
  print("x" |> env ?? "d")                  → nil      # 기대 "d"
  $ fung dump core -e 'y = "x" |> env ?? "d"'
  (call (coalesce (prelude env) "d") "x")
  print([1,2,3] |> find(x => x > 5) ?? 0)   → E0407 missing argument `f`
  ```
- 근거: `operators.md` "우선순위"(6 `|>`, 7 `??`). E0229가 파이프 끝의 `?`·`!`는 막는데 같은 함정인 `??`만 남았다. "파이프로 구한 뒤 없으면 기본값"은 가장 자연스러운 쓰임이다.
- 고치는 법(결정):
  - (가) **추천**: `??`를 `|>`보다 낮게(비교보다는 높게). `a ?? b |> f`는 `(a ?? b) |> f`로 쓴다.
  - (나) 우선순위는 두고 파이프 오른쪽 맨 위의 `??`를 컴파일 에러로(E0229를 넓힘).
  - (다) 문서만.
- 처리: 이 커밋, 결정 D35(점검 문서의 (가)가 아니라 섞으면 에러): `parser/expr.rs` `infix_loop`가 괄호 없는 `??`가 `|>`의 피연산자면 새 번호 E0127. 명세 `operators.md`(우선순위 예를 괄호로, "경계 사례"), `grammar.md`, 설계 01. 시험 `parser/tests.rs` `coalesce_and_pipe_do_not_mix_without_parentheses`, `tests/lang/err_pipe_coalesce.fung`·`err_coalesce_pipe.fung`.

### SP29. 음수 인덱스가 "하나 모자란 계산"을 말없이 맨 끝으로 바꾼다 (기존 결정 재검토) — 결정, 중간 (다시 확인)
- 재현: `xs = [1, 2, 3]; i = 0; print(xs[i - 1], xs[0..i - 1])` → `3 [1, 2]`.
- 근거: `access.md` "`xs[i]`: 음수는 뒤에서부터", "뒤에서 세는 음수 끝은 슬라이스 꼴에만 있다 … `xs[(2..=-1)]`은 E0522". E0507의 엄격함이 절반 사라지고 "슬라이스 = 같은 끝의 범위 값 색인"에 예외가 붙는다.
- 고치는 법(결정): (가) **추천** 음수 인덱스·끝을 없앤다(`last`, `len(xs) - 1`, `take`·`drop`, 필요하면 `take_last`·`drop_last`). (나) 그대로.
- 처리: 이 커밋, 결정 D37((가), 새 함수 없이): `value/index.rs`의 `norm_index`·문자열 인덱스·`slice_bounds`가 음수를 범위 밖 E0507로(범위 값 색인도 같은 길). 뒤에서 세던 `Text::byte_back`·`text::back`·`utf8_back`을 지움. 명세 `access.md`(경계 사례), 배우기 6·7장, 레퍼런스(`last`·문자열), 에러 E0507·E0404의 예, 레시피 `find-big-files`, VM 08·internals, 설계 01·02·07. 저장소: `extras/prelude.fung`(`last`), `extras/http.fung` 넷, `perf.rs` 셋, `tests/lang` 다섯. 시험 `value/tests.rs`·`m4_tests.rs`, `tests/lang/err_index_negative.fung`·`err_slice_negative_end.fung`·`err_string_index_negative.fung`·`err_range_index_negative.fung`(전 `err_range_index_reversed`).

### SP30. 쓰지 않는 변수·import, prelude 가리기가 `check`·`test`에서만 "경고"다 — 결정, 중간
- 재현:
  ```
  import fs
  fn f(xs):
    totl = len(xs)
    len = 3
    len
  ;;
  print(f([1]))     → run: 3, 아무 말 없음, 종료 0
  $ fung check      → W0101 unused import, W0102 unused variable `totl`, W0103 `len` shadows a prelude function (종료 0)
  ```
  최상위 `fn len(x) = 99`도 받아서 `[1,2,3] |> len`이 99다.
- 근거: 언어 명세에는 경고가 없다(`cli.md`·`functions.md`에만). `modules.md`는 모듈 이름 가리기를 E0308로 막는데 prelude 가리기는 경고다.
- 고치는 법(결정): (가) **추천** 셋 다 컴파일 에러(일부러 쥐는 값은 `_x`), prelude 이름도 다시 묶지 못함. 경고 등급이 사라진다. (나) `run`에서도 경고 출력. (다) 그대로.
- 처리: 이 커밋, 결정 D38: 쓰지 않는 지역 변수·매개변수·import는 새 번호 E0239(`resolve/names.rs` `report_unused`·`report_unused_file`; 진입 파일 최상위도 셈, import된 파일의 공개 이름·최상위 `fn`·`_` 이름은 빼고, 다른 에러가 있으면 내지 않음), prelude 가리기는 E0240. 경고 등급 은퇴(W0101~W0103, `Resolved::warnings`·`loader` 경고·`check`·`test`의 경고 출력 삭제). 명세 `scope.md`(규칙 6·7)·`modules.md`·`functions.md`·`cli.md`, 에러 E0239·E0240·W01xx, 설계 03·04·08. 시험 `tests/lang/err_unused_*.fung`·`unused_exempt.fung`, `resolve/tests.rs` `unused_and_hiding_names`.

### SP31. 순수 함수 호출을 문장으로 써도 조용하다 — 결정, 중간
- 재현:
  ```
  m = {a: 1}
  put(m, "b", 2)
  sort([3, 1])
  print(m)          → {a: 1}
  ```
- 근거: `blocks.md` "버려지는 자리의 식은 무언가를 해야 한다 … 호출 … 은 된다". E0234가 막는 것과 같은 실수다.
- 고치는 법(결정): (가) **추천** 효과가 없다고 알려진 prelude 함수(E0707이 이미 효과 목록을 가짐)의 문장 호출은 E0234. 함수를 받는 `map`·`filter`는 제외. (나) 버려지는 값이 `nil`·`Ok(nil)`이 아니면 런타임 에러(`_ =`가 많이 늘어 비추천). (다) 그대로.

- 처리: 이 커밋, 결정 D39: `vm/atom.rs` `has_no_effect`(효과 표 `PURE`에서 함수를 받는 것·`fail`·`_native.arg` 검사·`_native.range.pad`를 뺀 것)를 `spec/builtins.txt` 줄 끝의 `pure`로 적고(`boot/spec_file.rs`, 별칭 `env = _p.env`도), `builtins.rs` `prelude_pure`·`module_pure`, `Sig::pure`, `resolve/call.rs` `call`이 호출 대상이 컴파일 때 정해지는 호출을 위치로 적어 두면 `resolve/value.rs`가 버려지는 자리의 그 호출을 E0234(``the result of `put(…)` ``, 밑줄은 호출 대상)로. fung으로 쓴 prelude 함수와 사용자 함수는 제외. 명세 `blocks.md`, 에러 E0234, 설계 01. 시험 `builtins.rs` `pure_functions_are_marked`, `tests/lang/err_pure_call_stmt.fung`·`err_pure_call_member.fung`·`pure_call_stmt_ok.fung`.
### SP32. 다른 타입의 `==`가 조용히 `false`다 (기존 결정 재검토) — 결정, 중간
- 재현:
  ```
  print(1 == "1", 1 == true, nil == false, [1] == (1,))   → false false false false
  print("a" in 0..3, "a" in [1])                           → false false
  print(1 in "123")                                        → E0501
  print(nil in {a: 1})                                     → E0514
  ```
- 근거: `values.md` "타입이 다를 때 `false`(숫자끼리 제외). 단 심볼과 문자열은 에러". 스크립트의 바깥 값은 모두 문자열이라 `env("PORT") == 8080`은 심볼·문자열만큼 흔한 조용한 거짓이다. `in`은 컨테이너마다 답이 다르다.
- 고치는 법(결정): (가) **추천** 다른 타입의 `==`는 에러(E0517을 넓힘), 예외는 숫자끼리와 `nil`. (나) 문자열·숫자만 에러로 추가. (다) `in`의 갈림만 맞춤. 걸리는 점: 타입이 섞인 리스트의 `in`·`unique`.

- 처리: 이 커밋, 결정 D40: `value/compare.rs` `walk`가 엄격 모드에서 견주는 쌍마다 `check_types`(같은 `type_of` 이름, 숫자끼리, 한쪽 `nil`은 됨)로 E0517을 내고, E0517의 문구를 ``cannot compare values of different types: %s and %s``로 넓힘(심볼·문자열 쌍은 `sym(s)` 도움말). `in`은 리스트가 `eq`, 범위는 수·`nil` 밖의 값이 E0517(`value/index.rs`), `unique`는 `nil`이 아닌 첫 원소와 맨 타입을 견줌. 패턴은 `equal` 그대로(타입이 다르면 맞지 않음). 태그는 이름마다 다른 타입(`Ok(1) == Err(1)`도 에러). 후속(W7, D40의 뜻대로): 태그 값끼리는 이름이 달라도 한 타입으로 견준다(`Ok(1) == Err(1)`·`Circle(1) == Square(1)`은 `false`, `Ok(1) == 1`은 E0517). 채널의 두 끝은 `type_of`가 `'sender`·`'receiver`로 달라 그대로 에러. `value/compare.rs` `same_type`, 명세 `values.md`·에러 E0517·설계 02, 시험 `tests/lang/err_eq_types_tag.fung`·`value/tests.rs`. 명세 `values.md`·`operators.md`, `collections.md` `unique`, 에러 E0517, 배우기 4장, 설계 02·07. 시험 `value/tests.rs` `eq_refuses_pairs_of_different_types`, `tests/lang/eq_types.fung`·`err_eq_types*.fung`·`err_in_types.fung`·`err_in_range_types.fung`·`err_unique_types.fung`(`sym_str_compare.fung`·`eq_sym_str_pairs.fung` 고침).
### SP33. "인자 없는 태그 = 심볼"에서 예외 조항 여섯이 나온다 — 결정, 중간 (다시 확인)
- 재현:
  ```
  fn f(r) = match r:
    Ok  => "ok"
    Err => "err"
    _   => "?"
  ;;
  print(f(Ok(1)), f(Err(2)))        → ? ?         (에러 없음)
  x = Ok; print(x, is_ok(Ok))       → 'Ok false
  print(Red == 'Red, type_of(Red))  → true 'sym
  print(1 |> Ok)                    → Ok(1)
  print(map([1, 2], Ok))            → E0403 argument `f` must be fn, got sym
  ```
- 근거: `lexical.md` 심볼, `match.md` "괄호 없는 태그 패턴 `Ok`는 심볼 `'Ok`에만 맞고", `types.md`의 `Ok`·`Err` 예외, `operators.md` 파이프의 태그 조항, E0117. 11-10 D2가 "값이 정확히 하나"를 E0236으로 지키는데 0개인 맨 `Ok`만 빠져 있다.
- 고치는 법(결정): (가) **추천(작게)** 식·패턴의 맨 `Ok`·`Err`는 E0236(파이프 대상 `x |> Ok`만 남김). (나) 더해서 표기를 하나로(`'Red` 대문자 심볼 리터럴 금지 또는 그 반대). (다) 맨 태그를 심볼과 다른 "값 0개 태그"로 분리(큰 변경).
- 처리: 이 커밋, 결정 D41((가)와 (나)의 `'Red` 금지): desugar의 맨 `Ok`·`Err`(`BareTag`)와 패턴 파서의 맨 `Ok`·`Err`는 E0236(값 0개), 파이프 대상 `x |> Ok`만 남김. 렉서의 대문자 심볼 `'Red`는 새 번호 E0128(도움말 `Red`). `extras/prelude.fung`의 `is_ok`·`is_err`는 `match`로, `extras/http.fung`의 `type_of(x) == 'Err`는 `is_err`로. 명세 `lexical.md`·`match.md`·`results.md`·`types.md`·`values.md`, 설계 01. 시험 `desugar.rs` `a_bare_result_tag_is_an_error`, `lexer/string.rs` `an_uppercase_symbol_is_written_as_the_tag`, `tests/lang/err_bare_ok.fung`·`err_bare_err_pattern.fung`·`err_symbol_upper.fung`. 남는 것: 값의 표시는 그대로 `'Red`·`'Ok`라 소스로 다시 읽히지 않는다(결정 범위 밖, 보고).

### SP34. 람다 안의 `return`·`?` 규칙이 세 갈래이고 `for.md`의 이유가 맞지 않는다 — 결정, 중간
- 재현:
  ```
  fn g(xs) = find(xs, x => x > 1 then return true else false)
  print(g([1, 2]))                                   → 2      (람다의 return: 허용)
  fn f(xs) = for x in xs: parse_int(x)? ;;           → E0220
  fn f(xs) = map(xs, x => parse_int(x)?)             → E0226
  fn f(xs) = filter(xs, x => parse_int(x)? > 1)      → 실행 중 E0505 expected bool, got Err
  fn f(xs): m = map; m(xs, x => parse_int(x)?) ;;    → [1, Err(…)]  (별칭은 통과)
  ```
- 근거: `functions.md` "`return`은 감싸는 함수나 람다에서 돌아오고". `for.md` "본문이 사실은 람다이므로 `for` 본문 안의 `return`과 `?`는 컴파일 에러다". 람다는 허용이니 이유가 맞지 않고, E0226은 부르는 함수의 이름으로 정하는 특례다.
- 고치는 법(결정): (가) **추천** `=>` 람다 본문에 `return`·`?` 금지(일찍 돌아가려면 익명 `fn(x): … ;;`). `for`의 금지가 설명 그대로 따라 나오고 E0226·E0213·E0220이 한 규칙이 된다. (나) `for.md`의 이유만 고침.
- 처리: 이 커밋, 결정 D42: `FnInfo::lambda`(파서가 `=>` 람다에 표시), `resolve.rs` `early_return`이 람다와 `for` 본문의 `return`·`?`를 새 번호 E0237 하나로. E0213·E0220·E0226 은퇴(카탈로그에 은퇴로 남김), `map_like`·`map_arg`·`pmap_globals` 삭제. 명세 `functions.md`·`for.md`, 설계 03. 시험 `resolve/tests.rs` `for_body_return_and_question_outside_fn`·`question_in_a_lambda_given_to_pmap`, `tests/lang/err_lambda_return.fung`·`err_lambda_question.fung` 등.

### SP35. 컴파일 때 알 수 있는 호출 실수를 `check`가 통과시킨다 — 결정, 중간 (다시 확인; 도구 영역의 같은 항목을 합침)
- 재현:
  ```
  $ fung check -e 'fn add(a, b) = a + b
  print(add(1))
  print(len(1, 2, 3))
  print(add(1, zz: 2))'       → 출력 없음, 종료 0   (실행하면 E0407/E0402/E0406)
  ```
  E0809도 직접 호출 꼴은 컴파일 때 알 수 있다: `fn show() = later; print(show()); later = 5`가 `check` 통과, 실행 중 E0809.
- 근거: `functions.md` "인자 개수가 맞지 않거나 … 모르는 이름이면 런타임 에러"(같은 이름 인자 두 번은 컴파일 에러 E0116). 최상위 `fn`은 다시 대입할 수 없어 대상이 정해진다.
- 고치는 법(결정): (가) **추천** 호출 대상이 컴파일 때 정해지면(가려지지 않은 최상위 `fn`, prelude, `mod.f`) 개수·이름을 컴파일 때 검사(같은 번호). (나) 경고. (다) 그대로. REPL의 재정의와 `...rest`·기본값만 조심하면 된다.
- 처리: 이 커밋, 결정 D43: 새 `resolve/call.rs`가 `bind_in_place`와 같은 차례로 E0402·E0406·E0405·E0407(같은 문구, 호출 위치)을 낸다. 아는 함수는 최상위 `fn`, 이름으로 가져온 멤버, `read = _n.read` 같은 다리 멤버의 별칭(`global_sigs`), prelude(`Builtins::prelude_signature`: `spec/builtins.txt`의 `prelude fung` 줄이 매개변수를 적게 생성기를 고침), 모듈 멤버(`ModuleInfo::sigs`, 로더가 `Resolved::sigs`·`_native` 명세에서 채움). 펼침이 있으면 이름만. REPL(`redefine`)에서는 파일의 최상위 `fn`·가져온 이름을 검사하지 않는다. E0809는 그대로 런타임. E0405도 같은 묶음이라 함께 넣음(결정은 E0402·E0406·E0407을 적음). 명세 `functions.md`, 에러 E04xx 머리말·E0402·E0405~E0407(종료 코드 2), VM 09, 설계 03. 시험 `resolve/tests.rs` `calls_to_known_functions_are_checked_at_compile_time`·`the_repl_checks_only_calls_that_cannot_change`, `tests/lang/err_call_missing_static.fung`·`err_call_count_static.fung`·`err_call_name_static.fung`.

### SP36. 슬라이스 끝에서만 `??`·`|>`가 `..`보다 먼저 묶인다 (11-10 D7 재검토) — 결정, 중간
- 재현:
  ```
  xs = [1, 2, 3, 4]; a = nil
  print(xs[a ?? 0..3])    → [1, 2, 3]    # (a ?? 0)..3
  r = a ?? 0..3; print(r) → 0..3         # a ?? (0..3)
  ```
- 근거: `access.md` "두 끝은 `??`·`|>`까지 받는다 … 그보다 약한 연산자 … E0125다: 다른 자리에서는 `..`가 먼저 묶이므로 뜻이 말없이 바뀌지 않게". 그 근거와 스스로 모순이다.
- 고치는 법(결정): (가) **추천** 슬라이스 끝은 `..`의 피연산자 층(`add`)만, `??`·`|>`도 E0125(괄호). (나) 그대로.
- 처리: 이 커밋, 결정 D36: `parser/expr.rs` `looser_than_a_slice`가 `??`·`|>`도 E0125로. 명세 `access.md`(경계 사례 더함)·`grammar.md`, 설계 01. 시험 `parser/tests.rs` `a_slice_end_cannot_hide_a_looser_operator`, `tests/lang/err_slice_coalesce_end.fung`·`err_slice_pipe_end.fung`, `slice_bounds.fung`를 괄호로.

### SP37. 반환 타입을 적으면 꼬리 호출 보장이 달라진다 — 결정, 중간 (다시 확인)
- 재현:
  ```
  fn a(n) -> int = n == 0 then 0 else b(n - 1)
  fn b(n) -> int = n == 0 then 0 else a(n - 1)
  print(a(300000))      → E0804 stack overflow   ("-> int"를 지우면 0)
  ```
- 근거: `functions.md` "반환 타입을 적은 함수는 자기 자신으로의 꼬리 호출만 프레임을 재사용한다". 문서대로지만 타입 표기가 실행 의미를 바꾼다.
- 고치는 법(결정): (가) **추천** 꼬리 호출이 호출자의 반환 검사를 이어받게 한다. (나) 반환 타입 있는 함수의 꼬리 자리에서 다른 함수를 부르면 컴파일 때 알림. (다) 그대로.
- 처리: 이 커밋, 결정 D44: 컴파일러(`compiler/fn_.rs`)가 반환 타입 있는 함수의 꼬리 호출(자기 자신 `(self)` 제외)을 `TAILCHECK rt` + `TAILCALL`로 낸다. VM은 검사를 프레임의 `Extra::checks`(전의 `tail_from`과 한 상자, 프레임 48바이트 그대로)에 넘기고, 같은 (원형, 위치)면 하나로 합쳐 맨 뒤로 옮긴다. 부르는 것이 프레임이 도는 그 클로저면 넘기지 않는다. 프레임이 돌아올 때(`ret_with`, 꼬리 호출한 이어 가는 내장 함수의 끝 둘) 최근 것부터 검사해 E0404를 그 함수의 반환 위치로. 검증기는 `TAILCHECK` 뒤에 꼬리 호출이 없으면 거절. 명세 `functions.md`("꼬리 호출"), 에러 E0404, VM 08·09, 설계 03·09. 시험 `compiler/tests.rs` `compile_tail_calls`, `bytecode/tests.rs`, `tests/lang/tail_return_type.fung`(서로 부르는 `-> int` 깊이 300000)·`err_tail_return_type.fung`.

### SP38. `...rest`로 받을 수는 있지만 펼쳐 넘길 수 없다 — 결정, 중간
- 재현: `fn log(...xs) = print(...xs)` → E0110 unexpected `...`. `print([0, ...xs])` → E0110. `spec/builtins.txt`에 `apply` 없음.
- 문제: 가변 인자 함수(`print`, `fail`, `path.join`, 사용자 `...rest`)를 감쌀 수 없다.
- 고치는 법(결정): (가) **추천** 호출의 마지막 위치 인자에만 `f(a, ...xs)`(리스트 리터럴에는 두지 않음: `+`와 방법이 둘). (나) `apply(f, xs)`. (다) 가변 인자를 내장 특례로만.
- 처리: 이 커밋, 결정 D45: 파서가 `(spread e)`를 마지막 위치 인자로 읽고(이름 인자 앞), 다른 위치 인자 앞·리스트 리터럴·태그의 값은 새 번호 E0133. 부분 적용은 펼칠 값도 만들 때 한 번 평가(`desugar.rs`). 컴파일러는 `CALLSPREAD`·`TAILCALLSPREAD`, VM `spread_args`가 리스트·범위를 그 자리에 펼치고(범위는 `MAX_ELEMENTS`까지), 다른 값은 E0506 `cannot spread …`. D43은 펼친 호출의 개수를 검사하지 않고 이름만. 명세 `functions.md`·`grammar.md`, 에러 E0133, VM 08·09, 설계 03·09. 시험 `parser/tests.rs` `a_spread_is_the_last_positional_argument`, `compiler/tests.rs`, `tests/lang/spread_args.fung`·`err_spread_value.fung`·`err_spread_not_last.fung`·`err_spread_list.fung`.

### SP39. `nil`과 함수 같은 값이 보간·`join`에 말없이 들어간다 (11-08 SP2 재검토) — 결정, 중간
- 재현: `print("p=${nil} ${print}")` → `p=nil <fn print>`. `print(join([nil, 1, "a"], ","))` → `nil,1,a`. 곧 `"http://${env("HOST")}/"`는 `http://nil/`.
- 근거: `results.md` "결과값은 글에 말없이 넣지 않는다"(E0519). 같은 이유가 `nil`에도 그대로다. 로그 용도는 `print(x)`·`str(x)`가 맡는다.
- 고치는 법(결정): (가) **추천** 보간·`join`이 거절하는 값을 결과값 + `nil` + 핸들(함수·모듈·태스크·채널 끝·atom·파일)로. (나) `nil`만. (다) 그대로.

- 처리: 이 커밋, 결정 D46: `vm/m4.rs` `not_text`가 보간(`Op::Concat`)과 `join`의 값을 명시적 스택으로 훑어(나눠 가진 부분은 한 번) 결과값·`nil`·핸들(함수·모듈·태스크·범위·채널 끝·atom·파일·소켓)을 깊이와 상관없이 E0519로(깊으면 노트). E0519 문구를 ``%s in %s: it is not put into text silently``로 넓힘. 명세 `results.md`, `strings.md` `join`, 에러 E0519, 배우기 6장, 설계 02. 시험 `tests/lang/err_interp_nil.fung`·`err_interp_fn.fung`·`err_interp_deep.fung`·`err_join_nil.fung`·`err_join_deep.fung`.
### SP40. `"$name"`은 글자 그대로다; `$`를 쓰는 방법이 둘 — 결정, 낮음
- 재현: `x = 5; print("$x costs $5 \$x ${x}$")` → `$x costs $5 $x 5$`.
- 고치는 법(결정): (가) **추천** 문자열 안의 맨 `$`는 `${`가 아니면 컴파일 에러(도움말 "`${name}` 또는 `\$`"). (나) `$` 뒤가 식별자 글자일 때만.
- 처리: 이 커밋, 결정 D47((나): `$` 뒤가 이름의 첫 글자일 때만): `lexer/string.rs` `str`이 새 번호 E0129(도움말 `${name}`·`\$name`). 원시 문자열은 그대로. 명세 `lexical.md`, 설계 01. 시험 `lexer/string.rs` `a_dollar_before_a_name_is_an_error`, `tests/lang/err_string_dollar_name.fung`.

### SP41. 정수·실수 혼합 산술이 2^53 넘는 정수를 말없이 깎는다 — 결정, 낮음
- 재현: `print(9007199254740993 + 0.0)` → `9007199254740992.0`. 비교는 정확하다(`9007199254740993 == 9007199254740992.0` → `false`).
- 고치는 법(결정): (가) **추천** 정확히 옮겨지지 않으면 에러(명시적 `float(n)`은 반올림). (나) 혼합 산술 금지. (다) 문서.

- 처리: 이 커밋, 결정 D48: `value/arith.rs` `exact_float`(새 번호 E0523), 상수 접기 `compiler/fold.rs` `exact`는 그런 식을 VM에 맡김. 비교는 이미 정확. 명세 `operators.md`, 에러 E0523, 설계 07. 시험 `tests/lang/err_mixed_inexact.fung`.
### SP42. `**`의 결과 타입이 지수의 값에 따라 바뀐다 — 결정, 낮음
- 재현: `print(type_of(2 ** 0), 2 ** -1)` → `'int 0.5`.
- 고치는 법(결정): (가) **추천** 정수 ** 음의 정수는 에러(`2 ** -1.0`으로). (나) 그대로.

- 처리: 이 커밋, 결정 D49: `value/arith.rs` `pow`가 정수 ** 음의 정수를 새 번호 E0524로(`0 ** -1`도; `0.0 ** -1`은 E0502 그대로). 명세 `operators.md`, 에러 E0524, 설계 07. 시험 `tests/lang/err_pow_negative.fung`(`arith.fung` 고침).
### SP43. `return` 뒤의 죽은 코드와 피연산자 자리의 `return` — 결정, 낮음 (앞단·컴파일러 영역의 같은 항목을 합침)
- 재현:
  ```
  fn f(): return 1; print("dead"); 2 ;;      → 1 (에러·경고 없음, check 종료 0)
  fn f(): return
    5
  ;;                                          → nil   (5는 결코 계산되지 않음)
  fn f(a): x = 1 + return a; x ;;             → a
  fn f(a) = [a, return 2]                     → 2
  ```
- 근거: 도달할 수 없는 절은 E0235, 효과 없는 문장은 E0234로 막는데 같은 종류인 `return` 뒤 문장은 조용하다. 죽은 코드는 바이트코드에도 그대로 남는다.
- 고치는 법(결정): (가) **추천** `return` 문장 뒤의 문장은 컴파일 에러(새 번호, `resolve/value.rs`의 `Do` 가지), `return`은 문장 자리와 가지 끝에만. (나) 죽은 코드만. (다) 경고.
- 처리: 이 커밋, 결정 D50: 파서의 `return_ok`(문장, `then … else`의 가지, `fn f() = …`의 본문, 람다 본문·기본값은 resolve의 E0237·E0211을 위해)를 다음 피연산자가 가져가고, 다른 자리의 `return`은 새 번호 E0132. `return` 문장 뒤의 문장은 새 번호 E0131(`parser/stmt.rs` `after_return`, 블록과 `match` 절). 명세 `functions.md`·`grammar.md`, 설계 01·03. 시험 `parser/tests.rs` `return_stands_as_a_statement_or_ends_a_branch`·`nothing_follows_a_return_statement_in_its_block`, `tests/lang/err_return_dead.fung`·`err_return_operand.fung`.

### SP44. `match`: 같은 리터럴 절의 중복, 마지막 절 자리의 람다 문장 — 결정, 낮음
- 재현: `match x: 1 => "a"; 1 => "b"; _ => "d" ;;` 에러 없음. 절 1의 값으로 쓰려던 마지막 줄 `y => y + 1`은 새 절이 되어 `pick(1)`은 `nil`, `pick(5)`는 `6`.
- 고치는 법: 글자 그대로 같은 가드 없는 패턴의 중복은 E0235. 람다 쪽은 문서 한 줄.
- 처리: 이 커밋, 결정 D51: `resolve/pattern.rs` `repeated_literals`: 가드 없는 앞 절(또는 그 `|` 대안)과 쓴 그대로 같은 리터럴 패턴의 절은 E0235(뒤 절을 가리키고 앞 절의 줄을 말함; 뒤 절의 가드와는 상관없이 돌지 않으므로 함께 막음). E0235의 메시지를 `this arm %s`로 넓히고 밑줄 글을 뺌. 람다 쪽 문서는 이미 `match.md`에 있음. 시험 `resolve/tests.rs` `arms_after_a_catch_all_are_errors`·`a_repeated_literal_arm_names_the_first`, `tests/lang/err_match_dup_literal.fung`.

### SP45. 가리기 규칙이 묶는 방법마다 다르다 — 결정, 낮음
- 재현: `x = 10` 뒤 `for x in [1, 2]: x ;;`, `match 1: x => x ;;`, `x => x`는 조용히 가림(경고도 없음). `if true: (x, y) = (1, 2) ;;`는 E0205.
- 고치는 법(결정): (가) **추천** 바깥 지역 이름을 가리는 묶기는 방법과 상관없이 에러. (나) 그대로. SP30·FE47과 한 묶음으로 정한다.
- 처리: 이 커밋, 결정 D38: 바깥 이름을 가리는 묶기는 방법과 상관없이 에러(`resolve/names.rs` `outer_name`·`binds_fresh`: 매개변수·람다·`for`·`match` 패턴·구조 분해·지역 `fn`·최상위 `fn`·import). 바깥 변수에 대입은 E0205, 그 밖은 새 번호 E0240(노트: 바깥 이름을 묶은 줄). import된 파일의 인터페이스(공개 최상위 이름, 공개 `fn`의 매개변수)는 prelude 이름이어도 됨. 시험 `tests/lang/err_hide_*.fung`.

### SP46. 타입 표기의 남은 구멍 — 결정, 낮음
- 재현: `fn f(xs: list[int]) -> list[str] = xs; print(f(["a"]))` → `["a"]`. `(x: int) => x`는 E0112.
- 고치는 법(결정): 원소 타입(11-08 FE17 결정)은 "검사한다" 또는 "적지 못한다" 중 하나가 원칙 1에 맞다. **추천: 적지 못하게**(문법·설명이 줄어듦).
- 처리: 이 커밋, 결정 D52: `parser/fn_.rs` `type_term`이 `[`를 새 번호 E0130으로(도움말 `list`·`map`만). `TypeTerm::elems`, `resolve/alias.rs`의 원소 검사 삭제, E0232 은퇴, E0228 도움말에서 `list[int]`를 뺌. 명세 `types.md`·`grammar.md`, 설계 07. 시험 `parser/tests.rs` `a_type_has_no_element_type`, `tests/lang/err_element_type.fung`·`err_element_type_on_scalar.fung`(옛 `element_types.fung`·`err_element_type_unknown.fung` 삭제).

### SP47. 다시 대입 + 복사 포착 + "한 번만 대입된 최상위 이름" 규칙의 무게 — 결정, 낮음
- 재현: `n = 0; inc = () => n + 1; n = inc(); n = inc(); n` → `1`. `level = 1; level = 2; fn show() = level` → E0204(람다는 됨).
- 판단: 명세대로이고 진단도 친절하다. 다만 `scope.md`의 "따라 나오는 것" 단락 전체가 "다시 대입 허용"에서 나온다. 약한 추천: 최상위에서만 다시 대입을 금지하면 "한 번/두 번" 구분과 그 진단이 사라진다.
- 처리: 이 커밋, 결정 D53: `resolve.rs` `bind_new`가 파일 블록의 두 번째 묶기를 새 번호 E0238로(노트: 처음 묶은 줄; `redefine`이면 그대로). 패턴의 `|` 대안이 같은 이름을 다시 묶는 것은 한 묶기(`pattern_bound`). `declare_consts`가 최상위 이름을 모두 상수로 하고 `assigned`와 E0204의 "assigned more than once" 도움말을 지움(남은 도움말은 람다·블록 안 함수의 것). 명세 `scope.md`(규칙 다섯), 에러 E0204·E0238, 설계 03·05·09. 저장소 `tests/lang` 여섯(`err_top_assigned_twice.fung` 지움), 매뉴얼 펜스. 시험 `resolve/tests.rs`(`top_level_fns_see_names_assigned_once` 외), `tests/lang/err_top_reassign.fung`·`top_constants.fung`.

### SP48. 범위가 리스트인 자리와 아닌 자리 — 문서·결정, 낮음
- 재현: `len`·`reverse`·`first`·`sort`·`take`는 범위를 받음. `(0..3)[1]` → E0509, `(0..3) + [1]` → E0501, `(0..3) == [0, 1, 2]` → `false`, `flatten([0..2, [5]])` → `[0..2, 5]`.
- 고치는 법: 문서에 "함수의 입력으로만 리스트처럼"이라고 선을 긋고 `flatten`만 맞춘다(LB55).

- 처리: 이 커밋, 결정 D77(LB55와 함께): 명세 `values.md`의 범위 행에 "함수의 입력으로만 리스트처럼", `collections.md` `flatten`, 설계 06.
### SP49. 남아 있는 작은 예외 조항들 — 결정·문서, 낮음
1. `for x in xs: if c: print(x) ;; ;;`는 되고 `each(xs, x => if c: print(x) ;;)`는 E0223. 문서에 근거 한 줄.
2. 버려지는 `Err`는 "리스트 한 겹만": `{a: Err(1)}`, `(Err(1), 2)`, `[[Err(1)]]`, `Ok(Err(1))`은 통과, `[Err(1)]`만 E0606. 임의의 선이다.
3. import 끌어올림: "import는 다른 문장보다 앞에만(뒤면 에러)"가 더 엄격하고 개념이 하나 준다. **추천**.
4. import된 파일의 최상위 제한은 `_x = print("init c")`로 지나갈 수 있어 형식뿐이다.
5. 접두사·지수는 소문자만인데 16진 숫자는 대소문자 모두(`0xFF == 0xff`).
6. `results.md` "에러를 잡는 구문은 없고"지만 `supervise`가 런타임 에러를 `Err({kind: 'error, code: "E0507", …})`로 준다. 문장을 고친다.
7. `??`가 `Ok`를 벗기므로 `get({k: Ok(1)}, "k") ?? 0` → `1`. 의도된 합침이니 문서에 한 줄.
- 처리(3·5): 이 커밋. 3은 결정 D55: `parser.rs` `program`이 다른 문장 뒤의 `import`를 새 번호 E0313으로, 끌어올림 문장을 명세 `modules.md`·`grammar.md`·설계 04에서 지움(E0209의 note도 "imported on line N"으로). `tests/lang/timeouts.fung`의 `import`를 맨 앞으로. 시험 `parser/tests.rs` `imports_come_before_the_other_statements`, `tests/lang/err_import_late.fung`. 5는 결정 D56: `lexer/number.rs`의 16진 숫자는 소문자만(`is_lower_hex_digit`), `0xFF`·`0XFF`의 도움말 `0xff`. 문자열 이스케이프는 그대로. 시험 `lexer/number.rs` `uppercase_prefixes_suggest_the_lowercase_form`, `tests/lang/err_hex_upper.fung`. 1·2·4·6·7은 다른 묶음.
- 처리(2): 이 커밋, 결정 D54: `Probe::Err`가 깊이와 상관없이(`Value::holds_err`, `holds_mutable`과 한 걷기 `holds_deep`, 노드 기억 그대로), `vm/m4.rs` `discarded_err`가 리스트·맵·튜플·태그 안의 첫 `Err`를 한 번의 걷기(`err_path`, 나눠 가진 부분은 한 번)로 찾아 길을 노트로. 명세 `results.md`, 에러 E0606, 배우기 9장, 설계 02. 시험 `tests/lang/err_discard_deep_map.fung`·`err_discard_deep_ok.fung`·`err_discard_deep_tuple.fung`(`discard.fung` 고침).
- 처리(1·4·6·7): 이 커밋(W12, 문서). 1은 명세 `conditions.md`: 문장 `for`는 리스트를 만들지 않아 본문의 값이 쓰이지 않고, `each`의 람다는 값을 돌려주는 함수라 컴파일러가 버려짐을 알 수 없다(부수효과 순회는 문장 `for`). 4는 명세 `modules.md`: 최상위 제한(E0306)은 문장의 모양만 보며 `_x = print(…)` 같은 효과 있는 초기화는 돈다고 적음(막는 것은 맨 문장). 6은 명세 `results.md`와 레퍼런스 `prelude/results.md`: "런타임 에러를 잡는 구문은 없다"에 태스크 경계의 `supervise`(`on_fail`과 `await`의 `{kind: 'error, …}`)를 예외로 밝힘. 7은 명세 `results.md`: `??`는 `Ok(v)`를 늘 벗기므로 `get({k: Ok(1)}, "k") ?? 0`은 `1`. 모두 릴리스 바이너리로 확인.

### SP50. 명세가 말하지 않는 것 — 문서, 낮음
동작은 합리적이고 문서만 없다: 실수 출력(`1.0`, `1e21`·`1e-7`부터 지수, `-0.0`은 찍히지만 `0.0 == -0.0`, `unique([0.0, -0.0])`은 `[0.0]`),
리터럴 패턴의 같음(`match 1: 1.0 => …`이 맞는다), `7.5 // 2 == 3.0`·`7 % 2.0 == 1.0`, `1.0 in 0..3`은 `true`.
- 처리: 이 커밋(W12). 명세 `values.md`에 "실수" 절(출력: 10^21 이상이나 10^-4 미만이면 지수 — 점검 문서의 "1e-7부터"는 틀렸고 실제로는 `9e-5`부터 —, 가장 짧은 되읽기 자릿수, `-0.0`과 `unique`, 실수의 `//`·`%`, 숫자 리터럴 패턴과 범위의 `in`). `operators.md`의 `in`: 범위는 "정수 포함"이 아니라 같은 수가 있는지(`2.0 in 0..3`은 `true`, `1.5`는 `false`).

### SP51. `contains(c, x)` ≡ `x in c`, `is_empty(x)` ≡ `len(x) == 0` — 문서, 낮음
`collections.md`가 스스로 "같다"고 적는다. 파이프·조건 자리용이라는 이유가 있어 지우기를 추천하지는 않는다. `in`의 설명에 짝을 밝힌다.
- 처리: 이 커밋(W12). 명세 `operators.md`의 `in`에 `contains`·`is_empty`의 짝과 함수 꼴을 두는 까닭(파이프·함수를 넘기는 자리), 레퍼런스 `collections.md`의 `contains`·`is_empty`에 같은 한 줄.

## 5. 앞단과 진단 (FE)

### FE40. 타입 별칭 펼치기가 지수 시간·메모리다 — 버그, 높음 (다시 확인)
- 재현:
  ```
  type t0 = int
  type t1 = t0 | t0
  …
  type t24 = t23 | t23
  fn f(x: t24) = x
  print(f(1))
  ```
  `fung check`: 16단 0.11초·27 MB, 20단 1.65~1.9초·356 MB, 22단 8.4초·1.6 GB, **24단 40.9초·5.2 GB**(한 단마다 약 4배).
  같은 원인의 다항 시간: 한 줄 사슬 `type t1 = t0 … type t4999 = t4998` 5,000개가 33.5초.
- 원인 위치: `crates/fung-front/src/resolve/alias.rs:48` `expand`(별칭을 만날 때마다 처음부터 다시 펼침, 결과 유니온의 중복을 빼지 않음), `:31` `declare_aliases`, 순환 검사 `visiting.iter().any`.
- 고치는 법: 별칭 이름 → 펼친 결과를 기억하고(선언 순서대로 채움) 유니온은 이름으로 중복을 뺀다. `visiting`은 `HashSet`과 스택을 함께. 둘 다 선형이 된다.
- 처리: 이 커밋, `resolve/alias.rs`: 펼친 별칭을 `Resolver::expanded`에 기억하고 유니온은 이름으로 중복을 뺀다(`union_into`), 순환 검사는 `HashSet`. 시험 `resolve/tests.rs` `alias_expansion_is_linear`(60단 두 배 별칭, 2만 개 사슬).

### FE41. 연산자의 피연산자 자리에서 괄호 없는 람다를 받는다 — 버그, 중간 (다시 확인)
- 재현:
  ```
  $ fung dump ast -e 'f = 1 + x => x * 2'
  (program (assign f (binary + 1 (fn (x) (block (binary * x 2))))))
  print(1 |> x => x + 1 |> str |> len)     → 1   (람다가 뒤의 파이프를 모두 삼킴)
  f = a < x => x < 3                        → 받음(비교 연쇄 검사 E0111을 지나감)
  f = 2 ** x => x * 2 + 1                   → 런타임 E0501
  ```
- 근거: `grammar.md` `expr = lambda | then_expr`. 람다는 `expr` 층에만 있고 어떤 연산자의 피연산자도 아니다. `functions.md` "람다를 식의 한 조각으로 쓰려면 괄호로 감싼다". `not`에는 같은 금지가 있다.
- 원인 위치: `parser/expr.rs:332~336`(`Kind::Name` 다음이 `=>`면 `min_bp`와 무관하게 `lambda()`), `:352`.
- 고치는 법: `min_bp > 0`인 자리에서 람다가 시작되면 E0110 + 도움말 "put the lambda in parentheses". 인자·리스트 원소·대입의 오른쪽·`then … else`의 가지는 `expr(0)`이라 그대로 된다.
- 처리: 이 커밋, `parser/expr.rs` `prefix(min_bp)`·`lambda_here`: `min_bp > 0`에서 시작하는 람다는 E0110 + 도움말 "put the lambda in parentheses"(가드의 `=>`는 그대로 절의 화살표). 명세 `operators.md`·`functions.md`·`grammar.md`. 시험 `parser/tests.rs` `a_lambda_is_no_operand_without_parentheses`, `tests/lang/err_lambda_operand.fung`.

### FE42. 에러 머리 줄이 소스의 제어 문자를 그대로 찍는다(11-08 FE13의 나머지) — 버그, 중간
- 재현(출력은 `cat -v`):
  ```
  x = 1 "<ESC>[31mred"                     → error[E0110]: unexpected string `"^[[31mred"`
  x = {"<ESC>[31ma": 1, "<ESC>[31ma": 2}   → error[E0113]: duplicate key `^[[31ma` …
  x = 1 `raw<LF>line<ESC>[2J`              → 머리 줄이 두 줄이 되고 화면을 지움
  ```
  바로 아래의 발췌 줄은 `\u{1B}`로 가려 준다.
- 원인 위치: `parser.rs:84` `describe`, `parser/collections.rs:64`, `parser/pattern.rs:280`, `diag.rs:99` `message`의 `%s`.
- 고치는 법: `diag::message`가 `%s`·`%v` 인자의 제어 문자를 `visible`과 같은 규칙으로 바꾼다. 원시 문자열은 `describe`에서 첫 줄만, 길면 자른다.
- 처리: 이 커밋, `diag.rs` `message`가 `%s`·`%v` 인자의 제어 문자를 이스케이프(`escaped`: `\n`·`\t`·`\r`·`\u{X}`; 탭은 발췌와 달리 공백이 아니라 `\t`), `parser.rs` `describe`는 토큰 글의 첫 줄만 40글자까지(`headline_text`). 런타임 문구(`fail`의 E0803 등)도 같은 규칙이다. 시험 `diag.rs` `message_arguments_show_control_characters`, `parser/tests.rs` `a_token_in_a_headline_is_one_short_line`.

### FE43. 다른 언어 습관의 문법 실수가 모두 맨 E0110이고 위치가 한 토큰 뒤다 — 정리(진단), 중간 (`elif` 다시 확인)
| 입력 | 실제 출력 | 문제 |
|---|---|---|
| `elif false:` | ``unexpected `false` `` | `elif`가 아니라 그 뒤를 가리킴 |
| `while true:` | ``unexpected `true` `` | 같음 |
| `let x = 1`, `def f(x):` | ``unexpected name `x` `` / `` `f` `` | 같음 |
| `x = true && false` | `unexpected character "&"` | `and`를 권하지 않음 |
| `x = !true` | ``unexpected `!` `` | `not` |
| `x += 1`, `x++` | ``unexpected `=` `` / `` `+` `` | 설명 없음 |
| `f = (a, b) -> a + b` | ``unexpected `->` `` | `=>` |
| `fn f(x) { x }` | ``unexpected `{` `` | `:` … `;;` |
| `match = 1` | ``unexpected `=` `` | 예약어라는 말 없음 |
| `x = null` | ``undefined name `null` `` | `nil` 제안 없음 |
| `if x = 1:` | ``expected `:` after the if condition, found `=` `` | `==` 제안 없음 |
- 원인: 모르는 낱말이 이름 식으로 읽히고 다음 토큰에서 E0110이 난다.
- 고치는 법: 문장 첫 이름을 작은 표(`elif`→`else if`, `while`→`until`·`iterate`, `let`·`var`·`def`·`function`·`end`·`null`·`None`)와 견줘 그 이름 위치에 도움말. 렉서의 `&&`·`||`, 파서의 전위 `!`·`+=`·`->`도 같은 표. `'abc'`에는 이미 이런 도움말이 있다.
- 처리: 이 커밋. 한 표 `fung-front/src/habits.rs`(낱말 `WORDS`, 기호 `OPERATORS`, `compound`·`reserved`·`COMPARE`·`NAMED_ARG`)를 렉서·파서·이름 해석이 에러가 난 뒤에만 본다. 문장 첫 낱말(바로 뒤나 바로 뒤 괄호 다음에서 문법 에러)은 새 번호 E0134(``…is not a keyword in fung``, 낱말을 짚음), `&&`는 E0101에, `||`·앞의 `!`·`+=` 등·`++`·`->`·`{`·예약어 `=`(``unexpected reserved word `match` ``, 낱말을 짚음)는 E0110에, 조건의 `=`·`else {`는 E0112에, 인자의 `=`는 괄호 목록의 E0112에 도움말(`parser.rs` `habit`·`expected`). 값으로 쓴 `null`·`undefined`·`nullptr`는 E0204의 도움말 `nil`(`resolve.rs`). `None`은 태그라 그대로 값이다(되는 프로그램이라 진단하지 않음). 줄에 홀로 선 `end`는 이름으로 읽히므로 닫지 않은 블록의 E0112에 도움말. `x--`는 줄 이어짐 규칙으로 다음 줄과 이어져 알아보지 않는다. 시험 `parser/tests.rs` `habits_from_other_languages_get_their_help`, `tests/lang/err_habit_*.fung`. 매뉴얼 `errors.md` E0134(표)·E0110.

### FE44. 닫지 않은 블록의 에러가 파일 끝을 가리키고 여는 자리를 말하지 않는다 — 정리(진단), 중간
- 재현: `fn f(x):` / `  if x:` / `    print(1)` / (빈 줄) / `print(2)` → ``error[E0112]: expected `;;` to close the if, found end of file`` (6:1).
- 근거: 괄호는 11-10 FE26에서 ``note: the `(` opened at 1:6``을 얻었는데 블록은 종류만 말한다.
- 고치는 법: 블록을 열 때 위치를 쥐고 ``note: the `if` opened at 2:3``(괄호 쪽 note는 `parser.rs:230`). 더 나아가면 여는 줄보다 덜 들여쓴 첫 문장을 가리킨다.
- 처리: 이 커밋. 블록을 닫는 `;;`는 `Parser::close`가 기대하고, 없으면 ``note: the `if` opened at 2:3``(`fn`·`if`·`for`·`match`·`test`; `else if` 사슬은 처음 `if`). 파일 끝이면 여는 줄보다 덜(같게) 들여 쓴 첫 줄도 `note`로 짚는다(`Parser::outdented`, `else` 같은 이어짐 토큰 제외). 시험 `an_unclosed_block_says_where_it_opened`, `tests/lang/err_unclosed_block.fung`. 매뉴얼 `errors.md` E0112.

### FE45. 문법 에러는 첫 하나만 보이는데 문서는 "모든 에러"라고 한다 — 문서·결정, 중간
- 재현: `x = 1 @ 2` / `y = 3 $ 4` / `print(zz)` → E0101 하나만. `print(a)\nprint(b)`는 E0204 두 개.
- 근거: `cli.md` "스크립트 실행"과 `check` 절의 "모든 에러를 보인 뒤", "모두 보이고".
- 고치는 법: (가) **추천** 문서를 "문법 에러는 첫 하나, 이름 에러는 모두"로. (나) 파서에 문장 단위 복구(LSP 때 어차피 필요).
- 처리: 이 커밋, 결정 D60 (가). 매뉴얼 `cli.md`의 스크립트 실행과 `check` 절을 "문법 에러는 첫 하나, 이름·import 에러는 모두"로. VM 4·6·7장은 이미 그렇게 적혀 있었다. 파서 복구는 LSP 때.

### FE46. 최상위 `fn`의 맨 바깥 블록에서 상수와 같은 이름의 지역 `fn`을 정의하면 E0205다(11-10 FE23 고침의 빈틈) — 버그, 낮음
- 재현: `c = 1` / `fn f():` / `  fn c() = 5` → E0205. 같은 자리의 `c = 5`와 `[c, d] = [5, 6]`은 된다.
- 근거: `scope.md` "최상위 fn이 자기 본문의 맨 바깥 블록에서 묶는 이름은 그 함수의 지역이고 같은 이름의 상수를 가린다".
- 원인 위치: `resolve.rs:932~934` `outer_bindings`의 `Head::Fn | Head::Do | Head::Arm => {}`가 이름 있는 `fn` 문장이 묶는 이름까지 빠뜨린다.
- 고치는 법: `Head::Fn`이고 `fn_stmt(n)`이 이름을 주면 그 이름을 `out`에 넣는다(본문으로는 내려가지 않음).
- 처리: 이 커밋, 결정 D38: 상수와 같은 이름의 지역 `fn`은 E0240(맨 바깥 블록이든 아니든). 진단은 `fn` 키워드가 아니라 이름을 가리킨다(`FnInfo::name_pos`). 시험 `tests/lang/err_hide_local_fn.fung`.

### FE47. 최상위 `fn` 안에서 가릴 수 있는 바깥 이름이 종류마다 다르다 — 결정, 낮음
- 재현(최상위 `fn`의 맨 바깥 블록의 대입 `h = 2`): 바깥 `h`가 상수면 됨, prelude 함수면 됨(W0103), 다른 최상위 `fn` 이름이면 E0205, `import fs (exists)`로 가져온 이름이면 E0205. 매개변수로 쓰면 넷 다 된다.
- 문제: 파일에 `fn count()`를 하나 더하면 다른 함수의 지역 `count = 0`이 컴파일 에러가 된다.
- 원인 위치: `resolve.rs:659` `defined_outside`, `:673` `assigns_constant`의 `own` 예외가 `consts`에만 적용된다.
- 고치는 법(결정): (a) 맨 바깥 블록의 대입은 `fn`·import 이름도 가린다(상수와 같은 규칙). (b) 그대로 두고 `scope.md`에 적는다. (c) 상수도 가릴 수 없게 한다(가장 엄격). SP30·SP45를 "모두 에러"로 정하면 (c)가 일관된다. 함께 정한다.
- 처리: 이 커밋, 결정 D38((c)): 최상위 `fn` 안에서 상수·최상위 `fn`·import·prelude 이름을 다시 묶는 것은 모두 에러(상수 대입은 E0205, 나머지 E0240). `FnScope::own`·`own_bindings` 삭제(11-10 D4를 뒤집음). 시험 `tests/lang/err_assign_top_name_in_fn.fung`·`err_assign_top_fn_in_fn.fung`.

### FE48. 파이프의 평가 순서가 오른쪽의 모양에 따라 다르다 — 결정, 낮음 (컴파일러 영역의 같은 항목을 합침)
- 재현(`a()`는 "a"를, `mk()`는 "mk"를 찍는다): `a() |> mk()(2)` → mk, a. `a() |> (mk())` → a, mk.
- 근거: `functions.md`·`operators.md` "언제나 왼쪽에서 오른쪽". 한편 파이프는 "평범한 호출로 바뀌는 문법 설탕"이라 `mk()(a(), 2)`의 순서가 된다. 두 문장이 부딪친다.
- 원인 위치: `desugar.rs:300~352`.
- 고치는 법(결정): (a) **추천** 명세에 한 줄("`x |> f(a)`는 `f(x, a)`와 같으므로 `f`가 `x`보다 먼저 평가된다")을 넣고 값 모양도 같은 순서로 맞춘다. (b) 대상이 이름·필드가 아니면 왼쪽을 숨은 변수에 먼저 담는다.
- 처리: 이 커밋, 결정 D57((b)): `desugar.rs` `pipe_call`·`evaluates_quietly`: 호출 대상이 이름이나 필드 사슬이 아니면 왼쪽 값을 숨은 `$p` 변수에 먼저 담는다. 명세 `operators.md`·`functions.md`("평가 순서"), 설계 03. 시험 `desugar.rs` `a_pipe_evaluates_its_left_value_first`, `tests/lang/pipe_order.fung`.

### FE49. 지역이 상수를 가려서 못 읽을 때의 E0204 도움말이 틀린 까닭을 말한다 — 버그(진단), 낮음
- 재현: `limit = 10` / `fn f():` / `  print(limit)` / `  limit = 5` → E0204, 도움말은 "file-level variable … that this function cannot see". 실제 까닭은 4행의 대입이 함수 전체에서 `limit`를 지역으로 만든 것이다.
- 원인 위치: `resolve.rs:765~769`.
- 고치는 법: `in_top_fn() && fns[1].own.contains(name)`이면 "이 함수 안(N행)에서 대입되어 함수 전체에서 지역이다" 도움말.
- 처리: 이 커밋, 결정 D38: 최상위 `fn` 안의 상수 이름 대입이 E0205가 되어 "함수 전체의 지역이 상수를 가려 앞의 읽기가 E0204"인 경우가 사라짐. 도움말은 바꾸지 않음. 시험 `resolve/tests.rs` `fn_scope_errors`.

### FE50. 타입의 `[ ]` 안에서 줄바꿈이 공백이 아니다 — 버그, 낮음
- 재현: `type t = list[⏎ int ]`와 `fn f() -> list[int |⏎ str] = [1]`이 E0110 unexpected newline. 매개변수 괄호 안(`fn f(a: list[⏎ int]) = a`)에서는 된다.
- 근거: `blocks.md` 줄 이어짐 "괄호 `() [] {}` 안: 줄바꿈은 공백이다", "타입 유니온의 `|`".
- 원인 위치: `parser/fn_.rs:222` `type_term`이 `[`에서 `self.nest`를 올리지 않는다.
- 처리: 이 커밋, `parser/fn_.rs` `type_term`이 `[ ]` 안에서 `nest`를 올린다(`element_types`). 시험 `parser/tests.rs` `newlines_inside_a_types_brackets`.

### FE51. `return`의 "값 없음" 판정이 닫는 토큰 일부만 안다 — 버그, 낮음
- 재현: `fn f(): [return] ;;` → `E0110 unexpected ]`, `{a: return}` → `unexpected }`. `(return)`, `f(return, 1)`은 된다.
- 원인 위치: `parser/expr.rs:362~374`의 목록에 `RBracket`·`RBrace`가 없다. SP43을 (가)로 정하면 이 항목은 사라진다.
- 처리: 이 커밋, SP43(결정 D50)으로 사라짐: `[return]`·`{a: return}`은 E0132. 가지 끝의 값 없는 `return` 뒤에 `]`·`}`도 오도록 목록에 더함(`[c then 1 else return]`).

### FE52. 구조 분해 대입의 패턴 에러가 버려지고 식의 에러가 나온다 — 정리(진단), 낮음
- 재현: `[a, ...b, c] = [1, 2, 3]` → E0110 unexpected `...`. `(a) = 1` → E0110 unexpected `=`(패턴 파서에는 "write `(p,)` for a one-item tuple" 문구가 있다).
- 원인 위치: `parser/stmt.rs:178` `try_destructure`가 패턴 에러를 버린 뒤 식으로 다시 읽는다. `arm_head`의 둘째 절부터도 같다.
- 고치는 법: 문장이 여는 괄호로 시작하면 `self.pairs`로 짝 닫는 괄호를 찾고 그 다음 토큰이 `=`인지 본다(O(1)). `=`면 대입으로 정하고 패턴의 에러를 그대로 낸다. "읽어 보고 되돌리기"가 없어진다.
- 처리: 이 커밋, `parser.rs` `brackets_close_before`: 문장이 여는 괄호(또는 태그와 그 `(`)로 시작하고 짝 닫는 괄호 다음이 `=`면 `stmt.rs` `destructure_assign`으로 바로 읽고, 갈래 머리는 `=>`·`when`이면 `pattern.rs` `arm_head`가 패턴 에러를 그대로 낸다. 시험 `parser/tests.rs` `destructuring_keeps_the_patterns_error`.

### FE53. 발췌가 줄 전체를 찍는다 — 정리(진단), 낮음 (도구 영역의 같은 항목을 합침)
- 재현: `x = [0,1,…,199999] nope` 한 줄(1.3 MB) → 에러 출력 2.5 MB. 30만 글자 문자열 뒤의 E0101은 600 KB(밑줄 줄에 공백 30만 개).
- 원인 위치: `diag.rs:221` `write_excerpt`.
- 고치는 법: 캐럿 둘레의 창(앞뒤 60칸)만 보이고 잘린 쪽에 `…`.
- 처리: 이 커밋, `diag.rs` `write_excerpt`: 200글자(`EXCERPT_WHOLE`)를 넘는 줄은 캐럿 앞뒤 60글자(`EXCERPT_CONTEXT`, 밑줄은 120글자까지)만 보이고 잘린 쪽에 `…`. 고치는 법과 달리 손으로 쓴 길이의 줄(200글자까지)은 통째로 보인다: 80~160글자 줄이 잘리면 흔한 에러가 읽기 어려워져서. 줄 앞의 BOM은 발췌에 보이지 않는다. 시험 `diag.rs` `a_long_line_is_cut_around_the_caret`.

### FE54. 이름 목록만 가져오는 import도 파일 이름이 이름 모양이 아니면 `as`를 요구한다 — 버그, 낮음
- 재현: `import "./my-lib" (f)` → `E0305 my-lib is not a valid module name; use as`.
- 근거: `modules.md` "이름 목록만 쓰면 목록의 이름만 묶이고 모듈 이름은 묶이지 않는다".
- 원인 위치: `loader.rs:377`. 조건에 `&& im.names.is_none()`을 더한다.
- 처리: 이 커밋, `loader.rs`의 E0305 조건에 `im.names.is_none()`. 시험 `tests/lang/import_names_only.fung`(`_lib/my-lib.fung`).

### FE55. 숨은 이름이 사용자에게 보이고, 부분 적용의 매개변수는 이름으로 부를 수 있다 — 버그, 낮음
- 재현: `fn f((a, b), c) = 1; f()` → ``missing argument `$a0` ``. `g = sub(_, 1); g()` → ``missing argument `h0` ``, `g(h0: 5)` → `4`.
- 원인 위치: `desugar.rs:514` `hidden("a")`, `:375` `format!("h{holes}")`(다른 숨은 이름과 달리 `$`가 없다).
- 고치는 법: 홀 매개변수도 `$h0`으로 하고, E0407·E0406이 `$`로 시작하는 매개변수를 "argument 1"처럼 자리로 부른다.
- 처리: 이 커밋, `desugar.rs` 홀 매개변수 `$h0`, `vm/call.rs`가 `$` 매개변수를 E0407에서 자리(`missing argument 1`)로 부른다. 카탈로그 E0407은 `missing argument %s`(이름이면 `` `x` ``). E0406은 이름 인자가 `$`로 시작할 수 없으니 바꿀 것이 없다. 시험 `vm/m4_tests.rs` `hidden_parameters_are_called_by_place`.

### FE56. `1.foo`는 E0102인데 `1.5.foo`는 받는다 — 버그, 낮음
- 재현: `print(1.5.foo)` → 런타임 E0509.
- 원인 위치: `lexer/number.rs`의 11-08 FE2 검사가 정수 부분 뒤에만 있다. 숫자·기간 토큰 바로 뒤에 `.`과 식별자 시작이 오면 E0102로.
- 처리: 이 커밋, `lexer/number.rs` `number`: 숫자·기간 토큰 바로 뒤의 `.` + 식별자 시작은 E0102. 시험 `malformed_numbers`(`1.5.foo`·`1e5.foo`·`5s.foo`).

### FE57. 보간 안의 끝 구분자를 받는다 — 버그, 낮음
- 재현: `"${1;}"` → `1`. `"${1;2}"`는 E0110.
- 원인 위치: `parser/expr.rs:508` `sub.skip_seps()`. 그 줄을 지운다.
- 처리: 이 커밋, `parser/expr.rs`에서 `sub.skip_seps()`를 지움. 시험 `parser/tests.rs` `interpolation_takes_no_trailing_separator`.

### FE58. CRLF 파일의 원시 문자열은 값에 `\r`이 들어간다 — 결정, 낮음
- 재현: 같은 소스를 LF와 CRLF로 저장하면 `` x = `a⏎b` ``의 `len(x)`가 3과 4다.
- 원인 위치: `lexer/string.rs:224` `raw_str`.
- 고치는 법(결정): (a) **추천** 원시 문자열 안의 `\r\n`을 `\n`으로. (b) 그대로 두고 `lexical.md`에 적는다.
- 처리: 이 커밋, 결정 D58: `lexer/string.rs` `raw_str`이 `\r\n`을 `\n`으로. 결정의 "(홀로 선 `\r`은 지금처럼 E0101)"과 달리 지금 홀로 선 `\r`은 문자열·원시 문자열 안에서 받으므로(`lexical.md`) 그대로 두었다(보고). 명세 `lexical.md`, 설계 01. 시험 `lexer/string.rs` `crlf_in_a_raw_string_is_a_newline`.

### FE59. 주석·문자열 안의 양방향 제어 문자 — 결정, 낮음
- 재현: `x = "a" # <U+202E> evil`과 `"<U+202E>"`가 그대로 통과한다.
- 근거: 홀로 선 CR을 주석 안에서도 E0101로 한 까닭이 양방향 덮어쓰기(U+202A~202E, U+2066~2069; "Trojan Source")에도 똑같이 해당한다.
- 고치는 법(결정): (a) **추천** 주석과 문자열 리터럴 안의 양방향 제어 문자는 E0101(문자열에는 `\u{202E}`). (b) `check`의 경고. (c) open-questions에 적는다.
- 처리: 이 커밋, 결정 D59: `lexer.rs` `is_bidi_control`, 주석·문자열·원시 문자열 안이면 E0101(도움말 `\u{202E}`). 명세 `lexical.md`, 에러 쪽 E0101, 설계 01. 시험 `lexer/string.rs` `bidi_controls_are_refused`.

### FE60. 겹친 `fn` 정의는 중첩 한도 E0115에 세지 않고 검사가 제곱이다 — 버그·성능, 낮음
- 재현(`fn` 블록을 N겹): `check`가 5,000겹 0.1초, 20,000겹 0.75초, 40,000겹 3.2초, 400,000겹 100초 넘음(끊음). `dump ast`는 선형(40,000겹 0.11초).
- 근거: `if`·`match`·`for`·괄호는 10,000겹에서 E0115다. 알려진 한계의 "평평한 사슬"과 다른 것이다.
- 원인: 파서는 선형이므로 제곱은 resolve·컴파일러 쪽이다(위치는 확인하지 못했다).
- 고치는 법: `fn` 블록도 `MAX_DEPTH`에 센다.
- 처리: 이 커밋, `parser/fn_.rs` `fn_def`: 이름 있는 `fn` 정의가 `depth`를 올린다(익명 `fn`은 식으로 이미 셈). 시험 `parser/tests.rs` `nested_fn_definitions_are_bounded`.

### FE61. 진단의 작은 것들 — 정리(진단), 낮음
- `test "x":\n  parse_int("x")\n;;`의 E0606이 `test` 줄 1열을 가리킨다. 값을 만든 식을 가리켜야 한다.
- E0210·E0113·E0205가 첫 정의의 위치를 보이지 않는다. 밑줄은 대부분 한 칸이다(`Diag.len`이 있으니 파서가 토큰 길이를 넘기면 된다, `diag.rs:35`).
- `x = 99999999999999999999` → E0102 "malformed number". 모양이 아니라 값이 64비트를 넘은 것이다.
- `fs.raed("a")` → E0303, 도움말 없음. `pritn`은 "a similar name exists: print"가 나온다(`resolve.rs:775`).
- `(1 tail calls elided)`: 단수에도 `calls`. 태스크 안의 에러 경로에 띄운 자리(`spawn` 줄)가 없다.
- `import "./a/util" as _`가 통과한다(`parser/stmt.rs:280`). `x | x = 1`(같은 대안 둘)이 통과한다.
- `(-⏎9223372036854775808)`은 E0102다. `parser/expr.rs` `negation`이 날 토큰(`self.toks[self.i + 1]`)을 본다.
- `import "./priv" (_p)` 뒤 `_p`를 쓰면 E0304에 이어 E0204가 따라 나온다(`resolve/module.rs:55`).
- 블록 안의 지역 `fn a()`를 같은 블록에서 두 번 정의해도 조용하다(최상위는 E0210).
- 처리: 이 커밋. (1) `test` 블록이 돌려준 `Err`는 블록의 마지막 문장을 가리킨다(`compiler/m6.rs` `test_stmt`의 `TEST` 위치). (2) E0210은 첫 정의의 줄을 `note`로(`resolve.rs` `defined_twice`), E0113은 첫 키의 자리를(`parser.rs` `duplicate_key`). E0205는 이미 `note`가 있었다(D38). (3) 파서의 E0110·E0112·괄호 목록 에러가 토큰 길이만큼 밑줄(`tok_len`), 숫자 에러도 숫자 전체(`lexer/number.rs`). (4) 값이 타입에 담기지 않는 숫자는 새 번호 E0135(`number … is out of range`, 정수·실수 넘침과 0으로 아래 넘침; `-9223372036854775808 ** 2`도). (5) 모듈 멤버의 E0303에 비슷한 이름 도움말(`resolve/module.rs` `missing_member`). (6) `(1 tail call elided)`(`diag.rs`). (7) 태스크의 에러에 ``note: the task was spawned at 파일:줄``(`vm/scope.rs` `body_ended`). (8) `import … as _`는 E0112(도움말). (9) 결코 맞지 않는 `|` 대안(앞 대안이 모든 값에 맞거나 같은 대안)은 새 번호 E0241(`resolve/pattern.rs` `dead_alternatives`). (10) `(-⏎9223372036854775808)`(`parser/expr.rs` `negation`이 줄바꿈을 건너 봄). (11) import 목록의 E0304·E0303 뒤에도 이름을 묶어 E0204가 따라 나오지 않는다. (12) 한 블록의 지역 `fn` 두 번은 E0210(D38의 E0240이 잡지 않던 경우, `Scope::fns`). 시험 `parser/tests.rs`·`resolve/tests.rs`·`lexer/number.rs`·`diag.rs`·`vm/tests.rs`의 새 사례, `tests/lang/err_int_out_of_range.fung`·`err_alt_never_matches.fung`·`err_local_fn_twice.fung`.

### FE62. 앞단의 구조 — 정리, 낮음
- 단계 사이의 약속이 문자열과 모양이다: 연산자가 `Lit::Str("not_in")`·`"bool"`, 새 이름 묶기가 `Let`의 `"fresh"`, 숨은 이름이 `starts_with('$')`.
  `resolve/value.rs:235` `partial_shape`와 `:304` `hidden`은 desugar가 만든 트리를 모양으로 다시 알아본다. desugar가 모양을 바꾸면 E0234 판정이 조용히 틀어진다.
- `Resolver`의 "다음 함수에 전할 말"(`map_arg`, `top_arg`)을 필드에 적어 두고 `function`이 `take`한다. 인자로 넘기는 편이 낫다. `resolve.rs`는 973줄, 필드 24개다.
- `Kind::is_keyword`가 enum의 선언 순서에 기댄다(`KwFn..=KwNil`).
- 보간의 끝 표시: 렉서가 `Eof` 토큰의 `text`를 `"}"`로 두고 파서의 `describe`가 그 문자열로 가린다.
- `Token`이 크다: 모든 토큰이 `text: String`을 쥔다. `1 + 1 + …` 100만 항의 `check`가 2 GB를 쓴다. 급하지 않다.
- 처리: 이 커밋(값싼 것만). `Kind::is_keyword`가 키워드 표를 본다(선언 순서에 기대지 않음). desugar가 만든 숨은 이름의 표시는 `tree::HIDDEN`·`tree::is_hidden`, `(let fresh …)`는 `tree::FRESH`, 조건 검사 `(op bool …)`는 `tree::BOOL_CHECK`(모든 단계가 상수를 씀). `Resolver::top_arg` 필드를 없애고 `function(n, top)`의 인자로. `map_arg`는 이미 없었다. 하지 않은 것: 소스 연산자 이름(`"not_in"` 등)의 열거형화(트리·컴파일러·VM의 `assert` 비교까지 넓게 바뀜), `resolve/value.rs`의 `partial_shape`·`hidden`이 desugar 모양을 다시 알아보는 것(desugar가 표시를 남기도록 바꾸는 큰 일), 보간 끝 `Eof` 토큰의 `"}"` 표시(렉서 토큰 종류를 더해야 함), `Token`의 크기(급하지 않음).

## 6. 컴파일러·바이트코드·VM 실행 핵심 (VM)

명령 집합(54개)은 작고 겹침이 거의 없다. 인코딩 한도(인자 4095/4096, 24비트, 대안 256/257, 중첩 10000)는 모두 E0230·E0115로 끝나고 조용히 잘리는 곳이 없다.
꼬리 위치 판정과 에러 위치는 명세와 맞는다. 정확성 버그는 찾지 못했다.

### VM19. 문장이 없는 자리의 마지막 읽기는 값을 놓지 못해, 문장 하나의 유무로 교착이 생겼다 사라진다 — 결정, 중간 (재현 A 다시 확인)
- 재현 A:
  ```
  fn go(flag):
    (tx, rx) = channel(1)
    if flag:
      send(tx, 1)
    else:
      for x in rx: x ;;        # E0702 deadlock (tx를 go가 쥐고 있다)
    ;;
  ;;
  print(go(false))
  ```
  `else:` 아래에 `nil` 문장 하나를 넣으면 `[]`를 찍고 끝난다.
- 재현 B(클로저가 붙잡는 것이 마지막 읽기): `[spawn(s, () => send(tx, 1)) == nil, for x in rx: x ;;]` → E0702. `t = spawn(…)`을 따로 문장으로 두면 `[1]`.
- 재현 C(가드): `match 1: n when ready(tx) => for x in rx: x + n ;; …` → E0702.
- 근거: 설계 03 "값의 수명"에 적힌 동작이다("문장이 없는 자리의 읽기는 그 자리를 품은 문장 뒤에 놓인다"). 그런데 11-08 VM1을 고치며 기각한 이유("같은 코드를 `if` 안으로 옮기면 교착")와 같은 모양의 놀라움이다. 품은 문장이 함수의 마지막 식이면 "문장 뒤"가 없다.
- 원인 위치: `compiler/moves.rs:65`(`Head::Fn`은 읽기만 더함), `:97`, `compiler.rs:348` `let_go`, `:386` `branches`.
- 고치는 법(결정, 추천 (가)+(나)):
  - (가) 가지에 들어설 때 놓기: `if`/`match`/`??`/`and`/`or`의 각 가지 시작에서 `held − (그 가지 시작의 live)`를 `CLEARLOCAL`. 재현 A·C가 풀리고 "문장이 없는 자리" 예외가 사라진다.
  - (나) 붙잡기가 마지막 읽기면 `CLOSURE` 바로 뒤에 놓기. 재현 B가 풀린다.
  - (다) 그대로 두고 매뉴얼 `scope.md`에 이 경우와 재현 하나를 넣는다.
- 처리: 이 커밋, 결정 D61 (가)+(나). `compiler/moves.rs`의 뒤 걷기가 가지마다 "가지 앞에서는 살아 있는데 이 가지의 시작에서는 죽은 칸"(match 갈래는 가드만 읽은 칸까지)을 내고, 컴파일러가 `if`·`else`, `??`의 두 쪽(왼쪽 값을 쓰는 길은 `COALESCE` 바로 뒤), 조건 자리 `and`·`or`의 오른쪽, 패턴과 가드를 지난 갈래 본문의 첫머리에 `CLEARLOCAL`을 낸다(`Compiler::enter`). 붙잡기가 마지막 읽기면 `CLOSURE` 바로 뒤(`Compiler::captured`). 코드가 없는 길(조건 자리 `and`·`or`가 건너뛰는 쪽, `else` 없는 `if` 문장)에서 죽은 칸은 앞 걷기가 다음 놓는 곳으로 들고 간다. 함수를 부르지 않고 프레임이 끝나는 가지(꼬리 자리, `return`)와 꼬리 자리의 함수 값은 놓는 명령을 내지 않는다(볼 수 있는 것이 없다; 없으면 `if c: return 1 ;;`마다 살아 있는 칸 수만큼 명령이 붙는다). 세 재현과 `??`·`and`·`or`(값·조건 자리)는 `tests/lang/local_lifetime.fung`, 바이트코드 모양은 `compiler/tests.rs` `branches_and_captures_let_go_where_they_start`. 설계 03 "값의 수명"(예외 삭제, 기각 (다)), 05, 매뉴얼 `scope.md`(재현 A를 경계 사례로), VM 10장·8장·부록, `concurrency.md`.

### VM20. 캡처가 없는 람다·지역 `fn`도 평가할 때마다 클로저를 새로 할당한다 — 성능·결정, 중간 (다시 확인)
- 재현(1,000만 번):
  ```
  fn apply(f, x) = f(x)
  fn loop(i, acc) = i == 0 then acc else loop(i - 1, acc + apply(x => x + 1, i))   # 1.44~1.55 s
  ```
  `x => x + 1`을 최상위 `fn inc`로 올린 같은 프로그램은 1.06~1.13초. 람다 하나에 약 40 ns.
- 얼마나 흔한가: `examples`+`bench`의 람다 87개 가운데 50개, `tests/lang`+`manual`의 271개 가운데 162개(약 60%)가 아무것도 붙잡지 않는다. `for` 본문은 늘 람다다.
- 원인 위치: `compiler.rs` `expr`의 `Head::Fn`(늘 `Op::Closure`), `vm.rs:908`.
- 고치는 법: `info.captures`가 비어 있으면 링크 때 클로저 값을 한 번 만들어 상수 풀에 두고 `CONST`로 올린다.
- **정할 것**: 함수 `==`가 정체성 비교라 `f = () => (x => x)`에서 `f() == f()`가 지금 `false`인데 변경 뒤 `true`가 된다.

- 처리(일부): 이 커밋, 결정 D62의 같음 부분: `value/compare.rs`의 `==`·해시가 함수(클로저·내장 함수)에 E0501(``cannot compare fn and fn``), `<`는 이미 E0501. 명세 `values.md`·`operators.md`, 에러 E0501, 설계 02. 시험 `value/tests.rs`, `tests/lang/err_eq_fn.fung`·`err_unique_fn.fung`. 캡처 없는 클로저를 상수로 만드는 일은 W10.
- 처리: 71106ef2, 결정 D62의 나머지: `CLOSURE`가 붙잡는 것이 없는 자식을 모듈마다 한 번 만들어 부모 `RtProto::made`에 기억하고 다시 쓴다(다른 모듈의 전역 표면 새로 만든다; 자식에 두면 원형이 자기를 쥐는 고리라 부모에). 상수 풀에 넣는 대신 런타임 기억인 것은 클로저가 모듈의 전역 표를 쥐어 링크 때 만들 수 없어서다. 재현 1.44 → 1.11초. 시험 `vm/tests.rs` `closures_that_capture_nothing_are_made_once`, 매뉴얼 VM 9장·부록, `docs/notes/bench-languages.md` 4.12절.

### VM21. 기본값 있는 매개변수마다 호출 때 명령 둘(`LOADLOCAL; JUMPIFSET`)이 돈다 — 성능, 중간
- 재현(2,000만 번 자기 꼬리 호출, 인자 다섯 개 모두 줌): 기본값 넷인 `fn c(n, acc = 0, step = 1, m = nil, k = nil)`은 1.53~1.56초, 기본값 없는 같은 함수는 0.99~1.05초(+50%).
- 원인 위치: `compiler/fn_.rs:119~130`, `vm.rs:920`.
- 고치는 법: (가) **추천** 원형에 "기본값 머리말 뒤" 위치를 두고, 호출이 위치 인자로 모든 매개변수를 채웠으면(`Callee::exact`가 이미 아는 사실) `ip`를 그곳에서 시작한다. 자기 꼬리 호출도 같다. (나) `JUMPIFSET`이 스택 대신 칸을 직접 본다.
- 처리: 71106ef2, (가): `Proto::body`(기본값 서두 뒤)를 두고 위치 인자만으로 모든 매개변수를 채운 호출(`Callee::exact`)과 자기 꼬리 호출, 내장 함수의 되부름이 그곳에서 시작한다(`push_frame`·`reuse_frame`의 `start`). 재현 1.52 → 1.03초. 시험 `compiler/tests.rs` `body_starts_after_the_defaults_prologue`, 매뉴얼 VM 9장·부록, `docs/notes/bench-languages.md` 4.12절.

### VM22. 바이트코드 검증기가 없다 — 정리, 중간
- VM은 컴파일러를 전적으로 믿는다: 빈 칸을 읽으면 `expect("CHECKTYPE on an unassigned slot")` 같은 패닉, 스택이 어긋나면 `expect("VM stack underflow")`, 점프 대상은 검사 없음.
  - `compiler.rs:688` `end_tail`: 주석의 "꼬리 자리에서는 아무것도 그 끝으로 점프하지 않는다"가 깨지면 점프가 코드 끝 밖으로 떨어진다(지금은 지켜진다).
  - `compiler/fn_.rs:26` `call`: 인자를 소스 순서대로 낸다. VM(`call.rs:171`·`249`)은 "위치 인자 전부, 그 뒤 이름 쌍"을 가정한다. 파서의 E0126이 지켜 주는데 컴파일러에는 그 가정이 적혀 있지 않다.
- 고치는 법: `bytecode::verify(&Proto)`를 두고 디버그 빌드와 시험에서 모든 컴파일 결과에 돌린다. 검사: (1) 피연산자가 표 범위 안, (2) 점프 대상 < 코드 길이, (3) 모든 길이 `RETURN`/`TAILCALL`/`NOMATCH`로 끝남, (4) 합류점의 스택 높이 일치, (5) 칸의 "비었음/찼음" 추상 해석.
  점검 때 dump 출력에 대한 (2)~(4)를 Python 150줄로 썼고 위반 0이었다. 덤으로 최대 스택 높이가 나온다.
- 처리: 이 커밋, `bytecode/verify.rs` `verify`: (1) 피연산자 범위(`Operand`별 표, 자식의 붙잡기 포함), (2) 점프는 앞으로·코드 안, (3) 모든 길이 `Flow::Stop`으로 끝남, (4) 스택이 모자라지 않고 합류점의 높이가 같음, (5) 칸의 "모든 길에서 찼음" 추상 실행(`LOADLOCAL`·`MOVELOCAL`·`CHECKTYPE`·`CLOSURE`의 붙잡기; `LOADLOCAL; JUMPIFSET`의 기본값 검사, `MATCH` 뒤 `JUMPIFFALSE`를 지난 길의 묶기, `JUMPIFSET`의 점프 길은 찼음). 최대 스택 높이를 `Verified::max_stack`으로. 디버그 빌드의 `compile`이 모든 결과에 돌리고(위반은 E0801), 시험 `crates/fung-front/tests/verify.rs`가 릴리스에서도 `tests/lang`·`examples`·`bench`·`extras`·매뉴얼의 `fung run`·`fung check`·`dump bytecode` 펜스를 모두 검증한다(컴파일되는 파일 245개·펜스 908개, 위반 0). 잘못 만든 원형마다 잡는지는 `bytecode/tests.rs` `the_verifier_*`.

### VM23. 수명 상태(`held`)를 세 가지 갈래 코드가 손으로 이어 간다 — 정리, 중간
- `Compiler::held`는 코드를 내는 동안 바뀌는 상태이고, 갈래마다 "앞 상태에서 시작해 끝에서 합치기"를 따로 구현한다: `compiler.rs:386` `branches`, `:632` `cond`, `compiler/m4.rs:133` `match_expr`, `match_arm`의 `subject_done`·`held.remove`(세 곳). 11-08 VM1·11-10 VM14가 모두 이 이어 가기의 누락이었다.
- 분석 결과는 노드 **주소**(`*const Node`)를 키로 한 표 넷(`folds`·`moves`·`after`·`stored_live`)이다. 트리가 복사되면 조용히 "기록 없음 = 보수적"으로 떨어진다(`pattern.rs:100`이 패턴 노드를 복사해 `m4.rs:222`가 원래 노드를 키로 따로 넘긴다).
- 좋은 점: 분석이 보지 못한 노드는 "놓지 않음·넘기지 않음"이라 보수적인 쪽으로만 틀린다. 무작위 프로그램 2만여 개에서 한 번도 틀리지 않았다.
- 고치는 법: `moves::walk`가 "노드 → 그 뒤(또는 그 가지 시작)에 놓을 칸 목록"과 "넘기는 읽기"를 내고, 컴파일러는 표를 찾아 내기만 한다. VM19·VM24가 같은 변경으로 풀린다. 키는 노드 번호가 안전하다.
- 처리: 이 커밋. 노드에 번호(`Node::id`, resolve가 `Node::number`로 앞 순서대로 매김, 복사본은 같은 번호)를 두고, `moves::lives`가 번호를 키로 `Lives { moves, after, entry, closure, subject_moves }`를 낸다. 컴파일러의 `held`·`branches`·`stored`·`subject_done`·`has_call`, `cond`·`match_expr`·`match_arm`의 손 이어 가기를 지웠다. `fold::constants`의 표도 번호 키(`*const Node` 키가 남지 않음). 번호 0(번호 없는 트리)은 아무것도 기록하지 않아 보수적이다. 검증기(VM22)가 빈 칸 읽기를 잡으므로, 무작위 프로그램(가지·`match` 가드·대안·`??`·`and`·`or`·람다·`return`) 약 5,400개를 전·후 바이너리로 돌려 출력이 같고 검증 위반이 없음을 확인했다(그 과정에서 `??`의 왼쪽 길과 `else` 가지가 같은 키를 써 칸을 일찍 놓던 실수를 검증기가 잡았다; 키는 이제 늘 선택의 자식).

### VM24. 수명 분석이 "문장 수 × 살아 있는 지역 수"로 느려진다 — 성능, 낮음
- 재현(지역 N개를 차례로 대입하고 끝에서 모두 읽는 함수, `dump bytecode`): 5,000개 0.43초, 10,000개 1.65초, 20,000개 6.49초, 40,000개 30.7초. 지역이 곧 죽는 경우는 선형이다.
- 원인 위치: `compiler/moves.rs:116`(문장마다 live 집합 `BTreeSet`을 통째로 복사), `:74`, `compiler.rs:348`·`:370`.
- 현실성: 동시에 살아 있는 지역 수천 개는 생성된 코드에서만 나온다. 1,000개면 약 20 ms.
- 처리: 이 커밋. 뒤 걷기가 살아 있는 칸 집합 하나(`Vec<bool>`)를 제자리에서 고치고 바꾼 것을 기록(`log`)해 가지마다 되돌린다. 문장마다 집합을 남기지 않고, 앞 걷기는 코드가 없는 길에서 죽은 칸만 든다. 같은 재현(릴리스): 5,000개 0.59 → 0.10초, 10,000개 1.91 → 0.03초, 20,000개 7.29 → 0.05초, 40,000개 40.9 → 0.09초. 시험 `crates/fung-cli/tests/perf.rs` `forty_thousand_live_locals_compile_in_linear_time`(1초). 남은 한계: `return`마다 살아 있는 칸을 모두 죽이므로 `return`이 많고 동시에 살아 있는 지역이 많은 함수는 그 곱이다(지역 1만 개·`return` 1만 개 5.2초, 전 4.4초). `docs/notes/bench-languages.md` 4.11절.

### VM25. 상수가 아닌 중첩 리터럴은 깊이마다 "상수인가"를 처음부터 다시 걷는다 — 성능, 낮음
- 재현(원소 20개 + 안쪽 리스트를 D번 중첩, 맨 안에 변수 하나): 깊이 4,000에 2.75초, 8,000에 11.0초(모두 상수면 0.15초).
- 원인 위치: `compiler/m4.rs:21~30` `collection`이 노드마다 `constant(n, folds)`(`:279`)를 부른다. 11-10 VM18과 같은 꼴인데 그때는 `fold`만 고쳤다.
- 고치는 법: `fold::constants`의 한 번 걷기에 "이 노드는 상수 리터럴인가"를 같이 기록한다.
- 처리: 572e6124, `fold::constants`의 한 걸음이 리스트·맵·튜플·태그·범위 리터럴과 보간의 상수도 노드 번호로 기록하고, `collection`은 표를 찾기만 한다(`m4.rs`의 `constant` 삭제). 재현 4,000겹 2.80 → 0.03초, 8,000겹 11.5 → 0.07초. 시험 `perf.rs` `a_deep_literal_with_a_variable_compiles_in_linear_time`(1초), `docs/notes/bench-languages.md` 4.12절.

### VM26. `a ?? b`의 성공 길에 `JUMP`가 하나 더 있다 — 성능, 낮음
- 재현: `y = m[k] ?? 0` → `COALESCE 5; JUMP 6; CONST 0; STORELOCAL 2`.
- 원인 위치: `compiler/m4.rs:106~123` `coalesce`.
- 고치는 법: `COALESCE`의 뜻을 뒤집는다("nil/Err이 아니면 피연산자로 점프(값은 둠), 맞으면 버리고 다음 명령으로").
- 처리: 572e6124, 고치는 법대로 뜻을 뒤집었다: `COALESCE L`은 `nil`·`Err`이 아닌 값(`Ok`는 그 값)을 가지고 L로 가고, 아니면 버리고 바로 뒤의 오른쪽으로. 왼쪽 길에 놓을 칸이 있으면 그것을 오른쪽 뒤에 두고 오른쪽이 건너뛴다. 검증기는 점프 길이 하나 더 높다(`Flow::BranchKeep`). `??` 루프 0.838 → 0.815초(VM27·VM28과 함께). 시험 `compiler/tests.rs`, `tests/lang/jump_shapes.fung`, 매뉴얼 VM 부록, `docs/notes/bench-languages.md` 4.12절.

### VM27. 조건 자리의 `or`와 `not`에 `JUMPIFTRUE`가 없다 — 성능, 낮음
- 재현: `if x > 1 or x < 0:` → `GT; JUMPIFFALSE 5; JUMP 9; …`. `if not x:` → `NOT; JUMPIFFALSE`.
- 원인 위치: `compiler.rs:642~651`.
- 고치는 법: `JUMPIFTRUE`를 더하고 `cond`를 "참이면 갈 곳 / 거짓이면 갈 곳" 두 목록을 받는 꼴로. `not e`는 두 목록을 바꿔 `e`를 컴파일한다.
- 처리: 572e6124, `JUMPIFTRUE`를 더하고 `Compiler::cond_true`(참이면 갈 곳 목록)를 두었다. `or`의 왼쪽은 참이면 가지로, 불을 만드는 식의 `not e`는 두 목록을 바꿔 `e`를 컴파일한다(불이 아닐 수 있는 `not x`는 `not`의 에러를 지키려고 그대로 `NOT`). `or`·`not` 조건 루프 0.867 → 0.815초. 시험 `compiler/tests.rs` `compile_if_with_jumps`, `tests/lang/jump_shapes.fung`, 매뉴얼 VM 8장, `docs/notes/bench-languages.md` 4.12절.

### VM28. 점프가 점프·`RETURN`으로 간다 — 성능, 낮음
- 재현: `fn a(x, y) = str(x then (y then 1 else 2) else 3)`의 `JUMP 8`이 `JUMP 10`으로 간다. `fn i(a, b) = a == nil or b != nil and a < b`의 끝이 `JUMP 15; CONST false; RETURN`(꼬리 자리인데 가지가 바로 `RETURN`하지 않음: `compiler.rs:565`가 `(bool e)`의 `tail`을 버린다).
- 원인 위치: `compiler.rs:675` `end_branch`, `Proto::patch`.
- 고치는 법: (a) `end_branch`에서 마지막 명령이 `RETURN`/`TAILCALL`이면 `JUMP`를 내지 않는다. (b) 원형을 끝낼 때 대상이 무조건 `JUMP`인 점프를 그 대상으로 고친다.
- 처리: 572e6124, (a) `end_branch`가 프레임을 끝낸 가지(`Proto::ends_closed`: 마지막이 `RETURN`·`TAILCALL`·`NOMATCH`이고 그 뒤를 가리킨 점프가 없음) 뒤에 `JUMP`를 내지 않는다. (b) `Proto::thread_jumps`가 함수를 끝낼 때 `JUMP`로 가는 점프를 그 대상으로, `RETURN`으로 가는 `JUMP`를 `RETURN`으로 고친다. (c) 불을 만드는 식의 `(bool e)`가 꼬리 자리를 넘긴다. 재현의 `JUMP 8`·`JUMP 15; CONST false; RETURN`이 사라졌다. 숫자는 VM27과 함께. 시험 `compiler/tests.rs`(`match` 갈래 끝의 `JUMP` 0개), `tests/lang/jump_shapes.fung`, 매뉴얼 VM 8장, `docs/notes/bench-languages.md` 4.12절.

### VM29. `match`가 갈래마다 `LOADLOCAL; MATCH; JUMPIFFALSE` 셋이고 패턴 트리를 처음부터 해석한다 — 성능, 낮음 (VM 코어 영역의 같은 항목을 합침)
- 재현: `match r: Ok(v) => v  Err(e) => e ;;` — `Err`이면 명령 8개. `MATCH`는 늘 `JUMPIFFALSE`와 붙어 나온다(불을 스택에 만들고 꺼낸다).
  `match x: x when x > 0 => x …` → `MOVELOCAL 0; STORELOCAL 1; LOADLOCAL 1; STORELOCAL 2; LOADLOCAL 2`(숨은 변수 왕복).
  `MATCH`는 값을 복제해 `match_pattern`(`vm/m4.rs:302-313`, `Binder`와 `pending: Vec` 생성)을 탄다. 정수 리터럴 3절 `match`가 +56 ns다.
- 원인 위치: `compiler/m4.rs:168~244` `match_arm`, `vm/matcher.rs:88-150`.
- 고치는 법: (a) `RtPattern::Lit`가 `Int`/`Sym`/`Nil`/`Bool`이면 `equal` 대신 직접 비교. (b) `MATCH`가 실패하면 스스로 점프(피연산자 둘이 필요해 VM32와 함께). (c) 방금 `STORELOCAL n`한 것을 바로 올리는 쌍은 엿보기 최적화로. 결정 트리 컴파일은 권하지 않는다.
- 처리: 572e6124, (a) `RtPattern::Lit`가 같은 타입의 정수·심볼·불·`nil`은 바로 견준다(정수 리터럴 `match` 0.743 → 0.691초). (b) `MATCH L n`은 둘째 피연산자가 패턴, 첫째가 실패 때 갈 곳이라 `JUMPIFFALSE`가 없다(VM32와 함께, 0.696 → 0.652초). 실행 루프 안에 두니 `fib` +2.7%·꼬리 재귀 +9%여서 `#[inline(never)]` `match_op`로 뺐다. (c)는 넣지 않았다: 값을 남기는 저장 명령이 새로 필요한데 그 꼴은 가드 있는 이름 절에만 나온다. 검증기는 `MATCH`가 계속 가는 길에 묶는다. 시험 `compiler/tests.rs`, `bytecode/tests.rs`, 매뉴얼 VM 부록, `docs/notes/bench-languages.md` 4.12절.

### VM30. 태그·맵 리터럴·이름 인자의 이름을 `CONST`로 스택에 올린다 — 성능, 낮음
- 재현: `Ok(x)` → `CONST 'Ok; MOVELOCAL 0; TAG 1`. `{name: x, age: y}` → `CONST "name"; x; CONST "age"; y; MAP 2`.
- 원인 위치: `compiler/m4.rs:38~53`, `compiler/fn_.rs:35`.
- 고치는 법: `TAG`의 피연산자를 `CALL`처럼 나눠(이름 상수 번호 + 인자 수) 이름을 올리지 않는다. 맵은 "키 목록 상수 하나 + 값 n개". 효과는 재지 못했다.
- 처리: 572e6124, `TAG n`은 이름 상수, `MAP n`은 키 목록 상수(`Const::List`)를 둘째 피연산자로 받는다(VM32). 태그·맵 루프 1.95 → 1.83초(VM29 (b)와 함께). 이름 인자의 이름은 그대로 `CONST`: `bind_in_place`가 스택의 심볼을 읽는 호출 규약을 바꿔야 하고 이름 인자 호출은 드물다. 시험 `compiler/tests.rs`, `bytecode/tests.rs`(역어셈블), 매뉴얼 VM 8장, `docs/notes/bench-languages.md` 4.12절.

### VM31. 접히지 않는 상수식 — 성능, 낮음
- 재현: `"x" + "y"` → `CONST; CONST; ADD`. `"a${1}b"` → `CONCAT 3`. `if true: x else: x + 1 ;;` → `CONST true; JUMPIFFALSE`와 죽은 가지.
- 원인 위치: `compiler/fold.rs`(수 리터럴의 산술만). 이런 코드는 드물어 효과는 작다.
- 처리: 572e6124, 문자열 리터럴끼리의 `+`(바이트로 잇고 UTF-8이면 `Str`), 문자열·정수 리터럴만의 보간, 조건 자리의 `true`·`false`(참이면 아무것도, 거짓이면 `JUMP`; 쓰이지 않는 가지의 코드는 남아 놓기 위치가 그대로다). 효과는 따로 재지 않았다(VM27·VM28과 함께). 시험 `compiler/tests.rs` `compile_literals_and_concat`, `tests/lang/jump_shapes.fung`.

### VM32. 32비트 `Instr`는 이제 한도만 만드는 중간 표현이다 — 정리, 낮음
- `Instr`(8+24비트)는 `Proto::emit`이 만들고 `vm/link.rs:63~70`이 곧바로 `Code {op, arg: u32}`(8바이트)로 푼다. 남는 효과는 24비트 한도, `CALL`·`CHECKTYPE`의 12+12 묶음과 4095 한도, "피연산자 둘이 필요한 명령을 못 만든다"는 제약이다.
- `CHECKTYPE`의 `& 0xFFF`·`>> 12`는 도우미 없이 세 곳에 있다(`compiler/fn_.rs:135`, `bytecode/disasm.rs:61`, `fung-vm/src/vm.rs:926`).
- 고치는 법: 앞단이 처음부터 `{op, a: u32}`(또는 피연산자 둘, 여전히 8바이트)를 낸다. 급하지 않다. VM29·VM30을 할 때 같이.
- 처리: 572e6124, VM29·VM30에 필요한 만큼: 앞단 `Instr`는 64비트(명령 8 + 피연산자 24 + 둘째 피연산자 24, `Instr::with`·`Instr::b`, 뜻은 `Op::second`), VM의 `Code`는 둘째 피연산자를 명령 종류 뒤의 빈 3바이트에 담아 8바이트 그대로다(시험 `a_decoded_instruction_is_8_bytes`). 넘치면 E0230(그 피연산자의 표 이름). `CALL`·`CHECKTYPE`의 12+12 묶음은 그대로 두었다. 역어셈블은 `MATCH 8 0`처럼 둘째를 뒤에. 설계 09, 매뉴얼 VM 2·8장·부록.

### VM33. 명령 하나를 더하려면 표 일곱 곳을 고쳐야 한다 — 정리, 낮음
- `bytecode/op.rs`의 `enum Op`·`Op::ALL`·`name()`·`has_arg()`, `bytecode.rs:194` `check_operands`, `disasm.rs` `comment`, VM `step_loop`·`exec_m4`. 점프 대상을 갖는 명령 목록은 어디에도 없다.
- `op.rs`의 주석 "The numbers are fixed: `fung dump bytecode` and its expected outputs show them"은 사실과 다르다(dump는 이름만 보인다). 파일 이름 `compiler/m4.rs`·`m6.rs`는 마일스톤 번호다.
- 고치는 법: 선언 매크로 하나에서 열거형·이름·`has_arg`·스택 효과·점프 여부를 만든다(VM22의 검증기가 같은 표를 쓴다).
- 처리: 이 커밋, `bytecode/op.rs`의 `instructions!` 표 하나가 `Op`·`ALL`·`name`과 `Info`(`Operand` 피연산자의 뜻, `Effect` 스택 효과, `Flow` 다음 갈 곳)를 만든다. `has_arg`·`jumps`·`stack_effect`는 표에서, `check_operands`의 E0230 문구는 `Operand`에서, CHECKTYPE 풀기는 `decode_slot_type` 하나. 검증기가 같은 표를 쓴다. 틀린 "번호는 고정" 주석을 고쳤다(번호는 표의 자리, dump는 이름만 보인다). 파일 이름 `m4.rs`·`m6.rs`는 그대로 두었다(기록을 읽기 쉽게).

### VM34. 이름 인자가 많은 호출의 묶기가 O(n³)이다 — 성능, 낮음
- 재현(기본값 있는 매개변수 N개에 이름 인자 N개): 500개 0.04초, 1,000개 0.39초, 2,000개 3.43초, 4,095개 28~35초. 전부 실행 시간이다.
- 원인 위치: `crates/fung-vm/src/vm/call.rs:273~274`(매개변수마다 `(0..nnamed).any(…)`), `slot_of`(`:107`)가 선형 탐색.
- 고치는 법: 이름 인자를 한 번 돌며 칸 번호를 구해 비트 배열에 표시한다. 현실의 호출에는 영향이 없다.
- 처리: 71106ef2, `bind_in_place`가 이름 인자를 한 번 돌며 칸을 찾아 표시하고(`Fiber::filled`), 칸 번호를 값과 함께 버퍼에 둔다(`Fiber::named: Vec<(usize, Value)>`). 에러의 순서와 번호는 그대로. 4,095개 7.1 → 0.033초. 시험 `perf.rs` `four_thousand_named_arguments_bind_quickly`(1초), `docs/notes/bench-languages.md` 4.12절.

### VM35. 명령 해석의 남은 고정 비용 — 성능, 낮음
레지스터 VM·LTO·묶음 명령은 기각/보류라 다루지 않는다.
1. 명령마다 3단 따라가기(`vm.rs:795-801`): `frames[fi]` → `closure` → `proto` → `code[ip]`. `Closure`가 `code: Rc<[Code]>`를 나란히 쥐면 한 단 준다. 재 봐야 아는 실험 후보다.
2. `Value::clone`·`drop_in_place<Value>`가 인라인되지 않는다(`fib` 표본 6% + 2%). `LOADLOCAL`(`vm.rs:957`)·`CONST`(`:803`)에 `Int/Float/Bool/Nil/Sym/Dur`면 비트 복사하는 빠른 길.
3. M4 명령(`vm.rs:1007-1024`)이 에러가 없어도 `self.here(fi)`로 위치 표를 먼저 읽고 `exec_m4`에서 `op`를 다시 `match`한다. 11-10 VM16이 호출에서 없앤 것이 여기 남았다.
4. `result_kind`(`vm/m4.rs:44-48`)가 `Ok(x)`를 풀 때 페이로드를 복제한 뒤 태그를 버린다. 혼자 쥐었으면 옮기면 된다.

명령당 비용(500만 회 꼬리 루프, 루프 자체 46 ns/회): `len(xs)` +30 ns, 클로저 호출 +28 ns, `m.c` +18 ns, `"k${i}"` +136 ns, `[i, i]` +106 ns, `put({a: 1}, "b", i)` +200 ns.
`fib(35)` 표본: `step_loop` 83%, `Value::clone` 6%, `push_frame` 6%, `Frame` drop 3%.
- 처리: 71106ef2, (2) `CONST`·`LOADLOCAL`이 스칼라를 그 자리에서 복사(`copy_value`; `fib` 1.605 → 1.564, 꼬리 재귀 0.428 → 0.412초). (1) `Closure::code`가 원형의 코드를 나란히 쥔다(람다 루프 1.110 → 1.029, `match` 1.345 → 1.294, `fib` 1.590 → 1.613로 +1.4%지만 (2)와 합쳐 점검 전과 같아 넣었다). (3) M4 명령의 위치를 에러 때만 찾기와 (4) 혼자 쥔 `Ok`의 값 옮기기는 넣지 않았다: 잡음 안(`!` 루프 0.740/0.730/0.755, `??` 루프 0.822/0.832/0.823초). `docs/notes/bench-languages.md` 4.12절.

### VM36. 숨은 불변식과 전역 표의 `Rc` 고리 — 정리, 낮음 (기록용)
1. `Cont::resume` 중 패닉하면 그 상자는 `Hole`로 바뀐 채 `unwind`를 받는다(`vm/call.rs:497`).
2. `result_kind`의 `t.args[0]`은 "`Ok`/`Err`는 늘 값 하나"에 기댄다. `Op::Tag`에는 `debug_assert`만 있다. `Tag::ok(v)`/`Tag::err(v)` 생성자만 쓰게 하면 타입이 지킨다.
3. `Binder::matches`(`vm/matcher.rs:80-87`)는 실패한 절이 지역 칸을 덮어썼을 수 있다는 것을 컴파일러의 칸 배정 규칙에 기댄다.
4. `value.rs:230-236` `Closure.globals`와 그 표에 든 최상위 `fn` 클로저가 서로를 쥔다. 실행에는 영향이 없지만 한 프로세스에서 런타임을 거듭 만드는 임베딩·LSP가 생기면 문제가 된다. open-questions "알려진 한계"에 한 줄.
- 처리: 이 커밋(W12, 기록). 1~3은 코드 주석(`vm/call.rs` `Hole`, `vm/m4.rs` `result_kind`, `vm/matcher.rs` `Binder::matches`), 4는 `value.rs` `Closure.globals`의 주석. 넷 모두 open-questions "알려진 한계"(메모리·스택)에. `Tag::ok`·`Tag::err` 생성자로 타입이 지키게 하는 것은 하지 않았다(기록용 항목).

## 7. 값 층과 prelude (VA)

16바이트 `Value`, 명시적 스택으로 도는 출력·비교·해시·해제(100만 겹에서 정상), 양끝으로 자라는 영속 벡터와 노드 메모가 모두 시험에서 틀리지 않았다.
`unsafe`는 libc FFI와 `mallopt`뿐이고 VM·값에는 없다.

### VA28. 값 출력이 문자열 한도를 다 만든 뒤에 본다 — 버그, 높음(조건부) (다시 확인)
- 재현:
  ```
  x = fold(0..40, [1], (a, i) => [a, a])
  [y] = x
  → 에러가 나오지 않음. 4초에 RSS 965 MB, 10초에 1.27 GB, 계속 증가

  r = timeout(() => str(x), 1s)      → 1초 기한이 듣지 않음
  ```
  같은 `x`로 `print(x)`, `"${x}"`, `assert x != x`, `fail(x)`, `json.stringify(x)`, `match x: [] => 1;;`도 같다. `x`는 노드 41개짜리 값이고 논리 크기만 2^40이다(`x == x`·`len`은 즉시).
  공유 구조가 아니어도 낭비가 있다: 500만 원소 리스트의 패턴 실패(E0602)는 200자를 보여 주려고 전체 문자열을 만든다(RSS +45 MB). `assert xs == []`는 자르지 않은 전체 값을 `notes`에 넣는다(+134 MB).
- 조건부인 이유: 멈춤까지 가려면 논리 크기가 지수인 공유 구조 값이 있어야 한다.
- 원인 위치: `value/format.rs:61` `write`(와 `:6 format`, `:196 write_for_print`)에 예산이 없다. `vm/m4.rs:282`·`:287`·`:108`·`:116`·`:248`은 전체를 만든 뒤 자른다. `vm/m6.rs:88`·`:91`은 자르지도 않는다. `vm.rs:985-1005` `Op::Concat`과 `prelude/convert.rs:174 str_fn`은 다 쓴 뒤 검사한다.
- 고치는 법: `format::write`에 바이트 예산을 준다(원소 하나를 쓴 뒤 `out.len() > max`면 멈춤).
  - 에러 문구(`shown`): 예산 `SHOWN_CHARS * 4 + 여유`, 넘으면 `…`.
  - `str`·보간·`print`·`fail`: 예산 `MAX_CHARS * 4`바이트를 넘는 순간 E0806.
  - `assert`의 notes도 `shown`으로.
- 처리: 이 커밋, `value/format.rs`에 바이트 예산(`write_within`·`format_within`·`format_start`, 문자열은 예산까지만 씀). 에러 문구는 `vm/m4.rs` `shown_value`(예산 `SHOWN_CHARS * 4 + 16`): E0601·E0602·E0603·E0606, `assert`의 notes(`vm/m6.rs`), 변환의 E0513과 `parse_*`의 `Err`(`prelude/convert.rs`). `str`·보간(`Op::Concat`)·`print`·`eprint`·`fail`·`join`·`dbg`는 `format_text`로 `MAX_CHARS * 4`바이트를 넘는 순간 E0806(`write_for_print`가 `Result`). `json.stringify`는 이 항목에서 손대지 않았다. 시험 `value/tests.rs` `format_stops_at_its_budget`, `vm/m4_tests.rs` `errors_show_a_shared_value_at_once`. CHANGELOG.

### VA29. 리스트 한도가 `+`에는 걸리지 않는다 — 결정, 중간 (다시 확인)
- 재현: `xs = fold(1..=27, [1], (s, _) => s + s); print(len(xs))` → 134217728, RSS 2.4 GB. 25번이면 3,355만 원소·610 MB. 40번이면 30초에 14 GB. 문자열의 같은 꼴은 E0806.
- 근거: `errors.md` E0806의 설명(리스트 1억 원소)과 다르다.
- 원인 위치: `MAX_ELEMENTS`(`crates/fung-vm/src/native/nat.rs:261`)는 `nat.rs:273,303`(범위를 펼칠 때)에서만 본다.
- 고치는 법(결정): (가) **추천** 리스트 `+`·`flatten`·`put`의 결과 길이에 같은 한도(문자열과 대칭). (나) 문서를 "범위를 펼칠 때만"으로. 어느 쪽이든 "전체 메모리 한도는 없다"는 한 줄이 필요하다.

- 처리: 이 커밋, 결정 D63: `native/nat.rs` `check_len`을 `+`(`value/arith.rs`), `flatten`(리스트를 받으면 원소 길이를 먼저 더함), `put`·`merge`(맵 항목 수), `map`·`try_map`·`filter`·`group_by`와 받는 끝을 모으는 `all_items`(원소 하나마다), `bytes`가 씀. `read_lines`는 D22처럼 `Err({kind: 'io})`. 펼치기는 W4에서 이미. 문서 "전체 메모리 한도는 없다": `collections.md` 크기 제한, 에러 E0806, 설계 02. 시험 `native/nat.rs`(`check_len`), `tests/lang/err_list_limit_flatten.fung`. `+`·`put`의 1억 원소 시험은 메모리가 커서 두지 않음.
### VA30. 순회 네이티브가 람다 원소마다 시계를 읽는다 — 성능, 중간 (시간과 코드 다시 확인)
- 측정: `print(fold(0..30000000, 0, (s, i) => s + i))` 1.56~1.58초. 표본 783개 가운데 `mach_absolute_time`·`clock_gettime` 등 289개(37%). 같은 일을 하는 꼬리 재귀 루프는 1.25초.
- 원인 위치: `native/walk.rs:215-216` `Walker::advance`의 첫 줄 `let mut piece = std::time::Instant::now();`. `piece`는 plain 네이티브를 제자리에서 부르는 길(`:232-243`)에만 쓰이는데, 콜백이 fung 클로저면 `advance`가 원소마다 다시 불려 시계를 읽고 버린다. `Drainer::advance`(`:356-357`)도 같다.
- 고치는 법: `let mut piece: Option<Instant> = None;`으로 두고 `self.count == STEP_ITEMS`가 된 자리에서 `get_or_insert_with(Instant::now)`.
- 이득(추정): 원소당 약 19 ns(`fold` 합 약 −35%). `map`/`filter`/`each`도 같은 절대값.
- 처리: 2ed22e63, `vm/pieces.rs` `Piece`: 조각의 시계는 첫 `over`(첫 걸음 뒤 일이 남았을 때)에 처음 읽는다. `Walker`·`Drainer`·`run`·`Pieces`·`with_items` 모두. 측정 1.55 → 1.06초(−32%). `docs/notes/bench-languages.md` 4.12절.

### VA31. `holds_mutable`이 작은 컨테이너마다 `Vec`과 `HashSet`을 만든다 — 성능, 중간
- 측정(100만 번, 키 1만 개 맵, 값은 `{id, name, tags: [...], pos: (i, i), ok}` 레코드): `swap(a, m => put(m, "77", 레코드))` 7.18 µs/회, 아톰 없는 같은 `put` 1.22 µs/회. 표본의 77%가 `Value::holds_mutable` 아래(`hashbrown insert`·SipHash·`malloc`).
- 원인 위치: `value.rs:142-184`. 큰 리스트·맵은 노드 메모로 O(1)이지만 작은 컨테이너(32개 이하 리스트, 16개 이하 맵, 튜플, 태그, 클로저)는 메모가 없다. 바뀐 경로의 잎을 다시 훑을 때 항목마다 `Vec` 스택과 `HashSet<*const ()>`(`:146-147`)을 새로 만든다.
- 고치는 법: (가) 작은 컨테이너에도 메모(`Cell<u8>`, `pvec.rs`의 `Memo`와 같은 것). `seen` 집합이 필요 없어진다. (나) 싸게: `seen`을 방문 노드가 일정 수(64)를 넘을 때만 켠다.
- `send`(`vm/channel.rs:249-264`·`:351`·`:511`)와 `reset`도 같은 함수를 쓴다.
- 처리: 2ed22e63, (나)의 변형: `holds_deep`이 컨테이너 64개·깊이 16까지의 값은 할당 없는 재귀(`holds_small`)로 답하고, 넘으면 전처럼 스택과 공유 집합으로 걷는다. 같은 함수를 쓰는 D54의 `holds_err`(W6)가 버려지는 `swap` 결과마다 같은 비용을 더해 묶음 앞에서 3.90초까지 느려져 있었다. 측정 프로그램(레코드 1만 개 맵, `swap` 30만 번) 3.86 → 0.97초(점검 전 2.10). `docs/notes/bench-languages.md` 4.12절.

### VA32. 문자열 `+`와 보간이 매번 결과 전체를 세 번 훑는다 — 성능, 중간 (5만 번 다시 확인)
- 측정(`fold(0..n, "", (s, i) => s + X)`):

  | n | `X = "x"` | `X = "가"` | 리스트에 모아 `join` |
  |---|---|---|---|
  | 50,000 | 0.11 s | 2.10~2.15 s | 0.01 s |
  | 100,000 | 0.43 s | 8.44 s | 0.03 s |
  | 200,000 | 1.71 s | 33.71 s | 0.06 s |
- 원인 위치:
  - `value/text.rs:91-101` `Text::concat`: 새 `Vec`에 복사 → `Text::new`(`:37`)가 `String::from_utf8`로 전체 재검증 → `is_ascii()`로 또 훑음.
  - `value/arith.rs:129`: `add(&a, &b)`가 참조만 받아 혼자 쥔 왼쪽도 새로 만든다. 바이트코드는 이미 `MOVELOCAL s; CONST "x"; ADD`라 왼쪽 참조는 하나다.
  - `vm.rs:985-1005` `Op::Concat`: 용량 없는 `Vec::new()` → `check_chars` → `Value::bytes`(재검증).
- 고치는 법(작은 것부터): (1) `Text::concat`에서 둘 다 `Utf8`이면 `String::with_capacity` + `push_str`, `ascii = x.ascii && y.ascii`. (2) `Op::Add`에서 왼쪽 `Rc<Text>`가 `Rc::get_mut`으로 잡히면 제자리에 덧붙인다(상각 O(1); `fib` 회귀를 잴 것). (3) `Op::Concat`은 조각 길이를 미리 더해 `with_capacity`. (4) 조각(`value/index.rs:262-270`)도 재검증하지 않는다.
- open-questions는 "`+`가 문자열 전체를 한 번 복사한다"만 적었다. 누적의 제곱 비용과 비ASCII 20배는 없다.
- 처리: 2ed22e63, (1) `Text::concat`이 둘 다 UTF-8이면 `String`으로 잇고 다시 검사하지 않는다(`Text::ascii`, VA38). (2) `ADD`의 두 문자열은 `add_texts`가 왼쪽을 꺼내 `Rc::get_mut`이 되면 제자리에 덧붙인다(`Text::append`, 상각 O(1)). (3) `CONCAT`(`Task::concat`)는 크기를 미리 잡고, 모든 조각이 UTF-8이면 `String`에 정수를 바로 쓰고 검사하지 않는다(거절 검사의 순서는 그대로). (4) UTF-8 문자열의 자르기는 다시 검사하지 않는다. 측정: 쌓기 10만 번(ASCII) 0.45 → 0.009초, 5만 번(한글) 2.06 → 0.007초, 보간 루프 0.57 → 0.46초, `fib`는 그대로. 시험 `perf.rs` 사례 "grow a string", `docs/notes/bench-languages.md` 4.12절.

### VA33. 부모의 1/4보다 작은 조각은 통째로 복사된다 — 성능, 중간
- 측정(`xs`는 n개): `len(xs[i..])` 루프가 25,000에 0.14초, 50,000에 0.61초, 100,000에 2.60초. `xs[..i]`·`take(xs, i)`도 같다.
- 원인 위치: `value/list.rs:157-170`(`n * 4 >= vec.len()`이 아니면 `cloned().collect()`). 작은 조각이 큰 벡터를 붙잡지 않게 하려는 것이다.
- 고치는 법: L5에서 생긴 `Pvec::truncate`(`pvec.rs:418`)·`drop_front`(`:460`)가 O(log n)이고, `extendable_front`(`list.rs:246-262`)가 이미 "복제 → `truncate(hi)` → `drop_front(lo)`"로 자기 원소의 잎만 쥔 벡터를 만든다. `slice`의 복사 갈래를 `n > SMALL_MAX`일 때 이 길로 바꾼다.
- 처리: 2ed22e63, `List::slice`가 `SMALL_MAX`보다 크고 부모의 1/4보다 작은 조각을 트리 복제 + `truncate` + `drop_front`로 만든다(자기 잎만 쥠). 측정 2.41 → 0.056초. 시험 `value/tests.rs` `small_slices_of_a_big_list_are_cut_not_copied`, `perf.rs` 사례 "small slices of a long list", `docs/notes/bench-languages.md` 4.12절.

### VA34. `Work` 네이티브가 작은 입력에서도 시계를 1~4번 읽는다 — 성능, 낮음
- 측정: `sort([3, 1])` 루프 표본의 21%, `split("a,b", ",")` 8%가 `mach_absolute_time`. 호출당 `sort([3, 1])` 226 ns, `split("a,b", ",")` 240 ns, `join(["a","b"], ",")` 186 ns.
- 원인 위치: `vm/pieces.rs:59-70` `run`(`:60`), `over`(`:74-81`), `with_items`(`:117`), `Pieces::resume`(`:89`).
- 고치는 법: 첫 걸음을 시계 없이 하고, 처음 몇 걸음(4)은 세기만 한다.
- 처리: 2ed22e63, VA30의 `Piece`로: 한 걸음에 끝나는 일은 시계를 읽지 않는다(표본에서 `mach_absolute_time`이 사라짐). 이 기계에서는 `sort([3, 1])`·`split` 루프 1.16 → 1.13초로 작다(점검 때의 21%는 재현되지 않음). 처음 네 걸음을 세기만 하는 방법은 넣지 않았다. `docs/notes/bench-languages.md` 4.12절.

### VA35. 비ASCII 문자열 색인 루프가 O(n²)이다 — 성능, 낮음
- 측정(`s = repeat("가b", n)`, `s[i]` 루프): 25,000에 0.11초, 100,000에 1.56초. ASCII는 0.01초, `chars(s)`로 바꾸면 0.01초.
- 원인 위치: `value/index.rs:107-122`, `value/text.rs:116-124`. 알려진 한계에 연산 하나의 비용으로는 적혀 있다.
- 새 제안: `Text`에 마지막으로 찾은 (글자 번호, 바이트 위치)를 `Cell` 하나로 기억한다. 안 고친다면 매뉴얼에 "글자를 하나씩 볼 때는 `chars(s)`" 한 줄.
- 처리: 2ed22e63, 새 제안대로: `Text::offset_of`가 마지막으로 찾은 (글자 번호, 바이트 위치)를 `Cell`에 기억하고 그 뒤면 거기서 읽는다(색인과 자르기의 시작). 측정 1.55 → 0.019초. 시험 `perf.rs` 사례 "each character of a long non-ASCII string", `docs/notes/bench-languages.md` 4.12절.

### VA36. `<`·정렬과 해시가 공유 구조에서 지수 시간이다 — 버그, 낮음
- 재현: VA28의 `x`로 `print(x < x)` → 5초 뒤에도 실행 중. `print(len(unique([x])))`도 같다. `x == x`는 즉시다.
- 원인 위치: `value/compare.rs:199-246` `compare`에 `equal`의 `same_object`(`:184`) 지름길이 없다. `hash_value`(`:304`)는 공유 노드의 해시를 기억하지 않는다.
- 고치는 법: `compare`에서 쌍을 여는 자리에 `same_object`면 건너뛰기 한 줄. 해시는 다른 언어도 같은 한계라 그대로 둔다.
- 처리: 이 커밋, `value/compare.rs` `compare`: 같은 리스트·튜플끼리의 쌍은 한 번만 연다(`walked` 집합). 그냥 건너뛰면 `[nil] < [nil]`처럼 비교하지 못하는 값이 자기 자신과는 비교되므로, 처음 한 번은 걸어서 에러를 지킨다. 해시는 그대로. 시험 `value/tests.rs` `compare_skips_shared_parts`.

### VA37. 작은 리스트·맵의 머리가 큰 변종 크기로 잡힌다 — 성능, 낮음
- 측정(값 하나의 메모리): `[i]` 128 B, `(i, i)` 64 B, `{a: i}` 209 B, `Ok(i)` 80 B, `str(i)` 80 B.
- 원인 위치: `value/list.rs:22-29`의 `Big { vec: Pvec<Value>, lo, hi }`(`Pvec` 56바이트), `value/map.rs:23-26`의 `Big(Omap)`.
- 고치는 법: 큰 변종을 상자에. 작은 리스트 128 → 약 80 B, 작은 맵 209 → 약 130 B.
- 처리: 2ed22e63, 리스트의 `Pvec`과 맵의 `Omap`을 상자에. 100만 개의 `[i]` 최대 상주 174 → 127 MB, `{a: i}` 254 → 206 MB. 시간은 같거나 빠름(만들기 0.120 → 0.109·0.177 → 0.152초, `bench/list.fung`·`map.fung` 잡음 안). `docs/notes/bench-languages.md` 4.12절.

### VA38. `Text::new`를 아는 호출자도 탄다 — 정리, 낮음
`Value::bytes`(`value.rs:204`)·`Text::new`(`text.rs:37-46`)는 바이트가 뭔지 모를 때의 입구인데, 문자열 조각·`Op::Concat`·`Text::concat`·`upper`/`lower`/`replace` 결과도 이 길을 쓴다.
`Text::utf8(String)`과 `Text::ascii(String)`을 이름 있는 생성자로 두면 VA32-1이 그 첫 사례가 되고 불변식도 한곳에서 읽힌다.
- 처리: 2ed22e63, `Text::ascii(String, bool)`·`Text::utf8(String)`. `Text::concat`·`append`, `CONCAT`, 문자열 자르기가 쓴다. `upper`·`lower`·`replace`는 그대로(`String`에서 `Text::from`).

### VA39. 런타임 진단 문구의 어긋남 — 정리(진단), 낮음
- 인덱스 에러의 번호와 문구가 갈린다: `5[0]`·`nil[0]`은 `E0509 cannot index a value of type int`, `"s".a`·`[1].a`·`[1]["a"]`는 `E0501 cannot index list and str`. `[1].len`에 `len(xs)` 제안이 없다.
- 값 자리의 `for`가 "map"으로 불린다: 문장 `for i in 5:`는 `cannot iterate over int`, `print(for x in 5: x ;;)`는 `cannot map over int`.
- Result를 그냥 쓴 실수에 도움말이 없다: `parse_int("12") + 1` → `E0501 cannot add Ok and int`. 보간(E0519)에는 "unwrap it with `!` or `?` first"가 있다.
- 처리: 이 커밋. 인덱스 에러는 모두 E0509 ``cannot index list with str``(두 타입, `value/index.rs` `index_error`; 카탈로그 문구를 바꿈). 리스트·문자열 등의 필드 `xs.len`은 이름이 prelude 함수면 도움말 ``a list has no fields; to call `len` on it, write `len(…)` ``(`index::field_help`, VM의 `FIELD`도). `map`·`try_map`(값 자리 `for`)은 ``cannot iterate over int``. 연산자(산술·비교·부호)의 E0501·E0506에서 피연산자가 결과값이면 도움말 ``unwrap it with `!` or `?` first``(`vm.rs` `unwrap_first`, 에러 길에서만). 시험 `vm/tests.rs` `runtime_wording_of_the_third_review`, `tests/lang/err_field_of_list.fung`·`err_result_operand.fung`.

### VA40. prelude 버그 둘 — 버그, 낮음 (`pad_left` 다시 확인)
- `drop(rx, -1)`이 채널을 다 받은 뒤에 E0403을 낸다:
  ```
  (tx, rx) = channel(3); send(tx, 1); close(tx)
  drop(rx, -1) → E0403, 그 뒤 recv(rx) → Err('closed)   # 1이 사라짐
  take(rx, -1) → E0403, 그 뒤 recv(rx) → Ok(1)
  ```
  열린 채널이면 E0403 대신 멈춘다. 원인: `extras/prelude.fung:78-82`에서 `_list(xs, …)`가 `_clamp`보다 먼저다(11-10 SP18의 남은 자리). 맨 앞에서 `_arg.count("n", n)`을 부른다.
- `pad_left("x", 9223372036854775807)` → `error[E0513]: cannot convert 9223372036854776000.0 to int`(내부 에러가 샘). `pad_left("x", 200000000)`은 E0806. 원인: `extras/prelude.fung:96`의 `int(missing / len(pad))`가 실수 나눗셈을 거친다. `//`로 바꾼다.
- 처리: 이 커밋, `extras/prelude.fung`: `drop`(과 `take`)이 맨 앞에서 `_arg.count`, `_padding`은 `missing // len(pad)`. 시험 `tests/lang/err_drop_count_first.fung`·`err_pad_huge.fung`.

## 8. 동시성 런타임 (CC)

"보낸 값은 정확히 한 번 받는다"는 불변식은 기한과 경주시킨 수만 번의 `send`/`recv`에서 깨지지 않았다. 자료구조는 모두 선형이고 태스크당 비용도 작다(주차한 태스크 하나에 약 1.9 KB, 버퍼 없는 채널 핑퐁 메시지당 약 0.6 µs).
손으로 쓴 `unsafe impl Send/Sync`가 없고, 풀·닫기 스레드·`Ready`의 잠금에서 깨움을 잃을 자리를 찾지 못했다.

### CC14. FIFO 핸들 하나를 두 태스크가 읽거나 쓰면 이벤트 루프의 불변식이 깨진다 — 버그, 높음 (종합 때 다시 돌리지 않음)
- 재현(먼저 `mkfifo ff`, 다른 쪽에서 0.3초 간격으로 `one`, `two`, `three`를 씀):
  ```
  import fs
  h = fs.open("ff", 'read)!
  r = scope(fn(s):
    a = spawn(s, () => read_line(h))
    b = spawn(s, () => read_line(h))
    c = spawn(s, () => read_line(h))
    [await(a), await(b), await(c)]
  ;;)
  print(r)
  ```
  - 디버그 빌드: `error[E0801]: internal error: two waits on one fd and interest`(매번).
  - macOS 릴리스: `[Ok("one"), Ok("three"), Ok("two")]`(먼저 기다린 태스크가 나중 줄을 받음).
  - 쓰기도 같다(40 KB 줄을 세 태스크가 `fs.write_line(h, …)`).
  - **리눅스 릴리스는 앞 태스크가 영영 주차할 것으로 본다(코드 추론, 실행 미검증).**
- 원인 위치: `prelude/io.rs:129`(`pipe_lines`)와 `native/fs.rs:1183`(`write_pipe_line`)이 핸들마다 차례를 세우지 않고 곧장 `park_io`로 같은 fd·방향을 등록한다. `vm/world.rs:137`의 `debug_assert!`는 릴리스에서 사라진다.
  kqueue는 같은 ident·필터의 `EV_ADD`가 토큰을 덮어쓰고(`poller/kqueue.rs:74`), epoll 쪽도 `w.read = Some(token)`으로 덮어쓴다(`poller/epoll.rs:117`). macOS는 `native/pipe.rs:311`의 `relook`(다시 보기 타이머)이 앞 태스크를 우연히 구해 준다. 리눅스는 `relook`이 `None`이고, 주차가 `timed`라서 교착 판정도 하지 않는다.
- 소켓은 연결마다 차례 줄(`IoStep::Park`)이 있고 표준 입력은 읽기 스레드가 맡아 괜찮다.
- 고치는 법: FIFO 핸들에도 소켓과 같은 읽기·쓰기 차례 줄을 둔다(추천). 또는 `World::add_io`가 같은 fd·방향의 두 번째 등록을 릴리스에서도 줄 세운다. 시험은 리눅스 CI에서 위 재현을 그대로 쓴다.
- 처리: 이 커밋, 추천대로. 연결의 `Turnstile`을 `native/turn.rs`로 옮겨(`Hold`) 연결과 FIFO 핸들이 함께 쓴다. `native/file.rs`의 `File::turns`·`PipeTurn`, `prelude/io.rs` `pipe_lines`와 `native/fs.rs` `write_pipe_line`이 차례를 잡은 뒤에만 이벤트 루프에 등록하고 나머지는 `IoStep::Park`로 줄에 선다. 시험 `native/fs_tests.rs` `readers_of_one_fifo_handle_take_turns`·`writers_of_one_fifo_handle_take_turns`(차례 없이는 디버그 E0801로 실패함을 확인). 리눅스 epoll 경로는 실행하지 않았다(코드 추론: 차례를 쥔 태스크 하나만 fd·방향을 등록하므로 `w.read = Some(token)` 덮어쓰기가 생기지 않는다).

### CC15. `print`·`eprint`가 실행 스레드에서 막히면 모든 태스크, 기한, 첫 신호가 멈춘다 — 버그·결정, 중간
- 재현 1: 1 KB 줄 200개를 `timeout(…, 200ms)` 안에서 찍고, 표준 출력을 2초 뒤에야 읽는 쪽에 물린다(`fung run b1.fung | (sleep 2; wc -c)`) → `printer result: nil after 2s`. 200ms 기한이 2초 동안 보이지 않았고 결과도 `Err timeout`이 아니다.
- 재현 2: `with(() => 1, x => each(0..200, _ => print(line)), x => eprint("released"))`를 아무도 읽지 않는 파이프에 물리고 SIGINT를 한 번 보낸다. 아무 일도 없고 `release`도 돌지 않는다. 두 번째 SIGINT에야 130으로 끝난다.
- 원인 위치: `prelude/io.rs:57`·`:72`·`:80`이 실행 스레드에서 블로킹 `write_all`을 한다.
- 영향: `http.serve`로 서비스하면서 `print`로 로그를 느린 파이프에 쓰면 받는 쪽의 역압이 서버 전체를 세운다. 한계 목록(`manual/vm/12-scheduler.md`)에도 open-questions에도 없다.
- 고치는 법(결정): (1) **추천** 표준 출력·에러가 파이프·소켓이면 비차단으로 두고, 막히면 FIFO 쓰기처럼 `park_io`로 기다린다. (2) 그대로 두고 "알려진 한계"와 스케줄러 장에 적는다.
- 처리: 이 커밋, 결정 D64(쓰기 스레드). `vm/output.rs`: `print`·`eprint`·`dbg`는 쓰기 스레드의 큐에 넣고, 아직 쓰지 못한 바이트가 `ROOM`(64 KB)을 넘으면 바깥이 깨우는 주차(`write_std`). 표준 출력·에러가 같은 파일이면 줄 하나로 순서를 지키고 다르면 따로(`std_streams_apart`, `Runtime::with_streams`). 실행의 끝(`run_root`)·`release`·drop에서 다 쓴다. 쓰기 실패는 다음 쓰기나 실행의 끝의 E0801. 시험 `vm/output_tests.rs`, `fung-cli/tests/signals.rs` `a_deadline_ends_a_print_to_a_full_pipe`·`the_first_signal_stops_a_print_to_a_full_pipe`. 측정 `docs/notes/bench-languages.md` 4.10절. 매뉴얼 VM 13장 "표준 출력과 에러".

### CC16. 보호막(`release`)이나 범위 대기에서 돌아온 자리에서 지난 기한을 확인하지 않는다 — 버그, 중간 (다시 확인)
- 재현:
  ```
  r = timeout(fn():
    scope(fn(s):
      spawn(s, fn():
        with(() => 1, x => x, x => sleep(100ms))
      ;;)
      nil
    ;;)
    print("after scope: deadline passed 80ms ago, still running")
    "returned normally"
  ;;, 20ms)
  print(r)
  ```
  그 `print` 줄이 찍히고 `returned normally`가 나온다. 같은 태스크의 `with`도 같다.
- 근거: `concurrency.md` `timeout` "기한이 지나면 `f` 안에서 돌던 모든 것을 취소하고 `Err({kind: 'timeout})`을 돌려준다". `sleep`·채널·소켓에서 깨어나면 `after_park`(`vm.rs:512`)가 시계를 보고 곧바로 되감는다.
- 원인 위치: 태스크의 노드를 되돌려 놓는 두 곳이 깃발만 보고 시계를 보지 않는다: `vm/cleanup.rs:100`(`t.fiber.cancel = outer;`), `vm/scope.rs:708`(`t.fiber.cancel = d.own;`). 같은 일을 하는 `restore_cancel_node`(`vm/block.rs:542`)는 지난 기한을 그 자리에서 집행한다.
- 고치는 법: 두 자리에서 `restore_cancel_node`를 쓴다.
- 처리: 이 커밋, `vm/cleanup.rs` `released`와 `vm/scope.rs` `resume`이 `restore_cancel_node`로 노드를 되돌린다. 시험 `tests/lang/timeout_after_release.fung`.

### CC17. `await`·`timeout`·`scope`·`supervise`·`conc.retry`·`fs.with_open`은 "맨 값 또는 `Err`", `recv`는 `Ok`로 감싼다 — 결정, 중간 (라이브러리 영역의 같은 항목을 합침)
- 재현:
  ```
  print(scope(s => await(spawn(s, () => 5))!))   # E0605: `!` on a value of type int
  print(timeout(() => 5, 1s)!)                    # E0605 (성공한 때에만)
  print(timeout(() => Err({kind: 'timeout, message: "fake"}), 1s))   # 런타임의 것과 구별할 수 없음
  ```
- 문제: `!`·`?`를 붙일 수 있는지가 넘긴 `f`의 반환에 달려 있고, 사용자 함수가 돌려준 `Err`와 런타임이 만든 `Err`가 같은 모양이다. 규칙이 문서에 없고 결정 기록도 찾지 못했다.
- 고치는 법(결정):
  - (1) `await`·`timeout`이 성공을 `Ok(v)`로 감싼다. `await(t)!`가 되고 구별도 된다. 대신 `f`가 결과값을 돌려주면 `Ok(Ok(x))`가 된다.
  - (2) 그대로 두고 `functions.md` "읽는 법"에 통과 함수 목록과 "`!`·`?`를 쓰려면 `f`가 결과값을 돌려준다"를 적는다.
  - (3) 통과 함수의 `f`는 결과값만(아니면 E0605).
  - 두 영역의 추천이 갈렸다: 동시성 영역은 기조대로면 (1), 라이브러리 영역은 (2)((1)은 `Ok(Ok(x))`가 더 나쁘고 (3)은 `scope`·`await`까지 묶으면 너무 넓다). `timeout(() => http.get(…), 2s)` 같은 흔한 꼴이 걸리므로 사용자 결정이 필요하다.
- 처리: 이 커밋, 결정 D65((1)을 넓힘): 규칙 하나 "실행 중에 실패할 수 있는 함수는 결과값을 돌려준다". `await`(`vm/task.rs` `outcome`이 결과를 `Ok(v)`로 저장), `timeout`(`vm/deadline.rs`), `scope`·`supervise`(`vm/scope.rs` `settle`), `fs.with_open`·`fs.with_temp_dir`(`extras/fs.fung` `_with_ok`)가 `f`의 값을 `Ok`로 감싼다. 훑어보고 그대로 둔 것: `with`·`swap`(런타임 `Err`를 만들지 않음), `conc.pmap`(런타임 에러는 실패로 올라가고 범위가 스스로 취소되지 않음, 결과 리스트 그대로), `conc.fan_in`(받는 끝), `http.serve`의 `handler`·`ready`(값이 답이거나 버려짐), `until`·`iterate`. `conc.retry`는 `f`의 결과값 그대로이고 `f`가 결과값이 아니면 E0605(`nil`은 E0604; 새 `_native.arg.result`). E0607은 그대로 태스크 함수의 값 안을 본다(저장된 `Ok(v)`의 `v`). `extras/http.fung`의 `_within`은 `Ok`를 벗기고 `_call`은 `Ok(Ok(res))`로 가른다. 명세 `results.md`(규칙 문단)·`functions.md` "읽는 법", `concurrency.md`·`fs.md`·`conc.md`, 배우기 12·14장, 예제 모음 셋, 설계 05("통과 함수의 `Ok`")·동시성 v2 스펙. 저장소의 `.fung`·매뉴얼 펜스·Rust 시험 프로그램을 `!`·`?`·`match`로 고침. 시험 `tests/lang/pass_through_ok.fung`·`err_retry_not_result.fung`.

### CC18. REPL: 교착이 보고된 입력 중에 Ctrl-C를 누르면 세션이 말없이 끝난다 — 버그, 낮음
- 재현(표준 입력에 넣고 0.5초 뒤 SIGINT 한 번):
  ```
  (tx, rx) = channel()
  scope(s => spawn(s, () => with(() => 1, x => recv(rx), x => sleep(1s))))
  print("next input ran")
  ```
  SIGINT 없이는 E0702 보고 뒤 세션이 이어진다. `release`가 도는 1초 사이에 SIGINT 한 번이면 다음 입력을 돌리지 않은 채 종료 코드 1로 끝난다.
- 원인 위치: `crates/fung-cli/src/repl/mod.rs:297`~`302`. 끊긴 입력이면 `recover_deadlock()`(`:333`)을 건너뛴다.
- 고치는 법: 끊긴 경로에서도 `deadlocked`면 `recover_deadlock()`을 먼저 부른다.
- 처리: 이 커밋, `fung-cli/src/repl/mod.rs`: 끊긴 입력의 경로에서도 `recover_deadlock()`. 시험 `tests/signals.rs` `repl_ctrl_c_after_a_deadlock_goes_on`.

### CC19. "취소·시간 초과의 `Err`는 세지 않는다"는 문서와 달리, 함수가 돌려준 런타임의 `Err`는 E0607이다 — 문서, 낮음
- 재현: `scope(fn(s): spawn(s, () => timeout(() => sleep(1s), 10ms)); nil ;;)` → `error[E0607]: task's Err was never awaited: timed out after 10ms`.
- 문서: `manual/reference/prelude/concurrency.md:272`, `docs/design/05-concurrency.md:127`. 실제로 세지 않는 것은 태스크 자신이 취소되어 끝난 경우뿐이다.
- 고치는 법: 구현이 더 단순하고 엄격하므로 문서를 고친다.
- 처리: 이 커밋, `concurrency.md` `await`와 설계 05를 동작대로(태스크 자신이 취소되어 끝난 결과만 세지 않고, 함수가 돌려준 런타임의 `Err`는 셈).

### CC20. 레퍼런스가 취소 지점과 차례 넘기기를 실제와 다르게 적는다 — 문서, 낮음
- `cancel`(`concurrency.md:419`): "다음에 기다리는 자리에서 멈추고". 실제로는 다음 호출(내장 함수 포함)에서 멈춘다. VM 책(`14-cancellation.md:139`)과 설계 스펙은 맞게 적었다.
- `spawn`(`:216`): "태스크는 지금 태스크가 기다리는 틈에 돈다". 실제로는 호출 1,000번마다도 차례가 넘어간다.
- 받는 쪽이 여럿일 때 먼저 기다린 쪽이 먼저 받는다는 것(구현은 FIFO)도 레퍼런스에 없다.
- 처리: 이 커밋, `concurrency.md`: `cancel`은 다음 호출(내장 함수 포함)이나 기다리는 자리에서 멈춤, `spawn`은 기다리는 자리와 호출 1,000번마다 차례가 넘어감, `recv`·`send`는 먼저 기다린 쪽부터.

### CC21. 스케줄러의 바퀴마다 드는 작은 비용 — 성능, 낮음
spawn·await만 반복하는 프로그램의 표본(1518개): `mach_absolute_time` 7.4%(`Sched::timers_due`(`vm/sched.rs:458`)가 타이머가 없어도 바퀴마다 `Instant::now()`, `Sched::park`(`:525`)도 기한이 없어도 부름),
`pthread_mutex_lock`+`unlock` 2.9%(실행 스레드끼리의 깨움도 `Ready`의 뮤텍스, `:96`), SipHash + `HashMap::insert` 3.6%(`Sched::slots`(`:167`), `CancelNode`의 `children`(`vm/task.rs:65`)).
합쳐 10~14%로 추정한다(고친 뒤 재지 않았다). 고치는 법: 타이머 힙이 비었으면 시계를 보지 않는다. `slots`는 번호가 순서대로이니 슬랩이나 가벼운 해셔로.
- 처리: 2ed22e63, `timers_due`는 타이머가 없으면 시계를 읽지 않고, `park`·`after_park`는 기한이 있을 때만 읽는다. `Sched::slots`와 `CancelNode`의 `children`은 SipHash 대신 곱셈 해셔(`IdMap`). `Ready`의 뮤텍스는 다투지 않아 그대로. spawn·await 루프 0.367 → 0.325초(−11%). `docs/notes/bench-languages.md` 4.12절.

### CC22. `scope`·`supervise`의 핸들이 본문 밖으로 나온다 — 결정, 낮음
- 재현: `scope(s => s)` → `<scope>`, `cancel(scope(s => s))` → `nil`.
- 고치는 법(결정): 닫힌 범위의 핸들을 돌려주는 것을 E0607류의 에러로 할지, 닫힌 범위에 대한 연산만 에러로 할지. 확인만 했고 선택지는 좁히지 않았다.
- 처리: 이 커밋, 결정 D66. `spawn`은 이미 E0706이었으므로 새 번호 대신 E0706을 넓혔다: 메시지 `` `%s` on a closed scope ``(`spawn`·`cancel`)와 도움말. `conc.rs` `cancel`이 닫힌 범위면 E0706. 시험 `tests/lang/err_cancel_closed_scope.fung`. 매뉴얼 `errors.md` E0706, `concurrency.md` `spawn`·`cancel`, 설계 05.

**측정**(user 시간, 최대 RSS): 태스크 10만 개 spawn 후 `await` 0.10초·73 MB, 태스크 10만 개가 `sleep`으로 주차 0.19초·200 MB, 버퍼 없는 채널 핑퐁 100만 메시지 0.60초·9.5 MB,
한 범위에서 `await(spawn(…))` 100만 번 1.34초·9.3 MB(끝난 태스크를 쥐지 않음), 일찍 끝나는 60초 `timeout` 90만 번 1.35초·9.4 MB(낡은 타이머가 쌓이지 않음),
`exec(["true"])` 3,000개 동시 1.9초 user + 5.2초 sys(남은 프로세스 없음).

## 9. 표준 라이브러리 (LB)

잘못된 인자는 E0403(매개변수 이름을 말함)·E0506·E0402/E0407로 일관되게 갈리고, 같은 실수에 다른 번호가 나오는 자리를 찾지 못했다.
"컬렉션 = 리스트·범위·받는 끝" 규칙은 27개 함수(`first`·`last`·`take`·`drop`·`map`·`filter`·`fold`·`each`·`find`·`any`·`all`·`try_map`·`flatten`·`reverse`·`sort`·`sort_by`·`unique`·`group_by`·`sum`·`min`·`max`·`join`·`zip`·`for` 등)에서 한 칸도 어긋나지 않는다.

**돌려주는 값의 관례**(★가 벗어나거나 규칙에 없는 갈래):

| 분류 | 함수 | 실제 동작 | 판정 |
|---|---|---|---|
| 없음 → `nil` | `first`·`last`·`min`·`max`(빈 입력), `find`, `get`, `env`, `regex.find` | `nil` | 규칙대로 |
| 바깥 글 해석 | `parse_int`·`parse_float`·`parse_dur`, `json.parse`, `time.parse`, `cli.parse` | `Ok`/`Err` | 규칙대로 |
| 바깥 글 해석 ★ | `sym(s)`, `time.from_unix(n)` | E0513·E0403만 | LB53 |
| 파일·명령·네트워크 | `fs.*`, `exec`·`sh`, `http.*`, `read_line` | `Ok`/`Err({kind, message})` | 규칙대로, `kind`는 LB45·LB48 |
| 흐름의 끝 ★ | `read_line` 끝 → `Ok(nil)`, `recv` 끝 → `Err('closed)` | 같은 "정상적인 끝"이 둘 | LB52 |
| 닫기 ★ | `close(tx)` 두 번 → `Ok(nil)`, `close(file)` 두 번 → `Err('closed)` | 한 함수에 두 규칙 | LB47 |
| 통과 함수 ★ | `timeout`, `await`, `scope`·`supervise`, `conc.retry`, `fs.with_open` | "f의 맨 값" 또는 `Err` | CC17 |
| 코드의 잘못 ★ | `env("A=B")`·`env("")` → `nil` | `exec`의 `env`는 같은 이름에 E0403 | LB54 |

### LB41. `fs.read`가 문자열마다 64 KB 용량을 쥔 채 돌려준다 — 버그, 높음 (다시 확인; 동시성 점검의 부수 발견)
- 재현: `import fs` / `print(len(map(0..20000, i => fs.read("small.txt"))))`(12바이트 파일) → 최대 RSS 351 MB. 428바이트 파일 4만 개면 2.15 GB. 결과를 쥐지 않으면 11 MB.
- 원인 위치: `native/fs.rs:361`의 `buf.reserve(64 * 1024)`로 만든 `Vec`가 줄이지 않은 채 `Value::bytes`(`value.rs:219`)로 들어간다. `shrink_to_fit`이 `crates/fung-vm/src` 어디에도 없다. 직전 커밋의 11-10 LB29 수정(`read_within`)에서 생긴 것으로 보인다(확인하지 않은 추정).
- 고치는 법: 돌려주기 전에 `buf.shrink_to_fit()`. 또는 `metadata`의 크기로 한 번에 잡는다. `conc.pmap(paths, fs.read)`가 흔한 쓰임이라 급하다.
- 처리: 이 커밋, `native/fs.rs` `read_within`·`read_fifo`가 돌려주기 전에 `shrink_to_fit`. 시험 `native/fs_tests.rs` `read_keeps_no_spare_room`.

### LB42. `http.serve` 핸들러 응답 맵의 모르는 키를 말없이 무시한다 — 결정, 중간
- 재현: 핸들러가 `{statu: 404, boddy: "x"}`를 돌려주면 200과 빈 본문. 에러 줄도 없다. 같은 핸들러에서 `{status: "200"}`·`{body: 5}`는 `http: bad handler response: …`와 500이다.
- 원인 위치: `extras/http.fung:1907-1920` `_parts`가 `"status"`·`"body"`·`"headers"`만 본다.
- 고치는 법(결정): (가) **추천** 세 키 밖의 키는 `bad handler response: unknown key`(500 + 표준 에러 한 줄). (나) 그대로.
- 처리: 이 커밋, 결정 D67: `extras/http.fung` `_parts`가 세 키 밖의 키를 ``unknown key `statu`, expected status, body or headers``로 잘못된 답(500 + 표준 에러 한 줄). 레퍼런스 `http.md`, 설계 06. 시험 `crates/fung-cli/tests/http.rs` `the_server_checks_answers_and_requests`.

### LB43. `fs.glob`의 `**`가 말없이 `*`다 — 결정, 중간
- 재현(`m1/m2/x.txt`가 있는 디렉터리): `fs.glob("**/*.txt")` → `Ok([])`.
- 문서에 "`**`는 `*`와 같다"고 있지만 결과가 에러가 아니라 더 적은 파일이다. 같은 함수가 잘못된 패턴은 E0515로 막는다.
- 고치는 법(결정): (가) **추천** `**`는 E0515(`help: use fs.walk`). (나) 재귀로 구현. (다) 그대로.
- 처리: 이 커밋, 결정 D68: `native/glob.rs` `double_star`(이스케이프·클래스 밖의 `**`) → `Failure::DoubleStar`, `native/fs.rs` `glob_fn`이 E0515(도움말 `fs.walk`와 `filter`). 레퍼런스 `fs.md`·`errors.md`, 설계 06. 시험 `glob.rs` `a_double_star_is_refused`, `fs_tests.rs` `list_walk_glob`.

### LB44. `cli.parse`는 모르는 옵션을 거절할 길이 없다 — 결정, 중간 (첫 줄 다시 확인)
- 재현:
  ```
  cli.parse(["--dry-rnu", "prod"]) → Ok({flags: {dry_rnu: "prod"}, rest: []})
  cli.parse(["-v", "--x", "1"])    → Ok({flags: {x: "1"}, rest: ["-v"]})
  cli.parse(["---x"])              → Ok({flags: {_x: true}, rest: []})
  cli.parse(["--a b", "c"])        → Ok({flags: {"a b": "c"}, rest: []})
  ```
- 문제: `--dry-run`의 오타가 "실제로 실행"이 된다. 11-08 LB11·11-10 LB38은 `bools`와 반복만 다뤘다.
- 고치는 법(결정): (가) **추천** `values` 인자를 더해, `bools`·`values` 중 하나라도 주면 목록 밖의 `--이름`은 `Err('invalid)`. 옵션 이름은 식별자 모양만. (나) 언제나 닫힌 집합(목록 필수; 예외는 가장 적지만 한 줄 스크립트의 편의와 맞바꿈). (다) 그대로.
- 처리: 이 커밋, 결정 D69(점검 문서의 (나)에 가까운 "늘 닫힌 집합"): `extras/cli.fung` `parse(argv, bools = [], values = [])`, 목록 밖의 `--이름`·`-x`·값 없는 값 옵션은 `Err('invalid)`, 이름 검사와 두 목록의 겹침은 `_native.arg.options`(E0403). 저장소의 호출(예제 5개, 레시피 9개, 배우기 11장, 레퍼런스)을 고침. 레퍼런스 `cli.md`·레시피 `cli-options.md`, 설계 06. 시험 `tests/lang/cli_parse.fung`·`err_cli_option_name.fung`·`err_cli_option_twice.fung`, `extras_arguments.rs`.

### LB45. http의 실패 `kind`가 전부 `'network`다 — 결정, 중간
- 재현: 연결 거부, `ftp://x/`(unsupported scheme), `http://`(no host), `http://exa mple.com/`, 포트 `99999`, `max_body: 3` 초과, `http.serve("a", h)`(missing port)가 모두 `Err({kind: 'network, …})`.
- 문제: 다시 해 볼 만한 실패와 몇 번을 해도 같은 실패를 문구 없이는 가를 수 없다. `conc.retry(() => http.get(u))`가 잘못된 URL에도 끝까지 다시 시도한다.
- 고치는 법(결정): (가) **추천** URL·스킴·주소·프록시 값의 문법과 본문 한도는 `'invalid`(목록에 이미 있는 kind), 전송 실패는 `'network`. (나) 문서에 적음.
- 처리: 이 커밋, 결정 D70: `extras/http.fung` `_invalid`(URL·스킴·포트·프록시 값·`Location`·프록시 스킴)와 `_public_kind`(본문 한도 `'limit` → `'io`), `native/net.rs` `Fail::Invalid`(주소·포트), `native/tls.rs`(서버 이름). `http.serve`의 주소도 `'invalid`. 레퍼런스 `http.md`·prelude `results.md`("실패의 종류" 표), 설계 06. 시험 `http.rs` `failure_kinds_tell_mistakes_from_failures`, 기대 출력 `http/*.txt`, `net_tests.rs`.

### LB46. `time.parse`의 `Z` 토큰이 `"Z"`를 읽지 못한다 — 결정, 중간 (첫 줄 다시 확인)
- 재현(`TZ=UTC`):
  ```
  time.parse("2026-10-04T09:05:07Z", "YYYY-MM-DDTHH:mm:ssZ")      → Err({kind: 'parse, … expected a time zone offset})
  time.parse("2026-10-04T09:05:07+00:00", "YYYY-MM-DDTHH:mm:ssZ") → Ok(2026-10-04T09:05:07Z)
  time.format(x, "YYYY-MM-DDTHH:mm:ssZ")                          → 2026-10-04T09:05:07+00:00
  time.format(x)                                                  → 2026-10-04T09:05:07.123456789Z
  ```
- 문제: 기본 출력과 형식 없는 `time.parse`는 UTC를 `Z`로 쓰고 읽는데 토큰 `Z`는 거절한다. `Z`가 토큰이라 글자 `Z`를 형식에 적을 수도 없다(이스케이프는 보류 목록).
- 고치는 법(결정): (가) **추천** 읽을 때 `Z`·`ZZ` 토큰이 `Z`도 받는다. (나) (가) + 쓸 때 UTC면 `Z`. (다) 그대로.
- 처리: 이 커밋, 결정 D71(점검 문서의 (나)): `native/timefmt.rs` `zone`이 오프셋 0을 `Z`로 쓰고 `parse_zone`이 `Z`·`z`를 받는다(`Z`·`ZZ` 둘 다). 레퍼런스 `time.md`, 설계 06. 시험 `timefmt_tests.rs` `zone_tokens_write_and_read_z_for_offset_zero`, `time_tests.rs`, `tests/lang/time_module.fung`.

### LB47. `close(x)`가 보내는 끝과 파일에서 다르게 군다 — 결정, 중간 (채널 쪽 다시 확인)
- 재현: `close(tx)` 두 번 → `Ok(nil) Ok(nil)`. `close(file)` 두 번 → `Ok(nil) Err({kind: 'closed, …})`.
- `fs.with_open`이 이 차이 때문에 `Err(e) when e.kind == 'closed => nil`을 따로 쓴다(`extras/fs.fung:44-50`).
- 고치는 법(결정): (가) **추천** 둘 다 멱등 `Ok(nil)`(닫힌 핸들의 읽기·쓰기가 `Err('closed)`인 것은 그대로). (나) 둘 다 `Err('closed)`. (다) 그대로.
- 처리: 이 커밋, 결정 D72(둘 다 `Err('closed)`): `conc.rs` `close`가 닫힌 채널(받는 끝이 모두 사라진 것도)의 보내는 끝에 `Err({kind: 'closed})`. 소켓(`_net.close`)과 파일은 이미 그랬다. `extras/conc.fung` `fan_in`은 `_ = close(tx)`, `fs.with_open`의 특례는 그대로. 레퍼런스 `concurrency.md`, 설계 05. 시험 `channel_tests.rs` `closing_a_closed_channel_is_an_err`.

### LB48. `exec`·`sh`와 `fs`의 `kind`가 같은 원인에 다르다 — 결정, 중간
- 재현:
  ```
  exec(["/etc/hosts"])      → Err({kind: 'not_found, … permission denied})
  exec(["echo", "a\x00b"])  → Err({kind: 'not_found, … argument contains a NUL byte})
  sh("", cwd: "o.txt")      → Err({kind: 'not_found, … not a directory})
  fs.list("o.txt")          → Err({kind: 'io, … not a directory})
  fs.remove("비지 않은 디렉터리") → Err({kind: 'exists, … directory not empty})
  ```
- 고치는 법: `exec`는 errno대로(`ENOENT` → `'not_found`, `EACCES` → `'permission`, NUL → `'invalid`). `ENOTDIR`은 한 가지로. LB45와 한 묶음.
- 처리: 이 커밋, 결정 D70: `native/oserr.rs` `kind`가 errno 표 하나(`ENOTDIR` → `'not_found`, `EINVAL`·NUL → `'invalid` 더함), `native/process.rs`가 시작 실패마다 그 표의 kind(`Why`). 레퍼런스 `process.md`·`fs.md`·`results.md`, 설계 06. 시험 `fs_tests.rs` `kinds_follow_the_error_classes`, `boot/tests.rs` `commands_that_cannot_run_are_errs`, 기대 출력 `fs/runs.txt`·`prelude/runs.txt`.

### LB49. 용도(SRE 스크립트)에 비춰 빠진 함수들 — 결정, 중간
보류 목록에 이미 있는 것(`math`, `random`, `hash`, `base64`, `csv`, `regex.compile`, 시간 토큰 이스케이프·달력 산술, http 스트리밍·쿠키)은 뺐다. 실행으로 없음을 확인한 것:
- `regex.escape(s)`: 바깥 글을 패턴에 글자 그대로 넣을 길이 없다. 부분 문자열의 위치를 얻는 유일한 길이 `regex.find(…).start`라 더 아프다(`index_of` 없음).
- URL 인코딩: 쿼리 값에 `&`·`=`·`+`·`#`이 있으면 URL을 조립할 수 없다.
- 줄바꿈 없는 표준 출력: `print`는 늘 `\n`을 붙인다. 지금은 `fs.write("/dev/stdout", s)`로만 된다.
- `http`의 리다이렉트 끄기: 상태 점검이 301/302 자체를 볼 수 없다.
- 한쪽 `trim`, `exec`의 출력 흘려보내기, `fs.stat`의 링크 여부·권한, 필드에서 시각 만들기·이름 있는 시간대.
- 추천 순서: `regex.escape` → 줄바꿈 없는 출력 → URL 인코딩 → 리다이렉트 끄기 → 나머지.
- 처리: 이 커밋, 결정 D73: `regex.escape`(`native/regex.rs`), prelude `write`·`ewrite`(`prelude/io.rs`, 출력 스레드), `trim_left`·`trim_right`·`index_of`(`prelude/strings.rs`; `index_of`는 글자 인덱스), `http.url_encode`·`url_decode`(`native/net.rs`), `http.get`·`post`·`request`의 `redirects`. 효과 표(`vm/atom.rs`)에 넣어 spec이 `pure`를 표시. 레퍼런스 `strings.md`·`io.md`·`regex.md`·`http.md`, 설계 06, open-questions 보류(`exec` 흘려보내기, `fs.stat` 링크·권한, 필드에서 시각·이름 있는 시간대). 시험 `tests/lang/regex_escape.fung`·`write.fung`·`trim_sides.fung`·`index_of.fung`·`http_url_encode.fung`과 `err_*`, `output_tests.rs`, `http.rs` 리다이렉트 사례.

### LB50. 빈 문자열 인자의 답이 함수마다 다르다 — 결정, 낮음
- 재현: `split("abc", "")` → E0403(11-10 D18), `pad_left("x", 3, "")` → E0403, `replace("abc", "", "-")` → `"-a-b-c-"`, `trim("xax", "")` → `"xax"`.
- 추천: `replace`의 빈 `old`와 `trim`의 빈 `cut`은 E0403. `regex.split`의 빈 매치는 엔진 규칙이라 둔다.

- 처리: 이 커밋, 결정 D74: `prelude/strings.rs` `replace_fn`(빈 `old`의 길 삭제)·`trim_fn`이 E0403(`not_nonempty`). 레퍼런스 `strings.md`, 설계 06. 시험 `tests/lang/err_replace_empty.fung`·`err_trim_empty.fung`, prelude 실행 사례.
### LB51. http 클라이언트가 URL을 말없이 고치고, 규칙이 매뉴얼에 없다 — 결정·문서, 낮음
- 재현: 보낸 `/echo?a=1&b=x y&c=%zz` → 서버가 받은 `/echo?a=1&b=x%20y&c=%25zz`. 잘못된 이스케이프 `%zz`가 `%25zz`로 바뀐다(서버 쪽은 같은 것을 400으로 거절).
- 추천: 잘못된 `%` 이스케이프는 `Err`, 고치는 글자의 목록을 "약속"에 적는다.
- 처리: 이 커밋, 결정 D75: `extras/http.fung` `_parse`가 쿼리의 `%`도 `_escape_error`로 검사해 `Err('invalid)`(경로·조각·호스트는 이미 그랬다). 고쳐 보내는 글자 목록은 레퍼런스 `http.md` "약속". 시험 `http.rs` `the_query_is_escaped_in_the_request_line`, `tests/lang/http_url_encode.fung`.

### LB52. 흐름의 끝: `read_line`은 `Ok(nil)`, `recv`는 `Err('closed)` — 문서, 낮음
`'closed`가 파일에서는 "닫은 핸들을 씀", 채널에서는 "정상적인 끝"이다. 채널은 `nil`을 보낼 수 있어 `Ok(nil)`을 못 쓴다는 이유는 타당하다. 그대로 두고 kind 목록에 두 뜻을 적는다.
- 처리: 이 커밋(W12). 레퍼런스 `prelude/results.md` "실패의 종류" 아래에 `'closed`의 두 뜻과 흐름의 끝 두 모양(`read_line`은 `Ok(nil)`, `recv`는 `Err('closed)`, 까닭은 채널이 `nil`을 보낼 수 있음). 같은 표의 `try_recv`·`try_send`를 지금 이름 `recv_now`·`send_now`로 고침.

### LB53. 바깥 값을 받는데 `Err` 길이 없는 변환: `sym(s)`, `time.from_unix(n)` — 문서, 낮음
`sym`의 문서가 "파일이나 JSON에서 읽은 글"을 예로 드는데 실패는 E0513뿐이다. `time.from_unix(253402300800)`은 E0403. `sym`은 문서에 "바깥 글은 먼저 허용 목록으로 검사"를 적는 것으로 충분하다.
- 처리: 이 커밋(W12). 레퍼런스 `convert.md` `sym`의 상자에 "실패는 `Err`가 아니라 E0513이니 바깥 글은 허용 목록으로 먼저 검사"와 예, `time.md` `time.from_unix`에 "범위 밖은 E0403이니 바깥 수는 먼저 범위 확인"(UTC 기준 `-62167219200`~`253402300799`, 확인함).

### LB54. `env("A=B")`·`env("")`가 `nil`이다 — 결정, 낮음
`exec(…, env: {"A=B": "1"})`는 E0403인데 읽는 쪽은 있을 수 없는 이름에 `nil`. 추천: 같은 검사로 E0403.
- 처리: 이 커밋, 결정 D76: `native/process.rs` `env_fn`이 빈 이름·`=`·NUL을 E0403. 설계 06. 시험 `tests/lang/err_env_name.fung`, prelude 실행 사례.

### LB55. `flatten`이 리스트가 아닌 원소를 통과시킨다 — 결정, 낮음
- 재현: `flatten([1, [2], "ab", 0..2])` → `[1, 2, "ab", 0..2]`.
- 문서에 적힌 동작이지만 `map` 뒤의 `flatten`에서 "어떤 때는 리스트, 어떤 때는 값"인 버그를 숨긴다. 추천: 원소가 리스트(·범위)가 아니면 E0506. 원인 `crates/fung-vm/src/prelude/collections.rs:547-551`.

- 처리: 이 커밋, 결정 D77: `prelude/collections.rs` `flatten_fn`·`spliced_len`이 리스트와 범위 원소를 펼치고 다른 원소는 E0506(``cannot flatten an element of type int``, 도움말 `[x]`). 레퍼런스 `collections.md`, 설계 06. 시험 `tests/lang/err_flatten_scalar.fung`, prelude 실행 사례.
### LB56. `fs`의 같은 경로·파일 인자 — 결정, 낮음
- 재현: `fs.move("o.txt", "o.txt")` → `Ok(nil)`, `fs.copy("o.txt", "o.txt")` → `Err('exists)`, `fs.walk("o.txt")` → `Ok(["o.txt"])`(문서: dir), `fs.open(".")` → `Ok(<file ".">)`(읽을 때 `Err`).
- 추천: `move`도 대상이 있으면 `'exists`, `walk`는 디렉터리만, `open`은 디렉터리면 열 때 `Err`.
- 처리: 이 커밋, 결정 D78: `native/fs.rs` `rename`(같은 파일이면 대소문자만 바꾸는 이름 바꾸기 밖은 `EEXIST`, `overwrite`면 `Ok`), `walk`(뿌리는 `metadata`로 디렉터리만, 아니면 `ENOTDIR`), `native/file.rs` `open`(읽기로 연 디렉터리는 `EISDIR`). 레퍼런스 `fs.md`, 설계 06. 시험 `fs_tests.rs` `failures_give_exact_messages`·`list_walk_glob`·`listings_keep_gos_order`·`a_directory_does_not_open`, 기대 출력 `fs/runs.txt`.

### LB57. `fs.write_line`이 줄바꿈이 든 `line`을 받는다 — 결정, 낮음
`fs.write_line(f, "a\nb")` → `Ok(nil)`, 파일에는 두 줄. 문서는 "쓸 문자열(줄바꿈 없이)". 추천: `\n`이 있으면 E0403.
- 처리: 이 커밋, 결정 D79: `native/fs.rs` `write_line_fn`이 `\n`이 든 `line`을 E0403. 레퍼런스 `fs.md`, 설계 06. 시험 `tests/lang/err_write_line_newline.fung`.

### LB58. `json.stringify`의 말없는 변환 (기존 결정 재검토) — 결정, 낮음
- 재현: `json.stringify(['a, (1, 'b), {k: (2,)}])` → `["a",[1,"b"],{"k":[2]}]`. `1s`·`0..3`·`Ok(1)`은 E0513. `json.stringify(x, 0) == json.stringify(x)`.
- 심볼·튜플·시각은 바꿔 주고 기간·범위는 거절한다. 원칙대로면 "JSON에 있는 타입만"이 가장 간단하지만 `Err` 맵의 `kind`를 그대로 찍는 편의가 크다. 결정으로 묻되, 유지한다면 `indent: 0`은 E0403으로.
- 처리: 이 커밋, 결정 D80: `native/json.rs` `Encoder`가 JSON 타입만(심볼·튜플·시각은 E0513과 도움말), `indent`는 1 이상. 저장소의 `json.stringify`(배우기 13장, 레퍼런스 `json.md`·`errors.md`, http 시험의 `req`)를 고침. 설계 06. 시험 `json_tests.rs`, `tests/lang/json_stringify_types.fung`·`err_json_stringify_*.fung`, 기대 출력 `json/programs.txt`.

### LB59. 서머타임 빈틈의 시각을 밀어서 받는다 (기존 결정 재검토) — 결정, 낮음
`TZ=America/New_York`: `time.parse("2026-03-08 02:30", "YYYY-MM-DD HH:mm")` → `Ok(2026-03-08T03:30:00-04:00)`. 같은 함수가 `2026-02-30`은 거절한다. 추천: 빈틈은 `Err('parse)`, 겹침은 지금 규칙 유지.
- 처리: 이 커밋, 결정 D81: `native/tz.rs` `Zone::skips`, `native/timefmt.rs` `parse_time`이 지역 시간대의 빈틈 시각을 `Err('parse)`(``no such local time: the clocks skip it (daylight saving)``). 레퍼런스 `time.md`, 설계 06. 시험 `timefmt_tests.rs` `a_wall_time_in_a_gap_is_refused`, `time_expected.rs`.

### LB60. `sum`이 문자열·리스트도 잇는다 — 결정, 낮음
`sum(["a", "b"])` → `"ab"`, `sum([[1], [2]])` → `[1, 2]`. `join`·`flatten`과 겹치고 빈 입력은 `0`이라 타입이 바뀐다. 추천: `sum`은 수·기간만.

- 처리: 이 커밋, 결정 D82: `prelude/collections.rs` `sum_fn`이 수·기간 밖의 원소를 E0506(``cannot sum str``)으로(원소 하나일 때의 특례 삭제). 레퍼런스 `collections.md`, 설계 06. 시험 `tests/lang/err_sum_str.fung`·`err_sum_list.fung`(`collections.fung` 고침).
### LB61. 문서와 이름의 작은 어긋남 — 문서, 낮음
- "모든 함수는 대상이 첫 번째 인자"가 사실이 아닌 자리: `fs.write(path, text)`·`fs.append`·`fs.write_line(file, line)`·`send(tx, x)`는 받는 곳이 앞이라 `text |> fs.write(p)`가 안 된다. `http.request(method, url, …)`, `spawn(s, f)`도 그렇다. 순서 자체는 타당하니 `functions.md`의 문장을 좁혀 쓴다.
- `_all`의 뜻이 셋: `env_all`(전부), `regex.find_all`(모든 매치), `fs.remove_all`(재귀). 매개변수 이름: `replace(s, old, new)`와 `regex.replace(s, pattern, repl)`. `fs.list(dir, full = false)`는 불리언 하나가 반환 모양을 바꾼다.
- 술어 `is_*`와 `contains`·`starts_with`·`fs.exists`는 "형용사·명사는 `is_`, 동사는 그대로"로 읽히므로 문서에 한 줄.
- `exec(["grep", pattern, "app.log"])`에서 `pattern`이 `-`로 시작하면 옵션이 된다. `process.md`의 주입 상자에 `--` 한 줄.
- 처리: 이 커밋(W12, 문서만). 레퍼런스 `functions.md` "읽는 법": "모든 함수는 대상이 첫 인자"를 "값을 다루는 함수"로 좁히고 받는 곳이 앞인 함수(`fs.write`·`fs.append`·`fs.write_line`·`send`·`spawn`·`http.request`)를 적음, 이름 규칙 한 줄(술어의 `is_`, `_all`의 세 뜻). `process.md`의 주입 상자에 `--`와, D47 뒤로 셸의 `"$PATTERN"`은 원시 문자열에 쓴다는 것. 이름 자체(`replace`·`regex.replace`의 매개변수, `fs.list`의 `full`)는 바꾸지 않았다(결정 없는 문서 항목, open-questions "남은 일").

## 10. CLI 도구·시험·CI (TL)

약 40가지 인자 조합의 종료 코드가 문서와 모두 맞았다. 컴파일 에러는 `check`와 `run`이 글자까지 같다. 쓰이는 에러 번호 118개 가운데 어디에도 시험이 없는 것은 0개다.

### TL9. 저장소에 퍼징·속성·차등 시험이 없다 — 정리(시험), 중간
- `proptest`·`quickcheck`·`fuzz`를 저장소 전체에서 찾았으나 없다. 이번 점검의 0건(1절)은 사람이 임시 스크립트를 돌려 얻었고, 지키는 장치가 없다.
- 고치는 법: `crates/fung-cli/tests/`에 고정 씨앗의 변이 시험을 크레이트 없이 둔다(xorshift 몇 줄, `fung_front`를 직접 부르면 1초에 수만 건). 변이: 바이트 뒤집기, 토큰 삭제·복제·교환, 자르기, 구두점·무작위 바이트 넣기, 다른 파일 조각 붙이기, 공백·주석 끼우기.
  속성 넷: (1) 패닉 없음, (2) 포맷 멱등 + 포맷 전후 `dump core` 같음, (3) `check`가 통과하면 `run`이 컴파일 에러를 내지 않음, (4) `dump bytecode` 결정성. VM22의 검증기를 (5)로 더한다.
  컴파일러 쪽은 "무작위 프로그램을 참조 평가기와 견주기"가 수명 분석(VM23)을 고칠 때 가장 쓸모 있었다.
- 처리: 이 커밋. `crates/fung-front/tests/fuzz.rs`(앞단이 실행 파일 없이 불리므로 `fung-cli`가 아니라 `fung-front`에): 씨앗은 `tests/lang`·`examples`·`extras`·`bench`·`crates/fung-cli/tests/data`의 모든 `.fung`, 크레이트 없는 xorshift64*(씨앗은 splitmix64로 섞음). 변이는 고치는 법의 것에 키워드·작은 리터럴 넣기, 줄바꿈·끝 쉼표 넣기, 줄 지우기·되풀이를 더해 한 번에 1~3개. 속성: (1) 패닉 없음(`catch_unwind`, 명령줄처럼 64 MiB 스택), (2) 포맷 멱등·포맷 전후 파스 트리 같음·컴파일되면 코어 트리 같음, (3) 시험 블록을 넣어 컴파일되면 빼고도 컴파일됨(`check` → `run`), (4) 두 번 컴파일한 원형이 같음(상수 해시 색인을 뺀 모든 공개 필드), (5) 검증기 통과. 변이는 실행하지 않는다. 기본 2000건(디버그 약 5초, 릴리스 약 1초), `FUNG_FUZZ_CASES`·`FUNG_FUZZ_SEED`, 실패하면 다시 돌리는 환경 변수와 조각을 지워 줄인 변이를 보인다. 다른 씨앗 넷으로 20만 건씩(릴리스 약 80초씩) 돌려 찾은 문제는 없다. "참조 평가기와 견주기"는 넣지 않았다(평가기를 따로 써야 해서 이 묶음의 범위 밖). 설계 09 "테스트".

### TL10. CI의 빈 곳 — 정리, 낮음
- `schedule`이 없어 `cargo audit`·`cargo deny`가 푸시 때만 돈다.
- 액션이 태그 고정(`actions/checkout@v5`, `actions/cache@v4`, `attest-build-provenance@v3`). 릴리스 작업은 쓰기 권한과 서명을 가진다.
- linux/arm64는 릴리스 때만 빌드·검사한다.
- `[lints]`가 없어 `clippy::undocumented_unsafe_blocks`·`unsafe_op_in_unsafe_fn`이 꺼져 있다(`unsafe` 블록 83곳, 모두 libc 호출).
- 처리: 이 커밋, 결정 D84. 새 `.github/workflows/audit.yml`이 주 1회(월요일 03:17 UTC)와 손으로 `cargo deny check`·`cargo audit`를 ci.yml과 같은 버전·캐시로 돌린다. 세 워크플로의 서드파티 액션을 지금 쓰던 태그가 가리키는 커밋 SHA로 고정하고 주석에 그 버전을 적음(`checkout` v5.1.0, `cache` v4.3.0, `upload-artifact` v4.6.2, `download-artifact` v4.3.0, `attest-build-provenance` v3.0.0). ci.yml의 `rust` 행렬에 `ubuntu-24.04-arm`(빌드·clippy·시험 두 번). 작업 공간 `Cargo.toml`의 `[workspace.lints]`에 `unsafe_op_in_unsafe_fn`·`clippy::undocumented_unsafe_blocks`를 `deny`로, 세 크레이트가 `[lints] workspace = true`. 걸린 곳은 둘이었다(`native/pipe.rs` `set_blocking`의 두 번째 `fcntl`, `native/process_tests.rs`의 `wait4`): 블록마다 SAFETY 주석. `unsafe fn`은 없다. CI 변경은 푸시 전이라 GitHub에서 돌려 보지 못했다(YAML 문법만 확인).

### TL11. `fmt`의 자잘한 것 — 버그·문서, 낮음
- 읽기 전용 파일: 고친 목록에 경로를 쓴 뒤 `error: cannot write`와 종료 2. 목록을 읽는 스크립트는 고쳐졌다고 본다.
- 링크와 대상이 같은 디렉터리에 있으면 링크 이름만 적힌다.
- 보간 안은 포맷하지 않는다(`"a${ 1+2 }b"` 그대로). CRLF는 LF로, BOM은 지워진다. 셋 다 `cli.md`에 없다.
- 사용 오류마다 사용법 12줄 전체를 찍는다.
- 처리: 이 커밋, `fung-cli/src/fmt.rs`: 쓴 파일만 목록에 적는다. 매뉴얼 `cli.md`에 보간 안은 포맷하지 않음·CRLF→LF·BOM 지움. 링크 이름과 사용법 12줄은 이 묶음에서 다루지 않았다. 시험 `main.rs` 읽기 전용 파일 경우의 표준 출력.

### TL12. `fung test`의 자잘한 것 — 결정, 낮음
이름으로 골라 돌릴 수 없다. 태스크를 쓰지 않은 실패에도 `scheduler seed` 줄이 붙는다. `<load>` 진단만 들여쓰지 않는다.
- 처리: 이 커밋, 결정 D83. `fung test`의 `scheduler seed` 줄은 태스크를 띄운 파일(`Runtime::spawned`, `spawn` 때 `World::spawned`)에서 실패가 있을 때만. `<load>` 진단(읽기 실패·컴파일·부팅)은 실패한 시험처럼 두 칸 들여 씀. 이름으로 고르기는 open-questions "M8 후속"의 보류. 시험 `tools_expected.rs` `test_shows_the_seed_only_when_tasks_ran`·`test_indents_load_errors`. 매뉴얼 `cli.md`·배우기 14장.

### TL13. REPL의 앞 참조 — 문서, 낮음
`fn g() = h()`가 E0204다(파일에서는 됨). 서로 부르는 두 함수는 한 줄에 `;`로 써야 한다. 문서에 없다.
- 처리: 이 커밋(W12). 여전히 그렇다(릴리스 바이너리로 확인: 따로 입력하면 E0204, `fn g() = h(); fn h() = 1`이나 `:load`는 됨). 매뉴얼 `cli.md` REPL 절에 한 줄.

### TL14. 앞단의 패닉은 메시지 없는 한 줄이 된다 — 정리, 낮음 (재현 안 함, 코드 읽기)
`main.rs:74`가 패닉 훅을 비우고 명령 스레드의 패닉은 `error: internal error in the fung command`만 쓴다(`main.rs:89` 근처). VM 안의 패닉은 E0801에 메시지와 경로가 붙는다(`vm.rs:437`).
이번에는 한 번도 내지 못했으니 가정이다. 고치는 법: 훅이 메시지와 위치를 적어 두고 그 줄에 붙인다.
- 처리: 이 커밋, `fung-cli/src/main.rs`: 패닉 훅 `remember_panic`이 메시지와 위치를 `LAST_PANIC`에 적고, 명령 스레드의 패닉 줄에 붙인다. 시험 `a_command_panic_keeps_its_message`.

### TL15. E0229·E0518은 매뉴얼 예제만이 지킨다 — 정리(시험), 낮음
골든 한 줄씩이면 된다. 골든에 없는 번호는 26개지만 나머지는 다른 시험에 있다.
- 처리: 이 커밋. `tests/lang/err_pipe_postfix.fung`(E0229)·`err_text_not_utf8.fung`(E0518). 카탈로그의 번호를 골든·기대 출력·단위 시험과 견준 결과 시험이 하나도 없는 살아 있는 번호는 이 둘뿐이었다(나머지 E0213·E0220·E0226·E0232·E0704·E0802는 은퇴). 새 번호 E0134·E0135·E0241도 골든이 있다.

## 11. 확인했고 문제없던 것 (다시 검사하지 않아도 됨)

- **명세**: 평가 순서(인자, 이름 인자는 쓴 순서, 기본값, 리터럴 원소, 이항 연산, 슬라이스 두 끝, 단락 평가 넷), truthiness 없음(`if`·`then`·`and`/`or`/`not`·`assert`·술어 모두 E0505),
  정수 넘침(`-9223372036854775808 // -1` 포함)·0으로 나누기·`parse_float("nan"/"inf")` 거절, 줄 이어짐(조용히 남는 모호함을 찾지 못함), 블록 값 규칙(E0223·E0224),
  없는 것의 규약(색인·필드는 에러, `get`·`first`·`find`·`min`은 `nil`), 비교 연쇄 금지, 기간 산술의 차원.
- **앞단**: 깊은 중첩 9,000겹과 한도(E0115), 20만~300만 항 사슬, 숫자·기간 리터럴의 가장자리, 문자열 이스케이프·중첩 보간·비UTF-8 소스, 모델 C의 복사 시점과 E0204~E0209, 모듈 순환·비공개·가리기, 부분 적용의 한 번 평가.
- **컴파일러**: 꼬리 위치 판정(`functions.md`의 표 그대로), `end_tail`의 불변식, 패턴(대안의 부분 묶기, 중첩 대안, 가드 붙은 대안의 순서), 클로저 복사와 제자리 수정, 상수 풀(`Int(1)`과 `Float(1.0)` 구분),
  합류점의 스택 높이, 범위 상수, 상수 접기와 실행 시 계산의 일치(값·에러 번호), 기본값 매개변수 + 자기 꼬리 호출.
- **VM·값**: 영속 벡터(앞·뒤 붙이기, 조각, 이어 붙이기, 트라이 경계 32·1024·32768)와 맵(삽입 순서, 삭제 뒤 재삽입)의 차등 시험, 100만 겹 값의 출력·비교·해시·해제,
  `Rc` 고리 만들기 7가지(모두 E0708/E0709), 메모 무효화, 300만 회 할당 루프의 RSS(9 MB 일정), 점근(뒤·앞 붙이기, 색인, `[h, ...t]` 재귀, 맵 넣기·지우기 모두 문서대로).
- **동시성**: 메시지 불변식, 무작위 스트레스(`acquire` 수 = `release` 수, 남은 자식 프로세스 0), `release` 중 취소·두 번 `cancel`·0초 기한·겹친 기한,
  SIGINT·SIGTERM(계산 중, `swap` 안, 외부 명령 중 모두 `release`를 돌리고 130/143), 교착 판정, TERM을 무시하는 셸의 KILL, 표준 입력의 기한 읽기, 끝난 태스크와 낡은 타이머를 쥐지 않음.
- **라이브러리**: 조합 호출에서 패닉·멈춤 0, 정수 경계·거대 범위·비UTF-8 바이트·NUL·중첩 10,000단계 JSON·10 MB 표준 입력, 엄격한 해석기(`parse_*`, `json.parse`의 중복 키·서로게이트, `time.parse`의 자리 수·요일),
  안전한 기본값(`exec`는 리스트만, `fs.move`·`fs.copy`는 덮어쓰지 않음, `fs.remove_all`은 `/`·`.`·`..` 거절), `sort`는 안정 정렬.
- **도구**: 포매터(저장소 `.fung` 381개 포맷 두 번: 문법 에러 시험 33개 거절, 나머지 바뀜 0·멱등 위반 0; 손으로 만든 43개에서도 0), 넓은 글자·탭·ZWJ 이모지 뒤의 밑줄,
  꼬리 호출·콜백·클로저·태스크의 경로, 자원 한도(JSON 중첩 10,000, 문자열 1억 글자, 재귀 100,000, 정규식은 선형 엔진), TLS 검증 기본·`file://` 거절, 크레이트 허용 목록과 2주 규칙.

## 12. 확인하지 못한 의심

- **리눅스 전체**: CC14의 멈춤, epoll 경로, glibc/musl 차이. 이번 점검은 macOS arm64뿐이다.
- 성능 항목의 이득(VA30~VA34, VM20·VM21, CC21): 빌드를 하지 않아 고친 뒤를 재지 못했다.
- FE60의 제곱이 resolve인지 컴파일러인지.
- LB41이 11-10 LB29 수정의 퇴행인지(`git log -p`로 확인하면 된다).
- TLS, REPL의 대화형 줄 편집 중 신호, 풀 64개가 모두 막히는 상황(장치 읽기).
- 24비트 피연산자 넘침(1,600만 개)은 실행으로 확인하지 않았다(코드 읽기와 단위 시험 `an_operand_past_24_bits_is_e0230`에 기댐).
- 퍼징은 문법을 아는 생성기가 아니라 `check` 통과율이 25%였고, 동시성·`fs`·`http`·`time`은 변이 퍼징에서 뺐다.
- `stdin.rs`, `runtime.rs`의 종료 순서, `deadlock.rs`의 보고서 부분, `boot/`·`vm/module.rs`·`vm/failure.rs`는 실행으로만 보았거나 훑어만 봤다.
