---
description: fung을 설치하고, 한 줄 코드·스크립트 파일·REPL로 실행하고, fung check로 검사하고, 에러 메시지를 읽는 법.
lede: 설치부터 첫 스크립트까지. 이 장을 마치면 매뉴얼의 예제를 직접 돌려 볼 수 있다.
---
# 설치와 첫 프로그램

:::goals

- `fung`을 설치하고 설치를 확인하는 법
- 코드를 실행하는 세 가지 방법: `fung -e`, 스크립트 파일, REPL
- 스크립트에 명령줄 인자를 넘기는 법과 주석
- `fung check`로 실행하지 않고 검사하는 법
- 에러 메시지를 읽는 법과 종료 코드

:::

## 설치 {#install}

fung은 실행 파일 `fung` 하나다. Linux와 macOS(amd64, arm64)에서는 설치 스크립트로 받는다. 스크립트는 GitHub 릴리스에서 내 컴퓨터에 맞는 파일을 받아 체크섬(`SHA256SUMS`)을 대조한 뒤 `~/.local/bin`에 둔다. 관리자 권한(sudo)은 필요 없다.

```plain
curl -fsSL https://raw.githubusercontent.com/tunarider/fung/main/install.sh | sh
```

설치가 끝나면 터미널에서 버전을 찍어 본다.

```plain
$ fung version
fung 0.4.0
```

`fung: command not found`가 나오면 `~/.local/bin`이 `PATH`에 없는 것이다. 쉘 설정 파일(`~/.bashrc`, `~/.zshrc`)에 `export PATH="$HOME/.local/bin:$PATH"`를 넣고 터미널을 새로 연다.

- 특정 버전이나 다른 위치가 필요하면 환경 변수로 고른다: `FUNG_VERSION=0.4.0`, `FUNG_INSTALL_DIR=/usr/local/bin`.
- 같은 명령을 다시 실행하면 새 버전으로 바뀐다. 지우려면 그 파일 하나를 지우면 된다.
- Rust 도구가 있으면 저장소에서 직접 빌드할 수도 있다(`cargo build --release --locked`, 실행 파일은 `target/release/fung`). 이렇게 만든 것은 버전이 `0.0.0-dev`로 찍힌다.

:::box learn 터미널, 명령, PATH

**터미널(terminal)**은 글자로 명령을 치는 창이다. 이 매뉴얼에서 `$`로 시작하는 줄은 터미널에 치는 명령이고(`$`는 치지 않는다), 그 아래 줄은 명령이 찍은 결과다. `fung version`처럼 명령 이름만 쳐도 실행되는 것은, 쉘이 `PATH` 환경 변수에 적힌 디렉터리들에서 그 이름의 실행 파일을 찾아 주기 때문이다.

:::

## 한 줄 실행: fung -e {#eval}

가장 빠른 방법은 `-e` 뒤에 코드를 따옴표로 감싸 주는 것이다.

```plain
$ fung -e 'print("hello, world!")'
hello, world!
```

`print`는 괄호 안의 값을 화면에 찍는 함수다. 값을 여러 개 주면 공백으로 띄워 한 줄에 찍는다. 함수는 5장에서 자세히 배운다. 지금은 "이름 뒤에 괄호를 붙여 부르면 일을 하는 것"이라고만 알아 두자.

```fung run
print("disk", 91, "%")
print(24 * 60 * 60)
```

## 스크립트 파일 {#script}

여러 줄짜리 프로그램은 파일에 쓴다. 아무 텍스트 편집기로 `hello.fung`을 만들고 다음을 적는다.

```fung run
# 첫 fung 스크립트
print("hello, world!")
print("1 + 2 =", 1 + 2)
```

그리고 실행한다. `fung hello.fung`과 `fung run hello.fung`은 같다.

```plain
$ fung hello.fung
hello, world!
1 + 2 = 3
```

몇 가지 규칙이 이미 보인다.

