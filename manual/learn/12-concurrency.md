---
description: 태스크와 태스크 범위(scope·spawn·await), 실패를 격리하는 supervise, 기한 timeout, 취소, 채널, atom, conc.pmap.
lede: 서버 스무 대에 차례로 접속하면 기다리는 시간이 스무 배가 된다. 이 장은 여러 일을 동시에 진행시키는 법, 그리고 그때 생기는 실패·기한·공유 상태를 안전하게 다루는 법을 배운다.
---
# 동시에 여러 일 하기

:::goals
- 동시성이 무엇이고 언제 도움이 되는지
- `conc.pmap`으로 리스트의 일을 한꺼번에 처리하기
- 태스크 범위와 태스크: `scope`, `spawn`, `await`
- 실패를 다루기: 태스크 범위의 실패, `supervise`, `timeout`, 취소
- 태스크끼리 값을 주고받는 채널, 함께 쓰는 값 `atom`
:::

## 기다리는 시간을 겹치기 {#why}

스크립트가 쓰는 시간의 대부분은 계산이 아니라 **기다림**이다. HTTP 응답을 기다리고, `ssh` 명령이 끝나기를 기다리고, 디스크를 기다린다. 서버 다섯 대에 1초씩 걸리는 확인을 차례로 하면 5초가 걸린다. 하지만 다섯 대에 한꺼번에 물어 놓고 답을 기다리면 1초 남짓이면 된다. 기다리는 시간을 서로 겹치는 것, 이것이 **동시성**(concurrency)이다.

동시에 진행되는 일 하나하나를 **태스크**(task)라고 한다. 태스크는 함수 하나를 따로 실행하는 흐름이다. fung은 한 번에 한 태스크의 코드만 실행하고, 태스크가 무언가를 기다리기 시작하면 그사이 다른 태스크를 실행한다. 그래서 기다림이 많은 일(네트워크, 외부 명령, 파일)에서 크게 빨라진다. 반면 계산만 하는 일은 태스크를 나눠도 빨라지지 않는다. CPU 코어 여러 개를 함께 쓰는 **병렬성**(parallelism)과는 다른 이야기다.

이 장의 예제는 진짜 서버 대신 `sleep(기간)`으로 기다림을 흉내 낸다. `sleep(200ms)`는 그 태스크를 0.2초 동안 재운다.

## 가장 쉬운 길: conc.pmap {#pmap}

리스트의 원소마다 같은 일을 하고 결과를 모으는 것은 4장의 `map`이다. `conc` 모듈의 `conc.pmap(리스트, 함수)`는 원소마다 태스크를 띄워 **동시에** `map`한다. 결과는 `map`처럼 입력 순서대로다. 셋을 차례로 하면 0.6초 걸릴 일이 0.2초 남짓에 끝나는지 재 보자.

```fung run
import conc
import time

fn check(host):
  sleep(200ms)
  "${host}: ok"
;;

start = time.now()
results = conc.pmap(["web1", "web2", "db1"], check)
elapsed = time.now() - start

print(results)
print("faster than one by one:", elapsed < 600ms)
```

함수가 `Err`를 돌려주면 그 원소의 결과가 `Err`일 뿐, 나머지는 그대로 모인다(9장의 결과값). 서버 확인처럼 일부가 실패해도 전체 목록을 보고 싶을 때 맞는 모양이다. 한 번에 도는 태스크 수는 `limit:`으로 묶는다. 서버 수백 대에 한꺼번에 접속하지 않으려면 `limit: 10`처럼 준다.

```fung run
import conc

fn ping(host):
  sleep(10ms)
  host == "db1" then Err({kind: 'down, message: "${host} unreachable"}) else Ok(host)
;;

for r in conc.pmap(["web1", "web2", "db1", "web3"], ping, limit: 2):
  match r:
    Ok(host) => print("up  ", host)
    Err(e)   => print("DOWN", e.message)
  ;;
;;
```

## 태스크 범위와 태스크 {#scope}

`pmap`보다 자유롭게 태스크를 다루려면 직접 띄운다. fung에서 태스크는 반드시 **태스크 범위**(scope) 안에서 태어난다. `scope(fn(s): … ;;)`는 범위 `s`를 열고 본문을 실행한다. 본문에서 `spawn(s, 함수)`로 그 범위에 태스크를 띄우고, `await(태스크)`로 그 태스크가 끝나기를 기다려 결과를 받는다.

```fung run
fn fetch(name, d):
  sleep(d)
  "${name} (${d})"
;;

r = scope(fn(s):
  users = spawn(s, () => fetch("users", 50ms))
  orders = spawn(s, () => fetch("orders", 20ms))
  {users: await(users), orders: await(orders)}
;;)
print(r)
```

