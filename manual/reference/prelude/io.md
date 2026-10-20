---
lede: 프로그램이 바깥과 글자를 주고받는 세 통로, 곧 표준 출력·표준 에러·표준 입력을 다룬다. 결과는 <code>print</code>로 쓰고, 진행 상황과 경고는 <code>eprint</code>로, 들어오는 줄은 <code>read_line</code>·<code>read_lines</code>로 읽는다.
description: fung prelude 출력과 입력: print, eprint, read_line, read_lines, dbg.
---
# 출력과 입력

## 표준 입출력이란 {#streams}

명령줄에서 도는 프로그램은 처음부터 열린 통로 세 개를 받는다. **표준 출력**(stdout)은 프로그램의 결과가 나가는 곳이고, **표준 에러**(stderr)는 진단 메시지가 나가는 곳이며, **표준 입력**(stdin)은 들어오는 글자가 오는 곳이다. 터미널에서 그냥 돌리면 셋 다 터미널에 이어져 있어서 출력과 에러가 한 화면에 섞여 보인다. 하지만 `fung run report.fung > out.txt`처럼 출력을 파일로 돌리면 표준 에러는 여전히 화면에 남는다. 그래서 다른 프로그램이 읽을 결과는 표준 출력에, 사람이 볼 진행 상황·경고·에러는 표준 에러에 쓰는 것이 규칙이다.

표준 입력은 파이프(`cat hosts.txt | fung run check.fung`)나 리다이렉트(`fung run check.fung < hosts.txt`)로 다른 곳의 글자를 받는다. 터미널에서 아무것도 이어 주지 않고 돌리면 사용자가 치는 글자가 들어온다.

fung에서 출력은 실패하지 않는 연산으로 다루고(쓰기에 실패하면 프로그램이 끝난다), 입력은 끝이나 닫힌 파일처럼 정상적으로 일어나는 일이 있으므로 [결과값](results.html)(`Ok`/`Err`)으로 돌려준다. 파일을 직접 열어 읽고 쓰는 함수는 [`fs` 모듈](../modules/fs.html)에 있다.

## 함수 {#functions}

:::fn print(...args)

인자들을 공백 하나로 이어 한 줄로 표준 출력에 쓴다.

#### 설명

인자는 몇 개든 받는다(`...args`). 각 인자를 글자로 바꾼 뒤 사이에 공백 하나를 넣고, 끝에 줄바꿈을 붙여 쓴다. 인자가 없으면 빈 줄 하나를 쓴다.

