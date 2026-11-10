# 언어·VM 전체 점검 결과와 작업 목록 (2026-11-10)

0.6.0 릴리스 직후, 언어 명세와 구현 전체(앞단·컴파일러·VM·값 층·동시성 런타임·표준 라이브러리·CLI)를 "고칠 수 있는 문제" 기준으로 다시 점검한 결과다.
2026-11-08 점검(`docs/plans/2026-11-08-full-review.md`, 98개 모두 처리)의 후속이고 형식도 같다. 새 세션이 이 문서만 읽고 항목 하나(또는 한 묶음)를 이어서 고칠 수 있게 썼다.
항목을 처리하면 체크하고 `처리:` 줄에 커밋과 바꾼 문서를 적는다.

**상태 (2026-11-11)**: 74개 항목을 모두 처리했다(작업 브랜치, 아직 main에 스쿼시하지 않음). 결정은 `docs/plans/2026-11-10-full-review-fixes.md` 1절(D1~D34)이 이 문서의 선택지보다 앞서고,
항목마다 `처리:` 줄에 결정과 다르게 한 곳(컨트롤러 판단으로 넓힌 것 등)을 적었다. 12절(확인하지 못한 의심)은 이번 작업의 범위가 아니라 그대로 남았다.

## 0. 이 문서를 쓰는 법

- **기준 커밋**: `4fe5af3`(0.6.0 릴리스). 줄 번호는 이 커밋 기준이고 함수 이름을 함께 적었다. 줄이 밀리면 함수 이름으로 찾는다.
- **범위**: 설계 문서 00~08·background, 매뉴얼 언어 명세(`manual/reference/language/`, 0.6.0부터 규범)·함수 레퍼런스·VM 문서(`manual/vm/`), `spec/builtins.txt`·`spec/errors.txt`,
  동시성 스펙(`docs/superpowers/specs/2026-10-18-concurrency-v2-r7-design.md`), 앞단 전체, 컴파일러·바이트코드, VM·값 층·prelude(`extras/prelude.fung` 포함),
  동시성 런타임, 표준 라이브러리 모듈(`fs`·`path`·`json`·`time`·`regex`·`http`·`cli`·`conc`), CLI 도구(`run`·`check`·`test`·`fmt`·`dump`·REPL).
- **뺀 것**: 2026-11-08 점검의 98개 항목(모두 처리)과 그 11절(확인했고 문제없음), open-questions "남은 일"·"알려진 한계", 2026-10-20 검토의 고친 항목.
  다만 고친 것이 **불완전하거나 새 문제를 만든** 경우는 냈다(3절 마지막 칸 "지난 점검과의 관계"에 어느 항목의 후속인지 적었다). 2026-11-08 점검 12절(확인하지 못한 의심)은 모두 다시 확인했다(11절 첫 묶음).
- **방법**: 여섯 영역(명세·앞단·컴파일러와 VM·값과 prelude·동시성·표준 라이브러리와 CLI)을 따로 점검하고, 모든 주장을 릴리스 바이너리로 재현했다.
  "재현 안 함(코드 읽기)"이라고 적은 것만 예외다. 영역 사이에 겹친 항목은 하나로 합쳤다(제목 끝에 "… 합침"). 합친 쪽의 재현·근거·고치는 법은 남긴 항목에 녹였다.
  - 성능 수치는 Apple Silicon·macOS의 릴리스 빌드다.
  - 6절(VM)의 수치는 Apple M2 Max, 부하 평균 4~5에서 바이너리를 라운드마다 번갈아 9회 돌린 CPU 시간(user+sys)의 중앙값이다.
    "실험판"은 저장소 사본에서 그 부분만 바꿔 `CARGO_TARGET_DIR`을 따로 두고 빌드한 바이너리다(저장소에는 넣지 않았다). 같은 방식으로 HEAD를 빌드한 기준판은 저장소 바이너리와 같은 시간이 나왔다.
  - 7절(값)의 수치는 같은 프로세스 안에서 `time.monotonic()`의 차다. 메모리는 `/usr/bin/time -l`의 최대 상주 크기다.
- **재현**: `cargo build --release --locked` 뒤 저장소 루트에서 `F=target/release/fung`.
  - 여러 줄 소스는 `x.fung`로 저장해 `$F x.fung`. `$F -e '…'` 안의 줄바꿈은 그대로 붙여 넣는다.
  - 동시성 재현은 멈출 수 있으니 `perl -e 'alarm 10; exec @ARGV' $F x.fung`로 시간을 묶는다.
  - 큰 파일은 macOS `mkfile -n 200m big.bin`(Linux는 `truncate -s 200M big.bin`).
  - HTTP 재현은 로컬 루프백만 쓴다. 도우미는 이 절 끝에 있다. 9절에서 `canned '답'`은 `python3 -I canned.py 포트 '답' &`를 띄우고 fung 클라이언트로 `http.get("http://127.0.0.1:포트/…")`를 찍는 줄임이고,
    `raw '요청'`은 `srv.fung`(포트 23510)를 띄운 뒤 `python3 -I raw.py 23510 '요청'`의 줄임이다. 결과의 `p`·`q`·`h`는 `srv.fung`가 되돌려 준 `req.path`·`req.query`·`req.headers`다.
- **ID**: 지난 점검과 같은 두 글자 접두사를 쓴다. SP 명세 · FE 앞단 · VM 컴파일러와 VM · VA 값과 prelude · CC 동시성 · LB 표준 라이브러리 · TL CLI 도구.
  - **번호는 지난 점검(2026-11-08)에 이어 매긴다**: SP12~, FE23~, VM14~, VA18~, CC9~, LB23~, TL6~. 지난 점검은 SP1~11, FE1~22, VM1~13, VA1~17, CC1~8, LB1~22, TL1~5를 썼다.
  - 그래서 본문의 ID는 번호만 보고 가린다: **지난 범위 안의 번호는 2026-11-08 점검의 항목**이다(예: "VM4 후속"은 지난 점검 VM4의 후속). 헷갈릴 만한 곳에는 "(11-08 VM4)"로 표시했다.
    `SP5(2026-11-08)`처럼 날짜를 붙인 것도 지난 항목이다.
  - 다른 목록: L·S 번호는 0.5.0 목록(`docs/plans/0.5.0/`), F-·D- 번호는 매뉴얼을 쓰며 찾은 문제(`docs/plans/2026-10-20-manual-findings.md`), "12절 의심"은 지난 점검 문서의 12절이다.
  - 각 영역 절 안의 순서는 심각도(높음 → 중간 → 낮음)이고, 같은 심각도 안에서는 점검 때의 순서다. 번호는 이 순서로 매겼다.
- **분류**
  - **버그**: 명세가 정답을 말한다. 결정 없이 TDD로 고친다.
  - **결정**: 언어 동작이 바뀌거나 명세가 비어 있다. CLAUDE.md 규칙대로 선택지 + 추천 + 이유로 사용자에게 묻고, 결정 뒤 해당 문서를 고친다.
    각 항목의 선택지와 추천은 점검 때의 의견이고 결정이 아니다. 0.6.0의 사용자 지침(하위 호환 무시, 엄격함 우선, 예외·특수한 경우 최소화)을 기준으로 추천했다.
  - **성능**: 의미는 맞고 비용이 문제다. 측정 → 고침 → 같은 측정을 기록한다(`docs/notes/bench-languages.md` 3.4.1의 측정 규칙).
  - **정리**: 죽은 코드·중복·구조·진단 문구. 동작(결과 값)은 바뀌지 않는다.
  - **문서**: 문서끼리, 또는 문서와 구현이 어긋난다.
- **심각도**: 높음(정상적인 쓰임에서 프로그램이 죽거나 언어의 핵심 약속이 깨짐) · 중간 · 낮음.
- **고칠 때의 공통 절차**(CLAUDE.md)
  - 시험을 먼저 쓰고 실패를 확인한다. 언어 동작은 `tests/lang/*.fung`의 `# expect:`·`# expect-error:`, 내부는 해당 모듈의 `*_tests.rs`.
  - 커밋 전 검사 전체(`cargo fmt --check`, clippy 두 번, `cargo test` 두 번, `cargo deny`·`cargo audit`).
  - 사용자에게 보이는 변화는 `CHANGELOG.md`의 `[Unreleased]`. 문법·내장 함수·내부 구조가 바뀌면 매뉴얼 원고를 고치고
    `FUNG_UPDATE_MANUAL=1 cargo test -p fung-cli --test manual_build`로 다시 만든 뒤 diff를 검토한다.
  - 새 에러 번호는 `crates/fung-front/src/diag/catalog.rs`에서 붙이고 `FUNG_UPDATE_SPEC=1 cargo test spec_file`. 이미 붙은 번호의 뜻은 바꾸지 않고, 은퇴한 번호는 다시 쓰지 않는다.
  - `.fung` 파일이나 출력이 바뀌면 기대 출력을 `FUNG_EXPECTED=bless …`로 다시 쓰고 diff를 검토한다. 명령 번호는 고정이다(`bytecode/op.rs` 머리 주석).
  - **이번 점검의 교훈**: 항목의 약 30%(22개)가 지난 고침(지난 점검과 0.5.0 목록)의 후속이다(1절). 고칠 때 같은 개념을 판정하는 다른 자리(다른 함수, 다른 문법 자리, fung으로 쓴 모듈, 매뉴얼·설계 문서·스펙·코드 주석)를 `grep`으로 모두 찾고, 판정을 한 곳으로 모은다.
- **HTTP·TLS 재현 도우미**(임시 디렉터리에 저장해 `python3 -I`로 돌린다)
  ```python
  # canned.py PORT REPLY...: 연결마다 준비한 답 하나를 차례로 보내고, 받은 요청 머리를 표준 에러에 찍는다
  # usage: canned.py port reply1 [reply2 ...]  -- serves each reply to one connection, logs request heads to stderr
  import socket, sys, codecs
  port = int(sys.argv[1])
  replies = [codecs.decode(r, 'unicode_escape').encode('latin-1') for r in sys.argv[2:]]
  s = socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
  s.bind(('127.0.0.1', port)); s.listen(8); s.settimeout(8)
  for r in replies:
      try:
          c, _ = s.accept()
      except Exception:
          break
      c.settimeout(2)
      data = b''
      try:
          while b'\r\n\r\n' not in data:
              d = c.recv(65536)
              if not d: break
              data += d
      except Exception: pass
      sys.stderr.write('--- got: %r\n' % data); sys.stderr.flush()
      c.sendall(r)
      c.close()
  ```
  ```python
  # raw.py PORT REQ...: 날 요청을 하나씩 보내고 답의 첫 줄과 본문 앞부분을 찍는다
  import socket, sys, codecs, time
  port = int(sys.argv[1])
  for req in sys.argv[2:]:
      data = codecs.decode(req, 'unicode_escape').encode('latin-1')
      s = socket.create_connection(('127.0.0.1', port)); s.settimeout(3)
      s.sendall(data)
      out = b''
      try:
          while True:
              d = s.recv(65536)
              if not d: break
              out += d
      except Exception as e: out += b'<<%r>>' % e
      s.close()
      head, _, body = out.partition(b'\r\n\r\n')
      print(repr(data[:80]), '=>', head.split(b'\r\n')[0], body[:300])
  ```
  ```python
  # tlssrv.py PORT MODE REPLY: TLS 서버. 요청 머리를 받고 REPLY를 보낸 뒤,
  # MODE가 notify면 close_notify로 닫고, 그 밖(hard)이면 close_notify 없이 프로세스를 끝낸다(TCP FIN만 간다)
  import socket, ssl, sys, codecs
  port = int(sys.argv[1]); mode = sys.argv[2]; reply = codecs.decode(sys.argv[3], 'unicode_escape').encode('latin-1')
  ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER); ctx.load_cert_chain('tls/srv.pem', 'tls/srv.key')
  s = socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1); s.bind(('127.0.0.1', port)); s.listen(4); s.settimeout(8)
  c, _ = s.accept(); t = ctx.wrap_socket(c, server_side=True)
  data = b''
  while b'\r\n\r\n' not in data:
      d = t.recv(65536)
      if not d: break
      data += d
  t.sendall(reply)
  if mode == 'notify':
      t.unwrap(); c.close()
  else:
      # hard close: no close_notify
      import os; os._exit(0)
  ```
  `tlssrv.py`의 인증서(`tls/` 아래, 클라이언트는 `SSL_CERT_FILE=tls/ca.pem`). 점검 때는 openssl 4.0.3으로 아래 `ext.cnf` 설정의 사설 CA와 서버 인증서를 만들었다. 만드는 예:
  ```
  mkdir tls && cd tls
  printf 'subjectAltName=DNS:localhost,IP:127.0.0.1\nbasicConstraints=CA:FALSE\nextendedKeyUsage=serverAuth\n' > ext.cnf
  openssl req -x509 -newkey rsa:2048 -nodes -keyout ca.key -out ca.pem -days 30 -subj /CN=test-ca
  openssl req -newkey rsa:2048 -nodes -keyout srv.key -out srv.csr -subj /CN=localhost
  openssl x509 -req -in srv.csr -CA ca.pem -CAkey ca.key -CAcreateserial -out srv.pem -days 30 -extfile ext.cnf
  ```
  요청을 JSON으로 되돌려 주는 서버 `srv.fung`(`$F srv.fung &`):
  ```
  import http
  import json
  http.serve("127.0.0.1:23510", req => {status: 200, body: json.stringify({m: req.method, p: req.path, q: req.query, h: req.headers, b: req.body}) + "\n"}, ready: a => eprint("ready", a))!
  ```

## 1. 총평

언어 하나와 바이트코드 VM을 평가하는 입장에서 본 전체 인상이다. 개별 항목보다 이 절의 경향이 다음 작업의 방향을 정하는 데 더 쓸모 있다.

**잘 된 것.** 지난 점검의 98개 고침은 대부분 단단하다. 지난 항목을 다시 돌려 본 것(명세 SP1~SP11의 처리, 처리기 태스크(11-08 CC1), 표준 입력 스레드(11-08 CC7), 문장 뒤 놓기의 넓은 구성)이 모두 바이너리와 맞았다.
포매터는 저장소의 `.fung` 128개를 들여쓰기·공백을 뭉갠 뒤 돌려도 원본으로 돌아오고, 영속 리스트·맵은 큐·스택·뒤에서 붙이기 모양에서 선형이며, 동시성 런타임은 `timeout`+`with`+`scope`+`spawn`을 30만 번 돌려도 메모리가 늘지 않는다.
인자 타입 에러 60여 개 표본이 functions.md의 규칙(E0403/E0506)을 지킨다. 지난 12절의 의심 19개 가운데 11개는 이번 항목으로, 6개는 문제없음으로 판정했고 2개(TLS·EXDEV·raw 모드처럼 환경이 필요한 것)는 이번에도 확인하지 못했다(11절 첫 묶음).
남은 문제의 대부분은 구조가 아니라 **규칙이 닿지 않은 가장자리**다.

**가장 먼저: 취소와 E0607이 부딪친다(CC9, 높음).** 기한이나 `cancel(s)`이 읽는 쪽을 끊었을 때, 이미 `Err`로 끝나 있던 자식이 "읽지 않은 `Err`"(E0607)로 세어져 프로그램이 죽는다.
`timeout(() => conc.pmap(hosts, check), 5s)`처럼 문서가 권하는 쓰임이 걸리고, 자식이 기한 전에 끝났느냐에 따라 죽거나 살아 05의 "실패 여부는 실행 순서와 무관하다"가 깨진다.
같은 급으로 먼저 볼 것이 **VM14**다. 11-08 VM1 고침 뒤에도 실패한 `match` 갈래가 쓴 칸은 놓이지 않아, 같은 코드가 가드가 거짓인 길에 있기만 해도 교착한다.

**첫째 경향: 지난 고침이 한 경로만 막아 후속이 생겼다(항목의 약 30%).** 지난 점검의 1절이 "판정을 한 곳으로 모으라"고 권했는데, 모으지 않고 막은 자리에서 후속이 나왔다.

- 11-08 VM4 결정("값 둘 이상의 `Ok`는 보통 태그")은 `!`·`?`·패턴만 바꿨다. `is_ok`·보간·`join`·`conc.retry`·타입 표기는 이름만 본다. 판정 함수가 `result_kind`와 `result_tag` 둘이다(SP12).
- 11-08 VA15 고침은 타입 이름만 봐 `sum([nil])`·`min([[nil]])`이 통과한다(VA18). 11-08 VA9의 한도 검사는 기억된 글자 수를 쓰지 않아 `get`까지 O(n)이다(VA26). 11-08 VA8은 두 걸음을 맞췄지만 합치지 않아 `==`가 두 번 훑는다(VA22).
- 11-08 VM1은 문장 뒤 놓기를 넓혔지만 실패한 갈래의 쓰기를 추적하지 않는다(VM14). 11-08 FE3 고침은 `for` 변수를 놓쳤다(FE33). 11-08 FE22가 모은 "이름인가" 판정은 로더에 한 벌 남았다(FE30). 11-08 FE17은 원소 타입을 문자열로 다시 자른다(FE34).
- 11-08 LB2는 자격 증명 헤더를 버리지만 URL 속 비밀은 자동 `Referer`로 새 origin에 간다(LB24). 11-08 LB5가 정리한 "`0s` = 제한 없음"이 원시에 남았다(LB39). 11-08 LB11이 남긴 미결이 그대로다(LB38). 11-08 TL4의 규칙은 옵션·REPL 명령에 닿지 않는다(TL7).
- 문서 쪽 후속: 11-08 VM13 처리가 internals.md의 "인자 배열 재사용" 문장을 고치지 않았다(VM15). 11-08 LB9 뒤 매뉴얼 두 곳(LB40), 11-08 SP10 뒤 스펙·06·주석(CC13·SP22), 11-08 SP5는 문서만 구현에 맞추고 동작은 판단하지 않았다(SP13).

권고는 지난번과 같지만 더 구체적으로 적는다. 결과값 판정은 `result_kind` 하나, 문자열 한도는 `Text`의 기억된 글자 수 하나, "이름인가"는 렉서 하나, 프록시·리다이렉트의 경계는 `_leaves_origin` 하나로 모은다.
고친 뒤에는 같은 개념을 쓰는 다른 자리(fung으로 쓴 모듈, 매뉴얼, 설계 문서, 스펙, 코드 주석)를 `grep`으로 훑는 단계를 처리 줄에 남긴다.

**둘째: 조용한 무시·변환이 가장자리에 남았다(원칙 1).** 이번에도 가장 많은 모양이다.

- 언어: `nil?`만 결과값 규칙의 예외로 통과한다(SP13). 효과 없는 문장이 버려진다(줄 앞 `-`, 괄호 잊은 호출, `==` 오타, FE24). 이름 패턴이 바깥 변수와 "비교"되리라 기대한 `match` 절이 뒤 절을 죽인다(FE25).
  거꾸로 된 범위가 말없이 빈다(SP19). 0이 아닌 리터럴이 0으로 반올림된다(FE31). 음수 개수가 0으로 깎이거나 "제한 없음"이 된다(SP15). `time.format`이 토큰 없는 형식을 글 그대로 쓴다(SP25).
- 런타임·라이브러리: 읽기 에러가 입력의 끝이 된다(비차단 표준 입력은 빈 입력, CC10). `fs.exists`가 권한 실패를 `false`로 감춘다(SP16). close_notify 없는 TLS 끝이 온전한 본문이 된다(LB23).
  잘못 적은 프록시 변수가 무시되어 직접 연결한다(LB25). 받은 헤더 맵에 없는 헤더를 지어내고 있는 헤더를 지운다(LB31). `Set-Cookie`를 `", "`로 합쳐 되돌릴 수 없다(LB26). 쿼리의 반복 키·잘못된 쌍을 버린다(LB27).
  사용자 헤더의 대소문자 다른 같은 이름이 하나로 준다(LB34). 형식의 같은 필드가 두 번이면 마지막이 이긴다(LB36). 신호로 죽은 명령이 `code: -1`이다(LB28).

HTTP 쪽이 많다. `extras/http.fung`에 "받은 것을 친절하게 고쳐 주는" 코드가 여럿 있고, 서버 쪽은 11-08 LB3에서 "고치지 않고 거절한다"로 정했는데 받는 쪽은 그대로다.

**셋째: 바깥 실패와 코드 잘못의 경계(06 에러 규칙)가 흐리다.** 06은 "외부 요인 → `Ok`/`Err`, 코드가 틀림 → 런타임 에러"인데 반대로 간 자리가 있다.
파일·명령 출력이 문자열 한도를 넘으면 런타임 에러 E0806이다(LB29, `exec`는 다 읽은 뒤 판정해 메모리도 지키지 못한다). 사용자가 친 명령줄의 오타가 `cli.parse`에서 런타임 에러다(LB38). 권한 실패가 `bool`로 사라진다(SP16).
반대로 코드 잘못일 가능성이 큰 음수 개수는 말없이 받는다(SP15). SP15가 제안하는 "값이 없을 수 있는 정상 질문 → `nil`, 바깥 실패 → `Err`, 코드 잘못 → 런타임 에러" 한 절을 02·06에 적고, 그 표로 함수들을 한 번 훑기를 권한다.

**넷째: 같은 개념에 규칙이 둘이다(원칙 7).** `parse_int`·`parse_float`·`parse_dur`의 공백·부호·앞의 0(SP14), 맵 키 에러의 번호(SP21), 문자열을 받는 컬렉션 함수(SP23),
블록 안 대입이 최상위 `fn`에서만 허용(FE23), 패턴을 받는 자리마다 다른 모양(FE32), 괄호가 파이프와 슬라이스에서 투명(FE27·SP19), `split("", "")`(VA20), 범위의 `==`(VA21),
명령마다 다른 종료 코드·옵션 처리·UTF-8 규칙(TL6·TL7·TL8), 이름 표기(SP27). 하나하나는 작지만 매뉴얼에 "단, …"을 늘린다.

**다섯째: fung으로 쓴 라이브러리가 아직 "안쪽 코드"처럼 행동한다.** 11-08 VA7은 prelude만 고쳤다. `cli`·`conc`·`fs.with_open`은 타입 표기가 없어 사용자가 준 적 없는 인자 이름의 에러나 `cannot compare str and int`가 부작용 뒤에 난다(SP18).
호출 경로에 사용자가 열 수 없는 `extras/prelude.fung`의 줄과 `_list`·`<lambda>`가 보이고 `keys(1)`이 "pairs"를 탓한다(VA19). 원소 하나일 때 검사를 건너뛴다(VA18). `reverse`가 `sort`만큼 느리다(VA25).
"구현 언어는 사용자 코드에 영향이 없다"(open-questions)가 지켜지려면, extras의 공개 함수마다 틀린 인자 한 사례를 시험하는 표(SP18), 호출 경로에서 내장 프레임 감추기(VA19), 원소마다 람다를 부르는 컬렉션 함수의 Rust 이동(VA25)이 필요하다.

**여섯째: 규범의 자리를 옮긴 뒤의 빈틈.** 0.6.0에서 매뉴얼 언어 명세를 규범으로 정했는데(11-08 SP5·SP8), 설계 문서에만 있는 규칙이 많다.
import 끌어올림, 평가 순서, 부분 적용의 세부, 지역 `fn`, 태그의 일반 범주, BOM이 그렇다(SP17). 설계 문서는 아무도 지키지 않아 계속 낡는다(SP22: 06의 `fs.close`·`lines`·동시성 목록·`fsync` 모순, 03의 예, 08 표, `grammar.md`의 `params`).
스펙 문서(CC13)와 매뉴얼 VM 문서의 성능 문장(VM15·VM16)도 같다. SP17이 제안하는 "설계 문서의 규칙 문장 ↔ 매뉴얼" 대조 목록 시험을 결정으로 묻기를 권한다.

**일곱째: 성능은 여전히 호출 경로의 고정 비용이 크다.** 모두 `unsafe` 없이 된다.

- 내장 함수 호출마다 인자 `Vec`을 할당한다. 표본의 35%이고 내장 호출 비용의 대부분이다(VM15). 명령 해독이 크레이트 밖 함수 호출이고(표본 12%), 비꼬리 호출마다 인자를 한 칸 밀고, 명령마다 위치를 읽는다. 셋을 고치면 `fib`가 20% 빨라진다(VM16).
- 라이브러리 쪽은 알고리즘이다. 정렬된 입력에도 n log n(VA23), `==`의 두 번 훑기(VA22), fung으로 쓴 `reverse`·`keys`·`sum`(VA25), 큰 문자열의 글자 수 다시 세기(VA26), `swap` 안의 차가운 경로(CC11).
- 컴파일 시간의 이차 비용(FE36·FE37·VM18)은 정상 프로그램에서는 드물다.

**HTTP는 보안 쪽을 먼저 본다.** close_notify 없는 끝을 본문의 끝으로 받는다(LB23, 경로 위의 공격자가 TCP FIN으로 본문을 자를 수 있다). 리다이렉트의 `Referer`가 조각과 다른 origin으로 쿼리의 비밀값을 싣는다(LB24).
잘못 적은 프록시 변수가 프록시를 우회한다(LB25). 사용자 `Proxy-Authorization`이 https 터널에서 목적지 서버로 간다(LB30).

## 2. 추천 작업 순서

각 묶음 안의 항목은 서로 다른 파일을 건드리는 경우가 많아 병렬 세션으로 나눌 수 있다. 같은 파일을 건드리는 항목은 한 세션에 둔다.

0. **가장 먼저**: CC9(결정, 높음: 취소 중의 E0607. 추천안이 분명해 바로 묻고 고친다, `vm/scope.rs`·`vm/task.rs`), VM14(버그: 실패한 갈래의 칸, `compiler/m4.rs`). 둘은 파일이 겹치지 않는다.
1. **문서만 고치는 묶음**(결정 없음, 한 세션): SP17 SP20 SP22 CC13 LB40, VM15·VM16의 문서 문장(`manual/vm/internals.md` 145행, `08-bytecode.md` 73행과 "인자를 따로 복사하지 않는다").
   SP17의 "대조 목록 시험"과 SP22의 `fsync` 비용 문구는 결정으로 따로 묻는다.
2. **결정 없이 고칠 버그 묶음**. 다섯은 파일이 겹치지 않는다.
   - 앞단(`lexer`·`parser`·`resolve`·`loader`·`format`·`diag`): FE26 FE28 FE29 FE30 FE33 FE34 FE35.
   - 값·prelude와 fung 모듈(`value/`·`prelude/`·`extras/prelude.fung`·`extras/cli.fung`·`extras/conc.fung`·`extras/fs.fung`): VA18(`sort`의 결정 제외), SP18. SP18의 `pmap`의 `limit` 기본값은 SP15의 결정을 따르므로 SP15를 먼저 묻는다.
   - 동시성(`vm/stdin.rs`·`native/file.rs`·`conc.rs`·`vm/deadlock.rs`): CC10 CC12.
   - HTTP·TLS(`extras/http.fung`·`native/tls.rs`·`native/net.rs`): LB23 LB25 LB31 LB33 LB32 LB30, LB24의 버그 부분(조각), LB39.
   - 프로세스·시간(`native/process.rs`·`native/time.rs`·`native/timefmt.rs`): LB35 LB37.
3. **결정 묻기**. 영향이 큰 것부터.
   - 첫째(언어의 핵심 규칙): SP12(결과값은 값 하나: `Ok(1, 2)`를 컴파일 에러로 할지, 두 영역의 추천이 갈림), SP13(`nil?`), FE23(블록 안 대입), FE24(효과 없는 문장), FE25(도달할 수 없는 절), FE27(괄호, SP19와 함께).
   - 둘째(바깥 실패의 모양과 보안): SP16(`fs.exists`), LB29(바깥 값의 한도는 `Err`), LB24(`Referer`), LB28(신호 종료), CC10과 짝인 SP15(음수 개수와 nil·Err·에러의 기준).
   - 다음: SP14 SP21 SP23 FE31 FE32 FE38 VA19 VA20 VA21 LB26 LB27 LB34 LB36 LB38 TL6 TL8.
   - 나머지 결정 항목(SP24의 E0224·E0231, SP25 SP26 SP27의 이름 정리, VA18의 `sort`, VM15의 (가)/(나))은 한 번에 묶어 물어도 된다.
4. **성능**: VM15·VM16·VM17이 가장 크다(같은 `vm/call.rs`·`vm.rs`·`vm/link.rs`라 한 세션, VM17의 정리를 먼저 하면 VM16 (b)가 한 곳만 고친다). 그다음 VA25(VA18·VA19와 함께), VA23, VA22, VA26, VA24, CC11, FE36 FE37 VM18 FE39.
5. **정리**: VA27, SP24의 나머지(E0805 help, E0705 은퇴, E0506 동사), LB39. 해당 파일을 다른 일로 건드릴 때 함께 한다.

## 3. 목록

"지난 점검과의 관계"의 "후속"은 그 항목의 고침이 불완전하거나 새 문제를 만든 것, "관련"은 같은 규칙·코드를 건드리는 것, "12절 의심"은 지난 문서 12절의 의심을 확인한 것이다.

| ID | 한 줄 | 분류 | 심각도 | 지난 점검과의 관계 |
|---|---|---|---|---|
| SP12 | 값 둘 이상의 `Ok`/`Err`를 `is_ok`·보간·`retry`·타입 표기는 결과값으로 본다 | 버그·결정 | 중간 | VM4 후속 |
| SP13 | `nil?`만 결과값 규칙의 예외로 `nil`을 통과시킨다 | 결정 | 중간 | SP5 후속 |
| SP14 | `parse_int`·`parse_float`·`parse_dur`의 공백·부호·앞의 0 규칙이 서로 다르다 | 결정 | 중간 | FE1 관련 |
| SP15 | 개수·한도 인자의 음수·0과 빈 입력의 답이 함수마다 다르다 | 결정 | 중간 | 12절 의심 |
| SP16 | `fs.exists`가 권한·입출력 실패를 `false`로 감춘다 | 결정 | 중간 | 새 |
| SP17 | 규범인 매뉴얼 언어 명세에 설계 문서의 규칙 여럿이 없다 | 문서 | 중간 | SP5·SP8 후속 |
| SP18 | fung으로 쓴 모듈(`cli`·`conc`·`fs.with_open`)이 인자를 검사하지 않는다 | 버그 | 낮음 | VA7 후속 |
| SP19 | 거꾸로 된 범위가 말없이 비고, 범위 값으로는 슬라이스하지 못한다 | 결정 | 낮음 | 새(FE27 관련) |
| SP20 | 정규식의 `.`·문자 클래스가 UTF-8이 아닌 바이트에 맞지 않는데 문서는 "아무 글자" | 문서 | 낮음 | 12절 의심 |
| SP21 | 문자열이 아닌 맵 키가 자리마다 E0501과 E0514로 갈린다 | 결정·정리 | 낮음 | 새 |
| SP22 | 설계 문서·EBNF의 낡은 예와 표(03·06·08·`grammar.md`·CLAUDE.md) | 문서 | 낮음 | SP5·SP8·SP10·LB17 후속 |
| SP23 | `take`·`drop`은 문자열을 받는데 `first`·`last`·`reverse`는 E0506 | 결정 | 낮음 | 새 |
| SP24 | 에러 카탈로그의 정리거리(E0805, E0705/E0709, E0224/E0231, E0506 동사) | 정리 | 낮음 | FE4 관련 |
| SP25 | `time.format`이 토큰 없는 형식을 말없이 글 그대로 쓴다 | 결정 | 낮음 | LB20 관련 |
| SP26 | 심볼의 이름을 글로 얻는 길이 없다 | 결정 | 낮음 | 새 |
| SP27 | 이름·표기의 작은 어긋남(`0X`·`1E5`, 매개변수 `f`의 두 뜻, `valid_utf8` 등) | 정리 | 낮음 | 새 |
| FE23 | 최상위 `fn` 안의 블록에서 상수 이름에 대입하면 함수 전체에서 상수가 사라진다 | 결정 | 중간 | 새 |
| FE24 | 효과 없는 문장(줄 앞 `-`, 괄호 잊은 호출, 부분 적용, `==`)이 조용히 버려진다 | 결정 | 중간 | 12절 의심 |
| FE25 | 모두 맞는 절 뒤의 절이 조용히 죽는다; 절 본문의 람다 문장이 새 절이 된다 | 결정 | 중간 | 새 |
| FE26 | 가장 흔한 구문 에러의 진단이 일반 E0110뿐이다 | 정리 | 중간 | FE15 후속 |
| FE27 | 괄호가 트리에 남지 않아 파이프·슬라이스의 뜻이 괄호에 따라 갈린다 | 결정 | 중간 | 12절 의심 |
| FE28 | E0224가 패턴 대입에 "`this pattern`"을 코드처럼 쓰고 밑줄이 틀리다 | 버그 | 낮음 | 새 |
| FE29 | 모듈 읽기 실패가 모두 "cannot find", 경로 표시가 제각각, E0306 위치가 `(` | 버그 | 낮음 | 새 |
| FE30 | 파일 이름에서 온 모듈 이름이 예약어여도 받는다 | 버그 | 낮음 | FE22 후속 |
| FE31 | 숫자·기간 리터럴의 남은 비대칭과 0으로의 반올림 | 결정 | 낮음 | FE1·FE9 관련, 12절 의심 |
| FE32 | 패턴을 받는 자리(대입·`for`·매개변수·`match`)마다 받는 모양이 다르다 | 결정 | 낮음 | 새 |
| FE33 | 블록 밖 이름 진단이 묶는 방법마다 다르고 `for` 변수는 prelude로 돌아간다 | 버그 | 낮음 | FE3 후속 |
| FE34 | 원소 타입의 이름을 렉서와 다른 규칙으로 다시 자른다 | 버그 | 낮음 | FE17 후속 |
| FE35 | 포매터가 `true`·`false`·`nil`·`;;` 뒤의 `-`를 부호로 붙인다 | 버그 | 낮음 | 12절 의심 |
| FE36 | "비슷한 이름" 찾기가 에러 수 × 이름 수 | 성능 | 낮음 | FE16 후속 |
| FE37 | desugar가 함수마다 본문 전체를 훑어 중첩 함수가 O(n²) | 성능 | 낮음 | 12절 의심 |
| FE38 | 매개변수 기본값·`test` 블록의 `return`이 정해지지 않았다 | 결정 | 낮음 | 새 |
| FE39 | 매 실행의 불필요한 복사(모듈 트리, 토큰 문자열) | 정리 | 낮음 | 12절 의심 |
| VM14 | 실패한 `match` 갈래가 묶은 칸이 놓이지 않아 교착한다 | 버그 | 중간 | VM1 후속 |
| VM15 | 내장 함수 호출마다 인자 `Vec`을 할당한다(문서는 "재사용") | 성능·문서 | 중간 | VM5·VM13 후속 |
| VM16 | 명령 해독의 함수 호출, 비꼬리 호출의 칸 밀기, 명령마다 위치 읽기 | 성능 | 중간 | VM9 관련, 12절 의심 |
| VM17 | 호출 규약 코드의 중복과 쓰이지 않는 것, 닿지 않는 `RETURN` | 정리 | 낮음 | VM10 관련 |
| VM18 | 상수 접기가 긴 연산자 사슬에서 O(n²) | 성능 | 낮음 | 새 |
| VA18 | 원소가 하나면 `sum`·`min`·`max`가 타입 검사를 건너뛴다(`sum([nil])`이 `nil`) | 버그 | 중간 | VA15·VA7 후속 |
| VA19 | fung으로 쓴 prelude의 진단이 구현을 드러낸다(내부 프레임, `keys`가 "pairs") | 결정·정리 | 낮음 | 새 |
| VA20 | `split("", "")`가 `[]`라서 "언제나 원소가 하나 이상"과 어긋난다 | 결정·문서 | 낮음 | 새 |
| VA21 | 범위의 `==`가 표현을 비교한다(`0..5 != 0..=4`) | 결정 | 낮음 | 새 |
| VA22 | `==`가 컬렉션을 두 번 훑는다 | 성능 | 낮음 | VA8 후속 |
| VA23 | `sort`가 이미 정렬된 입력에서도 n log n | 성능 | 낮음 | 새 |
| VA24 | `sort_by`의 원소 벡터 복사, `unique`의 두 벌 | 성능 | 낮음 | 12절 의심 |
| VA25 | fung으로 쓴 `reverse`·`keys`·`values`·`sum`·`max`가 몇 배 느리다 | 성능 | 낮음 | 새 |
| VA26 | 1억 바이트 넘는 비ASCII 문자열은 반환마다 글자 수를 다시 센다 | 성능 | 낮음 | VA9 후속 |
| VA27 | 쓰이지 않는 `nat::str_args`, 두 곳에서 정의되는 `args` | 정리 | 낮음 | 새 |
| CC9 | 취소·기한이 읽는 쪽을 끊으면 이미 끝난 자식의 `Err`가 E0607이 된다 | 결정 | 높음 | 새 |
| CC10 | 표준 입력·파일 핸들의 읽기 에러가 "입력의 끝"이 된다 | 버그 | 중간 | 새 |
| CC11 | `swap` 함수 안의 차례: 05와 VM 문서가 어긋나고 호출마다 `end_turn` | 성능 | 낮음 | 새 |
| CC12 | `wait_closed`의 교착 보고에 "자기 보내는 끝" 도움말이 없다 | 버그 | 낮음 | L3·L7 후속 |
| CC13 | 스펙의 `timeout` 인자 순서, `channel.rs`·`stdin.rs` 머리 주석 | 문서 | 낮음 | SP10 후속 |
| LB23 | close_notify 없이 끊긴 TLS의 "닫힘까지" 본문을 온전한 응답으로 받는다 | 버그(보안) | 중간 | 새 |
| LB24 | 리다이렉트의 `Referer`가 조각과, 다른 origin에도 쿼리 전체를 싣는다 | 버그·결정(보안) | 중간 | LB2 후속 |
| LB25 | 잘못된 프록시 환경 변수가 무시되어 직접 연결하거나 엉뚱한 호스트가 된다 | 버그 | 중간 | L13 관련 |
| LB26 | 같은 이름 헤더를 `", "`로 이어 `Set-Cookie`·`Cookie`가 깨진다 | 결정 | 중간 | 새 |
| LB27 | `req.query`가 반복 키·잘못된 쌍을 버리고 원래 대상이 핸들러에 없다 | 결정 | 중간 | LB22 관련 |
| LB28 | 신호로 끝난 명령이 `code: -1`, "exited with code -1" | 결정 | 중간 | 새 |
| LB29 | 바깥 값이 문자열 한도를 넘으면 `Err`가 아니라 E0806, `exec`는 다 읽은 뒤 판정 | 결정 | 중간 | 새 |
| LB30 | 사용자 `Proxy-Authorization`이 두 번, 또는 터널에서 목적지로 간다 | 버그(보안) | 낮음 | LB2 관련 |
| LB31 | 받은 헤더 맵에 `cache-control`을 지어내고 `Connection`을 지운다 | 버그·정리 | 낮음 | LB3 관련 |
| LB32 | absolute-form 요청 대상의 호스트를 무시하고, 빈 경로가 `""` | 버그 | 낮음 | LB4 관련 |
| LB33 | 클라이언트가 HTTP/1.0 응답의 `Transfer-Encoding`을 지우고 읽는다 | 버그 | 낮음 | 12절 의심, LB13 관련 |
| LB34 | 클라이언트가 사용자 헤더·메서드를 말없이 고친다 | 결정 | 낮음 | LB3 관련 |
| LB35 | `exec`의 `cwd`: 문구가 디렉터리를 말하지 않고, `PWD`·`cwd: ""` | 버그 | 낮음 | 새 |
| LB36 | `time.parse` 형식의 같은 필드 두 번, `ddd`를 날짜와 대조하지 않음 | 결정 | 낮음 | LB20 관련 |
| LB37 | 지역 시간대로 보면 0000~9999년 밖이 찍히고 다시 읽지 못한다 | 버그 | 낮음 | 새 |
| LB38 | `cli.parse`의 남은 구멍(스위치의 `=값`, 반복 플래그, 잘못된 `bools`) | 결정 | 낮음 | LB11 후속 |
| LB39 | `_native.net.set_deadline(c, 0s)`가 "기한 없음" | 정리 | 낮음 | LB5 후속 |
| LB40 | LB9 뒤 매뉴얼 두 곳이 아직 `'invalid`를 말한다 | 문서 | 낮음 | LB9 후속 |
| TL6 | "스크립트를 읽지 못함"의 종료 코드가 명령마다 3·2·1 | 결정 | 낮음 | 새 |
| TL7 | 옵션·남는 인자 처리가 명령마다 다르다 | 정리 | 낮음 | TL4 후속 |
| TL8 | `fung fmt`가 UTF-8이 아닌 소스를 받아 통과시킨다 | 결정 | 낮음 | L4 관련 |

