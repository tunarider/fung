---
description: 명령줄 옵션과 도움말.
lede: 남이 써도 되는 작은 도구의 모양: <code>--help</code>, 기본값이 있는 옵션, 잘못 쓴 인자에 대한 친절한 메시지와 종료 코드 2. 예제는 오래된 백업을 지우는 도구다.
---
# 명령줄 옵션과 도움말

혼자 쓰는 스크립트는 인자 순서를 외우면 되지만, 팀이 함께 쓰거나 몇 달 뒤의 내가 쓰려면 `--help`가 있어야 하고, 잘못 쓰면 무엇이 틀렸는지 알려 줘야 한다. 특히 파일을 지우는 도구라면 미리 보기(`--dry-run`)가 필요하다. 여기서는 백업 디렉터리에서 최신 N개만 남기고 나머지를 지우는 `prune.fung`을 만들며 옵션을 다루는 방법을 보인다.

## 쉘로는 {#shell}

```shell
keep=7; dry=no
while getopts "k:nh" o; do
  case $o in k) keep=$OPTARG;; n) dry=yes;; h) echo "usage: ..."; exit 0;; *) exit 2;; esac
done
shift $((OPTIND - 1))
```

`getopts`는 `-k 7` 같은 한 글자 옵션만 다룬다. `--keep`처럼 긴 옵션은 직접 `case`를 짜거나, 플랫폼마다 다른 GNU `getopt`에 기대야 한다. `keep`이 숫자인지는 따로 확인해야 하고, 확인을 빠뜨리면 `ls | tail -n +$((keep+1))` 같은 줄이 엉뚱하게 돈다.

## fung으로 {#code}

```file db-2026-10-15.tar.gz
```

```file db-2026-10-16.tar.gz
```

```file db-2026-10-17.tar.gz
```

```file db-2026-10-18.tar.gz
```

```file db-2026-10-19.tar.gz
```

```file notes.txt
```

```fung run args="--keep 2 --dry-run ."
#!/usr/bin/env fung
# prune: keeps the newest N backups in a directory and deletes the rest.
import fs
import path
import cli

help = `usage: prune.fung [options] DIR

Keeps the newest backups in DIR and deletes the older ones. Backups are
the files matching --pattern; their names sort by date (db-2026-10-19.tar.gz).

options:
  --keep N         how many to keep (default 7)
  --pattern GLOB   which files are backups (default *.tar.gz)
  --dry-run        only print what would be deleted
  -h, --help       show this help`

known = ["keep", "pattern", "dry_run", "help"]

# A mistake in the arguments: say what is wrong, point at --help, exit 2.
fn usage_error(msg):
  eprint("prune.fung: ${msg}")
  eprint("try `prune.fung --help`")
  exit(2)
;;

opts = cli.parse(args, bools: ["dry-run", "help"])
if get(opts.flags, "help", false) or "-h" in opts.rest:
  print(help)
  exit(0)
;;
unknown = keys(opts.flags) |> filter(k => k not in known)
if not is_empty(unknown): usage_error("unknown option --${replace(unknown[0], "_", "-")}") ;;

dir = match opts.rest:
  [d] => d
  []  => usage_error("DIR is missing")
  _   => usage_error("only one DIR, got ${len(opts.rest)}")
;;
keep = match parse_int(get(opts.flags, "keep", "7")):
  Ok(n) when n >= 1 => n
  _                 => usage_error("--keep takes a whole number of at least 1")
;;
pattern = get(opts.flags, "pattern", "*.tar.gz")
dry = get(opts.flags, "dry_run", false)

backups = fs.glob(path.join(dir, pattern))! |> sort |> reverse
old = drop(backups, keep)
for f in old:
  print(dry then "would delete" else "delete", f)
  if not dry: fs.remove(f)! ;;
;;
print("kept ${len(backups) - len(old)}, ${dry then "would delete" else "deleted"} ${len(old)}")
```

`notes.txt`는 `--pattern`에 맞지 않아 아예 후보가 아니다. 다른 인자로 부르면 이렇게 된다(직접 돌린 출력).

```plain
$ fung prune.fung --keep x /backups
prune.fung: --keep takes a whole number of at least 1
try `prune.fung --help`
$ echo $?
2
$ fung prune.fung --kep 2 /backups
prune.fung: unknown option --kep
try `prune.fung --help`
$ fung prune.fung --help
usage: prune.fung [options] DIR

Keeps the newest backups in DIR and deletes the older ones. Backups are
the files matching --pattern; their names sort by date (db-2026-10-19.tar.gz).

options:
  --keep N         how many to keep (default 7)
  --pattern GLOB   which files are backups (default *.tar.gz)
  --dry-run        only print what would be deleted
  -h, --help       show this help
```

## 줄별 설명 {#how}

