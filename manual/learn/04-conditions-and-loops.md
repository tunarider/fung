---
description: 비교 연산자, 값을 내는 if·else와 한 줄 조건 then … else, for로 목록 돌기, map·filter·fold, while 대신 꼬리 재귀.
lede: 프로그램이 경우에 따라 다른 일을 하고, 같은 일을 여러 번 하게 만드는 법.
---
# 조건과 반복

:::goals

- 비교 연산자와 `and`·`or`·`not`, `in`
- `if`·`else if`·`else`, 그리고 `if`가 값을 낸다는 것
- 한 줄 조건 `c then a else b`
- `for`로 목록과 범위를 도는 법, `for`가 리스트를 만든다는 것
- `map`·`filter`·`fold`로 목록을 바꾸고 고르고 모으는 법
- `while`이 없는 이유와, 대신 쓰는 꼬리 재귀

:::

## 프로그램의 흐름 {#flow}

2장의 스크립트는 위에서 아래로 한 줄씩 한 번만 실행됐다. 실제 일은 그렇게 곧지 않다. "디스크 사용률이 90%를 넘으면 알린다"처럼 **경우에 따라** 다른 일을 해야 하고, "서버마다 확인한다"처럼 같은 일을 **여러 번** 해야 한다. 앞의 것을 **조건(condition)**, 뒤의 것을 **반복(loop)**이라고 한다. 거의 모든 프로그램은 값, 조건, 반복의 조합이다.

## 비교 {#comparison}

조건은 질문이고, 질문의 답은 bool(`true`나 `false`)이다. 질문은 비교 연산자로 만든다.

<table>
  <thead><tr><th>연산자</th><th>뜻</th><th>예</th></tr></thead>
  <tbody>
    <tr><td><code>==</code> <code>!=</code></td><td>같다, 다르다</td><td><code>status == 'ok</code></td></tr>
    <tr><td><code>&lt;</code> <code>&gt;</code></td><td>작다, 크다</td><td><code>usage &gt; 90</code></td></tr>
    <tr><td><code>&lt;=</code> <code>&gt;=</code></td><td>작거나 같다, 크거나 같다</td><td><code>retries &lt;= 3</code></td></tr>
    <tr><td><code>in</code> <code>not in</code></td><td>들어 있다, 없다</td><td><code>"ERROR" in line</code></td></tr>
  </tbody>
</table>

```fung run
print("web-1" == "web-1", "web-1" < "web-2", 10 < 9.5)
print(5 in 1..10, 10 in 1..10, "ERROR" in "2026-10-20 ERROR disk full")
print(1 == "1")
```

- 문자열은 사전 순서로 비교한다. 정확히는 글자의 유니코드 번호 순서라서, 대문자가 소문자보다 앞이다.
- `1..10`은 1부터 10 **앞까지**의 정수 범위다. 10까지 넣으려면 `1..=10`이라고 쓴다.
- 타입이 다른 값을 `==`로 비교하면 그냥 `false`다(정수와 실수는 숫자로 비교해서 `1 == 1.0`은 참이다). 크기 비교 `<`는 타입이 다르면 에러다.
- 비교는 이어 쓸 수 없다. `0 < x < 10`은 E0111 에러이고, `0 < x and x < 10`이라고 쓴다.

질문을 엮을 때는 `and`, `or`, `not`을 쓴다. `and`는 왼쪽이 거짓이면 오른쪽을 아예 계산하지 않는다(`or`는 왼쪽이 참이면). 그래서 아래처럼 "0이 아닐 때만 나눈다"를 한 줄로 쓸 수 있다.

```fung run
count = 0
print(count != 0 and 100 / count > 5)
```

## if와 else {#if}

`if`는 조건이 참일 때만 블록을 실행한다. 블록은 `:`로 열고 `;;`로 닫는다. 조건이 거짓일 때 할 일은 `else:` 뒤에, 조건을 더 나눌 때는 `else if`를 쓴다. 위에서부터 차례로 보고 처음 참인 가지 하나만 실행한다.

```fung run
usage = 91
if usage >= 90:
  print("critical: disk", usage, "%")
else if usage >= 75:
  print("warning: disk", usage, "%")
else:
  print("ok")
;;
```

