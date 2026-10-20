---
description: 여러 값을 묶는 리스트·맵·튜플을 만들고 꺼내는 법, 값을 고치는 대신 새 값을 만드는 법, 맵의 키, 자주 쓰는 컬렉션 함수.
lede: 서버 목록, 설정 한 벌, 이름과 상태의 짝. 값 여러 개를 하나로 묶어 다루는 법.
---
# 리스트·맵·튜플

:::goals

- 컬렉션이 무엇이고, 리스트·맵·튜플을 언제 쓰는지
- 리스트를 만들고, 인덱스와 슬라이스로 꺼내고, 이어 붙이는 법
- 맵을 만들고 키로 읽는 법, 없는 키를 다루는 법
- 컬렉션을 고치는 대신 새 값을 만드는 함수(`put`, `merge`, `remove`)
- 튜플로 작은 묶음을 만들고 풀어 받는 법
- 정렬, 묶기, 세기 같은 자주 쓰는 함수

:::

## 값을 묶기 {#collections}

지금까지의 값은 숫자 하나, 문자열 하나였다. 실제로는 "서버 열 대의 이름", "데이터베이스 접속 설정(호스트, 포트, 사용자)"처럼 여러 값을 한 덩어리로 다뤄야 한다. 여러 값을 담는 값을 **컬렉션(collection)** 또는 자료 구조라고 한다. fung의 컬렉션은 셋이다.

<table>
  <thead><tr><th>종류</th><th>모양</th><th>이럴 때</th></tr></thead>
  <tbody>
    <tr><td><strong>리스트</strong></td><td><code>["web-1", "web-2"]</code></td><td>같은 종류의 값이 여러 개, 순서가 있다(서버 목록, 로그 줄)</td></tr>
    <tr><td><strong>맵</strong></td><td><code>{host: "db-1", port: 5432}</code></td><td>이름 붙은 항목들(설정 한 벌, JSON 객체)</td></tr>
    <tr><td><strong>튜플</strong></td><td><code>("web-1", 200)</code></td><td>개수가 정해진 작은 묶음(이름과 상태 코드의 짝)</td></tr>
  </tbody>
</table>

셋 다 3장의 다른 값처럼 **바뀌지 않는다**. 원소를 더하거나 고치는 연산은 모두 새 컬렉션을 만든다. 원소로는 어떤 값이든, 다른 컬렉션도 담을 수 있다.

## 리스트 {#lists}

**리스트(list)**는 값들을 순서대로 늘어놓은 것이다. 대괄호 안에 쉼표로 나눠 쓴다. 문자열처럼 `len`으로 개수를 세고, 0부터 시작하는 인덱스로 꺼내고, 슬라이스로 자른다.

```fung run
servers = ["web-1", "web-2", "db-1", "cache-1"]
print(len(servers), servers[0], servers[-1])
print(servers[1..3], servers[..2], servers[2..])
print(first(servers), last(servers), first([]))
```

없는 인덱스를 읽으면 런타임 에러다. `first`와 `last`는 빈 리스트에서 에러 대신 `nil`을 돌려준다.

```fung run exit=1 error=E0507
servers = ["web-1", "web-2"]
print(servers[5])
```

### 새 리스트 만들기 {#build-lists}

리스트에는 `servers[0] = "x"` 같은 고치기가 없다(문법 에러다). 원소를 더하거나 빼거나 바꾼 리스트가 필요하면 새로 만든다.

- 이어 붙이기: `xs + ys`, 원소 하나는 `xs + [x]`
- 바꾸기·고르기: 4장의 `map`, `filter`, `for`
- 앞뒤 자르기: `take(xs, n)`, `drop(xs, n)` (슬라이스와 달리 개수가 모자라도 에러가 아니다)

```fung run
servers = ["web-1", "web-2"]
more = servers + ["db-1"]
print(servers, more)
print(take(more, 2), drop(more, 1), take(more, 10))
print(more |> map(upper), more |> filter(s => starts_with(s, "web")))
```

`servers`는 그대로다. 그래서 같은 리스트를 여러 곳에 넘겨도 안전하다. 새 리스트를 만든다고 원소 전체를 복사하는 것은 아니다. 새 리스트는 원래 리스트와 구조를 나눠 쓰므로, `acc + [x]`로 하나씩 쌓아도 빠르다.

