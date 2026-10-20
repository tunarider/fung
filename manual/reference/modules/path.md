---
lede: <code>import path</code>. 경로 문자열을 잇고 나누는 계산이다. 디스크를 전혀 건드리지 않으므로 실패하지 않고, 결과는 결과값이 아닌 문자열이다.
description: fung 모듈 path — 파일 시스템을 건드리지 않는 경로 계산(join, base, dir, ext, stem, abs).
---
# path — 경로 계산

## 경로란 {#concepts}

**경로**(path)는 파일 시스템 안의 위치를 적은 문자열이다. `/`로 나눈 조각 하나하나를 **요소**(component)라고 부른다. `/var/log/nginx/access.log`에서 요소는 `var`, `log`, `nginx`, `access.log`이고, 마지막 요소가 파일 이름이다. 파일 이름의 마지막 `.` 뒤(`.log`)가 **확장자**(extension)로, 파일의 종류를 나타내는 관례다.

- `/`로 시작하면 **절대 경로**, 아니면 현재 디렉터리에서 시작하는 **상대 경로**다.
- `.`은 "지금 디렉터리", `..`은 "한 단계 위"다. `a/./b`는 `a/b`, `a/../b`는 `b`와 같은 곳이다.
- `a//b`처럼 `/`가 겹쳐도 `a/b`와 같다.

