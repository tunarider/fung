---
description: 표준 모듈과 내 파일을 import하는 법, 무엇이 공개되는지, prelude, 라이브러리 파일의 규칙.
lede: 스크립트가 길어지면 한 파일에 다 담기 어렵다. 이 장은 코드를 파일 여러 개로 나누고, 표준 라이브러리의 모듈을 가져다 쓰는 법을 배운다.
---
# 모듈로 나누기

:::goals
- 모듈이 무엇이고 왜 나누는지
- 표준 모듈을 `import`해서 쓰기, import 없이 쓰는 prelude
- 내 파일을 모듈로 가져오기: 경로, 별칭 `as`, 이름만 가져오기
- 무엇이 공개되고 무엇이 숨는가: `_`로 시작하는 이름
- 가져오는 파일(라이브러리)의 최상위에 쓸 수 있는 것
:::

## 모듈이란 {#what}

**모듈**(module)은 관련된 함수와 값을 한데 묶은 단위다. fung에서는 **파일 하나가 모듈 하나**다. `textutil.fung`이라는 파일은 `textutil` 모듈이 된다.

나누는 이유는 셋이다. 첫째, 한 번 만든 함수를 여러 스크립트에서 다시 쓴다. 둘째, 파일마다 맡은 일이 분명해져서 읽기 쉽다. 셋째, 이름이 부딪히지 않는다. 파일 모듈에도 `read`가 있고 내 설정 모듈에도 `read`가 있어도, `fs.read`와 `config.read`로 구별된다. 모듈 이름이 앞에 붙는 이름의 영역을 **이름 공간**(namespace)이라고 한다.

## 표준 모듈 가져오기 {#std}

fung에는 자주 쓰는 일을 맡은 표준 모듈이 들어 있다. 파일과 디렉터리(`fs`), 경로 계산(`path`), 명령줄 옵션(`cli`), JSON(`json`), 시각(`time`), 정규식(`regex`), HTTP(`http`), 동시성 도우미(`conc`)다. 쓰려면 파일 위쪽에 `import 이름`을 쓰고, `모듈.함수`로 부른다.

```fung run
import path

file = "/var/log/nginx/access.log"
print(path.base(file))
print(path.dir(file))
print(path.ext(file))
print(path.join("backup", path.base(file)))
```

모듈도 값이다. 출력하면 `<module path>`처럼 보이고, `type_of`는 `'module`을 돌려준다. 함수에 인자로 넘길 수도 있다.

```fung run
import path

print(path, type_of(path))
```

:::box tip 쓰지 않는 import
가져오기만 하고 쓰지 않은 모듈은 `fung check`가 경고 W0101로 알려 준다(14장). 지우거나, 쓰는 코드를 마저 쓴다. 경고는 프로그램을 멈추지 않는다.
:::

## prelude: import 없이 쓰는 함수 {#prelude}

`print`, `len`, `map`, `split`, `parse_int`처럼 지금까지 import 없이 쓴 함수들은 **prelude**(전주곡이라는 뜻)에 들어 있다. prelude는 모든 파일에 미리 들어와 있는 기본 함수 묶음이다. 거의 모든 스크립트가 쓰는 함수만 prelude에 두고, 영역이 분명한 것은 모듈로 나눴다. 그래서 `pmap`(여러 일을 동시에 처리하는 `map`, 12장)은 prelude가 아니라 `conc` 모듈에 있다. prelude에 있는 줄 알고 그냥 쓰면, 에러 메시지의 도움말이 어느 모듈에 있는지 알려 준다.

```fung run exit=2 error=E0204
print(pmap([1, 2], x => x * 2))
```

```fung run
import conc

print(conc.pmap([1, 2], x => x * 2))
```

prelude의 이름은 예약어가 아니라서 같은 이름의 변수나 함수를 만들 수 있다. 그러면 그 파일 안에서는 prelude의 것이 가려진다. 실수일 때가 많으므로 `fung check`가 경고 W0103으로 알려 준다. 전체 목록은 [함수 목록](../reference/functions.html)에 있다.

## 내 파일 가져오기 {#local}

