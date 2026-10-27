# L2. 함수 본문 끝의 `if`·`for`

상태: **닫힘 (2026-10-27)**. 정할 것 1~4 모두 추천대로(사용자 결정 2026-10-27). 목록: [README](README.md).

결과: 함수 본문의 끝은 블록 본문이든 아니든 값을 쓰는 자리다(`resolve/value.rs`, `FnInfo::expr_body`는 쓰는 곳이 없어 지움). `test` 블록의 본문은 예외로 버려지는 자리이고,
`fung test`가 그 값에도 버림 검사를 한다(할 일에 없던 것: 테스트가 마지막 줄의 실패를 놓치던 같은 구멍). 버려지는 리스트는 한 겹 훑는다(`vm/m4.rs` `discarded_err`·`may_hold_err`).
걸린 곳은 일부러 에러를 내는 시험을 빼고 8곳이었다(`each`에 넘긴 람다 3, 함수 끝의 `if` 3, `_ = …`로 끝나는 release 2). `each`의 람다는 `for` 문장으로, 나머지는 `else: nil`·끝의 `nil`로 고쳤다.
리스트 훑기는 처음에 매번 원소를 다 봐서, `perf`의 "a queue in an atom"(10만 원소 리스트를 돌려주는 `swap` 문장 2,000번)이 0.3초 한도를 넘었다. 그래서 영속 벡터 노드의 기억(`pvec.rs`)을
"변하는 값을 품었나"와 "`Err`를 품었나" 두 질문(`Probe`)으로 넓혔다. 같은 리스트의 조각을 거듭 버리는 경우는 `perf`의 새 사례가 지킨다.
`bench/`의 계산 위주 측정(fib·loop·list·map·str, 7번 중앙값)은 기준판과 −1.5%~+1.9%로 잡음 범위 안이다.

## 문제

두 가지가 같은 뿌리다. 블록 본문 함수(`fn f(): … ;;`)의 마지막 항목은 반환값인데, 값을 쓰는 자리의 규칙(01 "식과 문장", L3·L1)은 그것을 반쯤만 본다.

**끝의 `if`.** 블록 본문 함수의 마지막 `if`는 값 자리가 아니라서 `else`가 없어도 E0223이 아니다. 그 함수를 값 자리에서 부르면 조용히 `nil`이다.

```
$ fung -e 'fn f(x):
  if x: 1 ;;
;;
y = f(false)
print(y)'
nil                                    # 종료 코드 0
```

**끝의 `for`.** 함수의 마지막 `for`는 값 자리라 `map`이 되어 리스트를 만든다. 호출한 쪽이 그 리스트를 버리면 안의 `Err`는 보이지 않는다. 같은 `for`를 문장으로 쓰면 E0606이다.

```
$ fung -e 'fn main():
  (tx, rx) = channel(1)
  close(tx)
  for f in [1, 2]: send(tx, f) ;;
;;
main()
print("after")'
after                                  # 종료 코드 0. main()의 값은
                                       # [Err({kind: 'closed, …}), Err({kind: 'closed, …})]

$ fung -e '(tx, rx) = channel(1)
close(tx)
for f in [1, 2]: send(tx, f) ;;'
error[E0606]: discarded Err: channel closed
  --> -e:3:1                           # 종료 코드 1
```

같은 구멍이 함수 밖에도 있다. `map([1, 2], x => Err(x))`를 문장으로 쓰면 아무 일도 없다(확인함). 02 "버려지는 Err"는 이것을 "잡지 않는 것"(함수가 돌려준 리스트 안의 `Err`)으로 적었다.
둘 다 원칙 1(조용히 넘어가는 동작은 에러)에 어긋난다. `main()`을 부르는 스크립트의 마지막 줄이 `for`로 끝나는 것은 셸 대용 스크립트에서 가장 흔한 모양이다. open-questions "남은 일"의 두 항목이다(끝의 `if`는 사용자 결정 2026-10-20으로 뺐다, 끝의 `for`는 L1 뒤의 한계).

## 지금 동작과 원인

