---
description: 서버별 결과를 표 하나로.
lede: 서버마다 커널 버전, CPU 수, 부하, 디스크 사용률을 한 번의 <code>ssh</code>로 묻고, 서버 한 줄·항목 한 칸의 표로 모은다. 실패한 서버도 표 안에 이유와 함께 남는다.
---
# 서버별 결과를 표 하나로

점검 회의 전에 "서버마다 커널이 몇 버전이고 디스크가 얼마나 찼나"를 표로 보고 싶을 때가 있다. 서버마다 명령을 여러 번 보내면 느리고, 출력이 제각각이라 맞춰 붙이기도 어렵다. 여기서는 서버에서 돌 작은 쉘 조각이 `key=value` 줄을 내게 하고, fung이 그것을 맵으로 읽어 열을 맞춘 표 하나로 찍는다. 원격 서버는 Linux를 가정한다(`nproc`, `/proc/loadavg`).

## 쉘로는 {#shell}

```shell
for h in $(cat hosts.txt); do
  printf '%s\t%s\t%s\n' "$h" "$(ssh "$h" uname -r)" "$(ssh "$h" df -P / | awk 'NR==2 {print $5}')"
done | column -t
```

서버 한 대에 `ssh`를 항목 수만큼 열고, 서버를 한 대씩 돈다. 한 항목이 실패하면 그 칸이 비어 열이 밀리고, 접속 자체가 실패한 서버는 에러 메시지가 표 사이에 섞여 나온다. `column`은 배포판마다 옵션이 다르다.

## fung으로 {#code}

```file hosts.txt
web1
web2
db1
db2
old1
```

```fung check
#!/usr/bin/env fung
# Asks every server for a few facts in one ssh call each and prints one
# table: a row per server, a column per fact.
# usage: fung inventory.fung hosts.txt
import conc
import fs

# Runs on the server (its own shell) and prints key=value lines.
probe = `echo kernel=$(uname -r)
echo cpus=$(nproc)
echo load=$(cut -d' ' -f1 /proc/loadavg)
df -P / | awk 'NR == 2 {print "disk=" $5}'`

columns = ["kernel", "cpus", "load", "disk"]

# "a=1\nb=2" → {a: "1", b: "2"}; lines without "=" are skipped.
fn parse_facts(text) = lines(text)
  |> map(l => split(l, "="))
  |> filter(kv => len(kv) >= 2)
  |> fold({}, (m, [k, ...v]) => put(m, trim(k), trim(join(v, "="))))

# One server → its row: the facts, or dashes and why it failed.
fn row(host):
  r = timeout(20s, () => exec(["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=5", host, probe]))
  match r:
    Ok(res) =>
      facts = parse_facts(res.out)
      [host] + map(columns, c => get(facts, c, "-")) + ["ok"]
    Err(e)  => [host] + map(columns, _ => "-") + [why(e)]
  ;;
;;

fn why(e) = match e:
  {kind: 'timeout}   => "no answer in 20s"
  {kind: 'exit, err} => trim(err)
  _                  => e.message
;;

# Rows of strings → aligned lines, two spaces between columns.
fn table(rows):
  widths = 0..len(rows[0]) |> map(i => rows |> map(r => len(r[i])) |> max)
  rows |> map(fn(r):
    n = len(r)
    cells = zip(take(r, n - 1), widths) |> map(((c, w)) => pad_right(c, w))
    join(cells + [last(r)], "  ")
  ;;)
;;

file = first(args) ?? fail("usage: inventory.fung hosts.txt")
hosts = fs.read_lines(file)! |> filter(h => h != "")
rows = conc.pmap(hosts, row, limit: 10)
each(table([["host"] + columns + ["status"]] + rows), print)
if any(rows, r => last(r) != "ok"): exit(1) ;;
```

```expect
$ fung inventory.fung hosts.txt
host  kernel                    cpus  load  disk  status
web1  6.8.0-45-generic          4     0.31  44%   ok
web2  6.8.0-45-generic          4     2.10  91%   ok
db1   5.15.0-122-generic        16    7.82  63%   ok
db2   -                         -     -     -     ssh: connect to host db2 port 22: Connection refused
old1  4.18.0-553.el8_10.x86_64  2     0.02  -     ok
```

`old1`은 접속은 됐지만 `disk` 줄을 내지 않아(예를 들어 `df`의 출력 형식이 다를 때) 그 칸만 `-`다.

## 줄별 설명 {#how}

