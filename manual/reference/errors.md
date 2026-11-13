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

1. **첫 줄** `error[E0403]: …`: 에러 번호와 한 줄 설명. 번호로 이 쪽의 항목을 찾는다.
2. **위치** `--> example.fung:1:12`: 파일 이름, 줄, 칸(열). 칸은 1부터 센다.
3. **소스 발췌**: 그 줄을 그대로 보이고 아래에 `^`로 문제의 자리를 가리킨다. `^` 옆에 짧은 설명(예: `not found in this scope`)이 붙기도 한다.
4. **호출 경로** `call path (innermost first)`: 실행 중의 에러에만 붙는다. 에러가 난 함수부터 바깥쪽으로, 누가 누구를 불렀는지 보여 준다. `<entry>`는 실행한 파일의 최상위, `<lambda>`는 이름 없는 함수다. 아주 깊은 재귀는 가운데를 `… n more`로 줄인다.
5. **도움말** `help: …`: 고치는 방법의 힌트. 카탈로그에 적힌 문구가 기본이고, 상황에 맞춰 바뀌기도 한다(비슷한 이름을 알려 주는 E0204처럼). 아래 각 번호의 "기본 도움말"이 그 기본 문구이고, 기본이 없는 번호도 상황에 따라 도움말이 붙을 수 있다.
6. **덧붙임**: `note: …` 줄이나 `assert`가 비교한 양쪽 값(E0807)처럼 그 에러에만 있는 정보가 더 붙을 수 있다.

컴파일 에러(실행 전에 찾은 에러)에는 호출 경로가 없다. 아직 아무것도 실행하지 않았기 때문이다.

```fung run exit=2 error=E0204
files = ["a.log", "b.log"]
print(len(fiels))
```

이 쪽의 에러 예는 모두 페이지를 만들 때 실제로 돌린 출력이 붙는다(출력 칸 머리의 종료 코드도 실제 값이다). `fung test`·REPL에서만 나는 에러만 그 명령의 실제 출력을 옮겨 적었다. 고친 코드도 실제로 실행된다.

## 종료 코드 {#exit-codes}

