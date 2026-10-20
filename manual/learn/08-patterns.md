---
description: 값의 모양을 보고 나누는 match와 패턴, 구조 분해 대입, 태그로 나만의 값 종류 만들기.
lede: 프로그램은 늘 "이 값이 어떤 모양인가"를 묻는다. 빈 리스트인가, 맵에 이 키가 있는가, 성공인가 실패인가. 이 장은 그 질문을 한 번에 쓰는 도구, 패턴과 <code>match</code>를 배운다.
---
# 패턴과 match

:::goals
- 패턴이 무엇이고 왜 `if`를 여러 번 쓰는 것보다 나은지
- `match`로 리터럴·리스트·맵·튜플·태그를 나눠 처리하기
- 나머지를 받는 `...rest`, 조건을 덧붙이는 `when`, 여러 패턴 중 하나 `|`
- 대입의 왼쪽에 패턴을 쓰는 구조 분해 대입
- 태그(`Circle(3)` 같은 값)로 나만의 값 종류를 만들기
:::

## 모양으로 나누기 {#shape}

4장에서 `if`로 조건을 나눴다. 조건이 "값이 무엇과 같은가"일 때는 `if`가 금세 길어진다. HTTP 상태 코드를 글로 바꾸는 함수를 생각해 보자. `if code == 200: … else if code == 404: … else if …`처럼 `code ==`를 매번 다시 써야 한다.

**패턴**(pattern)은 "값이 이런 모양이면"을 적는 틀이다. `200`이라는 패턴은 값이 200일 때 맞는다. `[first, ...rest]`라는 패턴은 값이 원소가 하나 이상인 리스트일 때 맞고, 맞으면 첫 원소를 `first`에, 나머지를 `rest`에 담아 준다. 확인하는 일과 꺼내는 일을 한 번에 한다는 것이 패턴의 힘이다.

`match`는 값 하나를 패턴 여러 개에 위에서부터 차례로 대 보고, 처음 맞는 절(arm)을 실행한다. 절은 `패턴 => 본문` 꼴이다.

```fung run
fn status_text(code) = match code:
  200 => "OK"
  404 => "Not Found"
  500 => "Internal Server Error"
  _   => "status ${code}"
;;

print(status_text(200))
print(status_text(404))
print(status_text(418))
```

마지막 절의 `_`는 "무엇이든"을 뜻하는 패턴이다. 위의 패턴이 하나도 맞지 않을 때 받아 주는 자리로 흔히 끝에 둔다. `match`도 `if`처럼 식이라서 값을 돌려준다. 위 함수는 `match`의 값을 그대로 돌려준다.

:::box learn 절은 위에서부터 본다
`match`는 첫 번째로 맞는 절 하나만 실행한다. 그래서 좁은 패턴을 위에, 넓은 패턴을 아래에 둔다. `_`를 맨 위에 두면 그 아래 절은 영영 실행되지 않는다. 시험 문제의 채점표를 위에서부터 읽어 내려가다 처음 해당하는 줄에서 멈추는 것과 같다.
:::

아무 절도 맞지 않으면 프로그램은 E0601로 멈춘다. 처리하지 못한 경우를 조용히 지나치지 않게 하려는 것이다.

```fung run exit=1 error=E0601
fn level(code) = match code:
  200 => "ok"
  404 => "not found"
;;

print(level(200))
print(level(503))
```

## 리터럴 패턴 {#literals}

숫자, 문자열, 심볼, 기간, `true`·`false`·`nil`은 그 값 자체가 패턴이다. 음수도 된다. 같은 본문을 쓰는 여러 패턴은 `|`로 묶는다.

```fung run
fn describe(x) = match x:
  0           => "zero"
  -1          => "minus one"
  "yes" | "y" => "agreed"
  'ok         => "the symbol ok"
  true        => "true"
  nil         => "nothing"
  1s          => "one second"
  _           => "something else"
;;

print(describe(0), describe(-1), describe("y"), describe('ok))
print(describe(true), describe(nil), describe(1s), describe(3.5))
```

## 리스트 패턴 {#lists}

