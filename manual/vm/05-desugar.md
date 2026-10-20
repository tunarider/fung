---
description: desugar와 코어 언어.
lede: 사람이 쓰기 편한 문법은 많을수록 좋지만, 컴파일러가 아는 형태는 적을수록 좋다. desugar는 편의 문법을 몇 안 되는 코어 형태로 풀어 그 둘을 함께 얻는다.
---
# desugar와 코어 언어

<ol class="stages"><li>소스</li><li>토큰</li><li>트리</li><li class="on">코어</li><li>바이트코드</li><li>실행</li></ol>

:::goals
- 편의 문법(syntactic sugar)과 desugar가 무엇인가
- 코어 언어를 따로 두는 이유
- fung의 코어 형태와, 겉 문법이 어떻게 그리로 풀리는가
- `and`·`or`, 파이프, `for`, 보간, `_` 부분 적용, `match`가 풀리는 실제 모양
:::

## 편의 문법 {#sugar}

`x |> f(a)`와 `f(x, a)`는 같은 일을 한다. 파이프 쪽은 "x를 f에 넘긴다"는 흐름이 왼쪽에서 오른쪽으로 읽혀서 길게 이어 쓸 때 편하다. 그러나 새로 할 수 있게 된 일은 없다. 이렇게 **표현력을 늘리지 않고 쓰기만 편하게 해 주는 문법**을 **편의 문법**, 영어로 syntactic sugar(문법 설탕)라고 한다. 프로그램을 달게 해 준다는 농담이다.

편의 문법을 기본 문법으로 바꿔 쓰는 일을 **desugar**(설탕 빼기)라고 한다. 파서가 만든 트리(겉 트리)를 받아, 편의 문법을 모두 풀어 낸 더 작은 트리를 만든다. 그 결과가 쓰는 말을 **코어 언어**(core language)라고 한다.

fung에서 편의 문법은 생각보다 많다. 파이프만이 아니다.

- `a and b`는 "a가 참이면 b, 아니면 거짓"인 `if`다.
- `cond then a else b`도 `if`다.
- `for x in xs: … ;;`는 "`xs`의 각 원소에 함수를 적용"인 `map` 호출이다.
- `"hi ${name}"`은 조각들을 잇는 연산이다.
- `f(1, _)`는 "나머지 인자 하나를 받는 함수"를 만드는 람다다.
- 짧은 함수 `fn f(x) = e`와 블록 함수 `fn f(x): e ;;`는 같은 것이다.

## 코어 언어를 두는 이유 {#why-core}

desugar 없이 컴파일러가 겉 트리를 바로 받아도 된다. 그러면 컴파일러가 `pipe` 노드, `for` 노드, `then` 노드, `and` 노드를 모두 따로 알아야 한다. 겉 문법을 하나 늘릴 때마다 컴파일러에 새 갈래가 생기고, 그 갈래마다 바이트코드를 맞게 내는지 따로 검사해야 한다.

코어 언어를 두면 일이 둘로 나뉜다.

- **desugar**: 겉 문법 → 코어. 트리를 트리로 바꾸는 일이라 단순하다. 새 편의 문법은 여기만 고친다.
- **컴파일러**: 코어 → 바이트코드. 몇 안 되는 형태만 안다. 겉 문법이 늘어도 그대로다.

그리고 트리를 다루는 뒤의 단계들(이름 해석, 값 자리 검사, 컴파일러)도 모두 작은 코어 위에서 일한다. `and`의 오른쪽이 실행되지 않을 수 있다는 사실을 이름 해석이 따로 알 필요가 없다. 이미 `if`가 되어 있기 때문이다.

## fung의 코어 형태 {#core-forms}

fung의 코어는 겉 트리와 **같은 모양**(균일 트리, 4장)이다. 머리 단어와 자식의 짜임만 바뀐다. 주요 형태는 이렇다.