글자로 바꾸는 규칙은 [`str`](convert.html#fn-str)과 같다. 맨 바깥의 문자열은 따옴표 없이 내용 그대로 찍고, 나머지 값은 소스에 쓰는 리터럴 모양으로 찍는다. 그래서 리스트나 맵 **안의** 문자열은 따옴표가 붙는다(`["a"]`). 심볼은 `'ok`, 기간은 `1m30s`, 결과값은 `Ok("x")`처럼 보인다. 문자열 안의 줄바꿈은 그대로 줄을 바꾼다.

태스크 여럿이 동시에 `print`해도 한 호출의 줄은 섞이지 않는다. 한 번의 `print`가 한 줄을 통째로 쓰기 때문이다.

구분자를 바꾸고 싶으면 [`join`](strings.html#fn-join)으로 먼저 문자열을 만들고, 값을 끼워 넣을 때는 문자열 보간(`"${x}"`)을 쓴다.

#### 매개변수

- `args`: 찍을 값들. 어떤 타입이든 된다.

#### 돌려주는 값

`nil`.

#### 에러

인자 때문에 실패하지는 않는다. 표준 출력을 읽던 쪽이 먼저 끝나면(`fung run x.fung | head -1`) 다른 명령줄 도구처럼 에러 메시지 없이 조용히 끝난다(SIGPIPE, 셸이 보는 종료 코드 141).

#### 예제

**여러 값을 한 줄로**

```fung run
print("total:", 3, [1, 2], 'ok)
```

**값의 모양: 바깥 문자열만 따옴표가 없다**

```fung run
print("web-1", ["web-1"], {host: "web-1", port: 22}, (1, "a"), nil, 1.5, 90s, Ok("x"))
```

**구분자를 바꿀 때는 join, 끼워 넣을 때는 보간**

```fung run
hosts = ["web-1", "web-2", "db-1"]
print(join(hosts, ","))
print("${len(hosts)} hosts")
print()
print("(위는 빈 줄)")
```

#### 같이 보기

[`eprint`](#fn-eprint), [`str`](convert.html#fn-str), [`join`](strings.html#fn-join), [`dbg`](#fn-dbg), [`fs.write_line`](../modules/fs.html#fn-fs-write_line)

:::

:::fn eprint(...args)

`print`와 같은 모양으로 표준 에러에 쓴다.

#### 설명

인자를 잇는 규칙, 줄바꿈, 줄이 섞이지 않는 성질은 [`print`](#fn-print)와 같고, 나가는 곳만 표준 에러다. 진행 상황, 경고, 실패 이유처럼 "결과가 아닌 것"을 쓴다. 그러면 결과를 파일이나 다음 명령으로 넘길 때 메시지가 섞여 들어가지 않는다.

`eprint`는 프로그램을 멈추지 않는다. 실패를 알리고 끝내려면 `eprint` 뒤에 [`exit(1)`](program.html#fn-exit)을 부르고, "여기 오면 버그"라면 [`fail`](program.html#fn-fail)을 쓴다.

#### 매개변수

- `args`: 찍을 값들.

#### 돌려주는 값

`nil`.

#### 에러

없다.

#### 예제

**결과와 진단을 나눠 쓰기**

```fung run
eprint("checking 2 hosts...")
print("web-1 ok")
print("web-2 ok")
```

이 매뉴얼의 출력 칸은 두 통로를 한데 모아 보여 준다. 셸에서 `fung run x.fung > out.txt`로 돌리면 `out.txt`에는 `web-1 ok`·`web-2 ok` 두 줄만 들어가고 `checking …`은 화면에 남는다.

**실패를 알리고 종료 코드 1로 끝내기**

```fung run exit=1
port = "abc"
match parse_int(port):
  Ok(n)  => print("port", n)
  Err(e) =>
    eprint("bad port:", e.message)
    exit(1)
;;
```

#### 같이 보기

[`print`](#fn-print), [`exit`](program.html#fn-exit), [`fail`](program.html#fn-fail)

:::

:::fn read_line(f = nil)

한 줄을 읽어 `Ok(줄)`을, 더 읽을 것이 없으면 `Ok(nil)`을 돌려준다.

#### 설명

`f`가 `nil`(기본값)이면 표준 입력에서, [`fs.open`](../modules/fs.html#fn-fs-open)으로 연 파일 핸들이면 그 파일에서 읽는다. 읽은 줄의 끝에 붙은 `\n`과 그 앞의 `\r`(Windows식 줄 끝)은 떼고 돌려준다. 그래서 빈 줄은 `Ok("")`이고 끝은 `Ok(nil)`이라 둘이 구별된다.

마지막 줄에 줄바꿈이 없어도 그 줄을 돌려준다. UTF-8이 아닌 바이트는 잘못된 부분마다 U+FFFD(`�`) 한 글자로 바뀐다(에러가 아니다).

한 줄씩 읽으면 입력 전체를 메모리에 올리지 않아도 되고, 사용자의 대답을 하나씩 받는 대화형 스크립트도 만들 수 있다. 입력을 다 읽어 리스트로 다룰 거라면 [`read_lines`](#fn-read_lines)가 간단하다.

#### 매개변수

- `f`: `nil`(표준 입력) 또는 파일 핸들.

#### 돌려주는 값

`Ok(문자열)`, 입력이 끝났으면 `Ok(nil)`.

#### 에러

- 닫힌 파일 핸들이면 `Err({kind: 'closed, message})`.
- 쓰기용으로 연 핸들이면 `Err({kind: 'invalid, message})`(`file … is not open for reading`).
- 그 밖의 읽기 실패는 [`fs`](../modules/fs.html)의 다른 함수와 같은 `Err`(`'io` 등).
- `f`가 파일도 `nil`도 아니면 런타임 에러 E0403.

#### 예제

**표준 입력을 끝까지 한 줄씩**

```file hosts.txt
web-1
web-2

db-1
```

```fung run stdin=hosts.txt
fn loop(n):
  match read_line()!:
    nil  => n
    ""   => loop(n)
    line =>
      print("host:", line)
      loop(n + 1)
  ;;
;;

print(loop(0), "hosts")
```

빈 줄은 `""`이라 건너뛰고, 끝(`nil`)에서 센 수를 돌려준다. fung에는 `while`이 없으므로 이런 반복은 꼬리 재귀로 쓴다([반복 패턴](loops.html)).

**파일 핸들에서 읽기, 닫힌 핸들**

```file app.log
start
ready
```

```fung run
import fs

f = fs.open("app.log")!
print(read_line(f))
print(read_line(f))
print(read_line(f))
fs.close(f)!
print(read_line(f))
```

#### 같이 보기

[`read_lines`](#fn-read_lines), [`fs.open`](../modules/fs.html#fn-fs-open), [`fs.read_lines`](../modules/fs.html#fn-fs-read_lines), [`lines`](strings.html#fn-lines)

:::

:::fn read_lines(f = nil)

남은 줄을 모두 읽어 `Ok(리스트)`로 돌려준다.

#### 설명

[`read_line`](#fn-read_line)을 끝까지 부른 것과 같다. 줄 끝의 `\n`·`\r`을 떼는 규칙, 마지막 줄바꿈이 빈 줄을 만들지 않는 규칙, 잘못된 UTF-8을 `�`로 바꾸는 규칙이 모두 같다. 입력이 비어 있으면 `Ok([])`다. 이미 몇 줄을 `read_line`으로 읽었다면 그 뒤의 줄만 돌려준다.

경로로 파일 전체를 줄 리스트로 읽을 때는 [`fs.read_lines(path)`](../modules/fs.html#fn-fs-read_lines)가 열고 닫는 일까지 해 준다. 이미 문자열로 가진 글을 줄로 나눌 때는 [`lines`](strings.html#fn-lines)다.

#### 매개변수

- `f`: `nil`(표준 입력) 또는 파일 핸들.

#### 돌려주는 값

`Ok(문자열의 리스트)`.

#### 에러

[`read_line`](#fn-read_line)과 같다: 닫힌 핸들은 `Err({kind: 'closed, message})`, 쓰기용 핸들은 `Err({kind: 'invalid, message})`, 잘못된 인자는 E0403.

#### 예제

**파이프로 받은 목록을 정리하기**

```file names.txt
db-1
web-2
web-1
web-2
```

```fung run stdin=names.txt
names = read_lines()!
print(len(names), "lines")
print(names |> unique |> sort)
```

**빈 입력은 빈 리스트**

```fung run
print(read_lines())
```

이 예제는 표준 입력에 아무것도 이어 주지 않고 돌렸다.

#### 같이 보기

[`read_line`](#fn-read_line), [`lines`](strings.html#fn-lines), [`fs.read_lines`](../modules/fs.html#fn-fs-read_lines)

:::

:::fn dbg(x)

`[파일:줄] 식 = 값`을 표준 에러에 찍고 `x`를 그대로 돌려준다.

#### 설명

디버깅용이다. 값을 돌려주기 때문에 식 한가운데에 끼워 넣어도 프로그램의 동작이 바뀌지 않는다: `total = dbg(a + b) * 2`. 파이프 중간에도 넣을 수 있다: `xs |> dbg |> sort`. 찍히는 **식**은 소스에 쓴 글자 그대로이고, 컴파일러가 부른 자리의 파일·줄과 함께 채워 넣는다. 그래서 `dbg`를 여러 군데 넣어도 어느 것이 찍었는지 바로 안다.

표준 에러에 쓰므로 결과 출력을 더럽히지 않는다. 다 고친 뒤에는 지운다.

#### 매개변수

- `x`: 살펴볼 값.

#### 돌려주는 값

`x` 그대로.

#### 에러

없다.

#### 예제

**식 한가운데에 끼우기**

```fung run
sizes = [120, 30, 450]
total = dbg(sum(sizes)) * 2
print(total)
```

**파이프 중간에 끼우기**

```fung run
top = [3, 9, 1] |> dbg |> sort |> reverse
print(top)
```

#### 같이 보기

[`eprint`](#fn-eprint), [테스트와 디버깅](../language.html#testing)

:::