프로그램이 끝나면 운영 체제에 종료 코드(exit code)라는 숫자를 남긴다. 셸에서는 `echo $?`로 본다. fung의 종료 코드는 다음과 같다(자세한 것은 [명령줄 도구](cli.html#errors)).

<table>
  <thead><tr><th>종료 코드</th><th>뜻</th></tr></thead>
  <tbody>
    <tr><td><code>0</code></td><td>정상 종료. 경고만 있으면 0이다.</td></tr>
    <tr><td><code>1</code></td><td>실행 중 실패: 런타임 에러, <code>!</code>가 <code>Err</code>를 만남, 아무도 확인하지 않은 태스크의 실패, 테스트 실패.</td></tr>
    <tr><td><code>2</code></td><td>컴파일 실패: 문법·이름·import 에러(E01xx~E03xx). 아무것도 실행하지 않는다.</td></tr>
    <tr><td><code>3</code></td><td><code>fung</code> 명령을 잘못 썼다(없는 명령, 읽을 수 없는 파일 등).</td></tr>
    <tr><td><code>n</code></td><td>스크립트가 <code>exit(n)</code>을 불렀다.</td></tr>
  </tbody>
</table>

`fung check`는 실행하지 않고 컴파일 단계만 검사하므로 종료 코드 2의 에러와 경고만 찾는다. 인자 개수나 타입(E04xx), 0으로 나누기(E05xx) 같은 것은 실행해야 드러난다.

## 찾아보기 {#index}

{{errors}}

## E01xx: 문법(렉서·파서) {#e01}

소스는 먼저 렉서(lexer)가 글자를 토큰(token, 숫자·이름·연산자 같은 낱말)으로 자르고, 파서(parser)가 토큰을 문장과 식의 구조로 묶는다. 이 단계에서 글자나 구조가 규칙에 맞지 않으면 E01xx다. 실행 전에 찾는 컴파일 에러라 종료 코드는 2이고, 문법 에러가 있으면 파일의 어떤 줄도 실행하지 않는다. [어휘](language/lexical.html)와 [문장과 블록](language/blocks.html)이 규칙이다.

:::error E0101

fung이 쓰지 않는 글자가 코드에 나왔다. 문자열과 주석 밖에서는 정해진 연산자와 괄호, 이름에 쓰는 글자만 올 수 있다.

흔한 원인: 다른 언어의 연산자(`@`, `&&`의 `&`, `$` 등)를 그대로 썼거나, 복사해 온 코드에 보이지 않는 특수 문자가 섞였다. 옛 Mac 줄 끝처럼 `\n` 없이 홀로 선 CR(`\r`)도 이 에러다(주석 안에서도). 양방향 제어 문자(U+202A~U+202E, U+2066~U+2069)는 주석과 문자열 안에서도 이 에러다: 글자가 읽히는 순서와 다르게 보여, 주석이나 문자열처럼 보이는 곳에 코드를 숨길 수 있기 때문이다(문자열에 필요하면 `"\u{202E}"`처럼 이스케이프로 쓴다). 에러 발췌는 그런 제어 문자를 `\r`처럼 보인다.

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

흔한 원인: `3x`처럼 숫자와 글자를 붙여 씀, 16진수에 없는 글자(`0xfg`), 기간 단위가 아닌 글자(`30sec`; 기간 단위는 `ms`·`s`·`m`·`h`뿐이다), 소수부 없는 `1.`(정수 바로 뒤의 `.`는 소수부나 범위 `..`만 된다). 대문자 접두사·16진 숫자·지수(`0XFF`, `0xFF`, `1E5`; 소문자만 된다). 모양은 맞는데 값이 타입에 담기지 않는 수는 [E0135](#E0135)다. `1.2.3` 같은 버전 문자열은 숫자 `1.2` 뒤의 필드 접근으로 읽혀 E0112가 된다. 버전은 문자열 `"1.2.3"`으로 쓴다.

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

문자열 `"…"` 안의 `\` 뒤에 fung이 모르는 글자가 왔다. 아는 이스케이프는 `\n` `\t` `\r` `\\` `\"` `\$` `\u{…}` `\xHH`뿐이고, 나머지는 실수로 보고 에러로 막는다. `\x` 뒤에는 16진 숫자가 꼭 둘 와야 한다(`\x4` 다음에 숫자가 아닌 글자가 오면 이 에러다). 메시지는 이스케이프를 쓴 그대로 보인다(``unknown escape `\l` ``). 보이지 않는 글자(탭)가 `\` 뒤에 오면 그 코드 포인트를 적는다. `\u{…}`의 값이 글자가 아니면 [E0106](#E0106)이다.

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

흔한 원인: 닫는 따옴표를 빠뜨림, 여러 줄 문자열을 `"…"`로 씀(여러 줄은 백틱 원시 문자열이나 `\n`). 줄 끝의 `\`도 줄을 잇지 않는다(이스케이프할 글자가 없어 이 에러다).

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

흔한 원인: 작은 단위를 먼저 씀(`30s5m`), 마지막 숫자에 단위를 빠뜨림(`1h30`), 성분을 `0`으로 시작함(`1h05m`), 마지막이 아닌 성분의 소수점(`1.5h30m`), 0 ns로 반올림되는 0이 아닌 값(`0.0000004ms`).

```fung run exit=2 error=E0105
wait = 30s5m
```

```fung run
wait = 5m30s
print(wait)
```

:::

:::error E0106

`\u{…}` 이스케이프의 값이 유니코드 글자(스칼라 값)가 아니다. `\u{…}`는 10FFFF까지의 코드 포인트를 받되, UTF-16이 쓰는 서로게이트(D800~DFFF)는 글자가 아니라서 받지 않는다.

흔한 원인: 다른 언어의 UTF-16 서로게이트 짝(`\ud83d\ude00`)을 옮겨 씀. 글자 하나는 코드 포인트 하나로 쓴다(`\u{1f600}`). 글자가 아닌 바이트가 필요하면 `\xHH`다.

```fung run exit=2 error=E0106
print("\u{d83d}")
```

```fung run
print("\u{1f600}")
```

:::

:::error E0110

이 자리에 올 수 없는 토큰이 나왔다. `%s` 자리에 그 토큰이 무엇인지 적힌다.

흔한 원인: `,`를 두 번 씀, 보간 `${…}` 안의 식이 덜 끝남(``unexpected `}` ``). 괄호 안 목록에서 `,`를 빠뜨리거나 괄호를 닫지 않은 것은 [E0112](#E0112)다. 문법이 읽고 나서 금지하는 모양(같은 매개변수 두 번, 태그의 이름 인자 등)은 저마다 번호가 있다([E0118](#E0118)~[E0126](#E0126)).

다른 언어의 습관이면 도움말이 fung의 쓰는 법을 알려 준다([E0134](#E0134)의 표): `a || b`는 `or`, 앞에 붙은 `!x`는 `not x`, `x += 1`·`x++`는 새 값을 다시 묶기, `(a, b) -> a + b`는 `=>`, `{ … }` 블록은 `:` … `;;`. 예약어를 이름으로 묶으면(`match = 1`) 메시지가 그 낱말을 짚는다(``unexpected reserved word `match` ``).

```fung run exit=2 error=E0110
print(1,, 2)
```

```fung run
print(1, 2)
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

흔한 원인: `if`·`for`·`match`·`fn` 뒤의 블록을 여는 `:`를 빠뜨림, 블록을 닫는 `;;`를 빠뜨림(그러면 `found end of file`), 괄호 안 목록(리스트·맵·튜플·인자·매개변수·패턴)에서 `,`를 빠뜨리거나 괄호를 닫지 않음(``expected `,` or `)` ``, 괄호를 연 자리를 `note`가 알려 준다), `.` 뒤에 이름이 아닌 것(`1.2.3`), 절이 없는 `match`나 몸이 빈 절(`x => ;;`), 빈 보간 `"${}"`, `test` 이름이나 `import` 경로에 보간이나 UTF-8이 아닌 바이트(`\xff`)를 씀.

```fung run exit=2 error=E0112
x = 5
if x > 0 print("positive") ;;
```

```fung run
x = 5
if x > 0: print("positive") ;;
```

`;;`를 빠뜨리면 파일 끝에서야 알아채므로, 위치가 파일의 마지막 줄 다음을 가리킨다(`expected ;; to close the if, found end of file`). 그래서 `note`가 그 블록을 연 자리(``note: the `if` opened at 2:3``)를 짚고, 그 뒤에 블록보다 덜 들여 쓴 줄이 있으면 블록을 거기서 닫으려던 것은 아닌지 그 줄도 알려 준다. 다른 언어처럼 `end`로 닫으려 했으면 도움말이 `;;`를 알려 준다. 닫지 않은 괄호는 괄호 안에서 줄바꿈이 공백이라 다음 줄에서 멈추고, 어디서 열었는지 `note`가 짚는다.

```fung run exit=2 error=E0112
fn check(x):
  if x > 0:
    print("positive")

check(1)
```

```fung run exit=2 error=E0112
ports = [80, 443
print(ports)
```

:::

:::error E0113

맵 리터럴이나 맵 패턴 하나에 같은 키를 두 번 썼다. 리터럴이면 어느 값을 뜻했는지 알 수 없고, 패턴이면 절대 맞지 않거나 같은 값을 두 이름으로 묶을 뿐이므로 에러다.

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

고치는 법: 테스트를 최상위로 꺼낸다. `fung run`은 테스트 블록을 무시한다. 함수 안에서 조건을 확인하고 싶으면 `assert`만 쓴다([테스트와 디버깅](language/testing.html)).

```fung run
fn half(n) = n // 2

test "half": assert half(4) == 2 ;;
print(half(9))
```

:::

:::error E0115

식이나 패턴이 너무 깊이 중첩되었다(한도 10,000단계). 파서가 끝없는 중첩으로 메모리를 다 쓰지 않도록 둔 한도다. 사람이 쓰는 코드에서는 거의 나오지 않고, 프로그램이 만든 코드에서 날 수 있다. 예는 괄호를 만 개 넘게 겹쳐야 해서 싣지 않는다. `x = ((((…1…))))`처럼 괄호를 10,001겹 쓴 파일은 `error[E0115]: expression nesting too deep (limit 10000)`으로 멈춘다. 0.4.0까지는 E0802였다.

고치는 법: 중첩을 풀어 중간값을 변수에 담는다.

:::

:::error E0116

한 호출에 같은 이름 인자를 두 번 주었다. 어느 값을 쓸지 정할 수 없으므로 실행하기 전에 막는다(맵 리터럴의 중복 키 [E0113](#E0113)과 같다). 위치 인자와 이름 인자가 같은 매개변수에 겹치는 것은 부르는 함수의 매개변수를 알아야 해서 실행 중의 [E0405](#E0405)다.

```fung run exit=2 error=E0116
fn connect(host, port) = "${host}:${port}"
print(connect(host: "db1", host: "db2"))
```

고치는 법: 하나만 남긴다.

```fung run
fn connect(host, port) = "${host}:${port}"
print(connect(host: "db1", port: 5432))
```

:::

:::error E0117

태그에 빈 괄호를 붙였다. 인자 없는 태그는 심볼이므로(`Red`는 `sym("Red")`) 괄호 없이 쓴다. `Red()`를 따로 두면 `Red`처럼 보이지만 같지 않은 값이 생기므로 막는다. 식에서도 패턴에서도 같다. 파이프의 오른쪽 `x |> Ok()`는 파이프가 인자를 채우므로(`Ok(x)`) 된다.

```fung run exit=2 error=E0117
light = Red()
```

고치는 법: 괄호를 뺀다. 값을 담으려면 인자를 준다(`Some(x)`).

```fung run
light = Red
print(light, light == Red)
```

:::

:::error E0118

범위를 다른 범위의 피연산자로 썼다. `..`와 `..=`는 결합하지 않으므로 `1..2..3`은 뜻이 없다. 슬라이스 `xs[0..1..]`도 같다. 실행해 보기 전에 막는다.

```fung run exit=2 error=E0118
xs = [1, 2, 3, 4]
print(xs[0..2..])
```

고치는 법: 범위를 하나만 쓴다. 범위의 범위를 정말 뜻한다면 안쪽을 괄호로 묶는다.

```fung run
xs = [1, 2, 3, 4]
print(xs[0..2])
```

:::

:::error E0119

한 함수(람다 포함)의 매개변수 둘이 같은 이름을 묶는다. 이름 하나, 패턴 안의 이름도 센다. 어느 인자를 뜻하는지 알 수 없으므로 막는다(호출의 같은 이름 인자 [E0116](#E0116)의 짝). 패턴 하나가 혼자 같은 이름을 두 번 묶는 것은 [E0221](#E0221)이다.

```fung run exit=2 error=E0119
fn area(w, w) = w * w
```

고치는 법: 다른 이름을 붙인다.

```fung run
fn area(w, h) = w * h
print(area(2, 3))
```

:::

:::error E0120

기본값이 있는 매개변수 뒤에 기본값 없는 매개변수가 왔다. 매개변수 순서는 필수 → 기본값 → `...rest`다. 앞의 기본값을 건너뛰고 뒤의 필수 인자만 위치로 줄 길이 없기 때문이다.

```fung run exit=2 error=E0120
fn connect(host = "localhost", port) = "${host}:${port}"
```

고치는 법: 기본값이 있는 매개변수를 끝으로 옮기거나, 뒤의 것에도 기본값을 준다.

```fung run
fn connect(port, host = "localhost") = "${host}:${port}"
print(connect(5432))
```

:::

:::error E0121

태그에 이름 인자를 주었다. 태그의 값은 위치로만 담는다(`Ok(1)`, `Pair(a, b)`). 이름을 붙인 필드가 필요하면 맵을 담는다.

```fung run exit=2 error=E0121
r = Point(x: 1, y: 2)
```

```fung run
r = Point({x: 1, y: 2})
print(r)
```

:::

:::error E0122

보간 `${…}`이 든 문자열을 상수만 오는 자리, 곧 맵 리터럴의 키나 패턴에 썼다. 그 자리의 글은 프로그램을 읽을 때 정해져야 한다.

```fung run exit=2 error=E0122
env = "prod"
m = {"db_${env}": 1}
```

고치는 법: 실행 중에 만든 키는 `put`으로 넣는다. 패턴이면 이름으로 묶고 `when`으로 견준다.

```fung run
stage = "prod"
m = put({}, "db_${stage}", 1)
print(m)
```

:::

:::error E0123

이름 인자 자리에 `_`를 썼다. `_` 부분 적용은 위치 인자 자리만 채운다. 이름 인자 자리를 비워 둔 함수가 필요하면 람다를 쓴다.

```fung run exit=2 error=E0123
fn greet(name, greeting = "hi") = "${greeting}, ${name}"
hello = greet("kim", greeting: _)
```

```fung run
fn greet(name, greeting = "hi") = "${greeting}, ${name}"
hello = g => greet("kim", greeting: g)
print(hello("hello"))
```

:::

:::error E0124

이름 없는 `fn`을 `=`로 썼다. `fn f(x) = …`는 이름 있는 함수 정의만의 모양이고, 이름 없는 함수는 블록 `fn(x): … ;;`이나 람다 `x => …`로 쓴다. 같은 일을 하는 길을 하나로 두려는 규칙이다.

```fung run exit=2 error=E0124
double = fn(x) = x * 2
```

```fung run
double = x => x * 2
print(double(4))
```

:::

:::error E0125

슬라이스 `xs[a..b]`의 한쪽 끝의 맨 위에 `..`보다 약하게 묶이는 연산자(`??`, `|>`, 비교, `in`, `not`, `and`·`or`, `then … else`)가 있다. 슬라이스 밖에서는 `..`가 먼저 묶이므로(`a ?? 0..2`는 `a ?? (0..2)`, `c then 0 else 1..2`는 `c then 0 else (1..2)`), 그런 끝을 그대로 받으면 같은 글자가 자리에 따라 다른 뜻이 된다. 그래서 괄호로 묶어 뜻을 정하게 한다. 0.6.0까지는 `??`와 `|>`를 그대로 끝으로 받았다.

```fung run exit=2 error=E0125
xs = [1, 2, 3]
c = true
print(xs[c then 1 else 0..])
```

```fung run
xs = [1, 2, 3]
c = true
print(xs[(c then 1 else 0)..])
```

:::

:::error E0126

이름 붙인 인자 뒤에 위치 인자를 썼다. 위치 인자는 매개변수를 앞에서부터 채우므로, 이름 인자가 어느 자리를 채운 뒤에 오는 위치 인자는 어느 매개변수인지 헷갈린다. 위치 인자(`_` 포함)를 먼저 쓴다.

```fung run exit=2 error=E0126
fn connect(host, port = 80, tls = false) = "${host}:${port} ${tls}"
print(connect(tls: true, "example.com"))
```

```fung run
fn connect(host, port = 80, tls = false) = "${host}:${port} ${tls}"
print(connect("example.com", tls: true))
```

:::

:::error E0127

`??`와 `|>`를 괄호 없이 섞었다. `x |> f ?? d`는 "파이프로 구한 뒤 없으면 기본값"으로도, "`f ?? d`를 파이프로 부르기"로도 읽히고, `a ?? b |> f`도 두 가지로 읽힌다. 어느 우선순위를 골라도 한쪽 읽기가 말없이 뒤집히므로(0.6.0까지는 `??`가 먼저 묶여 `"x" |> env ?? "d"`의 기본값이 사라졌다), 비교 연쇄(E0111)처럼 괄호로 순서를 적게 한다.

```fung run exit=2 error=E0127
print("NOT_SET" |> env ?? "default")
```

고치는 법: 먼저 할 쪽을 괄호로 감싼다.

```fung run
print(("NOT_SET" |> env) ?? "default")
```

:::

:::error E0128

대문자로 시작하는 심볼을 따옴표로 썼다. 인자 없는 태그 `Red`가 바로 그 심볼이므로 `'Red`는 같은 값의 둘째 표기다. 표기를 하나로 두려고 막는다. 기본 도움말은 따옴표를 뺀 태그를 보인다.

```fung run exit=2 error=E0128
color = 'Red
```

고치는 법: 따옴표를 뺀다. 결과값의 이름(`Ok`·`Err`)은 값 없이 쓸 수 없으므로(E0236) 결과값인지는 `is_ok`·`is_err`로 가린다.

```fung run
color = Red
print(color == Red, type_of(color))
```

:::

:::error E0129

보통 문자열 안에서 `$` 바로 뒤에 이름이 왔다. `"$name"`은 셸이나 다른 언어에서 보간이지만 fung에서는 보간이 아니라 글자 그대로라, 값이 들어간 줄 알기 쉽다. `$` 뒤가 이름의 첫 글자가 아니면(`"$5"`, 끝의 `$`) 글자다. 원시 문자열 `` `…` ``에는 보간이 없으므로 해당하지 않는다.

```fung run exit=2 error=E0129
user = "ann"
print("hello $user")
```

고치는 법: 값은 `${user}`로, 글자 그대로는 `\$user`로 쓴다. `$`가 많은 셸 명령은 원시 문자열로 쓴다.

```fung run
user = "ann"
print("hello ${user}, not \$user")
```

:::

:::error E0130

타입 표기에 원소 타입을 적었다(`list[int]`, `map[str]`). 런타임은 바깥 타입만 검사하므로 원소 타입은 적어도 지켜지지 않는다. 검사하지 않는 표기는 두지 않는다. 0.6.0까지는 원소 타입을 적을 수 있었다(이름만 검사했다).

```fung run exit=2 error=E0130
fn total(xs: list[int]) = sum(xs)
```

고치는 법: 바깥 타입만 쓴다.

```fung run
fn total(xs: list) = sum(xs)
print(total([1, 2, 3]))
```

:::

:::error E0131

같은 블록에서 `return` 문장 뒤에 문장이 더 있다. `return`이 함수를 끝내므로 뒤의 문장은 결코 실행되지 않는다. 지우려던 코드가 남았거나, `return`을 조건 안에 넣으려던 실수다.

```fung run exit=2 error=E0131
fn check(n):
  return n > 0
  print("checked")
;;
```

고치는 법: 실행되지 않는 문장을 지우거나, `return`을 그 뒤로 옮긴다.

```fung run
fn check(n):
  print("checked")
  n > 0
;;
print(check(1))
```

:::

:::error E0132

`return`을 값 자리에 썼다: 연산자의 피연산자, 호출 인자, 리스트·맵·튜플의 원소, 대입의 오른쪽, 괄호 안. `return`은 문장이거나 가지의 끝(`if`·`match` 절의 본문, `then … else`의 가지, `fn f() = …`의 본문)에만 온다. 값 자리의 `return`은 그 식의 나머지가 계산되지 않는다는 것을 숨긴다.

```fung run exit=2 error=E0132
fn port(cfg):
  p = cfg.port ?? return 80
  p + 1
;;
```

고치는 법: 조건을 문장으로 드러내거나 `then … else`의 가지로 쓴다.

```fung run
fn port(cfg):
  if cfg.port == nil: return 80 ;;
  cfg.port + 1
;;
print(port({port: nil}), port({port: 8000}))
```

:::

:::error E0133

펼침 `...xs`를 호출의 마지막 위치 인자가 아닌 곳에 썼다: 다른 위치 인자 앞, 리스트 리터럴 안, 태그의 값. 펼침은 리스트나 범위의 원소를 호출의 위치 인자로 늘어놓는 것이고, 리스트를 잇는 방법은 `+` 하나다.

```fung run exit=2 error=E0133
base = ["-v"]
print([...base, "-q"])
```

고치는 법: 리스트는 `+`로 잇고, 펼침은 이름 인자 앞의 마지막 위치 인자로 쓴다.

```fung run
base = ["-v"]
print(base + ["-q"])
print("args:", ...base + ["-q"])
```

:::

:::error E0134

다른 언어에서 키워드인 낱말을 문장 첫머리에 키워드처럼 썼다. fung에서 이 낱말들은 키워드가 아니라 보통 이름이라, 그대로 두면 뒤따르는 토큰에서 엉뚱한 에러가 난다. 그래서 그 낱말을 짚고 fung의 쓰는 법을 도움말로 알려 준다. 알아보는 낱말과 도움말은 한 표(`crates/fung-front/src/habits.rs`)에 있다.

- `elif`·`elsif`·`elseif` → `else if`
- `while`·`loop` → `for x in xs:`, 재귀, [`until`](prelude/loops.html)·[`iterate`](prelude/loops.html)
- `let`·`var`·`const`·`val` → 키워드 없이 `x = 1`
- `def`·`function`·`func`·`fun` → `fn name(x): … ;;`
- `lambda` → `x => x + 1`
- `switch`·`case` → `match`
- `try`·`catch`·`except`·`raise`·`throw` → 예외가 없다. `Err(…)`를 돌려주고 `?`·`!`·`match`로 다룬다
- `from` → `import m (a, b)`
- `end`·`endif`·`fi`·`done` → 블록은 `;;`로 닫는다(줄에 홀로 선 `end`는 이름으로 읽히므로 닫지 않은 블록의 [E0112](#E0112)에 도움말로 붙는다)

`null`·`undefined`·`nullptr`를 값으로 쓰면 없는 이름([E0204](#E0204))이고, 도움말이 `nil`을 알려 준다. 기호도 같은 표에서 도움말을 얻는다: `&&`([E0101](#E0101))는 `and`, `||`·앞에 붙은 `!`·`x += 1`·`x++`·`->` 람다·`{` 블록·`match = 1`([E0110](#E0110))은 각각 `or`·`not`·다시 묶기·`=>`·`:` … `;;`·예약어 안내, 조건 자리의 `=`([E0112](#E0112))는 `==`.

이 낱말을 이름으로 쓰는 것은 그대로 된다(`let = 1`, `while(x)` 같은 호출). 낱말 바로 뒤(또는 바로 뒤 괄호 다음)에서 문법 에러가 날 때만 이 에러다.

```fung run exit=2 error=E0134
x = 2
if x == 1:
  print("one")
elif x == 2:
  print("two")
;;
```

```fung run
x = 2
if x == 1:
  print("one")
else if x == 2:
  print("two")
;;
```

:::

:::error E0135

숫자 모양은 맞는데 값이 그 타입에 담기지 않는다. 정수는 64비트(-9223372036854775808부터 9223372036854775807까지)이고, 실수는 약 1.8e308까지이며 0이 아닌데 0.0으로 아래 넘치는 수(`1e-400`)도 받지 않는다. 가장 작은 정수 `-9223372036854775808`은 빼기 기호와 한 덩어리일 때만 된다. 그 수에 `**`·인덱스·호출·`!`·`?`가 바로 붙으면 그것이 빼기보다 먼저 묶여 크기만 남으므로 이 에러다(괄호로 묶는다).

```fung run exit=2 error=E0135
limit = 99999999999999999999
print(limit)
```

고치는 법: 정수 범위를 넘는 수는 실수로 쓴다.

```fung run
limit = 1e20
print(limit)
```

:::

:::error E0136

앞 문장의 다음 줄이 `-`로 시작한다. `-`는 줄을 잇는 토큰이 아니어서, 앞 줄을 이으려던 `- 2`는 새 문장이 되고 앞 줄의 값은 버려진다. 앞 줄이 함수 호출이면 그 결과가 조용히 사라지고 음수가 그 자리의 값이 되므로, 이 모양을 받지 않는다. 파일과 블록의 첫 문장, 여는 것과 같은 줄의 문장(`_ => -1`), `;` 뒤의 문장은 이을 앞 줄이 없으므로 된다.

```fung run exit=2 error=E0136
fn scale(n) = n * 10
fn f(a):
  scale(a)
  - 2
;;
print(f(5))
```

고치는 법: 앞 줄을 이으려면 연산자를 줄 끝에 둔다. 음수 값을 한 줄에 쓰려면 괄호로 감싼다.

```fung run
fn scale(n) = n * 10
fn f(a):
  scale(a) -
    2
;;
fn g(a):
  print(a)
  (-a)
;;
print(f(5), g(1))
```

:::

## E02xx: 이름과 스코프 {#e02}

문법이 맞으면 fung은 이름(변수, 함수, 타입)이 어디서 무엇을 가리키는지 정한다. 이것을 이름 해석(resolve)이라 하고, 이름이 보이는 범위를 스코프(scope)라 한다. 이 단계의 에러가 E02xx다. 역시 실행 전에 찾는 컴파일 에러라 종료 코드는 2이고, 한 번에 여러 개를 보고할 수 있다. 규칙은 [변수와 스코프](language/scope.html)에 있다: 이름은 위에서 아래로 묶이고, 블록은 바깥 변수를 읽을 수만 있고, 블록 안의 이름은 밖으로 새지 않는다.

:::error E0204

그 자리에서 보이지 않는 이름을 썼다.

흔한 원인: 오타(도움말이 비슷한 이름을 알려 준다), 대입보다 위에서 읽음, 람다가 아직 대입되지 않은 이름을 씀.

```fung run exit=2 error=E0204
files = ["a.log", "b.log"]
print(len(fiels))
```

고치는 법: 철자를 고치거나, 대입을 위로 옮기거나, 값을 인자로 넘긴다.

```fung run
files = ["a.log", "b.log"]
print(len(files))
```

:::

:::error E0205

블록 안에서 블록 바깥의 변수에 대입했다. 이름은 한 번만 묶이고([E0238](#E0238)), fung의 블록(`if`·`for`·함수 본문 등)은 바깥 이름을 읽을 수만 있다. 값이 어디서 바뀌는지 찾아 헤매지 않게 하려는 규칙이다. 최상위 `fn` 안에서 파일의 최상위 이름(상수)에 대입하는 것도 같다(본문의 맨 바깥 블록에서도). 노트가 바깥 이름을 묶은 줄을 알려 준다. 함수·import한 이름·prelude 함수처럼 변수가 아닌 이름을 다시 묶으면 [E0240](#E0240)이다.

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

블록 안에서 만든 이름을 블록 밖에서 읽었다. 블록 안의 이름은 블록이 끝나면 사라진다. `if` 가지, `match` 절(패턴이 묶은 이름 포함), `for`(변수와 본문에서 만든 이름)가 모두 블록이고, 도움말이 어느 것인지에 맞춰 그 값을 쓰는 길을 알려 준다.

흔한 원인: `if`의 두 가지에서 같은 변수에 대입하고 아래에서 쓰려 함, `for`가 끝난 뒤 그 변수로 마지막 원소를 읽으려 함.

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

쓰지 않는 번호다. `fn`으로 정의한 함수나 import한 이름에 대입할 때 내던 에러였는데, "이름은 한 번만 묶인다"는 한 규칙으로 [E0238](#E0238)에 합쳤다. 번호는 다시 쓰지 않는다. 나오지 않으므로 예가 없다.

:::

:::error E0210

같은 이름의 `fn`을 두 번 정의했다. 파일의 최상위든 한 블록 안이든 같다. `note`가 첫 정의의 줄을 알려 준다. fung에는 인자 개수로 함수를 고르는 오버로딩(overloading)이 없다.

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

함수 밖, 곧 실행하는 파일의 최상위에서 `return`을 썼다. `return`은 감싸는 함수에서 돌아오는 것이라 최상위에는 돌아갈 곳이 없다. 매개변수 기본값 안의 `return`과 `?`도 이 에러다(메시지 `return inside a parameter default`·`` `?` inside a parameter default ``): 기본값은 본문보다 먼저 계산되므로 거기서 돌아가면 본문을 건너뛴다. 기본값을 `nil`로 두고 본문에서 계산한다.

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

타입 표기에 없는 타입 이름을 썼다. 쓸 수 있는 것은 내장 타입 이름(`int` `str` `list` …), 태그 이름, 같은 파일의 `type` 별칭이다([타입 표기](language/types.html)).

흔한 원인: 다른 언어의 타입 이름(`integer`, `string`, `boolean`)을 씀, 별칭 이름의 오타, 내장 타입을 대문자로 씀(`Int`, `String`, `List`: 대문자 이름은 태그라서, 도움말이 소문자 이름을 알려 준다).

```fung run exit=2 error=E0212
fn retry(times: integer) = times
```

```fung run
fn retry(times: int) = times
print(retry(3))
```

:::

:::error E0213

쓰지 않는 번호다. `for` 본문 안의 `return`에 내던 에러였는데, 람다 안의 `return`·`?`와 한 규칙으로 [E0237](#E0237)에 합쳤다. 번호는 다시 쓰지 않는다. 나오지 않으므로 예가 없다.

:::

:::error E0214

함수 밖에서 `?`를 썼다. `x?`는 `Err`를 만나면 "지금 함수에서" 그 `Err`를 돌려주는 연산자인데, 최상위에는 돌려줄 함수가 없다.

```fung run exit=2 error=E0214
port = parse_int("8080")?
```

고치는 법: 스크립트 최상위에서는 `!`(실패하면 멈춤)나 `??`(기본값), `match`를 쓴다([결과값과 에러](language/results.html)).

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

쓰지 않는 번호다. `for` 본문 안의 `?`에 내던 에러였는데, 람다 안의 `return`·`?`와 한 규칙으로 [E0237](#E0237)에 합쳤다. 번호는 다시 쓰지 않는다. 나오지 않으므로 예가 없다.

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

값으로 쓰는 자리의 `if`에 `else`가 없다. `else`가 없는 `if`는 조건이 거짓일 때 `nil`이 되는데, 값을 쓰는 자리(대입의 오른쪽, 인자, 함수 본문의 끝, `return` 등)에서는 거의 늘 실수다. 함수 본문의 끝은 블록 본문 함수(`fn f(): … ;;`)도 포함한다. 부르는 쪽이 반환값을 쓸 수 있기 때문이다.

흔한 원인: 한 가지만 쓰고 나머지 경우를 잊음. `else if` 사슬의 마지막에 `else`가 없는 것도 같다.

```fung run exit=2 error=E0223
fn label(code) = if code >= 500: "server error" ;;
```

고치는 법: `else`를 쓴다. 정말 `nil`을 뜻하면 `else: nil`이라고 적는다. 돌려줄 것이 없는 함수는 `else: nil`이나 마지막 줄 `nil`로 맺는다. 값을 쓰지 않는 문장 자리의 `if`는 `else` 없이 써도 된다([조건](language/conditions.html)).

```fung run
fn label(code) = if code >= 500: "server error" else: "ok" ;;
print(label(503), label(200))

fn alert(code):
  if code >= 500: print("page someone") else: nil ;;
;;
alert(503)
```

:::

:::error E0224

값으로 쓰는 블록이 비었거나 문장(대입, `assert`, 이름 있는 `fn` 정의)으로 끝난다. 블록의 값은 마지막 식의 값인데 문장에는 값이 없으므로 블록의 값이 `nil`이 된다. 메시지가 블록이 어떤지 적는다(`is empty`, `` ends with an assignment to `y` ``, `ends with an assignment to a pattern`, `` ends with `assert` ``, `` ends with the definition of `g` ``). 값을 쓰는 자리는 대입의 오른쪽, 인자, 함수 본문의 끝 같은 곳이고, 문장 자리의 `if c: assert x ;;`는 된다. 콜백의 본문도 그 값을 돌려주는 함수(`fs.with_temp_dir`, `map`)에 넘기면 값을 쓰는 자리다.

```fung run exit=2 error=E0224
x = if true:
  y = 1
else:
  2
;;
```

```fung run exit=2 error=E0224
x = if false: 1 else: ;;
```

고치는 법: 블록의 마지막 줄에 값을 둔다. `nil`이 뜻이면 `else: nil`이나 마지막 줄의 `nil`로 쓴다. 대입으로 끝나는 블록은 이전에 경고 W0104였고, 빈 블록과 다른 문장으로 끝나는 블록은 E0231이었다(둘을 이 번호 하나로 합쳤다).

```fung run
x = if true:
  y = 1
  y
else:
  2
;;
z = if false: 1 else: nil ;;
print(x, z)
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

쓰지 않는 번호다. `map`·`conc.pmap`에 넘긴 람다 안의 `?`에 내던 에러였는데, 어느 람다든 `return`·`?`를 막는 [E0237](#E0237)에 합쳤다. 번호는 다시 쓰지 않는다. 나오지 않으므로 예가 없다.

:::

:::error E0227

파이프 `|>` 오른쪽 호출의 인자에 `_`를 썼다. 파이프 값은 늘 첫 인자로 들어가므로 `x |> f(1, _)`는 `f(x, 1, _)`, 곧 부분 적용된 함수가 되어 버린다. 뜻한 것과 다르므로 막는다([파이프](language/operators.html#pipe)).

```fung run exit=2 error=E0227
import path
print("a.txt" |> path.join("/tmp", _))
```

고치는 법: 다른 자리에 넣으려면 부분 적용을 괄호로 감싼다. 괄호 친 식은 파이프가 끼워 넣지 않고 값으로 부른다. 람다로 써도 된다.

```fung run
import path
print("a.txt" |> (path.join("/tmp", _)))
print("a.txt" |> (f => path.join("/tmp", f)))
```

:::

:::error E0228

나머지 매개변수 `...이름`에 `list`가 아닌 타입을 적었다. 나머지 매개변수는 남는 인자들을 모은 리스트라서, 다른 타입을 적으면 모든 호출이 실패한다.

흔한 원인: 원소의 타입을 적으려고 `...ns: int`라고 씀.

```fung run exit=2 error=E0228
fn total(...ns: int) = fold(ns, 0, (a, b) => a + b)
```

고치는 법: `list`로 적거나 아무것도 적지 않는다(원소 타입은 적지 않는다, E0130).

```fung run
fn total(...ns: list) = fold(ns, 0, (a, b) => a + b)
print(total(1, 2, 3))
```

:::

:::error E0229

파이프 `|>` 오른쪽 끝에 후위 `?`·`!`를 붙였다. 후위 연산자는 `|>`보다 먼저 묶이므로 `xs |> try_map(f)?`는 `xs |> (try_map(f)?)`로 읽힌다. 곧 파이프가 부르기도 전에 인자 하나로 `try_map`을 불러 그 결과에 `?`를 붙인 것이다. 파이프 끝에서 결과를 꺼내는 흔한 모양이라, 엉뚱한 에러로 멈추는 대신 실행 전에 막는다([파이프](language/operators.html#pipe)).

```fung run exit=2 error=E0229
nums = ["1", "2"] |> try_map(parse_int)!
print(nums)
```

고치는 법: 파이프 전체를 괄호로 감싼다.

```fung run
nums = (["1", "2"] |> try_map(parse_int))!
print(nums)
```

:::

:::error E0230

guard가 붙은 패턴의 대안이 너무 많거나(펼친 수 256개; 중첩된 `|`마다 곱해진다), 한 함수의 상수·명령·지역 변수, 한 호출의 인자, 한 함수의 매개변수 같은 것이 바이트코드 명령의 피연산자(24비트, 16,777,215)에 담기지 않을 만큼 많다. 메시지가 무엇이(`arguments`, `parameters`, `alternatives in a guarded pattern`, `constants in a function`, `instructions in a function` 등) 한도를 넘었는지 적는다. 바이트코드 명령이 개수와 번호를 24비트에 담기 때문에 둔 한도이고, 실행 전에 찾는 컴파일 에러(종료 코드 2)다. 사람이 쓰는 코드에서는 거의 나오지 않고, 프로그램이 만든 코드에서 날 수 있다. 예는 너무 커서 싣지 않는다. 0.4.0까지는 E0806이었고, 인자와 매개변수는 따로 4,095개가 한도였다.

고치는 법: 값을 리스트나 맵 하나에 모아 넘긴다.

:::

:::error E0231

쓰지 않는 번호다. 값으로 쓰는 블록이 비었거나 대입이 아닌 문장으로 끝날 때 내던 에러였는데, 같은 규칙의 [E0224](#E0224)로 합쳤다. 번호는 다시 쓰지 않는다. 나오지 않으므로 예가 없다.

:::

:::error E0232

쓰지 않는 번호다. 원소를 담지 않는 타입에 원소 타입을 붙였을 때(`int[str]`) 내던 에러였는데, 원소 타입이 언어에서 빠지면서(어디에 붙여도 [E0130](#E0130)) 쓰지 않는다. 번호는 다시 쓰지 않는다. 나오지 않으므로 예가 없다.

:::

:::error E0233

한 파일 안의 `test` 블록 이름이 다른 블록과 겹치거나 비었다. `fung test`의 보고는 실패한 시험을 이름으로 알리므로, 이름이 겹치면 어느 쪽인지 가릴 수 없다.

```fung run exit=2 error=E0233
test "parses":
  assert 1 == 1
;;
test "parses":
  assert 2 == 2
;;
```

고치는 법: 시험마다 무엇을 보는지 이름에 적는다(`"parses an empty line"`). 0.5.0까지는 그대로 받았다.

:::

:::error E0234

버려지는 자리(마지막이 아닌 문장, 그런 자리의 `if`·`match` 가지의 끝, 파일의 마지막 식, 문장 `for`·`test` 블록 본문의 끝)에 아무 일도 하지 않는 식이 있다. 이름, 리터럴, 람다·익명 `fn`, 이름과 리터럴만 넘긴 부분 적용(`f(_, x)`), 그리고 그것들만으로 된 연산자·필드·인덱스·튜플·리스트·맵·태그 값(`Ok(1)`, `Red`)은 값을 만들 뿐이라, 버리면 쓴 보람이 없다. 대개 실수다: 괄호를 잊은 호출(`cleanup`), `=` 대신 쓴 `==`(`test` 블록 안이면 빠뜨린 `assert`), 앞 줄을 이어 쓰려던 줄 앞의 `-`(`-`로 시작하는 줄은 새 문장이다).

```fung run exit=2 error=E0234
fn cleanup() = print("cleaned")
cleanup
```

효과가 없다고 알려진 내장 함수·표준 모듈 함수(`put`, `sort`, `replace`, `path.join`, `json.parse` 등 값을 계산해 돌려줄 뿐인 것)를 문장으로 부른 것도 같다(메시지 ``the result of `put(…)` is computed and discarded``). `put(m, k, v)`가 `m`을 바꾼다고 여긴 실수가 흔하다. 함수를 받는 내장 함수(`map`·`each` 등)와 사용자 함수는 무엇을 할지 모르므로 해당하지 않는다.

고치는 법: 호출이면 `()`를 붙이고, 대입이면 `=`를 쓰고, 효과 없는 함수면 결과를 이름에 담고(`m = put(m, k, v)`), 여러 줄 식이면 연산자를 줄 끝에 둔다. `fung -e`의 마지막 값도 버려지므로 보려면 `print(…)`로 감싼다. 일부러 버리려면 `_ = 식`. 예외는 맨 `nil` 하나다: 문장으로 쓴 `match`에서 할 일이 없는 절을 `_ => nil`로 적게 둔다. 실행 중에 계산된 `Err`를 버리는 것은 여전히 E0606이다. 0.6.0까지는 말없이 버렸다.

```fung run
fn cleanup() = print("cleaned")
cleanup()
```

:::

:::error E0235

`match`에 결코 돌지 않는 절이 있다. 두 경우다. guard 없는 절의 패턴이 모든 값에 맞는데(`_`, 맨 이름, 그런 갈래가 하나라도 있는 `|` 대안, 예: `1 | _`) 그 뒤에 절이 더 있으면, 앞 절이 모든 값을 가져가므로 뒤 절은 결코 돌지 않는다(에러가 앞 절을 가리킨다). guard 없는 앞 절이 이미 맞는 리터럴(값이 같은 수(`1` 뒤의 `1.0`), 쓴 그대로 같은 문자열·심볼·`nil` 등)을 패턴으로 쓴 절도 결코 돌지 않는다(에러가 뒤 절을 가리킨다). 패턴의 이름은 바깥 변수와 비교하지 않고 새 이름을 묶으므로 `limit => …`도 모든 값에 맞는다. 절 본문 안의 `y => y + 1` 줄도 새 절로 읽혀 여기에 걸린다.

```fung run exit=2 error=E0235
limit = 5
fn f(x) = match x:
  limit => "at limit"
  _     => "other"
;;
print(f(3))
```

고치는 법: 모든 값에 맞는 절을 마지막에 둔다. 바깥 변수와 비교하려면 guard를 쓴다(`v when v == limit =>`). 절의 값인 람다는 괄호로 감싼다(`1 => (y => y + 1)`). 겹친 리터럴 절은 하나를 지우거나 본문을 한 절로 합친다. 0.6.0까지는 말없이 뒤 절을 버렸다.

```fung run
limit = 5
fn f(x) = match x:
  v when v == limit => "at limit"
  _                 => "other"
;;
print(f(3), f(5))
```

:::

:::error E0236

`Ok`·`Err`에 값을 하나가 아닌 개수로 주었다. 결과값은 값을 정확히 하나 담는다. 식에서도 패턴에서도 같고, 파이프가 채운 값도 센다: `x |> Ok(2)`는 `Ok(x, 2)`라 이 에러다. 값이 없는 맨 `Ok`·`Err`도(개수 0) 이 에러다: 다른 태그라면 심볼이겠지만 결과값의 이름은 값 없이 쓰지 않는다. 파이프의 오른쪽 `x |> Ok`만 파이프가 값을 채우므로 된다. 0.6.0까지는 `Ok(1, 2)`가 "같은 이름의 보통 태그"였고 맨 `Ok`는 심볼이었다.

```fung run exit=2 error=E0236
fn divmod(a, b) = b == 0 then Err("division by zero") else Ok(a // b, a % b)
print(divmod(7, 2))
```

고치는 법: 여러 값은 튜플 하나에 담는다.

```fung run
fn divmod(a, b) = b == 0 then Err("division by zero") else Ok((a // b, a % b))
print(divmod(7, 2))
```

:::

:::error E0237

람다 `=>`나 `for` 본문 안에서 `return`이나 `?`를 썼다. 람다는 일찍 돌아갈 수 없다. 허락하면 `return`·`?`는 바깥 함수가 아니라 람다에서 돌아오고, 그 값은 받은 함수가 쓰는 대로 쓰인다: `map`이면 `Err`가 결과 리스트의 원소가 되고, `filter`면 조건 자리에서 런타임 에러가 된다. `for` 본문도 `map`이 부르는 람다라 같다. 0.6.0까지는 `for` 본문(E0213·E0220)과 `map`·`conc.pmap`에 바로 넘긴 람다의 `?`(E0226)만 막았다.

```fung run exit=2 error=E0237
fn ports(xs) = map(xs, s => parse_int(s)?)
```

고치는 법: 실패할 수 있는 변환은 첫 `Err`에서 멈추는 `try_map`, 찾기는 `find`, 검사는 `all`·`any`, 누적은 `fold`를 쓴다. 정말 일찍 돌아가야 하면 익명 함수 `fn(x): … ;;`로 쓴다.

```fung run
fn ports(xs) = try_map(xs, parse_int)
print(ports(["80", "443"]), ports(["80", "http"]))
fn first_big(xs) = find(xs, fn(x):
  if x < 0: return false ;;
  x > 100
;;)
print(first_big([10, 300]))
```

:::

:::error E0238

같은 블록에서 이미 묶은 이름을 다시 대입하거나 패턴·`fn`으로 다시 묶었다. 이름은 한 번만 묶인다. 파일의 맨 바깥에서도 함수 본문에서도, 변수든 `fn`의 이름이든 import한 이름이든 같다. 그래서 이름은 모두 상수이고, 함수가 본 값은 바뀌지 않는다. 노트가 처음 묶은 줄(import한 이름이면 import한 줄)을 알려 준다. 바깥 블록의 이름을 다시 묶으면 [E0205](#E0205)나 [E0240](#E0240)이다.

```fung run exit=2 error=E0238
retries = 3
retries = retries + 1
print(retries)
```

고치는 법: 바뀐 값에 새 이름을 붙이거나, 식 하나로 계산하거나, 함수 안에서 계산한다.

```fung run
retries = 3
max_tries = retries + 1
print(max_tries)
```

:::

:::error E0239

묶고 한 번도 읽지 않은 이름이 있다. 지역 변수, 매개변수(람다·`for`·`match` 패턴이 묶는 이름 포함), import가 모두 해당한다. 다시 대입하기만 하는 것은 읽는 것이 아니다. 진입 파일의 최상위 이름도 지역으로 센다. 계산해 놓고 쓰는 것을 잊었거나, 이름을 잘못 적었을 가능성이 크다. 메시지가 어떤 이름인지(`variable`, `parameter`, `import`) 알려 준다.

이 에러는 파일에 다른 에러가 없을 때만 나온다. 쓰지 않는 이름은 다른 에러(오타로 읽지 못한 이름, 다시 묶지 못한 이름)의 결과일 때가 많기 때문이다. 세지 않는 것: `_`로 시작하는 이름, 최상위 `fn`, import된 파일의 공개 이름(그 모듈의 인터페이스). 이전에는 경고 W0101·W0102였다.

```fung run exit=2 error=E0239
fn total(xs):
  count = len(xs)
  fold(xs, 0, (a, b) => a + b)
;;
print(total([1, 2]))
```

고치는 법: 쓰지 않는 대입·매개변수·import를 지운다. 일부러 쥐고 있는 값이나 받기만 하는 매개변수는 이름을 `_`로 시작한다(채널의 끝을 함수가 끝날 때까지 살려 둠, 콜백이 넘겨 주는 값을 쓰지 않음).

```fung run
fn total(xs) = fold(xs, 0, (a, b) => a + b)
fn on_fail(_e) = nil
(name, _size) = ("a.txt", 3)
print(total([1, 2]), on_fail(1), name)
```

:::

:::error E0240

새로 묶는 이름이 그 자리에서 보이는 바깥 이름을 가린다. 바깥 블록이나 바깥 함수의 이름, 최상위 이름과 최상위 `fn`, import한 이름, prelude 함수, 지역 `fn` 자신의 이름이 해당하고, 묶는 방법(매개변수, 람다의 매개변수, `for`의 변수, `match` 절의 패턴, 구조 분해, 지역 `fn`, 최상위 `fn`, import, 함수·prelude 이름에 대한 대입)과 상관없다. 한 자리에서 이름 하나는 한 가지만 뜻하게 하려는 규칙이다: 가려진 이름을 읽으려다 내 값을 읽는 실수(`match x: x => …`는 `x`와 비교하지 않는다)가 없어진다. 메시지가 무엇을 가리는지, 노트가 그 이름을 묶은 줄을 알려 준다. 바깥 변수에 대입하는 것은 [E0205](#E0205), 모듈 이름을 가리는 것은 [E0308](#E0308)이다. 이전에는 prelude 이름을 가리는 것만 경고 W0103이었다.

```fung run exit=2 error=E0240
limit = 10
fn clamp(xs, limit) = map(xs, x => x > limit then limit else x)
print(clamp([5, 20], limit))
```

```fung run exit=2 error=E0240
fn summary(xs):
  max = 0
  len(xs) + max
;;
print(summary([1]))
```

고치는 법: 새 이름에 다른 이름을 붙인다(`cap`, `biggest`).

```fung run
limit = 10
fn clamp(xs, cap) = map(xs, x => x > cap then cap else x)
print(clamp([5, 20], limit))
```

import된 파일의 인터페이스, 곧 공개 최상위 이름과 공개 `fn`의 매개변수는 prelude 함수와 같은 이름이어도 된다(`http.get`, `path.join`, `timeout:`). 부르는 쪽은 그것을 모듈 이름을 붙여 읽거나 인자 이름으로만 쓴다([모듈](language/modules.html)).

:::

:::error E0241

`|` 대안 가운데 하나가 결코 맞지 않는다. 앞의 대안이 모든 값에 맞거나(`_`, 맨 이름), 같은 대안을 다시 썼다(`1 | 1`, `x | x`). 앞 대안이 이미 모든 값을 가져가므로 뒤 대안은 쓸모가 없고, 대개 다른 값을 쓰려던 실수다. 절 단위로 결코 돌지 않는 절은 [E0235](#E0235)다.

```fung run exit=2 error=E0241
fn kind(code) = match code:
  200 | 200 => "ok"
  _         => "other"
;;
print(kind(201))
```

```fung run
fn kind(code) = match code:
  200 | 201 => "ok"
  _         => "other"
;;
print(kind(201))
```

:::

:::error E0242

`test` 블록의 본문이 비어 있다. 아무것도 확인하지 않는 시험은 늘 통과하므로, 쓰다 만 시험이 통과한 것처럼 보인다.

```fung run exit=2 error=E0242
fn half(n) = n // 2

test "half rounds down": ;;
print(half(5))
```

고치는 법: `assert`를 넣거나 블록을 지운다.

```fung run
fn half(n) = n // 2

test "half rounds down":
  assert half(5) == 2
;;
print(half(5))
```

:::

:::error E0243

`??`의 왼쪽이 리터럴 수·문자열·불·심볼이거나 리스트·맵·튜플 리터럴이다. 그런 값은 `nil`도 결과값도 될 수 없으므로 오른쪽은 결코 쓰이지 않는다. 같은 까닭으로 리터럴을 부르거나(`1(2)`), 불이 아닌 리터럴을 조건에 쓰거나(`not 1`), 두 리터럴에 뜻이 없는 연산을 쓰면(`1 + "a"`) 실행했을 때의 번호(E0401·E0505·E0501)로 컴파일할 때 멈춘다. 타입만으로 정해지는 것만 그렇다: `1 // 0`이나 정수 넘침처럼 값이 정하는 에러는 실행할 때 난다.

```fung run exit=2 error=E0243
port = 8080 ?? 80
print(port)
```

고치는 법: `??`를 지우거나, 없을 수 있는 값 뒤에 쓴다.

```fung run
port = env("FUNG_NO_SUCH_PORT") ?? "8080"
print(port)
```

:::

## E03xx: 모듈과 import {#e03}

모듈(module)은 다른 파일에 둔 코드를 가져다 쓰는 단위다. 파일 하나가 모듈 하나이고, `import fs`처럼 표준 라이브러리는 이름으로, `import "./lib/x"`처럼 내 파일은 경로로 가져온다([모듈](language/modules.html)). 가져올 파일을 찾고 읽고 이름을 잇는 일은 실행 전에 하므로, E03xx도 컴파일 에러(종료 코드 2)다. 에러가 가져온 파일 안에서 났으면 위치도 그 파일을 가리킨다.

:::error E0301

가져올 모듈을 찾지 못했다.

흔한 원인: 내 파일을 경로 없이 이름으로 가져옴(`import util`; 이름은 표준 라이브러리만 찾는다), 경로 오타, 없는 표준 모듈 이름. 경로로 가져올 때는 메시지에 찾아본 파일이 위치와 같은 모양으로 나온다(작업 디렉터리 아래면 상대 경로, 아니면 절대 경로). 파일은 있는데 읽지 못하면 [E0312](#E0312)다.

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

파일 이름이 fung의 이름 규칙에 맞지 않아 모듈 이름으로 쓸 수 없다. 모듈 이름은 파일 이름에서 오는데, `my-lib.fung`의 `my-lib`에는 `-`가 들어 있어 코드에서 부를 수 없다. 예약어(`if.fung`, `nil.fung`)와 대문자로 시작하는 이름(`Util.fung`, 태그)도 변수 이름이 될 수 없으니 같다.

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

다른 모듈이 지금 실행하는 파일(진입 파일)을 가져오려 했다. 진입 파일은 모듈로 쓸 수 없다. 진입 파일이 자기 자신을 가져와도 같다.

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

도움말은 가져온 그대로의 모듈 이름을 넣어 보여 준다(카탈로그의 기본은 `fs`). 고치는 법: 변수 이름을 바꾸거나 모듈을 `as`로 다른 이름으로 가져온다.

```fung run
import path
out_path = "/tmp/out"
print(path.join(out_path, "a.txt"))
```

:::

:::error E0309

이미 모듈이 묶인 이름에 다른 모듈을 또 가져왔거나, 한 파일이 같은 모듈을 두 번 가져왔다. 같은 모듈은 다른 이름으로도 두 번 가져올 수 없다(`import fs`와 `import fs as f2`): 별칭과 이름 목록이 필요하면 한 `import`에 함께 쓴다(`import "./m" as c (f)`). 이름이 같은 내 파일(`./fs.fung`)과 표준 모듈을 함께 가져와도 이 에러다.

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

`_native.`으로 시작하는 내부 모듈을 가져오려 했다. 이것은 표준 모듈이 안에서 쓰는 Rust 쪽 다리라서 직접 가져올 수 없다. 메시지가 그것을 감싼 쪽을 알려 준다: 같은 이름의 모듈(`_native.fs` → `fs`), `_native.net`이면 `http`, `_native.process`·`_native.range`이면 prelude 함수(`exec`·`sh`·`take` 등), `_native.arg`이면 표준 모듈이 인자를 검사하는 데 쓴다는 것.

```fung run exit=2 error=E0310
import _native.fs
```

고치는 법: 메시지가 말하는 공개 모듈을 가져오거나(`import fs`) prelude 함수를 그대로 쓴다.

:::

:::error E0311

`import`를 함수나 블록 안에 썼다. `import`는 파일 최상위의 맨 앞에만 둔다(다른 문장 뒤면 E0313). 함수 안의 `import`가 무시되므로 아래의 이름도 함께 E0204가 된다.

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

:::error E0312

가져온 모듈 파일이 있는데 읽지 못했다. 메시지가 까닭을 말한다: 권한이 없음(``cannot read module `lib/x.fung`: permission denied``), 파일이 아니라 디렉터리(`is a directory`), UTF-8이 아님. 파일이 없으면 [E0301](#E0301)이다.

가장 흔한 것은 UTF-8이 아닌 파일이다. fung 소스는 UTF-8 글이어야 한다. 잘못된 바이트를 U+FFFD(`�`)로 바꿔 읽으면 프로그램에 아무도 쓰지 않은 글자가 말없이 끼어들므로, 읽지 않고 멈춘다. 메시지가 처음 잘못된 바이트의 줄과 칸을 알려 준다: ``cannot read module `lib.fung`: not valid UTF-8 at line 3, column 7``. 실행하는 파일 자체나 `fung -e`의 코드가 UTF-8이 아니면 같은 이유로 ``error: cannot read `x.fung`: not valid UTF-8 at line 1, column 5``(종료 코드 3)다.

흔한 원인: 다른 인코딩(EUC-KR, Latin-1)으로 저장한 파일.

고치는 법: 파일을 UTF-8로 다시 저장한다(`iconv -f euc-kr -t utf-8`). 이때만 도움말 `fung source is UTF-8 text`가 붙는다. 바이트 값 자체가 필요하면 문자열 안에 `\xHH`로 쓴다.

```fung run
print(["caf\xe9", "caf\u{e9}"])
```

:::

:::error E0313

파일의 다른 문장 뒤에 `import`를 썼다. `import`는 파일 맨 앞, 다른 모든 문장보다 먼저 온다. 가져온 모듈은 파일의 첫 문장보다 먼저 실행되므로, 그 순서가 코드의 모양에도 드러나게 한다. 0.6.0까지는 어디에 쓰든 파일 맨 앞으로 끌어올렸다.

```fung run exit=2 error=E0313
print("start")
import json
```

고치는 법: 모든 `import`를 파일 맨 위로 옮긴다.

```fung run
import json

print("start", json.stringify([1]))
```

:::

## E04xx: 호출과 타입 표기 {#e04}

함수를 부를 때 인자가 맞지 않거나, 인자와 반환값이 적어 둔 타입과 다르면 E04xx다. fung은 동적 타입 언어라서 이것들은 대개 실행 중에, 그 호출이 실제로 일어날 때 검사하고, 종료 코드는 1이다. 메시지의 호출 경로가 어느 호출에서 났는지 알려 준다. 다만 호출 대상이 컴파일 때 정해지는 호출(가려지지 않은 최상위 `fn`, prelude 함수, import한 모듈의 함수)의 인자 개수와 이름(E0402·E0405·E0406·E0407)은 실행 전에 같은 번호와 문구로 잡고, 그때는 컴파일 에러라 `fung check`도 찾고 종료 코드는 2다. 규칙은 [함수](language/functions.html)와 [타입 표기](language/types.html)에 있다.

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

위치 인자를 함수가 받는 것보다 많이 넘겼다. 메시지는 함수 이름, 받는 개수, 넘긴 개수를 적는다. 내장 함수도 같다. 호출 대상이 컴파일 때 정해지면 실행 전에 잡는다(종료 코드 2). 변수에 든 함수 값이나 펼친 인자(`...xs`)는 부를 때 센다(종료 코드 1).

```fung run exit=2 error=E0402
fn connect(host, port) = "${host}:${port}"
print(connect("db1", 5432, 10s))
```

```fung run exit=1 error=E0402
connect = (host, port) => "${host}:${port}"
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

흔한 원인: 문자열 그대로 돌려줌, 어떤 가지에서만 다른 타입을 돌려줌. 꼬리 호출은 프레임을 재사용하고 그 검사를 이어받으므로, 꼬리 호출한 함수가 돌려준 값도 프레임이 돌아올 때 검사하고 에러는 반환 타입을 적은 함수를 가리킨다.

```fung run exit=1 error=E0404
fn port_of(url) -> int = split(url, ":")[1]
print(port_of("db1:5432"))
```

```fung run
fn port_of(url) -> int = parse_int(split(url, ":")[1])!
print(port_of("db1:5432"))
```

:::

:::error E0405

같은 매개변수를 위치 인자와 이름 인자로 두 번 주었다. 같은 이름 인자를 두 번 쓴 것은 실행 전에 [E0116](#E0116)으로 잡힌다.

```fung run exit=2 error=E0405
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

```fung run exit=2 error=E0406
fn connect(host, port = 22) = "${host}:${port}"
print(connect("db1", prot: 2222))
```

```fung run
fn connect(host, port = 22) = "${host}:${port}"
print(connect("db1", port: 2222))
```

:::

:::error E0407

기본값이 없는 매개변수에 인자를 주지 않았다. 소스에 이름이 없는 매개변수(패턴 매개변수, 부분 적용의 `_`)는 이름 대신 자리로 불린다(`missing argument 1`).

흔한 원인: 인자 빠뜨림, 함수의 첫 매개변수를 잊음(`spawn(f)`는 `spawn(s, f)`의 태스크 범위 `s`가 빠진 것이라 E0407이다).

```fung run exit=2 error=E0407
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

산술, 비교, 인덱스, 변환 같은 연산이 받은 값으로는 할 수 없을 때 E05xx다. fung은 값을 조용히 다른 타입으로 바꾸지 않는다(`"a" + 1`은 `"a1"`이 아니라 에러). 숫자가 넘치거나 0으로 나누는 것도 이상한 값을 만들어 계속 가는 대신 그 자리에서 멈춘다. 모두 실행 중의 에러라 종료 코드는 1이다([값과 타입](language/values.html), [연산자](language/operators.html)).

:::error E0501

이항 연산자가 받은 두 값의 타입 조합으로는 계산할 수 없다. 메시지는 연산(`add`, `subtract` …)과 두 타입을 적는다. 함수는 같음이 없어서 `==`·`!=`로 견주거나 `unique`에 넣어도 이 에러다(``cannot compare fn and fn``).

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

연산이나 내장 함수가 그 타입의 값에는 할 수 없는 일이다. 메시지가 하려던 일과 타입을 적는다(`cannot negate str`, `cannot take the length of int`, `cannot iterate over str`(`map`과 값 자리의 `for`), `cannot zip the elements of int`, `cannot get a key from list`, 리스트·범위가 아닌 값을 펼친 `cannot spread str` 등).

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

인덱스가 범위를 벗어났다. 리스트와 문자열의 인덱스는 0부터 `길이 - 1`까지이고, 음수는 언제나 범위 밖이다(뒤에서 세지 않는다). 슬라이스의 시작이나 끝이 길이를 넘어도 이 에러이고, 그때는 문구가 `slice end 5 is out of range (length 2)`처럼 어느 쪽인지 말한다.

```fung run exit=1 error=E0507
hosts = ["web1", "web2"]
print(hosts[2])
```

고치는 법: 길이를 먼저 확인한다. 마지막 원소는 `last(xs)`나 `xs[len(xs) - 1]`, 있는 만큼만 원하면 `take`·`drop`을 쓴다.

```fung run
hosts = ["web1", "web2"]
print(last(hosts), len(hosts) > 2 then hosts[2] else "none")
```

:::

:::error E0508

맵에 없는 키를 읽었다(`m.k`나 `m["k"]`).

흔한 원인: 선택 항목인 설정 키, JSON 응답에 없는 필드.

```fung run exit=1 error=E0508
cfg = {host: "db1"}
print(cfg.port)
```

고치는 법: 기본값이 필요하면 `get(m, k, 기본값)`을 쓰거나, `get(m, k)`가 없는 키에 돌려주는 `nil`에 `??`를 붙인다. 있는지만 보려면 `k in m`이다. 주의: `cfg.port ?? 5432`는 `cfg.port`를 읽는 순간 이 에러로 멈추므로 기본값이 되지 않는다.

```fung run
cfg = {host: "db1"}
print(get(cfg, "port", 5432), get(cfg, "port") ?? 5432, "port" in cfg)
```

:::

:::error E0509

인덱스 `x[i]`나 필드 `x.name`이 그 값에 뜻이 없다. 메시지가 두 타입을 적는다(`cannot index tuple with int`). 인덱스는 리스트·문자열(정수 위치나 범위), 맵(문자열 키), 모듈(멤버 이름)에만 된다. 리스트를 문자열로 찾는 것(`xs["a"]`)도 이 에러다. 필드 `x.name`은 맵과 모듈에만 있다. `xs.len`처럼 다른 언어의 메서드 습관으로 쓰면 도움말이 함수 호출 `len(…)`을 알려 준다.

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

흔한 원인: 숫자가 아닌 문자열을 `int`로 바꿈, 공백이 든 문자열을 `sym`으로 바꿈, JSON에 없는 타입의 값(심볼, 튜플, 시각, 기간, 태그 등)을 `json.stringify`에 넘김.

```fung run exit=1 error=E0513
print(int("8080x"))
```

고치는 법: 실패할 수 있는 입력은 `Ok`/`Err`를 돌려주는 `parse_int`·`parse_float`로 바꾸고 결과를 다룬다. JSON에는 문자열이나 숫자로 바꿔서 넣는다(심볼은 `sym_name`, 튜플은 리스트, 시각은 `time.format`).

```fung run
import json
print(parse_int("8080x"))
print(json.stringify({wait: str(5s), state: sym_name('ok)}))
```

:::

:::error E0514

문자열이 아닌 값을 맵의 키로 쓰려 했다. fung의 맵 키는 늘 문자열이다. 맵을 키로 찾거나 고치는 모든 자리(`m[k]`, `k in m`, `get`·`put`·`remove`, `group_by`의 키)가 같은 에러다. 심볼 키면 도움말이 문자열로 쓰는 법을 알려 준다.

흔한 원인: `group_by`의 키 함수가 숫자나 심볼을 돌려줌. JSON에서 읽은 맵을 심볼(`m['name]`)로 찾음.

```fung run exit=1 error=E0514
print(group_by([1, 2, 3, 4], x => x % 2))
```

```fung run
print(group_by([1, 2, 3, 4], x => str(x % 2)))
```

:::

:::error E0515

정규식이나 glob(`fs.glob`)의 패턴이 잘못되었다. 메시지에 패턴과 무엇이 틀렸는지가 붙는다. 패턴은 코드가 정하는 것이라 `Err` 값이 아니라 런타임 에러다.

흔한 원인: 괄호·대괄호를 닫지 않음, 글자 그대로 찾으려던 `(` `.` `*`를 이스케이프하지 않음, `fs.glob`에 하위 디렉터리까지의 `**`를 씀(`fs.walk`와 `filter`를 쓴다).

```fung run exit=1 error=E0515
import regex
print(regex.is_match("a(b", "(b"))
```

고치는 법: 특수 문자는 `\`로 이스케이프한다. 정규식은 백틱 원시 문자열로 쓰면 `\`를 두 번 쓰지 않아도 된다. 바깥에서 온 글을 패턴에 넣을 때는 [`regex.escape`](modules/regex.html#fn-regex-escape)를 쓴다.

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

쓰지 않는 번호다. `==`·`!=`·`in`이 타입이 다른 두 값을 견줄 때 내던 에러였다. 이제 타입이 다른 값은 그저 같지 않다(`"8080" == 8080`은 `false`). 번호는 다시 쓰지 않는다. 나오지 않으므로 예가 없다. 순서 비교(`<`, `sort`)에서 타입이 다르면 여전히 [E0501](#E0501)이다.

:::

:::error E0518

글(UTF-8 텍스트)이 꼭 필요한 곳에 UTF-8이 아닌 바이트가 든 문자열이 왔다. fung 문자열은 어떤 바이트든 담고, 파일·명령 출력·환경 변수·인자를 바이트 그대로 돌려준다. 대부분의 문자열 함수는 그런 바이트를 그대로 둔다. 그러나 JSON 텍스트, 정규식 패턴, 시간 형식, 호스트 이름처럼 형식 자체가 글을 요구하는 곳에서는 그 바이트를 어떻게 바꿀지 정할 수 없으므로 멈춘다.

```fung run exit=1 error=E0518
import json
print(json.stringify({name: "caf\xe9"}))
```

고치는 법: 바이트가 어디서 왔는지 확인한다. 다른 인코딩이면 바꾸고, 바이트를 그대로 옮겨야 하면 숫자 리스트([`bytes`](prelude/strings.html#fn-bytes))로 보낸다. 미리 알아보려면 [`is_utf8`](prelude/strings.html#fn-is_utf8)을 쓴다.

```fung run
import json
s = "caf\xe9"
print(is_utf8(s), json.stringify({name: is_utf8(s) then s else bytes(s)}))
```

:::

:::error E0519

문자열 보간이나 `join`에 글로 말없이 넣지 않는 값이 들어갔다: 결과값(`Ok`·`Err`), `nil`, 핸들(함수·모듈·태스크·범위·채널 끝·atom·파일·소켓). 리스트·맵·튜플·태그 안 어느 깊이에 있어도 같다. 결과를 풀지 않은 실수가 `Ok("…")` 같은 글이 되어, 없는 값이 `nil`이라는 글이 되어 경로·주소·명령 인자로 흘러가지 않도록 멈춘다. `print`와 `str`은 무엇이든 그대로 보인다.

```fung run exit=1 error=E0519
r = Ok("127.0.0.1:8080")
print("http://${r}/")
```

고치는 법: 결과는 `!`·`?`로 풀고, `nil`일 수 있는 값에는 `??`로 기본값을 준다. 값 자체를 글로 쓰려면(로그) `str(x)`로 바꾼다. 0.5.0까지는 결과값이, 0.6.0까지는 `nil`과 핸들, 컬렉션 안의 값이 말없이 글이 되었다.

```fung run
r = Ok("127.0.0.1:8080")
host = nil
print("http://${r!}/", str(r), "host=${host ?? "localhost"}")
```

:::

:::error E0520

파일 핸들을 연 모드와 다르게 썼다: `'write`·`'append`로 연 핸들에서 읽었거나, `'read`로 연 핸들에 썼다. 모드는 코드가 정하는 것이라 바깥 사정의 `Err`가 아니라 코드 오류다.

```fung run exit=1 error=E0520
import fs
f = fs.open("w.txt", 'write)!
read_line(f)
```

고치는 법: 하려는 일에 맞는 모드로 연다(`fs.open(p)`는 읽기, `fs.open(p, 'write)`는 쓰기). 0.5.0까지는 `Err({kind: 'invalid})`였다.

:::

:::error E0521

`regex.replace`의 바꿀 글이 패턴에 없는 그룹을 가리킨다. 없는 그룹은 빈 문자열이 되어 글을 말없이 지우므로 멈춘다. `$` 뒤의 이름은 가능한 한 길게 읽으므로, `$1a`는 그룹 1 뒤의 `a`가 아니라 이름이 `1a`인 그룹이다.

```fung run exit=1 error=E0521
import regex
print(regex.replace("k=v", `(\w)=(\w)`, "$1a"))
```

고치는 법: 패턴에 있는 그룹을 가리킨다. 그룹 뒤에 글자를 붙이려면 `${1}a`, `$` 글자는 `$$`로 쓴다. 0.5.0까지는 빈 문자열이었다.

```fung run
import regex
print(regex.replace("k=v", `(\w)=(\w)`, `${1}a`))
```

:::

:::error E0522

범위 값의 시작이 끝보다 뒤에 있다(`a..b`는 `a > b`, `a..=b`는 `a > b + 1`). 빈 범위는 `n..n`과 `n..=n-1`뿐이다. 거꾸로 세려던 `for i in n..0`이나 빈 리스트의 `0..len(xs) - 1`이 말없이 아무것도 하지 않는 대신 멈춘다.

```fung run exit=1 error=E0522
n = 3
for i in n..0:
  print(i)
;;
```

고치는 법: 거꾸로 세려면 `reverse(0..n)`을 쓴다. 끝이 시작보다 작아질 수 있는 계산이면 `a..max(a, b)`로 드러내거나, 마지막 인덱스 대신 길이를 끝으로 쓴다(`0..len(xs)`).

```fung run
n = 3
for i in reverse(1..=n):
  print(i)
;;
```

:::

:::error E0523

정수와 실수를 섞은 산술(`+ - * / // % **`)에서 정수가 실수로 정확히 옮겨지지 않는다. 실수는 2<sup>53</sup>까지의 정수만 모두 정확히 담으므로, 그보다 큰 일부 정수는 가장 가까운 실수로 바뀌어 값이 말없이 달라진다. 비교(`==`, `<`)는 정수와 실수를 정확히 견주므로 이 에러가 없다.

```fung run exit=1 error=E0523
id = 9007199254740993
print(id + 0.5)
```

고치는 법: 반올림해도 되면 `float(n)`으로 일부러 바꾼다. 정확한 값이 필요하면 정수끼리 계산한다. 0.6.0까지는 말없이 반올림했다.

```fung run
id = 9007199254740993
print(float(id) + 0.5, id + 1)
```

:::

:::error E0524

정수를 음의 정수로 거듭제곱했다(`2 ** -1`). 결과가 정수가 아니므로, 결과의 타입이 지수의 값에 따라 바뀌지 않게 에러로 한다.

```fung run exit=1 error=E0524
n = -1
print(2 ** n)
```

고치는 법: 지수(나 밑)를 실수로 쓴다. 0.6.0까지는 실수를 돌려줬다.

```fung run
n = -1
print(2 ** float(n), 2.0 ** n)
```

:::

## E06xx: 패턴과 결과값 {#e06}

`match`와 구조 분해 대입은 값을 패턴(pattern)에 맞춰 본다. 맞는 패턴이 없으면 E0601·E0602다. fung은 실패할 수 있는 일의 결과를 예외(exception)로 던지지 않고 `Ok(값)`·`Err(내용)`이라는 값으로 돌려준다. 이 결과값을 `!`·`?`로 잘못 다루거나 확인하지 않고 버리면 E0603~E0606이다([match와 패턴](language/match.html), [결과값과 에러](language/results.html)). 모두 실행 중의 에러다.

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
[_name, _addr, _role] = split(line, " ")
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
_text = fs.read("config.json")!
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

`nil`에 `!`·`?`나 그 규칙을 따르는 함수(`try_map`)를 썼다. 이들은 `Ok`/`Err`만 받는데, `nil`은 "없음"이지 실패가 아니다. 메시지 앞의 이름이 연산자나 함수다.

흔한 원인: `nil`을 돌려줄 수 있는 함수(`get`, `find`, `first`, `env` 등)의 결과에 `!`나 `?`를 붙임.

```fung run exit=1 error=E0604
cfg = {host: "db1"}
_port = get(cfg, "port")!
```

고치는 법: `nil`에는 `??`로 기본값을 준다. 없을 때 호출자에게 넘기려면 `match`나 `if x == nil: return nil ;;`로 드러내 쓴다.

```fung run
cfg = {host: "db1"}
port = get(cfg, "port") ?? 5432
print(port)
```

:::

:::error E0605

`!`·`?`나 그 규칙을 따르는 함수(`try_map`)가 결과값이 아닌 값을 받았다. 결과값은 `Ok(v)`·`Err(e)`뿐이다(`nil`은 E0604). 메시지 앞의 이름이 연산자나 함수다.

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

버려지는 자리: 문장으로 쓴 호출, 그런 자리의 `if`·`match` 가지의 끝, `for` 문장의 본문, 파일의 마지막 식, `each`에 준 함수, `with`의 `release`, `supervise`의 `on_fail`, `test` 블록이 돌려준 값. 외부 명령이 실패해도 `exec`가 `Err`를 돌려주므로 같다.

버려지는 값이 리스트·맵·튜플·태그면 그 안을 깊이와 상관없이 본다. `for`로 끝나는 함수를 문장으로 부르거나 `map(…)`을 문장으로 쓰면 리스트가 버려지는데, 그 안의 첫 `Err`가 E0606이 되고 `note: element 2 of the discarded list (of 3)`처럼 어디 있는지 덧붙는다(더 깊으면 `note: key "db" of the discarded map, then the value of `Ok(…)``처럼 길을 잇는다). 0.6.0까지는 리스트의 한 겹만 봤다.

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

:::error E0607

태스크의 함수가 `Err`를 돌려줬는데, 그 결과를 아무도 `await`로 읽지 않은 채 읽을 수 없게 되었다. 핸들을 버리고 띄운 일이 실패하면 아무도 모르게 지나가므로, 버려지는 `Err`(E0606)처럼 에러로 만든다. 에러는 `spawn`한 자리를 가리키고 그 범위를 실패시킨다(`supervise`면 `on_fail`이 받는다).

읽을 수 없게 되는 때는 셋이다. 태스크가 끝났을 때 핸들이 하나도 남지 않았거나, 끝난 뒤 마지막 핸들이 읽히지 않은 채 사라졌거나, 범위가 닫힐 때 읽지 않은 핸들이 범위 밖에 남았을 때다. 어느 경우든 에러인지 아닌지는 같고, 알리는 때만 다르다. 리스트·맵·튜플·태그를 돌려주면 그 안도 깊이와 상관없이 본다. 취소·시간 초과로 끝난 태스크의 `Err`는 런타임이 만든 것이라 세지 않는다.

```fung run exit=1 error=E0607
import fs
scope(s => spawn(s, () => fs.write("missing/out.txt", "data")))
print("saved")
```

고치는 법: 결과를 `await`로 읽어 다루거나, 실패해도 상관없는 일이면 태스크의 함수를 `nil`로 맺는다.

```fung run
import fs
scope(fn(s):
  t = spawn(s, () => fs.write("missing/out.txt", "data"))
  match await(t)!:
    Ok(_)  => print("saved")
    Err(e) => print("skip:", e.kind)
  ;;
  spawn(s, fn():
    _ = fs.remove("missing/old.txt")
    nil
  ;;)
;;)
```

:::

## E07xx: 동시성 {#e07}

동시성(concurrency)은 여러 일(태스크, task)을 번갈아 진행하는 것이다. fung의 태스크는 `scope` 안에서 `spawn`으로 띄우고, 채널(channel)로 값을 주고받고, 여럿이 함께 보는 값은 `atom`에 둔다([동시성 내장 함수](prelude/concurrency.html)). 이 규칙을 어기거나 모든 태스크가 서로를 기다리며 멈추면 E07xx다. 모두 실행 중의 에러다.

:::error E0701

태스크가 취소되었다. 취소는 보통 fung 안에서 조용히 처리된다: 태스크 범위가 끝나거나 다른 태스크가 실패하면 남은 태스크를 취소하고, 이 취소는 에러로 찍지 않는다. `fung run`에서는 이 번호를 거의 보지 않는다.

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
(_tx, rx) = channel()
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
;;)!
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

쓰지 않는 번호다. 채널의 끝을 같은 채널로 보낼 때 내던 에러였는데, 다른 채널·태스크를 거쳐 닫히는 고리와 같은 원인이라 [E0709](#E0709)로 합쳤다. 번호는 다시 쓰지 않는다. 나오지 않으므로 예가 없다.

:::

:::error E0706

이미 닫힌 태스크 범위의 핸들로 `spawn`하거나 `cancel`했다. 태스크 범위는 그 안의 태스크가 모두 끝나야 닫히는 울타리인데, 닫힌 뒤에 들어오는 태스크는 기다려 줄 사람이 없고, 취소할 것도 남아 있지 않다.

흔한 원인: `scope`의 함수가 태스크 범위 값 `s`를 돌려주고 밖에서 그것으로 `spawn`하거나 `cancel`함.

```fung run exit=1 error=E0706
s = scope(s => s)!
spawn(s, () => print("late"))
```

```fung run exit=1 error=E0706
s = scope(s => s)!
cancel(s)
```

고치는 법: 범위 핸들은 태스크 범위의 함수 안에서만 쓴다.

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

atom에 담을 수 없는 값을 넣었다. atom은 평범한 데이터(숫자, 문자열, 리스트, 맵 …)만 담는다. 다른 atom, 채널의 끝, 태스크, 태스크 범위, 파일, 소켓은 상태가 있는 자원이라 담을 수 없다.

```fung run exit=1 error=E0708
(tx, _rx) = channel()
_latest = atom(tx)
```

고치는 법: 데이터만 atom에 둔다. 자원은 변수나 인자로 넘긴다.

```fung run
latest = atom({host: "web1", up: true})
swap(latest, m => put(m, "up", false))
print(deref(latest))
```

:::

:::error E0709

보내거나 돌려줄 값이 그 값을 넣으려는 채널(또는 그 태스크 자신)에 바로, 또는 다른 채널의 버퍼나 끝난 태스크의 결과를 거쳐 다시 닿는다. fung은 메모리를 참조 계수로 회수하므로, 서로를 가리키는 고리는 모두 버려도 계수가 0이 되지 않아 영영 해제되지 않는다(채널은 닫히지도 않는다). 값을 나중에 받는 칸은 채널의 버퍼와 태스크의 결과 둘뿐이라, 고리가 닫히는 그 순간(`send`, 태스크의 끝)에 막는다. 채널에 그 채널의 끝을 보내는 것(리스트나 맵 안에 넣어도)은 가장 짧은 고리다(전에는 따로 E0705였다).

흔한 원인: 채널에 그 채널의 끝을 보냄, 두 채널이 서로의 받는 끝을 버퍼에 담음, 태스크가 채널로 받은 자기 핸들을 결과로 돌려줌.

```fung run exit=1 error=E0709
(tx, rx) = channel(1)
send(tx, rx)
```

```fung run exit=1 error=E0709
(ta, ra) = channel(1)
(tb, rb) = channel(1)
send(ta, rb)!
send(tb, ra)!
```

고치는 법: 채널의 끝이나 태스크 핸들 대신 데이터를 보내거나 돌려준다. 응답을 받을 채널을 요청과 함께 보내는 것은 고리가 아니므로 된다.

```fung run
(req_tx, req_rx) = channel(4)
scope(fn(s):
  spawn(s, fn():
    for (x, reply) in req_rx: send(reply, x * 2)! ;;
    nil
  ;;)
  (rtx, rrx) = channel(1)
  send(req_tx, (21, rtx))!
  print(recv(rrx)!)
  close(req_tx)
;;)
```

:::

## E08xx: 한도와 환경 {#e08}

나머지는 fung 자체의 한도(너무 깊은 중첩, 너무 깊은 재귀, 너무 큰 문자열), 직접 내는 에러(`fail`, `assert`), 실행 순서와 관련된 에러다. 내부 오류 E0801을 빼면 모두 실행 중의 에러(종료 코드 1)다. 실행 전에 찾는 한도(중첩 E0115, 인자 개수 E0230)는 문법·이름 분류에 있다.

:::error E0801

fung 내부의 오류다. 사용자 코드가 아니라 fung 구현의 버그를 뜻한다. 정상적인 프로그램으로는 일부러 낼 수 없으므로 예가 없다. 이 에러를 보면 메시지와 그것을 낸 코드를 저장소의 이슈로 알려 주기 바란다.

:::

:::error E0802

쓰지 않는 번호다. 0.4.0까지 식이나 패턴의 중첩 한도에 내던 에러였고, 지금은 문법 에러 [E0115](#E0115)가 맡는다. 실행 전에 찾는 한도인데 실행 중 에러의 분류(E08xx)에 있어 옮겼다. 번호는 한 번 붙이면 다른 뜻으로 다시 쓰지 않으므로 자리만 남아 있다. 지금의 fung에서는 나오지 않아 예가 없다.

:::

:::error E0803

`fail(메시지)`가 낸 에러다. 메시지가 그대로 에러의 메시지가 된다. "여기 오면 버그"라는 곳에서 직접 부른다. 표준 라이브러리 함수는 fung으로 쓴 것도 잘못된 인자를 이 번호가 아니라 [E0403](#E0403)으로 알린다. 띄운 태스크가 `fail`하면 메시지에 `note: scope opened at …`이 붙어 그 태스크가 속한 태스크 범위를 알려 준다.

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

쓰지 않는 번호다. 구현을 옮기던 동안 아직 만들지 않은 기능을 부르면 내던 자리표시 에러였고, 지금은 모든 내장 함수가 구현되어 나오지 않는다. 그래서 예가 없다(카탈로그의 도움말도 `not used: every builtin is implemented`).

:::

:::error E0806

fung의 크기 한도를 넘었다. 메시지가 무엇이(`characters`, `elements`) 한도를 넘었는지 적는다. 문자열은 1억 글자, 리스트(와 맵)는 1억 원소가 한도다. 리스트를 만드는 모든 길(`+`, `flatten`, `put`·`merge`, 범위나 받는 끝에서 만드는 `map`·`filter` 등, 펼치기)이 검사하고, 되도록 결과를 만들기 전에 검사하므로 메모리를 다 쓰기 전에 멈춘다(종료 코드 1). 값 하나의 한도이고 전체 메모리 한도는 없다. 값을 글로 바꾸는 `str`·보간·`print`·`fail`도 글이 한도의 바이트(글자당 4바이트)를 넘는 순간 멈춘다: 부분을 나눠 쓴 리스트는 작아도 글이 끝없이 길 수 있다. 인자·매개변수 개수 한도는 컴파일할 때 검사하는 [E0230](#E0230)이다(0.4.0까지는 그것도 E0806이었다).

```fung run exit=1 error=E0806
_line = repeat("=", 200_000_000)
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

:::error E0810

`close` 없이 버린 파일 핸들을 비우거나 닫다가 실패했다. 핸들의 마지막 사본이 사라지면 런타임의 닫기 스레드가 남은 버퍼를 비우고 파일을 닫는데, 그때 난 에러는 돌려줄 값이 없으므로 표준 에러에 이 번호로 알리고 종료 코드를 1로 한다. 실행은 멈추지 않는다(그 사이에 프로그램은 이미 다음 일을 하고 있다). NFS처럼 닫을 때 쓰기를 보내는 파일 시스템에서 할당량 초과·디스크 가득 참이 이렇게 드러난다.

고치는 법: 쓴 핸들은 [`close`](prelude/concurrency.html#fn-close)로 닫아 그 자리에서 에러를 받거나, [`fs.with_open`](modules/fs.html#fn-fs-with_open)으로 열어 끝나면 닫히게 한다.

```fung run
import fs
r = fs.with_open("report.txt", fn(h):
  fs.write_line(h, "saved")
;;, mode: 'write)
print(r)
```

:::

## W01xx: 은퇴한 경고 {#w01}

fung에는 경고가 없다. 예전에 `fung check`·`fung test`만 보이던 경고는 모두 에러가 되었고 번호는 다시 쓰지 않는다.

:::error W0101

쓰지 않는 번호다. 쓰지 않는 import에 내던 경고였는데, 지금은 에러 [E0239](#E0239)다. 나오지 않으므로 예가 없다.

:::

:::error W0102

쓰지 않는 번호다. 읽지 않는 지역 변수에 내던 경고였는데, 지금은 에러 [E0239](#E0239)다. 나오지 않으므로 예가 없다.

:::

:::error W0103

쓰지 않는 번호다. prelude 함수를 가리는 이름에 내던 경고였는데, 지금은 에러 [E0240](#E0240)이다. 나오지 않으므로 예가 없다.

:::

:::error W0104

쓰지 않는 번호다. 값으로 쓰는 블록이 대입으로 끝날 때 내던 경고였는데, 지금은 에러 E0224가 되었다. 나오지 않으므로 예가 없다.

:::
