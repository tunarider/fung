---
description: 설정 파일 값 바꾸기 (sed 대신).
lede: <code>key = value</code> 꼴의 설정 파일에서 지정한 키의 값만 바꾼다. 주석·공백·순서는 그대로 두고, 없는 키는 끝에 더하고, 무엇이 바뀌었는지 알려 준다.
---
# 설정 파일 값 바꾸기 (sed 대신)

배포 스크립트나 서버 준비 단계에서 설정 파일의 값 몇 개를 바꾸는 일은 흔하다. 포트를 바꾸고, 로그 레벨을 올리고, 새 키를 하나 더한다. 여기서는 `fung setconf.fung app.conf port=9090 log_level=debug`처럼 부르면 그 키들만 바꾸는 작은 도구를 만든다. 주석 처리된 옛 값(`# port = 9999`)은 건드리지 않고, 파일은 한 번에 통째로 바꿔서 중간에 멈춰도 반쯤 쓴 설정이 남지 않는다.

## 쉘로는 {#shell}

```shell
sed -i 's/^port *=.*/port = 9090/' app.conf
grep -q '^timeout' app.conf || echo 'timeout=30s' >> app.conf
```

`sed -i`는 GNU sed의 옵션이고, macOS의 sed는 `sed -i ''`처럼 빈 인자를 하나 더 받아야 해서 같은 줄이 두 곳에서 돌지 않는다. 새 값에 `/`나 `&`가 들어 있으면(`url=http://…`) 식이 깨지거나 다른 뜻이 된다. 바꿀 키가 없을 때 아무 말이 없어서 오타를 낸 키 이름을 알아채지 못하고, 없는 키를 더하려면 `grep`과 `echo`를 따로 써야 한다.

## fung으로 {#code}

```file app.conf
# app settings
listen = 0.0.0.0
port = 8080
log_level = info
# port = 9999  (old)
data_dir=/var/lib/app
```

```fung run args="app.conf port=9090 log_level=debug timeout=30s --print"
#!/usr/bin/env fung
# Sets KEY=VALUE pairs in a "key = value" config file, keeping comments,
# spacing and order. Keys that are not there are added at the end.
# usage: fung setconf.fung <file> KEY=VALUE... [--print]
import fs
import cli
import regex

# indent, key, separator, value; comment lines never match
setting = `^(\s*)([\w.-]+)(\s*=\s*)(.*)$`

# "port=9090" → ("port", "9090"); the value may itself hold "=".
fn pair(arg):
  [k, ...rest] = split(arg, "=")
  if k == "" or is_empty(rest): fail("not KEY=VALUE: ${arg}") ;;
  (k, join(rest, "="))
;;

# The line with its key's new value, or the line as it was.
fn update(line, wanted):
  m = regex.find(line, setting)
  if m == nil: return line ;;
  [indent, key, sep, old] = m.groups
  new = get(wanted, key, old)
  if new != old: eprint("${key}: ${old} -> ${new}") ;;
  indent + key + sep + new
;;

opts = cli.parse(args, bools: ["print"])
file = first(opts.rest) ?? fail("usage: setconf.fung <file> KEY=VALUE... [--print]")
wanted = drop(opts.rest, 1) |> map(pair) |> fold({}, (m, (k, v)) => put(m, k, v))

old = fs.read_lines(file)!
existing = old |> map(l => regex.find(l, setting)) |> filter(m => m != nil) |> map(m => m.groups[1])
added = keys(wanted) |> filter(k => k not in existing) |> map(k => "${k}=${wanted[k]}")
result = (old |> map(l => update(l, wanted))) + added
each(added, l => eprint("added:", l))

if get(opts.flags, "print", false):
  print(join(result, "\n"))
else:
  # a whole new file first, then one rename: never half a config
  fs.write(file + ".tmp", join(result, "\n") + "\n")!
  fs.move(file + ".tmp", file, overwrite: true)!
;;
```

`--print`를 주면 `sed`를 `-i` 없이 쓴 것처럼 새 내용을 표준 출력에 찍기만 하고 파일은 그대로 둔다. 바뀐 내용 보고(`port: 8080 -> 9090`)는 표준 에러로 가므로 `--print`의 출력을 파일로 돌려도 섞이지 않는다. `--print`를 빼면 파일을 바꾼다.

## 줄별 설명 {#how}

