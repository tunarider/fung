---
description: 파일 이름 한꺼번에 바꾸기.
lede: 디렉터리 안 파일 이름을 규칙 하나로 모두 바꾼다. 미리 보기(<code>--dry-run</code>)가 있고, 이미 있는 이름은 덮어쓰지 않고 건너뛴다.
---
# 파일 이름 한꺼번에 바꾸기

내려받은 파일, 스캔한 문서, 다른 팀이 보낸 보고서는 이름이 제각각이다. `Q3 Report (final).TXT`처럼 공백·괄호·대문자가 섞이면 쉘에서 따옴표를 빠뜨리기 쉽고 스크립트로 다루기도 번거롭다. 여기서는 디렉터리 안의 모든 파일 이름을 소문자와 `-`만 쓰는 꼴(kebab-case, 케밥 표기)로 바꾼다. `Q3 Report (final).TXT`는 `q3-report-final.txt`가 된다.

## 쉘로는 {#shell}

```shell
for f in *\ *; do mv -n "$f" "$(echo "$f" | tr 'A-Z ' 'a-z-')"; done
```

한 줄로 되지만 빈틈이 많다. `tr`은 공백만 `-`로 바꾸므로 괄호는 그대로 남고(`q3-report-(final).txt`), 이어진 공백은 `--`가 된다. 바꿀 이름이 이미 있으면 `mv -n`이 아무 말 없이 건너뛰어서 무엇이 안 바뀌었는지 모른다. `-n`을 빠뜨리면 다른 파일을 덮어쓴다. 무엇이 바뀔지 먼저 보려면 `mv` 앞에 `echo`를 붙였다 떼었다 해야 한다.

## fung으로 {#code}

```file "Q3 Report (final).TXT"
```

```file "IMG 2041.JPG"
```

```file "Server List.csv"
```

```file server-list.csv
```

```file my-notes.md
```

```file 참치.md
```

```fung run args=". --dry-run"
#!/usr/bin/env fung
# Renames the files in a directory to kebab-case:
#   "Q3 Report (final).TXT" -> "q3-report-final.txt"
# usage: fung rename.fung <dir> [--dry-run]
import fs
import path
import cli

# Lowercase, letters and digits kept, every other run of characters one "-".
fn kebab(name):
  kept = for c in chars(lower(name)):
    c in "abcdefghijklmnopqrstuvwxyz0123456789" then c else "-"
  ;;
  join(kept) |> split("-") |> filter(p => p != "") |> join("-")
;;

# The new name; a name with no ASCII letter or digit is kept.
fn target(file):
  stem = kebab(path.stem(file))
  stem == "" then file else stem + lower(path.ext(file))
;;

fn rename(dir, file, dry):
  to = target(file)
  if to == file: return 'same ;;
  if fs.exists(path.join(dir, to)):
    eprint("skip:", file, "->", to, "(already exists)")
    return 'skipped
  ;;
  print(file, "->", to)
  if not dry: fs.move(path.join(dir, file), path.join(dir, to))! ;;
  'renamed
;;

opts = cli.parse(args, bools: ["dry-run"])
dir = first(opts.rest) ?? fail("usage: rename.fung <dir> [--dry-run]")
dry = get(opts.flags, "dry_run", false)
results = fs.list(dir)! |> map(f => rename(dir, f, dry))
done = results |> filter(r => r == 'renamed) |> len
skipped = results |> filter(r => r == 'skipped) |> len
note = dry then " (dry run)" else ""
print("${done} renamed, ${skipped} skipped${note}")
```

출력을 보고 괜찮으면 `--dry-run`을 빼고 다시 돌린다. 그때 실제로 이름이 바뀐다. `server-list.csv`가 이미 있으므로 `Server List.csv`는 건너뛰고, 영문자나 숫자가 하나도 없는 `참치.md`는 그대로 둔다(`example.fung`은 스크립트 자신이고 이미 규칙에 맞는다).

## 줄별 설명 {#how}

