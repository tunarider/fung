---
lede: <code>import regex</code>. 정규식으로 문자열에서 모양을 찾고, 꺼내고, 바꾸고, 나눈다. 문법은 RE2 계열이라 역참조와 전후방 탐색이 없는 대신 항상 선형 시간에 돈다.
description: fung 모듈 regex — 정규식으로 찾기, 꺼내기, 바꾸기, 나누기(is_match, find, find_all, replace, split).
---
# regex — 정규식

## 정규식이란 {#concepts}

**정규식**(regular expression, regex)은 "문자열의 모양"을 적는 작은 언어다. "숫자가 하나 이상", "`ERROR`로 시작하는 줄", "`키=값` 꼴" 같은 조건을 짧은 패턴으로 쓴다. `grep`, `sed`, 편집기의 찾기 기능이 쓰는 그것이다. 고정된 글자를 찾을 때는 prelude의 [`contains`](../prelude/collections.html#fn-contains)·[`split`](../prelude/strings.html#fn-split)·[`replace`](../prelude/strings.html#fn-replace)가 더 간단하고 빠르다. 모양이 들쭉날쭉할 때 정규식을 쓴다.

자주 쓰는 문법:

<table>
  <thead><tr><th>패턴</th><th>뜻</th></tr></thead>
  <tbody>
    <tr><td><code>abc</code></td><td>글자 그대로</td></tr>
    <tr><td><code>.</code></td><td>줄바꿈이 아닌 아무 글자 하나</td></tr>
    <tr><td><code>\d</code> <code>\w</code> <code>\s</code></td><td>숫자, 단어 글자(글자·숫자·<code>_</code>), 공백 하나. 대문자(<code>\D</code> …)는 그 반대</td></tr>
    <tr><td><code>[abc]</code> <code>[a-z]</code> <code>[^0-9]</code></td><td>괄호 안의 글자 하나, 범위, 여집합</td></tr>
    <tr><td><code>x*</code> <code>x+</code> <code>x?</code></td><td>0번 이상, 1번 이상, 0번이나 1번</td></tr>
    <tr><td><code>x{3}</code> <code>x{2,5}</code></td><td>정확히 3번, 2~5번</td></tr>
    <tr><td><code>^</code> <code>$</code></td><td>문자열의 처음과 끝(<code>(?m)</code>을 붙이면 줄의 처음과 끝)</td></tr>
    <tr><td><code>\b</code></td><td>단어의 경계</td></tr>
    <tr><td><code>a|b</code></td><td><code>a</code> 또는 <code>b</code></td></tr>
    <tr><td><code>(…)</code></td><td>캡처 그룹: 맞은 부분을 따로 꺼낸다</td></tr>
    <tr><td><code>(?P&lt;name&gt;…)</code></td><td>이름 있는 캡처 그룹</td></tr>
    <tr><td><code>(?:…)</code></td><td>꺼내지 않는 묶음</td></tr>
    <tr><td><code>(?i)</code></td><td>대소문자 무시</td></tr>
    <tr><td><code>\.</code> <code>\(</code></td><td>특수 글자를 글자 그대로</td></tr>
  </tbody>
</table>

문법은 Rust `regex` 크레이트의 것(RE2 계열)이다. 역참조(`\1`)와 전후방 탐색(`(?=…)`, `(?<=…)`)이 없다. 그 대신 어떤 패턴과 입력이든 입력 길이에 비례하는 시간에 끝나므로, 외부 입력에 패턴을 돌려도 "끝나지 않는 정규식" 문제가 없다. `\d`·`\w`·`\s`·`\b`는 유니코드를 알아본다(아랍 숫자 `٣`도 `\d`다). ASCII만 원하면 `(?-u:\d)`처럼 쓴다. 그룹 이름은 ASCII 글자·숫자·`_`이고 겹치면 안 된다.

**패턴은 원시 문자열 `` `…` ``로 쓴다.** 보통 문자열 `"…"`에서는 `\d`가 모르는 이스케이프라 컴파일 에러이고, `${name}`은 보간이 되어 버린다. 원시 문자열은 쓴 글자 그대로다.

**위치는 글자 단위다.** 매치의 `start`·`end`는 바이트가 아니라 글자(유니코드 코드 포인트) 인덱스이므로 `s[m.start..m.end]`가 그대로 맞는다.

잘못된 패턴은 프로그램의 잘못이므로 런타임 에러 E0515다(``invalid pattern `a(`: unclosed group``). 패턴의 중첩은 250단계, 컴파일한 크기는 128 MiB까지다. 컴파일한 패턴은 캐시되므로, 같은 패턴을 반복문에서 여러 번 써도 다시 컴파일하지 않는다. 따로 "컴파일된 정규식" 값을 만들 필요가 없다.

### 매치 맵 {#match-map}

`regex.find`와 `regex.find_all`은 맞은 곳 하나를 다음 맵으로 준다.

<table>
  <thead><tr><th>키</th><th>값</th></tr></thead>
  <tbody>
    <tr><td><code>text</code></td><td>맞은 부분 전체(문자열)</td></tr>
    <tr><td><code>start</code> <code>end</code></td><td>맞은 부분의 글자 인덱스(<code>end</code>는 마지막 글자 다음)</td></tr>
    <tr><td><code>groups</code></td><td>캡처 그룹의 리스트(전체 매치는 빼고, 순서대로). 잡히지 않은 그룹은 <code>nil</code></td></tr>
    <tr><td><code>named</code></td><td>이름 있는 그룹의 맵(이름 → 문자열 또는 <code>nil</code>)</td></tr>
  </tbody>
</table>

## 함수 {#functions}

:::fn regex.is_match(s, pat)

`s`의 어딘가에 패턴이 맞는지 알려 준다.

#### 설명

패턴이 `s`의 어느 부분에든 맞으면 `true`다. 문자열 전체가 맞아야 한다면 `^…$`로 감싼다. 맞은 내용이 필요 없을 때 [`regex.find`](#fn-regex-find)보다 간단하다. 이름이 `match`가 아닌 것은 `match`가 예약어이기 때문이다.

#### 매개변수

- `s`: 검사할 문자열.
- `pat`: 패턴.

#### 돌려주는 값

`true` 또는 `false`.

#### 에러

- 런타임 에러 E0515: 패턴이 잘못되었다.
- 런타임 에러 E0403: `s`나 `pat`이 문자열이 아니다.

#### 예제

**로그 줄 고르기**

```fung run
import regex

lines = ["INFO ok", "ERROR disk full", "error: retry", "WARN slow"]
print(lines |> filter(l => regex.is_match(l, `(?i)^error`)))
```

**전체 모양 검사**

```fung run
import regex

fn is_host(s) = regex.is_match(s, `^[a-z][a-z0-9-]*-\d+$`)

print(is_host("web-1"), is_host("db-12"), is_host("Web-1"), is_host("web-1.example.com"))
```

**잘못된 패턴**

```fung run exit=1 error=E0515
import regex

print(regex.is_match("a", `a(`))
```

#### 같이 보기

[`regex.find`](#fn-regex-find), [`contains`](../prelude/collections.html#fn-contains), [`starts_with`](../prelude/strings.html#fn-starts_with)

:::

:::fn regex.find(s, pat)

처음 맞는 곳을 찾아 [매치 맵](#match-map)으로 돌려준다.

#### 설명

`s`에서 왼쪽부터 처음 맞는 곳 하나를 찾는다. 없으면 `nil`이다(실패가 아니라 "없음"이라 결과값이 아니다). 캡처 그룹으로 값을 꺼낼 때 쓴다. `nil`일 수 있으므로 `m.groups`를 읽기 전에 확인하거나 `match`로 나눈다.

#### 매개변수

- `s`: 찾을 대상 문자열.
- `pat`: 패턴.

#### 돌려주는 값

[매치 맵](#match-map) 또는 `nil`.

#### 에러

[`regex.is_match`](#fn-regex-is_match)와 같다.

#### 예제

**이름 있는 그룹으로 꺼내기**

```fung run
import regex

m = regex.find("listen port=8080 proto=tcp", `(?P<key>port)=(?P<val>\d+)`)
print(m.named.key, int(m.named.val) + 1)
print(m.text, m.start, m.end)
```

**없으면 `nil`**

```fung run
import regex

fn version(s) = match regex.find(s, `v(\d+)\.(\d+)`):
  nil => "unknown"
  m   => "major ${m.groups[0]}, minor ${m.groups[1]}"
;;

print(version("release v2.14 is out"))
print(version("nightly build"))
```

**글자 단위 위치와 잡히지 않은 그룹**

```fung run
import regex

m = regex.find("참치 42개", `(\d+)(개)?(마리)?`)
print(m)
print("참치 42개"[m.start..m.end])
```

#### 같이 보기

[`regex.find_all`](#fn-regex-find_all), [`regex.is_match`](#fn-regex-is_match)

:::

:::fn regex.find_all(s, pat)

겹치지 않는 모든 매치를 리스트로 돌려준다.

#### 설명

왼쪽부터 차례로 찾고, 한 매치가 끝난 자리에서 다음을 찾는다(겹치지 않는다). 앞 매치 바로 뒤의 빈 매치는 세지 않는다. 맞는 곳이 없으면 빈 리스트다. 각 원소는 [매치 맵](#match-map)이다.

#### 매개변수

- `s`: 찾을 대상 문자열.
- `pat`: 패턴.

#### 돌려주는 값

매치 맵의 리스트.

#### 에러

[`regex.is_match`](#fn-regex-is_match)와 같다.

#### 예제

**숫자 모두 꺼내기**

```fung run
import regex

ms = regex.find_all("took 12ms, 340ms and 7ms", `\d+`)
print(ms |> map(m => int(m.text)) |> sum)
```

**`키=값` 쌍을 맵으로**

```fung run
import regex

line = "user=kim role=admin  region=eu"
pairs = regex.find_all(line, `(?P<k>\w+)=(?P<v>\S+)`)
print(pairs |> fold({}, (acc, m) => put(acc, m.named.k, m.named.v)))
```

**로그 줄 나누기**

```fung run
import regex

log = ["GET /a 200", "POST /b 500", "junk", "GET /c 404"]
rows = log |> map(l => regex.find(l, `^(\w+) (\S+) (\d{3})$`)) |> filter(m => m != nil)
print(rows |> map(m => m.groups) |> filter(g => g[2] != "200"))
```

#### 같이 보기

[`regex.find`](#fn-regex-find), [`regex.split`](#fn-regex-split)

:::

:::fn regex.replace(s, pat, repl)

맞는 곳을 모두 바꾼 문자열을 돌려준다.

#### 설명

`s`에서 패턴이 맞는 곳을 **모두** `repl`로 바꾼다(첫 곳만이 아니다). 맞는 곳이 없으면 `s` 그대로다. `repl` 안에서 캡처 그룹을 다시 쓸 수 있다.

<table>
  <thead><tr><th><code>repl</code> 안의 표기</th><th>뜻</th></tr></thead>
  <tbody>
    <tr><td><code>$1</code> <code>$2</code></td><td>번호 그룹(1부터). <code>$0</code>은 전체 매치</td></tr>
    <tr><td><code>${name}</code></td><td>이름 있는 그룹</td></tr>
    <tr><td><code>${1}</code></td><td>번호 그룹, 뒤에 글자를 붙일 때</td></tr>
    <tr><td><code>$$</code></td><td><code>$</code> 글자 하나</td></tr>
  </tbody>
</table>

`$` 뒤의 이름은 가능한 한 길게 읽는다. 그래서 `$1x`는 "이름이 `1x`인 그룹"이고, 그런 그룹이 없으면 빈 문자열이 된다. 그룹 뒤에 글자를 붙이려면 `${1}x`로 쓴다. `${…}`를 쓰는 `repl`은 보간을 피하려고 원시 문자열 `` `…` ``로 쓴다.

#### 매개변수

- `s`: 대상 문자열.
- `pat`: 패턴.
- `repl`: 바꿀 문자열.

#### 돌려주는 값

바꾼 문자열.

#### 에러

[`regex.is_match`](#fn-regex-is_match)와 같다.

#### 예제

**날짜 꼴 바꾸기**

```fung run
import regex

print(regex.replace("2026-10-04, 2026-10-05", `(\d+)-(\d+)-(\d+)`, "$3/$2/$1"))
```

**비밀 가리기**

```fung run
import regex

line = "login user=kim password=hunter2 token=abc123"
print(regex.replace(line, `(?P<key>password|token)=\S+`, `${key}=***`))
```

**`$1x`와 `${1}x`, `$$`**

```fung run
import regex

print(regex.replace("ab", `(a)`, "$1x"))
print(regex.replace("ab", `(a)`, `${1}x`))
print(regex.replace("cost 5", `(\d+)`, "$$$1"))
```

:::box mistake 보통 문자열에 `${name}`

`regex.replace(s, pat, "<${w}>")`처럼 보통 문자열에 `${w}`를 쓰면 fung이 먼저 변수 `w`를 보간하려 해서 E0204(없는 이름)가 난다. 그룹을 가리키는 `${w}`는 원시 문자열 `` `<${w}>` ``에 쓴다.

:::

#### 같이 보기

[`replace`](../prelude/strings.html#fn-replace)(고정된 글자 바꾸기), [설정 파일 값 치환](../../recipes/replace-config.html)

:::

:::fn regex.split(s, pat)

맞는 곳을 경계로 문자열을 나눈다.

#### 설명

패턴이 맞는 곳마다 잘라 그 사이 조각들의 리스트를 돌려준다. prelude의 [`split`](../prelude/strings.html#fn-split)처럼 빈 조각도 남긴다(구분자가 맨 앞이나 맨 끝, 또는 연달아 있을 때). 문자열 맨 앞과 맨 끝의 **빈 매치**로는 자르지 않으므로 `regex.split("abc", "")`는 `["a", "b", "c"]`이고, 빈 문자열을 빈 패턴으로 나누면 `[]`다.

#### 매개변수

- `s`: 나눌 문자열.
- `pat`: 구분자 패턴.

#### 돌려주는 값

문자열 리스트.

#### 에러

[`regex.is_match`](#fn-regex-is_match)와 같다.

#### 예제

**여러 모양의 구분자**

```fung run
import regex

print(regex.split("web-1, web-2;db-1   cache-1", `[,;\s]+`))
```

**빈 조각**

```fung run
import regex

print(regex.split("a,,b,", `,`))
print(regex.split("  a  b ", `\s+`))
print(regex.split("abc", ""))
```

빈 조각이 필요 없으면 `filter(x => x != "")`로 걸러 낸다.

#### 같이 보기

[`split`](../prelude/strings.html#fn-split), [`lines`](../prelude/strings.html#fn-lines), [텍스트 응답을 정규식으로 쪼개기](../../recipes/regex-split.html)

:::

배우는 순서로는 [6장 문자열 다루기](../../learn/06-strings.html)를 본다.
