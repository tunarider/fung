---
description: JSON API에서 필드 뽑기 (curl + jq 대신).
lede: HTTP API의 JSON 응답에서 필요한 필드만 골라 표로 찍는다. 주소를 주면 직접 받고, 주지 않으면 표준 입력을 읽어서 <code>curl</code>이나 <code>kubectl … -o json</code> 뒤에도 붙는다.
---
# JSON API에서 필드 뽑기

배포 시스템, 모니터링, 클라우드 API는 거의 다 JSON을 돌려준다. 보고 싶은 것은 그중 몇 필드뿐이다. 여기서는 서비스 목록 API(`{"items": [...]}`)에서 이름, 상태, 준비된 복제본 수, 담당 팀을 뽑아 표로 찍는다. 깊이 들어 있는 필드(`status.replicas.ready`)가 어떤 서비스에는 없거나 `null`이어도 멈추지 않고 기본값을 쓴다. `--state degraded`로 상태를 거를 수 있다.

## 쉘로는 {#shell}

```shell
curl -s https://deploy.example.com/api/v1/services \
  | jq -r '.items[] | [.name, .status.state, "\(.status.replicas.ready // 0)/\(.status.replicas.want // 0)", (.labels.team // "-")] | @tsv'
```

`jq`는 이 일에 아주 잘 맞는 도구다. 다만 `jq`는 그 자체로 작은 언어라서 조금만 길어져도 따로 배워야 하고, 서버나 CI 이미지에 늘 깔려 있지는 않다. `curl -s`는 HTTP 500 응답의 에러 본문도 조용히 `jq`에 넘기므로 `-f`를 잊으면 `jq`가 이상한 에러를 낸다. 결과를 가지고 다른 일(다른 API 부르기, 파일 쓰기, 종료 코드 정하기)을 하려면 다시 쉘로 돌아와야 한다.

## fung으로 {#code}

```file services.json
{
  "items": [
    {"name": "api", "status": {"state": "running", "replicas": {"ready": 3, "want": 3}}, "labels": {"team": "core"}},
    {"name": "worker", "status": {"state": "degraded", "replicas": {"ready": 1, "want": 4}}, "labels": {"team": "data"}},
    {"name": "cron", "status": {"state": "running", "replicas": {"ready": 1, "want": 1}}, "labels": {}},
    {"name": "search", "status": {"state": "stopped", "replicas": null}, "labels": {"team": "core"}}
  ],
  "next": null
}
```

```fung run stdin=services.json
#!/usr/bin/env fung
# Prints the services of a deploy API as a table: name, state, replicas
# ready/wanted, team. Reads the JSON from a URL, or from standard input:
#   fung services.fung https://deploy.example.com/api/v1/services
#   curl -s https://deploy.example.com/api/v1/services | fung services.fung
# --state STATE keeps only the services in that state.
import cli
import http
import json

fn fetch(url):
  res = http.get(url, headers: {accept: "application/json"}, timeout: 10s)?
  if res.status != 200: return Err({kind: 'http, message: "${url}: HTTP ${res.status}"}) ;;
  Ok(res.body)
;;

# The value at a path of keys, or nil when any step is missing.
fn dig(v, path) = match path:
  []           => v
  [k, ...rest] => type_of(v) == 'map then dig(get(v, k), rest) else nil
;;

opts = cli.parse(args)
text = match first(opts.rest):
  nil => join(read_lines()!, "\n")
  url => fetch(url)!
;;
items = dig(json.parse(text)!, ["items"]) ?? []
wanted = get(opts.flags, "state")

rows = items
  |> filter(s => wanted == nil or dig(s, ["status", "state"]) == wanted)
  |> map(s => [
    s.name,
    dig(s, ["status", "state"]) ?? "?",
    "${dig(s, ["status", "replicas", "ready"]) ?? 0}/${dig(s, ["status", "replicas", "want"]) ?? 0}",
    dig(s, ["labels", "team"]) ?? "-",
  ])
for [name, state, ready, team] in rows:
  print(pad_right(name, 8), pad_right(state, 9), pad_left(ready, 5), " ", team)
;;
```

여기서는 준비해 둔 파일을 표준 입력으로 넣었다(`fung services.fung < services.json`). 실제로는 주소를 인자로 주거나 `curl -s … |` 뒤에 붙인다. `cron`은 팀 라벨이 없어 `-`, `search`는 `replicas`가 `null`이라 `0/0`이 되었다.

## 줄별 설명 {#how}