### 리스트 함수 {#list-functions}

자주 쓰는 것들이다. 모두 새 값을 돌려준다.

```fung run
nums = [340, 12, 7, 12, 98]
print(sort(nums), reverse(nums), unique(nums))
print(sum(nums), min(nums), max(nums), 12 in nums)
print(find(nums, n => n > 50), any(nums, n => n > 300), all(nums, n => n > 0))
print([[1, 2], [3]] |> flatten, range(5), range(5) |> map(i => i * i))
```

- `sort`는 작은 것부터, `reverse`는 순서를 뒤집는다. `unique`는 처음 나온 것만 남긴다.
- `find(xs, f)`는 `f`가 참인 첫 원소(없으면 `nil`), `any`는 하나라도, `all`은 모두 참인지 묻는다.
- `flatten`은 리스트의 리스트를 한 겹 편다. `range(n)`은 `0..n`이고, 범위도 리스트처럼 `map`할 수 있다.

## 맵 {#maps}

**맵(map)**은 **키(key)**와 **값(value)**의 짝들이다. 리스트가 "몇 번째"로 찾는다면 맵은 "이름"으로 찾는다. 중괄호 안에 `키: 값`을 쓴다.

```fung run
db = {host: "db-1", port: 5432, user: "app"}
print(db)
print(db.host, db["port"], len(db))
print("user" in db, "password" in db)
```

- 맵의 **키는 항상 문자열**이다. `{host: "db-1"}`은 `{"host": "db-1"}`을 줄여 쓴 것이다.
- 키가 이름 모양이 아니면(`-`나 공백이 들어감) 따옴표로 쓰고, 읽을 때도 `m["db-host"]`로 읽는다. `m.키`는 이름 모양의 키에만 쓴다.
- `m.k`와 `m["k"]`는 같다. `in`은 키가 있는지 묻는다.
- 맵은 넣은 순서를 지킨다. 찍거나 돌 때 그 순서로 나온다.
- 같은 키를 두 번 쓰면 컴파일 에러(E0113)다.

```fung run
headers = {"content-type": "application/json", "x-request-id": "abc123"}
print(headers["content-type"])
```

### 없는 키 {#missing-keys}

**없는 키를 읽으면 런타임 에러**다(E0508). `nil`을 돌려주는 언어가 많지만, 그러면 키 이름의 오타가 조용히 `nil`로 퍼진다. 없을 수도 있는 키는 `get(m, 키, 기본값)`으로 읽는다. 기본값을 주지 않으면 `nil`이다.

```fung run
db = {host: "db-1", port: 5432}
print(get(db, "user", "postgres"), get(db, "user"), get(db, "port", 0))
```

```fung run exit=1 error=E0508
db = {host: "db-1", port: 5432}
print(db.usr)
```

### 새 맵 만들기 {#build-maps}

맵도 고치지 않고 새로 만든다. `put(m, k, v)`는 키 하나를 넣거나 바꾼 맵, `merge(a, b)`는 `a`에 `b`를 덮은 맵, `remove(m, k)`는 키 하나를 뺀 맵이다.

```fung run
server = {name: "web-1", port: 8080}
moved = put(server, "port", 9090)
tagged = merge(server, {region: "kr", port: 80})
print(server)
print(moved)
print(tagged)
print(remove(tagged, "region"))
```

키가 변수에 들어 있으면 `put`을 쓴다. 맵 리터럴의 키 자리에 쓴 이름은 변수가 아니라 **글자 그대로의 키**다.

```fung run
key = "port"
print({key: 1})
print(put({}, key, 1))
```

값이 같은 이름의 변수일 때는 키만 써도 된다. `{host, port}`는 `{host: host, port: port}`다.

```fung run
host = "db-1"
port = 5432
print({host, port})
```

### 맵 돌기 {#map-iteration}

맵을 직접 `for`로 돌 수는 없다. `keys(m)`, `values(m)`으로 키나 값의 리스트를 얻거나, `pairs(m)`으로 `(키, 값)` 튜플의 리스트를 얻어서 돈다.

```fung run
ports = {web: 80, db: 5432, cache: 6379}
print(keys(ports), values(ports))
for (name, port) in pairs(ports):
  print(pad_right(name, 6), port)
;;
```

