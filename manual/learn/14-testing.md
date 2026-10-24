---
description: test 블록과 assert, fung test로 테스트 돌리기, dbg로 값 보기, fung fmt로 모양 맞추기, fung check의 경고.
lede: 스크립트를 고칠 때마다 손으로 다시 돌려 보는 것은 지치는 일이다. 이 장은 코드가 맞게 도는지 코드로 확인하는 테스트, 그리고 코드를 깔끔하게 지켜 주는 포맷터와 검사기를 배운다.
---
# 테스트와 포맷터

:::goals
- 테스트가 무엇이고 왜 쓰는지
- `test` 블록과 `assert`로 테스트 쓰기, `fung test`로 돌리기
- 실패한 테스트의 메시지 읽기
- 테스트하기 좋게 코드를 나누는 법
- `dbg`로 중간 값 보기, `fung fmt`로 모양 맞추기, `fung check`의 경고
:::

## 테스트란 {#why}

**테스트**(test)는 "이 함수에 이것을 넣으면 이것이 나와야 한다"를 코드로 적어 둔 것이다. 한 번 적어 두면 코드를 고칠 때마다 명령 하나로 전부 다시 확인할 수 있다. 어제까지 되던 것이 오늘 고친 코드 때문에 깨지는 일(**회귀**, regression)을 사람보다 먼저 찾아 준다.

테스트의 기본 도구는 `assert 조건`이다. 조건이 참이면 아무 일도 없고, 거짓이면 그 자리에서 에러 E0807로 멈춘다. 비교가 실패하면 왼쪽 식의 실제 값도 보여 준다.

```fung run exit=1 error=E0807
fn half(n) = n // 2

assert half(10) == 5
assert half(7) == 4
print("never printed")
```

`half(7)`이 4가 아니라 3이라는 것을 메시지가 알려 준다. `//`는 내림 나눗셈이다(3장). 기대가 틀렸는지 코드가 틀렸는지 판단하는 것은 사람의 몫이다.

## test 블록 {#blocks}

`assert`를 스크립트 중간에 흩어 두면 평소 실행에도 섞여 버린다. 그래서 테스트는 `test "이름": … ;;` 블록에 모은다. `test` 블록은 파일의 최상위에만 쓰고, `fung run`(보통 실행)은 이 블록을 건너뛴다. 아래 파일을 그냥 실행하면 마지막 줄의 출력만 나온다.

```fung run
fn level_of(row) = match split(row, " "):
  [level, ...] => lower(level)
;;

fn count_level(rows, level) = rows |> filter(r => level_of(r) == level) |> len

test "level_of takes the first word":
  assert level_of("ERROR disk full") == "error"
  assert level_of("info started") == "info"
;;

test "count_level counts matching rows":
  rows = ["ERROR a", "INFO b", "ERROR c"]
  assert count_level(rows, "error") == 2
  assert count_level(rows, "warn") == 1
;;

print(count_level(["ERROR x", "INFO y"], "error"))
```

## fung test로 돌리기 {#run}

테스트는 `fung test 파일`로 돌린다. 블록마다 `ok` 또는 `FAIL` 한 줄을 찍고, 실패면 이유를 덧붙이고, 끝에 통과·실패 수를 센다. 하나라도 실패하면 종료 코드는 1이다. 그래서 CI에서 `fung test`가 실패하면 빌드가 멈춘다. 위 파일을 `fung test example.fung`으로 돌린 결과다.

```fung check
fn level_of(row) = match split(row, " "):
  [level, ...] => lower(level)
;;

fn count_level(rows, level) = rows |> filter(r => level_of(r) == level) |> len

test "level_of takes the first word":
  assert level_of("ERROR disk full") == "error"
  assert level_of("info started") == "info"
;;

test "count_level counts matching rows":
  rows = ["ERROR a", "INFO b", "ERROR c"]
  assert count_level(rows, "error") == 2
  assert count_level(rows, "warn") == 1
;;

print(count_level(["ERROR x", "INFO y"], "error"))
```

```expect
1
ok   example.fung: level_of takes the first word
FAIL example.fung: count_level counts matching rows
  error[E0807]: assertion failed: count_level(rows, "warn") == 1
    --> example.fung:15:3
     |
  15 |   assert count_level(rows, "warn") == 1
     |   ^
     |
    count_level(rows, "warn") = 0
   call path (innermost first):
     example.fung:15 test: count_level counts matching rows
1 passed, 1 failed
```

