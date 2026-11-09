# 언어·VM 전체 점검 결과와 작업 목록 (2026-11-08)

0.5.0 릴리스 직후, 언어 명세와 구현 전체(앞단·컴파일러·VM·값 층·동시성 런타임·표준 라이브러리·CLI)를 "고칠 수 있는 문제" 기준으로 다시 점검한 결과다.
2026-10-20 검토(`docs/plans/2026-10-20-language-vm-review.md`)의 후속이고 형식도 같다. 새 세션이 이 문서만 읽고 항목 하나(또는 한 묶음)를 이어서 고칠 수 있게 썼다.
항목을 처리하면 체크하고 `처리:` 줄에 커밋과 바꾼 문서를 적는다.

## 0. 이 문서를 쓰는 법

- **기준 커밋**: `070fdb4`(0.5.0 릴리스). 줄 번호는 이 커밋 기준이고 함수 이름을 함께 적었다. 줄이 밀리면 함수 이름으로 찾는다.
- **범위**: 설계 문서 00~08, 매뉴얼 언어 명세(`manual/reference/language/`)·함수 레퍼런스, 앞단 전체, 컴파일러·바이트코드, VM·값 층·prelude,
  동시성 런타임, 표준 라이브러리 모듈(`fs`·`path`·`json`·`time`·`regex`·`http`·`cli`·`conc`), CLI 도구(`run`·`check`·`test`·`fmt`·`dump`·REPL).
  지난 검토와 달리 표준 라이브러리도 범위 안이다.
- **뺀 것**: open-questions "남은 일"·"알려진 한계", 지난 검토 10절(구조상 어쩔 수 없음)·11절(확인했고 문제없음), `docs/plans/0.5.0/README.md` 3절(0.5.0 뒤 새 기능).
  이미 고친 항목은 다시 내지 않았다. 다만 고친 것이 **불완전하거나 새 문제를 만든** 경우는 냈다(FE3·FE6이 지난 검토 F1·F4의 남은 부분).
- **방법**: 여섯 영역(명세·앞단·컴파일러와 VM·값과 prelude·동시성·표준 라이브러리와 도구)을 따로 점검하고, 모든 주장을 릴리스 바이너리로 재현했다.
  "재현 안 함(코드 읽기)"이라고 적은 것만 예외다. 영역 사이에 겹친 항목은 하나로 합쳤다. 중간 심각도 항목과 결정 항목의 대부분은 합친 뒤 한 번 더 돌려 확인했다.
  성능 수치는 Apple Silicon·macOS의 릴리스 빌드이고, "실험 판"은 저장소 사본에서 그 부분만 바꿔 빌드한 바이너리다(저장소에는 넣지 않았다).
- **재현**: `cargo build --release --locked` 뒤 저장소 루트에서 `F=target/release/fung`.
  - 여러 줄 소스는 `x.fung`로 저장해 `$F x.fung`. `$F -e '…'` 안의 줄바꿈은 그대로 붙여 넣는다.
  - 동시성 재현은 멈출 수 있으니 `perl -e 'alarm 10; exec @ARGV' $F x.fung`로 시간을 묶는다.
  - HTTP 재현은 로컬 루프백만 쓴다. 정해 둔 답을 차례로 보내는 서버 `canned.py`와 날 요청을 보내는 `raw.py`는 이 절 끝에 있다.
- **ID**: 지난 검토(L·T·F·V·C·P·D)와 0.5.0 목록(S·L)과 헷갈리지 않게 두 글자 접두사를 쓴다.
  SP 명세 · FE 앞단 · VM 컴파일러와 VM · VA 값과 prelude · CC 동시성 · LB 표준 라이브러리 · TL CLI 도구.
- **분류**
  - **버그**: 명세가 정답을 말한다. 결정 없이 TDD로 고친다.
  - **결정**: 언어 동작이 바뀌거나 명세가 비어 있다. CLAUDE.md 규칙대로 선택지 + 추천 + 이유로 사용자에게 묻고, 결정 뒤 해당 설계 문서를 고친다.
    각 항목의 선택지와 추천은 점검 때의 의견이고 결정이 아니다.
  - **성능**: 의미는 맞고 비용이 문제다. 측정 → 고침 → 같은 측정을 기록한다(`docs/notes/bench-languages.md` 3.4.1의 측정 규칙).
  - **정리**: 죽은 코드·중복·구조. 동작은 바뀌지 않는다.
  - **문서**: 문서끼리, 또는 문서와 구현이 어긋난다.
- **고칠 때의 공통 절차**(CLAUDE.md)
  - 시험을 먼저 쓰고 실패를 확인한다. 언어 동작은 `tests/lang/*.fung`의 `# expect:`·`# expect-error:`, 내부는 해당 모듈의 `*_tests.rs`.
  - 커밋 전 검사 전체(`cargo fmt --check`, clippy 두 번, `cargo test` 두 번, `cargo deny`·`cargo audit`).
  - 사용자에게 보이는 변화는 `CHANGELOG.md`의 `[Unreleased]`. 문법·내장 함수·내부 구조가 바뀌면 매뉴얼 원고를 고치고
    `FUNG_UPDATE_MANUAL=1 cargo test -p fung-cli --test manual_build`로 다시 만든 뒤 diff를 검토한다.
  - 새 에러 번호는 `crates/fung-front/src/diag/catalog.rs`에서 붙이고 `FUNG_UPDATE_SPEC=1 cargo test spec_file`. 이미 붙은 번호의 뜻은 바꾸지 않는다.
  - `.fung` 파일이나 출력이 바뀌면 기대 출력을 `FUNG_EXPECTED=bless …`로 다시 쓰고 diff를 검토한다. 명령 번호는 고정이다(`bytecode/op.rs` 머리 주석).
- **HTTP 재현 도우미**(임시 디렉터리에 저장해 `python3 -I`로 돌린다)
  ```python
  # canned.py PORT RESP...: 받은 요청을 stderr에 찍고, 정해 둔 답을 차례로 보낸다
  import socket,sys
  resps=[r.encode().decode('unicode_escape').encode('latin1') for r in sys.argv[2:]]
  l=socket.socket(); l.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1); l.bind(("127.0.0.1",int(sys.argv[1]))); l.listen(8)
  l.settimeout(8)
  for r in resps:
    try: c,_=l.accept()
    except Exception: break
    c.settimeout(2); d=b""
    try:
      while b"\r\n\r\n" not in d:
        x=c.recv(65536)
        if not x: break
        d+=x
    except Exception: pass
    sys.stderr.write(repr(d)+"\n"); sys.stderr.flush()
    c.sendall(r); c.close()
  ```
  ```python
  # raw.py PORT DATA: 날 요청을 보내고 받은 답을 그대로 찍는다
  import socket,sys
  port=int(sys.argv[1]); data=sys.argv[2].encode().decode('unicode_escape').encode('latin1')
  s=socket.create_connection(("127.0.0.1",port)); s.sendall(data)
  out=b""
  s.settimeout(3)
  try:
    while True:
      b=s.recv(65536)
      if not b: break
      out+=b
  except Exception as e: out+=b"<<"+str(e).encode()+b">>"
  sys.stdout.write(repr(out)+"\n")
  ```

## 1. 총평

언어 하나와 바이트코드 VM을 평가하는 입장에서 본 전체 인상이다. 개별 항목보다 이 절의 경향이 다음 작업의 방향을 정하는 데 더 쓸모 있다.

**잘 된 것.** 명세가 경계 사례까지 정밀하고, 에러가 번호·위치·도움말을 가진 카탈로그로 관리된다. 구조적 동시성(범위·기한·`with`·정지 신호)의 의미가 분명하고
런타임이 그 의미를 꽤 정확히 지킨다(이번에 확인한 넓은 범위에서 고리 검사·짝 맞추기·되감기의 구멍을 찾지 못했다). 시험 체계(골든·기대 출력·매뉴얼 펜스 실행·포매터 왕복)가 두텁고,
영속 벡터·맵·출력 왕복은 경계 크기와 무작위 입력으로 돌려도 틀리지 않았다. 마지막 읽기 이동(`MOVELOCAL`)은 넓은 구성에서 너무 이르게 놓는 경우가 없었다.
스택 기반 VM이라는 선택 자체는 문제가 아니다. 지금 병목은 명령 해석이 아니라 호출 경로의 할당이다(VM5~VM8).

**가장 흔한 문제: 규칙이 들어오는 길마다 다르게 적용된다(원칙 7).** 이번 항목의 3분의 1쯤이 이 모양이다.
한 규칙을 어느 한 경로에서 판정하고, 다른 경로로 같은 일을 하면 규칙이 빠진다.

- E0226(`map` 람다의 `?`)은 위치 인자만 보고 이름 인자 `f:`는 놓친다(SP1).
- 지난 검토 F1(블록 밖으로 나간 이름)의 고침은 같은 함수만 보고 람다 안은 놓친다(FE3).
- 문장 뒤 놓기(`CLEARLOCAL`)는 함수 본문의 문장에만 있어, 같은 코드를 `if` 가지나 최상위 블록으로 옮기면 교착한다(VM1·VM2).
- 1억 글자 한도는 연산마다 따로 검사해 `str`·`upper`로 넘는다(VA9). E0517은 `in`·`unique`·`==`가 다르게 판정한다(VA8).
- 리터럴 `'Red`는 되는데 변환 `sym("Red")`는 안 된다(SP4). 맵 리터럴은 문자열 키·중복 검사가 있는데 맵 패턴은 없다(SP3·FE12).
- `fs.move`는 덮지 않는데 `fs.copy`는 덮는다(LB7). 서버는 핸들러 헤더를 고쳐 쓰는데 클라이언트는 같은 경우를 거절한다(LB3).

고칠 때 항목 하나만 막지 말고 **판정을 한 곳으로 모으는 것**을 권한다. 예를 들면 생존 분석 하나로 모든 블록의 놓기를 계산하고(VM1), 한도 검사를 문자열 값을 만드는 한 자리에 두고(VA9),
같은 E0517 걸음으로 `==`와 판정을 함께 하고(VA8), 파서와 포매터의 이어짐 토큰을 한 상수로 나눈다(FE22).

**둘째: 조용한 변환이 아직 남아 있다(원칙 1).** 원칙은 잘 지켜졌지만 가장자리에 남은 것들이 있다.

- 언어: `0755`가 10진수 755가 되어 `perm:`으로 틀린 권한을 만든다(FE1). `1.`이 다음 줄을 필드로 삼킨다(FE2). 빈 `else:`가 값 자리에서 `nil`이 된다(FE4).
  보간이 `Ok(...)`/`Err(...)`를 글자로 넣는다(SP2).
- 라이브러리와 도구: `exit(256)`이 성공이 된다(TL1). `regex.replace`의 없는 그룹이 빈 문자열이 된다(LB10). `cli.parse`가 `_` 꼴 `bools`를 무시한다(LB11).
  `http` 타임아웃의 음수가 "제한 없음"이 된다(LB5).

하나하나는 작지만, 이 언어가 내세우는 "조용한 실패 없음"을 사용자가 가장 먼저 시험하는 자리들이다.

**셋째: 설계 문서가 결정을 따라가지 못했다.** 0.4.0의 E0517과 0.5.0의 바이트 문자열 뒤로 02·07·05에 낡은 문장이 남았다(SP5).
EBNF가 두 벌(01과 `grammar.md`)인데 서로 다르고 파서와도 어긋난다(SP8). 08의 에러 예시는 없는 번호를 보인다(SP7).
매뉴얼은 생성기가 실행해 지키지만 설계 문서는 아무도 지키지 않는다. 그래서 규범의 자리를 정하기를 권한다(결정). 하나는 설계 문서를 "결정의 기록"으로 두고, 규범은 매뉴얼 언어 명세 하나로 두는 것이다.
다른 하나는 EBNF처럼 두 곳에 있는 것을 시험으로 같게 지키는 것이다.

**넷째: VM 성능은 할당과 잠금에서 얻을 것이 크다.** 구조를 바꾸지 않고 얻을 수 있는 것들이다.

- 내장 함수 호출에 빠른 길이 없다. 고친 실험 판은 −25%였다(VM5). 기본값·이름 인자 호출은 호출마다 Vec 셋을 만든다(VM6).
- `Sym::name()`이 전역 Mutex를 잡는다(VM7). `Ok(x)` 하나에 할당이 두 번이다(VM8).
- 코드 생성에도 군더더기가 보인다: 조건 자리의 불 만들기, 반박할 수 없는 갈래의 `MATCH`, 상수 범위의 매 평가 할당(VM10).
- 슈퍼 명령이나 레지스터 VM보다 이것들이 먼저다(L15에서 레지스터 VM을 닫은 판단과 같다).

**다섯째: 동시성 런타임은 견고하다.** 남은 것은 경계 몇 곳이다.

- 풀 일이 끝나는 순간의 취소 확인이 빠졌다(CC2). `fs.copy`의 FIFO가 기한을 무시한다(CC3).
- 외부 명령의 `fork`가 실행 스레드에서 돌아, 힙이 크면 모든 태스크가 수십 ms 멈춘다(CC4).
- `supervise`의 E0607 처리가 범위가 닫힐 때까지 늦다(CC1). 두 번째 신호가 자식 프로세스를 고아로 남긴다(CC5).

**HTTP는 보안 쪽을 먼저 본다.** 리다이렉트가 포트·스킴이 바뀌어도 자격 증명을 보낸다(LB2). 서버가 핸들러 헤더의 CR/LF를 공백으로 바꾸고 NUL을 그대로 보낸다(LB3).
클라이언트가 쿼리를 인코딩하지 않아 요청 줄이 깨진다(LB1).

## 2. 추천 작업 순서

각 묶음 안의 항목은 서로 다른 파일을 건드리는 경우가 많아 병렬 세션으로 나눌 수 있다. 같은 파일을 건드리는 항목은 한 세션에 둔다.

1. **문서만 고치는 묶음**(결정 없음, 한 세션): SP5 SP6 SP7 SP8(구현을 정답으로 적는 부분) SP9 FE21 VA16 VM13 LB22.
   SP8의 "두 EBNF를 시험으로 같게"와 SP5의 "규범의 자리"는 결정으로 따로 묻는다.
2. **결정 없이 고칠 버그 묶음**. 넷은 파일이 겹치지 않는다.
   - 앞단(`lexer`·`parser`·`resolve`·`loader`): FE2 FE3 FE5 FE6 FE7 FE8 FE9 FE10 FE11 FE12 FE13 FE14 SP1.
   - VM·값(`vm`·`value`·`prelude`·`extras/prelude.fung`): VM3 VM11 VA2 VA3 VA4 VA6 VA7(`sum`의 결정 제외).
   - 동시성(`vm/offload.rs`·`native/fs.rs` copy·`vm/stdin.rs`): CC2 CC3 CC7. CC3과 LB6·LB7은 모두 `fs.copy`라 함께 하면 좋다(LB7은 결정).
   - 라이브러리·도구(`extras/http.fung`·`native/net.rs`·`fung-cli`): LB1 LB3 LB5 LB12 LB13 LB14 LB15 LB16 TL2 TL4.
3. **결정 묻기**. 영향이 큰 것부터.
   - 첫째: VM1·VM2(값의 수명 규칙: 블록 안 교착), LB2(자격 증명 유출), SP2(보간의 `Err`), FE1(`0755`), FE4(빈 블록), TL1(`exit(256)`).
   - 다음: CC1 CC5 LB7 LB8 LB9 LB10 LB11 SP3 SP4 FE17 FE18 FE20 VM4 VA8 VA9.
   - 나머지 결정 항목(SP10 CC6 LB17 LB18 LB19 LB20 TL3 TL5 VA14 VA15)은 한 번에 묶어 물어도 된다.
4. **성능**: VM5~VM9가 가장 크다(같은 `vm/call.rs`·`vm.rs`라 한 세션). 그다음 VA1, VA5, CC4, LB6, VM10, VA10~VA13, FE16.
5. **정리**: SP11 FE22 VM12 VA17 CC8 LB21. 해당 파일을 다른 일로 건드릴 때 함께 한다.

## 3. 목록

