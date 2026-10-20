---
description: fung의 모든 에러와 경고 번호: 뜻, 예시, 고치는 법.
lede: 에러 번호는 한 번 붙이면 바뀌지 않는다. 메시지 첫 줄의 <code>error[E0606]</code> 같은 번호로 이 쪽에서 찾는다.
---
# 에러 카탈로그

프로그램이 잘못되면 fung은 멈추고 에러(error) 메시지를 표준 에러로 찍는다. 메시지마다 번호가 붙어 있고, 이 쪽은 번호마다 뜻, 흔한 원인, 에러를 내는 짧은 예, 고치는 법을 모았다. 경고(warning)는 프로그램을 멈추지 않는 알림이다.

## 에러 메시지 읽는 법 {#reading}

예를 하나 돌려 보자. 정수를 받아야 하는 함수에 문자열을 넘겼다.

```fung run exit=1 error=E0403
fn backoff(n: int) = n * 2
print(backoff("3"))
```

메시지는 위에서부터 이렇게 읽는다.

1. **첫 줄** `error[E0403]: …`: 에러 번호와 한 줄 설명. 번호로 이 쪽의 항목을 찾는다. 경고는 `warning[W0102]`처럼 시작한다.
2. **위치** `--> example.fung:1:12`: 파일 이름, 줄, 칸(열). 칸은 1부터 센다.
3. **소스 발췌**: 그 줄을 그대로 보이고 아래에 `^`로 문제의 자리를 가리킨다. `^` 옆에 짧은 설명(예: `not found in this scope`)이 붙기도 한다.
4. **호출 경로** `call path (innermost first)`: 실행 중의 에러에만 붙는다. 에러가 난 함수부터 바깥쪽으로, 누가 누구를 불렀는지 보여 준다. `<entry>`는 실행한 파일의 최상위, `<lambda>`는 이름 없는 함수다. 아주 깊은 재귀는 가운데를 `… n more`로 줄인다.
5. **도움말** `help: …`: 고치는 방법의 힌트. 카탈로그에 적힌 문구가 기본이고, 상황에 맞춰 바뀌기도 한다(비슷한 이름을 알려 주는 E0204처럼).
6. **덧붙임**: `note: …` 줄이나 `assert`가 비교한 양쪽 값(E0807)처럼 그 에러에만 있는 정보가 더 붙을 수 있다.

컴파일 에러(실행 전에 찾은 에러)에는 호출 경로가 없다. 아직 아무것도 실행하지 않았기 때문이다.

```fung run exit=2 error=E0204
files = ["a.log", "b.log"]
print(len(fiels))
```

이 쪽의 에러 예는 모두 페이지를 만들 때 실제로 돌린 출력이 붙는다(출력 칸 머리의 종료 코드도 실제 값이다). `fung run`으로는 보이지 않는 경고(W01xx, `fung check`가 보여 준다)와 `fung test`·REPL에서만 나는 에러만 그 명령의 실제 출력을 옮겨 적었다. 고친 코드도 실제로 실행된다.

## 종료 코드 {#exit-codes}