- `probe`: 원격 서버의 쉘이 돌릴 명령들이다. 원시 문자열 `` `…` ``은 여러 줄을 그대로 담고 `$(…)`나 `$5`를 fung의 보간으로 읽지 않는다(fung의 보간은 `"…"` 안의 `${…}`뿐이다). `ssh host probe`처럼 인자 하나로 넘기면 원격 쉘이 줄마다 실행한다. 한 번의 접속으로 네 항목을 모두 받는다.
- `parse_facts`: 출력 줄을 `=`로 쪼개 맵에 모은다. `fold`의 람다는 매개변수 자리에서 리스트 패턴 `[k, ...v]`로 키와 나머지를 바로 나눈다. 값에 `=`가 있어도 `join(v, "=")`로 되살린다. 이 함수는 원격 쪽에서 무엇을 더 찍든(로그인 배너 등) `=`가 없는 줄은 버린다.
- `row`: 서버 하나의 표 한 줄을 문자열 리스트로 만든다. 성공하면 항목마다 `get(facts, c, "-")`로 값을 꺼내 빠진 항목은 `-`로 채우고, 실패하면 항목 칸을 모두 `-`로 채우고 마지막 칸에 이유를 쓴다. 성공과 실패가 같은 모양의 줄이 되므로 표가 깨지지 않는다.
- `why`: `Err` 안의 맵을 모양으로 가른다. `timeout`이 주는 `Err`에는 `message`가 없으므로 먼저 맞춘다. `ssh`가 접속에 실패하면 종료 코드 255와 함께 표준 에러에 이유를 남기므로 `err`를 쓴다.
- `table`: 열마다 가장 긴 칸의 길이를 구한다. `0..len(rows[0])`은 열 번호 범위이고, 그 위의 `map`이 열마다 `max`를 낸다. 마지막 열은 길이가 들쭉날쭉한 메시지가 오므로 채우지 않아 줄 끝에 공백이 남지 않게 한다. `map`에 여러 문장이 필요하면 람다 대신 익명 함수 `fn(r): … ;;`를 넘긴다.
- `conc.pmap(hosts, row, limit: 10)`: 10대씩 동시에 묻는다. 결과는 `hosts` 순서 그대로라 표의 줄 순서가 목록 파일과 같다.

표 부분은 서버 없이도 시험할 수 있다. 원격 출력 대신 문자열을 넣어 돌려 본다.

```fung run
fn parse_facts(text) = lines(text)
  |> map(l => split(l, "="))
  |> filter(kv => len(kv) >= 2)
  |> fold({}, (m, [k, ...v]) => put(m, trim(k), trim(join(v, "="))))

fn table(rows):
  widths = 0..len(rows[0]) |> map(i => rows |> map(r => len(r[i])) |> max)
  rows |> map(fn(r):
    n = len(r)
    cells = zip(take(r, n - 1), widths) |> map(((c, w)) => pad_right(c, w))
    join(cells + [last(r)], "  ")
  ;;)
;;

facts = parse_facts("Welcome to web1!\nkernel=6.8.0\ncpus=4\nopts=a=b\n")
print(facts)
each(table([["host", "kernel", "cpus"], ["web1", facts.kernel, facts.cpus], ["db1", "-", "-"]]), print)
```

:::box mistake 표를 문자열로 쌓아 가기

쉘에서는 `out="$out$line\n"`처럼 문자열 변수에 줄을 덧붙인다. fung에서 같은 모양은 두 가지로 막힌다. 반복문 안에서 바깥 `out`에 다시 대입하면 E0205이고, `fold`로 `acc + line`을 쌓으면 되기는 하지만 줄이 늘수록 매번 전체를 복사해 느려진다. 위처럼 줄을 리스트로 만든 뒤 `each(…, print)`로 찍거나 `join(lines, "\n")`으로 한 번에 잇는다.

:::

## 응용 {#vary}

- **정렬**: 가장 찬 디스크를 위로 올리려면 `rows |> sort_by(r => -(parse_int(replace(r[4], "%", "")) ?? -1))`처럼 열 값을 숫자로 바꿔 정렬한다. `-` 칸은 `?? -1`로 맨 아래로 보낸다.
- **항목 더하기**: `probe`에 `echo mem=$(free -m | awk '/Mem:/ {print $2}')` 줄을 넣고 `columns`에 `"mem"`을 더하면 끝이다. 원격 쪽 출력 형식을 `key=value`로 정해 두었기 때문이다.
- **CSV나 JSON으로**: 같은 `rows`를 [CSV와 JSON으로 저장하기](save-csv-json.html)의 방법으로 파일에 남기면 스프레드시트나 다른 도구에서 연다.
- **기준을 넘은 서버만**: `filter(rows, r => …)`로 디스크 90% 이상만 남기거나, 실패한 서버만 알리려면 [실패·무응답 서버만 알리기](unreachable-servers.html).

배우기: [12장 동시에 여러 일 하기](../learn/12-concurrency.html), [7장 컬렉션](../learn/07-collections.html), [8장 패턴](../learn/08-patterns.html). 레퍼런스: [`conc.pmap`](../reference/modules/conc.html#fn-conc-pmap), [`timeout`](../reference/prelude/concurrency.html#fn-timeout), [`exec`](../reference/prelude/process.html#fn-exec), [`fold`](../reference/prelude/collections.html#fn-fold), [`get`](../reference/prelude/maps.html#fn-get), [`pad_right`](../reference/prelude/strings.html#fn-pad_right), [문자열](../reference/language.html#lexical).
