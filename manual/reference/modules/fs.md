---
lede: <code>import fs</code>. 파일을 읽고 쓰고, 디렉터리를 살피고, 만들고 지우고 옮긴다. 실패할 수 있는 연산은 모두 <code>Ok</code>/<code>Err</code>를 돌려준다.
description: fung 모듈 fs — 파일과 디렉터리를 읽고 쓰고 만들고 지우는 함수들.
---
# fs — 파일과 디렉터리

## 모듈과 파일 시스템 {#concepts}

**모듈**(module)은 관련 있는 함수를 한 이름 아래 묶은 것이다. `print`나 `len`처럼 어디서나 쓰는 함수는 prelude(프렐류드, 처음부터 들어 있는 이름들)에 있고, 파일·JSON·HTTP처럼 필요할 때만 쓰는 기능은 모듈로 나뉘어 있다. 모듈은 파일 맨 위에서 `import fs`로 불러오고, 그 안의 함수는 `fs.read`처럼 `모듈.이름`으로 부른다. 불러오지 않고 `fs.read`를 쓰면 `fs`라는 이름이 없다는 컴파일 에러(E0204)다. 자세한 규칙은 [언어 명세의 모듈](../language.html#modules)에 있다.

**파일 시스템**(file system)은 운영체제가 디스크의 데이터를 정리하는 방식이다. 데이터 한 덩어리가 **파일**이고, 파일을 담는 상자가 **디렉터리**(directory, 폴더)다. 디렉터리 안에 다시 디렉터리가 들어갈 수 있어서 전체는 나무 모양이 된다. 파일의 위치는 **경로**(path)로 적는다.

- `/var/log/app.log`처럼 `/`로 시작하는 경로는 **절대 경로**다. 나무의 뿌리(`/`)부터 내려간다.
- `logs/app.log`처럼 `/`로 시작하지 않는 경로는 **상대 경로**다. 프로그램을 실행한 **현재 디렉터리**에서부터 찾는다. `.`은 현재 디렉터리, `..`은 한 단계 위다.

경로를 이어 붙이거나 확장자를 떼는 계산은 디스크를 건드리지 않으므로 [`path`](path.html) 모듈이 따로 맡는다.

파일 연산은 프로그램이 잘못하지 않아도 실패할 수 있다. 파일이 없거나, 권한이 없거나, 디스크가 찼을 수 있다. 그래서 `fs`의 함수는 실패를 런타임 에러로 멈추지 않고 **값으로** 돌려준다. 성공하면 `Ok(값)`, 실패하면 `Err({kind, message})`다. `kind`는 실패의 종류를 나타내는 심볼이고 `message`는 사람이 읽을 문장이다.

<table>
  <thead><tr><th><code>kind</code></th><th>뜻</th></tr></thead>
  <tbody>
    <tr><td><code>'not_found</code></td><td>경로가 없다(위의 디렉터리가 없는 경우도).</td></tr>
    <tr><td><code>'exists</code></td><td>이미 있다(<code>mkdir</code>할 자리에 파일이 있음, 비지 않은 디렉터리를 <code>remove</code>함, <code>move</code>할 자리에 이미 있음 등).</td></tr>
    <tr><td><code>'permission</code></td><td>권한이 없다.</td></tr>
    <tr><td><code>'io</code></td><td>그 밖의 입출력 실패(디렉터리를 파일처럼 읽음, 경로 중간이 파일임, 디스크가 참 등).</td></tr>
    <tr><td><code>'invalid</code></td><td>잘못된 glob 패턴(<code>fs.glob</code>), 읽기로 열지 않은 핸들로 읽기(또는 그 반대).</td></tr>
    <tr><td><code>'closed</code></td><td>닫힌 파일 핸들을 썼다(핸들 함수만).</td></tr>
  </tbody>
</table>

`message`는 ``cannot read `nope.txt`: no such file or directory``처럼 "무엇을 하려다 어디서 왜"의 꼴이다. 운영체제가 말한 이유는 소문자 짧은 문장이고, 흔한 이유는 어느 운영체제에서나 같은 문구다. 결과는 `!`(실패면 멈춤), `?`(실패면 함수에서 돌려줌), `match`(종류별로 처리)로 다룬다. 결과를 쓰지 않고 버리면, 그 값이 `Err`일 때 E0606으로 멈춘다([결과값과 에러](../language.html#results)).

```fung run
import fs

match fs.read("config.txt"):
  Ok(text)                => print(text)
  Err({kind: 'not_found}) => print("no config, using defaults")
  Err(e)                  => print(e.message)
;;
```

파일 내용은 문자열로 읽는다. fung의 문자열은 UTF-8 텍스트이므로, UTF-8이 아닌 바이트는 잘못된 부분마다 U+FFFD(`"\u{FFFD}"`) 하나로 바뀐다. 이미지 같은 바이너리 파일은 그대로 다룰 수 없다.

:::box mistake 이름이 같은 다른 함수

`fs.remove`·`fs.close`는 prelude의 `remove`(맵에서 키 빼기)·`close`(채널 닫기)와 이름만 같은 다른 함수다. 파일에는 반드시 `fs.`를 붙인다. 붙이지 않으면 prelude 함수가 불려 인자가 모자라거나(E0407) 타입이 맞지 않는(E0403) 런타임 에러가 난다.

:::

## 통째로 읽고 쓰기 {#whole}

작은 파일(설정, 로그 한 개, 목록)은 한 번에 읽고 한 번에 쓰는 것이 가장 쉽다. 수백 MB 이상의 큰 파일을 한 줄씩 다룰 때는 [파일 핸들](#handles)을 쓴다.

:::fn fs.read(path)

파일 전체를 문자열 하나로 읽는다.

#### 설명

`path`의 내용을 처음부터 끝까지 읽어 문자열로 돌려준다. 줄바꿈도 그대로 들어 있다. 줄 단위로 다루려면 [`fs.read_lines`](#fn-fs-read_lines)가 편하다. UTF-8이 아닌 바이트는 U+FFFD로 바뀐다.

#### 매개변수

- `path`: 읽을 파일의 경로(문자열).

#### 돌려주는 값

`Ok(문자열)`.

#### 에러

- `Err({kind: 'not_found})`: 파일이 없다. 빈 경로 `""`도 같다.
- `Err({kind: 'io})`: 디렉터리를 읽으려 했다(``is a directory``) 등.
- `Err({kind: 'permission})`: 읽을 권한이 없다.
- `path`가 문자열이 아니면 런타임 에러 E0403.

#### 예제

**설정 파일 읽기**

```file app.conf
port=8080
```

```fung run
import fs

text = fs.read("app.conf")!
print(len(text), trim(text))
```

**없는 파일**

```fung run
import fs

print(fs.read("missing.txt"))
```

#### 같이 보기

[`fs.read_lines`](#fn-fs-read_lines), [`fs.open`](#fn-fs-open), [`read_line`](../prelude/io.html#fn-read_line)

:::

:::fn fs.read_lines(path) fung

파일을 줄 리스트로 읽는다.

#### 설명

[`fs.read`](#fn-fs-read)로 읽은 뒤 prelude의 [`lines`](../prelude/strings.html#fn-lines)로 나눈다. 규칙도 `lines`와 같다: `\n`에서 나누고, 줄 끝의 `\r`은 떼고(Windows 줄바꿈), 파일 끝의 줄바꿈 하나는 빈 줄을 만들지 않는다. 빈 파일은 빈 리스트다. 파일을 통째로 메모리에 올리므로 아주 큰 파일에는 [`fs.with_open`](#fn-fs-with_open)과 `read_line`을 쓴다.

#### 매개변수

- `path`: 읽을 파일의 경로.

#### 돌려주는 값

`Ok(줄 리스트)`. 각 줄에는 줄바꿈이 없다.

#### 에러

[`fs.read`](#fn-fs-read)와 같다.

#### 예제

**로그에서 에러 줄만 고르기**

```file app.log
INFO start
ERROR disk full
INFO retry
ERROR disk full
```

```fung run
import fs

errors = fs.read_lines("app.log")! |> filter(l => starts_with(l, "ERROR"))
print(len(errors), "errors")
print(errors[0])
```

**줄 끝 규칙**

```fung run
import fs

fs.write("crlf.txt", "a\r\nb\n\n")!
print(fs.read_lines("crlf.txt")!)
```

마지막 `\n` 하나는 줄을 끝낼 뿐이고, 그 앞의 빈 줄 하나는 그대로 남는다.

#### 같이 보기

[`lines`](../prelude/strings.html#fn-lines), [`read_lines`](../prelude/io.html#fn-read_lines)(파일 핸들·표준 입력에서)

:::

:::fn fs.write(path, text, perm = nil)

파일에 문자열을 쓴다. 있던 내용은 지운다.

#### 설명

`path`에 `text`를 쓴다. 파일이 없으면 새로 만들고, 있으면 **내용을 모두 지우고** 덮어쓴다(셸의 `>`와 같다). 새 파일의 권한은 0666에서 umask를 뺀 것이다(셸과 같다). `perm`을 주면 파일은 정확히 그 권한이 된다(아래). 위의 디렉터리는 만들지 않으므로, 필요하면 먼저 [`fs.mkdir`](#fn-fs-mkdir)를 부른다. 줄바꿈은 자동으로 붙지 않는다.

#### 매개변수

- `path`: 쓸 파일의 경로.
- `text`: 쓸 내용(문자열). 다른 값은 `str(x)`나 [`json.stringify`](json.html#fn-json-stringify)로 바꿔서 넘긴다.
- `perm`: 파일의 권한 비트(`0o600`처럼 8진수로, 0부터 `0o777`까지). 주면 내용을 쓰기 전에 파일을 정확히 이 권한으로 맞춘다. umask를 빼지 않고, **이미 있는 파일도** 바꾼다(셸의 `install -m`과 같다). 기본 `nil`이면 새 파일은 0666에서 umask를 뺀 것이고 있던 파일은 그대로다. 범위 밖이면 런타임 에러 E0403.

#### 돌려주는 값

`Ok(nil)`.

#### 에러

- `Err({kind: 'not_found})`: 위의 디렉터리가 없다.
- `Err({kind: 'io})`: 경로가 디렉터리다.
- `Err({kind: 'permission})`: 쓸 권한이 없다. `perm`을 줬는데 권한을 바꿀 수 없는 파일(다른 사용자의 것)이어도 그렇다. 그때는 아무것도 쓰지 않는다.

#### 예제

**쓰고 다시 읽기**

```fung run
import fs

fs.write("hosts.txt", "web-1\nweb-2\n")!
print(fs.read_lines("hosts.txt")!)
fs.write("hosts.txt", "db-1\n")!
print(fs.read_lines("hosts.txt")!)
```

**디렉터리가 없으면**

```fung run
import fs

print(fs.write("out/report.txt", "x"))
fs.mkdir("out")!
print(fs.write("out/report.txt", "x"))
```

**비밀값은 `perm`으로**

토큰이나 비밀번호가 든 파일은 다른 사용자가 읽지 못하게 처음부터 `0o600`으로 쓴다. 쓴 뒤에 권한을 바꾸면 그 사이에 열려 있는 틈이 생긴다.

```fung frag
import fs

fs.write("token.txt", token, perm: 0o600)!
```

`perm`은 이미 있는 파일에도 적용된다. 지난 실행이 남긴 임시 파일이 0644여도 비밀값을 쓰기 전에 0600이 된다. 다만 그 전에 다른 프로세스가 열어 둔 핸들은 막지 못한다. 그런 걱정이 있으면 먼저 [`fs.remove`](#fn-fs-remove)로 지운다.

#### 같이 보기

[`fs.append`](#fn-fs-append), [`fs.write_line`](#fn-fs-write_line), [`fs.mkdir`](#fn-fs-mkdir), [`fs.move`](#fn-fs-move)(바꿔치기)

:::

:::fn fs.append(path, text, perm = nil)

파일 끝에 문자열을 덧붙인다.

#### 설명

있던 내용은 그대로 두고 끝에 `text`를 붙인다(셸의 `>>`와 같다). 파일이 없으면 새로 만든다. 로그나 감사 기록처럼 계속 쌓는 파일에 쓴다. 줄바꿈은 직접 넣는다.

#### 매개변수

- `path`: 파일의 경로.
- `text`: 덧붙일 문자열.
- `perm`: [`fs.write`](#fn-fs-write)와 같다. 이미 있는 파일도 이 권한으로 맞춘다.

#### 돌려주는 값

`Ok(nil)`.

#### 에러

[`fs.write`](#fn-fs-write)와 같다.

#### 예제

**기록 쌓기**

```fung run
import fs

each(["start", "check", "stop"], step => fs.append("audit.log", "${step}\n")!)
print(fs.read_lines("audit.log")!)
```

#### 같이 보기

[`fs.write`](#fn-fs-write), [`fs.open`](#fn-fs-open)(`'append` 모드)

:::

## 디렉터리 살펴보기 {#browse}

:::fn fs.list(dir, full = false)

디렉터리 바로 안의 이름들을 돌려준다.

#### 설명

`dir` 바로 안에 있는 파일과 디렉터리의 이름을 이름의 바이트 순서(영문은 알파벳순, 대문자가 소문자보다 앞)로 돌려준다. 하위 디렉터리 안으로 내려가지 않는다(그것은 [`fs.walk`](#fn-fs-walk)). `.`으로 시작하는 숨은 파일도 들어 있고, `.`과 `..`은 들어 있지 않다. 디렉터리인지 파일인지는 [`fs.stat`](#fn-fs-stat)으로 알아본다.

`full: true`면 이름 대신 `dir`을 앞에 붙인 경로를 준다(정리된 꼴이라 `"./logs"`는 `"logs/…"`가 된다). 받은 값을 곧바로 다른 `fs` 함수에 넘길 때 편하다.

#### 매개변수

- `dir`: 디렉터리 경로.
- `full`: `true`면 경로, `false`(기본)면 이름만.

#### 돌려주는 값

`Ok(문자열 리스트)`.

#### 에러

- `Err({kind: 'not_found})`: 디렉터리가 없다.
- `Err({kind: 'io})`: 디렉터리가 아니다.

#### 예제

**이름과 경로**

```fung run
import fs

fs.mkdir("logs")!
fs.write("logs/b.log", "")!
fs.write("logs/a.log", "")!
fs.write("logs/.keep", "")!
print(fs.list("logs")!)
print(fs.list("logs", full: true)!)
```

**디렉터리만 고르기**

```fung run
import fs

fs.mkdir("site/assets")!
fs.write("site/index.html", "")!
dirs = fs.list("site", full: true)! |> filter(p => fs.stat(p)!.is_dir)
print(dirs)
```

#### 같이 보기

[`fs.walk`](#fn-fs-walk), [`fs.glob`](#fn-fs-glob), [`path.join`](path.html#fn-path-join)

:::

:::fn fs.glob(pattern)

와일드카드 패턴에 맞는 경로들을 돌려준다.

#### 설명

셸의 `ls *.log`처럼 패턴으로 파일을 고른다. 패턴은 `/`로 나눈 단계마다 맞추고, 결과는 디렉터리마다 이름의 바이트 순서다. 맞는 것이 없으면 빈 리스트다(셸처럼 패턴 그대로를 돌려주지 않는다).

<table>
  <thead><tr><th>기호</th><th>뜻</th></tr></thead>
  <tbody>
    <tr><td><code>*</code></td><td><code>/</code>가 아닌 아무 글자 0개 이상</td></tr>
    <tr><td><code>?</code></td><td><code>/</code>가 아닌 아무 글자 하나</td></tr>
    <tr><td><code>[abc]</code> <code>[a-z]</code></td><td>괄호 안의 글자 하나. <code>-</code>는 범위</td></tr>
    <tr><td><code>[^abc]</code></td><td>괄호 안에 없는 글자 하나</td></tr>
    <tr><td><code>\</code></td><td>다음 글자를 그대로(<code>\*</code>는 별표 글자)</td></tr>
  </tbody>
</table>

셸과 다른 점이 셋 있다. `*`는 `.`으로 시작하는 숨은 파일에도 맞는다. `**`는 특별하지 않아서 `*`와 같다(하위 디렉터리까지 찾으려면 [`fs.walk`](#fn-fs-walk)와 `filter`를 쓴다). `!`는 보통 글자다(여집합은 `[^…]`). 읽지 못하는 디렉터리는 조용히 건너뛴다.

#### 매개변수

- `pattern`: 패턴 문자열. 상대 패턴이면 현재 디렉터리 기준이다.

#### 돌려주는 값

`Ok(경로 리스트)`. 경로는 패턴과 같은 꼴(상대 패턴이면 상대 경로)이다.

#### 에러

- `Err({kind: 'invalid})`: 패턴이 잘못되었다(``bad glob pattern `[a`: syntax error in pattern``). 파일을 읽기 전에 검사한다.

#### 예제

**확장자로 고르기**

```fung run
import fs

fs.mkdir("logs/old")!
each(["a.log", "b.log", "notes.txt", ".hidden.log", "old/c.log"], n => fs.write("logs/${n}", "")!)
print(fs.glob("logs/*.log")!)
print(fs.glob("logs/*/*.log")!)
print(fs.glob("logs/[ab].*")!)
```

**잘못된 패턴**

```fung run
import fs

print(fs.glob("logs/[a"))
```

#### 같이 보기

[`fs.walk`](#fn-fs-walk), [`fs.list`](#fn-fs-list), [`regex.is_match`](regex.html#fn-regex-is_match)

:::

:::fn fs.walk(dir)

디렉터리 아래의 모든 파일 경로를 하위 디렉터리까지 돌려준다.

#### 설명

`dir`에서 시작해 하위 디렉터리로 끝까지 내려가며 **파일**의 경로만 모은다(디렉터리 자체는 들어 있지 않다). 경로는 `dir`을 앞에 붙인 정리된 꼴이고, 디렉터리마다 이름의 바이트 순서로 깊이 우선(한 디렉터리를 다 본 뒤 다음으로) 나온다. 셸의 `find dir -type f`와 비슷하다. 도중에 읽지 못하는 디렉터리가 있으면 그 경로로 `Err`다.

#### 매개변수

- `dir`: 시작할 디렉터리.

#### 돌려주는 값

`Ok(경로 리스트)`.

#### 에러

- `Err({kind: 'not_found})`: `dir`이 없다.
- `Err({kind: 'permission})`: 아래의 어느 디렉터리를 읽을 수 없다.

#### 예제

**하위 디렉터리까지**

```fung run
import fs

fs.mkdir("proj/src/util")!
fs.write("proj/README", "")!
fs.write("proj/src/main.fung", "")!
fs.write("proj/src/util/str.fung", "")!
print(fs.walk("proj")!)
```

**재귀로 `.fung` 파일 세기**(`**` 대신)

```fung run
import fs

fs.mkdir("proj/src/util")!
fs.write("proj/src/main.fung", "")!
fs.write("proj/src/util/str.fung", "")!
fs.write("proj/notes.txt", "")!
n = fs.walk("proj")! |> filter(p => ends_with(p, ".fung")) |> len
print(n, "fung files")
```

#### 같이 보기

[`fs.glob`](#fn-fs-glob), [`fs.list`](#fn-fs-list), [`fs.stat`](#fn-fs-stat)

:::

:::fn fs.exists(path)

경로에 무엇이 있는지 `true`/`false`로 알려 준다.

#### 설명

파일이든 디렉터리든 있으면 `true`. 결과값이 아니라 그냥 참거짓이므로 `!` 없이 쓴다. 이유(없는지, 권한이 없는지)가 필요하면 [`fs.stat`](#fn-fs-stat)을 쓴다.

"있으면 읽는다"는 `fs.exists`로 먼저 묻기보다 [`fs.read`](#fn-fs-read)를 바로 부르고 `'not_found`를 `match`하는 편이 낫다. 묻고 읽는 사이에 다른 프로그램이 파일을 지울 수 있기 때문이다.

#### 매개변수

- `path`: 확인할 경로.

#### 돌려주는 값

`true` 또는 `false`. 빈 경로는 `false`.

#### 예제

```fung run
import fs

fs.write("lock", "")!
print(fs.exists("lock"), fs.exists("."), fs.exists("nope"))
```

#### 같이 보기

[`fs.stat`](#fn-fs-stat)

:::

:::fn fs.stat(path)

경로의 이름, 크기, 디렉터리 여부, 고친 시각을 알려 준다.

#### 설명

`{name, size, is_dir, mtime}` 맵을 돌려준다. `name`은 경로의 마지막 요소, `size`는 바이트 수(글자 수가 아니다: 한글 한 글자는 UTF-8로 3바이트), `is_dir`은 디렉터리면 `true`, `mtime`은 마지막으로 고친 시각으로 `time.now()`와 같은 지역 시간대의 [`time`](time.html) 값이다(파일 시스템이 시각을 주지 않으면 `nil`). 디렉터리의 `size`는 운영체제가 정한 값이라 내용의 크기와 관계가 없다.

#### 매개변수

- `path`: 알아볼 경로.

#### 돌려주는 값

`Ok({name, size, is_dir, mtime})`.

#### 에러

- `Err({kind: 'not_found})`: 경로가 없다.

#### 예제

**바이트와 글자**

```fung run
import fs

fs.write("tuna.txt", "참치\n")!
st = fs.stat("tuna.txt")!
print(st.name, st.size, st.is_dir, len(fs.read("tuna.txt")!))
```

**큰 파일 찾기**

```fung run
import fs

fs.write("small.log", "x")!
fs.write("big.log", repeat("x", 5000))!
big = fs.glob("*.log")! |> filter(p => fs.stat(p)!.size > 1000)
print(big)
```

#### 같이 보기

[`fs.exists`](#fn-fs-exists), [`fs.list`](#fn-fs-list)

:::

## 만들고 지우고 옮기기 {#change}

이 절의 함수는 디스크를 바꾼다. 특히 지우는 함수는 되돌릴 수 없으니(휴지통을 거치지 않는다) 경로를 계산해서 넘길 때 조심한다.

:::fn fs.mkdir(path)

디렉터리를 만든다. 위의 디렉터리도 없으면 함께 만든다.

#### 설명

셸의 `mkdir -p`와 같다. `a/b/c`를 만들 때 `a`와 `a/b`가 없으면 함께 만든다. 이미 디렉터리가 있으면 아무 일도 하지 않고 성공한다. 그래서 "있든 없든 이 디렉터리가 있게 하라"는 뜻으로 그냥 부르면 된다.

#### 매개변수

- `path`: 만들 디렉터리.

#### 돌려주는 값

`Ok(nil)`.

#### 에러

- `Err({kind: 'exists})`: 그 자리에 디렉터리가 아닌 파일이 있다.
- `Err({kind: 'io})`: 중간 단계가 파일이다(``not a directory``).
- `Err({kind: 'not_found})`: 빈 경로 `""`.
- `Err({kind: 'permission})`: 만들 권한이 없다.

#### 예제

**여러 번 불러도 된다**

```fung run
import fs

print(fs.mkdir("backup/2026/10"))
print(fs.mkdir("backup/2026/10"))
print(fs.stat("backup/2026/10")!.is_dir)
```

**파일이 막고 있으면**

```fung run
import fs

fs.write("data", "")!
print(fs.mkdir("data"))
print(fs.mkdir("data/cache"))
```

#### 같이 보기

[`fs.remove_all`](#fn-fs-remove_all), [`fs.with_temp_dir`](#fn-fs-with_temp_dir)

:::

:::fn fs.remove(path)

파일 하나나 빈 디렉터리 하나를 지운다.

#### 설명

셸의 `rm`(파일)이나 `rmdir`(빈 디렉터리)과 같다. 심볼릭 링크는 링크만 지운다. 비지 않은 디렉터리는 지우지 않고 `Err`를 돌려준다. 실수로 많이 지우는 일을 막는 안전장치다. 통째로 지우려면 [`fs.remove_all`](#fn-fs-remove_all).

prelude의 [`remove`](../prelude/maps.html#fn-remove)(맵에서 키 빼기)와는 다른 함수다.

#### 매개변수

- `path`: 지울 경로.

#### 돌려주는 값

`Ok(nil)`.

#### 에러

- `Err({kind: 'not_found})`: 경로가 없다.
- `Err({kind: 'exists})`: 비지 않은 디렉터리다(``directory not empty``).

#### 예제

```fung run
import fs

fs.mkdir("tmp")!
fs.write("tmp/a", "")!
print(fs.remove("tmp"))
print(fs.remove("tmp/a"), fs.remove("tmp"))
print(fs.exists("tmp"))
```

#### 같이 보기

[`fs.remove_all`](#fn-fs-remove_all)

:::

:::fn fs.remove_all(path)

디렉터리를 안의 것까지 통째로 지운다.

#### 설명

셸의 `rm -rf`와 같다. 파일을 넘기면 그 파일만 지운다. 없는 경로와 빈 경로 `""`에는 아무것도 하지 않고 성공한다(이미 "없는" 상태이므로). 쓴 것보다 많이 지우게 되는 경로, 곧 루트 `/`와 마지막 요소가 `.`이나 `..`인 경로(`"."`, `"logs/.."`)는 거절한다. 링크는 `link/`처럼 써도 링크만 지운다.

#### 매개변수

- `path`: 지울 경로.

#### 돌려주는 값

`Ok(nil)`.

#### 에러

- `Err({kind: 'io})`: 거절한 경로(``invalid argument``).
- `Err({kind: 'permission})`: 안의 무엇을 지울 권한이 없다.

#### 예제

**통째로 지우기**

```fung run
import fs

fs.mkdir("build/out")!
fs.write("build/out/app", "")!
print(fs.remove_all("build"), fs.exists("build"))
print(fs.remove_all("build"))
```

**위험한 경로는 거절한다**

```fung run
import fs

print(fs.remove_all("."))
print(fs.remove_all("/"))
```

#### 같이 보기

[`fs.remove`](#fn-fs-remove), [`fs.with_temp_dir`](#fn-fs-with_temp_dir)

:::

:::fn fs.move(from, to, overwrite = false)

파일이나 디렉터리의 이름을 바꾸거나 다른 곳으로 옮긴다.

#### 설명

셸의 `mv`와 비슷하지만 **이미 있는 것을 덮어쓰지 않는다**: `to`가 있으면 `Err({kind: 'exists})`다(셸의 `mv -n`과 같고, 실패를 알린다). 파일을 덮어쓰려면 `overwrite: true`를 준다. 새 내용을 임시 파일에 다 쓴 뒤 바꿔치기할 때 쓴다. 이름 바꾸기는 한 번에 일어나서, 그 사이에 멈춰도 원래 파일은 옛 내용 그대로다. 확인과 옮기기가 운영체제의 호출 하나라서(Linux `renameat2`, macOS `renamex_np`), 확인한 뒤 옮기기 전에 다른 프로그램이 같은 이름을 만들어도 덮어쓰지 않는다. 이미 있는 **디렉터리**는 `overwrite`를 줘도 덮지 않는다(`mv a dir`처럼 그 안으로 옮기지도 않는다). 디렉터리 안으로 옮기려면 대상 경로에 이름까지 적는다: `fs.move("a.txt", "dir/a.txt")`. 같은 디렉터리에서 대소문자만 바꾸는 이름 바꾸기도 된다.

#### 매개변수

- `from`: 옮길 경로.
- `to`: 새 경로.
- `overwrite`: `true`면 `to`에 있는 파일을 바꾼다. 기본은 `false`.

#### 돌려주는 값

`Ok(nil)`.

#### 에러

- `Err({kind: 'not_found})`: `from`이 없거나 `to`의 위 디렉터리가 없다.
- `Err({kind: 'exists})`: `to`가 이미 있다(``cannot move `a.txt` to `b.txt`: file exists``). 디렉터리면 `overwrite: true`여도 그렇다.

#### 예제

**이름 바꾸기**

```fung run
import fs

fs.write("report.tmp", "done")!
fs.move("report.tmp", "report.txt")!
print(fs.list(".")! |> filter(n => starts_with(n, "report")))
```

**덮어쓰지 않는다, 바꿔치기는 `overwrite`로**

```fung run
import fs

fs.write("app.conf", "port=80\n")!
fs.write("app.conf.tmp", "port=8080\n")!
print(fs.move("app.conf.tmp", "app.conf"))
fs.move("app.conf.tmp", "app.conf", overwrite: true)!
print(fs.read("app.conf")!)
```

**디렉터리 안으로**

```fung run
import fs
import path

fs.mkdir("archive")!
fs.write("a.log", "")!
print(fs.move("a.log", "archive"))
print(fs.move("a.log", path.join("archive", "a.log")), fs.list("archive")!)
```

#### 같이 보기

[`fs.copy`](#fn-fs-copy), [`path.join`](path.html#fn-path-join)

:::

:::fn fs.copy(from, to)

파일 하나를 복사한다.

#### 설명

`from`의 내용과 권한(실행 권한 등)을 `to`에 복사한다. `to`에 파일이 있으면 덮어쓴다. **파일만** 복사한다: 디렉터리를 복사하거나 대상으로 디렉터리를 주면 `Err`다. 디렉터리 전체는 [`fs.walk`](#fn-fs-walk)로 돌며 하나씩 복사한다.

#### 매개변수

- `from`: 원본 파일.
- `to`: 대상 파일 경로(이름까지).

#### 돌려주는 값

`Ok(nil)`.

#### 에러

- `Err({kind: 'not_found})`: 원본이 없다.
- `Err({kind: 'io})`: 원본이나 대상이 디렉터리다(``is a directory``).

#### 예제

**고치기 전에 백업**

```file nginx.conf
worker_processes 2;
```

```fung run
import fs

fs.copy("nginx.conf", "nginx.conf.bak")!
fs.write("nginx.conf", "worker_processes 4;\n")!
print(trim(fs.read("nginx.conf.bak")!), "->", trim(fs.read("nginx.conf")!))
```

**디렉터리는 안 된다**

```fung run
import fs

fs.mkdir("dir")!
print(fs.copy("dir", "dir2"))
```

#### 같이 보기

[`fs.move`](#fn-fs-move), [`fs.walk`](#fn-fs-walk)

:::

## 파일 핸들 {#handles}

[`fs.read`](#fn-fs-read)와 [`fs.write`](#fn-fs-write)는 파일을 한 번에 다룬다. 아주 큰 로그를 한 줄씩 읽거나, 결과를 조금씩 써 나갈 때는 파일을 **열어서**(open) 받은 **핸들**(handle, 열린 파일을 가리키는 값)로 읽고 쓴 뒤 **닫는다**(close). 핸들로는 prelude의 [`read_line`](../prelude/io.html#fn-read_line)·[`read_lines`](../prelude/io.html#fn-read_lines)로 읽고 [`fs.write_line`](#fn-fs-write_line)으로 쓴다.

열었으면 반드시 닫아야 한다. 쓴 내용은 64 KiB 버퍼에 모였다가 버퍼가 차거나 닫을 때 디스크에 닿기 때문이다. 직접 `open`·`close`를 짝짓기보다 **[`fs.with_open`](#fn-fs-with_open)을 권한다**. 중간에 에러가 나도 닫아 준다.

핸들은 태스크 사이에 나눠 써도 된다. `read_line`·`write_line`·`close` 같은 연산 하나하나가 차례로 처리되어 줄이 찢어지거나 섞이지 않는다. 다만 어느 태스크가 어느 줄을 받는지는 정해지지 않는다.

:::fn fs.open(path, mode = 'read, perm = nil)

파일을 열어 핸들을 돌려준다.

#### 설명

`mode`에 따라 읽기, 새로 쓰기, 덧붙이기로 연다. 출력하면 `<file "경로">`, `type_of`는 `'file`이다. 다 쓴 핸들은 [`fs.close`](#fn-fs-close)로 닫는다. 대부분은 [`fs.with_open`](#fn-fs-with_open)이 더 안전하다.

**FIFO**(`mkfifo`로 만든 이름 있는 파이프)는 기다림을 이벤트 루프에서 한다. 그래서 [`timeout`](../prelude/concurrency.html#fn-timeout)과 취소가 곧바로 끊고, 끊긴 [`read_line`](../prelude/io.html#fn-read_line)이 받은 바이트는 다음 읽기에 남는다. 읽기로 열면 곧바로 열리고 첫 읽기가 쓰는 쪽을 기다린다. 쓰기로 열면 읽는 쪽이 열 때까지 기다린다. FIFO에 쓴 줄은 버퍼에 모으지 않고 곧바로 보낸다.

<table>
  <thead><tr><th><code>mode</code></th><th>뜻</th></tr></thead>
  <tbody>
    <tr><td><code>'read</code></td><td>읽기(기본). 파일이 있어야 한다.</td></tr>
    <tr><td><code>'write</code></td><td>새로 쓰기. 없으면 만들고, 있으면 내용을 지운다.</td></tr>
    <tr><td><code>'append</code></td><td>덧붙이기. 없으면 만들고, 끝에 이어 쓴다.</td></tr>
  </tbody>
</table>

#### 매개변수

- `path`: 열 파일.
- `mode`: `'read`, `'write`, `'append` 중 하나.
- `perm`: `'write`·`'append`일 때 파일의 권한. [`fs.write`](#fn-fs-write)와 같다. `'read`에 주면 런타임 에러 E0403.

#### 돌려주는 값

`Ok(file)`.

#### 에러

- `Err({kind: 'not_found})`: `'read`로 여는데 파일이 없다, 또는 위의 디렉터리가 없다.
- `Err({kind: 'permission})`: 권한이 없다.
- `mode`가 셋 중 하나가 아니거나 `perm`이 맞지 않으면 런타임 에러 E0403.

#### 예제

**한 줄씩 읽기**

```file servers.txt
web-1
web-2
db-1
```

```fung run
import fs

h = fs.open("servers.txt")!
print(h, type_of(h))
print(read_line(h)!, read_line(h)!)
print(read_lines(h)!)
print(read_line(h))
fs.close(h)!
```

파일 끝에서 `read_line`은 `Ok(nil)`이다.

**모르는 모드**

```fung run exit=1 error=E0403
import fs

fs.open("servers.txt", 'readwrite)
```

#### 같이 보기

[`fs.with_open`](#fn-fs-with_open), [`fs.close`](#fn-fs-close), [`read_line`](../prelude/io.html#fn-read_line)

:::

:::fn fs.close(f)

파일 핸들을 닫는다.

#### 설명

버퍼에 남은 쓰기를 디스크에 비우고 핸들을 닫는다. 비우기나 닫기가 실패하면 그 에러를 돌려준다(NFS처럼 닫을 때에야 쓰기를 보내는 파일 시스템에서는 디스크 가득 참이 여기서 나온다). 닫힌 핸들로 읽고 쓰면 `Err({kind: 'closed})`이고, 한 번 더 닫아도 같다.

닫지 않은 핸들은 그 값의 마지막 사본이 사라지면 런타임의 닫기 스레드가 비우고 닫는다. 그래서 데이터가 파일에 닿는 때는 정해져 있지 않다(버린 바로 뒤의 `fs.read`가 그 줄을 못 볼 수 있다). 프로그램이 끝날 때는 모두 닫힌 뒤에 끝나므로 데이터를 잃지는 않는다. 그 비우기나 닫기가 실패하면 돌려받을 곳이 없으므로 표준 에러에 [E0810](../errors.html#E0810)을 쓰고 종료 코드가 1이 된다. 그래서 쓴 핸들은 직접 닫거나 [`fs.with_open`](#fn-fs-with_open)을 쓴다.

prelude의 [`close`](../prelude/concurrency.html#fn-close)(채널 닫기)와는 다른 함수다.

#### 매개변수

- `f`: `fs.open`이 준 핸들.

#### 돌려주는 값

`Ok(nil)`.

#### 에러

- `Err({kind: 'closed})`: 이미 닫혔다.
- `Err({kind: 'io})`: 버퍼를 비우다 실패했다(디스크가 참 등).

#### 예제

```fung run
import fs

h = fs.open("out.txt", 'write)!
fs.write_line(h, "one")!
print(fs.close(h))
print(fs.close(h))
print(fs.write_line(h, "two"))
```

#### 같이 보기

[`fs.open`](#fn-fs-open), [`fs.with_open`](#fn-fs-with_open)

:::

:::fn fs.write_line(f, line)

핸들에 한 줄을 쓴다.

#### 설명

`line` 뒤에 줄바꿈 `\n`을 붙여 쓴다. `line`은 문자열이어야 한다(숫자는 `str(n)`이나 `"${n}"`으로). 쓰기는 버퍼에 모였다가 닫을 때 디스크에 닿는다.

#### 매개변수

- `f`: `'write`나 `'append`로 연 핸들.
- `line`: 쓸 문자열(줄바꿈 없이).

#### 돌려주는 값

`Ok(nil)`.

#### 에러

- `Err({kind: 'closed})`: 닫힌 핸들이다.
- `Err({kind: 'invalid})`: `'read`로 연 핸들이다(``file `a.txt` is not open for writing``).
- `line`이 문자열이 아니면 런타임 에러 E0403.

#### 예제

**결과를 조금씩 쓰기**

```fung run
import fs

fs.with_open("squares.txt", fn(h):
  each(range(1, 4), n => fs.write_line(h, "${n} ${n * n}")!)
;;, mode: 'write)
print(fs.read_lines("squares.txt")!)
```

#### 같이 보기

[`fs.append`](#fn-fs-append), [`fs.with_open`](#fn-fs-with_open)

:::

:::fn fs.with_open(path, f, mode = 'read, perm = nil) fung

파일을 열어 함수에 넘기고, 끝나면 무슨 일이 있어도 닫는다.

#### 설명

`path`를 `mode`로 열고 `f(핸들)`을 부른 뒤, `f`가 값을 돌려주든 `?`로 `Err`를 돌려주든 런타임 에러로 멈추든 핸들을 닫는다. 결과는 `f`가 돌려준 값 그대로다. 열지 못하면 `f`를 부르지 않고 그 `Err`를 돌려준다. prelude의 [`with`](../prelude/program.html#fn-with)로 만들었다. 다른 언어의 `defer`나 `with open(...)` 블록 대신 쓰는 모양이다.

`f` 안에서 `?`를 쓰면 `f`(람다)에서 `Err`를 돌려주는 것이므로, 핸들이 닫힌 뒤 그 `Err`가 `with_open`의 결과가 된다. `f`가 스스로 핸들을 닫아도 실패가 아니다. 닫기가 실패하면(버퍼를 비우지 못함) 런타임 에러다. 핸들을 `f` 밖으로 돌려줘도 막지 않지만, 그때는 이미 닫혀 있어 쓰는 순간 `'closed`다.

#### 매개변수

- `path`: 열 파일.
- `f`: 핸들을 받는 함수. 그 결과가 `with_open`의 결과가 된다.
- `mode`: [`fs.open`](#fn-fs-open)과 같다.
- `perm`: [`fs.open`](#fn-fs-open)과 같다.

#### 돌려주는 값

`f`의 결과, 또는 열지 못했을 때의 `Err`.

#### 에러

- 열 때의 `Err`([`fs.open`](#fn-fs-open)과 같다).
- `f`가 돌려준 `Err`(그대로 전달).

#### 예제

**첫 줄만 읽기**

```file notes.txt
first line
second line
```

```fung run
import fs

first = fs.with_open("notes.txt", h => read_line(h))!
print(first)
```

**`?`로 중간에 나가도 닫힌다**

```fung run
import fs

r = fs.with_open("log.txt", fn(h):
  fs.write_line(h, "first")?
  fs.write_line(h, "second")?
  Ok("wrote 2 lines")
;;, mode: 'write)
print(r, fs.read_lines("log.txt")!)
print(fs.with_open("missing.txt", h => read_line(h)))
```

**밖으로 꺼낸 핸들은 닫혀 있다**

```fung run
import fs

fs.write("a.txt", "x\n")!
h = fs.with_open("a.txt", h => h)
print(read_line(h))
```

#### 같이 보기

[`fs.open`](#fn-fs-open), [`with`](../prelude/program.html#fn-with), [`fs.with_temp_dir`](#fn-fs-with_temp_dir)

:::

:::fn fs.with_temp_dir(f) fung

임시 디렉터리를 만들어 함수에 넘기고, 끝나면 통째로 지운다.

#### 설명

운영체제의 임시 디렉터리(`TMPDIR`, 없으면 `/tmp`) 아래에 이름이 겹치지 않는 새 디렉터리(`fung-…`)를 만들고 `f(경로)`를 부른다. `f`가 어떻게 끝나든(값, `Err`, 런타임 에러) 디렉터리를 안의 것까지 지운다. 중간 파일을 잠시 두거나, 테스트에서 깨끗한 작업 공간이 필요할 때 쓴다. 결과는 `f`의 결과이고, 지우기가 실패하면 런타임 에러다.

#### 매개변수

- `f`: 디렉터리 경로(문자열)를 받는 함수.

#### 돌려주는 값

`f`의 결과, 또는 디렉터리를 만들지 못했을 때의 `Err`.

#### 예제

**작업 후 사라진다**

```fung run
import fs
import path

kept = fs.with_temp_dir(fn(dir):
  p = path.join(dir, "part.txt")
  fs.write(p, "partial")!
  print(fs.read(p)!)
  dir
;;)
print(fs.exists(kept))
```

**경로는 실행마다 다르다**

```fung run hide-output
import fs

print(fs.with_temp_dir(dir => dir))
```

#### 같이 보기

[`fs.with_open`](#fn-fs-with_open), [`fs.remove_all`](#fn-fs-remove_all), [`with`](../prelude/program.html#fn-with)

:::

같은 주제를 처음부터 배우려면 [11장 파일·외부 명령·CLI](../../learn/11-files-and-commands.html)를, 실제 작업 예는 [파일 이름 일괄 변경](../../recipes/rename.html)과 [큰 파일 찾기](../../recipes/find-big-files.html)를 본다.
