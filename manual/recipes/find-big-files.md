---
description: 큰 파일, 오래된 파일 찾기.
lede: 디렉터리 아래를 모두 훑어 큰 파일부터 늘어놓는다. 크기 기준(<code>--min 10M</code>)과 개수(<code>--top 5</code>)를 옵션으로 받고, 오래된 파일은 <code>find</code>와 함께 고른다.
---
# 큰 파일, 오래된 파일 찾기

디스크가 찼다는 알림이 오면 먼저 "무엇이 자리를 차지하는가"를 본다. 로그가 회전되지 않았거나, 코어 덤프가 쌓였거나, 누가 큰 파일을 올려 두었을 수 있다. 여기서는 디렉터리 아래의 모든 파일을 크기순으로 늘어놓고, 크기 기준과 개수를 옵션으로 받는 스크립트를 만든다. 크기는 `1.5K`처럼 사람이 읽기 쉽게 찍는다.

## 쉘로는 {#shell}

```shell
find /var/log -type f -size +10M -exec du -h {} + | sort -rh | head -5
```

익숙한 한 줄이지만 `find`의 `-size`(`+10M`은 "10MiB 초과"이고 단위마다 반올림 규칙이 다르다), `du`의 `-h`, `sort`의 `-h`가 각자 다른 규칙으로 크기를 다룬다. `du`가 보여 주는 것은 파일 길이가 아니라 디스크에서 차지한 블록이라 작은 파일이나 듬성듬성한(sparse) 파일에서 숫자가 어긋난다. macOS와 Linux에서 옵션이 조금씩 달라 같은 줄이 두 곳에서 다르게 돈다.

## fung으로 {#code}

```file app.log
2026-10-20 03:00:01 INFO backup started
2026-10-20 03:12:44 INFO backup finished: 1204 files
2026-10-20 04:00:00 WARN disk /data at 81%
2026-10-20 04:30:00 WARN disk /data at 86%
2026-10-20 05:00:00 ERROR disk /data at 93%
```

```file app.log.1
2026-10-19 03:00:01 INFO backup started
2026-10-19 03:11:02 INFO backup finished: 1187 files
```

```file app.conf
port=8080
```

```fung run args=". --top 3 --min 50"
#!/usr/bin/env fung
# Lists the largest files under a directory, largest first.
# usage: fung bigfiles.fung [dir] [--top N] [--min SIZE]   (SIZE: 500, 10K, 2M, 1G)
import fs
import cli

# "10K" → 10240. A plain number is bytes.
fn parse_size(s):
  (num, unit) = match upper(s[-1..]):
    "K" => (s[..-1], 1024)
    "M" => (s[..-1], 1024 * 1024)
    "G" => (s[..-1], 1024 * 1024 * 1024)
    _   => (s, 1)
  ;;
  (parse_int(num) ?? fail("not a size: ${s}")) * unit
;;

# 1536 → "1.5K".
fn human(n):
  n < 1024 then "${n}B"
    else n < 1024 * 1024 then "${n * 10 // 1024 / 10}K"
    else "${n * 10 // (1024 * 1024) / 10}M"
;;

# A file that vanished or cannot be read since the walk counts as empty.
fn size_of(p) = match fs.stat(p):
  Ok(st) => st.size
  Err(_) => 0
;;

opts = cli.parse(args)
dir = first(opts.rest) ?? "."
top = parse_int(get(opts.flags, "top", "10")) ?? fail("--top must be a number")
min_size = parse_size(get(opts.flags, "min", "0"))

found = fs.walk(dir)!
  |> map(p => {path: p, size: size_of(p)})
  |> filter(f => f.size >= min_size)
  |> sort_by(f => -f.size)
  |> take(top)
for f in found:
  print(pad_left(human(f.size), 7), f.path)
;;
print(pad_left(human(found |> map(f => f.size) |> sum), 7), "total")
```

맨 위의 `example.fung`은 이 스크립트 자신이다. 50바이트보다 작은 `app.conf`는 빠졌다.

## 줄별 설명 {#how}

