---
lede: 외부 명령을 실행하고 환경 변수를 읽는다. 명령은 쉘을 거치지 않는 <code>exec</code>가 기본이다. 종료 코드가 0이 아니면 <code>Err</code>이므로 <code>exec(…)!</code>가 쉘의 <code>set -e</code>와 같다.
description: fung prelude 프로세스와 환경: exec, sh, env.
---
# 프로세스와 환경

## 프로세스와 환경이란 {#concepts}

실행 중인 프로그램 하나를 **프로세스**(process)라고 한다. fung 스크립트도 프로세스이고, 스크립트가 `git`이나 `ssh`, `kubectl`을 실행하면 그것은 **자식 프로세스**(child process)가 된다. 자식 프로세스는 세 가지를 남긴다: 표준 출력에 쓴 글(`out`), 표준 에러에 쓴 글(`err`), 종료 코드(`code`, 0이면 성공). [`exec`](#fn-exec)는 명령이 끝날 때까지 기다렸다가 이 셋을 맵으로 돌려준다. 출력은 터미널에 흘러나오지 않고 값으로 모인다.

명령은 **문자열의 리스트**로 준다: `["git", "commit", "-m", msg]`. 첫 원소가 실행할 프로그램이고 나머지가 인자다. 쉘을 거치지 않으므로 `msg`에 공백, 따옴표, `$`, `;`가 들어 있어도 한 인자로 그대로 간다. 쉘의 따옴표 규칙을 신경 쓸 필요가 없고, 바깥에서 온 값이 명령을 바꿔 버리는 **명령 주입**(command injection)도 일어나지 않는다. 파이프(`|`)나 글로브(`*.log`) 같은 쉘 기능이 꼭 필요할 때만 [`sh`](#fn-sh)를 쓴다.

**환경 변수**(environment variable)는 프로세스가 부모에게서 물려받는 `이름=값` 목록이다. `HOME`, `PATH`, 그리고 `DB_HOST`처럼 설정을 넘기는 데 쓰는 것들이 있다. [`env`](#fn-env)로 읽고, 자식 프로세스에 더 넘길 것은 `exec`의 `env` 인자로 준다. fung에는 자기 환경을 바꾸는 함수가 없다.

이 페이지의 세 함수는 fung으로 쓴 prelude가 네이티브 함수를 그대로 이름 붙인 것이다.

## 함수 {#functions}

:::fn exec(cmd, cwd = nil, env = nil, stdin = nil)

명령을 쉘 없이 실행하고 끝나기를 기다려 `Ok({out, err, code})`를 돌려준다. 0이 아닌 종료는 `Err`.

#### 설명

`cmd`의 첫 원소를 `PATH`에서 찾아 나머지를 인자로 실행한다(`/`가 든 이름이면 그 경로를 쓴다). 명령이 끝나면 표준 출력과 표준 에러를 문자열로 모아 돌려준다. 출력 끝의 줄바꿈도 그대로 들어 있으므로 값 하나를 얻을 때는 [`trim`](strings.html#fn-trim)하고, 여러 줄이면 [`lines`](strings.html#fn-lines)로 나눈다.

- **성공**(종료 코드 0): `Ok({out, err, code: 0})`.
- **0이 아닌 종료**: `Err({kind: 'exit, code, out, err, message})`. 출력도 들어 있으므로 실패 이유를 `e.err`로 보여 줄 수 있다. `exec(…)!`로 쓰면 실패할 때 멈추므로 쉘의 `set -e`와 같고, 문장으로만 쓴 `exec(…)`도 `Err`를 버리면 E0606으로 멈춘다.
- **실행할 수 없음**: 실행 파일이 없거나, 실행 권한이 없거나, `cwd`가 없으면 `Err({kind: 'not_found, message})`. 이유는 `command not found in PATH`, `permission denied` 등이다. 실행 권한이 없는 스크립트는 `["sh", "script.sh"]`로 돌린다.

`cwd`를 주면 그 디렉터리에서 실행한다. `env`는 지금 환경에 **덧붙일** 맵이다(같은 이름은 덮어쓴다). `stdin`을 주면 그 문자열을 명령의 표준 입력으로 넣고 닫는다. `stdin`이 `nil`이면 명령은 빈 입력을 받는다. fung이 받은 표준 입력을 명령에 넘기지 않는다. 이름 붙인 인자로 쓰면 읽기 좋다: `exec(["make"], cwd: "/src", env: {CC: "clang"})`.

**취소**: 부른 태스크가 취소되면(`timeout`, `exit`, 범위의 취소) 명령에 SIGTERM을 보내고, 2초 뒤에도 남아 있으면 SIGKILL을 보낸 뒤 취소로 되감긴다. 그래서 `timeout(30s, () => exec(cmd))`는 걸린 명령을 남겨 두지 않는다.

**터미널**: fung이 터미널의 전경에서 돌면(사람이 터미널에서 실행했으면) 명령도 fung과 같은 프로세스 그룹이라 비밀번호 프롬프트처럼 터미널을 직접 쓸 수 있고, Ctrl-C도 셸에서처럼 명령에 간다. 이때 취소는 명령 하나에만 간다. 터미널이 없으면(서비스, 컨테이너, CI, cron) 명령은 자기 프로세스 그룹에서 돌고, 취소는 그룹 전체에 가서 셸이 띄운 손자 프로세스까지 끝난다.

여러 명령을 동시에 돌리려면 태스크로 나눈다([`scope`](concurrency.html#fn-scope), [`conc.pmap`](../modules/conc.html#fn-conc-pmap)). 명령을 기다리는 동안에도 다른 태스크는 계속 돈다.

#### 매개변수

- `cmd`: 비지 않은 문자열 리스트. `[프로그램, 인자…]`.
- `cwd`: 실행할 디렉터리, 또는 `nil`(지금 디렉터리).
- `env`: 덧붙일 환경 변수(값이 문자열인 맵), 또는 `nil`.
- `stdin`: 표준 입력으로 넣을 문자열, 또는 `nil`.

#### 돌려주는 값

`Ok({out: 문자열, err: 문자열, code: 0})`, 또는 `Err({kind: 'exit, code, out, err, message})`, 또는 `Err({kind: 'not_found, message})`.

#### 에러

인자의 타입이 틀리면 런타임 에러 E0403: `cmd`가 문자열이거나(`exec("ls -l")`은 안 된다), 비었거나, 문자열이 아닌 원소가 있을 때, `env`의 값이 문자열이 아닐 때 등.

#### 예제

**출력 얻기**

```fung run
r = exec(["echo", "hi"])!
print(r)
print(trim(r.out))
```

**실패를 가르기**

```fung run
fn run(cmd) = match exec(cmd):
  Ok(r)                    => "ok: ${trim(r.out)}"
  Err({kind: 'exit, code}) => "exited with ${code}"
  Err({kind: 'not_found})  => "no such command"
;;

print(run(["echo", "fine"]))
print(run(["sh", "-c", "exit 3"]))
print(run(["no-such-command-xyz"]))
```

**셸의 set -e처럼 멈추기**

```fung run exit=1 error=E0603
exec(["sh", "-c", "echo building; exit 2"])!
print("not reached")
```

**인자에 공백과 특수 문자가 있어도 그대로**

```fung run
msg = "fix: it's \"done\"; rm -rf /"
print(exec(["printf", "%s\n", msg])!.out |> trim)
```

`msg`는 쉘을 거치지 않으므로 `;` 뒤가 명령으로 실행되지 않는다.

**디렉터리, 환경, 표준 입력**

```file notes.txt
b
a
```

```fung run
print(lines(exec(["ls"], cwd: ".")!.out))
print(trim(exec(["sh", "-c", "echo $GREETING"], env: {GREETING: "hello"})!.out))
print(lines(exec(["sort"], stdin: "web-2\nweb-1\n")!.out))
print(exec(["cat"])!.out == "")
```

마지막 줄: `stdin`을 주지 않은 `cat`은 빈 입력을 받고 곧바로 끝난다.

**기한을 두고 실행하기**

```fung run
print(timeout(100ms, () => exec(["sleep", "5"])))
```

`sleep`은 기한이 지나자 SIGTERM으로 끝났다.

#### 같이 보기

[`sh`](#fn-sh), [`env`](#fn-env), [`timeout`](concurrency.html#fn-timeout), [`lines`](strings.html#fn-lines), [`conc.pmap`](../modules/conc.html#fn-conc-pmap)

:::

:::fn sh(cmd, cwd = nil, env = nil, stdin = nil)

문자열 `cmd`를 `/bin/sh -c cmd`로 실행한다. 파이프나 글로브 같은 쉘 기능이 필요할 때만 쓴다.

#### 설명

`exec(["/bin/sh", "-c", cmd], …)`와 같다. 돌려주는 값, `cwd`·`env`·`stdin`, 취소의 규칙이 모두 [`exec`](#fn-exec)와 같다. 종료 코드는 쉘의 것, 곧 파이프라인의 마지막 명령의 것이다.

:::box mistake 바깥에서 온 값을 sh 문자열에 끼워 넣지 않는다

`sh("grep ${pattern} app.log")`에서 `pattern`이 사용자 입력이면, `x; rm -rf ~` 같은 값이 명령이 되어 실행된다(명령 주입). 값이 들어가는 명령은 `exec(["grep", pattern, "app.log"])`처럼 리스트로 쓴다. 쉘 기능이 꼭 필요하면 값을 `env`로 넘기고 쉘 안에서 `"$PATTERN"`으로 따옴표를 쳐서 읽는다.

:::

#### 매개변수

- `cmd`: 쉘 명령 문자열.
- `cwd`, `env`, `stdin`: [`exec`](#fn-exec)와 같다.

#### 돌려주는 값

[`exec`](#fn-exec)와 같다.

#### 에러

[`exec`](#fn-exec)와 같다.

#### 예제

**파이프와 글로브**

```file a.log
ok
ERROR one
```

```file b.log
ERROR two
ERROR three
```

```fung run
print(sh("cat *.log | grep -c ERROR")!.out |> trim)
```

**값은 env로 넘기기**

```fung run
pattern = "two; echo injected"
r = sh("echo \"$P\"", env: {P: pattern})!
print(trim(r.out))
```

**실패도 Err**

```fung run
print(sh("exit 2"))
```

#### 같이 보기

[`exec`](#fn-exec), [`fs.glob`](../modules/fs.html#fn-fs-glob), [`regex`](../modules/regex.html)

:::

:::fn env(name = nil)

환경 변수 `name`의 값(문자열)을, 없으면 `nil`을 돌려준다. 인자가 없으면 전체를 맵으로.

#### 설명

값은 언제나 문자열이다. 수가 필요하면 [`parse_int`](convert.html#fn-parse_int)로 바꾼다. 없는 변수는 에러가 아니라 `nil`이므로 `??`로 기본값을 붙이기 좋다: `env("PORT") ?? "8080"`. 꼭 있어야 하는 변수면 `env("DB_HOST") ?? fail("DB_HOST is required")`.

인자 없이 부르면 모든 환경 변수를 `{이름: 값}` 맵으로 돌려준다.

#### 매개변수

- `name`: 변수 이름(문자열), 또는 `nil`(전체).

#### 돌려주는 값

문자열 또는 `nil`. 인자가 없으면 맵.

#### 에러

`name`이 문자열도 `nil`도 아니면 런타임 에러 E0403.

#### 예제

**기본값과 함께**

```fung run
port = env("PORT") ?? "8080"
print(port)
```

**값이 있을 때**

```fung run env="DEPLOY_ENV=staging RETRIES=3"
print(env("DEPLOY_ENV"), parse_int(env("RETRIES") ?? "1")!)
print(env("NOT_SET_ANYWHERE"))
```

**전체를 맵으로**

```fung run env="APP_DEBUG=1 APP_NAME=api"
app = pairs(env()) |> filter(((k, _)) => starts_with(k, "APP_")) |> sort_by(((k, _)) => k)
print(app)
```

#### 같이 보기

[`exec`](#fn-exec), [`args`](program.html#fn-args), [`??`](../language.html#results)

:::
