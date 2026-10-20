---
description: 여러 서버에 명령 보내고 모으기.
lede: 서버 목록 파일의 모든 서버에 <code>ssh</code>로 같은 명령을 동시에 보내고, 출력 줄마다 서버 이름을 붙여 모은다. 대답이 없는 서버는 30초에서 끊고 따로 알린다.
---
# 여러 서버에 명령 보내고 모으기

"모든 웹 서버의 `uptime`을 보고 싶다", "전부에서 디스크를 확인하고 싶다". 서버가 몇 대일 때는 한 대씩 들어가 보지만, 수십 대가 되면 반복문을 쓰게 된다. 여기서는 `fung pssh.fung hosts.txt -- uptime`처럼 부르면 목록의 서버 모두에 명령을 **동시에** 보내고, 결과를 목록 순서대로 서버 이름을 앞에 붙여 찍는 스크립트를 만든다. fung에는 SSH 기능이 따로 없다. 늘 쓰는 `ssh` 명령을 외부 명령으로 부르므로 `~/.ssh/config`, 키, 점프 호스트 설정이 그대로 적용된다.

## 쉘로는 {#shell}

```shell
for h in $(grep -v '^#' hosts.txt); do ssh "$h" uptime | sed "s/^/$h | /"; done
```

한 대씩 차례로 돌므로 30대면 30배 걸리고, 한 대가 멈추면 전체가 멈춘다. 동시에 돌리려고 `&`와 `wait`를 붙이면 출력 순서가 뒤섞이고 어느 서버가 실패했는지 모은 종료 코드가 사라진다. `xargs -P`나 `parallel`, `pssh` 같은 도구를 설치하면 되지만 서버마다 있다는 보장이 없고 옵션도 제각각이다.

## fung으로 {#code}

```file hosts.txt
# production web and db
web1
web2
db1
```

```fung check
#!/usr/bin/env fung
# Runs one command on many servers at once over ssh and prints every
# output line after its host's name. Exit code 1 if any host failed.
# usage: fung pssh.fung hosts.txt -- uptime
import conc
import fs

# The arguments before "--" and after it.
fn split_args(xs, before) = match xs:
  []             => (before, [])
  ["--", ...cmd] => (before, cmd)
  [x, ...rest]   => split_args(rest, before + [x])
;;

# Ok(output) of cmd on one host, or Err(why).
fn run_on(host, cmd):
  ssh = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=5", host]
  match timeout(30s, () => exec(ssh + cmd)):
    Ok(res)                       => Ok(res.out)
    Err({kind: 'timeout})         => Err("no answer in 30s")
    Err({kind: 'exit, code, err}) => Err("exit ${code}: ${trim(err)}")
    Err(e)                        => Err(e.message)
  ;;
;;

usage = "usage: pssh.fung hosts.txt -- command..."
(before, cmd) = split_args(args, [])
file = first(before) ?? fail(usage)
if is_empty(cmd): fail(usage) ;;
hosts = fs.read_lines(file)! |> map(trim) |> filter(h => h != "" and not starts_with(h, "#"))

results = conc.pmap(hosts, h => run_on(h, cmd), limit: 8)
width = hosts |> map(len) |> max
for (host, r) in zip(hosts, results):
  match r:
    Ok(out)  => each(lines(out), l => print(pad_right(host, width), "|", l))
    Err(why) => eprint(pad_right(host, width), "! FAILED:", why)
  ;;
;;
if any(results, is_err): exit(1) ;;
```

```expect
$ fung pssh.fung hosts.txt -- uptime
web1 |  09:12:01 up 41 days,  2:03,  0 users,  load average: 0.31, 0.28, 0.25
web2 |  09:12:01 up 12 days,  7:44,  1 user,  load average: 2.10, 1.95, 1.80
db1  ! FAILED: exit 255: ssh: connect to host db1 port 22: Connection refused
$ echo $?
1
```

## 줄별 설명 {#how}

