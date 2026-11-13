---
description: 함수를 정의하고 부르는 법, 짧은 형태 fn f(x) = …, 람다, 기본값·이름·나머지 인자, 값으로서의 함수, 만들어질 때 보이는 값을 복사하는 클로저, 파이프 |>, 재귀와 꼬리 재귀.
lede: 계산에 이름을 붙여 다시 쓰는 법. fung에서 함수는 숫자나 문자열과 똑같은 값이다.
---
# 함수

:::goals

- 함수가 무엇이고 왜 쓰는지
- `fn`으로 함수를 정의하고 부르는 법, 짧은 형태 `fn f(x) = …`
- 기본값 있는 매개변수, 이름 붙인 인자, 나머지 인자 `...rest`
- 람다 `x => …`와 함수를 값으로 다루는 법
- 클로저: 함수는 만들어질 때 보이는 값을 복사한다
- 최상위 함수가 볼 수 있는 이름
- 파이프 `|>`와 부분 적용 `_`
- 재귀와 꼬리 재귀: `while` 없이 "될 때까지" 되풀이하는 법

:::

## 함수란 {#what}

같은 계산을 여러 곳에서 하게 되면, 그 계산에 이름을 붙여 두고 이름으로 부르는 것이 낫다. 고칠 때 한 곳만 고치면 되고, 읽는 사람은 `disk_level(91)`만 보고도 무슨 일을 하는지 안다. 이렇게 이름 붙인 계산이 **함수(function)**다.

함수는 값을 받아서 값을 돌려준다. 받는 값을 **인자(argument)**, 돌려주는 값을 **반환값(return value)**이라고 한다. 이미 많이 써 봤다. `len("web-1")`은 `len` 함수에 인자 `"web-1"`을 주고 반환값 `5`를 받는 것이다. 이렇게 함수를 쓰는 것을 "함수를 **부른다**(호출, call)"고 한다.

## 함수 정의하기 {#define}

함수는 `fn 이름(매개변수, …):`로 시작해 `;;`로 닫는 블록으로 정의한다. 괄호 안의 이름들을 **매개변수(parameter)**라고 하고, 부를 때 준 인자가 차례로 매개변수에 들어간다. 블록의 **마지막 식의 값**이 반환값이다.

```fung run
fn disk_level(usage):
  if usage >= 90:
    "critical"
  else if usage >= 75:
    "warning"
  else:
    "ok"
  ;;
;;

print(disk_level(91), disk_level(80), disk_level(12))
```

마지막 항목이 반환값이므로, 거기에 `else` 없는 `if`나 대입을 두면 컴파일 에러다(E0223·E0224). 부르는 쪽이 그 값을 쓰면 조용히 `nil`이 되기 때문이다. 돌려줄 것이 없는 함수는 `if`에 `else: nil`을 붙이거나 마지막 줄에 `nil`을 쓴다.

```fung run
fn warn(usage):
  if usage >= 90:
    print("disk almost full:", usage)
  else: nil ;;
;;

fn report(usages):
  for u in usages:
    if u >= 75: print("high:", u) ;;
  ;;
  nil
;;

warn(95)
report([80, 20])
```

본문이 식 하나뿐이면 `=` 뒤에 바로 쓴다. 짧은 함수는 대부분 이 모양이다.

```fung run
fn percent(used, total) = used / total * 100
fn is_log(name) = ends_with(name, ".log")

print(percent(45, 50), is_log("app.log"), is_log("app.conf"))
```

중간에 일찍 끝내고 싶으면 `return 값`을 쓴다. `return`만 쓰면 `nil`을 돌려준다.

```fung run
fn port_kind(port):
  if port < 1024:
    return "privileged"
  ;;
  port < 49152 then "registered" else "dynamic"
;;

print(port_kind(22), port_kind(8080), port_kind(50000))
```

파일 최상위의 `fn`은 프로그램이 시작하기 전에 모두 만들어진다. 그래서 정의보다 위에서 불러도 되고, 함수끼리 순서와 상관없이 서로 부를 수 있다.

```fung run
print(greet("ops"))

fn greet(team) = "hello, ${team}"
```

## 인자 {#arguments}

매개변수에 **기본값**을 주면 부를 때 그 인자를 생략할 수 있다. 부를 때 `이름: 값`으로 **이름 붙인 인자**를 주면 순서와 상관없이 원하는 매개변수만 고를 수 있다. 이름 붙인 인자는 위치 인자 뒤에 쓴다.