내가 쓴 파일은 이름 대신 **경로 문자열**로 가져온다. `./`로 시작하고, 끝의 `.fung`은 쓰지 않는다. 경로는 실행한 위치가 아니라 **import를 쓴 파일의 위치**를 기준으로 한다. 그래서 스크립트를 어느 디렉터리에서 실행해도 같은 파일을 찾는다.

아래 예제는 준비물로 `textutil.fung`을 두고, 같은 디렉터리의 `example.fung`에서 가져온다.

```file textutil.fung
# textutil: small string helpers.

fn slug(s) = s |> trim |> lower |> replace(" ", "-")

fn shout(s) = upper(s) + "!"

fn _squash(s) = s |> split(" ") |> filter(w => w != "") |> join(" ")

fn tidy(s) = _squash(trim(s))
```

```fung run
import "./textutil"

print(textutil.slug("  Release Notes "))
print(textutil.shout("deploy done"))
print(textutil.tidy("  too   many    spaces "))
```

모듈 이름은 파일 이름에서 온다. 다른 이름으로 부르고 싶으면 `as 별칭`을 붙인다. 자주 쓰는 함수 몇 개는 괄호 안에 이름을 적어 모듈 이름 없이 바로 쓸 수 있다. 이때 모듈 이름 자체는 묶이지 않는다.

```file textutil.fung
fn slug(s) = s |> trim |> lower |> replace(" ", "-")

fn shout(s) = upper(s) + "!"
```

```fung run
import "./textutil" as tu
import "./textutil" (slug)

print(tu.shout("hi"))
print(slug("Hello World"))
```

같은 모듈을 여러 번 import해도(여러 파일에서 가져와도) 그 파일은 처음 한 번만 읽어서 실행한다. 그 뒤로는 만들어 둔 모듈을 그대로 나눠 쓴다.

:::box mistake 파일 이름에 -를 쓰면
`my-lib.fung`처럼 이름 규칙(3장)에 맞지 않는 파일은 모듈 이름이 될 수 없어서 `import "./my-lib"`는 컴파일 에러 E0305다. `import "./my-lib" as my_lib`처럼 별칭을 붙인다. 처음부터 `my_lib.fung`으로 짓는 편이 낫다.
:::

## 무엇이 공개되는가 {#public}

모듈의 최상위 함수와 변수는 기본으로 **공개**(public)되어 다른 파일에서 쓸 수 있다. 이름이 `_`로 시작하면 **비공개**(private)다. 위 `textutil`의 `_squash`는 같은 파일의 `tidy`만 쓰는 도우미 함수다. 밖에서 부르면 컴파일 에러 E0304다.

```file textutil.fung
# textutil: small string helpers.

fn slug(s) = s |> trim |> lower |> replace(" ", "-")

fn shout(s) = upper(s) + "!"

fn _squash(s) = s |> split(" ") |> filter(w => w != "") |> join(" ")

fn tidy(s) = _squash(trim(s))
```

```fung run exit=2 error=E0304
import "./textutil"

print(textutil._squash("a  b"))
```

공개할 것은 적게 두는 편이 좋다. 공개한 이름은 다른 파일이 쓰기 시작하므로 바꾸기 어렵지만, 비공개 이름은 그 파일 안에서 마음대로 고칠 수 있다.

내가 import한 이름은 다시 내보내지 않는다. `a.fung`이 `import "./b" (x)`를 해도 `a.x`는 없다. 필요한 모듈은 쓰는 쪽에서 직접 가져온다.

## 라이브러리 파일의 규칙 {#library}

직접 실행하는 파일(**진입 파일**, entry file)과 다른 파일이 가져오는 파일(**라이브러리 파일**)은 최상위에 쓸 수 있는 것이 다르다. 라이브러리 파일의 최상위에는 `import`, `fn`, `type`, `test`(14장), `이름 = 식`만 온다. `print("…")` 같은 문장이 있으면 가져오는 순간 무언가 일어나 버리므로 컴파일 에러 E0306이다.

`report.fung`이 이렇게 생겼다고 하자.

```fung frag
print("loading report")

fn summary(xs) = "${len(xs)} items"
```

`example.fung`에서 이것을 가져오면 실행하기 전에 컴파일 에러로 멈춘다.

