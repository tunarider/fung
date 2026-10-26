# L6. Rust로 쓴 긴 내장 함수가 차례를 넘기지 않는다

상태: 열림 (2026-10-26). 0.5.0 전에 고친다. 목록: [README](README.md).

## 문제

실행 스레드 하나가 모든 태스크를 돈다(M:1, `vm/sched.rs`). fung 코드는 호출 `YIELD_EVERY`(1,000)번마다 차례를 넘기지만, Rust로 쓴 내장 함수 하나는 끝날 때까지 돌려주지 않는다.
그동안 다른 태스크는 하나도 돌지 못하고, 취소와 기한도 보이지 않는다. 릴리스 바이너리(2026-10-26, macOS), 약 6 MB JSON:

```
$ cat gap.fung
import json
import time
big = "[" + join(map(0..200000, i => "{\"a\": ${i}, \"b\": \"xxxxxxxx\"}"), ",") + "]"
fn ticker(last, worst, n) = match n:
  0 => worst
  _ =>
    sleep(1ms)
    t = time.now()
    ticker(t, max([worst, t - last]), n - 1)
;;
scope(fn(s):
  w = spawn(s, () => ticker(time.now(), 0ms, 200))
  sleep(20ms)
  len(json.parse(big)!)
  print("worst gap between 1ms ticks:", await(w))
;;)
$ fung run gap.fung        # 세 번
worst gap between 1ms ticks: 78.553ms
worst gap between 1ms ticks: 78.552ms
worst gap between 1ms ticks: 84.315ms
```

기한도 같은 이유로 늦는다. fung 코드의 반복은 5 ms에 끊기지만, 같은 기한 안의 `json.parse`(약 80 ms)와 콜백 없는 `sort`(200만 원소, 약 210 ms)는 끝까지 돌고 값을 준다.

```
$ fung -e 'print(timeout(5ms, () => fold(0..10000000, 0, (a, i) => a + i)))'
Err({kind: 'timeout, message: "timed out after 5ms"})
$ fung run long.fung       # timeout(5ms, () => len(json.parse(big)!)), timeout(5ms, () => len(sort(xs)))
200000 79.906ms
2000000 213.563ms
```

뒤의 것은 05의 규칙("본문이 되감기기 전에 값을 돌려줬으면 그 값이다")에 맞기는 하다. 하지만 기한이 75 ms 넘게 지난 뒤에야 끝나므로 "이 요청 전체를 2초 안에"(05 "기한은 범위 하나로")라는 약속이 입력 크기만큼 늦어진다.
R6이 HTTP로 같은 것을 쟀다(`crates/fung-cli/tests/perf.rs`의 `a_light_request_beside_a_heavy_parse`: 가벼운 요청이 혼자 약 1 ms, 무거운 파싱 옆에서 약 72~76 ms).
스펙 10.4(`docs/superpowers/specs/2026-10-18-concurrency-v2-r7-design.md`)는 이 지연을 "기록만"으로 두었다. 05의 "굶기지 않는다"는 공정성이 입력 크기에 묶여 있다는 점이 문제다.

## 지금 동작과 원인

- 차례 넘기기: `Task::tick`(`vm/block.rs` 557행)이 호출마다 `budget`을 줄이고, 0이 되면 `end_turn`이 기한을 보고 다음 양보 지점에서 실행 루프를 떠난다. 취소도 `tick`에서만 본다.
  `tick`은 fung 호출과 단계 내장 함수가 콜백을 부를 때만 지난다. 보통 내장 함수(`NativeBody`의 평범한 함수, `fn(&mut Task, Vec<Value>) -> Result<Value, Stop>`)는 안에서 아무것도 세지 않는다.
- 단계 내장 함수(`vm/cont.rs`의 `Flow`·`Next`: `Done`·`Call`·`Park`)는 콜백 사이나 주차 때만 실행 루프로 돌아온다. "할 일이 남았지만 차례만 넘긴다"는 갈래가 없다.
- 긴 것의 예(입력 크기에 비례하고 중간에 돌려주지 않음, 2026-10-26 코드에서 확인한 것):
  - `json.parse`(`native/json.rs`의 `parse_fn` → `decode`), `json.stringify`(`stringify_fn`).
  - `sort`(`prelude/collections.rs`의 `sort_fn` → `sorted_by`의 병합 정렬), `sort_by`도 키를 콜백으로 다 모은 뒤 같은 `sorted_by`를 한 번에 돈다. `unique`도 한 번에 돈다.
  - `fs.read`의 마지막 풀기(`native/fs.rs` `read_fn`의 `finish`, 파일 전체의 `decode_lossy`가 실행 스레드에서 돈다).
  - 문자열 내장 함수(`join`·`split`·`replace`·`repeat`·`lines` 등)와 정규식 찾기·바꾸기, 값 출력(`value/format.rs`). 이것들의 시간은 재지 않았다.
  - VM 연산 자체: 큰 값의 `==`(`value/compare.rs`), 긴 문자열의 `+`. 명령 하나라 내장 함수와 같은 문제다(이것도 재지 않음).