## 4. 언어 명세 (SP)

### SP12. 값 둘 이상의 `Ok`/`Err`를 결과값으로 보는 자리가 남았다(11-08 VM4 후속) — 버그·결정, 중간 (값 영역의 같은 항목을 합침)
- 재현:
  ```
  $ cat t.fung
  print(is_ok(Ok(1, 2)))
  Err(1, 2)                      # 버려지는 자리: E0606이 아니다(맞음)
  print("${Err(1, 2)}")
  ```
  실제: `true`, 그다음 `error[E0519]: an `Err` value in string interpolation: a result is not put into text silently`.
  `join([Ok(1, 2)])`도 E0519. `is_err(Err(1, 2))`도 `true`. `conc.retry(() => Err(1, 2), times: 3)`은 `Err(1, 2)`를 `Err`로 보고 세 번 다시 부른다.
  `fn load() -> Ok | Err = Ok` → `print(load())`가 `'Ok`(인자 없는 심볼이 반환 타입 `Ok`를 통과).
  `fn f(r: Ok) = r; f(Ok(1, 2))`도 통과한다.
  반면 연산자는 결과값으로 보지 않는다:
  ```
  $F -e 'x = Ok(1, 2)
  print(x!)'                                           → E0605 `!` on an Ok with 2 values, which is not a result
  $F -e 'f = () => Err(1, 2)
  f()
  print("after")'                                      → after   (버려지는 Err E0606이 아님: 결과값이 아니므로)
  $F -e 'print(1 |> Ok(2))'                            → Ok(1, 2)
  ```
  기대: VM4 결정대로 `Ok(1, 2)`는 "같은 이름의 보통 태그"이므로 `is_ok`는 `false`, 보간·`join`은 보통 태그처럼 글로 넣고(또는 다른 규칙), `retry`는 돌려준다.
  `!`·`?`·`try_map`(E0605), 버려지는 자리(E0606 아님), 패턴 `Ok(x)`는 이미 그렇게 한다. 같은 값을 연산마다 다르게 본다.
- 근거: `manual/reference/language/results.md` "`Ok`·`Err`는 값을 하나 담을 때만 결과값이다. `Ok(1, 2)`는 같은 이름의 보통 태그", 02 "결과값은 값 하나를 담는다(VM4, 사용자 결정): `Ok(v)`·`Err(e)`만 결과값이고, `Ok(1, 2)`·`Err(1, 2)`는 같은 이름의 보통 태그 값이다".
  `manual/reference/prelude/results.md`의 `is_ok`("`Ok(…)`이면 true") 예제 `filter(rs, is_ok) |> map(r => r!)`는 `Ok(1, 2)`가 섞이면 E0605로 깨진다.
  VM4 고침이 `!`·`?`·`??`·`try_map`·E0606만 바꾸고, 결과값을 묻는 다른 세 곳(`is_ok`·`is_err`·`value::result_tag`)은 이름만 본다.
  게다가 `Ok()`는 컴파일 에러(E0117)인데 `Ok(1, 2)`는 받아, "결과값 이름을 단 보통 태그"라는 특수한 값이 언어에 남아 있다.
- 원인 위치: 판정 함수가 둘이다. 올바른 쪽 `crates/fung-vm/src/vm/m4.rs:26` `result_kind`(인자 1개만), 낡은 쪽 `crates/fung-vm/src/value/format.rs:197` `result_tag`(이름만 봄; 호출 `vm.rs:965` 보간, `prelude/strings.rs:245` `join`).
  `extras/prelude.fung:140-141` `is_ok`·`is_err`가 `type_of(x) == 'Ok`. `extras/conc.fung:62` `retry`는 `is_err`를 쓴다. 타입 표기 `Ok`는 `crates/fung-vm/src/value/types.rs:116` `matches_type`의 T1 규칙(태그 이름은 같은 이름의 심볼도 받음).
- 고치는 법(결정). 두 영역의 추천이 갈렸다.
  - (가) **값 영역의 추천**: `Ok`·`Err`는 인자가 정확히 하나다. 식·패턴·파이프(`x |> Ok(2)`)에서 인자가 둘 이상이면 컴파일 에러(E0117을 "`Ok`/`Err` takes exactly one value"로 넓히거나 새 번호).
    그러면 VM4의 "보통 태그 `Ok`"라는 특수한 경우와 그 설명(02 한 단락, E0605 문구의 "with 2 values")이 사라지고, `is_ok`·`result_tag`는 지금 코드 그대로 맞다. 지침 "예외·특수한 경우 최소화"와 맞는다.
  - (나) **명세 영역의 추천**: 지금 규칙을 지킨다. `result_tag`를 `result_kind`와 같은 판정(인자 1개)으로 바꾸거나 하나로 합쳐 결과값 판정이 한 곳에 모이게 한다.
    `is_ok`·`is_err`는 `match x: Ok(_) => true; _ => false ;;` 꼴(또는 네이티브)로(`is_ok(Ok(1, 2))`는 false, `"${Ok(1, 2)}"`는 된다). 문서의 "`type_of(x) == 'Ok`와 같다"를 고친다.
  - 타입 표기는 (가)에서도 남는 결정이다: (a) 이름이 `Ok`·`Err`인 표기만 "인자 1개 태그"로 좁힘(추천: 결과값 규칙 하나가 연산·패턴·타입에 같게), (b) T1 그대로 두고 문서에 "`-> Ok | Err`는 심볼 `'Ok`도 받는다"를 적음.
- 처리(컴파일 쪽): 이 커밋, 새 번호 E0236(`` `%s` takes exactly one value, found %d ``; E0117은 "빈 괄호"라 뜻을 넓히지 않음): 식·파이프는 `desugar`, 패턴은 파서(구조 분해 대입 시도에서도 E0236을 그대로 냄). 타입 표기 `Ok`·`Err`는 `value/types.rs` `TypeSet::matches`에서 값 하나 태그만(심볼 `'Ok` 거절). 소스로는 개수가 틀린 `Ok`/`Err`를 만들 길이 없어졌다(태그 인자에 펼치기가 없고 심볼은 부를 수 없음): E0605의 "with N values" 갈래와 `result_tag`의 차이는 닿지 않는 코드로 W6(Task 7)에서 정리. 매뉴얼 `results.md`·`types.md`·`errors.md`(E0236), 설계 02, 결정 D2
- 처리(런타임 쪽): 이 커밋, 판정은 `vm/m4.rs` `result_kind` 하나(`value::result_tag` 삭제, 보간·`join`은 `ResultKind::result_name`, `try_map`도 `result_kind`). 이름이 `Ok`·`Err`인 태그는 늘 값이 하나라(컴파일러가 E0236으로, 런타임은 값 하나로만 만듦; `Op::Tag`에 debug_assert) 이름으로 판정한다. 런타임에 개수가 틀린 `Ok`/`Err`를 만드는 길은 찾지 못했다(펼치기·심볼 호출·`map(xs, Ok)` 모두 막힘, 네이티브는 값 하나): 새 런타임 에러 번호는 두지 않음. E0605의 `an Ok with N values` 갈래, `err_message`의 여러 값 갈래, `TypeSet::matches`의 개수 검사 삭제. `is_ok`·`is_err`(`type_of`)·`conc.retry`는 이 불변식으로 같은 판정이라 그대로. 설계 02(VM4 단락 대체), 매뉴얼 `language/results.md`·`errors.md`(E0605), 결정 D2

### SP13. `nil?`만 결과값 규칙의 예외로 `nil`을 통과시킨다(SP5(2026-11-08) 후속) — 결정, 중간
- 재현:
  ```
  fn f() = env("NOPE_X")?
  fn g() = find([1], x => x > 3)?
  print(f(), g())     # nil nil
  fn h() = 5?
  print(h())          # error[E0605]: `?` on a value of type int, which is not a result
  ```
  `nil!`은 E0604다. 결과값을 돌려주지 않는 함수(`first`·`find`·`env`·`regex.find`·`get`)에 `?`를 붙인 실수가 `nil`일 때만 조용히 흘러간다.
- 근거: 규범 매뉴얼끼리 모순이다. `manual/reference/language/values.md` 표: nil은 "`==`, `??`, 출력, 컬렉션에 담기 외의 연산은 에러", `results.md` 표: `nil`에 `x?`는 "`nil` 그대로".
  02 "에러 처리와 축약 연산자": "`!`·`?`를 결과값이 아닌 값에 쓰는 것은 거의 확실히 버그라 에러로 한다"와, 같은 절의 이유 "nil 반환이 조용히 호출자로 퍼지는 동작은 엄격함 원칙에 어긋난다"는 오히려 `nil?`을 에러로 하자는 논리다.
  2026-11-08 SP5는 문서를 구현에 맞췄을 뿐 동작의 옳고 그름은 판단하지 않았다.
- 원인 위치: `crates/fung-vm/src/vm/m4.rs:26` `result_kind`의 `ResultKind::Nil`을 `?`가 통과시키는 갈래(같은 파일의 `?` 처리).
- 고치는 법: 결정. (a) **추천**: `nil?`은 `nil!`과 같이 E0604(규칙 "`!`·`?`는 결과값만 받는다" 하나, 표의 nil 행이 `!`과 같아짐). "없으면 위로 넘긴다"가 필요하면 `match`나 `if x == nil: return nil ;;`로 드러내 쓴다. CHANGELOG 호환 깨짐. (b) 지금대로 두고 values.md의 nil 목록에 `?`를 더하고, 02의 이유 문장을 고친다.
- 처리: 이 커밋, `vm/m4.rs` `UnwrapTest`의 nil 갈래를 E0604(`` `?` on nil ``)로, `try_map`의 `f`가 돌려준 nil도 E0604. 시험 `tests/lang/err_question_nil.fung`. 설계 02(표·nil 절·이유 문장 삭제), 매뉴얼 `language/results.md`(표·경계 사례)·`language/values.md`·`errors.md`(E0604·E0605)·`prelude/collections.md`(`try_map`), CHANGELOG, 결정 D3

### SP14. 바깥 글을 읽는 `parse_int`·`parse_float`·`parse_dur`의 문법이 서로 다르고 "리터럴과 같다"는 문서와도 다르다 — 결정, 중간 (값 영역의 `parse_dur` 공백 항목을 합침)
- 재현:
  ```
  print(parse_dur(" 1s "), parse_int(" 1"), parse_float(" 1.0"))
  # Ok(1s) Err({kind: 'parse, message: "not an integer:  1"}) Err({kind: 'parse, message: "not a number:  1.0"})
  print(parse_int("1\n"), parse_dur("1s\n"))   # Err(…"not an integer: 1\n") Ok(1s)
  print(parse_int("01"), int("007"), parse_float("01.5"), parse_dur("01s"))
  # Ok(1) 7 Ok(1.5) Err({kind: 'parse, message: "not a duration: 01s"})
  print(parse_int("+1"), parse_float("+1.5"), parse_dur("+1s"))   # Ok(1) Ok(1.5) Err(…)
  ```
  ```
  $F -e 'print(parse_dur("\n5s\t"), parse_dur("\u{a0}5s"))'      → Ok(5s) Ok(5s)      (유니코드 공백도 받음)
  $F -e 'print(int(" 5"))'                                       → E0513
  ```
  | | 앞뒤 공백 | 앞의 `+` | 앞의 0 |
  |---|---|---|---|
  | `parse_int`·`int` | 거절 | 받음 | 받음 |
  | `parse_float`·`float` | 거절 | 받음 | 받음 |
  | `parse_dur` | **받음** | 거절 | **거절** |
  | 소스 리터럴 | — | 없음(단항 `-`만) | 거절(E0102, FE1) |

  `exec`·`fs.read`의 출력은 끝에 줄바꿈이 붙으므로 `parse_int(out)`은 늘 실패하는데 `parse_dur(out)`은 된다. 에러 문구가 입력을 따옴표 없이 붙여 `"not an integer:  1"`의 공백이 보이지 않는다.
- 근거: `manual/reference/prelude/convert.md` `int`: "정수 리터럴의 십진 문법을 받는다. … 앞의 0(`"007"`)이 된다" — 리터럴은 FE1(2026-11-08)부터 앞의 0이 E0102라 한 문장이 스스로 모순이다. 06 "에러 규칙": "문자열의 문법은 리터럴과 같다(01, 앞의 부호 하나 허용)".
  `convert.md`의 세 `parse_*`는 한 묶음으로 소개되는데("틀릴 수 있는 입력을 검사할 때"), `parse_int`·`parse_float`·`int`·`float`은 "앞뒤 공백 … 받지 않는다 … 먼저 `trim`한다"고, `parse_dur`만 "앞뒤 공백은 허용한다"고 적었다(그래서 셋이 다른 것은 문서로도 사실이다). 원칙 7, 지침 "예외 최소화". 원칙 1(조용한 변환)로 보면 엄격한 쪽이 기본이다.
  FE1의 이유("`0755`를 10진수로 읽으면 `perm: 0444` 같은 권한이 말없이 틀린다")는 설정 파일에서 읽은 `"0755"`에도 그대로 해당한다.
- 원인 위치: `crates/fung-vm/src/prelude/convert.rs:50` `parse_int`(Rust `i64::from_str`가 `+`와 앞의 0을 받음), `:61` `parse_float`, `:191` `parse_dur_fn`(194행 `trim_matches(char::is_whitespace)`).
- 고치는 법: 결정. 세 함수가 같은 표 하나를 따르게 한다.
  - (a) **추천**(두 영역 같음): "리터럴의 문법 + 앞의 부호 `-`/`+` 하나 + 앞뒤 공백 거절" 하나로: `parse_int("01")`·`int("007")`은 E0513/`Err`(FE1과 같은 이유; 도움말 `0o`), `parse_dur`도 공백 거절(`trim_matches` 삭제)·`+` 받음.
    문서는 "앞뒤 공백이 있을 수 있으면 먼저 `trim`" 한 줄(이미 `int`에 있음). 에러 문구는 입력을 따옴표로(`not an integer: " 1"`). `cli` 모듈 등 내부 호출자가 `parse_dur`의 trim에 기대는지 확인한다.
  - (b) 셋 모두 앞뒤 공백을 받음(명령 출력 편의). 그 경우도 `+`·앞의 0은 하나로 맞춘다. `int`/`float`은 엄격하게 둘지도 정해야 해서 덜 추천(단언형과 검사형의 문법이 갈라진다).
- 처리: 이 커밋. `prelude/convert.rs` `literal`이 렉서로 숫자 토큰 하나를 읽는다(앞의 부호 `-`·`+` 하나, ASCII, 숫자로 시작, 토큰이 글 전체): `parse_int`·`parse_float`·`parse_dur`·`int`·`float`이 같은 규칙. 앞의 0은 렉서의 도움말(`0o`)을 E0513·`Err` 메시지에, 메시지는 입력을 따옴표로. 실수 갈래는 10진 정수 꼴만 받음(`"0x10"`은 실수가 아님). 렉서와 같은 판정이라 0으로 반올림되는 실수(D9)·대문자 접두사(D32)도 함께 거절. `parse_dur`의 `trim_matches` 삭제. 내부 호출자: `extras/http.fung`이 포트·`Content-Length`·IPv4 조각·접두 길이·상태 코드를 `parse_int`로 읽어 앞의 0(`:0080`)에서 깨지므로 숫자만 받는 `_wire_int`(앞의 0을 떼고 읽음)로 바꿈. `cli`는 숫자를 읽지 않음. 매뉴얼 `convert.md`, 06 에러 규칙·`parse_dur`, CHANGELOG, 결정 D12.

### SP15. 개수·한도 인자의 음수·0과 빈 입력의 답이 함수마다 다르다(12절 의심의 판단) — 결정, 중간 (동시성 영역의 `pmap`·`retry`·`timeout` 음수 지적을 합침)
- 재현:
  ```
  print(repeat("a", -1), take([1], -1), drop([1], -5), pad_left("a", -3), iterate(1, x => x + 1, -3))
  #  [] [1] a 1                     (모두 말없이)
  conc.pmap([1], x => x, limit: -1)              # [1] (말없이 "제한 없음")
  conc.retry(() => Err(1), times: -1)            # Err(1) (한 번)
  conc.retry(() => Err(1), times: 2, delay: -1s) # 받아들임
  sleep(-1s)                                     # 말없이 0
  timeout(() => 1, -1s)                          # Err({kind: 'timeout, message: "timed out after 0s"}) (쓴 값과 다른 문구)
  channel(-1)       # error[E0403]: argument `n` must be a non-negative int, got -1
  exit(-1)          # error[E0403]: argument `code` must be an int from 0 to 255, got -1
  json.stringify([1], indent: -1)                # E0403
  http.get(url, max_body: -1)                    # E0403
  durs = []; print(sum(durs) / 1s)               # error[E0501]: cannot divide int and dur
  ```
  같은 "음수 개수"가 어떤 함수는 E0403, 어떤 함수는 0으로 깎기, `pmap`은 `0`과 음수가 "제한 없음"이라는 표식이다. 매뉴얼 `modules/conc.md`는 "`0`이면 제한 없음"만 말하고 음수는 말하지 않는다. `retry`의 `times: 0`·`-3`이 한 번 부르는 것은 매뉴얼에 적혀 있다.
  빈 입력의 답도 `sum([]) == 0`(원소 타입과 무관한 int), `min([])`·`first([])` = `nil`, `join([]) == ""`로 갈린다.
- 근거: 12절 의심. 판단: `first([])`·`max([])`·`find`·`env("X")`의 `nil`은 원칙 1과 맞는다(nil은 연산에 쓰면 곧 에러이고 `??`로 다룬다; "없음"은 실패가 아님). 다만 이 기준("값이 없을 수 있는 정상 질문 → nil, 바깥 실패 → Err, 코드 잘못 → 런타임 에러")이 02·06 어디에도 적혀 있지 않다.
  음수 개수는 다르다: `take(xs, n - k)`의 음수는 계산 실수일 가능성이 높고, `take`·`drop`이 "느슨한 길"인 이유는 **길이보다 큰** n이다(access.md "있는 만큼만"). `channel`·`exit`·`max_body`는 이미 E0403이라 원칙 7(같은 개념, 같은 규칙)과 어긋난다. `pmap`의 `limit = 0`은 "없음"을 뜻하는 숫자 표식이다(fung에는 그 일을 하는 `nil`이 있다; `http.*`의 `timeout: nil`처럼).
  `timeout` 문구: 05:160·매뉴얼은 시간 초과 문구가 "`timeout`에 준 기간"이라 한다(`-1s`를 준 사람에게 `0s`). background.md 원칙(엄격함, 예외 최소화), 06의 인자 에러 규칙(E0403).
- 원인 위치: `extras/prelude.fung:99` `_clamp`(take·drop), `crates/fung-vm/src/prelude/strings.rs:483` `repeat_fn`, `extras/prelude.fung:135` `_padding`, `:149` `iterate`, `extras/conc.fung:35` `pmap`(`limit <= 0`, 36~40), `:62` `retry`(64~67), `sleep`·`timeout`의 기간 검사(`crates/fung-vm/src/conc.rs`), `crates/fung-vm/src/vm/deadline.rs` `Timeout::end`(100행, `Value::Dur(self.d.max(0))`: 문구의 기간).
- 고치는 법: 결정.
  - (a) **추천**: "개수·한도는 음이 아닌 정수, 아니면 E0403(`must be a non-negative int`)" 하나로: `take`·`drop`·`repeat`·`pad_left/right`·`iterate`·`retry times`(1 이상; "`times` ≤ 1이면 한 번"이라는 특례를 매뉴얼에서 지움)·`pmap limit`. `pmap`의 "제한 없음"은 `limit = nil`(기본). `take`·`drop`의 위쪽 깎기는 그대로.
    기간: `sleep`·`timeout`·`retry delay`의 음수 기간도 E0403, 또는 `sleep`처럼 "기다리지 않음"으로 두되 `timeout` 문구는 준 값 그대로(`timed out after -1s`) 찍는다(동시성 영역은 `delay` 음수를 "기다리지 않음"으로 둬도 된다고 봤다).
    `sum`은 빈 입력에 `0`을 두고 문서에 "빈 리스트의 합은 정수 0; 기간이면 `fold(xs, 0s, …)`"(또는 `sum(xs, zero = 0)`). 02에 "nil·Err·에러를 고르는 기준" 한 절을 더한다.
  - (b) 지금대로 두고 함수마다 음수의 뜻을 문서에 적는다(`pmap`의 음수는 지금 문서에 없다).
  - `pmap`·`retry`의 타입 표기와 E0403은 SP18과 한 번에 고친다(`limit`의 기본값 `0`/`nil`이 이 결정을 따른다).
- 처리(prelude 쪽, Task 9): 이 커밋. `nat::need_count`·`nat::need_dur`(E0403 `must be a non-negative int`/`dur`)를 `take`·`drop`·`pad_left`·`pad_right`·`iterate`(`_r.count`), `repeat`, `sleep`, `timeout`, `_native.net.set_deadline`, `http`의 `timeout`·`drain`(새 `_net.need_dur`)이 씀. `timeout` 문구는 준 기간 그대로(`Timeout::end`의 `max(0)` 삭제). 저장소의 `.fung`에 음수가 될 수 있는 남은 시간 계산은 없었음(`http.fung`의 기간은 사용자 값과 상수, `_backoff`는 양수). `sum([])`은 정수 0으로 두고 `collections.md`에 `fold(xs, 0s, …)`. 02에 "nil·Err·런타임 에러를 고르는 기준" 절. `conc.pmap limit`·`conc.retry times`·`delay`의 인자 검사는 Task 11(W8). 매뉴얼 `collections.md`·`strings.md`·`loops.md`·`concurrency.md`·`http.md`, CHANGELOG, 결정 D11.
- 처리(`conc` 쪽, Task 11): 이 커밋. `pmap`의 `limit` 기본값 `nil`(제한 없음), 주면 `_arg.positive`(0·음수는 ``E0403 argument `limit` must be a positive int``); `retry`의 `times`도 1 이상("1 이하면 한 번" 특례 삭제), `delay`는 `_arg.dur`로 `f`를 부르기 전에 검사(전에는 첫 실패 뒤 `sleep`의 `d`를 말함). 매뉴얼 `conc.md`, 05·06, CHANGELOG, 결정 D11.

### SP16. `fs.exists`가 권한·입출력 실패를 `false`로 감춘다 — 결정, 중간
- 재현:
  ```
  $ mkdir -p locked/inner && touch locked/inner/x && chmod 000 locked
  import fs
  print(fs.exists("locked/inner/x"), fs.stat("locked/inner/x"))
  # false Err({kind: 'permission, message: "cannot stat `locked/inner/x`: permission denied"})
  ```
  파일은 있는데 `false`다. `if not fs.exists(p): fs.write(p, init)!` 같은 SRE 스크립트의 흔한 줄이 권한 문제를 "없음"으로 읽는다. 끊어진 심볼릭 링크도 `false`(링크는 있음; 코드로만 봄, 12절 참고).
- 근거: background 원칙 1(조용히 넘어가는 동작은 에러), 06 "에러 규칙": "외부 요인으로 실패할 수 있는 것 → `Ok`/`Err`". `manual/reference/modules/fs.md` `fs.exists`는 이유가 필요하면 `stat`을 쓰라고 하지만(표준 라이브러리 영역은 이 동작을 "문서대로"로 보았다), 대부분의 호출자는 이유를 알기 전에 이미 틀린 답을 받는다.
- 원인 위치: `crates/fung-vm/src/native/fs.rs:558` `exists_fn`(`metadata(...).is_ok()`).
- 고치는 법: 결정. (a) **추천**: "없음"(`ENOENT`·`ENOTDIR`)만 `false`, 그 밖의 실패는 `Err` — 반환을 `Ok(bool)`로(호출은 `fs.exists(p)!`; 06 규칙 그대로). (b) `bool`을 지키고 판정할 수 없으면 런타임 에러(코드 오류가 아니라 바깥 실패라 원칙과 덜 맞음). (c) 지금대로 두고 문서에 "권한이 없어 볼 수 없는 경로도 `false`"를 굵게. 링크는 `lstat` 기준인지도 함께 정한다.
- 처리: 이 커밋. `exists_fn`이 `Ok(bool)`: `ENOENT`·`ENOTDIR`만 `Ok(false)`, 그 밖은 `fs_err("stat", …)`(``cannot stat `p`: permission denied``). 링크는 따라감(`stat`과 같은 기준, 끊어진 링크는 `Ok(false)`)을 문서에 적음. 저장소의 호출(`tests/lang`·`bench`·매뉴얼 펜스·시험 문자열·설계 문서 01·03)에 `!`. 매뉴얼 `fs.md`, 06, CHANGELOG, 결정 D21.

### SP17. 규범인 매뉴얼 언어 명세에 설계 문서의 규칙 여럿이 없다 — 문서, 중간 (앞단 영역의 렉서·import 규칙 항목을 합침)
- 재현(동작은 있는데 `manual/reference/language/`가 말하지 않는 것):
  ```
  # 모듈: import는 끌어올려진다
  $ cat c.fung   →  _x = print("init c")
  $ cat h.fung   →  print("start") / import "./c" / print(c.g())
  $ fung h.fung  →  init c / start / 1
  import "./b" (f); print(b.f())          # error[E0204]: undefined name `b` (목록만 쓰면 모듈 이름은 안 묶임)
  import "./b" as c (f)                    # c와 f 둘 다 묶임
  import json; fn g(): json = 1 …         # error[E0308]: cannot shadow module `json` (매개변수 `fn f(path)`도)
  fn f(): import json …                    # error[E0311]
  # 평가 순서·부분 적용
  g = f(print("A"), _); print("B")         # A B  (`_` 옆 인자는 그 자리에서 한 번, 왼쪽부터)
  map([[1, 2]], len(_))                    # [2]  (`_`는 가장 안쪽 호출)
  # 렉서
  ǅx = 1; Ⅷ = 1                            # 둘 다 이름으로 받는다(태그 판정은 일반 범주 Lu뿐; ǅ는 Lt, Ⅷ는 Nl·대문자 속성)
  printf '\xef\xbb\xbfprint(1)' > bom.fung; $F bom.fung   # 1 (맨 앞 BOM은 건너뜀)
  ```
  설계에만 있는 규칙: 04 "import 문은 끌어올려진다", "목록만 쓰면 모듈 이름은 묶이지 않는다", "`as`와 목록은 둘 다"(04 27행), 04 "이름 가리기"(모듈 이름 가리기 E0308), E0311, 비공개 이름의 정적/동적 에러; 03 "평가 순서"(왼쪽→오른쪽, 단락 평가 목록);
  03 "부분 적용"의 `_` 여럿의 순서, 가장 안쪽 호출, 이름 인자 자리 금지(E0123); 03 "재귀와 이름 찾기"(최상위 `fn` 이름은 상수 E0209·E0210, 지역 `fn`은 자기만 부름 E0208, 지역 `fn`은 바깥 인자를 복사); 03 "람다도 이름 인자를 받는다";
  01 "람다 본문은 가능한 한 길게(`then`보다 낮게)"; 01 "`when` 조건의 맨 바깥 `이름 =>`는 절의 화살표"; FE25의 절 규칙; 01 94행 "태그는 Lu로 시작"(lexical.md는 "소문자, `_`, 또는 대소문자가 없는 글자로 시작하면 이름"이라 Lt·Nl 둘 다 어긋남); 08의 BOM.
  `map[T]`의 `T`가 키인지 값인지 `types.md`가 말하지 않는다(런타임은 검사하지 않으니 표기의 뜻만).
  `grep -rn '왼쪽에서\|평가 순서\|가장 안쪽\|끌어올\|E0308\|E0311\|E0208' manual/reference/language` → 기본값의 "왼쪽부터" 한 줄뿐.
- 근거: `docs/design/README.md` "규범은 매뉴얼의 언어 명세다 … 설계 문서들은 결정과 그 이유의 기록"(SP5·SP8(2026-11-08)로 매뉴얼 언어 명세가 규범이 됨). 규범 쪽에 없는 규칙은 "명세가 비어 있는 곳"이 되고, 다음 정리에서 설계 문서를 낡았다고 지울 위험이 있다.
- 원인 위치: `manual/reference/language/modules.md`·`functions.md`·`scope.md`·`match.md`·`operators.md`·`lexical.md`("이름과 태그"·"주석")·`types.md`·`grammar.md`.
- 고치는 법: 각 쪽에 규칙 한 줄과 `fung run`/`check` 경계 사례 펜스(생성기가 지킨다): modules.md(끌어올림·목록 import의 묶음·`as`와 목록·가리기 E0308·E0311, `grammar.md`에도), functions.md(평가 순서·부분 적용 세부·E0123·람다 이름 인자·람다 본문의 범위),
  scope.md(지역 `fn` E0208, 최상위 `fn` 상수 E0209·E0210), match.md(`when`의 화살표, 절 시작 규칙), lexical.md(태그는 "일반 범주 Lu(대문자)로 시작", 맨 앞 BOM), types.md(`map[T]`의 `T`는 값의 타입).
  이어서 설계 문서의 "규칙 문장"이 매뉴얼에 다 있는지 한 번 대조하는 시험(목록 파일)을 둘지 정한다(결정).
- 처리: 이 커밋. 매뉴얼 언어 명세에 빠진 규칙과 경계 사례 펜스: `modules.md`(끌어올림, 목록 import의 묶음, `as`와 목록, 가리기 E0308, E0311, 비공개·없는 멤버의 정적/동적 에러), `functions.md`(평가 순서, 람다 본문의 범위, 람다의 이름 인자, 부분 적용의 한 번 평가·`_` 여럿·가장 안쪽 호출·E0123), `scope.md`(최상위 `fn` 이름 E0209·E0210, 지역 `fn`의 복사와 E0208), `match.md`(절 시작 규칙과 E0235, guard의 맨 바깥 `=>`), `lexical.md`(태그는 일반 범주 Lu, Lt·Nl은 이름, 맨 앞 BOM), `types.md`(`map[T]`의 `T`는 값), `operators.md`(피연산자 왼쪽부터), `grammar.md`(import 주석, 토큰 주석). 대조 시험은 두지 않음. 설계 문서 00~08 머리에 "규범은 매뉴얼 언어 명세, 이 문서는 결정의 기록". 결정 D34

### SP18. fung으로 쓴 모듈이 인자를 검사하지 않아 내부 에러가 새고, 부작용 뒤에 실패한다(11-08 VA7 후속) — 버그, 낮음 (동시성 영역의 `conc` 인자 검사 지적을 합침)
- 재현:
  ```
  import cli;  print(cli.parse("x"))
  # error[E0601]: no pattern matched "x"   (call path: extras/cli.fung:15 _parse)
  import conc; print(conc.retry(() => Err(1), times: 2, delay: 5))
  # error[E0403]: argument `d` must be dur, got int   (사용자는 `d`를 준 적이 없다; 첫 실패 뒤에야 남)
  import conc; print(conc.pmap([1], x => x, limit: "a"))   # error[E0501]: cannot compare str and int
  import conc; print(conc.pmap([1, 2, 3, 4], x => x, limit: 2.5))
  # error[E0501]: cannot make a range from int and float   (호출 경로 extras/conc.fung:51 <lambda>)
  import conc; print(conc.retry(() => Err(1), times: "2"))  # error[E0501]: cannot compare str and int
  import fs;   print(fs.with_open("t.fung", 1))
  # error[E0401]: cannot call a value of type int  (파일을 연 뒤에 실패; extras/fs.fung:41 _with_ok)
  scope(s => conc.fan_in(s, 1))                  # error[E0506]: cannot take the length of int
  ```
  기대: 매개변수 이름을 말하는 E0403(`argument `delay` must be dur, got int`), 부작용 전에. 같은 실수에 prelude는 이미 E0403 `argument … must be …`을 낸다(`pad_left(s: str, n: int, …)`, `_r.count`).
- 근거: 06 "fung extras의 인자 검사도 원시와 같은 에러를 낸다(F-4, VA7): 타입은 매개변수의 타입 표기 … 에러는 사용자가 부른 함수의 인자를 말한다". VA7은 prelude만 고쳤고 `http`는 이미 표기가 있다.
- 원인 위치: `extras/cli.fung:13` `parse`, `extras/conc.fung:14` `fan_in`, `:35` `pmap`(36~40), `:62` `retry`(64~67), `extras/fs.fung:32` `with_open`, `:37` `with_temp_dir`.
- 고치는 법: 타입 표기를 단다: `parse(argv: list, bools: list = [])`, `retry(f: fn, times: int = 3, delay: dur = 0ms)`, `pmap(xs, f: fn, limit: int | nil = nil)`(기본값 `nil`/`0`과 음수의 처리는 SP15의 결정), `fan_in(s: scope, rxs: list)`, `with_open(path: str, f: fn, mode: sym = 'read, perm: int | nil = nil)`, `with_temp_dir(f: fn)`.
  `cli.parse`의 원소(문자열 리스트)는 `_native.net.need_*` 같은 도우미나 `_r.items`로(`bools`의 원소 검사는 LB38과 같은 고침). 시험: 모듈 함수마다 틀린 인자 한 사례가 E0403과 그 이름을 내는지.
