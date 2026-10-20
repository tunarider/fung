---
description: JSON 읽고 쓰기(json.parse·json.stringify), HTTP 요청(http.get·http.post)과 작은 서버(http.serve), 시각과 기간(time).
lede: 요즘 도구는 대부분 JSON으로 말하고 HTTP로 주고받는다. 이 장은 API에서 데이터를 받아 다루고, 작은 HTTP 서버를 띄우고, 시각을 계산하는 법을 배운다.
---
# JSON과 HTTP

:::goals
- JSON이 무엇이고 fung 값과 어떻게 대응하는지
- `json.parse`로 읽고 `json.stringify`로 쓰기
- HTTP의 요청과 응답, `http.get`·`http.post`로 API 부르기
- `http.serve`로 작은 서버 만들기
- `time`으로 시각을 읽고, 계산하고, 문자열로 바꾸기
:::

## JSON이란 {#json}

프로그램끼리 데이터를 주고받으려면 양쪽이 아는 글자 형식이 필요하다. **JSON**(JavaScript Object Notation)은 그런 형식 중 가장 널리 쓰인다. Kubernetes의 `kubectl get -o json`, 클라우드 API의 응답, 많은 설정 파일이 JSON이다. 생김새는 7장의 리스트·맵과 거의 같다.

```plain
{"name": "api", "replicas": 3, "ports": [80, 443], "debug": false, "owner": null}
```

JSON의 값은 fung의 값으로 이렇게 바뀐다.

<table>
  <thead><tr><th>JSON</th><th>fung</th></tr></thead>
  <tbody>
    <tr><td>객체 <code>{"k": v}</code></td><td>맵(키 순서를 지킨다)</td></tr>
    <tr><td>배열 <code>[1, 2]</code></td><td>리스트</td></tr>
    <tr><td>문자열 <code>"a"</code></td><td>문자열</td></tr>
    <tr><td>소수점·지수 없는 숫자 <code>3</code></td><td>정수(<code>int</code>)</td></tr>
    <tr><td>그 밖의 숫자 <code>0.5</code></td><td>실수(<code>float</code>)</td></tr>
    <tr><td><code>true</code> <code>false</code></td><td><code>true</code> <code>false</code></td></tr>
    <tr><td><code>null</code></td><td><code>nil</code></td></tr>
  </tbody>
</table>

## JSON 읽기 {#parse}

`json` 모듈의 `json.parse(문자열)`은 JSON 문자열을 fung 값으로 바꿔 `Ok(값)`을 준다. 형식이 틀렸으면 `Err`다. 아래 예제의 `` `…` ``는 이스케이프 없는 원시 문자열(6장)이라, 안의 `"`를 그대로 쓸 수 있다.

```fung run
import json

text = `{"name": "api", "replicas": 3, "ports": [80, 443], "debug": false, "owner": null}`
svc = json.parse(text)!

print(svc)
print(svc.name, svc.ports[0], svc.owner)
print(type_of(svc.replicas), len(svc.ports))
```

형식이 틀린 JSON의 `Err`는 몇째 줄 몇째 칸에서 틀렸는지(`line`, `col`)도 알려 준다.

```fung run
import json

print(json.parse(`{"name": "api",}`))
```

읽은 값은 평범한 맵과 리스트라서 7장의 함수와 8장의 패턴을 그대로 쓴다. 파일에 든 JSON은 `fs.read`와 이어서 읽는다(11장).

```file services.json
[
  {"name": "api", "status": "up", "replicas": 3},
  {"name": "worker", "status": "down", "replicas": 0},
  {"name": "cron", "status": "up", "replicas": 1}
]
```

```fung run
import fs
import json

services = json.parse(fs.read("services.json")!)!
for svc in services:
  match svc:
    {name, status: "down"} => print("DOWN", name)
    {name, replicas}       => print("up  ", name, "x${replicas}")
  ;;
;;
```

:::box mistake JSON의 문자열과 심볼을 비교하기
JSON에서 온 값은 늘 문자열이다. `svc.status == 'down`처럼 심볼과 비교하면 조용히 거짓이 되는 대신 런타임 에러 E0517로 멈춘다. 가장 흔한 함정이라서 일부러 에러로 만들었다. 문자열과 비교하거나(`svc.status == "down"`), `sym(svc.status) == 'down`으로 바꿔 비교한다. 패턴 `{status: "down"}`도 문자열로 쓴다.

