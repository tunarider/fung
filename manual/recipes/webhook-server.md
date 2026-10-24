---
description: 웹훅을 받는 작은 HTTP 서버.
lede: Alertmanager 같은 시스템이 보내는 웹훅(HTTP POST)을 받아 토큰을 확인하고, 알림을 JSON 줄로 파일에 남긴다. SIGTERM을 받으면 처리 중인 요청을 마치고 깔끔하게 멈춘다.
---
# 웹훅을 받는 작은 HTTP 서버

모니터링(Alertmanager), CI, 저장소 호스팅 서비스는 일이 생기면 정해 둔 주소로 JSON을 POST한다. 이것이 웹훅(webhook)이다. 받는 쪽은 작은 HTTP 서버 하나면 된다. 여기서는 Alertmanager의 알림을 받는 서버를 만든다. `POST /alerts`에 `Authorization: Bearer 토큰`이 맞으면 알림마다 한 줄을 `alerts.jsonl`에 덧붙이고 화면에도 찍는다. `GET /healthz`는 지금까지 받은 알림 수를 답한다.

## 쉘로는 {#shell}

```shell
while true; do
  printf 'HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok' | nc -l 9095 >> requests.log
done
```

`nc`(netcat)로 흉내는 낼 수 있지만, 요청을 하나씩만 받고, 요청 줄과 헤더와 본문이 뒤섞인 채 파일에 쌓이며, 토큰 확인이나 JSON 해석은 다른 도구에 넘겨야 한다. `nc`의 옵션은 배포판마다 다르다. 결국 Python이나 다른 언어로 작은 서버를 따로 쓰게 된다.

## fung으로 {#code}

```fung check
#!/usr/bin/env fung
# Receives Alertmanager webhooks: POST /alerts with a bearer token. Each
# alert becomes one JSON line in the log file and one line on screen.
# GET /healthz answers how many alerts came in. Stops cleanly on Ctrl-C
# or SIGTERM, letting the requests in flight finish.
# usage: WEBHOOK_TOKEN=secret fung alerthook.fung [--addr 127.0.0.1:9095] [--log alerts.jsonl]
import cli
import fs
import http
import json
import time

opts = cli.parse(args)
addr = get(opts.flags, "addr", "127.0.0.1:9095")
log_file = get(opts.flags, "log", "alerts.jsonl")
token = env("WEBHOOK_TOKEN") ?? fail("set WEBHOOK_TOKEN")
received = atom(0)

fn reply(status, data) =
  {status, headers: {"content-type": "application/json"}, body: json.stringify(data)}

# One alert → the line kept in the log.
fn entry(a):
  labels = get(a, "labels") ?? {}
  {
    at: time.format(time.now()),
    status: get(a, "status", "?"),
    name: get(labels, "alertname", "?"),
    severity: get(labels, "severity", "-"),
    instance: get(labels, "instance", "-"),
  }
;;

fn alerts(req):
  if get(req.headers, "authorization", "") != "Bearer ${token}":
    return reply(401, {error: "bad token"})
  ;;
  match json.parse(req.body):
    Ok({alerts: list}) =>
      es = map(list, entry)
      each(es, fn(e):
        fs.append(log_file, json.stringify(e) + "\n")!
        print(e.status, e.severity, e.name, e.instance)
      ;;)
      swap(received, n => n + len(es))
      reply(200, {accepted: len(es)})
    Ok(_)              => reply(400, {error: "no alerts list"})
    Err(e)             => reply(400, {error: e.message})
  ;;
;;

fn handle(req) = match (req.method, req.path):
  ("POST", "/alerts") => alerts(req)
  ("GET", "/healthz") => reply(200, {ok: true, received: deref(received)})
  (_, "/alerts")      => reply(405, {error: "use POST"})
  _                   => reply(404, {error: "not found"})
;;

# Ok(nil) after a clean stop, Err when the address cannot be bound
match http.serve(addr, handle, ready: a => eprint("listening on", a), drain: 5s):
  Err(e) => fail(e.message)
  _      => eprint("stopped after", deref(received), "alerts")
;;
```

