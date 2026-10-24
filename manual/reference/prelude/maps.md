---
lede: 맵은 이름(키)으로 값을 찾는 묶음이다. 맵은 불변이라 아래 함수들은 원본을 바꾸지 않고 모두 새 맵을 돌려준다.
description: fung prelude 맵: get, put, merge, remove, keys, values, pairs.
---
# 맵

## 맵이란 {#concepts}

**맵**(map)은 **키**(key)와 **값**(value)의 짝을 모은 값이다. 리스트가 원소를 0, 1, 2 같은 자리 번호로 찾는다면, 맵은 `"host"`, `"port"` 같은 이름으로 찾는다. 설정, JSON 객체, 서버 한 대의 정보처럼 "이름 붙은 칸들"을 담기에 알맞다. 다른 언어에서는 사전(dictionary), 해시(hash), 객체(object)라고도 부른다.

```fung run
server = {host: "web-1", port: 22, tags: ["prod"]}
print(server.host, server["port"], len(server))
```

fung의 맵에는 규칙이 셋 있다([값과 타입](../language.html#values)).

- **키는 언제나 문자열이다.** `{host: "a"}`는 `{"host": "a"}`의 축약이다. 수나 심볼은 키가 될 수 없으므로 `str`로 바꿔 쓴다. 값은 아무 타입이나 된다.
- **넣은 순서를 지킨다.** 출력, `keys`, `pairs`, `for`가 모두 넣은 순서대로 나온다. `==`는 순서를 보지 않는다(키와 값이 같으면 같다).
- **없는 키를 읽으면 런타임 에러다**(E0508). `m.port`는 "반드시 있다"는 뜻이다. 없을 수 있으면 [`get`](#fn-get)으로 기본값을 준다.

맵도 리스트처럼 **불변**이다. `put(m, "port", 8080)`은 `m`을 바꾸지 않고 `port`가 바뀐 새 맵을 돌려준다. 그래서 바뀐 맵을 쓰려면 새 이름에 담는다. 사본은 원본과 구조를 나눠 쓰므로 큰 맵에 하나를 바꿔도 전체를 복사하지 않는다.

모든 맵 함수가 맵을 첫 인자로 받으므로 파이프로 잇는다: `cfg |> put("port", 8080) |> remove("debug")`.

## 함수 {#functions}

:::fn get(m, k, default = nil)

키 `k`가 있으면 그 값을, 없으면 `default`를 돌려준다.

#### 설명

`m[k]`나 `m.k`는 키가 없으면 런타임 에러지만 `get`은 기본값을 준다. 설정 파일이나 JSON처럼 칸이 빠져 있을 수 있는 데이터를 읽을 때 쓴다. 기본값을 주지 않으면 `nil`이다.

키가 있고 값이 `nil`이면 기본값이 아니라 `nil`을 돌려준다(키가 **있는가**를 보기 때문이다). 키가 있는지만 알고 싶으면 `k in m`을 쓴다.

#### 매개변수

- `m`: 맵.
- `k`: 문자열 키.
- `default`: 키가 없을 때 돌려줄 값. 기본은 `nil`.

#### 돌려주는 값

값 또는 `default`.

#### 에러

- `m`이 맵이 아니면 E0506(`cannot get from list`). 리스트는 인덱스로 읽는다.
- `k`가 문자열이 아니면 E0501.

#### 예제

**기본값**

```fung run
cfg = {host: "localhost"}
print(get(cfg, "host", "0.0.0.0"), get(cfg, "port", 8080), get(cfg, "user"))
```

**값이 nil인 키**

```fung run
print(get({a: nil}, "a", 5), "a" in {a: nil})
```

**없는 키를 바로 읽으면 에러**

```fung run exit=1 error=E0508
cfg = {host: "localhost"}
print(cfg.port)
```

#### 같이 보기

[`contains`](collections.html#fn-contains), [`??`](../language.html#results), [인덱스·필드](../language.html#access)

:::

:::fn put(m, k, v)

키 `k`를 값 `v`로 둔 새 맵을 돌려준다.

#### 설명

없는 키면 맨 뒤에 더하고, 있는 키면 값만 바꾸며 자리(순서)는 지킨다. 원본 `m`은 그대로다. 여러 개를 한 번에 바꾸려면 [`merge`](#fn-merge)를 쓴다.

반복하며 맵을 쌓을 때는 `fold`의 누적값으로 `put`한다(바깥 변수를 다시 대입할 수 없기 때문이다).

#### 매개변수

- `m`: 맵.
- `k`: 문자열 키.
- `v`: 아무 값.

#### 돌려주는 값

새 맵.

#### 에러

`k`가 문자열이 아니면 런타임 에러 E0501(`cannot index map and sym`). `m`이 맵이 아니면 E0506(`cannot put into list`).

#### 예제

**더하기와 바꾸기**

```fung run
m = {a: 1, b: 2}
print(put(m, "c", 3), put(m, "a", 9))
print(m)
```

`a`를 바꾼 맵에서도 `a`가 맨 앞에 남고, 원본 `m`은 그대로다.

**fold로 맵 쌓기**

```fung run
hosts = ["web-1:80", "db-1:5432"]
ports = fold(hosts, {}, fn(acc, h):
  [name, port] = split(h, ":")
  put(acc, name, int(port))
;;)
print(ports)
```

**키는 문자열**

```fung run exit=1 error=E0501
print(put({}, 'retries, 3))
```

#### 같이 보기

[`merge`](#fn-merge), [`remove`](#fn-remove), [`fold`](collections.html#fn-fold)

:::

:::fn merge(a, b)

두 맵을 합친 새 맵을 돌려준다. 같은 키는 `b`의 값이 이긴다.

#### 설명

`a`의 키가 순서대로 먼저 오고, `b`에만 있는 키가 그 뒤에 온다. 둘 다 있는 키는 `a`의 자리에 `b`의 값이 들어간다. 기본 설정 위에 사용자 설정을 덮어쓰는 모양에 알맞다: `merge(defaults, user)`.

한 단계만 합친다. 값이 맵이어도 안으로 들어가 합치지 않고 `b`의 맵이 통째로 이긴다.

#### 매개변수

- `a`: 맵.
- `b`: 맵. 같은 키에서 이긴다.

#### 돌려주는 값

새 맵.

#### 에러

둘 중 하나가 맵이 아니면 런타임 에러 E0506(`cannot merge list`).

#### 예제

**기본값 위에 덮어쓰기**

```fung run
defaults = {host: "0.0.0.0", port: 8080, debug: false}
user = {port: 9090, debug: true}
print(merge(defaults, user))
```

**한 단계만 합친다**

```fung run
print(merge({db: {host: "a", port: 1}}, {db: {port: 2}}))
```

안쪽 맵까지 합치려면 그 칸을 따로 `merge`한다.

#### 같이 보기

[`put`](#fn-put), [`remove`](#fn-remove)

:::

:::fn remove(m, k)

키 `k`를 뺀 새 맵을 돌려준다. 없는 키면 그대로 돌려준다.

#### 설명

남은 키의 순서는 지킨다. 없는 키를 빼도 에러가 아니다. 뺀 키를 다시 `put`하면 맨 뒤에 들어간다.

#### 매개변수

- `m`: 맵.
- `k`: 문자열 키.

#### 돌려주는 값

새 맵.

#### 에러

`k`가 문자열이 아니면 런타임 에러 E0501.

#### 예제

**빼기**

```fung run
print(remove({a: 1, b: 2}, "a"), remove({a: 1}, "z"))
```

**비밀 값을 빼고 찍기**

```fung run
cfg = {user: "deploy", password: "hunter2", host: "db-1"}
print(cfg |> remove("password"))
```

#### 같이 보기

[`put`](#fn-put), [`merge`](#fn-merge)

:::

:::fn keys(m) fung

키의 리스트를 넣은 순서대로 돌려준다.

#### 설명

키는 언제나 문자열이므로 문자열의 리스트다. 알파벳 순서가 필요하면 `sort`한다.

#### 매개변수

- `m`: 맵.

#### 돌려주는 값

문자열의 리스트.

#### 에러

`m`이 맵이 아니면 런타임 에러 E0506.

#### 예제

**넣은 순서**

```fung run
m = {web: 3, db: 1, cache: 2}
print(keys(m), keys(m) |> sort, keys({}))
```

**필수 키가 빠졌는지**

```fung run
cfg = {host: "db-1"}
missing = ["host", "port", "user"] |> filter(k => k not in cfg)
print(missing)
```

#### 같이 보기

[`values`](#fn-values), [`pairs`](#fn-pairs)

:::

:::fn values(m) fung

값의 리스트를 키의 순서대로 돌려준다.

#### 설명

`keys(m)`과 같은 순서이므로 `zip(keys(m), values(m))`은 `pairs(m)`과 같다.

#### 매개변수

- `m`: 맵.

#### 돌려주는 값

리스트.

#### 에러

`m`이 맵이 아니면 런타임 에러 E0506.

#### 예제

**값만**

```fung run
usage = {"/": 71, "/var": 93, "/home": 40}
print(values(usage), max(values(usage)))
```

#### 같이 보기

[`keys`](#fn-keys), [`pairs`](#fn-pairs)

:::

:::fn pairs(m)

`(키, 값)` 튜플의 리스트를 넣은 순서대로 돌려준다.

#### 설명

맵을 도는 표준 방법이다. 맵을 그대로 `for`나 `map`에 넣을 수는 없고(E0506), `pairs`로 리스트를 만들어 돈다: `for (k, v) in pairs(m):`. 튜플은 패턴으로 꺼낸다. 람다의 인자에서 꺼낼 때는 괄호를 한 겹 더 쓴다: `map(pairs(m), ((k, v)) => …)`.

리스트 함수(`filter`, `sort_by` 등)를 쓴 뒤 다시 맵이 필요하면 `fold`와 `put`으로 만든다.

#### 매개변수

- `m`: 맵.

#### 돌려주는 값

튜플의 리스트.

#### 에러

`m`이 맵이 아니면 런타임 에러 E0506(`cannot list the pairs of list`).

#### 예제

**for로 돌기**

```fung run
for (k, v) in pairs({web: 2, db: 1}):
  print("${k}=${v}")
;;
```

**값으로 걸러서 다시 맵으로**

```fung run
usage = {"/": 71, "/var": 93, "/home": 40}
full = pairs(usage) |> filter(((_, pct)) => pct > 70)
print(full)
print(fold(full, {}, (acc, (k, v)) => put(acc, k, v)))
```

**맵은 바로 돌 수 없다**

```fung run exit=1 error=E0506
for k in {a: 1}:
  print(k)
;;
```

#### 같이 보기

[`keys`](#fn-keys), [`values`](#fn-values), [`sort_by`](collections.html#fn-sort_by), [for](../language.html#for)

:::