- `split_args`: 인자 리스트를 `--` 앞과 뒤로 나눈다. 리스트 패턴 `["--", ...cmd]`는 "첫 원소가 `--`이고 나머지가 `cmd`"다. 앞쪽은 `before + [x]`로 하나씩 쌓으며 자기 자신을 다시 부른다(꼬리 호출이라 길어도 된다). `--` 뒤는 원격 명령이므로 `-h` 같은 옵션도 그대로 넘어간다.
- `ssh` 리스트: `BatchMode=yes`는 비밀번호를 묻지 않고 바로 실패하게 한다. 동시에 수십 대에서 비밀번호 프롬프트가 뜨면 어느 서버의 것인지 알 수 없기 때문이다. `ConnectTimeout=5`는 접속 단계에서만 5초를 기다린다.
- `exec(ssh + cmd)`: 명령을 문자열이 아니라 리스트로 넘기므로 이쪽 쉘이 끼어들지 않는다. 서버 이름에 무엇이 들어 있어도 따옴표 걱정이 없다. 끝나면 `Ok({out, err, code})`, 종료 코드가 0이 아니면 `Err({kind: 'exit, code, out, err, message})`다.
- `timeout(30s, () => …)`: 30초 안에 끝나지 않으면 `ssh`를 끝내고 `Err({kind: 'timeout})`을 준다. 접속은 됐지만 명령이 멈춘 서버(NFS가 걸린 `df` 등)도 여기서 끊긴다. 이 `Err`에는 `message`가 없으므로 따로 맞춘다.
- `match`의 맵 패턴: `Err({kind: 'exit, code, err})`는 "`kind`가 `'exit`인 맵"에만 맞고 `code`와 `err`를 꺼낸다. 위에서 아래로 처음 맞는 가지 하나만 돈다.
- `conc.pmap(hosts, …, limit: 8)`: `map`처럼 서버마다 함수를 부르되 여러 개를 동시에 돌린다. `limit: 8`이면 8대씩 묶어서 돌고, 결과 리스트는 끝난 순서가 아니라 **입력 순서** 그대로다. 그래서 `zip(hosts, results)`로 서버와 결과를 바로 짝지을 수 있다.
- 실패는 런타임 에러가 아니라 `Err` 값으로 결과 리스트에 남는다. 한 대가 실패해도 나머지는 끝까지 돈다.
- `width`: 가장 긴 서버 이름의 글자 수에 맞춰 `pad_right`로 칸을 맞춘다.
- `any(results, is_err)`: 다 찍은 뒤 하나라도 실패했으면 종료 코드 1. cron이나 CI에서 실패를 알아챌 수 있다.

:::box mistake 반복문 안에서 실패 개수 세기

쉘처럼 `failed=0`을 두고 반복문 안에서 `failed = failed + 1`을 쓰면 E0205다. fung의 블록은 바깥 변수를 바꾸지 못한다. 위처럼 결과를 다 모은 다음 식으로 센다.

```fung run
results = [Ok("up"), Err("refused"), Ok("up"), Err("timeout")]
failed = results |> filter(is_err) |> len
print("${failed} of ${len(results)} failed")
```

:::

:::box tip 원격 쪽은 여전히 쉘이다

`exec`가 이쪽 쉘을 거치지 않아도, `ssh`는 받은 인자를 공백으로 이어 **원격 서버의 쉘**에 넘긴다. 그래서 `-- df -h /`는 그대로 되지만, `-- 'ls /var/log/*.gz | wc -l'`처럼 파이프나 글로브를 원격에서 쓰려면 명령 전체를 인자 하나로 준다. 원격에 넘기는 값에 사용자가 준 문자열이 섞이면 원격에서 해석된다는 것을 기억한다.

:::

## 응용 {#vary}

- **결과를 표로**: 출력 줄을 그대로 찍지 않고 값을 뽑아 서버마다 한 줄로 모으려면 [서버별 결과를 표 하나로](server-table.html).
- **실패만 알리기와 재시도**: 응답 없는 서버만 골라 알리고 한두 번 다시 시도하려면 [실패·무응답 서버만 알리기](unreachable-servers.html).
- **파일 보내기**: `ssh` 대신 `["scp", local, "${host}:${remote}"]`나 `["rsync", "-a", local, "${host}:${remote}"]`를 같은 `run_on`에 넣는다.
- **더 많이 동시에**: `limit`을 올리면 빨라지지만, 내 컴퓨터가 동시에 여는 `ssh` 프로세스 수와 점프 호스트의 접속 제한(`MaxStartups`)을 넘지 않게 한다.
- **로그 모으기**: `-- tail -n 500 /var/log/app.log`로 받은 줄을 [여러 로그에서 패턴 세고 요약하기](count-log-patterns.html)의 `parse`에 넘긴다.

배우기: [12장 동시에 여러 일 하기](../learn/12-concurrency.html), [11장 파일과 외부 명령](../learn/11-files-and-commands.html), [8장 패턴](../learn/08-patterns.html). 레퍼런스: [`exec`](../reference/prelude/process.html#fn-exec), [`timeout`](../reference/prelude/concurrency.html#fn-timeout), [`conc.pmap`](../reference/modules/conc.html#fn-conc-pmap), [`zip`](../reference/prelude/collections.html#fn-zip), [`pad_right`](../reference/prelude/strings.html#fn-pad_right), [`exit`](../reference/prelude/program.html#fn-exit).
