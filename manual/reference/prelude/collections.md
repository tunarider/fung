---
lede: 여러 값을 담은 묶음(리스트, 범위, 채널의 받는 끝)을 세고, 바꾸고, 고르고, 접고, 정렬한다. 모든 함수는 원본을 건드리지 않고 새 값을 돌려준다. 원소를 도는 함수는 Rust로, 그 위의 조합은 fung으로 썼다.
description: fung prelude 컬렉션: len, map, filter, fold, sort_by, group_by, take 등 리스트·범위·채널을 다루는 함수.
---
# 컬렉션

## 컬렉션이란 {#concepts}

**컬렉션**(collection)은 값 여러 개를 하나로 묶은 값이다. fung에서 가장 많이 쓰는 컬렉션은 **리스트**(list)다. `["web-1", "web-2", "db-1"]`처럼 대괄호 안에 순서대로 쓰고, `xs[0]`으로 첫 원소를, `xs[1..]`로 첫 원소를 뺀 나머지를 꺼낸다(인덱스는 0부터). 리스트 말고도 이 페이지의 함수가 받는 것이 둘 더 있다.

- **범위**(range): `0..10`(10은 빠짐), `1..=10`(10 포함). 정수를 차례로 내놓지만 리스트를 미리 만들지 않으므로 아주 큰 범위도 공짜로 만든다.
- **채널의 받는 끝**(receiver): 다른 태스크가 보내는 값의 흐름([동시성](concurrency.html)). 값이 올 때마다 처리하고, 채널이 닫히면 끝난다. 일찍 멈추는 함수(`find`·`any`·`all`·`take`·`first`)는 필요한 만큼만 받고 남은 값을 채널에 둔다. 전체가 필요한 함수(`sort`·`reverse`·`last` 등)는 닫힐 때까지 받는다.