리스트 패턴은 원소 개수와 각 원소의 모양을 함께 본다. `[]`는 빈 리스트, `[x]`는 원소가 꼭 하나인 리스트, `[a, b]`는 꼭 둘인 리스트다. 길이가 정해지지 않은 리스트는 `...`로 나머지를 받는다. `[first, ...rest]`는 "하나 이상"이고, `rest`에는 첫 원소를 뺀 리스트가 담긴다. 나머지가 필요 없으면 이름 없이 `[first, ...]`라고 쓴다.

```fung run
fn summary(hosts) = match hosts:
  []                => "no hosts"
  [only]            => "one host: ${only}"
  [a, b]            => "two hosts: ${a}, ${b}"
  [head, ...others] => "${head} and ${len(others)} more"
;;

print(summary([]))
print(summary(["web1"]))
print(summary(["web1", "web2"]))
print(summary(["web1", "web2", "db1", "db2"]))
```

리스트 패턴의 원소 자리에도 리터럴을 쓸 수 있다. 명령줄 인자나 공백으로 쪼갠 로그 줄처럼 "첫 낱말이 무엇인가"로 나누는 일에 잘 맞는다.

```fung run
fn run_command(words) = match words:
  ["start", name]       => "starting ${name}"
  ["stop", name]        => "stopping ${name}"
  ["restart", ...names] => "restarting ${join(names, ", ")}"
  _                     => "unknown command: ${join(words, " ")}"
;;

print(run_command(split("start api", " ")))
print(run_command(split("restart api worker", " ")))
print(run_command(split("delete everything", " ")))
```

## 맵 패턴 {#maps}

맵 패턴 `{name: n}`은 "키 `name`이 있는 맵"에 맞고, 그 값을 `n`에 담는다. 패턴에 적지 않은 키는 있든 없든 상관하지 않는다. 키 이름과 변수 이름이 같으면 `{name}`처럼 줄여 쓴다(`{name: name}`과 같다). 값 자리에 리터럴을 두면 그 값일 때만 맞는다.

```fung run
events = [
  {kind: "deploy", service: "api", version: "1.4.2"},
  {kind: "alert", level: 3, text: "disk 95%"},
  {kind: "alert", level: 1, text: "slow query"},
  {kind: "login", user: "kim"},
]

for e in events:
  match e:
    {kind: "deploy", service, version} => print("deployed ${service} ${version}")
    {kind: "alert", level: 3, text}    => print("PAGE: ${text}")
    {kind: "alert", text}              => print("note: ${text}")
    {kind}                             => print("ignored ${kind}")
  ;;
;;
```

패턴은 겹쳐 쓸 수 있다. 맵 안의 리스트, 리스트 안의 맵처럼 데이터가 생긴 그대로 패턴을 쓰면 된다. JSON에서 읽은 데이터(13장)를 다룰 때 특히 쓸모 있다.

```fung run
fn first_port(svc) = match svc:
  {name, ports: [p, ...]} => "${name} listens on ${p}"
  {name, ports: []}       => "${name} has no ports"
;;

print(first_port({name: "api", ports: [8080, 8443]}))
print(first_port({name: "batch", ports: []}))
```

## 튜플 패턴 {#tuples}

7장의 튜플은 개수가 정해진 값 묶음이다. 튜플의 원소는 인덱스로 꺼내지 않고 패턴으로만 꺼낸다. 값 두 개를 함께 보고 나눌 때 튜플로 묶어 `match`하는 모양이 흔하다.

```fung run
fn route(method, path) = match (method, path):
  ("GET", "/health") => "health check"
  ("GET", p)         => "read ${p}"
  ("POST", p)        => "write ${p}"
  (m, _)             => "method ${m} not allowed"
;;

print(route("GET", "/health"))
print(route("GET", "/users"))
print(route("DELETE", "/users"))
```

## 조건 붙이기: when {#guards}

패턴만으로는 "1024보다 크다" 같은 조건을 쓸 수 없다. 이때 패턴 뒤에 `when 조건`을 붙인다. 이것을 **가드**(guard)라고 한다. 패턴이 맞고 가드도 참일 때만 그 절이 실행된다.

