# P11: `regex.find`의 싼 길 (API 그대로) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `regex.find`가 맞을 때마다 결과 맵을 만드는 비용을 줄인다(open-questions P11). 2026-10-21 측정에서 `is_match`의 4배.

**결정 (사용자, 2026-10-25):** API는 그대로 두고 내부만 먼저 줄인다. 그래도 부족하면 새 함수(그룹만 돌려주는 것 등)를 선택지와 함께 다시 묻는다.

**원인 (프로파일):** 시간의 대부분이 할당·해제였다. 맞을 때마다 키 문자열 다섯(`text`·`start`·`end`·`groups`·`named`, 각각 `Rc` 상자와 문자열 버퍼),
이름 붙은 그룹이 없어도 빈 `named` 맵, 크기를 모르고 자라는 맵의 항목 벡터를 새로 만들었다.

**Architecture (`native/regex.rs`):**
- 키 다섯과 빈 `named` 맵을 실행 스레드마다 한 번 만들어 모든 결과가 나눠 쓴다(`thread_local`, 값은 그 스레드에만 산다).
- 결과 맵은 항목 다섯 자리를 미리 잡는다(`MapBuilder::with_capacity`).
- 이름 붙은 그룹이 있을 때만 `named` 맵을 만든다.
- 남는 것: 정규식 엔진의 캡처 검색(one-pass DFA) 자체, 글자 위치 세기, 맞은 글과 그룹의 문자열. 엔진 쪽은 손대지 않는다.

## 작업

- [x] **1. 시험**: 결과가 바뀌지 않음을 고정(`matches_read_the_same_on_ascii_and_multibyte_subjects`: ASCII 주제, 참여하지 않은 그룹, 이름 붙은 그룹, 멀티바이트 뒤의 `find_all`). 동작이 바뀌지 않는 최적화라 먼저 통과하는 특성 시험이다.
- [x] **2. 구현**: 위 Architecture.
- [x] **3. 측정·문서**: 20만 줄의 `find`, 비교의 `regex`, CHANGELOG, open-questions P11, 노트 3.3절.