들여쓰기는 읽기 좋으라고 하는 것이고 뜻은 없다. 블록의 끝은 오직 `;;`가 정한다. 짧으면 한 줄로 써도 된다: `if usage >= 90: print("critical") ;;`. 들여쓰기는 `fung fmt`가 맞춰 준다(14장).

조건 자리에는 반드시 bool이 와야 한다. `if count:`처럼 숫자를 주면 런타임 에러 E0505다. "0이 아니면"은 `if count != 0:`이라고 분명히 쓴다.

### if는 값을 낸다 {#if-value}

fung의 `if`는 **식(expression)**이다. 실행된 가지의 마지막 값이 `if` 전체의 값이 된다. 그래서 결과를 바로 이름에 대입할 수 있다. 3장에서 본 "블록은 바깥 변수를 바꿀 수 없다" 규칙은 이렇게 푼다: 가지 안에서 변수를 고치는 대신, `if`가 낸 값을 대입한다.

```fung run
usage = 78
level = if usage >= 90:
  "critical"
else if usage >= 75:
  "warning"
else:
  "ok"
;;
print(level)
```

값으로 쓰는 `if`에는 `else`가 반드시 있어야 한다. 없으면 조건이 거짓일 때 값이 `nil`이 되는데, 그것이 의도인지 실수인지 알 수 없어서 컴파일 에러 E0223으로 막는다. 정말 `nil`을 원하면 `else: nil`이라고 쓴다. 함수의 마지막 항목도 반환값이라 값으로 쓰는 자리다(5장).

### 한 줄 조건: then … else {#then}

값 두 개 중 하나를 고르는 짧은 조건은 `조건 then 값1 else 값2`로 쓴다. 다른 언어의 `조건 ? a : b`와 같은 것이다. `else`는 빠뜨릴 수 없고, 이어 쓸 수 있다.

```fung run
usage = 91
print(usage > 80 then "alert" else "fine")
size = 1_500_000
print(size > 1_000_000 then "big" else size > 1000 then "medium" else "small")
```

## for: 목록 돌기 {#for}

`for 이름 in 목록:`은 목록의 원소마다 블록을 한 번씩 실행한다. 원소는 차례로 `이름`에 들어간다. 목록 자리에는 리스트(7장)나 범위를 쓴다.

```fung run
for host in ["web-1", "web-2", "db-1"]:
  print("checking", host)
;;
for i in 1..=3:
  print("attempt", i)
;;
```

fung의 `for`도 식이다. 블록이 낸 값을 원소마다 모아 **새 리스트**를 만든다. 그 리스트를 이름에 대입하면 "목록의 각 원소를 바꾼 목록"이 된다.

```fung run
usages = [45, 91, 78, 99]
levels = for u in usages:
  u >= 90 then "critical" else "ok"
;;
print(levels)
```

위의 첫 예제처럼 결과를 아무도 쓰지 않는 자리의 `for`는 리스트를 만들지 않고 돌기만 한다. 문자열의 글자나 맵(7장)을 돌 때는 `chars(s)`, `pairs(m)`으로 리스트로 바꿔서 돈다.

## map, filter, fold {#map-filter-fold}

목록을 다루는 일은 거의 세 가지로 나뉜다. fung은 세 가지 각각에 함수를 둔다.

- **바꾸기** `map(xs, f)`: 원소마다 `f`를 적용한 새 리스트. `for`와 같다.
- **고르기** `filter(xs, f)`: `f`가 참인 원소만 남긴 새 리스트.
- **모으기** `fold(xs, init, f)`: 처음 값 `init`에서 시작해, 원소마다 "지금까지 모은 값"과 그 원소로 다음 값을 만든다. 합계, 개수, 가장 큰 값 같은 것.

`f` 자리에는 함수를 준다. `u => u >= 90`은 "`u`를 받아 `u >= 90`을 돌려주는 함수"를 짧게 쓴 것이다(5장에서 자세히 배운다).

```fung run
usages = [45, 91, 78, 99]
print(map(usages, u => u + 1))
print(filter(usages, u => u >= 90))
print(fold(usages, 0, (total, u) => total + u))
```

