---
description: URL 여러 개 상태 동시에 확인하기.
lede: 목록 파일의 URL을 모두 동시에 요청해 상태 코드와 걸린 시간을 찍고, 하나라도 실패하면 종료 코드 1로 끝난다. 배포 직후 점검이나 cron 감시에 쓴다.
---
# URL 여러 개 상태 동시에 확인하기

배포가 끝나면 서비스의 헬스 체크 주소, 대표 페이지, 내부 API 몇 개가 제대로 대답하는지 확인한다. 하나씩 `curl`을 돌리면 응답이 느린 주소 하나가 나머지를 기다리게 한다. 여기서는 URL 목록 파일을 받아 모두 **동시에** 요청하고, 결과를 목록 순서대로 한 줄씩 찍는다. 400 이상의 상태 코드, 연결 실패, 시간 초과는 모두 실패로 센다.

## 쉘로는 {#shell}

```shell
while read -r u; do
  curl -s -o /dev/null -m 5 -w "%{http_code} %{time_total}s $u\n" "$u"
done < urls.txt
```

`curl`의 `-w`는 편하지만 주소를 하나씩 차례로 요청한다. 연결이 안 되면 상태 코드 자리에 `000`이 찍혀서 "거절"인지 "이름 없음"인지 "시간 초과"인지 따로 봐야 한다(`curl`의 종료 코드를 보면 되지만 반복문 안에서 모으기 번거롭다). 하나라도 실패했을 때 스크립트 전체를 실패로 끝내려면 플래그 변수를 따로 관리해야 한다.

## fung으로 {#code}

```file urls.txt
# after deploy: the public site and the internal APIs
https://www.example.com/healthz
https://www.example.com/old-path
https://api.example.com/v1/missing
https://reports.internal.example.com/slow
https://old.example.com/
```

```fung check
#!/usr/bin/env fung
# Checks many URLs at once: status, time taken, and whether the answer
# was good (below 400, within the time limit). Exit code 1 if any was not.
# usage: fung checkurls.fung urls.txt [--timeout 5s]
import conc
import cli
import fs
import http
import time

fn check(url, limit):
  t0 = time.now()
  r = http.get(url, timeout: limit)
  ms = (time.now() - t0) // 1ms
  match r:
    Ok(res) => {url, ok: res.status < 400, status: str(res.status), ms, note: ""}
    Err(e)  => {url, ok: false, status: "-", ms, note: " (${e.message})"}
  ;;
;;

opts = cli.parse(args)
file = first(opts.rest) ?? fail("usage: checkurls.fung urls.txt [--timeout 5s]")
limit = parse_dur(get(opts.flags, "timeout", "5s")) ?? fail("--timeout takes a duration like 5s")
urls = fs.read_lines(file)! |> map(trim) |> filter(u => u != "" and not starts_with(u, "#"))

results = conc.pmap(urls, u => check(u, limit), limit: 16)
for r in results:
  mark = r.ok then "ok  " else "FAIL"
  print(mark, pad_left(r.status, 3), pad_left("${r.ms}ms", 7), r.url + r.note)
;;
bad = results |> filter(r => not r.ok) |> len
eprint("${len(results) - bad} ok, ${bad} failed")
if bad > 0: exit(1) ;;
```