<table>
  <thead><tr><th>코어 형태</th><th>뜻</th><th>여기로 풀리는 겉 문법</th></tr></thead>
  <tbody>
    <tr><td><code>(do e…)</code></td><td>순서대로 실행, 값은 마지막 것</td><td>파일 전체(<code>program</code>), 블록(<code>block</code>)</td></tr>
    <tr><td><code>(let 대상 e)</code></td><td>대입</td><td><code>x = e</code>, 구조 분해 <code>(a, b) = e</code></td></tr>
    <tr><td><code>(if c a b)</code></td><td>조건</td><td><code>if</code>/<code>else if</code>, <code>then … else</code>, <code>and</code>, <code>or</code></td></tr>
    <tr><td><code>(fn 이름 매개변수 본문)</code></td><td>함수</td><td><code>fn</code> 정의, 람다 <code>=&gt;</code>, 익명 함수, <code>_</code> 부분 적용, <code>for</code>의 본문</td></tr>
    <tr><td><code>(call f 인자…)</code></td><td>호출</td><td>호출, 파이프 <code>|&gt;</code>, <code>for</code></td></tr>
    <tr><td><code>(op 연산자 a b)</code></td><td>원시 연산</td><td>산술, 비교, <code>in</code>, <code>not</code>, 단항 <code>-</code></td></tr>
    <tr><td><code>(concat 조각…)</code></td><td>문자열 잇기</td><td>보간 <code>"…${e}…"</code></td></tr>
    <tr><td><code>(field x "k")</code> <code>(index x i)</code> <code>(slice x a b)</code></td><td>접근</td><td><code>x.k</code>, <code>x[i]</code>, <code>x[a..b]</code></td></tr>
    <tr><td><code>(tag Ok e…)</code></td><td>태그 값</td><td><code>Ok(e)</code>, <code>e |&gt; Ok</code></td></tr>
    <tr><td><code>(match e (arm 패턴 조건 본문)…)</code></td><td>패턴 매칭</td><td><code>match</code></td></tr>
    <tr><td><code>(unwrap e "!")</code> <code>(coalesce a b)</code></td><td>결과값 축약</td><td><code>e!</code>, <code>e?</code>, <code>a ?? b</code></td></tr>
    <tr><td><code>(return e)</code> <code>(import …)</code> <code>(test …)</code> <code>(assert e)</code></td><td></td><td>그대로</td></tr>
  </tbody>
</table>

리스트·맵·튜플·범위 리터럴과 수·문자열 같은 잎(leaf, 자식 없는 노드)은 그대로 지나간다. 연산자는 코어에서도 **값이 아니다**. `fold(xs, 0, +)`처럼 연산자를 함수로 넘기는 문법은 없고, 람다 `(a, b) => a + b`로 쓴다(설계 문서 09).

## 따라가기: 두 줄의 코어 {#follow}

`fung dump core`는 desugar에 더해 6장의 이름 해석까지 끝난 트리를 찍는다. 이 장에서는 **머리 단어**가 어떻게 바뀌었는지만 보자. 이름이 `(global 0)`이나 `(local 0)`으로 바뀐 것은 다음 장의 일이다.

```dump ast
fn double(x) = x * 2
print(double(21))
```

```dump core
fn double(x) = x * 2
print(double(21))
```

- `program`이 `do`가 되었다. 파일 전체는 "문장들을 순서대로"인 블록이다.
- 함수 정의 `(fn double(x) …)`가 `(let 이름 (fn double(x) …))`, 곧 "함수 값을 이름에 대입"이 되었다. 함수 정의도 결국 대입이다.
- 본문의 `block`이 `do`로, `binary`가 `op`로 바뀌었다.
- 호출 `(call …)`은 원래 코어 형태라 모양이 그대로다.

이 두 줄에는 편의 문법이 거의 없어서 변화가 작다. 편의 문법이 많은 예를 보자.

## 풀리는 모양들 {#lowerings}

### `and`와 `or`: 단락 평가가 공짜로 {#and-or}

`and`와 `or`는 코어에 없다. `if`로 풀린다.