`scope`의 가장 중요한 약속은 **태스크 범위는 그 안의 태스크가 모두 끝나야 돌아온다**는 것이다. `await`하지 않은 태스크도 범위가 끝날 때까지 기다린다. 그래서 `scope(` 다음 줄에 이르렀다면 그 안에서 시작한 일은 모두 끝났다. 어딘가에서 몰래 계속 도는 태스크가 없다. 본문이 먼저 끝나도, 늦은 태스크를 기다린 뒤에 `after scope`가 찍히는 것을 보자.

```fung run
scope(fn(s):
  spawn(s, fn():
    sleep(300ms)
    print("slow task done")
  ;;)
  spawn(s, fn():
    sleep(10ms)
    print("fast task done")
  ;;)
  print("body done")
;;)
print("after scope")
```

:::box learn 구조적 동시성
태스크의 수명을 코드 블록에 묶는 방식을 **구조적 동시성**(structured concurrency)이라고 한다. 함수 호출이 "부르면 끝나고 돌아온다"를 보장하듯, 태스크 범위는 "열면 그 안의 동시 작업이 다 끝나고 돌아온다"를 보장한다. 아무도 기다리지 않는 태스크가 실패를 삼키거나, 프로그램이 끝난 줄 알았는데 무언가 계속 도는 문제가 생기지 않는다.
:::

## 실패하는 태스크 {#failure}

태스크 하나가 런타임 에러로 죽으면 어떻게 될까? 태스크 범위는 나머지 태스크를 **취소**(cancel, 하던 일을 그 자리에서 그만두게 함)하고, 그 에러를 `scope(`를 연 곳에서 다시 낸다. 실패는 사라지지 않고 늘 범위를 연 코드에 닿는다. 아래에서 1초를 자는 태스크는 `never`를 찍지 못한다.

```fung run exit=1 error=E0502
r = scope(fn(s):
  spawn(s, () => 1 // 0)
  spawn(s, fn():
    sleep(1s)
    print("never")
  ;;)
  "body"
;;)
print(r)
```

에러 메시지의 `note: scope opened at …`가 실패가 올라온 태스크 범위를 알려 준다. 반면 태스크의 함수가 런타임 에러가 아니라 `Err` 값을 돌려주는 것은 실패가 아니다. 그냥 `await`가 받는 결과일 뿐이다.

### 실패를 격리하기: supervise {#supervise}

요청을 하나씩 처리하는 서버라면 요청 하나의 버그로 서버 전체가 멈추면 곤란하다. `supervise(본문, on_fail)`는 `scope`처럼 태스크 범위를 열되, 자식 태스크의 실패를 범위의 실패로 만들지 않고 `on_fail(실패)`에 넘긴다. 다른 태스크는 계속 돈다. 실패는 `{kind, code, message, at}` 맵이다.

```fung run
r = supervise(fn(s):
  spawn(s, () => 1 // 0)
  spawn(s, fn():
    sleep(10ms)
    print("other task finished")
  ;;)
  "body finished"
;;, e => eprint("task failed:", e.code, e.message, "at", e.at))
print(r)
```

## 기한: timeout {#timeout}

응답하지 않는 서버를 영원히 기다릴 수는 없다. `timeout(기간, 함수)`는 함수를 실행하되, 기간이 지나면 그 안의 일을 모두 취소하고 `Err({kind: 'timeout})`을 돌려준다. 기간 안에 끝나면 함수의 값을 그대로 돌려준다.

```fung run
fn answer(d):
  sleep(d)
  "answered after ${d}"
;;

print(timeout(1s, () => answer(10ms)))
print(timeout(1s, () => answer(5s)))
```

기한은 그 안에서 띄운 태스크, 외부 명령, HTTP 요청(13장)까지 모두에 걸린다. 기한이 지났을 때 도는 외부 명령은 끝내 준다(먼저 SIGTERM, 2초 안에 안 끝나면 SIGKILL). 그래서 `ssh`가 걸려 버린 서버도 정해진 시간 안에 포기할 수 있다.

```fung run
print(timeout(200ms, () => sh("sleep 10; echo finished")))
```

`pmap`과 함께 쓰면 "서버마다 2초씩만 기다린다"가 된다. 서버마다의 결과에 시간 초과가 `Err`로 섞인다.

```fung run
import conc

fn probe(host):
  sleep(host == "slow-db" then 5s else 20ms)
  Ok("${host} ok")
;;

results = conc.pmap(["web1", "slow-db", "web2"], h => timeout(1s, () => probe(h)))
print(results)
```

