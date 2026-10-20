---
description: 표준 입력을 받아 파이프 중간에서 쓰기.
lede: 파이프의 한 칸이 되는 스크립트. 표준 입력을 한 줄씩 읽어 거르고, 결과는 표준 출력으로, 요약은 표준 에러로 보낸다. <code>tail -f</code>에도 붙는다.
---
# 표준 입력을 받아 파이프 중간에서 쓰기

쉘의 힘은 `cat log | grep | sort | uniq -c` 같은 파이프에서 나온다. 한 칸만 조금 복잡해지면(형식을 읽고, 숫자를 비교하고, 셀 것이 생기면) `awk` 한 줄이 길어진다. 여기서는 웹 서버 접근 로그를 표준 입력으로 받아 실패한 요청(5xx)과 느린 요청만 탭으로 나눈 줄로 내보내는 스크립트를 만든다. 앞뒤에 다른 명령을 그대로 이을 수 있다.

```shell
tail -f /var/log/nginx/access.log | fung slowreq.fung --slow 500ms | cut -f2,5
```

## 쉘로는 {#shell}

```shell
awk '$9 >= 500 || $NF > 1 {print $4, $9, $NF, $6, $7}' access.log
```

짧지만 필드 번호가 로그 형식에 묶여 있다. 경로나 사용자 에이전트에 공백이 하나 끼면 번호가 밀린다. `$NF > 1`은 마지막 필드가 숫자가 아니면(깨진 줄) 문자열 비교로 바뀌어 엉뚱한 줄이 통과한다. 몇 줄을 버렸는지 알 길이 없고, `tail -f` 뒤에 붙이면 `awk`가 출력을 모아 두었다 내보내서(버퍼링) `fflush()`를 따로 넣어야 한다.

## fung으로 {#code}

```file access.log
10.0.0.5 - - [20/Oct/2026:09:00:01 +0000] "GET /api/users HTTP/1.1" 200 512 0.031
10.0.0.7 - - [20/Oct/2026:09:00:02 +0000] "POST /api/orders HTTP/1.1" 502 0 3.004
10.0.0.5 - - [20/Oct/2026:09:00:02 +0000] "GET /healthz HTTP/1.1" 200 2 0.001
10.0.0.9 - - [20/Oct/2026:09:00:03 +0000] "GET /api/report?month=09 HTTP/1.1" 200 88120 1.870
garbage line from a crashed writer
10.0.0.7 - - [20/Oct/2026:09:00:05 +0000] "POST /api/orders HTTP/1.1" 504 0 30.000
10.0.0.5 - - [20/Oct/2026:09:00:06 +0000] "GET /api/users HTTP/1.1" 200 498 0.029
```

```fung run stdin=access.log
#!/usr/bin/env fung
# Reads an access log on standard input and passes on the requests that
# failed (5xx) or were slow, one tab-separated line each:
#   time  status  milliseconds  method  path
# usage: tail -f access.log | fung slowreq.fung [--slow 1s] | sort ...
import cli
import regex

# 10.0.0.7 - - [20/Oct/2026:09:00:02 +0000] "POST /api/orders HTTP/1.1" 502 0 3.004
entry = `^\S+ \S+ \S+ \[([^\]]+)\] "(\S+) (\S+) [^"]*" (\d{3}) \d+ ([\d.]+)$`

fn parse(line) = match regex.find(line, entry):
  nil                                      => nil
  {groups: [at, method, path, code, secs]} =>
    {at, method, path, status: int(code), took: parse_float(secs)! * 1s}
;;