- `kebab`: `chars(lower(name))`로 글자 리스트를 만들고, `for`로 글자마다 "영문 소문자·숫자면 그대로, 아니면 `-`"를 고른다. `for`는 리스트를 돌려주는 식이라 결과를 바로 `kept`에 담는다. 그다음 `-`로 쪼개 빈 조각을 버리고 다시 `-`로 이으면, 연달아 나온 `-`가 하나로 줄고 앞뒤의 `-`도 사라진다.
- `target`: 이름을 몸통(`path.stem`)과 확장자(`path.ext`, 점 포함)로 나눠 몸통만 `kebab`으로 바꾸고 확장자는 소문자로 만든다. 몸통이 빈 문자열이 되면(한글만 있는 이름) 원래 이름을 지킨다.
- `rename`: 파일 하나를 맡아 무슨 일이 있었는지를 심볼 `'same`·`'skipped`·`'renamed`로 돌려준다. `fs.move`는 대상이 있으면 **말없이 덮어쓴다**. 그래서 먼저 `fs.exists`로 확인하고, 있으면 표준 에러에 이유를 찍고 건너뛴다.
- `fs.move(…)!`: 옮기기가 실패하면(권한 등) `!`가 그 자리에서 스크립트를 멈춘다. 반쯤 바꾼 상태로 조용히 넘어가지 않는다.
- `cli.parse(args, bools: ["dry-run"])`: `--dry-run`을 값 없는 스위치로 읽는다. 옵션 이름의 `-`는 `_`로 바뀌어 `opts.flags.dry_run`이 된다. 옵션이 아닌 인자는 `opts.rest`에 순서대로 들어간다.
- `first(opts.rest) ?? fail(…)`: 디렉터리를 주지 않았으면 사용법을 보이고 멈춘다.
- `done`·`skipped`: 결과 심볼 리스트에서 `filter`와 `len`으로 센다. 반복하면서 세는 변수를 따로 두지 않는다(아래 상자).

:::box mistake 반복문 안에서 개수 세기

쉘에서는 `n=$((n+1))`로 반복문 안에서 수를 센다. fung에서 같은 모양으로 쓰면 컴파일 에러 E0205다. 블록(`:`부터 `;;`까지)은 바깥 변수를 읽을 수는 있어도 바꿀 수는 없다.

```fung run exit=2 error=E0205
n = 0
for f in ["a.txt", "b.txt"]:
  n = n + 1  # E0205
;;
```

위 스크립트처럼 반복의 결과를 리스트로 모은 다음 `filter`·`len`으로 세거나, 누적이 필요하면 `fold`를 쓴다. 왜 이렇게 정했는지는 [배우기 3장](../learn/03-values-and-names.html)에 있다.

:::

## 응용 {#vary}

- **확장자만 고르기**: `fs.list(dir)!` 대신 `fs.glob(path.join(dir, "*.JPG"))!`를 쓰면 맞는 경로만 온다. 원소가 경로이므로 `path.base`로 이름만 떼어 넘긴다.
- **하위 디렉터리까지**: `fs.walk(dir)!`는 아래의 모든 파일 경로를 준다. 파일마다 `path.dir`과 `path.base`로 나눠 `rename(path.dir(p), path.base(p), dry)`를 부른다.
- **다른 규칙**: 날짜를 앞에 붙이거나(`"2026-10-20-" + to`) 번호를 매기는 것은 `target`만 고치면 된다. 번호는 `zip(1..=len(files), files)`처럼 짝을 지어 넘긴다.
- **같은 이름으로 모이는 파일**: `Server List.csv`와 `SERVER  LIST.csv`처럼 둘 다 `server-list.csv`가 되면, 실제로 돌릴 때는 먼저 처리된 쪽만 바뀌고 나머지는 `skip:`으로 남는다. 미리 보기는 파일을 옮기지 않으므로 둘 다 바뀔 것처럼 보인다. 미리 보기에서 같은 대상이 두 번 나오는지 살핀다.

배우기: [11장 파일과 외부 명령](../learn/11-files-and-commands.html), [7장 컬렉션](../learn/07-collections.html). 레퍼런스: [`fs.list`](../reference/modules/fs.html#fn-fs-list), [`fs.move`](../reference/modules/fs.html#fn-fs-move), [`fs.exists`](../reference/modules/fs.html#fn-fs-exists), [`path.stem`](../reference/modules/path.html#fn-path-stem), [`path.ext`](../reference/modules/path.html#fn-path-ext), [`cli.parse`](../reference/modules/cli.html#fn-cli-parse), [E0205](../reference/errors.html#E0205).