- 풀(`vm/pool.rs`, `nat::offload`)은 fung 값을 넘길 수 없다(`Rc`, `Runtime`은 `!Send`). 값을 만드는 일(파싱 결과, 정렬)은 실행 스레드에서 해야 한다.

## 고려할 점

- **얼마나 자주 넘기나.** 넘김 자체는 싸다(태스크 전환 약 1 µs, open-questions "정한 것"). 하지만 내장 함수 안의 셈이 빠른 길을 느리게 하면 계산 위주 5% 목표(스펙 10.4)를 깬다.
  일 단위로 예산을 깎는 방식("연료", BEAM의 reduction과 같은 생각)이면 작은 입력은 지금과 같은 길을 탄다.
- **상태를 어디에 두나.** 차례를 넘기면 Rust 스택이 사라진다. 파서·정렬의 진행 상태를 상자(`Box<dyn Cont>`)에 담아야 한다. 재귀 하강 파서는 명시적 스택으로 바꿔야 한다(L9와 같은 일).
- **멈춘 사이의 의미.** 값은 불변이라 다른 태스크가 입력을 바꿀 수 없다. 취소되면 반쯤 만든 결과는 버리면 된다(바깥을 바꾸지 않는 일이므로 05의 "무엇을 바꾸는 일은 시작했으면 끝낸다"에 걸리지 않음).
- **풀로 보내기.** JSON은 바이트를 복사해 풀에서 `Send`인 중간 나무로 파싱하고 실행 스레드에서 값으로 바꿀 수 있다. 하지만 바꾸는 단계가 여전히 O(n)이고, 작은 파싱마다 왕복 약 9 µs가 붙는다(R7 노트 R6 2절이 이 이유로 미뤘다). 정렬·비교에는 쓸 수 없다.
- 다른 언어: BEAM은 내장 함수가 reduction을 세어 일정량마다 스스로 양보한다(`enif_consume_timeslice`, 긴 것은 "dirty scheduler"). Go는 비동기 선점(신호)으로 끊는다. Node·Lua는 내장 함수 하나가 이벤트 루프를 막는다(fung의 지금 모양).

## 선택지

(가) **기록하고 둔다**(지금). 장점: 할 일 없음. 단점: 지연과 기한 초과가 입력 크기에 비례하고, 서버 하나가 큰 요청 하나에 모두 멈춘다.
(나) **연료와 이어 가기.** `Flow`/`Next`에 "차례만 넘기고 다시 불러 달라"(`Yield(Box<dyn Cont>)`)를 더하고, 긴 내장 함수는 일정량(예: 입력 64 KiB, 원소 1만 개, 비교 수 기준)마다
  태스크 예산을 깎고 예산이 다 되면 상태를 상자에 담아 넘긴다. 넘길 때 취소·기한도 본다. 장점: 지연이 일정량으로 묶이고 기한이 제때 끊김, 장치 하나(R2의 이어 가기)를 넓히는 것.
  단점: 파서·정렬·문자열 함수를 다시 짜야 함, 셈이 빠른 길에 얹힘.
(다) **풀로 보낼 수 있는 것만 보낸다.** 큰 입력의 `json.parse`만 문턱 넘으면 풀에서 중간 나무로. 장점: 파싱 시간 대부분이 풀로 감. 단점: 값 만들기가 남고, 정렬·비교·문자열 함수는 못 풀며, 규칙이 함수마다 다르다(원칙 7).
(라) **(나)의 일부: 끝난 뒤에 예산만 깎는다.** 내장 함수가 한 일만큼 예산을 깎아, 끝난 직후 바로 차례를 넘기고 기한을 본다. 장점: 작다. 단점: 끝나기 전 지연은 그대로, 기한 초과의 결과가 값에서 `Err`로 바뀔 뿐.

