---
description: 취소와 정리. 구조적 동시성(태스크 범위)과 취소 트리, 취소가 도는 코드와 기다리는 코드에 닿는 방법, 블록에 거는 기한 timeout, 무슨 일이 있어도 도는 정리 with, 취소와 다른 정지 요청, 신호, exit.
lede: fung의 태스크는 태스크 범위 안에서만 태어나고, 범위는 취소 트리의 노드 하나다. 기한도 그 노드에 실리고, 정리는 되감기가 반드시 지나가는 내장 함수 with 하나가 맡는다.
---
# 취소와 정리

<ol class="stages"><li>소스</li><li>토큰</li><li>트리</li><li>코어</li><li>바이트코드</li><li class="on">실행</li></ol>

:::goals
- 태스크를 멈추게 해야 하는 경우들과, 아무 데서나 멈추면 생기는 문제
- 구조적 동시성: 태스크를 태스크 범위 안에서만 띄우는 이유와 그 구현(취소 트리)
- 취소가 계산하는 태스크와 기다리는 태스크에 닿는 방법
- 연산이 아니라 블록에 거는 기한, 그리고 타이머 스레드가 없는 기한
- 에러·취소·`exit`에도 도는 정리 `with`, 그리고 `defer`를 두지 않은 이유
- "지금 멈춰라"(취소)와 "하던 일을 마치고 끝내라"(정지 요청), 그리고 신호
:::

12장과 13장은 태스크를 띄우고 기다리게 하는 법이었다. 이 장은 반대쪽, 태스크를 **멈추게** 하는 법이다. 동시성 런타임에서 가장 까다로운 부분이고, fung의 동시성 v2가 가장 많이 바꾼 부분이기도 하다.

## 왜 멈추게 해야 하나 {#why}

태스크를 도중에 멈춰야 하는 경우는 많다.

- **기한.** "이 요청은 2초 안에." 그 안에 DB 질의 하나, HTTP 호출 둘, 채널 받기 하나가 있다.
- **형제의 실패.** 사용자 정보와 주문 목록을 동시에 가져오다가 주문 쪽이 죽으면, 사용자 쪽을 계속할 이유가 없다.
- **사람의 중단.** Ctrl-C.
- **배포.** Kubernetes나 systemd가 SIGTERM을 보내고 유예 시간 뒤에 SIGKILL을 보낸다. 그 사이에 받은 요청은 마치고 끝내야 한다.

그런데 아무 데서나 멈추면 문제가 생긴다. 파일을 반쯤 썼거나, 커넥션을 풀에 돌려주지 않았거나, 잠금을 쥔 채였을 수 있다. 그래서 언어는 두 가지를 정해야 한다. **어디서** 멈추는가(멈춤이 닿는 지점), 그리고 멈출 때 **무엇이 반드시 도는가**(정리).

:::box other 다른 언어는
- **Java**의 `Thread.stop`은 아무 지점에서나 스레드를 죽였다. 잠금과 객체가 반쯤 바뀐 채 남아서 일찍이 쓰지 말라는 것이 되었고, 지금은 "취소를 요청하는 표시"인 `interrupt`를 쓴다. 표시를 보는 것은 각 코드의 몫이다.
- **Python**의 `asyncio`는 취소된 태스크의 다음 `await` 지점에서 `CancelledError` 예외를 던진다. `try`/`finally`가 정리를 맡는다.
- **JavaScript**는 `AbortController`의 신호를 함수마다 손으로 넘기고 확인한다.
- **Erlang**은 프로세스에 종료 신호를 보내고, 링크된 프로세스들이 함께 죽거나 supervisor가 받아서 다시 띄운다.
:::

## 구조적 동시성 {#structured}

0.2.0까지 fung의 `spawn(f)`는 어디서든 태스크를 띄웠다. 태스크의 수명이 어떤 코드 블록에도 묶여 있지 않았다. 이런 "자유로운 spawn"은 쓰기는 쉽지만, 언어가 답할 수 없는 질문을 남긴다(노트 R7-language의 L1).

- 태스크가 실패하면 **누가 아는가.** 아무도 `await`하지 않으면 실패가 사라진다. 그래서 "핸들이 사라질 때 보고한다"는 특별 규칙을 두어야 했다.
- 프로그램은 **언제 끝나는가.** 진입 파일이 끝나도 태스크가 남아 있을 수 있다. 그래서 "진입 파일은 남은 태스크를 기다린다"는 규칙이 붙었다.
- 자기가 띄운 태스크를 반드시 기다리는 도우미(`pmap`)는 보고에서 빼야 해서 내부 전용 `spawn_awaited`가 생겼다.