- 처리: 이 커밋. `cli.parse(argv: list, bools: list = [])`, `conc.fan_in(s: scope, rxs: list)`, `pmap(xs, f: fn, limit: int | nil = nil)`, `retry(f: fn, times: int = 3, delay: dur = 0ms)`, `fs.read_lines(path: str)`, `with_open(path: str, f: fn, mode: sym = 'read, perm: int | nil = nil)`, `with_temp_dir(f: fn)`. 표기로 말할 수 없는 검사(1 이상의 정수, `0s` 이상의 기간, 원소 타입)는 새 다리 `_native.arg`(`count`·`positive`·`dur`·`list_of`·`str_map`; `_native.net`의 `need_count`·`need_dur`·`need_str_map`을 옮김, open-questions의 "다른 모듈이 쓰게 되면 옮긴다")가 사용자의 인자 이름으로 E0403을 낸다. 모두 부작용 전에(`with_open`은 열기 전, `retry`는 `f` 전, `pmap`·`fan_in`은 태스크 전). 시험 `crates/fung-cli/tests/extras_arguments.rs`(+ `expected/extras/arguments.txt`): extras의 공개 `fn`마다 틀린 인자 한 사례 이상(39개)이 번호·인자 이름·출력 없음·파일 없음을 지키고, 새 공개 `fn`에 사례가 없으면 실패. 매뉴얼 `conc.md`·`cli.md`·`errors.md`(E0310), 06·05, CHANGELOG.
- 처리(최종 검토, 이 커밋): 개수 검사는 `_native.arg.count` 하나(`count`가 그 수를 돌려주고 prelude의 `take`·`drop`·`pad_left`·`pad_right`·`iterate`도 씀, `_native.range.count` 삭제). 빈 문자열이라 틀린 인자는 `nat::not_nonempty`로 값을 보인다(``must be a non-empty str, got ""``: `pad`, `split`의 `sep`, `exec`의 `cwd`; `exec([])`는 ``got []``). `fs.exists`의 `Err`는 자기 동사(``cannot check whether `p` exists: …``, D17). 매뉴얼 `strings.md`·`fs.md`, CHANGELOG.

### SP19. 거꾸로 된 범위 `5..0`이 말없이 비고, 범위 값은 슬라이스가 되지 않는다 — 결정, 낮음
- 재현:
  ```
  r = 5..0
  print(len(r), map(r, x => x), is_empty(r), r)   # 0 [] true 5..0
  xs = [1, 2, 3]
  print(xs[2..1])        # error[E0516]: slice start 2 is after its end 1 (length 3)
  print(xs[(0..2)])      # [1, 2]
  r2 = 0..2
  print(xs[r2])          # error[E0501]: cannot index list and range
  ```
  같은 `a..b`가 인덱스 안에서는 "시작이 끝보다 뒤면 에러"이고 값으로는 빈 범위다. `for i in n..0`(거꾸로 세기를 의도)는 말없이 아무것도 하지 않는다. 슬라이스는 문법이라 괄호 친 리터럴은 되고 변수에 담은 범위는 안 된다(FE27과 같은 "괄호 투명").
- 근거: values.md·operators.md "범위"는 시작이 끝보다 뒤인 범위에 대해 아무 말이 없다(명세 빈 곳). 원칙 1·7.
- 원인 위치: 범위 값 `crates/fung-vm/src/value/`의 range(길이 0), 슬라이스 `crates/fung-vm/src/value/index.rs`의 `Value::List` 색인(범위 값은 `type_error("index", …)`), 파서의 인덱스 안 `..`.
- 고치는 법: 결정. (a) **추천**: 명세에 "범위 값의 시작이 끝보다 뒤면 빈 범위"를 적고(지금 동작; `0..len(xs) - 1`의 빈 리스트 같은 정상 쓰임 때문에 에러로 바꾸지 않음), 대신 `xs[r]`에 범위 값을 받아 리터럴 슬라이스와 같은 규칙(E0516 포함)을 적용 — "슬라이스는 범위 값으로 하는 색인" 한 규칙. (b) 범위 값 자체도 시작 > 끝을 에러로(엄격하지만 빈 범위가 필요한 계산이 `max`를 써야 함).
- FE27과의 관계: 원인이 같은 곳(괄호가 트리에 남지 않음, 인덱스 안 `..`의 판정)이다. FE27 (a)로 괄호를 남기면 `xs[(0..2)]`가 "범위 값 색인"이 되므로, 여기 (a)의 "범위 값 색인을 받는다"를 함께 정해야 `xs[(0..2)]`가 지금처럼 동작한다. 두 항목은 한 세션에서 함께 묻고 고친다.
- 처리(파서 쪽): FE27과 같은 커밋. 괄호 친 범위 `xs[(0..2)]`는 범위 값 인덱스로 읽힌다(그 값을 슬라이스처럼 받는 런타임과 거꾸로 된 범위 값의 에러는 Task 9), 결정 D7.
- 처리(런타임 쪽): 이 커밋. 거꾸로 된 범위 값은 새 번호 E0522(`range start %d is after its end %d`; E0516은 슬라이스 전용으로 둠), 컴파일러는 거꾸로 된 상수 범위를 접지 않아 실행 때 E0522. `index`가 리스트·문자열의 범위 값 색인을 같은 끝의 슬라이스로(E0516 포함). 저장소의 `5..1` 꼴 빈 범위는 `5..5`·`5..=4`로. 02, `operators.md`, `access.md`, `errors.md` E0522, `collections.md` `len`, CHANGELOG, 결정 D7(★ 엄격한 쪽).

### SP20. 정규식의 `.`·문자 클래스가 UTF-8이 아닌 바이트에 맞지 않는다(12절 의심) — 문서, 낮음
- 재현:
  ```
  import regex
  print(regex.find_all("\xff\xfe", "."), regex.is_match("\xff", "^.$"))   # [] false
  print(regex.replace("\xffab", "[^a]", "X"))                             # "\xffaX"  (\xff는 [^a]에도 안 맞음)
  print(len("\xff"), chars("\xff"))                                       # 1 ["\xff"]
  ```
- 근거: 02 "UTF-8에 속하지 않는 바이트는 그 바이트 하나가 한 글자"; `manual/reference/modules/regex.md` 표: `.`은 "줄바꿈이 아닌 아무 글자 하나", `[^0-9]`는 "괄호 안의 글자 하나 … 여집합". 판단: 엔진(`regex::bytes`의 유니코드 모드)이 잘못된 바이트를 어떤 유니코드 클래스에도 넣지 않는 것은 바꾸기 어렵고(패턴을 다시 써야 함) 바꿀 이유도 약하다. 문서 문제로 본다.
- 원인 위치: `crates/fung-vm/src/native/regex*.rs`(유니코드 모드의 `regex::bytes`), 문서 `manual/reference/modules/regex.md`.
- 고치는 법: regex.md의 "위치는 글자 단위다" 근처에 한 단락: "UTF-8이 아닌 바이트는 `len`에서는 한 글자지만 `.`·`\w`·`[^…]` 같은 유니코드 클래스에는 맞지 않는다. 그런 바이트를 잡으려면 `(?-u:\xff)`·`(?-u:.)`". 02의 L4 절에도 정규식 예외를 한 줄.
- 처리: 이 커밋. `regex.md` "위치는 글자 단위다" 뒤에 단락과 `fung run` 펜스(`.`·`[^a]`가 `\xff`에 맞지 않음, `(?-u:.)`로 잡음), 설계 02 L4 절에 한 줄. 동작은 그대로.

### SP21. 문자열이 아닌 맵 키가 자리마다 E0501과 E0514로 갈린다 — 결정·정리, 낮음 (값 영역의 같은 항목을 합침)
- 재현:
  ```
  print({a:1}[1])        # error[E0501]: cannot index map and int
  print(get({}, 1))      # error[E0501]: cannot search map and int
  print(put({}, 1, 2))   # error[E0501]: cannot index map and int
  print(remove({}, 1))   # error[E0501]: cannot index map and int
  print('a in {a: 1})    # error[E0501]: cannot search map and sym
  print(group_by([1], x => x))   # error[E0514]: cannot use a value of type int as a map key
                                 #   help: map keys are strings: convert the key with `str(...)` …
  ```
- 근거: 02 "맵": "키는 항상 문자열(… 다른 값을 키로 쓰면 E0514이고 힌트가 `str(…)`을 권한다)", `values.md` "다른 값을 키로 쓰면 바꾸라는 힌트와 함께 멈춘다"; `manual/reference/functions.md`: 매개변수 타입이 틀리면 E0403, 여러 타입을 받는 함수는 E0506. `maps.md` `get`의 "`k`가 문자열이 아니면 E0501"은 그 규칙과도 다르다.
  같은 상황에 번호 둘이다. E0514의 카탈로그 설명은 "문자열이 아닌 값을 맵의 키로 쓰려 했다"로 정확히 이 경우인데 `group_by` 한 곳만 쓴다. E0501은 `errors.md`에서 "이항 연산자가 받은 두 값의 타입 조합"이라 `put`·`get`(연산자가 아님)의 에러로는 설명이 맞지 않고, `put`에서 동사가 "index"인 것도 구현(공용 `need_key("index", …)`)이 드러난 것이다.
  `values.md`가 약속한 "바꾸라는 힌트"는 E0514에만 붙어 있어 `put(m, 1, v)`에서는 나오지 않는다. 심볼 키(`m['a]`)는 JSON·문자열과 섞는 흔한 실수인데 E0517처럼 심볼 전용 안내도 없다.
- 원인 위치: `crates/fung-vm/src/value/index.rs` 100~101행(색인 `index`)·259~261행(`in`, `contains`)의 맵 갈래 `type_error`, `crates/fung-vm/src/native/nat.rs:289` `need_key`(E0501), `crates/fung-vm/src/prelude/collections.rs:560` `get_fn`(E0501 "search"), `collections.rs:467`(`group_by`의 E0514). `get`·`put`·`remove`는 같은 `index`/`search` 동사를 빌린다.
- 고치는 법(결정):
  - (가) **추천**(두 영역 같음): 맵을 키로 찾거나 고치는 모든 자리(`m[k]`·`m.k`의 동적 키·`k in m`·`get`·`put`·`remove`)에서 키가 문자열이 아니면 E0514로 통일하고, 심볼 키면 도움말을 "map keys are strings: write `m["a"]` or `str(k)`…"로. 맵 쪽의 `type_error("index"/"search", …)` 갈래가 사라진다.
    `errors.md` E0514·`maps.md`의 `get`·`put`·`remove` 에러 줄("E0501")과 values.md 경계 사례를 고친다.
  - (나) 지금대로 두고 `maps.md`·`errors.md`에 "키 타입은 E0501, `group_by`만 E0514"를 적는다(덜 추천: 특수한 경우를 문서로 굳힘).
- 처리: 이 커밋. `value/index.rs` `not_a_key`(E0514, 심볼이면 도움말 ``write `"a"` for `'a`, or convert the key with `sym_name(k)` ``)를 `index`·`contains`의 맵 갈래, `nat::need_key`(`get`·`put`·`remove`), `group_by`가 함께 씀. 카탈로그 E0514의 기본 도움말은 `group_by` 예 대신 ``convert the key with `str(k)` ``. 매뉴얼 `maps.md`·`collections.md` `contains`·`errors.md` E0514·`values.md` 경계 사례, 02, CHANGELOG, 결정 D13.

### SP22. 설계 문서·EBNF의 낡은 예와 표 — 문서, 낮음 (표준 라이브러리 영역의 06 항목을 합침)
- 재현과 근거:
  - 03 "스코프와 클로저"의 예 `double = x => x * 2; fn f(y) = double(y)  # E0204: double은 변수다` — L9 뒤로는 한 번만 대입된 최상위 이름이라 된다(`print(f(2))` → `4`). 같은 문서 위쪽 L9 설명과 모순.
  - 03 "구조 분해 대입": "이 이름은 … 다시 대입할 수 있다(`v => v = v + 1; v`)" — 람다 본문은 식 하나라 `error[E0110]: unexpected `=``. 익명 `fn(v): v = v + 1; v ;;`가 맞는 예다.
  - 06 "모듈" 표(116행)의 `fs` 줄에 `close`가 남았다(SP10(2026-11-08)에서 `fs.close` 삭제): `$F -e 'import fs
    print(fs.close)'` → `error[E0303]: module `fs` has no member `close``.
  - 06 21·34행과 "네 층" 표가 `lines`를 fung extras라 한다. `spec/builtins.txt`는 `prelude native lines(s)`(Rust)이고 `extras/prelude.fung`에 없다.
  - 06 104행 동시성 prelude 목록에 `timeout`·`send_now`·`recv_now`·`wait_closed`·`stop_signal`·`request_stop`이 없다(`spec/builtins.txt`에는 있다).
  - 06 251행(LB17(2026-11-08)) "임시 파일에 쓰고 `fsync`"와 260행(L11) "`close`·`write`·`append`·`copy`는 … `fsync`는 하지 않는다"가 모순이다. 실제는 `fs.write`만 `fsync`(`native/fs.rs` `write_whole` 265, `sync`).
    비용도 문서에 없다: 같은 파일 50개에 번갈아 2000번 `fs.write` 0.54 s, `fs.append` 0.08 s(macOS APFS, `fsync`가 싸다; F_FULLFSYNC가 아닌 `fsync`라 실제 디스크의 Linux ext4에서는 쓰기마다 ms 단위). `fs.md`의 `fs.write`에도 한 줄이 필요하다(작은 파일을 많이 쓰는 스크립트의 비용).
  - 06 "에러 규칙"·`convert.md` `int`의 "리터럴과 같은 문법"(SP14).
  - 08 "번호 체계" 표의 E07xx 칸은 E0703만 "쓰지 않음"이라 하고 E0704(쓰지 않음)·E0709를 빠뜨렸고, E08xx 칸은 E0805(쓰지 않음)를 말하지 않는다.
  - `grammar.md`의 `params = param { "," param }`·`param = [ "..." ] NAME [ ":" type ] [ "=" expr ]`는 `...rest`를 아무 자리에 두고 기본값도 줄 수 있게 읽히지만 파서는 거절한다(`fn f(...r = [])` → E0110 unexpected `=`, `fn f(a, ...r, b)` → E0110 unexpected name `b`). SP8(2026-11-08)의 "`...`의 자리"는 리스트 패턴에만 적혔다.
  - (저장소 안내) `CLAUDE.md` 55행 `extras/*.fung` 목록의 `agent`는 L5에서 지운 모듈이다(`extras/`에 없음).
- 원인 위치: `docs/design/03-functions-and-patterns.md` 270줄 근처와 "구조 분해 대입", `docs/design/06-native-and-stdlib.md` 16·21·34·104·116·251·260행, `docs/design/08-tooling-and-errors.md` 번호 체계 표, `manual/reference/language/grammar.md` `params`, `manual/reference/modules/fs.md` `fs.write`, `CLAUDE.md`.
- 고치는 법: 위 문장들을 고친다(06 260행은 "`write`는 LB17대로 `fsync`, 나머지는 하지 않는다"). EBNF는 `params = [ param { "," param } ] [ "," rest ] [ "," ]`, `rest = "..." NAME [ ":" type ]`처럼 rest를 따로.
- 처리: 이 커밋. 03의 `double` 예(L9 뒤로는 된다)와 람다 대입 예(`fn(v): v = v + 1; v ;;`), 06 네 층 표의 `lines`(Rust) → `zip`, 동시성 prelude 목록(`timeout`·`send_now`·`recv_now`·`wait_closed`·`stop_signal`·`request_stop`), `fs` 표의 `close` 삭제, `fsync` 문장(`fs.write`만, 비용 측정 0.6초 대 0.08초), 08 번호 체계 표(E0224·E0709, 은퇴 E0231·E0703·E0704·E0705·E0805), `grammar.md`의 `params`·`rest`(나머지 매개변수는 끝 하나, 기본값 없음), 매뉴얼 `fs.md` `fs.write`의 비용 단락, CLAUDE.md `extras` 목록의 `agent` 삭제.

### SP23. 문자열을 받는 컬렉션 함수가 갈린다 — 결정, 낮음
- 재현:
  ```
  print(take("abc", 1), drop("abc", 1), len("abc"), is_empty(""), "abc"[0], "abc"[-1])   # a bc 3 true a c
  print(first("abc"))    # error[E0506]: cannot take the first of str
  print(last("abc"))     # error[E0506]: cannot take the last of str
  print(reverse("abc"))  # error[E0506]: cannot reverse str
  print(min("bca"))      # error[E0506]: cannot take the min of str
  ```
- 근거: 2026-10-24에 `take`·`drop`이 문자열을 받게 됐는데(같은 "앞/뒤에서 꺼내기" 가족인) `first`·`last`·`reverse`는 그대로다. 02 "순회 대상"은 문자열을 `chars(s)`로 바꾸라 하므로 "문자열은 받지 않는다"도 일관된 규칙이지만, 지금은 둘이 섞였다(원칙 7).
- 원인 위치: `extras/prelude.fung:64` `reverse`, `:84` `first`, `:90` `last`(`_list`), `:104·112` `take`·`drop`의 `'str` 갈래.
- 고치는 법: 결정. (a) **추천**: 위치로 꺼내는 함수(`first`·`last`·`reverse`)도 문자열을 받아 글자(문자열)를 돌려준다(`take`와 같은 규칙: 빈 문자열의 `first`는 `nil`). `min`·`max`·`sum`은 "모으기"라 받지 않는다. (b) `take`·`drop`의 문자열 갈래를 되돌린다(슬라이스와 `chars`로).
  `reverse`를 Rust로 옮기는 VA25와 함께 하면 좋다.
- 처리: 이 커밋. `take`·`drop`의 문자열 갈래를 지움(E0506 `cannot take from str`), 저장소의 쓰는 곳은 슬라이스로. `collections.md`·`strings.md`에 관용구, 02, 결정 D14.

### SP24. 에러 카탈로그의 정리거리 — 정리, 낮음
- 재현과 근거:
  - E0805 `not supported yet: %s`는 쓰지 않는 번호인데(`grep -rn E0805 crates extras` → 시험·주석뿐; errors.md도 "쓰지 않는 번호") 카탈로그 help에 다른 은퇴 번호(E0703·E0704·E0802·W0104)처럼 "not used: …"가 없다.
  - E0705 `channel sent into itself`(도움말 없음)는 E0709 `… cycle through channels or tasks`의 한 경우다: `send(tx, rx)` → E0705, `send(t1, r2); send(t2, r1)` → E0709. 같은 원인(고리)에 번호 둘이고, E0709의 도움말("send the data, not the channel end")은 E0705에도 맞다(`crates/fung-vm/src/conc.rs:280` `refuse_cycle`의 `Cycle::Itself`).
  - E0224(`… is assigned as the last statement of a block used as a value`)와 E0231(`a block used as a value %s, so the value is nil`)은 "값 자리 블록은 식으로 끝나야 한다"(01, FE4(2026-11-08)) 한 규칙의 두 번호다. blocks.md는 한 단락에서 둘을 함께 설명한다. FE4가 새 번호를 붙인 것은 결정이었지만 번호 두 개를 유지할 이유가 문서에 없다(E0224의 문구·밑줄 버그는 FE28).
  - E0506 `cannot %s %s`의 동사 몇이 문장이 되지 않는다: `cannot unique int`, `cannot zip int`, `cannot get from list`(색인이 아니라 "맵이 아니다"를 말해야 함). `unique("aab")`의 `cannot unique str`과 `keys`의 "pairs"는 VA19에서도 다룬다.
  - (E0103 `unknown escape`가 `\u{D800}`에 쓰이는 문제는 FE26으로 옮겼다.)
- 고치는 법: E0805 help에 "not used: every builtin is implemented". E0705는 은퇴시키고 E0709로 합치거나(추천, 번호는 재사용하지 않음), 남기려면 E0709의 help를 붙인다.
  E0224·E0231은 결정: 합친다면 E0231 하나로 문구 `%s`에 "ends with an assignment to `x`"를 넣고 E0224는 은퇴(번호 유지 규칙상 E0224를 남기고 E0231을 은퇴하는 쪽이 더 오래된 번호를 지킨다).
  E0506 동사를 "remove duplicates from", "zip"→"zip a value of type"으로, `get`은 `cannot get a key from list`.
- 처리: 이 커밋. E0231을 E0224로 합침(문구 ``a block used as a value %s, so the value is nil``: `is empty`·``ends with an assignment to `x` ``·``ends with an assignment to `_` ``·`ends with an assignment to a pattern`·``ends with `assert` ``·``ends with the definition of `g` ``, FE28의 패턴 대입도 E0224), E0705를 E0709로 합침(`channel::Cycle`의 두 갈래를 `Closes` 하나로), 은퇴 번호는 help `not used: merged into …`로 남김. E0805 help `not used: every builtin is implemented`. E0506 `cannot zip the elements of int`·`cannot get a key from list`(`unique`는 VA19에서 이미 `take the unique elements of`). `spec/errors.txt`, 매뉴얼 `errors.md`(E0224·E0231·E0705·E0709·E0805·E0506)와 쓰는 곳, 설계 01·05·08·09·open-questions, CHANGELOG. 결정 D33

### SP25. `time.format`이 토큰 없는 형식을 말없이 글 그대로 쓴다 — 결정, 낮음
- 재현:
  ```
  import time
  t = time.from_unix(0)
  print(time.format(t, "%Y-%m-%d"))     # %Y-%m-%d
  print(time.format(t, "COMMIT YYYY"))  # CO01IT 1970
  print(time.format(t, "hh:mm"))        # hh:00
  print(time.parse("2026-01-01", "%Y-%m-%d"))
  # error[E0403]: argument `format` must be a format with the year, month and day, got "%Y-%m-%d"
  ```
- 근거: `parse`는 날짜 없는 형식을 E0403으로 거절하는데(LB20(2026-11-08)) 같은 형식 언어의 `format`은 아무것도 검사하지 않는다. strftime(`%Y`)이나 12시간제 `hh`를 쓴 흔한 실수가 말없이 엉뚱한 글이 된다(원칙 1). 이스케이프가 없는 것은 open-questions "M8 후속"에 있으니 그것은 내지 않는다.
- 원인 위치: `crates/fung-vm/src/native/time*.rs`의 형식 토큰 해석(`format`).
- 고치는 법: 결정. (a) **추천**: `format`도 토큰이 하나도 없거나 `%`가 든 형식은 E0403(`must be a format with at least one token`; `%`면 도움말 "tokens are `YYYY`, `MM`, … not `%Y`"). (b) 이스케이프를 더할 때 함께 정한다(open-questions에 이 사례를 덧붙임).
  같은 형식 언어의 `parse` 쪽 구멍(같은 필드 두 번, `ddd`)은 LB36.
- 처리: 이 커밋. `formats_something`(토큰 하나 이상, `%` 없음)이 아니면 `format_fn`이 ``E0403 argument `format` must be a format with at least one token``, `%`면 도움말 "tokens are `YYYY`, `YY`, `MM`, …, not `%Y`". 시험 `format_needs_a_token_and_no_percent`. 매뉴얼 `time.md`, 06, CHANGELOG, 결정 D28.

### SP26. 심볼의 이름을 글로 얻는 길이 없다 — 결정, 낮음
- 재현:
  ```
  print(str('a), "${'a}", str(Red))                # 'a 'a 'Red
  import json; print(json.stringify({k: 'a}))    # {"k":"a"}
  ```
  `'not_found`를 사람이 읽는 글이나 CLI 출력(`"status: ok"`), 파일 이름, HTTP 헤더 값으로 쓰려면 `str(s)[1..]`라는 표기 의존 꼼수나 JSON 왕복뿐이다. 반대 방향 `sym("ok")`는 있다.
- 근거: 02 "심볼"은 `str('a)`가 `"'a"`라 비교에 쓸 수 없다고만 적었다. 원칙 6(같은 일은 한 방법)으로 보면 "이름 → 심볼"과 짝인 "심볼 → 이름"이 빠졌다. `json.stringify`만 이름으로 바꾸는 것도 한 곳만의 규칙이다.
- 원인 위치: `crates/fung-vm/src/prelude/convert.rs`(`sym`·`str`).
- 고치는 법: 결정. (a) **추천**: `name(s)`(심볼 → 문자열, 다른 타입은 E0403) 하나를 prelude에. (b) `str`에 이름 인자(`str('a, quote: false)`) — 특례라 비추천. (c) 문서에 `str(s)[1..]`를 관용구로 적기.
- 처리: 이 커밋. `sym_name(s)`(`prelude/convert.rs`, 심볼이 아니면 E0403 `argument `s` must be sym`), `convert.md` 레퍼런스, 02·06, 결정 D15.

### SP27. 이름·표기의 작은 어긋남 — 정리, 낮음
- 재현과 근거(각각 작아서 하나로 묶음; 원칙 6·7):
  - 숫자 리터럴이 대문자 접두사·지수를 받는다: `print(0XFF, 0O17, 0B1, 1E5)` → `255 15 1 100000.0`. lexical.md·01은 소문자만 보이고, `0O17`은 `0` `O`가 거의 같아 보인다. 같은 일을 하는 표기가 둘이다(`crates/fung-front/src/lexer/number.rs:30,67,147-148`). 추천: 소문자만(대문자는 E0102, 도움말 소문자) 또는 lexical.md에 둘 다 된다고 적기. 리터럴의 다른 비대칭은 FE31.
  - 매개변수 이름 `f`가 두 뜻이다: 대부분 함수(`map(xs, f)`, `fs.with_open(path, f)`)인데 `read_line(f)`·`read_lines(f)`·`fs.write_line(f, line)`에서는 파일 핸들이다. 이름 인자로 쓰면 `fs.with_open(p, f: …)`와 `read_line(f: h)`가 같은 이름의 다른 것. 추천: 파일 핸들은 `file`(또는 `h`).
  - 패턴 인자 이름이 `regex.*(s, pat)`·`regex.replace(s, pat, repl)`과 `fs.glob(pattern)`으로 갈린다. 아무 값은 `x`(`str`, `type_of`, `dbg`)와 `v`(`atom`, `send`, `reset`, `json.stringify`)가 섞였다.
  - 술어 이름: `is_empty`·`is_ok`·`is_err`·`regex.is_match`인데 `valid_utf8`만 `is_` 없음.
  - 인자 수로 반환 타입이 바뀌는 함수: `env()`는 맵, `env(name)`은 `str | nil`; `range(n)`·`range(a, b)`는 `0..n`·`a..b`와 같은 일을 하는 두 번째 길(07 "`range(...)` 함수와 같은 범위 값").
- 고치는 법: 이름 정리는 하위 호환을 따지지 않는 지금 한 번에(CHANGELOG 호환 깨짐). 술어는 `is_utf8`, 핸들 매개변수는 `file`, 패턴은 `pattern` 하나로, 아무 값은 `x` 하나로. `env()` 전체 맵은 `env_all()`(또는 `environ`)로 나눌지 결정.
- 처리(리터럴): 이 커밋. 대문자 `0X`·`0O`·`0B`·`E`는 E0102, 도움말이 소문자 모양을 보인다. `lexical.md`·01, 결정 D32. 이름 정리는 Task 8.
- 처리(이름): 이 커밋. `valid_utf8` → `is_utf8`, `env()` → `env_all()`(`env(name)`은 인자 필수), `range()` 삭제, 매개변수 `file`(`read_line`·`read_lines`·`fs.write_line`·내부 `_native.fs.close`)·`pattern`(`regex.*`)·`x`(`atom`·`reset`·`send`·`send_now`·`json.stringify`). `spec/builtins.txt`, 레퍼런스, 02·06·07, 결정 D32. `put(m, k, v)`의 `v`는 키와 짝인 맵의 값이라 그대로 둠.

## 5. 앞단 (FE) — 렉서·파서·이름 해석·로더·포매터·진단

### FE23. 최상위 `fn` 안의 블록에서 상수 이름에 대입하면 함수 전체에서 그 상수가 사라진다 — 결정, 중간
- 재현:
  ```
  c = 1
  fn g():
    print(c)          # ← error[E0204]: undefined name `c` (not found in this scope)
    if true:
      c = 2
      print(c)
    ;;
    0
  ;;
  ```
  블록 뒤에서 읽어도 같다: `if true: c = 2 … ;;` 다음 줄의 `c`는 `E0206: c is not visible here: it was defined inside a block`.
  같은 모양을 다른 자리에서 쓰면 다르게 된다:
  - 최상위(`c = 1` 다음 `if true: c = 2 ;;`)와 람다(`f = () => if true: c = 2 … ;;`)는 `E0205 cannot reassign c here`.
  - 최상위 `fn` 안의 `for c in [5]: … ;;`와 match 절 `c => …`는 블록 안에서만 가리고, 블록 뒤 `c`는 상수 1을 읽는다(실행 결과 `5`, `1`).
  - 최상위 `fn` 안에서 `f = () => c` 를 먼저 만들고 그 뒤 블록에서 `c = 2`를 대입해도 람다의 `c`가 E0204.
- 근거: `manual/reference/language/scope.md` 규칙 4 "블록 안에서 만든 이름은 밖으로 새지 않는다", 그리고 "최상위 `fn`이 자기 본문에서 묶는 이름은 그 함수의 지역이고 같은 이름의 상수를 가린다". 블록 안의 대입이 블록 밖(앞뒤 모두)의 이름 해석을 바꾸므로 규칙 4와 부딪치고, 같은 블록 대입이 람다·최상위에서는 E0205, 최상위 `fn`에서는 허용이라 "예외 없음"과도 어긋난다. 에러 문구(`not found in this scope`)는 보이는 상수가 왜 안 보이는지 말하지 않는다.
- 원인 위치: `crates/fung-front/src/resolve.rs` `own_bindings`(832행: 함수의 모든 블록의 `Let`을 모음, `for`·match 묶음은 `Let`이 아니라 빠짐), `capture_from`(745행, 784행 `!self.fns[1].own.contains(name)`), `resolve/fn_.rs` 42행.
- 고치는 법(결정):
  - (a) **추천**: 최상위 `fn`의 지역으로 치는 것은 함수 본문 블록(맨 바깥)에서 묶는 이름만. 안쪽 블록의 `c = 2`는 람다·최상위와 같이 E0205("바깥(상수) `c`에 대입할 수 없다"). 규칙 4와 하나가 되고 예외가 없어진다. `own_bindings`가 `Do` 안의 `Do`로 내려가지 않게 하면 된다.
  - (b) 지금 동작 유지 + 진단만: 상수를 가린 지역 때문에 못 읽을 때 E0204에 "`c`는 이 함수 안(N행)에서 대입되어 상수 `c`를 가린다" 도움말.
  - (c) 상수와 같은 이름을 최상위 `fn` 안에서 묶는 것 자체를 금지(새 번호). 가장 엄격하지만 규칙이 하나 늘어난다.
  (a)가 "블록은 바깥 이름에 대입할 수 없다" 한 줄로 설명되므로 매뉴얼이 가장 간단하다. REPL(`redefine` 모드, 모든 입력의 이름을 상수로 봄)에서 어떻게 되는지는 확인하지 못했다(12절).
- 처리: 이 커밋(`fix!: assign constants only in a fn's outer block, return in defaults, one-value results`), `resolve.rs` `own_bindings`가 본문 맨 바깥 블록만 보고(`Do`·`Arm`·`Fn`으로 내려가지 않음) `assigns_constant`가 안쪽 블록·람다의 상수 대입을 E0205로(맨 바깥 블록이 묶는 이름 `own`은 제외: 함수의 지역이라 보통 블록 규칙, 컨트롤러 판단); `for`·`match` 묶음은 지금처럼 자기 블록에서만 가림(새 이름). REPL(`redefine`): 앞 입력의 전역·같은 입력의 이름 모두 안쪽 블록 대입은 E0205(시험 `repl_options`). 다만 REPL에서는 앞 입력의 전역에 맨 바깥 블록에서 대입하는 것도 E0205(파일에서는 지역)로, 그 차이는 그대로 둠. 매뉴얼 `scope.md`, 설계 03, 결정 D4

### FE24. 효과 없는 식 문장이 조용히 버려진다(줄 앞 `-`의 함정, 괄호 잊은 호출, 부분 적용, `==` 오타) — 결정, 중간 (명세 영역의 같은 항목을 합침)
- 재현:
  ```
  fn f(a):
    a
    - 2
  ;;
  print(f(5))        # 실제: -2   (기대: 3, 또는 컴파일 에러)
  ```
  최상위의 `x = 1` / `  - 2`도 `x`는 1이고 `-2`는 조용히 버려진다.
  ```
  fn cleanup() = print("cleaned")
  cleanup            # 괄호를 잊음: 아무 일도 없음
  print(1, _)        # 부분 적용 문장: 아무 일도 없음
  x => x
  1 + 2
  x == 1             # `=` 대신 `==`
  print("done")
  ```
  실제: `done`만 찍히고 종료 0, `fung check`도 아무것도 말하지 않는다. `print(_)` 한 줄도 아무것도 찍지 않고 종료 코드 0(부분 적용 함수가 버려짐, 12절 "명세"의 의심).
- 근거: `docs/design/background.md` 원칙 1 "조용한 실패 없음". 01 "버려지는 자리"와 `blocks.md`는 버려지는 `Err`만 E0606으로 막는다. 01 "줄 이어짐"이 `-`를 이어짐 전용 토큰에서 뺀 것(문장을 시작할 수 있으므로)은 옳지만, 그 결과 "앞 줄의 식을 이어 쓰려던 `- 2`"가 새 문장이 되어 앞 식을 버린다.
  버려지는 함수 값은 정적으로 대부분 보인다(맨 이름이 최상위 `fn`·prelude 함수, 람다 리터럴, `_`가 있는 호출). 비교·산술만의 문장도 결과를 버리면 쓸모가 없다(Python 린터의 "pointless statement", Rust `unused_must_use`).
- 원인 위치: 이런 판정이 없다. 넣을 자리는 `crates/fung-front/src/resolve/value.rs` `check`(값을 쓰는지 `used`를 이미 계산함). 문장의 값을 버리는 것은 컴파일러의 `POP` 경로, 경고를 둔다면 `crates/fung-front/src/resolve/warn.rs`.
- 고치는 법(결정):
  - (a) **추천**: 버려지는 자리(마지막이 아닌 문장, 버려지는 가지의 끝)에 놓인 "효과가 없는 식"을 컴파일 에러(새 번호, 예: E0234 "this value is computed and discarded"; 이름이면 "call it with `()`", 비교면 "did you mean `=`?" 도움말)로.
    효과 없는 식 = 이름, 리터럴, 람다·익명 `fn` 값, 부분 적용(`f(_, x)`), 그것들만으로 된 연산자·필드·인덱스·튜플·리스트·맵. 호출·`return`·`assert`·대입은 해당 없음. 정적으로 모르는 일반 함수 값의 버림(호출 결과)은 그대로. 일부러 버리려면 지금처럼 `_ = 식`. 단항 `-`로 시작하는 문장은 늘 여기에 걸리므로 함정이 닫힌다.
    (명세 영역은 범위를 (1) 함수로 알려진 맨 이름, (2) 람다·익명 함수 리터럴, (3) 부분 적용 호출, (4) 비교 연산이 맨 위인 식으로 좁혀 추천했다. 앞단 영역의 범위가 그것을 포함한다.)
  - (b) W 경고(check·test에서만). 08 규칙상 `fung run`에서는 보이지 않아 실제로는 놓치므로 원칙에 약하다.
  - (c) 줄 앞 `-`·`+`만 따로 막기. 특수한 경우를 늘린다.
- 처리: 이 커밋(`fix!: reject effect-free discarded values and arms after a catch-all`), E0234 `resolve/value.rs`(버려지는 자리 = `Place::Discarded`, REPL의 마지막 값은 `Kept`; 맨 `nil`은 "아무것도 하지 않음"이라 제외(문장 `match`의 `_ => nil`), 태그 값은 튜플처럼 인자가 모두 효과 없으면 해당, 부분 적용도 고정 인자가 모두 효과 없을 때만(컨트롤러 판단); `test` 본문의 `==`에는 `assert` 도움말), 매뉴얼 `blocks.md`·`errors.md`, 설계 01 "버려지는 자리", 결정 D5

### FE25. 가드 없는 모두 맞는 절 뒤의 절이 조용히 죽는다(이름 패턴의 "비교" 함정, 절 본문의 람다 문장) — 결정, 중간 (명세 영역의 `match` 절 항목을 합침)
- 재현 1(이름 패턴):
  ```
  limit = 5
  fn f(x) = match x:
    limit => "at limit"
    _ => "other"
  ;;
  print(f(3), limit)        # 실제: at limit 5
  ```
  `fung check`는 `W0102 unused variable limit`만 낸다(절 본문에서 `limit`를 쓰면 그것도 없다). `_ => 1` 다음 `1 => 2`도 조용히 받는다(`match x: _ => 1; 2 => 3 ;;`, `match x: n => 1; 2 => 3 ;;`; check는 W0102만).
- 재현 2(절 본문의 람다 문장이 새 절):
  ```
  fn pick(m) = match m:
    1 =>
      print("one")
      y => y + 1          # 의도: 절 1이 돌려줄 함수
    _ => (y => y)
  ;;
  print(pick(1), pick(2))
  ```
  실제: `one`, `nil 3`, 종료 0, `fung check`도 조용하다. `y => y + 1`이 "패턴 `y`의 새 절"로 읽혀 `_` 절은 영영 닿지 않고, `pick(2)`가 함수 대신 3이 된다.
  같은 줄에 쓰면 `1 => y => y + 1`은 `error[E0112]: expected the arm's body after `=>`, found `=>``, 앞에 대입이 있으면 `z = 1; y => y + z`는 엉뚱한 `error[E0206]: `z` is not visible here`.
- 근거: `match.md` "패턴 안의 이름은 항상 새로 묶는다. 바깥 변수와 비교하려면 guard". 문서로 알린 함정이지만, 뒤 절이 결코 맞지 않는 코드는 원칙 "조용한 실패 없음·엄격함"으로 정적으로 알 수 있다(Python `match`가 같은 함정에 SyntaxError "makes remaining patterns unreachable"를 낸다).
  01 "match": "절 본문 안의 람다 단독 문장은 새 절로 읽힌다. 람다를 만들고 버리는 문장은 쓸모가 없으므로 받아들인다" — 값 자리의 `match`에서는 절의 마지막 문장이 값이라 "버리는 문장"이 아니다(함수를 고르는 `match`는 흔한 모양). 이 규칙은 규범인 `manual/reference/language/match.md`와 `grammar.md`에 없다(EBNF의 `arm`은 이 모호함을 풀지 않는다).
