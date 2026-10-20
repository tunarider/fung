---
lede: fung에는 <code>while</code>이 없다. "조건이 맞을 때까지 되풀이"는 꼬리 재귀나 이 페이지의 두 도우미로 쓴다. 둘 다 fung으로 썼다.
description: fung prelude 반복 패턴: until, iterate.
---
# 반복 패턴

## while 없이 되풀이하기 {#concepts}

**반복**(loop)은 같은 일을 여러 번 하는 것이다. fung에서 반복의 기본은 [`for`](../language.html#for)다. 리스트나 범위의 원소마다 몸을 돌고, 각 결과를 모은 리스트를 돌려준다. 정해진 묶음을 도는 일은 거의 다 `for`나 [`map`](collections.html#fn-map)·[`fold`](collections.html#fn-fold)로 된다.

몇 번 돌지 미리 모르는 반복도 있다. "재시도가 성공할 때까지", "값이 100을 넘을 때까지" 같은 것이다. 다른 언어는 이것을 `while` 반복문과 바뀌는 변수로 쓴다. fung의 값은 불변이고 블록 안에서 바깥 변수를 다시 대입할 수 없으므로(E0205) `while`이 없다. 대신 **재귀**(recursion), 곧 함수가 자기 자신을 다시 부르는 방법을 쓴다. 바뀌는 값은 변수가 아니라 다음 호출의 인자로 넘긴다.

```fung run
fn countdown(n):
  if n == 0: return "liftoff" ;;
  print(n)
  countdown(n - 1)
;;

print(countdown(3))
```

함수의 마지막 일이 자기 호출인 것을 **꼬리 재귀**(tail recursion)라고 한다. fung은 꼬리 호출이 스택을 쌓지 않게 컴파일하므로, 백만 번을 돌아도 스택이 넘치지 않는다. 그래서 꼬리 재귀가 곧 fung의 `while`이다.

`until`과 `iterate`는 가장 흔한 두 모양을 함수로 만들어 둔 것이다. 직접 재귀를 쓰는 것보다 짧고, 이름이 의도를 말해 준다. 실패할 수 있는 일을 정해진 횟수만큼 다시 해 보는 일은 [`conc.retry`](../modules/conc.html#fn-conc-retry)가 맡는다.

## 함수 {#functions}

:::fn until(x, f, pred) fung

`pred(x)`가 참이 될 때까지 `x = f(x)`를 되풀이하고, 그때의 `x`를 돌려준다.

#### 설명

먼저 `pred(x)`를 본다. 참이면 `x`를 그대로 돌려주고(`f`를 한 번도 부르지 않는다), 거짓이면 `f(x)`로 바꿔 다시 본다. 다른 언어의 `while not pred(x): x = f(x)`와 같다. 꼬리 재귀로 쓰여 있어 몇 번을 돌아도 스택이 넘치지 않는다.

여러 값을 함께 바꿔야 하면 `x`를 튜플이나 맵으로 묶는다: `until((0, 1), ((a, b)) => (b, a + b), ((a, _)) => a > 100)`. `pred`가 끝내 참이 되지 않으면 끝나지 않으므로 상한을 조건에 넣어 둔다.

#### 매개변수

- `x`: 처음 값.
- `f`: 값을 받아 다음 값을 돌려주는 함수.
- `pred`: 값을 받아 `bool`을 돌려주는 함수. 참이면 멈춘다.

#### 돌려주는 값

`pred`를 처음 만족한 값.

#### 에러

`pred`가 `bool`이 아닌 값을 돌려주면 런타임 에러 E0505.

#### 예제

**두 배씩 늘려 100을 넘을 때까지**

```fung run
print(until(1, x => x * 2, x => x > 100))
print(until(500, x => x * 2, x => x > 100))
```

두 번째는 처음부터 조건을 만족하므로 그대로다.

**여러 값을 함께: 피보나치**

```fung run
(a, b) = until((0, 1), ((a, b)) => (b, a + b), ((a, _)) => a > 1000)
print(a, b)
```

**물러서는 대기 시간(백오프)과 상한**

```fung run
delays = until([100ms], ds => ds + [last(ds) * 2], ds => last(ds) >= 3s or len(ds) >= 10)
print(delays)
```

#### 같이 보기

[`iterate`](#fn-iterate), [`fold`](collections.html#fn-fold), [`conc.retry`](../modules/conc.html#fn-conc-retry), [함수](../language.html#functions)

:::

:::fn iterate(x, f, n) fung

`f`를 `x`에 `n`번 거듭 적용한 값을 돌려준다.

#### 설명

`iterate(x, f, 3)`은 `f(f(f(x)))`다. `n`이 0 이하면 `x`를 그대로 돌려준다. 중간 값이 모두 필요하면 [`fold`](collections.html#fn-fold)로 리스트에 모으거나 `for`를 쓴다.

#### 매개변수

- `x`: 처음 값.
- `f`: 값을 받아 다음 값을 돌려주는 함수.
- `n`: 적용할 횟수(정수).

#### 돌려주는 값

`n`번 적용한 값.

#### 에러

`f` 안의 에러가 그대로 난다.

#### 예제

**거듭 적용**

```fung run
print(iterate(1, x => x * 2, 10), iterate("a", s => s + "!", 3), iterate(5, x => x + 1, 0))
```

**복리 계산**

```fung run
balance = iterate(1000.0, b => b * 1.05, 3)
print(int(balance))
```

#### 같이 보기

[`until`](#fn-until), [`fold`](collections.html#fn-fold)

:::