- 값 자리 판정: `crates/fung-front/src/resolve/value.rs`의 `check`·`function`·`fn_body_used`. 블록 본문 함수는 `FnInfo::expr_body`가 거짓이라 본문의 마지막 항목을 `used = false`로 본다. 그래서 그 자리의 `if`는 E0223이 아니다(모듈 머리 주석: "a side-effect function often ends in an if").
- `for` 낮추기: `crates/fung-front/src/desugar.rs`의 `for_loop`. 버려지는 자리(`discard`)면 `each`, 아니면 `map`이다. 함수 본문은 버려지는 자리가 아니라서(01: "함수 본문의 마지막 식은 반환값이라 버려지지 않는다") 끝의 `for`는 `map`이다.
- 런타임의 버림 검사: `crates/fung-vm/src/vm.rs`의 `Op::Pop`이 `is_err`(태그 검사 하나)만 본다. 콜백 쪽은 `vm/m4.rs`의 `discarded`·`discarded_err`, `each`는 `prelude/collections.rs`. 리스트는 `Value::List`라 검사를 지나간다.
- 반환 타입 `-> nil`을 붙여도 끝의 `if`는 그대로 받아들여진다(`fn f(x) -> nil: if x: print(1) ;; ;;`이 `nil`, 확인함).

## 고려할 점

- 01은 이미 "함수 본문의 마지막 식은 반환값"이라고 말한다. 반환값은 호출한 쪽이 쓸 수 있는 값이므로, 정의하는 쪽에서 보면 값 자리다. 지금 규칙은 "부수효과 함수가 `if`로 끝나는 것이 흔하다"는 편의로 예외를 둔 것이다.
- 정적 판정은 정의하는 자리만 안다. 호출하는 쪽이 값을 쓰는지는 동적이다(함수가 값이다). 그래서 끝의 `if`는 정의에서 엄격하게 하거나, 런타임에 "값이 없음"을 따로 표시해야 한다. 후자는 `nil`과 다른 값을 들여 값 모델(02)을 바꾼다.
- 끝의 `for`는 반대다. 리스트를 돌려주려는 함수도 흔하다(`fn names(xs): for x in xs: x.name ;; ;;`). 정의에서 막으면 정상 코드를 깬다. 문제는 버리는 쪽이므로 버리는 자리에서 잡는 것이 맞다.
- 버리는 리스트를 훑는 비용은 리스트 크기에 비례한다. 그 리스트를 만든 비용보다 작고, 버리는 값이 리스트일 때만 든다. 실행 루프의 뜨거운 길에는 태그 검사 옆에 종류 검사 하나가 더해진다(P12에서 드문 길을 `#[cold]`로 뺀 것과 같이 다룬다).
- 다른 언어: Rust는 블록의 마지막 식이 값이고, `else` 없는 `if`는 `()` 타입이라 값 자리에 쓰면 타입 에러다. 결과를 버리는 `#[must_use]`(`Result`, `Vec<Result>`가 아니라 `Result` 자체)는 경고다. Swift는 `@discardableResult`가 없으면 버린 반환값을 경고한다. Python·Ruby는 `None`·`nil`이 조용히 흐른다. Elixir·Haskell의 `if`는 `else` 없이 쓰면 `nil`·문법 에러다.

## 선택지

끝의 `if`:
(가) **블록 본문 함수의 마지막 항목도 값을 쓰는 자리다.** 끝의 `else` 없는 `if`는 E0223, 끝의 대입은 E0224. 부수효과 함수는 `else: nil`이나 마지막 줄 `nil`로 맺는다. 장점: 01의 "마지막 식은 반환값"과 맞고 규칙에서 예외 하나가 사라진다. 단점: 기존 코드 고침(양은 세어 봐야 한다).
(나) (가)에 더해 반환 타입이 `-> nil`이면 예외. 뜻은 분명하지만 반환 타입이 있는 함수는 자기 꼬리 호출만 프레임을 재사용한다(03 T4). `if c: g() ;;`로 끝나는 상태 기계가 꼬리 호출을 잃는다.
(다) "값이 없음" 표시 값을 들여 쓰는 순간 런타임 에러. 값 모델이 바뀌고 모든 연산이 그 값을 알아야 한다.
(라) 그대로 두고 문서만.

끝의 `for`(와 버려지는 리스트 일반):
(가) **버려지는 리스트도 훑는다.** `Op::Pop`과 콜백 결과의 버림(`discarded`)에서 값이 리스트면 맨 위 원소를 훑어 `Err`가 있으면 E0606. `main()`의 버림, `map(…)` 문장, `each`의 `f`가 돌려준 리스트가 모두 잡힌다.
(나) 블록 본문 함수의 끝 `for`를 `each`로 낮추고 `nil`을 돌려준다. 리스트를 바라는 함수가 조용히 `nil`이 되어 끝의 `if`와 같은 문제를 새로 만든다.
(다) 끝이 `for`인 블록 본문 함수에 경고. `fung run`은 경고를 보이지 않아 효과가 작다.
(라) 끝의 `for`를 컴파일 에러로 하고 `= for …`·`map`·끝의 `nil` 중 하나를 쓰게 한다. 뜻은 분명해지지만 버려지는 `map(…)` 문장은 여전히 조용하다.