- `help`: 도움말은 여러 줄 원시 문자열 하나로 둔다. 따옴표나 `$`를 신경 쓰지 않고 화면에 보일 모양 그대로 쓴다. 맨 위에 `usage:` 한 줄, 그 아래 무엇을 하는지, 옵션 목록과 기본값. 이 순서가 대부분의 명령줄 도구가 따르는 관례다.
- `cli.parse(args, bools: […])`: `--name value`와 `--name=value`를 `flags` 맵에, 나머지 인자를 `rest` 리스트에 순서대로 넣는다. 옵션 이름의 `-`는 `_`가 되어 `--dry-run`은 `flags.dry_run`이다. 옵션과 인자의 순서는 자유라 `prune.fung . --keep 3`도 된다. `--` 뒤의 인자는 모두 `rest`로 간다(`-`로 시작하는 파일 이름을 넘길 때).
- `-h`: `cli.parse`는 `--`로 시작하는 것만 옵션으로 본다. 한 글자 옵션 `-h`는 `rest`에 들어오므로 직접 확인한다.
- `known`·`unknown`: `cli.parse`는 모르는 옵션도 그대로 받아 준다. 오타(`--kep`)를 조용히 무시하면 기본값 7로 지워 버리므로, 아는 옵션 목록과 비교해서 막는다.
- `usage_error`: 무엇이 틀렸는지 한 줄, 도움말 보는 법 한 줄을 표준 에러에 쓰고 종료 코드 2로 끝낸다. "사용법이 틀렸다"를 2로 알리는 것은 많은 명령의 관례다(`grep`, `ls` 등). `exit`는 돌아오지 않으므로 `match`의 가지에서 값 대신 불러도 된다.
- `dir = match opts.rest: …`: 위치 인자가 정확히 하나인지 리스트 패턴으로 가른다. `[d]`는 원소가 하나인 리스트, `[]`는 빈 리스트, `_`는 나머지 전부다.
- `keep = match parse_int(…)`: 숫자로 바뀌고 1 이상일 때만 받는다. guard `when n >= 1`이 범위를 함께 확인한다.
- `fs.glob(…)! |> sort |> reverse`: 이름에 날짜가 들어 있으므로 이름순 정렬이 곧 시간순이다. 뒤집어서 최신이 앞에 오게 하고 `drop(backups, keep)`으로 앞의 `keep`개를 뺀 나머지가 지울 대상이다.
- `if not dry: fs.remove(f)! ;;`: 미리 보기가 아니면 지운다. 지우기가 실패하면 `!`가 그 자리에서 멈춘다.

:::box mistake 스위치 옵션을 `bools`에 빠뜨리기

`cli.parse`는 옵션 뒤에 `--`로 시작하지 않는 인자가 있으면 그것을 옵션의 값으로 가져간다. 값을 받지 않는 스위치(`--dry-run`)를 `bools`에 넣지 않으면, 바로 뒤의 디렉터리 이름이 스위치의 값이 되어 버린다. 그러면 디렉터리를 주지 않은 것이 되고, `--dry-run`의 값은 `true`가 아니라 문자열이다.

```fung run
import cli

print(cli.parse(["--dry-run", "/backups"]))
print(cli.parse(["--dry-run", "/backups"], bools: ["dry-run"]))
```

:::

## 응용 {#vary}

- **환경 변수를 기본값으로**: `keep = get(opts.flags, "keep", env("PRUNE_KEEP") ?? "7")`. 명령줄 옵션이 환경 변수보다, 환경 변수가 코드의 기본값보다 우선한다.
- **기간 옵션**: `--older-than 30d`처럼 받으려면 `parse_dur`를 쓴다. 단위는 `ms`·`s`·`m`·`h`뿐이라 30일은 `720h`로 쓴다.
- **여러 번 쓰는 옵션**: `cli.parse`는 같은 옵션을 두 번 주면 마지막 값만 남긴다. 여러 값이 필요하면 `--hosts web1,web2`처럼 받아 `split(…, ",")`한다.
- **종료 코드의 뜻 나누기**: 사용법 오류는 2, 작업 실패는 1처럼 나누는 방법은 [종료 코드와 표준 에러](exit-codes.html).
- 저장소의 `examples/rename.fung`, `examples/tidy.fung`, `examples/envcheck.fung`도 `cli.parse`로 옵션을 받는다.

배우기: [10장 모듈](../learn/10-modules.html), [8장 패턴](../learn/08-patterns.html), [11장 파일과 외부 명령](../learn/11-files-and-commands.html). 레퍼런스: [`cli.parse`](../reference/modules/cli.html#fn-cli-parse), [`args`](../reference/prelude/program.html#fn-args), [`exit`](../reference/prelude/program.html#fn-exit), [`fs.glob`](../reference/modules/fs.html#fn-fs-glob), [`fs.remove`](../reference/modules/fs.html#fn-fs-remove), [명령줄 도구](../reference/cli.html).
