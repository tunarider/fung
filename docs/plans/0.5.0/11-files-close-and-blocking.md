# L11. 파일 닫기의 에러와 끝나지 않는 파일·명령 기다림

상태: 열림 (2026-10-26). 0.5.0 전에 고친다. 목록: [README](README.md).

## 문제

네 가지가 한 뿌리(파일과 명령의 기다림을 어디서, 어떻게 하는가)에 있다.

- **(A) `close(2)`의 에러가 사라진다.** `fs.close`·`fs.write`·`fs.append`·`fs.copy`, `close` 없이 버린 핸들의 버퍼 비우기 모두 `close(2)`의 결과를 버린다.
  NFS는 쓰기를 모아 두었다가 닫을 때 서버에 보내므로(close-to-open) 할당량 초과·디스크 가득 참·`EIO`가 `close`에서야 나온다. 로컬 파일 시스템에서는 재현되지 않는다(이 문서를 쓸 때 재현하지 못함).
  쓰기가 실패했는데 `Ok(nil)`이 나오는 것은 데이터를 조용히 잃는 일이다.
- **(B) 실행 스레드에서 닫는다.** 핸들의 마지막 사본이 사라지면 `Drop`이 버퍼 비우기(`write(2)` 최대 64 KiB)와 `close(2)`를 그 자리에서 한다. 보통 실행 스레드다. NFS가 멈추면 모든 태스크가 멈춘다.
- **(C) 끝나지 않는 파일 읽기.** 알려진 한계에는 "기한에 버려져도 풀 스레드를 붙잡는다"만 적혀 있지만, 실제로는 더 나쁘다. 핸들 연산은 기한 자체를 지키지 못한다.
  ```
  $ mkfifo ff
  $ fung -e 'import fs
  print(timeout(100ms, fn(): fs.read("ff") ;;))'
  Err({kind: 'timeout, message: "timed out after 100ms"})     # 0.13초, 풀 스레드 하나가 남음
  $ fung -e 'import fs
  print(timeout(100ms, fn(): fs.open("ff") ;;))'               # 끝나지 않음(5초 뒤 alarm으로 죽임)
  $ sleep 6 > ff &   # 쓰는 쪽을 열어 두고 아무것도 쓰지 않음
  $ fung -e 'import fs
  f = fs.open("ff")!
  print(timeout(100ms, fn(): read_line(f) ;;))'                # 끝나지 않음(4초 뒤 alarm)
  ```
  버려진 읽기가 쌓이면 다른 파일 일이 줄에 선다. 같은 FIFO 읽기를 70번 버린 뒤의 평범한 파일 쓰기와 읽기가 둘 다 `Err({kind: 'timeout, message: "timed out after 1s"})`였다(macOS, 릴리스 빌드).
- **(D) pidfd가 없는 Linux(5.3 전).** 명령마다 풀 스레드 하나가 `waitid`로 끝을 기다린다. 긴 명령 64개면 파일 일이 줄에 선다. 배포 판이 musl 정적 바이너리라 오래된 커널에서도 돈다.
  RHEL 8(4.18)이 `pidfd_open`을 되돌려 넣었는지는 확인하지 못했다.

## 지금 동작과 원인

- `crates/fung-vm/src/native/file.rs`: 핸들은 `Mutex<Option<Io>>`이고 `Io`는 `BufReader`/`BufWriter<std::fs::File>`. `File::close`는 `flush`의 결과만 돌려주고 `drop(w.into_parts())`로 닫는다.
  Rust 표준 `File`의 `Drop`은 `close(2)`의 에러를 버린다(표준 라이브러리에 실패를 돌려주는 닫기가 없음). 마지막 `Arc<File>`이 사라질 때 `BufWriter`의 `Drop`이 비우고 에러를 버린다.
