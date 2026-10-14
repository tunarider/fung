# M7.5. 구조 — core · native · extras, 무엇을 어디에 두었고 왜 그렇게 했나

M7.5는 새 기능이 없다. 지금까지 `internal/native` 한 패키지에 섞여 있던 "언어가 보장하는 값 연산"과 "바깥세상과의 다리"를
갈라 세 층으로 두고, 그 위에 fung으로 쓴 내장 extras를 `extras/`로 빼내고, 조합 함수 21개를 Go에서 fung으로 옮긴 작업이다.
골든 테스트 127개와 샘플 스크립트의 기대값은 하나도 바뀌지 않았다. 바뀐 것은 **어디에 무엇이 있고, 누가 누구를 알 수 있는가**다.

## 1. 네 층과 기준

| 층 | 어디 | 언어 | 무엇 |
|------|------|------|------|
| core | `internal/prelude` (+ `internal/vm`의 동시성) | Go | 값 연산 41개 + 동시성 11개. `print` `map` `filter` `fold` `sort` `split` `int` `spawn` `channel` … |
| native | `internal/native` | Go | `_native.fs` `_native.path` `_native.process` 다리 26개(동시성 다리 `_native.conc`는 vm이 제공) |
| 내장 extras | `extras/` | fung | `prelude.fung`(공개 29개: 조합 21 + 프로세스 재내보내기 3 + `until` `iterate` `fan_in` `pmap` `retry`), `fs` `path` `cli` 모듈. 실행 파일에 embed |
| 프로젝트 extras | (추후) | fung | 사용자 프로젝트의 `extras/`. 같은 메커니즘 |

기준은 사용자가 세운 두 문장 그대로다.

1. **core**: 최소한 언어 측면에서 이 정도는 보장해야 한다.
2. **extras**: core를 기반으로 직접 구성할 수 있지만(성능 측면에서도 문제 없음) 없으면 너무 불편하니까 언어 측면에서 넣어 둔다.
   가급적 Go native를 안 쓴다.

native는 그 사이의 좁은 다리다: fung으로 **쓸 수 없는** 것(파일, 프로세스, 시계, 고루틴)만, 숨은 이름(`_native.*`)으로, 최소한만.
사용자 코드는 `_native.*`를 import할 수 없다(E0310). 다리의 공개 이름은 항상 fung 파일이 정한다 — `exec`·`sh`·`env`도 이번에
`_native.process`로 내려가고 `prelude.fung`이 `exec = _p.exec`로 다시 내보낸다. 그래서 "어떤 이름이 보이는가"는 전부 fung 파일
하나를 읽으면 알 수 있다.

같은 기준으로 **왜 `flatten`은 Go에 남았는가**가 2절이고, **"가급적 Go를 안 쓴다"를 어떻게 지키게 만들었는가**가 3절이다.

## 2. 왜 원소를 도는 함수는 Go에 남겼나 — 측정값

옮기기 전에 기준 2의 괄호("성능 측면에서도 문제 없음")를 숫자로 확인했다. 전부 같은 기계, 같은 날, `time -p`의 real 초.

| 식 | Go(옮기기 전) | fung(옮긴 뒤) |
|------|------|------|
| `sum(0..1_000_000)` (fung `sum` = core `fold` 한 번) | — | 0.13 |
| 같은 합을 순수 fung 재귀 `s(n, acc)`로 | — | 0.19 |
| `reverse(0..100_000)` (`for` 인덱스 루프) | — | 0.03 |
| `lines(repeat("abc\n", 100_000))` (`split` + `map`) | — | 0.04 |
| `unique(…20_000개 전부 다름)` | 0.45 | 0.99 |
| `unique(…20_000개, 서로 다른 값 100개)` | 0.00 | 0.01 |
| `flatten(…20_000쌍)` | 0.00 | **2.02** |
| `flatten(…100_000쌍)` | 0.02 | **26.6** |
| 시작 `fung -e 'print(1)'` (10회 평균, 예열 후) | 3.8ms | 6.7ms (리뷰어 재측정: 둘 다 4.1ms) |

읽는 법:

