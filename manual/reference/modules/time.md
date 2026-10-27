---
lede: <code>import time</code>. 지금 시각을 얻고, 글자로 된 시각을 읽고 쓰고, 시간대를 바꾼다. 시각은 언어의 값 타입이라 <code>==</code>, <code>&lt;</code>, <code>t + 1h</code>, <code>t2 - t1</code>이 바로 된다.
description: fung 모듈 time — 시각 값 만들기, 읽고 쓰기, 시간대 바꾸기, 걸린 시간 재기(now, monotonic, parse, format, utc, local, unix, from_unix, parts).
---
# time — 시각

## 시각, 기간, 시간대 {#concepts}

시간을 다루는 값은 두 가지다. 둘을 구분하는 것이 시간 계산의 첫걸음이다.

- **시각**(instant, `time` 타입): "2026년 10월 4일 9시 5분"처럼 시간 위의 한 **점**이다. `time.now()`나 `time.parse`로 만든다.
- **기간**(duration, `dur` 타입): "90분"처럼 두 점 사이의 **길이**다. 리터럴로 바로 쓴다: `500ms`, `30s`, `1h30m`([기간 리터럴](../language.html#lexical)).

시각에 기간을 더하거나 빼면 시각이고, 시각에서 시각을 빼면 기간이다. 시각끼리는 `==`·`<`로 비교한다.

시계도 두 가지다. `time.now()`는 **벽시계**로 "지금 몇 시인가"를 준다. 사람이나 NTP가 고칠 수 있으므로 걸린 시간을 재는 데는 맞지 않다. 걸린 시간은 거꾸로 가지 않는 **단조 시계** `time.monotonic()`(기간을 준다)의 차로 잰다.

```fung run
import time

start = time.parse("2026-10-04T09:00:00Z")!
end = start + 1h30m
print(end, end - start, start < end)
print((end - start) / 1m, "minutes")
```

같은 순간도 사는 곳에 따라 벽시계가 다르게 읽힌다. 런던에서 0시 5분일 때 서울은 9시 5분이다. 지역마다 정한 시계의 규칙이 **시간대**(time zone)이고, 기준이 되는 시계가 **UTC**(협정 세계시)다. 시간대는 UTC와의 차이인 **오프셋**(offset)으로 적는다: 서울은 `+09:00`, UTC 자체는 `Z`. 어떤 지역은 여름에 시계를 한 시간 앞당기는 **서머타임**(DST)이 있어서 오프셋이 철마다 바뀐다.

fung의 시각 값은 순간과 함께 **시간대를 기억**한다. 출력하면 ISO 8601 꼴(`2026-10-04T09:05:07+09:00`, UTC면 `…Z`)로 그 시간대의 벽시계와 오프셋을 보인다. 시간대가 달라도 같은 순간이면 `==`는 참이다. 시간대를 정하지 않은 값은 컴퓨터의 **로컬 시간대**를 쓴다. 로컬 시간대는 환경 변수 `TZ`(없으면 `/etc/localtime`)로 정해진다. `TZ`는 `Asia/Seoul` 같은 이름(시간대 데이터베이스 `$TZDIR`나 `/usr/share/zoneinfo`에서 찾는다)이나 `KST-9` 같은 POSIX 시간대 문자열이고, `""`·`UTC`면 UTC, 모두 실패하면 UTC다. 이 매뉴얼의 예제는 `TZ=UTC`로 돈다.

**유닉스 시간**(Unix time)은 1970-01-01T00:00:00Z부터 센 초의 수다. 로그와 API가 시각을 정수 하나로 주고받을 때 흔히 쓴다.

시각은 UTC 기준으로 0000년부터 9999년까지 나타낼 수 있다. 계산이 이 범위를 넘으면 런타임 에러 E0503이다.

### 형식 토큰 {#tokens}

`time.parse`와 `time.format`의 `format` 인자는 토큰으로 모양을 적는다. 목록에 없는 글자는 그대로 쓰고 그대로 기대한다. 긴 토큰부터 맞춘다(`MMM`이 `MM`보다 먼저).

<table>
  <thead><tr><th>토큰</th><th>뜻</th><th>예</th></tr></thead>
  <tbody>
    <tr><td><code>YYYY</code></td><td>네 자리 연도</td><td><code>2026</code></td></tr>
    <tr><td><code>YY</code></td><td>두 자리 연도(읽을 때 2000~2099)</td><td><code>26</code></td></tr>
    <tr><td><code>MM</code></td><td>두 자리 월</td><td><code>10</code></td></tr>
    <tr><td><code>MMM</code></td><td>영문 월 약자(읽을 때 대소문자 무시)</td><td><code>Oct</code></td></tr>
    <tr><td><code>DD</code></td><td>두 자리 일</td><td><code>04</code></td></tr>
    <tr><td><code>ddd</code></td><td>영문 요일 약자(읽을 때 이름만 검사하고 날짜와 대조하지 않음)</td><td><code>Sun</code></td></tr>
    <tr><td><code>HH</code></td><td>두 자리 시(24시간제)</td><td><code>09</code></td></tr>
    <tr><td><code>mm</code></td><td>두 자리 분</td><td><code>05</code></td></tr>
    <tr><td><code>ss</code></td><td>두 자리 초</td><td><code>07</code></td></tr>
    <tr><td><code>SSS</code></td><td>밀리초 세 자리</td><td><code>123</code></td></tr>
    <tr><td><code>Z</code></td><td>오프셋(콜론 있음)</td><td><code>+09:00</code></td></tr>
    <tr><td><code>ZZ</code></td><td>오프셋(콜론 없음)</td><td><code>+0900</code></td></tr>
  </tbody>
</table>

읽을 때는 자리 수를 정확히 맞춰야 한다(`9:05`는 `HH:mm`에 맞지 않는다). 12시간제(AM/PM)와 이스케이프는 없다.

## 함수 {#functions}

:::fn time.now()

지금 시각을 돌려준다.

#### 설명

컴퓨터 시계의 지금 시각을 로컬 시간대로 돌려준다. 실행할 때마다 값이 다르다. 이 값은 시스템 시계(벽시계)를 읽은 것이다. 두 `now()` 사이에 시스템 시각이 바뀌면(NTP 보정, 사람이 바꿈) `t2 - t1`이 틀리거나 음수일 수 있으므로, **걸린 시간은 [`time.monotonic()`](#fn-time-monotonic)으로 잰다**. `now()`는 기록에 남길 시각, 파일 시각과의 비교처럼 "몇 시인가"가 필요할 때 쓴다.

#### 매개변수

없음.

#### 돌려주는 값

`time` 값.

#### 예제

**타임스탬프 찍기**

```fung run hide-output
import time

print("[${time.format(time.now(), "YYYY-MM-DD HH:mm:ss")}] backup started")
```

#### 같이 보기

[`time.monotonic`](#fn-time-monotonic), [`time.format`](#fn-time-format)

:::

:::fn time.monotonic()

거꾸로 가지 않는 시계의 지금 값을 기간(`dur`)으로 돌려준다. 걸린 시간을 잴 때 쓴다.

#### 설명

기준점부터 지난 기간이다. 기준점은 정하지 않았으므로(지금은 프로세스가 시작할 무렵) 값 하나에는 뜻이 없고, **두 값의 차만 뜻이 있다**: `time.monotonic() - t0`가 걸린 시간이다. 시스템 시각이 바뀌어도(NTP 보정, 사람이 바꿈) 흔들리지 않고 줄지 않는다. [`sleep`](../prelude/concurrency.html#fn-sleep)과 [`timeout`](../prelude/concurrency.html#fn-timeout)의 기한도 이 시계를 쓰므로 잰 값과 기한이 맞는다. 컴퓨터가 잠든(절전) 동안은 세지 않는다.

시각이 필요하면(기록에 남길 때, 파일 시각과 견줄 때) [`time.now()`](#fn-time-now)를 쓴다. 두 시계의 값은 섞어 쓸 수 없다: 하나는 `dur`, 하나는 `time`이다.

#### 매개변수

없음.

#### 돌려주는 값

`dur` 값.

#### 예제

**걸린 시간 재기**

```fung run
import time

start = time.monotonic()
sleep(50ms)
elapsed = time.monotonic() - start
print(elapsed >= 50ms)
```

#### 같이 보기

[`time.now`](#fn-time-now), [`sleep`](../prelude/concurrency.html#fn-sleep), [`timeout`](../prelude/concurrency.html#fn-timeout)

:::

:::fn time.parse(s, format = nil)

글자로 적은 시각을 읽어 시각 값으로 바꾼다.

#### 설명

`format`이 없으면 **RFC 3339**(ISO 8601의 인터넷용 꼴)를 엄격하게 읽는다: `2026-10-04T09:05:07Z`, `2026-10-04T09:05:07.5+09:00`. 날짜와 시각 사이는 `T`(공백은 안 됨), 필드는 모두 두 자리, 소수 초는 `.`과 1~9자리, 끝에는 `Z`나 오프셋이 꼭 있어야 한다. `T`와 `Z`는 소문자도 된다.

`format`이 있으면 [형식 토큰](#tokens)대로 읽는다. 형식에 시간대 토큰(`Z`·`ZZ`)이 없으면 로컬 시간대의 벽시계로 읽는다. 형식에 없는 날짜 필드는 0000-01-01, 시각 필드는 0으로 채운다. 숫자 오프셋으로 읽은 값은 그 오프셋을 그대로 기억한다.

로컬 시간으로 읽은 벽시계 시각이 서머타임 경계에 걸리면: 시계를 앞당겨 **없는** 시각(빈틈)은 빈틈만큼 뒤로 밀리고(`02:30` → `03:30`), 시계를 되돌려 **두 번 있는** 시각은 이른 쪽이다.

#### 매개변수

- `s`: 읽을 문자열.
- `format`: 형식 문자열. `nil`이면 RFC 3339.

#### 돌려주는 값

`Ok(time)`.

#### 에러

- `Err({kind: 'parse, message})`: 형식에 맞지 않거나 범위를 넘는다(13월, 2월 30일). `message`는 ``cannot parse `2026-13-04T09:05:07Z` as a time: month out of range``처럼 이유를 담는다.
- `s`가 문자열이 아니면 런타임 에러 E0403.

#### 예제

**RFC 3339**

```fung run
import time

print(time.parse("2026-10-04T09:05:07Z"))
print(time.parse("2026-10-04T09:05:07.5+09:00"))
print(time.parse("2026-10-04 09:05:07"))
```

**로그의 시각 읽기**

```fung run
import time

line = `10.0.0.7 - - [04/Oct/2026:09:05:07 +0900] "GET / HTTP/1.1" 200`
stamp = split(split(line, "[")[1], "]")[0]
t = time.parse(stamp, "DD/MMM/YYYY:HH:mm:ss ZZ")!
print(t, time.utc(t))
```

**시간대 없이 읽으면 로컬 시간**

```fung run env="TZ=KST-9"
import time

t = time.parse("2026-10-04 09:05", "YYYY-MM-DD HH:mm")!
print(t, time.utc(t))
```

**서머타임 경계**(미국 동부 규칙을 POSIX 문자열로 준 예)

```fung run env="TZ=EST5EDT,M3.2.0,M11.1.0"
import time

print(time.parse("2026-03-08 02:30", "YYYY-MM-DD HH:mm")!)
print(time.parse("2026-11-01 01:30", "YYYY-MM-DD HH:mm")!)
```

#### 같이 보기

[`time.format`](#fn-time-format), [`parse_dur`](../prelude/convert.html#fn-parse_dur)(기간 읽기)

:::

:::fn time.format(t, format = nil)

시각을 문자열로 쓴다.

#### 설명

`format`이 없으면 ISO 8601 꼴이고, `print`·`str`·`"${t}"`도 같은 꼴을 쓴다. 소수 초는 0이 아닐 때만 붙는다. `format`이 있으면 [형식 토큰](#tokens)대로 쓴다. 값이 기억하는 시간대의 벽시계로 쓰므로, 다른 시간대로 보이려면 먼저 [`time.utc`](#fn-time-utc)나 [`time.local`](#fn-time-local)로 바꾼다.

#### 매개변수

- `t`: 시각.
- `format`: 형식 문자열. `nil`이면 ISO 8601.

#### 돌려주는 값

문자열.

#### 예제

**여러 모양**

```fung run
import time

t = time.parse("2026-10-04T09:05:07.123+09:00")!
print(time.format(t))
print(time.format(t, "YYYY-MM-DD"))
print(time.format(t, "ddd, DD MMM YYYY HH:mm:ss ZZ"))
print(time.format(t, "HH:mm:ss.SSS Z"))
```

**날짜별 파일 이름**

```fung run
import time

t = time.parse("2026-10-04T23:59:00Z")!
print("backup-${time.format(t, "YYYYMMDD-HHmm")}.tar.gz")
print("backup-${time.format(t + 1m, "YYYYMMDD-HHmm")}.tar.gz")
```

#### 같이 보기

[`time.parse`](#fn-time-parse), [`time.parts`](#fn-time-parts)

:::

:::fn time.utc(t)

같은 순간을 UTC 시간대로 바꾼다.

#### 설명

순간은 그대로 두고 시간대만 UTC로 바꾼다. 출력과 [`time.format`](#fn-time-format)·[`time.parts`](#fn-time-parts)의 벽시계가 UTC 기준이 된다. 서버 여러 대의 로그를 합칠 때처럼, 시간대가 섞인 시각을 한 기준으로 맞출 때 쓴다.

#### 매개변수

- `t`: 시각.

#### 돌려주는 값

UTC 시간대의 `time` 값.

#### 예제

```fung run
import time

seoul = time.parse("2026-10-04T09:05:07+09:00")!
u = time.utc(seoul)
print(u, u == seoul, time.parts(u).hour)
```

#### 같이 보기

[`time.local`](#fn-time-local)

:::

:::fn time.local(t)

같은 순간을 로컬 시간대로 바꾼다.

#### 설명

순간은 그대로 두고 시간대를 [로컬 시간대](#concepts)로 바꾼다. 외부에서 받은 UTC 시각을 사람에게 보일 때 쓴다.

#### 매개변수

- `t`: 시각.

#### 돌려주는 값

로컬 시간대의 `time` 값.

#### 예제

```fung run env="TZ=KST-9"
import time

t = time.parse("2026-10-04T00:05:07Z")!
print(time.local(t), time.format(time.local(t), "HH:mm"))
```

#### 같이 보기

[`time.utc`](#fn-time-utc)

:::

:::fn time.unix(t)

시각을 유닉스 시간(초)으로 바꾼다.

#### 설명

1970-01-01T00:00:00Z부터 `t`까지의 초를 정수로 돌려준다. 초 밑은 과거 쪽으로 내린다(`23:59:59.5Z`는 `…59`초). 1970년 전의 시각은 음수다. 시간대와 무관하다.

#### 매개변수

- `t`: 시각.

#### 돌려주는 값

정수.

#### 예제

```fung run
import time

print(time.unix(time.parse("2026-10-04T09:05:07+09:00")!))
print(time.unix(time.parse("1969-12-31T23:59:59Z")!))
```

#### 같이 보기

[`time.from_unix`](#fn-time-from_unix)

:::

:::fn time.from_unix(n)

유닉스 시간(초)을 시각으로 바꾼다.

#### 설명

정수 `n`초를 로컬 시간대의 시각으로 돌려준다. 밀리초로 된 값은 `n // 1000`으로 나눠서 넘긴다.

#### 매개변수

- `n`: 1970-01-01T00:00:00Z부터 센 초(정수).

#### 돌려주는 값

로컬 시간대의 `time` 값.

#### 에러

- 런타임 에러 E0403: `n`이 정수가 아니거나, 0000~9999년 범위를 벗어난다.

#### 예제

**로그의 유닉스 시간 읽기**

```fung run
import time

print(time.from_unix(0))
print(time.from_unix(1791104707))
print(time.from_unix(1791104707123 // 1000))
```

**로컬 시간대로 나온다**

```fung run env="TZ=KST-9"
import time

print(time.from_unix(0))
```

#### 같이 보기

[`time.unix`](#fn-time-unix)

:::

:::fn time.parts(t)

시각을 연·월·일·시·분·초·요일로 나눈다.

#### 설명

`{year, month, day, hour, minute, second, weekday}` 맵을 돌려준다. 값은 모두 정수이고 `weekday`만 심볼(`'mon` `'tue` `'wed` `'thu` `'fri` `'sat` `'sun`)이다. 값이 기억하는 시간대의 벽시계 기준이다. "주말이면 건너뛴다", "매시 정각에만" 같은 조건에 쓴다.

#### 매개변수

- `t`: 시각.

#### 돌려주는 값

맵.

#### 예제

**나누기**

```fung run
import time

print(time.parts(time.parse("2026-10-04T09:05:07+09:00")!))
```

**주말 검사**

```fung run
import time

fn is_weekend(t) = time.parts(t).weekday in ['sat, 'sun]

print(is_weekend(time.parse("2026-10-04T12:00:00Z")!), is_weekend(time.parse("2026-10-05T12:00:00Z")!))
```

#### 같이 보기

[`time.format`](#fn-time-format)

:::

기간 값 자체(리터럴, 나누기, `parse_dur`)는 [언어 명세](../language.html#lexical)와 [`parse_dur`](../prelude/convert.html#fn-parse_dur)에 있다. 시간 제한은 prelude의 [`timeout`](../prelude/concurrency.html#fn-timeout)을 본다.
