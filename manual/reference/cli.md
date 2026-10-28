---
description: fung 명령(스크립트 실행, run, -e, check, test, fmt, dump, REPL, version, help)의 쓰는 법, 에러 메시지 읽는 법, 종료 코드.
lede: 실행 파일 <code>fung</code> 하나가 실행, 검사, 테스트, 포맷, 내부 단계 보기, REPL을 모두 맡는다. 이 페이지는 명령마다 쓰는 법과 옵션, 출력, 종료 코드를 모았다.
---
# 명령줄 도구

## 한눈에 {#overview}

터미널(셸)에서 `fung` 뒤에 무엇을 쓰느냐로 하는 일이 정해진다. `fung help`가 같은 목록을 보인다.

<table>
  <thead><tr><th>명령</th><th>하는 일</th></tr></thead>
  <tbody>
    <tr><td><a href="#run"><code>fung script.fung [args…]</code></a></td><td>스크립트 실행. <code>fung run script.fung [args…]</code>와 같다.</td></tr>
    <tr><td><a href="#eval"><code>fung -e 'code' [args…]</code></a></td><td>명령줄에 적은 코드 실행.</td></tr>
    <tr><td><a href="#repl"><code>fung</code></a></td><td>REPL(대화형으로 한 줄씩 실행).</td></tr>
    <tr><td><a href="#check"><code>fung check script.fung</code></a></td><td>실행하지 않고 문법·이름·import 검사, 경고 보이기.</td></tr>
    <tr><td><a href="#test"><code>fung test [path]</code></a></td><td><code>test</code> 블록 실행.</td></tr>
    <tr><td><a href="#fmt"><code>fung fmt [--check] path…|-</code></a></td><td>코드 모양을 한 가지 스타일로 고치기.</td></tr>
    <tr><td><a href="#dump"><code>fung dump ast|core|bytecode file</code></a></td><td>컴파일의 중간 단계 보기.</td></tr>
    <tr><td><a href="#version"><code>fung version</code></a></td><td>버전 보이기.</td></tr>
    <tr><td><a href="#version"><code>fung help</code></a></td><td>사용법 보이기(<code>-h</code>, <code>--help</code>도 같다).</td></tr>
  </tbody>
</table>

```plain
$ fung help
usage:
  fung <script.fung> [args...]            run a script (same as: fung run <script.fung>)
  fung                                    start the REPL
  fung -e '<code>'                        run one line of code
  fung check <script.fung>                check syntax and names without running
  fung fmt [--check] <path>...|-          format files in place (--check: list only; -: stdin)
  fung test [path]                        run tests
  fung dump ast|core|bytecode <script>    print an internal stage
  fung version                            print the version
  fung help                               print this text
```

모르는 명령이나 빠진 인자는 `error: …` 한 줄과 이 사용법을 표준 에러에 쓰고 종료 코드 3으로 끝난다.

```plain
$ fung bogus
error: unknown command `bogus`

usage:
  ...
```

## 스크립트 실행 {#run}

`fung script.fung`은 파일을 컴파일하고(문법·이름 검사) 처음부터 끝까지 실행한다. 첫 인자가 `.fung`으로 끝나거나 `/`가 들어 있으면 스크립트로 본다. 그래서 확장자 없는 스크립트는 `fung ./deploy`나 `fung run deploy`로 부른다. `-`로 시작하는 이름은 스크립트로 보지 않는다.

