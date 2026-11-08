---
description: test 블록, assert, dbg.
lede: <code>test</code> 블록, <code>assert</code>, <code>dbg</code>.
---
# 테스트와 디버깅

```fung run
fn half(n) = n // 2

test "half rounds down":
  assert half(5) == 2
  assert half(-5) == -3
;;

# [파일:줄] half(10) = 5 를 표준 에러로 찍고 값을 돌려준다
x = dbg(half(10)) + 1
print(x)
```

<ul>
  <li><code>test "이름": … ;;</code>는 최상위에만 둔다. 이름은 보간 없는 문자열이고, 한 파일 안에서 겹치거나 비면 E0233이다(보고가 실패한 시험을 이름으로 알린다). <code>fung run</code>은 무시하고 <code>fung test</code>가 실행한다. 블록 하나가 실패해도 다음 블록이 돈다. 다만 <code>exit(n)</code>이 불리거나(띄운 태스크에서도) 교착이 보고되면 그 파일의 나머지 블록은 건너뛰고, 건너뛰기를 실패 하나로 센다.</li>
  <li><code>assert 식</code>은 어디서나 쓴다. 비교가 실패하면 양쪽 값을 보여 준다(E0807).</li>
  <li><code>dbg(x)</code>는 식의 소스 텍스트와 값을 찍고 값을 그대로 돌려준다.</li>
</ul>

## 경계 사례와 에러 {#edges}

`assert`가 거짓이면 식을 보이며 멈춘다(`fung test` 밖에서도 같다).

```fung run exit=1 error=E0807
xs = [1, 2]
assert len(xs) == 2
assert len(xs) == 3
```

`test` 블록은 최상위에만 둔다.

```fung run exit=2 error=E0114
fn f():
  test "inner": assert true ;;
  1
;;
```