- `json.parse(text)!`: JSON 텍스트를 fung 값으로 바꾼다. 객체는 맵, 배열은 리스트, 숫자는 정수나 실수, `null`은 `nil`이 된다. 잘못된 JSON이면 `Err({kind: 'parse, line, col, message})`이고, `!`가 줄과 칸을 알려 주며 멈춘다.
- `dig`: jq의 `.status.replicas.ready`에 해당한다. 키 리스트를 따라 한 단계씩 들어가다가 중간에 맵이 아닌 것(없는 키의 `nil`, JSON의 `null`)을 만나면 `nil`을 돌려준다. `s.status.replicas.ready`처럼 바로 쓰면 없는 키에서 런타임 에러로 멈추므로, 있을 수도 없을 수도 있는 필드에는 `dig`나 `get(m, k, 기본값)`을 쓴다.
- `?? "?"`, `?? 0`, `?? "-"`: `dig`가 `nil`이면 기본값을 쓴다. jq의 `//`와 같은 일이다.
- `fetch`: `http.get`은 네트워크가 실패하면(이름 조회, 접속, 시간 초과) `Err`를, 서버가 대답하면 상태 코드와 상관없이 `Ok({status, headers, body})`를 준다. 그래서 `200`이 아니면 직접 `Err`를 만든다. `?`는 `Err`면 그것을 그대로 호출한 쪽으로 돌려보내고, 바깥의 `fetch(url)!`이 메시지를 찍고 멈춘다. `curl -f`를 잊는 실수가 생기지 않는다.
- `text = match first(opts.rest): …`: 주소가 없으면(`nil`) 표준 입력 전부를, 있으면 HTTP로 받는다. `match`도 값을 돌려주는 식이라 결과를 바로 대입한다.
- `map(s => [ … ])`: 서비스 하나를 문자열 네 칸의 리스트로 만든다. 괄호 안에서는 줄을 바꿔 써도 된다. 문자열 보간 `"${…}/${…}"` 안에는 어떤 식이든 쓸 수 있다.
- `for [name, state, ready, team] in rows`: `for`의 변수 자리에도 패턴을 쓴다. 칸 네 개를 이름으로 받는다.

:::box mistake JSON 값과 심볼 비교하기

fung에서는 상태 같은 이름을 `'running` 같은 심볼로 쓰는 일이 많다. 그런데 JSON·파일·명령 출력에서 온 값은 언제나 **문자열**이다. 둘을 `==`로 비교하면 조용히 거짓이 되는 대신 E0517로 멈춘다. 문자열끼리 비교하거나, 문자열을 `sym(s)`로 바꿔서 비교한다.

```fung run exit=1 error=E0517
import json

svc = json.parse(`{"name": "api", "state": "running"}`)!
print(svc.state == "running", sym(svc.state) == 'running)
print(svc.state == 'running)
```

:::

## 응용 {#vary}

- **인증 헤더**: `http.get(url, headers: {authorization: "Bearer ${env("API_TOKEN") ?? fail("API_TOKEN is not set")}"})`. 토큰은 스크립트에 쓰지 않고 환경 변수로 받는다.
- **kubectl 출력**: `kubectl get pods -o json | fung pods.fung`도 같은 방식이다. `dig(p, ["status", "phase"])`, 컨테이너별 재시작 횟수는 `(dig(p, ["status", "containerStatuses"]) ?? []) |> map(c => c.restartCount) |> sum`.
- **JSON으로 다시 내보내기**: 골라낸 것을 다음 도구에 넘기려면 `print(json.stringify(rows))`. 들여쓰기는 `json.stringify(v, indent: 2)`. [CSV와 JSON으로 저장하기](save-csv-json.html).
- **여러 페이지**: 응답의 `next`가 다음 주소를 가리키면 [페이지 넘기며 모두 받기](pagination.html)처럼 끝까지 따라간다.
- **여러 API 동시에**: 지역마다 API가 따로 있으면 `conc.pmap(urls, fetch)`로 한 번에 받는다.

배우기: [13장 JSON과 HTTP](../learn/13-json-and-http.html), [9장 에러](../learn/09-errors.html), [7장 컬렉션](../learn/07-collections.html). 레퍼런스: [`json.parse`](../reference/modules/json.html#fn-json-parse), [`http.get`](../reference/modules/http.html#fn-http-get), [`get`](../reference/prelude/maps.html#fn-get), [`type_of`](../reference/prelude/convert.html#fn-type_of), [`sym`](../reference/prelude/convert.html#fn-sym), [비교와 동일성](../reference/language.html#values), [E0517](../reference/errors.html#E0517).
