---
description: 실패를 값(Ok·Err)으로 다루는 법: match, ?, !, ??, 버려진 Err(E0606), 런타임 에러와 fail.
lede: 파일이 없거나, 숫자 자리에 글자가 오거나, 서버가 대답하지 않는다. 실패는 늘 일어난다. 이 장은 fung이 실패를 어떻게 다루는지 배운다.
---
# 에러는 값이다

:::goals
- 실패할 수 있는 일을 값으로 표현하는 법: `Ok`와 `Err`
- `match`로 성공과 실패를 나눠 처리하고, 실패의 종류(`kind`)로 다시 나누기
- `?`로 실패를 부른 쪽에 넘기기, `!`로 스크립트를 멈추기, `??`로 기본값 쓰기
- 실패를 그냥 버리면 왜 에러가 나는가(E0606)
- 결과값과 런타임 에러의 차이, `fail`을 쓰는 자리
:::

## 실패는 늘 일어난다 {#why}

프로그래밍 언어는 "실패"를 다루는 방법을 저마다 정해 둔다. 많은 언어는 **예외**(exception)를 쓴다. 실패하면 예외를 던지고, 어딘가에서 잡지 않으면 프로그램이 멈춘다. 편하지만, 코드만 봐서는 어느 줄이 실패할 수 있는지 알기 어렵다.

fung은 다른 길을 택했다. 실패할 수 있는 함수는 **결과값**(result)을 돌려준다. 성공하면 `Ok(값)`, 실패하면 `Err(이유)`다. 문자열을 정수로 바꾸는 `parse_int`로 보자.

```fung run
print(parse_int("42"))
print(parse_int("마흔둘"))
```

:::box learn "값"이라는 말
숫자 `42`나 문자열 `"hi"`처럼, 변수에 담고 함수에 넘기고 돌려받을 수 있는 것을 **값**이라고 한다(3장). "에러가 값이다"는 에러도 변수에 담고, 함수에 넘기고, 리스트에 모을 수 있다는 뜻이다. 던지고 잡는 특별한 무엇이 아니다.
:::

`Ok`와 `Err`는 8장에서 본 태그다. 표준 라이브러리가 돌려주는 `Err` 안에는 맵이 들어 있다. `kind`는 실패의 종류를 나타내는 심볼이고, `message`는 사람이 읽을 설명이다. 파일 읽기가 실패하면 이렇게 생겼다.

```fung run
import fs

print(fs.read("missing.txt"))
```

`Err`는 성공한 값이 아니므로 그대로 쓸 수 없다. `parse_int("x").message`라고 쓰면 맵이 아니라 `Err` 태그에서 키를 찾으므로 런타임 에러 E0509(`cannot index a value of type Err`)가 난다. 안의 값을 꺼내려면 아래의 방법 중 하나를 쓴다.

## match로 꺼내기 {#match}

결과값에서 안의 값을 꺼내는 가장 기본적인 방법은 8장의 `match`다. `Ok(n)`과 `Err(e)` 두 경우를 모두 적으므로, 실패를 잊고 지나갈 수 없다.

```fung run
fn double_text(input) = match parse_int(input):
  Ok(n)  => "두 배: ${n * 2}"
  Err(e) => "숫자가 아니에요: ${e.message}"
;;

print(double_text("21"))
print(double_text("abc"))
```

실패의 종류에 따라 하는 일이 다르면 `kind`까지 패턴에 쓴다. 설정 파일이 없을 때만 기본 설정을 쓰고, 권한 문제 같은 다른 실패는 멈추는 예다.

```fung run
import fs

fn load_config(p) = match fs.read(p):
  Ok(text)                => text
  Err({kind: 'not_found}) => "{}"
  Err(e)                  => fail(e.message)
;;

print(load_config("app.json"))
```

## ?로 위에 넘기기 {#question}

