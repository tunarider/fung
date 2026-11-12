---
description: for의 값, 순회 대상, 버려지는 자리.
lede: <code>for</code>의 값, 순회 대상, 버려지는 자리.
---
# for

<p><code>for</code>는 <code>map</code>의 설탕이고 결과로 리스트를 돌려준다. 값을 버리는 자리의 <code>for</code>는 리스트를 만들지 않고 <code>each</code>처럼 돌며, 본문이 낸 <code>Err</code>는 E0606이다. 순회 대상은 리스트, 범위, 채널의 받는 끝(값이 올 때마다, 닫히면 끝)이다. 문자열과 맵은 <code>chars(s)</code>, <code>pairs(m)</code>으로 리스트로 바꿔서 돈다.</p>

```fung run
squares = for x in 1..=3: x * x ;;
for (k, v) in pairs({a: 1, b: 2}):
  print("${k} = ${v}")
;;
print(squares)
```

<p>본문이 사실은 람다이므로 <code>for</code> 본문 안의 <code>return</code>과 <code>?</code>는 람다 안에서처럼 컴파일 에러 E0237이다(<a href="functions.html">함수</a>의 "람다의 본문"). 실패할 수 있는 변환은 첫 <code>Err</code>에서 멈추는 <code>try_map</code>, 찾기는 <code>find</code>, 검사는 <code>all</code>·<code>any</code>, 누적은 <code>fold</code>를 쓴다. <code>while</code>은 없다.</p>
<div class="aside">
<p><strong>누적과 재귀의 비용.</strong> 리스트와 맵은 불변이지만 사본이 원본과 구조를 공유하므로, <code>fold</code>로 <code>acc + [x]</code>·<code>[x] + acc</code>나 <code>put(acc, k, v)</code>를 쌓는 것과 <code>[x, ...rest]</code>·<code>xs[1..]</code>로 꼬리를 떼는 재귀는 모두 선형이다(2만 개에 수십 ms). 두 리스트를 이으면 짧은 쪽을 긴 쪽에 붙이므로 비용은 짧은 쪽의 길이에 비례한다. 피할 것은 문자열을 <code>acc + s</code>로 쌓는 것이다. 리스트에 모아 <code>join</code>한다. 자세한 것은 <a href="../../vm/internals.html#collections">컬렉션의 내부</a>.</p>
</div>

## 경계 사례와 에러 {#edges}

`for`는 리스트를 만든다. 그래서 몸통에서 `return`으로 빠져나갈 수 없다(`find`·`any`·`fold`를 쓴다).

```fung run exit=2 error=E0237
fn first_big(xs):
  for x in xs:
    if x > 2: return x else: nil ;;
  ;;
;;
print(first_big([1, 3]))
```

순회 대상은 리스트·범위·받는 끝이다. 맵은 `pairs`, 문자열은 `chars`로 먼저 바꾼다.

```fung run exit=1 error=E0506
m = {a: 1}
for (k, v) in pairs(m): print(k, v) ;;
for ch in "ab": print(ch) ;;
```
