---
description: 텍스트 응답을 정규식으로 쪼개기.
lede: JSON이 아닌 텍스트 응답(Prometheus 지표, 명령 출력)을 정규식으로 조각내 값으로 바꾼다. 예제는 node_exporter의 지표에서 파일 시스템 사용률을 계산한다.
---
# 텍스트 응답을 정규식으로 쪼개기

모든 것이 JSON을 주지는 않는다. Prometheus 익스포터의 `/metrics`, nginx의 `stub_status`, `ss`나 `df` 같은 명령 출력은 사람이 읽기 좋게 만든 텍스트다. 이런 출력에서 값을 꺼낼 때는 줄의 **모양**을 정규식(regular expression)으로 적고, 맞은 조각을 이름으로 받는다. 여기서는 node_exporter 지표를 `curl`로 받아 파이프로 넘기면 파일 시스템마다 사용률을 계산하고, 기준을 넘은 것이 있으면 종료 코드 1로 끝나는 스크립트를 만든다.

## 쉘로는 {#shell}

```shell
curl -s http://web1:9100/metrics | grep '^node_filesystem_avail_bytes' \
  | sed -E 's/.*mountpoint="([^"]*)".* (.*)/\1 \2/'
```

사용 가능 바이트만 뽑는 데도 `grep`과 `sed`가 필요하다. 사용률을 내려면 `node_filesystem_size_bytes`도 따로 뽑아 마운트 지점으로 맞춰 붙여야 하는데(`join`이나 `awk` 배열), 값이 `1.6234029056e+10`처럼 지수 표기라 쉘의 정수 계산(`$((…))`)으로는 다룰 수 없다. 라벨의 순서가 바뀌거나 라벨 값에 따옴표가 들어가면 `sed` 식이 엉뚱한 곳을 잡는다.

## fung으로 {#code}

```file metrics.txt
# HELP node_filesystem_avail_bytes Filesystem space available to non-root users in bytes.
# TYPE node_filesystem_avail_bytes gauge
node_filesystem_avail_bytes{device="/dev/sda1",fstype="ext4",mountpoint="/"} 1.6234029056e+10
node_filesystem_avail_bytes{device="/dev/sdb1",fstype="xfs",mountpoint="/data"} 3.221225472e+09
node_filesystem_avail_bytes{device="tmpfs",fstype="tmpfs",mountpoint="/run"} 8.28461056e+08
node_filesystem_avail_bytes{device="/dev/sdc1",fstype="ext4",mountpoint="/mnt/broken"} NaN
# HELP node_filesystem_size_bytes Filesystem size in bytes.
# TYPE node_filesystem_size_bytes gauge
node_filesystem_size_bytes{device="/dev/sda1",fstype="ext4",mountpoint="/"} 4.2006183936e+10
node_filesystem_size_bytes{device="/dev/sdb1",fstype="xfs",mountpoint="/data"} 1.073741824e+11
node_filesystem_size_bytes{device="tmpfs",fstype="tmpfs",mountpoint="/run"} 8.32716800e+08
node_filesystem_size_bytes{device="/dev/sdc1",fstype="ext4",mountpoint="/mnt/broken"} 1.073741824e+10
# HELP node_load1 1m load average.
node_load1 0.42
```

```fung run stdin=metrics.txt exit=1
#!/usr/bin/env fung
# Reads node_exporter metrics (Prometheus text format) on standard input
# and prints how full each filesystem is, fullest first. Exit code 1 if
# any is above --max percent (default 90).
# usage: curl -s http://web1:9100/metrics | fung diskfull.fung [--max 90]
import cli
import regex

# name{labels} value   (the labels are optional)
sample = `^(?P<name>[a-zA-Z_:][\w:]*)(\{(?P<labels>.*)\})?\s+(?P<value>\S+)$`
# key="value" pairs inside the braces
label = `(\w+)="((?:[^"\\]|\\.)*)"`

# One line → {name, labels: {…}, value}, or nil for comments, blanks and
# values that are not numbers (NaN, +Inf).
fn parse(line):
  m = regex.find(line, sample)
  if m == nil: return nil ;;
  labels = regex.find_all(m.named.labels ?? "", label)
    |> fold({}, (acc, l) => put(acc, l.groups[0], l.groups[1]))
  match parse_float(m.named.value):
    Ok(v)  => {name: m.named.name, labels, value: v}
    Err(_) => nil
  ;;
;;

# The samples of one metric as mountpoint → value.
fn by_mount(samples, name) = samples
  |> filter(s => s.name == name and "mountpoint" in s.labels)
  |> filter(s => get(s.labels, "fstype", "") != "tmpfs")
  |> fold({}, (acc, s) => put(acc, s.labels.mountpoint, s.value))

opts = cli.parse(args)
max_pct = parse_int(get(opts.flags, "max", "90")) ?? fail("--max takes a whole percent")
samples = read_lines()! |> map(parse) |> filter(s => s != nil)
avail = by_mount(samples, "node_filesystem_avail_bytes")
size = by_mount(samples, "node_filesystem_size_bytes")

rows = keys(size)
  |> filter(mp => mp in avail and size[mp] > 0)
  |> map(mp => (mp, int(100 - avail[mp] * 100 / size[mp]), size[mp] / 1024 ** 3))
  |> sort_by(((_, pct, _)) => -pct)
