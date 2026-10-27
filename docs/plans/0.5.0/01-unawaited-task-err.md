# L1. 아무도 `await`하지 않은 태스크의 `Err`

상태: **닫힘 (2026-10-27)**. 선택지 (가), 정할 것 1~4 모두 추천대로(사용자 결정 2026-10-27). 목록: [README](README.md).

결과: `TaskState`에 띄운 자리·범위(`Weak`)·읽지 않은 `Err`(`Unread`: E0607 진단과 `on_fail`용 값·보고)를 두고, `await`가 읽으면 지운다(`TaskState::read`).
판정은 `TaskEnd::body_ended`(핸들 없음: 태스크의 실패로), `TaskHandle`의 `Drop`(범위가 열려 있으면 `ScopeState::unread_lost`), `Scope::closed`(범위가 기억한 것 중 남은 것)의 세 자리다.
`supervise`에서 처리기가 없을 때 생긴 것은 대기열에 들어가 다음 처리기나, 범위가 닫힐 때 여는 태스크가 `on_fail`을 부른다(`Scope`의 `handling`). 진단은 L2의 `discarded_err`를 써서 리스트도 한 겹 본다(계획에 없던 것, L2와 맞춤).
저장소에서 걸린 곳은 시험 하나(`park_tests`의 시간 초과 `Err`를 돌려주는 태스크)뿐이었다. `http`의 연결 태스크는 `c`·`nil`을, `conc.fan_in`은 `close`의 `Ok(nil)`을 돌려준다.

## 문제

태스크의 함수가 `Err` 값을 돌려주고 그 핸들을 아무도 `await`하지 않으면, 그 `Err`는 흔적 없이 사라진다. 같은 `Err`를 문장에서 버리면 E0606인데, 태스크로 감싸면 조용하다.

```
$ fung -e 'scope(s => spawn(s, fn(): Err("x") ;;))
print("done")'
done                                   # 종료 코드 0

$ fung -e 'scope(fn(s): spawn(s, fn(): Err("x") ;;); nil ;;)
print("after")'
after                                  # 종료 코드 0

$ fung -e 'fn w(): Err("x") ;;
w()
print("after")'
error[E0606]: discarded Err: "x"
  --> -e:2:1                           # 종료 코드 1
```

런타임 에러로 죽은 자식은 범위를 실패시킨다(`1 / 0`이면 `error[E0502]` + `note: scope opened at -e:1`, 종료 코드 1). 05는 "함수가 `Err` 값을 돌려준 것은 실패가 아니다"라고 정했고, 02 "버려지는 Err"의 "잡지 않는 것"에 이 경우가 적혀 있다. 그래서 `spawn(s, () => fs.write(p, t))`처럼 결과를 돌려주는 일을 띄우고 핸들을 버리면 쓰기 실패가 사라진다. 원칙 1(조용히 넘어가는 동작은 에러, `background.md`)과 02의 E0606 규칙이 노리는 바로 그 실수다. L1 검토에서 보류했다(사용자 결정 2026-10-20, open-questions "남은 일").

## 지금 동작과 원인

- `crates/fung-vm/src/vm/scope.rs`의 `TaskEnd::body_ended`가 태스크 함수의 결과를 `task::outcome`으로 바꿔 `TaskState.result`에 넣고 기다리는 쪽을 깨운다. 함수가 돌려준 `Err`는 `Ok(Value::Tag(Err …))`라 그대로 저장된다.
- 범위의 실패 판정은 `ScopeState::child_done`이다. `Err(Stop::Error(d))`(취소·교착이 아닌 것)만 `fail`하고, 값으로 돌아온 `Err`는 보지 않는다. `supervise`의 줄(`Supervisor.queue`)도 `Stop::Error`만 넣는다.
- `crates/fung-vm/src/vm/task.rs`의 `TaskState`에는 결과 칸과 기다리는 쪽 목록만 있다. 결과를 누가 읽었는지(`await`, `conc.rs`의 `await_fn`)는 남지 않는다.
- 핸들은 `Value::Task(Rc<TaskHandle>)`이고 `TaskHandle.state`가 `Rc<TaskState>`를 쥔다. `TaskEnd`도 같은 `Rc<TaskState>`를 쥐므로, 태스크가 끝날 때 `Rc::strong_count(&state) == 1`이면 핸들이 하나도 남지 않은 것이다(값의 수명 규칙 03 "값의 수명"으로 결정적이다).
- 문장의 `Err` 검사는 `vm.rs`의 `Op::Pop`(`discarded_stop`)이고, 콜백의 것은 `vm/m4.rs`의 `discarded`다. 태스크 결과에는 어느 쪽도 닿지 않는다.

