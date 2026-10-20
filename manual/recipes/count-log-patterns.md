---
description: 여러 로그에서 패턴 세고 요약하기.
lede: 로그 파일 여러 개를 읽어 파일별로 레벨(INFO·WARN·ERROR)을 세고, 숫자만 다른 에러는 하나로 묶어 많이 나온 순서로 보여 준다.
---
# 여러 로그에서 패턴 세고 요약하기

장애가 나면 서버마다 로그를 열어 "에러가 몇 번 났나, 무슨 에러가 제일 많나"부터 본다. 여기서는 `*.log` 파일들을 모두 읽어 파일별 레벨 개수를 표로 찍고, 전체에서 가장 많이 나온 에러를 센다. `timeout after 3000 ms`와 `timeout after 5000 ms`처럼 숫자만 다른 줄은 같은 문제로 묶는다.

## 쉘로는 {#shell}

```shell
grep -c ERROR *.log
awk '$3=="ERROR" {$1=$2=$3=""; print}' *.log | sed -E 's/\b[0-9]+\b/N/g' | sort | uniq -c | sort -rn | head -3
```

첫 줄은 `ERROR`라는 글자가 들어간 줄을 세므로, 메시지 안에 `ERROR`가 있는 INFO 줄도 센다. 둘째 줄은 도구 다섯 개가 이어진 파이프라인이다. 필드 번호(`$3`)가 로그 형식에 묶여 있고, `\b`는 GNU sed에서만 되고 macOS의 sed에서는 되지 않는다. 레벨별 표를 한 번에 만들려면 `awk` 프로그램을 따로 써야 한다.

## fung으로 {#code}

```file web1.log
2026-10-20 09:00:01 INFO started on :8080
2026-10-20 09:01:13 ERROR upstream timeout after 3000 ms
2026-10-20 09:01:40 WARN slow query: 1840 ms
2026-10-20 09:02:05 ERROR upstream timeout after 3000 ms
2026-10-20 09:03:51 ERROR connection refused: db1:5432
2026-10-20 09:04:00 INFO healthcheck ok
```

```file web2.log
2026-10-20 09:00:02 INFO started on :8080
2026-10-20 09:01:15 ERROR upstream timeout after 5000 ms
2026-10-20 09:02:44 INFO healthcheck ok
--- rotated ---
2026-10-20 09:03:52 ERROR connection refused: db1:5432
```

```fung run
#!/usr/bin/env fung
# Counts the levels in many log files and the most frequent errors.
# Lines look like "2026-10-20 09:01:13 ERROR upstream timeout after 3000 ms".
# usage: fung logcount.fung [PATTERN] [--top N]      (PATTERN defaults to "*.log")
import fs
import cli
import regex

levels = ["INFO", "WARN", "ERROR"]

# One line → {level, message}, or nil when it is not a log line.
fn parse(line) = match split(line, " "):
  [_, _, level, ...words] when level in levels => {level, message: join(words, " ")}
  _                                            => nil
;;

# Numbers differ from line to line; "after 3000 ms" and "after 5000 ms"
# are the same problem.
fn shape(message) = regex.replace(message, `\b\d+\b`, "N")

opts = cli.parse(args)
pattern = first(opts.rest) ?? "*.log"
top = parse_int(get(opts.flags, "top", "3")) ?? fail("--top must be a number")
files = fs.glob(pattern)!
if is_empty(files): fail("no files match ${pattern}") ;;

# file → its parsed entries
logs = files |> map(f => (f, fs.read_lines(f)! |> map(parse) |> filter(e => e != nil)))

print(pad_right("file", 10), levels |> map(l => pad_left(l, 6)) |> join(" "))
for (file, entries) in logs:
  counts = group_by(entries, e => e.level)
  cells = levels |> map(l => pad_left(str(len(get(counts, l, []))), 6))
  print(pad_right(file, 10), join(cells, " "))
;;

errors = logs
  |> map(((_, entries)) => entries)
  |> flatten
  |> filter(e => e.level == "ERROR")
  |> group_by(e => shape(e.message))
  |> pairs
  |> map(((msg, es)) => (len(es), msg))
  |> sort_by(((n, _)) => -n)
  |> take(top)
print()
print("top errors:")
for (n, msg) in errors:
  print(pad_left(str(n), 4), msg)
;;
```

`--- rotated ---`처럼 형식에 맞지 않는 줄은 조용히 빠진다. `db1`의 `1`은 단어의 일부라 그대로 남고, 포트 `5432`만 `N`이 된다.

