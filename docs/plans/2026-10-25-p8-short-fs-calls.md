# P8 나머지: 혼자인 태스크의 `exists`·`stat` 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `fs.exists`·`fs.stat`이 호출마다 스레드 풀을 왕복하는 비용(건당 약 7.7µs, 일 자체는 1µs 미만)을 없앤다.
목표(open-questions P8): `bench/compare`의 `fs` 프로그램이 Python의 2배 안.

**결정 (사용자, 2026-10-25):**
- **B: 태스크가 혼자일 때만 실행 스레드에서 바로 부른다.** 아니면 지금처럼 풀에 넘긴다(`offload_abandoning`).
  - "혼자"는 셋을 모두 만족할 때다.
    1. 차례를 기다리는 다른 태스크가 없다. 스케줄러의 대기열, 다른 스레드가 깨워 `Ready`에 넣은 것, 이번 차례에 띄운 태스크(`new_fibers`)가 모두 비어 있다.
    2. 타이머나 바깥(풀 일·소켓·외부 명령·표준 입력·정지 신호)을 기다리며 주차한 태스크가 없다(`parked_timed == 0`).
    3. 그 태스크에 기한(`timeout`)이 없고, 취소되지 않았다.
  - 채널·`await`·연결의 차례 줄에서 주차한 태스크는 셈에 넣지 않는다. 이 태스크가 움직여야 깨어나므로, 이 태스크가 시스템 호출로 잠깐 막혀도 잃는 것이 없다.
- 함수를 둘로 나누지 않는다(`exists_now` 같은 짝 없음). 05의 "I/O는 평범한 함수 호출" 원칙, 결정 원칙 4·6·7. 검토 내용은 아래 "기각한 것".
- 범위는 `exists`·`stat`만. 바꾸는 호출(`mkdir`·`remove`·…)은 측정 근거가 생기면 따로 본다.

**언어 의미:** 바뀌지 않는다. 결과·에러가 같고, 바뀌는 것은 시간뿐이다.
보장이 하나 약해진다: 혼자인 태스크가 아주 느린 파일 시스템(멈춘 NFS)에서 `exists`·`stat`을 부르면, 그동안 온 정지 신호(SIGINT·SIGTERM)·REPL의 Ctrl-C가 호출이 끝난 뒤에 처리된다.
지금은 곧바로 되감는다(스펙 15절 "버리는 일"). 다른 태스크가 있거나 기한이 있으면 지금과 같다. 스펙 15절과 05에 이 예외를 적는다.

**Architecture:**
- `World`에 `others_waiting: bool`. 스케줄러가 차례를 줄 때마다(`Sched::turn`) `!queue.is_empty() || parked_timed > 0`으로 적는다. 차례 도중에 바뀌는 것(깨움, 띄움)은 호출할 때 `Ready`와 `new_fibers`를 보고 안다.
- `Ready::is_empty()`: 깨운 id와 사건이 없는지(잠금 하나).
- `vm/offload.rs`에 `alone(t)`과 `offload_short(t, job, finish)`: 혼자이면 `job`을 그 자리에서 부르고 `finish`, 아니면 `Kind::Abandon`으로 넘긴다. 일 안의 패닉은 실행 스레드의 패닉 울타리가 E0801로 만든다(풀과 같다).
- `nat::offload_short`가 그것을 감싸고, `exists_fn`·`stat_fn`이 부른다.

**Spec:** `docs/superpowers/specs/2026-10-18-concurrency-v2-r7-design.md` 15절, `docs/design/05-concurrency.md` "구현".

## Global Constraints

- TDD: 시험을 먼저 쓰고 실패를 본 뒤 고친다. 끝에 커밋 전 검사 전부(CLAUDE.md).
- 새 크레이트 없음. 기대 출력이 바뀌면 멈추고 원인을 본다(바뀔 이유가 없다).
- `chan`·`spawn`·`wait`·`exec`·계산 벤치마크가 느려지지 않는다(`scripts/bench.sh <전> <후>`).

## 작업

- [x] **1. 시험** (`crates/fung-vm/src/native/fs_tests.rs`): `offloads()` 탐침으로 센다.
  혼자(0), 자기 `timeout` 안(넘김), 띄웠지만 아직 돌지 않은 태스크가 있음(넘김), 다른 태스크가 채널에서 주차(0),
  그 태스크를 `send`로 깨움(넘김), 다른 태스크가 `sleep`에서 주차(넘김). 각 경우의 결과도 같아야 한다.
- [x] **2. 구현**: 위 Architecture.
- [x] **3. 문서**: 스펙 15절의 "버리는 일" 줄, 05 "구현", `native/fs.rs` 머리 주석, `manual/vm/internals.md`의 비용 줄, CHANGELOG `[Unreleased]` 성능.
- [x] **4. 측정**: `fs.exists("/")` 5만 번, `bench/compare/run.py`의 `fs`, `scripts/bench.sh`. open-questions P8과 `docs/notes/bench-languages.md` 3.1절 갱신.

## 기각한 것

- **언제나 바로 부르기(A)**: 느린 파일 시스템 하나가 서버의 모든 태스크를 멈춘다.
- **왕복만 싸게(spin)**: 보장은 그대로지만 건당 2~3µs까지로 보여 목표에 못 미치고 CPU를 태운다.
- **마운트 표로 로컬 판정**: 심볼릭 링크가 마운트를 넘나들어 판정에 구멍이 있다.
- **막히는 함수와 아닌 함수를 나누기**(사용자 제안, 검토 후 기각): 05 원칙을 이름으로 깨고 같은 일을 하는 방법이 둘이 된다.
  잘못 고른 대가가 비대칭이다(빠른 쪽을 서버에서 쓰면 파일 서버가 느려질 때만 전체가 멈춘다).
  B가 놓치는 경우(태스크가 여럿이면서 로컬 파일을 아주 많이 확인)가 실제로 문제가 되면, 먼저 런타임이 스스로 판단하는 길을 본다.
