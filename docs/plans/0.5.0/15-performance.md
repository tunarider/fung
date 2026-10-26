# L15. 성능: P14(glibc 판의 계산 속도), P12b(LTO), 레지스터 VM·인라인 캐시 판단

상태: 열림 (2026-10-26). 0.5.0 전에 고친다(또는 근거를 남기고 닫는다). 목록: [README](README.md).

세 항목은 모두 명령 해석 루프(`crates/fung-vm/src/vm.rs`의 `step_loop`)의 속도에 관한 것이다. 셋 다 "고칠 버그"라기보다
"측정하고 결정해 닫을 것"이다. 이 문서의 일은 원인을 찾거나 판단 근거를 재서, open-questions "성능" 절의 열린 항목을 닫는 것이다.

## 문제

**P14.** 같은 커밋(`88e7e94`)을 같은 Docker 가상 머신(arm64, Ubuntu 26.04)에서 glibc 판과 musl 정적 판으로 빌드하면
계산이 glibc 판에서 크게 느리다(`docs/notes/bench-languages.md` 4.1절, `scripts/bench.sh` 중앙값, 초).

| | glibc | musl |
|---|---|---|
| fib | 0.323 | 0.218 |
| loop | 0.428 | 0.275 |
| list | 0.364 | 0.294 |
| map / str / wait | 0.214 / 0.090 / 0.069 | 0.222 / 0.102 / 0.079 |

할당이 많은 것(`map`·`str`·`wait`)은 musl이 4~15% 느리고(할당기 몫으로 보임), 할당이 거의 없는 `fib`·`loop`는 거꾸로 glibc가 약 50% 느리다.
보통 기대와 반대다. 원인은 아직 모른다. 배포 판은 musl이라 사용자에게 닿지 않지만, `cargo install`이나 직접 빌드한 Linux 사용자는 glibc 판을 쓴다.
macOS 판(`fib` 0.206초)과 musl 판은 비슷하므로 glibc 판만 튄다. 가상 머신 안이라 재현 조건(Docker 이미지, 빌드 명령)은 노트에 적혀 있지 않다(확인함: 저장소에 Dockerfile 없음).

**P12b.** 릴리스 LTO(`lto = "fat"`·`codegen-units = 1`)는 실행을 5~10% 줄이고(`fib` −7%, `loop` −8%, `map` −7%, `str` −10%, 실행 파일 6.5 → 5.0 MB)
커밋 전 검사의 `cargo test --release --no-run`을 `vm.rs` 하나 고친 뒤 16초 → 70초(thin 54초)로 늘린다(3.4.1절). 2026-10-24 사용자 결정은 "지금은 넣지 않고 열어 둔다,
넣는다면 `[profile.release]`에 바로"다. 지금 작업 공간 `Cargo.toml`에는 `[profile.*]` 절이 아예 없다(확인함).

**레지스터 VM·인라인 캐시.** open-questions "성능"에 "측정을 보고 정한다"로 2026-10-14부터 열려 있다. 0.5.0 전에 하든 안 하든 정해 닫는다.

## 지금 동작과 원인

- 해석 루프: `vm.rs`의 `step_loop`(약 780행부터). 명령마다 현재 프레임을 다시 읽고(`self.frames[fi]`), `Instr`(32비트: 위 8비트 연산, 아래 24비트 인자)을
  `instr.op()`로 푼다. `Instr::op`는 `fung-front/src/bytecode/op.rs`에 있고 `Op::from_u8(..).expect(..)`라 검사가 있으며, 다른 크레이트라 인라인되지 않아
  명령마다 함수 호출이다(3.4.1절 프로파일: 표본의 약 13%). 스택은 `Vec<Option<Value>>`, 호출마다 `tick`(취소 원자 변수 읽기 + 예산 감소)을 지난다.
  `List`·`Map`·`Field`·`Match` 등은 `exec_m4`로, `Assert`·`Test`는 `exec_m6`로 빠진다.
- 스택 기계다. `fib`의 몸통은 18개 명령(`fung dump bytecode bench/fib.fung`): `LOADLOCAL 0; CONST 2; LT; JUMPIFFALSE; …; LOADSELF; LOADLOCAL; CONST 1; SUB; CALL 1; …; ADD; RETURN`.
- 전역은 이미 번호 칸이다(`LOADGLOBAL arg`는 `globals.borrow()[arg]`, 이름 조회 없음). 그래서 동적 언어에서 인라인 캐시가 노리는 "이름 → 칸" 조회가
  전역에는 없다. 남는 후보는 `Field`(맵·모듈 값의 필드, HAMT 조회)인데, 맵에 숨은 클래스(shape)가 없어 캐시할 "모양"이 없다. 모듈 필드(`fs.exists`)가
  어떻게 풀리는지는 이 문서를 쓰며 확인하지 못했다(`exec_m4`의 `Field`를 볼 것).
