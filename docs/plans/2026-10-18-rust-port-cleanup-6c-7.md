# Rust 이식 6c-7 (마무리) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 6c-7 결정표(1~10)대로 남은 흉내 동작 둘과 줄 편집기를 고치고, 코드·현재 문서에서 Go를 걷어내고, 지킴이 테스트로 지킨다. 6c 전체의 완료 기준(코드에 "Go"가 없고 현재 문서에는 언어 비교 문장만)을 만족시킨다.

**Spec:** `docs/superpowers/specs/2026-10-17-rust-port-cleanup-6c-design.md` (3절 완료 기준, 4절 "6c-7 마무리")

## Global Constraints

- 앞 단계들의 Global Constraints를 따른다. 역사 문서(`docs/notes`·`docs/plans`·`docs/superpowers`·`CHANGELOG.md`)의 내용은 바꾸지 않는다(06 링크만 예외).
- 언어 비교 문장은 남겨도 된다. 그 밖에는 Go를 "규칙과 이유"로 바꿔 쓴다.

## Review Focus

- 지킴이가 실제로 찾는다: 바늘을 조각으로 만들어 자신에 걸리지 않고, 범위의 파일 하나에 `Go`를 넣으면 실패한다. → Task 4.
- open-questions에서 옮긴 기록이 노트에 빠짐없이 있고, 열린 항목은 open-questions에 남는다. → Task 6.
- 06 링크가 깨지지 않는다(저장소 전체에서 옛 이름이 없다). → Task 6.

---

### Task 1: 작은 동작 (2a, 2b)
- [ ] RED: `tests/lang`이나 fung-vm 단위 테스트에 `a - b`(b = 최소 기간, 결과가 범위 안)가 값이 되는 사례, `[1,2,3][0..=9223372036854775807]`의 에러가 `index 9223372036854775807`을 보이는 사례.
- [ ] 구현(`value/dur.rs`, `value/index.rs`), bless, 커밋 `fix(vm): exact duration subtraction and the index as written`.

### Task 2: 줄 편집기 (9a, 9b)
- [ ] RED: `keys(b"\xff\xe2\x82a")`가 U+FFFD 둘과 `a`, 한글이 든 줄에서 커서를 왼쪽으로 옮기는 이스케이프의 칸 수가 표시 너비다.
- [ ] 구현(`repl/line.rs`, `repl/editor.rs`), 커밋 `fix(repl): decode keys and place the cursor by fung's rules`.

### Task 3: E0703 (8)
- [ ] 카탈로그·`spec/errors.txt`·08에 "쓰지 않음" 표시, 커밋.

### Task 4: 지킴이 (10)
- [ ] `crates/fung-cli/tests/repo_rust_only.rs`에 낱말 단위 검사와 허용 목록을 더하고 실패 목록을 본다(RED). 이후 Task 5·6이 통과시킨다.

### Task 5: 코드 주석과 테스트 이름 (1)
- [ ] `crates`(기대 출력 포함), `extras`, `scripts`, `deny.toml`, `.cargo`, `.github`, `install.sh`, `bench`의 Go 주석·이름. 크레이트마다 커밋.

### Task 6: 문서 (3~7)
- [ ] open-questions의 이식 기록을 학습 노트로 옮기고 열린 항목을 fung 말로. 00 "영향을 준 언어", README·`index.html` 요약, 이름 유래. 06 이름 바꾸기와 링크. `bench/README.md`. 설계 00~09·`background.md`·매뉴얼·`CLAUDE.md`·`docs/assets`.

### Task 7: 마무리
- [ ] 학습 노트 `docs/notes/R6c-cleanup.md`. 지킴이 통과. 최종 리뷰, 고침. 커밋 전 검사 전체(6c 끝).
