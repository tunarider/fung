# Changelog

fung의 사용자에게 보이는 변화를 버전별로 적는다. 형식은 [Keep a Changelog](https://keepachangelog.com/ko/1.1.0/),
버전은 [유의적 버전](https://semver.org/lang/ko/)을 따른다. `0.x` 동안은 마이너 버전에서 호환이 깨질 수 있다.

새 변화는 `[Unreleased]`에 쌓고, 릴리스할 때 그 절을 새 버전 제목으로 바꾼다. 릴리스 워크플로는 해당 버전 절을
GitHub 릴리스 노트로 쓰며, 절이 없으면 릴리스가 실패한다.

## [Unreleased]

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
- 동시성: `spawn`·`await`·`cancel`, 채널, `atom`, `sleep`. 교착 감지, 기다리지 않은 실패 보고.
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