모두 같은 뿌리에서 나온 예외 규칙이다. **구조적 동시성**(structured concurrency)은 그 뿌리를 뽑는다. 규칙은 하나다. **태스크는 태스크 범위 안에서만 태어나고, 범위는 그 안의 태스크가 모두 끝나야 닫힌다.** 함수 호출이 "부르면 끝나고 돌아온다"를 보장하듯, 범위는 "열면 그 안에서 시작한 동시 작업이 다 끝나고 돌아온다"를 보장한다.

:::box other 다른 언어는
2018년 Nathaniel J. Smith가 Python 라이브러리 **Trio**에서 "nursery"라는 이름으로 정리했다. 같은 생각이 **Kotlin**의 `coroutineScope`, **Swift**의 `TaskGroup`, **Java 21**의 `StructuredTaskScope`에 들어갔다. 실패를 격리하는 쪽은 **Erlang**의 supervisor가 원조다.
:::

:::box choice fung의 선택: 태스크 범위가 유일한 태스크 생성 방식
동시성 v2는 구조적 동시성을 **유일한** 태스크 생성 방식으로 두었다(스펙 1절). 규칙 하나로 위의 질문에 답이 정해진다(노트 R7-language의 L1).

- 실패는 범위를 연 코드로 간다. 자식 하나가 죽으면 형제와 본문이 취소되고, 그 에러가 `scope(` 호출에서 다시 난다. 보고할 "아무도 모르는 실패"가 없다.
- 범위 밖에 사는 태스크가 없으니 진입 파일이 끝나면 프로그램이 끝난다. 배경 태스크라는 종류도 없앴다(HTTP 서버도 자기 범위 안에서 돈다).
- 실패를 값으로 다뤄야 하는 곳(서버의 요청 하나하나)은 `supervise(f, on_fail)`다. 숨은 예외 규칙이 아니라 이름 붙은 정책이다.
:::

```fung run exit=1 error=E0502
fn slow(name) = with(
  () => name,
  n => sleep(10s),
  n => print("release", n),
)

scope(fn(s):
  spawn(s, () => slow("db"))
  spawn(s, () => 1 / 0)
;;)
```

두 번째 자식이 0으로 나누자 태스크 범위가 형제 `db`를 취소했다. `db`는 10초를 기다리지 않고 곧바로 되감기며 `release`를 지났고(아래 `with` 절), 그다음 범위가 자식의 에러를 그대로 다시 냈다. 취소된 형제가 여럿이면 그들이 되감기는 순서는 정해져 있지 않다. `note: scope opened at …`가 범위를 연 자리를 알려 준다.

### 취소 트리 {#cancel-tree}

구현의 중심은 **취소 노드**(`CancelNode`, `crates/fung-vm/src/vm/task.rs`)의 트리다.

- 태스크마다 노드가 하나 있다. 태스크 범위도 노드를 하나 만든다. 범위 노드는 범위를 연 태스크의 노드 아래에, 자식 태스크의 노드는 범위 노드 아래에 달린다. 범위의 본문도 범위 노드 아래에서 돈다(태스크의 노드를 잠시 바꿔 끼운다).
- 노드를 취소하면 그 아래가 모두 취소된다. 재귀가 아니라 명시적 목록으로 내려가므로 깊은 사슬도 괜찮다.
- 노드는 원자적 표시(`AtomicBool`) 하나와, 자식 목록, 그 노드 아래에서 기다리는 태스크의 깨움 표시 목록(`watchers`)을 가진다. 취소하면 표시를 세우고 기다리는 태스크를 모두 깨운다.
- 부모는 자식을 **강하게** 든다(자식은 부모를 약한 참조로 든다). 그래서 중간의 태스크나 핸들이 사라져도 살아 있는 노드 아래의 것은 루트에서 늘 닿는다. 루트를 취소하면 프로그램의 모든 태스크에 닿는다.

이 구조 덕에 여러 동작이 따로 장치 없이 나온다.

