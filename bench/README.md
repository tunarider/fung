# bench

fung 인터프리터 속도를 재는 스크립트. 테스트가 아니라 측정 도구다(결과가 기계마다 다르다).

| 파일 | 재는 것 |
|---|---|
| `fib.fung` | `fib(30)` 재귀: 함수 호출 |
| `loop.fung` | 300만 번 `i * i % 7` 누적: 산술과 반복 |
| `str.fung` | 숫자 50만 개를 문자열로 잇기 |
| `map.fung` | 키 20만 개 맵 만들고 다시 조회 |

```
go build -o /tmp/fung ./cmd/fung
time /tmp/fung bench/fib.fung
```

2026-10-14 기록과 다른 언어 비교는 `docs/design/open-questions.md`의 "성능" 절.
