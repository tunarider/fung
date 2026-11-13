---
description: fung의 prelude 함수와 표준 모듈(fs, path, cli, json, time, regex, http, conc) 참조.
lede: import 없이 쓰는 prelude와, 이름으로 가져오는 표준 모듈. 모든 서명은 런타임의 실제 매개변수 표와 맞춰 본다(테스트가 지킨다).
---
# 모든 함수

## 읽는 법 {#reading}

<ul>
  <li><code>f(a, b = 1, ...rest)</code>: <code>b</code>는 기본값이 있고 <code>rest</code>는 남는 위치 인자를 리스트로 받는다. 내장 함수도 이름 붙인 인자를 받는다: <code>exec(cmd, cwd: "/tmp")</code>.</li>
  <li>값을 다루는 함수는 그 값(컬렉션·문자열·맵)을 첫 번째 인자로 받아 파이프로 이어진다: <code>xs |&gt; filter(f) |&gt; map(g)</code>는 <code>map(filter(xs, f), g)</code>와 같다(<a href="language/operators.html#pipe">파이프</a>). 어디에 쓰거나 보내는 함수는 받는 곳이 첫 인자다: <code>fs.write(path, text)</code>·<code>fs.append</code>·<code>fs.write_line(file, line)</code>·<code>send(tx, x)</code>·<code>spawn(s, f)</code>·<code>http.request(method, url, …)</code>. 이들은 <code>text |&gt; fs.write(p)</code>로 잇지 않고 <code>fs.write(p, text)</code>로 쓴다.</li>
  <li>이름: 묻는 함수(술어)는 형용사·명사면 <code>is_</code>를 붙이고(<code>is_empty</code>, <code>is_ok</code>, <code>is_utf8</code>), 동사면 그대로 쓴다(<code>contains</code>, <code>starts_with</code>, <code>fs.exists</code>). 끝의 <code>_all</code>은 함수마다 뜻이 다르다: <code>env_all</code>은 전부, <code>regex.find_all</code>은 모든 매치, <code>fs.remove_all</code>은 디렉터리째(재귀).</li>
  <li>외부 요인으로 실패할 수 있는 함수는 <code>Ok(값)</code>/<code>Err({kind, message, …})</code>를 돌려준다. <code>kind</code>는 심볼이다: <code>'not_found</code> <code>'exists</code> <code>'permission</code> <code>'io</code> <code>'parse</code> <code>'invalid</code> <code>'timeout</code> <code>'closed</code> <code>'cancelled</code> <code>'error</code> <code>'exit</code> <code>'network</code> <code>'empty</code> <code>'full</code>(<code>'empty</code>·<code>'full</code>은 <a href="prelude/concurrency.html#fn-recv_now"><code>recv_now</code></a>·<a href="prelude/concurrency.html#fn-send_now"><code>send_now</code></a>). 뜻과 운영체제 에러의 표는 <a href="prelude/results.html#kinds">실패의 종류</a>에 있다.</li>
  <li>실행 중에 실패할 수 있는 함수는 결과값을 돌려준다. 넘겨받은 함수 <code>f</code>의 값을 내주는 함수(<code>scope</code>·<code>supervise</code>·<code>await</code>·<code>timeout</code>·<code>fs.with_open</code>·<code>fs.with_temp_dir</code>)도 성공하면 <code>Ok(f의 값)</code>이다(<a href="language/results.html">결과값</a>).</li>
  <li>인자 타입이 틀린 것은 코드 오류라 런타임 에러다: 매개변수 표에 적은 타입이 틀리면 E0403(<code>argument `n` must be int, got float</code>). 값을 훑거나 들여다보는 함수(<code>len</code>·<code>map</code>·<code>sum</code>·<code>get</code>·<code>put</code> 등)가 그럴 수 없는 값을 받으면 연산자처럼 그 일을 말하는 E0506(<code>cannot take the length of int</code>). 값을 다른 타입으로 바꾸는 함수(<code>int</code>·<code>float</code>·<code>sym</code> 등)가 바꿀 수 없는 것을 받으면 타입 탓이든 값 탓이든 E0513(<code>int(nil)</code>, <code>int(1e300)</code>).</li>
  <li><span class="by" style="margin-left:0">fung으로 작성</span> 표시는 fung 코드로 쓴 함수다. 나머지는 Rust로 구현했다(<a href="../vm/internals.html#layers">네 층</a>).</li>
</ul>

<p>import 없이 쓴다. 같은 이름의 변수·매개변수·함수를 만들면 컴파일 에러(E0240)다.</p>

## 목록 {#list}

{{functions}}