- 원인 위치: 도달성 판정 없음. 넣을 자리는 `crates/fung-front/src/resolve/pattern.rs`(절 패턴 해석)·`resolve/value.rs`, 또는 `crates/fung-front/src/compiler/m4.rs:131` `match_expr`. 절 시작 판정은 `crates/fung-front/src/parser/pattern.rs:384` `arm_body`(문장마다 `arm_head`를 먼저 시도).
- 고치는 법(결정):
  - (a) **추천**(두 영역 같음): 가드 없는 절의 패턴이 모두 맞는 것(`_`, 맨 이름, 그것들만으로 된 `|` 대안)이고 뒤에 절이 더 있으면 컴파일 에러(새 번호, "this arm matches everything, so the arms after it never run"). 맨 이름이 바깥 변수와 같은 이름이면 도움말 "compare with a guard: `v when v == limit`". 재현 2와 오타도 이것으로 잡힌다.
  - 함께(결정 없이 가능): 재현 2의 첫 사례(`=>` 바로 뒤 람다)의 E0112에 도움말 "a lambda as an arm's value goes in parentheses: `1 => (y => y + 1)`". match.md 경계 사례에 "절 본문의 줄이 `패턴 =>`로 읽히면 새 절이다, 람다는 괄호로"를 적는다(SP17).
  - (b) 경고. (c) 그대로 두고 문서만.
- 처리: 이 커밋, E0235 `resolve/pattern.rs` `match_expr`(`|` 대안은 갈래 하나라도 모든 값에 맞으면 모든 값에 맞음: D6의 "그것들만의 대안"을 넓힘(컨트롤러 판단), 맨 이름이 보이는 변수면 guard 도움말, 앞 절보다 깊이 들여 쓴 절이면 람다 괄호 도움말), `=>` 바로 뒤 람다의 E0112 도움말(`parser/pattern.rs`), 매뉴얼 `match.md`(경계 사례·절 본문의 `패턴 =>` 줄, SP17의 해당 부분)·`errors.md`, 설계 01 "match", 결정 D6

### FE26. 가장 흔한 구문 에러의 진단이 일반 E0110 "unexpected …"뿐이다(11-08 FE15 후속) — 정리(진단), 중간
- 재현(모두 종료 코드 2):
  - 닫히지 않은 괄호: `x = [1, 2` / `print(x)` → `E0110: unexpected name print` at 2:1. 여는 `[`(1:5)를 가리키지 않는다(닫히지 않은 블록 E0112는 시작 위치를 말한다).
  - 빠진 쉼표: `x = {a: 1 b: 2}` → `unexpected name b`, `print("a" "b")` → `unexpected string "b"`(기대: "expected `,` or `}`").
  - 이름 인자 뒤 위치 인자: `f(b: 2, 1)` → `unexpected integer 1`. `fn f(a, ...r, b = 1)` → `unexpected name b`. `fn f((a, b) = (1, 2))` → `unexpected =`.
  - 작은따옴표 문자열: `x = 'hello'` → `E0101 unexpected character "'"` at 닫는 따옴표(도움말 없음; `'hello`는 심볼로 이미 먹힘).
  - 빈 보간: `"${}"` → `E0110: unexpected end of file`(파일 끝이 아니다).
  - 이스케이프: `"\u{110000}"`·`"\u{d800}"`이 `E0103 unknown escape "\\u{110000}"` — 아는 이스케이프의 범위 밖 값(유니코드 스칼라가 아님)인데 "모르는 이스케이프"이고, 인자가 fung 따옴표 형식이라 `\`가 둘로 보인다(`"\q"`도 `unknown escape "\\q"`). 명세 영역도 같은 것을 짚었다(카탈로그 정리 SP24에서 옮김).
  - `x | y = 1` → `unexpected |`(E0216이 맞는 상황).
- 근거: FE15(지난 점검)가 E0110에 문장을 넣던 것을 번호로 나눴지만, "무엇을 기대했는지"가 없는 일반 E0110이 여전히 가장 흔한 실수(쉼표·괄호)를 덮는다. 08 진단 원칙(위치와 고치는 법).
- 원인 위치: `crates/fung-front/src/parser.rs` `unexpected`(193행)를 부르는 곳들: `parser/collections.rs` `list_items`·`map_entries`, `parser/expr.rs` 호출 인자, `parser/fn_.rs` 매개변수; `lexer/string.rs` `escape`(104행), 보간 렉싱.
- 고치는 법: 괄호 안 목록에서 멈추면 E0112 형식 "expected `,` or `]`, found …"에 여는 괄호 위치 노트(`note: the [ opened here`). 위치 인자가 이름 인자 뒤면 전용 문구. `'…'` 다음에 `'`가 오면 도움말 "strings use double quotes; `'name` is a symbol". 빈 보간은 "empty `${}`".
  `\u{…}` 범위 밖은 "`\u{D800}` is not a Unicode scalar value"로 따로(새 문구 또는 E0103의 `%s` 문구 변형), E0103 인자는 따옴표 없이 쓴 그대로.
- 처리: 이 커밋(Task 6). 괄호 안 목록(리스트·맵·튜플·인자·매개변수·람다 매개변수·패턴)에서 멈추면 E0112 ``expected `,` or `]`, found …`` + `note: the [ opened at L:C`(`Parser::list_end`). 이름 인자 뒤 위치 인자는 새 E0126. 나머지 매개변수 뒤는 E0112 "`)` after the rest parameter" + 도움말, 구조 분해 매개변수·나머지 매개변수의 `=`·`:`는 E0112 + 도움말. `'` 단독의 E0101에 큰따옴표 도움말. 빈 보간은 E0112 "an expression inside `${…}`", 보간 코드의 끝은 렉서가 `}`를 텍스트로 둔 Eof라 ``unexpected `}` ``. E0103 문구는 `%s`(쓴 그대로, 보이지 않는 글자는 U+XXXX), 글자가 아닌 `\u{…}`는 새 E0106. `x | y = 1`은 이미 E0216(W4). 매뉴얼 errors.md(E0103·E0106·E0110·E0112·E0126), learn/02, vm/03·04.

### FE27. 괄호가 트리에 남지 않아 괄호식과 슬라이스·파이프의 뜻이 모양에 따라 갈린다(12절 의심) — 결정, 중간 (명세 영역의 파이프 괄호 항목을 합침)
- 재현(슬라이스):
  ```
  x = [1,2,3]
  print(x[(0..2)])          # [1, 2]      괄호 친 범위도 슬라이스
  r = 0..2; print(x[r])     # E0501 cannot index list and range
  a = nil; print(x[a ?? 1..])    # [2, 3]
  print(x[a ?? 1..2])            # E0501 cannot index list and range  (?? 가 범위를 감쌈)
  b = nil; print(x[..b ?? 2])    # [1, 2]
  print(x[0..b ?? 2])            # E0501 cannot make a range from int and nil
  ```
- 재현(파이프):
  ```
  fn mk(a, b = 0) = "a=${a} b=${b}"
  fn adder(n) = x => x + n
  print(1 |> (mk(10)))        # a=1 b=10       (기대: mk(10)의 값을 1로 부름 → 에러)
  print(1 |> (adder(10)))     # error[E0402]: adder takes 1 argument(s), got 2   (기대: 11)
  fn scale(k, n) = k * n
  print(5 |> (scale(2, _)))   # error[E0227]: `_` in a call on the right of `|>`  (기대: 10)
  g = scale(2, _); print(5 |> g)          # 10
  print([1] |> (try_map(x => Ok(x))?))    # error[E0229]
  ```
  기본값 때문에 `1 |> (mk(10))`은 에러 없이 다른 값이 된다. `x |> (Ok(_))`도 E0227이다.
  반면 괄호 안의 다른 식은 값으로 부른다: `3 |> (f)`, `3 |> (x => x + 1)`, `3 |> m.f`, `3 |> [f][0]`, `3 |> (c then f else g)` 모두 "값을 구해 부른다".
- 근거: `grammar.md` `index = "[" ( expr | [ expr ] ".." [ expr ] | … ) "]"` — 두 자리 모두 `expr`인데, 구현은 끝 자리가 `..`보다 강한 식만, 시작 자리는 `..]`일 때만 약한 식을 받는다. 괄호식 `( expr )`는 문법상 하나의 값인데 `xs[(0..2)]`가 범위 값이 아니라 슬라이스 문법이 된다.
  `manual/reference/language/operators.md` "파이프": 규칙은 "오른쪽이 호출이면 맨 앞 인자로", "오른쪽이 이름뿐이면 그것으로 부른다" 둘뿐이라고 하지만, 실제 규칙은 "오른쪽이 (괄호를 벗긴) 호출·태그면 끼워 넣고, 그 밖의 식은 값을 구해 부른다"다(`m.f`·`[f][0]`·람다는 "이름"이 아니다). 두 규칙은 괄호식을 말하지 않는다.
  같은 쪽이 괄호를 묶음의 표시로 쓴다(`(xs |> try_map(f))?`, `5 |> (n => scale(2, n))`). `g = scale(2, _)`로 이름을 붙이면 되는데 그 자리에 식을 그대로 쓰면 안 된다: 원칙 7(규칙 하나에서 결과가 따라 나와야 함)과 어긋난다.
- 원인 위치: `crates/fung-front/src/parser/collections.rs:104` `paren_or_tuple_inner`가 `(e)`를 `e` 그대로 돌려줘 괄호가 트리에 남지 않는다. `crates/fung-front/src/parser/expr.rs` `index_or_slice_inner`(500행~: 파싱한 식의 머리가 `Head::Range`인지로 슬라이스 판정). `crates/fung-front/src/desugar.rs:292` `pipe`가 `rhs.head == Head::Call`만 본다(E0227·E0229 판정도 같은 노드).
- 고치는 법(결정):
  - (a) **추천**(두 영역 같음): 괄호는 묶음이다. 파서가 `(e)`를 표시(`Head::Paren`/`Head::Group`, 또는 노드의 플래그)로 남긴다.
    - 슬라이스는 `[` 바로 안의 최상위 `..`만(괄호 친 범위는 범위 값 → 지금처럼 E0501, 또는 범위 값 인덱스를 허용: SP19의 결정). 슬라이스의 시작·끝 자리는 둘 다 `..`보다 약한 식(`??`·`|>`)도 받게(`[a ?? 1..b ?? 2]`), 문법 주석과 일치.
    - 파이프는 괄호 친 오른쪽을 "값"으로 본다: `x |> (mk(10))`은 `mk(10)(x)`, `x |> (scale(2, _))`는 `scale(2, _)(x)`(E0227 아님), `(try_map(f)?)`는 E0229 아님. 명세 규칙을 "오른쪽이 괄호 없는 호출·태그면 끼워 넣고, 그 밖의 식은 값을 구해 부른다"로 고친다(규칙을 하나 더하지 않고 "호출 모양일 때만 끼워 넣는다"로 설명).
    - 포매터가 괄호를 지키는지도 확인.
  - (b) 동작 유지, `grammar.md`·`operators.md`에 "괄호는 뜻을 바꾸지 않는다, 괄호 친 호출도 끼워 넣는다, 슬라이스 끝 자리는 `..`보다 강한 식"을 적음(관용구 `x |> (v => …)`와 헷갈림이 남는다).
  - SP19(범위 값 슬라이스)와 한 세션에서 함께 묻고 고친다.
- 처리: 이 커밋(파서 쪽). 괄호는 노드의 `paren` 표시로 트리에 남고, 파이프는 괄호 없는 호출·태그에만 끼워 넣으며 그 밖의 식은 값으로 부른다(`desugar::pipe`). `[` 바로 안의 `..`·`..=`만 슬라이스이고 두 끝은 어떤 식이든(`parser/expr.rs` `index_or_slice`, `Parser::slice`). `grammar.md`·`operators.md`·`access.md`·01, 결정 D7. 리뷰 고침(같은 작업의 둘째 바퀴): `if` 조건·`match` 대상·`for` 순회 대상·`then` 가지 안의 `..`는 범위, 끝의 맨 위가 `??`·`|>`보다 약한 연산자면 새 E0125(괄호 요구), E0227 도움말이 괄호 친 부분 적용을 권함.

### FE28. E0224가 패턴 대입에 "`this pattern`"을 코드처럼 쓰고 밑줄이 그 낱말 길이다 — 버그, 낮음
- 재현:
  ```
  fn f():
    (a, b) = (1, 2)
  ;;
  print(f())
  ```
  → ``error[E0224]: `this pattern` is assigned as the last statement…`` 그리고 `^^^^^^^^^^^^`(12칸 = "this pattern"의 글자 수). `x = [for i in [1]: _ = i ;;]`는 밑줄이 줄 끝을 넘는다.
- 근거: 진단 위치·문구(08).
- 원인 위치: `crates/fung-front/src/resolve/value.rs` 56행 `name_error("E0224", n.pos, name)` + `resolve.rs` `name_error`(192행: 밑줄 = 인자 글자 수).
- 고치는 법: 패턴이면 다른 문구(예: "a destructuring assignment is the last statement …")와 패턴 노드의 실제 길이(또는 1칸)로 밑줄. E0224와 E0231을 합칠지(SP24)를 먼저 정하면 한 번에 고친다.
- 처리: 이 커밋(Task 6). 패턴 대입으로 끝나는 블록은 E0231 "ends in a destructuring assignment"(`_ = …`면 "ends in `_ = …`"), 위치는 패턴, 밑줄 1칸(패턴 노드에 끝 위치가 없음). E0224는 이름일 때만. SP24(D33)가 E0231을 E0224로 합칠 때 함께 옮겨진다.

### FE29. 모듈 진단: 읽기 실패가 모두 "cannot find", 경로 표시가 제각각, 문장 위치가 `(` — 버그, 낮음
- 재현:
  - 권한 없는 파일(`chmod 000 lib/noperm.fung`), 디렉터리 `lib/dir.fung/` → 둘 다 ``E0301: cannot find module `/private/tmp/…/front/lib/noperm.fung` ``. 파일은 있다.
  - E0301은 절대 경로, 같은 상황의 E0312는 `lib/bin.fung`(표시 경로), `import "lib/m"`은 쓴 그대로.
  - 가져온 모듈의 `print("side")` → E0306이 1:6(`(`)을 가리킨다(문장 시작 1:1이 맞음). E0301·E0305·E0307은 `import`의 1칸, E0309는 경로 전체에 밑줄.
- 근거: 원칙 "조용한 실패 없음"(원인을 바꿔 말함), 08 진단.
- 원인 위치: `crates/fung-front/src/loader.rs` `load_module`의 `Err(_) => … E0301 … key`(422행), `resolve/module.rs`의 E0306 위치(호출 노드 pos가 `(`).
- 고치는 법: `NotFound`만 E0301, 그 밖의 IO 에러는 E0312 형식 "cannot read module `lib/x.fung`: permission denied / is a directory"(E0312 문구를 일반 읽기 실패로 넓히거나 새 번호). 인자는 `display_path(&key)`. 문장 위치는 문장 노드의 시작.
- 처리: 이 커밋(Task 6). `loader::read_error`: `NotFound`만 E0301, 그 밖은 E0312 + 까닭(`fung_front::oserr::reason`, fung-vm의 `oserr::reason`을 옮겨 둘이 함께 씀), 경로는 둘 다 `display_path`. E0312의 UTF-8 도움말은 `InvalidData`일 때만(카탈로그 help 비움). E0306은 `Node::start_pos`(부분 트리의 가장 앞 위치; 괄호 친 식이면 괄호 안 첫 토큰). 밑줄 길이(E0301·E0305·E0307 대 E0309)는 그대로 둠. 매뉴얼 errors.md E0301·E0312.

