---
lede: 여러 일을 동시에 진행한다. 태스크는 범위(<code>scope</code>·<code>supervise</code>) 안에서만 띄우고, 범위는 안의 태스크가 모두 끝나야 돌아온다. 태스크 사이의 값은 채널로 주고받고, 나누는 상태는 <code>atom</code>에 둔다. 병렬 <code>map</code> 같은 도우미는 <a href="../modules/conc.html"><code>conc</code> 모듈</a>에 있다.
description: fung prelude 동시성: scope, supervise, spawn, await, timeout, cancel, channel, send, recv, close, stop_signal, request_stop, atom, swap, reset, deref, sleep.
---
# 동시성

## 태스크와 동시성 {#concepts}

스크립트가 하는 일의 대부분은 **기다림**이다. 서버 열 대에 `ssh`로 명령을 보내면 각 서버가 답할 때까지 기다리고, URL 스무 개를 확인하면 응답마다 기다린다. 하나씩 차례로 하면 기다리는 시간이 모두 더해진다. **동시성**(concurrency)은 하나를 기다리는 동안 다른 일을 진행해서 기다림을 겹치는 것이다. 열 대에 동시에 보내면 전체 시간은 가장 느린 한 대만큼이 된다.

fung에서 동시에 진행하는 일 하나를 **태스크**(task)라고 한다. 태스크는 운영체제 스레드가 아니다. 실행 스레드 하나가 모든 태스크를 차례로 돌리고, 태스크가 기다리는 일(외부 명령, 소켓, `sleep`, 채널)을 만나면 그 태스크를 잠시 세워 두고 다른 태스크를 돌린다([런타임](../../vm/internals.html#concurrency)). 그래서 태스크는 가볍고(수천 개도 괜찮다), 한 순간에는 한 태스크의 코드만 돈다. 계산만 하는 일을 여러 CPU에 나눠 빠르게 하는 **병렬성**(parallelism)은 목표가 아니다. 목표는 기다림을 겹치는 것이다.

```fung run
import time

start = time.now()
results = scope(fn(s):
  tasks = for h in ["web-1", "web-2", "web-3"]:
    spawn(s, fn():
      sleep(300ms)
      "${h} ok"
    ;;)
  ;;
  map(tasks, await)
;;)
print(results)
print("faster than one by one:", time.now() - start < 900ms)
```

세 태스크가 300ms씩 기다리지만 기다림이 겹치므로 전체는 300ms 남짓이다.

### 범위: 태스크는 혼자 떠돌지 않는다 {#structured}

fung의 태스크는 언제나 **범위**(scope) 안에서 태어난다. [`scope(f)`](#fn-scope)가 범위를 열고, 그 안에서 [`spawn(s, g)`](#fn-spawn)으로 태스크를 띄운다. 범위는 본문과 그 안의 모든 태스크가 끝나야 돌아온다. 이것을 **구조적 동시성**(structured concurrency)이라고 한다. 덕분에 세 가지가 보장된다.

- 범위가 돌아온 뒤에는 그 안의 태스크가 하나도 남아 있지 않다. 몰래 뒤에서 도는 태스크가 없다.
- 태스크가 런타임 에러로 죽으면 그 에러는 언제나 범위를 연 코드로 간다. 실패가 사라지지 않는다.
- 범위를 취소하면 그 안의 모든 것이 취소된다. 기한([`timeout`](#fn-timeout))도 범위 단위로 걸린다.

### 값을 나누는 법 {#sharing}

fung의 값은 불변이라 태스크 사이에 그냥 넘겨도 안전하다. 태스크에 일을 줄 때는 람다가 붙잡은 값으로 주고, 결과는 [`await`](#fn-await)로 받는다. 태스크끼리 계속 값을 주고받아야 하면 **채널**(channel)을 쓴다. 한쪽 끝에서 [`send`](#fn-send)한 값을 다른 끝에서 [`recv`](#fn-recv)로 받는다. 여러 태스크가 함께 읽고 바꾸는 상태가 꼭 필요하면 [`atom`](#fn-atom)에 둔다.

## 범위와 태스크 {#tasks}

:::fn scope(f)

범위를 열고 `f(s)`를 부른다. 본문과 그 안의 태스크가 모두 끝나면 본문의 결과를 돌려준다.

#### 설명

`s`는 범위 핸들이고, [`spawn(s, g)`](#fn-spawn)으로 이 범위에 태스크를 띄울 때 쓴다. `scope`는 본문 `f(s)`가 끝난 뒤에도 띄운 태스크가 모두 끝날 때까지 기다린다. 결과는 본문의 반환값이다. 태스크의 결과가 필요하면 본문에서 `await`한다.

**실패**: 태스크 하나가 런타임 에러로 죽으면 범위는 나머지 태스크와 본문을 취소하고, 모두 멈추기를 기다린 뒤 그 에러를 `scope`를 부른 자리에서 다시 낸다. 에러에는 `note: scope opened at …`가 붙어 어느 범위였는지 알려 준다. 함수가 `Err` **값**을 돌려준 것은 실패가 아니다(그냥 결과다). 한 태스크의 실패가 다른 태스크를 멈추지 않게 하려면 [`supervise`](#fn-supervise)를 쓴다.

**취소**: `cancel(s)`로 취소된 범위는 `Err({kind: 'cancelled})`를 돌려준다. 본문에서 스스로 `cancel(s)`하면 본문도 그 자리에서 멈춘다.

#### 매개변수

- `f`: 범위 핸들 `s`를 받는 함수.

#### 돌려주는 값

본문 `f(s)`의 결과. 취소되었으면 `Err({kind: 'cancelled})`.

#### 에러

안의 태스크나 본문의 런타임 에러를 다시 낸다.

#### 예제

**두 태스크의 결과 모으기**

```fung run
r = scope(fn(s):
  a = spawn(s, () => 6 * 7)
  b = spawn(s, () => 1 + 1)
  [await(a), await(b)]
;;)
print(r)
```

**결과를 기다리지 않아도 범위가 기다린다**

```fung run
scope(fn(s):
  spawn(s, fn():
    sleep(50ms)
    print("slow task done")
  ;;)
  print("body done")
;;)
print("scope returned")
```

**태스크 하나의 실패가 범위 전체를 멈춘다**

```fung run exit=1 error=E0803
scope(fn(s):
  spawn(s, fn():
    sleep(10ms)
    fail("worker died")
  ;;)
  spawn(s, fn():
    sleep(1s)
    print("never printed")
  ;;)
;;)
print("not reached")
```

두 번째 태스크는 취소되어 찍지 못한다.

**Err 값은 실패가 아니다**

```fung run
print(scope(fn(s):
  t = spawn(s, () => parse_int("x"))
  await(t)
;;))
```

#### 같이 보기

[`spawn`](#fn-spawn), [`await`](#fn-await), [`supervise`](#fn-supervise), [`conc.pmap`](../modules/conc.html#fn-conc-pmap)

:::

:::fn supervise(f, on_fail)

`scope`와 같지만 태스크의 런타임 에러가 다른 태스크나 본문에 번지지 않고 `on_fail(e)`로 간다.

#### 설명

서버 여러 대를 확인할 때처럼 하나가 실패해도 나머지는 끝까지 해야 할 때 쓴다. 태스크가 런타임 에러로 죽으면 `on_fail(e)`가 불린다. `e`는 `{kind: 'error, code, message, at}` 맵이다: 에러 번호(`"E0803"`), 메시지, 난 곳(`"deploy.fung:3"`). 그 태스크를 `await`하면 같은 맵을 담은 `Err(e)`를 받는다. 다른 태스크와 본문은 계속 돈다.

`on_fail`은 실패가 일어난 순서대로 하나씩 돈다. `on_fail`의 결과는 버리므로 `Err`를 돌려주면 E0606이고, `on_fail`이 런타임 에러를 내도 범위가 실패한다. 범위가 취소되어 처리하지 못한 실패는 `task failed:`로 보고되고 종료 코드가 1이 된다.

본문 자체의 런타임 에러는 `on_fail`로 가지 않고 `scope`처럼 범위를 실패시킨다.

#### 매개변수

- `f`: 범위 핸들 `s`를 받는 함수.
- `on_fail`: 실패 맵 `e`를 받는 함수.

#### 돌려주는 값

본문 `f(s)`의 결과. 취소되었으면 `Err({kind: 'cancelled})`.

#### 에러

본문이나 `on_fail`의 런타임 에러, `on_fail`이 돌려준 `Err`(E0606).

#### 예제

**하나가 죽어도 나머지는 돈다**

```fung run
supervise(fn(s):
  spawn(s, () => fail("boom"))
  spawn(s, () => print("still runs"))
;;, e => eprint("handled: ${e.message}"))
```

**실패 맵과 await**

```fung run
r = supervise(fn(s):
  t = spawn(s, () => 1 / 0)
  await(t)
;;, e => print("on_fail:", e.code, e.message))
print(r)
```

**모두 해 보고 실패만 모으기**

```fung run
hosts = ["web-1", "web-2", "web-3"]
r = supervise(fn(s):
  tasks = for h in hosts:
    spawn(s, () => h == "web-2" then fail("${h} unreachable") else "${h} ok")
  ;;
  map(tasks, await)
;;, _ => nil)
print(r |> filter(x => not is_err(x)))
print(r |> filter(is_err) |> len, "failed")
```

`on_fail`이 아무것도 하지 않아도 실패한 태스크의 결과는 `Err`로 남는다.

#### 같이 보기

[`scope`](#fn-scope), [`await`](#fn-await), [`conc.retry`](../modules/conc.html#fn-conc-retry)

:::

:::fn spawn(s, f)

범위 `s` 안에서 `f()`를 새 태스크로 띄우고 태스크 핸들을 돌려준다.

#### 설명

`f`는 인자 없는 함수다. 보통 그 자리의 람다로 쓰고, 태스크에 줄 값은 람다가 붙잡게 한다: `spawn(s, () => check(host))`. `spawn`은 곧바로 돌아오고, 태스크는 지금 태스크가 기다리는 틈에 돈다. 돌려받은 핸들(`<task>`, `type_of`는 `'task`)로 [`await`](#fn-await)하거나 [`cancel`](#fn-cancel)한다.

범위가 기다리는 동안에는 태스크 안에서도 같은 범위에 `spawn`할 수 있다(일감이 일감을 낳는 경우). `scope(`가 이미 돌아온 범위에 `spawn`하면 E0706이다. 범위 핸들을 범위 밖으로 들고 나가도 쓸 수 없다는 뜻이다.

#### 매개변수

- `s`: 범위 핸들(`scope`·`supervise`의 `f`가 받은 것).
- `f`: 인자 없는 함수.

#### 돌려주는 값

태스크 핸들.

#### 에러

닫힌 범위면 런타임 에러 E0706(`spawn into a closed scope`).

#### 예제

**리스트의 원소마다 태스크**

```fung run
squares = scope(fn(s):
  tasks = map(1..=4, n => spawn(s, () => n * n))
  map(tasks, await)
;;)
print(squares)
```

**닫힌 범위**

```fung run exit=1 error=E0706
leaked = scope(s => s)
spawn(leaked, () => 1)
```

#### 같이 보기

[`scope`](#fn-scope), [`await`](#fn-await), [`cancel`](#fn-cancel)

:::

:::fn await(t)

태스크가 끝나기를 기다려 그 결과를 돌려준다.

#### 설명

- 정상으로 끝났으면 태스크 함수의 반환값.
- 런타임 에러로 죽었으면 `Err({kind: 'error, code, message, at})`([`supervise`](#fn-supervise)와 같은 맵). `scope` 안에서는 그 전에 범위가 실패하므로, 이 경우는 주로 `supervise`에서 본다.
- 취소됐으면 `Err({kind: 'cancelled})`.

이미 끝난 태스크면 곧바로 돌려주고, 여러 번 불러도 같은 결과다. 기다리는 시간에 한도를 두려면 [`timeout`](#fn-timeout)으로 감싼다. 시간이 지나도 태스크 자체는 계속 돈다(멈추려면 `cancel`).

#### 매개변수

- `t`: 태스크 핸들.

#### 돌려주는 값

태스크의 결과, 또는 위의 `Err`.

#### 에러

`t`가 태스크가 아니면 런타임 에러 E0403.

#### 예제

**결과 받기**

```fung run
print(scope(s => await(spawn(s, () => 6 * 7))))
```

**기다리는 시간에 한도 두기**

```fung run
r = scope(fn(s):
  t = spawn(s, fn():
    sleep(1s)
    "late"
  ;;)
  r = timeout(20ms, () => await(t))
  cancel(t)
  r
;;)
print(r)
```

`timeout`이 기다림만 끊으므로 태스크는 `cancel`로 따로 멈춘다. 그러지 않으면 범위가 1초 동안 그 태스크를 기다린다.

**취소된 태스크**

```fung run
print(scope(fn(s):
  t = spawn(s, fn():
    sleep(1s)
    "late"
  ;;)
  cancel(t)
  await(t)
;;))
```

#### 같이 보기

[`spawn`](#fn-spawn), [`timeout`](#fn-timeout), [`cancel`](#fn-cancel)

:::

:::fn timeout(d, f)

`f()`를 부르되, `d`가 지나면 `f` 안의 모든 것을 취소하고 `Err({kind: 'timeout})`을 돌려준다.

#### 설명

`d` 안에 끝나면 `f()`의 결과를 그대로 돌려준다(`f`가 돌려준 `Err`도 그대로다). 기한이 지나면 `f` 안에서 돌던 모든 것, 곧 그 안에서 연 범위의 태스크까지 취소하고 `Err({kind: 'timeout})`을 돌려준다. 취소되는 동안에도 [`with`](program.html#fn-with)의 `release`는 끝까지 돈다. `d`가 0 이하면 `f`를 부르지 않고 곧바로 `Err({kind: 'timeout})`이다.

기한이 겹치면 이른 기한이 이긴다. 바깥 기한이 먼저 지나면 바깥 `timeout`이 답한다.

**무엇이 끊기나**: `sleep`, 채널 연산, 소켓은 곧바로 끊긴다. 외부 명령(`exec`·`sh`)은 SIGTERM으로 끝낸다(2초 뒤 SIGKILL). 경로만 읽는 파일 함수(`fs.read`·`read_lines`·`list`·`glob`·`walk`·`exists`·`stat`)와 정규식 컴파일도 곧바로 끊긴다. 다만 운영체제 스레드는 멈출 수 없어서 그 읽기는 뒤에서 끝까지 돈다. FIFO나 장치(`/dev/stdin` 등)를 읽던 것이면 나중에 온 바이트를 그 읽기가 가져가 버린다. 무엇을 바꾸는 파일 함수(쓰기, 지우기, 옮기기)와 핸들 읽기·쓰기(`read_line(f)` 등)는 이미 시작했으면 끝날 때까지 끊기지 않고, 끝나는 즉시 끊긴다. 취소된 쓰기가 나중에 몰래 일어나지 않게 하려는 것이다([런타임](../../vm/internals.html#concurrency)).

#### 매개변수

- `d`: 기간.
- `f`: 인자 없는 함수.

#### 돌려주는 값

`f()`의 결과, 또는 `Err({kind: 'timeout})`.

#### 에러

`f` 안의 런타임 에러는 그대로 난다.

#### 예제

**제때 끝날 때와 아닐 때**

```fung run
print(timeout(1s, () => 1 + 1))
(_tx, rx) = channel()
print(timeout(10ms, () => recv(rx)))
```

**걸린 명령 끊기**

```fung run
r = timeout(100ms, () => exec(["sleep", "10"]))
print(r ?? "gave up")
```

**안의 범위까지 취소된다**

```fung run
r = timeout(20ms, fn():
  scope(fn(s):
    spawn(s, fn():
      sleep(5s)
      print("never printed")
    ;;)
    "body"
  ;;)
;;)
print(r)
```

**서버마다 기한 두기**

```fung run
fn probe(host) = timeout(500ms, fn():
  sleep(host == "slow-1" then 1s else 10ms)
  "${host} up"
;;)

print(scope(s => map(["web-1", "slow-1"], h => spawn(s, () => probe(h))) |> map(await)))
```

#### 같이 보기

[`cancel`](#fn-cancel), [`await`](#fn-await), [`exec`](process.html#fn-exec), [`with`](program.html#fn-with), [`parse_dur`](convert.html#fn-parse_dur)

:::

:::fn cancel(t)

태스크나 범위를 취소한다. 그 아래의 태스크에도 전파된다.

#### 설명

취소는 "지금 멈춰라"다. 취소된 태스크는 다음에 기다리는 자리(또는 지금 기다리고 있는 자리)에서 멈추고, 지나는 [`with`](program.html#fn-with)의 `release`를 돌린 뒤 끝난다. 그 태스크를 `await`하면 `Err({kind: 'cancelled})`다. 범위를 취소하면 본문과 그 안의 모든 태스크가 취소되고 `scope`는 `Err({kind: 'cancelled})`를 돌려준다.

취소는 실패가 아니라 종료 코드에 영향이 없다. 이미 끝난 태스크를 취소하면 아무 일도 없다(`await`는 원래 결과를 준다). "하던 일을 마치고 끝내라"는 정중한 요청은 [`stop_signal`](#fn-stop_signal)의 몫이다.

#### 매개변수

- `t`: 태스크 핸들 또는 범위 핸들.

#### 돌려주는 값

`nil`.

#### 에러

`t`가 태스크도 범위도 아니면 런타임 에러 E0403.

#### 예제

**먼저 끝난 쪽만 쓰기**

```fung run
winner = scope(fn(s):
  (tx, rx) = channel(2)
  a = spawn(s, fn():
    sleep(10ms)
    send(tx, "mirror-a")!
  ;;)
  b = spawn(s, fn():
    sleep(1s)
    send(tx, "mirror-b")!
  ;;)
  first = recv(rx)!
  cancel(a)
  cancel(b)
  first
;;)
print(winner)
```

**범위 취소**

```fung run
r = scope(fn(s):
  spawn(s, fn():
    sleep(1s)
    print("never printed")
  ;;)
  cancel(s)
  "not returned"
;;)
print(r)
```

#### 같이 보기

[`timeout`](#fn-timeout), [`await`](#fn-await), [`stop_signal`](#fn-stop_signal)

:::

## 채널 {#channels}

:::fn channel(n = 0)

채널을 만들고 두 끝 `(tx, rx)`, 곧 보내는 끝과 받는 끝을 돌려준다.

#### 설명

**채널**은 태스크 사이로 값을 나르는 관이다. 보내는 끝 `tx`(`sender`)로 [`send`](#fn-send)한 값이 받는 끝 `rx`(`receiver`)의 [`recv`](#fn-recv)로 보낸 순서대로 나온다. 출력은 `<sender>`·`<receiver>`, `type_of`는 `'sender`·`'receiver`, 타입 표기도 `sender`·`receiver`다.

`n`은 버퍼 크기다. 0(기본)이면 받는 쪽이 받을 때까지 `send`가 기다린다(주고받기가 맞물린다). `n`이 있으면 버퍼가 찰 때까지는 기다리지 않는다. 생산자가 소비자보다 너무 앞서지 않게 하는 것이 버퍼 크기의 역할이다.

끝은 복사할 수 있다. 같은 `tx`를 여러 생산 태스크에, 같은 `rx`를 여러 일꾼 태스크에 줄 수 있다. 받는 끝은 `for`, `map`, `take` 같은 컬렉션 함수로 돌 수 있다. 값이 올 때마다 처리하고, 채널이 닫히면 끝난다.

**자동 닫힘**: 보내는 끝의 마지막 사본이 사라지면 채널이 닫힌다. 받는 쪽은 남은 값을 다 받은 뒤 `Err({kind: 'closed})`를 받고, 받는 끝을 돌던 `for`는 끝난다. 받는 끝의 마지막 사본이 사라지면 채널이 닫히고 버퍼를 버리며, 그 뒤의 `send`는 `Err({kind: 'closed})`다. 지역 변수는 마지막으로 읽힌 문장 뒤에(한 번도 읽지 않으면 함수가 돌아올 때), 태스크가 붙잡은 값은 태스크가 끝날 때, 최상위 변수는 프로그램이 끝날 때 사라진다([값의 수명](../language.html#scope)). 그래서 함수 안에서 만든 채널의 보내는 끝을 생산 태스크에만 넘기면 생산이 끝날 때 저절로 닫힌다. 최상위에서 만든 채널은 [`close`](#fn-close)로 직접 닫는다.

모든 태스크가 서로를 기다려 아무도 나아갈 수 없으면 **교착**(deadlock)이고 런타임 에러 E0702다. 받는 쪽이 보내는 끝을 스스로 쥐고 있어서 채널이 닫힐 수 없는 경우면 보고가 그것을 알려 준다.

#### 매개변수

- `n`: 버퍼 크기(정수). 기본은 0.

#### 돌려주는 값

튜플 `(sender, receiver)`.

#### 에러

`n`이 0 이상의 정수가 아니면 런타임 에러 E0403.

#### 예제

**생산자 태스크와 자동 닫힘**

```fung run
fn numbers(s, n):
  (tx, rx) = channel()
  spawn(s, () => each(0..n, i => send(tx, i)))
  rx
;;

scope(fn(s):
  for x in numbers(s, 3):
    print(x)
  ;;
;;)
```

생산 태스크가 끝나면 `tx`의 마지막 사본이 사라져 채널이 닫히고, `for`가 끝난다.

**일꾼 여럿이 한 채널에서 받기**

```fung run
out = scope(fn(s):
  (jobs_tx, jobs) = channel()
  (res_tx, results) = channel()
  for _ in 1..=2:
    spawn(s, fn():
      for j in jobs:
        send(res_tx, j * j)!
      ;;
    ;;)
  ;;
  spawn(s, () => each(1..=4, j => send(jobs_tx, j)))
  results |> take(4) |> sort
;;)
print(out)
```

결과가 오는 순서는 일꾼에 따라 다를 수 있으므로 정렬해서 본다.

**보내는 끝을 쥔 채 받으면 교착**

```fung run exit=1 error=E0702
(tx, rx) = channel()
for x in rx:
  print(x)
;;
```

최상위의 `tx`가 프로그램 끝까지 살아 있으므로 채널은 닫히지 않는다. 에러의 도움말이 그것을 짚어 준다.

#### 같이 보기

[`send`](#fn-send), [`recv`](#fn-recv), [`close`](#fn-close), [`conc.fan_in`](../modules/conc.html#fn-conc-fan_in), [컬렉션](collections.html)

:::

:::fn send(tx, v)

보내는 끝 `tx`로 값 `v`를 보낸다. `Ok(nil)`, 닫혔으면 `Err({kind: 'closed})`.

#### 설명

버퍼가 없거나 찼으면 받는 쪽이 받아 갈 때까지 기다린다. 채널이 닫혔거나 받는 끝이 모두 사라졌으면 `Err({kind: 'closed})`를 돌려준다. 결과값이므로 문장으로 쓴 `send(tx, v)`가 닫힌 채널을 만나면 E0606으로 멈춘다. 받는 쪽이 먼저 그만둘 수 있으면 `match`로 가르거나, 그만둬도 되면 `_ = send(tx, v)`로 쓴다.

값은 불변이므로 복사 걱정 없이 무엇이든 보낸다. 다른 채널의 끝(응답을 받을 채널)도 보낼 수 있다. 다만 채널에 **그 채널 자신의** 끝을 보내면(어느 끝이든, `v` 안 어디에 들어 있어도) 런타임 에러 E0705다. fung은 메모리를 참조 계수로 회수하므로 자기 끝을 담은 채널은 닫히지도 해제되지도 않기 때문이다. 채널 끝 같은 바뀌는 것을 품지 않은 값은 이 검사를 위해 훑지 않으므로 큰 값을 보내도 느리지 않다.

#### 매개변수

- `tx`: 보내는 끝.
- `v`: 보낼 값.

#### 돌려주는 값

`Ok(nil)` 또는 `Err({kind: 'closed})`.

#### 에러

- `tx`가 보내는 끝이 아니면(받는 끝이면) 런타임 에러 E0403.
- 채널에 자기 끝을 보내면 E0705.
- 아무도 받지 않는데 기다리기만 하면 E0702(교착).

#### 예제

**버퍼와 닫힌 채널**

```fung run
(tx, rx) = channel(2)
print(send(tx, 1), send(tx, 2))
print(timeout(10ms, () => send(tx, 3)))
close(tx)
print(send(tx, 4))
```

버퍼 2개가 차자 세 번째 `send`는 기다리다 기한에 끊긴다.

**응답 채널을 함께 보내기**

```fung run
scope(fn(s):
  (req_tx, reqs) = channel()
  spawn(s, fn():
    for (n, reply) in reqs:
      send(reply, n * 2)!
    ;;
  ;;)
  (reply_tx, reply_rx) = channel(1)
  send(req_tx, (21, reply_tx))!
  print(recv(reply_rx)!)
;;)
```

**받는 쪽이 먼저 그만둘 때**

```fung run
scope(fn(s):
  (tx, rx) = channel()
  spawn(s, fn():
    print("got", recv(rx)!)
  ;;)
  send(tx, "a")!
  r = match send(tx, "b"):
    Ok(_)                => "sent"
    Err({kind: 'closed}) => "receiver gone"
  ;;
  print(r)
;;)
```

받는 태스크가 하나만 받고 끝나면서 받는 끝의 마지막 사본이 사라졌으므로, 두 번째 `send`는 기다리다 `Err({kind: 'closed})`를 받는다.

#### 같이 보기

[`recv`](#fn-recv), [`channel`](#fn-channel), [`close`](#fn-close)

:::

:::fn recv(rx)

받는 끝 `rx`에서 값 하나를 받는다. `Ok(v)`, 닫혔고 남은 값이 없으면 `Err({kind: 'closed})`.

#### 설명

값이 올 때까지 기다린다. 채널이 닫혀도 버퍼에 남은 값은 먼저 다 준 뒤에 `Err({kind: 'closed})`를 준다. 그래서 닫힘은 "더 올 것이 없다"는 정상적인 끝이다. 받는 끝을 끝까지 돌 때는 `recv`를 되풀이하는 대신 `for x in rx:`를 쓴다.

기다리는 시간은 [`timeout`](#fn-timeout)으로 정한다: `timeout(1s, () => recv(rx))`. 기한이 지나면 받지 않은 값은 채널에 그대로 남는다.

#### 매개변수

- `rx`: 받는 끝.

#### 돌려주는 값

`Ok(값)` 또는 `Err({kind: 'closed})`.

#### 에러

- `rx`가 받는 끝이 아니면 런타임 에러 E0403.
- 보낼 수 있는 태스크가 없는데 기다리면 E0702(교착).

#### 예제

**받기**

```fung run
(tx, rx) = channel(1)
send(tx, "hi")!
print(recv(rx)!)
```

**닫혀도 남은 값은 받는다**

```fung run
(tx, rx) = channel(3)
send(tx, 1)!
send(tx, 2)!
close(tx)
print(recv(rx), recv(rx), recv(rx))
```

**기한을 두고 받기**

```fung run
(tx, rx) = channel()
print(timeout(10ms, () => recv(rx)) ?? "nothing yet")
close(tx)
```

#### 같이 보기

[`send`](#fn-send), [`timeout`](#fn-timeout), [for](../language.html#for)

:::

:::fn close(tx)

보내는 끝 `tx`의 채널을 지금 닫는다.

#### 설명

그 끝의 다른 사본이 살아 있어도 지금 닫는다. 보통은 [자동 닫힘](#fn-channel)으로 충분하고, 최상위에서 만든 채널처럼 사본이 오래 사는 경우에 쓴다. 닫은 뒤 받는 쪽은 남은 값을 받은 뒤 `Err({kind: 'closed})`를 받고, 그 뒤의 `send`는 `Err({kind: 'closed})`다. 이미 닫힌 채널을 다시 닫아도 에러가 아니다.

받는 끝은 닫을 수 없다(E0403). 받기를 그만두려면 받는 끝을 버린다(마지막 사본이 사라지면 닫힌다). 파일 핸들은 [`fs.close`](../modules/fs.html#fn-fs-close)로 닫는다. 이름이 같지만 다른 함수다.

#### 매개변수

- `tx`: 보내는 끝.

#### 돌려주는 값

`nil`.

#### 에러

`tx`가 보내는 끝이 아니면 런타임 에러 E0403.

#### 예제

**최상위 채널을 닫고 끝까지 받기**

```fung run
(tx, rx) = channel(3)
each(["a", "b", "c"], x => send(tx, x))
close(tx)
for x in rx:
  print(x)
;;
```

**받는 끝은 닫지 못한다**

```fung run exit=1 error=E0403
(tx, rx) = channel()
close(rx)
```

#### 같이 보기

[`channel`](#fn-channel), [`send`](#fn-send), [`fs.close`](../modules/fs.html#fn-fs-close)

:::

## 정지 요청 {#stop}

:::fn stop_signal()

정지 요청을 받는 끝(`receiver`)을 돌려준다. 정지 요청이 오면 이 끝이 닫힌다.

#### 설명

정지 요청은 "하던 일을 마치고 끝내라"이다. 첫 SIGINT·SIGTERM(터미널의 Ctrl-C, `kill`, 컨테이너 종료)이나 [`request_stop`](#fn-request_stop)이 보낸다. 요청이 오면 이 끝이 닫히므로 `recv`는 `Err({kind: 'closed})`를 돌려주고, 이 끝을 도는 `for`는 끝난다. 그때까지 이 끝으로 오는 값은 없다. 기다리기만 할 때는 `_ = recv(stop)`으로 쓴다(문장으로 쓴 `recv(stop)`은 그 `Err`를 버리므로 E0606이다). 이것은 [취소](#fn-cancel)("지금 멈춰라")와 다르다. 받은 쪽이 스스로 정리하고 끝낸다.

이 끝의 사본이 하나라도 살아 있으면 "보는 쪽이 있다". 보는 쪽이 있으면 정지 요청은 이 끝을 닫기만 하고, 프로그램은 진입 파일이 끝날 때 끝난다(종료 코드 0). 보는 쪽이 없으면 정지 요청은 프로그램을 취소한다(`with`의 해제는 돈다, 종료 코드 130·143). 그래서 정지 요청을 모르는 스크립트의 Ctrl-C는 바로 멈춘다. 두 번째 신호는 해제를 기다리지 않고 곧바로 끝낸다(128 + 신호 번호). 이 끝에서만 기다리는 것은 교착이 아니다(신호가 깨운다). 정지 요청이 이미 왔으면 닫힌 끝을 준다.

[`http.serve`](../modules/http.html)는 이것으로 새 연결을 그만 받고 처리 중인 요청을 마무리한다.

#### 매개변수

없다.

#### 돌려주는 값

받는 끝.

#### 에러

없다.

#### 예제

**요청을 받고 하던 일을 마치기**

```fung run
stop = stop_signal()
request_stop()
print(recv(stop))
print("cleaning up, then exiting normally")
```

**일꾼이 정지 요청까지 돌기**

```fung run
stop = stop_signal()
scope(fn(s):
  spawn(s, fn():
    for _ in stop:
      nil
    ;;
    print("worker: finished current job, stopping")
  ;;)
  spawn(s, fn():
    sleep(10ms)
    request_stop()
  ;;)
;;)
print("done")
```

실제 서비스에서는 두 번째 태스크 대신 Ctrl-C나 `kill`이 요청을 보낸다.

#### 같이 보기

[`request_stop`](#fn-request_stop), [`cancel`](#fn-cancel), [`with`](program.html#fn-with), [`http.serve`](../modules/http.html)

:::

:::fn request_stop()

코드에서 정지 요청을 보낸다. 첫 신호(SIGINT·SIGTERM)와 똑같이 동작한다.

#### 설명

보는 쪽([`stop_signal`](#fn-stop_signal)의 끝)이 있으면 그 끝들을 닫고 `nil`을 돌려준다. 보는 쪽이 없으면 `exit(0)`처럼 프로그램을 끝낸다. 관리용 요청이나 헬스체크 실패로 스스로 내려가는 서비스, 정상 종료를 시험하는 `fung test`에 쓴다. `fung test`는 테스트 블록마다 정지 상태를 새로 둔다. REPL에서도 같다. 보는 쪽이 없으면 `exit(0)`처럼 세션을 끝낸다.

#### 매개변수

없다.

#### 돌려주는 값

`nil`(보는 쪽이 있을 때). 없으면 돌아오지 않는다.

#### 에러

없다.

#### 예제

**보는 쪽이 있을 때**

```fung run
stop = stop_signal()
print(request_stop())
print(recv(stop))
```

**보는 쪽이 없으면 프로그램이 끝난다**

```fung run
print("before")
request_stop()
print("not reached")
```

#### 같이 보기

[`stop_signal`](#fn-stop_signal), [`exit`](program.html#fn-exit)

:::

## 나누는 상태 {#atoms}

:::fn atom(v)

값 `v`를 담은 칸(`atom`)을 만든다. 태스크 사이에 나누는 바뀌는 상태는 이것으로 둔다.

#### 설명

fung의 값은 불변이고 이름은 다시 대입할 수 없으므로, 여러 태스크가 함께 보는 카운터나 캐시 같은 **바뀌는 상태**는 `atom`에 둔다. 칸 자체는 그대로이고 안의 값을 [`swap`](#fn-swap)·[`reset`](#fn-reset)으로 바꾸며, [`deref`](#fn-deref)로 읽는다. 출력은 `<atom>`(값을 보이지 않는다), `type_of`는 `'atom`, 타입 표기는 `a: atom`, `==`는 같은 칸인지를 본다(같은 값을 담은 다른 칸은 다르다).

안에는 바뀌는 것(atom, 채널 끝, 태스크·범위 핸들, 파일, 소켓, 그리고 그것들을 품은 리스트·맵·튜플·태그·클로저)을 넣지 못한다(E0708). 상태는 평범한 불변 값이어야 하기 때문이다. 이 검사는 값이 커도 빠르다.

먼저 다른 방법을 생각한다. 태스크의 결과는 `await`로 모으고, 흐름은 채널로 나르는 것이 대개 더 단순하다. `atom`은 정말로 여러 태스크가 같은 상태를 봐야 할 때 쓴다.

#### 매개변수

- `v`: 처음 값. 바뀌는 것을 품지 않은 값.

#### 돌려주는 값

atom.

#### 에러

`v`가 바뀌는 것을 품고 있으면 런타임 에러 E0708.

#### 예제

**만들고 읽기**

```fung run
a = atom(0)
print(a, type_of(a), deref(a), a == a, atom(0) == atom(0))
```

**여러 태스크가 함께 세기**

```fung run
hits = atom(0)
scope(fn(s):
  for _ in 1..=100:
    spawn(s, () => swap(hits, n => n + 1))
  ;;
;;)
print(deref(hits))
```

**바뀌는 것은 넣지 못한다**

```fung run exit=1 error=E0708
(tx, _rx) = channel()
registry = atom({main: tx})
```

#### 같이 보기

[`swap`](#fn-swap), [`reset`](#fn-reset), [`deref`](#fn-deref), [`channel`](#fn-channel)

:::

:::fn swap(a, f)

`f(지금 값)`으로 atom의 값을 바꾸고 새 값을 돌려준다. 여러 태스크가 동시에 바꿔도 갱신이 사라지지 않는다.

#### 설명

**비교 후 교체**(compare-and-swap)로 동작한다. 지금 값을 읽어 `f`로 새 값을 계산하고, 그사이 아무도 바꾸지 않았으면 바꾼다. 누가 먼저 바꿨으면 그 새 값으로 `f`를 다시 돌린다. 그래서 `deref`로 읽고 `reset`으로 쓰는 두 단계 사이에 다른 태스크가 끼어들어 갱신이 사라지는 일이 없다.

`f`는 여러 번 돌 수 있으므로 **순수 계산만** 한다. 출력, 표준 입력, `exit`, 태스크·채널 연산, `sleep`, 다른 atom의 `swap`·`reset`, 파일·네트워크·외부 프로세스를 부르면 런타임 에러 E0707이다. 에러는 부른 함수의 이름을 댄다(`fs.with_open`, `http.get`, `print` 등). 읽기(`deref`, 시간, 환경 변수)는 된다. `f`가 실패하면 상태는 그대로다.

#### 매개변수

- `a`: atom.
- `f`: 지금 값을 받아 새 값을 돌려주는 순수 함수.

#### 돌려주는 값

새 값.

#### 에러

- `f` 안의 부수효과는 런타임 에러 E0707.
- 새 값이 바뀌는 것을 품으면 E0708.

#### 예제

**세기**

```fung run
hits = atom(0)
each(1..=3, _ => swap(hits, n => n + 1))
print(deref(hits))
```

**맵 상태 갱신하기**

```fung run
stats = atom({ok: 0, failed: 0})
for code in [200, 500, 200]:
  key = code < 400 then "ok" else "failed"
  swap(stats, m => put(m, key, m[key] + 1))
;;
print(deref(stats))
```

**f 안에서 출력하면 에러**

```fung run exit=1 error=E0707
a = atom(0)
swap(a, fn(n):
  print("updating")
  n + 1
;;)
```

#### 같이 보기

[`reset`](#fn-reset), [`deref`](#fn-deref), [`atom`](#fn-atom)

:::

:::fn reset(a, v)

atom의 값을 `v`로 바꾸고 `v`를 돌려준다.

#### 설명

지금 값과 상관없이 덮어쓴다. 지금 값을 바탕으로 바꾸는 일은 [`swap`](#fn-swap)으로 한다(`reset(a, deref(a) + 1)`은 그사이 다른 태스크의 갱신을 잃을 수 있다). 입출력이 필요한 갱신, 예를 들어 주기적으로 설정 파일을 다시 읽는 일은 갱신 태스크 하나가 읽어서 `reset`하고 나머지는 `deref`하는 모양으로 쓴다.

#### 매개변수

- `a`: atom.
- `v`: 새 값. 바뀌는 것을 품지 않은 값.

#### 돌려주는 값

`v`.

#### 에러

`v`가 바뀌는 것을 품으면 런타임 에러 E0708.

#### 예제

**덮어쓰기**

```fung run
cfg = atom({version: 1})
reset(cfg, {version: 2})
print(deref(cfg))
```

**갱신 태스크 하나, 읽는 쪽 여럿**

```fung run
current = atom("v1")
scope(fn(s):
  spawn(s, fn():
    sleep(10ms)
    reset(current, "v2")
  ;;)
  print("before:", deref(current))
  sleep(300ms)
  print("after:", deref(current))
;;)
```

#### 같이 보기

[`swap`](#fn-swap), [`deref`](#fn-deref)

:::

:::fn deref(a)

atom의 지금 값을 돌려준다. 기다리지 않는다.

#### 설명

읽는 순간의 값이다. 돌려받은 값은 불변이라 그 뒤에 다른 태스크가 atom을 바꿔도 이미 받은 값은 바뀌지 않는다.

#### 매개변수

- `a`: atom.

#### 돌려주는 값

지금 값.

#### 에러

`a`가 atom이 아니면 런타임 에러 E0403.

#### 예제

**읽은 값은 바뀌지 않는다**

```fung run
a = atom([1, 2])
before = deref(a)
swap(a, xs => xs + [3])
print(before, deref(a))
```

#### 같이 보기

[`atom`](#fn-atom), [`swap`](#fn-swap)

:::

## 기다리기 {#sleep}

:::fn sleep(d)

기간 `d`만큼 기다린다. 그동안 다른 태스크가 돈다.

#### 설명

지금 태스크만 세워 두고 다른 태스크는 계속 돈다. 취소되면(`timeout`, `cancel`, `exit`) 곧바로 깨어나 취소로 되감긴다. `d`가 0 이하면 기다리지 않는다. 재시도 사이의 간격이나 주기적인 작업에 쓴다. 다른 태스크를 기다리는 데 `sleep`을 쓰지는 않는다. 그럴 때는 `await`나 채널로 기다려야 정확하다.

#### 매개변수

- `d`: 기간(`dur`). 수가 아니다(`sleep(1)`은 에러, `sleep(1s)`).

#### 돌려주는 값

`nil`.

#### 에러

`d`가 기간이 아니면 런타임 에러 E0403.

#### 예제

**잠깐 기다리기와 기한**

```fung run
print(sleep(10ms))
print(timeout(10ms, () => sleep(1s)))
```

**주기적으로 하기**

```fung run
fn poll(n):
  if n == 0: return "done" ;;
  print("poll", n)
  sleep(10ms)
  poll(n - 1)
;;

print(poll(3))
```

**수가 아니라 기간**

```fung run exit=1 error=E0403
sleep(1)
```

#### 같이 보기

[`timeout`](#fn-timeout), [`parse_dur`](convert.html#fn-parse_dur), [`conc.retry`](../modules/conc.html#fn-conc-retry)

:::