- P12의 교훈(3.4절·3.4.1절): 루프는 함수 하나가 아주 커서 갈래 하나를 넣고 빼도 LLVM의 레지스터 배정과 코드 배치가 통째로 흔들린다.
  `#[cold]`로 뺀 것(`discarded_stop`·`unset_global`)은 효과가 있었고, `err`·`at`에 `#[cold]`, `Instr::op`에 `#[inline]`은 오히려 느렸다.
- P14의 후보 원인(모두 가설, 확인 안 됨): (1) 같은 소스라도 목표가 다르면 미리 빌드된 `std`와 코드 배치가 달라, P12처럼 루프 배치가 흔들림.
  (2) glibc 판은 PIE 동적 링크라 `memcpy`·`malloc` 같은 libc 호출이 PLT를 거침(musl 정적 판은 직접 호출). (3) 스레드 지역 변수 접근 모델 차이(루프 자체는
  `thread_local!`을 쓰지 않음, `value/hamt.rs`·`native/regex.rs`만 씀). (4) 실행 스레드의 4 GiB 스택 예약(`fung_vm::stack::ENTRY_STACKS`)이 두 libc에서 다르게 매핑됨.

## 고려할 점

- 용도: fung은 입출력 위주 스크립트 언어다. 기본 계산은 이미 Python의 1.3배·Ruby 4 YJIT의 1.5배(macOS 4.1절)이고, 약점은 텍스트 처리량(할당·호출 규약)이다.
  해석 루프를 빠르게 하는 일은 같은 노력으로 텍스트 쪽보다 얻는 것이 작다(4절 결론).
- 레지스터 VM(Lua 5, Dalvik): 명령 수가 줄고(스택 push/pop 대신 칸 번호) 보통 20~30% 빠르지만, 컴파일러(칸 배정), VM, `dump` 출력, 모든 dump 기대 출력,
  매뉴얼 VM 부록, 마지막 읽기에서 넘기기(`MOVELOCAL`·`CLEARLOCAL`)까지 다시 짜야 한다. CPython은 3.11에서 레지스터로 가지 않고 "특화 명령 + 인라인 캐시"
  (적응형 해석기)를 골랐다. 묶음 명령(superinstruction, 예: `LOADLOCAL`+`CONST`+`LT`)은 작고 국소적인 중간 길이다.
- 측정의 함정: Docker 가상 머신·부하 평균이 높은 Mac에서는 절대값보다 같은 시각 나란히 잰 비율만 믿는다(`scripts/bench.sh <전> <후>`).
  루프를 건드리는 변경은 `fib`·`loop`를 붙이는 것이 이미 규칙이다(3.4절).
- LTO의 대가는 실행이 아니라 커밋마다의 시험 빌드다. 원인은 `crates/fung-cli/tests/`의 통합 시험 실행 파일이 수십 개(파일 31개)라 링크마다 다시 최적화하는 것이다.
  시험을 실행 파일 하나로 모으면(`tests/main.rs` + `mod`) LTO 없이도 링크 시간이 줄지만, 시험 사이의 전역 상태(환경 변수, 잠금, `perf` 줄 세우기)를 점검해야 한다.

## 선택지

P14:
(가) 원인을 찾는다: Docker arm64에서 두 판을 빌드해 `perf stat`(명령 수·분기 실패·i-cache 실패)과 `perf record`로 `step_loop` 비교, 루프의 기계어 크기 비교,
`-C relocation-model=static`·`+crt-static` glibc 판, 두 판 모두 LTO, `x86_64`에서도 재현되는지. 상한 시간을 정해 두고 한다. 장점: 루프 배치 문제라면 P12와 같은 교훈을 문서로 남긴다. 단점: 원인이 "배치 운"이면 고칠 것이 없다.
(나) 기록만 하고 닫는다: 배포 판은 musl이다. 장점: 비용 없음. 단점: 직접 빌드하는 Linux 사용자가 50% 느린 판을 쓰는데 이유를 모른다.
추천: (가)를 하루 상한으로. 원인이 PIE·PLT면 고칠 길(정적 재배치, 해당 호출 피하기)이 있고, 배치 문제면 노트에 남기고 닫는다.

P12b:
(가) 넣지 않고 닫는다(기각으로 기록). (나) `lto = "thin"`: 이득이 `fib`에서만 크고 대가는 거의 같다. (다) `lto = "fat"`·`codegen-units = 1`을 `[profile.release]`에.
(라) 먼저 통합 시험을 실행 파일 하나로 모아 다시 재고, `--release --no-run`의 증가가 작으면(예: 15초 이하) (다).
추천: (라). 시험 모으기는 LTO와 상관없이 커밋 전 검사를 줄이므로 그 자체로 이득이 있고, 그 뒤의 수치로 (가)와 (다) 중 하나로 확실히 닫는다. (나)는 이득이 작아 뺀다.