`fold`는 처음엔 낯설다. 위의 합계는 이렇게 진행된다: `0`에서 시작해 `0 + 45 = 45`, `45 + 91 = 136`, `136 + 78 = 214`, `214 + 99 = 313`. 매 단계의 "모은 값"이 다음 단계로 넘어갈 뿐, 어떤 변수도 바뀌지 않는다. 합계는 자주 쓰므로 `sum(xs)`도 있다.

여러 단계를 이을 때는 **파이프** `|>`를 쓰면 데이터가 흐르는 순서대로 읽힌다. `xs |> filter(f)`는 `filter(xs, f)`와 같다. 파이프는 5장에서 자세히 다룬다.

```fung run
usages = [45, 91, 78, 99]
critical = usages |> filter(u => u >= 90) |> len
print(critical, "disk(s) over 90%")
```

## while이 없는 이유 {#no-while}

많은 언어에는 "조건이 참인 동안 되풀이한다"는 `while`이 있다. fung에는 없다. `while`은 반복할 때마다 바깥 변수를 고쳐야 끝날 수 있는데, fung에서는 값이 바뀌지 않고 블록이 바깥 변수를 고칠 수 없기 때문이다(3장). 대신 두 가지를 쓴다.

1. 목록이 있는 반복은 앞에서 본 `for`, `map`, `filter`, `fold`.
2. 목록 없이 "될 때까지" 되풀이하는 반복(재시도, 대기, 누적)은 **재귀(recursion)**, 곧 함수가 자기 자신을 부르는 것.

함수는 5장에서 배우지만, 모양은 간단하다. 아래는 8080번부터 시작해 이미 쓰는 포트를 건너뛰고 비어 있는 첫 포트를 찾는다. "쓰는 중이면 다음 번호로 같은 일을 다시 하고, 아니면 그 번호가 답"이다.

```fung run
fn next_free(port, used) = port in used then next_free(port + 1, used) else port
print(next_free(8080, [8080, 8081, 8083]))
```

`while`에서 바뀌던 변수(`port`)가 여기서는 **다음 호출의 인자**가 된다. 바뀌는 것이 없으니 3장의 규칙과 부딪히지 않는다.

이렇게 함수가 하는 마지막 일이 자기 호출인 것을 **꼬리 재귀(tail recursion)**라고 한다. fung은 꼬리 호출을 할 때 새 자리를 쌓지 않고 지금 자리를 다시 쓰므로, 백만 번을 돌아도 문제가 없다. 결과를 모아 가려면 모은 값을 인자로 넘긴다. 아래는 1초에서 시작해 두 배씩 늘리는 재시도 간격을 30초까지 모은다.

```fung run
fn backoff(delay, limit, acc) = delay > limit then acc else backoff(delay * 2, limit, acc + [delay])
print(backoff(1s, 30s, []))
```

자주 쓰는 모양은 함수로 준비되어 있다. `until(x, f, pred)`는 `pred`가 참이 될 때까지 `x`에 `f`를 되풀이하고, `iterate(x, f, n)`은 `f`를 `n`번 적용한다.

```fung run
print(until(1, n => n * 2, n => n > 100))
print(iterate(1s, d => d * 2, 4))
```

## 짚고 넘어가기 {#notes}

:::box learn 반복과 재귀

재귀는 "문제를 한 걸음 줄인 같은 문제로 바꾸는 것"이다. 3부터 0까지 세는 일은 "3을 찍고, 2부터 0까지 세는 일"이다. 줄이다 보면 더 줄일 수 없는 경우(0)에 닿고, 거기서 멈춘다. 이 멈추는 경우를 빠뜨리면 끝나지 않는다. 재귀 함수를 쓸 때는 늘 "언제 멈추는가"부터 쓴다.

```fung run
fn countdown(n):
  if n == 0:
    print("liftoff")
  else:
    print(n)
    countdown(n - 1)
  ;;
;;
countdown(3)
```

:::

:::box mistake 값으로 쓰는 if에 else가 없음 (E0223)

