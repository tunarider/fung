# L8. 라이브러리 함수의 꼬리 호출이 에러 위치와 호출 경로를 흐린다

상태: **닫힘 (2026-10-27)**. 선택지 (나), 정할 것 1~3 모두 추천대로(사용자 결정 2026-10-27). 목록: [README](README.md).

결과: `Frame::tail_from`(`TailFrom`: 함수·호출 자리·그전 꼬리 호출 수)을 `reuse_frame`의 사용자 → 내장 가지에서만 채우고, 내장 → 내장·내장 → 사용자는 물려받는다.
경계를 넘을 때마다 `tail_calls`를 0부터 센다(사용자 콜백도 라이브러리의 꼬리 호출을 물려받지 않음). `user_pos`·`call_path`·교착 보고의 함수 이름(`deadlock.rs`)이 이 칸을 읽는다.
세어 본 곳: 라이브러리가 사용자 콜백을 꼬리 호출하는 곳은 `fs._with_ok`(Rust `with`를 거침)와 `conc`의 태스크 람다(새 태스크)뿐이라 고리는 없었다.
옛 동작을 기대하던 시험 넷(`http.rs`의 꼬리 호출 사례, `boot/tests.rs`의 `fan_in` 위치, 교착 보고의 이름, 서버 실패의 `at`)을 고쳤다.
측정(7번 중앙값, `map`은 15번씩 두 차례): fib −1.7%, loop −0.5%, list +0.1%, map +1.0~1.5%, str +1.7%, 꼬리 재귀 2천만 번 +0.3%, 라이브러리 꼬리 호출 30만 번 −1.5%.

## 문제

fung으로 쓴 라이브러리 함수(`extras/*.fung`의 `http.*`, `fs.with_open`, prelude의 `take` 등)를 꼬리 위치에서 부르면 부른 함수의 프레임이 사라진다. 그 안에서 난 에러는 "사용자 코드의 가장 가까운 프레임"(06)으로 옮겨지는데, 그 프레임이 이미 없어서 **부른 쪽의 부른 쪽** 줄을 가리킨다.

```
# u.fung
import http

fn fetch(u):
  http.get(u, timeout: 5)              # 4행: 꼬리 위치
;;

fn main():
  r = fetch("http://127.0.0.1:1/")     # 8행
  print(r)
;;

main()
```

```
$ fung run u.fung
error[E0403]: argument `timeout` must be dur, got int
  --> u.fung:8:12                      # 틀린 줄: 실수는 4행에 있다
  |
8 |   r = fetch("http://127.0.0.1:1/")
  |            ^
  |
 call path (innermost first):
   extras/http.fung:29 get (1 tail calls elided)
   u.fung:8   main
   u.fung:12  <entry>
```

`fetch`의 4행을 `r = http.get(…)` 다음 줄 `r`로 바꾸면(꼬리가 아님) 머리가 4행(`http.get(`의 자리)이고 경로에 `fetch` 프레임(4행)이 있다(확인함). 같은 일이 교착 보고(`fn first_two(rx): take(rx, 2) ;;`의 교착이 `d.fung:7:17`, 부른 쪽 줄을 가리킴)와 사용자 람다의 에러(`fs.with_open(p, fn(h): 1 / 0 ;;)`가 꼬리 위치면 경로에서 `load`가 빠지고 `with_open (1 tail calls elided)`)에도 생긴다.

사용자는 라이브러리 함수를 "호출"로 생각한다. 머리줄이 실수한 줄이 아니라 그 위의 줄을 보이면 06의 규칙("에러의 위치는 사용자 코드의 가장 가까운 프레임")이 노린 것을 놓친다. open-questions "알려진 한계" → "태스크"의 꼬리 위치 호출 항목이다("모든 꼬리 호출에 저장 하나"라서 하지 않았다).

## 지금 동작과 원인