```fung run
fn endpoint(host, port = 443, scheme = "https") = "${scheme}://${host}:${port}"

print(endpoint("api.internal"))
print(endpoint("api.internal", 8080, "http"))
print(endpoint("api.internal", scheme: "http", port: 80))
```

마지막 매개변수 앞에 `...`를 붙이면 남는 위치 인자를 모두 리스트로 받는다. 이것을 **나머지 인자**라고 한다.

```fung run
fn remote(host, ...cmd) = "ssh ${host} ${join(cmd, " ")}"
print(remote("web-1", "df", "-h"))
print(remote("web-1"))
```

매개변수의 순서는 "필수 → 기본값 있는 것 → `...rest`"다. 인자 개수가 맞지 않거나 모르는 이름을 주면 에러다. 부르는 함수가 최상위 `fn`이나 내장 함수처럼 정해져 있으면 실행하기 전에 잡는다(종료 코드 2).

```fung run exit=2 error=E0406
fn endpoint(host, port = 443) = "${host}:${port}"
print(endpoint(prot: 80))
```

위에서는 `port`를 `prot`로 잘못 썼다. 모르는 인자 이름이라는 E0406으로 멈춘다.

거꾸로, 이미 리스트로 든 값을 위치 인자로 넘기려면 마지막 위치 인자 자리에 `...`를 붙여 **펼친다**.

```fung run
fn remote(host, ...cmd) = "ssh ${host} ${join(cmd, " ")}"
parts = ["df", "-h"]
print(remote("web-1", ...parts))
```

내장 함수도 똑같다. 예를 들어 `join(xs, sep = "")`는 `sep`에 기본값이 있어서 `join(["a", "b"])`로 부를 수 있다. 레퍼런스의 함수 항목에 이런 서명이 그대로 적혀 있다.

## 함수는 값이다 {#values}

fung에서 함수는 숫자나 문자열과 같은 **값**이다. 이름에 대입할 수 있고, 다른 함수에 인자로 넘길 수 있고, 함수가 함수를 돌려줄 수도 있다. 4장의 `filter(usages, u => u >= 90)`이 바로 함수를 인자로 넘긴 것이다.

이름 없이 그 자리에서 만드는 짧은 함수를 **람다(lambda)**라고 한다. `매개변수 => 식` 꼴이다. 매개변수가 둘 이상이거나 없으면 괄호로 감싼다.

```fung run
double = x => x * 2
add = (a, b) => a + b
hello = () => "hello"
print(double(21), add(1, 2), hello(), type_of(double))
```

람다의 본문은 식 하나다. 여러 문장이 필요하면 이름 없는 `fn(…): … ;;`를 쓴다.

```fung run
check = fn(usage):
  level = usage > 90 then "critical" else "ok"
  "${usage}% ${level}"
;;
print([95, 40] |> map(check))
```

함수를 넘길 때는 괄호 없이 이름만 쓴다. `map(xs, check)`는 `check` 함수를 넘기는 것이고, `check(95)`는 `check`를 불러 나온 **결과**다.

## 클로저: 만들어질 때 보이는 값을 복사한다 {#closures}

함수 안에서 함수를 만들면, 안쪽 함수는 바깥의 이름을 쓸 수 있다. 아래의 `tagger`는 `level`을 받아 "줄 앞에 `[level]`을 붙이는 함수"를 만들어 돌려준다. 돌려받은 함수는 `tagger`가 끝난 뒤에도 `level`을 기억한다. 이렇게 바깥 값을 품은 함수를 **클로저(closure)**라고 한다.

```fung run
fn tagger(level) = line => "[${level}] ${line}"

warn = tagger("WARN")
error = tagger("ERROR")
print(warn("disk 91%"))
print(["timeout", "refused"] |> map(error))
```

fung의 클로저 규칙은 하나다. **함수는 만들어질 때 보이는 변수의 값을 가진다.** 이름은 한 번만 묶이고 값은 바뀌지 않으므로, 함수가 가진 값은 언제 불러도 그대로다.

```fung run
fn targets(names):
  suffix = ".log"
  is_target = name => ends_with(name, suffix)
  filter(names, is_target)
;;
print(targets(["app.log", "notes.txt"]))
```

`is_target`은 만들어질 때의 `suffix`, 곧 `".log"`를 가지고 다닌다. `targets`가 끝난 뒤에 불려도 같다. 같은 이름에 다른 값을 다시 붙일 수 없으므로(E0238) "그때의 값"과 "지금의 값"이 섞일 일이 없다.