- 프로그램은 **위에서 아래로** 한 줄씩 실행된다.
- `#`부터 그 줄 끝까지는 **주석(comment)**이다. 사람을 위한 메모라서 실행되지 않는다. fung의 주석은 `#` 하나뿐이다.
- 한 줄에 문장 하나를 쓴다. 한 줄에 둘을 쓰고 싶으면 `;`로 나눈다(`print(1); print(2)`). 들여쓰기는 의미가 없다.

### 명령줄 인자 {#args}

스크립트 뒤에 적은 단어들은 **명령줄 인자(command-line argument)**라고 하고, 스크립트 안에서 `args`라는 이름으로 받는다. `args`는 문자열의 목록이다(목록은 7장에서 배운다).

```fung run args="web-1 db-1"
print("arguments:", args)
print("how many:", len(args))
```

위 예제는 `fung example.fung web-1 db-1`처럼 실행한 결과다. 인자가 없을 때를 대비해 기본값을 주려면 `??`를 쓴다. `first(args)`는 첫 인자이고, 인자가 하나도 없으면 "없음"을 뜻하는 `nil`이다. `a ?? b`는 `a`가 `nil`이면 `b`다(3장과 9장에서 더 다룬다).

```fung run
name = first(args) ?? "world"
print("hello, ${name}!")
```

`"…${name}…"`은 문자열 안에 값을 끼워 넣는 문법이다(6장).

### 실행 파일로 만들기 {#shebang}

파일 첫 줄에 `#!/usr/bin/env fung`을 적고 실행 권한을 주면, 쉘 스크립트처럼 이름만으로 실행할 수 있다. fung에게 이 줄은 그냥 주석이다.

```plain
$ cat greet.fung
#!/usr/bin/env fung
name = first(args) ?? "world"
print("hello, ${name}!")
$ chmod +x greet.fung
$ ./greet.fung ops
hello, ops!
```

## REPL: 대화하며 실행하기 {#repl}

`fung`을 인자 없이 실행하면 **REPL**이 열린다. 한 줄을 읽고(Read), 계산하고(Eval), 결과를 찍고(Print), 다시 기다리는(Loop) 대화형 실행기다. 함수가 어떻게 동작하는지 잠깐 확인할 때 가장 편하다.

```plain
$ fung
> 1 + 2
3
> hosts = ["web-1", "web-2"]
> len(hosts)
2
> upper("warn")
"WARN"
> print("hi")
hi
nil
> fn double(x):
.   x * 2
. ;;
> double(21)
42
> :quit
```

- `>`는 입력을 기다린다는 표시다. 입력한 식의 값을 바로 찍는다. 문자열은 따옴표를 붙여 찍어서, `"WARN"`이 문자열이라는 것이 보인다.
- `hosts = …` 같은 대입은 값을 찍지 않는다. 대입한 이름은 세션이 끝날 때까지 남는다.
- `print("hi")`는 `hi`를 찍고, `print` 자신의 값인 `nil`("돌려줄 값 없음")도 찍힌다.
- 블록이 덜 닫히면 `.`로 다음 줄을 더 받는다. `;;`로 닫으면 실행된다.
- `:help`는 명령 목록, `:load 파일.fung`은 파일을 이 세션에서 실행(정의가 남는다), `:quit`나 <kbd>Ctrl</kbd>+<kbd>D</kbd>는 종료다. 입력 기록은 `~/.fung_history`에 남아 위쪽 화살표로 다시 꺼낼 수 있다.

REPL에서 에러가 나도 세션은 계속된다. 고쳐서 다시 치면 된다.

## fung check: 실행하지 않고 검사하기 {#check}

`fung check 파일`은 실행하지 않고 문법, 이름, `import`만 검사한다. 파일을 지우거나 서버에 명령을 보내는 스크립트를 고친 뒤, 돌리기 전에 오타부터 잡고 싶을 때 쓴다. 문제가 없으면 아무것도 찍지 않고 끝난다.