```expect
$ WEBHOOK_TOKEN=s3cret fung alerthook.fung
listening on 127.0.0.1:9095
firing critical DiskFull web2:9100
resolved warning HighLoad db1:9100
^C
stopped after 2 alerts
```

다른 터미널에서 `curl`로 보내 본 결과다(서버를 직접 돌려 확인한 응답).

```plain
$ curl -s -X POST -H 'Authorization: Bearer s3cret' -d @alert.json http://127.0.0.1:9095/alerts
{"accepted":2}
$ curl -s -X POST -H 'Authorization: Bearer wrong' -d @alert.json http://127.0.0.1:9095/alerts
{"error":"bad token"}
$ curl -s -X POST -H 'Authorization: Bearer s3cret' -d '{bad' http://127.0.0.1:9095/alerts
{"error":"invalid JSON at line 1, column 2: expected a string for an object key"}
$ curl -s http://127.0.0.1:9095/healthz
{"ok":true,"received":2}
$ cat alerts.jsonl
{"at":"2026-10-20T14:19:40.619352Z","status":"firing","name":"DiskFull","severity":"critical","instance":"web2:9100"}
{"at":"2026-10-20T14:19:40.619491Z","status":"resolved","name":"HighLoad","severity":"warning","instance":"db1:9100"}
```

## 줄별 설명 {#how}

- `token = env("WEBHOOK_TOKEN") ?? fail(…)`: 비밀값은 코드나 명령줄 인자(다른 사용자가 `ps`로 볼 수 있다)가 아니라 환경 변수로 받는다. 없으면 서버를 띄우지 않는다.
- `received = atom(0)`: 받은 알림 수다. `http.serve`는 요청마다 새 태스크(task, 동시에 도는 작업 단위)에서 처리기를 부르므로, 요청 사이에 나누는 상태는 `atom`에 둔다. `swap(received, n => n + len(es))`가 이전 값으로 새 값을 만들어 한 번에 바꾸고, `deref(received)`가 지금 값을 읽는다.
- `reply`: 응답은 `{status, headers, body}` 맵이다. 처리기가 문자열만 돌려주면 200과 `text/plain`이 된다.
- `entry`: 알림 하나에서 필요한 것만 골라 기록할 줄을 만든다. `labels`가 없거나 `null`이어도 `get(a, "labels") ?? {}`로 빈 맵을 쓴다. `a.labels`로 바로 읽으면 키가 없을 때 런타임 에러다.
- `alerts`: 먼저 토큰을 확인하고 틀리면 401로 일찍 돌아간다(`return`). 맵 패턴 `Ok({alerts: list})`는 "JSON이 맞고 `alerts` 키가 있다"를 한 번에 확인하고 그 값을 `list`로 꺼낸다. JSON이 아니면 `Err(e)`, 모양이 다르면 `Ok(_)` 가지로 가서 400을 답한다.
- `fs.append(…)!`: 줄을 파일 끝에 덧붙인다(없으면 만든다). 디스크가 차서 실패하면 `!`가 이 요청의 처리만 실패시킨다. 서버는 그 요청에 500으로 답하고 표준 에러에 `http: handler failed: …`를 남긴 뒤 계속 돈다. 보낸 쪽(Alertmanager)은 실패 응답을 보고 다시 보낸다.
- `handle`: `(메서드, 경로)` 튜플 하나로 `match`하면 라우팅 표처럼 읽힌다. `(_, "/alerts")`는 "다른 메서드로 온 `/alerts`"다.
- `http.serve(…, drain: 5s)`: 주소에서 요청을 받으며 여기서 멈춰 있다. 첫 Ctrl-C나 SIGTERM(systemd의 `stop`, 쿠버네티스의 파드 종료)을 받으면 새 연결을 그만 받고, 처리 중인 요청을 최대 5초 기다린 뒤 `Ok(nil)`을 돌려준다. 포트를 이미 누가 쓰고 있으면 `Err`다.
- 주소 기본값 `127.0.0.1`: 이 컴퓨터 안에서만 받는다. 다른 서버의 Alertmanager가 보내게 하려면 `--addr 0.0.0.0:9095`처럼 열고, 앞에 TLS를 맡는 프록시를 둔다(fung의 서버는 평문 HTTP만 받는다).

