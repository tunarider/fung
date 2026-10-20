---
description: fung — Go처럼 쓰고 Lisp처럼 도는 작은 함수형 스크립트 언어. 기조, 문법, 내장 함수, 내부 구조.
lede: Go처럼 쓰고 Lisp처럼 도는 작은 함수형 스크립트 언어. 파일 이름 바꾸기, 로그 요약, 작은 웹서버 같은 일상의 스크립트를 위해 만들었다.
---
# fung

{{parts}}

</div>
<div class="wide">
<div class="twofold">
  <figure>
    <figcaption><b>쓰는 모양</b>괄호는 적게, 블록은 <code>:</code>로 열고 <code>;;</code>로 닫는다</figcaption>

```fung check
import fs

fn is_log(name) = ends_with(name, ".log")

logs = fs.list("/var/log")! |> filter(is_log)
match logs:
  []          => print("no logs")
  [name, ...] => print(len(logs), "logs, first:", name)
;;
```

  </figure>
  <figure>
    <figcaption><b>도는 모양</b><code>fung dump core</code>가 보여 주는, 컴파일러가 실제로 받는 트리</figcaption>

```core
(do
  (import "fs" (global 0))
  (let (global 1)
    (fn is_log(name)
      (do (call (prelude ends_with) (local 0) ".log"))))
  (let (global 2)
    (call (prelude filter)
      (unwrap !
        (call (field list (global 0)) "/var/log"))
      (global 1)))
  (do
    (let (local 0) (global 2))
    (match (local 0)
      (arm (plist) (nil)
        (do (call (prelude print) "no logs")))
      (arm (plist (pbind (local 1)) (prest)) (nil)
        (do (call (prelude print)
              (call (prelude len) (global 2))
              "logs, first:" (local 1)))))))
```

  </figure>
</div>
<p class="note">파이프 <code>|&gt;</code>는 평범한 호출로, <code>!</code>는 <code>unwrap</code>으로, 이름은 전부 슬롯 번호(<code>local</code>·<code>global</code>·<code>prelude</code>)로 풀렸다. 이것이 "겉은 Go, 속은 Lisp"의 실체다.</p>
</div>
<div class="col">

## 무엇을 위한 언어인가 {#what}

<p>fung은 개인 학습과 일상의 소소한 사용을 위한 언어다. 범용 언어를 목표로 하지 않는다.</p>
<ul>
  <li><strong>쓰는 곳</strong>: 쉘 스크립트로 짜던 작은 작업(파일 일괄 정리, 로그 집계, 설정 검사), 여러 URL을 병렬로 확인하는 도구, 간단한 JSON API 서버.</li>
  <li><strong>안 쓰는 곳</strong>: 본격적인 쉘 작업(그건 계속 쉘로), 성능이 중요한 처리, GUI.</li>
  <li><strong>우선순위</strong>: 성능보다 쓰기 편함. 다만 조용히 틀리는 동작은 편함보다 위험하다고 보고 막는다.</li>
</ul>
<p>Rust로 구현한 바이트코드 VM이고, 실행 파일 <code>fung</code> 하나로 배포된다. 확장자는 <code>.fung</code>. 이름은 한국어 "펑"(빠르게 만들고 치고 빠진다)에서 왔다. 문법의 겉모양은 Go·Python을, 내부 구조는 Lisp를, 태스크와 에이전트는 Erlang·Elixir를 닮았다.</p>

## 기조 {#principles}