- `cancel(s)`: 범위 노드를 취소한다. 본문과 모든 자식이 취소된다.
- 바깥의 취소(바깥 범위의 실패, `timeout`, `exit`): 트리를 타고 범위에 닿는다.
- `cancel(t)`: 그 태스크의 노드만 취소한다. `await(t)`는 `Err({kind: 'cancelled})`를 받는다.

```fung run
r = scope(fn(s):
  t = spawn(s, () => sleep(10s))
  cancel(t)
  await(t)
;;)
print(r)

r2 = scope(fn(s):
  spawn(s, () => sleep(10s))
  cancel(s)
  "body value"
;;)
print(r2)
```

### 태스크 범위가 하는 일 {#scope}

태스크 범위(`crates/fung-vm/src/vm/scope.rs`)는 자식의 핸들을 쥐지 않는다. **살아 있는 자식 수**만 센다. 자식이 범위 핸들을 붙잡는 일은 흔하다(손자를 띄우려고). 범위가 자식을 쥐면 범위 → 자식 → 범위의 `Rc` 고리가 생긴다(10장). 수만 세면 자식이 범위를 쥐는 것은 "도는 동안만"이라 고리가 없다.

본문이 끝나면 태스크 범위는 자식이 0이 될 때까지 기다린다. 이 기다림은 **취소로 끊기면 안 된다.** 바깥에서 취소가 왔다고 자식이 남은 채 돌아오면 "범위가 돌아오면 다 끝났다"는 약속이 깨진다. 그래서 기다리는 동안만 태스크의 노드를 **새 루트 노드**로 바꿔 끼운다. 바깥 취소는 그래도 자식에게 닿는다(자식은 원래 트리에 달려 있다). 자식들이 되감고 끝나면 기다림이 풀리고, 그다음에 범위가 취소를 이어서 되감는다. 이 기다림은 교착 판정에 든다(12장). 그래서 "모두가 범위 끝에서 서로를 기다린다"도 E0702로 잡힌다.

태스크 범위가 돌아올 때 결과는 이 순서로 정한다.

1. 자식의 실패: 자식의 위치와 호출 경로를 그대로 두고 `note: scope opened at …`, 동시에 난 다른 실패는 `also failed:`로 덧붙인다.
2. 본문 자신의 에러나 `exit`
3. 바깥의 취소: 계속 되감는다.
4. `cancel(s)`: `Err({kind: 'cancelled})`
5. 본문의 값

"취소는 그것이 겨눈 태스크 범위가 받는다"는 Trio의 cancel scope와 같은 규칙이다. 그래서 아래의 `timeout`도 "타이머가 취소하는 범위"로 같은 자리에 들어간다.

### supervise: 실패를 격리하기 {#supervise}

```fung run
fn handle(req) = req == "bad" then 1 / 0 else print("ok", req)

supervise(fn(s):
  each(["a", "bad", "c"], r => spawn(s, () => handle(r)))
;;, f => print("failed:", f.kind, f.code))
print("server still up")
```

`supervise`의 자식이 죽으면 형제를 취소하지 않고, 실패 값 `{kind, code, message, at}`를 `on_fail`에 준다. 처리기 태스크를 따로 띄워 두지 않는다. 늘 떠 있는 처리기가 있으면 태스크 범위가 그것을 기다리느라 닫히지 않기 때문이다. 대신 처리하는 쪽이 없을 때 **처음 실패한 자식이 처리기가 되어** 대기열이 빌 때까지 `on_fail`을 부르고, 그동안 범위의 살아 있는 자식으로 남는다. 범위가 취소되어 처리하지 못한 실패는 버리지 않고 `task failed:`로 보고한다(종료 코드 1).

## 취소가 닿는 법 {#delivery}

취소 표시가 섰다고 태스크가 곧바로 멈추지는 않는다. fung의 취소는 **협력형**이다. 태스크가 정해진 지점에서 표시를 보고 스스로 되감는다. 그 지점이 둘이다.

- **계산하는 태스크**: 모든 `CALL`·`TAILCALL`에서 `Task::tick`이 표시를 본다(12장의 차례 넘기기와 같은 자리). 표시가 서 있으면 그 자리에서 E0701 `task cancelled`로 되감기를 시작한다. 반복은 모두 호출을 지나므로, 끝없는 계산도 다음 호출에서 멈춘다. 표시 읽기는 원자 변수 읽기 하나라 평소에는 비용이 보이지 않는다.
- **기다리는 태스크**: 주차한 태스크는 자기 노드의 `watchers`에 등록되어 있다. 취소가 그것을 깨우고, 깨어난 태스크는 기다림이 끝난 이유가 취소임을 보고(`after_park`) 기다리던 상자의 `unwind`로 간다. 상자마다 "이미 건너온 값은 취소보다 먼저" 같은 자기 규칙을 정한다(채널).