프로그램이 끝나면 운영 체제에 종료 코드(exit code)라는 숫자를 남긴다. 셸에서는 `echo $?`로 본다. fung의 종료 코드는 다음과 같다(자세한 것은 [명령줄 도구](cli.html#errors)).

<table>
  <thead><tr><th>종료 코드</th><th>뜻</th></tr></thead>
  <tbody>
    <tr><td><code>0</code></td><td>정상 종료. 경고만 있으면 0이다.</td></tr>
    <tr><td><code>1</code></td><td>실행 중 실패: 런타임 에러, <code>!</code>가 <code>Err</code>를 만남, 아무도 확인하지 않은 태스크의 실패, 테스트 실패.</td></tr>
    <tr><td><code>2</code></td><td>컴파일 실패: 문법·이름·import 에러(E01xx~E03xx, 그리고 E0802와 인자 개수의 E0806). 아무것도 실행하지 않는다.</td></tr>
    <tr><td><code>3</code></td><td><code>fung</code> 명령을 잘못 썼다(없는 명령, 읽을 수 없는 파일 등).</td></tr>
    <tr><td><code>n</code></td><td>스크립트가 <code>exit(n)</code>을 불렀다.</td></tr>
  </tbody>
</table>

`fung check`는 실행하지 않고 컴파일 단계만 검사하므로 종료 코드 2의 에러와 경고만 찾는다. 인자 개수나 타입(E04xx), 0으로 나누기(E05xx) 같은 것은 실행해야 드러난다.

## 찾아보기 {#index}

{{errors}}

## E01xx: 문법(렉서·파서) {#e01}

소스는 먼저 렉서(lexer)가 글자를 토큰(token, 숫자·이름·연산자 같은 낱말)으로 자르고, 파서(parser)가 토큰을 문장과 식의 구조로 묶는다. 이 단계에서 글자나 구조가 규칙에 맞지 않으면 E01xx다. 실행 전에 찾는 컴파일 에러라 종료 코드는 2이고, 문법 에러가 있으면 파일의 어떤 줄도 실행하지 않는다. [어휘](language.html#lexical)와 [문장과 블록](language.html#blocks)이 규칙이다.

:::error E0101

fung이 쓰지 않는 글자가 코드에 나왔다. 문자열과 주석 밖에서는 정해진 연산자와 괄호, 이름에 쓰는 글자만 올 수 있다.

흔한 원인: 다른 언어의 연산자(`@`, `&&`의 `&`, `$` 등)를 그대로 썼거나, 복사해 온 코드에 보이지 않는 특수 문자가 섞였다.

```fung run exit=2 error=E0101
used = 75
print(used @ 100)
```

고치는 법: 뜻한 fung 연산자로 바꾼다. 논리 연산은 `and`·`or`·`not`이다.

```fung run
used = 75
print(used * 100)
```

:::

:::error E0102

숫자 모양으로 시작했는데 숫자 규칙에 맞지 않는다. 숫자 바로 뒤에 글자가 붙으면 하나의 잘못된 숫자로 본다.

흔한 원인: `3x`처럼 숫자와 글자를 붙여 씀, 16진수에 없는 글자(`0xfg`), 기간 단위가 아닌 글자(`30sec`; 기간 단위는 `ms`·`s`·`m`·`h`뿐이다). `1.2.3` 같은 버전 문자열은 숫자 `1.2` 뒤의 필드 접근으로 읽혀 E0112가 된다. 버전은 문자열 `"1.2.3"`으로 쓴다.

```fung run exit=2 error=E0102
retries = 3x
```

고치는 법: 숫자만 쓰거나, 글자가 필요하면 문자열로 만든다.

```fung run
retries = 3
print(retries)
```

:::

:::error E0103

문자열 `"…"` 안의 `\` 뒤에 fung이 모르는 글자가 왔다. 아는 이스케이프는 `\n` `\t` `\r` `\\` `\"` `\$` `\u{…}`뿐이고, 나머지는 실수로 보고 에러로 막는다.

흔한 원인: Windows 경로나 정규식을 보통 문자열에 그대로 썼다.

```fung run exit=2 error=E0103
print("C:\temp\logs")
```

`\t`는 탭이라 통과하고 `\l`에서 멈췄다. 경로가 조용히 탭으로 바뀌는 것도 버그이니, 고치는 법은 같다: 백틱 원시 문자열(raw string)을 쓰거나 `\\`로 쓴다.

```fung run
print(`C:\temp\logs`)
print("C:\\temp\\logs")
```

:::

:::error E0104

문자열을 열었는데 같은 줄에서 닫는 `"`가 없다. 보통 문자열에는 실제 줄바꿈을 넣을 수 없다.

흔한 원인: 닫는 따옴표를 빠뜨림, 여러 줄 문자열을 `"…"`로 씀(여러 줄은 백틱 원시 문자열이나 `\n`).

```fung run exit=2 error=E0104
print("hello)
```

위치는 문자열이 시작한 자리를 가리킨다.

```fung run
print("hello")
```

:::

:::error E0105

기간(duration) 리터럴의 모양이 틀렸다. 기간은 숫자와 단위(`ms` `s` `m` `h`)의 쌍을 큰 단위부터 붙여 쓴다.

흔한 원인: 작은 단위를 먼저 씀(`30s5m`), 마지막 숫자에 단위를 빠뜨림(`1h30`).

```fung run exit=2 error=E0105
wait = 30s5m
```

```fung run
wait = 5m30s
print(wait)
```

:::

:::error E0110

이 자리에 올 수 없는 토큰이 나왔다. `%s` 자리에 그 토큰이 무엇인지 적힌다.

흔한 원인: 인자 사이의 `,`를 빠뜨림, `,`를 두 번 씀, 보간 `${…}` 안의 식이 덜 끝남, 태그에 이름 인자를 줌(`Ok(a: 1)`).

```fung run exit=2 error=E0110
print("a" "b")
```

```fung run
print("a", "b")
```

:::

:::error E0111

비교 연산자를 이어 썼다. 수학처럼 `1 < x < 10`이라고 쓰고 싶지만, fung은 비교를 연쇄하지 않는다. 다른 언어마다 뜻이 달라 헷갈리기 쉬운 문법이라 아예 막았다.

```fung run exit=2 error=E0111
x = 5
print(1 < x < 10)
```

고치는 법: 도움말대로 `and`로 나눈다.

```fung run
x = 5
print(1 < x and x < 10)
```

:::

:::error E0112

파서가 특정 토큰을 기대했는데 다른 것이 나왔다. 메시지가 무엇을 기대했고(`expected`) 무엇을 만났는지(`found`) 알려 준다.

흔한 원인: `if`·`for`·`match`·`fn` 뒤의 블록을 여는 `:`를 빠뜨림, 블록을 닫는 `;;`를 빠뜨림(그러면 `found end of file`), `.` 뒤에 이름이 아닌 것(`1.2.3`).

```fung run exit=2 error=E0112
x = 5
if x > 0 print("positive") ;;
```

```fung run
x = 5
if x > 0: print("positive") ;;
```

`;;`를 빠뜨리면 파일 끝에서야 알아채므로, 위치가 파일의 마지막 줄 다음을 가리킨다(`expected ;; to close the if, found end of file`). 그때는 열어 둔 블록을 위로 거슬러 찾는다.

:::

:::error E0113

맵 리터럴 하나에 같은 키를 두 번 썼다. 어느 값을 뜻했는지 알 수 없으므로 에러다.

흔한 원인: 설정 맵을 고치다가 옛 줄을 지우지 않음.

```fung run exit=2 error=E0113
cfg = {host: "db1", port: 5432, host: "db2"}
```

고치는 법: 하나만 남긴다. 실행 중에 값을 바꾼 사본이 필요하면 `put`이나 `merge`를 쓴다.

```fung run
cfg = {host: "db2", port: 5432}
print(cfg)
```

:::

:::error E0114

`test "이름": … ;;` 블록이 파일 최상위가 아닌 곳(함수나 블록 안)에 있다. `fung test`는 최상위의 테스트 블록만 찾아 돌린다.

```fung run exit=2 error=E0114
fn check_all():
  test "inside": assert true ;;
;;
```

고치는 법: 테스트를 최상위로 꺼낸다. `fung run`은 테스트 블록을 무시한다. 함수 안에서 조건을 확인하고 싶으면 `assert`만 쓴다([테스트와 디버깅](language.html#testing)).

```fung run
fn half(n) = n // 2

test "half": assert half(4) == 2 ;;
print(half(9))
```

:::

## E02xx: 이름과 스코프 {#e02}

문법이 맞으면 fung은 이름(변수, 함수, 타입)이 어디서 무엇을 가리키는지 정한다. 이것을 이름 해석(resolve)이라 하고, 이름이 보이는 범위를 스코프(scope)라 한다. 이 단계의 에러가 E02xx다. 역시 실행 전에 찾는 컴파일 에러라 종료 코드는 2이고, 한 번에 여러 개를 보고할 수 있다. 규칙은 [변수와 스코프](language.html#scope)에 있다: 이름은 위에서 아래로 묶이고, 블록은 바깥 변수를 읽을 수만 있고, 블록 안의 이름은 밖으로 새지 않는다.

:::error E0204

그 자리에서 보이지 않는 이름을 썼다.

흔한 원인: 오타(도움말이 비슷한 이름을 알려 준다), 대입보다 위에서 읽음, 람다가 아직 대입되지 않은 이름을 씀, 최상위 `fn`이 두 번 이상 대입되는 최상위 변수를 읽음.

```fung run exit=2 error=E0204
files = ["a.log", "b.log"]
print(len(fiels))
```

최상위 `fn`은 한 번만 대입되는 최상위 이름(상수)만 볼 수 있다. 두 번 대입하는 이름을 읽으면 같은 E0204가 나고, 도움말이 이유를 설명한다.

```fung run exit=2 error=E0204
level = 1
level = 2
fn show() = level
print(show())
```

고치는 법: 철자를 고치거나, 대입을 위로 옮기거나, 값을 인자로 넘긴다.

```fung run
files = ["a.log", "b.log"]
print(len(files))
```

:::

:::error E0205

블록 안에서 블록 바깥의 변수에 다시 대입했다. fung의 블록(`if`·`for`·함수 본문 등)은 바깥 이름을 읽을 수만 있다. 값이 어디서 바뀌는지 찾아 헤매지 않게 하려는 규칙이다.

흔한 원인: 셸이나 다른 언어처럼 반복문 안에서 카운터를 늘림.

```fung run exit=2 error=E0205
files = ["a.log", "b.txt", "c.log"]
count = 0
for f in files:
  if ends_with(f, ".log"): count = count + 1 ;;
;;
print(count)
```

고치는 법: 값을 식으로 만든다. 세기는 `filter`와 `len`, 누적은 `fold`다.

```fung run
files = ["a.log", "b.txt", "c.log"]
count = files |> filter(f => ends_with(f, ".log")) |> len
total = fold(files, 0, (n, f) => n + len(f))
print(count, total)
```

:::

:::error E0206

블록 안에서 만든 이름을 블록 밖에서 읽었다. 블록 안의 이름은 블록이 끝나면 사라진다.

흔한 원인: `if`의 두 가지에서 같은 변수에 대입하고 아래에서 쓰려 함.

```fung run exit=2 error=E0206
port = 8080
if port < 1024:
  kind = "system"
else:
  kind = "user"
;;
print(kind)
```

고치는 법: `if`는 식이므로 블록의 값을 바로 대입한다.

```fung run
port = 8080
kind = if port < 1024:
  "system"
else:
  "user"
;;
print(kind)
```

:::

:::error E0208

함수 안에서 정의한 지역 함수가, 같은 블록에서 더 아래에 정의된 지역 함수를 불렀다. 지역 함수는 위에서 아래로 만들어지므로 아직 없는 함수를 부를 수 없다. 지역 함수가 부를 수 있는 것은 자기 자신(재귀)과 위에서 이미 만든 이름뿐이다.

흔한 원인: 서로를 부르는 두 함수(상호 재귀)를 함수 안에 둠.

```fung run exit=2 error=E0208
fn main():
  fn is_even(n) = n == 0 then true else is_odd(n - 1)
  fn is_odd(n) = n == 0 then false else is_even(n - 1)
  print(is_even(10))
;;
main()
```

고치는 법: 두 함수를 최상위로 옮긴다. 최상위 `fn`은 시작 전에 모두 만들어지므로 순서와 상관없이 서로 부른다.

```fung run
fn is_even(n) = n == 0 then true else is_odd(n - 1)
fn is_odd(n) = n == 0 then false else is_even(n - 1)

fn main() = print(is_even(10))
main()
```

:::

:::error E0209

`fn`으로 정의한 함수나 import로 가져온 이름에 값을 대입했다. 이런 이름은 파일 어디서나 같은 것을 가리켜야 하므로 바꿀 수 없다. (모듈 이름 자체에 대입하면 E0308이다.)

```fung run exit=2 error=E0209
fn greet(name) = "hello, ${name}"
greet = "hi"
```

고치는 법: 다른 이름을 쓴다.

```fung run
fn greet(name) = "hello, ${name}"
greeting = greet("fung")
print(greeting)
```

:::

:::error E0210

같은 이름의 `fn`을 두 번 정의했다. fung에는 인자 개수로 함수를 고르는 오버로딩(overloading)이 없다.

```fung run exit=2 error=E0210
fn area(w, h) = w * h
fn area(r) = 3.14 * r * r
```

고치는 법: 이름을 나누거나, 기본값 있는 인자나 태그와 `match`로 한 함수에 합친다.

```fung run
fn rect_area(w, h) = w * h
fn circle_area(r) = 3.14 * r * r
print(rect_area(2, 3), circle_area(1))
```

:::

:::error E0211

함수 밖, 곧 실행하는 파일의 최상위에서 `return`을 썼다. `return`은 감싸는 함수에서 돌아오는 것이라 최상위에는 돌아갈 곳이 없다.

흔한 원인: 셸 스크립트처럼 인자 검사 뒤에 `return`으로 끝내려 함.

```fung run exit=2 error=E0211
if len(args) == 0:
  print("usage: tool FILE")
  return 1
;;
```

고치는 법: 스크립트를 일찍 끝낼 때는 `exit(n)`을 쓴다. 아래 예는 인자 없이 돌았으므로 종료 코드 1로 끝난다.

```fung run exit=1
if len(args) == 0:
  print("usage: tool FILE")
  exit(1)
;;
print("working")
```

:::

:::error E0212

타입 표기에 없는 타입 이름을 썼다. 쓸 수 있는 것은 내장 타입 이름(`int` `str` `list` …), 태그 이름, 같은 파일의 `type` 별칭이다([타입 표기](language.html#types)).

흔한 원인: 다른 언어의 타입 이름(`integer`, `string`, `boolean`)을 씀, 별칭 이름의 오타.

```fung run exit=2 error=E0212
fn retry(times: integer) = times
```

```fung run
fn retry(times: int) = times
print(retry(3))
```

:::

:::error E0213

`for` 본문 안에서 `return`을 썼다. `for`는 사실 `map`이 부르는 람다라서, 본문의 `return`은 바깥 함수가 아니라 그 람다에서 돌아오게 된다. 뜻과 다르게 동작하므로 막았다.

흔한 원인: "찾으면 바로 돌려준다"를 반복문과 `return`으로 씀.

```fung run exit=2 error=E0213
fn first_big(sizes):
  for s in sizes:
    if s > 100: return s ;;
  ;;
;;
```

고치는 법: 찾기는 `find`, 검사는 `all`·`any`, 누적은 `fold`로 쓴다.

```fung run
fn first_big(sizes) = find(sizes, s => s > 100)
print(first_big([10, 300, 500]))
```

:::

:::error E0214

함수 밖에서 `?`를 썼다. `x?`는 `Err`를 만나면 "지금 함수에서" 그 `Err`를 돌려주는 연산자인데, 최상위에는 돌려줄 함수가 없다.

```fung run exit=2 error=E0214
port = parse_int("8080")?
```

고치는 법: 스크립트 최상위에서는 `!`(실패하면 멈춤)나 `??`(기본값), `match`를 쓴다([결과값과 에러](language.html#results)).

```fung run
port = parse_int("8080")!
print(port)
```

:::

:::error E0215

`_`를 쓸 수 없는 자리에 썼다. `_`는 호출의 인자 자리(부분 적용, `f(1, _)`)와 패턴(무엇이든 맞고 버림)에서만 뜻이 있다.

흔한 원인: `_ + 1`처럼 `_`로 짧은 람다를 만들려 함.

```fung run exit=2 error=E0215
add_one = _ + 1
```

고치는 법: 람다를 쓴다. 함수 호출이라면 부분 적용이 된다.

```fung run
add_one = x => x + 1
add = (a, b) => a + b
add_ten = add(10, _)
print(add_one(1), add_ten(5))
```

:::

:::error E0216

`|`로 묶은 대안 패턴들이 서로 다른 이름을 묶는다. 어느 대안이 맞았느냐에 따라 어떤 변수는 있고 어떤 변수는 없게 되므로 에러다.

```fung run exit=2 error=E0216
fn describe(r) = match r:
  Ok(v) | Err(e) => "done"
;;
```

고치는 법: 대안마다 같은 이름을 묶거나, 이름이 필요 없으면 `_`를 쓴다. 다른 처리가 필요하면 절을 나눈다.

```fung run
fn describe(r) = match r:
  Ok(_) | Err(_) => "done"
;;
print(describe(Ok(1)))
```

:::

:::error E0217

같은 이름의 `type` 별칭을 두 번 정의했다.

```fung run exit=2 error=E0217
type id = int
type id = str
```

```fung run
type user_id = int
type token = str
fn show(u: user_id, t: token) = "${u}:${t}"
print(show(7, "abc"))
```

:::

:::error E0218

타입 별칭이 자기 자신을 가리킨다(직접이든 다른 별칭을 거쳐서든). 별칭은 이미 있는 타입에 붙이는 다른 이름일 뿐이라, 자기를 풀면 끝이 없다.

흔한 원인: 트리 같은 재귀 구조를 별칭으로 표현하려 함. 그런 구조는 태그로 만든다. 태그 이름은 인자 모양을 검사하지 않으므로 `Node(l, r)`이 그대로 재귀 구조가 된다.

```fung run exit=2 error=E0218
type tree = tree
fn size(t: tree) = 1
```

```fung run
type tree = Leaf | Node
fn size(t: tree) = match t:
  Leaf       => 0
  Node(l, r) => 1 + size(l) + size(r)
;;
print(size(Node(Leaf, Node(Leaf, Leaf))))
```

:::

:::error E0219

`type` 별칭을 함수나 블록 안에 정의했다. 별칭은 파일 최상위에만 둔다.

```fung run exit=2 error=E0219
fn area(s):
  type shape = Circle | Rect
  s
;;
```

```fung run
type shape = Circle | Rect

fn area(s: shape) = match s:
  Circle(r)  => 3 * r * r
  Rect(w, h) => w * h
;;
print(area(Rect(2, 3)))
```

:::

:::error E0220

`for` 본문 안에서 `?`를 썼다. E0213과 같은 이유다: 본문은 람다라서 `?`가 바깥 함수가 아니라 람다에서 `Err`를 돌려주고, 그 `Err`는 결과 리스트의 원소가 될 뿐이다.

```fung run exit=2 error=E0220
fn ports(xs) = for x in xs: parse_int(x)? ;;
```

고치는 법: 첫 `Err`에서 멈추는 `try_map`을 쓴다. 결과는 `Ok(리스트)`나 첫 `Err`다.

```fung run
fn ports(xs) = try_map(xs, x => parse_int(x))
print(ports(["80", "443"]))
print(ports(["80", "http"]))
```

:::

:::error E0221

패턴 하나에 같은 이름을 두 번 썼다. 패턴 안의 이름은 늘 새로 묶는 것이지 "같은 값인지 비교"가 아니다.

```fung run exit=2 error=E0221
fn same(pair) = match pair:
  [a, a] => true
  _      => false
;;
```

고치는 법: 다른 이름으로 묶고 `when` 가드(guard)로 비교한다.

```fung run
fn same(pair) = match pair:
  [a, b] when a == b => true
  _                  => false
;;
print(same([1, 1]), same([1, 2]))
```

:::

:::error E0222

내장 타입 이름(`int`, `str`, `any`, `num` …)을 별칭 이름으로 썼다. 그러면 같은 이름이 파일마다 다른 뜻이 되므로 막는다.

```fung run exit=2 error=E0222
type str = int
```

```fung run
type name = str
fn hello(n: name) = "hello, ${n}"
print(hello("fung"))
```

:::

:::error E0223

값으로 쓰는 자리의 `if`에 `else`가 없다. `else`가 없는 `if`는 조건이 거짓일 때 `nil`이 되는데, 값을 쓰는 자리(대입의 오른쪽, 인자, `fn f() = …`나 람다의 본문, `return` 등)에서는 거의 늘 실수다.

흔한 원인: 한 가지만 쓰고 나머지 경우를 잊음. `else if` 사슬의 마지막에 `else`가 없는 것도 같다.

```fung run exit=2 error=E0223
fn label(code) = if code >= 500: "server error" ;;
```

고치는 법: `else`를 쓴다. 정말 `nil`을 뜻하면 `else: nil`이라고 적는다. 값을 쓰지 않는 문장 자리의 `if`는 `else` 없이 써도 된다([조건](language.html#conditions)).

```fung run
fn label(code) = if code >= 500: "server error" else: "ok" ;;
print(label(503), label(200))
```

:::

:::error E0224

값으로 쓰는 블록이 대입으로 끝난다. 블록의 값은 마지막 식의 값인데, 대입은 값이 없으므로 블록의 값이 `nil`이 된다.

```fung run exit=2 error=E0224
x = if true:
  y = 1
else:
  2
;;
```

고치는 법: 블록의 마지막 줄에 값을 둔다. 이전에는 경고 W0104였다.

```fung run
x = if true:
  y = 1
  y
else:
  2
;;
print(x)
```

:::

:::error E0225

대문자로 시작하는 이름을 변수로 쓰려 했다. fung에서 대문자로 시작하는 이름은 태그(`Ok`, `Circle`)라서 변수가 될 수 없다.

흔한 원인: 다른 언어 습관대로 상수를 `MAX_RETRIES`로 씀, 대문자 키(`HOME`)를 맵 축약 `{HOME} = cfg`나 `{HOME}`으로 꺼냄.

```fung run exit=2 error=E0225
cfg = {HOME: "/home/me"}
{HOME} = cfg
```

고치는 법: 변수 이름은 소문자로 쓴다. 대문자 키는 `{HOME: home} = cfg`로 꺼내거나 `cfg.HOME`으로 읽는다.

```fung run
max_retries = 3
cfg = {HOME: "/home/me"}
{HOME: home} = cfg
print(max_retries, home, cfg.HOME)
```

:::

:::error E0226

`map`에 바로 넘긴 람다 안에서 `?`를 썼다. 람다의 `?`는 람다에서 `Err`를 돌려줄 뿐이고, `map`은 그 `Err`를 결과 리스트의 원소로 넣는다. 바깥 함수는 실패를 모르고 지나간다.

```fung run exit=2 error=E0226
fn ports(xs) = Ok(xs |> map(s => parse_int(s)?))
```

```fung run
fn ports(xs) = try_map(xs, s => parse_int(s))
print(ports(["80", "443"]))
```

:::

:::error E0227

파이프 `|>` 오른쪽 호출의 인자에 `_`를 썼다. 파이프 값은 늘 첫 인자로 들어가므로 `x |> f(1, _)`는 `f(x, 1, _)`, 곧 부분 적용된 함수가 되어 버린다. 뜻한 것과 다르므로 막는다([파이프](language.html#pipe)).

```fung run exit=2 error=E0227
import path
print("a.txt" |> path.join("/tmp", _))
```

고치는 법: 다른 자리에 넣으려면 람다를 쓴다.

```fung run
import path
print("a.txt" |> (f => path.join("/tmp", f)))
```

:::

:::error E0228

나머지 매개변수 `...이름`에 `list`가 아닌 타입을 적었다. 나머지 매개변수는 남는 인자들을 모은 리스트라서, 다른 타입을 적으면 모든 호출이 실패한다.

흔한 원인: 원소의 타입을 적으려고 `...ns: int`라고 씀.

```fung run exit=2 error=E0228
fn total(...ns: int) = fold(ns, 0, (a, b) => a + b)
```

고치는 법: `list`나 `list[int]`로 적거나 아무것도 적지 않는다. `list[int]`의 원소 타입은 읽는 사람을 위한 표기이고 검사하지 않는다.

```fung run
fn total(...ns: list[int]) = fold(ns, 0, (a, b) => a + b)
print(total(1, 2, 3))
```

:::

## E03xx: 모듈과 import {#e03}

모듈(module)은 다른 파일에 둔 코드를 가져다 쓰는 단위다. 파일 하나가 모듈 하나이고, `import fs`처럼 표준 라이브러리는 이름으로, `import "./lib/x"`처럼 내 파일은 경로로 가져온다([모듈](language.html#modules)). 가져올 파일을 찾고 읽고 이름을 잇는 일은 실행 전에 하므로, E03xx도 컴파일 에러(종료 코드 2)다. 에러가 가져온 파일 안에서 났으면 위치도 그 파일을 가리킨다.

:::error E0301

가져올 모듈을 찾지 못했다.

흔한 원인: 내 파일을 경로 없이 이름으로 가져옴(`import util`; 이름은 표준 라이브러리만 찾는다), 경로 오타, 없는 표준 모듈 이름. 경로로 가져올 때는 메시지에 찾아본 절대 경로가 나온다.

```fung run exit=2 error=E0301
import util
```

고치는 법: 내 파일은 import하는 파일을 기준으로 한 상대 경로로 가져온다. `.fung`은 생략한다.

```file util.fung
fn shout(s) = upper(s) + "!"
```

```fung run
import "./util"
print(util.shout("hi"))
```

:::

:::error E0302

모듈들이 서로를 고리처럼 가져온다(순환 import). `a`를 읽으려면 `b`가 필요하고 `b`를 읽으려면 다시 `a`가 필요하니 끝나지 않는다. 메시지에 고리 전체가 나온다. 파일이 자기 자신을 가져와도 이 에러다.

```file a.fung
import "./b"

fn one() = 1
```

```file b.fung
import "./a"

fn two() = a.one() + 1
```

```fung run exit=2 error=E0302
import "./a"

print(a.one())
```


고치는 법: 둘이 함께 쓰는 부분을 세 번째 모듈로 떼어 내고, 두 모듈이 그것을 가져오게 한다.

```file shared.fung
fn one() = 1
```

```file a.fung
import "./shared"
fn one() = shared.one()
```

```file b.fung
import "./shared"
fn two() = shared.one() + 1
```

```fung run
import "./a"
import "./b"
print(a.one(), b.two())
```

:::

:::error E0303

모듈에 없는 이름을 꺼냈다. `fs.이름`으로 쓰든 `import "./x" (이름)`으로 가져오든 같다.

흔한 원인: 함수 이름 오타나 다른 언어의 이름(`fs.read_file`)을 씀. 모듈의 함수 목록은 [함수 목록](functions.html)에 있다.

```fung run exit=2 error=E0303
import fs
print(fs.read_file("notes.txt"))
```

```file notes.txt
hi
```

```fung run
import fs
print(fs.read("notes.txt")!)
```

:::

:::error E0304

모듈의 비공개 이름을 밖에서 꺼냈다. `_`로 시작하는 최상위 이름은 그 모듈 안에서만 쓰는 도우미라 내보내지 않는다.

```file greet.fung
fn hello(n) = "${_prefix()}${n}"
fn _prefix() = "hello, "
```

```fung run exit=2 error=E0304
import "./greet"

print(greet._prefix())
```


고치는 법: 공개된 함수를 쓴다. 정말 밖에서 써야 하면 그 모듈에서 이름의 `_`를 뗀다.

```file greet.fung
fn hello(n) = "${_prefix()}${n}"
fn _prefix() = "hello, "
```

```fung run
import "./greet"
print(greet.hello("fung"))
```

:::

:::error E0305

파일 이름이 fung의 이름 규칙에 맞지 않아 모듈 이름으로 쓸 수 없다. 모듈 이름은 파일 이름에서 오는데, `my-lib.fung`의 `my-lib`에는 `-`가 들어 있어 코드에서 부를 수 없다.

```fung run exit=2 error=E0305
import "./my-lib"
```

고치는 법: `as`로 쓸 수 있는 이름을 붙이거나 파일 이름을 `my_lib.fung`으로 바꾼다.

```file my-lib.fung
fn answer() = 42
```

```fung run
import "./my-lib" as my_lib
print(my_lib.answer())
```

:::

:::error E0306

가져오는 모듈의 최상위에 `import`, `fn`, `type`, `test`, 대입이 아닌 문장이 있다. 모듈은 처음 가져올 때 한 번 실행되는데, 그때 출력 같은 일이 일어나면 가져오는 것만으로 부작용이 생긴다.

흔한 원인: 실행 파일로 쓰던 파일을 모듈로 가져옴, 확인용 `print`를 남겨 둠.

```file config.fung
port = 8080
print("loading config")
```

```fung run exit=2 error=E0306
import "./config"

print(config.port)
```


고치는 법: 그 문장을 함수 안으로 옮기거나 실행 파일(진입 파일)로 옮긴다.

```file config.fung
port = 8080
fn describe() = "port ${port}"
```

```fung run
import "./config"
print(config.describe())
```

:::

:::error E0307

다른 모듈이 지금 실행하는 파일(진입 파일)을 가져오려 했다. 진입 파일은 모듈로 쓸 수 없다. (진입 파일이 자기 자신을 가져오면 E0302다.)

흔한 원인: 실행 파일에 둔 설정 값을 도우미 모듈이 쓰려 함.

```file helper.fung
import "./example"

fn twice(n) = n * 2
```

```fung run exit=2 error=E0307
import "./helper"

limit = 10
print(helper.twice(limit))
```


고치는 법: 함께 쓰는 값을 별도 모듈로 옮기거나 인자로 넘긴다.

```file settings.fung
limit = 10
```

```file helper.fung
import "./settings"
fn twice(n) = n * 2
fn doubled_limit() = twice(settings.limit)
```

```fung run
import "./helper"
print(helper.doubled_limit())
```

:::

:::error E0308

가져온 모듈의 이름에 값을 대입했다. 그러면 아래에서 `path.join`이 모듈의 함수인지 알 수 없게 된다.

흔한 원인: `path`, `time`, `json` 같은 흔한 낱말을 변수 이름으로 씀.

```fung run exit=2 error=E0308
import path
path = "/tmp/out"
```

도움말은 카탈로그에는 없고 이 경우에 붙는 것이다. 고치는 법: 변수 이름을 바꾸거나 모듈을 `as`로 다른 이름으로 가져온다.

```fung run
import path
out_path = "/tmp/out"
print(path.join(out_path, "a.txt"))
```

:::

:::error E0309

이미 모듈이 묶인 이름에 다른 모듈을 또 가져왔다. 같은 모듈을 두 번 가져와도, 이름이 같은 내 파일(`./fs.fung`)과 표준 모듈을 함께 가져와도 이 에러다.

```file fs.fung
fn x() = 1
```

```fung run exit=2 error=E0309
import fs
import "./fs"

print(1)
```


고치는 법: 중복된 줄을 지우거나 하나를 `as`로 다른 이름으로 가져온다.

```file fs.fung
fn x() = 1
```

```fung run
import fs
import "./fs" as local_fs
print(type_of(fs), local_fs.x())
```

:::

:::error E0310

`_native.`으로 시작하는 내부 모듈을 가져오려 했다. 이것은 표준 모듈이 안에서 쓰는 Rust 쪽 다리라서 직접 가져올 수 없다. 메시지가 대신 가져올 모듈을 알려 준다.

```fung run exit=2 error=E0310
import _native.fs
```

고치는 법: 메시지가 말하는 공개 모듈을 가져온다(`import fs`).

:::

:::error E0311

`import`를 함수나 블록 안에 썼다. `import`는 파일 최상위에만 둔다. 함수 안의 `import`가 무시되므로 아래의 이름도 함께 E0204가 된다.

```fung run exit=2 error=E0311
fn load(p):
  import fs
  fs.read(p)
;;
```

```fung run
import fs

fn load(p) = fs.read(p)
print(load("missing.txt"))
```

:::

## E04xx: 호출과 타입 표기 {#e04}

함수를 부를 때 인자가 맞지 않거나, 인자와 반환값이 적어 둔 타입과 다르면 E04xx다. fung은 동적 타입 언어라서 이것들은 실행 중에, 그 호출이 실제로 일어날 때 검사한다. 그래서 `fung check`로는 찾지 못하고, 종료 코드는 1이다. 메시지의 호출 경로가 어느 호출에서 났는지 알려 준다. 규칙은 [함수](language.html#functions)와 [타입 표기](language.html#types)에 있다.

:::error E0401

함수가 아닌 값을 함수처럼 불렀다. 메시지에 그 값의 타입이 나온다.

흔한 원인: 같은 이름의 변수가 함수를 가렸음(prelude 변수 `args`는 함수가 아니라 리스트다), 괄호를 잘못 붙임.

```fung run exit=1 error=E0401
name = "deploy"
print(name())
```

```fung run
fn deploy() = "deploying"
print(deploy())
```

:::

:::error E0402

위치 인자를 함수가 받는 것보다 많이 넘겼다. 메시지는 함수 이름, 받는 개수, 넘긴 개수를 적는다. 내장 함수도 같다.

```fung run exit=1 error=E0402
fn connect(host, port) = "${host}:${port}"
print(connect("db1", 5432, 10s))
```

고치는 법: 남는 인자를 지우거나, 함수에 기본값 있는 매개변수를 더한다.

```fung run
fn connect(host, port, wait = 5s) = "${host}:${port} (${wait})"
print(connect("db1", 5432, 10s))
```

:::

:::error E0403

인자가 매개변수에 적은 타입이 아니다. 위치는 그 매개변수를 가리키고, 호출 경로의 두 번째 줄이 누가 불렀는지 보여 준다.

흔한 원인: 명령줄 인자나 파일에서 읽은 문자열을 숫자로 바꾸지 않고 넘김, 열거형 별칭(`type color = Red | Green`)에 없는 태그를 넘김.

```fung run exit=1 error=E0403
fn backoff(n: int) = n * 2
print(backoff("3"))
```

고치는 법: 넘기기 전에 값을 바꾼다. 문자열은 `parse_int`로 숫자가 된다.

```fung run
fn backoff(n: int) = n * 2
print(backoff(parse_int("3")!))
```

:::

:::error E0404

함수가 돌려준 값이 `->` 뒤에 적은 반환 타입이 아니다.

흔한 원인: 문자열 그대로 돌려줌, 어떤 가지에서만 다른 타입을 돌려줌. 자기 자신을 꼬리 호출하는 함수는 반복이 끝난 뒤 한 번 검사한다.

```fung run exit=1 error=E0404
fn port_of(url) -> int = split(url, ":")[-1]
print(port_of("db1:5432"))
```

```fung run
fn port_of(url) -> int = parse_int(split(url, ":")[-1])!
print(port_of("db1:5432"))
```

:::

:::error E0405

같은 매개변수를 위치 인자와 이름 인자로 두 번 주었다.

```fung run exit=1 error=E0405
fn connect(host, port) = "${host}:${port}"
print(connect("db1", host: "db2"))
```

고치는 법: 위치 인자는 앞에서부터 채워지므로, 이름 인자로는 아직 채우지 않은 매개변수를 준다.

```fung run
fn connect(host, port) = "${host}:${port}"
print(connect("db1", port: 5432))
```

:::

:::error E0406

함수에 없는 이름의 인자를 주었다.

흔한 원인: 매개변수 이름의 오타.

```fung run exit=1 error=E0406
fn connect(host, port = 22) = "${host}:${port}"
print(connect("db1", prot: 2222))
```

```fung run
fn connect(host, port = 22) = "${host}:${port}"
print(connect("db1", port: 2222))
```

:::

:::error E0407

기본값이 없는 매개변수에 인자를 주지 않았다.

흔한 원인: 인자 빠뜨림, 함수의 첫 매개변수를 잊음(`spawn(f)`는 `spawn(s, f)`의 스코프 `s`가 빠진 것이라 E0407이다).

```fung run exit=1 error=E0407
fn connect(host, port) = "${host}:${port}"
print(connect("db1"))
```

고치는 법: 인자를 넘기거나, 흔한 값이 있으면 기본값을 준다.

```fung run
fn connect(host, port = 22) = "${host}:${port}"
print(connect("db1"))
```

:::

## E05xx: 런타임 연산 {#e05}

산술, 비교, 인덱스, 변환 같은 연산이 받은 값으로는 할 수 없을 때 E05xx다. fung은 값을 조용히 다른 타입으로 바꾸지 않는다(`"a" + 1`은 `"a1"`이 아니라 에러). 숫자가 넘치거나 0으로 나누는 것도 이상한 값을 만들어 계속 가는 대신 그 자리에서 멈춘다. 모두 실행 중의 에러라 종료 코드는 1이다([값과 타입](language.html#values), [연산자](language.html#operators)).

:::error E0501

이항 연산자가 받은 두 값의 타입 조합으로는 계산할 수 없다. 메시지는 연산(`add`, `subtract` …)과 두 타입을 적는다.

흔한 원인: 문자열과 숫자를 `+`로 이음, `nil`(없는 값)로 계산함, 기간을 정수로 `//` 나눔.

```fung run exit=1 error=E0501
count = 3
print("found " + count + " files")
```

고치는 법: 문자열 보간 `${…}`을 쓰거나 `str`로 바꾼다.

```fung run
count = 3
print("found ${count} files")
print("found " + str(count) + " files")
```

:::

:::error E0502

0으로 나눴다. `/`, `//`, `%` 모두, 기간끼리의 나눗셈도 같다.

흔한 원인: 빈 리스트의 평균을 구함.

```fung run exit=1 error=E0502
fn average(xs) = sum(xs) / len(xs)
print(average([]))
```

고치는 법: 나누기 전에 0인 경우를 따로 처리한다.

```fung run
fn average(xs) = len(xs) == 0 then 0 else sum(xs) / len(xs)
print(average([]), average([2, 4]))
```

:::

:::error E0503

정수 계산 결과가 64비트 정수의 범위(약 ±9.2×10<sup>18</sup>)를 넘었다. fung은 넘친 값을 조용히 감아 돌리지 않고 멈춘다.

흔한 원인: 큰 거듭제곱(`2 ** 64`), 끝없이 커지는 누적.

```fung run exit=1 error=E0503
print(9223372036854775807 + 1)
```

고치는 법: 값의 범위를 미리 확인한다. 정확도가 조금 떨어져도 괜찮으면 실수로 계산한다.

```fung run
fn add_safe(a, b) = a > 9223372036854775807 - b then Err("too big") else Ok(a + b)
print(add_safe(9223372036854775807, 1))
```

:::

:::error E0504

실수 계산 결과가 유한한 수가 아니다(무한대나 NaN). fung은 이런 값을 만들지 않고 그 자리에서 멈춘다.

```fung run exit=1 error=E0504
print(1e308 * 10)
```

고치는 법: 계산 전에 입력의 범위를 확인한다. 실수를 0.0으로 나누는 것은 E0502다.

:::

:::error E0505

조건 자리(`if`, `then`, `when`, `and`·`or`·`not`의 피연산자)에 `bool`이 아닌 값이 왔다. fung은 0, 빈 리스트, 빈 문자열을 거짓으로 보지 않는다.

흔한 원인: 다른 언어처럼 `if len(xs):`로 "비어 있지 않음"을 검사함.

```fung run exit=1 error=E0505
xs = [1, 2]
if len(xs): print("not empty") ;;
```

고치는 법: 비교를 직접 쓴다. 빈 컬렉션 검사는 `is_empty`도 있다.

```fung run
xs = [1, 2]
if len(xs) > 0: print("not empty") ;;
```

:::

:::error E0506

연산이나 내장 함수가 그 타입의 값에는 할 수 없는 일이다. 메시지가 하려던 일과 타입을 적는다(`cannot negate str`, `cannot take the length of int`, `cannot map over str` 등).

흔한 원인: 문자열에 단항 `-`, 숫자에 `len`, 문자열에 리스트 함수(`first("abc")`; 문자열은 `chars`로 리스트로 바꾼다).

```fung run exit=1 error=E0506
print(-"5")
```

```fung run
print(-parse_int("5")!)
print(first(chars("abc")))
```

:::

:::error E0507

인덱스가 범위를 벗어났다. 리스트와 문자열의 인덱스는 0부터 `길이 - 1`까지이고, 음수는 뒤에서부터 센다(`-1`이 마지막). 슬라이스의 끝이 길이를 넘어도 이 에러다.

```fung run exit=1 error=E0507
hosts = ["web1", "web2"]
print(hosts[2])
```

고치는 법: 길이를 먼저 확인한다. 마지막 원소는 `xs[-1]`, 있는 만큼만 원하면 `take`·`drop`을 쓴다.

```fung run
hosts = ["web1", "web2"]
print(hosts[-1], len(hosts) > 2 then hosts[2] else "none")
```

:::

:::error E0508

맵에 없는 키를 읽었다(`m.k`나 `m["k"]`).

흔한 원인: 선택 항목인 설정 키, JSON 응답에 없는 필드.

```fung run exit=1 error=E0508
cfg = {host: "db1"}
print(cfg.port)
```

고치는 법: 기본값이 필요하면 `get(m, k, 기본값)`을 쓰거나, `get(m, k)`가 없는 키에 돌려주는 `nil`에 `??`를 붙인다. 있는지만 보려면 `k in m`이다. 주의: `cfg.port ?? 5432`는 `cfg.port`를 읽는 순간 이 에러로 멈추므로 기본값이 되지 않는다. 도움말의 `??`는 `get`과 짝지어 쓰는 것이다.

```fung run
cfg = {host: "db1"}
print(get(cfg, "port", 5432), get(cfg, "port") ?? 5432, "port" in cfg)
```

:::

:::error E0509

인덱스 `x[i]`를 쓸 수 없는 값에 썼다. 인덱스는 리스트, 문자열, 맵에만 된다.

흔한 원인: 튜플을 인덱스로 꺼냄. 튜플의 원소는 패턴으로만 꺼낸다.

```fung run exit=1 error=E0509
pair = ("db1", 5432)
print(pair[0])
```

```fung run
pair = ("db1", 5432)
(host, port) = pair
print(host, port)
```

:::

:::error E0511

슬라이스 `x[a..b]`를 쓸 수 없는 값에 썼다. 슬라이스는 리스트와 문자열에만 된다.

```fung run exit=1 error=E0511
print((1, 2, 3)[0..2])
```

고치는 법: 튜플이라면 패턴으로 꺼내거나 처음부터 리스트로 만든다.

```fung run
print([1, 2, 3][0..2])
```

:::

:::error E0512

`in`의 오른쪽이 포함 여부를 검사할 수 없는 값이다. 리스트(원소), 맵(키), 문자열(부분 문자열), 범위(정수)만 된다.

```fung run exit=1 error=E0512
print(3 in 10)
```

```fung run
print(3 in 0..10, 3 in [1, 2, 3])
```

:::

:::error E0513

값을 다른 타입으로 바꿀 수 없다. 메시지가 값과 바꾸려던 타입을 적는다.

흔한 원인: 숫자가 아닌 문자열을 `int`로 바꿈, 공백이 든 문자열을 `sym`으로 바꿈, JSON으로 나타낼 수 없는 값(기간, 태그 등)을 `json.stringify`에 넘김.

```fung run exit=1 error=E0513
print(int("8080x"))
```

고치는 법: 실패할 수 있는 입력은 `Ok`/`Err`를 돌려주는 `parse_int`·`parse_float`로 바꾸고 결과를 다룬다. JSON에는 문자열이나 숫자로 바꿔서 넣는다.

```fung run
import json
print(parse_int("8080x"))
print(json.stringify({wait: str(5s)}))
```

:::

:::error E0514

문자열이 아닌 값을 맵의 키로 쓰려 했다. fung의 맵 키는 늘 문자열이다.

흔한 원인: `group_by`의 키 함수가 숫자나 심볼을 돌려줌.

```fung run exit=1 error=E0514
print(group_by([1, 2, 3, 4], x => x % 2))
```

```fung run
print(group_by([1, 2, 3, 4], x => str(x % 2)))
```

:::

:::error E0515

정규식 패턴이 잘못되었다. 메시지에 패턴과 무엇이 틀렸는지가 붙는다.

흔한 원인: 괄호·대괄호를 닫지 않음, 글자 그대로 찾으려던 `(` `.` `*`를 이스케이프하지 않음.

```fung run exit=1 error=E0515
import regex
print(regex.is_match("a(b", "(b"))
```

고치는 법: 특수 문자는 `\`로 이스케이프한다. 정규식은 백틱 원시 문자열로 쓰면 `\`를 두 번 쓰지 않아도 된다.

```fung run
import regex
print(regex.is_match("a(b", `\(b`))
```

:::

:::error E0516

슬라이스의 시작이 끝보다 뒤에 있다.

```fung run exit=1 error=E0516
xs = [1, 2, 3, 4]
print(xs[3..1])
```

고치는 법: 시작과 끝의 순서를 고친다. 끝이 길이를 넘는 것은 E0507이고, 있는 만큼만 원하면 `take`·`drop`을 쓴다.

```fung run
xs = [1, 2, 3, 4]
print(xs[1..3], take(drop(xs, 2), 10))
```

:::

:::error E0517

심볼과 문자열을 비교했다. 둘은 다른 타입이라 늘 다르다. 그런데 JSON, 파일, 명령 출력에서 온 값은 늘 문자열이라 `status == 'ok`가 조용히 거짓이 되는 함정이 흔해서, 이 비교를 에러로 만들었다. 리스트·맵 안에서 맞붙어도, `in`도 같다.

```fung run exit=1 error=E0517
import json
back = json.parse(`{"status": "ok"}`)!
print(back.status == 'ok)
```

고치는 법: 문자열 쪽을 `sym(s)`로 바꾸거나, 비교 대상을 문자열로 쓴다.

```fung run
import json
back = json.parse(`{"status": "ok"}`)!
print(sym(back.status) == 'ok, back.status == "ok")
```

:::

## E06xx: 패턴과 결과값 {#e06}

`match`와 구조 분해 대입은 값을 패턴(pattern)에 맞춰 본다. 맞는 패턴이 없으면 E0601·E0602다. fung은 실패할 수 있는 일의 결과를 예외(exception)로 던지지 않고 `Ok(값)`·`Err(내용)`이라는 값으로 돌려준다. 이 결과값을 `!`·`?`로 잘못 다루거나 확인하지 않고 버리면 E0603~E0606이다([match와 패턴](language.html#match), [결과값과 에러](language.html#results)). 모두 실행 중의 에러다.

:::error E0601

`match`의 어떤 절도 값에 맞지 않았다. 메시지에 맞지 않은 값이 나온다.

흔한 원인: 몇 가지 값만 적고 나머지 경우를 잊음.

```fung run exit=1 error=E0601
fn status_text(code) = match code:
  200 => "ok"
  404 => "not found"
;;
print(status_text(500))
```

고치는 법: 마지막에 나머지를 받는 절(`_ =>`나 이름 하나)을 둔다.

```fung run
fn status_text(code) = match code:
  200 => "ok"
  404 => "not found"
  c   => "other (${c})"
;;
print(status_text(500))
```

:::

:::error E0602

구조 분해 대입의 왼쪽 패턴에 값이 맞지 않는다. 메시지에 실제 값이 나온다.

흔한 원인: `split` 결과의 개수가 생각과 다름, 튜플의 원소 개수가 다름.

```fung run exit=1 error=E0602
line = "web1 10.0.0.1"
[name, addr, role] = split(line, " ")
```

고치는 법: 개수가 달라질 수 있으면 `...rest`로 나머지를 받거나 `match`로 경우를 나눈다.

```fung run
line = "web1 10.0.0.1"
[name, addr, ...rest] = split(line, " ")
print(name, addr, rest)
```

:::

:::error E0603

`!`가 `Err`를 만났다. `x!`는 "실패하면 프로그램을 멈춘다"는 뜻이라, 이것은 의도한 멈춤이다. 메시지에 `Err`의 내용(표준 라이브러리면 `message`)이 나온다.

```fung run exit=1 error=E0603
import fs
text = fs.read("config.json")!
```

고치는 법: 실패를 정말 멈춤으로 다루려면 그대로 둔다. 어떤 실패는 넘어가야 하면 `match`로 종류(`kind`)를 가려 처리한다.

```fung run
import fs
text = match fs.read("config.json"):
  Ok(t)                   => t
  Err({kind: 'not_found}) => "{}"
  Err(e)                  => fail(e.message)
;;
print(text)
```

:::

:::error E0604

`nil`에 `!`를 썼다. `!`는 `Ok`/`Err`를 받는 연산자인데, `nil`은 "없음"이지 실패가 아니다.

흔한 원인: `nil`을 돌려줄 수 있는 함수(`get`, `find`, `first` 등)의 결과에 `!`를 붙임.

```fung run exit=1 error=E0604
cfg = {host: "db1"}
port = get(cfg, "port")!
```

고치는 법: `nil`에는 `??`로 기본값을 준다.

```fung run
cfg = {host: "db1"}
port = get(cfg, "port") ?? 5432
print(port)
```

:::

:::error E0605

`!`·`?`나 그 규칙을 따르는 함수(`try_map`)가 `Ok`도 `Err`도 아닌 값을 받았다. 메시지 앞의 이름이 연산자나 함수다.

흔한 원인: 실패하지 않는 함수(`len` 등)에 `!`를 붙임, `try_map`에 결과값이 아닌 값을 돌려주는 함수를 줌(그때는 `map`을 쓴다).

```fung run exit=1 error=E0605
print(try_map([1, 2], x => x * 2))
```

```fung run
print(try_map(["1", "2"], parse_int), map([1, 2], x => x * 2))
```

:::

:::error E0606

값을 쓰지 않는 자리에서 `Err`가 나와 버려졌다. 실패가 아무도 모르게 지나가지 않도록 fung은 버려지는 `Err`를 에러로 만든다. 셸의 `set -e`를 늘 켜 둔 것과 같다.

버려지는 자리: 문장으로 쓴 호출, 그런 자리의 `if`·`match` 가지의 끝, `for` 문장의 본문, 파일의 마지막 식, `each`에 준 함수, `with`의 `release`, `supervise`의 `on_fail`. 외부 명령이 실패해도 `exec`가 `Err`를 돌려주므로 같다.

```fung run exit=1 error=E0606
import fs
fs.write("missing/out.txt", "data")
print("saved")
```

고치는 법: `!`(실패하면 멈춤), `?`(함수 안에서 호출자에게 넘김), `match`(경우마다 처리)로 다룬다. 실패해도 상관없으면 `_ = 식`으로 일부러 버린다.

```fung run
import fs
match fs.write("missing/out.txt", "data"):
  Ok(_)  => print("saved")
  Err(e) => print("skip:", e.kind)
;;
_ = fs.remove("missing/old.txt")
print("done")
```

:::

## E07xx: 동시성 {#e07}

동시성(concurrency)은 여러 일(태스크, task)을 번갈아 진행하는 것이다. fung의 태스크는 `scope` 안에서 `spawn`으로 띄우고, 채널(channel)로 값을 주고받고, 여럿이 함께 보는 값은 `atom`에 둔다([동시성 내장 함수](prelude/concurrency.html)). 이 규칙을 어기거나 모든 태스크가 서로를 기다리며 멈추면 E07xx다. 모두 실행 중의 에러다.

:::error E0701

태스크가 취소되었다. 취소는 보통 fung 안에서 조용히 처리된다: 스코프가 끝나거나 다른 태스크가 실패하면 남은 태스크를 취소하고, 이 취소는 에러로 찍지 않는다. `fung run`에서는 이 번호를 거의 보지 않는다.

보이는 경우: `fung test`에서 테스트 블록 안의 태스크가 `exit(n)`을 불러 기다리던 쪽이 취소되면, 그 테스트의 실패 이유로 나온다. `fung run`에서 보이지 않으므로 아래 출력은 `fung test example.fung`을 실제로 돌려 옮겨 적었다.

```fung frag
test "a task exits":
  scope(fn(s):
    t = spawn(s, () => exit(3))
    await(t)
  ;;)
;;
```

```plain
FAIL example.fung: a task exits
  error[E0701]: task cancelled
    --> example.fung:4:10
    |
  4 |     await(t)
    |          ^
    |
   call path (innermost first):
     example.fung:4 <lambda>
     example.fung:2 test: a task exits
0 passed, 1 failed
```

고치는 법: 테스트 안에서는 `exit`를 부르지 않는다. 끝낼 이유는 `Err`나 `fail`로 돌려준다.

:::

:::error E0702

교착(deadlock): 모든 태스크가 무언가를 기다리고 있어서 아무도 더 나아갈 수 없다. fung은 영원히 멈춰 있는 대신 이것을 알아채고 에러로 끝낸다. 상황에 따라 도움말이 원인을 짚어 준다.

흔한 원인: 받는 쪽이 보내는 끝을 자기도 쥐고 있어서 채널이 닫히지 않음, 버퍼 없는 채널(`channel()`)에 받는 태스크 없이 보냄.

```fung run exit=1 error=E0702
(tx, rx) = channel()
print(recv(rx))
```

`channel()`은 버퍼가 없어서 받는 쪽이 준비될 때까지 `send`가 기다린다. 같은 태스크가 보내고 나서 받으려 하면 그것도 교착이다.

```fung run exit=1 error=E0702
(tx, rx) = channel()
send(tx, "job")!
close(tx)
for job in rx: print(job) ;;
```

고치는 법: 보내기는 다른 태스크에 맡기고, 보내는 끝은 보내는 쪽만 쥔다. 함수 안의 변수는 마지막으로 읽힌 뒤 놓이므로, 보내는 끝을 태스크에만 넘기면 그 태스크가 끝날 때 채널이 닫혀 `for`가 끝난다. 혼자 보내고 받아야 하면 버퍼가 있는 `channel(n)`을 쓰고 다 보낸 뒤 `close`한다.

```fung run
fn collect() = scope(fn(s):
  (tx, rx) = channel()
  spawn(s, fn():
    send(tx, 1)!
    send(tx, 2)!
  ;;)
  for x in rx: x ;;
;;)
print(collect())
```

:::

:::error E0703

쓰지 않는 번호다. 예전 버전의 `atom`이 자기 `swap` 안에서 다시 바뀔 때 내던 에러였다. `atom`이 한때 에이전트(agent)가 되면서 E0704로 바뀌었고, 지금은 E0707이 그 경우를 맡는다. 번호는 한 번 붙이면 다른 뜻으로 다시 쓰지 않으므로 자리만 남아 있다. 지금의 fung에서는 나오지 않아 예가 없다.

:::

:::error E0704

쓰지 않는 번호다. 에이전트를 자기 `swap` 안에서 부를 때 내던 에러였다. 에이전트가 다시 엄격한 `atom`이 되면서 이 경우는 E0707(`swap`의 함수 안의 효과)로 잡힌다. 지금의 fung에서는 나오지 않아 예가 없다.

:::

:::error E0705

채널의 끝을 같은 채널로 보냈다. 그러면 채널이 자기 자신을 붙잡게 되어 영원히 닫히지 않는다. 리스트나 맵 안에 넣어 보내도 같다.

```fung run exit=1 error=E0705
(tx, rx) = channel(1)
send(tx, rx)
```

고치는 법: 답을 받을 통로가 필요하면 다른 채널을 만들어 그 끝을 보낸다.

```fung run
(tx, rx) = channel(1)
(reply_tx, reply_rx) = channel(1)
send(tx, reply_tx)!
send(recv(rx)!, "pong")!
print(recv(reply_rx))
```

:::

:::error E0706

이미 끝난 스코프에 태스크를 띄웠다. 스코프는 그 안의 태스크가 모두 끝나야 닫히는 울타리인데, 닫힌 뒤에 들어오는 태스크는 기다려 줄 사람이 없다.

흔한 원인: `scope`의 함수가 스코프 값 `s`를 돌려주고 밖에서 그것으로 `spawn`함.

```fung run exit=1 error=E0706
s = scope(s => s)
spawn(s, () => print("late"))
```

고치는 법: `spawn`은 스코프의 함수 안에서 부른다.

```fung run
scope(fn(s):
  spawn(s, () => print("inside"))
;;)
```

:::

:::error E0707

`swap`에 준 함수 안에서 효과(effect)가 있는 함수를 불렀다. 효과란 출력, 채널, 파일, 대기, 다른 atom의 변경처럼 바깥에 흔적을 남기는 일이다. `swap`의 함수는 옛 값을 받아 새 값을 계산하기만 해야 한다. 다른 태스크가 그사이 atom을 바꾸면 `swap`이 함수를 새 값으로 다시 돌리기 때문에, 효과가 있으면 두 번 일어날 수 있다. 다른 atom을 바꾸는 것(`reset`, `swap`)도 효과이고, 자기 `swap` 안에서 같은 atom을 다시 `swap`하는 것도 이 에러다.

```fung run exit=1 error=E0707
hits = atom(0)
swap(hits, n => dbg(n) + 1)
```

고치는 법: 효과는 `swap` 밖에서 한다. `swap`은 새 값을 돌려준다.

```fung run
hits = atom(0)
new = swap(hits, n => n + 1)
print("hits:", new)
```

:::

:::error E0708

atom에 담을 수 없는 값을 넣었다. atom은 평범한 데이터(숫자, 문자열, 리스트, 맵 …)만 담는다. 다른 atom, 채널의 끝, 태스크, 스코프, 파일, 소켓은 상태가 있는 자원이라 담을 수 없다.

```fung run exit=1 error=E0708
(tx, rx) = channel()
latest = atom(tx)
```

고치는 법: 데이터만 atom에 둔다. 자원은 변수나 인자로 넘긴다.

```fung run
latest = atom({host: "web1", up: true})
swap(latest, m => put(m, "up", false))
print(deref(latest))
```

:::

## E08xx: 한도와 환경 {#e08}

나머지는 fung 자체의 한도(너무 깊은 중첩, 너무 깊은 재귀, 너무 큰 문자열), 직접 내는 에러(`fail`, `assert`), 실행 순서와 관련된 에러다. E0802와 E0806의 인자·매개변수 개수 한도는 실행 전에 찾는 컴파일 에러(종료 코드 2)이고, 나머지는 실행 중의 에러(종료 코드 1)다.

:::error E0801

fung 내부의 오류다. 사용자 코드가 아니라 fung 구현의 버그를 뜻한다. 정상적인 프로그램으로는 일부러 낼 수 없으므로 예가 없다. 이 에러를 보면 메시지와 그것을 낸 코드를 저장소의 이슈로 알려 주기 바란다.

:::

:::error E0802

식이나 패턴이 너무 깊이 중첩되었다(한도 10,000단계). 파서가 끝없는 중첩으로 메모리를 다 쓰지 않도록 둔 한도이고, 실행 전에 찾는 컴파일 에러(종료 코드 2)다. 사람이 쓰는 코드에서는 거의 나오지 않고, 프로그램이 만든 코드에서 날 수 있다. 예는 괄호를 만 개 넘게 겹쳐야 해서 싣지 않는다. `x = ((((…1…))))`처럼 괄호를 10,001겹 쓴 파일은 `error[E0802]: expression nesting too deep (limit 10000)`으로 멈춘다.

고치는 법: 중첩을 풀어 중간값을 변수에 담는다.

:::

:::error E0803

`fail(메시지)`가 낸 에러다. 메시지가 그대로 에러의 메시지가 된다. "여기 오면 버그"라는 곳에서 직접 부르고, 일부 표준 라이브러리 함수도 잘못 쓰였을 때 이 번호로 알린다(예: `get`에 맵이 아닌 값을 주면 `get expects a map, got 'list`). 띄운 태스크가 `fail`하면 메시지에 `note: scope opened at …`이 붙어 그 태스크의 스코프를 알려 준다.

```fung run exit=1 error=E0803
fn port_of(cfg) = cfg.port ?? fail("config.port is required")
print(port_of({port: nil}))
```

고치는 법: 메시지가 말하는 조건을 맞춘다. 실패가 정상적인 경우의 하나라면 `fail` 대신 `Err`를 돌려주거나 기본값을 쓴다.

```fung run
fn port_of(cfg) = cfg.port ?? 8080
print(port_of({port: nil}))
```

:::

:::error E0804

꼬리 호출이 아닌 재귀가 너무 깊어졌다(한도 100,000단계). 호출 경로의 가운데는 `… n more`로 줄인다.

흔한 원인: 큰 입력에 `1 + f(n - 1)` 꼴의 재귀를 씀. 더하기가 재귀 호출이 돌아온 뒤에 일어나므로 호출이 하나씩 쌓인다.

```fung run exit=1 error=E0804
fn depth(n) = n == 0 then 0 else 1 + depth(n - 1)
print(depth(200_000))
```

고치는 법: 누적값을 인자로 넘겨 꼬리 호출로 바꾼다. 꼬리 호출은 프레임을 다시 쓰므로 깊이 한도가 없다. 리스트를 도는 것이라면 `fold`를 쓴다.

```fung run
fn depth(n, acc = 0) = n == 0 then acc else depth(n - 1, acc + 1)
print(depth(200_000))
```

:::

:::error E0805

쓰지 않는 번호다. 구현을 옮기던 동안 아직 만들지 않은 기능을 부르면 내던 자리표시 에러였고, 지금은 모든 내장 함수가 구현되어 나오지 않는다. 그래서 예가 없다.

:::

:::error E0806

fung의 크기 한도를 넘었다. 메시지가 무엇이(`characters`, `elements`, `arguments`, `parameters`) 한도를 넘었는지 적는다. 문자열은 1억 글자, `map` 등이 만드는 리스트는 1억 원소가 한도이고, 실행 중에 결과를 만들기 전에 검사하므로 메모리를 다 쓰기 전에 멈춘다(종료 코드 1). 한 호출의 인자와 한 함수의 매개변수는 4,095개까지이고, 이것은 컴파일할 때 검사한다(종료 코드 2).

```fung run exit=1 error=E0806
line = repeat("=", 200_000_000)
```

고치는 법: 한 번에 거대한 값을 만들지 말고 나눠서 처리한다(파일은 줄 단위로 읽고 쓰는 식).

```fung run
line = repeat("=", 20)
print(line)
```

:::

:::error E0807

`assert 식`의 식이 거짓이었다. 비교였다면 메시지 아래에 양쪽 식의 실제 값이 찍힌다. `assert`는 테스트 블록 밖에서도 쓰이고, 그때는 프로그램을 멈춘다.

```fung run exit=1 error=E0807
fn half(n) = n // 2
assert half(5) == 3
```

고치는 법: 찍힌 값을 보고 코드나 기대값 중 틀린 쪽을 고친다.

```fung run
fn half(n) = n // 2
assert half(5) == 2
print("ok")
```

:::

:::error E0808

이전에 실패한 대입의 이름을 읽었다. REPL(대화형 실행)에서만 나온다. REPL에서는 한 줄이 실패해도 세션이 이어지는데, 그 줄이 대입하려던 이름은 값이 없는 채로 남는다. 파일 실행에서는 대입이 실패하면 프로그램이 바로 멈추므로 이 에러가 나오지 않는다. 아래는 `fung`을 그냥 실행해 REPL에서 두 줄을 입력한 실제 화면이다(`>`가 입력 줄이다).

```plain
> x = 1 / 0
error[E0502]: division by zero
  --> <repl>:1:7
  |
1 | x = 1 / 0
  |       ^
  |
 call path (innermost first):
   <repl>:1   <entry>
> x
error[E0808]: `x` has no value: the statement that assigned it failed
  --> <repl>:1:1
  |
1 | x
  | ^
  |
 call path (innermost first):
   <repl>:1   <entry>
```

고치는 법: 실패한 대입을 고쳐 다시 입력한다.

:::

:::error E0809

파일이 시작하는 동안, 최상위 상수의 대입이 실행되기 전에 그 상수를 읽는 함수를 불렀다. 최상위 `fn`은 시작 전에 모두 만들어지지만, 상수는 위에서 아래로 차례대로 대입된다.

```fung run exit=1 error=E0809
x = twice()
limit = 10
fn twice() = limit * 2
print(x)
```

고치는 법: 도움말대로 대입을 위로 올리거나 호출을 아래로 내린다.

```fung run
limit = 10
fn twice() = limit * 2
x = twice()
print(x)
```

:::

## W01xx: 경고 {#w01}

경고는 틀렸다고 단정할 수는 없지만 실수일 가능성이 큰 코드를 알려 준다. 프로그램을 멈추지 않고 종료 코드도 바꾸지 않는다. 경고는 `fung check`와 `fung test`가 찍는다. `fung run`은 경고를 찍지 않고 그대로 실행한다. 그래서 아래 예의 출력은 `fung check example.fung`을 실제로 돌려 옮겨 적었다.

:::error W0101

가져오고 한 번도 쓰지 않은 모듈이 있다.

```fung frag
import fs
import path
print(path.join("a", "b"))
```

```plain
warning[W0101]: unused import `fs`
  --> example.fung:1:1
  |
1 | import fs
  | ^^
  |
```

고치는 법: 쓰지 않는 `import` 줄을 지운다.

:::

:::error W0102

함수 안에서 대입하고 한 번도 읽지 않은 변수가 있다. 계산을 해 놓고 결과를 쓰는 것을 잊었을 가능성이 크다.

```fung frag
fn total(xs):
  count = len(xs)
  fold(xs, 0, (a, b) => a + b)
;;
print(total([1, 2]))
```

```plain
warning[W0102]: unused variable `count`
  --> example.fung:2:3
  |
2 |   count = len(xs)
  |   ^^^^^
  |
 help: prefix it with `_` if it is intentional
```

고치는 법: 쓰지 않는 대입을 지운다. 일부러 쥐고 있는 값이라면(예: 채널의 끝을 함수가 끝날 때까지 살려 둠) 이름을 `_`로 시작한다(`_count`).

:::

:::error W0103

변수나 매개변수가 같은 이름의 prelude 함수를 가린다. 그 아래에서는 그 이름이 내장 함수가 아니라 내 변수를 가리키므로, 나중에 내장 함수를 부르려 하면 헷갈린다.

```fung frag
fn summary(xs):
  max = 0
  len(xs) + max
;;
print(summary([1]))
```

```plain
warning[W0103]: `max` shadows a prelude function
  --> example.fung:2:3
  |
2 |   max = 0
  |   ^^^
  |
```

고치는 법: 다른 이름(`biggest`, `limit` 등)을 쓴다.

:::

:::error W0104

쓰지 않는 번호다. 값으로 쓰는 블록이 대입으로 끝날 때 내던 경고였는데, 지금은 에러 E0224가 되었다. 나오지 않으므로 예가 없다.

:::
