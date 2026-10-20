---
lede: <code>import json</code>. JSON 텍스트를 fung 값으로 읽고(<code>parse</code>), fung 값을 JSON 텍스트로 쓴다(<code>stringify</code>). 모든 글자를 훑는 일이라 네이티브 코드로 만들었다.
description: fung 모듈 json — JSON 텍스트와 fung 값 사이의 변환(parse, stringify).
---
# json — JSON

## JSON이란 {#concepts}

**JSON**(JavaScript Object Notation)은 데이터를 글자로 적는 형식이다. 웹 API의 응답, 설정 파일, 로그 한 줄 등 프로그램끼리 데이터를 주고받는 곳 대부분이 JSON을 쓴다. 생김새는 fung의 맵·리스트 리터럴과 거의 같다.

```plain
{"name": "web-1", "port": 8080, "tags": ["prod", "eu"], "healthy": true, "owner": null}
```

JSON 텍스트는 그냥 문자열이다. 문자열인 채로는 `"port"` 값을 꺼낼 수 없으므로, 먼저 fung 값(맵, 리스트, 숫자 …)으로 바꿔야 한다. 이것을 **파싱**(parsing, 읽기)이라 하고, 반대로 fung 값을 JSON 텍스트로 바꾸는 것을 **직렬화**(serialization, 쓰기)라 한다. `json.parse`와 `json.stringify`가 각각을 맡는다.

<table>
  <thead><tr><th>JSON</th><th>fung</th><th>비고</th></tr></thead>
  <tbody>
    <tr><td>객체 <code>{"a": 1}</code></td><td>맵 <code>{a: 1}</code></td><td>키 순서를 지킨다. 키는 문자열.</td></tr>
    <tr><td>배열 <code>[1, 2]</code></td><td>리스트</td><td>쓸 때는 튜플도 배열이 된다.</td></tr>
    <tr><td>문자열</td><td>문자열</td><td>쓸 때는 심볼도 문자열이 된다.</td></tr>
    <tr><td>숫자 <code>42</code></td><td><code>int</code></td><td>소수점·지수가 없으면 정수(64비트).</td></tr>
    <tr><td>숫자 <code>2.5</code> <code>1e3</code></td><td><code>float</code></td><td>소수점이나 지수가 있으면 실수.</td></tr>
    <tr><td><code>true</code> <code>false</code></td><td><code>true</code> <code>false</code></td><td></td></tr>
    <tr><td><code>null</code></td><td><code>nil</code></td><td></td></tr>
  </tbody>
</table>