<p>설계 중 새 결정이 필요할 때마다 아래 일곱 가지로 판단했다. 문법과 라이브러리의 모양은 대부분 여기서 따라 나온다.</p>
<h3>엄격함이 우선이다</h3>
<p>조용히 넘어가는 동작은 에러다. 암묵적 형변환(<code>"5" + 3</code>), 참·거짓 흉내(<code>if []:</code>), 정수 오버플로의 감김, 없는 키를 <code>nil</code>로 읽기, <code>NaN</code>·<code>Inf</code>, 아무도 보지 않고 버려지는 <code>Err</code>는 모두 에러다. PHP의 "문법 에러만 아니면 일단 돈다"는 편안함은 부작용 때문에 버렸다.</p>
<h3>짧은 축약 문법을 좋아한다</h3>
<p>자주 쓰는 일에는 짧은 표기가 있다: 에러 처리 <code>!</code> <code>?</code> <code>??</code>, 한 줄 조건 <code>c then a else b</code>, 짧은 함수 <code>fn f(x) = …</code>, 람다 <code>x =&gt; …</code>, 파이프 <code>|&gt;</code>, 맵 축약 <code>{name, port}</code>.</p>
<h3>들여쓰기를 강제하지 않는다</h3>
<p>블록은 <code>:</code>로 열고 <code>;;</code>로 닫는다. 들여쓰기는 의미가 없어서 무엇이든 한 줄로도 쓸 수 있다: <code>if ok: a() else: b() ;;</code>. 모양은 포맷터 <code>fung fmt</code>가 맞춘다.</p>
<h3>구현 언어는 안에서 쓰고 밖으로 드러내지 않는다</h3>
<p>메모리 회수, 스레드, 표준 라이브러리는 구현 언어(Rust)에 기댄다. 하지만 문법·용어·함수 이름은 fung 고유의 것이고, 구현 언어의 동작(맵 순회 순서, 정수 감김, 패닉과 스택 추적)은 새어 나오지 않는다. 사용자는 Rust 도구 없이 <code>fung</code> 하나만 있으면 된다.</p>
<h3>핵심은 작게, 나머지는 fung으로</h3>
<p>언어 핵심은 최소한의 값 연산과 원시 연산만 가진다. <code>take</code>·<code>sum</code>·<code>conc.pmap</code>·<code>cli.parse</code>, 그리고 HTTP 프로토콜 같은 것은 fung으로 작성해 실행 파일에 넣었다. Rust로 쓰는 것은 바깥세상과의 다리(파일·소켓·TLS·프로세스)와 성능이 필요한 안쪽 루프(JSON 파싱, 정규식)뿐이다. prelude에는 확실한 것만 두고, 나머지는 <code>import</code>하는 모듈로 둔다.</p>
<h3>같은 일을 하는 방법은 하나다</h3>
<p>주석은 <code>#</code>만, 문자열은 보간이 되는 <code>"…"</code>와 원시 <code>`…`</code>로 역할이 나뉘고, 맵 키는 항상 문자열이다. 반복은 <code>map</code>·<code>filter</code>·<code>fold</code>와 꼬리 재귀뿐이고 <code>while</code>은 없다.</p>
<h3>예외 조항을 줄인다</h3>
<p>"이때는 이렇고 저때는 저렇다"는 규칙이 적어야 한다. 편의를 위한 특례를 더하기보다 규칙 하나에서 결과가 따라 나오게 한다. 예를 들어 클로저 규칙은 "함수는 만들어질 때 보이는 변수의 값을 복사한다" 하나뿐이고, 최상위 함수든 람다든 같다.</p>

## 핵심 개념 {#core-ideas}

<dl class="terms">
  <dt>값은 불변이다</dt>
  <dd>리스트·맵을 바꾸는 연산은 없다. <code>put(m, "k", v)</code>, <code>xs + [x]</code>처럼 새 값을 만든다. 그래서 태스크 사이에 값을 그냥 넘겨도 안전하다. 태스크 사이에 나눌 바뀌는 상태는 <code>atom</code> 하나가 가리키고, <code>swap</code>이 순수한 함수로 비교 후 교체한다.</dd>
  <dt>값을 만드는 구문은 모두 식이다</dt>
  <dd><code>if</code>, <code>match</code>, <code>for</code>, 블록이 값을 돌려준다. 문장은 대입과 선언(<code>fn</code>, <code>import</code>, <code>type</code>, <code>test</code>)뿐이다.</dd>
  <dt>실패는 값이거나 중단이다</dt>
  <dd>외부 요인으로 실패할 수 있는 일(파일, 네트워크, 파싱)은 <code>Ok(v)</code>/<code>Err(e)</code>를 돌려준다. 코드가 틀린 것(타입 오류, 범위 밖 인덱스)은 런타임 에러로 그 자리에서 멈춘다. 에러를 잡는 <code>try</code>는 없다.</dd>
  <dt>반복은 파이프와 꼬리 재귀다</dt>
  <dd>목록 처리는 <code>xs |&gt; filter(f) |&gt; map(g)</code>. 상태가 있는 반복(재시도, 폴링, 누적)은 꼬리 재귀로 쓰고, VM이 프레임을 재사용하므로 백만 번 돌아도 스택이 넘치지 않는다.</dd>
  <dt>동적 타입, 선택적 표기</dt>
  <dd><code>fn area(r: num) -&gt; float</code>처럼 타입을 적으면 호출과 반환 때 검사한다. 적지 않으면 검사하지 않는다.</dd>
  <dt>동시성은 원시 연산 몇 개</dt>
  <dd>범위(<code>scope</code>/<code>supervise</code>) 안의 <code>spawn</code>/<code>await</code>/<code>cancel</code>, 채널, <code>atom</code>, <code>sleep</code>. async/await 문법은 없고, 막히는 호출은 그냥 막히며 그동안 다른 태스크가 돈다.</dd>