```plain
$ fung check hello.fung
$ echo $?
0
```

`check`는 에러가 아닌 **경고(warning)**도 보여 준다. 예를 들어 가져오고 쓰지 않은 모듈이 있으면 이렇게 알린다. 경고는 실행을 막지 않는다.

```plain
warning[W0101]: unused import `fs`
  --> w.fung:1:1
  |
1 | import fs
  | ^^
  |
```

## 에러 메시지 읽기 {#errors}

에러 메시지는 적이 아니라 안내문이다. fung의 에러는 모두 같은 모양이라, 한 번 읽는 법을 익히면 된다. 아래 스크립트는 둘째 줄에 오타가 있다.

```fung run exit=2 error=E0204
print("start")
prnt("disk check")
```

한 줄씩 읽는다.

1. `error[E0204]`: 에러 번호. [에러 카탈로그](../reference/errors.html#E0204)에서 이 번호로 뜻과 고치는 법을 찾을 수 있다. 번호는 한 번 붙이면 바뀌지 않는다.
2. `undefined name` 뒤의 `prnt`: 무엇이 잘못됐는지. "`prnt`라는 이름은 정의된 적이 없다."
3. `--> example.fung:2:1`: 어디인지. 파일 이름(이 매뉴얼의 예제는 모두 `example.fung`이라는 파일로 돈다), 줄 번호, 그 줄의 몇 번째 글자인지.
4. 소스 발췌와 `^^^^`: 문제가 된 글자에 밑줄을 긋는다.
5. `help:`: 고치는 방법에 대한 힌트. 있을 때만 나온다.

눈여겨볼 점은 첫 줄의 `print("start")`도 **실행되지 않았다**는 것이다. 이름이 틀린 것 같은 실수는 실행 전 검사에서 잡히고, 그러면 프로그램은 한 줄도 돌지 않는다. 이런 에러를 **컴파일 에러**라고 한다.

반대로 실행해 봐야 알 수 있는 문제도 있다. 아래 스크립트는 문법은 맞지만, 실행 중에 문자열과 숫자를 더하려다 멈춘다. 이런 에러를 **런타임 에러(runtime error)**라고 한다. 앞 줄은 이미 실행되어 출력이 남아 있다.

```fung run exit=1 error=E0501
print("checking disks")
used = "91"
print(used + 1)
```

런타임 에러에는 `call path`(호출 경로)가 더 붙는다. 함수 안에서 에러가 나면, 어느 함수를 어디서 불러서 거기까지 갔는지 안쪽부터 차례로 보여 준다. 아래에서 에러는 `percent` 함수 안(1줄)에서 났고, `percent`는 파일의 3줄에서 불렸다. `<entry>`는 "파일 최상위"라는 뜻이다.

```fung run exit=1 error=E0502
fn percent(used, total) = used * 100 // total
print(percent(45, 50))
print(percent(45, 0))
```

### 종료 코드 {#exit-codes}

프로그램은 끝날 때 **종료 코드(exit code)**라는 숫자를 남긴다. 쉘에서 `echo $?`로 볼 수 있고, 다른 스크립트나 CI가 성공 여부를 이것으로 판단한다.

<table>
  <thead><tr><th>코드</th><th>뜻</th></tr></thead>
  <tbody>
    <tr><td><code>0</code></td><td>정상 종료</td></tr>
    <tr><td><code>1</code></td><td>실행 중 실패(런타임 에러 등)</td></tr>
    <tr><td><code>2</code></td><td>컴파일 실패(문법·이름 에러). 아무것도 실행되지 않았다</td></tr>
    <tr><td><code>3</code></td><td><code>fung</code> 명령을 잘못 씀(없는 파일, 모르는 명령)</td></tr>
  </tbody>
</table>

스크립트가 직접 원하는 코드로 끝내는 법(`exit(n)`)은 11장에서 다룬다.

## 짚고 넘어가기 {#notes}

:::box learn 컴파일 에러와 런타임 에러

많은 언어가 에러를 두 때로 나눈다. **실행 전에** 글만 보고 알 수 있는 것(괄호가 안 닫힘, 없는 이름)과, **실행해 봐야** 알 수 있는 것(파일에서 읽은 값이 숫자가 아님, 0으로 나눔)이다. 앞의 것은 일찍 잡을수록 좋다. 그래서 fung은 실행 전에 할 수 있는 검사를 최대한 하고, `fung check`로 그 검사만 따로 돌릴 수 있게 했다.

:::

:::box mistake 닫는 따옴표나 ;;를 빠뜨림

문자열의 닫는 `"`나 블록의 닫는 `;;`를 빠뜨리면 fung은 파일 끝까지 읽은 뒤에야 알아챈다. 그래서 에러 위치가 파일의 **마지막 줄 다음**으로 찍힌다. `print("checking ${host}"`처럼 닫는 괄호를 빠뜨려도 `E0110 unexpected end of file`이다. 위치가 파일 끝을 가리키면 그 위쪽에서 열고 닫지 않은 것을 찾는다.

```fung run exit=2 error=E0110
host = "web-1"
print("checking ${host}"
```

:::

:::box other Python과 쉘에서는

Python의 `python3`, Node의 `node`를 인자 없이 실행하면 열리는 대화형 실행기가 fung의 REPL과 같은 것이다. 쉘 스크립트는 틀린 명령이 있어도 그 줄에 닿을 때까지 앞 줄을 실행하지만, fung은 이름 오타 같은 것을 실행 전에 잡아 아무것도 하지 않고 멈춘다. 쉘의 `set -e`처럼 실패하면 그 자리에서 멈추는 동작이 fung에서는 기본이다.

:::

## 해 보기 {#exercises}

:::exercise 인사하는 스크립트

명령줄 인자로 이름을 받아 `hello, <이름>! you have <인자 개수> argument(s)`를 찍는 스크립트를 써 보자. 인자가 없으면 이름 자리에 `stranger`를 쓴다. 인자 개수는 `len(args)`다.

:::solution

```fung run args="alice"
name = first(args) ?? "stranger"
print("hello, ${name}! you have", len(args), "argument(s)")
```

:::

:::

:::exercise 에러 메시지로 고치기

다음 스크립트를 실행하면 `E0204`가 나고 `help:` 줄에 비슷한 이름 `print`가 있다고 나온다. 메시지만 보고 고쳐 보자.

```fung frag
total = 3
prnt("servers:", total)
```

:::solution

에러 위치(2줄 1칸)에 있는 `prnt`를 `print`로 고친다.

```fung run
total = 3
print("servers:", total)
```

:::

:::

## 정리 {#summary}

- 설치는 `install.sh` 한 줄, 확인은 `fung version`.
- 실행은 `fung -e '코드'`, `fung 파일.fung [인자…]`, 그리고 인자 없는 `fung`(REPL).
- 스크립트는 위에서 아래로 실행되고, `#`는 주석, 명령줄 인자는 `args`.
- `fung check`는 실행하지 않고 검사만 한다. 경고도 보여 준다.
- 에러는 번호, 내용, 위치, 발췌, 도움말 순서다. 컴파일 에러(종료 코드 2)면 아무것도 실행되지 않고, 런타임 에러(종료 코드 1)는 호출 경로를 보여 준다.

레퍼런스: [명령줄 도구](../reference/cli.html), [에러와 종료 코드](../reference/cli.html#errors), [에러 카탈로그](../reference/errors.html), [`print`](../reference/prelude/io.html#fn-print), [`args`](../reference/prelude/program.html#fn-args). REPL의 내부는 [REPL](../vm/16-repl.html)에서 다룬다.