## 줄별 설명 {#how}

- `levels`: 최상위에서 한 번만 대입한 이름은 상수라서 최상위 함수 `parse` 안에서도 읽을 수 있다.
- `parse`: 줄을 공백으로 쪼갠 리스트를 `match`로 모양을 맞춰 본다. `[_, _, level, ...words]`는 "날짜, 시각, 레벨, 나머지 단어들"이고, `when level in levels`가 레벨이 셋 중 하나일 때만 맞게 한다. 맞지 않는 줄은 `nil`이 되고 나중에 `filter`로 버린다. 필드 번호 대신 이름(`level`, `words`)으로 읽으므로 형식이 바뀌어도 고칠 곳이 한 곳이다.
- `{level, message: …}`: `{level: level}`의 축약이다.
- `shape`: 정규식 `\b\d+\b`(단어 하나가 통째로 숫자인 것)를 `N`으로 바꾼다. 정규식은 백슬래시를 그대로 쓰도록 원시 문자열 `` `…` ``에 넣는다.
- `fs.glob(pattern)!`: 쉘이 아니라 fung이 `*.log`를 펼친다. 그래서 따옴표 없이 `fung logcount.fung` 하고 불러도, `"/var/log/app/*.log"`를 인자로 주어도 똑같이 돈다. 맞는 파일이 없으면 빈 리스트이므로 직접 확인한다.
- `logs`: 파일마다 `(파일 이름, 항목 리스트)` 튜플을 만든다. 파일을 한 번만 읽고 표와 에러 집계가 함께 쓴다.
- 표: `group_by`가 레벨을 키로 하는 맵 `{ERROR: [...], INFO: [...]}`를 만든다. 한 번도 안 나온 레벨은 키가 없으므로 `get(counts, l, [])`로 빈 리스트를 기본값으로 준다. `pad_left`·`pad_right`로 칸을 맞춘다.
- `errors`: 모든 파일의 항목을 `flatten`으로 한 리스트로 펴고, ERROR만 골라, 모양(`shape`)별로 묶는다. `pairs`가 맵을 `(키, 값)` 튜플 리스트로 바꾸고, 람다의 매개변수 자리 패턴 `((msg, es))`가 튜플을 풀어 받는다. 개수의 음수로 정렬하면 많은 것부터다.

:::box mistake 파일마다 더해 가는 합계

쉘이나 `awk`에서는 `total += n`처럼 반복하면서 합계를 키운다. fung에서는 `for` 안에서 바깥의 `total`에 다시 대입하면 E0205다. 합계는 다 모은 리스트에서 식 하나로 만든다.

```fung run
counts = [3, 2, 0]
total = sum(counts)
running = fold(counts, [], (acc, n) => acc + [(last(acc) ?? 0) + n])
print(total, running)
```

중간 합계가 필요하면 위처럼 `fold`로 리스트를 쌓는다.

:::

## 응용 {#vary}

- **다른 로그 형식**: nginx 접근 로그처럼 공백으로 깔끔히 나뉘지 않는 형식은 `split` 대신 `regex.find(line, pattern)`의 `groups`로 읽는다. [정규식으로 텍스트 쪼개기](regex-split.html)를 참고한다.
- **여러 서버의 로그**: 로그가 원격에 있으면 `exec(["ssh", host, "cat", "/var/log/app.log"])!.out`을 `lines`로 쪼개 같은 `parse`에 넘긴다. 여러 대를 동시에 하는 방법은 [여러 서버에 명령 보내고 모으기](ssh-collect.html).
- **표준 입력으로**: `journalctl -u app | fung logcount.fung -`처럼 받으려면 [표준 입력을 받아 파이프 중간에서 쓰기](stdin-filter.html)의 `read_lines()`를 쓴다.
- **결과 저장**: 표를 CSV로 남기려면 [CSV와 JSON으로 저장하기](save-csv-json.html).

배우기: [8장 패턴](../learn/08-patterns.html), [7장 컬렉션](../learn/07-collections.html), [6장 문자열](../learn/06-strings.html). 레퍼런스: [`fs.glob`](../reference/modules/fs.html#fn-fs-glob), [`fs.read_lines`](../reference/modules/fs.html#fn-fs-read_lines), [`group_by`](../reference/prelude/collections.html#fn-group_by), [`flatten`](../reference/prelude/collections.html#fn-flatten), [`pairs`](../reference/prelude/maps.html#fn-pairs), [`regex.replace`](../reference/modules/regex.html#fn-regex-replace), [match와 패턴](../reference/language.html#match).
