---
lede: 글자를 나누고, 잇고, 다듬고, 바꾼다. 문자열은 불변인 바이트 열이고, 인덱스와 길이는 바이트가 아니라 글자 단위다.
description: fung prelude 문자열: chars, split, join, lines, trim, trim_left, trim_right, upper, lower, replace, starts_with, index_of, ends_with, repeat, pad_left, pad_right, byte_len, bytes, from_bytes, is_utf8.
---
# 문자열

## 문자열이란 {#concepts}

**문자열**(string)은 글자를 순서대로 늘어놓은 값이다. 큰따옴표로 쓴다: `"web-1"`. 스크립트가 다루는 데이터의 대부분, 곧 파일 내용, 명령 출력, 로그 줄, 환경 변수, HTTP 응답은 처음에 문자열로 들어온다. 그래서 문자열을 쪼개고 다듬는 일이 스크립트의 절반쯤 된다.

fung의 문자열에 대해 알아 둘 것은 다섯이다.

- **불변**이다. `upper(s)`는 `s`를 바꾸지 않고 대문자로 된 새 문자열을 돌려준다.
- **글자 단위**다. 문자열은 UTF-8로 저장되지만 `len`, 인덱스 `s[0]`, 슬라이스 `s[1..3]`은 바이트가 아니라 글자(유니코드 코드 포인트)를 센다. 그래서 `"참치"[0]`은 `"참"`이고 `len("참치")`는 2다.
- **어떤 바이트든 담는다**. 문자열은 바이트 열이고, 글자 연산이 그것을 UTF-8로 읽는다. 파일, 명령 출력, 환경 변수, 인자는 UTF-8이 아니어도 바이트 그대로 들어오고 그대로 나간다. UTF-8에 속하지 않는 바이트는 그 바이트 하나가 한 글자다. 값으로 보일 때는 `"a\xff"`처럼 `\xHH`로 보이고, 그 모양을 리터럴로 그대로 쓸 수 있다([바이트](#bytes)).
- **잇기는 `+`, 끼워 넣기는 보간**이다. `"a" + "b"`는 양쪽이 문자열이어야 한다. 값을 끼워 넣을 때는 `"${name}: ${count}"`처럼 쓴다. 보간은 어떤 값이든 [`str`](convert.html#fn-str)과 같은 규칙으로 바꿔 넣는다.
- **문자열은 컬렉션 함수에 넣지 않는다.** `map("abc", f)`도 `take("abc", 2)`·`first("abc")`도 에러다(E0506). 앞뒤를 자르려면 슬라이스(`s[..2]`, `s[1..]`, `s[len(s) - 1]`)를, 글자 하나하나를 다루려면 [`chars`](#fn-chars)로 글자 리스트를 만든다.

```fung run
s = "  ERROR disk full  "
clean = trim(s)
print("[${clean}]", len(clean), clean[0..5], lower(clean))
print(s == "  ERROR disk full  ")
```

리터럴의 이스케이프(`\n`, `\t`, `\u{301}`), 여러 줄 문자열, 원시 문자열은 [어휘](../language/lexical.html)에, 패턴으로 찾고 바꾸는 일은 [`regex` 모듈](../modules/regex.html)에 있다.

## 나누기와 잇기 {#split-join}

:::fn split(s, sep)

`s`를 구분자 `sep`로 나눈 문자열의 리스트를 돌려준다.

#### 설명

`sep`가 나올 때마다 자른다. 구분자가 연달아 있거나 맨 앞·맨 뒤에 있으면 그 사이의 **빈 조각도 남긴다**(`split("a,,b", ",")`는 `["a", "", "b"]`). 그래서 CSV처럼 칸 수가 중요한 데이터를 잘 다룬다. 구분자가 없으면 원래 문자열 하나짜리 리스트이고, 빈 문자열을 나누면 `[""]`이다. 그래서 원소 수는 언제나 구분자 수 + 1이고 `join(split(s, sep), sep) == s`다. `sep`는 빈 문자열일 수 없다(E0403). 글자 단위로 나누려면 [`chars`](#fn-chars)를 쓴다.

공백이 여러 개 섞인 글을 낱말로 나눌 때는 빈 조각을 걸러 낸다: `split(s, " ") |> filter(w => w != "")`. 줄로 나눌 때는 `split(s, "\n")` 대신 [`lines`](#fn-lines)를 쓴다(마지막 줄바꿈과 `\r`을 처리해 준다). 정규식으로 나누려면 [`regex.split`](../modules/regex.html#fn-regex-split).

#### 매개변수

- `s`: 나눌 문자열.
- `sep`: 구분자 문자열. 여러 글자여도 되고, 비어 있으면 안 된다.

#### 돌려주는 값

문자열의 리스트. 언제나 원소가 하나 이상이다.

#### 에러

`s`나 `sep`가 문자열이 아니거나 `sep`가 빈 문자열이면 런타임 에러 E0403(``argument `sep` must be a non-empty str, got ""``).

#### 예제

**빈 조각도 남긴다**

```fung run
print(split("a,,b", ","), split(",a,", ","), split("abc", "x"), split("", ","))
```

**여러 글자 구분자, 글자 단위는 chars**

```fung run
print(split("a::b::c", "::"), chars("abc"))
```

**`키=값` 줄 읽기**

```fung run
line = "DB_HOST=db-1.internal"
[key, value] = split(line, "=")
print(key, "->", value)
```

**공백이 여러 개인 줄을 낱말로**

```fung run
line = "web-1    up   12d"
print(split(line, " ") |> filter(w => w != ""))
```

#### 같이 보기

[`join`](#fn-join), [`lines`](#fn-lines), [`chars`](#fn-chars), [`regex.split`](../modules/regex.html#fn-regex-split)

:::

:::fn join(xs, sep = "")

리스트의 원소를 `sep`로 이어 문자열 하나로 만든다.

#### 설명

[`split`](#fn-split)의 반대다. 원소 사이에만 `sep`를 넣고 앞뒤에는 넣지 않는다. 빈 리스트는 `""`이다. 문자열이 아닌 원소는 [`str`](convert.html#fn-str)처럼 바꿔 넣으므로 수의 리스트도 바로 잇는다. 다만 문자열 보간과 같이 `nil`, 결과값(`Ok`·`Err`), 핸들(함수·모듈·태스크·범위·채널 끝·atom·파일)은 원소 안 어느 깊이에 있어도 넣지 않는다. `sep`를 주지 않으면 그냥 붙인다.

조각을 리스트에 모아 한 번에 `join`하면 구분자까지 한 번에 넣는다. 문자열을 뒤에 `+`로 붙여 쌓는 것도 선형이지만, 앞에 붙여 쌓는 것(`s + acc`)은 매번 전체를 복사하니 그때는 `join`을 쓴다. 여러 줄 출력을 만들 때는 `join(rows, "\n")`.

#### 매개변수

- `xs`: 리스트, 범위, 받는 끝.
- `sep`: 사이에 넣을 문자열. 기본은 `""`.

#### 돌려주는 값

문자열.

#### 에러

- `xs`가 리스트 등이 아니면 런타임 에러 E0506(`cannot join str`).
- `sep`가 문자열이 아니면 E0403.
- 원소(또는 원소 안)에 `nil`·`Ok`·`Err`·핸들이 있으면 E0519(글에 말없이 넣지 않는다, 보간과 같다). 그대로 보이려면 `map(xs, str)`로 먼저 바꾼다.

#### 예제

**잇기**

```fung run
print(join(["a", "b", "c"], ", "), join([1, 2, 3]), join(0..4, "-"), "[" + join([], "|") + "]")
```

**여러 줄 만들기**

```fung run
rows = [("web-1", "up"), ("db-1", "down")]
print(join(map(rows, ((h, s)) => "${h}\t${s}"), "\n"))
```

**나누고, 고치고, 다시 잇기**

```fung run
path = "/usr/local/bin"
print(split(path, "/") |> filter(p => p != "") |> map(upper) |> join("."))
```

#### 같이 보기

[`split`](#fn-split), [`str`](convert.html#fn-str), [`print`](io.html#fn-print)

:::

:::fn lines(s)

글을 줄의 리스트로 나눈다.

#### 설명

`\n`으로 나누되, 줄 끝의 `\r`(Windows식 줄 끝 `\r\n`)을 떼고, 마지막 줄바꿈 뒤에 빈 줄을 만들지 않는다. 그래서 파일 끝에 줄바꿈이 있든 없든 같은 결과다. 가운데의 빈 줄은 `""`로 남는다. 빈 문자열은 `[]`이다. [`read_lines`](io.html#fn-read_lines)와 같은 규칙이다.

`split(s, "\n")`을 쓰면 끝에 빈 문자열이 하나 더 붙으므로(`split("a\n", "\n")`은 `["a", ""]`), 글을 줄로 나눌 때는 언제나 `lines`를 쓴다.

#### 매개변수

- `s`: 문자열.

#### 돌려주는 값

문자열의 리스트.

#### 에러

`s`가 문자열이 아니면 런타임 에러 E0403.

#### 예제

**마지막 줄바꿈과 \r**

```fung run
print(lines("a\nb\n"), lines("a\r\nb"), lines("a\n\nb"), lines(""))
print(split("a\nb\n", "\n"))
```

**명령 출력을 줄로**

```fung run
out = exec(["printf", "web-1\nweb-2\n"])!.out
for (i, host) in zip(1..100, lines(out)):
  print(i, host)
;;
```

#### 같이 보기

[`split`](#fn-split), [`read_lines`](io.html#fn-read_lines), [`fs.read_lines`](../modules/fs.html#fn-fs-read_lines)

:::

:::fn chars(s)

문자열을 글자 하나짜리 문자열의 리스트로 나눈다.

#### 설명

글자는 유니코드 코드 포인트 하나다. 그래서 결합 문자로 쓴 `"e\u{301}"`(é)나 피부색이 붙은 이모지는 둘로 나뉜다. UTF-8에 속하지 않는 바이트는 그 바이트 하나짜리 문자열 한 글자다(`chars("a\xff")`는 `["a", "\xff"]`). 글자 리스트를 얻으면 컬렉션 함수(`map`, `filter`, `reverse`, `unique` …)를 모두 쓸 수 있고, 다 고친 뒤 `join`으로 다시 잇는다.

#### 매개변수

- `s`: 문자열.

#### 돌려주는 값

문자열의 리스트.

#### 에러

`s`가 문자열이 아니면 런타임 에러 E0403(``argument `s` must be str, got int``). 다른 문자열 함수와 같다.

#### 예제

**글자로 나누기**

```fung run
print(chars("참치a"), chars(""), len(chars("e\u{301}")))
```

**글자 단위로 다루기**

```fung run
word = "deploy"
print(chars(word) |> reverse |> join, chars(word) |> unique |> len)
print(chars("a1b2c3") |> filter(c => c >= "0" and c <= "9") |> join)
```

#### 같이 보기

[`split`](#fn-split), [`join`](#fn-join), [`len`](collections.html#fn-len)

:::

## 다듬기와 바꾸기 {#clean}

:::fn trim(s, cut = nil)

양 끝의 공백을 지운다. `cut`을 주면 그 글자들을 지운다.

#### 설명

`cut`이 없으면 양 끝의 공백 글자(스페이스, 탭, `\n`, `\r`, 유니코드 공백)를 모두 지운다. 가운데의 공백은 건드리지 않는다. 파일이나 명령 출력에서 읽은 값은 끝에 줄바꿈이 붙어 있는 일이 많으므로 비교하거나 수로 바꾸기 전에 `trim`한다.

`cut`을 주면 `cut`에 든 **글자 하나하나**를 지울 글자로 본다. 접두사·접미사 문자열이 아니다. `trim("abcxcba", "abc")`는 양 끝에서 `a`·`b`·`c` 중 하나인 글자를 계속 지워 `"x"`가 된다. 정해진 접두사 하나만 떼려면 `starts_with`와 슬라이스를 쓴다.

#### 매개변수

- `s`: 문자열.
- `cut`: 지울 글자들을 모은 문자열, 또는 `nil`(공백).

#### 돌려주는 값

문자열.

#### 에러

인자가 문자열이 아니거나 `cut`이 `""`이면(지울 글자가 없다) 런타임 에러 E0403.

#### 예제

**공백과 줄바꿈**

```fung run
print("[" + trim("  x ") + "]", "[" + trim("\t\n v1.2.3 \r\n") + "]")
```

**지울 글자 고르기**

```fung run
print(trim("--x--", "-"), trim("abcxcba", "abc"), trim("\"quoted\"", "\""))
```

**명령 출력에서 값 얻기**

```fung run
count = exec(["echo", "42"])!.out |> trim |> int
print(count + 1)
```

#### 같이 보기

[`replace`](#fn-replace), [`starts_with`](#fn-starts_with), [`lines`](#fn-lines)

:::

:::fn trim_left(s, cut = nil)

앞(왼쪽) 끝만 [`trim`](#fn-trim)한다.

#### 설명

[`trim`](#fn-trim)과 같은 규칙으로 앞 끝의 공백이나 `cut`의 글자를 지우고, 뒤 끝은 그대로 둔다. 이름은 [`pad_left`](#fn-pad_left)와 짝이다.

#### 매개변수

- `s`: 문자열.
- `cut`: 지울 글자들을 모은 문자열, 또는 `nil`(공백).

#### 돌려주는 값

문자열.

#### 에러

인자가 문자열이 아니거나 `cut`이 `""`이면 런타임 에러 E0403.

#### 예제

**앞의 들여쓰기와 0 지우기**

```fung run
print("[" + trim_left("   item  ") + "]", trim_left("000420", "0"))
```

#### 같이 보기

[`trim`](#fn-trim), [`trim_right`](#fn-trim_right), [`pad_left`](#fn-pad_left)

:::

:::fn trim_right(s, cut = nil)

뒤(오른쪽) 끝만 [`trim`](#fn-trim)한다.

#### 설명

[`trim`](#fn-trim)과 같은 규칙으로 뒤 끝의 공백이나 `cut`의 글자를 지우고, 앞 끝은 그대로 둔다. 줄 끝의 `\r`이나 뒤에 붙은 공백만 떼고 들여쓰기는 남길 때 쓴다.

#### 매개변수

- `s`: 문자열.
- `cut`: 지울 글자들을 모은 문자열, 또는 `nil`(공백).

#### 돌려주는 값

문자열.

#### 에러

인자가 문자열이 아니거나 `cut`이 `""`이면 런타임 에러 E0403.

#### 예제

**들여쓰기는 남기고 끝만 다듬기**

```fung run
print("[" + trim_right("  key: value \r\n") + "]", trim_right("path///", "/"))
```

#### 같이 보기

[`trim`](#fn-trim), [`trim_left`](#fn-trim_left), [`pad_right`](#fn-pad_right)

:::

:::fn upper(s)

대문자로 바꾼 문자열을 돌려준다.

#### 설명

유니코드 기본 대소문자 변환(전체 매핑)을 쓴다. 그래서 글자 수가 바뀔 수 있다: `upper("straße")`는 `"STRASSE"`. 한글처럼 대소문자가 없는 글자는 그대로다. UTF-8에 속하지 않는 바이트도 글자가 아니므로 그대로 둔다.

#### 매개변수

- `s`: 문자열.

#### 돌려주는 값

문자열.

#### 에러

`s`가 문자열이 아니면 런타임 에러 E0403.

#### 예제

**대문자로**

```fung run
print(upper("hello"), upper("straße"), upper("참치 v1"))
```

#### 같이 보기

[`lower`](#fn-lower)

:::

:::fn lower(s)

소문자로 바꾼 문자열을 돌려준다.

#### 설명

전체 매핑이고, 그리스어 시그마는 단어 끝에서 `ς`가 된다: `lower("ΟΔΟΣ")`는 `"οδος"`. 대소문자를 무시하고 비교하려면 양쪽을 `lower`해서 비교한다. UTF-8에 속하지 않는 바이트는 그대로 둔다.

#### 매개변수

- `s`: 문자열.

#### 돌려주는 값

문자열.

#### 에러

`s`가 문자열이 아니면 런타임 에러 E0403.

#### 예제

**소문자로, 대소문자 무시 비교**

```fung run
print(lower("ABC"), lower("ΟΔΟΣ"))
answer = "Yes"
print(lower(answer) == "yes")
```

#### 같이 보기

[`upper`](#fn-upper), [`sort_by`](collections.html#fn-sort_by)

:::

:::fn replace(s, old, new)

`s` 안의 모든 `old`를 `new`로 바꾼 문자열을 돌려준다.

#### 설명

왼쪽부터 겹치지 않게 찾아 모두 바꾼다(`replace("aaa", "aa", "b")`는 `"ba"`). `new`가 `""`면 지우는 것이다. `old`는 `""`일 수 없다(`split`의 빈 구분자와 같다). 첫 번째 것만 바꾸거나 패턴으로 바꾸려면 [`regex.replace`](../modules/regex.html#fn-regex-replace)를 쓴다.

#### 매개변수

- `s`: 문자열.
- `old`: 찾을 문자열.
- `new`: 바꿔 넣을 문자열.

#### 돌려주는 값

문자열.

#### 에러

- 인자가 문자열이 아니거나 `old`가 `""`이면 런타임 에러 E0403.
- 결과가 1억 글자를 넘으면 만들기 전에 E0806.

#### 예제

**모두 바꾸기, 지우기**

```fung run
print(replace("a-b-c", "-", "+"), replace("aaa", "aa", "b"), replace("v1.2.3", ".", ""))
```

**설정 템플릿 채우기**

```fung run
tmpl = "host={{HOST}} port={{PORT}}"
vals = {HOST: "db-1", PORT: "5432"}
print(fold(pairs(vals), tmpl, (acc, (k, v)) => replace(acc, "{{${k}}}", v)))
```

#### 같이 보기

[`regex.replace`](../modules/regex.html#fn-regex-replace), [`trim`](#fn-trim), [`split`](#fn-split)

:::

## 검사와 만들기 {#test-build}

:::fn starts_with(s, prefix)

`s`가 `prefix`로 시작하면 `true`.

#### 설명

빈 접두사는 언제나 참이다. 부분 문자열이 어디에든 있는지는 `x in s`(또는 [`contains`](collections.html#fn-contains)), 패턴은 [`regex.is_match`](../modules/regex.html#fn-regex-is_match)로 본다.

#### 매개변수

- `s`: 문자열.
- `prefix`: 문자열.

#### 돌려주는 값

`bool`.

#### 에러

인자가 문자열이 아니면 런타임 에러 E0403.

#### 예제

**접두사로 고르기**

```fung run
print(starts_with("fung.toml", "fung"), starts_with("a", ""))
log = ["# comment", "host=a", "", "# x", "port=1"]
print(filter(log, l => l != "" and not starts_with(l, "#")))
```

**접두사 떼기**

```fung run
tag = "v1.4.2"
print(starts_with(tag, "v") then tag[1..] else tag)
```

#### 같이 보기

[`ends_with`](#fn-ends_with), [`contains`](collections.html#fn-contains)

:::

:::fn index_of(s, sub)

`s`에서 `sub`가 처음 나오는 글자 위치. 없으면 `nil`.

#### 설명

위치는 바이트가 아니라 **글자** 단위다. 인덱스 `s[i]`·슬라이스·`len`과 같은 단위라서 `s[i..i + len(sub)]`이 찾은 부분이다. `sub in s`가 참이면 `index_of(s, sub)`는 `nil`이 아니다: 둘은 같은 것(바이트 열)을 찾는다. UTF-8이 아닌 바이트로 찾아 글자 안에서 맞으면 그 글자의 위치를 돌려준다. 있는지만 알면 될 때는 `sub in s`, 패턴으로 찾을 때는 [`regex.find`](../modules/regex.html#fn-regex-find)를 쓴다. 빈 `sub`는 어디에나 있으므로 에러다.

#### 매개변수

- `s`: 문자열.
- `sub`: 찾을 문자열. 비어 있으면 안 된다.

#### 돌려주는 값

`int`, 또는 `nil`.

#### 에러

인자가 문자열이 아니거나 `sub`가 `""`이면 런타임 에러 E0403.

#### 예제

**구분자 앞뒤로 나누기**

```fung run
line = "level=warn: 디스크 90%: 확인 필요"
i = index_of(line, ": ")
print(i, line[..i], line[i + 2..])
print(index_of(line, "error"))
```

#### 같이 보기

[`starts_with`](#fn-starts_with), [`split`](#fn-split), [`regex.find`](../modules/regex.html#fn-regex-find)

:::

:::fn ends_with(s, suffix)

`s`가 `suffix`로 끝나면 `true`.

#### 설명

확장자 검사에 많이 쓴다. 대소문자를 가리므로 `"A.LOG"`도 받으려면 `ends_with(lower(name), ".log")`.

#### 매개변수

- `s`: 문자열.
- `suffix`: 문자열.

#### 돌려주는 값

`bool`.

#### 에러

인자가 문자열이 아니면 런타임 에러 E0403.

#### 예제

**확장자로 고르기**

```fung run
files = ["app.log", "app.log.1", "ERROR.LOG", "notes.txt"]
print(filter(files, f => ends_with(lower(f), ".log")))
```

#### 같이 보기

[`starts_with`](#fn-starts_with), [`path.ext`](../modules/path.html#fn-path-ext)

:::

:::fn repeat(s, n)

`s`를 `n`번 이어 붙인 문자열을 돌려준다.

#### 설명

fung에는 `"a" * 3`이 없다(문자열과 수의 곱은 에러다). 구분선이나 들여쓰기를 만들 때 쓴다. `n`이 0이면 `""`이고, 음수면 E0403이다.

#### 매개변수

- `s`: 문자열.
- `n`: 정수.

#### 돌려주는 값

문자열.

#### 에러

- 인자의 타입이 틀리면 런타임 에러 E0403.
- 결과가 1억 글자를 넘으면 만들기 전에 E0806.

#### 예제

**구분선**

```fung run
print(repeat("ab", 3), "[" + repeat("x", 0) + "]")
print(repeat("-", 20))
```

#### 같이 보기

[`pad_left`](#fn-pad_left), [`join`](#fn-join)

:::

:::fn pad_left(s, n, pad = " ") fung

왼쪽을 `pad`로 채워 `n`글자로 만든다. 이미 `n`글자 이상이면 그대로.

#### 설명

수를 오른쪽으로 맞추거나 앞에 0을 채울 때 쓴다. `pad`가 여러 글자면 돌려 가며 채우고 모자란 만큼에서 자른다(`pad_left("1", 5, "ab")`는 `"abab1"`). 길이는 [`len`](collections.html#fn-len)처럼 글자 수로 센다. 한글처럼 화면에서 두 칸을 차지하는 글자도 한 글자로 세므로, 한글이 섞인 표는 터미널에서 줄이 어긋날 수 있다.

#### 매개변수

- `s`: 문자열. 수는 먼저 `str`로 바꾼다.
- `n`: 만들 글자 수. 음이 아닌 정수(음수면 E0403).
- `pad`: 채울 문자열. 기본은 공백 하나.

#### 돌려주는 값

문자열.

#### 에러

- `s`·`pad`가 문자열이 아니거나 `n`이 정수가 아니면 런타임 에러 E0403.
- `pad`가 `""`면 런타임 에러 E0403(``argument `pad` must be a non-empty str, got ""``).

#### 예제

**0 채우기, 오른쪽 맞춤**

```fung run
print(pad_left("7", 3, "0"), pad_left("abc", 2), pad_left("1", 5, "ab"))
for n in [5, 120, 42]:
  print(pad_left(str(n), 5))
;;
```

**표 만들기**

```fung run
rows = [("web-1", 72), ("db-1", 5)]
for (host, cpu) in rows:
  print(pad_right(host, 8) + pad_left(str(cpu), 4) + "%")
;;
```

#### 같이 보기

[`pad_right`](#fn-pad_right), [`repeat`](#fn-repeat)

:::

:::fn pad_right(s, n, pad = " ") fung

오른쪽을 `pad`로 채워 `n`글자로 만든다. 이미 `n`글자 이상이면 그대로.

#### 설명

[`pad_left`](#fn-pad_left)와 같은 규칙으로 오른쪽을 채운다. 표의 왼쪽 칸(이름)을 맞출 때 쓴다.

#### 매개변수

- `s`: 문자열.
- `n`: 만들 글자 수. 음이 아닌 정수(음수면 E0403).
- `pad`: 채울 문자열. 기본은 공백 하나.

#### 돌려주는 값

문자열.

#### 에러

[`pad_left`](#fn-pad_left)와 같다.

#### 예제

**칸 맞추기**

```fung run
print("[" + pad_right("ab", 4) + "]", pad_right("1", 4, "-="))
print(pad_right("name", 8, ".") + "value")
```

#### 같이 보기

[`pad_left`](#fn-pad_left)

:::

## 바이트 {#bytes}

fung 문자열은 바이트 열이다. 바깥에서 들어온 값(파일, 명령 출력, `env`, `args`, 표준 입력, 소켓)은 UTF-8이 아니어도 **바이트 그대로** 들어오고, 그대로 쓰면 그대로 나간다. 그래서 fung으로 읽고 쓰기만 하는 스크립트가 파일을 바꾸지 않는다.

글자 연산(`len`, `s[i]`, `chars`, 슬라이스)은 바이트를 UTF-8로 읽고, UTF-8에 속하지 않는 바이트는 그 하나가 한 글자다. 자르고 다시 이어도 바이트가 그대로 남는다. 값으로 보일 때(리스트 안, REPL, `dbg`)는 그런 바이트가 `\xff`처럼 보이고, 리터럴에도 `"\xff"`로 쓴다. 값으로 보인 문자열은 소스에 그대로 붙여 같은 값이 되는 리터럴이다: 보이지 않는 글자는 `\u{…}`(16진 숫자는 소문자)로, 리터럴에서 보간이나 E0129가 될 `$`(`{`나 이름의 첫 글자 앞)는 `\$`로 보인다. `\xHH`는 **바이트 하나**다: `"\xc3\xa9"`는 UTF-8로 `é`이고, `"\xe9"`는 `é`가 아니라 바이트 0xE9 하나다(글자 `é`는 `"\u{e9}"`).

대부분의 함수는 그런 바이트를 그대로 둔다: 비교(바이트 순, UTF-8이면 코드 포인트 순과 같다), `+`, `split`, `replace`, `trim`, `upper`·`lower`, 정규식. 형식 자체가 글을 요구하는 곳만 에러 [E0518](../errors.html#E0518)로 멈춘다: [`json.stringify`](../modules/json.html#fn-json-stringify), 정규식 패턴, 시간 형식, 호스트 이름. 바깥에서 온 값을 읽는 [`parse_int`](convert.html#fn-parse_int) 같은 함수에는 그런 바이트가 읽을 수 없는 입력일 뿐이라 `Err`다.

```fung run
s = "a\xffb"
print([s, s[1], s + "!"], len(s), byte_len(s), is_utf8(s))
```

:::fn byte_len(s)

문자열의 바이트 수.

#### 설명

`len`은 글자를, `byte_len`은 바이트를 센다. 둘은 ASCII 문자열에서만 같다. 파일 크기, 네트워크로 보낼 길이(`Content-Length`), 바이트 한도를 다룰 때 쓴다.

#### 매개변수

- `s`: 문자열.

#### 돌려주는 값

정수.

#### 에러

`s`가 문자열이 아니면 런타임 에러 E0403.

#### 예제

**글자와 바이트**

```fung run
print(len("참치"), byte_len("참치"), byte_len("ab"), byte_len("\xff"))
```

#### 같이 보기

[`len`](collections.html#fn-len), [`bytes`](#fn-bytes)

:::

:::fn bytes(s)

문자열의 바이트를 0부터 255까지의 정수 리스트로 돌려준다.

#### 설명

바이트 하나하나를 수로 다뤄야 할 때 쓴다: 퍼센트 인코딩, 체크섬, 이진 형식 읽기. JSON처럼 글만 받는 곳에 바이트를 옮길 때도 쓴다. [`from_bytes`](#fn-from_bytes)가 거꾸로다.

#### 매개변수

- `s`: 문자열.

#### 돌려주는 값

정수 리스트.

#### 에러

`s`가 문자열이 아니면 런타임 에러 E0403.

#### 예제

**UTF-8 바이트 보기**

```fung run
print(bytes("A가"), bytes(""), bytes("\xff"))
```

**간단한 체크섬**

```fung run
print(bytes("hello") |> sum |> (n => n % 256))
```

#### 같이 보기

[`from_bytes`](#fn-from_bytes), [`byte_len`](#fn-byte_len)

:::

:::fn from_bytes(xs)

0부터 255까지의 정수 리스트를 그 바이트로 된 문자열로 만든다.

#### 설명

[`bytes`](#fn-bytes)의 반대다. 바이트가 UTF-8이 아니어도 그대로 문자열이 된다. 값을 바꾸지 않으므로 `from_bytes(bytes(s)) == s`다.

#### 매개변수

- `xs`: 0부터 255까지의 정수 리스트.

#### 돌려주는 값

문자열.

#### 에러

`xs`가 리스트가 아니거나(범위 `65..70`도 받지 않는다: `from_bytes(map(65..70, b => b))`처럼 리스트로 만들어 준다), 0부터 255까지의 정수가 아닌 원소가 있으면 런타임 에러 E0403.

#### 예제

**바이트에서 문자열로**

```fung run
print(from_bytes([237, 149, 156]), [from_bytes([104, 105, 255])])
print(from_bytes(bytes("참치")) == "참치")
```

#### 같이 보기

[`bytes`](#fn-bytes), [`is_utf8`](#fn-is_utf8)

:::

:::fn is_utf8(s)

문자열이 올바른 UTF-8 글인지.

#### 설명

글이 필요한 곳(JSON, 정규식 패턴, 시간 형식, 호스트 이름)에 넘기기 전에 확인할 때 쓴다. 그런 곳에 UTF-8이 아닌 문자열을 넘기면 에러 [E0518](../errors.html#E0518)이다.

#### 매개변수

- `s`: 문자열.

#### 돌려주는 값

불.

#### 에러

`s`가 문자열이 아니면 런타임 에러 E0403.

#### 예제

**바깥에서 온 값 확인**

```fung run
for s in ["café", "caf\xe9", ""]:
  print(is_utf8(s))
;;
```

#### 같이 보기

[`bytes`](#fn-bytes), [`from_bytes`](#fn-from_bytes)

:::