- **한 원소마다 VM을 한 번 되부르는 비용**은 작다. `sum`은 Go `fold`가 돌면서 람다 `(acc, x) => acc + x`를 백만 번 부르는데 0.13초,
  순수 fung 재귀도 0.19초다. 그러니 "루프가 Go냐 fung이냐" 자체는 결정적이지 않다. `map` `filter` `fold` `sort` `find`를 Go에 둔
  이유는 속도보다 **이들이 리스트를 만드는 유일한 효율적 방법**이기 때문이다(다음 항목).
- **fung에서 리스트를 하나씩 누적하면 제곱 시간이다.** 값이 불변이라 `acc + [x]`는 매번 `acc`를 통째로 복사한다. 원소 n개면
  복사가 n²/2번. `flatten`을 `fold(xs, [], (acc, x) => acc + x)`로 썼더니 10만 쌍에 26초가 걸렸다(Go는 0.02초). 이것은 상수 배가
  아니라 **복잡도가 다른 것**이라 기준 2의 괄호를 어긴다. `flatten`은 Go core로 돌아갔다. 같은 이유로 "원소를 돌며 결과 리스트를
  만드는" 함수(`map` `filter` `for`)는 전부 Go여야 한다 — fung 쪽에서는 그 결과를 **슬라이스·인덱스·`for`**로만 다룬다.
  `take`·`drop`은 슬라이스 한 번, `reverse`·`zip`은 `for` 인덱스 루프라 선형이다.
- `unique`는 Go 버전도 제곱(원소마다 앞의 결과를 훑는다)이라 fung이 2배 느린 것은 상수 배다. 남겨 두었다. 둘 다 2만 개쯤에서
  1초라 언젠가 해시 기반 Go 구현이 필요해지면 그때 옮기면 된다 — 의미는 같으니 골든이 지켜 준다.
- **시작 시간**은 늘어도 몇 ms다(측정이 잡음 수준이라 리뷰어는 차이를 못 봤다). `prelude.fung`(135줄)을 매 실행마다 컴파일하고 돌리기 때문이다. 기준(50ms)에 한참 못 미치지만
  M8에서 `json`을 fung으로 쓰면 더 늘 수 있다 — 그래서 설계(06)대로 **prelude만 시작 시 컴파일하고 나머지 모듈은 import될 때**
  컴파일한다. `json`은 `import json`으로 쓰는 모듈이지 prelude가 아니다.

## 3. 의존 방향을 Go 패키지 경계로 강제하기

"core는 extras를 모른다"를 문서의 문장으로 두면 지켜지지 않는다. 그래서 **Go의 import 규칙**으로 만들었다.

- `extras/`는 `internal/` 밖에 있다. `internal/…`의 어떤 패키지도 `extras`를 import하지 않는다. 이것을 `tests/layers_test.go`가
  `go list -deps ./internal/...`의 결과에 `extras`가 없음을 보고 확인한다(반대 방향도: `extras`의 의존에 `internal/vm`이 없다).
  누군가 편의상 `internal/frontend`에서 `extras.Source`를 부르면 테스트가 떨어진다.
- 그러려면 core 쪽에 **"라이브러리의 모양"**이 있어야 한다. `frontend.Registry{Stdlib func(name) (src, ok), Natives map[string]*Module}` —
  "이름으로 fung 소스를 찾는 함수"와 "`_native.*` 모듈 표". 로더(`frontend.Loader`)는 이 둘만 받고, 그 내용이 내장 extras인지
  테스트용 가짜인지 모른다. `extras.Default()`가 내장 파일과 `native.Bridges()`를 이 모양에 담아 돌려주고, `cmd/fung`의
  `registry()`가 거기에 vm의 `_native.conc`를 더한다. **세 층이 만나는 곳은 그 함수 하나다.**
- 계획은 `Registry`를 `extras`에 두려 했지만 REPL(`internal/repl`)이 "무엇을 import할 수 있는지"를 받아야 해서 core(`frontend`)로
  옮겼다. 타입이 `extras`에 있었다면 REPL이 `extras`를 import해야 했고, 그 순간 층이 무너진다. **모양은 안쪽, 내용은 바깥쪽**.
