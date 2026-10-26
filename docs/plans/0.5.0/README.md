# 0.5.0 전에 고칠 문제 (2026-10-26)

0.5.0은 호환을 깬 판이다(`CHANGELOG.md` `[Unreleased]`의 "바뀜(호환 깨짐)"). 릴리스를 서두르지 않고, 그 전에 남은 **문제**를 모두 고친다.
지금은 사용자가 없으므로 하위 호환은 따지지 않고 가장 올바른 쪽을 고른다(바뀐 동작은 CHANGELOG에 계속 적는다).

출처: `docs/design/open-questions.md`("남은 일"·"알려진 한계"·"성능"), `docs/plans/2026-10-20-language-vm-review.md` 12절,
`docs/plans/2026-10-20-manual-findings.md`, 파이프 누수 감사(`claude/hopeful-lalande-9a4b7a`의 `164ce20`).

나눔:
- **작은 문제**: 원인과 고치는 법이 거의 보이고, 한 세션 안에서 시험을 먼저 쓰고 고칠 수 있는 것. 결정이 필요하면 그 자리에서 묻는다. 이 문서 1절, 2026-10-26 세션에서 고친다.
- **큰 문제**: 설계 결정이 크거나, 여러 층을 건드리거나, 측정이 필요한 것. 문제마다 문서를 따로 두고(2절) 개별 세션에서 하나씩 한다.
- **0.5.0 뒤**: 새 기능과 장기 과제(3절). 0.5.0의 범위가 아니다(사용자 결정 2026-10-26).

## 1. 작은 문제 (이 세션)

- [x] **S1** 파이프 누수 감사 브랜치 합치기(`164ce20`)와 그 후속: `testutil::pipe_pair`의 close-on-exec.
- [x] **S2** 취소된 순회가 다음 값을 하나 가져가 잃는다(05 "멈춘 뒤 남은 것은 채널에 남는다"와 어긋남): `each(rx, …)` 안에서 범위를 취소하면 값 3이 사라지고 4만 남는다. `Cursor::next`가 받기 전에 취소를 본다.
- [x] **S3** F-4의 남은 부분: fung으로 쓴 prelude 함수(`take`·`drop`·`first` 등)에 리스트도 문자열도 아닌 값을 주면 내부의 `cannot map over int`가 보인다.
- [x] **S4** 열린 끝 슬라이스의 시작에 `??`·`|>`를 쓰면 문법 에러(`xs[a ?? 1..]` → E0110). EBNF `[ expr ] ".."`와 맞춘다.
- [x] **S5** 같은 이름 인자 `f(a: 1, a: 2)`가 실행 때(E0405)만 잡힌다. 맵 리터럴의 중복 키(E0113)처럼 컴파일 때 잡는다.
- [x] **S6** `Ok()`(인자 없는 태그 값)와 `Ok`(심볼)의 관계를 명세가 말하지 않는다(지금 `Ok() == Ok`는 `false`, `type_of(Ok())`는 `'Ok`). 정하고 02에 적는다.
- [x] **S7** 대안 패턴과 가드: `(a, _) | (_, a) when a > 1`에 `(0, 5)`가 맞지 않는다(가드가 실패해도 두 번째 대안을 시도하지 않음). 정하고 고친다.
- [x] **S8** F-12: `fs.stat`에 `mtime`.
- [x] **S9** 동시성 겉모양과 잔손질(검토 12절): 교착 보고 뒤 멈춘 `release`가 `error[E0702]`를 한 줄 더 찍음, 교착 보고 뒤 `Task::tick`이 `unwound()`가 아니라 E0701로 되감음,
  주차 직전에 세운 `turn_due`가 깨어난 뒤에도 남아 차례를 한 번 더 넘김, `vm/stdin.rs`의 O(n) 중복 검사·제거.
- [x] **S10** 앞단 다듬기(R1 노트): `desugar`·`compiler`의 `lit_str`/`lit_index`가 잘못된 리터럴 종류에 빈 값을 돌려줌, 카탈로그 파싱이 깨진 줄을 받아들임, `SourceMap::add`의 `as u32`,
  맵 리터럴·매개변수 중복 검사의 O(n²), `Display for Node`의 임시 문자열, 깊은 입력 시험을 실제 CLI 스택 크기와 `MAX_DEPTH`에서.
  → 2026-10-26: 매개변수·맵 리터럴의 중복 검사는 집합으로, `SourceMap`은 2³²개를 넘으면 멈춤. 카탈로그는 이제 Rust 표라 파싱하지 않고, 매뉴얼 생성기의 `spec/errors.txt` 읽기가 깨진 줄에 실패한다. 깊은 입력은 dump 기대 출력이 실제 `fung`으로 E0115를 낸다.
    바꾸지 않음: `lit_str`/`lit_index`의 빈 값은 리터럴 없는 노드에 일부러 부르는 곳이 있어 설계다. 렉서의 토큰 문자열과 `Display for Node`의 임시 문자열은 컴파일이 10ms 미만이라 문제가 아니다.
