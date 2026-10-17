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

```
cargo build --release --locked -p fung-cli
time target/release/fung bench/fib.fung

go build -trimpath -o /tmp/fung-go ./cmd/fung
time /tmp/fung-go bench/fib.fung
```

## 기록: v0.1.0, Go HEAD, Rust (2026-10-17)

기계: Apple M2 Max (Mac14,13), macOS 27.0.1 arm64. 방법: 파일마다 바이너리마다 준비 운전 1회 뒤 9회 실행해 벽시계 시간의 중앙값(초,
Python `perf_counter`로 프로세스 시작~종료, 출력은 버림). 다른 무거운 작업은 돌리지 않았다. 바이너리 셋:

- **v0.1.0**: 사용자가 가진 릴리스. 태그 `v0.1.0`(커밋 `c18f487`)을 `git archive`로 풀어 `go build -trimpath`.
- **Go HEAD**: 릴리스되지 않은 Go 구현. 커밋 `a06e1f4`(Go 소스는 `a42db83` 이후 그대로)에서 `go build -trimpath`.
  v0.1.0 뒤의 성능 1·2단계(`f4e88e1`, 할당 0 호출 등)가 들어 있다.
- **Rust**: 0.2.0이 될 구현. 커밋 `a06e1f4`에서 `cargo build --release --locked -p fung-cli`(macOS, 시스템 동적 링크).

| 측정 | v0.1.0 (릴리스) | Go HEAD (미릴리스) | Rust | Rust / v0.1.0 | Rust / Go HEAD |
|---|---|---|---|---|---|
| `fib.fung` | 0.517 | 0.207 | 0.201 | 0.39 | 0.97 |
| `loop.fung` | 0.564 | 0.241 | 0.217 | 0.38 | 0.90 |
| `map.fung` | 0.494 | 0.455 | 0.361 | 0.73 | 0.79 |
| `str.fung` | 0.066 | 0.055 | 0.075 | 1.14 | 1.36 |
| `chan.fung` | 0.112 | 0.091 | 0.251 | 2.24 | 2.76 |
| `spawn.fung` | 0.010 | 0.010 | 0.126 | 12.6 | 12.6 |

musl 정적 빌드 열은 비어 있다. 이 기계에는 musl 타깃도 교차 도구도 없어 만들 수 없었고, 추정치는 적지 않는다.
Linux 기계나 CI에서 `scripts/build-release.sh`로 만든 musl 바이너리로 같은 방법으로 재서 채운다.
2026-10-14 기록과 다른 언어 비교는 `docs/design/open-questions.md`의 "성능" 절.
Go와 Rust 구현의 비교는 `docs/notes/R2-vm.md` 9절과 `docs/notes/R3a-runtime.md` 9절.