- `crates/fung-vm/src/native/fs.rs`: `write_fn`·`append_fn`은 `open_writing(..)?.write_all(&text)`, `copy`는 `write_file`(같은 모양)로 끝나 `File`이 문장 끝에서 버려진다. `close_fn`은 `File::close`를 `nat::offload`로 풀에서 돌린다.
- `crates/fung-vm/src/vm/offload.rs`: `Kind::Wait`(바꾸는 일, 돌기 시작하면 멈출 때도 끝까지 기다림)와 `Kind::Abandon`(읽기만, 바로 되감고 결과는 버림).
  `fs.read`·`list`·`glob`·`walk`는 `Abandon`, `fs.open`·`close`·`write_line`과 핸들의 `read_line`·`read_lines`(`prelude/io.rs`)는 `Wait`다. 그래서 (C)의 두 번째·세 번째 예가 기한을 넘긴다.
  핸들 읽기를 `Wait`로 둔 이유는 버려진 읽기가 뒤에서 바이트를 가져가 위치를 바꾸기 때문이다(`R7-scheduler.md` 스펙 4절과의 충돌 문단).
- `crates/fung-vm/src/vm/pool.rs`: `POOL_MAX = 64`, 다 바쁘면 FIFO 줄. 돌고 있는 일은 멈출 수 없다.
- `crates/fung-vm/src/vm/poller.rs`·`poller/epoll.rs`: `exit_watch`가 `pidfd_open`을 부르고 `Unsupported`면 `native/process.rs`의 `wait_on_the_pool`이 풀에서 `waitid(P_PID, WEXITED|WNOWAIT)`로 막힌다.
  풀이 일을 받지 못하면 `POLL_EXIT`(10 ms) 폴링으로 넘어간다. 시험 이음새 `disable_pidfd`가 있다.
- 신호: `crates/fung-vm/src/signals.rs`가 SIGINT·SIGTERM을 모든 스레드에서 막고 전용 스레드가 `sigwait`한다. 자식은 `clear_mask_in_child`로 마스크를 비운다. SIGCHLD는 다루지 않는다.

## 고려할 점

- 닫기 에러: Go `File.Close`, Python `f.close()`는 에러를 돌려준다. Rust는 `Drop`에서 버리고 `sync_all`을 권한다. 닫기 에러를 보고하는 것과 `fsync`(전원이 나가도 남음)는 다른 약속이다. 후자는 비싸고 새 기능에 가깝다.
- `close(2)`가 `EINTR`이면 Linux에서는 fd가 이미 닫혔으므로 다시 부르면 안 된다(다른 스레드가 받은 같은 번호를 닫게 됨). 성공으로 치거나 에러로 보고하되 재시도는 하지 않는다.
- 배경 닫기는 순서를 바꾼다. 지금은 핸들을 버린 바로 뒤의 `fs.read`가 쓴 줄을 본다. 배경으로 넘기면 그 순간이 정해지지 않는다. 지역 변수의 마지막 읽기에서 값을 놓으므로(`CLEARLOCAL`·`MOVELOCAL`) 버리는 순간은 이미 프로그램 글에서 잘 보이지 않는다. 프로그램이 끝날 때는 남은 닫기를 모두 기다려야 데이터를 잃지 않는다.
- 버린 핸들의 비우기 에러를 보고할 자리: 값이 없으므로 표준 에러뿐이다. 태스크 핸들의 `Drop`이 기록된 실패를 표준 에러로 보고하는 선례가 있다(`R3a-runtime.md`). L1(기다리지 않은 태스크의 `Err`)과 같은 "조용한 버림" 문제다.
- 끝나지 않는 읽기: 정규 파일은 늘 "준비됨"이라 이벤트 루프로 기다릴 수 없다(epoll은 정규 파일 등록을 `EPERM`으로 거절). FIFO·문자 장치·유닉스 소켓 파일은 기다릴 수 있다. libuv도 파일은 스레드 풀, 파이프·tty는 이벤트 루프로 나눈다.
  준비된 뒤에만 읽으면 취소가 바이트를 잃지 않는다. 그래서 `Wait`의 이유(위치를 바꾸는 버려진 읽기)가 없어지고 기한을 지킬 수 있다.
