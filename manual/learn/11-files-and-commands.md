---
description: 파일 읽기·쓰기, 디렉터리 돌기, 외부 명령 실행(exec·sh), 명령줄 인자와 cli.parse, 환경 변수, 표준 입력, 종료 코드.
lede: 스크립트가 하는 일의 대부분은 파일을 읽고, 명령을 돌리고, 결과를 남기는 것이다. 이 장은 fung으로 쉘 스크립트가 하던 일을 하는 법을 배운다.
---
# 파일·외부 명령·CLI

:::goals
- 파일을 읽고 쓰기: `fs.read`, `fs.read_lines`, `fs.write`, `fs.append`
- 디렉터리를 보고 돌기: `fs.list`, `fs.walk`, `fs.stat`, `fs.glob`, `path`
- 외부 명령 실행: `exec`와 `sh`, 종료 코드가 `Err`가 되는 규칙
- 명령줄 인자 `args`와 `cli.parse`, 환경 변수 `env`, 표준 입력
- 표준 에러와 종료 코드로 결과를 알리기
:::

## 파일과 경로 {#paths}

**파일**(file)은 디스크에 이름을 붙여 저장한 글자나 바이트의 묶음이고, **디렉터리**(directory, 폴더)는 파일과 다른 디렉터리를 담는 상자다. 파일의 위치는 **경로**(path)로 나타낸다. `/var/log/app.log`처럼 `/`로 시작하면 맨 위에서부터 적은 **절대 경로**, `logs/app.log`처럼 아니면 프로그램이 지금 서 있는 디렉터리(**현재 디렉터리**)에서부터 적은 **상대 경로**다.

fung에서 파일을 다루는 함수는 `fs` 모듈에, 경로를 계산하는 함수는 `path` 모듈에 있다(10장). 파일 연산은 늘 실패할 수 있으므로(파일이 없다, 권한이 없다) 대부분 9장의 결과값을 돌려준다. 실패의 `kind`는 `'not_found`, `'exists`, `'permission`, `'io` 중 하나다.

이 장의 예제는 빈 디렉터리에서 준비물 파일만 두고 돈다. 준비물은 예제 위의 접힌 상자에서 볼 수 있다.

## 파일 읽기 {#read}

`fs.read(경로)`는 파일 전체를 문자열 하나로, `fs.read_lines(경로)`는 줄의 리스트로 읽는다. 둘 다 결과값이므로 스크립트에서는 보통 `!`를 붙인다. 로그에서 `ERROR` 줄만 골라 세어 보자.

```file app.log
2026-10-20T09:00:01Z INFO server started
2026-10-20T09:03:12Z ERROR disk /data is 95% full
2026-10-20T09:03:40Z WARN slow query (1.2s)
2026-10-20T09:05:00Z ERROR upstream timeout: payments
2026-10-20T09:06:30Z INFO request served
```

```fung run
import fs

rows = fs.read_lines("app.log")!
errors = rows |> filter(r => "ERROR" in r)

print("${len(errors)} errors in ${len(rows)} lines")
for e in errors:
  print(" ", e)
;;
```

없는 파일을 읽으면 `Err`가 온다. `!`를 붙였다면 거기서 멈추고, 처리하고 싶다면 `match`나 `??`를 쓴다(9장).

```fung run
import fs

match fs.read("settings.ini"):
  Ok(text)                => print(len(text), "chars")
  Err({kind: 'not_found}) => print("no settings.ini, using defaults")
  Err(e)                  => print("cannot read:", e.message)
;;
```

## 파일 쓰기 {#write}

`fs.write(경로, 문자열)`은 파일을 만들거나 덮어쓰고, `fs.append(경로, 문자열)`은 끝에 덧붙인다(없으면 만든다). 줄바꿈은 자동으로 붙지 않으니 `"\n"`을 직접 넣는다. 성공하면 `Ok(nil)`이다. 쓰기가 실패했는데 모르고 지나가면 안 되므로, 결과를 버리면 9장의 E0606이다. 스크립트에서는 `!`를 붙인다.

```fung run
import fs

fs.write("report.txt", "disk check\n")!
for host in ["web1", "web2"]:
  fs.append("report.txt", "${host}: ok\n")!
;;
print(fs.read("report.txt")!)
```

쓰려는 디렉터리가 없으면 쓰기가 실패한다. `fs.mkdir`는 중간 디렉터리까지 한꺼번에 만들고, 이미 있으면 그대로 둔다(`mkdir -p`와 같다).