추천: **(나)**. 규칙이 "모든 일은 정해진 양마다 차례를 넘긴다" 하나가 되고, fung 코드와 내장 함수의 차이(원칙 4가 숨기려는 구현 언어의 흔적)가 사라진다.
(라)를 첫 단계로 함께 넣어(모든 보통 내장 함수에 일의 양만큼 예산을 깎게) 나머지 내장 함수도 적어도 끝난 직후에는 기한을 보게 한다.

## 정할 것

1. 일정량의 단위와 크기. 추천: "약 1 ms"를 목표로 함수마다 재서 정한다(JSON 입력 64 KiB 안팎, 정렬 비교 1만 번 안팎). 상수는 `YIELD_EVERY` 옆에 두고 근거를 주석으로.
2. 어느 내장 함수를 이어 가게 바꾸나. 추천: 입력 크기에 비례하는 것 전부를 목록으로 만들고(`spec/builtins.txt`에서 훑기), 이번에는 `json.parse`·`json.stringify`·`sort`·`sort_by`의 정렬 단계·`unique`·`fs.read`의 풀기·`join`·`split`·`replace`·`repeat`·`lines`·정규식 찾기.
   나머지는 (라)로. 값 출력은 L9와 함께 한다.
3. VM 연산(`==`·문자열 `+`·큰 맵 키의 해시)도 넘기게 하나. 추천: 이번에는 (라)만(일의 양만큼 예산 깎기). 명령 한가운데서 넘기면 해석 루프가 복잡해진다. 측정해서 문제가 크면 따로.
4. 05에 무엇을 약속하나. 추천: "내장 함수를 포함해 한 태스크가 다른 태스크를 막는 시간은 입력 크기와 무관하게 묶인다(약 1 ms 단위)", "기한은 그 단위 안에 끊긴다". 스펙 10.4의 지연 항목에 목표(예: 무거운 파싱 옆 가벼운 요청 5 ms 이하)를 둔다.

## 할 일

1. 시험 먼저: 위 `gap.fung`·`long.fung` 모양을 `crates/fung-cli/tests/perf.rs`(릴리스 전용)에 넣는다. 최악 간격이 목표 이하인지, `timeout(5ms, () => json.parse(big))`·`sort`가 `Err 'timeout`인지.
   `a_light_request_beside_a_heavy_parse`를 기록에서 단언으로 바꾼다. 순서는 상태로 정한다(`testutil`의 탐침, `sleep` 대신). 실패를 먼저 확인한다.
2. `vm/cont.rs`에 양보 갈래를 더하고 실행 루프(`drive`)·스케줄러(`vm/sched.rs`)가 그 태스크를 대기열 뒤로 보내게 한다. 넘길 때 `tick`과 같은 취소·기한 확인.
3. 일의 양만큼 예산을 깎는 도우미를 `Task`에 두고(예: `charge(n)`), 2번의 내장 함수를 상태 상자로 다시 쓴다. JSON 파서는 명시적 스택(L9와 맞춤), 정렬은 단계별 병합.
4. 계산 위주 벤치마크가 5% 안인지 `scripts/bench.sh`로 전후 비교(`bench/str.fung`·`map.fung`·`list.fung` 등)하고, `bench/`에 큰 JSON·큰 정렬 벤치마크를 더할지 정한다. 결과는 `docs/notes/R7-scheduler.md`나 새 노트에.
5. 설계 문서: 05("기한은 범위 하나로"의 구현 줄, 공정성), 09(실행 모델), 스펙 10.4의 지연 항목. open-questions "알려진 한계"의 해당 줄을 지우고 닫은 기록으로 옮긴다.
6. 매뉴얼: VM 부록의 스케줄러 설명(`manual/vm/`), 동시성 장의 기한 설명을 고치고 다시 만든다. CHANGELOG `[Unreleased]`에 "긴 내장 함수가 차례를 넘김, 기한이 제때 끊김".

## 끝의 기준

- `gap.fung`의 최악 간격과 HTTP 지연 시험이 정한 목표 이하이고 릴리스 시험이 그것을 단언한다.
- `timeout(5ms, …)` 안의 큰 `json.parse`·`sort`가 `Err({kind: 'timeout, …})`이다.
- 계산 위주 벤치마크가 기준선보다 5% 넘게 느려지지 않는다(기록을 노트에).
- open-questions에 이 한계가 없고, 커밋 전 검사가 통과한다.