### FE30. 파일 이름에서 온 모듈 이름이 예약어여도 받는다(11-08 FE22 후속) — 버그, 낮음
- 재현: `lib/if.fung`(`x = 1`)에 `import "./lib/if"` → 에러 없이 `if`라는 이름의 모듈이 묶인다. 쓸 길이 없다: `x = if.x` → `E0110 unexpected .`.
- 근거: `modules.md` "모듈 이름은 파일명이고, 이름 규칙에 맞지 않으면 `as`가 필요"; `lexical.md` 예약어는 "변수나 함수 이름으로는 쓸 수 없다". `as if`는 파서가 막는다.
- 원인 위치: `crates/fung-front/src/loader.rs` `is_name`(461행)이 렉서(`lexer.rs` `ident`의 `KEYWORDS`)와 다른 두 번째 "이름인가" 판정이고 예약어를 보지 않는다(11-08 FE22에서 모은 판정의 남은 한 벌).
- 고치는 법: 렉서의 판정 하나(`KEYWORDS` + `is_upper` + UAX #31)를 `lexer`에서 공개해 로더가 쓰게 하고, 예약어면 E0305.
- 처리: 이 커밋(Task 6). `lexer::is_name`(렉서의 `word_kind` 하나: 예약어·태그·이름)을 로더가 쓰고 `loader::is_name` 삭제. 예약어·대문자 파일 이름은 E0305. 매뉴얼 errors.md E0305, modules.md, learn/10.

### FE31. 숫자·기간 리터럴의 남은 비대칭과 조용한 0(12절 의심) — 결정, 낮음
- 재현:
  - `print(-9223372036854775808)` → 된다, `print(-0x8000000000000000)` → `E0102 malformed number "0x8000000000000000"`.
  - `print(05m)` → E0102(11-08 FE1의 앞자리 0 규칙), `print(1h05m)` → `1h5m`, `print(1h007m)` → `1h7m`.
  - `print(1.5h30m)` → `2h`(소수가 마지막이 아닌 성분에 와도 받음).
  - 12절 의심 확인: `print(0.0000004ms)` → `0s`, `print(1e-400)` → `0.0`. 반대쪽 `1e400`·`99999999999999h`는 E0102/E0105.
    런타임 변환도 같다: `float("1e-400")`·`parse_float("1e-400")`은 조용히 `0.0`(`-1e-400`은 `-0.0`)인데 `"1e400"`은 거절한다(`convert.md` `float`은 큰 쪽만 말한다; 값 영역은 같은 뿌리라 따로 내지 않았다).
- 근거: `lexical.md` "진법: 범위는 10진 정수와 같은 64비트", 11-08 FE1의 "두 자리 이상이면 0으로 시작하지 않는다", 원칙 "조용한 실패 없음"(넘침은 에러인데 아래 넘침은 조용히 0).
- 원인 위치: `crates/fung-front/src/lexer/number.rs` `prefixed_int`(144행, 최소 정수 특례가 10진에만), `duration`(190행: 둘째 성분부터 앞자리 0·소수 위치 검사 없음, `round()`가 0이 되어도 통과), `number`의 float 경로(`f.is_finite()`만 봄). 런타임은 `crates/fung-vm/src/prelude/convert.rs` `parse_float`.
- 고치는 법(결정): (a) **추천**: 진법 정수도 `-` 뒤 최소값 특례를 같이, 기간의 모든 성분에 앞자리 0 규칙, 소수는 마지막 성분만(`1h30.5m`은 되고 `1.5h30m`은 E0105), 0이 아닌 리터럴이 0으로 반올림되면(기간 ns, 실수 아래 넘침) E0102/E0105. 런타임의 `float`·`parse_float`은 같은 뿌리라 함께 정한다(SP14). (b) 문서에 지금 동작을 적음.
  대문자 접두사·지수(`0XFF`·`1E5`)는 SP27.
- 처리: 이 커밋. 진법의 최소 정수 특례(`prefixed_int`가 2^63을 토큰으로, `is_min_int_magnitude`), 기간 성분마다 앞자리 0 규칙·소수는 마지막 성분만·0 ns 반올림은 E0105, 실수 아래 넘침은 E0102. 런타임 `float`·`parse_float`은 Task 9(D12). `lexical.md`·01, 결정 D9.

### FE32. 패턴을 받는 자리마다 받는 모양이 다르다 — 결정, 낮음
- 재현:
  - `for 1 in [1, 2]: print("x") ;;` → `x` 찍고 둘째에서 `E0602`(`for` 머리는 리터럴·대안까지 다 받음).
  - 대입: `(1, x) = (1, 2)` 된다, `1 = 1`·`'a = 'a`·`nil = nil`·`"a" | "b" = "a"` → `E0110 unexpected =`/`|`; `Ok(x) | Err(x) = Err(5)`는 된다; `x | y = 1` → `E0110 unexpected |`.
  - 매개변수: `fn g(1)`·`(1) => 1` → E0112(리터럴은 어디에도 없음, `functions.md`).
  - match: `-1` 패턴은 되는데 `-1s` → `E0110 unexpected duration 1s`(값 `-1s`는 됨; `match.md` "리터럴(음수, …, 기간 …)").
- 근거: `grammar.md` `assign = pattern "=" expr`, `for_expr = "for" pattern …`(둘 다 `pattern` 전체)인데 대입은 첫 토큰이 리터럴·심볼·`nil`·맨 이름+`|`이면 받지 않는다. 문법이 구현을 "그대로 옮긴" 것이 아니다.
- 원인 위치: `crates/fung-front/src/parser/stmt.rs` `statement`(33행~: `try_destructure`를 `( [ { Tag`일 때만), `parser/pattern.rs` `pat_term`(`-` 뒤 INT·FLOAT만), `for_expr`(407행).
- 고치는 법(결정): (a) **추천**: 대입·`for`·매개변수 모두 "구조 분해 패턴"(리터럴을 맨 바깥에 두지 않음, 안쪽 리터럴은 허용 여부를 하나로)으로 통일하고 `grammar.md`에 `destructure` 규칙을 따로 둠; match 리터럴에 `-` 기간 추가. (b) 문법 주석에 지금 제한을 적음.
- 처리: 이 커밋. 대입·`for`·매개변수가 `parser/pattern.rs` `destructure` 하나(맨 바깥 리터럴은 E0112, 안쪽 리터럴과 `|` 대안은 된다; 대입은 리터럴로 시작하는 문장과 `이름 |`도 패턴으로 시도), `pat_term`의 `-` 뒤 기간. `grammar.md`(`destructure`·`d_term`)·`match.md`·`functions.md`·03, 결정 D10.

### FE33. 블록 밖 이름 진단이 묶는 방법마다 다르고, 도움말이 틀린다(11-08 FE3 후속) — 버그(진단), 낮음
- 재현:
  - match 절 묶음: `r = match 1: q => q ;;` 다음 `print(q)` → `E0206 … help: assign the block's value instead: q = if … ;;`(match에 `if` 도움말).
  - `for` 변수: `for i in [1]: 0 ;;` 다음 `print(i)` → `E0204 undefined name i`(블록 밖이라는 말 없음). `for len in [1]: 0 ;;` 다음 `print(len([1,2]))` → `2`(prelude로 조용히 돌아감). 같은 일을 match 절 `len => len`으로 하면 뒤의 `len(...)`가 E0206. FE3(지난 점검)이 막은 "블록 밖 이름이 prelude가 됨"이 `for`에는 남았다.
  - 자기를 부르는 람다 `f = n => … f(n - 1)` → E0204만(도움말 "a lambda copies names when it is made; use `fn f(n) = …`" 없음).
  - `x = 0` 다음 `import "./lib/m" (x)` → `E0209 cannot reassign x` 가 1행(먼저 쓴 대입)을 가리킴.
- 근거: `scope.md` 규칙 4(E0206), 11-08 FE3 처리 문장 "람다·안쪽 함수에서 닫힌 이름은 prelude가 아니다".
- 원인 위치: `crates/fung-front/src/resolve.rs` 676행(도움말 고정 문자열), `pop_block`(433행: `closed`는 블록 이름만, `for`는 desugar가 함수 매개변수로 만들어 빠짐), E0209는 가져온 이름을 먼저 묶는 순서.
- 고치는 법: `for` 변수도 `closed`에 넣기(desugar가 표시를 남기거나 resolve가 `for`에서 온 함수를 알게), E0206 도움말을 묶은 모양별로("use the value of the `match`/`for`"), 자기 참조 람다에 도움말, E0209는 뒤에 오는 쪽(import 이름)을 가리키거나 import가 먼저 처리된다고 노트.
- 처리: 이 커밋(Task 6). `FnScope::closed`가 블록을 만든 것(`scope::Made`: Block·Arm·For)을 기억, E0206 도움말이 그에 맞음. `for` 본문 함수가 끝나면 그 함수의 이름(변수·본문 이름, `$` 제외)을 둘러싼 함수의 `closed`에 For로 넘김: `for` 뒤의 변수는 E0206이고 prelude로 돌아가지 않음. 자기를 부르는 람다(`f = n => … f(…)`)의 E0204에 `fn f(n) = …` 도움말(`Resolver::lambda_targets`). E0209(import보다 앞의 대입)에 "imports are bound before …" note. 매뉴얼 scope.md(규칙 4와 `for` 예), errors.md E0206, vm/06.

### FE34. 원소 타입의 이름을 렉서와 다른 규칙으로 다시 자른다(11-08 FE17 후속) — 버그, 낮음
- 재현(`q` + U+0301 결합 부호, NFC로도 합쳐지지 않음):
  ```
  type q́x = int
  fn f(x: q́x) = x          # 된다
  fn g(x: list[q́x]) = x    # error[E0212]: unknown type `q` / unknown type `x`, 둘 다 3:6(매개변수 x)을 가리킴
  ```
- 근거: `lexical.md` 이름 = UAX #31, `types.md` "안쪽 이름은 컴파일 때 있는 타입인지만 본다(E0212)".
- 원인 위치: `crates/fung-front/src/resolve/alias.rs` `check_elements`(90행: `c.is_alphanumeric() || c == '_'`로 문자열을 다시 자름). 파서 `parser/fn_.rs` `type_term`(206행)이 타입을 문자열로 이어 붙여 두기 때문.
- 고치는 법: 파서가 원소 타입을 이름 목록(또는 작은 트리)으로 남기고 resolve는 그것을 본다. 최소한 `unicode::is_name_continue`를 쓰고, 위치는 타입 표기 자리로.
- 처리: 이 커밋(Task 6). 파서가 타입을 `tree::TypeTerm { name, pos, elems }` 트리로 남김(`Param.types`·`FnInfo.ret`·`TypeAlias.alts`), resolve `check_elements`가 트리를 봄(문자열 다시 자르기 삭제), E0212·E0232는 이름을 쓴 자리. 원소 타입의 중첩도 E0115로 묶음(전에는 한도 없는 재귀). resolve 뒤에는 바깥 이름만 남아 컴파일러·dump 출력은 같음.

### FE35. 포매터가 `true`·`false`·`nil`·`;;` 뒤의 `-`를 부호로 붙인다(12절 의심) — 버그, 낮음
- 재현: `printf 'y = [false - 1]\nz = [nil-1, true -2]\nb = [if c: 1 else: 2 ;; - 1]\n' | $F fmt -` →
  `y = [false -1]`, `z = [nil -1, true -2]`, `b = [if c: 1 else: 2 ;; -1]`. 다시 돌려도 같고(멱등) 토큰도 같지만 겉모양이 뜻(이항 뺄셈)과 다르다. 덤: `w = if a: 1 else: 2 ;; ; q = 3` → `;;; q = 3`(렉서가 `;;` `;`로 자르니 뜻은 같지만 읽기 어려움).
- 근거: 01 "단항 `-`", 포매터는 뜻을 보이는 대로 써야 함.
- 원인 위치: `crates/fung-front/src/format/spacing.rs` `prefix_context`(29행: `k.is_keyword()` 전체와 `DoubleSemi`를 앞 문맥으로 봄).
- 고치는 법: 값으로 끝나는 예약어(`true` `false` `nil`)와 `;;`를 `prefix_context`에서 빼기(`return`·`not`·`and` 등은 남김). `;;` 뒤 `;`는 공백 하나.
- 처리: 이 커밋(Task 6). `prefix_context`에서 `true`·`false`·`nil`과 `;;`를 뺌, `would_join`에 (`;;`, `;`)를 더해 한 칸.

### FE36. 정의되지 않은 이름의 "비슷한 이름" 찾기가 에러 수 × 이름 수(정렬·복사 포함) — 성능, 낮음
- 재현: 이름 n개 대입 + 없는 이름 n개 읽기(`v0 = 1 …`, `print(u0) …`), `fung check`: n=2000 1.47 s, 4000 6.30 s, 8000 26.3 s(에러 8000개를 모두 출력). 보이는 이름이 prelude뿐이면 16000개 0.63 s.
- 근거: FE16(지난 점검)의 "선형 진단" 목표.
- 원인 위치: `crates/fung-front/src/resolve.rs` `visible`(811행: 에러마다 모든 블록 이름·prelude를 `String`으로 복사), `resolve/similar.rs` `similar`(6행: 매번 정렬, 후보마다 `Vec<Vec<usize>>` 할당).
- 고치는 법: 길이 차가 예산보다 큰 후보는 거리 계산 전에 건너뛰기, 정렬 대신 (거리, 이름) 최소값, 한 줄짜리 DP 배열 재사용, `visible`은 참조로. 에러 개수 상한(예: 처음 100개 + "N more")도 고려.
- 처리: 이 커밋(Task 6). `resolve/similar.rs`의 `Index`: 계속 보이는 이름(prelude, 파일의 맨 바깥 블록, 끝난 블록의 이름; `FnScope::lasting`)을 지우기 이웃(최대 2글자)으로 색인해 두고 처음 E0204 때 만들어 이어 채움, 열린 블록·안쪽 함수의 이름만 하나씩 잼. 정렬·복사 없이 (거리, 이름) 최소, 표 한 벌 재사용. 측정(`fung check`, 이름 n개 + 없는 이름 n개, CPU 중앙값): n=2000 1.49 s → 0.032 s, 4000 6.24 s → 0.073 s, 8000 26.2 s → 0.176 s(출력 같음). 릴리스 시험 `perf.rs` `similar_names_for_many_errors_stay_linear`(8,000개 2 s 안).

### FE37. desugar가 함수마다 본문 전체를 훑어 중첩 함수가 O(n²)(12절 의심) — 성능, 낮음
- 재현(`fung check`): 람다 n겹 `f = x0 => x1 => … x0`: 2000 0.04 s, 4000 0.14 s, 8000 0.50 s. 익명 `fn():` 4000겹: `dump ast` 0.01 s, `dump core` 0.13 s. `[fn():` 4000겹 0.20 s. E0115 한도(10,000)가 상한이라 최악 약 0.8 s.
- 원인 위치: `crates/fung-front/src/desugar.rs` `fn_node`(450행: 기본값·패턴 매개변수가 없어도 매번 `names_in(본문)`, 결과는 `Vec`이라 `contains`도 선형).
- 고치는 법: 패턴 매개변수와 기본값이 둘 다 있을 때만 `names_in`을 돌리고 `HashSet`으로.
- 처리: 이 커밋(Task 6). `fn_node`가 패턴 매개변수와 기본값이 함께 있을 때만 `names_in`, 기본값마다의 `names_in`은 앞선 패턴이 있을 때만, 결과는 `HashSet`. 측정(`fung check`, CPU 중앙값 9회): 람다 2000겹 0.041 → 0.009 s, 4000 0.139 → 0.016 s, 8000 0.50 → 0.028 s, 익명 `fn():` 4000겹 0.135 → 0.015 s. 릴리스 시험 `nested_lambdas_desugar_in_linear_time`(9,000겹 0.3 s 안).

### FE38. `return`이 매개변수 기본값과 `test` 블록에서 무엇인지 정하지 않았다 — 결정, 낮음
- 재현: `fn f(a = return 5) = a` / `print(f())` → `5`(기본값 평가 중에 함수에서 돌아옴). `test "a": return Err("x") … ;;` → `fung test`가 E0606으로 실패(곧 test 본문은 함수처럼 `return`을 받음). `fn f() = [return 1, 2]`도 `1`.
- 근거: `functions.md` "return은 감싸는 함수나 람다에서 돌아오고, for 본문과 진입 파일 최상위에서는 쓸 수 없다" — `test` 블록은 진입 파일 최상위에 있고, 기본값은 호출하는 쪽에서 보이는 식이다. `testing.md`는 말하지 않는다.
- 원인 위치: `crates/fung-front/src/resolve.rs`의 E0211·E0213 판정(기본값·test를 함수로 봄), desugar가 test 블록을 함수로 만듦.
- 고치는 법(결정): (a) **추천**: 기본값 안의 `return`은 컴파일 에러(E0211 문구 확장: "return inside a default value"), test 블록의 `return`은 허용하고 `testing.md`에 "test 본문은 함수 본문이라 `return`으로 일찍 끝낼 수 있다" 한 줄. (b) 둘 다 허용하고 문서화.
- 처리: 이 커밋, E0211 문구를 `%s %s`로(`return outside a function`/`return inside a parameter default`/`` `?` inside a parameter default ``, 기본값은 도움말을 바꿈; D8을 컨트롤러 판단으로 `?`까지 넓힘, `!`은 일찍 돌아가지 않고 멈추므로 해당 없음), `FnScope::in_default`, `test` 블록의 `return`은 허용(`testing.md` 한 줄), 매뉴얼 `functions.md`·`errors.md`, 설계 03, 결정 D8

### FE39. 매 실행의 불필요한 복사(12절 의심) — 정리, 낮음 (재현 안 함, 코드 읽기)
- 재현: 측정하지 못함. `import http`가 `-e '1'`보다 약 9 ms 더 드는 것은 지난 점검과 같다(`/usr/bin/time` 해상도로 0.00 s 대 0.01 s).
- 원인 위치: `crates/fung-front/src/loader.rs` `compile_file`이 모듈마다 `b.surface = Some(surface.clone())`(223행), `b.core = Some(res.core.clone())`(258행) — `dump`가 아니면 쓰지 않는다; `compiler::Options.source: src.to_string()`; `lexer.rs` `punct`(554행)가 연산자 토큰마다 `String` 세 개를 할당; `desugar.rs` 451행 `fn_info(n).clone()`.
- 고치는 법: `Stage`가 `Ast`/`Core`일 때만 트리를 옮겨(clone 대신 move) 담기, `punct`는 `&str` 앞부분 비교. 측정은 `bench`로 `import http` 반복.
- 처리: 이 커밋(Task 6). `compile_file`이 `surface`는 `Stage::Ast`(옮김)와 REPL 진입 파일(`keep_value`, 값 출력 판정에 씀)일 때만, `core`는 `Stage::Core`일 때만(옮김) 담음. `compiler::Options`가 `source`·`file`을 빌림(`Options<'s>`). `lexer::punct`는 `&str` 앞부분 비교. `desugar`의 `fn_info(n).clone()`은 새 노드를 만드는 데 필요해 그대로. 측정(`-e 'import http'`, 20회 묶음 9회 CPU 중앙값): 14.1 ms → 11.8 ms(FE37 포함).

## 6. 컴파일러·바이트코드·VM 실행 핵심 (VM)

### VM14. 실패한 `match` 갈래가 묶은 칸은 놓이지 않아, 가드가 거짓이거나 패턴이 중간에 어긋나면 교착한다(11-08 VM1 후속) — 버그, 중간
- 재현 1(가드가 거짓):
  ```
  fn go():
    (tx, rx) = channel(1)
    p = (tx, 2)
    print(match p:
      (t, n) when n == 1 => send(t, n)
      _ => "other"
    ;;)
    for x in rx: print(x) ;;
    print("done")
  ;;
  go()
  ```
  출력: `other`, 그다음 `error[E0702]: deadlock: all tasks are waiting --> b3.fung:8:3`(help: "it still holds that channel's sending end itself (a variable of `go`)").
  기대: `other`, `done`. `_` 갈래로 간 길에서 `t`는 더 읽힐 수 없으므로 `print(…)` 문장이 끝날 때 놓여야 하고, 그러면 `tx`의 마지막 사본이 사라져 채널이 닫힌다.
  dump: 0010 `MATCH 1 ; ($3, $4)`가 `t`를 슬롯 3에 쓰고, 가드의 `JUMPIFFALSE 22`로 빠진 길에는 슬롯 3의 `CLEARLOCAL`이 끝내 없다(문장 뒤에는 `CLEARLOCAL 2`(p)뿐).
- 재현 2(패턴이 중간에 어긋남): `MATCH`가 `t`를 슬롯에 쓴 뒤 `1`에서 실패하고, 같은 E0702가 난다.
  ```
  fn mk():
    (tx, rx) = channel(1)
    ((tx, 2), rx)
  ;;
  fn go():
    (p, rx) = mk()
    print(match p:
      (t, 1) => "one"
      _ => "other"
    ;;)
    for x in rx: print(x) ;;
    print("done")
  ;;
  go()
  ```
  (갈래를 하나만 두거나 `t`를 받는 갈래가 성공하면 교착하지 않는다. 코드를 다른 갈래에 두기만 해도 교착이 생긴다는 점에서 VM1(2026-11-08)과 같은 모양이다.)
- 근거: 03 "값의 수명" "변수는 그 길에서 더 읽힐 수 없게 된 첫 문장이 끝나면 놓인다(VM1)… 다른 가지에서만 읽히는 변수는 이 가지의 첫 문장 뒤에 놓인다", `manual/reference/language/scope.md` 같은 문장.
  재현 2의 `t`는 그 갈래가 선택되지 않았으므로 언어상 묶인 적이 없다. "한 번도 읽지 않는 변수는 돌아올 때까지 산다"(`_watch` 관용구)는 묶인 변수의 규칙이라 여기에 해당하지 않는다.
- 원인 위치:
  - `crates/fung-front/src/compiler/m4.rs` `match_expr`(131행)가 `Compiler::branches`(`compiler.rs` 377행)로 갈래를 컴파일한다. `branches`는 갈래마다 `held`를 "선택 전"으로 되돌리므로, 앞 갈래의 `MATCH`가 이미 쓴 칸(`stored(pat, alt)`로 그 갈래의 `held`에만 들어감)은 실패해서 넘어온 다음 갈래의 길에서 추적되지 않는다. 그래서 어느 `let_go`도 그 칸을 비우지 않는다.
  - VM 쪽 `crates/fung-vm/src/vm/matcher.rs` `Binder::matches`(90행 `RtPattern::Bind`)는 지역 칸을 곧바로 쓰고, 실패해도 되돌리지 않는다(전역만 `pending`으로 미룬다). `desugar.rs` `match_expr`의 숨은 변수 예외("a pattern that fails part way may have written it")도 이 동작 때문이다.
- 고치는 법: 컴파일러만 고치면 된다. `match_expr`에서 `branches` 대신 갈래 루프를 직접 돌며, 갈래 i의 시작 `held`를 `선택 전 ∪ (앞 갈래들의 패턴이 쓰는 칸, moves::written_slots)`으로 둔다. 실패 길에서 그 칸들은 뒤에서 읽히지 않으므로(갈래 이름은 그 갈래 안에서만 보이고, 옆 갈래가 같은 칸을 쓰면 쓰기가 먼저다) 바깥 문장 끝의 `let_go`가 `CLEARLOCAL`로 놓는다.
  가드가 있는 대안 펼치기(`pattern::alternatives`)의 앞 대안도 같은 집합에 넣는다. 시험: 위 두 재현을 `tests/lang/local_lifetime.fung`에 더한다.
  (덤으로 `Binder`가 실패한 매치의 지역 쓰기를 되돌리게 하면 desugar의 숨은 변수 예외가 사라져 VM10 (b)(2026-11-08)가 더 많은 `match`에 적용되지만, 가드 실패는 패턴이 끝까지 맞은 뒤라 컴파일러 쪽 수정이 어차피 필요하다.)
- 처리: 이 커밋. `compiler/m4.rs` `match_expr`가 갈래 i를 "선택 전 ∪ 앞 갈래 패턴이 쓰는 칸"(`moves::written_slots`, 대안 포함)에서 시작해 문장 끝의 `let_go`가 놓는다. 시험 `tests/lang/local_lifetime.fung`(가드 거짓·패턴 중간 어긋남·가드 붙은 대안). CHANGELOG 고침.

### VM15. 내장 함수 호출마다 인자 `Vec`을 새로 할당한다. 문서는 "태스크마다 하나를 재사용한다"고 한다(11-08 VM5·VM13 후속) — 성능·문서, 중간
- 재현:
  ```
  fn loop(i, acc, xs) = i == 0 then acc else loop(i - 1, acc + len(xs), xs)
  print(loop(30000000, 0, [1, 2]))
  ```
  3.11초. `len(xs)`를 `2`로 바꾼 같은 모양(`acc + 2`)은 1.67초라, `len` 한 번에 약 48 ns다.
  `sample`로 3초를 뜨면 실행 스레드 표본 2,163개 가운데 749개(35%)가 인자 배열을 만들고 버리는 데 있다: `Vec::from_iter` 181, `Drain::drop` 90, `_xzm_xzone_malloc_tiny`·`_malloc_zone_malloc` 150, `free`·`_xzm_free_main` 126과 그 안의 `mach_absolute_time` 148(macOS 할당기의 free가 시계를 읽는다).
  호출당으로 보면 내장 함수 호출 비용의 대부분이 이 할당이다. `map(xs, str)`처럼 내장 함수를 콜백으로 원소마다 부르는 길(`call_plain`)도 같은 `take_args`를 지난다.
- 근거:
  - `manual/vm/internals.md` 145행 "호출 규약 — 빠른 길": "Rust로 쓴 내장 함수는 인자를 스택에서 바로 꺼내 받는다. 인자를 잠시 담는 배열도 태스크마다 하나를 재사용한다. 호출된 쪽이 인자를 곧바로… 새 배열로 옮겨 두기 때문에, 다음 호출이 그 배열을 덮어써도 안전하다." 그런 배열은 없다.
    지난 점검 VM13이 이 문장을 짚었는데, VM13의 `처리:` 줄은 다른 두 문장만 "이미 맞았다"고 하고 이 문장은 고치지 않았다(VM13 후속).
  - VM5(2026-11-08)의 처리 기록도 "인자 Vec 하나"를 남은 비용으로 적었다(VM5 후속).
- 원인 위치: `crates/fung-vm/src/vm/call.rs` `Task::take_args`(699행, `drain(at..).map(…).collect()`), 이를 부르는 `call_op`·`start_call`·`call_plain`. 서명은 `crates/fung-vm/src/value.rs` `NativeFn`·`StepFn`(459·464행, `Vec<Value>`를 받는다).
- 고치는 법:
  - (가) **추천**: 내장 함수가 받는 인자를 `Vec<Value>` 대신 작은 값은 안에 담는 타입으로 바꾼다. 이미 있는 `value/items.rs` `Items`(1~2개는 안, 그 이상은 `Box<[Value]>`)를 3~4개까지 안에 담게 넓히거나 같은 모양의 `Args`를 둔다. `Deref<Target=[Value]>`와 소유 `IntoIterator`가 있으므로 `args[0]`·`args.into_iter()`를 쓰는 내장 함수(약 90곳)는 대부분 서명만 바뀐다. `unsafe` 없이 된다.
  - (나) 문서만 고친다: internals.md의 두 문장을 "내장 함수는 호출마다 인자 배열 하나를 새로 받는다"로.
  - 어느 쪽이든 internals.md 145행은 고친다. 측정은 위 두 프로그램과 `bench/list.fung`(콜백 위주). (가)의 효과는 프로파일 비율로만 추정했다(12절).
- 처리: 이 커밋. (가): 내장 함수가 `value/args.rs`의 `Args`(네 개까지 값 안, 더 많으면 `Vec`, `unsafe` 없음)를 받는다(`NativeFn`·`StepFn`·`Task::take_args`). `len` 루프 3.06 → 2.56초(VM15만), 끝에 2.25초. `bench/list.fung`은 득 없음. internals.md "빠른 길"을 실제대로(`Args`, 많으면 호출마다 배열 하나), `docs/notes/bench-languages.md` 4.8절. 결정 D20.

### VM16. 해석 루프·호출의 고정 비용 셋: 명령 해독이 크레이트 밖 함수 호출, 비꼬리 호출마다 인자를 한 칸 밀기, 명령마다 위치 읽기(12절 의심) — 성능, 중간
12절의 의심 "명령당 약 5 ns가 경계 검사와 `Rc<RtProto>` 이중 간접에서 오는지"를 실험판으로 나눠 쟀다. 세 가지 모두 `unsafe` 없이 되고, 셋을 함께 넣으면 `fib`가 20% 빨라진다.

| 판 | `bench/fib` | `bench/loop` | `bench/list` | 호출 위주 루프(아래 `c.fung`) |
|---|---|---|---|---|
| 기준(HEAD) | 0.183 | 0.219~0.221 | 0.229 | 0.937~0.947 |
| (a) 링크 때 명령 해독 | 0.174~0.175 | 0.217 | 0.224~0.225 | 0.925~0.935 |
| (a)+(b) 피호출자 칸 밀기 없앰 | 0.156~0.157 | 0.214~0.215 | 0.225 | 0.834~0.841 |
| (a)+(b)+(c) 위치를 읽지 않음(상한) | 0.147 | 0.210 | — | 0.733 |

`c.fung`는 `fn inc(x) = x + 1`을 천만 번 부르는 꼬리 재귀다(아래). (a)+(b) 판은 골든 시험(`--test golden`)과 `fung-vm` 단위 시험 804개를 모두 통과했다.
```
fn inc(x) = x + 1
fn loop(i, acc) = i == 0 then acc else loop(i - 1, inc(acc))
print(loop(10000000, 0))
```

- (a) **`Instr::op`가 명령마다 함수 호출이다.** `sample`로 `fib(35)`를 뜨면 실행 스레드 표본의 12%(1,235 중 148)가 `fung_front::bytecode::op::Instr::op`다. `Op::from_u8(b).expect(…)`(`Op::ALL` 표의 경계 검사 + `Option`)이고, 다른 크레이트의 함수라 인라인되지 않는다.
  `bench-languages.md` 3.4.1은 `#[inline]`을 붙여 보고 흔들림만 봤다. 여기서는 VM이 링크 때(`vm/link.rs` `link`) `code`를 VM 쪽 구조체 `{op: Op, arg: u32}`의 배열로 한 번 풀어 두고, `step_loop`가 필드를 읽게 했다(바뀐 곳 두 파일, 14줄). 실행 시 해독이 사라진다.
  `manual/vm/08-bytecode.md` 73행 "명령을 꺼낼 때는 비트 연산 둘이면 된다"도 지금 구현(표 조회와 함수 호출)과 다르다.
- (b) **비꼬리 fung 호출마다 `Vec::remove(callee_at)`가 인자를 한 칸씩 민다.** `call_op`의 두 길(`call.rs` 206·227행)이 피호출자 칸을 지우려고 그 위의 인자 전부를 `memmove`한다(표본의 `_platform_memmove`·`DYLD-STUB$$memmove` 30개 남짓).
  실험판은 피호출자 칸을 빈 채로 두고 `Frame`에 "돌아갈 때 자를 높이"(`floor`, 보통 `base`, `CALL`로 들어온 프레임만 `base - 1`)를 두어 `ret_with`·`deliver`·`unwind`의 `truncate`가 그것을 쓰게 했다. `fib` −10%, 호출 위주 루프 −11%.
  `08-bytecode.md` "스택에 이미 놓인 인자가 그 자리에서 새 프레임의 첫 슬롯이 된다. 인자를 따로 복사하지 않는다"도 지금은 한 칸 밀린 자리라 정확하지 않고, (b)를 하면 맞게 된다.
- (c) **모든 명령이 `proto.pos[ip]`(12바이트)를 읽는다**(`vm.rs` 773행). 위치는 에러와 호출 경로에만 쓰이고, 그때는 `Frame::current_pos`가 `ip - 1`로 찾을 수 있다.
  상한을 재려고 위치를 상수(`no_pos()`)로 바꾼 판은 `fib`가 (a)+(b)보다 6%, 호출 위주 루프가 12% 빨랐다. 상수라서 LLVM이 더 지운 몫이 섞였을 수 있으므로 상한으로만 본다.
- 근거: `manual/vm/internals.md` "호출 규약 — 빠른 길", 09 "호출 규약", 위의 08 문장들. `Rc<RtProto>` 이중 간접은 따로 떼어 재지 못했다.
- 원인 위치: `crates/fung-front/src/bytecode/op.rs` `Instr::op`(267행), `crates/fung-vm/src/vm.rs` `Task::step_loop`(767~777행), `crates/fung-vm/src/vm/call.rs` `Task::call_op`(206·227행), `ret_with`·`deliver`·`unwind`, `crates/fung-vm/src/vm/frame.rs` `Frame`.
- 고치는 법: (a) 링크 때 해독한 명령 배열(`RtProto::code: Vec<Code>`), (b) 피호출자 칸을 남기고 프레임에 자를 높이, (c) 해석 루프는 위치를 읽지 않고, 에러를 만드는 자리(`at`·`err`의 호출부, `tick`의 E0701, `call_op`의 에러)가 `frames[fi].current_pos()`로 구한다. 위치를 인자로 받는 도우미가 많아 (c)가 가장 손이 크다. `scripts/bench.sh`의 `fib`·`loop`와 `c.fung` 같은 호출 루프로 잰다.
  덤으로, 모듈 멤버 호출 `m.f(x)`는 호출마다 `LOADGLOBAL` + `FIELD`(문자열로 모듈 멤버 조회)라서 같은 루프가 `import "./m" (f)`보다 약 15 ns(1.09초 대 0.94초, 천만 번) 느리다(`m.fung`는 `fn inc(x) = x + 1`, 위 `c.fung`의 `inc(acc)`를 `import "./m"` 뒤 `m.inc(acc)`로 부른 판과 `import "./m" (inc)` 판의 비교).
  링크 뒤에는 바뀌지 않는 조회라 `IMPORT` 결과에 번호로 접근하는 명령이나 상수 캐시로 없앨 수 있지만, 크기가 작아 위 셋 뒤의 일이다.
- 처리: 이 커밋. (a) `vm/link.rs`의 `Code {op, arg}`로 링크 때 해독, (b) 비꼬리 `CALL`이 피호출자 칸을 비워 두고 프레임의 `floor`까지 자름(`base`·`floor`는 32비트 둘로 프레임 48바이트 유지: `floor: usize`로 56바이트가 되자 `fib`가 20% 느려졌다), (c) 실행 루프는 위치를 읽지 않고 에러·내장 함수 호출·`import` 등에서 `Task::here`로. 에러 위치·호출 경로·`dump`는 그대로(기대 출력 변화 없음). `fib35` 1.96 → 1.55초(−21%), 호출 루프 0.91 → 0.73초(−20%). manual/vm 08-bytecode.md(해독, 스택 그림, `floor`, 위치)·09-closures.md(CALL 그림, 프레임 표)·internals.md, `docs/notes/bench-languages.md` 4.8절.

### VM17. 호출 규약 코드의 중복과 쓰이지 않는 것 — 정리, 낮음
- 재현: 코드 읽기와 `fung dump bytecode`.
  - `vm/call.rs`의 `Params`(22행)와 `Callee`(93행)가 같은 두 갈래(fung·내장)를 두 번 감싸고, `Callee::params()`가 `Params`를 돌려준다. `exact_positional`(80행)과 `exact_native`(87행)도 같은 판정이다.
  - `call_op`의 fung 빠른 길과 묶기 길이 꼬리 호출(`drain` + `reuse_frame`)·깊이 검사(E0804)·`remove` + `push_frame` 열두 줄을 똑같이 두 번 쓴다(195~208행, 218~229행). VM16 (b)를 하면 두 곳을 함께 고쳐야 한다.
  - `RtProto::return_type`(`vm/link.rs` 44행)은 링크 때 채우기만 하고 VM 어디서도 읽지 않는다(반환 검사는 `CHECKRETURN`의 피연산자).
  - 함수 끝에 늘 `RETURN`을 내므로, 꼬리 호출로 끝나는 함수는 `TAILCALL` 뒤에 닿지 않는 `RETURN`이 붙는다. `examples`·`bench/compare` 함수 139개 가운데 56개. `TAILCALL`은 fung 함수·내장 함수·단계 내장 함수 모두 스스로 프레임을 끝내므로(`call_op`의 `ret_with`, `ContFrame::tail`) 그 뒤로 떨어지는 길이 없다.
  - 가드가 붙은 이름 하나 패턴(`m when … =>`)은 늘 맞는데도 `MATCH` + `JUMPIFFALSE`를 낸다(VM10 (c)(2026-11-08)의 `always_matches`가 가드 있는 갈래를 빼므로). 예: `fn f(m) = match m: m when len(m) > 1 => …` dump의 0003~0004.
- 원인 위치: 위 각 줄. 컴파일러는 `compiler/fn_.rs` `function`(본문 뒤 `RETURN`), `compiler.rs` `compile`, `compiler/m4.rs` `match_arm`.
- 고치는 법: `Params`를 없애고 `Callee`에 `len`·`name(i)`·`rest`·`has_default`·`default`를 둔다. `call_op`의 두 fung 길은 "묶기가 필요하면 `bind_in_place`" 한 번 뒤 같은 꼬리/비꼬리 코드로 합친다. `RtProto::return_type`은 지운다.
  본문 끝 `RETURN`은 마지막 명령이 `TAILCALL`·`RETURN`이면 내지 않는다(`?` 스텁의 `patch`는 그 뒤에 붙으므로 영향 없음). 이름 하나 + 가드 패턴은 `MATCH` 없이 `STORELOCAL` 뒤 가드만 검사한다. dump 기대 출력이 바뀌므로 한 번에 bless한다.
- 처리: 이 커밋. `call.rs`의 `Params`를 없애고 `Callee`에 `len`·`param_name`·`rest`·`has_default`·`default`·`exact`(`exact_positional`·`exact_native` 합침), `call_op`의 fung 두 길을 "필요하면 `bind_in_place`" 뒤 꼬리/비꼬리 한 곳으로. `RtProto::return_type` 삭제. 컴파일러 `end_tail`: 꼬리 자리 끝(함수 본문 끝, 꼬리 가지 끝, `return`)이 `TAILCALL`·`RETURN`이면 `RETURN`(과 `CHECKRETURN`)을 내지 않음. 가드 붙은 이름 하나·`_` 패턴은 `MATCH` 없이 `STORELOCAL`(이름이면) 뒤 가드만. dump 기대 출력·매뉴얼(`internals.md` match 설명, `09-closures.md` 한 문장) 다시 만듦.

### VM18. 상수 접기가 연산자 노드마다 왼쪽 사슬 전체를 다시 내려가 긴 식이 O(n²) — 성능, 낮음 (앞단 영역에서 찾은 컴파일러 원인)
- 재현: `fn f(x) = x + x + … + x`(16000항): `dump ast` 0.01 s, `dump core` 0.02 s, `check`·`dump bytecode` 1.27 s. 8000항 0.31 s, 4000항 0.08 s. `x * x …`도 같다. 정수 리터럴 사슬은 접혀서 40000항 0.05 s. `sample`로 본 시간은 전부 `compiler::fold::fold` 재귀.
- 원인 위치: `crates/fung-front/src/compiler.rs` 509행·`compiler/m4.rs` 269행이 `Op` 노드마다 `fold::fold`를 부르고, `compiler/fold.rs` `fold`(13행)가 `fold(&kids[0])?`부터 내려가 왼쪽으로 깊은 사슬에서 매번 맨 아래까지 간다.
- 고치는 법: 접기를 아래에서 위로 한 번만(결과를 노드에 기억하거나 desugar 뒤 한 패스), 최소한 오른쪽 자식부터 보아 상수가 아니면 바로 `None`.
- 처리: 이 커밋. `compiler/fold.rs` `constants`가 컴파일 전에 트리 전체를 아래에서 위로 한 번 돌아 접히는 연산자 노드의 상수를 표로 만들고, 컴파일러(`expr`의 `Head::Op`, `m4.rs` `constant`)는 표를 찾기만 한다. 16,000항 `x + … + x` 1.26 s → 0.03 s. 시험 `perf.rs` `a_long_operator_chain_compiles_in_linear_time`(릴리스 전용, 0.3 s). CHANGELOG 성능.

## 7. 값 층과 prelude (VA)

### VA18. 원소가 하나뿐이면 `sum`·`min`·`max`가 타입 검사를 건너뛴다(`sum([nil])`이 `nil`, 11-08 VA15·VA7 후속) — 버그, 중간
- 재현:
  ```
  $F -e 'print(sum([nil]))'        → nil          (기대: E0501 cannot add nil and …)
  $F -e 'print(sum([true]))'       → true
  $F -e "print(sum(['a]))"         → 'a
  $F -e 'print(sum([{a: 1}]))'     → {a: 1}
  $F -e 'print(sum([nil, 1]))'     → E0501 cannot add nil and int   (원소가 둘이면 에러)
  $F -e 'print(min([[nil]]))'      → [nil]        (`[nil] < [nil]`은 E0501 cannot compare nil and nil)
  $F -e 'print(min([(nil,)]))'     → (nil,)
  ```
  참고로 `sort([nil])`·`sort([{a: 1}])`·`sort_by([1], x => nil)`도 비교를 한 번도 하지 않아 에러 없이 돌려준다.
- 근거: 02 "nil은 조용히 퍼지지 않는다", `collections.md` `sum` "더할 수 없는 원소가 섞이면 E0501". 지난 점검 VA15(사용자 결정 (가): `max([nil])`은 에러)와 같은 구멍이 `sum`에 그대로 남았고,
  VA15의 고침(`_ordered`)은 타입 이름만 봐서 `list`·`tuple` 안의 순서 없는 값은 지나간다(VA15·VA7 후속).
  `sum(xs)`의 결과는 바로 계산에 쓰이므로 `sum([nil])`의 `nil`은 몇 줄 뒤 엉뚱한 자리에서 터진다.
- 원인 위치: `extras/prelude.fung:21` `sum`(첫 원소 `ys[0]`을 씨앗으로 `fold(ys[1..], …)`: 원소가 하나면 `+`가 한 번도 돌지 않음), `:62` `_ordered`(타입 목록만 봄).
- 고치는 법:
  - `sum`: 씨앗에도 `+`의 검사를 한 번 건다. 예: `_addable(x) = type_of(x) in ['int, 'float, 'str, 'list, 'dur] then x else x + x`(`x + x`가 `+`와 같은 E0501을 낸다). 원소 하나를 두 배로 만드는 비용을 피하려고 타입으로 먼저 거른다.
  - `_ordered`: 타입 목록 대신 늘 `x < x`를 한 번 부른다(리스트·튜플 안의 `nil`도 `<`와 같은 E0501). 큰 리스트 하나를 자기와 비교하는 O(n)이 걱정이면 `list`·`tuple`일 때만 `x < x`.
  - `min`·`max`·`sum`을 Rust로 옮기면(VA25) `compare`/`add`로 한 번에 풀린다.
  - `sort`의 원소 하나는 결정: (가) 지금대로(비교가 없으면 에러도 없음, 문서에 한 줄) **추천** — `sort`는 값을 꺼내 계산하지 않고 담아 돌려줄 뿐이라 nil이 퍼지지 않는다. (나) `min`과 같게 원소 하나도 `x < x`.
- 처리: 이 커밋. Rust로 옮긴 `sum`(원소 하나는 `+`가 받는 타입이 아니면 `x + x`의 E0501)·`min`·`max`(원소 하나는 `compare(x, x)`), `sort`·`sort_by`도 키 하나를 `compare(k, k)`로 검사(`Sort::step`). `collections.md`, 결정 D16.

### VA19. fung으로 쓴 prelude의 진단이 구현을 드러낸다(호출 경로의 내부 프레임, `keys`가 "pairs", `chars`만 E0506) — 결정·정리, 낮음
- 재현:
  ```
  # cp.fung: fn top(xs) = reverse(xs) / print(top("abc"))
  error[E0506]: cannot reverse str
   --> cp.fung:1:21
   call path (innermost first):
     extras/prelude.fung:31 _list
     extras/prelude.fung:65 reverse
     cp.fung:1  top
     cp.fung:2  <entry>
  # 같은 모양의 sort(xs)(Rust)는 호출 경로가 cp2.fung:1 top / cp2.fung:2 <entry> 뿐
  $F -e 'max([1, "a"])'   → 호출 경로에 extras/prelude.fung:58 <lambda>, extras/prelude.fung:58 max
  $F -e 'keys([1])'       → E0506 cannot list the pairs of list      (사용자는 pairs를 부르지 않음)
  $F -e 'values(1)'       → E0506 cannot list the pairs of int
  $F -e 'unique("aab")'   → E0506 cannot unique str
  $F -e 'chars(1)'        → E0506 cannot list the characters of int   (다른 문자열 함수는 모두 E0403 argument `s` must be str: byte_len·split·lines·trim·upper·repeat·bytes·valid_utf8)
  ```
- 근거: 08 "런타임 에러는 호출 경로(안쪽부터)를 덧붙인다. 구현 언어의 스택 추적은 보이지 않는다" — 사용자가 열 수 없는 `extras/prelude.fung`의 줄과 `_list`·`_ordered`·`<lambda>`는 사실상 구현의 스택이다.
  open-questions "구현 언어 지침": "구현 언어 이동은 사용자 코드에 영향이 없어 나중에 바꿔도 된다" — 지금은 함수를 Rust↔fung로 옮기면 진단 모양이 바뀐다.
  `docs/notes/M7b-structure.md` 111행·"에러 번호는 같아도 메시지의 동사가 바뀐다" 단락이 알고만 넘긴 것으로, open-questions에도 결정 기록에도 없다. 주 위치는 이미 사용자 프레임으로 옮긴다(M7b), 호출 경로만 남았다.
- 원인 위치: 호출 경로 조립(`crates/fung-vm/src/vm/` 에러에 경로를 붙이는 곳, `Builtin` 표시를 이미 씀), `extras/prelude.fung:121-122` `keys`·`values`(`pairs`의 검사에 기댐), `collections.rs:636` `chars_fn`(E0506), `collections.rs:279` `unique_fn`의 동사 "unique".
- 고치는 법(결정):
  - (가) **추천**: 호출 경로에서 `Builtin` 표시가 된 프레임을 빼고(생략 수만 "…(n internal frames)"로 남기거나 아예 생략), 주 위치를 옮길 때와 같은 규칙을 경로에도 쓴다.
    `keys`·`values`는 첫 줄에서 `_r.items`처럼 자기 동사로 검사하거나(VA25처럼 Rust로 옮기면 저절로), `chars`는 문자열 함수 묶음과 같게 E0403 `argument `s` must be str`(문서 `strings.md` `chars` 에러 줄도), `unique`의 동사는 "take the unique elements of" 같은 문장으로(E0506 동사 정리는 SP24와 함께).
  - (나) 경로는 그대로 두고 08에 "fung으로 쓴 내장 함수의 프레임은 보인다"를 적는다.
- 처리: 이 커밋. 호출 경로가 `builtin` 프레임을 뺀다(`vm.rs` `call_path`, 꼬리 호출로 남긴 사용자 프레임은 그대로). `keys`·`values`는 자기 동사, `unique`는 `take the unique elements of`, `chars`는 E0403 `argument `s` must be str`. 08, `manual/vm/07-diagnostics.md`·`11-modules.md`, `maps.md`·`strings.md`, 결정 D17.

### VA20. `split("", "")`가 `[]`라서 문서의 "언제나 원소가 하나 이상"과 어긋난다 — 결정·문서, 낮음
- 재현:
  ```
  $F -e 'print(split("", ","), split("", ""), len(split("", "")))'   → [""] [] 0
  ```
- 근거: `manual/reference/prelude/strings.md` `split` "돌려주는 값: 문자열의 리스트. **언제나 원소가 하나 이상이다**", 설명 "빈 문자열을 나누면 `[""]`이다. `sep`가 `""`면 글자 단위로 나눈다(`chars`와 같다)" — 세 문장이 `split("", "")`에서 서로 부딪힌다. 코드 주석(`strings.rs:137-138`)은 "splitting the empty string by it gives no parts"로 일부러 `[]`다.
- 원인 위치: `crates/fung-vm/src/prelude/strings.rs:139` `split_fn`(빈 `sep`이면 `chars_of`).
- 고치는 법(결정):
  - (가) **추천**: 빈 구분자를 받지 않는다(E0403 `argument `sep` must be a non-empty str`, `pad`의 빈 문자열과 같은 꼴). 글자로 나누는 길은 `chars` 하나가 되고, `split`은 "원소 수 = 구분자 수 + 1"·"`join(split(s, sep), sep) == s`" 두 성질이 예외 없이 선다. 지침 "예외·특수한 경우 최소화".
  - (나) 지금 동작을 지키고 `strings.md`의 "언제나 하나 이상"에 "`split("", "")`는 `[]`" 예외를 적는다.
- 처리: 이 커밋. `strings.rs` `split_fn`이 빈 `sep`에 E0403(`argument `sep` must be a non-empty str`). 저장소의 `split(s, "")`(`tests/lang/bytes.fung`, 매뉴얼 `strings.md` 예제, 실행 사례)는 `chars`로. `regex.split`의 빈 패턴 설명에서 `split("", "")` 비유를 `chars("")`로. 매뉴얼 `strings.md`·`regex.md`, 06, CHANGELOG, 결정 D18.

### VA21. 범위의 `==`가 표현을 비교해, 같은 수열이 다르다(`0..5 != 0..=4`, 빈 범위끼리도 다름) — 결정, 낮음
- 재현:
  ```
  $F -e 'print(0..5 == 0..=4, 0..0 == 5..1, [0..=4] == [0..5])'   → false false false
  $F -e 'print(unique([0..5, 0..=4]), unique([0..0, 3..1]))'      → [0..5, 0..=4] [0..0, 3..1]
  $F -e 'print(len(0..5) == len(0..=4), map(0..5, x => x) == map(0..=4, x => x))'  → true true
  ```
- 근거: 02 표 "range | 시작·끝·포함 여부 같음"이라 명세대로다. 하지만 02는 범위를 "게으른 정수 범위"(값)로 정의하고, 비교 표의 다른 값 타입은 모두 "값이 같으면 같다"인데 범위만 "쓴 모양이 같으면"이다. 같은 원소를 내는 두 범위가 다르고, 비어 있는 범위가 끝 값에 따라 여럿이 된다.
  (명세 영역도 `0..3 == 0..=2`가 `false`인 것을 의도인지 확인하지 못한 의심으로 남겼다.)
- 원인 위치: `crates/fung-vm/src/value/compare.rs:64` `equal_leaf`(`x == y`, 구조체 비교), `:432` `hash_leaf`(`start, end, inclusive`).
- 고치는 법(결정):
  - (가) **추천**: 원소 열로 비교한다: 둘 다 비면 같고, 아니면 첫 수와 마지막 수(`_r.last`와 같은 i128 계산)가 같으면 같다. 해시도 정규화한 `(first, last)` 또는 빈 범위 표지로. 출력(`0..=4`)은 쓴 모양을 그대로 둔다. 02 표의 range 칸을 "같은 정수 열"로.
  - (나) 지금대로 두고 02·`values.md`에 "범위는 쓴 모양이 같아야 같다(`0..5 != 0..=4`)"를 예와 함께 적는다.
- 처리: 이 커밋(범위 값 규칙). `Range`의 `==`·해시가 `ends()`(첫 수·마지막 수, 빈 범위는 하나)를 견준다. 02 비교 표, `values.md` 비교 표, CHANGELOG, 결정 D19.

### VA22. `==`가 컬렉션을 두 번 훑는다(E0517 검사 + 비교, 11-08 VA8 후속) — 성능, 낮음
- 재현: 서로 다른 객체인 같은 문자열 200만 개 리스트 둘의 `a == b` 10번이 595.5 ms(한 번 59.5 ms). Python 같은 작업 29.0 ms(10번 290 ms), 약 2배.
  ```
  import time
  a = map(0..2_000_000, x => "s${x}")
  b = map(0..2_000_000, x => "s${x}")
  t0 = time.monotonic()
  r = fold(0..10, 0, (acc, i) => acc + ((a == b) then 1 else 0))
  print("eq x10", time.monotonic() - t0)
  ```
- 근거: `eq`는 `sym_meets_str(a, b)?`로 쌍을 첫 차이까지 다 훑고(같은 리스트면 끝까지, 잎마다 `equal` 호출), 다시 `equal(a, b)`로 한 번 더 훑는다. VA8(지난 점검)이 두 걸음을 "같은 방식으로" 멈추게 했지만 합치지는 않았다. 같은 값일수록(흔한 경우: 기대값 비교, `in`, 시험의 `==`) 두 배가 된다. open-questions "알려진 한계"의 "큰 리스트의 `==` 20 ms(200만 정수)"도 이 두 배를 담은 수치다.
- 원인 위치: `crates/fung-vm/src/value/compare.rs:152` `eq`, `:174` `sym_meets_str`, `:16` `equal`.
- 고치는 법: `equal`을 `Result<bool, Diag>`를 내는 한 걸음으로 합친다: 잎 쌍이 `(Sym, Str)`이면 E0517, 아니면 지금 `equal_leaf`. `equal`(패턴용, E0517 없음)은 같은 걷기에 "E0517을 끔" 표지를 준다. 스택·멈춤 규칙(11-08 VA8의 "실제로 견준 쌍")은 그대로라 동작이 바뀌지 않는다.
- 처리: 이 커밋. `compare.rs`의 `walk` 하나가 E0517 켬/끔 표지를 받아 검사와 비교를 한 걸음으로(`eq`는 켬, `equal`은 끔). 측정 `docs/notes/bench-languages.md` 4.7절, 결정 없음(성능).

### VA23. `sort`가 이미 정렬된 입력에서도 n log n 비교를 한다 — 성능, 낮음
- 재현(200만 정수): 이미 정렬된 리스트 `sort` 113.2 ms, 무작위 192.1 ms, 정렬된 리스트 `sort_by(xs, x => x)` 206.1 ms. Python `sorted`는 정렬된 입력 9.0 ms(12.6배 차), 무작위 105.0 ms(1.8배). 200만 문자열 무작위 651 ms(Python 352 ms).
  ```
  import time
  xs = map(0..2_000_000, x => x)                          # 정렬된 입력
  ys = map(0..2_000_000, x => (x * 7919) % 2_000_003)     # 무작위 입력
  zs = map(0..2_000_000, x => "k${(x * 7919) % 2_000_003}")
  t0 = time.monotonic(); a = sort(xs); print("sorted", time.monotonic() - t0)
  t0 = time.monotonic(); b = sort(ys); print("random", time.monotonic() - t0)
  t0 = time.monotonic(); c = sort_by(xs, x => x); print("sort_by sorted", time.monotonic() - t0)
  t0 = time.monotonic(); d = sort(zs); print("strings", time.monotonic() - t0)
  ```
- 근거: 스크립트의 정렬은 이미 거의 정렬된 데이터(로그 시각, 앞서 정렬한 키에 몇 개 더한 것, 디렉터리 목록)가 흔하다. 지금 구현은 너비 1부터 모든 단계를 합치는 상향식 병합이라 입력 모양을 보지 않는다.
- 원인 위치: `crates/fung-vm/src/prelude/collections.rs:144-232` `Sort`(`width: 1`에서 시작, 달리기(run) 검출 없음).
- 고치는 법: 첫 단계에서 이어진 오름차순 달리기(엄격한 내림차순은 뒤집어서 — 안정성 유지)를 찾아 그 경계로 병합하는 "자연 병합 정렬"로 바꾼다. 조각 실행(`Work`)·안정성·비교 예산 구조는 그대로 쓸 수 있고, 정렬된 입력은 n−1번 비교로 끝난다. 짧은 달리기는 32개 정도까지 삽입 정렬로 늘리면 무작위 입력도 단계 수가 5개 준다(timsort의 minrun).
- 처리: 이 커밋. `Sort`가 자연 병합 정렬(달리기 검출·엄격한 내림차순 뒤집기·32개까지 이분 삽입). 정렬된 입력 112 → 28 ms, 무작위 문자열은 득 없음(4.7절), 결정 없음. 시험 `sort_finds_runs_and_stays_stable`·`sort_of_sorted_input_is_linear`.

### VA24. `sort_by`가 원소 벡터를 한 번 더 복사하고, `unique`가 남긴 원소를 두 벌 쥔다(12절 의심) — 성능, 낮음
- 재현(200만 정수, `/usr/bin/time -l` 최대 상주): 리스트만 만들기 79.9 MB, `sort` 147.4 MB, `sort_by(xs, x => x)` 211.9 MB. `sort_by`의 64 MB 차이 중 32 MB(200만 × 16 B)는 열쇠 벡터로 필요하지만 나머지 32 MB는 `xs.clone()`이다.
- 근거: 12절 "`sort_by`가 원소 벡터를 한 번 더 복사한다(`xs: xs.clone()`)" — 확인했다. `unique`도 `out: Vec<Value>`(비교용)와 `list: List`(결과)에 남긴 원소를 둘 다 넣는다(`self.list.push(x.clone()); self.out.push(x)`).
- 원인 위치: `crates/fung-vm/src/prelude/collections.rs:362-371` `sort_by_fn`(`Keys { xs: xs.clone(), … }`와 `Cursor::Items(xs, 0)`), `:279-336` `unique_fn`의 `out`·`list`.
- 고치는 법: `sort_by`는 `Keys::args`에서 받은 `x`를 `self.xs.push(x.clone())`로 모으고 `Cursor::Items`에 원래 벡터를 넘긴다(커서가 원소를 꺼내 주므로 복사 한 벌로 끝). `unique`는 `out`만 두고 끝에서 `List::new(out)`으로 만들거나(마지막 한 걸음이 길어지면 `Sort`처럼 결과를 조각으로 옮김), 반대로 `list`만 두고 비교 때 `list.at(i)`로 읽는다.
- 처리: 이 커밋. `sort_by`는 `Rc<Vec<Value>>` 하나를 커서와 `Sort`가 함께 읽고, `unique`는 `list` 한 벌만 쥔다. 최대 상주 `sort_by` 201 → 156 MB, `unique` 290 → 258 MB(4.7절), 결정 없음.

### VA25. fung으로 쓴 prelude 컬렉션 함수가 같은 일을 하는 Rust 함수보다 몇 배 느리다(`reverse`가 `sort`와 비슷) — 성능, 낮음
- 재현(200만 원소 리스트 `xs = map(0..2_000_000, x => x)` 또는 무작위 `(x * 7919) % 2_000_003` / 100만 키 맵 `m = fold(0..1_000_000, {}, (acc, i) => put(acc, "k${i}", i))`, 각 호출 앞뒤의 `time.monotonic()` 차):
  ```
  reverse(xs)   168.8 ms   (같은 리스트의 sort 192.7 ms; Python xs[::-1] 5.1 ms)
  max(xs)       125.5 ms   (Python 19.3 ms)
  sum(xs)       105.8 ms   (Python 6.9 ms)
  sum(0..2_000_000) 98.5 ms, sum(0..200_000_000) 9.86 s  (닫힌 식이면 O(1))
  pairs(m)       45.2 ms
  keys(m)       113.5 ms   values(m) 111.5 ms   (pairs의 2.5배; Python list(m.keys()) 4.5 ms)
  ```
- 근거: open-questions "구현 언어 지침"은 "핵심이 아닌 조합 → fung"이지만 `reverse`·`keys`·`values`·`sum`·`min`·`max`는 원소마다 람다 호출(약 50~80 ns)과 `ys[n-1-i]`의 O(log n) 인덱스를 쓴다. `keys`·`values`는 튜플 리스트를 만든 뒤 다시 훑어 버린다. `min`·`max`는 범위를 끝에서 답하면서(지난 점검 VA4 고침) `sum`은 범위를 끝까지 접는다.
- 원인 위치: `extras/prelude.fung:64` `reverse`, `:121-122` `keys`·`values`, `:21` `sum`, `:49-60` `min`·`max`.
- 고치는 법: `reverse`·`keys`·`values`를 `pairs`처럼 Rust `Work`로 옮긴다(결과 리스트를 제자리로 키우는 패턴이 이미 있다; VA19의 진단 문제도 함께 사라짐). `sum`의 범위는 i128 닫힌 식 `n*(first+last)/2`(int를 넘으면 E0503, 지금 `+`와 같은 에러). `min`·`max`·`sum`은 Rust로 옮기면 VA18의 원소 하나 문제도 `compare`/`add`로 한 번에 풀린다. 0.5.0 결정처럼 "fung에 둔다"를 지킨다면 최소한 `reverse`를 `_r` 다리의 한 함수로. `reverse`의 문자열 받기(SP23)도 함께 정한다.
- 처리: 이 커밋. `reverse`·`keys`·`values`·`sum`·`min`·`max`를 `prelude/collections.rs`의 Rust 함수로(조각 실행: `pieces::run`·`drain_with`), 범위의 `sum`은 i128 계산이되 `+` 접기와 같은 넘침 판정(중간 합의 최솟값은 꼭짓점 옆, 최댓값은 끝; 리뷰 고침). 측정 `docs/notes/bench-languages.md` 4.6절, 레퍼런스의 "fung으로 작성" 표시를 뗌.

### VA26. 1억 바이트를 넘는 비ASCII 문자열은 그것을 돌려주는 내장 함수 호출마다 글자 수를 다시 센다(`get`도, 11-08 VA9 후속) — 성능, 낮음
- 재현(한글 4천만 자 = 120 MB, 이미 `len(s)`로 센 뒤):
  ```
  20 × byte_len(str(s))           1347 ms   (호출당 67 ms)
  20 × byte_len(get({k: s}, "k")) 1347 ms   (맵에서 꺼내기만 하는데 67 ms)
  20 × byte_len(trim(s))          1354 ms   (trim은 아무것도 자르지 않고 s를 그대로 돌려줌)
  같은 길이 ASCII 문자열의 trim 20번 0.01 ms
  ```
  ```
  import time
  s = repeat("가", 40_000_000)
  print(len(s))
  t0 = time.monotonic()
  n = fold(0..20, 0, (a, i) => a + byte_len(str(s)))
  t1 = time.monotonic()
  m = fold(0..20, 0, (a, i) => a + byte_len(get({k: s}, "k")))
  t2 = time.monotonic()
  k = fold(0..20, 0, (a, i) => a + byte_len(trim(s)))
  print("str(s) x20", t1 - t0, "get x20", t2 - t1, "trim x20", time.monotonic() - t2)
  ```
- 근거: VA9(지난 점검)가 모든 내장 함수의 반환값을 `check_returned`로 재게 했고 02 "크기 한도"는 "바이트 길이가 한도 안이면 길이 비교 한 번이다"라고 적었다. 바이트가 한도를 넘으면 `chars_fit`이 `fung_front::text::char_count`로 원본 바이트를 매번 센다. `Text`에는 이미 센 글자 수가 기억되어 있는데(`Text::char_len`, (11-08 VA12) 고침) 쓰지 않는다. 값을 그대로 넘겨주는 `get`·`first`·`find`·`str(s)`까지 O(n)이 된다.
- 원인 위치: `crates/fung-vm/src/value/arith.rs:81` `chars_fit`, `:101` `check_returned`(`s.as_bytes()`를 넘김), `:114` `add`의 문자열 갈래.
- 고치는 법: `check_returned`·`check_chars`가 `&Text`를 받아 `char_len()`(기억된 값)을 쓴다. `+`는 두 쪽의 `char_len()`을 더해 판정하고, 결과 `Text`에 그 합을 미리 기억시킨다(UTF-8 두 개를 이으면 글자 수는 합이다; 바이트 문자열이면 경계에서 합쳐질 수 있으니 기억하지 않음). 그러면 한 문자열은 평생 한 번만 센다.
- 처리: 이 커밋. `check_returned`·`+`가 `Text`의 기억된 글자 수를 쓰고 `+`의 결과에 합을 기억시킴(`Text::concat`). 한글 4천만 자 `trim` ×20 1340 ms → 0.007 ms(4.7절), 결정 없음. 시험 `a_wide_string_is_counted_once`.

### VA27. 쓰이지 않는 `nat::str_args`, 두 곳에서 정의되는 `args` — 정리, 낮음
- 재현(코드 읽기): `grep -rn str_args crates/` → 정의(`crates/fung-vm/src/native/nat.rs:340`) 하나뿐. `pub`이라 컴파일러 경고가 나지 않는다.
  `args`는 `crates/fung-vm/src/prelude.rs:36` `core()`가 빈 리스트로 넣고, `crates/fung-vm/src/boot.rs:47-58` `native_prelude`가 같은 이름으로 다시 넣어 덮는다.
- 근거: CLAUDE.md "불필요한(죽은·중복) 코드" 관점.
- 원인 위치: 위와 같다.
- 고치는 법: `str_args` 삭제. `core()`에서 `args`를 빼고 `native_prelude` 한 곳에서만 넣는다(`core()`를 쓰는 다른 호출자 — 시험·`spec_file` — 가 `args`를 기대하는지 확인; `spec/builtins.txt`의 `prelude native args` 줄은 `native_prelude`에서 나오게).
- 처리: 이 커밋. `nat::str_args` 삭제, `prelude::core()`에서 `args`를 빼 `boot::native_prelude` 한 곳에서만 넣음. 결정 없음.

## 8. 동시성 런타임 (CC)

### CC9. 취소·기한이 읽는 쪽을 끊으면, 이미 끝난 자식의 `Err`가 E0607 런타임 에러가 된다(`timeout`·`cancel(s)`·`conc.pmap`) — 결정, 높음
- 재현 1(`e1.fung`):
  ```
  r = timeout(() => scope(fn(s):
    t = spawn(s, () => Err("bad input"))
    sleep(1s)
    await(t)
  ;;), 50ms)
  print("timeout gave", r)
  ```
  실제: `error[E0607]: task's Err was never awaited: "bad input"` (위치 `spawn(`, `note: scope opened at e1.fung:1`), 종료 코드 1. 기대: `timeout gave Err({kind: 'timeout, …})`.
  `cancel(s)`도 같다: 자식이 `Err`로 끝난 뒤 본문이 `cancel(s)` → `Err({kind: 'cancelled})` 대신 E0607, 종료 코드 1.
  ```
  r = scope(fn(s):
    t = spawn(s, () => Err("bad input"))
    sleep(20ms)
    cancel(s)
    await(t)
  ;;)
  print("scope gave", r)
  ```
- 재현 2(실제 쓰임): 호스트 하나가 느리고 둘이 `Err`를 돌려주는 `check`를 `timeout(() => conc.pmap([1, 2, 3], check), 50ms)`로 부르면
  `error[E0607]: task's Err was never awaited: "down: 3"` + `note: also failed: … "down: 2"`, 종료 코드 1. `pmap` 결과에 `Err`가 섞이는 것은 문서가 허락하는 정상 결과인데(`modules/conc.md` "f가 돌려준 Err는 결과"), 바깥에 기한을 걸면 프로그램이 죽는다.
  ```
  import conc
  fn check(h):
    if h == 1:
      sleep(1s)
      return Ok(h)
    ;;
    Err("down: ${h}")
  ;;
  r = timeout(() => conc.pmap([1, 2, 3], check), 50ms)
  print("pmap under timeout:", r)
  ```
- 재현 3(`supervise`): 같은 모양을 `supervise`로 감싸면 `task failed: error[E0607] …`이 표준 에러에 찍히고 종료 코드 1(`timeout`은 `Err timeout`을 돌려주지만 실패로 셈).
  ```
  r = timeout(() => supervise(fn(s):
    t = spawn(s, () => Err("bad input"))
    sleep(1s)
    await(t)
  ;;, e => print("on_fail", e.code)), 50ms)
  print("timeout gave", r)
  ```
- 실행 순서에 달린다: 자식이 기한 **뒤에** `Err`를 돌려주게(`sleep(100ms)` 뒤) 바꾸면 자식은 취소되어 아무 일도 없다(`timeout gave Err(timeout)`, 종료 코드 0). 같은 프로그램이 자식이 기한 전에 끝났느냐에 따라 죽거나 산다.
  ```
  r = timeout(() => scope(fn(s):
    t = spawn(s, fn(): sleep(100ms); Err("bad input") ;;)
    sleep(1s)
    await(t)
  ;;), 50ms)
  print("timeout gave", r)
  ```
- 근거:
  - 05:120 "값의 수명이 결정적이라 실패인지 아닌지는 실행 순서와 상관없이 같고, 알리는 때만 다르다" — 위처럼 순서에 따라 실패 여부가 갈린다.
  - 05:203 "`timeout(f, d)`는 … `d`가 지나면 `f` 안의 모든 것을 취소한 뒤 `Err({kind: 'timeout})`을 준다", 05:129 "`cancel(s)`로 취소된 범위는 `scope(`에서 `Err({kind: 'cancelled})`를 돌려주고 호출한 쪽은 이어 간다".
  - 05:164 "취소된 태스크는 실패가 아니다", 05:125 "취소·시간 초과·교착으로 끝난 태스크의 `Err`는 … 세지 않는다": 취소로 포기한 결과를 실패로 세지 않는다는 취지와 어긋난다.
  - 사용자가 피할 길이 없다: 기한 안에서 모든 `Err`를 미리 읽을 수는 없으니, 기한을 거는 코드의 태스크는 결국 `nil`로 맺어 `Err`를 버려야 한다(원칙 1을 거꾸로 강요).
- 원인 위치: `crates/fung-vm/src/vm/task.rs` `TaskHandle::drop`(251~260) → `vm/scope.rs` `ScopeState::unread_lost`(212~216, 열려 있으면 무조건 `lose`), `ScopeState::lose`(221~231), `Scope::closed`(594~603, 닫힐 때 남은 `unread`를 무조건 `lose`).
  범위의 노드가 취소되어 본문이 되감기는 중에 핸들이 놓여도 똑같이 실패로 센다.
- 고치는 법(결정):
  - (가) **추천**: 범위가 **취소로** 되감기는 중(범위 노드가 취소되었고 그 원인이 자식의 실패가 아님: `cancel(s)`, 바깥 취소, 기한, `exit`)에 읽을 수 없게 된 `Err`는 실패로 세지 않는다. `lose`·`closed`에서 `self.node.is_cancelled() && self.failure.borrow().is_none()`이면 `Unread`를 버린다(`supervise`는 줄에 넣지 않음).
    이유: 취소는 "그 결과를 더는 원하지 않는다"는 프로그램의 결정이고, 취소된 태스크의 `Err`를 세지 않는 규칙(05:125·164)과 같은 선이다. 순서와 무관해진다(05:120). `FailFast`가 자식 실패로 취소된 경우는 지금처럼 `note: also failed`로 남겨도 된다.
    범위가 취소되지 않았는데 읽는 태스크만 취소된 경우(`cancel(t)`로 읽을 태스크만 끊음)는 지금처럼 E0607로 둔다(범위는 계속 돌고, 그 `Err`는 정말 버려졌다).
  - (나) 지금대로 두고 05·매뉴얼에 "기한·취소 안의 태스크는 `Err`를 돌려주면 안 된다(`nil`로 맺어라)"를 적는다. `conc.pmap`+`timeout`이 쓸 수 없게 되어 비추천.
  - 시험: 위 재현 다섯(`scope`·`supervise`·`cancel(s)`·`pmap`·기한 뒤 `Err`)이 모두 `Err timeout`/`Err cancelled`, 종료 코드 0. 그리고 취소 없는 E0607 세 경우(05:117~119)는 그대로.
- 처리: 이 커밋(W1, `ScopeState::given_up`: 취소로 되감기는 범위에서 읽을 수 없게 된 `Err`는 버림; 본문 자신의 에러·자식 실패는 그대로), 05 "읽지 않은 `Err`", 매뉴얼 `reference/prelude/concurrency.md`, CHANGELOG, 결정 D1.

### CC10. 표준 입력(과 파일 핸들)의 읽기 에러가 "입력의 끝"으로 바뀐다: 비차단 표준 입력은 빈 입력이 된다 — 버그, 중간
- 재현 1(O_NONBLOCK 표준 입력, 부모가 비차단으로 둔 파이프·터미널을 물려받는 흔한 경우):
  ```
  echo 'print(read_lines())' > rl2.fung
  (sleep 0.3; printf 'a\nb\n') | perl -e 'use Fcntl; my $f = fcntl(STDIN, F_GETFL, 0); fcntl(STDIN, F_SETFL, $f | O_NONBLOCK); exec @ARGV' $F rl2.fung
  ```
  실제: `Ok([])`, 종료 코드 0 (첫 `read`가 EAGAIN이라 끝으로 봄). 같은 입력을 차단 모드로 주면 `Ok(["a", "b"])`. 입력을 조용히 잃는다.
- 재현 2: `i1.fung`(`print(read_line())` / `print(read_lines())`)을 `$F i1.fung < /tmp`로(표준 입력이 디렉터리, `read`가 EISDIR) → `read_line()`이 `Ok(nil)`, `read_lines()`가 `Ok([])`, 종료 코드 0.
- 재현 3(파일 핸들, 같은 원인): `f = fs.open("/tmp")!` 뒤 `read_line(f)` → `Ok(nil)`. 읽기 에러가 줄 중간에 나면 그때까지 읽은 조각을 온전한 줄로 돌려준다(코드 읽기).
- 근거: 매뉴얼 `reference/prelude/io.md` `read_line` "에러": "그 밖의 읽기 실패는 `fs`의 다른 함수와 같은 `Err`(`'io` 등)". 원칙 1(조용한 실패 없음).
- 원인 위치:
  - `crates/fung-vm/src/vm/stdin.rs` `Pump::read_on_demand`(186~190): `Err(e) if Interrupted => continue`, **`Err(_) => true`**(끝). `WouldBlock`도 여기로 간다.
  - `crates/fung-vm/src/native/file.rs` `File::read_line`(105~117): `Err(_) if line.is_empty() => Ok(None)`, `Err(_) => Ok(Some(line))`. 머리 주석이 "A read error ends the input"이라고 의도로 적었지만 매뉴얼과 어긋난다.
- 고치는 법:
  - 표준 입력: `WouldBlock`이면 그 fd를 `poll(POLLIN)`으로 기다린 뒤 다시 읽는다(읽기 스레드 안이라 막혀도 됨; 파일 상태 플래그는 부모와 공유하므로 O_NONBLOCK을 끄지 않는다).
    그 밖의 에러는 줄 대기열에 에러 표시(`Err(io::Error)`)를 넣고, `read_line`·`read_lines`가 `Err({kind: 'io, message})`로 돌려준다(그 앞까지 읽은 줄은 먼저 준다). 다음 읽기는 다시 시도.
  - 파일 핸들: 에러를 `Fault::Io`로 돌려준다. 줄 중간의 에러면 조각을 핸들 버퍼에 남기고 에러를 준다(조각을 줄로 내주지 않음). `fs.open`이 디렉터리를 여는 것 자체는 표준 라이브러리 쪽 판단(열 때 `'invalid`로 거절할지).
  - 시험: 위 세 재현이 `Err({kind: 'io, …})`(1은 기다렸다가 `Ok(["a", "b"])`).
- 처리: 이 커밋(W1, 표준 입력 대기열이 `Input::Failed`를 실음·`blocking_reads`가 비차단 표준 입력을 `poll(2)`로 기다림(읽기 스레드 안, O_NONBLOCK은 끄지 않음)·파일 핸들은 `Reading`이 조각을 남기고 `Fault::Io`), 매뉴얼 `reference/prelude/io.md`, CHANGELOG. 이벤트 루프 대신 읽기 스레드에서 기다리는 쪽(05·R7-scheduler의 "읽기 스레드는 막혀도 됨"과 같음).

### CC11. `swap` 함수 안의 차례: 05와 VM 매뉴얼이 어긋나고, 한 차례 몫이 끝나면 그 뒤의 모든 호출이 `end_turn`(시계·잠금)을 탄다 — 성능, 낮음
- 재현(`spin`은 꼬리 재귀 1,000만 번):
  ```
  a = atom(0)
  fn spin(n) = n > 10000000 then n else spin(n + 1)
  spin(0)                                            # 403.7 ms
  swap(a, x => spin(x))                              # 472.4 ms (+17%)
  _ = timeout(() => swap(a, x => spin(0)), 1h)       # 554.4 ms (+37%)
  ```
  (각 줄 앞뒤의 `time.monotonic()` 차.) `swap` 안에서는 첫 1,000번 뒤 `budget`이 1로 남아 호출마다 `end_turn`(차가운 경로)에 들어가고, 거기서 `apply_in_swap`(Ready 뮤텍스 잠금)과, 기한이 있으면 `Instant::now()`를 매번 한다.
  또 3,000만 번 도는 `swap` 함수가 1.4초 동안 다른 모든 태스크를 세운다(다른 태스크의 `sleep(5ms)`가 1.40초에 깸). 기한·신호는 지킨다.
  ```
  import time
  a = atom(0)
  fn spin(n) = n > 30000000 then n else spin(n + 1)
  t0 = time.monotonic()
  scope(fn(s):
    spawn(s, fn(): sleep(5ms); print("other task ran at", time.monotonic() - t0) ;;)
    swap(a, x => spin(x))
    print("swap done at", time.monotonic() - t0)
  ;;)
  ```
- 근거:
  - 05:263~264 "`f` 도중에 차례가 넘어가 다른 태스크가 바꿔도 갱신이 사라지지 않고" ↔ 매뉴얼 `vm/12-scheduler.md:214`·`R7-scheduler.md:224`·`atom.rs` 머리 "swap 안에서는 차례를 넘기지 않는다". 05는 일어나지 않는 일을 근거로 든다.
  - 긴 `swap` 함수가 런타임 전체를 세우는 것은 open-questions "알려진 한계"의 "한 번에 도는 연산"에 없다(서버의 핸들러 하나가 모든 연결을 세울 수 있음).
- 원인 위치: `crates/fung-vm/src/vm/block.rs` `Task::end_turn`(618~642, `pure > 0`이면 `budget = 1`), `vm/atom.rs` `Swap::resume`/`unwind`.
- 고치는 법:
  - 성능: `swap` 안에서 몫이 끝나면 `budget`을 `YIELD_EVERY`로 다시 채우고 "넘길 차례가 밀림" 표시(`turn_owed`)만 세운다. `Swap::resume`/`unwind`에서 `pure`가 0으로 돌아올 때 표시가 있으면 `turn_due = true`. 주석이 걱정한 "몫을 나누어떨어지는 반복이 영영 차례를 주지 않음"은 표시가 막는다. 신호·기한 확인은 몫마다(1,000번) 그대로.
  - 문서: 05:263~264를 "M:1에서는 `f`가 한 차례 안에 끝난다(넘기지 않음). 비교 후 교체는 여러 코어에서도 같은 의미를 지키려는 것"으로 고치고, 알려진 한계에 "`swap`의 함수는 끝날 때까지 다른 태스크를 세운다(기한·신호는 봄)"를 더한다.
- 처리: eab6367과 그 고침 커밋(W1, `turn_owed`: `swap` 안의 몫 끝은 예산을 다시 채우고 빚만 남겨 `swap`이 끝날 때 넘김; 다시 시도 사이에는 넘기지 않음), 05 "공유 상태", 매뉴얼 `vm/12-scheduler.md`, open-questions "알려진 한계", CHANGELOG, 측정 `docs/notes/bench-languages.md` 4.5절.

### CC12. `wait_closed`로 기다리는 쪽이 그 채널의 보내는 끝을 쥐고 있어도 교착 보고가 그렇게 말하지 않는다(L3·L7 후속) — 버그, 낮음
- 재현:
  ```
  (tx, rx) = channel()
  wait_closed(rx)
  ```
  실제: `error[E0702]: deadlock: all tasks are waiting` + 위치와 호출 경로뿐. 같은 모양의 `for x in rx:`(아래)는 `help: this task waits to receive, but it still holds that channel's sending end itself (the global `tx`), … close it with `close(tx)` first …`를 붙인다.
  ```
  (tx, rx) = channel()
  for x in rx:
    print(x)
  ;;
  ```
  `wait_closed`는 "닫힐 때까지" 기다리므로 자기가 보내는 끝을 쥔 것이 곧 교착의 원인인데, 가장 필요한 곳에 도움말이 빠졌다(다른 태스크 줄의 `, holds its own sending end`도 같음).
- 근거: 05:188~190 "받으려고 기다리는 태스크가 같은 채널의 보내는 끝을 쥐고 있으면 … `help:`" (L7). `wait_closed`는 L3에서 생겼고 그 판정에 들어가지 않았다.
- 원인 위치: `crates/fung-vm/src/conc.rs` `wait_closed`(403~416, `park_until_then`의 `Parked`라 `Cont::receives_on`이 없음), `vm/deadlock.rs` `Task::own_sender`(253, `receives_on()`만 봄).
- 고치는 법: `wait_closed`를 `receives_on`을 돌려주는 작은 상자(`RecvWait`처럼 끝을 쥠)로 바꾸거나, `Parked`에 선택적 "기다리는 받는 끝"을 실어 `receives_on`이 그것을 돌려주게 한다. 05:188의 "받으려고"를 "받거나 닫히기를 기다리는"으로 고친다. 시험: `deadlock_tests`에 `wait_closed` 사례.
- 처리: 이 커밋(W1, `wait_closed`가 `ClosedWait` 상자로 `receives_on`을 줌, 도움말 "waits for that channel to close"), 05 교착 절, 매뉴얼 `reference/prelude/concurrency.md`, CHANGELOG.

### CC13. 문서·주석이 구현과 어긋난다(스펙의 `timeout` 인자 순서, `channel.rs`·`stdin.rs` 머리 주석) — 문서, 낮음
- 스펙 `docs/superpowers/specs/2026-10-18-concurrency-v2-r7-design.md`(CLAUDE.md가 "구속력 있음"이라 함) 65·135·140·297행이 `timeout(d, f)`·`timeout(1s, () => recv(rx))`. SP10(2026-11-09)에서 `timeout(f, d)`로 바뀌었는데 스펙에 처리 표시가 없다. 05·매뉴얼은 `timeout(f, d)`.
- `crates/fung-vm/src/vm/channel.rs` 머리 주석 끝: "Two channels holding each other's ends are not caught." — L7(2026-10-28)부터 E0709로 잡는다(같은 파일의 `cycle_to`).
- `crates/fung-vm/src/vm/stdin.rs` 머리 주석: "What the program never asked for stays in the input: a command after fung in a shell reads it". 실제로는 한 번에 64 KiB 덩어리를 가져간다:
  `rl.fung`가 `print("fung got", read_line())`일 때 `printf 'a\nb\nc\n' | { $F rl.fung; cat; }` → fung이 `a`를 읽고 `cat`은 아무것도 받지 못한다(파일 리다이렉트도 같음). 06:89~90("fung이 읽어 간 버퍼 뒤부터 받는다")이 맞고 주석이 틀렸다.
- (`read_lines`의 에러 문서가 `'invalid`라고 하는 것은 같은 원인의 LB40으로 옮겼다.)
- 고치는 법: 스펙 해당 행에 "SP10에서 `timeout(f, d)`로 바뀜" 표시(또는 고쳐 씀). 두 주석을 지금 동작대로 고침.
- 처리: 이 커밋(W1, 스펙의 `timeout(f, d)`와 SP10 표시, `channel.rs`·`stdin.rs` 머리 주석).

## 9. 표준 라이브러리 (LB)

### LB23. TLS 연결이 close_notify 없이 끊겨도 "연결 닫힘까지" 본문을 온전한 응답으로 받는다 — 버그(보안), 중간
- 재현(0절의 `tlssrv.py`와 `tls/`):
  ```
  # 답을 보낸 뒤 SSL 종료 없이 프로세스를 끝내는 서버(os._exit: TCP FIN만 간다)
  python3 -I tlssrv.py 23524 hard 'HTTP/1.1 200 OK\r\n\r\npartial-body' &
  SSL_CERT_FILE=tls/ca.pem $F -e 'import http
  print(http.get("https://localhost:23524/", timeout: 3s))'
  ```
  실제: `Ok({status: 200, headers: {}, body: "partial-body"})`. 기대: `Err 'network`(잘린 본문). 같은 서버가 `Transfer-Encoding: chunked` 본문을 끝내지 않고 끊으면 `connection closed`로 `Err`가 된다(틀이 끝을 알려 주므로).
- 근거: RFC 8446 6.1(close_notify 없는 끝은 잘림일 수 있다), RFC 9112 9.8("a client … SHOULD treat an incomplete close as an incomplete message when the message has no length"). 06 "약속"의 "응답 본문은 `Content-Length`, chunked, 연결 닫힘까지 중 하나로 읽는다"가 셋째 경우의 TLS 끝을 말하지 않는다.
  `native/tls.rs` 머리 주석의 이유("HTTP knows where its bodies end")는 길이도 청크도 없는 본문에는 맞지 않는다: 그때는 TCP FIN 하나(경로 위의 공격자가 넣을 수 있다)가 본문의 끝이 된다.
- 원인 위치: `crates/fung-vm/src/native/tls.rs` `fill`(231~, 258~264: 레코드 사이의 `UnexpectedEof`를 `Ok(0)`으로), 이를 그대로 쓰는 `native/net.rs` `read_all`(706)·`read_all_fn`(1634), `extras/http.fung` `_until_close`(994).
- 고치는 법: 연결이 "close_notify로 끝났는지"를 기억해(`Session`에 한 비트) `read_all`이 TLS에서 close_notify 없이 끝나면 `Err 'network`(`connection closed without TLS close_notify`)를 주게 한다. `read_line`·`read_bytes`처럼 길이를 아는 읽기는 지금 그대로(레코드 사이의 끝 = 끝) 둔다. 시험: `tls_tests`에 "끊긴 TLS + 길이 없는 본문은 Err".
  (길이를 아는 응답에는 영향이 없어야 하지만, close_notify 없이 닫는 실제 사이트로는 확인하지 않았다: 12절.)
- 처리: 이 커밋(W10a). `tls::Session`이 close_notify로 끝났는지(`clean`)를 기억하고 `Conn::read_all`이 TLS에서 close_notify 없이 끝나면 `Err 'network`(``connection closed without TLS close_notify``); `read_line`·`read_bytes`·`read_bytes_into`는 그대로(레코드 사이의 TCP 끝 = 끝). 시험 `tls_tests` `reading_until_close`(둘째 경우가 `Err`)·`a_body_to_the_end_needs_close_notify`(길이 없는 https 본문: TCP 끝은 `Err`, close_notify·`Content-Length`는 `Ok`). `tls.rs` 머리 주석, 06, 매뉴얼 `http.md`, CHANGELOG(보안).

### LB24. 리다이렉트의 `Referer`가 조각(`#…`)까지, 그리고 다른 origin에도 쿼리 전체를 싣는다(11-08 LB2 후속) — 버그·결정(보안), 중간
- 재현:
  ```
  python3 -I canned.py 23550 'HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:23551/landing\r\nContent-Length: 0\r\n\r\n' &
  python3 -I canned.py 23551 'HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok' &
  $F -e 'import http
  _ = http.get("http://127.0.0.1:23550/cb?access_token=SECRET#frag", headers: {Authorization: "Bearer x"})'
  ```
  두 번째 서버(다른 포트 = 다른 origin)가 받은 요청: `GET /landing` … `Referer: http://127.0.0.1:23550/cb?access_token=SECRET#frag`. `Authorization`은 LB2대로 빠졌지만 쿼리의 비밀값과 조각은 간다.
- 근거: RFC 9110 10.1.3 "The Referer field MUST NOT include the fragment and userinfo components" — 조각은 명세 위반(버그). 쿼리는 결정 문제다: LB2가 "origin을 떠나면 비밀값을 버린다"를 정했는데 같은 리다이렉트에서 URL의 비밀값(OAuth 콜백의 `code`·`access_token`, 서명된 URL)은 자동 `Referer`로 새 origin에 간다. 브라우저의 기본 정책(strict-origin-when-cross-origin)은 다른 origin에 origin만 보낸다.
- 원인 위치: `extras/http.fung` `_hop_headers`(212~222: `_url_string(put(prev, "user", nil))`), `_url_string`(1315)·`_url_tail`(1332)이 조각을 붙임.
- 고치는 법: (버그) `Referer`는 조각을 뺀 URL로. (결정) 선택지 a) origin을 떠나면(`_leaves_origin`이 참인 `strip`) `Referer`를 origin만(`http://host:port/`)으로, b) 아예 자동 `Referer`를 보내지 않음(curl·Python requests처럼; 사용자가 원하면 헤더로), c) 지금대로.
  **추천 b**: 자동 `Referer`는 클라이언트 라이브러리가 할 일이 아니고(브라우저 흉내), 원칙 "예외 최소화"로 규칙 하나(https→http 예외, 사용자 값 우선)를 통째로 없앤다. a도 LB2와 같은 경계를 쓰므로 무난하다.