</dl>

## 빠른 시작 {#start}

<h3>설치</h3>
<p>Linux와 macOS(amd64, arm64)는 설치 스크립트로 받는다. GitHub 릴리스에서 내 플랫폼의 바이너리를 받아 <code>SHA256SUMS</code>와 대조한 뒤 <code>~/.local/bin</code>에 둔다. sudo는 필요 없다. Linux 바이너리는 정적 링크(musl)라 배포판·glibc 버전과 상관없이 돈다.</p>

```plain
curl -fsSL https://raw.githubusercontent.com/tunarider/fung/main/install.sh | sh
fung version
```

<p>버전과 위치는 환경 변수로 고른다: <code>FUNG_VERSION=0.3.0</code>, <code>FUNG_INSTALL_DIR=/usr/local/bin</code>. 다시 실행하면 업데이트, 지우려면 그 파일을 지우면 된다. 바이너리 하나에 표준 모듈이 다 들어 있어 따로 설치할 것이 없다.</p>
<p>Rust 1.94 이상이 있다면 저장소에서 직접 빌드할 수도 있다. 실행 파일은 <code>target/release/fung</code>이다.</p>

```plain
git clone https://github.com/tunarider/fung && cd fung
cargo build --release --locked
cp target/release/fung ~/.local/bin/
```

<p><code>cargo install</code>로 바로 설치할 수도 있다(<code>~/.cargo/bin</code>에 들어간다). 이렇게 빌드한 바이너리는 <code>fung version</code>이 <code>0.0.0-dev</code>다.</p>

```plain
cargo install --locked --git https://github.com/tunarider/fung --tag v0.3.0 fung-cli
```

<p class="note">받은 파일이 이 저장소의 GitHub Actions에서 빌드된 것인지는 <code>gh attestation verify &lt;파일&gt; --repo tunarider/fung</code>로 확인할 수 있다.</p>
<h3>첫 스크립트</h3>
<p>디렉터리의 파일을 크기순으로 보여 주는 스크립트다. 파일 첫 줄에 shebang을 두면 직접 실행할 수 있다.</p>

```fung run args=". 3"
#!/usr/bin/env fung
# usage: fung biggest.fung <dir> [n]
import fs
import path

dir = first(args) ?? fail("usage: biggest.fung <dir> [n]")
n = int(get_arg(1, "5"))

fn get_arg(i, default) = i < len(args) then args[i] else default

sizes = fs.list(dir)!
  |> map(f => (f, fs.stat(path.join(dir, f))!.size))
  |> sort_by(((_, size)) => -size)
  |> take(n)

for (name, size) in sizes:
  print(pad_left(str(size), 10), name)
;;
```

<p class="note">최상위 <code>fn</code>은 프로그램 시작 전에 만들어지므로 정의보다 앞에서 불러도 된다. 최상위에서 한 번만 대입되는 이름(상수)은 부를 때 읽고, 두 번 이상 대입되는 변수는 볼 수 없어서 인자로 넘긴다(<a href="reference/language.html#scope">스코프</a>).</p>
<h3>테스트를 같은 파일에</h3>

```fung run
fn slug(s) = lower(s) |> split(" ") |> filter(w => w != "") |> join("-")

test "slug joins words with dashes":
  assert slug("Hello  World") == "hello-world"
  assert slug("") == ""
;;
```

<p><code>fung run</code>은 <code>test</code> 블록을 무시하고, <code>fung test</code>가 그것만 돌린다. 실패하면 양쪽 값이 함께 찍힌다.</p>

## 명령 {#commands}

