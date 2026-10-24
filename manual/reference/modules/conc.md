---
lede: <code>import conc</code>. prelude의 동시성 함수(<code>scope</code>, <code>spawn</code>, <code>channel</code> …) 위에 fung으로 쓴 조합 세 개다. 여러 일을 한꺼번에 돌리고(<code>pmap</code>), 여러 채널을 하나로 모으고(<code>fan_in</code>), 실패하면 다시 해 본다(<code>retry</code>).
description: fung 모듈 conc — 병렬 map, 채널 합치기, 재시도(pmap, fan_in, retry).
---
# conc — 동시성 도우미

## 동시성 도우미 {#concepts}

서버 스무 대에 같은 명령을 보내는 스크립트를 생각해 보자. 한 대가 1초씩 걸리면 차례로 하면 20초, 한꺼번에 하면 1초 남짓이다. 이렇게 여러 일을 겹쳐서 진행하는 것이 **동시성**(concurrency)이다. 기다리는 시간이 대부분인 일(네트워크, 외부 명령, 파일)에서 효과가 크다.

fung에서 동시에 도는 일 하나하나는 **태스크**(task)다. 태스크는 [`spawn`](../prelude/concurrency.html#fn-spawn)으로 띄우고, 반드시 **범위**(scope, [`scope`](../prelude/concurrency.html#fn-scope)) 안에서 돈다. 범위는 안에서 띄운 태스크가 모두 끝나야 끝나므로 "띄워 놓고 잊은" 태스크가 남지 않는다. 태스크끼리 값을 주고받을 때는 **채널**(channel, [`channel`](../prelude/concurrency.html#fn-channel))을 쓴다. 이 규칙은 [동시성 함수](../prelude/concurrency.html)와 [12장 동시에 여러 일 하기](../../learn/12-concurrency.html)에 있다.

`conc` 모듈은 이 기본 함수로 자주 쓰는 모양 세 가지를 만들어 둔 것이다. 모두 fung으로 썼고(`extras/conc.fung`), prelude에는 모든 스크립트에 필요한 것만 두므로 모듈로 뺐다. `import conc` 없이 `pmap`을 부르면 E0204이고, 도움말이 모듈 이름을 알려 준다.

fung의 태스크는 실행 스레드 하나 위에서 번갈아 돈다. 그래서 CPU 계산 자체가 빨라지지는 않고, 기다리는 일(외부 명령, 네트워크, `sleep`)을 겹치는 데 쓴다. 자세한 원리는 VM 책의 [태스크와 스케줄러](../../vm/12-scheduler.html)에 있다.

:::fn conc.pmap(xs, f, limit = 0) fung

리스트의 원소마다 `f`를 동시에 돌리고, 결과를 입력 순서대로 모은다.

#### 설명

`map(xs, f)`의 동시 실행판이다. 원소마다 태스크를 하나씩 띄워 `f(x)`를 돌리고, 모두 끝나면 결과를 **입력 순서대로** 리스트로 돌려준다(끝난 순서가 아니다). 자기만의 범위 안에서 돌므로, `pmap`이 돌아올 때 남은 태스크는 없다.

`limit`은 한꺼번에 도는 태스크의 수다. `0`(기본)이면 제한 없이 모두 한꺼번에 띄운다. 양수면 작업자 `limit`개가 원소를 앞에서부터 하나씩 집어 가고, 하나를 끝낸 작업자가 곧바로 다음 원소를 집는다. 그래서 느린 원소 하나는 자기 작업자만 붙잡고 나머지는 계속 돈다. 서버 수백 대에 접속할 때 상대 쪽이나 자기 쪽(열린 연결 수)을 지키려면 `limit`을 준다.

실패는 두 가지로 다르게 다룬다.

- `f`가 **런타임 에러**를 내면(버그, `!`가 `Err`를 만남, `fail`) 그 범위의 다른 태스크를 취소하고 `pmap` 전체가 그 에러로 실패한다.
- `f`가 **`Err` 값을 돌려주면** 그것은 그냥 결과 하나다. 다른 원소는 계속되고, 결과 리스트에 `Err`가 그대로 들어간다.

그래서 "한 서버가 실패해도 나머지는 받고 싶다"면 `f` 안에서 `!` 대신 결과값을 그대로 돌려준다. 원소마다 시간 제한을 두려면 `f` 안에서 [`timeout`](../prelude/concurrency.html#fn-timeout)을 쓴다.

#### 매개변수

- `xs`: 리스트(또는 `range` 같은, `map`할 수 있는 값).
- `f`: 원소 하나를 받는 함수.
- `limit`: 한꺼번에 돌릴 수. `0`이면 제한 없음.

#### 돌려주는 값

`f`의 결과 리스트(입력 순서).

#### 에러

- `f`의 런타임 에러가 그대로 `pmap`의 에러가 된다.
- `xs`가 `map`할 수 없는 값이면 런타임 에러 E0506.

#### 예제

**기본**

```fung run
import conc

print(conc.pmap([1, 2, 3], x => x * x))
print(conc.pmap(range(1, 6), x => x * 10, limit: 2))
```

**`Err`는 결과, 런타임 에러는 실패**

```fung run
import conc

r = conc.pmap(["a", "bad", "c"], x => x == "bad" then Err("cannot reach ${x}") else Ok(x))
print(r)
print(r |> filter(is_err) |> len, "failed")
```

```fung run exit=1 error=E0502
import conc

print(conc.pmap([5, 0, 1], x => 10 // x))
```

**원소마다 시간 제한**

```fung run
import conc

fn check(host):
  sleep(host == "slow" then 10s else 1ms)
  "${host} ok"
;;

print(conc.pmap(["a", "slow", "b"], h => timeout(1s, () => check(h))))
```

**서버 여러 대에 명령 보내기**

실제 서버가 필요해서 이 예는 검사만 한다.

```fung check
import conc

hosts = ["web-1", "web-2", "db-1"]
results = conc.pmap(hosts, h => timeout(5s, () => exec(["ssh", h, "uptime"])), limit: 10)
for (h, r) in zip(hosts, results):
  match r:
    Ok(out) => print(h, trim(out.out))
    Err(e)  => print(h, "FAILED:", e.kind)
  ;;
;;
```

```expect
web-1 10:02:11 up 31 days,  2:10,  0 users,  load average: 0.08, 0.03, 0.01
web-2 10:02:11 up 31 days,  2:09,  0 users,  load average: 0.00, 0.01, 0.00
db-1 FAILED: timeout
```

#### 같이 보기

[`scope`](../prelude/concurrency.html#fn-scope), [`spawn`](../prelude/concurrency.html#fn-spawn), [`timeout`](../prelude/concurrency.html#fn-timeout), [`exec`](../prelude/process.html#fn-exec), [여러 서버에 명령 보내기](../../recipes/ssh-collect.html)

:::

:::fn conc.fan_in(s, rxs) fung

여러 받는 끝(receiver)을 하나로 합친 받는 끝을 돌려준다.

#### 설명

채널의 받는 끝 여럿(`rxs`)에서 오는 값을 모두 받아 새 받는 끝 하나로 넘겨 준다. 받는 쪽은 그 하나만 `recv`하면 된다. 값은 도착한 순서로 섞여 나오고, 같은 입력 안의 순서는 지켜진다. 입력마다 값을 넘겨 주는 태스크가 범위 `s` 안에서 돈다. 모든 입력이 닫히면 돌려준 받는 끝도 닫힌다(`recv`가 `Err({kind: 'closed})`, `for`가 끝난다).

받는 쪽이 먼저 그만두면(돌려준 받는 끝이 사라지면) 넘겨 주던 태스크가 멈추고 입력을 놓는다. 그러면 입력 쪽 생산자의 `send`도 `Err`를 받으므로, 생산자가 그것을 보고 멈출 수 있다.

#### 매개변수

- `s`: 넘겨 주는 태스크가 돌 범위(`scope`가 넘겨 준 값).
- `rxs`: 받는 끝의 리스트.

#### 돌려주는 값

받는 끝(`receiver`) 하나.

#### 예제

**두 생산자의 값을 한 곳에서 받기**

```fung run
import conc

fn produce(s, name, n):
  (tx, rx) = channel(1)
  spawn(s, fn():
    each(range(1, n + 1), i => send(tx, "${name}-${i}")!)
    close(tx)
  ;;)
  rx
;;

got = scope(fn(s):
  merged = conc.fan_in(s, [produce(s, "web", 2), produce(s, "db", 1)])
  for v in merged:
    v
  ;;
;;)
print(sort(got))
```

어느 생산자의 값이 먼저 오는지는 정해지지 않으므로 정렬해서 보였다.

#### 같이 보기

[`channel`](../prelude/concurrency.html#fn-channel), [`recv`](../prelude/concurrency.html#fn-recv), [`close`](../prelude/concurrency.html#fn-close), [`scope`](../prelude/concurrency.html#fn-scope)

:::

:::fn conc.retry(f, times = 3, delay = 0ms) fung

`f()`가 `Err`가 아닌 값을 돌려줄 때까지 몇 번 다시 부른다.

#### 설명

`f()`를 부르고, 결과가 `Err`면 `delay`만큼 쉰 뒤 다시 부른다. 최대 `times`번 부르고, `Err`가 아닌 값이 나오면 곧바로 그 값을 돌려준다. 끝내 모두 `Err`면 **마지막** `Err`를 돌려준다. `times`가 1 이하면 한 번만 부른다. 일시적인 네트워크 오류나 아직 뜨지 않은 서비스처럼 "잠시 뒤에는 될 수 있는" 실패에 쓴다.

`f`가 런타임 에러를 내면 다시 부르지 않고 그대로 실패한다. 다시 해 볼 실패는 `Err` 값으로 돌려줘야 한다. 쉬는 시간은 매번 같다(점점 늘리는 방식은 직접 쓴다).

#### 매개변수

- `f`: 인자 없는 함수.
- `times`: 최대 시도 횟수.
- `delay`: 시도 사이에 쉬는 기간.

#### 돌려주는 값

`f`가 처음 돌려준 `Err`가 아닌 값, 또는 마지막 `Err`.

#### 예제

**세 번째에 성공**

```fung run
import conc

tries = atom(0)
r = conc.retry(fn():
  n = swap(tries, x => x + 1)
  print("try", n)
  n < 3 then Err({kind: 'network, message: "connection refused"}) else Ok("connected")
;;, times: 5, delay: 10ms)
print(r)
```

**끝내 실패하면 마지막 `Err`**

```fung run
import conc

print(conc.retry(() => Err("still down"), times: 2))
print(conc.retry(() => 42))
```

**HTTP 요청 다시 하기**

네트워크가 필요해서 이 예는 검사만 한다.

```fung check
import conc
import http

r = conc.retry(() => http.get("https://example.com", timeout: 2s), times: 5, delay: 1s)
match r:
  Ok(resp) => print("status", resp.status)
  Err(e)   => print("gave up:", e.message)
;;
```

```expect
status 200
```

#### 같이 보기

[`timeout`](../prelude/concurrency.html#fn-timeout), [`sleep`](../prelude/concurrency.html#fn-sleep), [`until`](../prelude/loops.html#fn-until), [실패·무응답 서버만 알리기](../../recipes/unreachable-servers.html)

:::

배우는 순서로는 [12장 동시에 여러 일 하기](../../learn/12-concurrency.html)를 본다.