```fung run
fn classify(svc) = match svc:
  {port} when port < 1024  => "${svc.name}: system port"
  {port} when port > 49151 => "${svc.name}: dynamic port"
  {port}                   => "${svc.name}: registered port ${port}"
;;

print(classify({name: "ssh", port: 22}))
print(classify({name: "api", port: 8080}))
print(classify({name: "tmp", port: 60000}))
```

:::box mistake 패턴의 이름은 늘 새 변수다
패턴에 쓴 이름은 바깥 변수와 비교하지 않는다. 언제나 **새 이름을 묶는다**. 아래에서 `wanted` 절은 "값이 `wanted`와 같으면"이 아니라 "무엇이든 받아서 `wanted`라고 부른다"는 뜻이라 늘 맞는다. 에러도 나지 않으니 찾기 어려운 버그가 된다.

```fung run
wanted = "db"

fn pick(name) = match name:
  wanted => "matched ${wanted}"
;;

print(pick("web"))
```

바깥 값과 비교하려면 가드를 쓴다: `n when n == wanted => …`.
:::

## 구조 분해 대입 {#destructuring}

패턴은 `match`에만 쓰는 것이 아니다. 대입 `=`의 왼쪽에도 패턴을 쓸 수 있다. 이것을 **구조 분해 대입**(destructuring assignment)이라고 한다. 값의 각 부분을 한 줄에 여러 이름으로 꺼낸다.

```fung run
(quotient, remainder) = (17 // 5, 17 % 5)
[host, port] = split("db.internal:5432", ":")
{name, replicas: n} = {name: "api", replicas: 3, image: "api:1.4"}
[top, ...rest] = ["a.log", "b.log", "c.log"]

print(quotient, remainder)
print(host, int(port))
print(name, n)
print(top, rest)
```

`match`와 달리 대입은 "다른 경우"가 없다. 값이 패턴에 맞지 않으면 E0602로 멈춘다. 그래서 모양을 확신할 때만 대입에 패턴을 쓰고, 모양이 여럿일 수 있으면 `match`를 쓴다.

```fung run exit=1 error=E0602
[host, port] = split("db.internal", ":")
print(host, port)
```

함수의 매개변수 자리와 `for`의 변수 자리에도 같은 패턴을 쓴다. 맵을 `pairs`로 `(키, 값)` 튜플의 리스트로 바꿔 돌 때 자주 보는 모양이다.

```fung run
fn distance((x1, y1), (x2, y2)) = (x2 - x1) ** 2 + (y2 - y1) ** 2

print(distance((0, 0), (3, 4)))

limits = {cpu: "500m", memory: "256Mi"}
for (key, value) in pairs(limits):
  print("${key}=${value}")
;;
```

## 태그: 나만의 값 종류 {#tags}

대문자로 시작하는 이름에 괄호를 붙이면 **태그**(tag) 값이 된다. `Circle(3)`, `Rect(2, 5)`, `Ok(42)`가 모두 태그다. 미리 선언할 필요가 없다. 쓰는 순간 그런 값이 생긴다. 태그는 "이름표가 붙은 값 묶음"이고, 이름표가 곧 그 값의 종류다. `type_of`는 태그 값의 종류로 그 이름을 돌려준다.

태그는 `match`와 짝을 이룬다. 패턴 `Circle(r)`은 `Circle` 태그일 때만 맞고 안의 값을 `r`에 담는다.

```fung run
fn area(shape) = match shape:
  Circle(r)  => 3.14 * r * r
  Rect(w, h) => w * h
  Square(s)  => s * s
;;

shapes = [Circle(1), Rect(2, 5), Square(3)]
print(shapes)
print(shapes |> map(area))
print(type_of(Rect(2, 5)), Circle(1) == Circle(1))
```

괄호 없이 쓴 태그 `Red`는 심볼 `'Red`와 같은 값이다. 몇 가지 중 하나를 고르는 값(열거형, enumeration)에 쓴다. 9장의 `Ok`와 `Err`도 특별한 문법이 아니라 이런 태그다.

