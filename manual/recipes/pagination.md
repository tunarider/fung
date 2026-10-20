---
description: 페이지 넘기며 모두 받기.
lede: 한 번에 일부만 돌려주는 API를 끝 페이지까지 따라가며 모두 받는다. 요청이 너무 잦다는 429 응답에는 서버가 말한 만큼 기다렸다가 같은 페이지를 다시 요청한다.
---
# 페이지 넘기며 모두 받기

인벤토리, 사용자 목록, 감사 로그 같은 API는 결과가 많으면 한 번에 50개, 100개씩 나눠서(페이지네이션, pagination) 준다. 응답에 "다음 페이지 주소"가 들어 있고, 그 주소가 없으면 끝이다. 여기서는 `{"items": [...], "next": "https://…?page=2"}` 꼴의 API를 끝까지 따라가 모든 항목을 한 줄에 JSON 하나씩(JSON Lines) 찍는다. 출력을 파일로 돌리면 다른 도구가 한 줄씩 읽기 좋다.

## 쉘로는 {#shell}

```shell
url="https://inventory.example.com/api/hosts"
while [ "$url" != "null" ]; do
  page=$(curl -sf "$url") || exit 1
  echo "$page" | jq -c '.items[]'
  url=$(echo "$page" | jq -r '.next')
done
```

쉘에서는 `while`과 바꿔 가는 변수 `url`로 쓴다. 짧지만 `curl -f`를 빠뜨리면 에러 응답의 본문이 `jq`로 가서 무한 반복이 되기도 하고, 429(요청 과다)를 받으면 그냥 실패한다. 응답을 `jq`로 두 번 읽고, 페이지 수 상한이 없어서 API가 같은 `next`를 계속 돌려주면 끝나지 않는다.

## fung으로 {#code}

```fung check
#!/usr/bin/env fung
# Follows a paged JSON API to the end and prints every item as one JSON
# line. Each page is {"items": [...], "next": URL or null}.
# usage: fung allpages.fung https://inventory.example.com/api/hosts > hosts.jsonl
import http
import json

max_pages = 1000
token = env("API_TOKEN")
headers = token == nil then {accept: "application/json"}
  else {accept: "application/json", authorization: "Bearer ${token}"}

# One page as a map. A 429 (too many requests) waits as the server asks
# and tries the same page again.
fn page(url, tries = 1):
  res = http.get(url, headers: headers, timeout: 30s)?
  match res.status:
    200                => json.parse(res.body)
    429 when tries < 5 =>
      wait = parse_int(get(res.headers, "retry-after", "1")) ?? 1
      eprint("rate limited, waiting ${wait}s")
      sleep(wait * 1s)
      page(url, tries + 1)
    code               => Err({kind: 'http, message: "${url}: HTTP ${code}"})
  ;;
;;

# All the items from url on: pages are fetched one after another, each
# naming the next. Returns Ok(items) or the first Err.
fn all_items(url, acc, n):
  if n > max_pages: return Err({kind: 'limit, message: "more than ${max_pages} pages"}) ;;
  p = page(url)?
  items = acc + get(p, "items", [])
  match get(p, "next"):
    nil  => Ok(items)
    next => all_items(next, items, n + 1)
  ;;
;;

url = first(args) ?? fail("usage: allpages.fung URL")
items = all_items(url, [], 1)!
each(items, i => print(json.stringify(i)))
eprint("${len(items)} items")
```

```expect
$ API_TOKEN=… fung allpages.fung https://inventory.example.com/api/hosts
rate limited, waiting 1s
{"id":11,"name":"host-1a"}
{"id":12,"name":"host-1b"}
{"id":21,"name":"host-2a"}
{"id":22,"name":"host-2b"}
{"id":31,"name":"host-3a"}
{"id":32,"name":"host-3b"}
6 items
```

세 페이지를 따라갔고, 두 번째 요청에서 429를 받아 1초 기다린 뒤 같은 페이지를 다시 받았다. `rate limited…`와 `6 items`는 표준 에러로 가므로 `> hosts.jsonl`로 돌리면 파일에는 항목 줄만 남는다.

## 줄별 설명 {#how}

