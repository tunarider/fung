# Changelog

fung의 사용자에게 보이는 변화를 버전별로 적는다. 형식은 [Keep a Changelog](https://keepachangelog.com/ko/1.1.0/),
버전은 [유의적 버전](https://semver.org/lang/ko/)을 따른다. `0.x` 동안은 마이너 버전에서 호환이 깨질 수 있다.

새 변화는 `[Unreleased]`에 쌓고, 릴리스할 때 그 절을 새 버전 제목으로 바꾼다. 릴리스 워크플로는 해당 버전 절을
GitHub 릴리스 노트로 쓰며, 절이 없으면 릴리스가 실패한다.

## [Unreleased]

### 고침

- `fung fmt`가 `fn` 타입 표기가 있는 매개변수(`fn f(h: fn, x = 1):`) 뒤의 블록 몸통을 내어쓰던 문제. 타입 이름 `fn`을 함수 머리로 읽었다.
- `fung fmt`가 BOM(U+FEFF)으로 시작하는 파일을 "formatter changed the code"로 거절하던 문제. 이제 BOM을 떼고 포맷한다.
- `fung fmt --check -`가 `--check`를 무시하고 포맷 결과를 찍던 문제. 이제 바뀔 게 있으면 `<stdin>`을 찍고 종료 코드 1이다.

### 성능

- 함수 호출과 반복이 약 2.5배 빨라졌다. fung 함수 호출은 힙 할당 없이 인자를 프레임에 바로 놓고, `fold`·`map`·`each`·`filter`는
  범위를 리스트로 펼치지 않는다. 산술 명령을 고르던 맵 조회도 없앴다.

## [0.1.0] - 2026-10-14

첫 공개 릴리스. 언어의 기본 기능, 표준 모듈, 개발 도구를 갖춘 알파다.

### 언어

- 값: 정수·실수·문자열(보간 `${}`)·불리언·`nil`·심볼·기간(`5s`, `2h`)·시간, 리스트·맵·튜플·범위, 태그(`Ok(x)`, `Err(e)`).
- 블록은 `:`로 열고 `;;`로 닫는다. 들여쓰기는 의미가 없다.
- 함수: `fn` 짧은 형·블록 형, 람다 `x => …`, 위치·이름 인자, 기본값, 타입 표기 검사, 꼬리 호출 최적화.
  클로저는 만들어질 때 보이는 변수를 복사한다.
- 패턴: `match`, 구조 분해 대입, guard, `...rest`, 매개변수 패턴.
- 에러: `fail`, 결과값 연산자 `!`·`?`·`??`, 소스 발췌와 호출 경로가 붙은 에러 출력, 에러 번호 카탈로그.
- 파이프 `|>`, 자리표시 `_`, `for`(→ `map`), `in`, `type` 별칭.
- 모듈: `import`, 모듈별 전역.
- 동시성: `spawn`·`await`·`cancel`, 채널, `atom`, `sleep`. 교착 감지.
- 끝내 아무도 `await`하지 않은 태스크의 실패는 보고되고 종료 코드가 1이 된다. 보고 여부는 실행 순서와 무관하다.
- 리스트와 맵은 영속 자료구조다. 누적과 꼬리 재귀가 선형이다.

### 표준 모듈

- prelude: 컬렉션·문자열·변환·결과값·입출력·프로세스 함수.
- `fs`, `path`, `cli`, `time`, `json`, `regex`, `http`(클라이언트와 `serve`).
- prelude와 표준 모듈을 합쳐 내장 함수 128개.

### 도구

- `fung <script>`, `fung -e`, REPL(줄 편집, 기록 `~/.fung_history`).
- `fung test`(같은 파일의 `test` 블록과 `assert`), `fung check`(경고 W0101~W0104), `fung fmt [--check]`, `fung dump`.
- 셰뱅 `#!/usr/bin/env fung`.

### 배포

- Linux·macOS(amd64, arm64) 정적 바이너리, `SHA256SUMS`, 빌드 증명.
- 설치 스크립트 `install.sh`, `go install github.com/tunarider/fung/cmd/fung@latest`.
- HTML 매뉴얼(`docs/`, GitHub Pages).

[Unreleased]: https://github.com/tunarider/fung/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/tunarider/fung/releases/tag/v0.1.0