- FIFO 열기의 의미: 막히는 `open`은 상대를 기다린다. `O_NONBLOCK` 읽기 열기는 곧바로 되고 쓰는 쪽이 없으면 읽기가 0(끝)을 준다. 쓰기 열기는 읽는 쪽이 없으면 `ENXIO`다. 상대가 아직 안 연 FIFO의 `poll` 결과는 Linux와 macOS가 다르다고 알려져 있다(구현 때 두 OS에서 확인).
- pidfd 없는 대안: SIGCHLD를 막아 두고 신호 스레드가 받아 실행 스레드에 알리면, 기다리는 명령마다 `waitid(.., WNOHANG|WNOWAIT)`로 본다. libuv가 이렇게 한다. SIGCHLD는 합쳐져 한 번 올 수 있으므로 기다리는 명령을 모두 본다(O(명령 수)).

## 선택지

(A) 닫기 에러
(가) `std::fs::File`을 `into_raw_fd`로 꺼내 `libc::close`를 직접 부르는 `close_checked` 하나를 두고, `File::close`·`write`·`append`·`copy`가 모두 그것으로 닫는다. 에러는 지금의 `fs_err`·`fault_value`(`'io` 등) 그대로. 장점: 작고 정확하다. 단점: 없음에 가깝다.
(나) (가)에 더해 `write`·`copy`가 `fsync`. 내구성까지 약속하지만 느리고 새 기능이다.
추천: (가). `fsync`는 0.5.0 뒤의 옵션 후보로 둔다.

(B) 버린 핸들의 닫기
(가) 닫기 전용 스레드 하나(무한 줄). `Drop`이 `Io`를 꺼내 넘기고, 그 스레드가 비우고 닫는다. 런타임이 끝날 때 줄이 빌 때까지 기다린다. 에러는 표준 에러로 알린다. 장점: 실행 스레드가 막히지 않고 풀이 가득 차도 닫기가 밀리지 않는다. 단점: 버린 뒤 곧바로 읽으면 못 볼 수 있다.
(나) 풀에 닫기 일을 넣는다. 장점: 새 스레드 없음. 단점: (C)처럼 풀이 가득 차면 닫기가 줄에 서고, 런타임 끝에서 기다릴 대상이 섞인다.
(다) 그대로 두고 문서만. 모든 태스크가 멈출 수 있는 문제가 남는다.
추천: (가). "`close`나 `with_open` 없이는 파일에 닿는 때를 정하지 않는다"를 06에 적는다.

(C) 끝나지 않는 파일 읽기
(가) 열 때 `fstat`해 정규 파일이 아니면(FIFO·문자 장치·소켓) `O_NONBLOCK`으로 두고 읽기·쓰기를 이벤트 루프(`poller.rs`)로 한다. FIFO 열기는 열기 전 `stat`으로 판정해 버릴 수 있는 열기(`Abandon`, 늦게 열린 fd는 결과와 함께 닫힘)로 한다. 정규 파일은 지금처럼 풀. 장점: 기한·취소를 지키고 바이트를 잃지 않으며 풀 스레드를 쥐지 않는다. 단점: 핸들에 두 갈래가 생기고 OS 차이를 시험해야 한다.
(나) 풀이 오래 막힌 일을 세지 않고 스레드를 더 붙인다(Go식). 줄 서기는 풀리지만 기한을 넘기는 문제(두 번째·세 번째 예)는 그대로다.
(다) 핸들 연산을 모두 `Abandon`으로. 기한은 지키지만 버려진 읽기가 바이트를 가져가 다음 `read_line`이 줄을 잃는다.
추천: (가). 문제의 원인(준비되지 않은 것을 막혀 기다림)을 없앤다.

(D) pidfd 없는 Linux
(가) SIGCHLD를 `signals.rs`의 막는 집합과 `sigwait`에 더하고, 받으면 실행 스레드에 알려 기다리는 명령들이 `WNOHANG|WNOWAIT`로 본다. 장점: 풀 스레드를 쓰지 않는다. 단점: 신호 경로가 하나 는다.
(나) 5.3 미만을 지원하지 않는다고 선언하고 에러로 끝낸다. 오래된 사내 서버(사용자의 쓰임)를 버린다.
(다) 그대로.
추천: (가). `wait_on_the_pool`과 `POLL_EXIT` 폴링을 지울 수 있다.