되감기(unwinding)는 프레임을 하나씩 걷어 내며 위로 올라간다. 그 길에 이어 가기 상자(12장)가 있으면 그 상자의 `unwind`가 본다. 기본은 그대로 올려 보내기이고, `with`·`timeout`·`scope`가 여기서 끼어든다. E0701은 사용자에게 찍히지 않는다. 태스크의 핸들이 그것을 `Err({kind: 'cancelled})`로 바꾸고, 태스크 범위가 결과를 정한다.

## 기한: timeout {#timeout}

0.2.0까지 기한은 연산마다 따로 있었다(`await(t, timeout: d)`, `recv(ch, timeout: d)`). 연산 하나라면 충분하지만 웹서비스의 기한은 대개 **요청 하나 전체**에 걸린다. 연산마다의 기한으로는 남은 시간을 계산해 다음 연산에 넘겨야 하고, 그 안에서 띄운 태스크에는 기한이 닿지 않으며, 바깥 함수가 정한 기한을 안쪽 함수가 모른다.

Trio의 글 "Timeouts and cancellation for humans"(2018)가 이 문제를 정리했다. 기한을 연산이 아니라 **코드 블록**에 건다. 그러면 기한이 합쳐지고(바깥과 안쪽 중 이른 쪽), 그 안의 모든 연산과 태스크가 따른다. fung은 이미 태스크 범위를 들였으므로 `timeout(d, f)`는 "타이머가 취소하는 범위"로 같은 자리에 들어갔다(노트 R7-language의 L2).

```fung run
print(timeout(100ms, () => sleep(10s)))
print(timeout(1s, () => "fast"))
print(timeout(1s, () => ["inner answered", timeout(50ms, () => sleep(10s))]))
print(timeout(50ms, () => ["inner answered", timeout(1s, () => sleep(10s))]))
print(timeout(200ms, () => exec(["sleep", "5"])))
```

세 번째 줄은 안쪽 기한이 먼저라 안쪽 `timeout`이 답했고 바깥은 그 값을 받았다. 네 번째 줄은 바깥 기한이 먼저라 안쪽은 값을 내지 못하고 되감겼고, 바깥 `timeout`이 답했다. 마지막 줄은 외부 명령 `sleep 5`가 200 ms에 끝났다(아래 "외부 명령").

### 기한은 노드에 {#deadline-node}

`timeout`은 기한이 실린 취소 노드를 만든다(`CancelNode::with_deadline`). 그 아래에 생기는 노드(태스크 범위, 그 안의 태스크)는 부모의 기한을 그대로 물려받는다. 안쪽 `timeout`의 기한이 바깥보다 늦으면 바깥 기한을 물려받으므로, "이른 쪽이 이긴다"가 저절로 된다. 노드는 기한과 함께 그 기한의 **주인**(그 기한을 만든 `timeout`의 노드)을 기억한다.

**타이머 스레드는 없다.** 처음 떠오르는 구현은 타이머 스레드가 기한에 노드를 취소하는 것이지만, 스레드가 하나 더 늘고 교착 판정이 그 스레드를 모르게 된다(노트 R7-language의 L2). 대신 **노드 아래에 있는 쪽이 스스로 기한을 본다.**

- 주차하는 태스크는 기한까지만 잔다. 스케줄러가 주차할 때 노드의 기한을 기다림의 기한과 합쳐 타이머 힙에 넣는다(`Sched::park`). 기한이 있는 기다림은 "타이머가 끝낼 기다림"이라 교착으로 몰리지 않는다.
- 계산하는 태스크는 차례를 넘길 때(호출 1,000번마다, `end_turn`) 시계를 본다. 그 사이 수천 번의 호출에는 비용이 없다.
- 기한이 지난 것을 처음 본 쪽이 주인 노드를 **만료**시킨다(`expire`). 만료는 "기한으로 취소됨" 표시를 세우고 취소하는 일이다. 그러면 아래가 모두 취소된다.

`timeout`이 끝날 때의 규칙은 태스크 범위와 같은 모양이다(`crates/fung-vm/src/vm/deadline.rs`의 `Timeout`).