- 컴파일러: `crates/fung-front/src/compiler/fn_.rs`의 `Compiler::call`이 꼬리 위치면 `TAILCALL`을 낸다(반환 타입이 있으면 자기 호출만, 03 T4). 호출 대상이 라이브러리 함수인지는 컴파일 때 모른다(값이다).
- VM: `crates/fung-vm/src/vm/call.rs`의 `reuse_frame`이 지금 프레임을 새 함수로 바꾸고 `tail_calls += 1`. 이때 이미 "바뀌는 프레임이 내장인가, 새 함수가 내장인가"를 보고 `Frame::entry`(E0707이 이름을 대는 라이브러리 함수, `user_callee`)를 고른다. 바뀐 프레임의 함수와 호출 자리는 남지 않는다.
- 에러에 위치와 경로를 붙이는 곳: `crates/fung-vm/src/vm.rs`의 `attach`(가장 안쪽이 내장 프레임이면 `user_pos`로 위치를 옮김), `user_pos`(내장이 아닌 가장 안쪽 프레임의 지금 명령 위치), `call_path`(프레임마다 한 줄, `elided: f.tail_calls`). 교착 보고는 `vm/deadlock.rs`의 `wait_site`가 같은 `user_pos`를 쓴다.
- 프레임: `crates/fung-vm/src/vm/frame.rs`의 `Frame`(`proto`·`ip`·`base`·`closure`·`globals`·`tail_calls`·`entry`). 내장 여부는 `RtProto::builtin`.
- 표시 형식: `crates/fung-front/src/diag.rs`의 `CallSite`(`elided`가 있으면 ` (N tail calls elided)`).

## 고려할 점

- 꼬리 호출 최적화는 선택 기능이 아니다(00, 03). 상태가 있는 반복은 꼬리 재귀다. 고치는 방법이 스택을 키우거나 사용자 코드끼리의 꼬리 호출을 느리게 하면 안 된다.
- 사용자 코드끼리의 꼬리 호출이 경로에서 빠지는 것은 설계가 받아들인 것이다(08: `(tail calls elided)` 표시). 문제는 경계, 곧 **사용자 코드 → 라이브러리**로 넘어가는 꼬리 호출이다. 그때 사라지는 프레임이 06이 위치로 삼는 바로 그 프레임이다.
- 경계를 넘는 꼬리 호출은 드물고, 그 판단은 `reuse_frame`이 이미 하는 비교 안에 있다. 거기에만 저장을 두면 뜨거운 길(사용자 함수끼리의 꼬리 재귀)의 비용은 그대로다.
- 라이브러리가 다시 사용자 함수를 꼬리 호출하는 고리(사용자 → 라이브러리 → 사용자 …)가 무한 반복이면 프레임을 남기는 방식은 스택이 자란다. 지금 저장소의 예(`fs._with_ok`가 `f(x)`를 꼬리 호출)는 Rust로 쓴 `with`를 거쳐 이미 경계가 있다. 다른 곳은 확인하지 못했다.
- 다른 언어: Lua는 꼬리 호출을 지우고 추적에 `(...tail calls...)`를 적는다(지금의 fung). Scheme 구현 중 MIT Scheme은 최근 꼬리 호출 몇 개를 고리 버퍼("history")에 남기고, Safari의 JavaScriptCore는 디버거용으로 꼬리 호출을 따로 기록해(ShadowChicken) 지운 프레임을 다시 보인다. Erlang도 마지막 호출 최적화로 스택 추적에서 프레임이 빠진다. Python과 Rust(`become`은 아직 제안)는 꼬리 호출을 보장하지 않아 이 문제가 없다.

## 선택지

(가) **사용자 코드 → 라이브러리의 꼬리 호출은 프레임을 재사용하지 않는다.** `call_op`에서 그 경우만 보통 호출로 바꾼다. 경로와 위치가 꼬리 아닌 호출과 똑같아진다. 단점: 위의 고리가 있으면 스택이 자란다. 경로에 프레임이 하나 더 남는다는 점 말고는 동작이 같다.

(나) **경계를 넘을 때만 사라지는 프레임의 함수와 호출 자리를 새 프레임에 적는다.** `Frame`에 `tail_from: Option<(Rc<RtProto>, Pos)>`(이름은 구현이 정함)를 두고 `reuse_frame`의 "사용자 → 내장" 가지에서만 채운다. `call_path`는 그것을 프레임 한 줄로 보이고(`u.fung:4 fetch`), `user_pos`는 내장 프레임의 `tail_from`도 사용자 위치로 본다. 내장 → 내장 꼬리 호출은 물려받는다(`entry`와 같은 규칙). 스택이 자라지 않고, 비용은 경계에서 저장 하나다. 단점: 프레임에 칸이 하나 는다(프레임 크기만큼의 메모리, 측정 필요).