- 처리: 이 커밋(W10a). `_hop_headers`의 자동 `Referer`를 없앰(조각 문제도 함께 사라짐; 사용자가 준 `Referer`는 다른 헤더처럼 그대로). 시험 `http.rs` `redirects_add_no_referer`, 기대 출력 `redirects.txt`·`tunnel.txt`의 `Referer` 줄이 빠짐. 06, 매뉴얼 `http.md`, CHANGELOG(보안), 결정 D23.

### LB25. 잘못된 프록시 환경 변수가 말없이 무시되어 직접 연결하거나, 엉뚱한 호스트 이름으로 바뀐다 — 버그, 중간
- 재현(`canned.py`를 23502에 띄움, URL 호스트는 루프백 규칙을 피하려고 `localhost.`):
  ```
  HTTP_PROXY='http://%zz'      $F -e 'import http
  print(http.get("http://localhost.:23502/x", timeout: 2s))'
  → Ok({status: 200, …})            # 프록시를 건너뛰고 서버에 바로 갔다
  HTTP_PROXY='http://a b:3128' …  → Err(… "proxy `http:`: no such host")
  HTTP_PROXY='http://[::1:3128' … → Err(… "proxy `http:`: no such host")
  ```
  기대: 셋 다 프록시 변수가 잘못됐다는 `Err 'network`(``invalid HTTP_PROXY `http://%zz`: …``). 지금은 첫째가 프록시를 말없이 우회하고(프록시를 거쳐야만 나갈 수 있는 망·감사 정책을 조용히 어김), 둘째·셋째는 `"http://" + s`로 다시 읽은 결과 호스트가 `http:`가 되어 이유가 엉뚱하다.
- 근거: 배경 원칙 1(조용한 실패 없음). L13이 "`http`·`https` 밖의 스킴인 프록시"를 에러로 만든 것과 같은 이유(잘못 적은 프록시는 에러)인데, 해석에 실패한 값만 "프록시 없음"이 된다. Go `ProxyFromEnvironment`도 `invalid proxy address`를 돌려준다.
- 원인 위치: `extras/http.fung` `_parse_proxy`(424~440: 다시 읽기에도 실패하면 `first ?? nil` → `nil`, 다시 읽기는 스킴이 있는 값에도 시도).
- 고치는 법: `"http://"`를 붙여 다시 읽는 것은 `://`가 없는 값에만 하고, 어느 쪽이든 해석에 실패하면 `_proxies`가 `Err`를 담아 그 프록시를 쓸 요청이 ``invalid HTTPS_PROXY `…`: <이유>``로 실패하게 한다(`NO_PROXY`로 빠지는 요청은 영향 없음).
  함께: 변수의 "빈 값" 규칙이 둘이다 — 프록시 변수는 빈 값 = 없음인데 `SSL_CERT_FILE=`(빈 값)은 "있음"이라 ``cannot read SSL_CERT_FILE ``: no such file or directory``가 된다(`native/tls.rs` `config` 380~, 크레이트가 정함). 06에 한 줄로 어느 쪽인지 적거나 빈 값을 없음으로 맞춘다(추천: 맞춘다).
- 처리: 이 커밋(W10a). `_parse_proxy`가 `://`가 없는 값에만 `http://`를 붙여 한 번 읽고, 읽지 못하거나 호스트 이름이 없으면(`http://`, `http://:3128`) `Err("invalid <읽은 변수 이름> `…`: <이유>")`를 `_proxies`에 담는다. `_proxy_for`가 CGI·`NO_PROXY`(루프백) 판정 뒤에 그 `Err`를 돌려줌(프록시를 쓰지 않는 요청은 영향 없음). 검토 1차 고침: 문구 속 값의 비밀번호를 `***`로(`_hide_proxy_password`, 읽히지 않는 값·스킴 없는 값도), 스킴 뒤에 `//`가 없는 값(`http:/proxy:3128`)은 ``missing `//` after `http:` ``. 시험 `http.rs` `an_invalid_proxy_is_an_error`. `SSL_CERT_FILE=`(빈 값)은 맞추지 않고 06에 적음: rustls-native-certs가 변수를 직접 읽고 운영체제 저장소만 읽는 공개 함수가 없어, 빈 값을 "없음"으로 보려면 환경을 바꾸거나(`remove_var`는 2024판에서 `unsafe`) 크레이트의 플랫폼 로더를 다시 만들어야 한다. 06, 매뉴얼 `http.md`, CHANGELOG.

### LB26. 같은 이름 헤더를 `", "`로 이어 `Set-Cookie`(그리고 서버의 `Cookie`)가 깨진다 — 결정, 중간
- 재현:
  ```
  canned 'HTTP/1.1 200 OK\r\nSet-Cookie: a=1; Expires=Wed, 21 Oct 2026 07:28:00 GMT\r\nSet-Cookie: b=2\r\nContent-Length: 0\r\n\r\n'
  → "set-cookie": "a=1; Expires=Wed, 21 Oct 2026 07:28:00 GMT, b=2"     # 어디서 나눌지 알 수 없다
  서버: 'Cookie: a=1\r\nCookie: b=2' → req.headers.cookie == "a=1, b=2"   # 쿠키 구분자는 "; "
  ```
- 근거: RFC 9110 5.3("Set-Cookie … often appears in a response message across multiple field lines and does not use the list syntax … the recipient MUST NOT combine"), RFC 6265 4.1·5.4. 원칙 1: 값이 말없이 되돌릴 수 없게 바뀐다. 쿠키 처리 기능은 open-questions "M8 후속"의 보류지만, 지금은 사용자가 직접 읽는 길도 막혀 있다.
- 원인 위치: `extras/http.fung` `_header_map`(885), `_group`(882).
- 고치는 법: a) `set-cookie`만 리스트 값으로(맵 값의 타입이 이름에 따라 달라짐), b) 응답에 `header_lines: [(name, value)]`(받은 순서 그대로)를 더하고 `headers`는 지금대로, c) `Set-Cookie`가 둘 이상이면 `"\n"`으로 잇기(값에 나올 수 없는 글자). **추천 b**: 특수한 이름 규칙 없이 정보를 잃지 않는다. 서버 `req`에도 같은 필드를 두면 `Cookie`·반복 헤더 문제가 함께 풀린다. 문서 06·`http.md`.
- 처리: 이 커밋(W10b). 클라이언트 응답과 서버 `req`에 `header_lines: [(name, value)]`(받은 순서, 이름은 받은 대소문자 그대로; `_net.read_fields`가 이름을 표준 대소문자로 바꾸지 않게 하고, 안쪽 맵 `_group`은 소문자 키). `headers` 맵은 그대로 `", "`로 이음. 시험 `http.rs` `a_response_keeps_its_headers_as_received`·`the_handler_gets_the_request_as_received`, 기대 출력 `http/*.txt`·`tls_tests`. 06, 매뉴얼 `http.md`·13장·`check-urls`·`json-fields`, CHANGELOG, 결정 D24.

### LB27. 서버의 `req.query`가 반복 키·잘못된 이스케이프·`;`가 든 쌍을 말없이 버리고, 원래 요청 대상은 핸들러에 없다 — 결정, 중간
- 재현(`srv.fung`, `raw`):
  ```
  GET /a%2Fb?x=1&x=2&y=%zz&z=1;2  → p: "/a/b", q: {x: "1"}        # x=2, y, z가 사라짐; %2F와 / 구분 불가
  GET /a?k=%zz&k=2                 → q: {k: "2"}                    # "첫 값"이 잘못된 쌍을 건너뛴 둘째 값
  GET /a%zz                        → 400 Bad Request               # 같은 잘못이 경로면 400
  ```
