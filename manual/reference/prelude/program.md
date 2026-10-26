---
lede: 프로그램이 받은 인자, 프로그램을 끝내는 법, 버그를 알리는 법, 무슨 일이 있어도 정리를 하는 법을 다룬다.
description: fung prelude 에러와 종료: fail, with, exit, args.
---
# 에러와 종료

## 프로그램의 시작과 끝 {#concepts}

명령줄 프로그램은 **인자**(argument)를 받아 시작하고 **종료 코드**(exit code)를 남기고 끝난다. 인자는 `fung run deploy.fung prod web-1`에서 스크립트 경로 뒤의 낱말들(`prod`, `web-1`)이고, fung에서는 [`args`](#fn-args)로 읽는다. 종료 코드는 0부터 255까지의 수로, 0은 성공, 그 밖은 실패를 뜻한다. 셸의 `$?`, `&&`·`||`, CI, cron, systemd가 모두 이 수로 성공을 판단한다.

fung 프로그램이 끝나는 길은 다음과 같다.

<table>
  <thead><tr><th>끝나는 길</th><th>종료 코드</th></tr></thead>
  <tbody>
    <tr><td>진입 파일의 마지막 줄까지 실행</td><td>0</td></tr>
    <tr><td><code>exit(n)</code></td><td><code>n</code></td></tr>
    <tr><td>런타임 에러(<code>fail</code>, <code>r!</code>의 <code>Err</code>, 0으로 나누기, 없는 키 …)</td><td>1</td></tr>
    <tr><td>컴파일 에러(문법, 정의되지 않은 이름 …): 실행 전에 멈춘다</td><td>2</td></tr>
    <tr><td>SIGINT·SIGTERM(Ctrl-C, <code>kill</code>)으로 취소</td><td>130·143(<a href="concurrency.html#fn-stop_signal"><code>stop_signal</code></a>)</td></tr>
  </tbody>
</table>

**런타임 에러**는 "여기 오면 코드가 틀린 것"을 알리는 멈춤이다. 메시지, 에러 번호, 난 곳과 호출 경로를 표준 에러에 찍고 끝난다. 실패할 수 있는 일(파일 없음, 수가 아닌 입력)은 [결과값](results.html)으로 돌려주고, 런타임 에러는 버그에만 쓴다는 것이 fung의 나눔이다. 런타임 에러를 잡아 되살리는 `try`/`catch`는 없다. 그 대신 끝나기 전에 꼭 해야 하는 정리(임시 파일 지우기, 잠금 풀기, 연결 닫기)는 [`with`](#fn-with)로 묶어 두면 어떻게 끝나든 돈다.

## 함수 {#functions}

:::fn fail(...args)

인자를 `print`처럼 이은 메시지로 런타임 에러(E0803)를 내고 멈춘다.

#### 설명

"여기 오면 버그"라고 말할 때 쓴다. 메시지는 [`print`](io.html#fn-print)와 같은 규칙으로 인자를 공백으로 이은 것이다. 인자가 없으면 메시지는 `fail`이다. 런타임 에러이므로 종료 코드는 1이고, 지나는 [`with`](#fn-with)의 정리는 돈다. 태스크 안에서 부르면 그 태스크가 실패하고, 태스크 범위가 그 실패를 받는다([`scope`](concurrency.html#fn-scope), [`supervise`](concurrency.html#fn-supervise)).

`fail`은 값을 돌려주지 않으므로 어떤 식 자리에도 쓸 수 있다. 특히 `??`와 짝이 맞는다: `cfg.port ?? fail("config.port is required")`. 사용자가 고칠 수 있는 실패(잘못된 입력, 없는 파일)라면 `fail` 대신 `eprint`로 이유를 알리고 `exit(1)`하거나 `Err`를 돌려주는 편이 친절하다. `fail`의 메시지에는 소스 위치와 호출 경로가 붙기 때문이다.

#### 매개변수

- `args`: 메시지를 이룰 값들.

#### 돌려주는 값

돌아오지 않는다.

#### 에러

언제나 런타임 에러 E0803.

#### 예제

**필수 값이 없을 때**

```fung run exit=1 error=E0803
cfg = {host: "db-1"}
port = get(cfg, "port") ?? fail("config.port is required")
print(port)
```

**여러 값으로 메시지 만들기**

```fung run exit=1 error=E0803
fn check_port(n) = n > 0 and n < 65536 then n else fail("bad port:", n, "(want 1..65535)")

print(check_port(443))
print(check_port(70000))
```

#### 같이 보기

[`exit`](#fn-exit), [`eprint`](io.html#fn-eprint), [결과값과 에러](../language.html#results), [E0803](../errors.html#E0803)

:::

:::fn with(acquire, use, release)

`acquire()`로 얻은 것을 `use`에 넘기고, `use`가 어떻게 끝나든 `release`를 부른다.

#### 설명

자원(resource)을 빌리고 돌려주는 모양을 한 함수로 묶는다. 파일, 잠금, 임시 디렉터리, 서버 연결처럼 "얻었으면 반드시 돌려줘야 하는 것"에 쓴다.

1. `acquire()`를 부른다. 여기서 런타임 에러가 나면 얻은 것이 없으므로 `release`는 돌지 않는다.
2. 얻은 값 `r`로 `use(r)`를 부른다.
3. `use`가 어떻게 끝나든, 곧 값을 돌려주든, `Err`를 돌려주든, 런타임 에러가 나든, 취소되든, `exit`가 불리든 `release(r)`를 부른다.
4. `use`가 정상으로 끝났으면 그 값이 `with`의 결과다.

`acquire`가 `Err`를 돌려줘도 그것이 "얻은 값"이다. `use`와 `release`는 그 `Err`를 그대로 받으므로 열기에 실패할 수 있는 자원은 둘 다에서 `match`로 가른다. [`fs.with_open`](../modules/fs.html#fn-fs-with_open)과 [`fs.with_temp_dir`](../modules/fs.html#fn-fs-with_temp_dir)이 이것을 대신 해 주는 `with`이므로 파일과 임시 디렉터리에는 그쪽을 쓴다.

`release`는 그것을 부르게 만든 취소나 기한([`timeout`](concurrency.html#fn-timeout))에 끊기지 않는다. 정리가 반쯤 하다 마는 일이 없게 하려는 것이다.

`release`의 실패는 버리지 않는다. `use`도 실패했으면 `use`의 에러에 `note: also failed in release`로 붙고, 아니면 `release`의 에러가 난다. `release`의 결과는 버리는 값이라 `Err`를 돌려주면 E0606이다. 실패해도 되는 정리는 `fn(c): _ = close(c) ;;`처럼 일부러 버린다. 프로그램이 끝날 때의 정리도 `with`로 한다(따로 `at_exit` 같은 것은 없다). 프로그램 전체를 `with`의 `use` 안에 두면 된다.

#### 매개변수

- `acquire`: 인자 없이 불려 자원을 돌려주는 함수.
- `use`: 자원을 받아 일을 하는 함수. 그 결과가 `with`의 결과다.
- `release`: 자원을 받아 정리하는 함수. 결과는 버린다.

#### 돌려주는 값

`use`의 결과.

#### 에러

- `use`나 `release`의 런타임 에러는 그대로 난다(둘 다면 `use`의 에러에 `release`의 것이 붙는다).
- `release`가 `Err`를 돌려주면 E0606.

#### 예제

**정상으로 끝날 때**

```fung run
print(with(() => "conn", c => c + " used", c => print("closed", c)))
```

`release`가 먼저 돌고, 그 뒤에 `with`의 결과가 찍힌다.

**use가 실패해도 정리는 돈다**

```fung run exit=1 error=E0803
with(() => "/tmp/lock", fn(lock):
  print("working with", lock)
  fail("disk full")
;;, lock => print("removed", lock))
```

**exit와 기한에도 정리는 돈다**

```fung run
r = timeout(20ms, () => with(() => "lock", fn(c):
  sleep(1s)
  "done"
;;, fn(c):
  sleep(30ms)
  print("released", c)
;;))
print(r)
```

`release`의 `sleep(30ms)`는 이미 지난 기한에 끊기지 않고 끝까지 돈다.

**실패해도 되는 정리**

```fung run
fn cleanup(path) = exec(["rm", path])

r = with(() => "./no-such-file", p => "used ${p}", fn(p):
  _ = cleanup(p)
;;)
print(r)
```

`rm`은 실패하지만 `_ =`로 일부러 버렸으므로 멈추지 않는다.

#### 같이 보기

[`fs.with_open`](../modules/fs.html#fn-fs-with_open), [`fs.with_temp_dir`](../modules/fs.html#fn-fs-with_temp_dir), [`exit`](#fn-exit), [`timeout`](concurrency.html#fn-timeout), [`stop_signal`](concurrency.html#fn-stop_signal)

:::

:::fn exit(code = 0)

프로그램을 끝내고 종료 코드 `code`를 남긴다.

#### 설명

어느 태스크에서 불러도 프로그램 전체를 끝낸다. 남은 태스크는 모두 취소되고, 되감기가 지나는 모든 [`with`](#fn-with)의 `release`가 돈 뒤 그 코드로 끝난다. 성공은 `exit()`이나 `exit(0)`, 실패는 보통 `exit(1)`이다. 진입 파일의 끝까지 가면 저절로 0으로 끝나므로 성공 때는 `exit`를 쓸 필요가 없다.

종료 코드는 운영체제가 하위 8비트만 남기므로 0부터 255까지를 쓴다. 그 밖의 수는 256으로 나눈 나머지가 된다(`exit(256)`은 0, `exit(-1)`은 255).

#### 매개변수

- `code`: 정수. 기본은 0.

#### 돌려주는 값

돌아오지 않는다.

#### 에러

`code`가 정수가 아니면 런타임 에러 E0403.

#### 예제

**실패를 알리고 끝내기**

```fung run exit=2
missing = ["DB_HOST", "DB_USER"] |> filter(k => env(k) == nil)
if not is_empty(missing):
  eprint("missing env:", join(missing, ", "))
  exit(2)
;;
print("all set")
```

**exit 뒤에도 정리는 돈다**

```fung run exit=3
with(() => "tmp", fn(t):
  print("exiting")
  exit(3)
;;, t => print("cleaned", t))
```

#### 같이 보기

[`fail`](#fn-fail), [`with`](#fn-with), [`eprint`](io.html#fn-eprint), [`request_stop`](concurrency.html#fn-request_stop)

:::

:::fn args

스크립트 경로 뒤의 명령행 인자를 담은 문자열 리스트. 함수가 아니라 값이다.

#### 설명

`fung run deploy.fung prod --dry-run`이면 `args`는 `["prod", "--dry-run"]`이다. 스크립트 경로와 `fung run`은 들어가지 않는다. 인자가 없으면 `[]`다. 함수가 아니므로 괄호 없이 `args`로 쓴다.

인자는 언제나 문자열이라 수나 기간이 필요하면 [`parse_int`](convert.html#fn-parse_int)·[`parse_dur`](convert.html#fn-parse_dur)로 바꾼다. `--name value` 같은 옵션과 도움말이 필요하면 [`cli` 모듈](../modules/cli.html)이 `args`를 읽어 준다.

#### 매개변수

없다(값이다).

#### 돌려주는 값

문자열의 리스트.

#### 에러

없다.

#### 예제

**인자 읽기**

```fung run args="prod web-1 web-2"
match args:
  []                   => print("usage: deploy ENV HOST...")
  [env_name, ...hosts] => print("deploying to", env_name, hosts)
;;
```

**인자가 없을 때**

```fung run exit=2
if is_empty(args):
  eprint("usage: count N")
  exit(2)
;;
print(parse_int(args[0])!)
```

#### 같이 보기

[`cli` 모듈](../modules/cli.html), [`env`](process.html#fn-env), [명령줄 도구](../cli.html)

:::