- 본문이 값을 돌려줬으면 그 값이다.
- 본문이 취소로 되감겼고, 이 노드가 **자기 기한으로** 만료됐고, 바깥은 취소되지 않았으면 `Err({kind: 'timeout})`이다.
- 나머지는 그대로 올라간다. 바깥 기한이 먼저면 바깥 `timeout`이 답하는 이유다.

`timeout(0s, f)`는 이미 지난 기한이라 `f`가 첫 호출에서 되감긴다. 예외 규칙 없이 일관되지만 "기다리지 않고 한 번 보기"로는 쓸 수 없다.

```fung run
fn double(x) = x * 2

print(timeout(0s, () => double(21)))
print(timeout(1s, () => double(21)))
```

### 운영체제를 기다리던 중이었다면 {#os-waits}

13장의 기다림들은 기한과 취소를 이렇게 받는다.

- **소켓·표준 입력**: 주차이므로 곧바로 깨어나 되감는다. 이벤트 루프 등록은 그 기다림이 끝날 때 지운다.
- **풀의 일**: 줄에서 아직 시작하지 않은 일은 버린다. 읽기만 하는 일은 돌던 중이어도 태스크가 곧바로 되감고 결과는 버린다. 무엇을 바꾸는 일은 끝날 때까지 기다린 뒤 기한을 확인한다. 취소된 쓰기가 나중에 일어나지 않게 하려는 것이다.
- **외부 명령**: 명령에 SIGTERM을 보내고, 2초 뒤에도 남아 있으면 SIGKILL을 보낸다(Kubernetes와 같은 모양). 명령은 자기 **프로세스 그룹**으로 띄우므로 `sh -c "a | b"`의 `a`·`b`처럼 셸이 띄운 손자도 함께 끝난다. 다만 fung이 터미널의 전경 그룹이면 명령을 같은 그룹에 둔다. 다른 그룹의 프로세스가 터미널을 읽으면 운영체제가 멈춰 세우므로(SIGTTIN), `sh("sudo …")`의 비밀번호 프롬프트가 멈춰 버렸기 때문이다(노트 R7-language의 L6).

## 정리: with {#with}

취소가 흔해지면 "취소돼도 반드시 도는 정리"가 꼭 필요해진다. 서버에서는 요청 하나가 기한에 걸릴 때도 커넥션을 풀에 돌려줘야 한다.

언어들의 답은 대개 셋 중 하나다. `try`/`finally` 블록(Java, Python, JavaScript), 자원을 여는 문법(Python의 `with`, Java의 try-with-resources), 그리고 "이 함수가 끝날 때 이것을 하라"는 `defer`. 함수형 쪽에는 Haskell의 `bracket(acquire, release, use)`가 있다.

:::box choice fung의 선택: 평범한 내장 함수 with(acquire, use, release) 하나
`defer`는 기각했다(`background.md`, 노트 R7-language의 L3).

- 할 일을 프레임에 매달면 꼬리 호출 최적화와 충돌한다. 꼬리 호출은 프레임을 다시 쓰는데(9장), 매달린 일이 그 프레임에 있다.
- VM에 프레임마다의 목록과 되감기 로직이 필요해진다.

`with`는 **평범한 내장 함수**라서 둘 다 피한다. `use`는 내장 함수 안에서 부르므로 처음부터 꼬리 호출이 아니고, 정리할 일은 이어 가기 상자(12장)의 상태로 들고 있으면 된다. 되감기는 이미 있는 길(상자의 `unwind`)을 그대로 쓴다. 문법이 아니라 함수라서 "무엇을 어떻게 닫는가"를 언어가 알 필요도 없다. 예전의 `at_exit`(프로세스가 끝날 때 도는 처리기)도 지웠다. `exit`·교착·진입 파일의 에러가 모두 "루트 취소 → 되감기"이므로, 진입 파일 본문을 `with`로 감싸면 같은 일을 한다. 정리 장치가 하나면 규칙도 하나다.
:::

`with`는 `acquire()`를 부르고, 그 값으로 `use(r)`를 부르고, `use`가 **어떻게 끝나든** `release(r)`를 부른다. 값, `Err` 반환, 런타임 에러, 취소(`cancel`·`timeout`·태스크 범위의 실패), `exit` 모두다(`crates/fung-vm/src/vm/cleanup.rs`의 `run_with`).