```fung run
fn hex(color) = match color:
  Red   => "#ff0000"
  Green => "#00ff00"
  Blue  => "#0000ff"
;;

print(hex(Red), hex(Blue))
print(Red == 'Red, type_of(Red))
```

:::box tip 함수에 받을 종류를 적어 두기
`type`으로 종류의 목록에 이름을 붙이고 매개변수에 적을 수 있다: `type color = Red | Green | Blue`, `fn hex(c: color) = …`. 다른 값을 넘기면 함수에 들어가기 전에 E0403으로 멈춘다. 타입 표기는 선택이고 런타임에 검사한다. 규칙은 [타입 표기](../reference/language.html#types)에 있다.
:::

:::box other 다른 언어는
태그와 `match`는 Rust의 `enum`과 `match`, OCaml·Haskell의 대수적 데이터 타입과 같은 생각이다. 다만 fung은 태그를 미리 선언하지 않는다. Python 3.10의 `match`와도 모양이 비슷하다. 쉘의 `case "$x" in start) … ;; esac`는 문자열 리터럴만 나눌 수 있지만, 패턴은 리스트·맵의 안쪽까지 본다.
:::

:::exercise 로그 줄 나누기
`"ERROR disk full"`, `"WARN slow"`, `"INFO started"` 같은 로그 줄을 받아, 수준(`ERROR`·`WARN`·그 밖)에 따라 다른 문자열을 돌려주는 함수 `triage(line)`를 써 보자. `ERROR`면 `"page: <나머지>"`, `WARN`이면 `"ticket: <나머지>"`, 그 밖이면 `"skip"`이다. 빈 줄도 `"skip"`이다. 힌트: `split(line, " ")`의 결과를 리스트 패턴으로 나눈다.

:::solution

```fung run
fn triage(line) = match split(line, " "):
  ["ERROR", ...words] => "page: ${join(words, " ")}"
  ["WARN", ...words]  => "ticket: ${join(words, " ")}"
  _                   => "skip"
;;

for line in ["ERROR disk full", "WARN slow", "INFO started", ""]:
  print(triage(line))
;;
```

:::
:::

:::exercise 설정 검사
`{name: "api", replicas: 3}` 같은 맵을 받아 `replicas`가 0이면 `"api: stopped"`, 1이면 `"api: single"`, 2 이상이면 `"api: 3 replicas"`처럼 돌려주는 함수를 맵 패턴과 가드로 써 보자.

:::solution

```fung run
fn replicas_text(cfg) = match cfg:
  {name, replicas: 0}                 => "${name}: stopped"
  {name, replicas: 1}                 => "${name}: single"
  {name, replicas} when replicas >= 2 => "${name}: ${replicas} replicas"
;;

print(replicas_text({name: "api", replicas: 3}))
print(replicas_text({name: "cron", replicas: 1}))
print(replicas_text({name: "old", replicas: 0}))
```

:::
:::

## 정리 {#summary}

- 패턴은 값의 모양을 확인하고 동시에 안의 값을 꺼낸다.
- `match`는 절을 위에서부터 대 보고 처음 맞는 절을 실행한다. 아무 것도 맞지 않으면 E0601이다. `_`는 무엇이든 맞는다.
- 리터럴, 리스트(`[first, ...rest]`), 맵(`{name, port: p}`, 적지 않은 키는 무시), 튜플, 태그(`Circle(r)`) 패턴이 있고 서로 겹쳐 쓸 수 있다.
- `when`으로 조건을 붙이고, `|`로 여러 패턴을 묶는다. 패턴의 이름은 늘 새 변수다.
- 대입·매개변수·`for`의 왼쪽에도 패턴을 쓴다. 맞지 않으면 E0602다.
- 대문자 이름은 태그다. 선언 없이 만들고 `match`로 나눈다.

정확한 규칙은 레퍼런스의 [match와 패턴](../reference/language.html#match)과 [타입 표기](../reference/language.html#types)에 있다. 에러는 [E0601](../reference/errors.html#E0601), [E0602](../reference/errors.html#E0602)를 보라. 다음 장에서는 태그 `Ok`·`Err`로 실패를 다루는 법을 배운다.