## 튜플 {#tuples}

**튜플(tuple)**은 개수가 정해진 값의 묶음이다. 괄호 안에 쉼표로 쓴다. "서버 이름과 응답 시간"처럼 늘 함께 다니는 두세 개의 값을 묶을 때 쓴다. 원소가 하나뿐인 튜플은 `(x,)`처럼 쉼표를 붙인다.

튜플은 인덱스로 꺼내지 않는다. 같은 모양으로 **풀어서 받는다**. 대입의 왼쪽에 괄호와 이름을 쓰면 원소가 차례로 이름에 들어간다. `for (a, b) in …`, 람다의 `((a, b)) => …`도 같다. 이렇게 모양을 맞춰 꺼내는 것을 **패턴(pattern)**이라고 하고, 8장에서 리스트와 맵에도 쓰는 법을 배운다.

```fung run
result = ("web-1", 200)
(host, code) = result
print(result, host, code, type_of(result))

checks = [("web-1", 340), ("db-1", 7), ("web-2", 12)]
for (name, ms) in checks:
  print(name, "${ms}ms")
;;
print(checks |> sort_by(((_, ms)) => ms))
```

`sort_by(xs, f)`는 `f`가 돌려준 값을 기준으로 정렬한다. 위에서는 튜플의 둘째 값(응답 시간)이 기준이다. 쓰지 않는 자리는 `_`로 비운다. 큰 것부터 정렬하려면 기준에 `-`를 붙인다.

`zip(xs, ys)`는 두 리스트를 짝지어 튜플의 리스트를 만든다.

```fung run
print(zip(["web-1", "web-2"], [200, 503]))
```

## 세고 묶기 {#count}

로그를 다루다 보면 "수준별로 몇 줄인가"를 자주 센다. 맵을 모은 값으로 하는 `fold`로 쓸 수 있다. 줄마다 그 수준의 지금 개수를 `get`으로 읽고(처음이면 0), 하나 더한 값을 `put`한 새 맵을 다음 단계로 넘긴다.

```fung run
log = ["INFO start", "ERROR disk full", "WARN slow", "ERROR timeout", "INFO done"]
fn level(line) = split(line, " ")[0]
counts = log |> fold({}, (acc, l) => put(acc, level(l), get(acc, level(l), 0) + 1))
print(counts)
```

같은 기준으로 원소를 묶기만 하려면 `group_by(xs, f)`가 있다. `f`의 결과(문자열이어야 한다)가 키, 그 키에 해당하는 원소의 리스트가 값인 맵을 돌려준다.

```fung run
servers = ["web-1", "db-1", "web-2", "cache-1"]
by_role = servers |> group_by(s => split(s, "-")[0])
print(by_role)
print(by_role.web)
```

## 짚고 넘어가기 {#notes}

:::box learn 왜 0부터 세나

대부분의 프로그래밍 언어는 첫 원소를 `0`번으로 센다. 인덱스를 "처음에서 몇 칸 떨어졌나"로 보기 때문이다. 첫 원소는 0칸 떨어져 있다. 이렇게 세면 `xs[a..b]`의 길이가 정확히 `b - a`이고, `xs[..n]`이 앞의 `n`개가 되어 계산이 깔끔하다.

:::

:::box mistake 없는 키 읽기 (E0508)

JSON이나 설정에서 온 맵의 키는 있을 수도 없을 수도 있다. `cfg.timeout`처럼 바로 읽으면 키가 없을 때 E0508로 멈춘다. 없을 수 있는 키는 `get(cfg, "timeout", 30s)`로 읽고, 반드시 있어야 하는 키만 `cfg.timeout`으로 읽는다. 그러면 코드만 보고도 어느 키가 필수인지 알 수 있다.

:::

:::box mistake 변수를 키로 쓰려다 글자 그대로 들어감

`{key: 1}`의 `key`는 변수 `key`의 값이 아니라 문자열 `"key"`다. 키 자리의 이름은 늘 글자 그대로의 문자열이다. 그래서 `{type: "deploy"}`처럼 예약어도 키로 쓸 수 있다. 변수에 든 키로 맵을 만들려면 `put({}, key, 1)`을 쓴다.

:::

:::box mistake 튜플을 인덱스로 읽기 (E0509)