스크립트 이름 뒤의 인자는 모두 스크립트의 것이다. prelude 값 [`args`](prelude/program.html#fn-args)에 문자열 리스트로 들어간다(스크립트 이름은 들어 있지 않다). `--`로 시작해도 `fung`이 가로채지 않는다. 옵션으로 나누려면 [`cli.parse`](modules/cli.html#fn-cli-parse)를 쓴다.

```fung run args="--env prod web-1"
print(args, len(args))
```

표준 입력은 [`read_line`](prelude/io.html#fn-read_line)·[`read_lines`](prelude/io.html#fn-read_lines)로 읽으므로 파이프 중간에 둘 수 있다.

```shell
grep ERROR app.log | fung count.fung --by-hour > report.txt
```

파일 첫 줄에 `#!/usr/bin/env fung`을 쓰고 실행 권한을 주면 스크립트를 명령처럼 바로 실행할 수 있다. 그 줄은 fung에게는 그냥 주석이다.

```shell
$ head -1 tool.fung
#!/usr/bin/env fung
$ chmod +x tool.fung
$ ./tool.fung x
from shebang ["x"]
```

스크립트 파일을 읽지 못하면 `error: cannot read `nofile.fung`: no such file or directory`를 쓰고 종료 코드 3으로 끝난다. 컴파일 에러가 하나라도 있으면 아무것도 실행하지 않고 모든 에러를 보인 뒤 2로 끝난다. 실행 중 에러는 1이다([종료 코드](#exit-codes)).

**신호**: 실행 중 첫 Ctrl-C(SIGINT)나 SIGTERM은 **정지 요청**이다. [`stop_signal`](prelude/concurrency.html#fn-stop_signal)을 보는 코드가 없으면 프로그램을 취소하고(`with`의 정리는 돈다) 130(SIGINT)·143(SIGTERM)으로 끝난다. 보는 코드가 있으면 그 코드가 스스로 정리하고 끝낸다(예: [`http.serve`](modules/http.html#fn-http-serve)는 처리 중인 요청을 마치고 돌아온다). 두 번째 신호는 정리를 기다리지 않고 곧바로 끝낸다. 출력을 읽는 쪽이 사라지면(`fung x.fung | head -1`) 다른 셸 명령처럼 SIGPIPE로 조용히 끝난다(셸에는 141). SIGHUP(터미널이 닫힘)은 정리 없이 곧바로 129로 끝난다. REPL이 이렇게 끝나도 터미널 설정은 되돌려 놓는다.

## 코드 한 줄 실행: `-e` {#eval}

`fung -e '코드'`는 파일 없이 코드를 실행한다. 간단한 계산이나 셸 스크립트 안의 한 줄 처리에 쓴다. 코드 안의 줄바꿈도 된다. 뒤의 인자는 `args`가 된다. 에러 위치는 파일 이름 대신 `-e`로 보인다.

```shell
$ fung -e 'print(2 ** 10)'
1024
$ fung -e 'print(args)' a b
["a", "b"]
$ echo '{"n": 41}' | fung -e 'import json
print(json.parse(read_line()!)!.n + 1)'
42
```

`fung check -e '코드'`와 `fung dump core -e '코드'`처럼 `check`·`dump`도 `-e`를 받는다.

## REPL {#repl}

`fung`을 인자 없이 실행하면 **REPL**(Read-Eval-Print Loop)이 뜬다. 한 줄을 읽고(read), 실행하고(eval), 값을 보이고(print), 다시 묻는다(loop). 함수 하나를 시험해 보거나 내장 함수의 동작을 확인할 때 쓴다.

```plain
$ fung
> 1 + 2
3
> x = 10
> fn double(n) = n * 2
> double(x)
20
> "text"
"text"
> nums = [1, 2,
.   3]
> nums
[1, 2, 3]
> print("hi")
hi
nil
> nope
error[E0204]: undefined name `nope`
  --> <repl>:1:1
  |
1 | nope
  | ^^^^ not found in this scope
  |
> :quit
```

- 프롬프트는 `> `이다. 입력이 블록이나 괄호 중간에서 끝나면 `. ` 프롬프트로 이어서 받는다.
- 식을 입력하면 그 값을 리터럴 꼴로 보인다(문자열은 따옴표와 함께). 대입, `fn`, `import`, `test`, `assert`는 값을 보이지 않는다. `print(…)`는 출력한 뒤 자신의 값 `nil`을 보인다.
- 정의(변수, 함수, `import`)는 세션 동안 남는다. 같은 이름을 다시 정의할 수 있고, 앞서 만든 함수도 새 정의를 본다.
- 에러는 그 입력만 실패시키고 세션은 계속된다. 에러 위치는 `<repl>:줄:열`이고, 앞 입력에서 정의한 함수 안의 에러는 그 입력의 줄을 보인다. 한 입력 안에서 생긴 교착(deadlock, 모든 태스크가 서로를 기다림)도 그 입력만 실패시킨다. 다음 입력을 기다리는 동안 살아 있는 태스크는 교착으로 보지 않는다.
- `exit(n)`(띄운 태스크에서 부른 것도)은 세션을 코드 n으로 끝낸다. [`request_stop()`](prelude/concurrency.html#fn-request_stop)은 보는 쪽이 없으면 `exit(0)`처럼 끝낸다. 세션이 끝나면 남은 태스크는 취소된다(`with`의 정리는 돈다).

<table>
  <thead><tr><th>REPL 명령</th><th>하는 일</th></tr></thead>
  <tbody>
    <tr><td><code>:help</code></td><td>명령 목록을 보인다.</td></tr>
    <tr><td><code>:quit</code></td><td>끝낸다(빈 줄에서 Ctrl-D도 같다). 종료 코드 0.</td></tr>
    <tr><td><code>:load file.fung</code></td><td>파일을 이 세션에서 실행한다. 그 파일의 정의가 세션에 남는다.</td></tr>
  </tbody>
</table>

모르는 `:` 명령은 `unknown command :what`을 쓰고 계속한다.

터미널에서 띄우면 줄 편집이 된다: ←→로 커서 이동, Home·End(Ctrl-A·Ctrl-E), Backspace·Delete, ↑↓로 이전 입력, Ctrl-L로 화면 지우기, Ctrl-C로 지금 줄 버리기. 입력 기록은 `~/.fung_history`(`HOME`이 없으면 메모리에만)에 최근 1,000줄까지 남는다. 실행 중의 Ctrl-C는 그 입력을 취소하고 프롬프트로 돌아온다. 정리가 끝나지 않을 때 한 번 더 누르면 그 입력의 태스크를 버린다.

표준 입력이 터미널이 아니면(파이프, 파일) 줄 편집 없이 한 줄씩 읽는다. 그래서 `fung < inputs.txt`로 입력을 차례로 넣어 볼 수 있다.

## 검사: `check` {#check}

`fung check script.fung`은 **실행하지 않고** 컴파일만 해서 문법 에러, 없는 이름, 잘못된 `import`를 찾는다. 경고(쓰지 않는 import·지역 변수, prelude 이름을 가림)는 `check`와 `test`에서만 보인다. 서버에 올리기 전이나 편집기의 저장 훅에서 쓴다. 파일은 하나만 받고 디렉터리는 받지 않는다. `fung check -e '코드'`도 된다.

```shell
$ fung check w.fung
warning[W0101]: unused import `fs`
  --> w.fung:1:1
  |
1 | import fs
  | ^^
  |
warning[W0103]: `len` shadows a prelude function
  --> w.fung:6:1
  |
6 | len = 3
  | ^^^
  |
$ echo $?
0
```

에러가 없으면(경고만 있어도) 0, 컴파일 에러가 있으면 모두 보이고 2, 파일을 읽지 못하면 3이다. 검사는 실행하지 않으므로 런타임 에러(0으로 나누기, 없는 키, 버려진 `Err`)는 찾지 못한다.

## 테스트: `test` {#test}

`fung test [path]`는 파일 안의 `test "이름": … ;;` 블록을 돌린다([테스트 문법](language.html#testing)). `path`가 없으면 현재 디렉터리다.

- `path`가 파일이면 그 파일만, 디렉터리면 그 아래의 `.fung` 파일을 모두 이름 순서로 돈다. `_`나 `.`으로 시작하는 디렉터리(`_fixtures`, `.git`)에는 들어가지 않는다. 그 아래에서 디렉터리를 가리키는 링크는 따라가지 않는다(`.fung` 파일을 가리키는 링크는 파일처럼 센다). `fmt`와 같은 규칙이다.
- `test` 블록이 없는 파일은 실행하지도, 목록에 올리지도 않는다.
- 파일마다 따로 실행된다. 최상위 코드가 먼저 돌고(그 출력도 보인다), 이어서 `test` 블록이 차례로 돈다. 한 파일이 띄운 태스크는 다음 파일로 넘어가지 않는다.
- 블록마다 `ok   파일: 이름`이나 `FAIL 파일: 이름`과 그 에러를 쓰고, 끝에 `N passed, M failed`를 쓴다.
- 읽지 못한 파일, 컴파일하지 못한 파일은 `FAIL 파일: <load>`로 실패 하나로 센다.
- 파일 안에서 `exit(n)`이 불리거나(띄운 태스크에서도) 교착이 생기면 그 파일의 나머지 블록을 건너뛰고 실패 하나로 센다. 그 `n`은 명령의 종료 코드가 되지 않는다.
- **태스크의 순서를 섞는다.** 언어는 함께 띄우거나 함께 깨어난 태스크가 도는 순서를 약속하지 않는다. 그래서 `fung test`는 그 순서를 씨앗에서 뽑아 섞어, 순서에 기대는 시험이 언젠가 드러나게 한다(`fung run`은 섞지 않는다). 씨앗은 환경 변수 `FUNG_SCHED_SEED`(정수)를 주면 그것을 쓰고, 없으면 실행마다 고른다. 고른 씨앗은 실패가 있을 때만 표준 에러에 `scheduler seed N: FUNG_SCHED_SEED=N runs the tasks in the same order again`으로 알린다. 같은 씨앗을 주면 같은 순서로 다시 돈다.

```shell
$ fung test
ok   lib/text.fung: shout adds a bang
FAIL lib/text.fung: this one fails
  error[E0807]: assertion failed: shout("a") == "a!"
    --> lib/text.fung:14:3
     |
  14 |   assert shout("a") == "a!"
     |   ^
     |
    shout("a") = "A!"
   call path (innermost first):
     lib/text.fung:14 test: this one fails
ok   math.fung: add
2 passed, 1 failed
$ echo $?
1
```

`assert`가 실패하면 식과 함께 그 안의 하위 식의 값(`shout("a") = "A!"`)을 보인다. 종료 코드는 컴파일하지 못한 파일이 있으면 2, 아니면 실패한 테스트가 있으면 1, 모두 통과하면 0이다. 인자가 둘 이상이거나 경로가 없으면 3이다. 도중에 신호가 오면 그때 돌던 파일에 정지 요청을 보내고, 다음 파일은 돌리지 않고 128 + 신호 번호로 끝난다.

## 포맷: `fmt` {#fmt}

`fung fmt path…`는 코드의 공백·들여쓰기·정렬을 한 가지 스타일로 고친다. 팀의 모든 코드가 같은 모양이면 읽기 쉽고, 모양을 두고 다툴 일이 없다. 줄바꿈 위치는 그대로 두고, 줄 안의 공백과 들여쓰기, 연속된 줄의 정렬(`match` 갈래의 `=>`, 맵의 값 등)만 맞춘다. 정렬은 화면 칸 수로 맞춘다(한글·한자는 2칸).

```plain
$ cat ugly.fung
x=1
fn f(a,b)=a+b
print( f(x,2) )
$ fung fmt ugly.fung
ugly.fung
$ cat ugly.fung
x = 1
fn f(a, b) = a + b
print(f(x, 2))
```

- 파일을 그 자리에서 고치고, 고친 파일의 경로를 한 줄씩 쓴다. 이미 맞는 파일은 건드리지도 쓰지도 않는다.
- 고칠 때는 옆에 임시 파일을 쓰고 이름을 바꿔 덮으므로, 도중에 죽거나 디스크가 차도 파일이 반만 남지 않는다. 권한은 그대로이고, 링크는 링크로 남는다(가리키는 파일을 고친다). 하드 링크가 여럿이거나 남의 파일이면 그 자리에 바로 쓴다.
- 디렉터리를 주면 [`test`와 같은 규칙](#test)으로 그 아래의 `.fung` 파일을 모두 고친다: `fung fmt .`
- `--check`는 고치지 않고 고쳐야 할 파일만 쓴다. 하나라도 있으면 종료 코드 1이라 CI에서 쓴다.
- `-`는 표준 입력을 읽어 고친 결과를 표준 출력에 쓴다(편집기 연동용). `--check -`는 고칠 게 있으면 `<stdin>`을 쓰고 1, 없으면 아무것도 쓰지 않고 0이다.
- 문법 에러가 있는 파일은 에러를 보이고 그대로 둔다(종료 코드 2).

```shell
$ fung fmt --check .
ugly.fung
$ echo $?
1
$ fung fmt - < ugly.fung > fixed.fung
```

종료 코드는 문법 에러가 있으면 2, `--check`에서 고칠 파일이 있거나 읽고 쓰지 못한 파일이 있으면 1, 아니면 0, 경로가 없거나 모르는 옵션이면 3이다.

## 내부 단계 보기: `dump` {#dump}

`fung dump 단계 file`은 fung이 소스를 실행하기 전에 거치는 중간 결과를 보인다. 언어가 어떻게 만들어지는지 공부하거나 버그를 좁힐 때 쓴다. 단계는 셋이다.

- `ast`: 파서가 만든 구문 트리(쓴 그대로의 모양).
- `core`: 문법 설탕을 걷어 낸 코어 트리(이름이 `global`·`local`·`prelude` 칸으로 풀린다).
- `bytecode`: VM이 실행하는 명령의 목록. 함수마다 `== 이름 ==` 아래에 `번호 줄:열 명령 인자 ; 설명`이 나온다.

```dump bytecode
fn double(x) = x * 2
print(double(21))
```

`fung dump core -e 'x = 1'`처럼 `-e`도 받는다. 컴파일 에러가 있으면 에러를 보이고 2로 끝난다. 각 단계의 뜻은 [fung VM](../vm/01-how-programs-run.html) 책에서 다룬다.

## 버전과 도움말 {#version}

`fung version`은 `fung 0.4.0`처럼 버전 한 줄을 쓴다. 릴리스가 아닌 개발 빌드는 `fung 0.0.0-dev`다. `fung help`(`-h`, `--help`)는 [위의 사용법](#overview)을 표준 출력에 쓰고 0으로 끝난다. `--version`은 모르는 명령이다.

## 에러 메시지 읽는 법 {#errors}

fung의 에러는 모두 같은 꼴이다. 내부 구현의 패닉이나 스택 추적은 나오지 않는다.

```fung run exit=1 error=E0502
fn ratio(done, total) = done * 100 // total

fn report(jobs) = "${ratio(len(jobs), 0)}% done"

print(report(["a", "b"]))
```

1. **머리줄** `error[E0502]: division by zero`: 번호와 메시지. 번호로 [에러 카탈로그](errors.html)에서 뜻과 고치는 법을 찾는다. 경고는 `warning[W0101]`이다.
2. **위치** `--> example.fung:1:31`: 파일, 줄, 열(1부터, 글자 단위).
3. **발췌**: 그 줄과 `^` 표시. 표시 옆에 짧은 설명이 붙기도 한다(`not found in this scope`).
4. **호출 경로** `call path (innermost first)`(런타임 에러만): 에러가 난 함수부터 바깥으로, 각 호출의 파일·줄과 함수 이름. `<entry>`는 파일의 최상위 코드, `<lambda>`는 이름 없는 함수, `test: 이름`은 테스트 블록이다. 꼬리 호출로 사라진 단계는 `(3 tail calls elided)`처럼 수만 보인다.
5. 덧붙는 줄: `help:`(고치는 방법), `note:`(관련된 다른 위치, 예: 실패한 태스크의 `scope opened at …`).

```fung run exit=1 error=E0508
m = {host: "web-1"}
print(m.port)
```

컴파일 에러는 실행 전에 한꺼번에 나오고 호출 경로가 없다.

```plain
$ cat typo.fung
total = 0
print(totl)
$ fung typo.fung
error[E0204]: undefined name `totl`
  --> typo.fung:2:7
  |
2 | print(totl)
  |       ^^^^ not found in this scope
  |
 help: a similar name exists: `total`
$ echo $?
2
```

에러 번호의 첫 두 자리가 분류다.

<table>
  <thead><tr><th>번호</th><th>분류</th></tr></thead>
  <tbody>
    <tr><td><a href="errors.html#e01"><code>E01xx</code></a></td><td>문법(렉서·파서)</td></tr>
    <tr><td><a href="errors.html#e02"><code>E02xx</code></a></td><td>이름과 스코프</td></tr>
    <tr><td><a href="errors.html#e03"><code>E03xx</code></a></td><td>모듈과 import</td></tr>
    <tr><td><a href="errors.html#e04"><code>E04xx</code></a></td><td>호출과 타입 표기(인자 수, 인자 타입)</td></tr>
    <tr><td><a href="errors.html#e05"><code>E05xx</code></a></td><td>런타임 연산(타입 불일치, 0 나누기, 오버플로, 범위 밖 인덱스, 없는 키)</td></tr>
    <tr><td><a href="errors.html#e06"><code>E06xx</code></a></td><td>패턴과 결과값(맞는 패턴 없음, <code>!</code>가 <code>Err</code>를 만남, 버려진 <code>Err</code>)</td></tr>
    <tr><td><a href="errors.html#e07"><code>E07xx</code></a></td><td>동시성(취소, 교착, 닫힌 태스크 범위, atom 규칙)</td></tr>
    <tr><td><a href="errors.html#e08"><code>E08xx</code></a></td><td>한도와 환경(스택 깊이, <code>fail</code>, <code>assert</code>, 내부 오류)</td></tr>
    <tr><td><a href="errors.html#w01"><code>W01xx</code></a></td><td>경고(<code>check</code>·<code>test</code>에서만): 안 쓰는 import·변수, prelude 이름 가리기</td></tr>
  </tbody>
</table>

## 종료 코드 {#exit-codes}

프로그램이 끝날 때 운영체제에 돌려주는 숫자가 **종료 코드**(exit code)다. 셸에서는 `$?`로 보고, `&&`·`||`·`set -e`가 이것으로 성공과 실패를 가른다. 0이 성공이다.

<table>
  <thead><tr><th>종료 코드</th><th>뜻</th></tr></thead>
  <tbody>
    <tr><td><code>0</code></td><td>정상 종료(정지 요청을 받아 스스로 정리하고 끝난 경우도)</td></tr>
    <tr><td><code>1</code></td><td>실행 중 실패: 런타임 에러, <code>!</code>가 <code>Err</code>를 만남, 버려진 <code>Err</code>, 끝내 아무도 확인하지 않은 태스크의 실패, 테스트 실패, <code>fmt --check</code>에서 고칠 파일이 있음</td></tr>
    <tr><td><code>2</code></td><td>컴파일 실패: 문법·이름·import 에러</td></tr>
    <tr><td><code>3</code></td><td><code>fung</code> 명령을 잘못 씀: 모르는 명령·옵션, 빠진 인자, 읽을 수 없는 스크립트 파일</td></tr>
    <tr><td><code>n</code></td><td>스크립트가 <code>exit(n)</code>을 부름. 운영체제는 0~255만 전하므로 256으로 나눈 나머지가 된다(<code>exit(300)</code>은 44, <code>exit(-1)</code>은 255)</td></tr>
    <tr><td><code>130</code> <code>143</code></td><td>SIGINT·SIGTERM으로 취소됨(128 + 신호 번호). 두 번째 신호로 곧바로 끝난 경우도 같다</td></tr>
    <tr><td><code>141</code></td><td>출력을 읽는 쪽이 사라짐(SIGPIPE, 셸이 보이는 값)</td></tr>
  </tbody>
</table>

```fung run exit=4
import fs

match fs.read("config.txt"):
  Ok(_)  => print("loaded")
  Err(e) =>
    eprint("fatal:", e.message)
    exit(4)
;;
```

## 환경 변수 {#env}

`fung`이 읽는 환경 변수다.

<table>
  <thead><tr><th>변수</th><th>쓰임</th></tr></thead>
  <tbody>
    <tr><td><code>HOME</code></td><td>REPL 기록 파일 <code>~/.fung_history</code>의 위치</td></tr>
    <tr><td><code>TZ</code>, <code>TZDIR</code></td><td>로컬 시간대(<a href="modules/time.html#concepts"><code>time</code></a>)</td></tr>
    <tr><td><code>TMPDIR</code></td><td>임시 디렉터리(<a href="modules/fs.html#fn-fs-with_temp_dir"><code>fs.with_temp_dir</code></a>)</td></tr>
    <tr><td><code>PATH</code></td><td>외부 명령 찾기(<a href="prelude/process.html#fn-exec"><code>exec</code></a>)</td></tr>
    <tr><td><code>HTTP_PROXY</code>, <code>HTTPS_PROXY</code>, <code>NO_PROXY</code></td><td>HTTP 프록시(<a href="modules/http.html#promises"><code>http</code></a>)</td></tr>
    <tr><td><code>SSL_CERT_FILE</code>, <code>SSL_CERT_DIR</code></td><td>https 루트 인증서</td></tr>
  </tbody>
</table>

스크립트에서는 [`env`](prelude/process.html#fn-env)로 아무 환경 변수나 읽는다. 처음 설치하고 실행하는 과정은 [2장 설치와 첫 프로그램](../learn/02-first-program.html)에 있다.
