---
description: CSV나 JSON으로 저장하기.
lede: 모은 결과를 스프레드시트가 여는 CSV와 다른 도구가 읽는 JSON으로 남긴다. 쉼표나 따옴표가 든 값도 깨지지 않게 CSV 규칙대로 감싼다.
---
# CSV나 JSON으로 저장하기

스크립트로 모은 결과는 화면에서 보고 끝나지 않는 일이 많다. 팀에 공유하려면 스프레드시트로 여는 CSV가, 다른 스크립트나 대시보드에 넘기려면 JSON이 편하다. 여기서는 [페이지 넘기며 모두 받기](pagination.html)가 만든 것 같은 JSON Lines 파일(한 줄에 JSON 객체 하나)을 읽어 `report.csv`와 `report.json` 두 파일로 저장한다. 값에 쉼표나 따옴표가 있어도, 어떤 줄에 필드가 빠져 있어도 열이 밀리지 않는다.

## 쉘로는 {#shell}

```shell
jq -r '[.id, .name, .role, .disk, .note] | @csv' hosts.jsonl > report.csv
jq -s '.' hosts.jsonl > report.json
```

`jq`가 있으면 이렇게 끝난다. 다만 `@csv`는 문자열을 모두 따옴표로 감싸고 머리글 줄을 따로 넣어 줘야 한다. `jq`가 없을 때 흔히 쓰는 `echo "$id,$name,$role"`은 값에 쉼표가 하나만 들어 있어도 열이 하나 늘어나 표 전체가 어긋난다. 입력 중 한 줄이 잘못된 JSON이면 그 앞까지만 쓰인 파일이 남는다.

## fung으로 {#code}

```file hosts.jsonl
{"id": 11, "name": "web1", "role": "web", "disk": 44, "note": null}
{"id": 12, "name": "web2", "role": "web", "disk": 91, "note": "replace disk, then \"reboot\""}
{"id": 21, "name": "db1", "role": "db, primary", "disk": 63}
```

```fung run args="hosts.jsonl report"
#!/usr/bin/env fung
# Turns JSON Lines (one object per line) into a CSV file and one JSON
# file. Missing fields and null become empty cells.
# usage: fung save.fung hosts.jsonl report   (writes report.csv, report.json)
import fs
import json

columns = ["id", "name", "role", "disk", "note"]

# One CSV cell: quoted when it holds a comma, a quote or a line break,
# with quotes doubled inside.
fn cell(v):
  s = v == nil then "" else type_of(v) == 'str then v else str(v)
  quote = any([",", "\"", "\n", "\r"], c => c in s)
  quote then "\"" + replace(s, "\"", "\"\"") + "\"" else s
;;

fn csv(rows) = ([columns] + map(rows, r => map(columns, c => get(r, c))))
  |> map(cells => cells |> map(cell) |> join(","))
  |> map(line => line + "\n")
  |> join

(src, out) = match args:
  [a, b] => (a, b)
  _      => fail("usage: save.fung input.jsonl output-name")
;;
rows = (fs.read_lines(src)! |> filter(l => trim(l) != "") |> try_map(json.parse))!

fs.write("${out}.csv", csv(rows))!
fs.write("${out}.json", json.stringify(rows, indent: 2) + "\n")!
print("wrote ${out}.csv and ${out}.json: ${len(rows)} rows")

# show what was written
print(trim(fs.read("${out}.csv")!))
print(trim(fs.read("${out}.json")!))
```

맨 아래 두 줄은 무엇이 쓰였는지 보여 주려고 넣은 것이다. 실제 스크립트에서는 지운다. JSON 쪽은 입력에 있던 그대로 `db1`에 `note`가 없고, CSV 쪽은 빈 칸이다.

## 줄별 설명 {#how}