경로는 그냥 문자열이니 `"${dir}/${name}"`처럼 이어 붙여도 된다. 하지만 `dir`이 `/`로 끝나거나 비어 있으면 `//`나 `/name` 같은 엉뚱한 경로가 나온다. `path` 모듈은 이런 경우를 정리해 준다. 이 모듈의 함수는 **문자열만 계산**하고 디스크를 보지 않는다. 그래서 파일이 실제로 있는지와 상관없이 늘 같은 답을 주고, 실패하지 않는다. 파일이 있는지는 [`fs.exists`](fs.html#fn-fs-exists)로 알아본다.

**정리**(clean)는 같은 곳을 가리키는 가장 짧은 꼴로 바꾸는 것이다: 겹친 `/`를 하나로, `.` 요소를 빼고, `이름/..`을 지우고, 끝의 `/`를 뗀다. 빈 결과는 `.`이 된다. `path.join`, `path.dir`, `path.abs`가 결과를 정리한다. 정리는 글자만 보고 하므로 심볼릭 링크를 따라가지 않는다.

```fung run
import path

p = "/var/log/nginx/access.log"
print(path.dir(p), path.base(p), path.stem(p), path.ext(p))
```

## 함수 {#functions}

:::fn path.join(...parts)

경로 조각들을 이어 붙이고 정리한다.

#### 설명

인자로 받은 문자열을 `/`로 잇고 [정리](#concepts)한다. 빈 조각은 건너뛴다. 조각이 없거나 모두 비면 `""`다. 다른 언어의 경로 함수와 달리, **뒤의 절대 경로가 앞을 지우지 않는다**: `path.join("a", "/c")`는 `"a/c"`다. 사용자가 준 이름이 `/`로 시작해도 지정한 디렉터리 밖으로 나가지 않게 하려는 선택이다. 다만 `..`은 정리되므로 `path.join("a", "../..")`는 `".."`이다. 믿을 수 없는 이름을 붙일 때는 `..`을 따로 검사한다.

인자는 몇 개든 줄 수 있다. 리스트를 그대로 넘길 수는 없으므로, 리스트에 담긴 조각은 `fold(xs, "", path.join)`으로 잇는다.

#### 매개변수

- `...parts`: 이을 문자열들(0개 이상).

#### 돌려주는 값

정리된 경로 문자열.

#### 에러

- 문자열이 아닌 인자는 런타임 에러 E0403.

#### 예제

**이어 붙이기와 정리**

```fung run
import path

print(path.join("/var/log", "nginx", "access.log"))
print(path.join("logs/", "/app.log"))
print(path.join("./a/./b/", "../c"))
print(path.join("a", "..", ".."))
```

**리스트로 받은 조각**

```fung run
import path

parts = ["etc", "nginx", "conf.d"]
print(fold(parts, "", path.join))
print(fold(parts, "/", path.join))
```

#### 같이 보기

[`path.dir`](#fn-path-dir), [`path.abs`](#fn-path-abs), [`fs.list`](fs.html#fn-fs-list)(`full: true`)

:::

:::fn path.base(p)

경로의 마지막 요소(파일 이름)를 돌려준다.

#### 설명

셸의 `basename`과 같다. 끝의 `/`는 무시한다(`"a/b/"` → `"b"`). `"/"`는 `"/"`, 빈 경로는 `"."`이다.

#### 매개변수

- `p`: 경로 문자열.

#### 돌려주는 값

마지막 요소(문자열).

#### 예제

```fung run
import path

print(path.base("/var/log/app.log"), path.base("backup/2026/"), path.base("/"), path.base(""))
```

#### 같이 보기

[`path.dir`](#fn-path-dir), [`path.stem`](#fn-path-stem)

:::

:::fn path.dir(p)

마지막 요소를 뺀 나머지(들어 있는 디렉터리)를 돌려준다.

#### 설명

셸의 `dirname`과 비슷하다. 결과는 정리된 꼴이다. 디렉터리가 없는 이름(`"app.log"`)은 현재 디렉터리 `"."`, `"/x"`는 `"/"`이다. 끝에 `/`가 있으면 그 앞까지가 디렉터리로 남는다(`"a/b/"` → `"a/b"`).

#### 매개변수

- `p`: 경로 문자열.

#### 돌려주는 값

디렉터리 부분(문자열).

#### 예제

**들어 있는 디렉터리**

```fung run
import path

print(path.dir("/var/log/app.log"), path.dir("app.log"), path.dir("/x"), path.dir("a/b/"))
```

**같은 디렉터리에 다른 파일 만들기**

```fung run
import path

conf = "deploy/prod/app.yaml"
print(path.join(path.dir(conf), "secrets.yaml"))
```

#### 같이 보기

[`path.base`](#fn-path-base), [`path.join`](#fn-path-join)

:::

:::fn path.ext(p)

확장자를 점까지 포함해 돌려준다.

#### 설명

마지막 요소에서 마지막 `.`부터 끝까지다: `"a.tar.gz"` → `".gz"`. 확장자가 없으면 `""`. `.`으로 시작하는 숨은 파일 이름(`".bashrc"`)은 확장자가 없는 것으로 본다. 디렉터리 이름의 점은 보지 않는다(`"a/b.c/d"` → `""`).

#### 매개변수

- `p`: 경로 문자열.

#### 돌려주는 값

`.`으로 시작하는 확장자, 또는 `""`.

#### 예제

**확장자별로 나누기**

```fung run
import path

files = ["a.log", "b.txt", "c.log", "Makefile", ".env"]
print(files |> group_by(path.ext))
```

**여러 겹 확장자**

```fung run
import path

print(path.ext("backup.tar.gz"), path.ext(".hidden"), path.ext("dir.d/file"))
```

#### 같이 보기

[`path.stem`](#fn-path-stem)

:::

:::fn path.stem(p)

마지막 요소에서 확장자를 뺀 이름을 돌려준다.

#### 설명

[`path.base`](#fn-path-base)에서 [`path.ext`](#fn-path-ext)를 뗀 것이다. 디렉터리 부분은 남지 않는다(`"dir/report.csv"` → `"report"`). 확장자는 마지막 하나만 떼므로 `"a.tar.gz"`는 `"a.tar"`다. 숨은 파일 이름은 그대로다.

#### 매개변수

- `p`: 경로 문자열.

#### 돌려주는 값

확장자 없는 이름(문자열).

#### 예제

**확장자 바꾸기**

```fung run
import path

src = "reports/2026-10.csv"
print(path.join(path.dir(src), path.stem(src) + ".json"))
print(path.stem("a.tar.gz"), path.stem(".bashrc"))
```

#### 같이 보기

[`path.ext`](#fn-path-ext), [`path.base`](#fn-path-base)

:::

:::fn path.abs(p)

현재 디렉터리를 기준으로 한 절대 경로를 돌려준다.

#### 설명

`p`가 상대 경로면 현재 디렉터리 앞에 붙이고 정리한다. 이미 절대 경로면 정리만 한다. 파일이 있는지는 보지 않고, 심볼릭 링크도 풀지 않는다. 로그나 에러 메시지에 "어느 파일인지" 분명히 적을 때 쓴다.

#### 매개변수

- `p`: 경로 문자열.

#### 돌려주는 값

절대 경로(문자열).

#### 예제

이 매뉴얼의 예제는 빈 디렉터리에서 돌고, 그 디렉터리의 실제 경로는 출력에서 `.`으로 바꿔 보인다. 실제로는 `/home/me/work/app.log`처럼 나온다.

```fung run
import path

print(path.abs("app.log"))
print(path.abs("/etc/../tmp"))
```

#### 같이 보기

[`path.join`](#fn-path-join), [`fs.exists`](fs.html#fn-fs-exists)

:::

파일을 실제로 읽고 쓰는 함수는 [`fs`](fs.html)에 있다. 배우는 순서로는 [11장 파일·외부 명령·CLI](../../learn/11-files-and-commands.html)를 본다.