```fung run
import fs

print(fs.write("out/2026/report.txt", "x"))
fs.mkdir("out/2026")!
fs.write("out/2026/report.txt", "x")!
print(fs.exists("out/2026/report.txt"))
```

:::box tip 큰 파일과 핸들
`fs.read`는 파일을 통째로 메모리에 읽는다. 아주 큰 파일을 한 줄씩 다루거나 많은 줄을 차례로 쓸 때는 파일 **핸들**(handle, 열린 파일을 가리키는 값)을 연다. `fs.with_open(경로, f, mode: 'write)`는 파일을 열어 `f(핸들)`을 부르고, `f`가 어떻게 끝나든 닫아 준다. 핸들에는 `read_line(h)`·`fs.write_line(h, 줄)`을 쓴다. 자세한 것은 [fs 레퍼런스](../reference/modules/fs.html#fn-fs-with_open)에 있다.
:::

## 디렉터리 돌기 {#dirs}

`fs.list(디렉터리)`는 바로 안의 이름들을, `fs.walk(디렉터리)`는 아래의 모든 파일 경로를 하위 디렉터리까지 내려가며 돌려준다(디렉터리 자신은 빼고). `fs.stat(경로)`는 `{name, size, is_dir}`을 돌려준다. `size`는 바이트 수다. 셋을 합치면 "로그 디렉터리에서 큰 `.log` 파일 찾기"가 된다.

```file api.log
GET /users 200
GET /users 200
POST /orders 500
```

```file worker.log
job 1 done
```

```file notes.txt
remember to rotate
```

```fung run
import fs
import path

print(fs.list(".")!)

sizes = fs.walk(".")!
  |> filter(p => path.ext(p) == ".log")
  |> map(p => (p, fs.stat(p)!.size))
  |> sort_by(((_, size)) => -size)

for (p, size) in sizes:
  print(pad_left(str(size), 4), p)
;;
```

예제의 디렉터리에는 준비물과 예제 파일 `example.fung`만 있다. `fs.walk`는 하위 디렉터리가 있으면 `logs/old/api.1.log`처럼 그 경로까지 붙여서 준다.

`fs.glob("*.log")`는 쉘의 `*`·`?`처럼 패턴에 맞는 경로를 준다. `path`의 `join`·`base`·`dir`·`ext`·`stem`은 파일을 건드리지 않고 경로 문자열만 계산한다. 경로를 `+`와 `"/"`로 이어 붙이기보다 `path.join`을 쓰면 `/`가 겹치거나 빠지는 실수가 없다.

```file api.log
GET /users 200
```

```file worker.log
job 1 done
```

```fung run
import fs
import path

for p in fs.glob("*.log")!:
  backup = path.join("backup", path.stem(p) + ".old")
  print(p, "->", backup)
;;
```

:::box mistake 블록 안에서 바깥 변수 늘리기
쉘에서는 `total=$((total + size))`를 반복문 안에서 쓴다. fung에서 `for` 본문 안에 `total = total + size`를 쓰면 컴파일 에러 E0205다. 블록 안에서는 바깥 변수에 다시 대입할 수 없기 때문이다(3장). 합계는 반복이 만든 리스트로 계산한다: `sizes |> map(((_, s)) => s) |> sum`, 또는 `fold`.
:::

## 외부 명령 실행 {#exec}

fung으로 하기 어려운 일, 이미 있는 도구가 잘하는 일은 외부 명령(`git`, `kubectl`, `ssh` …)에 맡긴다. `exec`는 명령을 **문자열 리스트**로 받는다. 첫 원소가 실행할 프로그램이고 나머지가 인자다. 쉘을 거치지 않으므로 인자에 공백이나 `*`가 있어도 쪼개지거나 펼쳐지지 않는다.

```fung run
r = exec(["echo", "hello", "fung"])
print(r)
out = exec(["echo", "hello", "fung"])!
print(trim(out.out))
```

결과는 `Ok({out, err, code})`다. `out`과 `err`는 명령이 표준 출력과 표준 에러에 쓴 글자다. 명령이 0이 아닌 종료 코드로 끝나면 `Err({kind: 'exit, code, out, err, message})`이고, 명령을 찾지 못하면 `Err({kind: 'not_found, …})`다. 그래서 `exec(…)!`는 쉘의 `set -e`처럼 "실패하면 멈춘다"가 된다.

```fung run
print(exec(["sh", "-c", "echo partial; exit 3"]))
print(exec(["no-such-tool", "--version"]))
```

파이프(`|`), 리다이렉션(`>`), 글로브처럼 쉘 기능이 꼭 필요하면 `sh(문자열)`을 쓴다. `/bin/sh -c 문자열`로 돌리고 결과의 모양은 `exec`와 같다. 둘 다 이름 붙인 인자 `cwd:`(실행할 디렉터리), `env:`(덧붙일 환경 변수 맵), `stdin:`(표준 입력으로 줄 문자열)을 받는다.

```fung run
r = sh("printf 'a\\nERROR x\\nERROR y\\n' | grep -c ERROR")!
print("errors:", trim(r.out))

greeting = exec(["sh", "-c", "echo \"hi $WHO\""], env: {WHO: "ops"})!
print(trim(greeting.out))

sorted = exec(["sort"], stdin: "pear\napple\nfig\n")!
print(lines(sorted.out))
```

:::box mistake 명령을 문자열 하나로 주기
`exec("ls -l")`은 런타임 에러 E0403(`argument `cmd` must be a list of strings`)이다. `exec`는 쉘처럼 문자열을 쪼개 주지 않는다. `exec(["ls", "-l"])`로 나눠 쓰거나, 쉘 문법이 필요하면 `sh("ls -l | head")`를 쓴다. 바깥에서 온 값(파일 이름, 사용자 입력)을 `sh`의 문자열에 끼워 넣으면 쉘이 그것을 명령으로 해석할 수 있으니, 그런 값은 `exec`의 리스트 원소로 넘긴다.
:::

## 명령줄 인자 {#args}

`fung script.fung a b c`로 실행하면 스크립트 이름 뒤의 인자가 `args`에 문자열 리스트로 들어온다(2장). `--dir logs` 같은 옵션을 받으려면 `cli` 모듈의 `cli.parse(args)`를 쓴다. 결과는 `{flags, rest}`다. `--이름 값`과 `--이름=값`은 `flags`에, 나머지는 순서대로 `rest`에 간다. 값 없는 `--이름`은 `true`이고, `bools:`에 적은 이름은 다음 인자를 값으로 먹지 않는다. 이름의 `-`는 `_`로 바뀐다(`--dry-run` → `dry_run`).

```fung run args="--dir logs --min 100 --dry-run a.log b.log"
import cli

opts = cli.parse(args, bools: ["dry-run"])
print(opts)

dir = get(opts.flags, "dir", ".")
min_size = int(get(opts.flags, "min", "0"))
dry_run = get(opts.flags, "dry_run", false)
print(dir, min_size, dry_run, opts.rest)
```

옵션 값은 늘 문자열이라 숫자가 필요하면 직접 바꾼다. 주지 않았을 수도 있는 옵션은 `get`으로 기본값을 준다(`opts.flags.dir`은 옵션이 없으면 런타임 에러다, 9장).

## 환경 변수 {#env}

**환경 변수**(environment variable)는 프로그램을 띄운 쪽(쉘, systemd, 컨테이너)이 넘겨주는 이름-값 쌍이다. `env("이름")`은 값을 문자열로, 없으면 `nil`을 돌려준다. 그래서 `??`로 기본값을 붙이는 모양이 흔하다. `env()`는 전체를 맵으로 준다.

```fung run env="APP_ENV=staging"
app_env = env("APP_ENV") ?? "production"
port = int(env("PORT") ?? "8080")
print(app_env, port)
```

## 표준 입력 {#stdin}

파이프 중간에서 쓰는 스크립트(`cat app.log | fung count.fung`)는 **표준 입력**(standard input)을 읽는다. `read_lines()`는 인자 없이 부르면 표준 입력 전체를 줄 리스트로, `read_line()`은 한 줄을 `Ok(줄)`로, 끝에 닿으면 `Ok(nil)`로 준다. 아래 예제는 준비물 `in.txt`를 표준 입력으로 넣어 돌렸다.

```file in.txt
web1 200
web2 500
web3 200
db1 503
```

```fung run stdin=in.txt
bad = read_lines()!
  |> map(row => split(row, " "))
  |> filter(([_, code]) => starts_with(code, "5"))
  |> map(([host, _]) => host)

print("failing:", join(bad, ", "))
```

## 표준 에러와 종료 코드 {#exit}

프로그램이 끝날 때 남기는 **종료 코드**(exit code)는 0이면 성공, 그 밖이면 실패라는 관례다. cron, CI, systemd, 쉘의 `&&`가 모두 이 숫자를 본다. fung 스크립트는 끝까지 가면 0, 런타임 에러나 `!`가 `Err`를 만나면 1, 컴파일 에러면 2로 끝난다. 원하는 코드로 끝내려면 `exit(n)`을 부른다.

결과는 표준 출력(`print`)에, 사람에게 보이는 경고와 에러는 **표준 에러**(`eprint`)에 쓴다. 그래야 `fung check.fung > result.txt`처럼 출력을 파일로 받아도 에러가 섞이지 않는다.

```fung run exit=2
import fs

missing = ["app.conf", "db.conf"] |> filter(f => not fs.exists(f))
if not is_empty(missing):
  eprint("missing config:", join(missing, ", "))
  exit(2)
;;
print("all config present")
```

파일 첫 줄에 `#!/usr/bin/env fung`을 쓰고 실행 권한을 주면(`chmod +x check.fung`) `./check.fung`으로 바로 실행할 수 있다. fung에게 그 줄은 그냥 주석이다.

:::box other 쉘과 비교하면
쉘 스크립트는 명령을 잇는 데 강하지만, 변수가 모두 문자열이고 공백이 든 파일 이름에서 자주 깨지며, 실패를 놓치기 쉽다(`set -euo pipefail`을 잊으면). fung은 값에 타입이 있고, `exec`가 인자를 쪼개지 않으며, 실패가 결과값이라 무시하면 E0606으로 멈춘다. 그 대신 짧은 한 줄짜리 일에는 쉘이 더 간결하다. 둘은 경쟁이 아니라, 파이프 한 줄은 `sh`에 맡기고 나머지 논리는 fung으로 쓰는 식으로 섞어 쓴다.
:::

:::exercise 큰 로그 찾기
`--dir 디렉터리`와 `--min 바이트`를 받아, 그 디렉터리 아래에서 크기가 `min` 이상인 `.log` 파일을 큰 순서로 출력하는 스크립트를 써 보자. 하나도 없으면 표준 에러에 `no log files over N bytes`를 쓰고 종료 코드 1로 끝낸다. `--dir`의 기본값은 `.`, `--min`의 기본값은 `0`이다.

:::solution

준비물 파일 셋을 두고 `--min 20`으로 돌린 결과다(`--dir`을 주지 않았으니 현재 디렉터리).

```file api.log
GET /users 200
GET /users 200
POST /orders 500
```

```file worker.log
job 1 done
```

```file health.log
GET /health 200
GET /health 200
```

```fung run args="--min 20"
import cli
import fs
import path

opts = cli.parse(args)
dir = get(opts.flags, "dir", ".")
min_size = int(get(opts.flags, "min", "0"))

found = fs.walk(dir)!
  |> filter(p => path.ext(p) == ".log")
  |> map(p => (p, fs.stat(p)!.size))
  |> filter(((_, size)) => size >= min_size)
  |> sort_by(((_, size)) => -size)

if is_empty(found):
  eprint("no log files over ${min_size} bytes")
  exit(1)
;;
for (p, size) in found:
  print(size, p)
;;
```

:::
:::

## 정리 {#summary}

- `fs.read`·`fs.read_lines`로 읽고, `fs.write`·`fs.append`로 쓴다. 모두 결과값이라 스크립트에서는 `!`를 붙인다.
- `fs.list`·`fs.walk`·`fs.glob`로 파일을 찾고, `fs.stat`로 크기를 본다. 경로 계산은 `path`.
- `exec([프로그램, 인자…])`는 쉘 없이 명령을 돌리고 `Ok({out, err, code})`를 준다. 0이 아닌 종료는 `Err({kind: 'exit, …})`. 쉘 기능이 필요할 때만 `sh`.
- 인자는 `args`, 옵션은 `cli.parse(args)`, 환경 변수는 `env("이름") ?? 기본값`, 표준 입력은 `read_lines()`.
- 결과는 `print`, 경고와 에러는 `eprint`, 끝내는 코드는 `exit(n)`.

함수마다의 자세한 설명은 레퍼런스의 [fs](../reference/modules/fs.html), [path](../reference/modules/path.html), [cli](../reference/modules/cli.html), [프로세스와 환경](../reference/prelude/process.html), [입출력](../reference/prelude/io.html), [에러와 종료](../reference/prelude/program.html)에 있다. 명령줄 도구의 종료 코드 표는 [명령줄 도구](../reference/cli.html#exit-codes)에 있다. 실무 예는 예제 모음의 [큰 파일 찾기](../recipes/find-big-files.html), [종료 코드와 stderr](../recipes/exit-codes.html)를 보라.