### 최상위 함수가 볼 수 있는 이름 {#top-level}

위 규칙을 최상위 `fn`에 적용하면 결론이 하나 나온다. 최상위 `fn`은 프로그램 시작 **전에** 만들어지므로, 그때는 복사할 변수가 아직 없다. 그래서 최상위 `fn`이 볼 수 있는 파일의 이름은 다음뿐이다.

- 다른 최상위 `fn`과 `import`한 모듈(10장)
- 최상위의 다른 이름, 곧 상수(3장). 최상위 이름은 한 번만 붙으니 바뀔 일이 없어 부를 때 읽는다.

```fung run
fn log_path(name) = "${log_dir}/${name}"
log_dir = "/var/log/app"
print(log_path("api.log"))
```

## 파이프 {#pipe}

함수 결과를 다음 함수에 넣고, 그 결과를 또 다음 함수에 넣다 보면 괄호가 깊어진다. `len(filter(lines, is_error))`는 안쪽부터 거꾸로 읽어야 한다. **파이프** `|>`는 같은 것을 데이터가 흐르는 순서대로 쓰게 해 준다. 규칙은 둘뿐이다.

- `x |> f(a, b)`는 `f(x, a, b)`다. 왼쪽 값이 오른쪽 호출의 **맨 앞 인자**로 들어간다.
- `x |> f`는 `f(x)`다.

```fung run
line = "  2026-10-20 ERROR disk full  "
words = line |> trim |> split(" ")
print(words)
log = ["INFO start", "ERROR disk full", "ERROR timeout", "INFO done"]
print(log |> filter(l => starts_with(l, "ERROR")) |> len)
```

그래서 내장 함수는 모두 **다루는 대상을 첫 매개변수**로 받는다: `filter(xs, f)`, `split(s, sep)`, `put(m, k, v)`. 직접 만드는 함수도 대상을 맨 앞에 두면 파이프에 그대로 이어진다. 대상이 첫 자리가 아닌 함수에는 람다로 넘긴다.

```fung run
fn scale(factor, n) = factor * n
print(5 |> (n => scale(2, n)))
```

### 부분 적용 {#partial}

호출의 인자 자리에 `_`를 쓰면, 그 자리를 비워 둔 새 함수가 만들어진다. `scale(10, _)`는 `n => scale(10, n)`과 같다. `map`에 넘길 함수를 짧게 만들 때 쓴다.

```fung run
fn scale(factor, n) = factor * n
print([1, 2, 3] |> map(scale(10, _)))
```

## 재귀: 자기를 부르는 함수 {#recursion}

4장에서 fung에는 `while`이 없고, 목록 없이 "될 때까지" 되풀이하는 반복은 **재귀(recursion)**, 곧 함수가 자기 자신을 부르는 것으로 쓴다고 했다. 이제 함수를 알았으니 그 모양을 보자. 아래는 8080번부터 시작해 이미 쓰는 포트를 건너뛰고 비어 있는 첫 포트를 찾는다. "쓰는 중이면 다음 번호로 같은 일을 다시 하고, 아니면 그 번호가 답"이다.

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

4장의 `until`·`iterate`도 이렇게 꼬리 재귀로 쓴 함수다.

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

:::box learn 매개변수와 인자, 정의와 호출

**정의**는 "이런 계산이 있다"고 적어 두는 것이고, **호출**은 그 계산을 실제로 하는 것이다. `fn percent(used, total) = …`의 `used`와 `total`이 매개변수(빈칸의 이름)이고, `percent(45, 50)`의 `45`와 `50`이 인자(빈칸에 넣은 값)다. 정의만 하고 부르지 않으면 아무 일도 일어나지 않는다.

:::

:::box mistake 최상위에서 이름 다시 쓰기 (E0238)

이미 붙인 이름에 다시 대입하면 최상위에서도 함수 안에서도 E0238이다. 바뀐 값에는 새 이름을 붙이거나 한 번에 계산한다(예: `level = if … ;;`). 최상위 `fn`이 부를 때 읽는 최상위 이름이 바뀔 수 있다면 함수의 결과가 호출 시점에 따라 달라지는데, 최상위 이름이 한 번만 붙으니 fung에는 그런 숨은 의존이 없다.

:::

:::box mistake 파이프 오른쪽 호출의 _ (E0227)