외부에서 오는 JSON은 깨져 있을 수 있으므로 `json.parse`는 결과값(`Ok`/`Err`)을 돌려준다. 반대로 `json.stringify`가 실패하는 것은 JSON에 없는 값(함수, 기간 등)을 넘긴 프로그램의 잘못이라 런타임 에러로 멈춘다([에러 규칙](../language.html#results)).

:::fn json.parse(s)

JSON 텍스트를 fung 값으로 읽는다.

#### 설명

`s` 전체가 JSON 값 하나여야 한다. 앞뒤 공백은 괜찮지만 값 뒤에 다른 글자가 남으면 `Err`다. 변환 규칙은 [위의 표](#concepts)와 같다. 표준(RFC 8259)을 엄격하게 따른다: 끝의 쉼표(`[1, 2,]`), 주석, 작은따옴표 문자열은 받지 않는다. 같은 키가 두 번 나오거나, 짝 없는 서로게이트 이스케이프(`\ud800`)가 있거나, 64비트 정수나 실수의 범위를 넘는 숫자가 있거나, 중첩이 10,000단계를 넘어도 `Err`다. 공백은 스페이스·탭·LF·CR만 공백이다.

파싱한 맵은 보통의 fung 맵이라 `data.name`이나 `data["name"]`으로 읽는다. 키가 없을 수 있으면 [`get`](../prelude/maps.html#fn-get)으로 기본값을 준다.

#### 매개변수

- `s`: JSON 텍스트(문자열).

#### 돌려주는 값

`Ok(값)`.

#### 에러

- `Err({kind: 'parse, line, col, message})`: 형식이 틀렸다. `line`·`col`은 1부터 세는 문제 글자의 위치(열은 글자 단위)이고, `message`는 ``invalid JSON at line 1, column 9: expected a string for an object key``처럼 위치와 이유를 담는다. 이유의 예: `unexpected end of input`, `unexpected text after the value`, ``duplicate key "a"``, `number out of range`, `nesting too deep (limit 10000)`.
- `s`가 문자열이 아니면 런타임 에러 E0403.

#### 예제

**API 응답에서 필드 꺼내기**

```file user.json
{"user": {"name": "kim", "roles": ["admin", "ops"]}, "active": true}
```

```fung run
import fs
import json

data = json.parse(fs.read("user.json")!)!
print(data.user.name, data.user.roles[0], data.active)
print(get(data.user, "email", "(none)"))
```

**숫자의 타입**

```fung run
import json

print(json.parse(`[1, 2.5, 1e3, -0, 12.50]`)!)
```

**잘못된 입력**

```fung run
import json

print(json.parse(`{"a": 1,}`))
print(json.parse(`{"a": 1, "a": 2}`))
match json.parse(`[1, 2`):
  Ok(v)  => print(v)
  Err(e) => print("bad json at ${e.line}:${e.col}")
;;
```

**한 줄에 하나씩(JSON Lines)**

```file events.jsonl
{"level": "info", "msg": "start"}
{"level": "error", "msg": "disk full"}
{"level": "error", "msg": "timeout"}
```

```fung run
import fs
import json

events = fs.read_lines("events.jsonl")! |> map(l => json.parse(l)!)
print(events |> filter(e => e.level == "error") |> map(e => e.msg))
```

#### 같이 보기

[`json.stringify`](#fn-json-stringify), [`http.get`](http.html#fn-http-get), [JSON API에서 필드 뽑기](../../recipes/json-fields.html)

:::

:::fn json.stringify(v, indent = nil)

fung 값을 JSON 텍스트로 쓴다.

#### 설명

`v`를 JSON 문자열로 바꾼다. 맵·리스트·문자열·정수·실수·`true`·`false`·`nil`은 [위의 표](#concepts)대로 그대로 가고, 튜플은 배열, 심볼은 이름 문자열(`'ok` → `"ok"`), 시각은 ISO 8601 문자열(`"2026-10-04T09:05:07Z"`)이 된다. 맵은 키 순서대로 쓴다. 실수는 늘 소수점을 붙인다(`1.0`). JSON에 대응하는 것이 없는 값(기간, 함수, 태그 `Ok(1)`, 범위 `range(…)`, 채널, atom 등)은 런타임 에러 E0513이다. 기간은 `d / 1s`처럼 숫자로, 범위는 `map(r, x => x)`처럼 리스트로 바꿔서 넘긴다.

`indent`가 없으면(또는 `0`) 공백 없는 한 줄이다. 숫자를 주면 그 칸 수로 들여 쓴 여러 줄이 된다(사람이 읽을 파일에). 문자열은 꼭 필요한 것만 이스케이프한다: `"`, `\`, U+0020 미만의 제어 문자. 한글 같은 비ASCII 글자는 그대로 둔다.

#### 매개변수

- `v`: 바꿀 값.
- `indent`: `nil`(한 줄) 또는 들여쓰기 칸 수(0 이상의 정수).

#### 돌려주는 값

JSON 문자열.

#### 에러

- 런타임 에러 E0513: JSON으로 바꿀 수 없는 값이 들어 있다(``cannot convert dur to JSON``).
- 런타임 에러 E0403: `indent`가 음수다.
- 런타임 에러 E0806: 결과가 1억 글자를 넘는다.

#### 예제

**한 줄과 여러 줄**

```fung run
import json

report = {host: "web-1", ok: true, ms: [12, 15.5], note: nil}
print(json.stringify(report))
print(json.stringify(report, indent: 2))
```

**심볼, 튜플, 시각, 이스케이프**

```fung run
import json
import time

print(json.stringify({status: 'ok, pair: (1, "a"), at: time.from_unix(0)}))
print(json.stringify("탭\t따옴표\" 줄\n"))
```

**바꿀 수 없는 값**

```fung run exit=1 error=E0513
import json

print(json.stringify({elapsed: 1500ms}))
```

기간은 숫자로 바꿔서 넘긴다: `{elapsed_ms: 1500ms / 1ms}`.

#### 같이 보기

[`json.parse`](#fn-json-parse), [`http.post`](http.html#fn-http-post), [`fs.write`](fs.html#fn-fs-write), [CSV/JSON으로 저장](../../recipes/save-csv-json.html)

:::

배우는 순서로는 [13장 JSON과 HTTP](../../learn/13-json-and-http.html)를 본다.