실패할 수 있는 일을 여러 번 이어서 할 때, 매번 `match`를 쓰면 길어진다. 대부분은 "실패하면 나도 그 실패를 돌려준다"면 충분하다. 이때 값 뒤에 `?`를 붙인다. `Ok(v)`면 `v`를 꺼내고, `Err`면 지금 함수를 그 자리에서 끝내며 그 `Err`를 돌려준다.

```fung run
fn add_texts(a, b):
  x = parse_int(a)?
  y = parse_int(b)?
  Ok(x + y)
;;

print(add_texts("1", "2"))
print(add_texts("1", "둘"))
```

`add_texts`는 성공하면 `Ok(합)`, 실패하면 처음 실패한 `parse_int`의 `Err`를 돌려준다. 이런 함수를 부른 쪽도 다시 `match`나 `?`로 결과를 다룬다. 실패는 이렇게 호출을 거슬러 올라가며, 그것을 처리할 줄 아는 곳에서 처리된다.

`?`는 "함수에서 돌아간다"는 뜻이므로 함수 안에서만 쓴다. 스크립트 최상위에 쓰면 컴파일 에러 E0214이고, 프로그램은 한 줄도 실행되지 않은 채 종료 코드 2로 끝난다. 최상위에서는 다음 절의 `!`를 쓴다.

```fung run exit=2 error=E0214
n = parse_int("x")?
print(n)
```

:::box mistake map의 람다 안에서 ?
`xs |> map(x => parse_int(x)?)`는 컴파일 에러 E0226이다. `?`는 람다에서 돌아갈 뿐 바깥 함수에서 돌아가지 않으므로, 실패가 그냥 리스트의 원소가 되어 버리기 때문이다. 리스트의 원소를 하나씩 바꾸다 처음 실패에서 멈추려면 `try_map`을 쓴다. 모두 성공하면 `Ok(리스트)`, 아니면 처음 `Err`다.

```fung run
print(try_map(["1", "2", "3"], parse_int))
print(try_map(["1", "b", "c"], parse_int))
```

:::

## !로 멈추기 {#bang}

짧은 스크립트에서는 "실패하면 그냥 멈추면 된다"인 경우가 많다. 설정 파일이 없으면 더 할 일이 없다. 이때 `!`를 붙인다. `Ok(v)`면 `v`를 꺼내고, `Err`면 프로그램을 종료 코드 1로 멈추며 이유를 보여 준다(E0603).

```fung run exit=1 error=E0603
port = parse_int("8080")!
print("port:", port)
retries = parse_int("three")!
print("never printed")
```

`?`와 `!`의 차이는 "누가 처리하는가"다. `?`는 부른 쪽에 맡기고, `!`는 그 자리에서 끝낸다. 다른 코드가 쓰는 함수(10장의 모듈)에는 `?`가 어울리고, 스크립트 맨 위에서 한 번만 하는 일에는 `!`가 어울린다.

## ??로 기본값 {#default}

`a ?? b`는 `a`가 `Ok(v)`면 `v`, `Err`나 `nil`이면 `b`다. 실패해도 쓸 기본값이 있을 때 쓴다. 환경 변수를 읽는 `env`는 변수가 없으면 `nil`을 돌려주므로 `??`와 잘 어울린다.

```fung run
retries = parse_int("three") ?? 3
port = env("PORT") ?? "8080"
print(retries, port)
```

:::box mistake ??는 모든 실패를 덮는다
`fs.read(p) ?? "{}"`는 파일이 없을 때만이 아니라 권한이 없을 때도 `"{}"`다. 정말 문제인 실패까지 조용히 덮을 수 있다. 특정한 실패만 기본값으로 바꾸려면 위의 `load_config`처럼 `match`로 `kind`를 가린다.
:::

## 버리면 안 된다 {#discard}

결과값을 아무 데도 쓰지 않으면 어떻게 될까? 아래 첫 줄은 `parse_int`의 결과를 변수에 담지도, 출력하지도 않는다. 값이 그냥 버려진다. 버려진 값이 `Err`면 fung은 거기서 멈춘다(E0606).