```fung run exit=1 error=E0517
import json

svc = json.parse(`{"status": "down"}`)!
print(svc.status == 'down)
```

:::

## JSON 쓰기 {#stringify}

`json.stringify(값)`은 fung 값을 JSON 문자열로 바꾼다. `indent:`에 칸 수를 주면 사람이 읽기 좋게 줄을 나눈다. 튜플은 배열, 심볼은 문자열, 시각은 ISO 8601 문자열이 된다. JSON에 없는 값(기간, 함수, 태그 등)은 런타임 에러 E0513이므로 `str(d)`처럼 먼저 바꿔 둔다.

```fung run
import json

report = {host: "web1", status: 'ok, load: (0.5, 0.7), checked_in: str(1m30s)}
print(json.stringify(report))
print(json.stringify({services: ["api", "cron"], healthy: true}, indent: 2))
```

JSON 설정 파일의 값 하나를 고쳐 다시 쓰는 일은 읽기, `put`(7장), 쓰기를 잇는다.

```file deploy.json
{"name": "api", "replicas": 2, "image": "api:1.4"}
```

```fung run
import fs
import json

cfg = json.parse(fs.read("deploy.json")!)!
scaled = put(cfg, "replicas", cfg.replicas + 1)
fs.write("deploy.json", json.stringify(scaled, indent: 2) + "\n")!
print(fs.read("deploy.json")!)
```

## HTTP: 요청과 응답 {#http}

**HTTP**는 웹에서 쓰는 주고받기 규칙이다. 한쪽(**클라이언트**)이 **요청**(request)을 보내면 다른 쪽(**서버**)이 **응답**(response)을 돌려준다. 요청에는 **메서드**(무엇을 할지: `GET`은 가져오기, `POST`는 보내기), **URL**(어디에), **헤더**(덧붙이는 정보), **본문**(보내는 내용)이 있다. 응답에는 **상태 코드**(200 성공, 404 없음, 500 서버 오류 …), 헤더, 본문이 있다.

`http` 모듈의 `http.get(url)`은 GET 요청을 보내고 `Ok({status, headers, body})`를 준다. 응답을 받기만 했으면 상태 코드가 404여도 `Ok`다. 상태 코드를 보는 것은 내 코드의 몫이다. 연결이 안 되거나 이름을 못 찾으면 `Err({kind: 'network, …})`, 시간 안에 끝나지 않으면 `Err({kind: 'timeout})`이다. 시간 한도는 `timeout:`으로 주고 기본은 30초다. 응답 헤더의 이름은 소문자다.

네트워크 예제는 이 매뉴얼을 만들 때 실행하지 않는다. 아래 출력은 실제로 돌렸을 때의 모양이고, 날짜처럼 바뀌는 값은 다를 수 있다.

```fung check
import http

r = http.get("https://example.com", timeout: 10s)!
print(r.status, r.headers["content-type"])
print(len(r.body) > 0)
print(http.get("http://127.0.0.1:1/"))
```

```expect
200 text/html; charset=utf-8
true
Err({kind: 'network, message: "cannot request `http://127.0.0.1:1/`: connection refused"})
```

JSON API는 `http.get`과 `json.parse`를 잇는다. 실패할 수 있는 단계가 둘이므로 함수로 묶고 `?`로 넘기면 깔끔하다(9장). 상태 코드가 200이 아닌 것도 실패로 바꿔 준다.

```fung check
import http
import json