```expect
$ fung checkurls.fung urls.txt --timeout 2s
ok   200    41ms https://www.example.com/healthz
ok   200    87ms https://www.example.com/old-path
FAIL 404    52ms https://api.example.com/v1/missing
FAIL   -  2000ms https://reports.internal.example.com/slow (request to `https://reports.internal.example.com/slow` timed out)
FAIL   -     3ms https://old.example.com/ (cannot request `https://old.example.com/`: no such host)
2 ok, 3 failed
$ echo $?
1
```

`/old-path`는 301로 다른 주소를 가리키는데, `http.get`이 리다이렉트를 따라가서(10번까지) 마지막 응답의 상태 `200`을 보여 준다. 걸린 시간은 리다이렉트를 모두 포함한다.

## 줄별 설명 {#how}

- `check`: URL 하나를 요청해 결과를 맵 하나로 정리한다. 성공과 실패가 같은 키(`url`, `ok`, `status`, `ms`, `note`)를 가지므로 출력하는 쪽은 경우를 나누지 않는다.
- `time.now() - t0`: 시각끼리 빼면 기간(duration)이고, `// 1ms`로 나누면 밀리초 정수가 된다.
- `http.get(url, timeout: limit)`: 서버가 대답하면 상태 코드가 404든 500이든 `Ok({status, headers, body})`다. 그래서 "성공"의 기준(여기서는 400 미만)은 스크립트가 정한다. 대답을 못 받으면 `Err`이고 `kind`는 `'network`(이름 조회, 연결 거절, 인증서 문제)나 `'timeout`, `message`에 이유가 있다. `timeout`은 연결과 리다이렉트까지 포함한 요청 전체의 한도다.
- `parse_dur(…) ?? fail(…)`: `--timeout 2s`, `--timeout 500ms`처럼 기간으로 받는다. 기간 문자열이 아니면 알려 주고 멈춘다.
- `conc.pmap(urls, …, limit: 16)`: 16개씩 동시에 요청한다. 결과는 끝난 순서가 아니라 목록 순서다. 그래서 위 출력에서 느린 주소가 끝나기를 기다린 뒤에도 줄 순서가 파일과 같다.
- `mark = r.ok then "ok  " else "FAIL"`: 한 줄짜리 조건식이다. `"ok  "`의 공백 두 개는 `FAIL`과 너비를 맞추려는 것이다.
- `eprint(…)`와 `exit(1)`: 요약은 표준 에러로, 실패가 있으면 종료 코드 1. `fung checkurls.fung urls.txt || 알림-명령`처럼 쓸 수 있다.

:::box mistake `http.get(…)!`로 상태 확인하기

`curl -f`처럼 `!`가 404에서 멈춰 줄 것이라 생각하기 쉽다. 그러나 `!`는 `Err`일 때만 멈추고, 404 응답은 `Ok`다. `http.get(url)!.body`는 404 페이지의 본문을 아무 일 없다는 듯 돌려준다. 상태 코드가 중요하면 `res.status`를 직접 본다.

```fung frag
res = http.get(url)!
if res.status >= 400: fail("${url}: HTTP ${res.status}") ;;
```

:::

## 응용 {#vary}

- **본문까지 확인**: 헬스 체크가 `{"status":"ok"}`를 돌려주는지 보려면 `Ok(res)` 가지에서 `json.parse(res.body)`를 하고 필드를 확인한다. [JSON API에서 필드 뽑기](json-fields.html).
- **느린 응답도 실패로**: `ok: res.status < 400 and ms < 1000`처럼 기준에 시간을 더한다.
- **재시도**: 한 번 실패로 알리지 않으려면 `conc.retry(() => …, times: 3, delay: 1s)`로 감싼다. 이때 `check`가 실패를 `Err`로 돌려주게 바꿔야 `retry`가 다시 시도한다. [실패·무응답 서버만 알리기](unreachable-servers.html).
- **결과 저장**: 매번 결과를 JSON 줄로 `fs.append`해 두면 시간에 따른 응답 시간을 볼 수 있다. [CSV와 JSON으로 저장하기](save-csv-json.html).
- 저장소의 `examples/checkurls.fung`·`examples/urlcheck.fung`는 JSON 응답의 최상위 키까지 보여 주는 판이고, `test` 블록 안에서 서버를 띄워 스스로를 시험한다.

배우기: [13장 JSON과 HTTP](../learn/13-json-and-http.html), [12장 동시에 여러 일 하기](../learn/12-concurrency.html). 레퍼런스: [`http.get`](../reference/modules/http.html#fn-http-get), [`conc.pmap`](../reference/modules/conc.html#fn-conc-pmap), [`time.now`](../reference/modules/time.html#fn-time-now), [`parse_dur`](../reference/prelude/convert.html#fn-parse_dur), [`exit`](../reference/prelude/program.html#fn-exit).