### 취소 {#cancel}

`cancel(태스크)`는 그 태스크를 취소하고, `cancel(태스크 범위)`는 그 범위의 본문과 모든 자식을 취소한다. 취소된 태스크를 `await`하면 `Err({kind: 'cancelled})`를 받는다. "결과를 하나 받았으니 나머지는 그만"처럼 쓴다.

```fung run
scope(fn(s):
  t = spawn(s, fn():
    sleep(1s)
    print("never")
  ;;)
  cancel(t)
  print(await(t))
;;)
```

## 채널로 주고받기 {#channels}

태스크끼리 값을 넘기는 통로가 **채널**(channel)이다. `channel()`은 두 끝을 튜플로 준다. 보내는 끝(`tx`)에는 `send(tx, 값)`을, 받는 끝(`rx`)에는 `recv(rx)`나 `for x in rx`를 쓴다. 받는 쪽은 값이 올 때까지 기다린다. `for x in rx`는 채널이 **닫힐** 때까지 값마다 돈다.

채널은 보내는 끝의 사본이 모두 사라지면 저절로 닫힌다. 그래서 채널을 만드는 함수 안에서 보내는 끝을 생산 태스크에만 넘기면, 생산이 끝날 때 채널이 닫히고 받는 `for`도 끝난다.

```fung run
fn countdown(s, n):
  (tx, rx) = channel()
  spawn(s, fn():
    for i in reverse(1..=n):
      send(tx, i)!
    ;;
  ;;)
  rx
;;

scope(fn(s):
  for x in countdown(s, 3):
    print("got", x)
  ;;
;;)
```

`reverse(1..=n)`은 `n`부터 1까지 거꾸로 센 리스트다. `send`는 받는 쪽이 모두 사라진 채널이면 `Err({kind: 'closed})`를 돌려주므로 결과를 `!`로 다룬다.

채널로 일감을 나누는 **일꾼**(worker) 모양도 흔하다. 일감 채널 하나를 일꾼 여럿이 함께 받고, 결과는 다른 채널로 모은다.

```fung run
fn worker(id, jobs, results):
  for job in jobs:
    sleep(10ms)
    send(results, "worker ${id} did job ${job}")!
  ;;
;;

done = scope(fn(s):
  (jobs_tx, jobs_rx) = channel()
  (res_tx, res_rx) = channel()
  each([1, 2], id => spawn(s, () => worker(id, jobs_rx, res_tx)))
  spawn(s, () => each(1..=4, j => send(jobs_tx, j)!))
  for r in res_rx: r ;;
;;)
print(len(done), "jobs done")
```

:::box mistake 보내는 끝을 쥔 채 기다리기
채널을 최상위나 받는 함수 안에서 만들고 그 함수가 보내는 끝을 계속 쥐고 있으면, 채널은 닫히지 않는다. 받는 `for`는 영원히 기다리고, 다른 태스크도 모두 기다리는 중이면 fung은 E0702(`deadlock: all tasks are waiting`, 교착)로 멈추고 누가 보내는 끝을 쥐었는지 도움말로 알려 준다. 보내는 끝은 생산하는 쪽에만 넘기거나, 다 보냈으면 `close(tx)`로 직접 닫는다.

```fung run exit=1 error=E0702
(tx, rx) = channel()
scope(fn(s):
  spawn(s, () => each(["a", "b"], x => send(tx, x)!))
  for x in rx:
    print(x)
  ;;
;;)
```

:::

## 함께 쓰는 값: atom {#atom}

여러 태스크가 한 값을 함께 고쳐야 할 때가 있다(처리한 요청 수, 캐시). fung의 리스트와 맵은 한 번 만들면 바뀌지 않으므로(7장), 바뀌는 칸이 따로 필요하다. **atom**이 그 칸이다. `atom(처음 값)`으로 만들고, `deref(a)`로 지금 값을 읽고, `swap(a, 함수)`로 "지금 값을 받아 새 값을 돌려주는 함수"를 적용해 바꾼다. 여러 태스크가 동시에 `swap`해도 바꿈이 사라지지 않는다.

```fung run
done = atom(0)

scope(fn(s):
  each(1..=50, _ => spawn(s, () => swap(done, n => n + 1)))
;;)
print(deref(done), done)
```

`swap`은 다른 태스크와 부딪히면 함수를 다시 실행할 수 있다. 그래서 `swap`에 주는 함수 안에서는 `print`, 파일 쓰기, 명령 실행 같은 **효과**(effect, 바깥에 흔적을 남기는 일)를 쓸 수 없다. 쓰면 E0707이다. 값만 계산하고, 효과는 `swap` 밖에서 한다. 바꿀 값을 미리 정했으면 `reset(a, 값)`으로 통째로 바꾼다.

