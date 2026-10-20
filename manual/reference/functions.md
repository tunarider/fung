---
description: fung의 prelude 함수와 표준 모듈(fs, path, cli, json, time, regex, http, conc) 참조.
lede: import 없이 쓰는 prelude와, 이름으로 가져오는 표준 모듈. 모든 서명은 런타임의 실제 매개변수 표와 맞춰 본다(테스트가 지킨다).
---
# 모든 함수

## 읽는 법 {#reading}

<ul>
  <li><code>f(a, b = 1, ...rest)</code>: <code>b</code>는 기본값이 있고 <code>rest</code>는 남는 위치 인자를 리스트로 받는다. 내장 함수도 이름 붙인 인자를 받는다: <code>exec(cmd, cwd: "/tmp")</code>.</li>
  <li>모든 함수는 "대상이 첫 번째 인자"라서 파이프로 이어진다: <code>xs |&gt; filter(f) |&gt; map(g)</code>는 <code>map(filter(xs, f), g)</code>와 같다(<a href="language.html#pipe">파이프</a>).</li>
  <li>외부 요인으로 실패할 수 있는 함수는 <code>Ok(값)</code>/<code>Err({kind, message, …})</code>를 돌려준다. <code>kind</code>는 심볼이다: <code>'not_found</code> <code>'exists</code> <code>'permission</code> <code>'io</code> <code>'parse</code> <code>'invalid</code> <code>'timeout</code> <code>'closed</code> <code>'cancelled</code> <code>'error</code> <code>'exit</code> <code>'network</code>.</li>
  <li>인자 타입이 틀린 것은 코드 오류라 런타임 에러(E0403)다.</li>
  <li><span class="by" style="margin-left:0">fung으로 작성</span> 표시는 fung 코드로 쓴 함수다. 나머지는 Rust로 구현했다(<a href="../vm/internals.html#layers">네 층</a>).</li>
</ul>

<p>import 없이 쓴다. 같은 이름의 변수나 함수를 만들면 경고(W0103)가 난다.</p>

## 목록 {#list}

{{functions}}