for (mp, pct, gib) in rows:
  flag = pct > max_pct then "  <-- over ${max_pct}%" else ""
  print(pad_right(mp, 8), pad_left("${pct}%", 4), pad_left("${int(gib)} GiB", 8) + flag)
;;
if any(rows, ((_, pct, _)) => pct > max_pct): exit(1) ;;
```

`/data`가 90%를 넘어서 종료 코드가 1이다. `tmpfs`는 일부러 뺐고, 값이 `NaN`인 `/mnt/broken`은 사용 가능 바이트를 알 수 없어 표에서 빠졌다.

## 줄별 설명 {#how}

- `sample`: 지표 한 줄의 모양이다. `(?P<name>…)`처럼 이름 붙인 조각(named group)은 `m.named.name`으로 꺼낸다. 번호(`groups[0]`)보다 읽기 쉽고, 정규식 중간에 조각을 더해도 번호가 밀리지 않는다. 중괄호 부분 `(\{…\})?`는 없어도 되므로, `node_load1 0.42`처럼 라벨이 없는 줄에서는 `m.named.labels`가 `nil`이다.
- `label`: 중괄호 안의 `key="value"` 하나다. 값 부분 `(?:[^"\\]|\\.)*`는 "따옴표나 백슬래시가 아닌 글자, 또는 백슬래시와 그다음 한 글자"의 반복이라 `\"`가 든 값도 끝까지 잡는다. 원시 문자열 `` `…` ``이라 백슬래시를 정규식에 적힌 그대로 쓴다.
- `regex.find`와 `regex.find_all`: `find`는 처음 맞는 곳 하나(`{text, start, end, groups, named}`)나 `nil`, `find_all`은 맞는 곳 전부의 리스트를 준다. 라벨은 `find_all`로 모두 찾아 `fold`로 맵 하나에 모은다.
- `parse_float(…)`: `1.6234029056e+10` 같은 지수 표기도 읽는다. fung의 실수는 `NaN`·`Inf`가 될 수 없어서, Prometheus가 보내는 `NaN`·`+Inf`는 `Err`가 되고 그 줄을 버린다.
- `by_mount`: 지표 하나를 "마운트 지점 → 값" 맵으로 만든다. 두 지표를 같은 키로 맞춰 놓으면 `avail[mp]`와 `size[mp]`로 바로 짝지어 계산할 수 있다(쉘의 `join`에 해당).
- 사용률 계산: `/`는 늘 실수 나눗셈이고 `int(…)`는 소수점 아래를 버린다. `1024 ** 3`은 GiB다(`**`는 거듭제곱).
- `sort_by(((_, pct, _)) => -pct)`: 튜플 세 칸 중 가운데만 이름으로 받아 큰 것부터 정렬한다.

:::box mistake 공백이 여러 개인 출력을 `split(s, " ")`로 쪼개기

`ss`, `df`, `ps` 같은 명령은 열을 맞추려고 공백을 여러 개 넣는다. `split`은 구분자 하나마다 자르므로 빈 문자열이 잔뜩 생기고 열 번호가 줄마다 달라진다. 공백 덩어리로 자르려면 `regex.split`을 쓴다(앞뒤 공백은 `trim`으로 먼저 뗀다). 쉘의 `awk '{print $5}'`가 해 주던 일이다.

```fung run
import regex

line = "tcp   LISTEN 0      4096   127.0.0.1:9100   0.0.0.0:*"
print(split(line, " "))
print(regex.split(trim(line), `\s+`))
```

:::

## 응용 {#vary}

- **HTTP로 바로 받기**: 파이프 대신 `http.get("http://${host}:9100/metrics")!.body |> lines`로 받으면 [여러 서버](ssh-collect.html)를 `conc.pmap`으로 한 번에 훑을 수 있다.
- **다른 지표**: `by_mount`의 조건만 바꾸면 된다. 예를 들어 `node_load1`은 라벨이 없으므로 `find(samples, s => s.name == "node_load1")`로 꺼낸다.
- **nginx stub_status**: `Active connections: 291`처럼 정해진 문장은 정규식 `Active connections: (\d+)`를 원시 문자열에 담아 `regex.find(text, pat)`로 찾고, `groups[0]`의 숫자를 `int`로 바꾼다.
- **바꾸기**: 찾은 조각으로 문자열을 바꿀 때는 `regex.replace(s, pat, repl)`이고 `repl` 안의 `$1`이 첫 조각이다. [설정 파일 값 바꾸기](replace-config.html)에 주의점이 있다.

배우기: [6장 문자열](../learn/06-strings.html), [8장 패턴](../learn/08-patterns.html). 레퍼런스: [`regex.find`](../reference/modules/regex.html#fn-regex-find), [`regex.find_all`](../reference/modules/regex.html#fn-regex-find_all), [`regex.split`](../reference/modules/regex.html#fn-regex-split), [`split`](../reference/prelude/strings.html#fn-split), [`parse_float`](../reference/prelude/convert.html#fn-parse_float), [`read_lines`](../reference/prelude/io.html#fn-read_lines).