- 근거: 06 "`query`는 … 이름마다 첫 값, … 잘못된 `%` 이스케이프나 `;`가 든 쌍은 빠짐"(문서화된 선택)이지만, 같은 잘못(`%zz`)이 경로에서는 400, 쿼리에서는 말없이 빠져 "예외·특수한 경우 최소화"와 원칙 1에 어긋난다. LB22(2026-11-08)에서 1만 쌍 초과를 말없는 `{}`에서 400으로 바꾼 것과도 방향이 다르다.
  `req`에 원래 대상(`target`/raw query)이 없어 핸들러는 `?tag=a&tag=b`·`%2F`를 되살릴 길이 없다(Go `URL.RawQuery`·`RawPath`, Python `environ['QUERY_STRING']`).
- 원인 위치: `extras/http.fung` `_query`(1687)·`_query_pair`(1698), `_request_head`가 돌려주는 맵(1640 `Ok({method, target, …})`: `target`은 있으나 `_respond`가 핸들러 `req`에 넣지 않음).
- 고치는 법(결정): a) `req`에 `target`(받은 그대로의 요청 대상)을 더하고, 잘못된 쿼리 이스케이프는 경로처럼 400, b) a + `query`의 값을 늘 리스트로, c) a만(쿼리 규칙은 그대로, 필요한 사람은 `target`을 직접 나눔). **추천 a**: 규칙 하나(잘못된 이스케이프는 400)로 맞추고, 반복 키는 `target`으로 풀 수 있게 한다. b는 흔한 경우(`req.query.id`)를 불편하게 만든다.
- 처리: 이 커밋(W10b). `req.target`(받은 그대로), `req.query_pairs`(`_query_pairs`: 받은 순서, 반복 키 보존), 잘못된 쿼리 이스케이프는 400(쌍 하나라도), `query`는 `query_pairs`의 맵이라 반복 키는 마지막 값(계획 D25의 "지금 규칙(마지막이 이김)"을 따름; 고치기 전 코드는 첫 값이었다). 빈 쌍과 `;`가 든 쌍은 지금처럼 `query`·`query_pairs` 어디에도 넣지 않음(결정에 없어 그대로 두고 06·매뉴얼에 적음). 기대 출력 `server.txt`의 쿼리 요청에서 `%zz`를 떼고 400 요청을 따로 더함(`SERVER_REQUESTS` 96). 시험 `the_handler_gets_the_request_as_received`. 06, 매뉴얼 `http.md`, CHANGELOG, 결정 D25. 검토 1차 고침(이 작업의 다음 커밋, 결정): `;`는 보통 글자(WHATWG form-urlencoded, 쌍은 `&`로만 나눔), 빈 조각만 쌍이 아님.

### LB28. `exec`·`sh`: 신호로 끝난 명령이 `code: -1`, 문구는 "exited with code -1" — 결정, 중간
- 재현:
  ```
  $F -e 'print(sh("kill -9 $$"))'
  → Err({kind: 'exit, code: -1, out: "", err: "", message: "`/bin/sh` exited with code -1"})
  ```
  어느 신호인지(SIGKILL = OOM killer, SIGSEGV, SIGTERM)가 사라지고, 문구는 "종료 코드로 끝났다"고 틀리게 말한다.
- 근거: 06 "그 외: `Err({kind: 'exit, code, out, err, message})`"는 신호를 말하지 않고, 매뉴얼 `prelude/process.md`도 -1을 적지 않는다(코드 주석 process.rs 104에만). 사용자는 SRE이고 "명령이 죽은 이유"가 운영에서 가장 흔히 필요한 정보다. 셸·`subprocess`(음수 신호 번호)·Go(`ExitError.Sys()`)는 모두 신호를 알린다.
- 원인 위치: `crates/fung-vm/src/native/process.rs` `CommandWait`의 끝(660: `status.code().map_or(-1, …)`).
- 고치는 법(결정): a) `code`를 셸 관례 128 + n으로, `signal: n` 필드와 문구 ``…`was killed by signal 9 (SIGKILL)` ``, b) `code: nil` + `signal: n`, c) 지금대로 문서만. **추천 a**: `exec(...)!`·`e.code != 0` 같은 기존 판정이 그대로 맞고(0이 아님), 셸 스크립트를 옮겨 온 사용자의 기대(`$?` = 137)와 같다. fung 자신의 신호 종료 코드(08: 128 + n)와도 같은 규칙이다.
- 처리: 이 커밋. 결정 D26은 b: `CommandWait::finish`가 `ExitStatusExt::signal`로 `code: nil`·`signal: n`, 문구 ``…`was killed by signal 9 (SIGKILL)` ``(흔한 신호 이름 표 `signal_name`), 종료 코드로 끝나면 `signal: nil`이고 `Ok`에도 `signal: nil`(결과가 늘 같은 모양). 시험 `a_signal_ending_a_command_is_named`, 기대 출력 `prelude/runs.txt`. 매뉴얼 `process.md`(예제에 신호 갈래)·11장·`ssh-collect.md`, 06, CHANGELOG, 결정 D26.

### LB29. 바깥에서 온 값이 문자열 한도(1억 글자)를 넘으면 `Err`가 아니라 런타임 에러 E0806이고, `exec`는 다 읽은 뒤에 판정한다 — 결정, 중간
- 재현:
  ```
  mkfile -n 200m big.bin
  $F -e 'import fs
  print(type_of(fs.read("big.bin")))'                    → error[E0806]: too many characters (limit 100000000), 종료 1
  $F -e 'r = exec(["head", "-c", "300000000", "/dev/zero"])
  print(type_of(r))'                                    → E0806, 최대 RSS 312 MB(/usr/bin/time -l)
  ```
- 근거: 06 "에러 규칙": 외부 요인으로 실패할 수 있는 것(파일, 네트워크, 파싱) → `Ok`/`Err`, 코드가 틀린 것 → 런타임 에러. 파일·명령 출력의 크기는 프로그램이 정하지 못하는 바깥 요인이다. HTTP는 같은 문제를 `max_body` → `Err 'network`로 풀었다(L13, "메모리를 끝없이 쓰지 않음"). `fs.md`·`process.md`는 E0806을 말하지 않는다.
  `exec`는 출력 전체를 버퍼에 모은 뒤(`check_chars`) 판정하므로 `yes` 같은 명령은 메모리를 다 쓸 때까지 읽는다(한도가 메모리를 지키지 못함).
- 원인 위치: `crates/fung-vm/src/native/process.rs` 끝 처리(645~650 `check_chars`), `native/fs.rs` `read_fn`(320~, 결과를 `Value::bytes`로 만들 때의 한도), `value/arith.rs` `check_chars`(88).
- 고치는 법(결정): a) 파일·명령·표준 입력·소켓에서 읽은 값이 한도를 넘으면 그 함수의 `Err`(`'io` ``file is longer than the string limit (100000000 characters)``), `exec`는 읽는 중에 한도를 넘으면 명령을 끝내고(SIGTERM→SIGKILL, 취소와 같은 길) `Err`, b) 지금대로 두고 문서에 E0806을 적음. **추천 a**: 06의 에러 규칙 그대로이고, `exec`의 메모리도 지킨다.
- 처리: 이 커밋. `nat::fits_limit`·`nat::too_long`(``<what> is longer than the string limit (100000000 characters)``)과 조각마다 한 번 세는 `nat::CharCount`. `exec`는 `CommandWait::check_limit`가 읽는 중에 판정해 버퍼를 비우고 파이프를 닫고 취소와 같은 길(SIGTERM, `TERM_GRACE` 뒤 SIGKILL)로 끝낸 뒤 `Err 'io`(``standard output of `head` is …``; 끝의 `check_chars` 삭제). `fs.read`(FIFO 포함)는 `Err 'io`(``cannot read `p`: file is …``)이고 4억 바이트보다 큰 정규 파일은 읽지 않음. `read_line`·`read_lines`(표준 입력·파일 핸들·FIFO)의 줄은 ``cannot read standard input: line is …``. 소켓 `read_all`·`read_bytes`·`take_body`는 모두 ``body is …``(셋 다 http 본문의 길), `take_body`는 `Ok(text)`/`Err`(``body is …``)로 바꾸고 `http.fung`의 `_trailer`·`_drop`·`_drained`가 따름. 메모리 시험 `an_output_past_the_string_limit_ends_the_command`: 자식 프로세스로 다시 돌려 `wait4`의 최대 상주 메모리가 300 MB 미만(macOS 약 217 MB: 128 MiB 버퍼와 늘리며 복사한 64 MiB; 고치기 전에는 E0806). 디버그 빌드에서도 0.1초 남짓이라 릴리스 전용으로 묶지 않음. 검토 1차 고침(이 작업의 다음 커밋): 줄 읽기(표준 입력의 읽기 스레드 `vm/stdin.rs`, 파일 핸들 `Reading::line`, FIFO `Pipe::read_line`)와 FIFO의 `fs.read`(`Pipe::read_to_end`)도 `CharCount`로 읽는 중에 판정해 넘는 순간 멈추고 `nat::too_long_error`(읽은 조각은 버림, 줄의 나머지는 다음 읽기가 이어 읽음). `Pipe::read_line`은 `\n`을 한 번만 찾음(`scanned`; 전에는 채울 때마다 버퍼 전체를 훑어 제곱). 시험 `stdin.rs`의 끝없는 입력 두 개, `fs_tests` `a_line_past_the_string_limit_stops_its_read`. 02 "크기 한도", 06, 매뉴얼 `fs.md`·`io.md`·`process.md`, CHANGELOG, 결정 D22.
- 처리(최종 검토, 이 커밋): `fs.read`가 크기를 먼저 볼 수 없는 파일(문자 장치 `/dev/zero`·`/dev/urandom`)도 읽는 중에 끊는다. 풀에서 64 KiB씩 읽으며 `nat::CharCount`로 세다가 한도를 넘으면 `file is longer than the string limit`의 `Err`(`read_within`). 전에는 `std::fs::read`가 끝없이 읽어 `timeout`이 버린 뒤에도 메모리를 다 썼다. 시험 `a_device_past_the_string_limit_stops_its_read`. open-questions "파일", CHANGELOG.

### LB30. 사용자 `Proxy-Authorization`: http 프록시에는 두 번 가고, https 터널에서는 프록시 대신 목적지 서버로 간다 — 버그(보안), 낮음
- 재현:
  ```
  python3 -I canned.py 23560 'HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok' 'HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n' &
  HTTP_PROXY=http://u:p@127.0.0.1:23560 HTTPS_PROXY=http://u:p@127.0.0.1:23560 $F -e 'import http
  _ = http.get("http://example.test/x", headers: {"Proxy-Authorization": "Basic USERS"})
  print(http.get("https://example.test/x", headers: {"Proxy-Authorization": "Basic USERS"}))'
  ```
  http: 프록시가 받은 머리에 `Proxy-Authorization: Basic USERS`와 `Proxy-Authorization: Basic dTpw` 둘. https: `CONNECT`에는 `Basic dTpw`만 있고, 사용자 값은 `_request_text`가 터널 안의 요청(목적지 서버)에 쓴다(코드 읽기: `via == nil`이면 사용자 헤더를 모두 씀; 터널 안쪽은 TLS라 가짜 서버로 보지 못함, 12절).
- 근거: RFC 9110 11.7.2(Proxy-Authorization은 다음 프록시를 위한 것, 목적지로 전달하지 않음). LB2(2026-11-08)가 `Proxy-Authorization`을 민감 헤더로 다루는 것과도 어긋난다.
- 원인 위치: `extras/http.fung` `_request_text`(716~727: `_not_written`에 없음, `auth`를 덧붙임), `_tunnel`(653~667).
- 고치는 법: 사용자 `Proxy-Authorization`(·`Proxy-Connection`)은 요청에 쓰지 않고 프록시 쪽에만: http 프록시면 URL 사용자 정보가 없을 때 그 값을, https면 `CONNECT`에. 프록시가 없으면 보내지 않는다. 또는 단순히 사용자 헤더에서 금지(E0403)하고 프록시 자격 증명은 `*_PROXY` URL로만(추천: 이쪽이 규칙 하나).
- 처리: 이 커밋(W10a, 계획 4.10의 "프록시에만 한 번, 터널에서는 `CONNECT`에만"). `Proxy-Authorization`을 `_not_written`에 넣고 `_proxy_auth(p, hs)`가 값을 정함: 사용자 값(비어 있지 않으면)이 프록시 URL의 사용자 정보보다 앞섬(`Authorization`과 URL 사용자 정보의 규칙 `_basic_auth`와 같게; 점검 문서의 "URL 사용자 정보가 없을 때"와 반대 순서). http 프록시면 요청 끝에 한 번, https면 `_tunnel`의 `CONNECT`에만, 프록시가 없으면 보내지 않음. `Proxy-Connection`은 손대지 않음(사용자 헤더 그대로). 검토 1차 결정: origin을 떠나는 리다이렉트 뒤에는 사용자 `Proxy-Authorization`을 계속 버림(홉마다 프록시가 다를 수 있음), 06·`http.md`에 적고 시험 `a_users_proxy_authorization_stays_behind_on_another_origin`. 시험 `http.rs` `a_users_proxy_authorization_goes_to_the_proxy_alone`, 기대 출력 `redirects.txt`(프록시 없는 요청에서 `Proxy-Authorization: p` 줄이 빠짐). 06, 매뉴얼 `http.md`, CHANGELOG(보안).

### LB31. 응답·요청 헤더 맵이 받은 것과 다르다: `Pragma`에서 `cache-control`을 지어내고, HTTP/1.1의 `Connection`은 통째로 지운다 — 버그·정리, 낮음
- 재현:
  ```
  canned 'HTTP/1.1 200 OK\r\nPragma: no-cache\r\nContent-Length: 0\r\n\r\n'
  → headers: {"cache-control": "no-cache", "content-length": "0", pragma: "no-cache"}   # 서버는 cache-control을 보내지 않았다
  canned 'HTTP/1.1 200 OK\r\nConnection: close, Upgrade\r\nUpgrade: h2c\r\nContent-Length: 0\r\n\r\n'
  → headers: {"content-length": "0", upgrade: "h2c"}        # Connection 전체가 사라짐(Upgrade 토큰도)
  canned 'HTTP/1.0 200 OK\r\nConnection: close\r\n…'  → headers: {connection: "close", …}   # 1.0이면 남음
  서버: raw 'GET / HTTP/1.1\r\nHost: h\r\nPragma: no-cache\r\n\r\n' → req.headers에 "cache-control": "no-cache"
  ```
- 근거: 06 "응답 `headers`의 키는 소문자, 같은 이름이 여럿이면 `", "`로 잇는다" — 더하거나 빼는 규칙은 없다(06·`http.md` 어디에도 Pragma·Connection 언급 없음). RFC 9111 5.4의 Pragma 규칙은 **캐시**가 할 일이고 fung은 캐시가 아니다. LB3(2026-11-08)에서 서버의 "헤더를 고치거나 빼지 않는다"로 정한 것과 같은 방향이 받는 쪽에는 없다.
- 원인 위치: `extras/http.fung` `_fix_pragma`(889, 쓰는 곳 858·1603), `_close_header`(961~969, `_finish` 873).
- 고치는 법: 둘 다 지운다(받은 헤더 그대로). `_close_header`가 필요했던 이유가 "어차피 닫는다"라면 그것은 맵을 바꿀 이유가 아니다. 기대 출력 `http/client.txt` 등의 해당 줄이 바뀐다.
- 처리: 이 커밋(W10b). `_fix_pragma`·`_close_header` 삭제. "받은 그대로"를 틀 헤더에도 맞춰 `_transfer_encoding`·`_length`·`_trailer_header`는 판정만 하고 맵에서 빼지 않는다(`Transfer-Encoding`·`Trailer`·조각 전송의 `Content-Length`·같은 `Content-Length` 여럿도 받은 그대로). 클라이언트와 서버 둘 다. 시험 `a_response_keeps_its_headers_as_received`, 기대 출력 `http/*.txt`(`connection: "close"` 줄이 남음). 06, 매뉴얼 `http.md`, CHANGELOG.

### LB32. absolute-form 요청 대상: 핸들러는 대상의 호스트 대신 `Host` 헤더를 보고, 빈 경로는 `"/"`가 아니라 `""` — 버그, 낮음
- 재현(`srv.fung`, `raw`):
  ```
  GET http://other/x HTTP/1.1 / Host: h   → p: "/x", h.host: "h"      # 대상의 호스트는 other
  GET http://other HTTP/1.1  / Host: h    → p: ""                      # match req.path: "/" => … 에 안 걸림
  GET http://other?q=1 HTTP/1.1           → p: "", q: {q: "1"}
  ```
- 근거: RFC 9112 3.2.2 "When an origin server receives a request with an absolute-form of request-target, the origin server MUST ignore the received Host header field (if any) and instead use the host information of the request-target". RFC 9110 4.2.3(http(s) URI의 빈 경로는 `/`와 같다). LB4(2026-11-08)로 `host`를 핸들러에 준 이유(가상 호스트 가르기)가 이 형태에서 틀린 값을 준다.
- 원인 위치: `extras/http.fung` `_request_target`(1653~1676: authority를 검사만 하고 버림, 빈 경로를 그대로), `_request_head`(1589~).
- 고치는 법: absolute-form이면 대상의 authority로 `host`를 바꾸고(헤더 맵의 `host`), 빈 경로는 `"/"`. 시험 `the_server_checks_answers_and_requests`에 두 줄.
- 처리: 이 커밋(W10b). `_request_target`이 absolute-form·authority-form의 호스트(사용자 정보 뺌, 포트 포함)를 돌려주고, `req.host`(새 필드)가 그것, 아니면 `Host` 헤더 값, 둘 다 없으면 `nil`. 헤더 맵의 `host`는 LB31대로 받은 그대로(헤더 맵을 고치지 않고 필드를 더함). absolute-form의 빈 경로는 `"/"`, `CONNECT`의 authority-form은 경로가 없어 `""`. 시험 `the_handler_gets_the_request_as_received`. 06, 매뉴얼 `http.md`, CHANGELOG. 검토 1차 고침(결정): 호스트가 빈 absolute-form(`http:///x`, `http://:80/x`)과 사용자 정보가 든 것(`http://u@other/x`)은 400(RFC 9110 4.2.1·4.2.4). 1.x가 아닌 요청은 `Transfer-Encoding` 판정보다 먼저 505(전에는 HTTP/0.9 + TE가 501).

### LB33. 클라이언트가 HTTP/1.0 응답의 `Transfer-Encoding`을 지우고 `Content-Length`(또는 닫힘)로 읽는다(12절 의심의 판정) — 버그, 낮음
- 재현:
  ```
  canned 'HTTP/1.0 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Length: 3\r\n\r\n2\r\nok\r\n0\r\n\r\n'
  → Ok({status: 200, headers: {"content-length": "3"}, body: "2\r\n"})     # 틀이 깨진 본문이 Ok, TE 헤더는 사라짐
  ```
- 판정: 연결마다 요청 하나이고 응답 하나를 읽은 뒤 닫으므로(프록시를 거쳐도 클라이언트–프록시 연결을 다시 쓰지 않음) 요청 밀반입·응답 꼬임은 생기지 않는다. 보안상 무해하다. 다만 RFC 9112 6.1 "MUST treat the message as if the framing is faulty"를 어기고, 서버 쪽(LB13(2026-11-08): 같은 요청을 400)과 규칙이 반대다.
- 원인 위치: `extras/http.fung` `_transfer_encoding`(896~912: 1.0이면 `Ok((rest, false))`).
- 고치는 법: 1.0 응답에 `Transfer-Encoding`이 있으면 `Err 'network`(`Transfer-Encoding in an HTTP/1.0 response`). 서버와 같은 규칙 하나.
- 처리: 이 커밋(W10b). `_transfer_encoding`이 HTTP/1.0 응답의 `Transfer-Encoding`을 `Err("Transfer-Encoding in an HTTP/1.0 response")`로. 서버 쪽은 이미 400이라 같은 함수를 지나기 전에 거른다. 시험 `a_transfer_coding_in_an_http_1_0_response_is_an_error`. 06, 매뉴얼 `http.md`, CHANGELOG.

### LB34. 클라이언트가 사용자 헤더·메서드를 말없이 고친다: 대소문자만 다른 같은 이름은 하나만 가고, 메서드는 대문자로, 빈 메서드는 `GET` — 결정, 낮음
- 재현:
  ```
  http.get(url, headers: {"x-a": "1", "X-A": "2"})   → 보낸 머리: "X-A: 2" 하나("1"은 사라짐)
  http.request("", url)                               → GET (코드 읽기: `_attempt` 106)
  http.request("get", url) / ("propfind", …)          → 대문자로 바뀌어 감
  ```
- 근거: 서버는 같은 경우(핸들러 답의 `X-A`·`x-a`)를 잘못된 답으로 막는다(LB3(2026-11-08), 06 "답의 틀"). 클라이언트는 같은 입력을 말없이 하나로 줄인다(원칙 1). RFC 9110 9.1 "The method token is case-sensitive"; 빈 문자열이 `GET`인 것은 문서에 없는 특례다(`http.md`도 06도 말하지 않음).
- 원인 위치: `extras/http.fung` `request`(33: `upper(method)`), `_attempt`(105~117: `method0 == "" then "GET"`, `fold … put(m, _canonical(k), v)`).
- 고치는 법: 대소문자만 다른 이름 둘은 E0403(헤더 맵은 코드가 정함), 빈 메서드는 E0403, 메서드는 쓴 그대로(문서의 예가 대문자이면 충분; 바꾼다면 "대문자로 바꾼다"를 06·`http.md`에 적는다). 추천: 셋 다 고침(서버 규칙과 대칭).
- 처리: 이 커밋(W10b). `request`의 `upper(method)`와 `_attempt`의 빈 메서드 → `GET` 삭제, 메서드는 쓴 그대로. `_request`가 맨 앞에서 인자를 검사: 새 `_native.arg.token`(토큰이 아니면 E0403 ``argument `method` must be a token (RFC 9110 5.6.2), got ""``; 빈 메서드만이 아니라 토큰이 아닌 메서드 모두 — 전에는 `"G T"`가 `Err 'network`라 규칙이 둘이 되지 않게), `str_map`, 새 `_native.arg.distinct_names`(``… must be a map without names that differ only in case, got `x-a` and `X-A` ``). 그래서 `http.get("::bad", headers: {a: 1})`은 URL의 `Err`보다 E0403이 먼저. 저장소 시험의 소문자 메서드(`"head"`·`"put"`·`"patch"`)는 대문자로, 빈 메서드 줄은 `"get"`으로 바꿈(쓴 그대로 가는 것을 보임). 시험 `the_method_and_headers_go_as_written`, `arg_tests`. 06, 매뉴얼 `http.md`, CHANGELOG, 결정 D27. 검토 1차 고침(결정): 토큰이 아닌 헤더 이름과 탭 밖의 제어 문자가 든 값도 E0403(`_bad_header`의 `Err 'network` 삭제, 스킴과 무관). `_native.arg.distinct_names`를 세 검사를 하는 `_native.arg.headers`로 바꿈. CRLF 끼워 넣기 시험을 `the_method_and_headers_go_as_written`에.
- 처리(최종 검토, 이 커밋): 사용자 `headers`의 `Content-Length`·`Transfer-Encoding`·`Trailer`(대소문자 무관)도 보내기 전에 E0403(``argument `headers` must be a map without Content-Length, Transfer-Encoding or Trailer (the request frames its body itself), got `content-length` ``). 전에는 `_not_written`이 말없이 뺐다(D27, 말없는 실패 금지). `_not_written`은 `Host`·`User-Agent`·`Proxy-Authorization`만. 서버의 핸들러 헤더 검사(`_parts`)는 클라이언트와 같은 Rust 규칙 `_native.arg.header_problem`(nil 또는 이유 문자열)을 쓰고 500은 그대로, 토큰 규칙은 `_native.arg.is_token` 하나(fung의 `_is_tchar`·`_valid_value` 삭제). 매뉴얼 `http.md`, CHANGELOG 호환 깨짐.

### LB35. `exec`의 `cwd`: 에러 문구가 디렉터리를 말하지 않고, `env`를 주면 `PWD`가 `cwd`가 아니며, `cwd: ""`는 현재 디렉터리 — 버그, 낮음
- 재현(`w/`에 하위 디렉터리 `a`, `w`에서 실행):
  ```
  exec(["true"], cwd: "/nope")        → Err 'not_found "cannot run `true`: no such file or directory"     # 명령이 없는 것처럼 읽힘
  sh("exit 0", cwd: "nope")           → "cannot run `/bin/sh`: no such file or directory"               # /bin/sh가 없는 것처럼
  exec(["/usr/bin/env"], cwd: "a")              → PWD=…/w/a
  exec(["/usr/bin/env"], cwd: "a", env: {X: "1"}) → PWD=…/w        # 부모의 PWD 그대로
  ```
- 근거: 06 "시작하지 못하면 … 인자 순서대로 검사한다: … 작업 디렉터리(NUL, 없음)" — 다른 항목(`empty command name`, `command not found in PATH`)은 무엇이 틀렸는지 말하는데 이것만 운영체제 이유뿐이다. `PWD`는 `env`가 없을 때만 맞춰 주는 규칙이 코드 주석에만 있다(`prepare` 767). `cwd: ""`는 `filter(|d| !d.is_empty())`로 "지정 안 함"이 되는데 `fs.mkdir("")`은 `'not_found`(06)라 빈 경로의 뜻이 함수마다 다르다.
- 원인 위치: `crates/fung-vm/src/native/process.rs` `run_command`(118~120), `prepare`(787~803).
- 고치는 법: `cwd` 실패는 ``working directory `/nope`: no such file or directory``; `PWD`는 `env`와 상관없이 `cwd`가 있으면 넣는다(사용자가 `env`에 `PWD`를 주면 그것); `cwd: ""`는 `Err 'not_found`(또는 E0403). 시험 `process_tests`.
- 처리: 이 커밋. `prepare`가 작업 디렉터리의 이유를 ``working directory `<cwd>`: <이유>``로(디렉터리가 아니면 `not a directory`; NUL은 전과 같은 문구), `PWD`는 `env`에 `PWD`가 없으면 늘 `cwd`의 절대 경로, `cwd: ""`는 ``E0403 argument `cwd` must be a non-empty str``(`split`의 `sep`과 같은 꼴). 시험 `the_working_directory_is_named_and_set`, 기대 출력. 매뉴얼 `process.md`, 06, CHANGELOG.

### LB36. `time.parse`의 형식에 같은 필드가 두 번 있으면(`MM`과 `MMM`, `YYYY` 둘, `HH` 둘) 말없이 마지막 값이 이기고, `ddd`는 날짜와 대조하지 않는다 — 결정, 낮음
- 재현(`TZ=America/New_York`, `Asia/Seoul`):
  ```
  time.parse("2026-01-02 oct", "YYYY-MM-DD MMM")     → Ok(2026-10-02T…)        # 01월이 10월로
  time.parse("2026-01-02 2027", "YYYY-MM-DD YYYY")   → Ok(2027-01-02T…)
  time.parse("2026-01-02 03 04", "YYYY-MM-DD HH HH") → Ok(…T04:00:00…)
  time.parse("2026-01-02 Mon", "YYYY-MM-DD ddd")     → Ok(2026-01-02…)          # 2026-01-02는 금요일
  ```
- 근거: 원칙 1·엄격함. LB20(2026-11-08)에서 "날짜 없는 형식"을 E0403(형식은 코드가 정함)으로 막은 것과 같은 종류의 형식 오류다. `ddd` 비대조는 06에 적힌 선택이지만 입력이 스스로 모순인데 `Ok`인 유일한 경우다.
- 원인 위치: `crates/fung-vm/src/native/timefmt.rs` `parse_time`(154~231), `names_a_date`(132: 필드를 세지 않고 있음만 봄).
- 고치는 법: 같은 필드(연·월·일·시·분·초·초 소수·오프셋)를 두 번 쓰는 형식은 `names_a_date`와 같은 자리에서 E0403. `ddd`는 읽은 날짜의 요일과 다르면 `Err 'parse`(``weekday Mon does not match 2026-01-02 (Fri)``). 06 "시간 형식"과 `time.md` 고침. `format` 쪽의 형식 검사(SP25)와 함께 정한다.
- 처리: 이 커밋. `timefmt.rs`에 `tokens`·`field`로 `names_a_date`를 다시 쓰고 `repeats_a_field`(연·월·일·요일·시·분·초·밀리초·오프셋; `MM`/`MMM`, `YYYY`/`YY`, `Z`/`ZZ`는 같은 필드)를 `parse_fn`의 날짜 검사 다음에 E0403(``a format naming each field once``). `ddd`는 읽은 날짜의 요일과 대조해 `Err 'parse`. 시험 `parse_formats_name_each_field_once_and_check_the_weekday`. 매뉴얼 `time.md`, 06, CHANGELOG, 결정 D28.

### LB37. 지역 시간대로 보이면 0000~9999년 밖(10000년, -0001년)이 찍히고, 그 글을 다시 읽지 못한다 — 버그, 낮음
- 재현:
  ```
  TZ=Asia/Seoul $F -e 'import time
  t = time.from_unix(253402300799)          # 9999-12-31T23:59:59Z
  print(t); print(time.parse(str(t))); print(time.parts(t).year)'
  → 10000-01-01T08:59:59+09:00
  → Err(… "expected `-` after the year")
  → 10000
  TZ=America/New_York … time.from_unix(-62167219200)   → -0001-12-31T19:03:58-04:56:02
  ```
  `json.stringify`도 같은 글을 쓴다(RFC 3339가 아닌 JSON 값).
- 근거: 06 "범위와 오프셋: 시각은 UTC로 0000~9999년" — 범위를 UTC로만 지켜, 보여 주는 쪽(벽시계 날짜)이 범위를 벗어난다. 06 "형식을 지정하지 않으면 ISO 8601"·`parse(s)`가 RFC 3339(4자리 연도)라 출력과 입력이 짝이 맞지 않는다.
- 원인 위치: `crates/fung-vm/src/native/time.rs` `from_unix`·범위 검사(160~170), `timefmt.rs` `format_time`(39~).
- 고치는 법: 범위를 "그 값의 벽시계 날짜가 0000~9999"로 정의(만들 때와 `local`·`utc`로 시간대를 바꿀 때 검사, 넘으면 E0403/E0503), 또는 범위 끝을 양쪽 14시간씩 좁힘. 추천: 앞의 것(규칙이 "보이는 날짜"라 설명이 쉽다).
- 처리: 이 커밋. 추천(앞의 것): `Time::from_unix`가 그 시간대의 벽시계(`wall`)로 범위를 보고(`shown`), `in_loc`·`utc`·`local`이 `Option`이라 `time.utc`·`time.local`은 범위 밖이면 E0403(``a time in years 0000 to 9999 in UTC``), `from_unix`의 E0403 문구는 "in the local zone", 산술은 전처럼 E0503, 읽은 글은 `Err 'parse`(`time out of range`). 그래서 찍힌 시각은 늘 네 자리 연도이고 `parse`가 다시 읽는다(`json.stringify`도). `9999-12-31T23:59:59-05:00`처럼 UTC로는 10000년인 글은 이제 읽힌다(그 시각의 `utc`가 E0403). 시험 `the_range_is_of_the_shown_date`·`edges_are_of_the_date_shown`(전의 `edges_show_in_any_offset`를 뒤집음). 매뉴얼 `time.md`, 06, CHANGELOG.

### LB38. `cli.parse`의 남은 구멍: 스위치의 `=값`은 문자열, 같은 플래그의 반복은 말없이 덮어씀, 잘못된 `bools`는 `s`라는 인자를 탓함(11-08 LB11 후속) — 결정, 낮음
- 재현:
  ```
  cli.parse(["--v=false"], bools: ["v"])          → {flags: {v: "false"}}      # 스위치인데 문자열; if flags.v: 에서 비로소 에러
  cli.parse(["--x", "1", "--x", "2"])             → {flags: {x: "2"}}          # "1"이 사라짐
  cli.parse(["--a-b", "1", "--a_b", "2"])         → {flags: {a_b: "2"}}
  cli.parse(["--"], bools: [1])                   → error[E0403]: argument `s` must be str, got int   (호출 경로 extras/cli.fung:35 _key)
  ```
- 근거: 지난 점검 LB11의 "고치는 법"이 "bools 스위치의 `=값`도 E0803으로 막을지 함께 정한다"를 남겼는데 처리 줄은 그것을 말하지 않는다(미결로 남음). 06 36행 "에러는 사용자가 부른 함수의 인자를 말한다"(VA7(2026-11-08)) — `bools` 대신 `replace`의 `s`를 말한다(인자 검사 일반은 SP18).
  덧붙여: `cli.parse`의 입력은 바깥 입력(사용자가 친 명령줄)인데 `--=x`는 E0803 런타임 에러(호출 경로에 내장 파일 줄)라 스크립트가 사용자 오타에 깔끔한 안내를 줄 길이 없다(06 에러 규칙의 "외부 요인 → Err"와 긴장).
- 원인 위치: `extras/cli.fung` `parse`(13: `map(bools, _key)`가 타입 검사 없이), `_parse`(18~21: `put` 덮어쓰기), `_flag`(23~31).
- 고치는 법(결정): `bools: list`의 원소를 `str`로 검사(`bools`를 말하는 E0403; SP18과 함께); 같은 플래그의 반복은 a) 마지막이 이김을 문서화, b) 값을 리스트로 모음, c) `fail`; 스위치의 `=값`은 `true`/`false`만 받고 그 밖은 같은 실패. 실패의 꼴은 a) 지금처럼 E0803, b) `parse`가 `Ok({flags, rest})`/`Err({kind: 'invalid, message})`를 돌려줌.
  **추천**: 반복은 a(흔한 CLI 관례, 문서 한 줄), 스위치 값은 `true`/`false`만, 실패는 b(바깥 입력은 `Err` — 06의 규칙; 스크립트는 `cli.parse(args)!`로 지금처럼 쓸 수 있다).
- 처리: 이 커밋. `cli.parse`가 `Ok({flags, rest})`/`Err({kind: 'invalid, message})`: 이름 없는 옵션(전에는 E0803), 같은 플래그의 반복(쓴 꼴만 다른 `--a-b`·`--a_b`도, 결정 D29의 ★), `bools` 스위치의 `=값`은 `true`/`false`만(불리언이 됨). `bools`·`argv`의 원소 검사는 `_arg.list_of`(E0403이 `bools`를 말함). 저장소의 호출(examples 5, 매뉴얼 레시피·11장, `module_tests.rs`)에 `!`, `cli-options` 레시피는 `match`로 `usage_error`. `extras/cli.fung`의 `test` 블록은 내부 다리를 가져와 `fung test`로 돌 수 없게 되어 `tests/lang/cli_parse.fung`(옛 `cli_bools_names.fung`)로 옮김. 매뉴얼 `cli.md`·`learn/11`·레시피, 06, CHANGELOG, 결정 D29.

### LB39. `_native.net.set_deadline(c, 0s)`는 "기한 없음" — LB5(2026-11-08)로 공개 API에서 없앤 특례가 원시에 남았다 — 정리, 낮음
- 재현: 코드 읽기. `crates/fung-vm/src/native/net.rs` `set_deadline_fn`(1754~1770, 주석 "`0s` removes the deadline"), 쓰는 곳 `extras/http.fung` 1760(`_respond`의 `_net.set_deadline(c, 0s)`).
- 근거: LB5(2026-11-08)가 "같은 이름의 인자가 함수마다 반대 뜻"을 없애며 `timeout: nil` = 한도 없음, `0s` 이하 = 곧바로 시간 초과로 맞췄다. 숨은 원시지만 같은 저장소 안에서 `0s`가 두 뜻이고, 음수 기간의 뜻은 정해져 있지 않다.
- 고치는 법: `set_deadline(c, d: dur | nil)`: `nil`이 기한 없음, `d <= 0`은 곧바로 기한 지남. `_respond`는 `nil`을 넘긴다.
- 처리: 이 커밋(W10a). `set_deadline(c, d: dur | nil)`: `nil`이 기한 없음, `0s`는 지금(곧바로 기한 지남), 음수는 전처럼 E0403(`nat::opt_dur` 새로). `http.fung` `_respond`는 `nil`을 넘김; 다른 호출(`_header_timeout`·`500ms`)은 양수라 그대로. 시험 `net_tests` `a_deadline_ends_reads_with_a_timeout`(`nil` 해제, `0s` 곧바로 시간 초과), `tls_tests`의 `0s`를 `nil`로. 06.

### LB40. LB9(2026-11-08) 뒤 매뉴얼 두 곳이 아직 `'invalid`를 말한다 — 문서, 낮음 (동시성 영역의 `read_lines` 문서 지적을 합침)
- 재현: `$F -e 'import fs
  f = fs.open("o.txt", '"'"'write)!
  print(read_lines(f))'` → `error[E0520]: file `o.txt` is not open for reading`(런타임 에러). 그런데
  - `manual/reference/prelude/io.md` 223(`read_lines`의 에러): "쓰기용 핸들은 `Err({kind: 'invalid, message})`" — 바로 위 148(`read_line`)은 E0520이라 쪽 안에서 어긋난다.
  - `manual/reference/modules/fs.md` 27(`kind` 표): "`'invalid` — 잘못된 glob 패턴(`fs.glob`), 읽기로 열지 않은 핸들로 읽기(또는 그 반대)" — 둘 다 이제 E0515·E0520이다(같은 쪽 359·892가 맞게 적음). 지금 `fs`에서 `'invalid`가 나는 곳은 `fs.copy`의 "정규 파일이 아님"(742·759)뿐이다.
