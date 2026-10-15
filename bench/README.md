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
go build -o /tmp/fung ./cmd/fung
time /tmp/fung bench/fib.fung
```

2026-10-14 기록과 다른 언어 비교는 `docs/design/open-questions.md`의 "성능" 절.
Go와 Rust 구현의 비교는 `docs/notes/R2-vm.md` 9절과 `docs/notes/R3a-runtime.md` 9절.