| ID | 한 줄 | 분류 | 심각도 |
|---|---|---|---|
| SP1 | `map`의 이름 인자 `f:`로 넘긴 람다의 `?`를 E0226이 놓친다 | 버그 | 중간 |
| SP2 | 보간·`join`이 `Ok`/`Err`를 말없이 글자로 넣는다 | 결정 | 중간 |
| SP3 | 맵 패턴은 문자열 키를 못 쓴다(리터럴은 됨) | 결정 | 중간 |
| SP4 | `'Red`는 되는데 `sym("Red")`는 E0513: 심볼 이름 규칙이 세 곳에서 다르다 | 결정 | 중간 |
| SP5 | 설계 02·05·07이 E0517·바이트 문자열 뒤로 낡았다 | 문서 | 중간 |
| SP6 | 07·06이 없는 `math` 모듈(비트 연산·`abs`·`round`)을 있는 것처럼 적는다 | 문서 | 낮음 |
| SP7 | 08의 에러 예시가 없는 번호 E0510과 나오지 않는 도움말을 보인다 | 문서 | 낮음 |
| SP8 | 두 벌의 EBNF가 서로 다르고 파서와도 어긋난다 | 문서 | 낮음 |
| SP9 | `kind` 어휘에 `'empty`·`'full`이 없고 "인자 타입 에러는 E0403"이 틀렸다 | 문서 | 낮음 |
| SP10 | `try_`가 두 뜻이고, 기간·대상 인자 자리가 함수마다 다르다 | 결정 | 낮음 |
| SP11 | `dbg`의 숨은 매개변수 `text`·`where`를 사용자가 줄 수 있다 | 정리 | 낮음 |
| FE1 | 앞자리 0 정수 `0755`가 조용히 10진수다 | 결정 | 중간 |
| FE2 | `1.`이 다음 줄의 이름을 필드로 삼킨다 | 버그 | 중간 |
| FE3 | 람다 안에서는 블록 밖으로 나간 이름이 조용히 prelude 함수가 된다 | 버그 | 중간 |
| FE4 | 값 자리의 빈 블록·`fn`·`assert`로 끝나는 블록이 조용히 `nil` | 결정 | 중간 |
| FE5 | 심볼릭 링크·대소문자 차이로 같은 모듈이 여러 번 적재된다 | 버그 | 중간 |
| FE6 | 괄호 안 줄바꿈 규칙의 남은 위반(`..` `]`, `not` `in`, 이름 `:`) | 버그 | 낮음 |
| FE7 | 줄 끝의 `in`(for)과 `|`(패턴·타입)에서 줄이 이어지지 않는다 | 버그 | 낮음 |
| FE8 | 비결합인 범위가 `1..2..3`을 받는다 | 버그 | 낮음 |
| FE9 | 최소 정수 리터럴의 접기가 `**`·후위 연산보다 먼저다 | 버그 | 낮음 |
| FE10 | 비UTF-8 바이트가 든 `test` 이름·`import` 경로가 빈 문자열이 된다 | 버그 | 낮음 |
| FE11 | `..._`가 `_`라는 이름을 묶어 거짓 에러를 낸다 | 버그 | 낮음 |
| FE12 | 맵 패턴의 중복 키를 받는다(리터럴은 E0113) | 버그 | 낮음 |
| FE13 | 홀로 선 CR이 주석은 끝내지만 줄바꿈이 아니다 | 버그 | 낮음 |
| FE14 | 문자열 끝의 `\`가 깨진 E0103 문구를 낸다 | 버그 | 낮음 |
| FE15 | E0110 "unexpected %s"에 문장을 넣어 깨진 문구가 여러 곳 | 정리 | 낮음 |
| FE16 | 깊은 괄호·보간 중첩이 이차 시간 | 성능 | 낮음 |
| FE17 | 원소 타입 표기(`list[lsit]`, `int[str]`)를 검사하지 않는다 | 결정 | 낮음 |
| FE18 | 태그 부분 적용 `Ok(_)`가 틀린 문구의 E0215로 거절된다 | 결정 | 낮음 |
| FE19 | 파이프의 태그 판정이 `char::is_uppercase`로 추측한다 | 정리 | 낮음 |
| FE20 | 같은 이름(과 빈 이름)의 `test` 블록을 받는다 | 결정 | 낮음 |
| FE21 | 01의 "camelCase 경고"가 없고 `format.rs` 머리 주석이 낡았다 | 문서 | 낮음 |
| FE22 | 포매터가 문법을 다시 판정하고 패턴 이름 모으기가 네 벌이다 | 정리 | 낮음 |
| VM1 | 중첩 블록의 문장은 `CLEARLOCAL`을 받지 않아 `if` 가지 안에서 교착한다 | 결정 | 중간 |
| VM2 | 진입 파일 최상위 블록의 지역은 넘기지도 놓지도 않는다 | 결정 | 중간 |
| VM3 | REPL: 실패한 전역을 람다가 붙잡으면 E0808 대신 E0801 내부 오류 | 버그 | 중간 |
| VM4 | `Ok(1, 2)!`는 튜플을 꺼내는데 패턴 `Ok(x)`는 맞지 않는다 | 결정 | 낮음 |
| VM5 | 바이트코드에서 부르는 내장 함수에 빠른 길이 없다 | 성능 | 중간 |
| VM6 | 기본값·이름 인자·`...rest` 호출이 호출마다 Vec 셋과 심볼 잠금 | 성능 | 중간 |
| VM7 | `Sym::name()`이 전역 Mutex를 잡는다 | 성능 | 낮음 |
| VM8 | 태그·튜플·클로저가 할당 두 번 | 성능 | 중간 |
| VM9 | 해석 루프의 작은 고정 비용 셋 | 성능 | 낮음 |
| VM10 | 코드 생성의 군더더기 | 성능·정리 | 낮음 |
| VM11 | 24비트를 넘는 피연산자를 조용히 자른다 | 버그 | 낮음 |
| VM12 | `Task`와 `Fiber`가 같은 필드 열여섯 개를 따로 둔다 | 정리 | 낮음 |
| VM13 | VM 설명 문서가 구현과 다른 곳 | 문서 | 낮음 |
| VA1 | 큰 맵의 제거 압축이 영속 사용에서 상각을 잃는다 | 성능 | 중간 |
| VA2 | `xs[..=-(len+1)]`이 E0507 대신 빈 결과 | 버그 | 중간 |
| VA3 | 슬라이스 끝의 리터럴 `nil`이 열린 끝이 된다 | 버그 | 낮음 |
| VA4 | `min`·`max`·`zip`이 범위를 펼쳐 E0806과 수 초 | 버그 | 중간 |
| VA5 | `trim`이 글자 표 전체를 만든다(50 MB에 900 MB) | 성능 | 중간 |
| VA6 | `parse_dur`가 주석과 `- 5s`를 받는다 | 버그 | 낮음 |
| VA7 | fung으로 쓴 prelude 몇이 인자를 검사하지 않아 내부 에러가 샌다 | 버그 | 낮음 |
| VA8 | 심볼·문자열의 E0517 판정이 연산마다 다르다 | 결정 | 낮음 |
| VA9 | 1억 글자 한도를 `str`·`upper`·`from_bytes`로 넘는다 | 결정 | 낮음 |
| VA10 | 조각으로 나누지 않은 긴 내장 함수가 알려진 한계 목록보다 많다 | 문서·성능 | 낮음 |
| VA11 | 한 글자 문자열마다 할당(5천만 글자에 4.6 GB) | 성능 | 낮음 |
| VA12 | `s[i]`가 늘 전체 글자 수를 먼저 센다 | 성능 | 낮음 |
| VA13 | 문자열 리스트의 `str`이 12배 느리다 | 성능 | 낮음 |
| VA14 | `0 ** -1`이 E0502가 아니라 E0504 | 결정 | 낮음 |
| VA15 | `max([nil])`이 `nil`이라 빈 입력과 구별되지 않는다 | 결정 | 낮음 |
| VA16 | 02의 "자르기는 O(1)"이 구현과 다르다 | 문서 | 낮음 |
| VA17 | VM 타입 표에 앞단이 거절하는 `conn`·`listener`가 있다 | 정리 | 낮음 |
| CC1 | `supervise`에서 처리기 없이 줄에 선 E0607이 범위가 닫힐 때까지 늦다 | 결정 | 중간 |
| CC2 | 시작한 "바꾸는" 풀 일 중의 취소·기한이 일의 값으로 새어 나간다 | 버그 | 중간 |
| CC3 | `fs.copy`가 FIFO에서 막혀 기한·취소를 영영 무시한다 | 버그 | 중간 |
| CC4 | 외부 명령의 `fork`가 실행 스레드에서 돌아 힙이 크면 모두 멈춘다 | 성능 | 중간 |
| CC5 | 두 번째 신호의 즉시 종료가 자기 그룹의 외부 명령을 고아로 남긴다 | 결정 | 중간 |
| CC6 | 쥐고만 있는 `stop_signal()` 끝이 Ctrl-C를 조용히 삼킨다 | 결정 | 낮음 |
| CC7 | 표준 입력 읽기 스레드를 못 띄우면 패닉하고 입력을 잃는다 | 버그 | 낮음 |
| CC8 | `TaskEnd`의 `scope: Option`은 늘 `Some`이다 | 정리 | 낮음 |
| LB1 | http 클라이언트가 쿼리를 인코딩하지 않아 요청 줄이 깨진다 | 버그 | 중간 |
| LB2 | 리다이렉트가 포트·스킴이 바뀌어도 Authorization·Cookie를 보낸다 | 결정(보안) | 중간 |
| LB3 | 서버가 핸들러 헤더를 말없이 고치거나 버리고, NUL은 그대로 보낸다 | 버그 | 중간 |
| LB4 | 서버의 `req.headers`에 `host`가 없다 | 결정·문서 | 중간 |
| LB5 | `http.*`의 `timeout: -1s`가 "제한 없음"이다(prelude와 반대) | 버그 | 중간 |
| LB6 | `fs.copy`가 파일 전체를 메모리에 읽는다 | 성능 | 중간 |
| LB7 | `fs.copy`는 덮고 `fs.move`는 거절한다 | 결정 | 중간 |
| LB8 | `fs.move`가 다른 파일 시스템 사이에서 늘 실패한다 | 결정 | 중간 |
| LB9 | 코드 오류가 `Err` 값으로 돌아오는 자리(파일 모드, glob 패턴) | 결정 | 중간 |
| LB10 | `regex.replace`의 없는 그룹(`$9`, `${nope}`)이 빈 문자열 | 결정 | 중간 |
| LB11 | `cli.parse`가 `_` 꼴 `bools`를 무시하고 빈 이름을 받는다 | 결정 | 중간 |
| LB12 | 핸들러의 1xx가 "1xx + 200" 두 답이 되고 HTTP/1.0에도 간다 | 버그 | 낮음 |
| LB13 | 서버가 HTTP/1.0 요청의 Transfer-Encoding을 무시해 본문을 잃는다 | 버그 | 낮음 |
| LB14 | 클라이언트가 `+20`·`999`·`HTTP/2.0` 상태 줄을 받는다 | 버그 | 낮음 |
| LB15 | `need_count`의 E0403이 값 대신 타입을 보인다 | 버그 | 낮음 |
| LB16 | 본문 한도를 보기 전에 `100 Continue`, HTTP/1.0 Expect에 417 | 버그 | 낮음 |
| LB17 | `fs.write`는 원자적이지 않다 | 결정 | 낮음 |
| LB18 | `fs.glob`은 읽지 못하는 디렉터리를 말없이 건너뛴다 | 결정 | 낮음 |
| LB19 | `path`의 끝 `/` 규칙이 함수마다 달라 `stem + ext != base` | 결정 | 낮음 |
| LB20 | `time`: 날짜 없는 형식의 LMT 오프셋, 윤초·잘못된 `TZ`의 침묵 | 결정·문서 | 낮음 |
| LB21 | 두 벌의 헤더 코드, 문구로 하는 판정, 낡은 주석 | 정리 | 낮음 |
| LB22 | 06 표의 http 서명이 늦고 쿼리 규칙이 매뉴얼에 없다 | 문서 | 낮음 |
| TL1 | `exit(256)`이 성공(0)으로 끝나고 08과 매뉴얼이 서로 다르다 | 결정·문서 | 중간 |
| TL2 | REPL에서 여러 줄 원시 문자열을 입력할 수 없다 | 버그 | 낮음 |
| TL3 | 표준 입력이 파이프인 REPL: 프롬프트가 섞이고 에러에도 0 | 결정 | 낮음 |
| TL4 | 남는 명령줄 인자를 명령마다 다르게 다룬다 | 버그 | 낮음 |
| TL5 | `fung test`가 시험 0개에 성공하고 잘못된 씨앗을 무시한다 | 결정 | 낮음 |

## 4. 언어 명세 (SP)

### SP1. `map`의 이름 인자 `f:`로 넘긴 람다의 `?`를 E0226이 놓친다 — 버그, 중간
- 재현:
  ```
  $F -e 'fn g(xs) = map(xs, f: s => parse_int(s)?); print(g(["1","x"]))'
  → [1, Err({kind: 'parse, message: "not an integer: x"})]
  $F -e 'fn g(xs) = map(xs, s => parse_int(s)?)'        → E0226 (비교)
  ```
  같은 함정이 남는 길이 둘 더 있다(결정 대상). `m = map; m(xs, s => parse_int(s)?)`와 `conc.pmap(xs, s => parse_int(s)?)`도 `Err`가 원소로 들어간다.
- 명세 근거: 03 "`map`에 바로 쓴 람다 안의 `?`도 컴파일 에러 E0226 … 막는 것은 prelude `map`의 인자 자리에 쓴 람다 리터럴(파이프 포함)뿐이다". 이름 인자 `f:`도 인자 자리다. `for.md`도 같은 문장이다.
- 원인 위치: `crates/fung-front/src/resolve.rs:462-469`(`Head::Call` 처리). `self.map_arg = to_map && k.head == Head::Fn …`이 인자 노드가 곧바로 `Fn`일 때만 켜진다. 이름 인자는 한 번 감싸져 있어 빠진다.
- 고치는 법: 이름 인자 노드면 그 값 노드를 보고 판정한다(이름이 `f`일 때만). `conc.pmap`은 결정이다.
  - (가) **추천**: `conc.pmap`의 `f` 자리도 같은 검사를 한다. `pmap`은 `map`의 병렬판이라 규칙이 "map 계열의 f" 한 줄로 남는다.
  - (나) 03에 "`pmap`은 해당 없음"을 적는다.
- **처리(2026-11-08, 사용자 결정 (가))**: `resolve.rs` `Head::Call`이 이름 인자 `f:`의 값도 보고, `map_like`가 prelude `map`과 `conc.pmap`(모듈 이름·별칭, `import conc (pmap)`의 전역 `pmap_globals`)을 가린다. E0226 문구에 함수 이름(`%s`). 시험 `resolve::tests::for_body_return_and_question_outside_fn`·`question_in_a_lambda_given_to_pmap`, `tests/lang/err_map_named_f_question.fung`·`err_pmap_question.fung`, 문서 03·`for.md`·`collections.md`·`conc.md`·`errors.md`.

### SP2. 보간·`join`이 `Ok`/`Err`(와 `nil`)를 말없이 글자로 넣는다 — 결정, 중간
- **처리(2026-11-08, 사용자 결정 (가))**: 새 번호 E0519. 보간(`Op::Concat`)과 `join`이 맨 값의 `Ok`·`Err`를 거절한다(`value::result_tag`). 시험 `tests/lang/err_interp_result.fung`·`err_join_result.fung`·`result_text.fung`, 문서 02·`results.md`·`convert.md`·`strings.md`·`errors.md`·CHANGELOG.
- 재현(점검 중에 실제로 밟았다):
  ```
  addr = recv(rx)               # Ok("127.0.0.1:50078")을 풀지 않음
  http.get("http://${addr}/")   # → Err 'network: invalid URL `http://Ok("127.0.0.1:50078")/`
  $F -e 'print("${Err(1)} ${nil}")'            → Err(1) nil
  $F -e 'print(join([nil, Err(1)], ","))'       → nil,Err(1)
  ```
  `Err`가 담긴 문자열은 E0606(버려지는 Err)에도 걸리지 않는다. 실패가 글자로 바뀌어 파일·명령 인자·URL로 흘러간다.
- 명세 근거: background 원칙 1, 02 "암묵적 형변환 없음", 02 nil "허용: `==` `!=`, `??`, `print`, 담기 … 그 외는 즉시 런타임 에러"(보간은 목록에 없다), 02 "버려지는 Err"의 취지.
  반면 `convert.md`·`strings.md`는 "보간도 str과 같은 규칙", "join은 문자열이 아닌 원소를 str로 바꿔 넣는다"고 적어, 원칙과 함수 문서가 어긋난다.
- 원인 위치: `crates/fung-vm/src/vm.rs:978` `Op::Concat`(보간), `crates/fung-vm/src/prelude/strings.rs` `join`. 둘 다 아무 값이나 `format`으로 글자로 바꾼다.
- 고치는 법(결정):
  - (가) **추천**: 보간과 `join`에서 `Ok`·`Err` 태그 값이면 런타임 에러를 낸다(새 번호, 도움말 "`!`·`?`로 풀거나, 일부러면 `str(x)`"). `str(x)`·`print`는 명시적 변환·표시라 그대로 둔다.
    흔한 실수(`recv`·`fs.read`·`exec`의 결과를 풀지 않음)가 그 자리에서 잡히고, 의도한 경우는 `str(r)` 한 단어로 된다.
  - (나) (가)에 더해 `nil`도 보간에서 에러로 한다. 02의 nil 목록과는 맞지만, 로그에 `nil`을 찍는 정상 용도가 있어 덜 추천한다.
  - (다) 그대로 두고 02에 "보간은 print와 같다"를 적는다.

### SP3. 맵 패턴은 문자열 키를 못 쓴다 — 결정, 중간
- **처리(2026-11-08, 사용자 결정 (가))**: `parser/pattern.rs` `pmap`. 시험 `tests/lang/map_pattern_string_keys.fung`, 문서 01·02·`grammar.md`·CHANGELOG.
- 재현:
  ```
  $F -e 'print(match {"a-b": 1}: {"a-b": v} => v; _ => 0 ;;)'   → E0112 expected a map key, found string `"a-b"`
  $F -e '{"a b": v} = {"a b": 1}'                                  → E0110 unexpected `=`
  ```
  맵 리터럴은 `{"a-b": 1}`을 받는다. 식별자 모양이 아닌 키(HTTP 헤더 `content-type`, JSON의 `"2xx"`, 공백이 든 키)는 패턴·구조 분해로 꺼낼 수 없고 `m["a-b"]`·`get`만 된다.
- 명세 근거: EBNF `entry = (key_name | STRING) ":" expr …` 대 `pat_entry = key_name ":" pattern | NAME`(01·`grammar.md`). 02 "축약할 수 없다(문자열 키)"는 축약 이야기이고, 문자열 키 패턴을 막는 이유는 어디에도 없다. 원칙 7.
- 원인 위치: `crates/fung-front/src/parser/pattern.rs:194-199` `pmap`(`at_name_like`만 받음).
- 고치는 법(결정):
  - (가) **추천**: `pat_entry = (key_name | STRING) ":" pattern | NAME`로 넓힌다(보간 없는 문자열만). 식과 같은 규칙이 되고, 응답 헤더를 패턴으로 가르는 코드가 된다.
  - (나) 02·`match.md`에 "패턴의 키는 식별자 모양만"과 이유를 적는다.

### SP4. 심볼 이름의 규칙이 세 곳에서 다르다(`'Red`는 되는데 `sym("Red")`는 E0513) — 결정, 중간
- **처리(2026-11-08, 사용자 결정 (가))**: `prelude/convert.rs` `is_name`이 대문자 시작도 받는다. VA 쪽의 같은 항목(D5)도 이것으로 닫힘. 시험 `tests/lang/sym_tag_names.fung`, 문서 01·`lexical.md`·`convert.md`·CHANGELOG.
- 재현:
  ```
  $F -e "print('Ok, 'Ok == Ok, 'if, sym(\"if\"))"     → 'Ok true 'if 'if
  $F -e 'print(sym("Red"))'                           → E0513 cannot convert "Red" to sym
  $F -e 'type t = Red | Green
  fn f(c: t) = c
  print(f(sym("red")))'                               → E0403 argument `c` must be Red | Green, got sym
  ```
  규칙이 셋이다. 렉서는 `'` 뒤에 식별자면 무엇이든 받는다(대문자 시작·예약어 포함). `sym`은 "소문자 이름 모양"만 받는다(예약어는 됨, 대문자는 거절).
  01·`lexical.md`는 "`'` 뒤에 이름 규칙"이라고 적는다(이름은 소문자로 시작하고 예약어가 아님).
  그 결과 인자 없는 태그 `Red`는 심볼 `'Red`인데(02, 07 T1), 바깥 문자열 `"Red"`를 그 값으로 바꿀 수 없다. 설정·JSON에서 `type color = Red | Green`의 값을 읽는 길이 막힌다.
  E0517의 도움말은 `sym(s)`를 권하는데, 바로 그 길이 닫혀 있다.
- 명세 근거: 01 "심볼: `'` 뒤에 이름 규칙", 02 "`Red` 인자 없으면 심볼 'Red", 07 T1(열거형 = 태그 심볼), `convert.md` `sym`("대문자로 시작하는 것은 E0513").
- 원인 위치: `crates/fung-front/src/lexer/string.rs:216` `symbol`, `crates/fung-vm/src/prelude/convert.rs:144-162` `is_name`·`sym_fn`.
- 고치는 법(결정):
  - (가) **추천**: `sym`이 렉서와 같은 규칙(식별자 모양이면 대문자도)을 받고, 01·`lexical.md`를 "`'` 뒤에 식별자(이름·태그 모양, 예약어도)"로 고친다.
    리터럴로 만들 수 있는 값은 변환으로도 만들 수 있어야 한다(원칙 7). 인턴 누수 경고는 그대로 둔다.
  - (나) 그대로 두고 매뉴얼에 `match s: "Red" => Red …`로 손으로 옮기라고 적는다.
  - (다) 대문자 심볼 리터럴 `'Red`를 금지한다. 그래도 `Red`가 `'Red`로 표시되는 문제가 남아 비추천이다.

### SP5. 설계 02·05·07이 0.4.0(E0517)·0.5.0(바이트 문자열) 뒤로 낡았다 — 문서, 중간
- 낡은 문장과 실제(모두 실행해 확인):
  - 02:39 "`'ok == "ok"`는 거짓" → 실제는 E0517이다. 같은 문서 아래 표도 E0517이라 문서 안에서 모순이다.
  - 07:50 "타입이 다르면 에러가 아니라 `false`" → E0517 예외가 빠졌다.
  - 02:140 비교 표 "str | 글자열 | 코드 포인트 순" → L4 뒤로 바이트 순이다(02 "문자열" 절과 `values.md`는 이미 "바이트 순").
  - 02:45 nil "그 외(… `!`, `?`)는 즉시 런타임 에러" ↔ 같은 문서의 표와 `results.md` "`nil`에 `x?`는 `nil` 그대로". 실제 `fn f() = nil?; print(f())` → `nil`.
  - 02:102 "`for`, `map`, `filter`, `fold` 등이 받는 것은 리스트와 범위뿐(for는 받는 끝도)" → 실제로는 `map`·`filter`·`fold`·`find`·`sum`·`take`·`first` 모두 받는 끝을 받는다(`collections.md`는 그렇게 적는다).
  - 02:114 `map(0..10^18, 1)`: `^`는 fung에 없는 연산자다(`**`).
  - 07:30 "`int(실수)`가 범위를 넘을 때" 정수 넘침(E0503) → 실제 `int(1e19)`은 E0513이다(`convert.md`와 같음).
  - 05:151 `Err({kind: 'error', message: …})`: 따옴표가 하나 더 있고, L11 뒤의 `code`·`at`이 빠졌다(같은 문서 136줄은 맞다).
  - 05:260~262 효과 목록에 `try_send`·`try_recv`·`wait_closed`·`request_stop`이 없다(`vm/atom.rs` `EFFECTS`에는 있다).
    반대로 `stop_signal`은 순수 표(`PURE`)에 있는데 부르면 런타임의 "보는 쪽"을 바꾼다(CC6). 왜 순수인지 근거 한 줄이 필요하다.
  - 05:132~133 "본문이 이미 값을 돌려준 뒤 `cancel(s)`돼도 결과는 `Err({kind: 'cancelled})`" 문장이 L14 "차례는 한 바퀴에 한 번" 항목 아래에 붙어 있다. 취소 항목(124~127행) 자리로 옮긴다.
- 명세 근거: CHANGELOG 0.4.0 "심볼과 문자열 … E0517", 0.5.0 "문자열은 바이트 열". CLAUDE.md "결정 후 해당 문서를 갱신".
- 고치는 법: 위 문장들을 `values.md`·`results.md`·`collections.md`·실제 값과 같게 고친다.
  - 결정 하나를 함께 묻는다. 설계 문서와 매뉴얼 언어 명세 중 **어느 쪽이 규범인지** 정한다(1절 셋째 단락).
  - **추천**: 매뉴얼 언어 명세를 규범으로 둔다. 생성기가 펜스를 실행해 지키기 때문이다. 설계 문서는 "결정과 이유의 기록"으로 두고, 규칙을 옮겨 적은 문장은 매뉴얼로 링크한다.
- **처리(2026-11-08, 사용자 결정: 매뉴얼 언어 명세가 규범)**: 02(심볼 E0517·nil의 `?`·바이트 순·받는 끝·`10**18`)·05(`'error` 결과 모양·효과 목록·취소 문장 자리)·07(`int(실수)` E0513·E0517)을 실제와 맞춤. `docs/design/README.md`에 "규범은 매뉴얼, 설계 문서는 결정의 기록". `stop_signal`은 근거를 적는 대신 효과로 옮김(`vm/atom.rs`, `tests/lang/err_atom_stop_signal.fung`).

### SP6. 07·06이 없는 `math` 모듈을 가리킨다; 비트 연산·`abs`·`round`가 아예 없다 — 문서(+결정), 낮음
- 재현: `$F -e 'import math'` → E0301 cannot find module `math`. prelude에도 `abs`·`floor`·`round`·`sqrt`·비트 연산이 없다(`spec/builtins.txt`).
- 명세 근거: 07:82-84 "비트 연산: 연산자 없음. `math.band` … 등 함수로"(있는 것처럼 씀), 06:123 "`math`, `random` (M8 이후)". open-questions에 항목이 없다.
- 고치는 법: 07을 "아직 없음, 06의 `math` 후보"로 고치고, open-questions "알파에서 써 보며 조정할 것"에 `math`(abs·floor·ceil·round·비트)를 올린다.
  지금은 `int(x)`(0 쪽 버림)만 있어 `round`·`floor`를 직접 짜야 한다. SRE 스크립트(퍼센트·반올림·권한 비트)에서 자주 부딪힐 것이다.
- **처리(2026-11-08)**: 07 비트 연산을 "아직 없음, `math` 후보"로, 06 표의 `math`·`random` 줄, open-questions "알파에서 써 보며 조정할 것"에 `math` 항목.

### SP7. 08의 에러 예시가 없는 번호 E0510과 나오지 않는 도움말을 보인다 — 문서, 낮음
- 재현: `printf 'fn f(size) = size // 1024 + "kb"\nprint(f(3))\n' > e.fung; $F e.fung` → `error[E0501]: cannot add int and str`, 도움말 없음.
- 명세 근거: 08:24-34 예시 `error[E0510]: cannot add str to int: 3 + "kb"` … `help: use str(...) or "${...}"`, 08:61 번호 표 "타입 불일치 `E0510`". `spec/errors.txt`에 E0510은 없다.
- 고치는 법: 예시와 표를 실제 출력(E0501)으로 바꾼다. 도움말은 결정이다.
  - **추천**: `str + 비str`일 때만 `help: use "${…}" or str(…)`을 붙인다. 흔한 실수이고 `operators.md` 경계 사례가 바로 이 예다.
  - 원인 위치는 `crates/fung-front/src/diag/catalog.rs`의 E0501, 또는 VM의 `+` 타입 에러를 만드는 자리다.
- **처리(2026-11-08)**: `value/arith.rs` `add_error`가 한쪽만 글인 `+`의 E0501에 도움말을 붙임(시험 `adding_text_and_another_value_has_a_help`), 08의 예를 실제 출력(E0501)으로, 표의 `E0510`을 `E0501`로.

### SP8. 두 벌의 EBNF(01·`grammar.md`)가 서로 다르고 파서와도 어긋난다 — 문서, 낮음
- 파서가 EBNF보다 넓게 받는 것(모두 실행해 확인):
  - `xs[..=2]` → `[1, 2, 3]`. EBNF `index`는 `..=`에 양 끝을 요구한다(`expr "..=" expr`). `access.md` 목록에도 없다.
  - `fn f() -> map | nil`, `fn f(x: fn)`. EBNF `type_term = NAME … | TAG`인데 `nil`·`fn`은 예약어라 NAME이 아니다(`parser/fn_.rs:207`이 따로 받음).
  - 끝 쉼표: 리스트·맵 패턴, 람다 매개변수, `[...rest,]`.
  - 리터럴 매개변수 패턴: 람다 `(1) => 0`·`('a) => 0`은 되고 `fn f(1) = 0`은 E0112다. EBNF `param = … | pattern`은 둘 다 받는다고 말한다(`pattern_param_ahead`가 튜플·리스트·맵·태그·`_`만 받음).
- 파서가 EBNF보다 좁게 받는 것:
  - 절 0개 `match 1: ;;`와 빈 절 `_ => ;;`. EBNF는 `{ arm }`이고 `body`는 비어도 된다.
  - `import path (base,)`는 E0112다. 호출·매개변수·리스트·맵은 끝 쉼표를 받는다.
  - 보간 문자열을 맵 키(`{"${1}": 2}`), 패턴, 테스트 이름에서 거절한다. EBNF는 `STRING(보간 조각 포함)` 하나만 쓴다.
- 두 EBNF 사이의 차이: `grammar.md`는 `entry … | NAME | TAG`, `pat_entry … | NAME | TAG`이고 01은 `| NAME`(대문자는 E0225)이다. 매뉴얼은 "파서가 그대로 옮긴 EBNF"라고 말한다.
- 그 밖:
  - `grammar.md` 첫 줄 "식의 층은 **위** 우선순위 표와 같다": 쪽을 나눈 뒤 그 쪽에 표가 없다(`operators.md#precedence`로 링크해야 한다).
  - 줄 이어짐 규칙 1의 토큰 목록에 `then`·`else`가 없는데, 줄 끝 `then`도 잇는다. 목록이 닫힌 목록처럼 읽힌다.
  - `lexical.md` 경계 사례의 이스케이프 목록에 `\r`이 빠졌다(본문에는 있다).
  - `[ expr ] ".."`가 `..`보다 약한 연산자를 시작으로 받는다는 것(`xs[a ?? 0..]`)은 01 본문에만 있다.
- 명세 근거: 01 "형식 문법"("파서는 이 문법을 그대로 옮긴다").
- 원인 위치: `docs/design/01-syntax.md:404-476`, `manual/reference/language/grammar.md`·`blocks.md`·`lexical.md`. 파서 쪽은 `parser/expr.rs:503-509`, `parser/pattern.rs:173·183·218·319·328`, `parser/fn_.rs:307`.
- 고치는 법: 구현을 정답으로 두 EBNF를 같게 고친다. 끝 쉼표는 어디서나 허용으로 통일하고(`import` 목록 포함), `[..=b]`·`nil`·`fn` 타입·보간 금지 자리를 적는다.
  빈 `match`·빈 절은 FE15에서 전용 문구로 막고 EBNF를 `arm { arm }`으로 고친다.
  - 다시 어긋나지 않게(결정): **추천**은 EBNF를 매뉴얼 한 곳에만 두고 01은 그곳을 링크하는 것이다. 다른 길은 두 벌이 같음을 시험으로 지키는 것이다.
- **처리(2026-11-08, 사용자 결정: EBNF는 매뉴얼 한 곳)**: `manual/reference/language/grammar.md`를 파서에 맞게 다시 씀(끝 쉼표, `[..=b]`, `nil`·`fn` 타입, `PLAIN_STRING` 자리, 절 하나 이상·빈 본문 없음, 범위 비결합, `...`의 자리, 우선순위 표 링크). 01의 EBNF는 링크로. 파서는 `import` 목록 끝 쉼표를 받고(`parser/stmt.rs`), 람다 매개변수를 `fn`과 같게(`parser/fn_.rs`; 시험 `lambda_and_fn_parameters_take_the_same_patterns`·`an_import_list_takes_a_trailing_comma`). `blocks.md` 이어짐 규칙 1에 `then`·`else`, `lexical.md` 이스케이프 목록에 `\r`.

### SP9. `kind` 어휘에 `'empty`·`'full`이 없고 "인자 타입 에러는 E0403"이 틀렸다 — 문서, 낮음
- 재현:
  ```
  $F -e '(tx, rx) = channel(1); print(try_recv(rx))'                     → Err({kind: 'empty, …})
  $F -e '(tx, rx) = channel(1); send(tx, 1); print(try_send(tx, 2))'      → Err({kind: 'full, …})
  $F -e 'print(len(1))'              → E0506 cannot take the length of int
  $F -e 'print(put([1], "a", 1))'    → E0506
  ```
- 명세 근거: `manual/reference/functions.md:12`와 06:71-72의 `kind` 어휘 12개에 `'empty`·`'full`(L3)이 없다. `functions.md:13`은 "인자 타입이 틀린 것은 코드 오류라 런타임 에러(E0403)다"라고 하는데, 컬렉션 함수들은 E0506이고 일부는 E0501이다.
- 고치는 법: 어휘에 둘을 더한다. 13줄은 "E0403(매개변수 표의 타입), 여러 타입을 받는 함수는 E0506"처럼 실제를 적는다. 번호는 불변 규칙이 있으니 문서 쪽을 고친다.
- **처리(2026-11-08)**: 06과 `functions.md`의 `kind` 어휘에 `'empty`·`'full`, `functions.md`의 인자 타입 에러 문장을 "매개변수 표의 타입은 E0403, 여러 타입을 받는 함수는 E0506"으로.

### SP10. `try_`가 두 뜻이고, 기간·대상 인자 자리가 함수마다 다르다 — 결정, 낮음
- 내용(`spec/builtins.txt`):
  - `try_map(xs, f)`는 "실패할 수 있는 변환, 첫 Err에서 멈춤"이고, `try_recv`·`try_send`는 "기다리지 않음"이다.
  - `timeout(d, f)`는 기간이 먼저이고, `conc.retry(f, times, delay)`는 함수가 먼저다.
  - `close(tx)`(채널)와 `fs.close(f)`(파일)가 서로의 값을 E0403으로 거절한다.
  - prelude `read_lines(f)`는 핸들을, `fs.read_lines(path)`는 경로를 받는다.
- 명세 근거: `operators.md` "내장 함수는 모두 다루는 대상을 첫 번째 매개변수에 둔다"(`timeout`의 대상은 `f`), 원칙 6·7.
- 고치는 법(결정):
  - (가) 이름을 정리한다. 기다리지 않는 쪽을 `poll_recv`/`poll_send`(또는 `recv_now`)로 바꾸고, `try_`는 "Result를 모으는 변형"에만 쓴다. `close`는 채널과 파일을 함께 받게 하거나 `fs.close`를 없앤다.
  - (나) **추천**: 그대로 두고 함수 레퍼런스 "읽는 법"에 두 뜻을 적는다. `timeout`의 인자 순서만 결정에 올린다.
    `timeout(1s, () => …)`는 Trio·Go의 읽는 순서와 같아서, 파이프 규칙의 예외로 문서에 적는 편이 비용이 작다.
    하위 호환을 따지지 않는 지금이 바꿀 때라는 점은 (가)의 근거다.
- **처리(2026-11-08, 사용자 결정)**: `timeout(f, d)`, `send_now`·`recv_now`, `close(x)` 하나(`conc.rs`, `native/fs.rs` `close_fn`, `extras/fs.fung`에서 `close` 뺌), 매뉴얼·예제·시험의 모든 호출, `tests/lang/try_recv.fung` → `recv_now.fung`.

### SP11. `dbg`의 숨은 매개변수 `text`·`where`를 사용자가 줄 수 있다 — 정리, 낮음
- 재현:
  ```
  $F -e 'x = dbg(1, text: "lie", where: "nowhere:9")'     → [nowhere:9] lie = 1
  $F -e 'x = map([1, 2], dbg)'                            → "dbg = 1" (위치 없음)
  ```
- 명세 근거: `testing.md`·`io.md` `dbg(x)` "식의 소스 텍스트와 값을 찍는다". `functions.md` "모든 서명은 런타임의 실제 매개변수 표와 맞춰 본다(테스트가 지킨다)"인데, `crates/fung-cli/tests/manual.rs:220-224`가 `dbg`만 서명을 바꿔 끼운다.
- 원인 위치: `crates/fung-vm/src/prelude/io.rs:29`, 컴파일러의 채우기 `crates/fung-front/src/compiler/m6.rs`.
- 고치는 법: 컴파일러가 채우는 값을 이름 인자가 아니라 숨은 경로로 넘긴다(전용 명령이나, 사용자가 이름으로 줄 수 없는 매개변수 이름). 값으로 쓴 `dbg`의 출력 모양을 `io.md`에 적는다.
- **처리(2026-11-08)**: prelude `dbg`는 `dbg(x)`, 직접 부른 `dbg(e)`는 컴파일러가 새 상수 `Const::Dbg`(dump에는 `<fn dbg>`)로 숨은 3인자 내장 함수(`prelude::dbg_at`)를 부른다. `atom.rs`의 효과 표에 `dbg_at`도(E0707). `crates/fung-cli/tests/manual.rs`의 `dbg` 서명 바꿔 끼우기 삭제. 시험 `prelude/run_cases.rs`(숨은 인자 두 사례: E0402·E0406), `atom_tests.rs`(`dbg`가 swap 안에서 E0707), 기대 출력 bless. 문서 `manual/reference/prelude/io.md`(값으로 넘긴 `dbg`의 출력 모양과 예제), 설계 08, `spec/builtins.txt`.

## 5. 앞단 (FE) — 렉서·파서·이름 해석·로더·포매터·진단

### FE1. 앞자리 0 정수 `0755`가 조용히 10진수 755다 — 결정, 중간
- **처리(2026-11-08, 사용자 결정 (가))**: `lexer/number.rs` `Lexer::number`가 정수부를 읽은 뒤 검사한다(도움말 `0o755`). 시험 `lexer::tests::leading_zero_is_e0102`·`tests/lang/err_number_leading_zero.fung`, 문서 01·`lexical.md`·CHANGELOG.
- 재현:
  ```
  $F -e 'print(0755, 007)'                       → 755 7
  $F -e 'import fs
  fs.write("p.txt", "x", perm: 0444)!'           → 성공. ls -l: -rw-rwxr-- (0o674 = 444)
  ```
  `perm: 0644`·`0600`은 우연히 0o777을 넘어 E0403(`got 0o1204`)으로 잡힌다. 하지만 0o777 이하로 떨어지는 `0444`·`0440`·`0400`·`0500`은 조용히 틀린 권한으로 파일을 만든다.
  셸(`chmod 0755`)과 Go(표면 문법의 기준 언어로, `0755`는 8진수)에서 익힌 표기다. F-14(`perm`)를 들이면서 생긴 함정이다.
- 명세 근거: 원칙 1. 01 "숫자"와 `lexical.md` 숫자 표는 앞자리 0을 말하지 않는다.
- 원인 위치: `crates/fung-front/src/lexer/number.rs:35-99` `Lexer::number`.
- 고치는 법(결정):
  - (가) **추천**: 두 자리 이상인 10진 정수·기간·실수의 정수부가 `0`으로 시작하면 E0102를 내고 도움말 "8진수는 `0o755`"를 붙인다(Python 3와 같음). `0`, `0.5`, `0s`, `0x…`는 그대로 둔다.
  - (나) 8진수로 읽는다(Go 옛 규칙). 같은 일을 하는 길이 `0o`와 둘이 되어 원칙 6과 부딪힌다.
  - (다) 그대로 두고 문서에 경고만 적는다.
  - 함께 닫을 것: `-0x8000000000000000`은 E0102인데 `-9223372036854775808`은 된다(지난 검토 F6의 남은 의심). 진법 리터럴은 i64 범위만이라고 01에 한 줄로 적는다.

### FE2. `1.`이 다음 줄의 이름을 필드로 삼킨다 — 버그, 중간
- 재현:
  ```
  printf 'x = 1.\nprint(x)\n' > dot.fung; $F dot.fung
  → error[E0204]: undefined name `x`  --> dot.fung:2:7
  ```
  `1.`(01이 금지한 모양)이 E0102가 아니라 `1 .` + 줄 끝 `.`의 이어짐으로 읽혀 `x = 1.print(x)`가 된다. 에러는 엉뚱한 줄의 엉뚱한 이름을 가리킨다.
  같은 줄의 `print(1.e5)`는 실행해야 `E0509 cannot index a value of type int`가 난다.
- 명세 근거: 01 "숫자"(`.5`, `1.` 불허), `lexical.md` 숫자 표("`1.`은 안 됨").
- 원인 위치: `lexer/number.rs:39`(`.` 뒤가 숫자가 아니면 정수로 끝냄) + `parser/expr.rs:237` `field`(`.` 뒤 줄바꿈을 건너뜀).
- 고치는 법: 렉서에서 10진 정수 바로 뒤의 `.`가 숫자도 `.`(범위 `..`)도 아니면 E0102(`1.`, `1.e5`, `1.foo`). 정수에는 필드가 없으므로 잃는 것이 없다.
- **처리(2026-11-08)**: `lexer/number.rs` `Lexer::number`가 10진 정수 뒤의 `.`(숫자·`.`가 아닌 것이 뒤따르면)를 E0102로. 시험 `lexer::number::tests::malformed_numbers`·`malformed_number_error_shows_the_whole_token`, `tests/lang/err_number_trailing_dot.fung`, 문서 01·`lexical.md`·`errors.md`.

### FE3. 람다·안쪽 함수에서는 블록 밖으로 나간 이름이 조용히 prelude 함수가 된다 — 버그, 중간
- 재현(지난 검토 F1의 고침이 불완전하다):
  ```
  $F -e 'if true: lines = ["a"] ;;
  g = () => lines
  print(g())'                                   → <fn lines>
  $F -e 'if true: lines = ["a"] ;;
  print(map([1], _ => lines))'                  → [<fn lines>]
  $F -e 'if true: lines = ["a"] ;;
  print(lines)'                                 → E0206 (같은 함수 안이면 잡힌다)
  $F -e 'if true: x = 1 ;;
  g = () => x'                                  → E0204 (E0206이어야 할 자리)
  ```
- 명세 근거: 03 "스코프와 클로저" 규칙 4, 지난 검토 F1.
- 원인 위치: `crates/fung-front/src/resolve.rs:629` `Resolver::name`. `closed`를 `self.cur()`(지금 함수)에서만 본다. 바로 아래 E0208의 `later`는 모든 `fns`를 본다.
- 고치는 법: `self.fns.iter().any(|f| f.closed.contains(name))`로 바깥 함수의 닫힌 이름도 본다. 최상위 `fn`은 파일 변수보다 먼저 만들어지므로 그 시점의 `fns[0].closed`는 비어 있어 결과가 바뀌지 않는다.
- **처리(2026-11-08)**: `resolve.rs` `Resolver::name`이 모든 `fns`의 `closed`를 본다. 시험 `resolve::tests::a_closed_name_is_not_read_as_the_prelude`, `tests/lang/err_closed_name_in_lambda.fung`.

### FE4. 값 자리의 빈 블록·`fn` 정의·`assert`로 끝나는 블록이 조용히 `nil` — 결정, 중간
- **처리(2026-11-08, 사용자 결정 (가))**: `resolve/value.rs`. 대입은 지금처럼 E0224(번호의 뜻을 넓히지 않으려고), 빈 블록·`assert`·`fn` 정의로 끝나는 블록은 새 번호 E0231. 숨은 대입은 세지 않는다. 걸린 예제 둘(`bigfiles`·`staticfiles`의 `with_temp_dir` 콜백)과 배우기 14장에 `nil`. 문서 01·`blocks.md`·`errors.md`·CHANGELOG.
- 재현:
  ```
  $F -e 'x = if false: 1 else: ;;
  print(x)'                                     → nil (else 없는 if는 E0223인데 빈 else는 통과)
  $F -e 'fn f(n) = if n > 0: n else: ;;
  print(f(-1))'                                 → nil
  $F -e 'x = if true: fn g() = 1 else: 2 ;;
  print(x)'                                     → nil
  $F -e 'x = if true: assert true else: 2 ;;
  print(x)'                                     → nil
  $F -e 'fn f((a, b)): ;;'                      → E0224 "`this pattern` is assigned as the last statement…" (대입을 쓰지 않았다)
  $F -e 'fn f(): ;;
  print(f())'                                   → nil (같은 빈 본문인데 에러 없음)
  ```
  `match`의 빈 절은 문법 에러("an empty match arm")라, 세 블록 종류가 서로 다르다.
- 명세 근거: 01 "식과 문장"(값을 쓰는 자리의 nil은 조용한 버그: E0223·E0224, "nil이 의도라면 `else: nil`"), 원칙 7. 01은 대입만 적어서, 다른 문장(`fn` 정의·`assert`)과 빈 블록은 비어 있다.
- 원인 위치: `crates/fung-front/src/resolve/value.rs:17-75` `check`(`Head::Let`만 E0224를 내고, 빈 `Do`와 이름 있는 `Head::Fn`·`Head::Assert`는 보지 않음).
  패턴 매개변수의 엉뚱한 문구는 `desugar.rs:485-489` `fn_node`가 본문 앞에 넣는 숨은 `Let fresh`를 `value::check`가 사용자 대입으로 보기 때문이다.
- 고치는 법(결정):
  - (가) **추천**: 규칙을 "값 자리의 블록은 식으로 끝나야 한다" 하나로 한다. 빈 블록, 대입·`fn` 정의·`assert`로 끝나는 블록 모두 E0224(문구를 "ends in a statement"로).
    숨은 `Let fresh`(이름이 `$`로 시작)는 사용자 문장으로 세지 않는다. 함수 본문 끝도 같은 규칙이므로 `fn f(): assert x ;;`는 끝에 `nil`을 쓴다.
  - (나) 빈 블록만 막는다. 가장 흔한 `else: ;;`만 막고 `assert`·`fn`은 예외로 남는다.

### FE5. 심볼릭 링크·대소문자 차이로 같은 모듈이 여러 번 적재된다 — 버그, 중간
- 재현(macOS):
  ```
  mods/lib.fung:  counter = atom(0)
                  fn bump() = swap(counter, n => n + 1)
                  fn get() = swap(counter, n => n)
  ln -s lib.fung mods/lib2.fung
  mods/main.fung: import "./lib"
                  import "./Lib" as l3
                  import "./lib2"
                  lib.bump()
                  print(lib.get(), lib2.get(), l3.get())
  $F mods/main.fung                              → 1 0 0
  ```
  한 파일이 상태(`atom`)를 세 벌 갖는다. 같은 이유로, 진입 파일을 링크나 대소문자만 다른 경로로 가져오면 E0307을 피해 진입 파일이 라이브러리로 한 번 더 적재된다. NFD로 저장된 파일 이름(macOS)을 NFC로 쓴 경로도 다른 키가 된다.
- 명세 근거: 04 "모듈은 처음 import될 때 한 번 … 실행하고 캐시한다", "진입 파일을 다른 파일에서 import하면 에러. 판단 기준은 같은 경로인가".
- 원인 위치: `crates/fung-front/src/loader.rs:322-350` `load_file`, `:497` `abs_key`(쓴 그대로의 경로를 키로 씀), `:337` E0307 비교.
- 고치는 법: 키를 `fs::canonicalize`(파일이 있으면)나 (장치, inode)로 정하고, 보이는 경로(`display_path`)는 지금처럼 쓴 대로 둔다. 읽기 함수를 바꿔 끼우는 시험(`read_file`)을 위해 정규화도 갈아 끼울 수 있게 한다.
- **처리(2026-11-08)**: `loader.rs`에 `Loader::file_id`(기본 (장치, inode))와 `State::keys`(정체 → 처음 닿은 키). 보이는 경로·묶는 이름은 쓴 그대로, 링크 너머의 상대 import는 처음 닿은 경로의 디렉터리에서. 시험 `loader::tests::one_file_under_several_paths_is_one_module`(심볼릭 링크, 대소문자 무시 파일 시스템이면 대소문자, 링크로 진입 파일 E0307), 문서 04·`modules.md`.

### FE6. 괄호 안 줄바꿈 규칙의 남은 위반: `..` `]`, `not` `in`, 이름 `:` — 버그, 낮음
- 재현(지난 검토 F4가 불완전하다):
  ```
  $F -e 'xs=[1,2,3]
  print(xs[1..
  ])'                                           → E0110 unexpected `]`   (xs[\n1..] 은 된다)
  $F -e 'print(1 not
  in [2])'                                      → E0110 unexpected `not`
  $F -e 'f = (a, b) => a
  print(f(a
  : 1, b: 2))'                                  → E0110 unexpected `:`
  ```
- 명세 근거: 01 "줄 이어짐 규칙" 3("`( ) [ ] { }` 안: 줄바꿈은 공백").
- 원인 위치: `crates/fung-front/src/parser.rs:147` `kind_after`(줄바꿈을 건너뛰지 않는 날 토큰)를 쓰는 곳: `parser/expr.rs:176`(`..]` 판정), `:179`(`not in`), `:399`(이름 붙인 인자), `:514`(`..]`).
- 고치는 법: 이 네 곳을 `self.kind_from(self.i + 1)`(nest > 0이면 줄바꿈을 건너뜀)로 바꾼다. `stmt.rs`의 `kind_after`는 nest가 0인 문장 시작이라 그대로 맞다.
- **처리(2026-11-08)**: `parser/expr.rs`의 `..]`·`not in`·이름 인자·`at_hole`·`[a..]`이 `kind_from(self.i + 1)`. 시험 `parser::tests::newlines_inside_brackets_around_two_token_forms`, `tests/lang/line_continuations_more.fung`, 문서 01·`blocks.md`.

### FE7. 줄 끝의 `in`(for 머리)과 `|`(패턴 대안·타입 유니온)에서 줄이 이어지지 않는다 — 버그, 낮음
- 재현:
  ```
  $F -e 'for x in
    [1, 2]:
    print(x)
  ;;'                                           → E0110 unexpected newline
  $F -e 'type t = int |
    str'                                        → E0110 unexpected newline
  $F -e 'match Ok(1):
    Ok(x) |
    Err(x) => print(x)
  ;;'                                           → E0110 unexpected newline
  ```
  긴 열거형 별칭과 긴 대안 패턴을 괄호 밖에서 나눌 길이 없다(타입 표기에는 괄호도 없다).
  포매터는 줄 끝 `|`·`in`을 이어짐으로 보고 들여쓴다(`format/layout.rs:72` `ends_open`의 `Bar`, `:58` `KwIn`). 두 곳의 판정이 다르다(FE22).
- 명세 근거: 01 "줄 이어짐 규칙" 1(모든 이항 연산자 … `in` …로 끝나는 줄은 이어짐).
- 원인 위치: `parser/pattern.rs:372` `for_expr`, `parser/pattern.rs:16` `pattern`, `parser/fn_.rs:218` `type_spec`(`in`·`|` 뒤에 `skip_newlines_after_operator`가 없음).
- 고치는 법: 세 곳에서 `skip_newlines_after_operator()`를 부른다. 01의 규칙 1 목록에 `|`(패턴·타입)를 명시한다.
- **처리(2026-11-08)**: `parser/pattern.rs` `pattern`·`for_expr`, `parser/fn_.rs` `type_spec`이 `skip_newlines_after_operator`. 포매터 `format/layout.rs` `split_arm_head`(줄 끝 `|`의 절 머리는 절). 시험 `parser::tests::a_line_ending_in_in_or_bar_continues`·`format::tests::an_arm_head_split_after_a_bar`, `tests/lang/line_continuations_more.fung`, 문서 01·`blocks.md`.

### FE8. 비결합인 범위 `..`가 `1..2..3`을 받는다 — 버그, 낮음
- 재현:
  ```
  $F -e 'print(1..2..3)'                        → E0501 cannot make a range from range and int (런타임)
  $F -e 'xs=[1,2,3]
  print(xs[0..1..])'                            → E0501 cannot slice with int and range (런타임)
  ```
- 명세 근거: 01 EBNF `range = add [ ( ".." | "..=" ) add ]`(한 번만). 비교의 연쇄는 E0111로 막는다.
- 원인 위치: `parser/expr.rs:56-57` `infix`(`..`의 왼쪽 결합력이 그대로라 다시 받음). `infix_loop`에 비교처럼 막는 판정이 없다.
- 고치는 법: `infix_loop`에서 직전 연산이 범위면 다음 범위 연산자를 문법 에러로 한다(E0111과 같은 꼴의 새 번호, 도움말 "괄호로").
- **처리(2026-11-08)**: `parser/expr.rs` `infix_loop`의 `last_was_range`, 새 번호 E0118. 시험 `parser::tests::a_range_is_not_an_operand_of_a_range`, `tests/lang/err_range_chain.fung`, 문서 07·`errors.md`.

### FE9. 최소 정수 리터럴의 접기가 `**`·후위 연산보다 먼저 일어난다 — 버그, 낮음
- 재현:
  ```
  $F -e 'print(-9223372036854775808 ** 1)'      → -9223372036854775808
  $F -e 'print(-9223372036854775808 ** 2)'      → E0503 integer overflow (런타임)
  ```
  `-a ** b = -(a ** b)`(01·07)라면 `9223372036854775808 ** 1`의 크기가 범위 밖이므로 E0102여야 한다. 지금은 `(MIN) ** 1`로 읽는다.
- 원인 위치: `parser/expr.rs:358-366` `negation`(다음 토큰이 그 크기면 곧바로 리터럴로 접고 `**`·`.`·`[`를 그 리터럴에 붙인다).
- 고치는 법: 접은 뒤 다음 토큰이 `**`·`.`·`[`·`(`·`!`·`?`이면 E0102. 또는 `unary`가 피연산자를 다 읽은 뒤, 피연산자가 그 리터럴 하나일 때만 접는다.
- **처리(2026-11-08)**: `parser/expr.rs` `negation`이 접은 뒤 `**`·`.`·`[`·`(`·`!`·`?`가 오면 E0102(`.`는 FE2로 렉서가 먼저 막음). 시험 `parser::tests::least_int_folds_only_as_a_whole_operand`.

### FE10. 비UTF-8 바이트가 든 `test` 이름·`import` 경로가 조용히 빈 문자열이 된다 — 버그, 낮음
- 재현:
  ```
  printf 'test "a\\xffb":\n  assert 1 == 2\n;;\n' > t.fung; $F test t.fung   → FAIL t.fung: (이름이 빔)
  printf 'import "./x\\xff.fung"\n' > imp.fung; $F imp.fung                  → E0301 cannot find module ``
  ```
- 명세 근거: 01 "문자열"(L4: `\xHH`는 바이트 하나, 글이 필요한 곳은 E0518), 원칙 1.
- 원인 위치: `crates/fung-front/src/parser/stmt.rs:9-18` `plain_text`. `Part.text`만 이어 붙이는데, 비UTF-8 부분은 `text`가 비고 바이트가 `Part.bytes`에 있다(`lexer.rs:203`).
  같은 일을 바르게 하는 `Token::plain`(`lexer.rs:241`)이 이미 있다(중복 코드).
- 고치는 법: `plain_text`를 지우고 `Token::plain`을 쓴다. 결과가 `Err(bytes)`면 test 이름·import 경로는 글이어야 하므로 컴파일 에러(E0518 또는 E0112)로 한다.
- **처리(2026-11-08)**: `parser/stmt.rs` `plain_text`가 `Token::plain` 위의 얇은 함수가 되어(중복 제거), UTF-8이 아니면 E0112 "expected a plain test name that is text (valid UTF-8), found string \"a\xffb\"". 시험 `parser::tests::test_names_and_import_paths_are_text`, 문서 `errors.md`(E0112).

### FE11. `..._`가 `_`라는 이름을 묶어 거짓 에러를 낸다 — 버그, 낮음
- 재현:
  ```
  $F -e 'fn f([a, ..._], [b, ..._]) = a + b'                → E0110 unexpected duplicate parameter `_`
  $F -e '([a, ..._], [b, ..._]) = ([1, 2], [3, 4])'         → E0221 name `_` is bound twice in one pattern
  $F -e 'match [1, 2]: [a, ..._] | [a] => print(a) ;;'      → E0216 alternatives must bind the same names
  ```
- 명세 근거: 01 EBNF `"..." [ NAME ]`, 03(패턴의 `_`는 아무거나). `...`와 `..._`가 같은 뜻이어야 한다.
- 원인 위치: `parser/pattern.rs:169-171` `plist`(`_`도 이름으로 `rest.lit`에 넣음). 그 뒤 `parser/fn_.rs:31` `pattern_names`와 `resolve/pattern.rs:21` `binds`가 이름으로 센다.
- 고치는 법: `plist`에서 `_`이면 이름 없는 `Prest`로 둔다. 매개변수 `fn f(..._)`도 같게 한다.
- **처리(2026-11-08)**: `parser/pattern.rs` `plist`가 `..._`를 이름 없는 `prest`로. 매개변수 `fn f(..._)`는 전에도 `_`를 묶지 않았다(확인). 시험 `parser::tests::rest_wildcard_binds_nothing`, `tests/lang/rest_wildcard.fung`, 문서 03.

### FE12. 맵 패턴의 중복 키를 받는다(맵 리터럴은 E0113) — 버그, 낮음
- **처리(2026-11-08)**: SP3과 같은 커밋(`pmap`의 키 집합). E0113 문구를 "in a map literal or pattern"으로. 시험 `tests/lang/err_map_pattern_dup_key.fung`.
- 재현:
  ```
  $F -e 'print(match {a: 1}: {a: 1, a: 2} => 1; _ => 0 ;;)'   → 0 (절대 맞지 않는 갈래)
  $F -e '{a: x, a: y} = {a: 1}; print(x, y)'                   → 1 1
  $F -e 'x = {a: 1, a: 2}'                                    → E0113 duplicate key `a` in map literal
  ```
- 명세 근거: 02 "리터럴 안에서 같은 키를 두 번 쓰면 컴파일 에러. 조용히 하나를 버리지 않는다", 03 "한 패턴 안에 같은 이름이 두 번 나오면 컴파일 에러", 원칙 7.
- 원인 위치: `parser/pattern.rs:194-226` `pmap`(키 중복 검사 없음). `parser/collections.rs:43-65` `map_entries`에는 있다.
- 고치는 법: `pmap`에 같은 `HashSet<Vec<u8>>` 검사를 넣고 E0113(문구를 "in a map literal or pattern"으로).

### FE13. 홀로 선 CR(`\r`)은 주석을 끝내지만 줄바꿈이 아니다 — 버그, 낮음
- 재현:
  ```
  printf '# hello\rprint("hidden")\n' > cr.fung; $F cr.fung     → hidden (주석 안처럼 보이는 코드가 돈다)
  printf '# a\rprint(2)\n' | $F fmt -                          → "print(2)  # a" (주석과 코드의 순서를 바꿔 씀)
  printf 'x = 1\ry = 2\n' > cr2.fung; $F cr2.fung              → E0110, 발췌가 날 CR을 그대로 찍어 터미널에서 줄이 덮인다
  ```
  `cat`으로 보면 `print("hidden")`이 주석을 덮어 쓴 것처럼 보이고, 에디터에서는 두 줄로 보인다. 어느 쪽이든 렉서의 해석(같은 줄, 주석 밖)과 다르다.
- 명세 근거: 01 "주석"(`#`부터 줄 끝까지), 원칙 1.
- 원인 위치: `lexer.rs:442` `comment`(`\r`에서 멈춤), `lexer.rs:420` `run`(홀로 선 `\r`은 공백), `diag.rs` `write_excerpt`(줄을 그대로 찍음).
- 고치는 법: **추천**은 `\r\n` 밖의 `\r`을 E0101로 하는 것이다(문자열·원시 문자열 안은 그대로). 주석은 `\n`(또는 `\r\n`)에서만 끝난다. 발췌는 제어 문자를 `quote`처럼 보이게 한다.
- **처리(2026-11-08, 사용자 결정)**: `lexer.rs` `step`(홀로 선 `\r`은 공백이 아니라 E0101), `comment`(`\n`·`\r\n`에서만 끝나고 그 사이의 CR은 E0101), `diag.rs` `visible`(발췌의 제어 문자를 이스케이프로, 밑줄 칸도 그에 맞춤). 시험 `lexer::tests::lone_carriage_return_is_e0101`·`diag::tests::excerpt_shows_control_characters`, 기대 출력 `fmt/stdin.txt`의 CR 사례, 문서 01·`lexical.md`·`errors.md`(E0101).

### FE14. 문자열 끝의 `\`가 줄바꿈·파일 끝에서 깨진 E0103 문구를 낸다 — 버그, 낮음
- 재현:
  ```
  printf 'print("a\\\n")\n' > esc.fung; $F esc.fung    → error[E0103]: unknown escape \<실제 줄바꿈>
                                                         in string
  printf 'print("a\\' > esc2.fung; $F esc2.fung       → error[E0103]: unknown escape \� in string
  ```
- 원인 위치: `lexer/string.rs:84-128` `escape`(`advance`가 줄바꿈이나 끝의 U+FFFD를 이스케이프 글자로 돌려줌). 카탈로그 E0103 `\%s`(따옴표 없음).
- 고치는 법: `\` 뒤가 줄바꿈·끝이면 E0104(unterminated string). E0103의 인자는 `%q`처럼 보이게 한다.
- **처리(2026-11-08)**: `lexer/string.rs` `escape`가 `\` 뒤 줄 끝·파일 끝이면 문자열 시작 위치의 E0104, 보간 안의 중첩 문자열도. E0103 문구를 `unknown escape %q in string`(인자는 쓴 그대로 `\p`). 시험 `lexer::string::tests::backslash_at_line_or_input_end`·`escape_error_arguments`, `tests/lang/err_string_backslash_at_line_end.fung`, 문서 01·`errors.md`.

### FE15. E0110 "unexpected %s"에 문장을 넣어 깨진 문구가 여러 곳 — 정리, 낮음
- 재현:
  ```
  fn f(a, a) = 1          → error[E0110]: unexpected duplicate parameter `a`
  fn f(a = 1, b) = 1      → unexpected required parameter `b` after an optional one
  match 1: ;;             → unexpected a match without arms
  match 1: _ => ;;        → unexpected an empty match arm
  x = Ok(a: 1)            → unexpected named argument in a tag (tags take values by position)
  x = {"${1}": 2}         → unexpected interpolation in a map key
  f(a: _)                 → unexpected `_` in a named argument
  f = fn(x) = x           → unexpected `=` (01은 "fn(x) = …는 없다"고 이유까지 적는다)
  ```
  `manual/reference/errors.md`의 E0110 쪽은 "뜻밖의 토큰"만 설명한다.
- 명세 근거: 08 에러 카탈로그(번호마다 한 종류의 문제, "코드 일부, 정확한 위치, 해결 힌트"), 저장소 규칙(번호의 뜻은 바꾸지 않음).
- 원인 위치: `parser/fn_.rs:49·146·180`, `parser/pattern.rs:78·156·321·329`, `parser/collections.rs:53·147`, `parser/expr.rs:408`.
- 고치는 법: 문법상 받지만 금지하는 것들(중복 매개변수, 필수 뒤 선택 매개변수, 절 없는 match, 빈 절, 익명 `fn(x) = …` 등)에 새 번호를 붙이거나 E0112 "expected …"로 옮긴다.
  익명 `fn`에는 도움말 "anonymous functions take a block: `fn(x): … ;;`, or write a lambda `x => …`"를 단다. 중복 매개변수는 E0116 "given twice"의 짝이 된다.
- **처리(2026-11-08, 사용자 결정)**: 새 번호 E0119(중복 매개변수, E0116의 짝)·E0120(기본값 뒤 필수)·E0121(태그의 이름 인자)·E0122(맵 키·패턴의 보간)·E0123(이름 인자의 `_`)·E0124(익명 `fn … =`, 도움말), 절 없는 match·빈 절·괄호 패턴은 E0112. `parser/fn_.rs`·`pattern.rs`·`collections.rs`·`expr.rs`. 시험 `parser::tests::refused_forms_have_their_own_codes`와 바뀐 기존 사례, `tests/lang/err_tag_named.fung`(E0121), 문서 03·`errors.md`(새 항목 일곱과 E0110·E0112 설명).

### FE16. 깊은 괄호·보간 중첩이 이차 시간이라 E0115에 닿기 전에 수십 초 — 성능, 낮음
- 재현(`python3 -I -c "print('x = ' + '('*n + '1' + ')'*n)" > p.fung; time $F check p.fung`):
  - 괄호 n겹: 8,000겹 0.05 s, 100,000겹(파일 0.2 MB)은 4.78 s 뒤 E0115.
  - 보간 `"${"${…1…}"}"` n겹: 4,000겹 0.20 s, 8,000겹 0.92 s, 16,000겹 2.78 s, 100,000겹(0.5 MB)은 31.5 s 뒤 E0115.
  - 중첩 `match` 8,000겹도 0.31 s로 이차다.
  - `check`·`fmt`·`run`(그리고 나중의 LSP)이 프로그램이 만든 입력이나 잘린 파일 하나에 멈춘다.
- 원인 위치: `parser/fn_.rs:241` `lambda_params_ahead`(식 자리의 `(`마다 짝 괄호까지 훑음: 깊이 × 길이), `parser/expr.rs:450` `string_literal`(보간 한 겹마다 안쪽 글을 `lex_at`으로 다시 렉싱), `lexer/string.rs:135` `skip_interpolation`(깊이 검사 없는 재귀).
- 고치는 법: 렉싱 직후 괄호 짝 표를 O(n)에 한 번 만들고, `lambda_params_ahead`가 짝 위치를 바로 본다. 보간은 렉서가 안쪽 토큰을 한 번만 만들어 `Part`에 담거나, 렉서에도 `MAX_DEPTH` 검사를 둔다.
- **처리(2026-11-08)**: `parser.rs` `bracket_pairs`(괄호 짝 표, `lambda_params_ahead`가 O(1)), `lexer/string.rs` `interpolation`(보간 코드를 문자열과 함께 한 번 렉싱해 `Part::toks`에 담음, 깊이 한도 E0115, 긴 보간 문자열 토큰의 글은 짧게), `parser/expr.rs` `string_literal`이 `lex_at` 대신 그 토큰을 씀. 측정(릴리스, `fung check`): 괄호 10만 겹 6.66 s → 0.08 s, 보간 10만 겹 39.5 s → 0.06 s, 괄호 8천 겹 0.01 s, 보간 8천 겹 1.17 s → 0.07 s. 시험 `parser::tests::deep_parentheses_and_interpolations_are_linear`.
- **후속(2026-11-09)**: 남아 있던 중첩 `match`의 이차 시간은 두 곳이었다. (1) 진단마다 `write_excerpt`가 소스를 `split('\n').nth`로 처음부터 읽음(경고 수 × 파일 크기): `SourceMap`이 파일마다 줄 시작 색인을 두고 `line`이 바로 꺼냄(시험 `diag::tests::many_excerpts_are_linear`). (2) `resolve`의 `lookup`·`capture_from`·`defined_outside`가 블록 사슬을 훑음: `FnScope::bound_in`(이름 → 묶은 블록들, 바깥부터)과 `bind`·`innermost`·`pop_scope`(시험 `resolve::tests::deep_blocks_resolve_in_linear_time`, 고치기 전 디버그 7.3 s). 측정(릴리스, 블록마다 이름을 묶는 `match`의 `fung check`): 4천 겹 0.18 s → 0.03 s, 8천 겹 0.66 s → 0.06 s. 같은 날 `http`의 `_encode_path`가 이미 있는 `%XX`를 풀어 `%2F`가 구분자가 되던 것도 고침(이스케이프는 쓴 그대로, `http.rs` 시험 사례).

### FE17. 원소 타입 표기를 검사하지 않는다(`list[lsit]`, `int[str]`, `Ok[int]`) — 결정, 낮음
- **처리(2026-11-08, 사용자 결정 (가))**: 파서가 `list[str]`를 쓴 그대로 남기고, `resolve/alias.rs` `expand`가 안쪽 이름을 검사(펼치지 않음, 재귀 표기 허용)하고 바깥만 남긴다. 새 번호 E0232. SP5의 같은 항목도 이것으로 닫힘. 시험 `err_element_type_unknown`·`err_element_type_on_scalar`·`element_types`, 문서 07·`types.md`·`errors.md`·CHANGELOG.
- 재현:
  ```
  $F -e 'fn f(x: nosuch) = x'                           → E0212 unknown type `nosuch`
  $F -e 'fn f(x: list[lsit]) = x; print(f([1]))'        → [1] (에러 없음)
  $F -e 'fn f(x: int[str]) = x; print(f(1))'            → 1
  ```
- 명세 근거: 07 "원소 타입은 지금은 문서 역할이고, 추후 정적 검사기가 생기면 그때 검사한다". 문서 역할인 표기에 오타가 말없이 통과하면 문서가 틀린다. 지금 받아 둔 오타는 검사기가 생길 때 호환을 깬다.
  `int[…]`·`Ok[…]`처럼 원소가 없는 타입의 원소 표기는 뜻이 없다.
- 원인 위치: `crates/fung-front/src/parser/fn_.rs:201-217` `type_spec`(`[ … ]` 안을 파싱한 뒤 결과를 버림, 그래서 `resolve/alias.rs`의 E0212에 닿지 않음).
- 고치는 법(결정):
  - (가) **추천**: 원소 타입도 이름만 검사(E0212)하고, `[…]`는 `list`·`map`(과 그것으로 펼쳐지는 별칭)에만 허용한다. 런타임 검사는 그대로 바깥 타입만 한다.
  - (나) 지금처럼 두고 07에 "이름도 검사하지 않는다"를 적는다.

### FE18. 태그 부분 적용 `Ok(_)`가 틀린 문구의 E0215로 거절된다 — 결정, 낮음
- **처리(2026-11-08, 사용자 결정 (가))**: `desugar.rs` `partial_with`가 태그를 만드는 함수로 푼다. 파이프 오른쪽 태그의 `_`는 E0227. 시험 `tag_partial`·`err_pipe_tag_hole`, 문서 03·`functions.md`·CHANGELOG.
- 재현:
  ```
  $F -e 'print(map([1], Ok(_)))'     → E0215 `_` can only be a call argument or a pattern
  $F -e 'print(map([1], Ok))'        → E0403 argument `f` must be fn, got sym
  $F -e 'print(1 |> Ok)'             → Ok(1)
  ```
  `_`는 분명 호출 모양의 인자 자리에 있는데 메시지는 그 반대를 말한다. 파이프는 "태그의 `Ok(x)`는 호출 형태이므로" 태그를 호출처럼 다룬다(01 L6, `operators.md`). EBNF도 `Ok(_)`를 받는다.
- 원인 위치: `parser/collections.rs:134-152` `tag_expr`(구멍을 받음) → `desugar.rs:226`(`Head::Tag`는 `map_kids`) → `resolve.rs:429`·`:620` E0215. `desugar.rs:271` `partial`이 태그에 닿지 않는다.
- 고치는 법(결정):
  - (가) **추천**: `Ok(_)`·`Pair(_, 2)`를 `x => Ok(x)`로 푼다. 파이프와 같은 "태그는 호출 형태" 규칙 하나가 되고, `map(xs, Ok(_))`는 흔한 쓰임이다.
  - (나) 막되 전용 문구("a tag cannot be partially applied; write `x => Ok(x)`")를 내고 03·`functions.md`에 한 줄 적는다.

### FE19. 파이프의 태그 판정이 `char::is_uppercase`로 추측한다 — 정리, 낮음
- 재현: `$F -e 'Ⅰ = 5` 뒤 줄에 `print(Ⅰ, 1 |> 'Ⅰ, type_of(1 |> 'Ⅰ))'` → `5 Ⅰ(1) 'Ⅰ`.
  `Ⅰ`(U+2160, Nl)은 렉서가 이름으로 읽는데(Lu가 아님), 파이프는 `char::is_uppercase`(Other_Uppercase 포함)로 태그로 본다.
- 명세 근거: 01 "식별자"(첫 글자가 일반 범주 Lu면 태그).
- 원인 위치: `desugar.rs:303-307` `pipe`(맨 태그 `Red`와 심볼 `'Red`가 같은 `Head::Symbol`이라 글자로 다시 판정). 렉서는 `unicode::is_upper`(`lexer.rs:466`)를 쓴다.
- 고치는 법: 최소한 `crate::unicode::is_upper`를 쓴다. 더 낫게는 파서가 맨 태그를 표시해(`Head::Tag` + 인자 0, 또는 lit에 표시) 추측을 없앤다.
- **처리(2026-11-08)**: 파서가 맨 태그를 `Head::BareTag`(`bare_tag`, dump ast에서는 `Red`로 보임)로 표시하고, `desugar.rs` `pipe`는 그것과 `Head::Tag`만 태그 호출로 만든다. 그 밖에서는 desugar가 심볼로 바꾼다. 시험 `desugar::tests::pipe_into_tags`, `tests/lang/err_pipe_quoted_symbol.fung`, 문서 01·`operators.md`.

### FE20. 같은 이름(과 빈 이름)의 `test` 블록을 받는다 — 결정, 낮음
- **처리(2026-11-08, 사용자 결정 (가))**: `resolve.rs` `check_test_names`, 새 번호 E0233. TL5의 같은 이름 부분도 이것으로 닫힘. 시험 `err_test_name_twice`·`err_test_name_empty`, 문서 08·`testing.md`·`errors.md`·CHANGELOG.
- 재현: 한 파일에 `test "a": assert true ;;`와 `test "a": assert false ;;` → `ok … a` / `FAIL … a`. `test "": …`도 통과한다. 보고에서 어느 블록인지 이름으로 가릴 수 없다.
- 명세 근거: 최상위 `fn` 둘은 E0210, 맵 키 둘은 E0113(원칙 7). `testing.md`는 이름 규칙을 말하지 않는다.
- 원인 위치: `parser/stmt.rs:169` `test_block`, `resolve.rs:423`.
- 고치는 법(결정): **추천**은 한 파일 안의 같은 이름과 빈 이름을 컴파일 에러로 하는 것이다(새 번호). 다른 길은 그대로 두고 보고에 줄 번호를 붙이는 것이다.

### FE21. 문서가 구현과 다르다: camelCase 경고, 포매터 머리 주석 — 문서, 낮음
- `docs/design/01-syntax.md:100` "camelCase는 문법 에러가 아니고 포맷터·경고 몫이다": 포매터는 이름을 건드리지 않고, 그런 경고(W01xx)도 없다.
  문장을 "검사하지 않는다(규약)"로 고치거나, 경고를 정한다(결정이지만 작다).
- `crates/fung-front/src/format.rs:5-7` 머리 주석 "the lexer reads the source decoded (one U+FFFD …, as everywhere in fung)": L4 뒤로 소스는 UTF-8이 아니면 거절한다(포매터만 받는다). 주석을 고친다.
- **처리(2026-11-08)**: 01의 camelCase 문장을 "검사하지 않는다(snake_case는 규약)"로, `format.rs` 머리 주석을 L4 뒤의 사실로.

### FE22. 같은 판정이 여러 벌 — 정리, 낮음
- 포매터(`format/layout.rs`)가 문법을 토큰 휴리스틱으로 다시 판정한다: `ends_open`(줄 끝 이어짐), `is_continuation`, `pattern_start`·`top_arrow`(절 머리), `fn_head`, `then`/`else` 짝 세기.
  `format_bytes`는 `parser::parse`를 이미 부르고 결과를 버린다(`format.rs:76`). FE7처럼 둘이 어긋나도 안전장치(토큰 비교)는 들여쓰기 차이를 잡지 못한다.
  고치는 법: 파서가 줄마다 필요한 사실(블록 열림·닫힘, 이어짐, 절 머리)을 표로 내주고 포매터가 그것을 쓴다. 적어도 이어짐 토큰 목록은 파서와 한 상수로 나눈다.
- 패턴 이름 모으기가 네 벌이다: `parser/fn_.rs:31` `pattern_names`, `resolve.rs:783` `pattern_names`(`Head::Name` 포함), `resolve/pattern.rs:26` `pattern_names`(정렬), `desugar.rs:535` `names_in`.
  FE11이 셋 모두에 같은 실수를 남긴 예다. 하나로 모은다.
- 문자열 토큰의 평문 꺼내기가 두 벌이다(`Token::plain`, `stmt.rs` `plain_text`): FE10에서 함께 정리한다.
- **처리(2026-11-08)**: 패턴 이름 모으기 셋(`parser/fn_.rs`, `resolve.rs`, `resolve/pattern.rs`)을 `Node::bound_names` 하나로(`desugar.rs` `names_in`은 읽는 이름을 모으는 다른 일이라 그대로). 이어짐 토큰 목록을 `Kind::starts_continuation`(파서·포매터)·`Kind::ends_open`(포매터, 파서 시험이 맞춤)로. `plain_text` 중복은 FE10. 시험 `parser::tests::the_parser_continues_after_every_open_line_end`. 포매터를 파서 출력 기반으로 바꾸는 일은 하지 않음.

## 6. 컴파일러·바이트코드·VM 실행 핵심 (VM)

### VM1. 중첩 블록 안의 문장은 `CLEARLOCAL`을 받지 않아, 같은 코드가 `if`·`match` 가지 안에서는 교착한다 — 결정, 중간
- **처리(2026-11-08, 사용자 결정 (가))**: 모든 블록의 문장 뒤 놓기를 `compiler/moves.rs`의 생존 분석 하나로(`Lives::after`, `Compiler::let_go`·`branches`). 분석 중 대입과 `match` 패턴이 슬롯의 생존을 끊게 했다(옆 블록의 슬롯 재사용이 앞 블록의 값을 붙잡던 같은 뿌리의 교착). 시험 `tests/lang/local_lifetime.fung`, 문서 03·`scope.md`·VM 10장·CHANGELOG.
- 재현:
  ```
  fn top():
    (tx, rx) = channel(1)
    f = () => send(tx, 1)
    f()!
    for x in rx: print("top", x) ;;
  ;;
  fn nested(c):
    if c:
      (tx, rx) = channel(1)
      f = () => send(tx, 1)
      f()!
      for x in rx: print("nested", x) ;;
    else:
      nil
    ;;
  ;;
  _ = top()
  _ = nested(true)
  ```
  출력: `top 1`, `nested 1`, 그다음 `error[E0702]: deadlock: all tasks are waiting --> :12:5`(help: "it still holds that channel's sending end itself (a variable of `nested`)").
  `top`에서는 `f = …` 문장 뒤에 `CLEARLOCAL tx`가 나와, `f()`가 끝나면 채널이 닫힌다. `nested`에서는 같은 문장이 가지 블록 안이라 `tx`가 함수가 돌아올 때까지 칸에 남는다.
- 명세 근거: 05:50 "지역 변수는 마지막으로 읽힐 때 넘어가고, **그 문장이 끝나면 놓인다**", `scope.md` "값의 수명"(같은 말).
  반면 03 "값의 수명"의 "가지 안에서만 읽으면 그 가지를 품은 문장 뒤에 놓인다"는 지금 동작(함수 본문의 문장 단위)을 적은 것이다. 두 문서가 다른 규칙을 말한다.
  코드를 `if` 안으로 옮기기만 해도 교착이 생기는 것은 "놀라운 동작 없음"과도 맞지 않는다.
- 원인 위치: `crates/fung-front/src/compiler.rs` `Compiler::block`(`body_block`일 때만 `read_later`·`clear_unread`. 주석 "a block inside an expression cannot see what the rest of that expression reads").
- 고치는 법(결정):
  - (가) **추천**: 문장 경계의 놓기를 `compiler/moves.rs`의 역방향 생존 분석 하나로 계산한다. `last_reads`가 이미 함수 전체에서 "이 지점 뒤에 읽히는 슬롯"을 안다.
    어느 블록의 문장이든, 끝난 뒤의 생존 집합을 기록해 두고 "이 문장까지 읽혔고 뒤에서 읽히지 않는 슬롯"에 `CLEARLOCAL`을 낸다.
    주석의 걱정(바깥 식이 뒤에서 읽는 것)은 함수 전체 분석이면 사라진다. 규칙이 "마지막으로 읽힌 문장 뒤" 하나가 되어 05·`scope.md`와 맞는다.
    관찰되는 차이는 "더 일찍 놓인다"뿐이고 03이 허용하는 방향이다. VM10 (a)도 같은 분석 결과로 함께 고친다.
  - (나) 지금대로 두고, 05·`scope.md`를 03처럼 "함수 본문의 문장 단위"로 고친다. E0702 도움말에 "가지 블록 안이면 함수가 돌아올 때까지 남는다"를 더한다.

### VM2. 진입 파일 최상위 블록의 지역 변수는 마지막 읽기에서 넘기지도, 문장 뒤에 놓지도 않는다 — 결정, 중간
- **처리(2026-11-08, 사용자 결정 (가))**: `compile`(진입 파일·모듈 본문)도 `last_reads`를 돌린다. VM1과 같은 커밋.
- 재현:
  ```
  fn drop_it(t) = nil
  fn inside():
    (tx, rx) = channel(1)
    drop_it(tx)
    print("fn:", recv(rx))
  ;;
  inside()
  if true:
    (tx, rx) = channel(1)
    drop_it(tx)
    print("top block:", recv(rx))
  ;;
  ```
  출력: `fn: Err({kind: 'closed, message: "channel closed"})`, 그다음 `error[E0702]: deadlock … :11:27 … (a variable of `<entry>`)`.
  함수 안에서는 `drop_it(tx)`가 `MOVELOCAL`이라 `tx`가 놓인다. 진입 파일의 `if` 블록 안에서는 `LOADLOCAL`이고 `CLEARLOCAL`도 없다.
- 명세 근거: 03 "값의 수명"은 "지역 변수(매개변수, 함수 안에서 묶은 이름)"와 "최상위 변수(전역)는 프로그램 끝까지"만 말한다. 최상위 블록의 지역(전역이 아닌 슬롯)이 어느 쪽인지는 비어 있다.
- 원인 위치: `crates/fung-front/src/compiler.rs` `compile`(`moves: HashSet::new()`, `body_block: false`). 모듈 본문도 같은 `compile`을 지난다.
- 고치는 법(결정):
  - (가) **추천**: `<main>`과 모듈 본문에도 `moves::last_reads(&res.core)`와 VM1의 문장 경계 놓기를 적용한다. 전역은 슬롯이 아니라 영향이 없다. 스크립트는 최상위 `if`·`match` 블록에 코드를 많이 쓴다.
  - (나) 03에 "진입 파일·모듈 본문의 블록 지역도 프로그램 끝까지 산다"를 적는다.

### VM3. REPL: 실패한 문장이 선언한 전역을 최상위 람다가 붙잡으면 E0808 대신 E0801 내부 오류 — 버그, 중간
- 재현:
  ```
  printf 'x = fail("a")\ng = () => x\nfn h() = x\nh()\n' | $F
  > error[E0803]: a …
  > error[E0801]: internal error: captured an unassigned global
  > > error[E0808]: `x` has no value: the statement that assigned it failed
  ```
  같은 이름을 함수가 읽으면(`h()`) 올바르게 E0808인데, 람다가 붙잡으면(`CaptureKind::Global`) 패닉이 E0801이 된다.
- 명세 근거: 카탈로그 E0808("an input declared the name, then failed before storing it"), `vm.rs` 머리 주석 "any other read of an empty slot is a VM bug".
- 원인 위치: `crates/fung-vm/src/vm.rs` `Task::make_closure`, `CaptureKind::Global => … .expect("captured an unassigned global")`.
- 고치는 법: `make_closure`가 `Result<Value, Stop>`를 돌려주고, 빈 전역이면 이미 있는 `unset_global(fi, c.index, pos)`(E0808/E0809)로 에러를 낸다. 스크립트에서는 resolve가 E0204로 막으므로 REPL에만 있는 길이다. REPL 시험을 하나 더한다.
- **처리(2026-11-08)**: `vm.rs` `Task::make_closure`가 `Result`를 돌려주고 빈 전역은 `unset_global`(E0808/E0809). 시험 `vm/repl_tests.rs` `capturing_a_failed_global_is_e0808`.

### VM4. 인자가 둘 이상인 `Ok`에 `!`·`?`·`??`는 튜플을 꺼내지만 패턴 `Ok(x)`는 맞지 않는다 — 결정, 낮음
- **처리(2026-11-08, 사용자 결정 (가))**: `vm/m4.rs` `result_kind`와 `prelude.rs` `try_map`이 인자 하나인 `Ok`·`Err`만 결과값으로 본다. E0605 문구를 고침(`not_result`). 시험 `err_ok_two_values`·`result_arity`·`m4_tests`, 문서 02·`results.md`·`errors.md`·CHANGELOG.
- 재현:
  ```
  $F -e 'print(Ok(1, 2)!)'                                 → (1, 2)
  $F -e 'print(Ok(1, 2) ?? 0)'                             → (1, 2)
  $F -e 'print(match Ok(1, 2): Ok(x) => x; _ => "no" ;;)'  → no
  ```
- 명세 근거: 02 "에러 처리와 축약 연산자" 표는 `Ok(v)`만 말하고, "그 외 값"은 `!`·`?`에서 런타임 에러다. `Ok(a, b)`를 튜플로 푸는 규칙은 어느 문서에도 없다. 표준 라이브러리는 인자가 하나인 `Ok`만 만든다.
- 원인 위치: `crates/fung-vm/src/vm/m4.rs` `result_kind`(`"Ok" => ResultKind::Ok(Value::tuple(t.args.clone()))`).
- 고치는 법(결정):
  - (가) **추천**: 인자가 하나가 아닌 `Ok`·`Err`는 `!`·`?`에서 E0605(결과값이 아님), `??`에서는 "그 외 값"(그대로)으로 본다. 패턴과 축약 연산자가 같은 모양을 결과값으로 보고, 조용한 튜플 만들기가 사라진다.
  - (나) 지금 동작을 02에 적는다. 그러면 `match`에서도 `Ok(x)`가 맞아야 일관된다.

### VM5. 바이트코드 `CALL`로 부르는 내장 함수에 빠른 길이 없다 — 성능, 중간
- 재현:
  ```
  fn loop(i, acc, xs) = i == 0 then acc else loop(i - 1, acc + len(xs), xs)
  print(loop(10000000, 0, [1, 2]))
  ```
  1.61초다. `len`이 없는 같은 모양은 0.64초라, 내장 함수 호출 한 번에 약 90 ns가 든다(fung 함수 호출 약 47 ns의 두 배).
  `call_op`에서 "위치 인자만, 개수가 정확한 Plain 내장 함수"를 `call_plain`으로 보내는 실험 판은 1.50 → 1.12초(−25%)였다.
- 명세 근거: `manual/vm/internals.md` "호출 규약 — 빠른 길"(힙 할당 없음)과 09 "호출 규약". 지금은 fung 함수만 빠른 길이고, 내장 함수가 부르는 콜백(`issue`)만 `call_plain`을 쓴다.
- 원인 위치: `crates/fung-vm/src/vm/call.rs` `Task::call_op`. 빠른 길이 `Value::Closure`만이다. 나머지는 `positional: Vec` 수집 → `enter` → `bind_args`가 `slots` Vec을 또 만들고 → `args`로 다시 모은다.
- 고치는 법: fung 빠른 길 바로 뒤에 다음 조건의 길을 둔다: `nnamed == 0`, `Value::Native`, `NativeBody::Plain`, 매개변수 수가 정확, 나머지 매개변수 없음.
  피호출자 칸을 빼고 `call_plain`과 같은 길(인자 Vec 하나, `bind_args` 없음)로 부르고, 에러에 `pos`를 붙인다.
  `step_loop`가 이미 `tick`을 했으므로, 이 길에서는 `call_plain`의 `tick`을 빼야 예산이 두 번 줄지 않는다.
- **처리(2026-11-08)**: `vm/call.rs` `Task::call_op`에 Plain 내장 함수의 빠른 길(이름 인자 없음·개수 정확·rest 없음; 이미 tick한 호출이라 다시 tick하지 않음, `check_effect`는 함). 측정(CPU 시간, 중앙값 5회): `vm5_native`(점검 문서의 `len` 루프) 2.198 → 1.176초(−46%, 점검 문서 실험판 1.50 → 1.12), 같은 모양의 `len` 없는 루프 0.725 → 0.572(VM9 몫). 문서 `manual/vm/internals.md` "호출 규약". 의미 시험은 기존 시험 전부.

### VM6. 기본값·이름 인자·`...rest`가 끼는 호출은 호출마다 Vec 셋과 심볼 잠금 — 성능, 중간
- 재현(천만 번 호출):
  - 기본값: `fn add(a, b = 1)`을 `add(acc)`로 부르면 1.78초.
  - 이름 인자: `add(acc, b: 1)`은 2.29초.
  - 같은 모양의 위치 인자 호출은 1.11초다.
  - 호출당 기본값 +67 ns, 이름 인자 +118 ns. 표본에서 `bind_args`·`Vec::from_iter`·`malloc`/`free`가 위쪽을 차지한다.
- 명세 근거: 03 "인자"(기본값·이름 인자는 흔한 규약, `serve(port: …)`·`exec(cmd, cwd: d)`), 09 "호출 규약".
- 원인 위치: `vm/call.rs` `call_op`(named Vec, positional Vec) → `enter` → `bind_args`(`vec![None; …]` 새 슬롯 배열, 이름마다 `specs.name(i) == sym.name()` 문자열 비교, `Sym::name()`은 전역 `Mutex`, VM7) → `self.stack.extend(slots)`.
- 고치는 법: 스택 위에서 제자리로 묶는다.
  - 피호출자 칸 자리부터 프레임을 잡는다. 위치 인자는 이미 그 자리에 있으니 그대로 둔다.
  - 남는 위치 인자는 rest 리스트로 모으고, 이름 인자는 위쪽에서 꺼내 해당 칸에 쓴다.
  - 링크 때 `RtProto`에 매개변수 이름의 `Sym` 목록을 두면 이름 찾기가 `u32` 비교가 된다. 내장 함수는 `NativeParam`에 같은 표를 둔다.
- **처리(2026-11-08)**: `vm/call.rs` `Task::bind_in_place`(먼저 모두 검사 → 이름 인자를 위에서 꺼내 태스크의 재사용 버퍼 `Fiber::named`에 → rest 리스트 → 칸 채우기), `RtProto::param_syms`(링크 때 매개변수 이름의 `Sym`), `call_op`와 내장 함수 콜백(`start_call`)이 사용. 내장 함수의 이름 인자·기본값은 예전 `bind_args` 그대로(`Sym::name`이 VM7로 잠금 없음). 측정: 기본값 `add(acc)` 1.901 → 1.234초, 이름 인자 `add(acc, b: 1)` 2.542 → 1.362초, 위치 인자 1.238 → 1.081초(점검 문서 1.78·2.29·1.11초 기준 측정과 같은 모양). 시험 `vm/tests.rs` `arguments_bind_in_place`. 문서 `internals.md` "호출 규약".
- **후속(2026-11-09)**: 내장 함수도 같은 `bind_in_place`로 묶는다(`Callee::Fung`·`Callee::Native`, 내장 함수의 기본값 상수는 묶을 때 채움, `Native::param_syms`는 만들 때 심볼로). `bind_args`·`enter`는 없어지고 내장 함수 실행은 `run_native` 하나(`call_op`·`start_call`이 씀). 측정(릴리스, `get` 500만 번): 기본값 생략 0.84 → 0.67초, 이름 인자 1.02 → 0.74초, 위치 인자 0.57 → 0.59초(잡음). 시험 `native_arguments_bind_in_place`. 같은 날 VM9의 남은 것인 프레임의 `proto`·`globals` 따로 쥐기도 없앰: `Frame`은 클로저 하나(`Frame::new`·`Frame::body`, 모듈 본문은 붙잡은 값 없는 클로저, `proto()`·`globals()`), `fib`·`loop`·위 벤치에서 차이 없음(잡음 안).

### VM7. `Sym::name()`이 전역 `Mutex`를 잡는다 — 성능, 낮음
- 재현: `fn f(i) = Ok(g(i)? + 1)`을 `f(i)!`로 천만 번 부르면 3.15초다. `result_kind`·`may_hold_err`가 미리 만든 심볼과 `u32`로 비교하는 실험 판은 2.99초(−5%)였다.
  `!`·`?`·`??`·`POP`의 Err 검사·이름 인자 묶기·태그 해시가 매번 잠근다.
- 명세 근거: `internals.md` "값의 표현"(Sym은 "전역 인턴 표의 번호라 비교가 정수 비교").
- 원인 위치: `crates/fung-vm/src/value/sym.rs` `Sym::name`(`table().lock()`). 쓰는 곳은 `vm/m4.rs` `result_kind`·`may_hold_err`, `vm/call.rs` `bind_args`, `value/compare.rs` 태그 해시, 그리고 `try_map`·`Probe::Err`·`is_ok` 류(`tag.name.name() == "Ok"`).
- 고치는 법: `Ok`·`Err`를 시작할 때 한 번 intern해 두고(`OnceLock`이나 미리 예약한 상수 번호) id로 비교한다. `name()` 자체는 이름이 추가만 되므로 잠금 없는 표(실행 스레드 전용 `thread_local`이나 조각 배열)로 바꿀 수 있다.
- **처리(2026-11-08)**: `value/sym.rs` `Ok`·`Err`를 고정 번호(`Sym::OK`·`Sym::ERR`, `intern`이 표 없이 돌려줌), `Sym::name`은 실행 스레드마다의 이름 사본(`thread_local`, 표가 늘기만 하므로 사본이 맞고, 모르는 번호에서만 잠금). `vm/m4.rs` `result_kind`·`may_hold_err`·`not_result`, `value.rs` `Probe::Err`, `prelude.rs` `try_map`, `value/format.rs` `result_tag`가 번호 비교. 측정: `f(i)!`·`Ok(g(i)? + 1)` 루프 3.706 → 3.514초(−5%, 점검 문서 실험 3.15 → 2.99와 같은 비율). 시험 `value/sym.rs` `reserved_ids_and_names_on_every_thread`.

### VM8. 태그 값(과 튜플·클로저)이 할당 두 번이다: `Rc<Tag>` + `Vec<Value>` — 성능, 중간
- 재현: VM7의 프로그램 표본에서 `malloc`·`free`·`Vec::from_iter`가 약 28%다. `f(Ok(i))`를 `match`로 푸는 천만 번 루프는 2.07초로, 같은 모양의 일반 호출 1.11초의 거의 두 배다.
  `Ok(x)`는 실패할 수 있는 모든 함수의 반환이라 스크립트에서 가장 흔한 할당이다.
- 원인 위치: `crates/fung-vm/src/value.rs` `pub struct Tag { name: Sym, args: Vec<Value> }`, `struct Tuple(Vec<Value>)`, `Closure { captured: Vec<Value>, … }`. `vm/m4.rs` `Op::Tag`(`pop_n`이 Vec을 만들고 `Rc::new`가 또 하나).
- 고치는 법:
  - 인자가 하나인 태그를 할당 한 번으로 둔다. 예: `enum Args { One(Value), Many(Box<[Value]>) }`로 `Ok(x)`·`Err(e)`는 `Rc` 하나. `unsafe` 없이 된다.
  - 튜플은 길이 2~3이 흔하므로 같은 작은 배열 표현을 쓰거나 `Rc<[Value]>`로 바꾼다.
  - 측정은 위 두 프로그램과 `bench/compare`로 한다.
- **처리(2026-11-08)**: `value/items.rs` `Items`(값 1·2개는 안에, 그 이상은 `Box<[Value]>`; `Deref<Target=[Value]>`, `From<Vec<Value>>`, `FromIterator`, 소유 `IntoIterator`). `Tag::args`, `Tuple`, `Closure::captured`가 사용. `TAG`·`TUPLE`·`CLOSURE`가 스택에서 바로 모으고 `nat::ok`가 `Vec` 없이 만든다. 바뀐 생성 지점: `nat::tag`(`args.into()`), `prelude.rs` 하나, `link.rs`, `m4.rs`, 시험의 구조체 리터럴 몇. 측정: `f(Ok(i))`+`match` 2.012 → 1.798초, 튜플 `f((i, 1))` 2.277 → 2.110초, VM7 루프 3.220 → 2.766초. 시험 `value/items.rs` `items_of_every_length`.

### VM9. 해석 루프·호출의 작은 고정 비용 셋 — 성능, 낮음
- 재현(각각 따로 바꾼 실험 판, 천만 번 루프):
  1. **자기 꼬리 호출이 Rc 셋을 갈아 끼운다.** `reuse_frame`이 같은 클로저로의 꼬리 호출에도 `proto`·`globals`·`closure`의 계수를 올렸다 내리고 `entry`를 다시 계산한다.
     "같은 클로저면 `tail_calls += 1; ip = 0`만" 하면 `loop` 0.62 → 0.57초다. 프레임이 `proto`·`globals`를 클로저와 따로 쥐는 것도 `push_frame`마다 계수 두 번을 더한다.
  2. **스칼라 피연산자를 꺼낼 때 `Value`의 drop 함수를 부른다.** `binary`·`compare`가 `pop()`으로 꺼낸 두 값을, 정수여도 `drop_in_place::<Value>`(줄 밖 함수, 표본의 약 9%)로 떨군다.
     스택 위에서 정수인지 보고 제자리에 결과를 쓰는 판은 `loop` 0.56 → 0.51초, `bench/fib.fung`(1과 함께)는 0.20 → 0.17초였다.
  3. **`EQ`·`NE`·`FLOORDIV`·`MOD`에 정수 빠른 길이 없다.** `EQ`는 정수끼리도 `value::eq → equal → equal_leaf`를 지난다(표본 약 6%). `ADD`·`LT`처럼 정수 갈래를 두면 된다.
- 원인 위치: `vm/call.rs` `reuse_frame`·`push_frame`, `vm.rs` `Task::binary`·`Task::compare`·`Op::Eq | Op::Ne`, `vm/frame.rs` `Frame`.
- 고치는 법: 위 각 실험 그대로이고, 셋 다 `unsafe` 없이 된다. `scripts/bench.sh`의 `fib`·`loop`로 잰다. `Instr::op`의 `#[inline]`은 이미 시험했으므로(`bench-languages.md` 3.4.1) 다시 하지 않는다.
- **처리(2026-11-08)**: (1) `vm/call.rs` `reuse_frame`이 같은 클로저면 `tail_calls += 1; ip = 0`만. (2) `vm.rs` `top_ints`·`put_over_ints`: 두 정수를 스택 위에서 보고 결과를 제자리에(꺼낸 정수는 `mem::forget`; `unsafe` 없음). (3) `EQ`·`NE`·`FLOORDIV`·`MOD`의 정수 갈래(`value::floor_div_i64`·`rem_i64` 재사용, 0과 넘침은 값 층으로). 프레임의 `proto`·`globals` 따로 쥐기는 그대로(main 프레임은 클로저가 없어 합치면 실행 루프에 간접 참조가 하나 는다). 측정: `vm9_loop` 0.918 → 0.685초, `bench/fib` 0.242 → 0.216초, `bench/loop` 0.294 → 0.253초(VM5·VM6과 같은 커밋이라 셋을 함께 잰 값; 각 벤치는 주로 한 항목만 지난다). 시험 `vm/tests.rs` `int_operators_agree_with_the_value_layer`.

### VM10. 코드 생성의 군더더기 — 성능·정리, 낮음
- **처리(부분, 2026-11-08)**: (a)는 VM1과 같은 커밋에서 고쳤다(넘긴 칸은 그 길에서 비었다고 세어 다시 비우지 않는다). (b)~(g)는 남았다.
`fung dump bytecode`로 본 것이다. 묶음 명령(superinstruction)이 아니라 컴파일러가 덜 내면 되는 것만 모았다.
- (a) **`MOVELOCAL` 뒤의 같은 슬롯 `CLEARLOCAL`.** 예: `0006 MOVELOCAL 0` … `0010 CLEARLOCAL 0`. 문장 안에서 모든 길이 넘긴 슬롯은 비울 필요가 없다(`moves`가 아는 정보).
- (b) **`match`의 주제가 이미 지역이어도 임시 슬롯에 복사한다.** `fn g(m) = match m: …`이 `MOVELOCAL 0; STORELOCAL 1; LOADLOCAL 1`로 시작하고, 갈래마다 `LOADLOCAL 1`을 한다.
  명령 둘과 슬롯 하나가 늘 뿐 아니라, 임시 칸이 주제를 계속 쥐어 갈래에서 묶은 이름(`m => put(m, k, v)`)이 홀로 쥔 값이 되지 못한다(제자리 수정 불가). desugar가 주제가 `(local n)`이면 그대로 쓰면 된다.
- (c) **`_`나 이름 하나만 묶는 갈래도 `MATCH`+`JUMPIFFALSE`**이고, 그 뒤의 `LOADLOCAL; NOMATCH`는 닿지 않는 코드다. 반박할 수 없는 패턴은 `STORELOCAL`(또는 아무것도)로 내고, 그 뒤 갈래와 `NOMATCH`를 생략한다.
- (d) **`and`·`or`가 조건 자리에서도 불을 만들었다가 다시 검사한다.** `JUMPIFFALSE 9 … 0009 CONST false; 0010 JUMPIFFALSE 13`처럼 점프가 상수를 지나 다시 점프한다. 조건 문맥에서는 바로 대상으로 점프한다("jumping code").
  비교·`not`·`and`/`or` 결과 뒤의 `CHECKBOOL`도 이미 불이라 필요 없다.
- (e) **문장 자리의 `else` 없는 `if`가 `nil`을 만들어 `POP`한다**(`JUMP 7; CONST nil; POP`). 가지마다 `POP`(E0606 검사는 그대로)하면 명령 둘이 준다.
- (f) 꼬리 자리 가지 끝의 `JUMP`가 `RETURN`으로만 간다(`0005 JUMP 11` → `0011 RETURN`). 그 자리에서 `RETURN`을 바로 낸다.
  `?`는 성공 길에 `JUMP`가 하나 더 있다(`UNWRAPTEST 3; JUMP 4; RETURN`). 실패 시 돌아가는 조각을 함수 끝으로 모아 `UNWRAPTEST`가 그리로 점프하게 하면, 성공 길이 명령 하나가 된다.
- (g) **상수 접기가 리터럴 모음에만 있다.**
  - `1 + 2 * 3`은 명령 다섯이고, 음수 리터럴 `-3`은 `CONST 3; NEG`다.
  - 끝이 상수인 범위 `0..=3`은 `CONST; CONST; RANGE`로, 평가할 때마다 `Rc<Range>`를 할당한다. `docs/plans/0.5.0/05-collections.md` "정할 것 2"의 "범위·`dur`는 이미 상수다"와 다르다.
  - 정수·실수 산술은 넘침·0 나누기면 접지 않고 그대로 두면, 에러 시점도 같다.
- 원인 위치: `crates/fung-front/src/compiler.rs`(`block`·`if_expr`·`op`), `compiler/m4.rs`(`match_expr`·`unwrap`·`collection`의 `constant`), desugar의 `match` 주제 처리.
- 고치는 법: 위 각 항목. (b)(c)(d)는 컴파일러만 고치면 되고, (a)는 `moves`의 결과로 `clear_unread`를 거른다(VM1과 함께). dump 기대 출력이 바뀌므로 한 번에 묶어 bless한다.
- (b)~(g) **처리(2026-11-09)**: (b) `desugar.rs` `match_expr`가 이름인 대상은 숨은 변수 없이 쓰되, 어느 절의 패턴이 그 이름을 묶으면 숨은 변수(부분 실패한 패턴이 슬롯을 쓸 수 있음). 함수를 부르는 본문을 가진 절은 뒤에서 읽지 않는 대상 슬롯을 놓는다(`compiler/m4.rs` `subject_done`, 03 허용 방향). (c) `always_matches`: `_`·이름 절은 `MATCH`·`JUMPIFFALSE` 없이, 뒤 절과 `NOMATCH` 생략(이름이면 `MOVELOCAL`/`LOADLOCAL` + 저장). (d) `compiler.rs` `cond`·`logic`·`makes_bool`: 조건 자리의 `and`·`or`는 점프 코드, 이미 불인 값의 `CHECKBOOL` 생략, 에러 위치는 같음. (e) `if_stmt`: 문장 자리의 `else` 없는 `if`. (f) `end_branch`·`emit_return`·`ret_pos`: 꼬리 자리 가지는 `[CHECKRETURN] RETURN`(위치는 함수 본문·`return`의 자리 그대로), `?`는 `emit_stubs`로 함수 끝 뒤에 반환 조각. (g) `compiler/fold.rs`: 정수 `+ - * // %`(넘침·0이면 안 접음), 실수 `+ - * /`(유한하고 −0.0이 아닐 때), 단항 `-`; `Const::Range`(끝이 정수 상수인 범위), 리터럴 모음 안에서도. dump 명령 수 10861 → 10596: JUMP −148, CONST −86, RANGE −43, LOADLOCAL −43, STORELOCAL −16, CHECKBOOL −18, MATCH −18, NOMATCH −18, JUMPIFFALSE −18, NEG −15, MOVELOCAL −12, ADD −11, MUL·SUB −4, DIV·FLOORDIV·MOD −2, RETURN +107(가지의 반환), CLEARLOCAL +74(절의 대상 놓기), CHECKRETURN +9, LOADGLOBAL +3, STOREGLOBAL +2. 측정(VM8 커밋 대비, 같은 날 다른 회): `vm8_tag` 1.758 → 1.633초, `vm8_tuple` 1.861 → 1.738초, `bench/fib` 0.199 → 0.195초, 나머지는 측정 오차 안. 시험 `compiler/tests.rs`(갱신·추가), `desugar.rs` `desugar_m4`(두 사례 추가), `vm/tests.rs` `leaner_code_keeps_its_meaning`, dump 기대 출력 bless. 문서 `manual/vm/01`·`02`(예제를 `a = 2; print(1 + a * 3)`로, 상수 접기 언급)·`03`(`-3`)·`05`(`match` 숨은 변수 예와 이름 대상)·`08`(스택 표, 상수 접기 단락, 꼬리 자리 `RETURN`, `and`·`or` 점프, 연습 문제)·`internals.md`(`size` 예의 산문, 호출 규약).

### VM11. 24비트를 넘는 피연산자를 조용히 자른다 — 버그(조용한 실패), 낮음
- 재현 안 함(코드 읽기). 한 원형에 상수나 명령이 1,600만 개 넘게 필요하다.
- 명세 근거: 원칙 "조용한 실패 없음", D-3 결정(컴파일 단계 한도는 E0115·E0230). VM 매뉴얼 08장은 "24비트면 충분하다"고만 한다.
- 원인 위치: `crates/fung-front/src/bytecode/op.rs` `Instr::new`(`(arg as u32) & ARG_MASK`), `bytecode.rs` `Proto::emit`·`Proto::patch`.
  상수 번호·점프 위치·`LIST`/`TUPLE`/`CONCAT` 개수·슬롯 번호·패턴·원형 번호가 모두 검사 없이 잘린다. 생성한 스크립트(큰 데이터 표를 코드로)라면 닿을 수 있고, 그러면 엉뚱한 상수·점프로 돈다.
- 고치는 법: `emit`·`patch`에서 `arg > ARG_MASK`면 E0230("constants in a function"·"instructions in a function" 등, limit 16777215)을 낸다. `emit`을 `Result`로 바꾸기 번거로우면, `Compiler`에 넘침 표시를 두고 끝에서 한 번 확인한다.
- **처리(2026-11-08)**: `bytecode.rs` `Proto::emit`·`patch`가 24비트를 넘는 첫 피연산자를 기억하고(`overflow`), `Proto::check_operands`가 E0230(`constants in a function`·`instructions in a function` 등, 한도 16777215). `compile`·`function`이 원형마다 부른다. `MAX_OPERAND`. 시험 `bytecode/tests.rs` `an_operand_past_24_bits_is_e0230`. 문서 `errors.md` E0230, `manual/vm/08-bytecode.md`, 설계 09.

### VM12. `Task`와 `Fiber`가 같은 필드 열여섯 개를 따로 두고, `stop`·`floor`는 늘 0이다 — 정리, 낮음
- 재현: 코드 읽기. `stop`·`floor`에 0 말고 다른 값을 쓰는 곳이 없다(`vm.rs` 초기화와 `exchange`의 `swap`뿐). `cont.rs` 머리 주석도 "`floor` is always 0: it is a guard"라고 적는다.
- 원인 위치: `crates/fung-vm/src/vm.rs` `struct Fiber`·`struct Task`·`Task::exchange`(필드마다 `swap`, `#[cfg(test)]` 탐침까지), `vm/call.rs` `deliver`(`frames.len() == self.stop`), `vm.rs` `drive`·`mark`(`self.floor`).
- 고치는 법: `Task { fiber: Fiber, shared, world }`로 두고 `exchange`를 `mem::swap(&mut self.fiber, f)` 한 줄로 줄인다. `stop`은 지우고 `frames.is_empty()`를 쓴다. `floor`는 지우거나 `debug_assert`로 둔다.
  지금 구조는 새 필드를 하나 더할 때마다 두 곳과 `exchange`를 함께 고쳐야 해서 빠뜨리기 쉽다.
  `Op::Jump`의 뒤로 가는 점프 갈래는 컴파일러가 내지 않는 경로다. 지우기보다 `debug_assert`와 주석으로 둔다.
- **처리(2026-11-08)**: `vm.rs` `Task { fiber: Fiber, shared, world }`, `exchange`는 `mem::swap` 한 줄, `stop`·`floor` 삭제(`frames.is_empty()`, `conts.is_empty()`), 뒤로 가는 `JUMP`는 `debug_assert`와 주석. `Task::in_pure` 추가(정규식). `cont.rs` 머리 주석. 동작 변화 없음(전체 시험).

### VM13. VM 설명 문서가 구현과 다른 곳 — 문서, 낮음
- `manual/vm/08-bytecode.md`:
  - 70행 "fung은 지금 53가지를 쓴다", 313행 "53가지다" → `Op::ALL`은 54개다(`MOVELOCAL`을 더한 뒤 안 고침).
  - "문장과 지역 변수"의 "`CLEARLOCAL 0`: `xs`(슬롯 0)는 3줄 뒤로 다시 읽히지 않는다…" → 실제 dump에서 `xs`는 `0006 MOVELOCAL 0`이 이미 넘겼고, `CLEARLOCAL 0`은 중복이다(VM10 a). 본문이 `MOVELOCAL`을 설명하지 않는다.
- `manual/vm/internals.md`:
  - "패턴 매칭"의 `size` 예: 산문은 "절마다 `LOADLOCAL 0`", "둘째 절의 패턴 `$1`은 슬롯 1에 묶는다(n)"라고 한다.
    생성된 dump는 `MOVELOCAL 0; STORELOCAL 1` 뒤 갈래마다 `LOADLOCAL 1`이고 패턴은 `$2`다. 펜스는 실제 출력이라 맞고 산문만 낡았다(생성기는 산문을 검사하지 않는다).
  - "호출 규약 — 빠른 길"의 "인자를 잠시 담는 배열도 태스크마다 하나를 재사용한다" → 그런 배열이 없다. 빠른 길은 인자를 스택에 두고, 나머지 길은 호출마다 새 `Vec`을 만든다(VM5·VM6).
- `docs/design/09-implementation-plan.md` "호출 규약"·"결과값 축약":
  - 명령 이름이 옛 이름이다. `CHECK_TYPE`·`CHECK_RETURN`·`TAIL_CALL`·`LOAD_SELF`·`UNWRAP_RETURN` → 지금은 `CHECKTYPE`·`CHECKRETURN`·`TAILCALL`·`LOADSELF`·`UNWRAPTEST`.
  - "반환 타입은 … `CHECK_RETURN`(그래서 TCO 제외)"는 T4 뒤로 틀렸다(자기 꼬리 호출은 TCO).
  - "`COALESCE L`: … 그 외는 그대로 올리고 L 너머로" → 실제는 `COALESCE`(오른쪽으로 점프) + 다음 `JUMP`, 명령 둘이다.
- `crates/fung-front/src/compiler/m4.rs` `constant()` 위 doc 주석 첫 단락 "An empty list is left to LIST: the runtime reads an empty list constant as the prelude's `args`" → 지금은 `[]`도 `CONST`이고 `args`는 `Const::Args`다(주석 두 개가 겹쳐 있다).
- `fung dump bytecode`의 `SLICE` 설명이 포함 비트(4)를 보이지 않는다. `xs[0..=1]`(`SLICE 7`)과 `xs[0..1]`(`SLICE 3`)이 모두 `; start, end`다. `bytecode/disasm.rs` `comment`의 `Op::Slice`에 `inclusive`를 더한다(`RANGE`는 이미 `; inclusive`).
- 고치는 법: 위 각 줄을 구현에 맞추고 매뉴얼을 다시 만든다. dump 설명이 바뀌는 `SLICE`는 기대 출력을 bless한다.
- **처리(2026-11-08)**: `08-bytecode.md` 53 → 54가지, 09의 명령 이름(`CHECKTYPE`·`CHECKRETURN`·`TAILCALL`·`LOADSELF`·`UNWRAPABORT`·`UNWRAPTEST`)과 T4 뒤의 TCO 문장·`COALESCE` + `JUMP`, `compiler/m4.rs`의 낡은 doc 주석, `disasm.rs`의 `SLICE`에 `inclusive`(`compiler/testdata/data.txt`). `internals.md`의 패턴 매칭 산문과 `08-bytecode.md`의 `MOVELOCAL` 설명은 VM 묶음 뒤에 이미 맞았다.

## 7. 값 층과 prelude (VA)

### VA1. 큰 맵의 제거 압축이 영속 사용에서 상각을 잃는다(같은 판에서 거듭 `remove`하면 O(n)씩) — 성능, 중간
- 재현:
  ```
  import time
  n = 20000
  full = fold(0..n, {}, (m, i) => put(m, "k${i}", i))
  half = fold(0..n // 2, full, (m, i) => remove(m, "k${i}"))   # live 10000, dead 10000
  fresh = fold(n // 2..n, {}, (m, i) => put(m, "k${i}", i))    # 같은 내용, dead 0
  ks = keys(half)
  # 키마다 "그 키를 뺀 맵"을 만든다
  a = fold(ks, 0, (acc, k) => acc + len(remove(fresh, k)))
  b = fold(ks, 0, (acc, k) => acc + len(remove(half, k)))
  ```
  결과는 같고, `fresh` 13.7 ms 대 `half` 10203.8 ms로 약 740배 차이다.
- 명세 근거: 02 "비용의 약속"(영속 컬렉션), `omap.rs` 머리 주석 "removal stays cheap on average". 평균은 판 하나를 한 줄로 이어 쓸 때만 맞다. 불변 값이라 같은 판을 여러 번 고치는 것은 언어의 정상 사용이다.
- 원인 위치: `crates/fung-vm/src/value/omap.rs:109` `Omap::remove`. `dead > live`이면 `*self = Omap::of(self.entries())`로 O(n) 재구성한다. 재구성 결과는 새 판에만 남고 옛 판은 문턱에 그대로라, 그 판에서 `remove`할 때마다 다시 O(n)이다.
- 고치는 법(결정):
  - (가) **추천**: 공유되지 않은 맵만 압축한다. `remove_fn`은 `take_map`으로 소유를 안다(`Rc::strong_count == 1`일 때만).
    공유된 판은 묘비를 남기고, 더 큰 상한(`dead > 4·live + 64`)에서만 압축한다. 고치는 범위가 `remove` 하나이고, 흔한 한 줄 사용의 비용은 그대로다.
  - (나) 삽입 번호를 키로 하는 영속 트리(B+트리)로 순서를 지켜 묘비 자체를 없앤다. 근본적이지만 omap 전체를 다시 짜야 한다.
  - (다) 한계로 적기만 한다. 조용한 O(n²)라 비추천.
- **처리(2026-11-08, 사용자 결정 (가) 공유되지 않은 맵만 압축)**: `prelude/collections.rs` `remove_fn`이 `Rc::strong_count`로 소유를 보고, 공유된 판은 `Map::remove_shared` → `Omap::remove(k, shared)`가 `dead > 4·live + 64`에서만 다시 만든다. 시험 `value::map_tests::only_a_map_held_alone_is_compacted_early`. 재현(`fresh`/`half`) half 10618.7 ms → 15.3 ms(fresh 15.8 → 14.7 ms), 20만 개 넣고 모두 빼기 309 → 245 ms. 문서 매뉴얼 `vm/internals.md`, `omap.rs` 머리 주석.

### VA2. 포함 끝 슬라이스 `xs[..=-(len+1)]`이 범위 밖인데 빈 결과를 낸다 — 버그, 중간
- 재현:
  ```
  $F -e 'xs = [1,2,3]
  print(xs[..=-4], xs[0..=-4], "abc"[..=-4])'   → [] [] (빈 문자열)
  $F -e 'xs = [1,2,3]
  print(xs[..=-5])'                              → E0507 slice end -5 is out of range (length 3)
  $F -e 'xs = [1,2,3]
  print(xs[..=3])'                               → E0507 slice end 3 is out of range
  ```
  양수 쪽은 `..=len-1`까지만 되는데, 음수 쪽은 `..=-len`보다 하나 더(`-len-1`)가 통과한다.
- 명세 근거: 02 리스트 "인덱스·슬라이스가 범위를 벗어나면 런타임 에러 … 에러(E0507)는 프로그램이 쓴 인덱스를 그대로 보인다", "Python식 슬라이스 클램핑은 채택하지 않음".
- 원인 위치: `crates/fung-vm/src/value/index.rs:169` `slice_bounds`. `hi = resolve(e) + inclusive`로 먼저 1을 더한 뒤 `0..=len`을 검사한다. `-4 + 3 = -1`에 1을 더해 0이 되어 통과한다.
- 고치는 법: 포함 끝은 더하기 전에, 실제 원소의 인덱스인지(`0..len`, 음수는 `-len..=-1`) 본다. 그러면 `xs[..=-4]`는 E0507 `slice end -4`다.
  빈 리스트의 `[][..=-1]`(지금은 `[]`, 같은 원인)도 이 규칙이면 E0507이 된다. `xs[..=-1]`을 "전부"로 쓰는 코드가 빈 리스트에서 깨지지 않게 하려면, "`..=-1`만 빈 리스트에서 허용"을 따로 정해야 한다(결정 한 줄).
- **처리(2026-11-08, 사용자 결정: 특별 허용 없음)**: `value/index.rs` `slice_bounds`가 포함 끝을 1 더하기 전에 `0..len`(음수 `-len..=-1`)인지 본다. 시험 `value::tests::an_inclusive_end_must_be_an_element`, `slice_errors_name_the_index_as_written`에 사례 추가. 문서 02, 매뉴얼 `language/access.md`.

### VA3. 슬라이스 끝에 리터럴 `nil`을 쓰면 열린 끝이 된다 — 버그, 낮음
- 재현:
  ```
  $F -e 'xs = [1,2,3]
  print(xs[1..nil], xs[nil..], xs[nil..=nil])'      → [2, 3] [1, 2, 3] [1, 2, 3]
  $F -e 'xs = [1,2,3]
  e = find(xs, x => x > 5)
  print(xs[1..e])'                                  → E0501 cannot slice with int and nil
  ```
- 명세 근거: 02 리스트 "끝에 nil이 오면 nil 인덱스처럼 E0501이다(`find`가 못 찾은 nil이 조용히 끝까지 자르지 않게)". `value/index.rs:133` `bound`의 주석도 "nil is not an open end"다.
- 원인 위치: 파서가 빠진 끝을 `Head::Nil` 노드로 둔다(`fung dump ast` → `(slice xs 1 (nil))`). `crates/fung-front/src/compiler/m4.rs:59` `Head::Slice`는 `head != Head::Nil`로 끝의 유무를 판정해, 사용자가 쓴 `nil`과 구분하지 못한다.
- 고치는 법: 빠진 끝을 별도 표시(`Head::Missing` 또는 슬라이스 노드의 플래그)로 파싱하고, 컴파일러는 그 표시만 본다. 포매터도 같은 표시를 쓰므로 `xs[1..nil]`이 그대로 남는지 시험한다.
- **처리(2026-11-08)**: 파서가 빠진 슬라이스 끝을 `Head::Missing`(`missing`)으로, `compiler/m4.rs` `Head::Slice`가 그것만 열린 끝으로 본다. 포매터는 토큰 기반이라 `xs[1..nil]`을 그대로 둔다. 시험 `parser::tests::newlines_inside_brackets_around_two_token_forms`(`xs[1..nil]` → `(slice xs 1 (nil))`), `tests/lang/err_slice_nil_end.fung`, 문서 07(`access.md`는 이미 그렇게 적혀 있었음).

### VA4. `min`·`max`·`zip`이 범위를 리스트로 펼쳐, 큰 범위에서 E0806과 수 초가 든다 — 버그, 중간
- 재현:
  ```
  $F -e 'print(min(0..200_000_000))'                  → E0806 too many elements (limit 100000000)
  $F -e 'print(zip(0..200_000_000, [1, 2]))'          → E0806 (결과는 두 개뿐)
  /usr/bin/time $F -e 'print(min(0..50_000_000))'     → 0, 5.53 real
  $F -e 'print(sum(0..200_000_000))'                  → 정상 (fold는 펼치지 않음)
  ```
- 명세 근거: 02 "크기 한도": "결과가 입력보다 작을 수 있는 함수(`filter`·`fold`·`each`)는 한도 없이 돈다". `extras/prelude.fung:26` 주석 "first/last/take/drop build only what they return".
  에러 문구도 사용자가 부른 함수가 아니라 안쪽 `map`의 것이다(F-4의 규칙).
- 원인 위치: `extras/prelude.fung:40-62` `min`·`max`·`zip`이 `_list(xs, …)`(= `map(_r.items(xs), x => x)`)로 범위를 펼친다.
- 고치는 법: 범위는 펼치지 않고 바로 답한다. `min(r)`은 비었으면 nil, 아니면 시작이다. `max(r)`은 `_r.last(r)`다. `zip`은 짧은 쪽 길이만큼 `_r.slice`로 잘라 펼친다.
- **처리(2026-11-08)**: `extras/prelude.fung` `min`(범위는 `_start`)·`max`(`_r.last`)·`zip`(범위는 다른 쪽 길이만큼 `_r.slice`로 잘라 `_upto`). 시험 `prelude::tests::prelude_in_fung_checks_like_a_native`. `min`+`max(0..5천만)` 15583.8 ms(1.78 GB) → 0.09 ms, `zip(0..5천만, [1, 2])` 2364.0 → 0.02 ms. 문서 매뉴얼 `prelude/collections.md`(zip·min·max).

### VA5. `trim`이 문자열 전체의 글자 표를 만든다(50 MB에 900 MB, 0.5초) — 성능, 중간
- 재현: `s = repeat("a", 50_000_000); t = trim(s)` → 0.51초, 최대 상주 메모리 911 MB(`s`만 만들면 0.10초, 59 MB).
- 원인 위치: `crates/fung-vm/src/prelude/strings.rs:271` `trim_fn`. `let us: Vec<(usize, Unit)> = units(s).collect();`(글자마다 16바이트)로 양 끝을 찾는다.
- 고치는 법: 앞에서는 `units(s)`로 첫 남길 글자를, 뒤에서는 끝에서 거꾸로 한 글자씩(UTF-8 뒤에서 읽기, 잘못된 바이트는 한 바이트) 찾는다. 메모리는 O(1)이고 시간은 잘라 낼 부분에 비례한다. 유효한 UTF-8이면 `str::trim_matches`로 바로 된다.
- **처리(2026-11-08)**: `prelude/strings.rs` `trim_fn`이 앞에서 `units`로, 뒤에서 새 `fung_front::text::last`(끝에서 거꾸로 한 글자, 앞에서 자른 것과 같음)로 찾고, 자를 것이 없으면 같은 값을 돌려준다. 시험 `text::tests::the_last_character_agrees_with_the_units`, `prelude::tests::trim_cuts_whole_characters_from_both_ends`. 50 MB: 406.1 → 0.003 ms, 앞뒤 공백 408.2 → 4.7 ms, 끝 `\xff  ` 428.4 → 24.7 ms; 프로그램 최대 상주 2.71 GB → 260 MB.

### VA6. `parse_dur`가 주석과 `-` 뒤의 공백을 받는다 — 버그, 낮음
- 재현: `$F -e 'print(parse_dur("5s # hi"), parse_dur("- 5s"), parse_dur("5s\n\n"))'` → `Ok(5s) Ok(-5s) Ok(5s)`.
- 명세 근거: 06 prelude "`parse_dur(s)`: 기간 리터럴 문법(`1h30m`, `500ms`, `-5s`, 앞뒤 공백 허용)". 주석·부호 뒤 공백은 리터럴 하나가 아니다. 같은 성격의 `parse_int(" 5")`는 `Err`다.
- 원인 위치: `crates/fung-vm/src/prelude/convert.rs:206` `parse_dur`. 남은 글을 렉서에 통째로 넣고 `Eof`/`Newline`이 아닌 토큰만 거부한다. 렉서는 주석과 공백을 버리므로 통과한다.
- 고치는 법: 다듬은 문자열의 첫 바이트가 숫자(부호 바로 뒤 숫자)인지 보고, 토큰이 `Duration` 하나이며 그 끝이 문자열 끝인지 본다. 렉서를 거치지 않고 앞단의 기간 리터럴 해석기를 함께 쓰는 편이 더 깔끔하다.
- **처리(2026-11-08)**: `prelude/convert.rs` `parse_dur`가 부호 바로 뒤 숫자를 요구하고, 기간 토큰 하나가 문자열 끝까지(`off == 0 && end == len`)인지 본다. 시험 `prelude::convert::tests::duration_is_one_literal`, `prelude/runs.txt` bless(`"- 5s"`가 `Err`). 문서 매뉴얼 `prelude/convert.md`.

### VA7. fung으로 쓴 prelude 몇이 인자를 검사하지 않아 내부 에러가 샌다 — 버그(+결정), 낮음
- 재현:
  ```
  pad_right("ab", 5, ["x"])   → E0403 argument `s` must be str, got list   (틀린 것은 pad인데 s라고 함)
  pad_left("ab", 5, 1)        → E0506 cannot take the length of int
  pad_left("a", 3.5)          → E0501 cannot slice with int and float
  pad_left("a", "3")          → E0501 cannot subtract str and int
  pad_left("a", 3, "")        → E0803 pad must be a non-empty string   (fail, 다른 내장 함수는 E0403)
  iterate(1, x => x * 2, 3.5) → 16 (n이 실수여도 조용히 4번)
  iterate(1, x => x, "n")     → E0501 cannot compare str and int
  until(1, x => x + 1, 3)     → E0401 cannot call a value of type int
  sum([1s, 2s])               → E0501 cannot add int and dur
  ```
  메시지가 사용자가 쓰지 않은 연산(뺄셈·비교·호출)을 말한다. 같은 파일의 `take`·`drop`은 E0403으로 바로 알린다.
- 명세 근거: `native/range.rs` 머리 주석과 F-4(2026-10-24) "the error names the function the user called"(`pad_*`·`iterate`·`until`은 이 규칙에서 빠졌다).
  06 "fung extras의 인자 검사는 `fail`(E0803)로 한다"는 F-4 뒤 규칙과 어긋난다. `strings.md:535` "`s`가 문자열이 아니면 E0506"은 리스트·맵이면 E0501이라 문서와도 다르다.
- 원인 위치: `extras/prelude.fung:19`(`sum`), `:108-116`(`_padding`·`pad_left`·`pad_right`), `:126`(`until`), `:129`(`iterate`).
- 고치는 법: 타입 표기(`n: int`, `pad: str`, `pred: fn`)를 붙이거나 `_r.count` 같은 다리로 앞에서 검사해 E0403으로 만든다. 06의 "extras는 `fail`" 문장도 지금 규칙으로 고친다.
  `sum`은 결정이다.
  - (가) **추천**: 빈 리스트면 0, 아니면 첫 원소부터 접는다. 기간 합계가 된다(`sum(durations)`는 흔하다).
  - (나) 숫자만 받는다고 문서에 적는다.
- **처리(2026-11-08, 사용자 결정 `sum` (가), 빈 `pad`는 E0403)**: `extras/prelude.fung` `pad_left`·`pad_right`(`s: str, n: int, pad: str`)·`iterate`(`f: fn, n: int`)·`until`(`f: fn, pred: fn`) 타입 표기, 빈 `pad`는 새 다리 `_native.range`의 `pad`(E0403 `argument `pad` must be a non-empty str`), `sum`은 `_r.items` 검사 뒤 첫 원소부터 접기(범위는 0부터). 시험 `prelude::tests::prelude_in_fung_checks_like_a_native`, `boot::tests::fung_prelude_functions`(E0803 → E0403). 문서 06("extras는 `fail`" 문장), 매뉴얼 `prelude/strings.md`·`collections.md`(sum)·`loops.md`·`errors.md`(E0803), `spec/builtins.txt`.

### VA8. 심볼과 문자열의 E0517 판정이 연산마다 다르다 — 결정, 낮음
- **처리(2026-11-08, 사용자 결정 (가))**: `value/compare.rs` `sym_meets_str`가 `equal`처럼 첫 차이에서 멈춘다. 시험 `eq_sym_str_pairs`·`err_eq_sym_str_compared`, 문서 02·`values.md`·CHANGELOG.
- 재현:
  ```
  $F -e "print(\"b\" in ['a, \"b\"])"          → E0517 ('a와 "b"), "b"가 실제로 들어 있는데도
  $F -e "print(unique(['a, \"b\"]))"           → ['a, "b"] (에러 없음)
  $F -e "print(unique(['a, \"a\"]))"           → E0517
  $F -e "print([1, 'a] == [2, \"a\"])"         → E0517 (== 는 1 대 2에서 이미 거짓)
  $F -e "print(['a] == [\"a\", 1])"            → false (길이가 달라 안을 보지 않음)
  ```
- 명세 근거: 02 "`sort`, `unique`, `contains`, `in`은 모두 이 표를 따른다", "리스트·튜플·태그·맵 안에서 맞붙어도 같다". 어디서 "맞붙는가"가 정해져 있지 않다.
  `compare.rs:166` 주석은 "the first such pair `equal` would meet"이라고 하는데, `equal`은 첫 차이에서 멈추고 `sym_meets_str`는 멈추지 않아 주석과 동작이 다르다.
- 원인 위치: `crates/fung-vm/src/value/compare.rs:174` `sym_meets_str`, `value/index.rs:223` `contains`, `prelude/collections.rs` `unique_fn`.
- 고치는 법(결정):
  - (가) **추천**: "`equal`이 실제로 견주는 쌍만"으로 정의한다. `sym_meets_str`를 `equal`과 한 걸음으로 합쳐 첫 차이에서 멈춘다. `in`은 원소를 차례로 견주다 찾으면 멈추고, `unique`는 해시 통 안의 쌍만 본다(둘 다 지금과 같음). 비용이 한 걸음이고, 규칙이 "실제로 견준 쌍"으로 설명된다.
  - (나) 값 전체를 늘 훑어 어디든 맞붙으면 에러로 한다. 엄격하지만, `unique`가 O(n²) 비교를 해야 같아진다.
  - 어느 쪽이든 02와 `values.md`에 한 줄로 적는다.

### VA9. 1억 글자 한도를 넘는 문자열을 만드는 길이 여럿이다 — 결정, 낮음
- **처리(2026-11-08, 사용자 결정 (가))**: `vm/call.rs`의 내장 함수 반환 네 곳(`returned`)이 `value::check_returned`로 잰다(문자열과 태그 한 겹). 외부 명령의 출력은 `process.rs`에서. 호출 비용 변화 없음(천만 번 1.61초). 시험 `value::tests::a_returned_string_keeps_to_the_limit`, 릴리스 바이너리로 `upper`·`str` 재현 확인. 문서 02·CHANGELOG.
- 재현:
  ```
  s = repeat("ß", 60_000_000); u = upper(s)      # len(u) = 120000000, 에러 없음
  xs = map(0..9_000_000, i => "abcdefghijk")
  s = str(xs)        # len 135000000, 에러 없음
  t = "${xs}"        # E0806 too many characters
  ```
  `from_bytes`·`lower`·`json.stringify`·`fs.read`도 세지 않는다(코드 읽기).
- 명세 근거: 02 "크기 한도": "문자열은 1억 글자까지다(`repeat`, `join`, `replace`, `+`, 보간)". 목록 밖이라 명세 위반은 아니다.
  하지만 같은 결과를 내는 `str(x)`와 `"${x}"` 중 한쪽만 에러인 것은 원칙 7과 어긋나고, 한도의 목적(메모리 폭주를 일찍 멈춤)이 다른 길로 샌다.
- 원인 위치: `prelude/convert.rs:138` `str_fn`, `prelude/strings.rs:284` `upper_fn`·`lower_fn`, `:474` `from_bytes_fn`. 보간은 `value/arith.rs` `check_chars`를 쓴다.
- 고치는 법(결정):
  - (가) **추천**: 문자열을 **만드는** 모든 내장 함수가 결과에 `check_chars`를 부른다(세기 한 번. `str`은 쓰는 중에 바이트 수로 먼저 끊을 수 있다). 규칙이 "문자열 값은 1억 글자를 넘지 않는다" 한 줄이 된다.
  - (나) 바깥에서 읽은 값(`fs.read`·소켓)은 예외로 두고, 만드는 연산만 막는다((가)의 범위를 정하는 문제).
  - (다) 지금대로 두고 02에 "한도는 나열한 연산만"을 분명히 적는다.

### VA10. 조각으로 나누지 않은 긴 내장 함수가 "알려진 한계" 목록보다 많다 — 문서·성능, 낮음
- 재현: `timeout(2ms, f)`로 감싸 기한이 언제 보이는지 쟀다.
  - 조각이 기한을 보는 것: `sort`·`unique`·`split`·`replace`·`join`·`reverse`·`pairs` 만들기가 약 2.3~3.1 ms에 끊긴다.
  - 기한을 못 보고 끝까지 도는 것:
    - `chars`(9 MB 문자열) 352.8 ms, `merge`(100만 + 100만 맵) 648.4 ms
    - `trim`(9 MB) 72.8 ms, `bytes`(9 MB) 47.6 ms
    - `pairs`(100만 맵) 32.0 ms, `flatten`(200만) 12.4 ms, `from_bytes`(500만) 9.0 ms
- 명세 근거: open-questions "알려진 한계 – 태스크"의 "한 번에 도는 연산" 목록에 `chars`·`bytes`·`from_bytes`·`pairs`·`keys`·`values`·`merge`·`group_by`의 결과 맵 만들기가 빠졌다. 특히 `merge`는 0.65초 동안 모든 태스크를 세운다.
- 원인 위치: `prelude/collections.rs:552` `chars_fn`, `:488` `pairs_fn`, `:536` `merge_fn`(`value/map.rs` `Map::merge`), `prelude/strings.rs:465` `bytes_fn`, `:474` `from_bytes_fn`.
- 고치는 법: `chars`·`bytes`·`from_bytes`·`pairs`는 `Parts`처럼 `Work`로 바꾸기 쉽다(출력 `List`를 제자리로 키우는 패턴이 이미 있다). `merge`는 `put` 루프를 `Work`로 바꾼다. 당장 하지 않을 것은 목록에 더한다.
- **처리(2026-11-08)**: `chars`(`strings::chars_of`, `split(s, "")`과 같은 `Work`)·`bytes`·`from_bytes`·`pairs`·`merge`(첫 인자를 혼자 쥐면 제자리)·`trim`·`group_by`의 결과 맵(`PerItem::finish`)을 `Work`로. 시험 `vm::park_tests::more_long_natives_give_their_turn`. `timeout(2ms)`이 끊기까지(전 → 뒤): chars 357.6 → 2.1 ms, bytes 55.9 → 2.3, from_bytes 8.1 → 2.1, pairs 32.0 → 2.2, keys·values 30 → 2.3, merge 664.3 → 4.7, trim(9 MB 공백) 76.3 → 2.6, trim(cut) 102.0 → 2.7 ms. 문서 open-questions "알려진 한계 – 태스크"의 목록(남은 것: 큰 리스트의 `==`·`in`·`+`·`str`, `flatten`의 긴 원소, 정규식, `upper`·`lower`, 큰 맵의 해제, 문자열 전체를 한 번 읽는 연산), 매뉴얼 `vm/12-scheduler.md`.

### VA11. 한 글자 문자열마다 할당해 `chars`·`split(s, "")`·`s[i]`가 무겁다 — 성능, 낮음
- 재현: `s = repeat("a", 50_000_000); t = chars(s)` → 4.07초, 최대 상주 메모리 4.64 GB(글자당 약 90바이트).
- 명세 근거: 02 "글자를 훑을 때는 `chars(s)`로 리스트를 만든다". 권하는 길이 가장 비싸다.
- 원인 위치: `prelude/collections.rs:552` `chars_fn`, `prelude/strings.rs` `split_fn`의 `Chars`, `value/index.rs:80` `index`(Str). 모두 `Value::bytes(…to_vec())`로 `Rc<Text>`+`Vec`을 새로 만든다.
- 고치는 법: 바이트 하나짜리 문자열 256개(ASCII 128개 + 잘못된 바이트 128개)를 스레드 지역 표로 미리 만들어 `Rc`를 나눠 준다. 리스트 칸 16바이트만 남아 메모리가 약 1/5이 되고, 할당이 사라진다.
- **처리(2026-11-08)**: `Value::char_str`가 한 바이트 문자열 256개를 스레드 지역 표에서 나눠 준다(`chars`·`split(s, "")`·`s[i]`). 시험 `value::tests::one_byte_characters_are_shared`. 5천만 글자 `chars` 1914.9 ms·5.78 GB → 526.6 ms·1.77 GB.

### VA12. `s[i]`가 늘 전체 글자 수를 먼저 세고, UTF-8이 아닌 문자열에서는 15~80배 느리다 — 성능, 낮음
- 재현(200만 글자 한글, 6 MB):
  - `len(s)`는 0.23 ms(유효 UTF-8). 앞쪽 몇 글자인 `s[i]`(`i < 1000`)도 0.23 ms라, 전체를 센다.
  - 잘못된 바이트 하나를 붙인 `b = s + "\xff"`에서는 `len(b)`가 3.37 ms, `b[1_999_990]`이 19.2 ms다.
- 명세 근거: 02 "룬 단위라 `s[i]`는 O(n)". O(n)은 약속이지만 `n`은 `i`까지여야 하고, 잘못된 바이트 하나 때문에 수십 배 느려질 이유는 없다.
- 원인 위치: `value/index.rs:80-87` `index`(Str: `norm_index(*n, s.char_len())` 뒤 `byte_at`), `value/text.rs` `char_len`(UTF-8이면 매번 `chars().count()`), `crates/fung-front/src/text.rs:77` `byte_at`(잘못된 바이트가 있으면 글자마다 `from_utf8`을 부르는 `units().nth`).
- 고치는 법:
  - 음이 아닌 `i`는 `byte_at`으로 바로 찾고, 끝을 넘으면 그때만 길이를 센다.
  - `Text`가 만들 때 글자 수를 재 두면(이미 UTF-8 검사로 한 번 훑는다) `len`도 O(1)이 된다.
  - 잘못된 바이트가 섞인 경우의 `byte_at`·`char_count`는 `utf8_chunks`로 덩어리마다 건너뛴다.
- **처리(2026-11-08)**: `value/text.rs` `Text`가 글자 수를 처음 셀 때 기억(`Cell`), `byte_at`은 UTF-8이면 64바이트 덩어리로 세어 건너뛰는 `text::utf8_offset`, 아니면 4 KB 창마다 검사하는 `text::byte_at`; `index`(Str)는 음이 아닌 `i`를 바로 찾고 음수는 끝에서(`text::back`·`utf8_back`), 범위 밖일 때만 길이를 센다. 시험 `text::tests::offsets_agree_with_the_units`, `value::tests::string_index_reads_up_to_the_character`. 한글 200만 자(100번 평균): `len(s)` 0.243 → 0.003 ms, `s[10]` 0.327 → 0.0007, `s[1_999_990]` 7.55 → 0.99, `len(b)`(끝에 `\xff`) 4.63 → 0.035, `b[10]` 4.48 → 0.0025, `b[1_999_990]` 23.0 → 5.4, `b[-1]` 36.1 → 0.0003 ms. 문서 02.

### VA13. 문자열 리스트의 `str`이 정수 리스트보다 12배 느리다 — 성능, 낮음
- 재현: 11글자 문자열 200만 개의 `str`은 429.6 ms, 정수 200만 개는 35.3 ms다. `str([repeat("a", 50_000_000)])`은 0.56초에 최대 메모리 561 MB다(문자열은 50 MB).
- 원인 위치: `crates/fung-front/src/consts.rs:175` `quote_bytes`. 문자열마다 (`$` 다음 `{`를 보려고) `Vec<Unit>`을 모으고 새 `String`을 만든다. 그것을 `value/format.rs` `leaf_text`가 다시 복사한다. ASCII 빠른 길이 없다.
- 고치는 법: `quote_bytes`를 `&mut String`에 쓰는 형태로 바꾸고, 다음 글자는 앞질러 보기(peekable)로 본다. 이스케이프가 필요 없는 ASCII 구간은 한 번에 `push_str`한다.
- **처리(2026-11-08)**: `fung_front::consts::write_quoted`가 `&mut String`에 쓰고 이스케이프 없는 ASCII 구간을 한 번에 옮긴다(`quote_bytes`는 그 위에). `value/format.rs`가 문자열·맵 키를 출력 버퍼에 바로 쓴다. 시험 `consts::tests::quoting_writes_what_it_did_a_character_at_a_time`(옛 구현과 같은 글). 11글자 문자열 200만 개 `str` 463.6~492.1 → 97.4~155.0 ms, `str([5천만 글자])` 466.4~470.8 → 72.9~173.9 ms, 최대 상주 755 → 355 MB, 사용자 CPU 1.2 → 0.42 s.

### VA14. `0 ** -1`이 0으로 나누기(E0502)가 아니라 E0504다 — 결정, 낮음
- 재현: `$F -e 'print(0 ** -1)'` → `E0504 result is not a finite number`. `0.0 ** -1`도 같다. `1 / 0`은 E0502다.
- 명세 근거: 07 "0으로 나누면 런타임 에러", "`2 ** -1 == 0.5`"(음수 지수는 역수). 음수 지수의 0은 사실상 0으로 나누기다. Python도 `ZeroDivisionError`를 낸다.
- 원인 위치: `value/arith.rs:299` `pow`.
- 고치는 법(결정): **추천**은 밑이 0이고 지수가 음수면 E0502를 내는 것이다(사용자가 보는 원인이 맞다). 다른 길은 그대로 두고 07에 적는 것이다.
- **처리(2026-11-08, 사용자 결정: E0502)**: `value/arith.rs` `pow`가 밑 0·음수 지수를 `divzero()`로. 시험 `value::tests::pow_cases`. 문서 07, 매뉴얼 `language/operators.md`.

### VA15. `max([nil])`이 `nil`이라 빈 입력과 구별되지 않는다 — 결정, 낮음
- 재현: `max([nil])` → `nil`, `min([])` → `nil`, `sort([nil])` → `[nil]`, `sort([nil, nil])` → E0501.
- 명세 근거: `extras/prelude.fung:37` 주석 "the first element seeds the fold so a nil element fails as `<` does" — 원소가 하나면 실패하지 않는다. 02 "nil은 조용히 퍼지지 않는다".
- 고치는 법(결정):
  - (가) 원소가 하나여도 그 원소가 순서를 가진 타입(`num`·`str`·`dur`·`time`·`list`·`tuple`)인지 확인한다.
  - (나) **추천**: 그대로 두고 매뉴얼에 "빈 입력과 `[nil]`을 가리려면 `is_empty`"를 적는다. 비용이 없고 드문 경우다.
- **처리(2026-11-08, 사용자 결정 (가))**: `extras/prelude.fung` `_ordered`가 첫 원소의 타입을 보고, 순서가 없는 타입이면 `x < x`로 `<`와 같은 E0501을 낸다. 시험 `prelude::tests::prelude_in_fung_checks_like_a_native`(`max([nil])`·`min([{}])`·`max([true])`). 문서 매뉴얼 `prelude/collections.md`(min).

### VA16. 02의 "자르기는 O(1)"이 구현과 다르다 — 문서, 낮음
- 재현 안 함(코드 읽기). `value/list.rs:157` `List::slice`는 결과가 벡터의 1/4보다 작거나 32개 이하이면 원소를 **복사**한다(큰 벡터를 붙잡지 않으려고).
  되풀이 자르기의 합은 등비라 선형으로 남는다(큐·덱 모양 2만 번이 8 ms이고 길이와 무관함을 측정으로 확인).
- 고치는 법: 02 "`len`·자르기(`xs[a..b]`, `...rest`)는 O(1)"을 "자르기는 O(1)이거나, 결과가 원래 벡터의 1/4보다 작으면 결과 길이에 비례하는 복사"로 고친다.
- **처리(2026-11-08)**: 02의 비용 약속을 "자르기는 O(1)이거나, 결과가 원래의 1/4보다 작거나 32개 이하면 결과 길이에 비례하는 복사"로.

### VA17. VM의 타입 이름 표에 앞단이 거절하는 `conn`·`listener`가 있다 — 정리, 낮음
- 재현: `$F -e 'fn f(x: conn) = 1'` → `E0212 unknown type conn`. 그런데 `value/types.rs:44-45` `kind_of_name`은 두 이름에 비트를 준다(닿지 않는 코드).
- 명세 근거: 07 "`_native.net` 안쪽 값(`conn`·`listener`)은 표기에 두지 않는다".
- 고치는 법: 두 줄을 지운다. 또는 `kind_of_name`을 앞단의 내장 타입 이름 표와 한 곳(`fung_front`)에서 만들어 어긋나지 않게 한다.
- **처리(2026-11-08)**: `value/types.rs` `kind_of_name`에서 `conn`·`listener` 삭제, 앞단 표 `fung_front::resolve::BUILTIN_TYPES`를 공개해 시험 `type_names_match_the_front_end`가 둘이 어긋나지 않게 지킴.

## 8. 동시성 런타임 (CC)

### CC1. `supervise`에서 처리기 없이 줄에 선 E0607은 범위가 닫힐 때까지 `on_fail`에 가지 않는다 — 결정, 중간
- **처리(2026-11-08, 사용자 결정 (가))**: `vm/scope.rs`. `lose`가 처리 중이 아니면 `World::handler_requests`에 범위를 올리고, 스케줄러의 `adopt`가 `start_handlers`로 처리 상태에서 시작하는 처리기 태스크(`TaskEnd::handler`)를 범위의 자식으로 띄운다. 시험 `tests/lang/supervise_lost_err_now.fung`(옛 판에서 교착), 섞는 스케줄러 씨앗 30개. 문서 05·CHANGELOG.
- 재현:
  ```
  import time
  t0 = time.monotonic()
  r = supervise(fn(s):
    t = spawn(s, () => Err("x"))
    sleep(10ms)
    print("type", type_of(t))          # 핸들을 마지막으로 읽음 → 이 문장 뒤에 놓인다
    print("dropped handle at", time.monotonic() - t0)
    sleep(500ms)
    print("body done at", time.monotonic() - t0)
    1
  ;;, fn(e):
    print("on_fail at", time.monotonic() - t0, e.code)
  ;;)
  print(r)
  ```
  출력: `dropped handle at 11.1ms` → `body done at 512ms` → `on_fail at 512.3ms E0607`. 본문이 끝없이 도는 서버(`http.serve`의 받기 반복)라면, `on_fail`은 정지할 때까지 오지 않는다.
  게다가 같은 실패라도 누가 처리하느냐에 따라 `on_fail`이 할 수 있는 일이 다르다.
  - 범위가 닫힐 때 여는 태스크가 처리한 E0607의 `on_fail`에서 `spawn(s, …)`(일꾼 다시 띄우기)를 하면, E0706 `spawn into a closed scope`로 프로그램이 죽는다.
  - 자식이 처리기가 된 런타임 에러(`1 / 0`)의 `on_fail`에서는 같은 `spawn`이 된다.
- 명세 근거: 05:112~122. 118행은 "다음에 실패한 자식이 처리기가 되어 함께 처리하거나, 범위가 닫힐 때"라고 지금 동작을 적지만, 121행의 이유는 "버린 순간에 알리므로 오래 사는 서버도 늦지 않는다"이다.
  `scope`(FailFast)는 버린 순간 실패하지만 `supervise`는 늦다. 스펙 `2026-10-18-concurrency-v2-r7-design.md` 3.3절은 "본문이 도는 중에도 실패를 바로 처리한다", "처리기 태스크는 실패가 있을 때 런타임이 띄우는 범위의 평범한 자식"이라고 했다.
- 원인 위치: `crates/fung-vm/src/vm/scope.rs` `ScopeState::lose`(Supervise 가지: 줄에 넣기만 함), `ScopeState::unread_lost`, 닫힐 때의 처리 `Scope::closed`·`Scope::next_failure`(이때 `open`은 이미 false).
- 고치는 법(결정):
  - (가) **추천**: `lose`가 Supervise 줄에 넣을 때 `handling`이 false면 그 범위에 처리기 파이버를 하나 띄운다(`TaskEnd`의 처리 상자만 가진 범위의 살아 있는 자식으로, 실패를 다 처리하면 끝남).
    스펙 3.3의 원래 모양이고 121행의 이유와 맞는다. 닫힐 때의 `on_fail`도 범위가 열린 동안 돌게 된다(`Scope::wait`가 처리기를 기다림). 비용은 드문 경로의 파이버 하나다.
  - (나) 05의 121행 이유를 `scope`에만 해당한다고 고치고, 닫힐 때의 `on_fail`에서 `spawn`이 E0706임을 적는다. 서버에서 실패가 늦게 보이는 구멍이 남는다.

### CC2. 시작한 "바꾸는" 풀 일 중의 취소·기한이 일의 값으로 새어 나간다 — 버그, 중간
- 재현(`mkfifo ff`, 0.4초 뒤 `echo hi > ff`로 막힌 일을 풀어 준다):
  ```
  import fs
  r = scope(fn(s):
    t = spawn(s, () => fs.copy("ff", "/nonexistent/dir/x"))
    sleep(100ms)
    cancel(t)
    print("cancelled")
    nil
  ;;)
  ```
  출력: `cancelled` 뒤 `error[E0607]: task's Err was never awaited: cannot copy to `/nonexistent/dir/x`…`, 종료 코드 1. 일부러 취소한 태스크가 범위의 실패가 된다.
  기한도 같다. `timeout(100ms, () => fs.copy("ff", "/nonexistent/dir/x"))`가 약 400ms에 `Err({kind: 'not_found, …})`를 준다(`'timeout`이 아님).
- 명세 근거: 05:209~210 "무엇을 바꾸는 일(쓰기, 지우기, 핸들)은 시작했으면 끝날 때까지 기한이 밀리고, **끝나는 즉시 끊긴다**". 스펙 15절 "버리는 일" 줄. 05:159 "취소된 태스크는 실패가 아니다".
  같은 상황의 외부 명령은 `CommandWait::finish`가 `t.check_cancelled()?`로 끊는다.
- 원인 위치: `crates/fung-vm/src/vm/offload.rs` `Job::ended`. 노드를 되돌리고 기한만 `expire_due`한 뒤 `finish`의 값을 그대로 돌려준다.
  확인 지점(`tick`)은 다음 호출에 있으므로, 래퍼(`extras/fs.fung`)가 그 값을 곧바로 돌려주면 태스크 함수의 결과가 되어 `TaskEnd::body_ended`로 간다(`Timeout::end`에도 `Ok(v)`로 간다).
- 고치는 법: `ended`에서 `own`을 되돌린 경우(`Kind::Wait`가 되감기 중이었음), `finish`를 돌린 뒤 결과를 버리고 `t.check_cancelled()`의 E0701로 되감는다. `finish`를 먼저 돌리는 이유는 열린 핸들 같은 것이 값으로 만들어져야 버려지며 닫히기 때문이다. `CommandWait::finish`와 같은 규칙이다.
  - 시험: 위 두 재현. 취소된 태스크는 `Err({kind: 'cancelled})`, `timeout`은 `'timeout`이어야 한다.
  - `fs.open`처럼 `finish`가 핸들을 만드는 일에서, 되감기와 함께 핸들이 닫히는지(닫기 스레드로 가는지)도 확인한다.
- **처리(2026-11-08)**: `vm/offload.rs` `Job::ended`가 되감기 중이던 `Kind::Wait`면 `restore_cancel_node` 뒤 `finish`를 돌리고(만든 핸들이 값이 되어 버려지며 닫기 스레드로 감) 결과를 버린 채 `check_cancelled`의 E0701로 되감는다. 시험 `park_tests.rs` `a_deadline_while_a_job_runs_wins_once_it_ends`(옛 `an_offload_wait_is_not_cut_by_a_cancel`의 nc1 기대를 `'timeout`으로), `a_cancel_while_a_job_runs_drops_its_value`(`block_job`과 `/dev/null`을 여는 `block_open_job`; 핸들이 놓였는지 `handle_alive`로 확인, testutil에 두 탐침 추가). 문서 05(풀 일 줄), 매뉴얼 `reference/prelude/concurrency.md`("그 일의 결과는 버린다").

### CC3. `fs.copy`가 FIFO에서 풀 스레드로 막혀 기한·취소를 영영 무시한다 — 버그, 중간
- **처리(2026-11-08)**: LB7과 같은 커밋(정규 파일이 아니면 `Err 'invalid`).
- 재현(쓰는 쪽·읽는 쪽이 없는 FIFO `ff`):
  ```
  import fs
  print(timeout(100ms, () => fs.copy("ff", "ff_out")))   # 원본이 FIFO
  print(timeout(100ms, () => fs.copy("in.txt", "ff")))   # 대상이 FIFO
  ```
  둘 다 alarm에 죽을 때까지 돌아오지 않는다. 같은 FIFO의 `fs.read`·`fs.write`·`fs.open`은 100ms에 `Err({kind: 'timeout})`/`Ok(<file>)`로 바로 돌아온다.
  `copy`는 `Kind::Wait`라, 일이 돌기 시작하면 끝날 때까지 기다린다. 상대가 오지 않으면 태스크·범위·`exit`·정지 요청의 루트 취소가 모두 멈추고, 풀 스레드도 하나를 영원히 쥔다.
- 명세 근거: 06:253~256 "FIFO는 이벤트 루프로 읽고 쓴다(L11) … 기한·취소를 지키고", 05 "기한은 정확히 지킨다".
- 원인 위치: `crates/fung-vm/src/native/fs.rs` `copy_fn`(689)·`copy`(708). `std::fs::read`·`write_file`을 `nat::offload`(Kind::Wait)로 돌리고, `pipe::is_fifo` 판정이 없다(`read_fn`·`write_fn`·`append_fn`에는 있다).
- 고치는 법: **추천**은 FIFO·문자 장치가 낀 `copy`를 `Err({kind: 'invalid, message: "not a regular file"})`로 거절하는 것이다. 복사는 정규 파일의 일이고, 필요하면 `read`+`write`로 쓸 수 있다.
  다른 길은 FIFO면 실행 스레드로 돌아와 `read_fifo`·`write_fifo`를 조합하는 것이다. 다른 `Kind::Wait` 일(`move`·`remove`·`mkdir` 등)이 FIFO에서 막히지 않음도 함께 확인한다. LB6·LB7과 같은 함수다.

### CC4. 외부 명령의 `fork`가 실행 스레드에서 돌아, 힙이 크면 모든 태스크가 멈춘다 — 성능, 중간
- 재현: 1ms마다 깨는 태스크의 최대 간격을, 본문이 `exec(["/usr/bin/true"])`를 20번 하는 동안 잰다.
  ```
  import time
  big = 0..3000000 |> map(i => "item number ${i}")
  worst = atom(0ms)
  scope(fn(s):
    done = atom(false)
    spawn(s, fn():
      fn tick(last):
        sleep(1ms)
        now = time.monotonic()
        swap(worst, w => now - last > w then now - last else w)
        deref(done) then nil else tick(now)
      ;;
      tick(time.monotonic())
    ;;)
    sleep(50ms)
    reset(worst, 0ms)
    each(1..=20, _ => exec(["/usr/bin/true"])!)
    reset(done, true)
  ;;)
  print("len", len(big), "worst gap", deref(worst))
  ```
  작은 힙(`big` 없음)에서는 최대 간격이 2.4ms, 300만 원소 리스트를 쥔 힙에서는 18.3~19.6ms다. exec 20번에 드는 시간도 42ms에서 146ms로 는다.
  `pre_exec` 훅이 있어 표준 라이브러리가 `posix_spawn` 대신 `fork`+`exec`를 쓰고, `fork`의 페이지 표 복사가 상주 메모리에 비례한다.
  같은 자리에서 `prepare`의 `$PATH` 훑기(`look_path`의 `metadata`), `cwd`의 `metadata`, `kernel_can_run`의 파일 읽기도 실행 스레드에서 막혀 돈다.
- 명세 근거: 스펙 10.1 "스레드 풀: `outside`를 지나는 작업을 맡는다. fs 전부, **외부 프로세스**…", 10.4 지연 목표(30ms 이하), 05 "I/O는 평범한 함수 호출이고, 기다리는 동안 다른 태스크가 실행된다".
- 원인 위치: `crates/fung-vm/src/native/process.rs` `run_command`(`prepare(...).and_then(|cmd| cmd.spawn())`), `prepare`·`look_path`·`kernel_can_run`.
- 고치는 법:
  - (가) **추천**: `prepare`와 `spawn`을 풀 일(`Kind::Wait`, 시작 전 취소면 버림)로 넘기고, `Child`(Send)를 받아 `CommandWait::start`로 잇는다. 실행 스레드는 멈추지 않고 `fork`도 풀 스레드에서 일어난다.
  - (나) `pre_exec`를 없애 `posix_spawn`을 쓰게 한다. 시그널 마스크는 `posix_spawnattr_setsigmask`로 되지만, `RLIMIT_NOFILE` 되돌리기를 대신할 길이 없어 비추천이다.
- **처리(2026-11-08, (가))**: `native/process.rs` `run_command`가 `own_group`·`prepare`·`spawn`을 풀 일로 넘기고(새 `offload::Kind::Start`: `Wait`처럼 기다리되 `Shared::starts` 풀, `pool.rs` `STARTS_MAX` 4), 받은 `Child`로 `CommandWait::start`. 일이 도는 중 취소가 오면 `finish`의 `check_cancelled`가 되감고, 시작된 명령은 `Started`의 `Drop`이 SIGKILL하고 풀에서 거둔다(REPL이 버린 경우도). 시험 `process_tests.rs` `exec_starts_on_the_pool_and_parks_on_the_loop`(offload 1번), Linux 전용 `without_pidfd_commands_take_no_pool_thread`는 그대로 통과(시작은 따로 풀이라 파일 풀 스레드 0), 릴리스 `perf.rs` `fifty_commands_at_once`(스레드 20개 미만)가 지킨다. 문서 05, 매뉴얼 `vm/internals.md`. 측정은 아래 "남은 것".
- **후속 결정(2026-11-09, 사용자 결정: 지금대로)**: 큰 힙의 `fork` 멈춤(17~19 ms)은 `libc::posix_spawn`으로 바꾸지 않고 알려진 한계로 둔다(열린 파일 한도를 되돌리는 일을 지키고, Linux에서 재지 못한 변경을 넣지 않음). open-questions "알려진 한계".

### CC5. 두 번째 신호(와 SIGHUP)의 즉시 종료가 자기 그룹의 외부 명령을 고아로 남긴다 — 결정, 중간
- **처리(2026-11-08, 사용자 결정 (가))**: `native/process.rs`의 그룹 등록부(`groups`, `kill_own_groups`)와 `signals::exit_now`. 시험 `signals.rs` `a_second_signal_ends_the_commands_it_started`, 문서 05·CHANGELOG.
- 재현(터미널 없이 실행해, 명령이 자기 프로세스 그룹에 있게):
  ```
  stop = stop_signal()
  r = scope(fn(s):
    t = spawn(s, () => sh("sleep 37"))
    wait_closed(stop)
    print("stop requested; draining")
    await(t)
  ;;)
  ```
  `kill -TERM`을 두 번 보내면 fung은 143으로 끝나지만, `pgrep -fl "sleep 37"`에 `sleep 37`이 남는다.
  첫 신호에서 보는 쪽이 없을 때, 그리고 `exit`·취소에서는 SIGTERM→SIGKILL로 끝낸다(05:187).
- 명세 근거: 05:72 "두 번째 신호는 정리를 기다리지 않고 곧바로 끝낸다", 05:187~191(자기 그룹은 "신호는 그룹 전체에 가서 손자까지 끝난다"는 취지). "정리"에 자식 끝내기가 드는지 명세가 말하지 않는다.
  서비스·cron에서 남은 자식은 아무도 거두지 않는다(systemd·컨테이너는 cgroup으로 끝내지만, 맨 cron은 그렇지 않다).
- 원인 위치: `crates/fung-vm/src/signals.rs` `exit_now`(`stop_requests`·`deliver`·SIGHUP이 부름). 자식 목록은 런타임 밖에서 보이지 않는다(`process.rs`의 `CommandWait`에만 있다).
- 고치는 법(결정):
  - (가) **추천**: `Shared`(스레드에 안전)에 자기 그룹으로 띄운 명령의 그룹 번호 집합을 둔다. `CommandWait`가 시작할 때 넣고 거둘 때 뺀다.
    `exit_now`는 끝나기 전에 그 그룹마다 `killpg(SIGKILL)`한다. 신호 스레드에서 잠금 하나와 시스템 호출 몇 번이라 "즉시"를 해치지 않는다.
  - (나) 지금대로 두고, 05·08에 "두 번째 신호·SIGHUP은 자기 그룹의 명령을 남긴다"를 적는다.

### CC6. 쥐고만 있는 `stop_signal()` 끝이 Ctrl-C를 조용히 삼킨다 — 결정, 낮음
- 재현:
  ```
  stop = stop_signal()
  r = timeout(1ms, () => wait_closed(stop))
  print("served", r)
  sleep(3s)
  print("finished normally")
  ```
  0.5초에 SIGINT를 보내도 아무 일 없이 3초를 다 자고 `finished normally`를 찍고, 종료 코드 0으로 끝난다.
  최상위 이름은 프로그램 끝까지 살아서 "보는 쪽"으로 남는다. 서버를 끝낸 스크립트가 그 뒤 긴 일을 하면 첫 Ctrl-C가 아무것도 하지 않는다(두 번째에야 130).
- 명세 근거: 05:62 "보는 쪽은 그 끝의 사본이 하나라도 살아 있는 상태"(지금 동작은 명세대로다). 반면 05·08의 의도 "정지 요청을 모르는 스크립트의 Ctrl-C는 지금처럼 곧바로 멈춘다"와 "조용한 실패 없음".
- 원인 위치: `crates/fung-vm/src/vm/stop.rs` `request`(`w.stop.rx.strong_count() > 0`).
- 고치는 법(결정):
  - (가) **추천**: 판정은 그대로 둔다. 다만 정지 요청이 보는 쪽에 갔는데 그 순간 그 채널에서 기다리는(`recv`·`wait_closed`·순회) 태스크가 하나도 없으면, 표준 에러에 note 한 줄(`stop requested; press again to end now`)을 쓴다. 의미는 그대로 두고 침묵만 없앤다.
  - (나) "보는 쪽"을 "지금 기다리는 태스크가 있음"으로 좁힌다. 쥐고 있다가 나중에 보는 정상 프로그램(`drain` 중의 서버)이 취소당할 수 있어 비추천이다.
  - (다) 문서에 함정으로만 적는다.
- **처리(2026-11-08, 사용자 결정 (가))**: `vm/stop.rs` `stop`이 닫기 전에 정지 채널에 기다리는 받는 쪽(`Channel::waited_on`: 막힌 `recv`·순회·`wait_closed`)이 있는지 보고, 보는 쪽은 있는데 기다리는 태스크가 없으면 표준 에러에 `note: stop requested; press again to end now`. 시험 `stop_tests.rs` `a_stop_nobody_waits_for_says_so`. 문서 05·08, 매뉴얼 `reference/prelude/concurrency.md`(`stop_signal`), `vm/14-cancellation.md`.

### CC7. 표준 입력 읽기 스레드를 못 띄우면 패닉하고 입력을 잃는다 — 버그, 낮음
- 재현 안 함(코드 읽기).
- 명세 근거: 스펙 15 "스레드를 만들 수 없을 때"는 E0801(`cannot start a job`), "조용한 실패 없음".
- 원인 위치: `crates/fung-vm/src/vm/stdin.rs` `Stdin::poll`. `s.source.take()`로 입력을 꺼낸 뒤 `.expect("start the standard input reader")`로 스레드를 띄운다.
  패닉 울타리가 E0801로 바꿔도 `source`는 이미 사라졌다. 그래서 다른 태스크의 다음 `read_line`은 대기 목록에 올라 바깥이 깨울 것으로 세어지고(교착으로 판정되지 않음) 영원히 기다린다.
- 고치는 법: 스레드를 먼저 띄우고 성공했을 때만 `source`를 넘긴다(실패면 되돌려 놓음). 또는 실패를 `Err`/E0801로 돌려주는 경로를 둔다.
- **처리(2026-11-08)**: `vm/stdin.rs` `Stdin::poll`이 스레드를 먼저 띄우고(스레드가 돌기 시작하면 `source`를 스스로 꺼냄), 실패하면 입력을 그대로 두고 에러를 돌려준다. `Task::stdin_poll`(vm.rs, 반환형만)·`prelude/io.rs`의 `read_line`·`read_lines`가 E0801 `cannot start a job: …`로. 시험 `park_tests.rs` `a_reader_that_cannot_start_is_e0801_and_keeps_the_input`(시험 전용 `fail_start_for_tests`).

### CC8. `TaskEnd`의 `scope: Option`은 늘 `Some`이다 — 정리, 낮음
- 재현 안 함(코드 읽기): `TaskEnd::new`를 부르는 곳은 `Task::start` 하나이고, `start`를 부르는 곳은 `spawn_in` 하나(`Some(s.clone())`)다.
- 원인 위치: `crates/fung-vm/src/vm/task.rs` `Task::start`, `TaskState::new`의 `unwrap_or_default`. `vm/scope.rs` `TaskEnd::body_ended`의 `let Some(s) = … else { return Ok(self.ended(t, r)) }` 가지에는 닿지 않는다.
- 고치는 법: `start`를 `spawn_in`에 합치고, `TaskEnd`는 `Rc<ScopeState>`를 쥐게 한다(`Option`은 `ended`의 take 용도만). 범위 없는 태스크가 원리상 없다는 것(05 "태스크 범위 안에서만")이 타입에 드러난다.
- **처리(2026-11-08)**: `vm/task.rs` `Task::start`를 `spawn_in`에 합침, `vm/scope.rs` `TaskEnd::new`가 `Rc<ScopeState>`를 받음(`Option`은 `ended`의 take 용도만), 닿지 않던 범위 없는 가지 삭제.

## 9. 표준 라이브러리 (LB)

### LB1. http 클라이언트가 URL의 쿼리를 인코딩하지 않고 요청 줄에 넣는다 — 버그, 중간
- 재현:
  ```
  python3 -I canned.py 23463 'HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok' 'HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok' &
  $F -e 'import http
  print(http.get("http://127.0.0.1:23463/a b?x=1 HTTP/1.0 2"))
  print(http.get("http://127.0.0.1:23463/p?q=한글"))'
  ```
  서버가 받은 요청 줄은 `GET /a%20b?x=1 HTTP/1.0 2 HTTP/1.1`이다. 경로의 공백은 `%20`이 됐지만 쿼리의 공백은 그대로라, 요청 줄에 토큰이 더 생긴다.
  둘째는 `GET /p?q=\xed\x95\x9c\xea\xb8\x80 HTTP/1.1`로, UTF-8 바이트가 그대로 나간다. 두 요청 모두 `Ok`다.
  CR/LF는 `_parse`의 제어 문자 검사가 막아 헤더를 끼워 넣을 수는 없지만, 요청 줄은 깨진다.
- 명세 근거: RFC 9112 3.2(request-target은 공백 없는 ASCII), RFC 3986 3.4. 06 "URL 경로의 UTF-8이 아닌 바이트는 퍼센트 인코딩해 보내고"는 경로만 말하고 쿼리는 말하지 않는다.
- 원인 위치: `extras/http.fung:729` `_request_uri`(쿼리 `u.query`를 그대로 붙인다). 경로만 `_escaped_path`(1333)를 거친다.
- 고치는 법: 쿼리에도 `_escaped_query`를 둔다. 쿼리에 쓸 수 있는 글자(RFC 3986 query = pchar / "/" / "?")와 `%XX`는 그대로 두고, 나머지 바이트는 `%XX`로 바꾼다. `_url_tail`(에러와 Referer)도 같이 바꾼다.
- **처리(2026-11-08)**: `extras/http.fung` `_escaped_query`(`_request_uri`·`_url_tail`), `_percent`를 경로 인코딩과 함께 씀. 시험 `http.rs` `the_query_is_escaped_in_the_request_line`, 기대 출력 `http/client.txt`·`proxies.txt`(요청 줄), 문서 06 "약속 밖"·`http.md` 약속.

### LB2. 리다이렉트가 포트·스킴만 바뀌어도 Authorization·Cookie를 그대로 보낸다 — 결정(보안), 중간
- **처리(2026-11-08, 사용자 결정 (가))**: `extras/http.fung` `_leaves_origin`·`_port_of`. https → http도 TLS 가짜 서버로 확인했다. 시험 `http.rs` `a_redirect_to_another_origin_drops_the_credentials`, 기대 출력 `http/redirects.txt`(포트가 바뀌는 세 사례), 문서 06·`http.md`·CHANGELOG "보안".
- 재현:
  ```
  python3 -I canned.py 23461 'HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:23462/x\r\nContent-Length: 0\r\n\r\n' &
  python3 -I canned.py 23462 'HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok' &
  $F -e 'import http
  print(http.get("http://127.0.0.1:23461/a", headers: {Authorization: "Bearer secret", Cookie: "s=1"}))'
  ```
  두 번째 서버(다른 포트이므로 다른 origin)가 `Authorization: Bearer secret`과 `Cookie: s=1`을 받는다.
  `https://h/` → `http://h/` 리다이렉트도 같은 코드 경로를 탄다(재현 안 함, 코드 읽기). 그러면 비밀값이 평문으로 나간다.
- 명세 근거: 06 "HTTP" 리다이렉트 "다른 호스트(첫 요청의 호스트나 그 하위 도메인이 아닌 곳)로 넘어가면 … 민감한 헤더를 버린다". 지금 동작은 이 문장대로다.
  다만 curl은 CVE-2022-27774(같은 호스트라도 스킴·포트가 바뀌면 자격 증명이 새던 문제) 뒤로 이런 경우 자격 증명을 버린다. Go는 기본 포트를 붙인 `host:port`끼리 견준다.
- 원인 위치: `extras/http.fung:184` `_hop`의 `away`, 215 `_same_site`(주석 "port and case aside").
- 고치는 법(결정):
  - (가) **추천**: 스킴이 https에서 http로 내려가거나 유효 포트가 바뀌면 `strip`한다(하위 도메인 허용은 그대로). 보안 사고를 막는 것이 편의보다 앞서고, curl·Go와 맞는다.
  - (나) https → http만 버린다.
  - (다) 지금처럼 두고 문서에 위험을 적는다.

### LB3. 서버가 핸들러 헤더를 검사하지 않고 조용히 고치거나 버리며, NUL은 그대로 보낸다 — 버그, 중간
- 재현(핸들러를 `http.serve("127.0.0.1:0", req => match req.path: … ;;, ready: a => print(a))!`로 띄우고 `raw.py`로 요청):
  - 핸들러가 `{headers: {"X Bad": "1", "X-Ok": "2"}}`를 돌려주면, 답에 `X-Ok: 2`만 있다. `X Bad`는 아무 말 없이 빠진다.
  - `{headers: {"X-A": "a\r\nSet-Cookie: evil=1", "X-N": "n\u{0}ul"}}`이면 `X-A: a  Set-Cookie: evil=1`(CR/LF가 말없이 공백으로 바뀜)과 `X-N: n\x00ul`(NUL을 그대로 보냄)이 나간다.
- 명세 근거: RFC 9110 5.5(필드 값에 NUL·CR·LF 금지), 5.1(이름은 token). 06 "서버" L13: "답의 틀은 서버가 정한다 … 잘못된 답이다"(TE·중복 이름은 500 + `http: bad handler response`). 원칙 1.
  클라이언트 쪽 `_bad_header`는 같은 경우를 에러로 거절하는데 서버 쪽만 다르다.
- 원인 위치: `extras/http.fung:1939` `_header_lines`(토큰이 아닌 이름은 filter로 빼고, `\r`·`\n`만 공백으로 바꾼다). 1852 `_parts`는 이름과 값의 문법을 보지 않는다.
- 고치는 법: `_parts`에서 이름이 `_is_token`이 아니거나 값이 `_valid_value`가 아니면 잘못된 답(500과 보고)으로 한다. 그러면 `_header_lines`의 바꾸기·거르기는 필요 없다. 매뉴얼의 잘못된 답 목록에도 더한다.
- **처리(2026-11-08)**: `extras/http.fung` `_parts`가 토큰 아닌 이름·제어 문자 든 값을 잘못된 답으로, `_header_lines`는 거르기·바꾸기 없이 앞뒤 공백만 뗌. 시험 `http.rs` `the_server_checks_answers_and_requests`, `SERVER`의 `/headers`를 바른 헤더로, 기대 출력 `http/server.txt`, 문서 06·`http.md`.

### LB4. 서버의 `req.headers`에 `host`가 없다 — 결정·문서, 중간
- 재현: `GET /hdr HTTP/1.1\r\nHost: example.org\r\nX-Y: 1` → 핸들러가 보는 `req.headers`는 `{"x-y": "1"}`이다.
- 명세 근거: `manual/reference/modules/http.md` "서버" `headers: 소문자 키의 맵`. 06의 `req` 설명에도 Host를 뺀다는 말이 없다. 핸들러가 가상 호스트를 가를 길이 없다.
- 원인 위치: `extras/http.fung:1648` `_request_head`(`_header_map(remove(h, "Host"))`).
- 고치는 법: **추천**은 `host`를 그대로 두는 것이다. 검사는 이미 `_valid_host`에서 했고, 지울 까닭이 없으며, 다른 서버 라이브러리는 모두 보여 준다. 다른 길은 지금처럼 두고 매뉴얼에 적는 것이다.
- **처리(2026-11-08, 사용자 결정)**: `extras/http.fung` `_request_head`가 `Host`를 지우지 않음. 시험 `the_server_checks_answers_and_requests`, 기대 출력 `http/server.txt`(echo의 `host`), 문서 06·`http.md`.

### LB5. `http.*`의 `timeout`에 음수를 주면 "제한 없음"이다 — 버그, 중간
- 재현: 3초 뒤에야 답하는 서버에 `http.get(url, timeout: -1s)` → 3초를 기다려 `Ok`.
  같은 바이너리에서 `timeout(-1s, () => 1)`은 곧바로 `Err({kind: 'timeout, message: "timed out after 0s"})`다.
- 명세 근거: 06 "HTTP" "`0s`면 제한 없음"(음수 이야기는 없다). 05 기한 `d <= 0`은 곧바로 시간 초과다. 같은 이름의 인자가 함수마다 반대 뜻이 된다(원칙 7).
- 원인 위치: `extras/http.fung:65` `_request`(`limit > 0s then … else _attempt(...)`).
- 고치는 법: **추천**은 음수를 E0403으로 거절하는 것이다(`timeout`은 0 이상, `max_body`처럼). 다른 길은 prelude처럼 곧바로 시간 초과로 하는 것이다.
  "0이면 제한 없음"과 prelude의 "0이면 곧바로 시간 초과"가 다른 것도 매뉴얼에 나란히 적는다.
- **처리(2026-11-08, 사용자 결정)**: `extras/http.fung` `get`·`post`·`request`의 `timeout: dur | nil`, `_request`가 `nil`이면 한도 없이, 아니면 늘 `_within`(prelude `timeout`). 시험 `http.rs` `a_timeout_of_zero_or_less_times_out_at_once`, `cases`·`ARG_ERRORS`의 `timeout: 0s`를 `nil`로, 문서 06·`http.md`.

### LB6. `fs.copy`가 파일 전체를 메모리에 읽는다 — 성능, 중간
- **처리(2026-11-08)**: LB7과 같은 커밋(64 KiB 버퍼로 흘려 보냄).
- 재현: `mkfile -n 500m big.bin; /usr/bin/time -l $F -e 'import fs` 뒤 줄에 `print(fs.copy("big.bin", "big2.bin"))'` → 최대 RSS 533,807,104바이트(`cp`는 1,146,880).
  메모리가 작은 컨테이너에서는 큰 로그나 이미지 하나를 복사하다 OOM으로 죽는다.
- 원인 위치: `crates/fung-vm/src/native/fs.rs:708` `copy`(`std::fs::read` 다음 `write_file`).
- 고치는 법: 열린 두 파일 사이를 `std::io::copy`로 흘려 보낸다. Linux는 `copy_file_range`, macOS는 `fcopyfile`을 쓰는 `std::fs::copy`도 된다. 권한과 닫기 에러를 보고하는 방식(L11)은 그대로 둔다. CC3·LB7과 함께 한다.

### LB7. `fs.copy`는 있는 파일을 말없이 덮고, `fs.move`는 거절한다 — 결정, 중간
- **처리(2026-11-08, 사용자 결정 (가))**: `native/fs.rs` `copy_paths`(`overwrite`). LB6·CC3도 같은 커밋. 시험 `fs_tests::copy_refuses_an_existing_target_unless_asked`, 문서 06·`fs.md`·CHANGELOG.
- 재현: `fs.copy("d/f", "d/c1")`을 두 번 부르면 두 번 다 `Ok`(덮음)다. `fs.copy("d/f", "d/lnk")`(링크)는 링크가 가리키는 파일을 덮는다. 반면 `fs.move`는 `Err 'exists`다.
- 명세 근거: 06 `fs` F-11 "`move`는 이미 있는 것을 덮지 않는다(… 이름 충돌로 파일을 말없이 잃던 것)". 같은 까닭이 `copy`에도 그대로 들어맞는다. 원칙 7.
- 원인 위치: `fs.rs:708` `copy` → `write_file`(`create + truncate`).
- 고치는 법(결정):
  - (가) **추천**: `copy(from, to, overwrite = false)`로 한다. 기본은 `O_EXCL`로 만들고, 있으면 `Err 'exists`다. `move`와 같은 규칙이 된다.
  - (나) 지금 동작을 두고 매뉴얼에 두 함수가 다르다고 적는다.

### LB8. `fs.move`가 다른 파일 시스템 사이에서는 늘 실패한다 — 결정, 중간
- **처리(2026-11-08, 사용자 결정 (가))**: `native/fs.rs` `rename`이 EXDEV면 `move_across`. 실제 EXDEV는 돌리지 못해 함수 단위 시험 `fs_tests::a_move_across_file_systems_copies_a_file`.
- 재현 안 함(코드 읽기, macOS에서 다른 볼륨을 만들지 않음). `rename`, `renameat2`·`renamex_np`, `hard_link`가 모두 EXDEV로 실패해 `Err({kind: 'io, message: "cannot move `…`: invalid cross-device link"})`가 된다.
  Linux에서는 `/tmp`가 tmpfs인 경우가 흔해, "`with_temp_dir`에서 만들고 `fs.move`로 제자리에 놓기"가 실패한다. 셸의 `mv`는 복사 + 지우기로 되므로, 셸 대신 쓰려는 언어(00·background "쉘 대용")에서 흔히 걸린다.
- 명세 근거: 06 `fs` move 규칙도 매뉴얼 `fs.md`도 이 경우를 말하지 않는다.
- 원인 위치: `crates/fung-vm/src/native/fs.rs:590` `rename`, 612 `rename_noreplace`.
- 고치는 법(결정):
  - (가) **추천**: EXDEV이면 파일은 복사(LB6의 흘려 보내기, no-replace) + 지우기로 한다. 디렉터리는 거절한다.
  - (나) 지금처럼 두고 매뉴얼에 "같은 파일 시스템 안에서만"과 에러 문구를 적는다.

### LB9. 코드가 틀린 것이 `Err` 값으로 돌아오는 자리들(파일 모드, glob 패턴) — 결정, 중간
- **처리(2026-11-08, 사용자 결정 (가))**: glob은 E0515를 함께 쓴다(문구가 "invalid pattern"이라). 핸들 모드는 새 번호 E0520(`fs::fault_value`가 `Result`). 기대 출력 사례에서 맞지 않는 호출을 빼고 Rust 시험으로 옮김. 시험 `err_file_wrong_mode`·`err_glob_pattern`·`fs_tests`, 문서 06·`fs.md`·`io.md`·`errors.md`·CHANGELOG.
- 재현:
  - `'write`로 연 핸들에 `read_line(f)`를 부르면 `Err({kind: 'invalid, message: "file `d/w.txt` is not open for reading"})`다. 읽기 핸들에 `fs.write_line`도 마찬가지다.
  - `fs.glob("d/[")`는 `Err({kind: 'invalid, message: "bad glob pattern …"})`다. 그런데 잘못된 정규식 패턴은 런타임 에러 E0515다.
- 명세 근거: 06 "에러 규칙"은 외부 요인이면 `Ok`/`Err`, "코드가 틀린 것은 런타임 에러"다. 모드와 패턴은 코드가 정하는 값이라 외부 요인이 아니다. 06 "정규식" "잘못된 패턴은 런타임 에러 E0515(코드 오류)"와 glob이 서로 어긋난다.
- 원인 위치: `crates/fung-vm/src/native/fs.rs:374` `glob_fn`(BadPattern → `err_value("invalid")`), 핸들의 모드 검사는 `fault_value`(813) 쪽.
- 고치는 법(결정): **추천**은 둘 다 런타임 에러로 하는 것이다(새 번호, 또는 glob은 E0515와 같은 꼴). 규칙이 하나가 된다.
  바깥 입력으로 받은 glob 패턴을 미리 검사할 길이 필요하면, open-questions의 `regex.compile` 후보와 함께 본다.

### LB10. `regex.replace`에서 패턴에 없는 그룹(`$9`, `${nope}`)이 빈 문자열이 된다 — 결정, 중간
- **처리(2026-11-08, 사용자 결정 (가))**: `native/regex.rs` `check_repl`(크레이트 `expand`와 같은 이름 읽기), 새 번호 E0521. 기대 출력: 800개 패턴에는 어느 패턴에서나 유효한 형식만, 그룹 형식은 그룹을 모두 가진 패턴 하나의 프로그램 사례로. 시험 `err_regex_replace_group`·`regex_replace_groups`·`regex_tests::replacement_templates`, 문서 06·`regex.md`·`errors.md`·CHANGELOG.
- 재현: `regex.replace("abc", "b", "$9")` → `"ac"`, ``regex.replace("abc", "b", `${nope}`)`` → `"ac"`.
- 명세 근거: `manual regex.md` "그런 그룹이 없으면 빈 문자열이 된다"(문서화돼 있다). 원칙 1에 걸린다. 그룹 이름 오타가 데이터를 말없이 지운다. 패턴과 `repl`을 호출할 때 둘 다 알고 있으므로, 그룹이 있는지는 매치 전에 알 수 있다.
- 원인 위치: `crates/fung-vm/src/native/regex.rs`의 replace(크레이트의 `Captures::expand`를 그대로 씀).
- 고치는 법(결정): **추천**은 `repl`을 미리 해석해, 패턴에 없는 번호나 이름이면 E0515 꼴의 런타임 에러(``replacement refers to group `nope`, which the pattern does not have``)로 하는 것이다.
  있는 그룹이 매치에 안 잡힌 경우는 지금처럼 빈 문자열이다.

### LB11. `cli.parse`: `bools`를 `_` 꼴로 주면 말없이 무시되고, 빈 이름도 받는다 — 결정, 중간
- **처리(2026-11-08, 사용자 결정 (가))**: `extras/cli.fung`이 `bools`를 `_key`로 바꿔 견주고, 빈 이름은 `fail`. 시험 `cli_bools_names`·`err_cli_empty_flag`·`cli.fung`의 test, 문서 `cli.md`·CHANGELOG.
- 재현:
  - `cli.parse(["--dry-run", "x"], bools: ["dry_run"])` → `{flags: {dry_run: "x"}, rest: []}`. 스위치가 `"x"`를 값으로 먹는다.
  - `cli.parse(["--=x", "--", "a"])` → `{flags: {"": "x"}, …}`.
  - `--dry-run=false`(bools에 있음) → 문자열 `"false"`.
- 명세 근거: `manual cli.md` "`bools`에 적은 이름(… `-`는 그대로: `"dry-run"`)"(문서화돼 있다). 결과 키는 `dry_run`이라 사용자가 그 꼴로 적기 쉽다. 틀린 꼴이 에러가 아니라 다른 결과로 이어진다(원칙 1).
- 원인 위치: `extras/cli.fung:25` `_flag`(`body in bools`가 바꾸기 전의 이름과 견준다).
- 고치는 법(결정): **추천**은 `bools`도 `_key`로 바꿔 견주는 것이다. 그러면 두 꼴이 모두 된다. 빈 이름 `--=x`는 `fail`(E0803)로 한다. bools 스위치의 `=값`도 E0803으로 막을지 함께 정한다.

### LB12. 핸들러의 1xx 상태가 "1xx + 200" 두 답이 되고, HTTP/1.0 클라이언트에도 1xx를 보낸다 — 버그, 낮음
- 재현: 핸들러가 `{status: 103, headers: {Link: "</x>"}, body: "e"}`를 돌려줄 때 `GET /early HTTP/1.0`을 보내면,
  `HTTP/1.0 103 Early Hints\r\nContent-Type: …\r\nLink: </x>\r\n\r\nHTTP/1.0 200 OK\r\n…e`가 온다.
- 명세 근거: RFC 9110 15.2 "A server MUST NOT send a 1xx response to an HTTP/1.0 client". 매뉴얼과 06에는 1xx를 200으로 바꾼다는 말이 없고, 101만 잘못된 답으로 적혀 있다. 핸들러가 돌려준 상태가 말없이 200이 되는 것은 원칙 1과 맞지 않는다.
- 원인 위치: `extras/http.fung:1913` `_reply`(1xx 갈래), 1866 `_parts`.
- 고치는 법: **추천**은 101처럼 1xx 모두를 잘못된 답(500)으로 하는 것이다. 핸들러는 답을 하나만 돌려주므로 1xx를 보낼 길이 원래 없다.
- **처리(2026-11-08)**: `extras/http.fung` `_parts`의 상태 범위를 200~599로(101 따로 두지 않음), `_reply`의 1xx 갈래 삭제. 시험 `the_server_checks_answers_and_requests`, 기대 출력 `http/server.txt`(`/early`·`/switch`), 문서 06·`http.md`.

### LB13. 서버가 HTTP/1.0 요청의 Transfer-Encoding을 무시해, 조각 전송 본문이 빈 본문이 된다 — 버그, 낮음
- 재현:
  - `POST / HTTP/1.0\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n` → 핸들러에 `len=0 body=`(본문을 말없이 잃음).
  - `Transfer-Encoding: gzip` + `Content-Length: 3`(1.0) → 코딩을 무시하고 `abc`를 그대로 넘긴다. 같은 요청이 1.1이면 501이다.
- 명세 근거: RFC 9112 6.1 "A server or client that receives an HTTP/1.0 message containing a Transfer-Encoding header field MUST treat the message as if the framing is faulty".
- 원인 위치: `extras/http.fung:925` `_transfer_encoding`(1.1 미만이면 `Ok((rest, false))`). 서버(1626)와 클라이언트(888)가 함께 쓴다.
- 고치는 법: 서버에서는 HTTP/1.0 + TE를 400으로 한다. 클라이언트는 연결 끝까지 읽기로 다루는 지금도 괜찮다(응답 하나 뒤에 닫으므로). 함수에 "요청인지" 인자를 두거나, 서버 쪽에서 먼저 검사한다.
- **처리(2026-11-08)**: `extras/http.fung` `_request_head`에서 HTTP/1.0 + `Transfer-Encoding`은 400(클라이언트는 그대로). 시험 `the_server_checks_answers_and_requests`, 기대 출력 `http/server.txt`, 문서 06·`http.md`.

### LB14. 클라이언트가 `+20`·`999`·`HTTP/2.0` 상태 줄을 받아들인다 — 버그, 낮음
- 재현: `canned.py`가 차례로 `HTTP/1.1 +20 OK`, `HTTP/1.1 999 X`, `HTTP/2.0 200 OK`로 답한다 → `Ok({status: 20, …})`, `Ok({status: 999, …})`, `Ok({status: 200, …})`.
  (`Content-Length: +2`는 `invalid Content-Length`로 바르게 거절한다.)
- 명세 근거: RFC 9112 4 `status-code = 3DIGIT`, RFC 9110 15 "Values outside the range 100..599 are invalid". HTTP/1.1 요청에 HTTP/2.0 상태 줄은 틀이 맞지 않는다.
- 원인 위치: `extras/http.fung:788` `_status_line`. `_atoi`(808)가 부호를 받고, 3바이트 검사가 `+20`을 지나보내며, 범위 검사는 `code < 0`뿐이다. `_version`이 major 2를 받는다.
- 고치는 법: 세 글자가 모두 숫자이고 100~599인지 본다. major가 1이 아니면 `invalid HTTP version`으로 거절한다.
- **처리(2026-11-08)**: `extras/http.fung` `_status_line`이 세 자리 숫자 100~599와 `HTTP/1.`을 봄(`_atoi` 삭제). 시험 `http.rs` `status_lines_need_three_digits_and_http_1`, 문서 06·`http.md`.

### LB15. `_native.net.need_count`의 E0403이 값 대신 타입을 보인다 — 버그, 낮음
- 재현: `http.get("http://127.0.0.1:1/", max_body: -1)` → `error[E0403]: argument `max_body` must be a non-negative int, got int`.
  json은 같은 경우에 `got -1`이라고 바르게 쓴다(`json.stringify([1], indent: -1)`).
- 원인 위치: `crates/fung-vm/src/native/net.rs:1283` `need_count`(`nat::wrong_arg`가 `type_name`을 쓴다). 바른 꼴은 `json.rs:490`, `conc.rs:255`다.
- 고치는 법: `nat::fail("E0403", [name, "a non-negative int", 값])`으로 쓴다. 이 셋을 `nat`의 도우미 하나로 모은다.
- **처리(2026-11-08)**: `native/nat.rs` `not_count` 하나로 `net.rs` `need_count`·`json.rs` indent·`conc.rs` channel의 E0403을 모음(값을 보임). 시험 `net::tests::wrong_arguments_are_e0403`.

### LB16. 서버가 본문 한도를 보기 전에 `100 Continue`를 보내고, HTTP/1.0의 Expect에 417로 답한다 — 버그, 낮음
- 재현 안 함(코드 읽기). `_respond`가 `Content-Length: 20000000` + `Expect: 100-continue`에 먼저 100을 보낸다. 그래서 클라이언트는 20 MB를 다 보낸 뒤에야 413을 받는다. 큰 본문을 보내기 전에 거절하려는 Expect의 목적에 어긋난다.
  HTTP/1.0 요청의 모르는 Expect에도 417을 준다.
- 명세 근거: RFC 9110 10.1.1 "a server MAY … send a final status code instead of 100 (Continue)", "A server that receives a 100-continue expectation in an HTTP/1.0 request MUST ignore that expectation".
- 원인 위치: `extras/http.fung:1765` `_respond`, 1789 `_request_body`.
- 고치는 법: 100을 보내기 전에 `rq.length > _max_body`면 곧바로 413을 보낸다. Expect 검사는 1.1 이상일 때만 한다.
- **처리(2026-11-08)**: `extras/http.fung` `_respond`가 `Expect`를 HTTP/1.1 이상에서만 보고, `100-continue`인데 `Content-Length`가 16 MiB를 넘으면 100 없이 413. 시험 `the_server_checks_answers_and_requests`, 문서 06·`http.md`.

### LB17. `fs.write`는 그 자리에서 잘라 쓴다(원자적이지 않음) — 결정, 낮음
- 재현 안 함(코드 읽기). `write_file`/`write_closed`는 `O_TRUNC` 다음에 `write_all`이다. 도중에 죽거나 디스크가 차면 설정 파일이 비거나 반만 남고, 동시에 읽는 쪽은 빈 파일을 볼 수 있다.
  `fung fmt`는 "옆에 임시 파일을 쓰고 이름을 바꿔 덮으므로 … 반만 남지 않는다"(`manual/reference/cli.md`)인데, 사용자에게는 같은 도구가 없다. LB8 때문에 임시 디렉터리를 거쳐 옮기는 방법도 막힐 수 있다.
- 고치는 법(결정): **추천**은 `fs.write(path, text, perm = nil, atomic = false)`(같은 디렉터리의 임시 파일 + rename, `fmt`의 코드를 재사용)를 0.5.0 뒤 후보로 open-questions에 적는 것이다. 다른 길은 매뉴얼에 패턴만 적는 것이다.
- **처리(2026-11-08, 사용자 결정)**: `native/fs.rs` `write_whole`(같은 디렉터리 임시 파일 `.<이름>.<pid>-<n>.tmp`, `fsync`, `close_checked`, `rename`; 실패하면 임시 파일을 지우고 원래 파일 그대로; 하드 링크 여럿·소유자 다름·정규 파일 아님·빈 링크·임시 파일을 못 만드는 디렉터리는 그 자리에)를 `fs.write`와 `fung fmt`(`fmt.rs` `write`)가 함께 씀. 시험 `fs::tests::write_replaces_a_regular_file_whole`, 문서 06·`fs.md`·`cli.md`.

### LB18. `fs.glob`은 읽지 못하는 디렉터리를 말없이 건너뛴다(`list`·`walk`는 에러) — 결정, 낮음
- 재현: `chmod 000 d/nr` 뒤 `fs.glob("d/nr/*")` → `Ok([])`. `fs.list("d/nr")`와 `fs.walk("d")`는 `Err({kind: 'permission, …})`.
- 명세 근거: `manual fs.md` 345 "읽지 못하는 디렉터리는 조용히 건너뛴다"(문서화돼 있다). 이것은 원칙 1, 그리고 06 "`walk`가 읽지 못한 디렉터리는 그 경로로 에러가 난다"와 다르다. 권한 문제로 정리 스크립트가 "지울 파일 없음"으로 끝날 수 있다.
- 고치는 법(결정): **추천**은 패턴이 들어가는 디렉터리를 읽지 못하면 `walk`처럼 그 경로의 `Err`로 하는 것이다. 다른 길은 문서대로 두는 것이다(셸·Go의 `Glob`과 같음).
- **처리(2026-11-08, 사용자 결정)**: `native/glob.rs` `glob`이 `Failure::{Pattern, Path}`를 돌려줌(읽지 못한 디렉터리·stat 실패는 그 경로, 없음·파일은 맞는 것 없음), `fs.rs` `glob_fn`이 `fs_err("glob", …)`. 시험 `fs::tests::listings_keep_gos_order`(잠긴 디렉터리 네 사례), 기대 출력 `fs/runs.txt`(잠긴 디렉터리를 지나는 패턴은 `[^l]*`·`ln*`로 나눔), 문서 06·`fs.md`.

### LB19. `path`의 끝 `/` 규칙이 함수마다 달라 `stem + ext != base`다 — 결정, 낮음
- 재현:
  ```
  a/b/   dir: a/b    base: b      ext: ""   stem: b
  dir.d/ dir: dir.d  base: dir.d  ext: ""   stem: dir
  ```
  `stem("dir.d/") + ext("dir.d/")`가 `base("dir.d/")`가 아니다. `dir("a/b/")`과 `base("a/b/")`를 다시 이으면 `a/b/b`다(셸 `dirname a/b/`은 `a`).
- 명세 근거: `manual path.md` `stem` "`path.base`에서 `path.ext`를 뗀 것"(`ext`가 `p`의 것인지 `base(p)`의 것인지 둘 다로 읽힌다). `base`는 "끝의 `/`는 무시", `dir`는 "끝에 `/`가 있으면 그 앞까지가 디렉터리"(Go와 같음). 원칙 7.
- 원인 위치: `crates/fung-vm/src/native/path.rs` `stem_fn`(`ext(&base(p))`), `ext`(`p`를 그대로 본다).
- 고치는 법(결정): **추천**은 적어도 `ext`를 `base(p)`에서 계산해 `stem + ext == base`가 늘 서게 하는 것이다. `dir`도 끝 `/`를 무시할지(셸 `dirname`처럼)는 함께 묻는다.
- **처리(2026-11-08, 사용자 결정)**: `native/path.rs` `trimmed`로 `dir`·`ext`가 끝 `/`를 무시. 시험 `path::tests::join_base_dir_ext_stem`·`tests/lang/path_rules.fung`, 기대 출력 `prelude/runs.txt` 한 줄, 문서 06 표·`path.md`.

### LB20. `time`: 날짜 없는 형식이 지역 LMT 오프셋을 내고, 윤초·잘못된 `TZ`는 말없이 처리된다 — 결정·문서, 낮음
- 재현:
  - `TZ=Asia/Seoul`에서 `time.parse("12:30", "HH:mm")` → `Ok(0000-01-01T12:30:00+08:27:52)`. 0000년은 문서에 있다(`time.md` 138). 그 결과 서울의 지방 평균시 오프셋이 붙는 것은 놀랍다. `TZ=America/New_York`에서는 `-04:56:02`다.
  - `time.parse("2026-12-31T23:59:60Z")` → `second out of range`. RFC 3339 5.6은 `time-second`에 60을 허용한다. 거절 자체는 괜찮지만(Go도 거절) 06·매뉴얼에 적혀 있지 않다.
  - `TZ="Asia/Seoul "`(끝 공백), `TZ="garbage!!"` → 아무 말 없이 UTC. 문서에는 "모두 실패하면 UTC"라고 있다. 그래도 cron 환경의 오타 하나가 시각을 말없이 9시간 틀리게 만든다.
- 고치는 법(결정):
  - 형식에 날짜가 없으면 E0403 또는 `Err 'parse`로 하거나, 1970-01-01 UTC 같은 고정 기준을 쓴다.
  - 윤초는 매뉴얼에 "60은 받지 않음"을 적는다(이것은 꼭 한다).
  - `TZ`를 읽지 못하면 첫 사용 때 표준 에러에 경고 한 줄을 쓴다.
- **처리(2026-11-08, 사용자 결정)**: `native/timefmt.rs` `names_a_date`, `time.rs` `parse_fn`의 E0403; `native/tz.rs` `resolve`(이름을 읽었는지)와 `local()`의 경고 한 줄; 윤초는 문서. 시험 `time::tests::parse_formats_need_a_date`, `tz::tests::a_tz_naming_no_zone_is_told`, `time_expected.rs`(날짜 없는 형식 사례를 날짜 있는 형식으로 바꾸고 E0403 사례 하나 더함, 사례 수 176·22; `No/Such_Zone`의 경고), `tests/lang/time_rules.fung`, 문서 06 "시간 형식"·`time.md`.

### LB21. 두 벌의 헤더 코드, 문구로 하는 판정, 낡은 주석 — 정리, 낮음
- 헤더 줄 규칙이 Rust(`net.rs` `Fields::flush`/`read`)와 fung(`extras/http.fung:843` `_field_step`, 874 `_flush`, 862 `_fields_of`) 두 벌이다. fung 쪽은 이제 트레일러(`_trailer_lines` → `_fields_of`)만 쓴다.
  트레일러도 `_net.read_fields`(예산 4096)로 읽으면 fung 쪽 세 함수를 지울 수 있다. http.fung 머리 주석(15~18줄)은 아직 `_field_step`을 서버와 함께 쓰는 리더로 소개한다.
- `extras/http.fung:1518` `_accept`가 리스너가 닫혔는지를 `e.message == "listener is closed"` 문자열로 판정한다(`net.rs:66` `LISTENER_CLOSED`). 문구를 바꾸면 서버가 끝없이 백오프로 돈다. `kind: 'closed`(06 kind 어휘)를 주고 kind로 판정한다.
- `crates/fung-cli/src/paths.rs`의 `base`·`read_dir`이 `fung_vm::native::path::base`, `native/fs.rs` `read_dir`과 거의 같다(`join_bytes`·`os_error_text`는 이미 가져다 쓴다).
- 낡은 주석이 둘 있다.
  - `crates/fung-vm/src/native/time.rs:4` "runs under the execution right": 실행권 런타임(3a)은 R5에서 지웠다.
  - `crates/fung-cli/src/test.rs`의 `/// Compiles one file with its tests and runs it.`가 `sched_seed` 위에 붙어 있다. `test_file`의 문서 주석이 자리를 잃었다.
- **처리(2026-11-08)**: 트레일러를 `_net.read_fields`로 읽고 `extras/http.fung`의 `_field_step`·`_fields_of`·`_flush`·`_trailer_lines`·`_trailer_end`·`_trailer_err` 삭제, 머리 주석 고침; `net.rs` `Fail::Closed`(`kind: 'closed`, 닫힌 연결·리스너)와 `_accept`의 kind 판정; `fung-cli/src/paths.rs`가 `native::path::base`·`native::fs::read_dir`를 씀; `time.rs` 머리 주석(실행권), `fung-cli/src/test.rs`의 `test_file` 문서 주석 제자리. 시험 `net::tests::close_wakes_a_task_blocked_on_the_same_handle`·`the_fd_index_empties_with_the_table`(kind), 기대 출력 `http/client.txt` 한 줄.

### LB22. 06 표의 http 서명이 늦고, 쿼리 규칙이 매뉴얼에 없다 — 문서, 낮음
- 06 "모듈" 표의 `http` 줄은 `get(url, headers = {}, timeout = 30s)`이고 `max_body`(L13)가 없다. `serve(addr, handler, ready = nil)`에도 `drain`·`reuse_port`가 없다(아래 "서버" 절에는 있다).
- 서버의 `query`는 `;`가 든 쌍을 버리고(`http.fung:1708` `_query_pair`), 1만 쌍을 넘으면 통째로 `{}`다. `manual http.md` "서버"에는 "잘못된 `%` 이스케이프가 든 쌍은 빠진다"만 있다.
  1만 쌍 넘는 쿼리를 말없이 비우는 것은 원칙 1에도 걸린다. 400이 더 맞다(이 부분은 결정).
- **처리(2026-11-08, 사용자 결정)**: 06 표의 `http` 서명(`max_body`, `timeout: dur | nil`, `serve`의 `drain`·`reuse_port`), `extras/http.fung` `_query`가 1만 쌍을 넘으면 `nil` → 400. 시험 `the_server_checks_answers_and_requests`, 기대 출력 `http/server.txt`, 문서 06·`http.md`(`&`·`;`·잘못된 이스케이프·1만 쌍).

## 10. CLI 도구 (TL)

### TL1. `exit(256)`이 성공(0)으로 끝난다. 08과 매뉴얼이 서로 다르다 — 결정·문서, 중간
- **처리(2026-11-08, 사용자 결정 (가))**: `prelude/io.rs` `exit_fn`이 0~255 밖을 E0403으로. 시험 `tests/lang/err_exit_range.fung`·`err_exit_negative.fung`, 문서 08·`cli.md`·`program.md`·CHANGELOG.
- 재현: `$F -e 'exit(256)'; echo $?` → `0`. `exit(-256)` → `0`.
- 명세 근거: 08 "종료 코드" "`exit(n)`은 사용자가 원하는 코드를 그대로 낸다", `manual cli.md` "256으로 나눈 나머지가 된다". 두 문서가 서로 다르다.
  `exit(len(failed))` 같은 흔한 코드가, 실패가 256개면 성공이 된다(원칙 1).
- 원인 위치: `crates/fung-cli/src/run.rs` `status`.
- 고치는 법(결정):
  - (가) **추천**: `exit`의 인자는 0~255이고, 밖이면 E0403이다. 조용한 성공을 막는다.
  - (나) 0이 아닌데 나머지가 0이면 1로 한다.
  - (다) 지금대로 두고 08을 매뉴얼에 맞춘다.

### TL2. REPL에서 여러 줄 원시 문자열을 입력할 수 없다 — 버그, 낮음
- 재현: 파일에서는 ``x = `a⏎b` ``가 된다. 그런데 ``printf 'x = `a\nb`\nprint(x)\n' | $F``는 첫 줄에서 `error[E0104]: unterminated string`을 낸다.
- 명세 근거: 08 "REPL" "블록이 열려 있거나 … 줄이 이어짐 토큰으로 끝나면 다음 줄을 이어 받는다". 01이 허용하는 여러 줄 리터럴을 REPL만 못 받는다.
- 원인 위치: `crates/fung-cli/src/repl/mod.rs` `needs_more`(렉서 에러면 `false`).
- 고치는 법: 렉서의 "닫히지 않은 문자열" 에러가 입력 끝에서 났으면 `true`로 한다.
- **처리(2026-11-08)**: `crates/fung-cli/src/repl/mod.rs` `needs_more`가 E0104일 때 끝에 `` ` ``를 붙여 다시 렉싱해 그 에러가 사라지면(열린 원시 문자열) 이어 받는다. 입력이 형식 중간에서 끝나면 모은 입력을 컴파일해 문법 에러를 보인다. 시험 `needs_more_when_the_parser_stops_at_the_end`에 사례, `repl/sessions.txt`에 "a raw string over lines". 문서 08 REPL, 매뉴얼 `reference/cli.md`.

### TL3. 표준 입력이 파이프인 REPL: 프롬프트와 `nil`이 표준 출력에 섞이고, 에러가 나도 종료 코드는 0이다 — 결정, 낮음
- 재현: `printf '1/0\nprint("after")\n' | $F; echo $?` → 표준 출력 `> > after\nnil\n> \n`, 표준 에러에 E0502, 종료 코드 `0`.
- 명세 근거: `manual cli.md` 146 "`fung < inputs.txt`로 입력을 차례로 넣어 볼 수 있다", 08 "`echo 'print(1)' | fung`이 된다". 결과를 파이프로 쓰면 출력이 더러워지고, `set -e`가 실패를 보지 못한다.
- 원인 위치: `crates/fung-cli/src/repl/line.rs` `PlainReader`(프롬프트를 stdout에 씀), `repl/mod.rs` `Session::run`.
- 고치는 법(결정):
  - (가) **추천**: 터미널이 아니면 프롬프트를 쓰지 않고, 에러가 한 번이라도 났으면 1로 끝낸다.
  - (나) 터미널이 아니면 표준 입력 전체를 스크립트로 실행한다(`node`·`sh`처럼). 그러면 `read_line`과 입력을 나눠 쓰는 지금의 쓰임이 깨진다.
- **처리(2026-11-08, 사용자 결정 (가))**: `repl/line.rs` `PlainReader`가 프롬프트를 쓰지 않고 `LineReader::interactive`가 false, `repl/mod.rs` `Session`이 에러마다 `failed`를 세우고 `finish`가 `exit(n)` 아니면 에러 시 1(런타임의 `failed()`도 봄). 시험 `tools_expected.rs` `repl/sessions.txt`(다시 씀, "exit after an error" 추가, `REPL_CASES` 32), `repl_*` 개별 시험들, `repl_tty.rs` `a_pipe_never_sets_raw_mode`·`repl/tty.txt`. 문서 08(REPL·종료 코드 표), 매뉴얼 `reference/cli.md`(REPL·종료 코드 표).

### TL4. 남는 명령줄 인자를 명령마다 다르게 다룬다 — 버그, 낮음
- 재현:
  - `fung dump core -e '1' extra junk` → 남는 인자를 무시하고 실행해 0. `fung version extra`, `fung help extra`도 무시하고 0.
  - `fung check -e '1' extra` → 3. `fung check ok.fung extra`도 3인데, 문구가 `check needs a script path or -e code`다(경로를 줬으므로 틀린 안내).
- 명세 근거: 08 종료 코드 3 "잘못된 인자". 08 `check`는 남는 인자를 사용 오류로 한다고 정했다.
- 원인 위치: `crates/fung-cli/src/main.rs` `run`(version/help), `run_dump`(`[e, rest @ ..]`), `check.rs`의 문구.
- 고치는 법: 남는 인자는 모두 `usage_error`로 한다(``dump takes one path or -e CODE``). `check`의 문구를 경우마다 나눈다. `USAGE`에 `check -e`·`dump … -e`를 더한다.
- **처리(2026-11-08)**: `main.rs` `run`(version/help 남는 인자), `run_dump`·`check.rs` `run_check`의 경우 나누기와 문구, `USAGE`에 `check -e`·`dump … -e`. 시험 `main.rs` `extra_arguments_are_usage_errors`, `check.rs` `arguments`, 기대 출력 `cli/help.txt`(`version extra` 추가, `HELP_CASES` 9)·`cli/version.txt`·`check/arguments.txt`·`dump/dumps.txt` 등 사용법 바뀐 곳. 문서 08, 매뉴얼 `reference/cli.md`(사용법·남는 인자).

### TL5. `fung test`가 시험 0개에도 성공하고, 잘못된 `FUNG_SCHED_SEED`를 말없이 무시한다 — 결정, 낮음
- 재현: `fung test empty_dir` → `0 passed, 0 failed`, 종료 0(경로 오타나 빈 디렉터리가 CI에서 통과한다). `FUNG_SCHED_SEED=abc`나 `-1`이면 아무 말 없이 무작위 씨앗을 쓴다(`test.rs` `sched_seed`, `parse().ok()`).
  같은 이름의 시험은 FE20에서 다룬다.
- 고치는 법(결정): **추천**은 시험 0개에 경고 한 줄(또는 pytest처럼 다른 종료 코드), 잘못된 씨앗에 사용 오류(3)를 내는 것이다.
- **처리(2026-11-08, 사용자 결정)**: `test.rs` `run_test`가 시험 0개면 `no tests found`(표준 에러)와 1, `sched_seed`가 `Result`로 숫자만의 u64가 아니면 ``error: FUNG_SCHED_SEED must be a whole number from 0 to 18446744073709551615, not `abc` ``와 3(파일을 돌기 전). 시험 `tools_expected.rs` `test_without_tests_fails`·`test_refuses_a_bad_scheduler_seed`, 기대 출력 `test/repository.txt`(시험 없는 파일 170개가 1)·`test/fixtures.txt`(`no_tests.fung`). 문서 08, 매뉴얼 `reference/cli.md`.

## 11. 확인했고 문제없던 것 (다시 검사하지 않아도 됨)

- **명세·연산자**
  - 우선순위 표 13단계가 07, `operators.md`, 두 EBNF에서 같다.
  - `not`의 위치(`a == not b`는 에러), 비교 연쇄 E0111, `-2 ** 2 == -4`, `2 ** -1`, `**`의 오른쪽 결합.
  - `?`·`!`·`??` 표(nil·Err·그 외).
  - E0606이 `then`·`??`·파이프 문장·`for` 문장·`scope`·`timeout`·`map` 문장에서 모두 난다.
  - 인자 규약: E0116, E0405, `...rest` 뒤 금지, `f(a: _)` 금지, 모듈 import 형태(E0304·E0310·E0306).
- **값**
  - 정수 넘침 전부(`MIN // -1`, `MIN % -1 == 0`, `(-2) ** 63`), 실수 `//`·`%`(CPython `float_divmod`와 같음), Inf/NaN 거절.
  - `-0.0`의 출력·`==`·해시, `int(실수)`의 경계, 정확한 int/float 비교.
  - 기간 산술과 출력 왕복. 시간대가 다른 같은 순간의 `==`·해시.
  - 출력과 다시 읽기: 무작위 바이트 400개를 담은 모든 컬렉션 종류, 실수 경계값.
  - 영속 벡터: 경계 크기(31~32800) × 씨앗 17개 × 300~400단계의 무작위 시험.
  - 맵: 키 공간 5~2000에서 넣기·덮기·빼기·`merge`를 섞은 기준 비교, `==`·해시, 작은↔큰 전환.
  - 문자열: 잘못된 바이트가 한 글자라는 규칙이 연산 전체에서 같다. `join(chars(s)) == s`. `split`·`replace`·`lines`가 조각 경계(128 KiB 창)에 걸친 구분자를 한 번씩 찾는다.
  - 대소문자 변환(`ß`, 끝 시그마, `İ`, `ǅ`).
  - 범위: 빈 범위, int 폭 범위의 `is_empty`·`last`·`take`, `len`의 E0503.
  - 조각 실행: `sort`·`unique`·`split`·`replace`·`join`·`reverse`가 2~3 ms 안에 기한을 본다.
- **앞단**
  - 렉서: `\u{…}` 서러게이트와 범위, `\xHH` 두 자리 강제, 보간 안의 중첩 문자열, BOM, NUL, CRLF.
  - 렉서: 정수 넘침과 `9223372036854775808` 특례, 진법 접두사 뒤 `_`, `1e400`, 기간의 단위 순서, 식별자 NFC.
  - 파서: 결합 방향, `x = y = 1`·`if x = 1:` 거절, `xs[a ?? 0..]`, `else` 줄 시작 이어짐, `match` 절 머리 한 번 읽기, 중첩 인덱스·`if` 8,000겹 0.03 s.
  - 이름 해석: 규칙 1~4, 클로저 복사, 최상위 상수(L9), 재대입, E0308, W0103·W0101.
  - 로더: E0302·E0307(같은 글자 경로), E0310, 다이아몬드 import 한 번 적재.
  - 포매터: 저장소의 `.fung` 전부에 무작위 변형 936개를 돌려 E0801 0건, 비멱등 0건.
  - 진단: 발췌의 탭·넓은 글자·BOM·CRLF. 카탈로그 E01xx~E03xx는 은퇴한 E0704·E0802 말고 모두 쓰인다.
- **컴파일러·VM**
  - `MOVELOCAL`의 정확성(갈래, `match` 주제·가드, 중첩 `match`, `return`, `?`·`!`, 파이프, 이름 인자, 보간, 인덱스·슬라이스, 클로저 붙잡기 순서, 기본값이 패턴 매개변수를 읽기)에서 빈 칸 읽기가 없었다.
  - `CLEARLOCAL`은 보수적이다(너무 이르게 비우는 경우가 없음).
  - 호출 규약 에러(E0402·E0405·E0406·E0407·E0228), 타입 검사 위치.
  - 꼬리 호출(자기·상호·`??` 오른쪽·반환 타입), 생략 수가 호출 경로에 정확히 나온다. E0804 깊이 한도, `map` 콜백 재귀 40,000단.
  - 상수 풀 해시 색인(NaN·-0.0), 모듈 본문 실패 뒤 REPL의 재import, 시작 시간(6 ms, `import http` 15 ms).
- **동시성**
  - `swap` 안의 `swap`·`reset` E0707, 조각과 `end_turn`이 `swap` 안에서 차례를 넘기지 않음.
  - 채널 `Line`의 떠남 표시, 짝 맞추기의 `claimed`, `try_*`의 짝짓기, 고리 검사 E0705·E0709(새 구멍 없음).
  - 닫히기 직전의 `spawn`, E0607의 세 경우, `Timeout::end`, `with`의 방패 노드, `Kind::Abandon`의 즉시 되감기, FIFO의 `read`·`write`·`open` 기한.
  - 외부 명령 수백 번 + 기한에 끊긴 명령 뒤 fd 누수·남은 자식이 없다.
  - `conc.pmap`이 `map`과 같은 결과를 낸다(20만 원소 0.2초).
- **표준 라이브러리**
  - json: int64 경계, `1e400`, 중복 키, 서러게이트, 앞의 0, `[1,]`, BOM, 제어 문자, 실수 왕복.
  - time: RFC 3339 경계, 서머타임 빈틈과 겹침, POSIX `TZ` 문자열, `unix`의 버림.
  - regex: 바이트 문자열의 글자 인덱스, 빈 매치·`split` 규칙, `$$`·`${1}`, 40만 매치 선형.
  - fs: 자기 자신 `copy`, 링크 처리, `perm`, 두 번 `close`, `walk`·`list`의 순서, 깊이가 PATH_MAX로 묶임.
  - http 서버: 중복 Host·Content-Length, 1.1의 `gzip, chunked`, 요청 줄 검사, 이른 431/413, `OPTIONS *`.
  - http 클라이언트: `Content-Length: +2` 거절, 사용자 헤더 검사, 리다이렉트 횟수, 비밀번호 가리기.
  - CLI: 종료 코드 0/1/2/3과 신호 코드, `-e` 뒤 인자 전달, 비UTF-8 인자.

## 12. 확인하지 못한 의심

- **명세**
  - `x |> (mk(10))`: 괄호로 감싼 호출에도 파이프 값이 첫 인자로 들어간다(`mk(1, 10)`). "오른쪽이 호출이면/이름이면" 두 규칙이 괄호식을 말하지 않아, `x |> (v => …)` 관용구와 헷갈린다. 문서 한 줄이 필요해 보이지만 동작을 바꿀지는 판단하지 않았다.
  - `first([])`·`max([])`·`env("X")`의 `nil`과 `repeat(s, -1) == ""`·`take(xs, -1) == []`는 문서에 적힌 선택이다. 원칙 1과의 관계를 설명하는 이유는 설계 문서에 없다.
  - regex의 `.`·문자 클래스는 UTF-8이 아닌 바이트를 한 글자로 보지 않는다(`regex.find_all("\xff\xfe", ".")`은 `[]`). 02의 "잘못된 바이트 하나가 한 글자"와 어긋나는지, 엔진의 유니코드 모드라는 의도인지 `regex.md`가 말하지 않는다.
  - 버려지는 자리의 부분 적용 문장 `print(x, _)`는 아무것도 하지 않고 조용히 끝난다. 함수 값을 버리는 문장 일반에 경고(W)를 둘지는 정하지 않았다.
- **앞단**
  - `desugar::fn_node`가 함수마다 본문 전체에 `names_in`을 돌려, 중첩 람다 n겹이 O(n²)다(4,000겹 0.14 s).
  - 기간 리터럴이 ns 아래로 반올림되어 0이 되는 경우(`0.0000004ms`)와 실수 아래 넘침(`1e-400`은 0.0)은 조용하다.
  - `loader.rs`가 모듈마다 surface·core 트리를 `clone`해 `Build`에 담는다(`dump`가 아니면 쓰지 않음). `import http`가 실행마다 더하는 약 9 ms 중 얼마가 복사인지는 재지 않았다.
  - 포매터가 `true`·`false`·`nil` 뒤의 `-`도 부호로 본다(`y = [false - 1]` → `y = [false -1]`). 토큰과 뜻은 같고 겉모양만 틀리다.
- **VM**
  - 명령당 약 5 ns가 경계 검사와 `Rc<RtProto>` 이중 간접에서 오는지는 LLVM 배치의 흔들림 때문에 따로 떼어 재지 못했다.
  - Step 내장 함수 → `issue` 경로에서 `tick`이 한 호출에 두 번 불린다. 예산이 두 배로 빨리 줄 뿐이지만, 차례 길이 측정에 영향이 있는지는 재지 않았다.
- **값**
  - `sort_by`가 원소 벡터를 한 번 더 복사한다(`xs: xs.clone()`). 큰 입력에서 메모리가 두 배일 수 있다.
  - 큰 맵(n=10000)의 덮어쓰기 `put`이 n=200000보다 5배 느렸다(6.8 µs 대 1.4 µs). 원인을 가리지 못했다.
  - `replace_fits`의 글자 수 추정은 바꾼 결과에서 UTF-8 경계가 붙거나 갈라질 때 몇 글자 다를 수 있다. 한도 판정에만 쓰인다.
- **동시성**
  - epoll 쪽(`poller/epoll.rs`)은 macOS에서 돌리지 못했다. 같은 번호의 새 fd가 `dup`된 옛 파일과 겹칠 때를 보지 못했다.
  - 신호 스레드의 `exit_now`가 실행 스레드의 출력 도중에 불릴 때(버퍼에 남은 표준 출력)를 보지 못했다.
  - kqueue `EVFILT_PROC`를 이미 끝난(좀비) 자식에 등록할 때의 결과를 보지 못했다.
- **표준 라이브러리**
  - https → http 리다이렉트의 자격 증명 유출(LB2)은 실제로 보지 못했다(TLS 서버를 띄우지 않음). EXDEV(LB8)도 돌리지 못했다.
  - 클라이언트가 HTTP/1.0 응답의 `Transfer-Encoding` + `Content-Length`를 CL로 읽는 것이, 프록시를 거친 경우에도 무해한지 따지지 않았다.
  - REPL 줄 편집기(raw 모드)는 가짜 터미널 없이 시험하지 못했다. `tls.rs`·`sock.rs`·`pipe.rs`는 훑어보기만 했다.