```fung run exit=1 error=E0606
parse_int("abc")
print("여기까지 오지 않는다")
```

실패를 조용히 무시하는 버그를 막으려는 규칙이다. 쉘 스크립트에서 `set -e`를 켜지 않아 실패한 명령 뒤로 스크립트가 계속 달려가는 사고를 생각하면 된다. fung에서는 늘 켜져 있는 셈이다. 파일 쓰기처럼 성공하면 쓸 값이 없는 함수(`Ok(nil)`을 돌려준다)도 마찬가지다. `fs.write(p, text)`만 쓰면 쓰기가 실패했을 때 E0606으로 멈춘다.

정말 버려도 되는 실패라면 `_ = …`로 "일부러 버린다"고 적는다. 지우려던 임시 파일이 이미 없어도 상관없는 경우가 그렇다.

```fung run
import fs

_ = fs.remove("tmp-work.txt")
print("cleaned up")
```

:::box learn 값을 쓰는 자리와 버리는 자리
블록에서 마지막이 아닌 식, 문장으로 쓴 함수 호출, 값을 버리는 `for`의 본문이 "버리는 자리"다. 함수의 마지막 식은 반환값이므로 버려지지 않는다. 그래서 `fn f() = parse_int("x")`는 괜찮고, `f()`를 부른 쪽이 그 `Err`를 다뤄야 한다.
:::

## 내 함수에서 실패 돌려주기 {#own}

직접 만드는 함수도 같은 모양을 따르면 `match`·`?`·`!`·`??`가 그대로 통한다. 성공은 `Ok(값)`, 실패는 `Err(…)`로 돌려준다. `Err` 안에는 아무 값이나 넣을 수 있지만, 표준 라이브러리처럼 `{kind, message}` 맵을 넣으면 부르는 쪽이 `kind`로 나누고 `message`를 보여 주기 쉽다. 일찍 돌아갈 때는 `return`을 쓴다(5장).

```fung run
fn check_port(p):
  if p < 1 or p > 65535:
    return Err({kind: 'bad_port, message: "port out of range: ${p}"})
  ;;
  Ok(p)
;;

print(check_port(443))
print(check_port(70000))
print(check_port(0) ?? 8080)
```

## 결과값과 런타임 에러 {#runtime}

모든 실패가 결과값은 아니다. 0으로 나누기, 리스트 범위 밖의 인덱스, 맵에 없는 키 읽기, 잘못된 타입의 인자는 **런타임 에러**(runtime error)다. 런타임 에러는 값이 아니라 프로그램을 멈추는 사건이고, 잡아서 계속하는 구문은 없다.

```fung run exit=1 error=E0508
limits = {cpu: "500m"}
print(limits.memory)
```

둘의 구분은 이렇다. 프로그램 바깥 세상 때문에 생기는, 정상적인 프로그램에서도 일어나는 실패(파일이 없다, 입력이 숫자가 아니다, 서버가 꺼졌다)는 결과값이다. 코드가 틀려서 생기는 실패(버그)는 런타임 에러다. 버그는 감추지 말고 빨리 드러내는 편이 낫기 때문이다. 없을 수도 있는 키는 `get(m, 키, 기본값)`으로 읽으면 런타임 에러가 나지 않는다. `limits.memory ?? "128Mi"`처럼 `??`를 붙이는 것으로는 안 된다. `??`에 닿기 전에 키를 읽는 순간 멈추기 때문이다.

직접 "여기 오면 버그다"라고 말하고 싶으면 `fail(메시지)`로 런타임 에러를 낸다(E0803). 꼭 있어야 하는 값이 없을 때 `??`와 함께 자주 쓴다.

```fung run exit=1 error=E0803
fn require_port(cfg) = get(cfg, "port", nil) ?? fail("config.port is required")

print(require_port({port: 8080}))
print(require_port({}))
```