- `setting`: 설정 줄 하나를 네 조각(들여쓰기, 키, `=`와 주변 공백, 값)으로 나누는 정규식이다. 줄 처음에 `#`이 오면 키 자리(`[\w.-]+`)에 맞지 않으므로 주석 줄은 저절로 빠진다. 정규식은 원시 문자열 `` `…` ``에 써서 백슬래시를 두 번 쓰지 않는다.
- `pair`: 인자를 `=`로 쪼갠다. 첫 조각이 키이고, 나머지를 다시 `=`로 이어 값으로 쓴다. 그래서 `url=http://x/?a=b`처럼 값 안의 `=`도 지켜진다. `split`은 늘 원소가 하나 이상인 리스트를 주므로 `[k, ...rest]` 구조 분해는 실패하지 않는다.
- `update`: `regex.find`는 맞는 것이 없으면 `nil`, 있으면 `{text, groups, …}` 맵을 준다. 맞으면 `groups`를 네 이름으로 풀고, 키가 바꿀 목록에 있으면 값 조각만 갈아 끼운다. 들여쓰기와 구분자(`" = "`인지 `"="`인지)는 원래 줄의 것을 그대로 쓴다. 문자열을 이어 붙여 만들기 때문에 새 값에 `/`, `&`, `$` 같은 글자가 있어도 아무 일이 없다.
- `wanted`: `(키, 값)` 튜플 리스트를 `fold`로 맵 하나에 모은다. 람다의 매개변수 자리에서 `(k, v)` 패턴이 튜플을 바로 푼다.
- `existing`·`added`: 파일에 이미 있는 키를 모으고, 없는 키만 `key=value` 줄로 만든다.
- 마지막 `if`: 새 내용을 `app.conf.tmp`에 다 쓴 뒤 `fs.move`로 이름을 바꾼다. 같은 파일 시스템 안의 이름 바꾸기는 한 번에 일어나므로, 쓰는 도중에 디스크가 차거나 스크립트가 멈춰도 `app.conf`는 옛 내용 그대로다. `fs.move`는 대상이 있으면 거절하므로 바꿔치기가 목적인 여기서는 `overwrite: true`를 준다. 새 파일은 기본 권한(보통 0644)으로 만들어지므로, 비밀값이 든 설정이면 `fs.write(…, perm: 0o600)`으로 써서 바꿔치기한 뒤에도 남이 읽지 못하게 한다.

:::box mistake `replace`는 정규식이 아니다

`replace(s, old, new)`는 글자 그대로 찾아 바꾼다. `replace(line, "port=.*", "port=9090")`은 `.*`라는 글자를 찾으므로 아무것도 바꾸지 않는다. 패턴으로 바꾸려면 `regex.replace(s, pat, repl)`을 쓰는데, 그 `repl` 안의 `$1`·`$0`은 잡은 조각을 가리키는 특별한 표기다. 사용자가 준 값처럼 무엇이 들어 있을지 모르는 문자열은 위 스크립트처럼 조각을 잡아서(`regex.find`) 이어 붙이는 쪽이 안전하다.

```fung run
import regex

line = "port = 8080"
print(replace(line, "port = .*", "port = 9090"))
print(regex.replace(line, `=.*`, "= 9090"))
print(regex.replace("price = 10", `\d+`, "$5"))
```

마지막 줄의 `$5`는 "다섯째 조각"으로 읽혀 빈 문자열이 되었다.

:::

## 응용 {#vary}

- **파일 권한**: 새로 쓴 `.tmp` 파일은 기본 권한(보통 `0644`)으로 만들어지므로, 이름을 바꾸고 나면 원래 파일의 권한이 사라진다. 비밀값이 든 `0600` 파일이면 뒤에 `exec(["chmod", "600", file])!`를 더한다. 권한을 지키는 쪽이 더 중요하면 `fs.write(file, …)!`로 제자리에 쓰되, 그러면 한 번에 바뀌는 성질은 잃는다.
- **값 뒤의 주석**: `port = 8080  # default`의 값 조각은 `8080  # default`까지다. 주석을 지키려면 정규식의 값 부분을 `([^#]*?)(\s*#.*)?$`처럼 나눈다. 주석이 없는 줄에서는 마지막 조각이 `nil`로 온다.
- **백업**: `fs.copy(file, file + ".bak")!`을 쓰기 전에 둔다.
- **env 파일 검사**: 필수 키가 다 있는지는 저장소 `examples/envcheck.fung`·`examples/dotenv.fung`가 보여 준다.
- **여러 서버에**: 설정을 바꾼 뒤 `exec(["scp", file, "${host}:/etc/app/app.conf"])!`로 보낸다. 여러 대를 동시에 하는 방법은 [여러 서버에 명령 보내고 모으기](ssh-collect.html).

배우기: [6장 문자열](../learn/06-strings.html), [11장 파일과 외부 명령](../learn/11-files-and-commands.html). 레퍼런스: [`regex.find`](../reference/modules/regex.html#fn-regex-find), [`regex.replace`](../reference/modules/regex.html#fn-regex-replace), [`replace`](../reference/prelude/strings.html#fn-replace), [`fs.write`](../reference/modules/fs.html#fn-fs-write), [`fs.move`](../reference/modules/fs.html#fn-fs-move), [`fold`](../reference/prelude/collections.html#fn-fold).