- `token`·`headers`: 토큰은 스크립트에 적지 않고 환경 변수에서 읽는다. 없으면(`nil`) `authorization` 헤더 없이 보낸다. 최상위에서 한 번만 대입한 이름이라 아래 함수들이 그대로 읽는다.
- `page`: 페이지 하나를 받아 JSON을 맵으로 바꾼다. `http.get(…)?`의 `?`는 네트워크 실패면 그 `Err`를 곧바로 돌려보낸다. 상태 코드는 `match`로 가른다. `200`이면 `json.parse`의 결과(`Ok(맵)`이나 `Err`)가 곧 반환값이다.
- `429 when tries < 5`: 리터럴 패턴 `429`에 guard를 붙여 다섯 번까지만 기다린다. `Retry-After` 헤더(응답 헤더 키는 늘 소문자다)의 초를 읽고, 숫자가 아니면 1초로 친다. `sleep(wait * 1s)`처럼 숫자에 `1s`를 곱하면 기간이 된다. 기다린 뒤 같은 URL로 자기 자신을 다시 부른다.
- `code =>`: 나머지 상태 코드는 이름 `code`에 묶어 에러 메시지에 넣는다. 패턴의 이름은 늘 새로 묶는 변수다.
- `all_items`: 반복을 `while` 대신 꼬리 재귀로 쓴다. 지금까지 모은 항목 `acc`와 페이지 번호 `n`을 인자로 넘기고, `next`가 `nil`(JSON의 `null`이거나 키가 없음)이면 끝이다. 마지막 줄의 호출이 꼬리 호출이라 페이지가 수천 개여도 스택이 쌓이지 않는다.
- `max_pages`: API가 실수로 같은 `next`를 계속 돌려줘도 1000페이지에서 멈춘다. 끝나지 않는 반복을 막는 안전장치다.
- `acc + get(p, "items", [])`: 리스트 이어 붙이기는 앞 리스트와 구조를 공유해서, 이렇게 뒤에 쌓아 가도 전체 시간은 항목 수에 비례한다.
- `all_items(url, [], 1)!`: 어느 페이지에서든 실패하면 첫 `Err`가 여기까지 올라오고, `!`가 메시지를 찍고 종료 코드 1로 멈춘다. 반쯤 받은 결과를 성공처럼 내보내지 않는다.

:::box mistake `while`과 바뀌는 변수

쉘의 `while [ "$url" != "null" ]; do …; url=…; done`을 옮기려 하면 막힌다. fung에는 `while`이 없고, 블록 안에서 바깥의 `url`이나 `all`에 다시 대입하면 E0205다. "바뀌는 변수"는 "다음 호출의 인자"로 바꿔 쓴다. 위의 `all_items(next, items, n + 1)`가 그 모양이다. 정해진 횟수나 조건까지 되풀이하는 간단한 경우는 [`until`](../reference/prelude/loops.html#fn-until)도 있다.

```fung run
fn pages(n, acc) = n > 3 then acc else pages(n + 1, acc + ["page-${n}"])
print(pages(1, []))
print(until(1, x => x * 2, x => x > 100))
```

:::

## 응용 {#vary}

- **페이지 번호 방식**: `?page=1`, `?page=2`…로 넘기고 빈 페이지에서 끝나는 API라면 `next` 대신 번호를 인자로 넘긴다: `is_empty(p.items) then Ok(acc) else all_items(base, acc + p.items, n + 1)`, 주소는 `"${base}?page=${n}"`.
- **Link 헤더 방식**: GitHub처럼 다음 주소를 `link: <https://…?page=2>; rel="next"` 헤더로 주면 원시 문자열 정규식 `<([^>]+)>;\s*rel="next"`을 `regex.find(get(res.headers, "link", ""), pat)`에 넘겨 얻은 첫 조각이 다음 주소다. 이때 `page`는 본문과 함께 헤더도 돌려주게 바꾼다.
- **파일로 바로**: `each` 대신 `fs.write("hosts.json", json.stringify(items, indent: 2))!`. [CSV와 JSON으로 저장하기](save-csv-json.html).
- **필드만 골라**: 항목마다 필요한 필드만 남기려면 [JSON API에서 필드 뽑기](json-fields.html)의 `dig`를 쓴다.
- **동시에 받기**: 전체 페이지 수를 첫 응답에서 알 수 있으면 나머지 페이지 주소를 만들어 `conc.pmap(urls, page, limit: 4)`로 동시에 받는다. 서버에 부담을 주지 않도록 `limit`을 작게 둔다.

배우기: [13장 JSON과 HTTP](../learn/13-json-and-http.html), [5장 함수](../learn/05-functions.html)(꼬리 호출), [9장 에러](../learn/09-errors.html). 레퍼런스: [`http.get`](../reference/modules/http.html#fn-http-get), [`json.parse`](../reference/modules/json.html#fn-json-parse), [`json.stringify`](../reference/modules/json.html#fn-json-stringify), [`sleep`](../reference/prelude/concurrency.html#fn-sleep), [`until`](../reference/prelude/loops.html#fn-until), [결과값과 에러](../reference/language.html#results).
