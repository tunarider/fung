---
description: 종료 코드와 표준 에러.
lede: 다른 프로그램이 내 스크립트의 결과를 읽는 두 통로, 종료 코드와 출력. 예제는 모니터링 도구(Nagios·Icinga·Sensu)의 규칙을 따르는 점검 스크립트다.
---
# 종료 코드와 표준 에러

스크립트를 cron, CI, systemd, 모니터링 도구가 부르면 사람이 아니라 프로그램이 결과를 읽는다. 그 프로그램이 보는 것은 두 가지다. **종료 코드**(exit code, 끝날 때 남기는 숫자 하나, 쉘의 `$?`)와 **출력**이다. 출력도 둘로 나뉜다. 결과 데이터는 표준 출력(stdout)으로, 진행 상황·경고·에러 메시지는 표준 에러(stderr)로 보내야 파이프와 리다이렉트가 데이터만 받는다. 여기서는 모니터링 점검의 관례(0 정상, 1 경고, 2 위험, 3 점검 불가)를 따르는 로그 점검 스크립트를 만든다.

## 쉘로는 {#shell}

```shell
n=$(grep -c ' ERROR ' "$1") || true
if [ "$n" -ge 10 ]; then echo "CRITICAL - $n errors"; exit 2
elif [ "$n" -ge 1 ]; then echo "WARNING - $n errors"; exit 1
else echo "OK - $n errors"; exit 0; fi
```

`grep -c`는 맞는 줄이 없으면 종료 코드 1을 돌려줘서 `set -e` 아래에서는 `|| true`가 필요하다. 로그 파일이 없으면 `grep`이 에러를 표준 에러에 찍고 `n`이 빈 문자열이 되어, `[ "" -ge 10 ]`이 다시 에러를 내고 결국 엉뚱한 상태(대개 OK)로 끝난다. 점검이 **돌지 못한** 것과 **문제가 없는** 것이 구별되지 않는다.

## fung으로 {#code}

```file app.log
2026-10-20 09:00:01 INFO started on :8080
2026-10-20 09:01:13 ERROR upstream timeout after 3000 ms
2026-10-20 09:01:40 WARN slow query: 1840 ms
2026-10-20 09:02:05 ERROR upstream timeout after 3000 ms
2026-10-20 09:03:51 ERROR connection refused: db1:5432
2026-10-20 09:04:00 INFO healthcheck ok
```

```fung run args="app.log --warn 2 --crit 5" exit=1
#!/usr/bin/env fung
# A monitoring check (Nagios, Icinga, Sensu): counts ERROR lines in a log.
# One status line on standard output; the exit code is the verdict:
#   0 OK, 1 WARNING, 2 CRITICAL, 3 UNKNOWN (the check itself could not run)
# usage: fung check_errors.fung LOG [--warn 1] [--crit 10]
import cli
import fs

fn finish(code, text):
  names = ["OK", "WARNING", "CRITICAL", "UNKNOWN"]
  print("${names[code]} - ${text}")
  exit(code)
;;

fn number(opts, name, default) = match parse_int(get(opts.flags, name, str(default))):
  Ok(n)  => n
  Err(_) => finish(3, "--${name} is not a number")
;;

opts = cli.parse(args)
file = first(opts.rest) ?? finish(3, "usage: check_errors.fung LOG [--warn N] [--crit N]")
warn = number(opts, "warn", 1)
crit = number(opts, "crit", 10)

log_lines = match fs.read_lines(file):
  Ok(ls) => ls
  Err(e) => finish(3, e.message)
;;
errors = log_lines |> filter(l => " ERROR " in l) |> len
eprint("debug: ${len(log_lines)} lines read from ${file}")

text = "${errors} errors in ${file} | errors=${errors};${warn};${crit}"
if errors >= crit: finish(2, text) ;;
if errors >= warn: finish(1, text) ;;
finish(0, text)
```

에러 줄이 3개라 경고 기준(2)은 넘고 위험 기준(5)은 넘지 않았다. 그래서 `WARNING`과 종료 코드 1이다. `debug:` 줄은 표준 에러로 갔으므로 모니터링 도구는 보지 않는다. 없는 파일을 주면 이렇게 끝난다(직접 돌린 출력).

```plain
$ fung check_errors.fung nope.log; echo "exit $?"
UNKNOWN - cannot read `nope.log`: no such file or directory
exit 3
```

## 줄별 설명 {#how}