두 가지를 눈여겨보자.

- 둘째 블록이 실패해도 첫째 블록의 결과는 그대로이고, 블록이 더 있었다면 다음 블록도 돌았다. 블록 하나가 실패하면 그 블록만 멈춘다.
- 맨 위의 `1`은 파일 최상위의 `print`가 찍은 것이다. `fung test`는 파일의 최상위 코드를 먼저 실행한 뒤 테스트 블록을 돌린다. 테스트 블록이 최상위의 함수와 값을 쓰게 하려는 것이다. 그래서 최상위에서 파일을 지우거나 서버에 요청하는 스크립트는 테스트할 때도 그 일을 한다. 다음 절의 방법으로 나눈다.

이 테스트에서 틀린 쪽은 코드가 아니라 기대(`warn` 줄은 없으니 0이 맞다)다. 마지막 `assert`를 `== 0`으로 고치면 `2 passed, 0 failed`가 된다.

## 테스트하기 좋은 코드 {#design}

테스트하기 쉬운 함수는 **입력을 받아 값을 돌려주기만 하는** 함수다. 파일을 읽고, 계산하고, 출력하는 일을 한 함수에 몰면 테스트에 파일과 출력이 필요해진다. 읽기·쓰기는 얇게 두고 계산을 따로 떼어 내면, 계산은 문자열과 리스트만으로 테스트할 수 있다.

10장의 모듈로 나누면 더 좋다. 계산하는 함수와 그 테스트는 라이브러리 파일에 두고, 진입 파일은 인자를 읽고 라이브러리를 불러 결과를 출력하기만 한다. 라이브러리 파일의 최상위에는 `print` 같은 문장이 올 수 없으므로(E0306), 테스트할 때 엉뚱한 일이 일어나지 않는다.

```file slug.fung
# slug: turns titles into file-name-safe slugs.

fn slug(title) = title |> trim |> lower |> split(" ") |> filter(w => w != "") |> join("-")

test "spaces become single hyphens":
  assert slug("Release  Notes") == "release-notes"
;;

test "edges are trimmed":
  assert slug("  Hello ") == "hello"
;;
```

```fung run args="Weekly  Ops Review"
import "./slug"

print(slug.slug(join(args, " ")) + ".md")
```

`fung test 디렉터리`(또는 인자 없이 `fung test`)는 그 아래의 `.fung` 파일을 모두 찾아 테스트를 돌린다. 테스트가 없는 파일은 건너뛴다. 위 두 파일이 있는 디렉터리에서 `fung test`를 돌리면 이렇게 나온다.

```plain
ok   slug.fung: spaces become single hyphens
ok   slug.fung: edges are trimmed
2 passed, 0 failed
```

파일을 다루는 함수는 `fs.with_temp_dir`로 임시 디렉터리를 만들어 테스트한다. 임시 디렉터리는 테스트가 어떻게 끝나든 지워진다.

```fung check
import fs
import path

# Moves every .log file in dir to <name>.log.1 and returns how many it moved.
fn rotate(dir):
  logs = fs.glob(path.join(dir, "*.log"))?
  try_map(logs, p => fs.move(p, p + ".1", overwrite: true))?
  Ok(len(logs))
;;

test "rotate renames each log":
  fs.with_temp_dir(fn(d):
    fs.write(path.join(d, "a.log"), "x")!
    fs.write(path.join(d, "b.txt"), "y")!
    assert rotate(d)! == 1
    assert fs.list(d)! == ["a.log.1", "b.txt"]
  ;;)
;;
```

```expect
ok   example.fung: rotate renames each log
1 passed, 0 failed
```

:::box mistake 테스트 블록을 함수 안에 쓰기
`test` 블록은 최상위에만 둔다. 함수나 다른 블록 안에 쓰면 컴파일 에러 E0114(`test must be at the top level`)다. 여러 경우를 반복해서 확인하고 싶으면 블록 안에서 `for`를 쓴다: `for (input, want) in cases: assert slug(input) == want ;;`.
:::