추천: 끝의 `if`는 (가), 끝의 `for`는 (가). 앞의 것은 정의 자리의 규칙을 하나로 만들고, 뒤의 것은 문제가 생기는 버림 자리에서 잡아 더 넓은 구멍(버려지는 `map`)까지 닫는다. 둘 다 정상 동작은 바꾸지 않고 조용한 실패만 에러로 만든다.

## 정할 것

1. 끝의 `if`에 (가)를 고를지. 추천: (가). 고치기 전에 저장소에서 걸리는 곳을 센다(할 일 2). 수십 곳을 넘고 대부분 같은 모양이면 결과를 들고 (나)를 다시 묻는다.
2. 부수효과 함수의 맺음 권장형. 추천: 마지막 줄 `nil`보다 `else: nil`(E0223의 도움말과 같음). E0223 도움말에 "end the function with `nil` if it returns nothing"을 더할지도 묻는다.
3. 리스트 훑기의 깊이. 추천: 리스트의 맨 위 원소만. 튜플·맵·중첩 리스트는 훑지 않는다(02에 적음). 범위(`Range`)는 값을 품지 않으니 해당 없다.
4. 리스트 안의 `Err` 보고 형식. 추천: 번호는 E0606 그대로, 첫 `Err`의 내용을 보이고 `note: element 3 of the discarded list (of 10)`를 붙인다. 위치는 지금처럼 문장의 첫 글자.

## 할 일

1. 시험 먼저: `resolve/tests.rs`의 `values_used_must_not_be_nil`에 블록 본문 함수 끝의 `if`(E0223)·대입(E0224) 사례, 골든 `tests/lang/err_open_if_fn_end.fung`, `err_discard_list.fung`(끝 `for`의 `main()`, `map` 문장, `each`의 `f`가 돌려준 리스트), `discard.fung`에 `Err` 없는 리스트 버림이 조용한 사례. 실패를 먼저 확인한다.
2. `value.rs`에서 블록 본문 함수의 마지막을 `used = true`로 바꾸고 `extras`·`examples`·`tests/lang`·`crates/fung-cli/tests/data`·매뉴얼 펜스를 돌려 걸리는 곳을 센다. 정할 것 1을 그 숫자와 함께 묻는다. 고치면 `fmt_repo`·기대 출력(`FUNG_EXPECTED=bless`, diff 검토)이 함께 바뀐다.
3. `vm.rs`의 `Op::Pop`과 `m4.rs`의 `discarded_err`에 리스트 훑기(드문 길은 `#[cold]`). `prelude/collections.rs`의 `each`도 같은 함수를 쓴다. `bench/`의 계산 위주 측정이 잡음(±2%) 밖으로 느려지지 않는지 본다.
4. 설계 문서: 01 "식과 문장"(값을 쓰는 자리 목록에서 "블록 본문 함수의 마지막 항목" 예외를 지움, 버려지는 자리에 리스트 훑기), 02 "버려지는 Err"("잡지 않는 것"에서 리스트를 지우고 깊이 규칙을 적음), 08 E0223 설명, `resolve/value.rs` 머리 주석, open-questions 두 항목을 닫음.
5. 매뉴얼: `manual/learn/04-conditions-and-loops.md`·`05-functions.md`·`09-errors.md`, `manual/reference/language.md`, `manual/reference/errors.md`, `manual/vm/06-resolve.md`. 다시 만들고 diff 검토.
6. CHANGELOG `[Unreleased]` "바뀜(호환 깨짐)": 블록 본문 함수 끝의 `else` 없는 `if`가 E0223, 버려지는 리스트 안의 `Err`가 E0606.

## 끝의 기준

- 문제 절의 첫 예가 `error[E0223]`(종료 코드 2), 둘째 예와 `map([1, 2], x => Err(x))` 문장이 `error[E0606]`(종료 코드 1)이다.
- `fn names(xs): for x in xs: x ;; ;;`를 값 자리에서 쓰는 코드와 `Err` 없는 리스트를 버리는 코드는 그대로 돈다.
- 커밋 전 검사(CLAUDE.md)가 통과하고, 01·02·08·open-questions·매뉴얼·CHANGELOG가 같은 규칙을 말한다.