```plain
a and b   →  (if a (op bool b) false)
a or b    →  (if a true (op bool b))
```

`if`는 조건에 따라 한쪽 가지만 실행하므로, `a`가 거짓이면 `b`는 아예 실행되지 않는다. 이것을 **단락 평가**(short-circuit evaluation)라고 한다. 따로 구현하지 않았는데 `if`로 풀었기 때문에 저절로 생겼다(마일스톤 노트 M1).

```dump core
print(false and (1 / 0 == 1))
```

```fung run
print(false and (1 / 0 == 1))
```

`1 / 0`이 있지만 에러가 나지 않는다. 실행되지 않았기 때문이다.

`(op bool b)`는 "b가 불(bool)이 아니면 에러"라는 검사다. fung은 조건 자리에 `true`·`false`만 받는다. `if`의 조건 `a`는 VM의 점프 명령이 검사하지만, 가지로 실행된 `b`의 값은 따로 검사해야 한다. 그래서 `true and 1`은 실행 시점의 `E0505`다.

```fung run exit=1 error=E0505
print(true and 1)
```

### 파이프: 평범한 호출로 {#pipe}

`x |> f(a)`는 `f(x, a)`, `x |> f`는 `f(x)`가 된다. 파이프가 이어지면 안쪽부터 풀린다.

```dump ast
xs = [1, 2, 3]
print(xs |> filter(x => x > 1) |> len)
```

```dump core
xs = [1, 2, 3]
print(xs |> filter(x => x > 1) |> len)
```

겉 트리의 `pipe` 두 개가 코어에서는 흔적도 없다. `len(filter(xs, …))`와 똑같은 트리다. 그래서 파이프에는 실행 비용이 없다. 바이트코드도 그냥 호출이다.

파이프의 오른쪽 호출에 `_`를 쓰면 desugar가 거절한다(`E0227`). 파이프 값은 항상 첫 인자로 들어가므로, `_`를 함께 쓰면 "첫 인자"와 "빈 자리"가 겹쳐 뜻이 모호해진다.

### `for`: `map`이나 `each` 호출로 {#for}

`for x in xs: 본문 ;;`은 "`xs`와, 본문을 담은 람다로 `map` 부르기"가 된다.

```dump core
xs = for x in [1, 2, 3]: x * 10 ;;
print(xs)
```

값이 버려지는 자리의 `for`는 `map` 대신 `each`가 된다. 결과 리스트를 만들 필요가 없기 때문이다.

```dump core
for x in [1, 2]:
  print(x)
;;
```

두 가지를 눈여겨볼 만하다. 첫째, 코어에는 `(prelude map)`이 직접 박혀 있다. 사용자가 `map`이라는 이름을 자기 함수로 덮어써도 `for`는 내장 `map`을 부른다. 둘째, 본문이 람다가 되었으므로 `for` 안에서 `return`을 쓰면 람다에서 돌아올 뿐 바깥 함수는 끝나지 않는다. 그 함정을 막으려고 desugar는 이 람다에 "for의 본문"이라는 표시를 해 두고, 다음 장의 이름 해석이 그 안의 `return`을 `E0213`으로 거절한다.

### 보간과 `then`: 조각 잇기와 `if` {#interp}

```dump ast
name = "fung"
n = 3
print("hi ${name}", n > 2 then "big" else "small")
```

```dump core
name = "fung"
n = 3
print("hi ${name}", n > 2 then "big" else "small")
```

`interp`는 `concat`이 되었다. 8장에서 이것은 `CONCAT` 명령 하나가 된다. `then`은 블록 `if`와 똑같은 `(if c a b)`다. 한 줄 조건과 여러 줄 조건이 코어에서는 구별되지 않는다.

### `_` 부분 적용: 숨은 변수와 람다 {#partial}

