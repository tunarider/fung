# P10: `lines`를 네이티브로 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 텍스트 경로의 군살을 뺀다. `lines`는 fung으로 쓴 prelude 함수(`split` 뒤 줄마다 `_chomp` 람다)라 날 `split`의 2배다(open-questions P10, `docs/notes/bench-languages.md` 3.3절).

**결정 (사용자, 2026-10-25):** `lines`를 Rust로 옮긴다(원칙 5와 맞바꿈, `get`의 전례). 규칙은 그대로다(06, `read_lines`와 같음):
`\n`으로 나누고, 줄마다 끝의 `\r` 하나를 떼고, 마지막 줄바꿈은 빈 줄을 만들지 않으며, 빈 문자열은 줄이 없다.
문자열이 아닌 값은 E0403(`argument `s` must be str`)이고, 이제 호출 경로에 prelude 내부(`_chomp`)가 보이지 않는다.

**Architecture:** `prelude/strings.rs`에 `lines_fn`, `extras/prelude.fung`에서 `lines`와 이제 쓰이지 않는 `_chomp`를 지운다(다른 곳에서 쓰면 남긴다).
`spec/builtins.txt`의 `prelude fung lines`가 `prelude native lines(s)`가 된다(`FUNG_UPDATE_SPEC=1`).
줄마다 단어를 나누는 `split`은 이미 네이티브다. 측정해서 군살(할당)이 보이면 함께 본다.

## 작업

- [x] **1. 시험 먼저** (`prelude/tests.rs`): 규칙의 경우들(`""`, `"a\nb"`, `"a\n"`, `"a\r\nb\r\n"`, `"a\n\n"`, `"\n"`, `"a\r"`, `"\r\n"`, `"x\r\r\n"`)이 지금과 같고, `lines(1)`의 호출 경로에 `extras/prelude.fung`가 없다.
- [x] **2. 구현**, 명세 갱신, 기대 출력(dump·run의 prelude 프레임)이 바뀌면 검토 후 bless.
- [x] **3. 문서**: 매뉴얼 함수 레퍼런스(구현 위치를 말하면), CHANGELOG(성능·고침), open-questions P10, 노트 3.3절.
- [x] **4. 측정**: `bench/compare/run.py`의 `text`·`regex`, 3.3절의 프로브.