레지스터 VM·인라인 캐시:
(가) 하지 않는다로 닫고 다시 볼 조건을 적는다. (나) 묶음 명령 몇 개만 실험(가장 흔한 명령 쌍을 세어서). (다) 레지스터 VM으로 다시 짠다.
추천: (가). 인라인 캐시는 전역이 이미 칸 번호라 대상이 거의 없고, 레지스터 VM은 0.5.0의 "문제 고치기" 범위를 넘는 재작성이다(README 3절의 장기 과제 성격).
다시 볼 조건: 계산이 주 용도인 사용 사례가 생기거나, 텍스트 쪽 할당 문제(L4·`Rc<str>`)를 고친 뒤에도 `core` 묶음이 Python의 1.5배를 넘을 때. (나)는 0.5.0 뒤로.

## 정할 것

1. P14를 얼마나 팔지: 하루 상한으로 원인을 찾고, 못 찾으면 기록하고 닫는가? (추천: 예)
2. P14의 원인이 루프 배치 운으로 나오면, glibc 판을 위해 루프를 손볼 것인가? (추천: 아니오. 배포 판과 macOS 판이 기준이고, P12에서 배치를 쫓는 변경은 다른 판을 흔들었다)
3. P12b 전에 통합 시험을 실행 파일 하나로 모으는 일을 먼저 할 것인가? (추천: 예. 모은 뒤 `--release --no-run` 증가가 15초 이하면 fat LTO를 넣고, 넘으면 기각으로 닫는다. 기준 초는 사용자와 정한다)
4. 레지스터 VM·인라인 캐시를 "하지 않는다"로 닫고 다시 볼 조건을 적는가? (추천: 예, 조건은 위 추천대로)
5. 묶음 명령 실험을 0.5.0에 넣을 것인가? (추천: 아니오, 0.5.0 뒤)

## 할 일

1. 측정 재현 조건을 먼저 적는다: Docker 이미지·빌드 명령(glibc: `cargo build --release --locked -p fung-cli`, musl: `--target aarch64-unknown-linux-musl`, `CC_aarch64_unknown_linux_musl=musl-gcc`)을 `bench/README.md`에.
2. P14: 위 (가)의 실험을 하고 결과 표를 `docs/notes/bench-languages.md`에 새 절로 붙인다. 고칠 것이 나오면 `scripts/bench.sh <전> <후>`로 macOS·musl·glibc 세 판을 잰다.
   시험으로 지킬 동작 변화가 없으므로 TDD 대상은 아니다. 고칠 때 동작이 바뀌는 곳이 있으면 시험을 먼저 쓴다.
3. P12b: 통합 시험을 모으는 경우 먼저 사례 수 단언(`DUMP_CASES` 등)과 시험 이름이 그대로인지 확인하는 방법을 정하고, 모으기 전후의
   `cargo test --release --no-run`(처음부터, `vm.rs` 한 파일 고친 뒤)과 `cargo test --release` 전체 시간을 잰다. 그 뒤 LTO를 켜고 같은 것을 잰다.
   넣으면 `Cargo.toml`에 `[profile.release]`를 더하고, `scripts/build-release.sh`·CI(`ci.yml`·`release.yml`) 시간을 확인한다. `cargo-deny`·`deps.rs`에는 닿지 않는다.
4. 레지스터 VM: open-questions의 항목을 결정과 조건으로 바꾼다. 매뉴얼 VM 부록(`manual/vm/internals.md`)이 레지스터 기계를 비교로 말하면 결정과 맞는지 본다.
5. 문서: open-questions "성능"의 P14·P12b·레지스터 VM 항목을 닫고(닫은 것은 `docs/notes/open-questions-closed.md`로), `CLAUDE.md` "현재 상태"의 성능 줄을 고친다.
6. CHANGELOG `[Unreleased]`: 사용자에게 보이는 변화(LTO로 빨라짐·작아진 실행 파일, glibc 판 수정)가 있을 때만 적는다. 측정·결정만이면 적지 않는다.
7. 커밋 전 검사 전부(LTO를 넣었다면 늘어난 시간을 커밋 메시지나 노트에 적는다).

## 끝의 기준

- P14: 원인이 노트에 적혔거나(재현 조건과 실험 표 포함), 정한 상한 안에 찾지 못했다는 기록과 함께 닫혔다. 고쳤다면 glibc 판 `fib`·`loop`가 musl 판의 10% 안이다.
- P12b: `[profile.release]`에 LTO가 들어갔거나 기각으로 닫혔고, 어느 쪽이든 전후 시험 빌드 시간과 실행 시간 표가 3.4.1절 옆에 있다.
- 레지스터 VM·인라인 캐시: open-questions에 결정과 다시 볼 조건이 적혔다.
- open-questions "성능" 절에 이 세 항목이 열린 채로 남지 않는다. 커밋 전 검사가 모두 통과한다.