- `columns`: CSV의 열과 순서를 정한다. JSON 객체는 줄마다 키가 다를 수 있으므로, 열은 데이터가 아니라 스크립트가 정한다.
- `cell`: 값 하나를 CSV 칸 하나로 만든다. `nil`(JSON의 `null`이나 빠진 필드)은 빈 칸, 문자열은 그대로, 숫자 같은 나머지는 `str`로 바꾼다. 쉼표·따옴표·줄바꿈이 있으면 칸 전체를 따옴표로 감싸고 안의 따옴표는 두 번 쓴다(`"` → `""`). 스프레드시트와 대부분의 CSV 도구가 따르는 규칙(RFC 4180)이다.
- `csv`: 머리글 줄과 데이터 줄들을 한 리스트로 만든 뒤, 줄마다 칸을 `,`로 잇고, 줄 끝에 `\n`을 붙이고, 전부를 하나의 문자열로 잇는다(`join`의 구분자를 생략하면 빈 문자열이다). 문자열을 `+`로 계속 붙여 가지 않고 리스트를 한 번에 `join`하는 것이 빠르다.
- `(src, out) = match args: …`: 인자가 정확히 둘이면 튜플로 받고, 아니면 사용법을 보인다. `[src, out] = args`로 바로 풀면 인자 수가 다를 때 E0602로 멈춘다.
- `try_map(json.parse)`: 줄마다 `json.parse`를 부르되, 하나라도 `Err`면 거기서 멈추고 그 `Err`를 돌려준다. 모두 성공하면 `Ok(리스트)`. 그래서 잘못된 줄이 있으면 파일을 하나도 쓰기 전에 멈춘다.
- `json.stringify(rows, indent: 2)`: 들여쓰기 2칸으로 사람이 읽기 좋게 쓴다. `indent` 없이 쓰면 한 줄이 된다. `nil`은 `null`로 나간다.
- `fs.write(…)!`: 파일 전체를 덮어쓴다. 실패하면(디렉터리가 없음, 권한) `!`가 멈춘다.

:::box mistake 파이프 끝의 `!`

후위 연산자 `!`·`?`는 `|>`보다 먼저 묶인다. 그래서 `xs |> try_map(f)!`는 "파이프 전체에 `!`"가 아니라 `try_map(f)!`, 곧 인자 하나로 `try_map`을 부른 것에 `!`를 붙인 것이 된다. 뜻과 다르므로 실행 전에 E0229로 막는다. 파이프 전체의 결과에 붙이려면 괄호로 감싼다. 위 스크립트의 `rows = (… |> try_map(json.parse))!`가 그 모양이다.

```fung run exit=2 error=E0229
nums = ["1", "2"] |> try_map(parse_int)!
print(nums)
```

:::

## 응용 {#vary}

- **결과를 쌓아 가기**: 실행할 때마다 한 줄씩 남기려면 `fs.append("checks.jsonl", json.stringify({at: str(time.now()), ok}) + "\n")!`. 파일이 없으면 만든다. 나중에 이 파일을 다시 이 스크립트에 넣으면 CSV가 된다.
- **표준 출력으로**: 파일 대신 `print(csv(rows))`로 내보내면 `fung save.fung … > out.csv`처럼 쓸 수 있다.
- **CSV 읽기**: 따옴표가 없는 단순한 CSV는 `lines`와 `split(line, ",")`로 읽힌다. 따옴표로 감싼 칸이 있으면 `split`으로는 안 되고, 칸 단위 정규식 `"(?:[^"]|"")*"|[^,]*`로 `regex.find_all`한 뒤, 따옴표로 감싼 칸은 앞뒤 따옴표를 떼고 `""`를 `"`로 되돌린다.
- **엑셀에서 한글**: 엑셀은 BOM이 없는 UTF-8 CSV를 다른 인코딩으로 열기도 한다. 그럴 때는 파일 앞에 `"\u{FEFF}"`를 붙여 쓴다.
- **잘못된 줄 위치 알리기**: `json.parse`의 에러에는 JSON 문자열 안의 줄·칸이 있다. 파일의 몇째 줄인지 알리려면 `zip(1..=len(ls), ls)`로 번호를 붙여 에러 메시지에 넣는다.

배우기: [13장 JSON과 HTTP](../learn/13-json-and-http.html), [11장 파일과 외부 명령](../learn/11-files-and-commands.html), [9장 에러](../learn/09-errors.html). 레퍼런스: [`json.stringify`](../reference/modules/json.html#fn-json-stringify), [`json.parse`](../reference/modules/json.html#fn-json-parse), [`try_map`](../reference/prelude/collections.html#fn-try_map), [`fs.write`](../reference/modules/fs.html#fn-fs-write), [`fs.append`](../reference/modules/fs.html#fn-fs-append), [`join`](../reference/prelude/strings.html#fn-join), [우선순위](../reference/language.html#precedence).