- 고치는 법: 두 줄을 고치고(`io.md`의 `read_lines` 에러 줄을 `read_line`과 같게 E0520) `FUNG_UPDATE_MANUAL=1`로 `docs/latest` 다시 만듦.
- 처리: 이 커밋. `io.md` `read_lines`의 쓰기용 핸들은 E0520, `fs.md` `kind` 표의 `'invalid`는 `fs.copy`의 정규 파일 아님만(glob은 E0515, 모드는 E0520).

## 10. CLI 도구 (TL)

### TL6. "스크립트를 읽지 못함"의 종료 코드가 명령마다 3·2·1로 다르다 — 결정, 낮음
- 재현:
  ```
  $F run nope.fung / $F nope.fung / $F check nope.fung / $F dump ast nope.fung   → 3
  $F test d1        (d1 안에 chmod 000 파일)   → FAIL d1/b.fung: <load> …, 종료 2
  $F fmt --check d1 (같은 디렉터리)            → error: cannot read `d1/b.fung`: permission denied, 종료 1
  $F fmt d1/x.fung  (없는 파일 하나)           → 3
  ```
- 근거: 08 종료 코드 표: 3은 "`fung` 명령 자체의 사용 오류(모르는 하위 명령, 잘못된 인자, 아직 구현되지 않은 기능)". 권한이 없는 파일은 사용 오류가 아니다. 같은 사건(파일을 읽지 못함)이 명령과 "인자로 줬는지/디렉터리에서 찾았는지"에 따라 3·2·1이 된다. `fmt --check`의 1은 "고칠 파일이 있음"과 구별되지 않아 CI가 "포맷 안 됨"으로 오해한다(`cli.md` 239가 문서화하지만 이유는 없다).
- 원인 위치: `crates/fung-cli/src/main.rs` `read_error`(EXIT_USAGE), `fmt.rs` `run_fmt`(읽기 실패 → EXIT_RUNTIME), `test.rs` `test_file`(compile_failed → 2).
- 고치는 법(결정): a) 인자로 준 경로가 없거나 디렉터리 순회가 실패하면 3(사용 오류 — 경로 오타), 읽기 실패(권한·UTF-8 아님)는 모든 명령에서 2(실행 전 실패, `check`의 "문제를 찾음"과 같은 칸), b) 지금대로 두고 08에 명령별 표. **추천 a**: 08 표의 뜻에 맞고, `fmt --check`의 1을 "고칠 파일 있음" 하나로 되돌린다.
- 처리: 읽지 못한 파일의 종료 코드를 있는 파일 2·없는 경로 3으로 통일, `fmt --check`의 1은 "고칠 파일 있음" 하나, 08 종료 코드 표·manual/reference/cli.md·CHANGELOG, 결정 D30 (이 커밋).

### TL7. 옵션·남는 인자 처리가 명령마다 다르다(11-08 TL4 후속) — 정리, 낮음
- 재현:
  ```
  $F check --foo / $F check -h / $F test --help   → error: cannot read `--foo`: no such file or directory   (경로로 읽음)
  $F fmt --help                                   → error: unknown option --help
  $F dump --help                                  → error: unknown dump stage `--help`
  $F --version                                    → error: unknown command `--version`   (그런데 -h·--help는 help)
  $F fmt - -  (또는 fmt - a.fung)                 → error: cannot read `-`: no such file or directory
  printf ':quit now\n' | $F ; echo $?             → 0   (남는 인자를 말없이 무시)
  printf ':load nope.fung\n:foo\n' | $F           → 표준 에러 "cannot read `nope.fung`: …", "unknown command :foo" (`error: ` 머리 없음)
  ```
- 근거: 08 "남는 인자는 모든 명령에서 사용 오류(3)"(TL4(2026-11-08))·"파일을 읽거나 쓰지 못하면 모든 명령이 … ``error: cannot read `p`: …``". `fmt`만 `-`로 시작하는 인자를 옵션으로 보고, 나머지는 경로로 읽어 엉뚱한 문구가 된다. REPL의 `:` 명령은 TL4의 규칙 밖이다.
- 원인 위치: `crates/fung-cli/src/main.rs` `run`·`run_dump`, `check.rs` `run_check`, `test.rs` `run_test`, `fmt.rs` `run_fmt`(`-`는 인자가 하나일 때만 표준 입력), `repl/mod.rs`의 `:` 명령 처리.
- 고치는 법: 공통 규칙 하나 — `-`로 시작하는 인자(단독 `-`와 `-e` 제외)는 그 명령이 아는 옵션이 아니면 ``unknown option `--foo` ``(3); `--version`은 `version`과 같게(또는 모르는 옵션); `fmt`의 `-`는 다른 경로와 함께면 사용 오류; REPL `:quit`·`:help`의 남는 인자는 에러(세션 실패), REPL 에러 줄에도 `error: ` 머리.
- 처리: 모든 명령이 `crates/fung-cli/src/args.rs`의 한 옵션 파서(`-h`·`--help`·`--version`·`--`·``unknown option `X` ``)를 쓰고, `fmt - -` 거절, REPL `:` 명령의 남는 인자 거절과 `error: ` 머리, 08·cli.md·CHANGELOG (이 커밋).

### TL8. `fung fmt`(·`--check`)가 UTF-8이 아닌 소스를 받아 통과시킨다 — `check`·`run`은 같은 파일을 거절한다 — 결정, 낮음
- 재현:
  ```
  printf 'x = "\xff"\n' > d1/c.fung
  $F check d1/c.fung   → error: cannot read `d1/c.fung`: not valid UTF-8 at line 1, column 6   (3)
  $F fmt --check d1/c.fung → (출력 없음) 0
  ```
- 근거: `manual/vm/15-formatter.md` 158이 의도로 적었다("포맷터는 그런 파일도 받는다"). 그러나 L4가 "비UTF-8 소스는 읽기 거절"을 언어 규칙으로 정했으므로, CI의 `fmt --check`가 컴파일되지 않을 파일을 "정상"으로 보는 예외가 남는다(예외 최소화).
- 원인 위치: `crates/fung-front/src/format.rs` `format_bytes`(74)·`lex_bytes`, `crates/fung-cli/src/fmt.rs`.
- 고치는 법(결정): a) `fmt`도 `read_file`(`decode_source`)로 읽어 같은 에러(파일은 건드리지 않음, 종료 코드는 TL6의 결정), b) 지금대로(바이트를 지키는 포매터). 추천 a: 소스의 정의가 하나가 되고, 포매터의 바이트 매핑 코드(`lex_bytes`의 `map`)가 필요 없어진다.
- 처리: `fung fmt`가 `decode_source`로 읽어 UTF-8이 아닌 소스를 거절(종료 코드 2, 파일 그대로), `format_bytes`·`lex_bytes`의 바이트 매핑 삭제(`format`은 `&str`), 08·01·manual/vm/15-formatter.md·CHANGELOG, 결정 D31 (이 커밋).

## 11. 확인했고 문제없던 것 (다시 검사하지 않아도 됨)

- **지난 점검 12절 의심의 판정**(19개)
  - 이번 항목이 된 것(11개): 명세의 파이프 괄호(FE27)·nil과 깎기(SP15)·정규식(SP20)·효과 없는 문장(FE24), 앞단의 `names_in` O(n²)(FE37)·0으로의 반올림(FE31)·모듈 트리 복사(FE39, 재지 못함)·포매터의 `false - 1`(FE35),
    VM의 명령당 5 ns(VM16), 값의 `sort_by` 복사(VA24), 라이브러리의 HTTP/1.0 TE+CL(LB33, 보안상 무해).
  - 문제없던 것(6개):
    - Step 내장 함수 → `issue` 경로의 `tick`은 한 호출에 두 번이 아니다(코드 읽기). `CALL`이 내장 함수 자신의 몫으로 한 번, `issue` → `start_call`이 콜백 호출마다 한 번(`call_plain`은 Plain이고 개수가 정확할 때만 그 안에서 한 번, 아니면 tick 전에 `None`).
    - 큰 맵의 덮어쓰기 `put`이 n=10000에서 5배 느린 것은 재현되지 않았다(20만 번 평균). 혼자 쥔 맵 n=20·1천·1만·10만·20만·100만에서 0.17·0.19·0.20·0.26·0.30·0.49 µs, 공유된 판에 `put` 0.46·0.70·0.85·1.06·1.12·1.37 µs, 읽기 0.12~0.44 µs로 n에 단조롭다. HAMT 해시는 프로세스마다 무작위 씨앗(`RandomState`)이라 키를 골라 한 칸에 몰기 어렵다.
    - `replace_fits`의 추정은 실제로 모자라게 셀 수 있다(`replace(repeat("ठ", 6천만), "\xa4", "")`는 추정 0자 증가, 실제 1억 2천만 자). 하지만 11-08 VA9의 `check_returned`가 결과를 다시 재 E0806으로 멈춘다(1.26 s·310 MB를 쓴 뒤). 한도는 새지 않고 낭비만 있다.
    - 두 번째 SIGTERM의 `exit_now`: 종료 코드 143, 그 전에 찍은 줄을 잃지 않는다(표준 출력은 `std::io::stdout` 줄 버퍼, `print`는 줄마다 비움, `process::exit`가 정리 때 비움). 줄 하나를 쓰는 도중이면 잘릴 수는 있으나 "곧바로 끝냄"의 의미 안.
    - kqueue `EVFILT_PROC`: 이미 끝난 자식이면 `ESRCH`를 "끝났음"(`Ok(false)`)으로 받아 곧바로 다시 본다(`poller/kqueue.rs` `register_exit`). 태스크 50개 × `exec(["/usr/bin/true"])` 40번 = 2,000번이 1.2초에 멈춤 없이 끝남.
    - epoll(재현 안 함·코드 읽기): 등록 데이터가 fd 번호이고, `fork`~`exec` 사이처럼 옛 파일이 dup된 채 번호가 다시 쓰여도 새 fd는 `MOD`가 ENOENT라 `ADD`로 따로 등록된다(epoll 키는 (fd, file)). 옛 등록이 남아 무장돼 있으면 같은 번호로 한 번 가짜 이벤트가 오지만 원샷이라 한 번뿐이고, 깨어난 태스크는 시스템 호출을 다시 해 EAGAIN이면 다시 등록하므로 무해. 기다림이 끝날 때 `leave_all`이 fd가 닫히기 전에 `DEL`한다.
  - 이번에도 확인하지 못한 것(2개): https → http 리다이렉트의 자격 증명 유출과 EXDEV(11-08 LB8)(이번 점검은 다른 포트로의 리다이렉트만 확인했다), REPL 줄 편집기(raw 모드)와 `~/.fung_history`(12절에 다시 둠).
- **명세·연산자**
  - 2026-11-08 SP1~SP11의 처리: `map`/`conc.pmap`의 `f:` 람다 `?`(E0226), 보간·`join`의 결과값(E0519, 단 SP12의 둘 이상 값 예외), 맵 패턴의 문자열 키, `sym("Red")`, `timeout(f, d)`·`send_now`·`recv_now`·`close(x)`, `dbg`의 숨은 인자, `kind` 어휘(`'empty`·`'full`), E0501 도움말 — 모두 바이너리와 맞다.
  - 비교·동일성 표: `1 == 1.0`, `unique([1, 1.0])`·`unique([0.0, -0.0])`·2⁵³ 넘는 정수와 실수의 해시 일치, `sort`의 섞인 타입 E0501, `nil`·태그 순서 비교 에러, 맵 `==`의 순서 무시.
    해시와 `==`의 일치: `unique([1, 1.0])` → `[1]`, `-0.0`/`0`/`0.0` 한 통, 2^63 근처 실수 갈래(`hash_leaf`의 `< 2^63`), 맵 해시가 순서와 무관.
  - 산술: `-7.5 % 2`, `7.5 // 2`, 정수 오버플로 E0503, 기간 `/ // %`·`1s / 0.3`의 ns 반올림과 표시(`3s333.333333ms`, 리터럴로 다시 읽힘), 기간 리터럴의 단위 순서·중복(`1s1s` E0105).
  - 어휘: 예약어 키(`{type: …}`, `m.match`), `'if`·`'nil`·`'_` 심볼, 이스케이프 목록, `0x_ff`·`0b_1` 거절, 앞자리 0(E0102).
  - 패턴: 리터럴 `1`이 `1.0`에 맞음(`==`와 같은 규칙), 대안·가드의 차례, 구조 분해 실패 E0602, 맵 패턴의 부분 일치. 패턴의 심볼과 문자열(`match 'a: "a" =>`이 조용히 안 맞음)은 02가 명시적으로 예외로 둔다.
  - 범위·받는 끝을 받는 컬렉션 함수 27개의 행렬(`sort`·`unique`·`reverse`·`sum`·`min`·`max`·`first`·`last`·`take`·`drop`·`join`·`flatten`·`group_by`·`sort_by`·`try_map`·`zip`·`filter`·`find`·`fold`·`any`·`each`): 02 "순회 대상"과 맞다(`len`·`in`·`is_empty`는 받는 끝을 받지 않음이 문서와 같다).
  - 인자 타입 에러 60여 개 표본: 매개변수 표에 타입이 있는 함수는 E0403, 여러 타입을 받는 함수는 E0506(functions.md 규칙) — SP21의 맵 키와 SP18의 fung 모듈만 예외.
  - 모듈: 비공개 이름 E0304, 없는 멤버 E0303, 다시 내보내지 않음(`a.f`·`a.json`), 같은 모듈을 두 이름으로, `import _native.fs` E0310, `import "fs"` E0301.
  - `exec`·`sh`의 0 아닌 종료는 `Err 'exit`(조용한 실패 없음), `http.*`의 `max_body` 초과는 `'network`로 감싸져 내부 `'limit`·`'eof`가 새지 않음.
  - `type_of`가 돌려주는 이름이 values.md 표·convert.md 목록과 같다. 경고 W0101~W0103이 `fung check`에서 나온다.
  - `spec/errors.txt`의 모든 번호가 `manual/reference/errors.md`에 항목이 있다(두 목록의 차이 없음).
- **앞단**
  - 줄 이어짐: 괄호 안 줄바꿈(`[1\n,2]`, `(a,\nb) =>`, 이름 인자 `a\n:1`, `xs[1\n..=\n2]`), 줄 끝 `|`(패턴·타입)·`in`·`??`·`and`, 주석 사이의 `|>` 이어짐(11-08 FE6·FE7 고침 유지).
  - 가드 끝의 괄호·이름이 람다로 오인되지 않음(`y when (y > 1) =>`, `y when big =>`).
  - 비교 연쇄 E0111, `1..2..3` E0118, `-2 ** 2`, `2 ** 3 ** 2`, 최소 정수(10진) 접기와 괄호 `-(…)` 거절(11-08 FE9), `..._`(11-08 FE11), 대안 이름 E0216, 중복 이름 E0221.
  - 포매터: `tests/lang`·`examples`·`extras`의 128개 파일을 들여쓰기를 없애고(또 공백을 뭉개고) 돌린 결과가 원본과 같고 다시 돌려도 같음(차이는 주석·문자열 안을 점검 때 뭉갠 것뿐). 주석 붙은 match 정렬, 블록 인자, 빈 줄 접기.
  - 보간: 중첩 문자열 `"${"a${"b"}c"}"`, 보간 안 줄바꿈은 E0104.
  - 이름: NFC(조합형·분해형 같은 이름·심볼), 맵 키는 쓴 그대로, 한글·데바나가리 이름, CRLF, 탭이 있는 줄의 밑줄 칸.
  - 모듈: 순환 경로 전체(E0302), 진입 파일 import(E0307), 비공개 `_` E0304, prelude를 가리는 모듈·가져온 이름의 W0103, `import "./lib/../lib/m"`와 다른 별칭이 같은 모듈, 절대 경로 import, 모듈 최상위 문장 제한(E0306).
  - 선형성: `+` 정수 사슬 40000, 리스트 원소 40000, 최상위 fn 10000(상호 참조), 최상위 이름 40000, 지역 8000, 깊은 리스트 리터럴 8000, 보간 리스트 16000, 괄호·보간 중첩(11-08 FE16) 모두 0.1 s 안팎.
  - `return` 자리: 최상위 E0211, `for` 안 E0213. `assert`는 문장만. 지역 함수 상호 재귀 E0208(문서대로).
- **컴파일러·VM**
  - 상수 접기(11-08 VM10 g)가 실행 결과와 같다: `1 + 2.5`, `9007199254740993 / 1`, `9007199254740993 * 1.0`, `MAX - 0.5`, `-(0.0)`, `0.0 * -1`, `7 // 2.0`, `1e308 * 10`(E0504), `- MAX - 1`, `-(MIN)`(E0503), `0.1 + 0.2`, `10 / 4` 모두 접은 판과 `id()`로 감싼 판이 같다.
  - 인자 규약: `fn f(a, b = a + 1, ...r)`에 `f(1)`·`f(1, 5)`·`f(1, 5, 6, 7)`·`f(1, b: 5)`·`f(b: 5, a: 1)`이 명세대로, `f(1, 2, b: 5)` E0405, `f(1, r: […])` E0406. 콜백 길(`map`·`fold`)의 기본값 채우기와 E0407 위치.
  - `?`의 반환 조각(11-08 VM10 f)이 반환 타입을 검사한다(`-> int`에서 `?`가 돌려주는 Err는 E0404, `?` 자리). `return`·`?`의 `for` 본문 금지(E0213·E0220), 최상위 금지(E0211·E0214).
  - `match` 대상 직접 쓰기(11-08 VM10 b): 가드 있는 갈래에서도 `subject_done`이 대상을 놓아 `put`이 제자리 수정을 받는다. 최상위 `match`의 이름·`_` 갈래.
  - `[h, ...t]` 재귀가 선형이다(10만~80만 원소 0.02~0.15초).
  - 24비트 패킹: `CHECKTYPE`(슬롯 12비트 + 타입 12비트)와 `CALL`(12 + 12비트)은 `MAX_ARGS = 4095`가 매개변수·인자 수를 막아 넘치지 않는다.
  - 생존 분석의 평가 순서: `Index`·`Slice`·`Map`(키 상수 뒤 값)·`Coalesce`·`dbg`·`assert`의 쪼갠 비교(`not in` 포함)·기본값 식(분석 밖이라 늘 복사)이 컴파일 순서와 걸음 순서가 같다.
  - `cond`의 `and`·`or`가 넘긴 칸을 다른 길에 다시 "쥔 것"으로 되돌리는 처리(빈 칸의 `CLEARLOCAL`은 무해).
- **값**
  - 제자리 수정의 안전성: 클로저가 붙잡은 맵·리스트, 기본값 `{a: 1}`을 두 번 부름, 튜플 구조 분해 뒤 한쪽에 `+`, 같은 인자에서 `put`/`+`를 두 번(`(a, b)`가 서로 안 보임), `atom` 안의 리스트, 큰 맵(100) 스냅숏 뒤 `remove`, 리스트 안·맵 안의 리스트에 `+` — 모두 원본이 바뀌지 않았다.
  - 영속 리스트의 쓰임 모양: 100만 원소에서 `xs[1..]` 재귀 349 ms, 큐 `q[1..] + [x]` 390 ms, 스택 `s[..-1]`/`s + [x]` 295 ms, `[x] + acc` 340 ms — 모두 선형(10만의 10배).
  - 변환 문법이 문서와 같다: `int`/`parse_int`(부호, `_`, 앞의 0, 공백·16진·소수점 거절), `float`/`parse_float`(`.5`·`1.`·`1__0`·`_1`·`inf`·`nan`·`1e400` 거절, `1_0.0` 허용), `sym`(예약어 `'if`·`'true`도 되고 NFC). (문서와 같다는 뜻이고, 문법끼리의 어긋남은 SP14.)
  - 기간: `parse_dur(str(d))`가 `1s/7`, `-1s/7`, `1s/1e9`(0.000001ms)에서 왕복, `2562047h` 경계와 넘침 E0503, `1s/0`·`1s/0.0` E0502.
  - `exit`의 0~255 밖·비정수 E0403. `read_line`/`read_lines`/`lines`의 `\r`·빈 줄·끝 줄바꿈 규칙이 서로 같다.
  - E0806: `sort`·`unique`·`sort_by`·`join`·`reverse`·`zip`·`drop`·`take`·`group_by`·`map`이 1억 넘는 범위를 일 시작 전에 거절(0.00~0.01 s).
  - 맵 출력: 예약어·대문자·`_` 키(`{if: 1}`·`{Ok: 1}`)는 그대로 다시 읽히고, 식별자 모양이 아닌 키는 따옴표. 보간·`join`의 E0519가 맨 값만 보는 것은 11-08 SP2 결정(가)대로(`"${[Ok(1)]}"`은 됨).
  - `get`의 "키는 있고 값이 nil"은 문서대로 `nil`. `x in range`의 비정수 `false`는 리스트의 `in`(`==`)과 같은 규칙.
- **동시성**
  - 자기 핸들을 채널로 받아 `await` → E0702 보고(`1 other task is waiting: await at …`).
  - `timeout` 안에서 바깥 범위에 띄운 태스크는 기한에 묶이지 않는다(노드가 바깥 범위 아래). 중첩 `timeout`, 본문이 값을 낸 뒤 자식을 기다리는 중의 기한 → `Err timeout`, 본문이 끝난 뒤 자식이 `cancel(s)` → `Err cancelled`. `timeout(f, 0ms)`·음수 → `Err timeout`(음수의 문구는 SP15), 큰 `d`(Instant 넘침)는 기한 없음.
  - 11-08 CC1 고침(처리기 태스크): 핸들을 버려 생긴 E0607을 `supervise`가 범위가 열린 동안 곧바로 `on_fail`로 받는다(범위 안의 작업자가 채널로 받은 범위에 태스크를 띄우고 핸들을 버리는 모양).
  - `request_stop()`: 보는 쪽이 없으면 자식에서 불러도 종료 코드 0으로 끝남, 보는 쪽이 있으면 닫힌 끝·같은 끝(`stop_signal()` 재호출 `==`). 정지 채널을 `recv`·`for`·`take`·`wait_closed`로 기다리면 교착이 아니고, `timeout`으로는 끊긴다.
  - `exit(3)` 뒤 멈춘 `release` → 교착 보고(`in release, holds its own sending end`), 종료 코드 3 유지. 느린 `release`(타이머)는 `exit` 뒤에도 끝까지 돈다.
  - 오래 사는 루프의 누수: `timeout`+`with`+`scope`+`spawn`+`send`/`recv`를 30만 번 → 최대 RSS 9.8 MB로 1만 번과 같음(취소 노드·shield·타이머 더미가 쌓이지 않음).
  - 비용(릴리스, M-series): `spawn` 0.71 µs, `scope` 0.49 µs, `timeout` 0.37 µs, 맞교환 채널 주고받기 0.46 µs/값.
  - 인자 검사: `channel(-1)`, `close(rx)`, `cancel(1)`, `spawn(1, …)`, `supervise(…, 5)`, `timeout(() => 1, 1)`·`timeout(5, 1s)` 모두 E0403.
  - HTTP 클라이언트: 5초 자는 핸들러 앞에서 `timeout(…, 100ms)`이 100 ms 남짓에 `Err timeout`, `cancel(s)`로 서버 범위가 바로 정리된다.
  - `fan_in`: 소비자가 그만두면 한 전달 태스크의 `send` 실패가 `cancel(inner)`로 다른 전달 태스크(조용한 입력에서 `recv` 중인 것 포함)까지 끝낸다.
  - 표준 입력 읽기 스레드 시작 실패(11-08 CC7 고침), `CancelNode` 트리의 비재귀 취소, `WaitList`의 중복 방지·구멍 압축, 채널 `Line`, 풀의 버려진 일 압축, 타이머 더미의 오래된 기한 거르기를 코드로 확인.
  - 죽은 코드: 동시성 파일들의 함수는 모두 쓰인다(쓰이지 않는 것은 `#[cfg(test)]` 탐침뿐).
  - 모듈 초기화의 동시 import: import는 모듈 최상위에서 순서대로만 돌아 두 태스크가 같은 모듈을 처음 import하는 경주가 생기지 않는다(05:248의 "실행 중 순환" 오판 우려 없음, 코드 읽기).
- **표준 라이브러리**
  - TLS 검증: 사설 CA(시스템 저장소에 없음) → `unknown certificate authority`; `SSL_CERT_FILE=ca.pem`이면 `localhost`·`127.0.0.1`·`localhost.`·`LOCALHOST` 성공, `[::1]`(SAN에 없음) → ``certificate is not valid for `::1` ``; `SSL_CERT_FILE=/nope` → `no root certificates found (cannot read SSL_CERT_FILE `/nope`: …)`. `SSL_CERT_DIR`만 잘못되고 `SSL_CERT_FILE`이 루트를 주면 조용한 것은 `config`의 설계(실패할 때만 이유를 덧붙임)대로다.
  - 서버 요청 틀(`raw.py`): 중복 `Content-Length` 같은 값은 받음, `TE: chunked` + `Content-Length`는 청크(연결마다 요청 하나라 밀반입 없음), `TE` 두 줄 → 501, `Chunked`·앞뒤 공백 허용, 청크 확장·트레일러, 본문 뒤의 남는 바이트 무시, 첫 헤더 줄의 접힘 → 400, `Host :` → 400, 요청 줄의 겹 공백 → 400, `Content-Length: -1`·`0x3` → 400, `Host: a b` → 400, `%00`은 바이트 그대로, 소문자 메서드는 그대로(대소문자 구분), `HEAD`의 본문 없음.
  - 클라이언트: 짧은 본문(`Content-Length: 5`에 2바이트) → ``connection closed after 2 of 5 bytes``, 사용자 `Content-Length`·`Transfer-Encoding`은 쓰지 않음(문서대로), 11-08 LB2의 자격 증명 버리기(다른 포트), 큰 `stdin` + 출력의 `exec`(20 MB `cat`), 읽는 쪽이 일찍 끝나는 `stdin`(EPIPE를 조용히 처리).
  - `http.serve` 주소: `:0`은 `[::]`(이중 스택), `localhost:0`은 `127.0.0.1`, 포트 99999·포트 없음·없는 호스트는 `Err 'network`.
  - `path`: `join`(빈 인자, 절대 경로, `..`), `base`·`dir`·`ext`·`stem`의 빈 문자열·`/`·끝 `/`·`.` 시작 이름이 Go `path`와 같고 11-08 LB19 규칙(`stem + ext == base`)을 지킴.
  - `fs`: `list`·`walk`·`glob`의 순서와 경로 꼴(`./` 정리), `**`는 문서대로 `*`, `walk(파일)` → 그 파일, `stat`이 링크를 따라감, 끊긴 링크는 `'not_found`, 권한 없는 `exists` → `false`(문서대로; 동작 자체를 바꿀지는 SP16), `with_temp_dir`은 0700·`TMPDIR`이 없으면 `Err`, `copy`의 FIFO → `'invalid`, `fs.write`의 임시 파일이 `create_new`(O_EXCL)·소유자 확인.
  - `json`: `indent: 0`은 한 줄(문서대로), `-0` → `0`, `1E2` → `100.0`, `\u0000`, U+2028 그대로, `Ok`·범위 → E0513, 음수 `indent` → E0403.
  - `regex`: 이름 그룹이 안 잡히면 `nil`, `split`의 캡처 그룹은 결과에 넣지 않음, `$0`, `(?i)`, 글자 인덱스.
  - `time`: 형식 뒤의 남는 글·한 자리 필드·소수 10자리·`+24:00` → `Err 'parse`, 서머타임 빈틈(`02:30` → `03:30`), `Z`·`ZZ` 출력.
  - `_native.net`의 원시 22개는 모두 `extras/http.fung`이 쓴다(죽은 원시 없음).
- **CLI**
  - `-e` 뒤의 `--`는 `args`로 그대로, `fung test`의 인자 둘 → 3, 시험 없는 실행 → `no tests found`·1, REPL 파이프 모드의 종료 코드 1과 에러의 표준 에러 출력, `:help`.

## 12. 확인하지 못한 의심

- **명세**
  - SP16의 끊어진 심볼릭 링크: `exists`가 `metadata`(링크를 따라감)라 `false`인 것은 코드로만 봤다(재현 안 함).
  - `fs.with_temp_dir(1)`이 임시 디렉터리를 만든 뒤 E0401로 실패하고 `release`가 지우는지는 돌려 보지 않았다(SP18과 같은 원인으로 추정).
  - `**`의 결과 타입이 지수의 **값**에 따라 바뀐다(`2 ** 3`은 int, `2 ** -1`은 float). Python과 같고 07에 적혀 있어 내지 않았지만, "타입이 값에 따라 바뀌는 특례"로 다시 볼 만하다.
  - `close(x)`를 이미 닫은 채널 끝·파일에 두 번 부르는 경우의 반환 모양(파일은 `Err 'closed`로 문서화)과 채널 쪽이 같은지는 보지 않았다.
  - `read_line`은 끝에서 `Ok(nil)`, `recv`는 닫힌 끝에서 `Err('closed)`: "흐름의 정상적인 끝"을 두 모양으로 알린다. 05·06에 각각 이유가 있는지 찾지 못해 항목으로 내지 않았다.
  - (명세 영역이 남긴 "타이틀 케이스 글자 `ǅ`가 이름이 됨"은 SP17에, "`0..3 == 0..=2`가 `false`"는 VA21에 들어갔다.)
- **앞단**
  - REPL(`redefine` 모드)에서 FE23의 `own`·상수 규칙이 어떻게 되는지(REPL은 모든 입력의 이름을 상수로 봄) 보지 못했다.
  - `loader.rs` `clean`(어휘적 `..` 정리)이 심볼릭 링크 디렉터리를 지나는 `../`에서 OS 해석과 다를 수 있다(`./link/../x`). 11-08 FE5의 파일 정체 비교가 덮는지 재현하지 않았다.
  - 식별자에 ZWJ(U+200D)가 들어간 `x‍y`는 UAX #31(15.1 이후)대로 받아지지만 `xy`와 눈으로 구별되지 않는다. 혼동 문자 검사(UTS #39)를 둘지는 판단하지 않았다.
  - FE39의 실제 비용(모듈 트리 복사)은 재지 못했다.
- **VM**
  - `Rc<RtProto>`(프레임 → 클로저 → 원형) 이중 간접과 `frames[fi]`·`code[ip]` 경계 검사의 몫은 따로 떼어 재지 못했다.
  - VM16 (c)의 수치는 위치를 상수로 바꾼 판이라 상한이다. 실제로 에러 자리에서 `current_pos`로 구하는 판은 만들지 않았다.
  - VM15 (가)의 효과는 프로파일 비율로만 추정했다(인자 타입을 바꾸는 실험판은 내장 함수 90곳을 고쳐야 해서 만들지 않았다).
  - 측정은 macOS(시스템 할당기)에서만 했다. VM15의 할당 비용은 musl 배포판에서 더 클 수 있다(musl 할당기가 느린 것은 `bench-languages.md` 4.4).
- **값**
  - 원소 수 한도(E0806 elements)는 **범위 입력**에만 걸린다. 문자열은 11-08 VA9로 "모든 반환값"이 되었지만, 리스트 `+`(`xs + xs`를 거듭)·`flatten`(긴 리스트들)·채널에서 1억 넘게 받는 `map(rx, …)`는 한도가 없다(코드 읽기: `value/arith.rs` `add`의 리스트 갈래, `nat::limit_elements`). 1.6 GB가 넘는 시험이라 돌리지 않았다. 한도의 목적("죽는 대신 이유를 알림")으로 보면 문자열과 같은 규칙이 맞을 수 있다(결정 거리, LB29와 함께 볼 것).
  - HAMT `del`은 자식이 하나 남은 가지를 접지 않는다. 지운 뒤 찾기 경로가 길게 남지만 죽은 항목이 많아지면 `Omap::of`로 다시 지으므로 상한은 있다. 재지 않았다.
  - `values(m)`/`keys(m)`를 한 프로세스에서 되풀이하면 시간이 115~357 ms로 흔들렸다(앞 결과의 조각 해제 `release_piece`가 겹치는 것으로 보임). 원인을 가리지 않았다.
  - `zip(rx, …)`·`last(rx)`·`reverse(rx)` 등 받는 끝을 끝까지 받는 함수는 닫히지 않는 채널에서 끝나지 않는다. `drop`의 문서만 이것을 적고("닫히지 않는 채널에서는 끝나지 않는다"), 나머지는 "닫힐 때까지 받는다"로만 적었다. 문서 한 줄 거리로 보이지만 각 쪽을 다 확인하지 않았다.
  - (`float("1e-400")`이 조용히 `0.0`인 것은 FE31에 넣었다.)
- **동시성**
  - `CancelNode`가 `Arc` + `Mutex<HashMap>` + 원자 변수다. 다른 스레드가 만지는 길은 `Shared::root()` 하나로 보이는데(신호는 사건으로 게시되어 실행 스레드가 처리), 그렇다면 `Rc`/`RefCell`로 `timeout`(0.37 µs)·`scope`(0.49 µs)·`spawn`의 고정 비용을 줄일 수 있다. 대안을 빌드해 재지 못했다.
  - `timeout`이 끝날 때 `node.cancel()`이 그 노드에 등록된 **자기** waiter를 깨워(주차 때 `watch`) 가짜 차례가 하나 생기는 듯하다. `timeout(() => recv(rx), 1h)` 루프가 맨 `recv`보다 약 1.8배 느리다(20만 값 114 ms 대 205 ms). 이 중 `timeout` 자체 비용(약 0.37 µs)을 뺀 나머지가 가짜 깨움인지 계측하지 못했다.
  - REPL의 두 번째 Ctrl-C(`Sched::abandon`)가 `World::handler_requests`를 비우지 않는다. 버려진 평가의 `supervise` 범위 핸들이 전역 채널 버퍼 같은 곳에 살아 있으면 다음 평가의 `adopt`가 그 범위에 처리기를 띄워 엉뚱한 때 `on_fail`이 돌 수 있다(재현 안 함, 매우 드묾).
  - `conc.fan_in`에 입력이 하나뿐이고 조용하면, 소비자가 그만둔 뒤 생산자의 다음 `send`는 전달 태스크가 받아 성공하고 그 값은 버려진다. 생산자는 그다음 `send`에서야 `Err`를 본다(매뉴얼 "받는 쪽이 먼저 그만두면 … 생산자의 send도 Err를 받으므로"보다 한 값 늦음). 입력이 둘 이상이면(10 ms마다 보내는 생산자와 2초마다 보내는 생산자를 `fan_in`으로 합치고 첫 값만 받은 뒤 범위를 끝냄) 다른 전달 태스크의 실패가 곧바로 끊어 주어 드러나지 않았다.
  - Linux 전용 경로(epoll, pidfd, pidfd 없는 SIGCHLD 대기)는 macOS라 실행하지 못했다.
- **표준 라이브러리·CLI**
  - LB23의 해법이 `Content-Length`가 있는 응답에서 "close_notify 없이 닫는 유명 사이트"를 깨지 않는지: 길이를 아는 읽기는 그대로 두는 안이라 영향이 없어야 하지만, 실제 사이트로 확인하지 않았다(바깥 망을 쓰지 않음).
  - LB30의 https 쪽(사용자 `Proxy-Authorization`이 터널 안의 목적지로 감)은 코드 읽기다. 가짜 프록시가 `CONNECT`에 200을 준 뒤 TLS를 끝까지 중계하는 시험은 만들지 않았다.
  - `exec`의 `$PATH` 찾기는 부모의 `PATH`를 쓴다(`env: {PATH: …}`를 주어도 그 값으로 찾지 않음, `look_path`). Go와 같지만 문서(`process.md`)가 말하지 않는다. 의도인지 판단하지 않았다.
  - `fs.write`가 있던 파일의 모드를 `& 0o777`로 옮겨 setuid·setgid·sticky 비트를 잃는다(`write_whole` 245). 그런 파일을 `fs.write`로 바꾸는 쓰임이 있는지, 일부러 떨어뜨리는 것(안전)인지 판단하지 않았다.
  - 서버가 머리에서는 홀로 선 LF를 줄 끝으로 받지만(`GET / HTTP/1.1\nHost: h\n\n` → 200) 청크 줄의 홀로 선 LF는 400이다. RFC 9112 2.2가 둘 다 허용하는 "MAY"라 틀린 것은 아니지만 규칙이 둘이다. 클라이언트 쪽은 보지 않았다.
  - `fung test`가 로드 실패만 있을 때도 `scheduler seed N: …`을 찍는다(순서와 무관한 실패). 사소해 항목으로 내지 않았다.
  - REPL 줄 편집기(raw 모드)와 `~/.fung_history`는 가짜 터미널 없이 시험하지 못했다(지난 점검과 같음). EXDEV(11-08 LB8)도 돌리지 못했고, https → http 리다이렉트의 자격 증명은 따로 확인한 기록이 없다.
  - `http.serve`의 요청 본문 한도(16 MiB 고정)와 클라이언트 `max_body`(인자)의 비대칭: 업로드를 받는 서버는 한도를 바꿀 수 없다. 06에 "고정값"으로 적힌 결정이라 항목으로 내지 않았다.
