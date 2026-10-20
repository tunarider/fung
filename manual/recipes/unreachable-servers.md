---
description: 실패·무응답 서버만 알리기 (시간 제한, 재시도).
lede: 모든 서버에 <code>ssh</code>로 접속해 보고, 세 번 시도해도 대답이 없는 서버만 알린다. 모두 살아 있으면 아무것도 찍지 않아 cron에 걸어 두기 좋다.
---
# 실패·무응답 서버만 알리기

서버 목록을 주기적으로 훑어 "접속이 안 되는 서버"만 알고 싶을 때가 있다. 이때 중요한 것은 세 가지다. 멈춘 서버 하나가 점검 전체를 붙잡지 않도록 시간 제한을 두고, 순간적인 네트워크 흔들림에 알림이 울리지 않도록 몇 번 다시 시도하고, 문제가 없으면 조용해야 한다. cron은 명령이 무엇이든 출력하면 메일을 보내므로 "문제없음"을 찍지 않는 것이 곧 기능이다. 여기에 환경 변수 `ALERT_WEBHOOK`이 있으면 Slack 같은 웹훅에도 보낸다.

## 쉘로는 {#shell}

```shell
for h in $(cat hosts.txt); do
  ok=no
  for i in 1 2 3; do timeout 10 ssh -o BatchMode=yes "$h" true && { ok=yes; break; }; sleep 2; done
  [ "$ok" = no ] && echo "DOWN $h"
done
```

동작은 하지만 서버를 한 대씩, 시도마다 최대 10초씩 기다리므로 죽은 서버가 많을수록 점검 한 번이 몇 분씩 걸린다. 실패 이유(이름 조회 실패인지, 접속 거부인지, 무응답인지)가 남지 않는다. `timeout` 명령은 GNU coreutils에 있고 macOS에는 기본으로 없다.

## fung으로 {#code}

```file hosts.txt
web1
web2
flaky
db2
slow
```

```fung check
#!/usr/bin/env fung
# Checks that every server answers over ssh, trying each up to three
# times. Prints nothing when all answer (quiet in cron); otherwise one
# line per server that did not, and exit code 1. With ALERT_WEBHOOK set,
# also posts the list there as JSON.
# usage: fung reachable.fung hosts.txt
import conc
import fs
import http
import json

# One try: Ok(nil) when the server ran `true`, else Err(why).
fn try_once(host):
  cmd = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=5", host, "true"]
  match timeout(10s, () => exec(cmd)):
    Ok(_)                   => Ok(nil)
    Err({kind: 'timeout})   => Err("no answer in 10s")
    Err({kind: 'exit, err}) => Err(trim(err))
    Err(e)                  => Err(e.message)
  ;;
;;

fn check(host) = conc.retry(() => try_once(host), times: 3, delay: 2s)

file = first(args) ?? fail("usage: reachable.fung hosts.txt")
hosts = fs.read_lines(file)! |> filter(h => h != "")
down = zip(hosts, conc.pmap(hosts, check, limit: 20))
  |> filter(((_, r)) => is_err(r))
  |> map(((host, Err(why))) => {host, why})

if is_empty(down): exit(0) ;;
for d in down:
  print("DOWN", d.host, "-", d.why)
;;
match env("ALERT_WEBHOOK"):
  nil => nil
  url =>
    text = "${len(down)} of ${len(hosts)} servers down: ${down |> map(d => d.host) |> join(", ")}"
    http.post(url, json.stringify({text}), headers: {"content-type": "application/json"})!
;;
exit(1)
```

```expect
$ ALERT_WEBHOOK=https://hooks.example.com/T000/B000 fung reachable.fung hosts.txt
DOWN db2 - ssh: connect to host db2 port 22: Connection refused
DOWN slow - no answer in 10s
$ echo $?
1
```

`flaky`는 첫 시도에 실패했지만 두 번째에 대답해서 목록에 없다. 웹훅에는 `{"text":"2 of 5 servers down: db2, slow"}`가 간다. 서버가 모두 대답하면 아무것도 찍지 않고 종료 코드 0으로 끝난다.

## 줄별 설명 {#how}

