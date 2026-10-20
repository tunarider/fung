---
lede: 값의 타입을 알아보고, 한 타입의 값을 다른 타입으로 바꾼다. <code>int</code>·<code>float</code>·<code>str</code>·<code>sym</code>은 잘못된 입력에 런타임 에러를 낸다(코드가 틀린 것으로 본다). 믿을 수 없는 입력은 <code>parse_*</code>로 검사한다.
description: fung prelude 타입과 변환: type_of, int, float, str, sym, parse_int, parse_float, parse_dur.
---
# 타입과 변환

## 타입과 변환이란 {#types}

모든 값에는 **타입**(type)이 있다. `42`는 정수(`int`), `"42"`는 문자열(`str`), `'ok`는 심볼(`sym`)이다. 타입은 그 값으로 무엇을 할 수 있는지 정한다. 정수끼리는 더할 수 있지만 문자열 `"42"`에 1을 더할 수는 없다(`"42" + 1`은 런타임 에러다). fung은 타입을 몰래 바꿔 주지 않는다. 바꾸려면 변환 함수를 직접 부른다. 타입 목록과 각 타입의 연산은 [값과 타입](../language.html#values)에 있다.

변환이 필요한 가장 흔한 경우는 **바깥에서 온 글자**다. 명령줄 인자, 환경 변수, 파일 내용, 외부 명령의 출력은 모두 문자열로 들어온다. 포트 번호 `"8080"`을 수로 쓰려면 정수로 바꿔야 한다.

변환 함수는 두 무리로 나뉜다. 둘 중 무엇을 쓸지는 "실패하면 누구 잘못인가"로 정한다.

- `int`·`float`·`str`·`sym`: 변환할 수 없으면 **런타임 에러**(E0513)로 프로그램을 멈춘다. 입력이 틀릴 리 없는 자리, 곧 틀렸다면 코드가 틀린 자리에 쓴다.
- `parse_int`·`parse_float`·`parse_dur`: 실패를 **값**으로 돌려준다(`Ok`/`Err`). 사용자 입력이나 설정 파일처럼 틀릴 수 있는 입력을 검사할 때 쓴다. 결과는 `!`·`?`·`??`·`match`로 푼다([결과값](results.html)).

## 함수 {#functions}

:::fn type_of(x)

값의 타입 이름을 심볼로 돌려준다.

#### 설명

돌려주는 심볼은 [값과 타입](../language.html#values) 표의 이름이다: `'int` `'float` `'str` `'bool` `'nil` `'sym` `'dur` `'time` `'list` `'map` `'tuple` `'range` `'fn` `'module` `'file` `'task` `'scope` `'sender` `'receiver` `'atom`. 내장 함수와 람다는 모두 `'fn`이다.

태그 값(`Ok(1)`, `Circle(3)`)은 태그 이름 자체가 타입이라 `'Ok`, `'Circle`을 돌려준다. 그래서 결과값인지 가리는 [`is_ok`](results.html#fn-is_ok)는 `type_of(x) == 'Ok`로 쓰여 있다.

값의 모양에 따라 갈라지는 코드는 보통 `match`가 더 읽기 좋다. `type_of`는 에러 메시지에 타입을 보여 주거나, 여러 타입을 받는 함수의 입구에서 검사할 때 쓴다.

#### 매개변수

- `x`: 아무 값.

#### 돌려주는 값

심볼.

#### 에러

없다.

#### 예제

**여러 값의 타입**

```fung run
print(type_of(1), type_of(1.5), type_of("a"), type_of(nil), type_of('x), type_of(2s))
print(type_of([1]), type_of({a: 1}), type_of((1, 2)), type_of(0..3), type_of(print))
```

**태그 값은 태그 이름**

```fung run
print(type_of(Ok(1)), type_of(Err("x")), type_of(Circle(3)))
```

**입구에서 타입 검사하기**

```fung run exit=1 error=E0803
fn port_of(x) = match type_of(x):
  'int => x
  'str => int(x)
  t    => fail("port must be int or str, got ${t}")
;;

print(port_of(8080), port_of("9090"))
print(port_of(1.5))
```

#### 같이 보기

[값과 타입](../language.html#values), [`is_ok`](results.html#fn-is_ok), [match와 패턴](../language.html#match)

:::

:::fn int(x)

문자열이나 실수를 정수로 바꾼다. 변환할 수 없으면 E0513.

#### 설명

- 문자열: 정수 리터럴의 십진 문법을 받는다. 앞의 `+`·`-`, 자리 구분 `_`(`"1_000"`), 앞의 0(`"007"`)이 된다. 앞뒤 공백, 소수점(`"1.5"`), 16진(`"0x1f"`)은 받지 않는다.
- 실수: 0 쪽으로 버린다(`int(3.9) == 3`, `int(-3.9) == -3`). 정수 범위(64비트)를 넘는 실수는 에러다.
- 정수: 그대로.

그 밖의 타입(`bool`, `nil` 등)은 에러다. 사용자 입력처럼 틀릴 수 있는 문자열은 [`parse_int`](#fn-parse_int)로 검사한다. 앞뒤 공백이 있을 수 있으면 먼저 [`trim`](strings.html#fn-trim)한다.

#### 매개변수

- `x`: `str`, `float`, `int`.

#### 돌려주는 값

정수.

#### 에러

변환할 수 없으면 런타임 에러 E0513(`cannot convert "abc" to int`).

#### 예제

**문자열과 실수에서**

```fung run
print(int("42") + 1, int("-1_000"), int("+5"), int("007"))
print(int(3.9), int(-3.9), int(2.5e3))
```

**변환할 수 없는 값은 멈춘다**

```fung run exit=1 error=E0513
print(int(" 4"))
```

**공백이 있을 수 있으면 trim부터**

```fung run
print(int(trim(" 4\n")))
```

#### 같이 보기

[`parse_int`](#fn-parse_int), [`float`](#fn-float), [`trim`](strings.html#fn-trim), [E0513](../errors.html#E0513)

:::

:::fn float(x)

정수나 문자열을 실수로 바꾼다. 변환할 수 없으면 E0513.

#### 설명

- 정수: 같은 값의 실수(`float(3)`은 `3.0`).
- 문자열: 실수 리터럴 문법을 받는다. 앞의 부호, 정수 꼴, 지수, `_`가 된다: `"2.5"`, `"-1e3"`, `"42"`, `"1_000.5"`. 점 앞이나 뒤가 빈 꼴(`".5"`, `"1."`), 16진 실수, 앞뒤 공백, `"NaN"`·`"inf"`, 실수 범위를 넘는 수(`"1e400"`)는 거부한다. fung의 실수는 `NaN`·`Inf`가 될 수 없기 때문이다.
- 실수: 그대로.

#### 매개변수

- `x`: `int`, `str`, `float`.

#### 돌려주는 값

실수.

#### 에러

변환할 수 없으면 런타임 에러 E0513.

#### 예제

**정수와 문자열에서**

```fung run
print(float(3), float("2.5"), float("-1e3"), float("42"))
print(7 / 2, float(7) / 2)
```

**받지 않는 꼴**

```fung run exit=1 error=E0513
print(float(".5"))
```

#### 같이 보기

[`parse_float`](#fn-parse_float), [`int`](#fn-int)

:::

:::fn str(x)

값을 문자열로 바꾼다. 문자열이면 그대로, 아니면 리터럴 모양의 문자열.

#### 설명

[`print`](io.html#fn-print)가 값을 찍을 때와 같은 글자를 만든다. 바깥의 문자열은 따옴표 없이 그대로 돌려주고, 리스트·맵 안의 문자열에는 따옴표가 붙는다. 실수는 정수 꼴이라도 `.0`을 남긴다(`str(1.0)`은 `"1.0"`). 기간은 `2m`처럼, 심볼은 `'ok`처럼 바뀐다.

문자열을 이어 붙이는 `+`는 양쪽이 문자열이어야 하므로 수를 섞을 때 `str`을 쓴다. 보통은 보간 `"${x}"`가 더 짧다(보간도 같은 규칙으로 바꾼다). 맵의 키는 문자열이어야 해서 수로 묶을 때도 쓴다([`group_by`](collections.html#fn-group_by)).

#### 매개변수

- `x`: 아무 값.

#### 돌려주는 값

문자열.

#### 에러

없다.

#### 예제

**여러 값을 문자열로**

```fung run
print(str(42) + "!", str(1.0), str(2m), str('ok), str(nil))
print(str(["a", 1]), str({k: "v"}), str(Ok("a")))
```

**`+`로 이을 때와 보간**

```fung run
n = 3
print("found " + str(n) + " hosts")
print("found ${n} hosts")
```

#### 같이 보기

[`print`](io.html#fn-print), [`join`](strings.html#fn-join), [`int`](#fn-int)

:::

:::fn sym(x)

이름 모양의 문자열을 심볼로 바꾼다(`sym("ok") == 'ok`).

#### 설명

심볼은 이름만 있는 값이다. 문자열은 심볼과 비교할 수 없으므로(E0517) 파일이나 JSON에서 읽은 `"not_found"`를 `'not_found`와 견주려면 먼저 `sym`으로 바꾼다.

받는 문자열은 소문자 이름의 모양이어야 한다: 소문자나 `_`, 또는 대소문자가 없는 글자(한글 등)로 시작하고 글자·숫자·`_`가 이어지는 것(`"ok"`, `"_x"`, `"a1"`, `"한글"`). 대문자로 시작하는 것(`"Ok"`, 태그 모양), 빈 문자열, 공백이나 `-`가 든 것은 E0513이다. 심볼이면 그대로 돌려준다.

:::box mistake 외부 입력을 그대로 심볼로 만들지 않는다

만든 심볼은 프로그램이 끝날 때까지 해제되지 않는다. 요청 값이나 로그의 낱말처럼 종류에 끝이 없는 입력을 `sym`에 넣으면 메모리가 계속 는다. 정해진 몇 개의 이름(상태, 명령 이름)만 바꾸고, 나머지는 문자열로 다룬다.

:::

#### 매개변수

- `x`: 문자열 또는 심볼.

#### 돌려주는 값

심볼.

#### 에러

이름 모양이 아니면 런타임 에러 E0513.

#### 예제

**문자열을 심볼과 견주기**

```fung run
status = "not_found"
print(sym(status) == 'not_found)
```

**정해진 이름만 받아 match로 가르기**

```fung run
fn handle(cmd) = match sym(cmd):
  'start => "starting"
  'stop  => "stopping"
  _      => "unknown command"
;;

print(handle("start"), handle("stop"))
```

**이름 모양이 아닌 문자열**

```fung run exit=1 error=E0513
print(sym("not-found"))
```

#### 같이 보기

[비교와 동일성](../language.html#values), [E0517](../errors.html#E0517), [`str`](#fn-str)

:::

:::fn parse_int(s)

문자열을 정수로 읽어 `Ok(정수)` 또는 `Err({kind: 'parse, message})`를 돌려준다.

#### 설명

받는 문법은 [`int`](#fn-int)의 문자열 규칙과 같다(부호, `_`, 앞의 0은 되고, 공백·소수점·16진은 안 된다). 다른 점은 실패를 런타임 에러 대신 `Err` 값으로 돌려준다는 것 하나다. 그래서 사용자 입력, 설정 값, 외부 명령 출력처럼 틀릴 수 있는 글자에 쓴다.

#### 매개변수

- `s`: 문자열. 문자열이 아니면 E0403(코드의 잘못이다).

#### 돌려주는 값

`Ok(정수)` 또는 `Err({kind: 'parse, message: "not an integer: …"})`.

#### 에러

`s`가 문자열이 아니면 런타임 에러 E0403.

#### 예제

**성공과 실패**

```fung run
print(parse_int("1_000"), parse_int("-7"))
print(parse_int("x"))
print(parse_int(" 1"))
```

**기본값으로 물러나기**

```fung run
port = parse_int(env("PORT") ?? "") ?? 8080
print(port)
```

`PORT`가 없거나 수가 아니면 `8080`이다. `??`는 모든 `Err`를 기본값으로 바꾼다.

**여러 개를 한 번에: 하나라도 틀리면 Err**

```fung run
print(["1", "2", "3"] |> try_map(parse_int))
print(["1", "two", "3"] |> try_map(parse_int))
```

#### 같이 보기

[`int`](#fn-int), [`try_map`](collections.html#fn-try_map), [결과값과 에러](../language.html#results)

:::

:::fn parse_float(s)

문자열을 실수로 읽어 `Ok(실수)` 또는 `Err({kind: 'parse, message})`를 돌려준다.

#### 설명

문법은 [`float`](#fn-float)의 문자열 규칙과 같다(`".5"`, `"1."`, 공백, `NaN`은 거부). 정수 꼴(`"3"`)은 `Ok(3.0)`이다.

#### 매개변수

- `s`: 문자열.

#### 돌려주는 값

`Ok(실수)` 또는 `Err({kind: 'parse, message: "not a number: …"})`.

#### 에러

`s`가 문자열이 아니면 런타임 에러 E0403.

#### 예제

**성공과 실패**

```fung run
print(parse_float("2.5"), parse_float("3"), parse_float("1e3"))
print(parse_float(".5"))
```

**match로 가르기**

```fung run
fn ratio(s) = match parse_float(s):
  Ok(x)  => "ratio ${x}"
  Err(e) => "skip: ${e.message}"
;;

print(ratio("0.75"))
print(ratio("high"))
```

#### 같이 보기

[`float`](#fn-float), [`parse_int`](#fn-parse_int)

:::

:::fn parse_dur(s)

기간 리터럴 문법의 문자열(`"1h30m"`, `"500ms"`)을 `Ok(dur)` 또는 `Err({kind: 'parse, message})`로 읽는다.

#### 설명

`--timeout 30s` 같은 명령줄 옵션이나 설정 값을 기간으로 바꿀 때 쓴다. 받는 문법은 소스의 [기간 리터럴](../language.html#lexical)과 같다: 단위는 `h` `m` `s` `ms` 넷이고 큰 것부터 붙여 쓰며(`"1h30m"`), 앞에 `-`를 붙일 수 있고, 소수도 된다(`"1.5s"`는 `1s500ms`). 앞뒤 공백은 허용한다. 사이의 공백(`"1h 30m"`), 작은 단위를 먼저 쓴 것(`"1m1h"`), 모르는 단위(`"2d"`, `"2us"`)는 거부한다.

얻은 기간은 [`sleep`](concurrency.html#fn-sleep), [`timeout`](concurrency.html#fn-timeout), 시각 계산에 그대로 쓴다. 기간끼리는 비교할 수 있다.

#### 매개변수

- `s`: 문자열.

#### 돌려주는 값

`Ok(기간)` 또는 `Err({kind: 'parse, message: "not a duration: …"})`.

#### 에러

`s`가 문자열이 아니면 런타임 에러 E0403.

#### 예제

**여러 꼴**

```fung run
print(parse_dur("90s")!, parse_dur(" 1h30m ")!, parse_dur("1.5s")!, parse_dur("-5s")!)
print(parse_dur("soon"))
print(parse_dur("1h 30m"))
```

**옵션 값을 검사하고 상한 두기**

```fung run args="45m"
limit = parse_dur(args[0])!
print(limit > 30m then "too long, using 30m" else "using ${limit}")
```

#### 같이 보기

[값과 타입](../language.html#values), [`timeout`](concurrency.html#fn-timeout), [`sleep`](concurrency.html#fn-sleep), [`cli` 모듈](../modules/cli.html)

:::
