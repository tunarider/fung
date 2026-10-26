# L3. 채널을 기다리는 방법: 정지 신호만 기다리기, 기다리지 않고 보기

상태: 열림 (2026-10-26). 0.5.0 전에 고친다. 목록: [README](README.md).

## 문제

세 가지가 한 자리(받는 끝의 연산)에 모인다.

**정지 신호만 기다리기.** 정지 요청은 정지 채널을 닫아서 알린다(05 "정지 요청과 신호"). 그래서 `recv(stop)`은 언제나 `Err({kind: 'closed})`이고, 문장으로 쓰면 버려지는 `Err`라 E0606이다.

```
$ fung -e 'stop = stop_signal()
request_stop()
recv(stop)
print("after")'
error[E0606]: discarded Err: channel closed
  --> -e:3:1                           # 종료 코드 1
```

그래서 `_ = recv(stop)`으로 쓴다(`extras/http.fung`의 `_stopper`, 매뉴얼 `manual/vm/14-cancellation.md`의 예 둘, `manual/reference/prelude/concurrency.md`의 `stop_signal` 설명). `_ =`는 "실패를 일부러 버린다"는 표시인데 여기서 `Err`는 실패가 아니라 기다리던 사건이다. 읽는 사람에게 거꾸로 말한다. `for _ in stop: nil ;;`도 되지만(확인함) 관용구로 알려져 있지 않다.

**기다리지 않고 한 번 보기가 없다.** 일꾼이 일 사이에 "정지 요청이 왔나", "쌓인 값이 있나"를 막히지 않고 볼 방법이 없다.

```
$ fung -e 'print(try_recv)'
error[E0204]: undefined name `try_recv`

$ fung -e '(tx, rx) = channel(1)
send(tx, 5)!
print(timeout(0s, () => recv(rx)))
print(recv(rx))'
Err({kind: 'timeout, message: "timed out after 0s"})
Ok(5)
```

**`timeout(0s, …)`는 언제나 시간 초과다.** 값이 이미 있어도 받지 않는다. 05가 Trio의 `move_on_after(0)`을 따라 그렇게 정했고(L2) "기다리지 않고 한 번 보는 연산은 없다(open-questions)"고 적었다. 이 동작 자체는 맞다: 이미 지난 기한이다. 빠진 것은 한 번 보기 연산이다.

open-questions "남은 일"의 두 항목(정지 신호를 기다리기만 하는 코드, `try_recv`)이다. 앞의 것은 사용자 결정(2026-10-20)으로 "써 보며 판단"하기로 했다.

## 지금 동작과 원인

- `recv`: `crates/fung-vm/src/conc.rs`의 `recv`가 `channel::begin_recv`를 부르고, 닫혔으면 `received_value`가 `err_kind("closed", "channel closed")`를 만든다. 기다려야 하면 `RecvK`로 주차한다.
- 채널 안에는 이미 기다리지 않는 연산이 있다: `crates/fung-vm/src/vm/channel.rs`의 `Channel::try_recv`(버퍼의 맨 앞, 없으면 기다리는 보내는 쪽의 값; 버퍼 0인 채널도 된다)와 `Channel::try_send`, `Channel::closed`. 공개 내장 함수만 없다.
- 정지 채널: `crates/fung-vm/src/vm/stop.rs`의 `stop_signal`이 바깥에서 깨우는 채널(`channel::pair_external`)의 받는 끝을 준다. 보는 쪽이 있는지는 받는 끝의 `Weak`가 살아 있는지다. 요청이 오면 `request`가 보내는 끝을 놓아 닫는다. 값은 한 번도 오지 않는다.
- `timeout`: `crates/fung-vm/src/vm/deadline.rs`의 `run_timeout`이 `d <= 0`이면 기한을 지금으로 두어 `f`가 첫 호출에서 되감긴다.
- 효과 있는 내장 함수는 `vm/atom.rs`의 효과 표에 있어야 `swap`의 함수 안에서 E0707이 된다. 교착 보고의 종류 이름(`recv`·`send` …)은 주차한 내장 함수의 이름에서 저절로 온다(`vm/deadlock.rs`의 `wait_site`).