`pair[0]`은 `cannot index a value of type tuple` 에러다. 튜플은 `(a, b) = pair`처럼 풀어서 받는다. 원소가 많아져 순서를 헷갈릴 정도면 튜플 대신 이름 붙은 키가 있는 맵을 쓴다.

:::

:::box mistake 앞에 붙이기

`[x] + acc`로 리스트의 **앞**에 하나씩 붙이면 매번 전체를 복사해서 원소가 많으면 느려진다. 뒤에 붙인 뒤(`acc + [x]`) 마지막에 `reverse`한다.

:::

:::box other 다른 언어는

fung의 리스트는 Python의 list나 JavaScript의 배열, 맵은 Python의 dict나 JavaScript의 객체, 튜플은 Python의 tuple과 비슷하다. 다른 점은 셋 다 고칠 수 없다는 것이다(`append`, `xs[0] = …`, `d["k"] = …`가 없다). Python의 dict와 달리 키는 문자열뿐이고, 없는 키를 읽으면 JavaScript처럼 `undefined`가 아니라 에러다. `jq '.items[0].name'`은 fung에서 `data.items[0].name`이다(JSON은 13장).

:::

## 해 보기 {#exercises}

:::exercise 포트 표 만들기

`["web-1:80", "db-1:5432", "cache-1:6379"]`처럼 `호스트:포트` 문자열의 리스트를 `{"web-1": 80, "db-1": 5432, "cache-1": 6379}` 맵으로 바꾸고, `db-1`의 포트를 찍어 보자. 포트는 정수여야 한다.

:::solution

`fold`로 빈 맵에서 시작해 하나씩 `put`한다. 키에 `-`가 있으므로 `ports["db-1"]`로 읽는다.

```fung run
specs = ["web-1:80", "db-1:5432", "cache-1:6379"]
fn add_spec(m, spec):
  parts = split(spec, ":")
  put(m, parts[0], int(parts[1]))
;;
ports = specs |> fold({}, add_spec)
print(ports)
print(ports["db-1"])
```

:::

:::

:::exercise 느린 서버 찾기

`[("web-1", 340), ("db-1", 7), ("web-2", 1200), ("cache-1", 95)]`은 서버 이름과 응답 시간(ms)의 리스트다. 100ms를 넘는 서버만 골라 느린 순서대로 이름만 찍어 보자. 결과는 `["web-2", "web-1"]`이다.

:::solution

```fung run
checks = [("web-1", 340), ("db-1", 7), ("web-2", 1200), ("cache-1", 95)]
slow = checks
  |> filter(((_, ms)) => ms > 100)
  |> sort_by(((_, ms)) => -ms)
  |> map(((name, _)) => name)
print(slow)
```

:::

:::

## 정리 {#summary}

- 리스트 `[…]`는 순서 있는 목록, 맵 `{…}`은 문자열 키와 값의 짝, 튜플 `(…)`은 개수가 정해진 묶음이다.
- 리스트는 `xs[i]`, `xs[a..b]`로 꺼내고, 맵은 `m.k`나 `m["k"]`로 읽는다. 없는 인덱스나 키는 에러다. 없을 수 있는 키는 `get(m, k, 기본값)`.
- 셋 다 바뀌지 않는다. `xs + [x]`, `put`, `merge`, `remove`, `map`, `filter`가 새 값을 만든다.
- 맵 리터럴의 키 자리 이름은 글자 그대로다. 변수에 든 키는 `put`으로.
- 튜플은 `(a, b) = t`처럼 풀어서 받는다. `pairs`, `zip`, `for (a, b) in …`이 튜플을 쓴다.
- 세기는 `fold`와 `put`/`get`, 묶기는 `group_by`, 정렬은 `sort`·`sort_by`.

레퍼런스: [리스트 함수](../reference/prelude/collections.html), [맵 함수](../reference/prelude/maps.html), [`get`](../reference/prelude/maps.html#fn-get), [`put`](../reference/prelude/maps.html#fn-put), [`group_by`](../reference/prelude/collections.html#fn-group_by), [값과 타입](../reference/language.html#values), [인덱스·슬라이스·필드](../reference/language.html#access), [E0508](../reference/errors.html#E0508). 새 리스트가 구조를 나눠 쓰는 방법은 [값과 메모리](../vm/10-values-and-memory.html)에서 다룬다.