- `internal/` 안의 테스트는 되도록 `extras`를 쓰지 않는다. REPL 테스트는 `fs` 모듈 한 줄짜리 가짜 라이브러리를 직접 만든다. 다만 vm 테스트는
  `prelude.fung`의 함수(`take`·`is_err`…)를 쓰는 케이스가 있어서 테스트 파일에서만 `extras`를 import해 `cmd`의 `bootPrelude`와 같은
  일을 하는 `bootTestPrelude`를 둔다(`go list -deps`는 테스트 의존을 세지 않는다). vm이 cmd를 import할 수 없어서 생긴 중복이다.
- `internal/nat`은 prelude와 native가 **둘 다** 쓰는 도우미(매개변수 선언 `Def`·`Param`·`Opt`·`Rest`, `Ok`·`ErrValue`, 인자 검사,
  테스트용 가짜 `Context`)다. 이게 없으면 native가 prelude를 import하거나 그 반대가 되어 층이 섞인다. 작은 패키지 하나가 두 층을
  독립시킨다.

## 4. `extras/`가 프로젝트 extras로 이어지는 그림

```
import fs            ← 이름으로: Registry.Stdlib("fs")
import "./lib/x"     ← 경로로: 내 파일

Registry (cmd/fung이 조립)
  ├─ core prelude      Go 값 연산 + 동시성          (항상, 가장 먼저)
  ├─ 내장 extras        extras/*.fung (embed)        ← 지금
  └─ 프로젝트 extras    <프로젝트>/extras/<name>.fung ← 추후
```

이름으로 하는 import는 `Registry.Stdlib(name)` 한 함수를 거친다. 프로젝트 extras는 이 함수를 **하나 더 끼우는 것**이다:
"프로젝트 `extras/` 폴더에서 먼저 찾고, 없으면 내장에서 찾는다". 우선순위(core > 내장 extras > 프로젝트 extras, 04)는 이름 해석
순서이지 새 메커니즘이 아니다. 의존성 관리자가 생기면 그것이 하는 일은 그 폴더를 채우는 것뿐이다. 다리(`_native.*`)는 프로젝트
extras에 열지 않는다 — 06의 "사용자 Go 확장은 열지 않는다"와 같은 결정이다.

## 5. 옮기면서 발견한 것

- **fung 함수는 E0403을 낼 수 없다.** 호출 규약 에러(인자 타입)는 Go 원시가 내는 번호다. `pad_left("a", 3, "")`는 Go에서
  E0403이었지만 fung에서는 `fail("pad must be a non-empty string")` → E0803이다. 계획은 "`repeat`가 대신 내게 하자"였는데
  `repeat("", n)`은 그냥 `""`라 성립하지 않았다. 규칙으로 정리했다(06): **fung extras의 인자 검사는 `fail`로 한다.** 이게 층의 자연스러운
  결과다 — fung 코드는 fung의 에러 경로를 쓴다. 바뀐 기대값은 단위 테스트 한 줄이고 골든에는 없었다.
  같은 이유로 **인자 타입이 틀렸을 때의 번호**도 옮긴 함수들에서 바뀌었다(리뷰에서 확인): `range(1.5)`·`pad_left("a", "3")`·
  `take(xs, 1.0)`은 E0403 → E0501(`..`·`-`·`<`가 낸다), `pad_left(1, 3)`은 E0403 → E0506(`len`이 낸다), `get(nil, k)`는 E0506 →
  E0803(`get`이 맵이 아니면 `fail`한다). 정상 입력의 결과와 골든은 그대로다. 번호를 꼭 지켜야 했다면 그 함수는 `flatten`처럼
  Go에 남겨야 했는데, 이 번호들을 쓰는 스크립트는 없고 06의 새 규칙이 더 단순해서 fung에 두었다.
- **옮긴 함수 안에서 난 에러는 사용자의 줄을 가리켜야 한다.** 처음 옮겼을 때 `first("abc")`의 에러는 `--> extras/prelude.fung:24:53`을
  가리켰고 발췌도 없었다(내장 파일은 `Sources`에 없다). 사용자는 그 파일을 열 수 없다. 그래서 로더가 내장 파일에서 컴파일한 `Proto`에
  `Builtin` 표시를 하고, VM이 에러에 호출 경로를 붙일 때 **가장 안쪽 프레임이 내장이면 가장 가까운 사용자 프레임의 위치**로 주 위치를
  옮긴다. 호출 경로에는 내장 프레임이 그대로 남는다(`extras/prelude.fung:24 _list`). M8에서 `json`을 fung으로 쓰면 모든 파싱 에러가
  이 규칙을 탄다.