## 고려할 점

- 05의 원칙은 "최소한의 원시 연산"이다. 고수준 패턴은 fung으로 쓴다. 그러니 새 연산은 fung으로 만들 수 없는 것이어야 한다. 막히지 않는 받기는 fung으로 만들 수 없다(`timeout(0s)`는 받지 않는다). 정지 기다리기는 `_ = recv(stop)`으로 이미 된다. 빠진 것은 뜻을 말하는 이름이다.
- 정지 신호의 "보는 쪽"은 받는 끝을 쥔 동안이다. 그래서 `wait_stop()`처럼 인자 없이 그때 끝을 얻는 함수는 기다리기 전에 온 요청을 놓친다(그때는 보는 쪽이 없어 루트가 취소된다). 기다리는 함수는 쥐고 있던 끝을 받아야 한다.
- 막히지 않는 받기의 "비었음"을 어떻게 돌려줄지. `Err`로 주면 다른 실패처럼 `?`·`match`로 다루지만, 문장에서 버리면 E0606이다(문장으로 쓰는 `try_recv`는 뜻이 없으니 맞다). `nil`로 주면 받은 값 `nil`과 구별되지 않는다.
- 한 번 보기는 폴링 반복을 부른다(`sleep` 없이 돌면 차례만 먹는다). 매뉴얼은 "기다릴 때는 `recv`, 다른 일 사이에 볼 때만 `try_recv`"를 말해야 한다.
- 다른 언어: Go는 `<-ctx.Done()`(받은 값을 버리는 문장이 자연스럽다)과 `select`의 `default`로 한 번 보기를 한다. Rust의 `std::sync::mpsc`·`crossbeam`은 `try_recv`가 `Err(Empty)`·`Err(Disconnected)`를 주고, tokio의 `CancellationToken`은 `cancelled().await`(기다리기)와 `is_cancelled()`(보기)를 따로 둔다. Python `asyncio.Queue`는 `get_nowait`가 `QueueEmpty`를 던지고, `asyncio.Event`는 `wait()`와 `is_set()`이다. Trio는 `receive_nowait`가 `WouldBlock`을 던진다.

## 선택지

(가) **`try_recv(rx)`만 더한다.** `Ok(v)`, 비었으면 `Err({kind: 'empty, message: "channel empty"})`, 닫혔고 비었으면 `Err({kind: 'closed})`. 정지 기다리기는 `_ = recv(stop)`을 그대로 둔다. 원시 연산이 하나만 는다. 하지만 기다리는 쪽의 어색함은 남는다.

(나) **`try_recv(rx)`와 `wait_closed(rx)`를 더한다.** `wait_closed(rx)`는 그 채널이 닫힐 때까지 기다리고 `nil`을 준다. 값을 꺼내지 않는다(버퍼에 남은 값은 남는다). 정지 기다리기는 `wait_closed(stop)`, 정지 확인은 `try_recv(stop)`이 `Err({kind: 'closed})`인지. 둘 다 채널 일반의 연산이라 정지 신호만을 위한 예외가 없다. "생산자가 모두 끝나기를 값을 꺼내지 않고 기다리기"에도 쓴다.

(다) **정지 전용 함수를 둔다.** `wait_stop(stop)`·`stop_requested(stop)`. 이름은 가장 분명하지만 받는 끝을 인자로 받아야 해서(위) 결국 채널 연산과 같고, 정지 채널에만 맞는 함수가 둘 는다.

(라) **정지 신호를 토큰 타입으로 바꾼다.** 새 값 종류, `type_of`, 출력, 순회 규칙이 따라온다. 정지 채널이 "닫히는 받는 끝"이라 `for`·`each`·`fan_in`과 섞이는 지금의 장점을 잃는다.