- [x] **S11** REPL: 밖에서 온 SIGTERM·SIGHUP이나 EPIPE로 죽으면 터미널이 raw로 남음, 세션의 `SourceMap`과 런타임이 입력마다 글을 한 벌씩 더 쥠(`Arc<str>`로 나눔).
- [x] **S12** 포매터: 제자리 `fmt`가 원자적이지 않음(임시 파일 + 이름 바꾸기로), 여러 줄 함수 머리의 몸통 들여쓰기, `align_runs`의 O(n²).
- [x] **S13** 작은 동작 결함: ~~`exec`·`sh`의 `env`에 이름에 `=`가 든 키를 주면 조용히 무시됨(에러로)~~ (2026-10-26 E0403), 호스트 이름을 준 `listen`이 풀 스레드에서 소켓을 만들어 그 순간 띄운 명령이 듣는 소켓을 물려받음.
- [x] **S14** 시험 부채(open-questions "남은 일"): `read_all` 쪽 취소 시험의 실패 먼저 확인, `fung-vm`의 `free_port`, 없는 시험(`SSL_CERT_FILE`의 깨진 PEM, `SSL_CERT_DIR`만 있는 경우, TLS 읽기 도중의 취소),
  헤더 10초 한도 시험이 실제로 10초를 기다림, `manual.rs`의 손으로 적은 `MODULES`와 줄 훑기 서명 읽기, `functions.md`의 U+FFFD 글자, 앞단 시험의 이른 실패 때 임시 파일 정리와 `has_source`, `check-release.sh`의 ELF 패턴.
  → 2026-10-26: `free_port`·TLS 시험 셋·`read_all` RED 확인·`MODULES`와 여러 줄 머리·앞단 시험의 정리·U+FFFD. 바꾸지 않음: 헤더 10초 시험(한도를 줄이려면 새 옵션), `check-release.sh`의 ELF 패턴(macOS와 Linux의 `file`은 같은 libmagic이라 문구가 같다), 비UTF-8 이름(macOS가 만들지 못함).
- [x] **S15** 매뉴얼 잔손질(open-questions "매뉴얼"): `fung check` 펜스의 `expect`를 실제 출력과 비교, 진짜 빌드에서 `spec/errors.txt`가 없으면 실패, `latest/reference/functions.html#fn-X` 옛 깊은 링크,
  "범위"(1..10)와 태스크 "범위"(scope) 용어, 배우기 1~7장과 8~14장의 장 끝 구성, 4장의 이른 람다·재귀 소개.
  → 2026-10-26: `expect check` 펜스, 카탈로그가 없거나 깨지면 실패, 옛 깊은 링크는 `manual.js`가 넘김, "태스크 범위"로 용어 정리. 장 끝 구성과 4장은 글을 새로 쓰는 일이라 큰 문제 L18로 옮김.
- [x] **S16** 내부 자리: `need_str_map`의 자리, 가짜 터미널 도우미 `fung_vm::term::pty`가 라이브러리에 있음, `conc`와 `_native.conc`의 `Module` 이름이 둘 다 "conc". → 2026-10-26 셋 다 바꾸지 않음(이유는 open-questions).
- [x] **S18** E0310의 도움말이 없는 모듈을 권한다(``import `net` instead``, L12 문서 작성 중 발견).
- [x] **S19** HTTP 에러 문구가 URL의 비밀번호를 그대로 보인다(리다이렉트 문구만 `***`로 가림). 에러는 로그에 남으므로 모든 에러 URL을 가린다. L13 H8에서 옮김.
- [x] **S20** 큰 문제 문서를 쓰며 드러난 open-questions "알려진 한계"의 낡은 문장: 깊은 `map`·`fold` 중첩은 이제 E0804(L9), 디버그 빌드의 체인 한도는 약 60만(L9),
  "곧바로 끝나는 태스크의 자식은 한 번도 돌지 못한다"는 옛 자유 `spawn`의 설명(L14), 보내는 끝만 서로 쥔 두 채널은 새지 않음(L7).
