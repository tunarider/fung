# bench

fung 인터프리터 속도를 재는 스크립트. 테스트가 아니라 측정 도구다(결과가 기계마다 다르다).

| 파일 | 재는 것 |
|---|---|
| `fib.fung` | `fib(30)` 재귀: 함수 호출 |
| `loop.fung` | 300만 번 `i * i % 7` 누적: 산술과 반복 |
| `str.fung` | 숫자 50만 개를 문자열로 잇기 |
| `map.fung` | 키 20만 개 맵 만들고 다시 조회 |
| `chan.fung` | 버퍼 없는 채널로 값 10만 개 주고받기: 태스크 전환 |
| `spawn.fung` | 태스크 2,000개 띄우고 모두 기다리기: 태스크 생성 |
| `list.fung` | 리스트 50만 개에 `map`·`filter`·`each`·`find`·`sort_by`: 콜백을 부르는 내장 함수의 리스트 순회 |
| `wait.fung` | 태스크 2만 개가 저마다의 채널에서 함께 기다리기: 기다리는 태스크의 비용 |
| `fs.fung` | `fs.exists`·`fs.stat` 5만 번을 차례로: 스레드 풀에 맡기는 일의 비용 |
| `exec.fung` | `exec(["true"])` 500번을 차례로: 외부 명령을 띄우고 끝과 파이프를 기다리는 비용 |

```
cargo build --release --locked -p fung-cli
time target/release/fung bench/fib.fung
```

`scripts/bench.sh <바이너리>...`는 아래 기록의 방법(파일마다 바이너리마다 준비 1회 + 9회 중앙값)으로 모든 파일을 재서 표로 찍는다.
두 바이너리를 주면 같은 기계에서 나란히 비교한다(예: `scripts/bench.sh /tmp/fung-base target/release/fung`).

다른 언어(Python·Lua·Ruby·Node)와 같은 작업으로 견주는 프로그램은 `compare/`에 있다: `python3 -I bench/compare/run.py target/release/fung`.
fung만 매번 재고, 다른 언어의 결과는 `compare/records/<기계>.json`의 기록을 다시 쓴다(재는 방식·프로그램·데이터·명령·해석기 판이 그대로일 때. `--remeasure`로 모두 다시 잼).
방법·결과·원인 분석은 `docs/notes/bench-languages.md`(2026-10-21), 거기서 나온 할 일은 open-questions "성능" 절의 P8~P13.

## 기록: 0.1.0과 0.2.0 (2026-10-17)

기계: Apple M2 Max (Mac14,13), macOS 27.0.1 arm64. 방법: 파일마다 바이너리마다 준비 운전 1회 뒤 9회 실행해 벽시계 시간의 중앙값(초,
Python `perf_counter`로 프로세스 시작~종료, 출력은 버림). 다른 무거운 작업은 돌리지 않았다. 바이너리 둘:

- **0.1.0**: 사용자가 가진 릴리스(태그 `v0.1.0`, 커밋 `c18f487`).
- **0.2.0**: 0.2.0이 될 구현. 커밋 `a06e1f4`에서 `cargo build --release --locked -p fung-cli`(macOS, 시스템 동적 링크).

| 측정 | 0.1.0 | 0.2.0 | 0.2.0 / 0.1.0 |
|---|---|---|---|
| `fib.fung` | 0.517 | 0.201 | 0.39 |
| `loop.fung` | 0.564 | 0.217 | 0.38 |
| `map.fung` | 0.494 | 0.361 | 0.73 |
| `str.fung` | 0.066 | 0.075 | 1.14 |
| `chan.fung` | 0.112 | 0.251 | 2.24 |
| `spawn.fung` | 0.010 | 0.126 | 12.6 |

musl 정적 빌드는 2026-10-25에 같은 Mac의 Docker 가상 머신(arm64 Linux)에서 glibc 판과 나란히 쟀다(위 표와는 기계가 달라 따로 둔다).
할당이 많은 것은 musl이 4~15% 느리고, 계산은 거꾸로 glibc 판이 느리다. 표와 해석은 `docs/notes/bench-languages.md` 4.1절.
2026-10-14 기록과 다른 언어 비교는 `docs/design/open-questions.md`의 "성능" 절. 0.2.0 구현의 측정 배경은 `docs/notes/R2-vm.md` 9절과
`docs/notes/R3a-runtime.md` 9절(미릴리스 중간 구현과의 비교 포함).