<table>
  <thead><tr><th>명령</th><th>하는 일</th></tr></thead>
  <tbody>
    <tr><td><code>fung script.fung [args…]</code></td><td>스크립트 실행. <code>fung run script.fung</code>과 같다. 뒤의 인자는 <code>args</code>(문자열 리스트)로 들어간다.</td></tr>
    <tr><td><code>fung -e 'code'</code></td><td>코드 한 줄 실행.</td></tr>
    <tr><td><code>fung</code></td><td>REPL. 줄 편집, 기록(<code>~/.fung_history</code>), 정의 유지, <code>:help</code> <code>:load</code> <code>:quit</code>. 에러는 그 코드를 정의한 입력의 줄을 보인다. 한 입력 안의 교착은 그 입력만 실패시키고 세션은 계속된다. 띄운 태스크가 <code>exit(n)</code>을 부르면 세션이 코드 n으로 끝난다. 다음 입력을 기다리는 태스크는 교착이 아니다. 전역에 담은 에이전트는 다음 입력에서도 쓸 수 있고 세션이 끝날 때 끝나며, 아무도 붙잡지 않은 에이전트는 핸들이 사라질 때(대개 그 입력 끝에) 더 일찍 끝난다.</td></tr>
    <tr><td><code>fung check script.fung</code></td><td>실행하지 않고 문법·이름·import를 검사하고 경고를 보여 준다.</td></tr>
    <tr><td><code>fung test [path]</code></td><td><code>test</code> 블록 실행. 경로가 디렉터리면 그 아래 <code>.fung</code> 전부(<code>_</code>나 <code>.</code>으로 시작하는 디렉터리 제외, <code>fmt</code>와 같은 규칙). 테스트가 없는 파일은 건너뛴다. 읽지 못한 파일과 종료 핸들러의 실패도 실패로 센다. 파일 안에서 <code>exit(n)</code>이 불리거나(띄운 태스크에서도) 교착이 보고되면 그 파일의 나머지 블록을 건너뛰고 실패 하나로 센다.</td></tr>
    <tr><td><code>fung fmt [--check] path…</code></td><td>공백·들여쓰기·정렬을 단일 스타일로 고친다. 줄바꿈은 그대로 둔다. 정렬은 화면 칸 수로 맞춘다(한글·한자는 2칸). <code>--check</code>는 고칠 파일만 알려 주고, <code>-</code>는 표준 입력을 받는다. <code>--check -</code>는 고칠 게 있으면 <code>&lt;stdin&gt;</code>을 찍고 종료 코드 1, 없으면 아무것도 찍지 않고 0.</td></tr>
    <tr><td><code>fung dump ast|core|bytecode file</code></td><td>컴파일의 중간 단계를 보여 준다(<a href="vm/internals.html">내부 구조</a>).</td></tr>
  </tbody>
</table>

## 에러와 종료 코드 {#errors}

<p>에러는 번호, 메시지, 소스 발췌, 런타임 에러면 호출 경로를 보여 준다. 구현 언어의 패닉이나 스택 추적은 나오지 않는다.</p>

```plain
error[E0507]: index 5 is out of range (length 2)
  --> report.fung:2:8
  |
2 | print(x[5])
  |        ^
  |
 call path (innermost first):
   report.fung:2 <entry>
```

<table>
  <thead><tr><th>번호</th><th>분류</th></tr></thead>
  <tbody>
    <tr><td><code>E01xx</code></td><td>문법(렉서·파서)</td></tr>
    <tr><td><code>E02xx</code></td><td>이름과 스코프</td></tr>
    <tr><td><code>E03xx</code></td><td>모듈과 import</td></tr>
    <tr><td><code>E04xx</code></td><td>호출과 타입 표기</td></tr>
    <tr><td><code>E05xx</code></td><td>런타임 연산(타입 불일치, 0 나누기, 오버플로, 범위 밖 인덱스, 없는 키)</td></tr>
    <tr><td><code>E06xx</code></td><td>패턴과 결과값(맞는 패턴 없음, <code>!</code>가 <code>Err</code>를 만남, 버려진 <code>Err</code> E0606)</td></tr>
    <tr><td><code>E07xx</code></td><td>동시성(취소, 교착, 채널에 그 채널의 끝 보내기 E0705, 닫힌 범위 E0706, atom의 <code>f</code> 안 효과 E0707, atom에 못 담는 값 E0708)</td></tr>
    <tr><td><code>E08xx</code></td><td>한도와 환경(스택 깊이, <code>fail</code>, <code>assert</code>, 내부 오류)</td></tr>
    <tr><td><code>W01xx</code></td><td>경고(<code>check</code>·<code>test</code>에서만): 안 쓰는 import·변수, prelude 이름 가리기</td></tr>
  </tbody>
</table>
<table>
  <thead><tr><th>종료 코드</th><th>뜻</th></tr></thead>
  <tbody>
    <tr><td><code>0</code></td><td>정상 종료</td></tr>
    <tr><td><code>1</code></td><td>실행 중 실패: 런타임 에러, <code>!</code>가 <code>Err</code>를 만남, 끝내 아무도 확인하지 않은 태스크의 실패, 테스트 실패</td></tr>
    <tr><td><code>2</code></td><td>컴파일 실패: 문법·이름·import 에러</td></tr>
    <tr><td><code>3</code></td><td><code>fung</code> 명령을 잘못 씀</td></tr>
    <tr><td><code>n</code></td><td>스크립트가 <code>exit(n)</code>을 부름</td></tr>
  </tbody>
</table>