:::box tip `http.serve(…)!`로 줄여 쓰기

멈춘 뒤 할 일이 없으면 `match` 대신 `!`로 충분하다. 깔끔하게 멈추면 `Ok(nil)`이라 `!`가 지나가고 종료 코드 0으로 끝나며(systemd 같은 관리 도구가 정상 종료로 기록한다), 듣지 못했을 때만 그 `Err`로 멈춘다. 아래 예는 `ready`에서 `request_stop()`을 불러, 띄우자마자 SIGTERM을 받은 것처럼 멈추게 한 것이다.

```fung run
import http

r = http.serve("127.0.0.1:0", req => "hi", ready: _ => request_stop())
print("serve returned", r)
http.serve("127.0.0.1:0", req => "hi", ready: _ => request_stop())!
print("stopped cleanly")
```

:::

:::box mistake 처리기 안에서 바깥 변수 세기

요청 수를 `count = count + 1`로 세려고 하면, 처리기도 블록이라 바깥 변수에 대입할 수 없어 컴파일 에러 E0205다. 대입이 된다 해도 요청들이 동시에 돌기 때문에 숫자가 어긋난다. 요청 사이에 나누는 값은 `atom`에 두고 `swap`으로 바꾼다.

:::

## 응용 {#vary}

- **알림에 반응하기**: `severity`가 `critical`이면 `exec(["systemctl", "restart", "app"])`처럼 명령을 돌릴 수 있다. 명령이 오래 걸리면 보낸 쪽이 응답을 기다리다 시간 초과로 다시 보내므로, 처리기 안에서는 기록만 하고 무거운 일은 따로 도는 스크립트가 `alerts.jsonl`을 읽어 하게 하는 편이 안전하다.
- **다른 웹훅**: CI나 저장소 호스팅의 웹훅도 같은 모양이다. 서명(HMAC)으로 확인하는 서비스는 fung에 해시 함수가 없으므로 지금은 토큰 방식이나 앞단 프록시의 확인에 맡긴다.
- **systemd로 띄우기**: `ExecStart=/usr/local/bin/fung /opt/ops/alerthook.fung --log /var/log/alerthook.jsonl`, `Environment=WEBHOOK_TOKEN=…`(또는 `EnvironmentFile=`). `systemctl stop`이 보내는 SIGTERM에 `drain`이 맞춰 멈춘다.
- **포트 0**: `--addr 127.0.0.1:0`이면 운영체제가 빈 포트를 고르고, `ready`가 실제 주소를 알려 준다. 시험할 때 쓴다.
- **시험하기**: 저장소 `examples/api.fung`의 `test` 블록은 `scope` 안에서 서버를 띄우고 `http.get`·`http.post`로 확인한 뒤 끈다. `fung test`로 돌린다.

배우기: [13장 JSON과 HTTP](../learn/13-json-and-http.html), [12장 동시에 여러 일 하기](../learn/12-concurrency.html), [14장 테스트](../learn/14-testing.html). 레퍼런스: [`http.serve`](../reference/modules/http.html#fn-http-serve), [`atom`](../reference/prelude/concurrency.html#fn-atom), [`swap`](../reference/prelude/concurrency.html#fn-swap), [`request_stop`](../reference/prelude/concurrency.html#fn-request_stop), [`fs.append`](../reference/modules/fs.html#fn-fs-append), [`json.parse`](../reference/modules/json.html#fn-json-parse), [E0205](../reference/errors.html#E0205).