```fung run
fn conn(name) = with(
  () => print("open", name),
  _ => sleep(10s),
  _ => print("close", name),
)

r = timeout(100ms, () => conn("db"))
print(r)
```

```fung run exit=3
with(
  () => "tmp",
  _ => exit(3),
  _ => print("cleanup runs even on exit"),
)
print("not reached")
```

### 차단 노드 {#shield}

`release`는 그것을 부른 취소 안에서 돌면 안 된다. 요청이 기한으로 취소됐는데 `release`가 커넥션을 닫으려고 쓰기를 하면, 그 쓰기가 곧바로 E0701로 끊긴다. 그래서 `release`는 **새 루트 노드**(차단 노드, shield) 아래에서 돈다. 바깥 취소도 기한도 닿지 않는다.

대가는 "끊을 수 없는 일"이 생긴다는 점이다. `exit` 뒤의 되감기에서 `release`가 `recv(channel())`에 멈추면 아무도 깨우지 못하고 프로세스가 영원히 남는다. 그래서 도는 `release`의 차단 노드를 목록(`World::shields`)에 올려 두고, 교착 판정은 그 목록이 비어 있지 않으면 앞선 `exit`·교착과 무관하게 판정하며, 교착이면 차단 노드들을 취소한다. 교착 보고는 그 태스크가 `release` 안에서 기다린다고 말한다.

### 에러를 합치는 규칙 {#merge}

`use`와 `release`가 둘 다 실패할 수 있다. 원칙은 "<strong>`release`의 실패는 버리지 않는다</strong>"이다.

1. `use`의 런타임 에러(release도 실패했으면 그것은 `note: also failed in release`로 붙는다. 원인 쪽을 보여 준다)
2. 그 밖에는 `release`의 런타임 에러
3. 그 밖에는 `release`가 부른 `exit`이나 교착 판정이 끊은 것
4. 그 밖에는 `use`의 결과(값, `Err`, 취소, `exit`)

```fung run exit=1 error=E0502
fn close_conn(c):
  print("closing", c)
  fail("close failed")
;;

with(() => "db", c => 1 / 0, close_conn)
```

## 정지 요청과 신호 {#stop}

### 취소와 정지 요청은 다르다 {#stop-vs-cancel}

배포할 때 오케스트레이터는 SIGTERM을 보내고 유예 시간(Kubernetes 기본 30초)을 준 뒤 SIGKILL을 보낸다. 웹 서비스가 그 사이에 해야 할 일은 "새 요청은 그만 받고, 받은 요청은 마치고 끝내기"다(노트 R7-language의 L6).

- **취소**는 "지금 멈춰라"다. 모든 자손에게 곧바로 가고, 도는 코드는 다음 확인 지점에서 되감긴다. 처리 중인 요청도 끊긴다.
- **정지 요청**은 "하던 일을 마치고 끝내라"다. 받는 쪽이 무엇을 마칠지 스스로 정한다.

그래서 둘을 따로 둔다. `stop_signal()`은 정지 요청이 오면 닫히는 채널의 받는 끝을 준다. 정지 요청을 **보는 쪽이 없으면** 그때만 취소로 넘어간다. 그래서 정지 요청을 모르는 스크립트는 Ctrl-C에 지금처럼 곧바로 멈춘다.

**보는 쪽이 있는지는 참조 수로 안다**(`crates/fung-vm/src/vm/stop.rs`). 런타임은 정지 채널의 보내는 끝을 쥐고, 받는 끝은 약한 참조(`Weak`)로만 기억한다. 받는 끝의 사본이 하나라도 살아 있으면 약한 참조가 되살아난다. 그것이 "보는 쪽이 있다"다. 정지 요청이 오면,

- 보는 쪽이 있으면: 런타임이 보내는 끝을 놓는다. 채널이 닫히고(마지막 사본이 사라졌으므로) 기다리던 쪽이 모두 깨어난다. 새 장치가 필요 없었다. 프로그램은 스스로 끝날 때 끝나고, 종료 코드는 0이다.
- 보는 쪽이 없으면: `exit`처럼 루트를 취소한다. 종료 코드는 128 + 신호 번호(SIGINT 130, SIGTERM 143), `request_stop()`이면 0이다.

```fung run
stop = stop_signal()

scope(fn(s):
  spawn(s, fn():
    wait_closed(stop)
    print("worker: stop requested, finishing")
  ;;)
  request_stop()
;;)
print("clean exit")
```

