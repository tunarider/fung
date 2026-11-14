---
description: x[i], x[a..b], x.name의 규칙과 범위 밖.
lede: <code>x[i]</code>, <code>x[a..b]</code>, <code>x.name</code>의 규칙과 범위 밖.
---
# 인덱스·슬라이스·필드

<ul>
  <li><code>xs[i]</code>: 0부터 센다. 음수를 포함해 범위를 벗어나면 에러다. 뒤에서 세는 인덱스는 없다: 마지막 원소는 <code>last(xs)</code>나 <code>xs[len(xs) - 1]</code>.</li>
  <li><code>xs[a..b]</code>, <code>xs[..b]</code>, <code>xs[a..]</code>, <code>xs[a..=b]</code>: 리스트와 문자열의 슬라이스. 두 끝도 0부터 센다. 범위를 벗어나거나(음수 끝도) 시작이 끝보다 뒤면 에러이고, <code>xs[a..=b]</code>는 정확히 <code>xs[a..b + 1]</code>이며(<code>xs[0..=-1]</code>은 빈 슬라이스, 빈 리스트의 <code>xs[..=0]</code>은 <code>xs[..1]</code>이라 에러), 마지막 하나를 뺀 앞부분은 <code>xs[..len(xs) - 1]</code>이다, 리스트에서 있는 만큼만 원하면 <code>take</code>·<code>drop</code>을 쓴다(컬렉션 함수라 문자열은 받지 않는다). 열린 끝은 비워 쓴다: 끝에 <code>nil</code>을 주면 에러다.</li>
  <li>슬라이스는 <code>[</code> 바로 안에 쓴 <code>..</code>·<code>..=</code>다. 두 끝은 다른 자리의 범위와 같이 <code>..</code>의 피연산자(<code>+</code>·<code>-</code>와 그보다 강한 연산자)다. 그보다 약한 연산자(<code>??</code>, <code>|&gt;</code>, 비교, <code>in</code>, <code>not</code>, <code>and</code>·<code>or</code>, <code>then … else</code>)가 끝의 맨 위에 오면 E0125다: 다른 자리에서는 <code>..</code>가 먼저 묶이므로(<code>a ?? 0..3</code>은 <code>a ?? (0..3)</code>) 뜻이 말없이 바뀌지 않게 괄호로 쓴다(<code>xs[(i ?? 0)..]</code>, <code>xs[(c then 0 else 1)..]</code>). 괄호 친 범위나 <code>if</code> 조건·<code>match</code> 대상·<code>for</code>의 순회 대상·<code>then</code> 가지 안의 범위는 슬라이스가 아니라 범위 값이고, 그것 하나로 하는 인덱스다(<code>xs[(0..2)]</code>).</li>
  <li><code>m.name</code>은 <code>m["name"]</code>과 같다. 모듈의 <code>fs.read</code>도 같은 규칙이다.</li>
  <li>튜플은 인덱스로 꺼내지 않는다. 패턴으로 꺼낸다: <code>(a, b) = pair</code>.</li>
</ul>

```fung run
xs = [10, 20, 30, 40]
print(xs[len(xs) - 1], xs[1..3], xs[..2], "참치김밥"[1..3])
```

## 경계 사례와 에러 {#edges}

인덱스는 범위 밖이면 에러다. 느슨하게 자르려면 `take`·`drop`.

```fung run exit=1 error=E0507
xs = [10, 20, 30]
print(last(xs), take(xs, 5), drop(xs, 1))
print(xs[3])
```

음수 인덱스와 음수 끝도 범위 밖이다. 뒤에서 세는 규칙이 있으면 `i - 1`처럼 하나 모자란 계산이 말없이 맨 끝 원소를 주기 때문이다.

```fung run exit=1 error=E0507
s = "1024K"
print(s[..len(s) - 1], s[len(s) - 1..])
print(s[..-1])
```

슬라이스의 시작이 끝보다 뒤면 범위 밖이 아니라 따로 에러다.

```fung run exit=1 error=E0516
xs = [1, 2, 3]
print(xs[1..])
print(xs[2..1])
```

끝을 포함하는 슬라이스 `xs[a..=b]`는 정확히 `xs[a..b + 1]`이다. 값도 에러도 같다. 그래서 `n`개를 앞에서 자르는 `xs[0..=n - 1]`은 `n`이 0이면 빈 슬라이스다.

```fung run exit=1 error=E0507
xs = [1, 2, 3]
print(xs[0..=-1], xs[1..=0], xs[..=2])
print(xs[..=3])
```

범위 값으로도 색인한다. `xs[r]`은 같은 끝을 쓴 슬라이스 `xs[a..b]`와 같고, 에러도 같다. 범위 값은 만들 때 먼저 검사하므로 시작이 끝보다 뒤인 `(2..1)`은 색인하기 전에 E0522다.

```fung run
xs = [1, 2, 3]
r = 1..3
print(xs[r], "fung"[(0..=1)])
```

슬라이스의 끝에 `??`를 쓰려면 괄호로 감싼다. 다른 자리에서 `a ?? 0..2`는 `a ?? (0..2)`이기 때문이다.

```fung run exit=2 error=E0125
xs = [1, 2, 3]
start = nil
print(xs[(start ?? 0)..2])
print(xs[start ?? 0..2])
```

없는 키의 필드는 에러다. 기본값이 필요하면 `get`.

```fung run exit=1 error=E0508
m = {name: "a"}
print(get(m, "age", 0))
print(m.age)
```