- **리뷰에서 잡힌 의미 차이 세 가지**: `first`·`last`가 `len`으로 빈 것을 검사해 채널을 못 받았다(05: 순회하는 함수는 채널을 받는다) →
  먼저 `_list`로 바꾼다. `get([10, 20], 0)`이 `in`의 원소 검사 때문에 조용히 `nil`을 돌려줬다 → 맵이 아니면 `fail`. `min([nil, 1])`이
  `nil` 표지 때문에 `1`을 돌려줬다(Go는 E0501) → 첫 원소로 fold를 시작한다. 셋 다 "리스트가 아닌 입력"에서 나왔다. 옮긴 함수마다
  str·tuple·map·nil·range·channel을 넣어 보는 표가 있었다면 먼저 잡혔을 것이다.
- **에러 번호는 같아도 메시지의 동사가 바뀐다.** (`first("abc")`는 "cannot map over str" — `_list`가 `map`을 쓰기 때문이다.) `keys([])`는 전에 "cannot list the keys of list"였고 지금은 `pairs`가 내는
  "cannot list the pairs of list"다. `get({}, 1)`은 "cannot index map and int"에서 `in`이 내는 "cannot search map and int"로. 번호
  (E0506, E0501)는 같다. 골든은 번호와 사용자 메시지 전체를 비교하므로 골든에 없는 조합이라 통과했다 — 번호를 "한 번 붙이면 바꾸지
  않는" 이유가 여기서 또 드러난다.
- **`is_ok`·`is_err`는 `type_of(x) == 'Ok`다.** 태그의 `type_of`가 태그 이름이므로 Go의 "이름만 본다"와 정확히 같다(`Ok(1, 2)`도 참).
  `match x: Ok(_) => …`로 썼다면 인자 하나짜리만 잡았을 것이다.
- **`put`은 맵을 통째로 복사한다.** `fold(0..20_000, {}, (m, i) => put(m, str(i), i))`가 18초다. 이번 작업과 무관한 core의 기존 동작이고
  의도(불변 맵)의 결과지만, 2만 개 키를 모으는 스크립트는 실제로 있을 수 있다. 뒤로 미룬 숙제: 영속 맵(persistent map)이든 `group_by`
  같은 Go 빌더를 늘리든, 측정하고 정한다.
- **보이지 말아야 할 것이 안 보이는지** 확인했다: `prelude.fung`의 `_list`·`_clamp`·`_padding`·`_chomp`는 모듈의 비공개 이름이라
  prelude 표에 들어가지 않는다(`print(_list)` → E0204). 사용자가 `fn take(x) = x`를 쓰면 여전히 W0103이 난다 — fung으로 옮긴 이름도
  prelude 표에 있기 때문이다. `prelude.fung` 자신에게는 경고가 나지 않는다(내장 파일의 경고는 숨긴다, M6).
- **`dump`·`check`·`test`·REPL이 전부 같은 `registry()`를 쓴다.** 전에는 `frontend.Compile`이 몰래 내장 stdlib을 알고 있어서 `dump`만
  다른 길로 갔다. 이제 `Compile`은 라이브러리 없는 순수 컴파일이고, 라이브러리가 필요한 곳은 `Loader`에 `Registry`를 넣는다.

## 6. 다음: M8은 전부 extras다

`json`은 fung으로 쓴다(파서·직렬화 모두 문자열 연산과 `match`로 가능하고, 성능이 필요해지면 그때 측정한다). `regex`·`time`·`http`는
fung으로 쓸 수 없는 부분만 `_native.regex`·`_native.time`·`_native.http` 다리로 내리고, 공개 이름과 편의 함수는 `extras/*.fung`이
정한다. 이 노트의 2절(누적은 Go에서)과 5절(인자 검사는 `fail`로)이 그 작업의 규칙이다.