```fung run
fn job(_):
  request_stop()
  sleep(10s)
;;

with(() => "job", job, _ => print("release runs"))
print("not reached")
```

첫 예는 일꾼이 정지 채널을 보고 있으므로 채널이 닫히기만 했다. 둘째 예는 보는 쪽이 없으므로 루트가 취소되어 `sleep`이 끊기고 `release`를 지나 끝났다. `http.serve`는 정지 요청을 보고 리스너를 닫은 뒤 처리 중인 요청을 `drain`(기본 10초)까지 마친다.

정지 채널은 "바깥에서 깨우는 채널"로 표시되어 있다. 그 채널만 기다리는 프로그램은 교착이 아니다.

### 신호는 전용 스레드가 받는다 {#signals}

유닉스 신호 처리기(signal handler)는 아무 스레드의 아무 지점에 끼어든다. 그래서 처리기 안에서는 잠금을 잡거나 메모리를 할당하면 안 되고(async-signal-safe 함수만 부를 수 있다), 할 수 있는 일은 표시 하나를 세우는 정도다.

fung은 처리기를 쓰지 않는다(`crates/fung-vm/src/signals.rs`).

- 명령이 시작하자마자, 다른 스레드가 생기기 전에 SIGINT·SIGTERM을 막는다(`pthread_sigmask`). 막힌 상태는 나중에 생기는 스레드도 물려받는다.
- 전용 스레드 하나가 `sigwait`로 기다렸다가 신호를 받는다. 받은 뒤는 보통 코드라서, 실행 스레드에 사건(`Event::Stop`)을 게시하고 깨우개를 당긴다. 스케줄러가 한 바퀴의 처음에 그것을 처리한다(12장).
- 두 번째 신호는 정리를 기다리지 않고 곧바로 끝낸다. 멈춘 정리에 갇히지 않게 하는 탈출구다.
- 막은 마스크는 `exec`을 지나도 자식에게 남는다. 그래서 자식 프로세스는 `fork`와 `exec` 사이에서 마스크를 비우고 시작한다. 비우지 않으면 fung이 띄운 명령은 SIGTERM을 받지 못한다.

자식 프로세스가 부모(fung)에게 SIGTERM을 보내게 해서 실제 신호로 확인해 보자.

```fung run
stop = stop_signal()

scope(fn(s):
  spawn(s, fn():
    wait_closed(stop)
    print("got the stop request: draining")
  ;;)
  exec(["sh", "-c", "kill -TERM $PPID"])!
;;)
print("finished cleanly")
```

```fung run exit=143
fn work(_):
  exec(["sh", "-c", "kill -TERM $PPID"])!
  sleep(10s)
;;

with(() => "lock", work, _ => print("release: lock returned"))
print("not reached")
```

보는 쪽이 있으면 종료 코드 0(사용자 결정: 정상 종료라서 systemd·Kubernetes가 배포마다 "실패"로 적지 않는다), 없으면 143이다.

## exit {#exit}

`exit(n)`은 어느 태스크에서 불러도 같다. 첫 요청의 코드를 기록하고 프로그램의 루트 노드를 취소한다(`Task::request_exit`). 부른 태스크는 `exit`로, 다른 태스크는 E0701로, 어디서 기다리든 되감기며 모든 `with`의 `release`를 지난다. `release`는 차단 노드 아래에서 돌므로 `exit`가 닿지 않는다. 종료 코드는 처음 요청된 것이 이긴다. `exit`는 운영체제의 프로세스 종료를 곧바로 부르지 않고 이렇게 "되감기는 신호"로 다루므로, 같은 프로세스에서 VM을 돌리는 단위 시험도 무사하다.

## 코드에서 찾아보기 {#source}