- `try_once`: 접속해서 아무 일도 하지 않는 명령 `true`를 돌린다. 접속과 로그인과 명령 실행이 모두 되어야 성공이다. 결과를 `Ok(nil)`이나 `Err(이유 문자열)` 둘로만 줄인다. `timeout`의 `Err`에는 `message`가 없어서 따로 맞춘다.
- `timeout(10s, …)`과 `ConnectTimeout=5`: `ConnectTimeout`은 접속 단계만 지킨다. 접속은 됐는데 로그인 뒤에 멈추는 서버(디스크가 걸린 서버의 셸 초기화 등)는 `timeout`이 10초에서 `ssh`를 끝낸다.
- `conc.retry(f, times: 3, delay: 2s)`: `f()`가 `Err`를 돌려주면 2초 쉬고 다시 부른다. 세 번 모두 실패하면 마지막 `Err`를 돌려준다. `f`는 인자 없는 함수여야 하므로 `() => try_once(host)`로 감싼다.
- `conc.pmap(hosts, check, limit: 20)`: 20대씩 동시에 점검한다. 죽은 서버가 몇 대든 점검 시간은 대략 "한 대가 세 번 실패하는 시간"(10초 × 3 + 쉼 2초 × 2)으로 묶인다.
- `zip` 뒤의 두 줄: `(서버, 결과)` 짝에서 실패만 남기고, 람다의 매개변수 패턴 `((host, Err(why)))`로 튜플과 그 안의 `Err`를 한 번에 풀어 맵을 만든다. 앞 줄에서 `Err`만 남겼으므로 이 패턴은 늘 맞는다.
- `if is_empty(down): exit(0) ;;`: 문제가 없으면 여기서 조용히 끝난다.
- `match env("ALERT_WEBHOOK")`: 환경 변수가 없으면 `nil`이다. 있으면 그 주소로 JSON을 보낸다. `{text}`는 `{text: text}`의 축약이다. 웹훅 전송이 실패하면 `!`가 멈추고 종료 코드는 1이 된다. 알림이 안 갔다는 것을 숨기지 않기 위해서다.
- 마지막 `exit(1)`: 하나라도 죽었으면 1로 끝내 cron 래퍼나 모니터링이 실패로 볼 수 있게 한다.

:::box mistake 재시도할 함수 안의 `!`

`conc.retry`는 `f`가 `Err` **값을 돌려줄 때만** 다시 시도한다. 쉘의 `set -e`처럼 `exec(…)!`를 습관으로 붙이면, 첫 실패에서 `!`가 프로그램 전체를 멈춰서 재시도할 기회가 없다.

```fung run exit=1 error=E0603
import conc

r = conc.retry(() => exec(["false"])!, times: 3)
print(r)
```

재시도할 함수는 `Err`를 그대로 돌려주게 쓰고(`() => exec(["false"])`), `!`는 재시도가 다 끝난 결과에 붙인다.

:::

## 응용 {#vary}

- **HTTP 서비스도 함께**: 같은 모양으로 `try_once`만 `http.get(url, timeout: 5s)`로 바꾸면 웹 서비스 점검이 된다. 상태 코드까지 보는 판은 [URL 여러 개 상태 동시 확인](check-urls.html).
- **알림 폭주 막기**: 직전 결과를 `fs.write("/var/tmp/reachable.last", …)`로 남기고, 이번 목록이 지난번과 같으면 웹훅을 건너뛴다. 새로 죽은 서버와 살아난 서버만 알릴 수 있다.
- **종료 코드로 나누기**: 모니터링 도구의 규칙(0 정상, 1 경고, 2 위험)에 맞추려면 죽은 비율로 `exit(1)`과 `exit(2)`를 고른다. [종료 코드와 표준 에러](exit-codes.html).
- **cron에 걸기**: `*/5 * * * * fung /opt/ops/reachable.fung /opt/ops/hosts.txt`. 출력이 있을 때만 cron이 메일을 보낸다.

배우기: [12장 동시에 여러 일 하기](../learn/12-concurrency.html), [9장 에러](../learn/09-errors.html). 레퍼런스: [`conc.retry`](../reference/modules/conc.html#fn-conc-retry), [`conc.pmap`](../reference/modules/conc.html#fn-conc-pmap), [`timeout`](../reference/prelude/concurrency.html#fn-timeout), [`env`](../reference/prelude/process.html#fn-env), [`http.post`](../reference/modules/http.html#fn-http-post), [`exit`](../reference/prelude/program.html#fn-exit), [결과값과 에러](../reference/language.html#results).