## 정할 것

1. (A) 닫기 에러만 보고하고 `fsync`는 하지 않는다 — 추천: 예.
2. (B) 버린 핸들의 비우기·닫기 에러를 표준 에러에 알린다(새 번호를 카탈로그에서 붙임) — 추천: 예, 조용히 잃는 것보다 낫다. 아니면 지금처럼 버리고 문서에 적는다.
3. (B) 버린 핸들의 데이터가 파일에 닿는 때를 "정하지 않음"으로 바꾸고, 프로그램 끝에서만 보장한다 — 추천: 예.
4. (C) 정규 파일이 아닌 것을 이벤트 루프로 — 추천: 예. 쓰기 쪽(FIFO에 `write_line`)도 같이 할지 — 추천: 예, 읽는 쪽이 없으면 같은 식으로 멈춘다.
5. (D) SIGCHLD 경로 — 추천: 예, 그리고 풀 대기 경로는 지운다.

## 할 일

1. 시험 먼저(실패 확인). 시간 대신 상태로 판정한다(`testutil`의 탐침, `pool_state()`).
   - (A) `#[cfg(test)]` 이음새로 `close_checked`가 `EIO`를 내게 하고, `fs.close`·`write`·`append`·`copy`가 `Err({kind: 'io, ..})`인지.
   - (B) 닫기를 막는 이음새(래치)로 버린 핸들의 닫기를 붙잡은 채 다른 태스크가 진행하는지, 런타임 끝이 닫기를 기다리는지, 에러가 표준 에러에 나오는지.
   - (C) FIFO의 `fs.open`·`read_line`·`read_lines`·`write_line`이 `timeout` 안에서 `'timeout`으로 끝나는지, 취소 뒤 쓴 줄을 다음 `read_line`이 잃지 않는지, 버린 읽기 70개 뒤 정규 파일 일이 도는지(`fs_tests.rs`의 FIFO 도우미 `release_fifo`).
   - (D) Linux에서 `disable_pidfd` 아래 명령 70개가 풀 스레드를 하나도 쓰지 않고 모두 끝나는지(`process_tests.rs`).
2. 코드: `native/file.rs`(`close_checked`, 두 갈래 핸들), `native/fs.rs`, `prelude/io.rs`, `vm/poller.rs`, 닫기 스레드(런타임 끝의 기다림은 `vm/runtime.rs`), `signals.rs`, `native/process.rs`.
3. 문서: 06 `fs` 절(224행 근처 "그때의 쓰기 에러는 보고되지 않는다"), 05/스펙 15절의 `Wait`·`Abandon` 설명, `open-questions.md` "알려진 한계"의 파일·FIFO·pidfd 줄 삭제, `docs/notes/`에 학습 노트(무엇을 왜).
4. 매뉴얼: `manual/reference/modules/fs.md`, `manual/reference/prelude/concurrency.md`(`timeout`의 FIFO 문장), `manual/vm/12-scheduler.md`·`13-event-loop.md`·`14-cancellation.md`. `FUNG_UPDATE_MANUAL=1`로 다시 만들고 diff 검토.
5. `CHANGELOG.md` `[Unreleased]`: 닫기 에러 보고, 버린 핸들의 닫힘 시점(호환 깨짐), FIFO의 기한.
6. 커밋 전 검사 전체(CLAUDE.md), Linux CI에서 epoll·pidfd 없음 경로 확인.

## 끝의 기준

- 위 시험이 모두 있고, 고치기 전에 실패했고 지금 통과한다(릴리스 50번 반복 포함).
- 이 문서 (C)의 세 재현이 모두 100ms 근처에서 `'timeout`으로 끝나고, 70번 버린 뒤의 파일 쓰기가 `Ok(nil)`이다.
- `close(2)`의 결과를 버리는 곳이 `crates/fung-vm/src/native/`에 없다(`std::fs::File`을 그냥 떨어뜨리는 쓰기 경로 없음).
- open-questions "알려진 한계"에서 해당 네 줄이 사라지고, 06·매뉴얼·CHANGELOG가 새 동작을 말한다.