- `parse_size`: 마지막 글자(`s[-1..]`)를 보고 단위를 정한다. `s[..-1]`은 마지막 글자를 뺀 앞부분이다. `match`가 `(숫자 부분, 곱할 수)` 튜플을 돌려주고, 대입의 왼쪽 `(num, unit)`이 그것을 둘로 나눠 받는다. 숫자가 아니면 `parse_int`가 `Err`를 주고, `?? fail(…)`이 알아보기 쉬운 메시지로 멈춘다.
- `human`: `then … else`를 사슬로 이어 세 경우를 고른다. `n * 10 // 1024`로 소수 첫째 자리까지 남긴 정수를 만든 뒤 `/ 10`으로 나누면 실수가 되어 `1.2`처럼 찍힌다(`/`는 늘 실수, `//`는 내림 나눗셈).
- `size_of`: `fs.walk`로 목록을 받은 뒤 `fs.stat`을 부르기 전에 파일이 지워질 수 있다. 그런 파일 하나 때문에 전체가 멈추지 않도록 `Err`면 0으로 친다.
- `fs.walk(dir)!`: 아래의 모든 파일 경로(디렉터리는 빼고)를 리스트로 준다. 디렉터리를 읽지 못하면 `!`가 멈춘다.
- 파이프 `|>` 네 줄: 경로마다 `{path, size}` 맵을 만들고, 기준보다 작은 것을 거르고, 크기의 음수로 정렬해 큰 것부터 세우고, 앞에서 `top`개를 자른다. 각 줄이 앞 줄의 결과를 첫 인자로 받는다.
- `pad_left(…, 7)`: 크기를 7칸 오른쪽 정렬로 맞춰 경로가 한 줄로 늘어서게 한다.

:::box mistake `int`와 `parse_int`

명령줄 값은 언제나 문자열이다. `int("abc")`는 바꿀 수 없으면 런타임 에러 E0513으로 바로 멈추고, `parse_int("abc")`는 `Err({kind: 'parse, …})`를 돌려준다. 사용자가 잘못 넣을 수 있는 값에는 `parse_int(s) ?? fail("…")`처럼 써서 무엇이 잘못됐는지 직접 말해 준다. `int`는 이미 숫자임이 확실한 값(정규식으로 고른 숫자 등)에 쓴다.

:::

## 응용 {#vary}

**오래된 파일.** `fs.stat`의 `mtime`은 마지막으로 고친 시각(`time` 값)이다. `time.now()`에서 빼면 기간(`dur`)이 되므로 `720h`(30일)와 견준다.

```fung check
# Files under a directory not modified for more than 30 days, largest first.
import fs
import time

dir = first(args) ?? "."
now = time.now()
old = fs.walk(dir)! |> map(p => (p, fs.stat(p)!)) |> filter(((_, st)) => now - st.mtime > 720h)
for (p, st) in old |> sort_by(((_, st)) => -st.size):
  print(pad_left(str(st.size), 10), p)
;;
```

- **지우거나 옮기기**: 고른 파일을 `fs.move(p, path.join(trash, path.base(p)))!`로 휴지통 디렉터리에 옮긴다. 바로 지우는 `fs.remove`보다 안전하다. 먼저 [이름 바꾸기](rename.html)처럼 `--dry-run`을 두는 것이 좋다.
- **디렉터리별 합계**: `found` 대신 모든 파일을 `group_by(files, f => path.dir(f.path))`로 묶고 묶음마다 `sum`을 내면 `du`처럼 디렉터리 크기를 본다.
- **크기를 CSV로**: [CSV와 JSON으로 저장하기](save-csv-json.html).

배우기: [11장 파일과 외부 명령](../learn/11-files-and-commands.html), [7장 컬렉션](../learn/07-collections.html). 레퍼런스: [`fs.walk`](../reference/modules/fs.html#fn-fs-walk), [`fs.stat`](../reference/modules/fs.html#fn-fs-stat), [`sort_by`](../reference/prelude/collections.html#fn-sort_by), [`take`](../reference/prelude/collections.html#fn-take), [`parse_int`](../reference/prelude/convert.html#fn-parse_int), [`exec`](../reference/prelude/process.html#fn-exec).