```fung run exit=1 error=E0707
hits = atom(0)
swap(hits, fn(n):
  print("old value", n)
  n + 1
;;)
```

:::box other 다른 언어는
Python의 `threading`·`asyncio`, JavaScript의 `Promise`·`async/await`, Java의 스레드는 태스크를 아무 데서나 띄울 수 있다. fung의 태스크 범위는 Python Trio의 nursery, Kotlin의 `coroutineScope`, Java 21의 `StructuredTaskScope`와 같은 구조적 동시성이다. `atom`과 `swap`은 Clojure의 atom에서, 실패를 격리하는 `supervise`는 Erlang의 supervisor에서 생각을 빌렸다. 쉘의 `cmd &`와 `wait`도 동시성이지만, 실패한 작업의 종료 코드를 모으는 일은 손으로 해야 한다.
:::

:::exercise 서버 상태 표
서버 이름 리스트를 받아, 서버마다 동시에 확인하고 `web1  up` / `db1   TIMEOUT`처럼 한 줄씩 출력해 보자. 확인 함수 `probe(host)`는 이미 있다고 하자(아래 풀이는 `sleep`으로 흉내 낸다). 한 서버에 0.5초 넘게 걸리면 `TIMEOUT`이다. 마지막에 실패한 서버가 있으면 종료 코드 1로 끝낸다.

:::solution

```fung run exit=1
import conc

fn probe(host):
  sleep(host == "db1" then 5s else 10ms)
  Ok("up")
;;

hosts = ["web1", "web2", "db1"]
results = conc.pmap(hosts, h => timeout(500ms, () => probe(h)))
rows = zip(hosts, results)
for (host, r) in rows:
  status = match r:
    Ok(s)                 => s
    Err({kind: 'timeout}) => "TIMEOUT"
    Err(e)                => "ERROR ${e.message}"
  ;;
  print(pad_right(host, 6), status)
;;
if any(results, is_err): exit(1) ;;
```

:::
:::

:::exercise 줄 세는 일꾼
채널로 문자열 리스트를 일꾼 태스크 하나에 보내고, 일꾼이 받은 줄들의 글자 수 합을 돌려주게 해 보자. 본문은 일꾼을 `spawn`하고, 줄을 다 보낸 뒤 `await`로 합계를 받는다. 힌트: 다 보냈으면 `close(tx)`로 닫아야 일꾼의 `for`가 끝난다.

:::solution

```fung run
fn count_chars(rx) = fold(rx, 0, (acc, row) => acc + len(row))

total = scope(fn(s):
  (tx, rx) = channel()
  counter = spawn(s, () => count_chars(rx))
  each(["alpha", "beta", "gamma"], row => send(tx, row)!)
  close(tx)
  await(counter)
;;)
print(total)
```

:::
:::

## 정리 {#summary}

- 동시성은 기다리는 시간을 겹치는 것이다. 네트워크·명령·파일처럼 기다림이 많은 일이 빨라진다.
- 리스트의 일을 동시에 하려면 `conc.pmap(xs, f, limit: n)`. 결과는 입력 순서대로다.
- 태스크는 `scope` 안에서 `spawn(s, f)`로 띄우고 `await`로 결과를 받는다. 태스크 범위는 모든 태스크가 끝나야 돌아온다.
- 태스크의 런타임 에러는 태스크 범위를 실패시키고 나머지를 취소한다. 격리하려면 `supervise(f, on_fail)`.
- `timeout(d, f)`는 기한이 지나면 안의 일을 모두 취소하고 `Err({kind: 'timeout})`. `cancel`로 직접 취소한다.
- 채널 `(tx, rx) = channel()`로 값을 주고받는다. 보내는 끝이 모두 사라지면 닫힌다. 쥔 채 기다리면 E0702.
- 함께 고치는 값은 `atom`·`deref`·`swap`. `swap`의 함수에는 효과를 쓸 수 없다(E0707).

함수마다의 규칙은 레퍼런스의 [동시성](../reference/prelude/concurrency.html)과 [conc](../reference/modules/conc.html)에 있다. 에러는 [E0702](../reference/errors.html#E0702), [E0707](../reference/errors.html#E0707)을 보라. fung이 태스크를 어떻게 번갈아 실행하는지는 VM 책의 [스케줄러](../vm/12-scheduler.html)와 [취소](../vm/14-cancellation.html)에서 다룬다. 실무 예는 예제 모음의 [여러 서버에 ssh 보내기](../recipes/ssh-collect.html)를 보라.