문자열은 컬렉션 함수에 넣지 않는다(`map("abc", f)`는 에러). 글자로 다루려면 [`chars`](strings.html#fn-chars)로 글자 리스트를 만든다. 맵은 [맵](maps.html) 함수로 다루고, 쌍의 리스트가 필요하면 [`pairs`](maps.html#fn-pairs)를 쓴다.

### 불변: 원본은 바뀌지 않는다 {#immutable}

fung의 리스트는 **불변**(immutable)이다. 한 번 만든 리스트는 바뀌지 않는다. `sort(xs)`는 `xs`를 정렬하는 것이 아니라 정렬된 **새** 리스트를 돌려준다. 그래서 결과를 쓰려면 이름에 담는다: `sorted = sort(xs)`. 원본을 여러 곳이 같이 보고 있어도 한 곳의 작업이 다른 곳을 몰래 바꾸지 않으므로, 태스크 사이에 리스트를 그냥 넘겨도 안전하다. 사본은 원본과 구조를 나눠 쓰므로 `acc + [x]`로 하나씩 붙여 가도 매번 전체를 복사하지 않는다([누적과 재귀의 비용](../language.html#for)).

### 함수를 받는 함수 {#higher-order}

`map`·`filter`·`fold`처럼 **함수를 인자로 받는** 함수를 고차 함수(higher-order function)라고 한다. 반복문을 직접 쓰는 대신 "각 원소에 무엇을 할지"만 함수로 건넨다. 보통 그 자리에서 람다(`x => x * 2`)를 쓰고, 이미 있는 함수 이름(`len`, `parse_int`)을 넘겨도 된다.

모든 함수가 컬렉션을 첫 인자로 받으므로 파이프 `|>`로 단계를 이을 수 있다. `xs |> filter(f) |> map(g)`는 `map(filter(xs, f), g)`와 같고, 위에서 아래로 읽힌다([파이프](../language.html#pipe)).

```fung run
hosts = ["web-1", "db-1", "web-2", "web-3"]
web = hosts |> filter(h => starts_with(h, "web")) |> map(upper)
print(web, len(web))
print(hosts)
```

마지막 줄처럼 원본 `hosts`는 그대로다.

### 크기 제한 {#limits}

결과가 모든 원소를 담는 함수(`map`, `sort`, `reverse`, `unique` 등)는 원소 1억 개를 넘는 범위를 받으면 만들기 전에 E0806으로 거절한다. 메모리를 다 쓰고 죽는 대신 이유를 알려 주기 위해서다. 결과가 작을 수 있는 `filter`·`fold`와 일찍 멈추는 `find`·`any`·`take`는 큰 범위도 받는다(원소를 하나씩 만들어 보므로 시간은 든다). `len`은 원소를 만들지 않고 계산한다.

## 세기와 고르기 {#basics}

:::fn len(x)

문자열의 글자 수, 리스트·맵·튜플·범위의 원소 수를 돌려준다.

#### 설명

문자열은 바이트가 아니라 **글자**(유니코드 코드 포인트)를 센다. 그래서 `len("참치")`는 2다. 결합 문자로 쓴 `"e\u{301}"`(é)는 코드 포인트 둘이라 2다. 범위는 원소를 만들지 않고 계산하므로 `len(0..1_000_000_000_000)`도 바로 답한다. 끝이 시작보다 작은 범위(`5..1`)는 비어 있어 0이다.

채널의 받는 끝은 몇 개가 올지 미리 알 수 없으므로 길이가 없다(E0506). 다 받아 세려면 `len(map(rx, x => x))`처럼 리스트로 만든다.

#### 매개변수

- `x`: `str`, `list`, `map`, `tuple`, `range`.

#### 돌려주는 값

정수.

#### 에러

그 밖의 타입이면 런타임 에러 E0506(`cannot take the length of int`).

#### 예제

**여러 타입의 길이**

```fung run
print(len("참치"), len([1, 2]), len({a: 1, b: 2}), len((1, 2, 3)))
print(len(0..10), len(0..=10), len(5..1), len(0..1_000_000_000_000))
```

**길이가 없는 값**

```fung run exit=1 error=E0506
print(len(42))
```

#### 같이 보기

[`is_empty`](#fn-is_empty), [`chars`](strings.html#fn-chars)

:::

:::fn is_empty(x) fung

비었으면 `true`. `len(x) == 0`과 같다.

#### 설명

`len`이 받는 모든 값을 받는다. 범위는 길이를 세지 않고 원소가 하나라도 있는지만 본다. `[nil]`은 원소가 하나 있으므로 비어 있지 않다.

조건 자리에는 `bool`만 오므로, 리스트가 비었는지 물을 때 `if xs:`라고 쓸 수 없다. `if is_empty(xs):` 또는 `if not is_empty(xs):`로 쓴다.

#### 매개변수

- `x`: `str`, `list`, `map`, `tuple`, `range`.

#### 돌려주는 값

`bool`.

#### 에러

`len`과 같다(E0506).

#### 예제

**여러 값**

```fung run
print(is_empty([]), is_empty(""), is_empty({}), is_empty(0..0))
print(is_empty([nil]), is_empty(5..1), is_empty("x"))
```

**조건에 쓰기**

```fung run
failed = []
print(is_empty(failed) then "all ok" else "${len(failed)} failed")
```

#### 같이 보기

[`len`](#fn-len), [조건](../language.html#conditions)

:::

:::fn first(xs) fung

첫 원소를, 비었으면 `nil`을 돌려준다.

#### 설명

`xs[0]`과 달리 빈 리스트에서도 에러가 나지 않는다. 그래서 "있으면 쓰고 없으면 기본값"을 `first(xs) ?? 기본값`으로 쓸 수 있다. 다만 원소 자체가 `nil`인 것과 비어 있는 것은 구별되지 않는다.

범위는 리스트를 만들지 않고 첫 수를 준다. 채널의 받는 끝이면 값 하나를 받고(나머지는 채널에 남는다), 닫혀 있고 남은 값이 없으면 `nil`이다.

#### 매개변수

- `xs`: 리스트, 범위, 받는 끝.

#### 돌려주는 값

첫 원소 또는 `nil`.

#### 에러

다른 타입이면 런타임 에러 E0506.

#### 예제

**리스트와 범위**

```fung run
print(first([3, 1, 2]), first([]), first(5..10), first(0..0))
```

**기본값과 함께**

```fung run
mirrors = []
print(first(mirrors) ?? "https://default.example")
```

#### 같이 보기

[`last`](#fn-last), [`find`](#fn-find), [`take`](#fn-take)

:::

:::fn last(xs) fung

마지막 원소를, 비었으면 `nil`을 돌려준다.

#### 설명

`xs[-1]`과 같지만 빈 리스트에서 에러 대신 `nil`이다. 범위는 계산으로 마지막 수를 준다. 채널의 받는 끝이면 닫힐 때까지 모두 받고 마지막 값을 준다.

#### 매개변수

- `xs`: 리스트, 범위, 받는 끝.

#### 돌려주는 값

마지막 원소 또는 `nil`.

#### 에러

다른 타입이면 런타임 에러 E0506.

#### 예제

**리스트와 범위**

```fung run
print(last([3, 1, 2]), last([]), last(0..10), last(1..=10))
```

**가장 최근 것 고르기**

```fung run
releases = ["v0.2.0", "v0.3.0", "v0.4.0"]
print("latest:", last(releases) ?? "none")
```

#### 같이 보기

[`first`](#fn-first), [`reverse`](#fn-reverse)

:::

:::fn take(xs, n) fung

앞에서 최대 `n`개를 리스트로 돌려준다.

#### 설명

슬라이스 `xs[..n]`과 달리 `n`이 길이보다 커도 에러가 아니다. 있는 만큼만 준다. `n`이 0 이하면 `[]`다. 범위는 필요한 수만 만들므로 `take(0..1_000_000_000, 3)`도 빠르다. 채널의 받는 끝이면 `n`개를 받을 때까지(또는 닫힐 때까지) 받고 멈추며, 남은 값은 채널에 그대로 있다.

#### 매개변수

- `xs`: 리스트, 범위, 받는 끝.
- `n`: 정수.

#### 돌려주는 값

리스트.

#### 에러

`n`이 정수가 아니면 런타임 에러 E0803(`take expects an int count, got 'float`).

#### 예제

**넘쳐도 에러가 아니다**

```fung run
print(take([1, 2, 3], 2), take([1], 5), take([1, 2], -1), take(0..1_000_000_000, 3))
```

**상위 3개**

```fung run
sizes = {"a.log": 120, "b.log": 900, "c.log": 40, "d.log": 300}
top = pairs(sizes) |> sort_by(((_, n)) => -n) |> take(3)
print(top)
```

**채널에서 몇 개만 받기**

```fung run
(tx, rx) = channel(5)
each(1..=5, i => send(tx, i))
print(take(rx, 2))
print(take(rx, 2))
```

두 번째 `take`는 남아 있던 값에서 이어 받는다.

#### 같이 보기

[`drop`](#fn-drop), [`first`](#fn-first), [슬라이스](../language.html#access)

:::

:::fn drop(xs, n) fung

앞의 `n`개를 뺀 나머지를 리스트로 돌려준다.

#### 설명

슬라이스 `xs[n..]`과 달리 `n`이 길이보다 커도 에러가 아니고 `[]`를 준다. `n`이 0 이하면 전부를 준다. 범위는 필요한 부분만 만든다. 받는 끝이면 닫힐 때까지 모두 받은 뒤 앞의 `n`개를 버린다(그래서 닫히지 않는 채널에서는 끝나지 않는다).

#### 매개변수

- `xs`: 리스트, 범위, 받는 끝.
- `n`: 정수.

#### 돌려주는 값

리스트.

#### 에러

`n`이 정수가 아니면 런타임 에러 E0803.

#### 예제

**넘쳐도 에러가 아니다**

```fung run
print(drop([1, 2, 3], 1), drop([1], 5), drop([1, 2], -1), drop(0..5, 3))
```

**머리줄 빼기**

```fung run
rows = ["name,size", "a.log,120", "b.log,900"]
print(drop(rows, 1))
```

#### 같이 보기

[`take`](#fn-take), [슬라이스](../language.html#access)

:::

:::fn range(a, b = nil) fung

범위 값을 만든다. `range(n)`은 `0..n`, `range(a, b)`는 `a..b`.

#### 설명

범위 리터럴(`0..n`)을 함수로 쓰는 것이다. 끝은 들어가지 않는다. 끝을 넣으려면 리터럴 `a..=b`를 쓴다. 돌려주는 값은 리스트가 아니라 범위라서, 리스트가 필요하면 `map(range(n), x => x)`처럼 만든다. 끝이 시작보다 작으면 빈 범위다.

#### 매개변수

- `a`: 정수. `b`가 없으면 끝, 있으면 시작.
- `b`: 정수 또는 `nil`.

#### 돌려주는 값

범위.

#### 에러

정수가 아니면 런타임 에러 E0501(`cannot make a range from int and str`).

#### 예제

**범위 만들기**

```fung run
print(range(3), range(2, 5), map(range(3), x => x * 10))
```

**번호 붙이기**

```fung run
names = ["web", "db", "cache"]
for i in range(len(names)):
  print(i + 1, names[i])
;;
```

#### 같이 보기

[범위](../language.html#values), [`zip`](#fn-zip), [for](../language.html#for)

:::

:::fn contains(c, x) fung

`x in c`와 같다: 리스트의 원소인지, 맵의 키인지, 부분 문자열인지.

#### 설명

`c`의 타입에 따라 묻는 것이 다르다.

- 리스트·범위: `x`와 `==`인 원소가 있는가. 비교 규칙을 따르므로 `contains([1], 1.0)`도 참이다.
- 맵: `x`가 키인가(값이 아니다). 키는 문자열이므로 `x`도 문자열이어야 한다.
- 문자열: `x`가 부분 문자열인가. 빈 문자열은 언제나 들어 있다.

심볼과 문자열을 견주면 E0517이다(`contains(['a], "a")`). 외부에서 온 문자열은 [`sym`](convert.html#fn-sym)으로 바꿔서 찾는다.

#### 매개변수

- `c`: 리스트, 범위, 맵, 문자열.
- `x`: 찾을 값.

#### 돌려주는 값

`bool`.

#### 에러

- 맵에서 문자열이 아닌 키를, 문자열에서 문자열이 아닌 값을 찾으면 런타임 에러 E0501(`cannot search map and int`).
- 심볼과 문자열을 견주면 E0517.

#### 예제

**세 가지 뜻**

```fung run
print(contains([1, 2], 2), contains({a: 1}, "a"), contains({a: 1}, "b"), contains("hello", "ell"))
print(contains(0..10, 5), 2 in [1, 2])
```

**허용 목록 검사**

```fung run
allowed = ["prod", "staging"]
for target in ["prod", "dev"]:
  print(target, contains(allowed, target) then "ok" else "refused")
;;
```

#### 같이 보기

[`find`](#fn-find), [`any`](#fn-any), [`starts_with`](strings.html#fn-starts_with), [비교와 동일성](../language.html#values)

:::

## 바꾸기와 고르기 {#transform}

:::fn map(xs, f)

각 원소에 `f`를 부른 결과를 모아 새 리스트로 돌려준다.

#### 설명

`map([a, b, c], f)`는 `[f(a), f(b), f(c)]`다. 원소 수는 그대로이고 순서도 지킨다. 원본은 바뀌지 않는다. `for x in xs: 식 ;;`도 리스트를 만들지만, 이미 있는 함수를 넘기거나 파이프로 이을 때는 `map`이 짧다.

`f`는 인자 하나를 받는 함수다. 범위를 주면 리스트가 된다(`map(0..3, x => x)`는 `[0, 1, 2]`). 받는 끝을 주면 값이 올 때마다 `f`를 부르고 채널이 닫히면 끝난다.

:::box mistake map 안의 ? 는 컴파일 에러다 (E0226)

`map(xs, s => parse_int(s)?)`처럼 `map`에 바로 쓴 람다 안의 `?`는 컴파일 에러 E0226이다. `?`는 람다에서 돌아올 뿐이라, `Err`가 바깥 함수로 가지 않고 결과 리스트의 원소가 되어 버리기 때문이다. 실패할 수 있는 변환은 [`try_map`](#fn-try_map)을 쓴다. 처음 나온 `Err`에서 멈추고 그것을 돌려준다.

:::

#### 매개변수

- `xs`: 리스트, 범위, 받는 끝.
- `f`: 원소 하나를 받는 함수.

#### 돌려주는 값

리스트.

#### 에러

- `xs`가 문자열·맵·튜플 등이면 런타임 에러 E0506(`cannot map over str`).
- `f`가 함수가 아니면 E0403, 인자 수가 맞지 않으면 E0407.
- 결과가 1억 개를 넘으면 E0806.

#### 예제

**각 원소를 바꾸기**

```fung run
print([1, 2, 3] |> map(x => x * 10))
print(map(1..=4, x => x * x))
```

**이미 있는 함수를 넘기기**

```fung run
print(map(["web", "db"], upper), map(["a", "bb", "ccc"], len))
```

**맵의 리스트에서 필드 하나씩**

```fung run
servers = [{name: "web-1", cpu: 72}, {name: "db-1", cpu: 15}]
print(map(servers, s => s.name))
print(map(servers, s => "${s.name}: ${s.cpu}%"))
```

**맵을 돌 때는 pairs로**

```fung run
ports = {web: 80, db: 5432}
print(pairs(ports) |> map(((k, v)) => "${k}=${v}"))
```

**문자열은 바로 받지 않는다**

```fung run exit=1 error=E0506
print(map("abc", upper))
```

`map(chars("abc"), upper)`로 쓴다.

#### 같이 보기

[`try_map`](#fn-try_map), [`filter`](#fn-filter), [`each`](#fn-each), [for](../language.html#for), [`conc.pmap`](../modules/conc.html#fn-conc-pmap)(병렬)

:::

:::fn try_map(xs, f)

`f`가 돌려준 결과값을 모아, 모두 `Ok`면 `Ok(리스트)`, 아니면 처음 나온 `Err`를 돌려준다.

#### 설명

`f`는 원소마다 `Ok(v)`나 `Err(e)`를 돌려주는 함수다(`parse_int` 같은). 모두 `Ok(v)`면 `Ok([v, …])`를 돌려준다. 처음 `Err`가 나오면 그 `Err`를 그대로 돌려주고 남은 원소에는 `f`를 부르지 않는다. 그래서 "하나라도 틀리면 전체가 실패"인 변환에 쓴다. 결과는 보통 `!`나 `?`로 푼다.

이미 결과값의 리스트를 가지고 있으면 `try_map(rs, r => r)`로 하나로 모은다. 실패한 것만 건너뛰고 싶으면 `try_map` 대신 `map`과 [`filter(is_ok)`](results.html#fn-is_ok)를 쓴다.

:::box mistake 파이프 끝의 ? 와 ! 는 괄호로 감싼다

후위 `?`·`!`는 `|>`보다 단단히 붙는다([우선순위](../language.html#precedence)). 그래서 `xs |> try_map(f)?`는 `xs |> (try_map(f)?)`로 읽혀 `try_map`이 인자 하나로 불린다(E0407 `missing argument`). 파이프 전체를 괄호로 감싸 `(xs |> try_map(f))?`로 쓰거나, 파이프 없이 `try_map(xs, f)?`로 쓴다.

:::

#### 매개변수

- `xs`: 리스트, 범위, 받는 끝.
- `f`: 원소 하나를 받아 `Ok`나 `Err`를 돌려주는 함수.

#### 돌려주는 값

`Ok(리스트)` 또는 처음 나온 `Err(e)`.

#### 에러

`f`가 결과값이 아닌 값을 돌려주면 런타임 에러 E0605. 나머지는 `map`과 같다.

#### 예제

**모두 성공, 하나 실패**

```fung run
print(["1", "2"] |> try_map(parse_int))
print(["1", "x", "y"] |> try_map(parse_int))
```

**함수 안에서 ? 와 함께**

```fung run
fn ports(spec):
  nums = (split(spec, ",") |> try_map(s => parse_int(trim(s))))?
  Ok(sort(nums))
;;

print(ports("443, 80, 8080"))
print(ports("443, http"))
```

**결과값 리스트를 하나로**

```fung run
rs = [Ok(1), Ok(2), Ok(3)]
print(try_map(rs, r => r))
```

#### 같이 보기

[`map`](#fn-map), [결과값과 에러](../language.html#results), [E0226](../errors.html#E0226)

:::

:::fn filter(xs, f)

`f(x)`가 `true`인 원소만 모은 새 리스트를 돌려준다.

#### 설명

원소의 순서는 지킨다. `f`는 반드시 `bool`을 돌려줘야 한다. `0`이나 `nil`, 빈 문자열을 거짓으로 쳐 주지 않는다(E0505). 그래서 "값이 있는 것만"은 `x => x != nil`처럼 조건을 다 쓴다.

#### 매개변수

- `xs`: 리스트, 범위, 받는 끝.
- `f`: 원소를 받아 `bool`을 돌려주는 함수.

#### 돌려주는 값

리스트.

#### 에러

- `f`가 `bool`이 아닌 값을 돌려주면 런타임 에러 E0505(`expected bool, got int`).
- `xs`가 받지 않는 타입이면 E0506.

#### 예제

**조건에 맞는 것만**

```fung run
print(filter([1, 2, 3, 4], x => x % 2 == 0), filter(1..10, x => x > 7))
```

**로그에서 에러 줄만**

```fung run
log = ["INFO start", "ERROR disk full", "INFO ok", "ERROR timeout"]
errors = log |> filter(l => starts_with(l, "ERROR"))
print(len(errors), errors)
```

**nil 빼기**

```fung run
print(filter([1, nil, 2], x => x != nil))
```

**조건은 bool이어야 한다**

```fung run exit=1 error=E0505
print(filter([0, 1, 2], x => x))
```

#### 같이 보기

[`find`](#fn-find), [`any`](#fn-any), [`map`](#fn-map), [`is_ok`](results.html#fn-is_ok)

:::

:::fn fold(xs, init, f)

`init`에서 시작해 원소를 하나씩 `f`로 합쳐 값 하나로 접는다: `f(f(init, x1), x2)`….

#### 설명

`fold`는 "누적값"을 들고 원소를 왼쪽부터 차례로 지나간다. 처음 누적값은 `init`이고, 원소마다 `acc = f(acc, x)`로 바꾼다. 다 지나간 뒤의 `acc`가 결과다. 빈 리스트면 `init`을 그대로 돌려준다.

합계, 최댓값, 개수 세기, 리스트나 맵 쌓기처럼 "여러 개에서 하나를 만드는" 거의 모든 일을 `fold`로 할 수 있다. 자주 쓰는 것은 따로 있다: [`sum`](#fn-sum), [`max`](#fn-max), [`group_by`](#fn-group_by). fung에는 바깥 변수를 바꾸는 반복문이 없으므로(블록 안에서 바깥 이름에 다시 대입하면 E0205) 반복하며 값을 쌓는 일은 `fold`나 재귀로 한다.

:::box tip 문자열은 리스트에 모아 join

`fold(xs, "", (acc, s) => acc + s)`처럼 문자열을 하나씩 이어 붙이면 매번 문자열 전체를 새로 만든다. 리스트에 모은 뒤 [`join`](strings.html#fn-join)하는 것이 빠르다. 리스트를 쌓을 때는 앞에 붙이는 `[x] + acc` 대신 뒤에 붙이는 `acc + [x]`를 쓴다.

:::

#### 매개변수

- `xs`: 리스트, 범위, 받는 끝.
- `init`: 처음 누적값.
- `f`: `(누적값, 원소)`를 받아 새 누적값을 돌려주는 함수.

#### 돌려주는 값

마지막 누적값.

#### 에러

`xs`가 받지 않는 타입이면 런타임 에러 E0506(`cannot fold str`). `f` 안의 에러는 그대로 난다.

#### 예제

**합계와 곱**

```fung run
print(fold([1, 2, 3], 0, (acc, x) => acc + x))
print(fold(1..=5, 1, (acc, x) => acc * x))
print(fold([], 99, (acc, x) => acc + x))
```

**한 번에 여러 개 세기**

```fung run
codes = [200, 404, 200, 500, 200]
counts = fold(codes, {ok: 0, bad: 0}, fn(acc, c):
  key = c < 400 then "ok" else "bad"
  put(acc, key, acc[key] + 1)
;;)
print(counts)
```

**기간 더하기: sum은 0(정수)에서 시작하므로 fold로**

```fung run
durations = [1500ms, 2s, 250ms]
print(fold(durations, 0s, (acc, d) => acc + d))
```

**리스트를 쌓기**

```fung run
evens = fold(1..=6, [], (acc, x) => x % 2 == 0 then acc + [x * x] else acc)
print(evens)
```

#### 같이 보기

[`sum`](#fn-sum), [`map`](#fn-map), [`group_by`](#fn-group_by), [`until`](loops.html#fn-until), [for](../language.html#for)

:::

:::fn each(xs, f)

각 원소에 `f`를 부르고 결과는 버린다. 출력이나 전송처럼 부수효과만 필요할 때 쓴다.

#### 설명

`map`과 같은 순서로 `f`를 부르지만 결과를 모으지 않고 `nil`을 돌려준다. `for` 문장도 같은 일을 하지만 `for`는 식이라 결과 리스트를 만든다. 값을 모을 필요가 없는 일(찍기, 채널에 보내기, 파일에 쓰기)에 `each`를 쓴다.

`f`의 결과를 버리므로, `f`가 `Err`를 돌려주면 그 자리에서 E0606(버려지는 `Err`)으로 멈춘다. 실패를 조용히 넘기지 않기 위해서다. 실패해도 상관없으면 블록 함수 안에서 일부러 버린다: `fn(x): _ = 식 ;;`(람다 `x => …`의 몸에는 대입을 쓸 수 없다)([버려지는 `Err`](../language.html#results)).

#### 매개변수

- `xs`: 리스트, 범위, 받는 끝.
- `f`: 원소 하나를 받는 함수.

#### 돌려주는 값

`nil`.

#### 에러

`f`가 `Err`를 돌려주면 런타임 에러 E0606. 나머지는 `map`과 같다.

#### 예제

**찍기**

```fung run
each(["web-1", "web-2"], h => print("deploy", h))
```

**채널에 보내기**

```fung run
(tx, rx) = channel(3)
each(1..=3, i => send(tx, i * 100))
close(tx)
print(map(rx, x => x))
```

`send`는 `Ok(nil)`을 돌려주고, 닫힌 채널이면 `Err`라서 E0606으로 멈춘다.

**버려지는 Err는 멈춘다**

```fung run exit=1 error=E0606
each(["8080", "http"], s => parse_int(s))
```

#### 같이 보기

[`map`](#fn-map), [for](../language.html#for), [E0606](../errors.html#E0606)

:::

:::fn find(xs, f)

`f(x)`가 `true`인 첫 원소를, 없으면 `nil`을 돌려준다.

#### 설명

앞에서부터 보다가 처음 맞는 원소에서 멈춘다. 그 뒤의 원소에는 `f`를 부르지 않으므로 아주 큰 범위나 끝없는 채널에도 쓸 수 있다. `f`는 `bool`을 돌려줘야 한다.

찾은 원소가 아니라 "있는가"만 알면 되면 [`any`](#fn-any)를, 맞는 것 전부가 필요하면 [`filter`](#fn-filter)를 쓴다.

#### 매개변수

- `xs`: 리스트, 범위, 받는 끝.
- `f`: 원소를 받아 `bool`을 돌려주는 함수.

#### 돌려주는 값

원소 또는 `nil`.

#### 에러

`f`가 `bool`이 아닌 값을 돌려주면 런타임 에러 E0505.

#### 예제

**첫 번째 맞는 것**

```fung run
print(find([1, 5, 10], x => x > 3), find([1], x => x > 3))
print(find(0..1_000_000_000, x => x * x > 50))
```

**이름으로 찾기**

```fung run
servers = [{name: "web-1", ip: "10.0.0.1"}, {name: "db-1", ip: "10.0.0.9"}]
db = find(servers, s => s.name == "db-1")
print(db == nil then "not found" else db.ip)
```

#### 같이 보기

[`any`](#fn-any), [`filter`](#fn-filter), [`first`](#fn-first)

:::

:::fn any(xs, f)

`f(x)`가 `true`인 원소가 하나라도 있으면 `true`.

#### 설명

처음 `true`가 나오면 거기서 멈추고 나머지는 보지 않는다. 빈 컬렉션은 `false`다. `f`는 `bool`을 돌려줘야 한다.

#### 매개변수

- `xs`: 리스트, 범위, 받는 끝.
- `f`: 원소를 받아 `bool`을 돌려주는 함수.

#### 돌려주는 값

`bool`.

#### 에러

`f`가 `bool`이 아닌 값을 돌려주면 런타임 에러 E0505.

#### 예제

**하나라도**

```fung run
print(any([1, 2], x => x > 1), any([], x => true))
```

**처음 맞는 곳에서 멈춘다**

```fung run
r = any([1, 2, 3, 4], fn(x):
  print("checking", x)
  x >= 2
;;)
print(r)
```

#### 같이 보기

[`all`](#fn-all), [`find`](#fn-find), [`contains`](#fn-contains)

:::

:::fn all(xs, f)

모든 원소에서 `f(x)`가 `true`면 `true`. 빈 컬렉션은 `true`.

#### 설명

처음 `false`가 나오면 멈춘다. 빈 컬렉션이 `true`인 것은 "어긋나는 원소가 하나도 없다"는 뜻이기 때문이다(수학의 관례와 같다). 빈 목록을 "실패"로 다루고 싶으면 `not is_empty(xs) and all(xs, f)`로 쓴다.

#### 매개변수

- `xs`: 리스트, 범위, 받는 끝.
- `f`: 원소를 받아 `bool`을 돌려주는 함수.

#### 돌려주는 값

`bool`.

#### 에러

`f`가 `bool`이 아닌 값을 돌려주면 런타임 에러 E0505.

#### 예제

**모두**

```fung run
print(all([2, 4], x => x % 2 == 0), all([2, 3], x => x % 2 == 0), all([], x => false))
```

**모든 검사가 통과했는가**

```fung run
checks = [{name: "disk", ok: true}, {name: "dns", ok: false}]
print(all(checks, c => c.ok) then "healthy" else "unhealthy")
print(checks |> filter(c => not c.ok) |> map(c => c.name))
```

#### 같이 보기

[`any`](#fn-any), [`filter`](#fn-filter)

:::

## 정렬과 모양 바꾸기 {#order}

:::fn sort(xs)

작은 것부터 정렬한 새 리스트를 돌려준다.

#### 설명

순서는 [비교와 동일성](../language.html#values) 표의 `<`를 따른다. 수는 크기 순이고 정수와 실수를 섞어도 된다. 문자열은 코드 포인트 순이라 대문자가 소문자보다 앞선다(`"B" < "a"`). 리스트와 튜플은 앞 원소부터 사전식으로 비교한다.

**안정 정렬**(stable sort)이라 같은 값끼리는 원래 순서를 지킨다. 맵, `bool`, 심볼, 태그처럼 `<`가 없는 값이 있거나, 서로 비교할 수 없는 타입(수와 문자열)이 섞이면 처음 실패한 비교에서 에러가 난다. 큰 것부터는 `sort(xs) |> reverse`, 다른 기준은 [`sort_by`](#fn-sort_by).

#### 매개변수

- `xs`: 리스트, 범위, 받는 끝.

#### 돌려주는 값

리스트.

#### 에러

- 비교할 수 없는 값이 섞이면 런타임 에러 E0501(`cannot compare str and int`).
- `xs`가 맵 등이면 E0506.

#### 예제

**수, 문자열, 리스트**

```fung run
print(sort([3, 1, 2]), sort([2.5, 1, 3]))
print(sort(["b", "a", "B"]), sort([[2, 1], [1, 9]]))
```

**큰 것부터**

```fung run
print([5, 3, 9] |> sort |> reverse)
```

**맵은 비교할 수 없다**

```fung run exit=1 error=E0501
print(sort([{cpu: 3}, {cpu: 1}]))
```

맵의 리스트는 `sort_by(xs, m => m.cpu)`로 기준을 준다.

#### 같이 보기

[`sort_by`](#fn-sort_by), [`reverse`](#fn-reverse), [`unique`](#fn-unique), [`min`](#fn-min)

:::

:::fn sort_by(xs, f)

`f(x)`를 키로 삼아 안정 정렬한 새 리스트를 돌려준다.

#### 설명

원소 대신 `f(x)`의 값을 비교한다. 원소는 바뀌지 않고 순서만 바뀐다. 키의 비교 규칙은 [`sort`](#fn-sort)와 같다. 안정 정렬이라 키가 같은 원소는 원래 순서를 지킨다.

- **내림차순**: 수 키면 `-`를 붙인다(`x => -x.cpu`). 문자열 키면 정렬한 뒤 [`reverse`](#fn-reverse)한다.
- **여러 기준**: 튜플이나 리스트를 키로 쓴다. `p => (p.team, p.name)`은 팀으로 먼저, 같은 팀 안에서는 이름으로 정렬한다.
- **대소문자 무시**: `sort_by(xs, lower)`.

#### 매개변수

- `xs`: 리스트, 범위, 받는 끝.
- `f`: 원소를 받아 키를 돌려주는 함수.

#### 돌려주는 값

리스트.

#### 에러

키끼리 비교할 수 없으면 런타임 에러 E0501. `f`가 함수가 아니면 E0403.

#### 예제

**길이 순, 내림차순**

```fung run
print(["bb", "a", "ccc"] |> sort_by(len))
print([3, 1, 2] |> sort_by(x => -x))
```

**필드로, 같은 키는 원래 순서**

```fung run
servers = [{n: "web-1", cpu: 70}, {n: "db-1", cpu: 15}, {n: "web-2", cpu: 70}]
print(servers |> sort_by(s => -s.cpu) |> map(s => s.n))
```

`web-1`과 `web-2`는 키가 같아 원래 순서대로다.

**여러 기준**

```fung run
people = [{team: "sre", name: "kim"}, {team: "dev", name: "lee"}, {team: "sre", name: "ahn"}]
print(people |> sort_by(p => (p.team, p.name)) |> map(p => "${p.team}/${p.name}"))
```

**대소문자 무시, 맵의 값으로**

```fung run
print(sort_by(["Banana", "apple", "cherry"], lower))
print(sort_by(pairs({web: 3, db: 1, cache: 2}), ((_, v)) => v))
```

#### 같이 보기

[`sort`](#fn-sort), [`reverse`](#fn-reverse), [`group_by`](#fn-group_by)

:::

:::fn reverse(xs) fung

순서를 거꾸로 뒤집은 새 리스트를 돌려준다.

#### 설명

리스트, 범위, 받는 끝(닫힐 때까지 받는다)을 받는다. 문자열은 받지 않으므로 글자를 뒤집으려면 `chars`와 `join`을 쓴다.

#### 매개변수

- `xs`: 리스트, 범위, 받는 끝.

#### 돌려주는 값

리스트.

#### 에러

다른 타입이면 런타임 에러 E0506.

#### 예제

**뒤집기**

```fung run
print(reverse([1, 2, 3]), reverse(0..3), reverse([]))
```

**문자열 뒤집기**

```fung run
print(chars("fung") |> reverse |> join)
```

#### 같이 보기

[`sort`](#fn-sort), [`last`](#fn-last)

:::

:::fn unique(xs)

처음 나온 순서를 지키며 중복을 뺀 새 리스트를 돌려준다.

#### 설명

같은지는 `==`로 본다. 그래서 `1`과 `1.0`은 같은 값이고(앞의 것이 남는다), 리스트나 맵도 내용이 같으면 중복이다. 대소문자는 구별한다(`"a"`와 `"A"`는 다르다). 정렬하지 않으므로 정렬된 결과가 필요하면 `unique` 뒤에 `sort`한다. 원소 수에 비례하는 시간이 든다.

#### 매개변수

- `xs`: 리스트, 범위, 받는 끝.

#### 돌려주는 값

리스트.

#### 에러

`xs`가 받지 않는 타입이면 런타임 에러 E0506.

#### 예제

**중복 빼기**

```fung run
print(unique([1, 2, 1, 3, 2]), unique([1, 1.0]), unique([[1], [1], [2]]))
```

**로그에 나온 호스트 목록**

```fung run
hits = ["web-2", "web-1", "web-2", "db-1", "web-1"]
print(unique(hits), unique(hits) |> sort)
```

#### 같이 보기

[`sort`](#fn-sort), [`group_by`](#fn-group_by)

:::

:::fn flatten(xs)

리스트의 리스트를 한 단계만 펼친다: `[[1, [2]], 3]` → `[1, [2], 3]`.

#### 설명

원소가 리스트면 그 원소들을 꺼내 잇고, 리스트가 아닌 원소는 그대로 둔다. 한 단계만 펼치므로 더 깊은 리스트는 남는다. 각 원소가 여러 개를 만드는 변환은 `map` 뒤에 `flatten`을 붙인다.

#### 매개변수

- `xs`: 리스트, 범위, 받는 끝.

#### 돌려주는 값

리스트.

#### 에러

`xs`가 받지 않는 타입이면 런타임 에러 E0506.

#### 예제

**한 단계만**

```fung run
print(flatten([[1, [2]], 3]), flatten([[], [1]]), flatten([[[1]]]))
```

**원소 하나가 여러 개가 될 때**

```fung run
groups = {web: ["web-1", "web-2"], db: ["db-1"]}
print(values(groups) |> flatten)
print(flatten(map([1, 2], x => [x, x * 10])))
```

#### 같이 보기

[`map`](#fn-map), [`values`](maps.html#fn-values)

:::

:::fn group_by(xs, f)

`f(x)`를 키로 원소를 묶은 맵을 돌려준다.

#### 설명

결과 맵의 키는 `f(x)`의 값이고, 값은 그 키를 가진 원소의 리스트다. 키의 순서와 각 리스트 안의 순서는 모두 처음 나온 순서다.

맵의 키는 언제나 문자열이므로 `f`는 문자열을 돌려줘야 한다. 수나 심볼로 묶으려면 `str`로 바꾼다: `group_by(xs, x => str(x % 2))`. 각 묶음의 개수가 필요하면 결과의 값에 `len`을 부른다.

#### 매개변수

- `xs`: 리스트, 범위, 받는 끝.
- `f`: 원소를 받아 문자열 키를 돌려주는 함수.

#### 돌려주는 값

맵(키 → 원소의 리스트).

#### 에러

`f`가 문자열이 아닌 값을 돌려주면 런타임 에러 E0514(`cannot use a value of type int as a map key`).

#### 예제

**글자 수로 묶기**

```fung run
print(group_by(["ab", "c", "de"], s => str(len(s))))
```

**환경별 서버 목록**

```fung run
servers = [{env: "prod", host: "a"}, {env: "dev", host: "b"}, {env: "prod", host: "c"}]
by_env = group_by(servers, s => s.env)
for (name, group) in pairs(by_env):
  print(name, map(group, s => s.host))
;;
```

**상태 코드별 개수**

```fung run
codes = [200, 404, 200, 500, 200]
counts = group_by(codes, str)
print(pairs(counts) |> map(((code, xs)) => "${code}: ${len(xs)}"))
```

**키는 문자열이어야 한다**

```fung run exit=1 error=E0514
print(group_by([1, 2, 3], x => x % 2))
```

#### 같이 보기

[`pairs`](maps.html#fn-pairs), [`unique`](#fn-unique), [`fold`](#fn-fold)

:::

:::fn zip(xs, ys) fung

두 컬렉션의 원소를 차례로 짝지은 튜플의 리스트를 돌려준다.

#### 설명

`zip([a, b], [x, y])`는 `[(a, x), (b, y)]`다. 길이가 다르면 짧은 쪽에 맞추고 남는 원소는 버린다. 튜플은 패턴으로 꺼낸다: `for (name, ip) in zip(names, ips):`. 번호를 붙일 때는 범위와 짝짓는다.

#### 매개변수

- `xs`, `ys`: 리스트, 범위, 받는 끝.

#### 돌려주는 값

튜플의 리스트.

#### 에러

받지 않는 타입이면 런타임 에러 E0506.

#### 예제

**짝짓기**

```fung run
print(zip([1, 2, 3], ["a", "b"]))
```

**두 목록을 나란히 돌기**

```fung run
names = ["web-1", "db-1"]
ips = ["10.0.0.1", "10.0.0.9"]
for (name, ip) in zip(names, ips):
  print(pad_right(name, 6), ip)
;;
```

**번호 붙이기**

```fung run
for (i, step) in zip(1..100, ["build", "test", "deploy"]):
  print("${i}. ${step}")
;;
```

#### 같이 보기

[`range`](#fn-range), [튜플](../language.html#values), [for](../language.html#for)

:::

## 수 모으기 {#numbers}

:::fn sum(xs) fung

수의 합을 돌려준다. 빈 리스트는 0.

#### 설명

`fold(xs, 0, (acc, x) => acc + x)`와 같다. 정수와 실수를 섞으면 실수가 된다. 0(정수)에서 시작하므로 기간(`dur`)의 리스트는 더할 수 없다. 기간은 `fold(ds, 0s, (acc, d) => acc + d)`로 더한다.

#### 매개변수

- `xs`: 수의 리스트, 범위, 받는 끝.

#### 돌려주는 값

수.

#### 에러

수가 아닌 원소가 있으면 런타임 에러 E0501(`cannot add int and str`).

#### 예제

**합계**

```fung run
print(sum([1, 2, 3]), sum([]), sum([1.5, 2]), sum(1..=100))
```

**필드의 합**

```fung run
files = [{name: "a.log", size: 120}, {name: "b.log", size: 900}]
print(files |> map(f => f.size) |> sum)
```

#### 같이 보기

[`fold`](#fn-fold), [`max`](#fn-max)

:::

:::fn min(xs) fung

가장 작은 원소를, 비었으면 `nil`을 돌려준다.

#### 설명

비교는 `<`로 한다([`sort`](#fn-sort)와 같은 규칙). 그래서 수뿐 아니라 문자열이나 리스트에도 쓸 수 있다. `nil`이 원소로 섞여 있으면 건너뛰지 않고 에러를 낸다. 필드 기준의 최솟값은 `sort_by(xs, f) |> first`로 원소째 얻는다.

#### 매개변수

- `xs`: 리스트, 범위, 받는 끝.

#### 돌려주는 값

원소 또는 `nil`.

#### 에러

비교할 수 없는 값이 섞이면 런타임 에러 E0501.

#### 예제

**최솟값**

```fung run
print(min([3, 1, 2]), min([]), min(["b", "a"]), min(5..10))
```

**가장 한가한 서버**

```fung run
servers = [{n: "web-1", cpu: 70}, {n: "web-2", cpu: 12}]
print(min(map(servers, s => s.cpu)), first(sort_by(servers, s => s.cpu)).n)
```

#### 같이 보기

[`max`](#fn-max), [`sort_by`](#fn-sort_by)

:::

:::fn max(xs) fung

가장 큰 원소를, 비었으면 `nil`을 돌려준다.

#### 설명

[`min`](#fn-min)의 반대다. 같은 규칙으로 비교하고, 빈 컬렉션은 `nil`이다.

#### 매개변수

- `xs`: 리스트, 범위, 받는 끝.

#### 돌려주는 값

원소 또는 `nil`.

#### 에러

비교할 수 없는 값이 섞이면 런타임 에러 E0501.

#### 예제

**최댓값**

```fung run
print(max([3, 1, 2]), max([1, 2.5]), max([]), max([[1, 2], [1, 3]]))
```

**빈 목록의 기본값**

```fung run
latencies = []
print("worst:", max(latencies) ?? 0)
```

#### 같이 보기

[`min`](#fn-min), [`sum`](#fn-sum)

:::
