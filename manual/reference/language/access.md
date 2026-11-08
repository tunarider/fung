---
description: x[i], x[a..b], x.name의 규칙과 범위 밖.
lede: <code>x[i]</code>, <code>x[a..b]</code>, <code>x.name</code>의 규칙과 범위 밖.
---
# 인덱스·슬라이스·필드

<ul>
  <li><code>xs[i]</code>: 음수는 뒤에서부터(<code>xs[-1]</code>). 범위를 벗어나면 에러.</li>
  <li><code>xs[a..b]</code>, <code>xs[..b]</code>, <code>xs[a..]</code>, <code>xs[a..=b]</code>: 리스트와 문자열의 슬라이스. 범위를 벗어나거나 시작이 끝보다 뒤면 에러이고, 있는 만큼만 원하면 <code>take</code>·<code>drop</code>을 쓴다(리스트와 문자열 모두). 열린 끝은 비워 쓴다: 끝에 <code>nil</code>을 주면 에러다.</li>
  <li><code>m.name</code>은 <code>m["name"]</code>과 같다. 모듈의 <code>fs.read</code>도 같은 규칙이다.</li>
  <li>튜플은 인덱스로 꺼내지 않는다. 패턴으로 꺼낸다: <code>(a, b) = pair</code>.</li>
</ul>

```fung run
xs = [10, 20, 30, 40]
print(xs[-1], xs[1..3], xs[..2], "참치김밥"[1..3])
```

## 경계 사례와 에러 {#edges}

인덱스는 범위 밖이면 에러다. 음수는 뒤에서 센다. 느슨하게 자르려면 `take`·`drop`.

```fung run exit=1 error=E0507
xs = [10, 20, 30]
print(xs[-1], take(xs, 5), drop(xs, 1))
print(xs[3])
```

슬라이스의 시작이 끝보다 뒤면 범위 밖이 아니라 따로 에러다.

```fung run exit=1 error=E0516
xs = [1, 2, 3]
print(xs[1..])
print(xs[2..1])
```

없는 키의 필드는 에러다. 기본값이 필요하면 `get`.

```fung run exit=1 error=E0508
m = {name: "a"}
print(get(m, "age", 0))
print(m.age)
```