:::box tip 결과값 테스트하기
`Ok`·`Err`도 값이라 `==`로 비교할 수 있다. `assert parse_int("42") == Ok(42)`, `assert is_err(parse_int("x"))`처럼 쓴다. 성공해야 하는 호출은 `assert f()! == 기대값`처럼 `!`를 붙이면, 실패했을 때 그 `Err`의 이유가 메시지에 나온다.
:::

## dbg로 중간 값 보기 {#dbg}

테스트가 실패했는데 이유를 모르겠으면 중간 값을 찍어 본다. `print`를 끼워 넣으면 식을 쪼개야 하지만, `dbg(식)`은 식을 감싸기만 하면 된다. 표준 에러에 `[파일:줄] 식의 글자 = 값`을 찍고, 값을 그대로 돌려준다. 파이프 끝에 `|> dbg`로 붙일 수도 있다.

```fung run
fn score(xs) = xs |> filter(x => x > 2) |> len

n = dbg(score([1, 5, 3])) + 1
print(n)
tens = [1, 2, 3] |> map(x => x * 10) |> dbg
print(len(tens))
```

확인이 끝나면 `dbg`를 지운다. 출력이 표준 에러로 가므로 스크립트의 결과(표준 출력)는 더럽히지 않는다.

## fung fmt: 모양 맞추기 {#fmt}

들여쓰기와 띄어쓰기는 사람마다 다르게 쓰기 쉽다. **포맷터**(formatter)는 코드의 모양을 정해진 하나의 스타일로 고쳐 주는 도구다. `fung fmt 파일`은 파일을 제자리에서 고치고, `fung fmt --check 파일`은 고칠 것이 있는 파일 이름만 찍고 종료 코드 1로 끝난다(CI용). fung의 포맷터에는 설정이 없다. 모두가 같은 모양을 쓰면 모양을 두고 다툴 일도 없다.

포맷터는 공백, 들여쓰기(2칸), `match`의 `=>` 줄 맞추기, 꼬리 주석의 위치만 고친다. **줄바꿈은 쓴 대로 둔다.** 이렇게 쓴 코드가 있다고 하자.

```fung frag
fn   status_text(code)=match code:
    200=>"OK"
    404 =>  "Not Found"
    _=>"status ${code}"
;;
hosts=["web1","web2"]   # servers
for h in hosts :
print( h,status_text(200) )
;;
```

`fung fmt`를 돌리면 이렇게 바뀐다. 이 매뉴얼의 모든 예제도 이 모양을 지킨다.

```fung run
fn status_text(code) = match code:
  200 => "OK"
  404 => "Not Found"
  _   => "status ${code}"
;;
hosts = ["web1", "web2"]  # servers
for h in hosts:
  print(h, status_text(200))
;;
```

## fung check: 실행하기 전에 검사 {#check}

`fung check 파일`은 프로그램을 실행하지 않고 문법, 이름, import를 검사한다(2장). 에러가 없어도 실수일 가능성이 큰 것은 **경고**(warning)로 알려 준다. 경고는 `fung check`와 `fung test`에서만 보이고, `fung run`은 경고를 찍지 않는다. 매일 도는 스크립트의 로그가 경고로 더러워지지 않게 하려는 것이다.

<table>
  <thead><tr><th>번호</th><th>뜻</th><th>고치는 법</th></tr></thead>
  <tbody>
    <tr><td><code>W0101</code></td><td>가져오고 쓰지 않는 import</td><td>지운다</td></tr>
    <tr><td><code>W0102</code></td><td>만들고 읽지 않는 변수</td><td>지우거나, 일부러면 이름 앞에 <code>_</code></td></tr>
    <tr><td><code>W0103</code></td><td>prelude 함수와 같은 이름의 변수·함수</td><td>다른 이름을 짓는다</td></tr>
  </tbody>
</table>

아래 파일을 `fung check example.fung`으로 검사한 결과가 그 아래에 있다. 셋 다 경고라서 종료 코드는 0이다.

```fung check
import fs
import path

fn report(rows):
  total = len(rows)
  lines = rows |> map(r => "- ${r}")
  join(lines, "\n")
;;

print(path.base("/tmp/x.log"))
print(report(["a", "b"]))
```