파이프 값은 늘 맨 앞 인자로 들어가므로 `"a.txt" |> path.join("/tmp", _)`는 `path.join("a.txt", "/tmp", _)`가 되어 버린다. 그래서 이 모양은 컴파일 에러다. 다른 자리에 넣으려면 람다를 쓴다: `"a.txt" |> (f => path.join("/tmp", f))`.

:::

:::box mistake 꼬리가 아닌 깊은 재귀 (E0804)

`fn total(n) = n == 0 then 0 else n + total(n - 1)`은 자기 호출이 끝난 뒤 `n +`을 해야 하므로 꼬리 호출이 아니다. 깊이가 100,000을 넘으면 `stack overflow` 에러다. 모은 값을 인자로 넘기는 꼬리 재귀(4장)로 바꾸거나 `fold`·`sum`을 쓴다.

:::

:::box other 다른 언어는

Python의 `def f(x): return …`가 fung의 `fn f(x) = …`, `lambda x: …`가 `x => …`다. 이름 붙인 인자와 기본값, `*args`(fung의 `...rest`)도 Python과 비슷하다. 다른 점은 클로저다. Python과 JavaScript의 클로저는 변수 **자체**를 붙잡아서 나중에 변수가 바뀌면 함수의 결과도 바뀌지만, fung은 만들어질 때의 **값**을 복사한다. 파이프는 Elixir의 `|>`와 같은 쪽(대상이 첫 인자)이고, 쉘의 `|`처럼 왼쪽 결과를 오른쪽으로 넘긴다.

:::

## 해 보기 {#exercises}

:::exercise 접속 주소 만들기

`host`를 받고, `user`(기본값 `"deploy"`)와 `port`(기본값 `22`)를 고를 수 있는 함수 `ssh_target`을 만들어 `deploy@web-1:22`, `root@db-1:2222` 같은 문자열을 돌려주게 하자. 서버 목록 `["web-1", "web-2"]`에 `map`으로 적용해 본다.

:::solution

```fung run
fn ssh_target(host, user = "deploy", port = 22) = "${user}@${host}:${port}"

print(["web-1", "web-2"] |> map(ssh_target))
print(ssh_target("db-1", user: "root", port: 2222))
```

:::

:::

:::exercise 문턱값 함수 만들기

숫자 `limit`을 받아 "`limit`보다 큰가?"를 묻는 함수를 돌려주는 `over(limit)`을 만들자. 그것으로 `[45, 91, 78, 99]`에서 `80`보다 큰 값과 `95`보다 큰 값을 각각 골라 본다.

:::solution

`over(80)`이 돌려주는 람다는 만들어질 때의 `limit`(80)을 복사해 둔다.

```fung run
fn over(limit) = n => n > limit

usages = [45, 91, 78, 99]
print(usages |> filter(over(80)))
print(usages |> filter(over(95)))
```

:::

:::

## 정리 {#summary}

- 함수는 `fn 이름(매개변수):` … `;;`로 정의하고, 마지막 식이 반환값이다. 식 하나면 `fn f(x) = …`.
- 최상위 `fn`은 시작 전에 만들어지므로 순서와 상관없이 부를 수 있다.
- 매개변수는 필수 → 기본값 → `...rest` 순서. 부를 때 `이름: 값`으로 골라 줄 수 있다.
- 함수는 값이다. 람다 `x => …`로 그 자리에서 만들고, 인자로 넘기고, 돌려준다.
- 함수는 만들어질 때 보이는 변수의 값을 복사한다. 최상위 `fn`은 상수만 읽을 수 있다.
- `x |> f(a)`는 `f(x, a)`. 대상이 첫 인자라는 규칙 덕분에 파이프가 이어진다. `f(a, _)`는 부분 적용.
- 목록 없는 반복은 재귀로 쓴다. 마지막 일이 자기 호출인 꼬리 재귀는 몇 번을 돌아도 스택을 쓰지 않는다. 언제 멈추는지부터 쓴다.

레퍼런스: [함수](../reference/language/functions.html), [변수와 스코프](../reference/language/scope.html), [파이프](../reference/language/operators.html#pipe), [타입 표기](../reference/language/types.html)(인자와 반환값의 타입을 적어 검사하는 법), [E0238](../reference/errors.html#E0238), [E0227](../reference/errors.html#E0227). 클로저가 값을 복사하는 방법과 꼬리 호출이 프레임을 다시 쓰는 방법은 [함수와 클로저](../vm/09-closures.html)에서 다룬다.