- `crates/fung-vm/src/vm/task.rs`: 취소 노드 `CancelNode`(`child`, `with_deadline`, `cancel`, `watch`, `expire_due`), 태스크 핸들 `TaskHandle`.
- `crates/fung-vm/src/vm/scope.rs`: `ScopeHandle`, `ScopeState`, `run_scope`, `supervise`의 `Supervisor`, 태스크의 끝 상자 `TaskEnd`.
- `crates/fung-vm/src/vm/deadline.rs`: `Deadline`(기한과 주인), `run_timeout`, `Timeout` 상자.
- `crates/fung-vm/src/vm/cleanup.rs`: `run_with`, `With` 상자, 차단 노드.
- `crates/fung-vm/src/vm/block.rs`: 취소 확인 `Task::tick`, `check_cancelled`, 기한을 보는 `end_turn`.
- `crates/fung-vm/src/vm/stop.rs`: 정지 채널 `StopState`, `stop_signal`, `StopHandle`.
- `crates/fung-vm/src/signals.rs`: 신호 막기 `init`, `sigwait` 스레드, 정책 `Policy`, 자식의 마스크 비우기.
- `crates/fung-vm/src/native/process.rs`: 취소된 명령의 SIGTERM·SIGKILL(`TERM_GRACE`), 프로세스 그룹.
- `crates/fung-vm/src/vm/deadlock.rs`: `release` 안의 교착 판정.

:::exercise 기한 맞히기
아래 다섯 줄이 무엇을 찍을지 먼저 적어 보고 실행해 확인하자. 셋째 줄과 넷째 줄은 어느 `timeout`이 답하는가? 마지막 줄에서 `sleep 5` 명령은 언제 어떻게 끝나는가?

```fung frag
print(timeout(100ms, () => sleep(10s)))
print(timeout(1s, () => "fast"))
print(timeout(1s, () => ["inner answered", timeout(50ms, () => sleep(10s))]))
print(timeout(50ms, () => ["inner answered", timeout(1s, () => sleep(10s))]))
print(timeout(200ms, () => exec(["sleep", "5"])))
```

:::solution
```fung run
print(timeout(100ms, () => sleep(10s)))
print(timeout(1s, () => "fast"))
print(timeout(1s, () => ["inner answered", timeout(50ms, () => sleep(10s))]))
print(timeout(50ms, () => ["inner answered", timeout(1s, () => sleep(10s))]))
print(timeout(200ms, () => exec(["sleep", "5"])))
```

셋째 줄은 안쪽 기한(50 ms)이 먼저 지나 안쪽 노드가 **자기 기한으로** 만료되므로 안쪽 `timeout`이 `Err({kind: 'timeout})`을 내고, 바깥은 그 값이 든 리스트를 받는다. 넷째 줄은 안쪽 노드가 바깥 기한(50 ms)을 물려받았다. 만료되는 것은 바깥 노드이고, 안쪽은 "자기 기한이 아닌 취소"라 그대로 되감긴다. 그래서 리스트가 만들어지지 않고 바깥 `timeout`이 답한다. 마지막 줄은 200 ms에 명령에 SIGTERM이 가고, `sleep`이 그 신호로 끝나서 2초를 기다리지 않는다.
:::
:::

## 정리 {#summary}

- 태스크는 `scope`·`supervise`가 연 태스크 범위 안에서만 태어나고, 범위는 자식이 모두 끝나야 돌아온다. 그래서 아무도 모르는 실패와 끝나지 않는 배경 태스크가 없다.
- 취소는 노드의 트리다. 태스크 범위의 노드 아래에 자식이, 부모는 자식을 강하게 든다. 노드를 취소하면 아래가 모두 취소되고 기다리던 태스크가 깨어난다.
- 취소는 협력형이다. 계산하는 태스크는 호출마다, 기다리는 태스크는 깨어나서 표시를 보고 E0701로 되감는다. 되감기는 이어 가기 상자의 `unwind`를 지난다.
- `timeout`은 기한이 실린 노드다. 타이머 스레드 없이, 주차한 태스크는 기한까지만 자고 계산하는 태스크는 차례를 넘길 때 시계를 본다. 이른 기한이 이긴다.
- `with`는 무슨 일이 있어도 `release`를 부르는 평범한 내장 함수다. `release`는 차단 노드 아래에서 돌고, 그 실패는 버리지 않는다. `defer`와 `at_exit`는 없다.
- 정지 요청은 취소와 다르다. 보는 쪽(받는 끝의 사본)이 있으면 채널을 닫기만 하고, 없으면 루트를 취소한다. 신호는 처리기 없이 `sigwait` 스레드가 받는다.

레퍼런스: [scope](../reference/prelude/concurrency.html#fn-scope), [timeout](../reference/prelude/concurrency.html#fn-timeout), [with](../reference/prelude/program.html#fn-with), [stop_signal](../reference/prelude/concurrency.html#fn-stop_signal). 4부는 여기까지다. 5부는 언어 둘레의 도구, 포맷터·REPL·테스트 러너를 본다.