```file report.fung
print("loading report")

fn summary(xs) = "${len(xs)} items"
```

```fung run exit=2 error=E0306
import "./report"

print(report.summary(["a"]))
```

`이름 = 식`의 오른쪽에는 어떤 식이든 올 수 있다. 처음 import될 때 한 번 계산된다. 설정을 한 번 읽어 두는 모듈이 이렇게 생겼다.

```file config.fung
# config: settings read once, on the first import.

settings = {port: 8080, retries: parse_int(env("RETRIES") ?? "3")!}
```

```fung run
import "./config"

print(config.settings)
print("port:", config.settings.port)
```

:::box mistake 서로를 가져오는 두 파일
`a.fung`이 `b.fung`을 가져오고 `b.fung`이 다시 `a.fung`을 가져오면 순환 import로 컴파일 에러 E0302이고, 메시지가 `a.fung → b.fung → a.fung`처럼 고리 전체를 보여 준다. 둘이 같이 쓰는 부분을 세 번째 파일로 옮겨 둘 다 그것을 가져오게 한다. 진입 파일을 다른 파일이 가져오는 것도 같은 이유로 막혀 있다.
:::

:::box mistake 모듈 이름을 변수로 쓰기
`import path` 뒤에 `for path in files:`처럼 모듈과 같은 이름의 변수를 만들면 컴파일 에러 E0308이다. 모듈을 가리면 그 아래에서 `path.base`를 부를 수 없기 때문이다. 변수 이름을 바꾸거나 `import path as p`로 가져온다.
:::

:::box other 다른 언어는
Python도 파일 하나가 모듈이고 `import os`, `from x import y`, `import x as y`가 fung의 `import`, `(이름)`, `as`에 해당한다. Python은 `_` 이름을 관례로만 숨기지만 fung은 컴파일러가 막는다. JavaScript의 ES 모듈은 `export`를 붙인 것만 공개하는 반대 방식이다. 쉘의 `source lib.sh`는 이름 공간 없이 모든 것을 한 곳에 섞는다.
:::

:::exercise 로그 요약을 둘로 나누기
로그 줄 리스트를 받아 수준별 개수를 세는 함수 `count_levels(rows)`를 `logstat.fung`에 두고, `example.fung`에서 가져와 `{ERROR: 2, INFO: 1}`처럼 출력해 보자. 줄의 첫 낱말이 수준이다. 한 줄의 수준을 꺼내는 도우미는 비공개로 둔다. 힌트: `fold`로 맵에 개수를 쌓는다(7장).

:::solution

```file logstat.fung
# logstat: counts log lines by level.

fn _level(row) = split(row, " ")[0]

fn count_levels(rows) = fold(rows, {}, (acc, row) => put(acc, _level(row), get(acc, _level(row), 0) + 1))
```

```fung run
import "./logstat"

rows = ["ERROR disk full", "INFO started", "ERROR timeout"]
print(logstat.count_levels(rows))
```

:::
:::

## 정리 {#summary}

- 파일 하나가 모듈 하나이고, 모듈 이름은 파일 이름이다.
- 표준 모듈은 `import fs`처럼 이름으로, 내 파일은 `import "./textutil"`처럼 그 파일 기준의 경로로 가져온다. `as`로 별칭을, `(이름)`으로 이름만 가져온다.
- prelude의 함수는 import 없이 쓴다. 가리면 W0103 경고다.
- `_`로 시작하는 최상위 이름은 비공개다(E0304). 나머지는 공개다.
- 라이브러리 파일의 최상위에는 `import`·`fn`·`type`·`test`·`이름 = 식`만 온다(E0306). 처음 import될 때 한 번 실행된다.
- 순환 import(E0302), 모듈 이름 가리기(E0308)는 컴파일 에러다.

정확한 규칙은 레퍼런스의 [모듈](../reference/language.html#modules)에 있다. 표준 모듈마다의 함수는 [fs](../reference/modules/fs.html), [path](../reference/modules/path.html), [conc](../reference/modules/conc.html) 등 레퍼런스의 "표준 모듈"을 보라. 모듈을 어떻게 읽고 실행하는지는 VM 책의 [모듈](../vm/11-modules.html)에서 다룬다.