`add(1, _)`는 "인자 하나를 받아 `add(1, 그것)`을 부르는 함수"다. 그런데 단순히 `h0 => add(1, h0)`으로 풀면 문제가 있다. `add(expensive(), _)`라고 썼을 때 `expensive()`가 함수를 부를 때마다 다시 계산된다. desugar는 그래서 함수와 다른 인자를 먼저 **한 번만** 계산해 숨은 변수에 담고, 그 변수를 쓰는 람다를 만든다.

```plain
f(a, _, b)  →  (do (let $p0 f) (let $p1 a) (let $p2 b)
                   (fn (h0) (do (call $p0 $p1 h0 $p2))))
```

`$p0` 같은 이름은 `$`로 시작해서 사용자가 쓸 수 있는 어떤 이름과도 겹치지 않는다. 실제 출력에서는 이름 해석을 거쳐 슬롯 번호가 되어 보인다.

```dump core
add = (a, b) => a + b
inc = add(1, _)
print(inc(41))
```

`(let (local 0) (global 0))`이 `add`를, `(let (local 1) 1)`이 인자 `1`을 미리 담는다. 람다 안에서는 그 둘이 `(captured 0)`, `(captured 1)`, 곧 람다가 붙잡은 값으로 보인다(9장).

### `match`: 대상을 한 번만 계산 {#match}

`match`도 대상을 숨은 변수에 한 번 담은 뒤 절마다 그 변수를 비교한다. 절이 몇 개든 대상 식은 한 번만 실행된다.

```dump core
r = Ok(3)
match r:
  Ok(n) => print(n)
  Err(e) => print(e)
;;
```

맨 앞의 `(let (local 0) (global 0))`이 숨은 변수(`$m0`)에 대상을 담는 부분이다. 패턴 매칭 자체(`MATCH` 명령과 패턴 트리)는 8장 이후에서 본다.

:::box learn 같은 모양의 트리를 돌려준다
desugar는 겉 트리를 **읽기만** 하고 새 트리를 만든다(1장). 입력과 출력이 같은 `Node` 모양이라 desugar는 커다란 `match` 하나다. "이 머리 단어면 이렇게 바꿔서 새 노드를 만들고, 자식들도 같은 함수로 바꾼다." 대부분의 머리 단어(`list`, `index`, `return` …)는 자식만 바꾸고 머리는 그대로 둔다. 변환이 필요한 것만 따로 갈래를 둔다.
:::

:::box other 다른 언어는
- **Scheme**: 언어 명세가 처음부터 "핵심 형태(special forms)"와 "파생 형태(derived forms)"를 나눈다. `let`은 람다 호출로, `cond`는 `if` 사슬로 풀린다고 명세에 적혀 있다. fung의 코어 언어가 빌려 온 생각이다.
- **Rust**: 컴파일러 안에서 `for` 반복문을 `loop`와 `match`로 푸는 단계(HIR로 내리기)가 있다. 겉 문법보다 작은 중간 언어 위에서 검사와 최적화를 한다.
- **Haskell**(GHC): 풍부한 겉 문법을 아주 작은 "Core"라는 언어로 푼 뒤, 최적화를 모두 Core 위에서 한다.
- **JavaScript**: Babel 같은 도구는 새 문법(화살표 함수, 클래스)을 옛 문법으로 풀어 오래된 엔진에서도 돌게 한다. 언어 바깥에 있는 desugar 도구다.
- **CPython**: 별도의 desugar 단계 없이, 컴파일러가 AST 노드 종류마다 바이트코드를 직접 낸다.
:::