`label = if ok: "up" ;;`은 컴파일 에러다. 조건이 거짓이면 `label`이 `nil`이 되는데, 대개 `else`를 빠뜨린 실수이기 때문이다. 결과를 쓰는 `for`(`xs = for …`) 안의 `if`도 값을 내는 자리라 같다. `else`를 쓰거나, 원소를 고르는 것이 목적이면 `filter`를 쓴다. 결과를 쓰지 않는 `for` 문장 안이나 `if`를 문장으로 쓸 때는 `else` 없이 써도 된다. 함수의 마지막 `if`는 반환값이라 문장이 아니다.

```fung run exit=2 error=E0223
ok = false
label = if ok: "up" ;;
print(label)
```

:::

:::box mistake 조건에 bool이 아닌 값 (E0505)

`if len(errors):`는 런타임 에러 `expected bool, got int`다(`if name:`이면 `got str`). fung은 `0`, 빈 문자열, 빈 리스트를 거짓으로 봐 주지 않는다. `if len(errors) > 0:`, `if name != "":`, `if not is_empty(errors):`처럼 질문을 분명히 쓴다.

:::

:::box mistake 반복 안에서 합계 쌓기 (E0205)

`for` 안에서 `total = total + x`는 E0205다(3장). 합계는 `sum`, 개수는 `len`이나 `filter` 뒤의 `len`, 그 밖의 누적은 `fold`로 쓴다.

:::

:::box other 다른 언어는

Python의 `x if c else y`, JavaScript의 `c ? x : y`가 fung의 `c then x else y`다. Python의 리스트 내포 `[f(x) for x in xs]`가 fung의 `for x in xs: f(x) ;;`와 같다. 쉘의 `while read line`처럼 줄을 읽는 반복은 fung에서 줄 목록을 받아 `for`로 돈다(11장). Python과 JavaScript는 깊은 재귀에서 스택이 넘치지만, fung은 꼬리 재귀를 반복처럼 돌린다.

:::

## 해 보기 {#exercises}

:::exercise 디스크 경보

서버들의 디스크 사용률이 `[45, 91, 78, 99, 82]`다. 각 사용률을 `90` 이상은 `"critical"`, `75` 이상은 `"warning"`, 나머지는 `"ok"`로 바꾼 리스트를 찍고, `"critical"`이 몇 개인지 찍어 보자.

:::solution

```fung run
usages = [45, 91, 78, 99, 82]
levels = for u in usages:
  u >= 90 then "critical" else u >= 75 then "warning" else "ok"
;;
print(levels)
print(levels |> filter(l => l == "critical") |> len)
```

:::

:::

:::exercise 가장 큰 값 찾기

`fold`로 `[45, 91, 78, 99, 82]`에서 가장 큰 값을 찾아 보자. 처음 값은 첫 원소로 한다. (실제로는 `max(xs)`가 있다.)

:::solution

모은 값 `best`와 원소 `u` 중 큰 것을 다음 단계로 넘긴다.

```fung run
usages = [45, 91, 78, 99, 82]
print(fold(usages, usages[0], (best, u) => u > best then u else best))
print(max(usages))
```

:::

:::

## 정리 {#summary}

- 비교는 bool을 낸다. 비교는 이어 쓰지 않고 `and`로 엮는다. `and`·`or`는 필요할 때만 오른쪽을 계산한다.
- `if`·`else if`·`else`는 블록이고 식이다. 값으로 쓸 때는 `else`가 꼭 있어야 한다(E0223).
- 짧은 조건은 `c then a else b`.
- `for x in xs: … ;;`는 원소마다 블록을 돌고, 그 값들로 새 리스트를 만든다.
- 바꾸기는 `map`, 고르기는 `filter`, 모으기는 `fold`.
- `while`은 없다. 목록이 없는 반복은 꼬리 재귀로 쓰고, `until`·`iterate`가 흔한 모양을 대신한다.

레퍼런스: [조건](../reference/language.html#conditions), [for](../reference/language.html#for), [꼬리 호출](../reference/language.html#functions), [`map`](../reference/prelude/collections.html#fn-map), [`filter`](../reference/prelude/collections.html#fn-filter), [`fold`](../reference/prelude/collections.html#fn-fold), [반복 패턴](../reference/prelude/loops.html), [E0223](../reference/errors.html#E0223). 꼬리 호출이 프레임을 다시 쓰는 방법은 [함수와 클로저](../vm/09-closures.html)에서 다룬다.