## 고려할 점

- "아무도 기다리지 않았다"는 한 순간에 정해지지 않는다. 핸들이 살아 있으면 나중에 `await`할 수 있다. 05는 "닫힌 범위의 태스크를 `await`해도 저장된 결과를 준다"고 해서 핸들이 범위 밖으로 나가는 것도 허락한다.
- 판정이 실행 순서에 따라 달라지면 안 된다. 자식이 먼저 끝나느냐 핸들이 먼저 사라지느냐로 에러 여부가 바뀌면 경쟁 상태다. 보고되는 **때**는 달라도 되지만 **에러인지 아닌지**는 같아야 한다.
- 오래 사는 서버가 있다. 2026-10-14에 기각한 규칙("핸들이 사라질 때 실패 보고", 05 "범위")은 서버가 핸들을 들고만 있어 끝날 때까지 보고가 없던 것이 문제였다. 범위가 닫힐 때만 보면 같은 일이 생긴다(`http.serve`의 `conns`는 서버가 끝날 때 닫힌다). 핸들을 버린 순간 알리는 길이 필요하다.
- 취소·시간 초과·교착으로 끝난 태스크의 결과도 `Err({kind: 'cancelled})`다. 이것은 함수가 돌려준 값이 아니라 런타임이 만든 결과이므로 세면 안 된다. `http`의 `_serve_on`은 `w = spawn(top, …)`을 `cancel(w)`로 끝낸다.
- 일부러 버리는 길이 있어야 한다. 문장은 `_ = 식`이고, 태스크는 함수 끝을 `nil`로 맺으면 된다(`spawn(s, fn(): _ = f() ;;)`).
- 다른 언어: Trio와 Python `asyncio.TaskGroup`은 자식의 반환값을 버리고 예외만 올린다(fung의 `Err`는 값이라 그대로면 버려진다). 다만 asyncio는 예외를 품은 채 회수되는 태스크에 "Task exception was never retrieved"를 남긴다(핸들이 사라질 때의 보고). Go `errgroup`은 일이 돌려준 `error`를 실패로 보고 `Wait`가 첫 것을 준다(`WithContext`면 나머지를 취소). Swift의 discarding task group은 결과를 버리되 자식의 에러는 그룹을 취소하고 다시 던진다. Erlang은 프로세스 함수의 반환값을 버리고 비정상 종료 사유만 링크로 알린다.

## 선택지

(가) **읽을 수 없게 된 `Err`는 버려진 `Err`다.** 함수가 돌려준 `Err` 결과는 범위가 닫히기 전에 누군가 `await`해야 한다. 판정은 세 자리에서 하고 결과는 같다.
1. 태스크가 끝날 때 핸들이 하나도 없으면(`strong_count == 1`) 그 자리에서 태스크의 실패로 바꾼다. `body_ended`에서 `r`을 `Err(Stop::Error(…))`로 바꾸면 `scope`의 실패, `supervise`의 `on_fail`로 그대로 흘러간다. 가장 흔한 `spawn(s, f)` 문장이 여기서 곧바로 잡힌다.
2. 끝난 태스크의 마지막 핸들이 읽히지 않은 채 사라지면(`TaskHandle`의 `Drop`) 범위가 열려 있는 동안 그 범위의 실패로 넣는다.
3. 범위가 닫힐 때 아직 읽지 않은 `Err`가 남아 있으면(핸들이 범위 밖으로 나감) 범위의 실패다.
- 장점: 결정적이고, 버린 순간에 알린다(서버도). 비용은 태스크 끝과 핸들의 `Drop`에서 검사 하나, `TaskState`에 "읽음" 표시 하나다.
- 단점: 범위 밖으로 핸들을 내보내 나중에 `await`하는 쓰임이 `Err` 결과에서는 막힌다(본문이 `await(t)`를 돌려주면 된다). `Drop`에서 실패를 기록하는 길이 새로 생긴다.

(나) **범위가 닫힐 때만 본다.** `await`하지 않은 `Err` 결과가 남은 자식이 있으면 범위의 실패. 구현은 가장 작다. 하지만 서버처럼 오래 사는 범위는 끝날 때까지 보고가 없어, 기각한 2026-10-14 규칙의 문제를 되풀이한다.

(다) **핸들이 사라질 때만 본다.** asyncio식. 핸들이 범위가 닫힌 뒤까지 살면(최상위 변수) 보고가 프로그램 끝으로 밀리고, 그때는 알릴 범위가 없다.