:::box choice fung의 선택: 작은 코어, 같은 모양의 트리
설계 문서 00은 파싱 결과를 균일한 트리로 만들고 이를 작은 코어 언어(`if`, `fn`, `let`, `do`, `match` …)로 풀어낸 뒤 컴파일한다고 정했다. 영향을 준 언어 목록에서는 이 구조를 Lisp에서 빌렸다고 적었다(Elixir·Julia도 이렇게 한다). 얻는 것은 두 가지다. 컴파일러는 코어 언어만 알므로 겉 문법을 늘려도 컴파일러와 VM은 그대로이고, 사용자 매크로를 나중에 넣을 자리가 열려 있다(매크로는 아직 없다). desugar가 겉 트리를 받아 **같은 형태의 더 작은 트리**로 바꾸는 것도 설계 문서 09의 규칙이다. 노드 모양을 Rust 타입이 검사해 주지 않는 대가(4장)는 설계상 desugar가 맡는다. 지금 코드에서 그 검사는 desugar가 모르는 머리 단어를 만났을 때 내부 오류 `E0801`(사용자 실수가 아니라 구현의 버그)을 내는 것이고, 자식의 수와 순서는 파서가 노드를 만들 때 정한 모양을 뒤 단계들이 그대로 믿는다.
:::

## 코드에서 찾아보기 {#source}

- `crates/fung-front/src/desugar.rs`: 입구 `desugar(n, keep_value)`. `keep_value`는 파일의 마지막 식이 값으로 남는가(REPL)를 정한다. 아니면 그 자리는 버려지는 자리다.
  - `Desugarer::walk_node`: 머리 단어마다 갈래를 타는 큰 `match`. `and`·`or`를 `if`로 바꾸는 곳, `binary`·`unary`를 `op`로, `then`을 `if`로, `interp`를 `concat`으로 바꾸는 곳이 모두 여기다. `check_bool`이 `(op bool …)`을 만든다.
  - `Desugarer::pipe`: 파이프와 `E0227`. `Desugarer::call`과 `Desugarer::partial`: `_` 부분 적용.
  - `Desugarer::for_loop`: `for`를 `(prelude map)` 또는 `(prelude each)` 호출로.
  - `Desugarer::fn_node`: 함수. 구조 분해 매개변수를 숨은 이름 `$aN`과 본문 맨 앞의 대입으로 푼다.
  - `Desugarer::match_expr`: `match`의 대상을 숨은 변수에 담는다.
  - `Desugarer::hidden`: `$p0`, `$m0` 같은 숨은 이름을 만든다.
- `crates/fung-front/src/compiler.rs`의 `Compiler::expr`: 컴파일러가 아는 코어 형태의 목록이 이 `match`의 갈래들이다. 겉 문법의 머리(`pipe`, `for`, `then`, `binary`)는 여기 없다.

:::exercise 코어를 예상하기
다음 프로그램의 `fung dump core`를 예상해 보자. 특히 `or`와 파이프가 어떻게 바뀔지 적어 보고 확인하자.

```fung frag
names = ["web1", "db1"]
ok = len(names) > 0 or false
print(ok, names |> map(upper))
```

:::solution
`or`는 `(if 조건 true (op bool 오른쪽))`, 파이프는 `(call (prelude map) names (prelude upper))`가 된다.

```dump core
names = ["web1", "db1"]
ok = len(names) > 0 or false
print(ok, names |> map(upper))
```

```fung run
names = ["web1", "db1"]
ok = len(names) > 0 or false
print(ok, names |> map(upper))
```

:::

:::

## 정리 {#summary}

- 편의 문법은 표현력을 늘리지 않고 쓰기만 편하게 하는 문법이다. desugar는 그것을 코어 언어로 푼다.
- 코어를 두면 컴파일러는 몇 안 되는 형태만 알면 되고, 겉 문법을 늘려도 컴파일러와 VM은 그대로다.
- `and`·`or`·`then`은 `if`로, 파이프와 `for`는 호출로, 보간은 `concat`으로 풀린다. `and`·`or`의 단락 평가는 `if`에서 공짜로 나온다.
- `_` 부분 적용과 `match`는 숨은 변수(`$p0`, `$m0`)를 써서 식을 한 번만 계산한다.
- fung의 코어는 겉 트리와 같은 균일 트리이고, 이 구조는 Lisp에서 빌렸다.

다음 장에서는 코어 트리의 이름을 하나씩 "어디에 사는 값인가"로 바꾸는 이름 해석을 본다. 겉 문법 각각의 뜻은 [언어 레퍼런스](../reference/language.html)에 있다.