(다) **모든 꼬리 호출을 최근 N개 남긴다**(MIT Scheme의 history). 사용자 코드끼리의 생략도 보인다. 꼬리 호출마다 저장 하나라 알려진 한계에 적은 비용 그대로다.

(라) 그대로 두고 매뉴얼에 "꼬리 위치의 라이브러리 호출은 한 줄 위를 가리킨다"를 적는다.

추천: (나). 문제가 생기는 경계에서만 값을 치르고, 꼬리 호출의 공간 보장(00·03)을 그대로 지킨다. (가)는 더 단순하지만 고리가 없다는 것을 저장소 밖 코드에 대해서는 장담할 수 없다.

## 정할 것

1. (나)로 갈지. 추천: (나). (가)가 더 단순하니, 저장소를 훑어 라이브러리가 사용자 콜백을 꼬리 호출하는 곳이 Rust 경계 없이 있는지 세어 보고 결과와 함께 묻는다.
2. 경로의 표시. 추천: 되살린 프레임은 보통 줄처럼 `u.fung:4   fetch`로 적고, 그 프레임이 그전에 겪은 꼬리 호출 수가 있으면 거기에 `(N tail calls elided)`를 붙인다. 라이브러리 프레임의 숫자는 경계를 넘은 것을 빼고 센다.
3. 사용자 코드 → 사용자 코드의 생략은 지금처럼 둘지. 추천: 둔다(08의 설계, 비용).

## 할 일

1. 시험 먼저: `crates/fung-vm/src/vm/tests.rs`에 꼬리 위치의 라이브러리 호출에서 난 인자 에러의 위치(`fetch`의 줄)와 경로, 꼬리 위치 `fs.with_open`의 람다 에러 경로(`load`가 보임), 꼬리 위치 `take`의 교착 위치, 사용자 함수끼리 꼬리 재귀 백만 번이 스택을 키우지 않음(`tail_calls_do_not_grow_the_stack` 옆), E0707의 `user_callee`가 그대로임. 골든 `tests/lang/err_tail_library.fung`. 실패를 먼저 확인한다.
2. `frame.rs`·`call.rs`(`reuse_frame`, 모듈 본문 프레임 `module.rs`와 `push_frame`의 초기값)·`vm.rs`(`call_path`·`user_pos`·`user_callee`)·`deadlock.rs`(`wait_site`).
3. 성능: `bench/`의 계산 위주 측정(꼬리 재귀가 많은 것)과 `perf` 시험. 잡음(±2%) 밖이면 기록하고 묻는다.
4. 기대 출력: 에러 경로가 바뀌는 기대 출력·골든을 `FUNG_EXPECTED=bless`로 다시 쓰고 diff를 검토한다(경로 한 줄이 늘어나는 것만이어야 한다).
5. 설계 문서: 03 "꼬리 호출 최적화"의 제약, 08 "에러 메시지" 형식(경계의 프레임은 남는다), 06의 extras 위치 규칙, 09 "프레임"(있으면), open-questions "알려진 한계"에서 항목을 지움.
6. 매뉴얼: `manual/vm/07-diagnostics.md`·`08-bytecode.md`(꼬리 호출 설명), `manual/vm/internals.md`, `manual/learn/05-functions.md`(꼬리 재귀의 경로 설명이 있으면). 다시 만들고 diff 검토.
7. CHANGELOG `[Unreleased]` "고침": 꼬리 위치의 라이브러리 호출에서 난 에러가 그 호출의 줄을 가리킨다.

## 끝의 기준

- 문제 절의 `u.fung`가 `--> u.fung:4:…`를 보이고 경로에 `u.fung:4 fetch`가 있다. 교착 예는 `take`를 부른 줄을 가리킨다.
- 사용자 함수의 꼬리 재귀가 깊이와 상관없이 돈다(기존 시험). 계산 측정이 잡음 안이다.
- 커밋 전 검사가 통과하고, 03·06·08·open-questions·매뉴얼·CHANGELOG가 같은 것을 말한다.