fn get_json(url):
  r = http.get(url, headers: {accept: "application/json"}, timeout: 5s)?
  if r.status != 200:
    return Err({kind: 'http, message: "${url}: status ${r.status}"})
  ;;
  json.parse(r.body)
;;

repo = get_json("https://api.github.com/repos/tunarider/fung")!
print(repo.full_name, repo.default_branch)
print(get_json("https://api.github.com/repos/tunarider/no-such-repo-xyz"))
```

```expect
tunarider/fung main
Err({kind: 'http, message: "https://api.github.com/repos/tunarider/no-such-repo-xyz: status 404"})
```

`http.post(url, 본문)`은 본문을 문자열로 받는다. JSON을 보내려면 `json.stringify`로 바꾸고 `content-type` 헤더를 붙인다. 웹훅에 알림을 보내는 모양이다. 아래의 `httpbin.org/post`는 받은 요청을 JSON으로 되돌려 주는 시험용 서비스라서, 보낸 본문이 응답의 `json` 필드에 그대로 돌아온다.

```fung check
import http
import json

payload = json.stringify({text: "deploy finished: api 1.4.2"})
r = http.post("https://httpbin.org/post", payload, headers: {"content-type": "application/json"})!
print(r.status, json.parse(r.body)!.json)
```

```expect
200 {text: "deploy finished: api 1.4.2"}
```

## 작은 HTTP 서버 {#serve}

`http.serve(주소, 처리기)`는 주소(`":8080"`, `"127.0.0.1:8080"`)에서 요청을 기다리는 서버를 띄운다. 요청이 올 때마다 새 태스크(12장)에서 `처리기(req)`를 부른다. `req`는 `{method, path, query, headers, body}` 맵이다. 처리기는 문자열(상태 200의 텍스트) 또는 `{status, headers, body}` 맵을 돌려준다. 처리기가 런타임 에러를 내면 그 요청에만 500으로 답하고 서버는 계속 돈다. `ready:`에 함수를 주면 서버가 듣기 시작한 직후 실제 주소로 한 번 부른다.

서버는 멈추지 않고 계속 돌기 때문에 이 예제도 매뉴얼에서는 실행하지 않는다. Ctrl-C(SIGINT)나 SIGTERM을 받으면 새 연결을 그만 받고 처리 중인 요청을 마친 뒤 `nil`을 돌려주며 끝난다. 주소를 열지 못하면(이미 쓰는 포트 등) `Err`를 돌려주므로, 아래처럼 문장으로 두면 그 `Err`가 버려져 E0606으로 멈춘다(9장). `nil`에 `!`를 붙이면 에러(E0604)이므로 여기에는 `!`를 붙이지 않는다.

```fung check
import http
import json

hits = atom(0)

fn handle(req):
  swap(hits, n => n + 1)
  match (req.method, req.path):
    ("GET", "/health") => "ok"
    ("GET", "/hello")  => "hello, ${get(req.query, "name", "stranger")}"
    ("GET", "/stats")  => {status: 200, headers: {"content-type": "application/json"}, body: json.stringify({hits: deref(hits)})}
    _                  => {status: 404, body: "not found: ${req.path}"}
  ;;
;;

http.serve("127.0.0.1:8080", handle, ready: addr => print("listening on", addr))
```

```expect
listening on 127.0.0.1:8080
```

다른 터미널에서 `curl`로 부르면 이렇게 답한다.

```shell
$ curl -s 'http://127.0.0.1:8080/hello?name=fung'
hello, fung
$ curl -s http://127.0.0.1:8080/stats
{"hits":2}
$ curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8080/nope
404
```

처리기는 여러 요청을 동시에 처리할 수 있으므로, 요청 수 같은 공유 상태는 12장의 `atom`에 둔다.

## 시각과 기간 {#time}

**기간**(duration)은 3장에서 본 `500ms`, `30s`, `1h30m` 같은 값이다. 기간끼리 더하고 빼고, 숫자를 곱하고, 같은 단위로 나눠 숫자로 바꾼다. 문자열의 기간은 `parse_dur`로 읽는다.

```fung run
d = 90s
print(d, d / 1s, d // 1m)
print(1h30m + 15m, 3 * 45s)
print(parse_dur("2m30s"))
```

**시각**(time)은 `time` 모듈이 다룬다. `time.now()`는 지금 시각, `time.parse(문자열)`은 ISO 8601(RFC 3339) 문자열을 시각으로 읽는다. 시각에 기간을 더하면 시각, 시각에서 시각을 빼면 기간이다. `time.format(t, 형식)`은 `YYYY` `MM` `DD` `HH` `mm` `ss` 같은 토큰으로 문자열을 만든다.

```fung run
import time

deployed = time.parse("2026-10-20T09:30:00Z")!
checked = time.parse("2026-10-21T10:00:00Z")!

print(deployed + 90m)
print(checked - deployed, (checked - deployed) // 1h, "hours")
print(time.format(deployed, "YYYY-MM-DD HH:mm"))
print(time.parse("20/10/2026 18:00", format: "DD/MM/YYYY HH:mm"))
```

걸린 시간을 재려면 두 `time.now()`의 차를 구한다. 지금 시각은 실행할 때마다 다르므로 아래 예제는 출력을 싣지 않았다. 직접 돌려 보면 `now: 2026-10-20T13:54:07.173139Z`처럼 지금 시각과 `took about 50 ms`쯤이 찍힌다.

```fung run hide-output
import time

start = time.now()
sleep(50ms)
print("now:", start)
print("took about", (time.now() - start) // 10ms * 10, "ms")
```

:::box other 다른 언어는
쉘에서는 `curl`로 받고 `jq`로 JSON을 다루고 `date`로 시각을 계산하며, 세 도구의 문법이 저마다 다르다. fung에서는 셋이 같은 값(맵·리스트·시각) 위에서 이어진다. Python의 `json`·`urllib`·`datetime`이나 JavaScript의 `JSON.parse`·`fetch`·`Date`와 하는 일이 같다.
:::

:::exercise 내려간 서비스 알리기
`services.json`(위와 같은 모양)을 읽어 `status`가 `"down"`인 서비스 이름을 쉼표로 이어 출력하고, 하나라도 있으면 종료 코드 1로 끝내 보자. 없으면 `all services up`을 출력한다.

:::solution

```file services.json
[
  {"name": "api", "status": "up"},
  {"name": "worker", "status": "down"},
  {"name": "mailer", "status": "down"}
]
```

```fung run exit=1
import fs
import json

services = json.parse(fs.read("services.json")!)!
down = services |> filter(s => s.status == "down") |> map(s => s.name)
if is_empty(down):
  print("all services up")
else:
  eprint("down:", join(down, ", "))
  exit(1)
;;
```

:::
:::

:::exercise 점검 기록을 JSON으로
서버 이름과 응답 시간(기간)의 리스트 `[("web1", 120ms), ("web2", 2s)]`를 받아, `{"checked": 2, "slow": ["web2"], "times_ms": {"web1": 120, "web2": 2000}}` 꼴의 JSON 문자열을 만들어 보자. 1초 이상이면 느린 서버다. 힌트: 기간을 ms 정수로 바꾸려면 `d // 1ms`.

:::solution

```fung run
import json

fn summarize(results) = {
  checked: len(results),
  slow: results |> filter(((_, d)) => d >= 1s) |> map(((h, _)) => h),
  times_ms: fold(results, {}, (acc, (h, d)) => put(acc, h, d // 1ms)),
}

print(json.stringify(summarize([("web1", 120ms), ("web2", 2s)])))
```

:::
:::

## 정리 {#summary}

- `json.parse`는 JSON 문자열을 맵·리스트·문자열·숫자·`nil`로 바꿔 `Ok`를 주고, `json.stringify`는 그 반대다(`indent:`로 보기 좋게).
- JSON에서 온 값은 문자열이다. 심볼과 비교하면 E0517이므로 문자열과 비교하거나 `sym`으로 바꾼다.
- `http.get`·`http.post`는 응답을 받으면 상태 코드와 상관없이 `Ok({status, headers, body})`, 전송 실패는 `Err('network)`, 시간 초과는 `Err('timeout)`.
- `http.serve(주소, 처리기)`는 요청마다 태스크에서 처리기를 부른다. 처리기는 문자열이나 `{status, headers, body}`를 돌려준다.
- 기간은 `90s`·`1h30m`처럼 쓰고, 시각은 `time.now`·`time.parse`·`time.format`으로 다룬다. 시각 ± 기간은 시각, 시각 − 시각은 기간이다.

함수마다의 규칙은 레퍼런스의 [json](../reference/modules/json.html), [http](../reference/modules/http.html), [time](../reference/modules/time.html)에 있다. 실무 예는 예제 모음의 [JSON API에서 필드 뽑기](../recipes/json-fields.html), [URL 상태 동시 확인](../recipes/check-urls.html), [웹훅 서버](../recipes/webhook-server.html)를 보라.