추천: (나). 막히지 않는 받기는 fung으로 만들 수 없는 진짜 빈자리이고, `wait_closed`는 "닫힘을 기다린다"는 뜻을 이름으로 말해 `_ =`의 거꾸로 된 신호를 없앤다. 둘 다 채널 일반의 연산이라 05의 규칙 수가 늘지 않는다. `timeout(0s, …)`는 그대로 둔다(이미 지난 기한이라는 뜻이 맞고, 한 번 보기는 `try_recv`가 맡는다).

## 정할 것

1. (나)로 갈지. 추천: (나).
2. 비었음의 모양. 추천: `Err({kind: 'empty, message: "channel empty"})`. 다른 `Err` 맵처럼 `message`가 있다(05).
3. `try_send(tx, v)`도 더할지. 추천: 더한다. 같은 내부 연산(`Channel::try_send`)이 있고, "가득 차면 버리는" 로그 전송 같은 역압 패턴은 fung으로 만들 수 없다. 가득 참은 `Err({kind: 'full, message: "channel full"})`.
4. `wait_closed`의 이름. 추천: `wait_closed`. `closed(rx)`는 `close(tx)`와 헷갈리고 bool을 돌려줄 것처럼 읽힌다.
5. 교착 판정에서 `wait_closed`의 종류 이름. 추천: `wait_closed`(05 "종료"의 "종류는 기다리는 내장 함수의 이름"). 정지 채널만 기다리면 지금처럼 교착이 아니다.

## 할 일

1. 시험 먼저(`crates/fung-vm/src/vm/channel_tests.rs`, `stop_tests.rs`, 골든 `tests/lang/try_recv.fung`·`wait_closed.fung`): 값이 있음·비었음·닫힘, 버퍼 0 채널에서 기다리는 보내는 쪽이 있을 때 받음, `try_recv`가 주차하지 않음(옆 태스크가 차례를 받지 않아도 돌아옴), `swap`의 함수 안에서 E0707, `wait_closed`가 닫힘에 깨어남·취소에 끊김·기한에 끊김, 정지 채널의 `wait_closed`가 교착이 아님, `try_send`의 가득 참.
2. `conc.rs`에 내장 함수와 매개변수 표, `vm/atom.rs`의 효과 표. 교착 보고에 `wait_closed`가 제 이름으로 나오는지 시험으로 본다. `spec/builtins.txt` 갱신(`FUNG_UPDATE_SPEC=1 cargo test spec_file`).
3. `extras/http.fung`의 `_ = recv(stop)`을 `wait_closed(stop)`으로. 매뉴얼의 같은 예도.
4. 설계 문서: 05 "원시 연산" 표, "정지 요청과 신호", "타임아웃·취소"의 "기다리지 않고 한 번 보는 연산은 없다" 문장, 06의 prelude 목록(있으면), open-questions 두 항목을 닫음.
5. 매뉴얼: `manual/reference/prelude/concurrency.md`(새 함수 셋, `stop_signal` 설명의 `_ = recv(stop)`), `manual/learn/12-concurrency.md`, `manual/vm/14-cancellation.md`, `manual/reference/functions.md`. 함수 레퍼런스는 매개변수 표와 같아야 한다(`manual.rs`). 다시 만들고 diff 검토.
6. CHANGELOG `[Unreleased]` "더함": `try_recv`·`try_send`·`wait_closed`.

## 끝의 기준

- `stop = stop_signal()` 뒤 `wait_closed(stop)` 문장이 정지 요청에 깨어나 `nil`이고 E0606이 없다.
- `try_recv`가 위 세 경우에 정해진 값을 주고 주차하지 않는다. `timeout(0s, …)`의 동작은 그대로다.
- 저장소에 `_ = recv(stop)`이 남지 않는다(`grep`). 커밋 전 검사가 통과하고 05·open-questions·매뉴얼·CHANGELOG가 같은 것을 말한다.