(라) **`spawn`을 둘로 나눈다.** 결과를 버리는 `spawn`(`Err`면 실패)과 결과를 돌려주는 `spawn_result`. 정적으로 분명하지만 원시 연산이 늘고, 같은 함수를 두 이름으로 부르게 된다(05 "최소한의 원시 연산").

추천: (가). 결정적이면서 버린 순간에 알리는 것은 이것뿐이다. 범위 밖 `await`를 잃는 것은 작다: 범위가 모든 자식을 기다리므로 결과는 본문 안에서 읽을 수 있다.

## 정할 것

1. 에러 번호. 추천: 새 번호 **E0607** `task's Err was never awaited: %s`(라벨 "this task's result is discarded", 도움말 "`await` it, or end the task's function with `nil`"). E0606의 도움말(`_ = ...`)은 태스크에 맞지 않는다.
2. 에러 위치. 추천: `spawn(` 호출 자리. `TaskState`에 띄운 위치를 남긴다. 함수의 반환 자리는 끝난 뒤 남아 있지 않다. 덧붙임 `note: scope opened at …`은 범위 실패와 같다.
3. `supervise`에서 2·3번 자리(핸들의 `Drop`, 범위 닫힘)의 실패를 `on_fail`로 보낼지. 추천: 보낸다(1번은 이미 그렇게 흐른다). 처리하는 태스크가 없을 때 누가 `on_fail`을 돌릴지는 구현이 정한다. 어렵다면 "처리하지 못한 실패"처럼 `task failed:` 보고 + 종료 코드 1로 하고 사용자에게 다시 묻는다.
4. 범위 밖으로 나간 핸들의 `Err`(3번)를 막는 것에 동의하는지. 추천: 막는다. 05의 "닫힌 범위의 태스크를 `await`해도 저장된 결과를 준다"는 `Err`가 아닌 결과와 이미 보고된 경우로 좁힌다.

## 할 일

1. 시험 먼저(`crates/fung-vm/src/vm/scope_tests.rs`, 골든 `tests/lang/err_task_err_*.fung`): 핸들을 버린 `Err`(범위 실패, 종료 코드 1), `await`한 `Err`(조용함), 취소된 태스크(조용함), `nil`로 맺은 태스크(조용함), 핸들을 쥐었다 읽지 않고 놓음(실패), 범위 밖으로 나간 핸들(실패), `supervise`의 `on_fail`이 받는 `{kind: 'error, code: "E0607"}`. 실패를 먼저 확인한다.
2. `TaskState`에 띄운 위치와 "읽음" 표시(`await_fn`이 켬), `TaskEnd::body_ended`의 판정, `TaskHandle`의 `Drop`, 범위가 닫히는 자리(`Draining::closed`·`settle`)의 남은 것 훑기.
3. `extras/*.fung`·`examples`·`tests/lang`을 돌려 걸리는 곳을 센다. 먼저 볼 곳: `http`의 `_spawn_conn`(연결 태스크가 `_conn`의 결과를 돌려줌, 쓰기 실패가 `Err`인지 확인), `conc.fan_in`, `_serve_on`의 `_stopper`(취소로 끝나므로 세지 않아야 함).
4. 카탈로그(`crates/fung-front/src/diag/catalog.rs`)와 `FUNG_UPDATE_SPEC=1 cargo test spec_file`.
5. 설계 문서: 05 "범위"·"태스크의 결과와 에러"(규칙, "닫힌 범위의 `await`" 문장), 02 "버려지는 Err"의 "잡지 않는 것"에서 이 항목을 지움, 08 에러 번호, open-questions "남은 일"의 항목을 닫음(`open-questions-closed.md`로).
6. 매뉴얼: `manual/reference/prelude/concurrency.md`(`spawn`·`await`·`scope`), `manual/learn/12-concurrency.md`, `manual/reference/errors.md`, 필요하면 `manual/vm/14-cancellation.md`. `FUNG_UPDATE_MANUAL=1 cargo test -p fung-cli --test manual_build`, diff 검토.
7. CHANGELOG `[Unreleased]` "바뀜(호환 깨짐)": 핸들을 버린 태스크의 `Err` 결과가 E0607.

## 끝의 기준

- `scope(s => spawn(s, fn(): Err("x") ;;))`가 `error[E0607]`과 `note: scope opened at`으로 종료 코드 1이다. `await`하거나 `nil`로 맺으면 조용하다.
- 같은 프로그램을 50번 돌려 결과가 같다(릴리스 `perf`의 반복 방식).
- 위 시험이 모두 있고 커밋 전 검사(CLAUDE.md)가 통과한다. 05·02·08·open-questions·매뉴얼·CHANGELOG가 같은 규칙을 말한다.