:::box other 다른 언어는
Python·Java·JavaScript는 예외를 던지고 `try`로 잡는다. Rust는 fung과 같은 `Result`와 `?`를 쓰고, 버그에는 `panic!`을 쓴다. fung의 결과값과 `?`는 Rust에서 왔고, `fail`이 `panic!`에 해당한다. 쉘은 종료 코드로 실패를 알리고 `set -e`·`||`로 다룬다.
:::

:::exercise 두 인자 더하기
명령줄 인자 두 개(`args`, 2장)를 정수로 바꿔 더해 출력하는 스크립트를 써 보자. 인자가 두 개가 아니거나 숫자가 아니면 표준 에러에 이유를 쓰고(`eprint`) 종료 코드 1로 끝낸다(`exit(1)`). 계산은 결과값을 돌려주는 함수로 만들고, 맨 위에서 한 번 `match`한다.

:::solution

인자로 `3 4`를 주고 돌린 결과다.

```fung run args="3 4"
fn add_args(xs) = match xs:
  [a, b] => Ok(parse_int(a)? + parse_int(b)?)
  _      => Err({kind: 'usage, message: "usage: add.fung A B"})
;;

match add_args(args):
  Ok(total) => print(total)
  Err(e)    =>
    eprint("error:", e.message)
    exit(1)
;;
```

같은 스크립트에 `3 four`를 주면 이렇게 끝난다.

```fung run args="3 four" exit=1
fn add_args(xs) = match xs:
  [a, b] => Ok(parse_int(a)? + parse_int(b)?)
  _      => Err({kind: 'usage, message: "usage: add.fung A B"})
;;

match add_args(args):
  Ok(total) => print(total)
  Err(e)    =>
    eprint("error:", e.message)
    exit(1)
;;
```

:::
:::

:::exercise key=value 줄 읽기
`["port=8080", "host=db", "debug=true"]` 같은 줄들을 `(키, 값)` 튜플의 리스트로 바꾸는 함수 `parse_all(lines)`를 써 보자. `=`가 없는 줄이 하나라도 있으면 그 줄을 알려 주는 `Err`를 돌려준다. 힌트: 한 줄을 바꾸는 함수를 먼저 만들고 `try_map`에 넘긴다.

:::solution

```fung run
fn parse_kv(line) = match split(line, "="):
  [k, v] => Ok((k, v))
  _      => Err({kind: 'syntax, message: "not key=value: ${line}"})
;;

fn parse_all(rows) = try_map(rows, parse_kv)

print(parse_all(["port=8080", "host=db"]))
print(parse_all(["port=8080", "broken", "debug=true"]))
```

:::
:::

## 정리 {#summary}

- 실패할 수 있는 함수는 `Ok(값)` 또는 `Err(이유)`를 돌려준다. 표준 라이브러리의 이유는 `{kind, message}` 맵이다.
- `match`로 성공과 실패를 나누고, `Err({kind: 'not_found})`처럼 종류로 다시 나눈다.
- `?`는 실패를 부른 쪽에 넘기고(함수 안에서만), `!`는 그 자리에서 멈추고(E0603), `??`는 모든 실패와 `nil`을 기본값으로 바꾼다.
- 버려진 `Err`는 에러다(E0606). 일부러 버릴 때는 `_ = …`.
- 리스트를 바꾸다 처음 실패에서 멈추려면 `try_map`.
- 버그는 결과값이 아니라 런타임 에러다. `fail(메시지)`로 직접 낼 수 있다.

정확한 규칙은 레퍼런스의 [결과값과 에러](../reference/language.html#results), 함수는 [결과값](../reference/prelude/results.html)과 [`try_map`](../reference/prelude/collections.html#fn-try_map), [`fail`](../reference/prelude/program.html#fn-fail)에 있다. 에러 번호는 [E0603](../reference/errors.html#E0603), [E0606](../reference/errors.html#E0606), [E0214](../reference/errors.html#E0214), [E0226](../reference/errors.html#E0226)을 보라.