```expect
warning[W0101]: unused import `fs`
  --> example.fung:1:1
  |
1 | import fs
  | ^^
  |
warning[W0102]: unused variable `total`
  --> example.fung:5:3
  |
5 |   total = len(rows)
  |   ^^^^^
  |
 help: prefix it with `_` if it is intentional
warning[W0103]: `lines` shadows a prelude function
  --> example.fung:6:3
  |
6 |   lines = rows |> map(r => "- ${r}")
  |   ^^^^^
  |
```

`lines`를 가린 것은 지금은 문제가 없지만, 같은 함수 안에서 나중에 prelude의 `lines(문자열)`을 부르려 하면 함수가 아니라 그 리스트를 부르게 되어 에러가 난다. `rows_text`처럼 다른 이름을 짓는다.

:::box tip 커밋 전에 세 가지
스크립트를 저장소에 올리기 전에 `fung fmt`, `fung check`, `fung test`를 차례로 돌리는 습관을 들이면 좋다. 셋 다 디렉터리를 받으므로 `fung fmt --check . && fung test .`처럼 CI에 그대로 넣을 수 있다.
:::

:::box other 다른 언어는
Python의 `pytest`는 `test_`로 시작하는 함수를, Rust는 `#[test]`를 붙인 함수를 테스트로 모은다. fung은 Rust처럼 테스트를 같은 파일에 두는 쪽이고, 테스트 이름을 문자열로 쓴다. `fung fmt`는 Python의 `black`, Rust의 `rustfmt`처럼 설정 없는 포맷터다. 쉘 스크립트에서는 `shellcheck`가 `fung check`와 비슷한 일을 한다.
:::

:::exercise 크기 표시 함수 테스트
바이트 수를 `"512 B"`, `"1.5 KiB"`, `"3.0 MiB"`처럼 바꾸는 함수 `human(n)`을 쓰고, 경계(1023, 1024, 1048576)를 확인하는 테스트 블록을 붙여 보자. 1024 미만은 `B`, 1024² 미만은 `KiB`, 그 이상은 `MiB`다. 소수 첫째 자리까지만 보이려면 `int(x * 10) / 10`처럼 자른다.

:::solution

풀이와, 그것을 `fung test example.fung`으로 돌린 결과다.

```fung check
fn one_decimal(x) = int(x * 10) / 10

fn human(n):
  if n < 1024:
    "${n} B"
  else if n < 1024 * 1024:
    "${one_decimal(n / 1024)} KiB"
  else:
    "${one_decimal(n / (1024 * 1024))} MiB"
  ;;
;;

test "bytes, KiB and MiB":
  assert human(512) == "512 B"
  assert human(1023) == "1023 B"
  assert human(1024) == "1.0 KiB"
  assert human(1536) == "1.5 KiB"
  assert human(1048576) == "1.0 MiB"
;;
```

```expect
ok   example.fung: bytes, KiB and MiB
1 passed, 0 failed
```

:::
:::

## 정리 {#summary}

- `assert 조건`은 거짓이면 E0807로 멈추고, 비교의 실제 값을 보여 준다.
- 테스트는 최상위의 `test "이름": … ;;` 블록에 둔다. `fung run`은 건너뛰고 `fung test`가 돌린다. 하나라도 실패하면 종료 코드 1.
- `fung test`는 파일의 최상위 코드도 먼저 실행한다. 계산은 라이브러리 파일로 떼어 내고 진입 파일은 얇게 둔다.
- `dbg(식)`은 식과 값을 표준 에러에 찍고 값을 그대로 돌려준다.
- `fung fmt`는 공백·들여쓰기·정렬을 하나의 스타일로 고친다(줄바꿈은 그대로). `--check`는 CI용.
- `fung check`는 실행 없이 검사하고 W0101·W0102·W0103 경고를 보여 준다.

정확한 규칙은 레퍼런스의 [테스트와 디버깅](../reference/language.html#testing)과 [명령줄 도구](../reference/cli.html#test)에 있다. 포맷터와 테스트 실행기가 어떻게 만들어졌는지는 VM 책의 [포맷터](../vm/15-formatter.html)와 [테스트 실행기](../vm/17-test-runner.html)에서 다룬다. 여기까지 왔다면 fung으로 쓸 수 있는 스크립트의 재료는 모두 갖췄다. 예제 모음의 레시피에서 실제 작업에 맞춰 조합해 보자.