# One line at a time until the input ends, so it works on `tail -f` too.
# `seen` counts lines; returns (lines seen, lines kept, lines not understood).
fn filter_lines(slow, seen, kept, bad):
  line = read_line()!
  if line == nil: return (seen, kept, bad) ;;
  r = parse(line)
  if r == nil: return filter_lines(slow, seen + 1, kept, bad + 1) ;;
  if r.status < 500 and r.took < slow: return filter_lines(slow, seen + 1, kept, bad) ;;
  print(join([r.at, str(r.status), str(r.took // 1ms), r.method, r.path], "\t"))
  filter_lines(slow, seen + 1, kept + 1, bad)
;;

opts = cli.parse(args)
slow = parse_dur(get(opts.flags, "slow", "1s")) ?? fail("--slow takes a duration like 500ms")
(seen, kept, bad) = filter_lines(slow, 0, 0, 0)
eprint("kept ${kept} of ${seen} lines, ${bad} not understood")
```

마지막 줄은 표준 에러로 간다. 위 출력에는 둘이 함께 보이지만, 파이프로 이으면 다음 명령에는 앞의 세 줄만 간다.

## 줄별 설명 {#how}

- `entry`: 접근 로그 한 줄을 읽는 정규식이다. 괄호로 묶은 다섯 조각(시각, 메서드, 경로, 상태, 걸린 초)이 `groups`에 순서대로 들어온다. 필드 번호를 세지 않고 줄의 모양을 적으므로, 따옴표 안의 요청 줄에 무엇이 오든 경로를 정확히 고른다.
- `parse`: 맞지 않는 줄은 `nil`. 맞으면 `groups`를 이름 다섯 개로 풀어 맵을 만든다. 상태는 정규식이 숫자 세 개임을 보장하므로 `int`로 바꾸고, 걸린 시간은 `parse_float(secs)! * 1s`로 **기간**(duration) 값으로 만든다. 그러면 `--slow 500ms` 같은 옵션과 단위 걱정 없이 비교할 수 있다.
- `filter_lines`: `read_line()`은 한 줄을 읽어 `Ok(줄)`을, 입력이 끝나면 `Ok(nil)`을 준다. 한 줄을 처리하고 자기 자신을 다시 부르는데, 함수의 마지막에서 부르는 호출(꼬리 호출)은 스택을 쌓지 않으므로 몇백만 줄이어도 된다. `return filter_lines(…)`도 꼬리 호출이다. 세는 값(`seen`·`kept`·`bad`)은 변수를 바꾸는 대신 다음 호출의 인자로 넘긴다.
- `print(join(…, "\t"))`: 다음 명령이 `cut -f`나 `sort -t$'\t'`로 다루기 쉽게 탭으로 나눈다. 걸린 시간은 `r.took // 1ms`로 밀리초 정수로 바꿔서 `sort -n`이 숫자로 정렬할 수 있게 한다. `print`는 줄마다 바로 내보내므로 `tail -f` 뒤에서도 줄이 늦게 나오지 않는다.
- `parse_dur(…) ?? fail(…)`: `"500ms"`, `"2s"`, `"1m30s"` 같은 문자열을 기간으로 바꾼다. 잘못된 값이면 알려 주고 멈춘다.
- `eprint`: 요약은 표준 에러로. 파이프를 따라 흐르는 데이터와 사람이 읽을 메시지를 나눈다.

뒤의 명령이 먼저 끝나도(`| head -3`) fung은 에러 메시지 없이 조용히 끝난다. 쉘의 다른 명령처럼 SIGPIPE로 끝난 것으로 보고된다(종료 코드 141).

:::box mistake `while read line` 대신

쉘의 `while read -r line; do …; done`에 해당하는 `while`은 fung에 없다. 입력이 작고 끝이 있으면 `read_lines()!`로 전부 받아 `for`나 `filter`로 다루는 것이 가장 쉽다. 다만 `read_lines`는 입력이 **끝날 때까지** 기다리므로 `tail -f` 뒤에서는 아무것도 내보내지 않는다. 끝없는 입력은 위처럼 `read_line`과 꼬리 재귀로 한 줄씩 다룬다. 루프 안에서 `kept = kept + 1`처럼 바깥 변수를 바꾸려 하면 E0205이므로, 세는 값은 인자로 넘긴다.

```file hosts.txt
web1
# web2 is retired
web3

db1
```

```fung run stdin=hosts.txt
hosts = read_lines()! |> map(trim) |> filter(h => h != "" and not starts_with(h, "#"))
print(len(hosts), "hosts:", join(hosts, ", "))
```

:::

## 응용 {#vary}

- **파일도 받기**: `src = first(args)`가 있으면 `fs.open(src)!`로 연 핸들을 `read_line(f)`에 넘긴다. 그러면 `fung slowreq.fung access.log`와 `cat access.log | fung slowreq.fung` 둘 다 된다. 핸들은 [`fs.with_open`](../reference/modules/fs.html#fn-fs-with_open)으로 열면 끝날 때 저절로 닫힌다.
- **경로별 집계**: 거른 줄을 출력하지 않고 리스트로 모아 `group_by(r => r.path)`로 묶으면 "어느 API가 제일 느린가"가 된다. 끝이 있는 입력에만 맞는다. [여러 로그에서 패턴 세고 요약하기](count-log-patterns.html)와 같은 방법이다.
- **JSON 줄 로그**: 줄마다 JSON인 로그(`{"level":"error",…}`)는 정규식 대신 `json.parse(line)`으로 읽는다. [JSON API에서 필드 뽑기](json-fields.html).
- **명령의 출력을 바로**: 파이프 대신 스크립트 안에서 `lines(exec(["journalctl", "-u", "app", "--since", "1h ago"])!.out)`으로 받아도 된다.

배우기: [6장 문자열](../learn/06-strings.html), [5장 함수](../learn/05-functions.html)(꼬리 호출), [11장 파일과 외부 명령](../learn/11-files-and-commands.html). 레퍼런스: [`read_line`](../reference/prelude/io.html#fn-read_line), [`read_lines`](../reference/prelude/io.html#fn-read_lines), [`eprint`](../reference/prelude/io.html#fn-eprint), [`parse_dur`](../reference/prelude/convert.html#fn-parse_dur), [`regex.find`](../reference/modules/regex.html#fn-regex-find), [꼬리 호출](../reference/language.html#functions).