- [x] **S17** 검토 12절의 남은 의심: `replace`가 키운 문자열이 메모리를 다 쓰면 E0806이 아니라 할당 실패로 끝날 수 있음(확인), 포매터와 파서의 절 판정 차이(확인).

## 2. 큰 문제 (개별 문서, 개별 세션)

| 번호 | 문서 | 요지 |
|---|---|---|
| L1 | [01-unawaited-task-err.md](01-unawaited-task-err.md) | 아무도 `await`하지 않은 태스크의 `Err`가 조용히 사라진다 |
| L2 | [02-trailing-if-for.md](02-trailing-if-for.md) | 함수 본문 끝의 `if`·`for`가 값 자리에서 조용히 `nil`·리스트가 된다 |
| L3 | [03-channel-wait-api.md](03-channel-wait-api.md) | 정지 신호만 기다리는 `_ = recv(stop)`, 기다리지 않고 보기(`try_recv`)가 없음 |
| L4 | [04-strings-and-bytes.md](04-strings-and-bytes.md) | 문자열이 바이트를 품지 못해 비UTF-8 입력이 U+FFFD로 바뀐다 |
| L5 | [05-collections.md](05-collections.md) | 앞에 붙이기의 n², 맵·튜플 상수 접기 없음, 변하는 것을 품은 값의 `send` 비용 |
| L6 | [06-long-natives.md](06-long-natives.md) | Rust로 쓴 긴 내장 함수(큰 `json.parse`, 콜백 없는 정렬)가 차례를 넘기지 않는다 |
| L7 | [07-rc-cycles.md](07-rc-cycles.md) | 회수되지 않는 `Rc` 순환, E0705가 훑지 않는 곳 |
| L8 | [08-tail-call-errors.md](08-tail-call-errors.md) | 라이브러리 함수의 꼬리 호출이 에러 위치와 호출 경로를 흐린다 |
| L9 | [09-deep-values-and-stacks.md](09-deep-values-and-stacks.md) | 재귀인 값 출력과 깊은 네이티브 재귀의 스택 한도 |
| L10 | [10-monotonic-clock.md](10-monotonic-clock.md) | `now()`뿐이라 경과 시간이 시스템 시각 변경에 흔들린다 |
| L11 | [11-files-close-and-blocking.md](11-files-close-and-blocking.md) | `close(2)` 에러를 보고하지 않음, 실행 스레드의 `Drop` 닫기, 끝없는 파일 읽기가 풀 스레드를 붙잡음 |
| L12 | [12-sockets-tls.md](12-sockets-tls.md) | 막힌 호출에 닿지 않는 `set_deadline`, 주소 계열 경주 없음, 이름 붙은 포트, TLS 세부 |
| L13 | [13-http.md](13-http.md) | HTTP 클라이언트·서버의 세부 한계(연결 재사용, 본문 상한, 프록시, URL, 서버 헤더) |
| L14 | [14-scheduling.md](14-scheduling.md) | 형제 태스크의 시작 순서, 띄운 태스크가 바로 돌지 않음, 공정성, REPL Ctrl-C 두 번 |
| L15 | [15-performance.md](15-performance.md) | P14(glibc 판의 계산 속도), P12b(LTO), 레지스터 VM·인라인 캐시 판단 |
| L16 | [16-manual-depth.md](16-manual-depth.md) | 언어 명세를 레퍼런스 깊이로, VM 부록의 손으로 적은 `dump`를 검사되는 펜스로 |
| L17 | [17-display-width.md](17-display-width.md) | 표시 너비가 글자 단위라 ZWJ 이모지·VS16에서 터미널과 다르다 |
| L18 | [18-learn-chapters.md](18-learn-chapters.md) | 배우기 8~14장에 짚고 넘어가기·해 보기가 없음, 4장이 아직 배우지 않은 람다·재귀를 씀 (S15에서 옮김) |

## 3. 0.5.0 뒤 (새 기능·장기 과제)

0.5.0의 범위가 아니다. 자리는 `open-questions.md`에 그대로 있다.

- 여러 코어(M:N, 태스크별 힙), 채널 대신 메일박스.
- 배포: Windows, Homebrew tap, `fung upgrade`, macOS 코드 서명·공증, CI 컴파일 캐시.
- 모듈 기능: `time`의 12시간제·토큰 이스케이프·달력 산술, `http`의 스트리밍·쿠키·TLS 서버·SOCKS, `regex.compile`, 배포용 단일 실행 파일.
- 도구: tree-sitter, LSP, 패키지 관리, 매뉴얼 전문 검색.
- 알파에서 써 보며 조정할 것(prelude 목록, `fst`/`snd`).