- `finish`: 상태 이름과 설명을 한 줄로 찍고 그 코드로 끝낸다. 모니터링 도구는 표준 출력의 첫 줄을 상태 메시지로 쓴다. `exit`는 돌아오지 않고, 끝나기 전에 열려 있는 `with`의 정리(파일 닫기 등)를 모두 돌린다.
- `number`: 옵션 값을 정수로 바꾸고, 안 되면 `3`(점검 불가)으로 끝낸다. 설정 실수를 "문제 없음"이나 "경고"로 보고하지 않는다.
- `first(opts.rest) ?? finish(3, …)`: 파일 이름이 없으면 사용법을 상태 메시지로 내고 3으로 끝난다.
- `match fs.read_lines(file)`: 파일을 못 읽은 경우를 `Err`로 받아 직접 3으로 바꾼다. `!`를 쓰지 않는 이유는 아래 상자에 있다.
- `" ERROR " in l`: 문자열의 `in`은 부분 문자열 검사다. 앞뒤 공백까지 넣어 `ERRORS` 같은 단어에 걸리지 않게 한다.
- `eprint(…)`: 사람을 위한 진단은 표준 에러로. 수동으로 돌릴 때는 보이고, 도구는 무시한다.
- `| errors=3;2;5`: 세로 막대 뒤는 Nagios 계열의 성능 데이터(performance data) 형식이다. 그래프를 그리는 도구가 읽는다.
- 마지막 세 줄: 위험부터 확인한다. 기준을 넘으면 `finish`가 끝내므로 아래 줄은 돌지 않는다.

:::box mistake 점검 스크립트 안의 `!`

평소 스크립트에서는 `fs.read_lines(file)!`가 가장 간단하다. 그러나 `!`가 `Err`를 만나면 fung은 에러를 표준 에러에 찍고 **종료 코드 1**로 끝난다. 모니터링 도구에게 1은 `WARNING`이다. 파일을 못 읽어 점검이 돌지 못했는데 "경고 하나"로 보고되는 것이다. 다른 규칙의 종료 코드를 쓰는 스크립트에서는 `Err`를 `match`로 받아 직접 코드를 정한다.

```fung run exit=1 error=E0603
import fs

log_lines = fs.read_lines("nope.log")!
print(len(log_lines))
```

fung 자신의 종료 코드는 0(정상), 1(실행 중 실패), 2(문법 같은 컴파일 실패), 3(`fung` 명령을 잘못 씀), 그리고 `exit(n)`의 `n`이다([명령줄 도구](../reference/cli.html#errors)). 컴파일 실패의 2는 모니터링 쪽에서 `CRITICAL`로 읽히므로, 배포하기 전에 `fung check`로 확인해 둔다.

:::

## 응용 {#vary}

- **명령의 종료 코드 받기**: `exec`는 0이 아닌 종료를 `Err({kind: 'exit, code, out, err})`로 준다. `systemctl is-active nginx`는 멈춘 서비스에 3을 돌려주므로 `match exec(["systemctl", "is-active", "nginx"]): Ok(_) => finish(0, "nginx active"); Err({kind: 'exit, out}) => finish(2, "nginx ${trim(out)}"); Err(e) => finish(3, e.message) ;;`처럼 옮긴다.
- **종료 코드 그대로 넘기기**: 감싸는 스크립트라면 `Err({kind: 'exit, code}) => exit(code)`로 안쪽 명령의 코드를 그대로 돌려준다.
- **cron과 systemd**: cron은 출력이 있으면 메일을 보내고 종료 코드는 보지 않는다. systemd는 0이 아닌 코드를 실패로 기록하고 `Restart=on-failure`면 다시 띄운다. 어떤 프로그램이 부를지에 따라 출력과 코드를 정한다.
- **사용법 오류는 2**: 일반 명령줄 도구에서는 잘못 쓴 인자에 2를 쓰는 관례가 있다. [명령줄 옵션과 도움말](cli-options.html).
- **실패한 서버만**: 여러 대를 점검해 하나라도 실패하면 1로 끝내는 모양은 [실패·무응답 서버만 알리기](unreachable-servers.html).

배우기: [9장 에러](../learn/09-errors.html), [11장 파일과 외부 명령](../learn/11-files-and-commands.html). 레퍼런스: [`exit`](../reference/prelude/program.html#fn-exit), [`eprint`](../reference/prelude/io.html#fn-eprint), [`exec`](../reference/prelude/process.html#fn-exec), [`fs.read_lines`](../reference/modules/fs.html#fn-fs-read_lines), [에러와 종료 코드](../reference/cli.html#errors), [결과값과 에러](../reference/language.html#results).
