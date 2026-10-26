# L5. 앞에 붙이기의 n², 맵·튜플 상수 접기 없음, 변하는 것을 품은 값의 `send` 비용

상태: 열림 (2026-10-26). 0.5.0 전에 고친다. 목록: [README](README.md).

## 문제

세 가지가 한 묶음이다. 모두 영속 컬렉션(`value/pvec.rs`·`value/list.rs`)과 그 위의 비용 약속에 걸린다.

1. **앞에 붙이기.** `[x] + acc`로 쌓으면 매번 전체를 복사해 n²다. 뒤에 붙이기(`acc + [x]`)는 선형이다. 릴리스 바이너리(2026-10-26):

   ```
   $ fung -e 'print(len(fold(0..N, [], (acc, i) => [i] + acc)))'          # N = 1만 / 2만 / 4만
   real 0.27 / 1.09 / 4.60
   $ fung -e 'print(len(reverse(fold(0..N, [], (acc, i) => acc + [i]))))'
   real 0.00 / 0.01 / 0.02
   ```

   함수형 언어에서 가장 흔한 누적 모양인데 매뉴얼(`manual/learn/07-collections.md` 237행)이 "뒤에 붙이고 `reverse`"를 안내해 피한다.
   언어의 약속(open-questions의 "누적은 선형", 07장 67행 "`acc + [x]`로 하나씩 쌓아도 빠르다")이 한쪽 방향에만 맞는다. 표현식의 펼치기 `[i, ...acc]`는 없다(E0110).
2. **상수 접기(P7)가 리스트만.** 원소가 모두 상수인 리스트 리터럴은 `CONST` 하나가 되지만, 맵·튜플은 매번 만든다. 빈 리스트를 품은 리스트도 접지 않는다.

   ```
   $ fung dump bytecode c.fung      # a = [1, 2, 3]; b = {x: 1, y: 2}; c = (1, "a"); d = [[], [1]]
   0000  1:5   CONST        0    ; [1, 2, 3]
   0002  2:6   CONST        1    ; "x"
   ...
   0006  2:5   MAP          2
   0008  3:6   CONST        2    ; 1
   0009  3:9   CONST        5    ; "a"
   0010  3:5   TUPLE        2
   0012  4:6   LIST         0
   0013  4:10  CONST        6    ; [1]
   0014  4:5   LIST         2
   ```

   빈 리스트 상수는 VM이 prelude의 `args`로 읽기 때문이다. 상수 종류 하나가 두 뜻을 갖는 것이 문제의 뿌리다.
3. **`send`의 훑기 비용.** 변하는 것(채널 끝·태스크 등)을 품은 값은 E0705 검사에서 크기에 비례해 훑는다.
   같은 영속 벡터를 나눠 쓰는 보기 여럿은 보기마다 처음부터 다시 훑는다. 20만 원소 + 채널 끝 하나인 리스트의 꼬리 보기 k개를 보내면:

   ```
   $ fung -e '(tx, rx) = channel(1)
   (t2, r2) = channel(1)
   big = map(0..200000, i => i) + [t2]
   vs = map(0..K, i => big[i..])
   send(tx, vs)
   print(len(recv(rx)!))'                  # K = 1 / 1000 / 4000
   real 0.01 / 0.15 / 0.56
   ```

   `t2` 대신 `0`을 넣으면 K = 4000에서 0.01초다(훑지 않음). open-questions는 일부러 만든 입력에서 지수적이라고 적었다. 그 입력은 이번에 재현하지 않았다(보기의 보기를 겹치는 모양으로 보인다; 세션에서 확인).
   05의 "`send`는 품은 값만 그 크기에 비례해 훑는다"(246행)는 지키지만, "그 크기"가 공유 구조를 펼친 크기라 영속 구조의 약속(공유는 공짜)과 어긋난다.

## 지금 동작과 원인

- 앞에 붙이기: `value/arith.rs` `add`의 `(List, List)` 갈래 → `List::concat`(`value/list.rs`). 왼쪽이 `Repr::Flat`(32개 이하)이면 두 쪽을 새 `Vec`에 모두 복사하고,
  왼쪽이 크면 왼쪽을 복제하고 오른쪽을 `push`한다(오른쪽 길이에 비례). `[x] + acc`는 왼쪽이 늘 작아 매번 `acc` 전체를 복사한다.
  `Pvec`(Clojure식 32갈래 트라이 + 뒤쪽 tail)는 뒤로만 자란다. 앞쪽 버퍼·시작 오프셋이 없다. `List::Big{vec, lo, hi}`의 `lo`는 자르기용 보기일 뿐이다.
- 상수 접기: `fung-front/src/compiler/m4.rs` `collection`의 `Head::List` 갈래가 `constant_list`를 부른다. `Head::Map`·`Head::Tuple`은 늘 `CONST`들 + `MAP`/`TUPLE`.
  `Const`(`fung-front/src/consts.rs`)에 맵·튜플 종류가 없다. `constant_list`는 빈 리스트를 `None`으로 거른다: `compiler.rs` 159행이 `args`를 빈 `Const::List`로 내고,
  `fung-vm/src/vm/link.rs` `constant`가 빈 `Const::List`를 prelude의 `args` 값으로 바꾼다(85~92행). 리스트 상수는 상수 풀에서 합치지 않는다(`bytecode.rs` `ConstKey::of`가 `None`).
- `send` 훑기: `vm/channel.rs` `contains`. 먼저 `Value::holds_mutable`(`value.rs`)로 거르고(큰 리스트·맵은 노드의 기억 `Memo`로 O(1), `pvec.rs` `node_holds`·`holds_mutable_in`),
  품었으면 명시적 스택으로 모든 원소를 `visit`한다. 같은 `Rc`를 두 번 보지 않으려고 `seen`에 **컨테이너 `Rc`의 주소**를 넣는데, 보기마다 `Rc<List>`가 따로라
  같은 `Pvec` 노드를 다시 간다. 큰 리스트 안에서도 `l.iter()`로 모든 원소를 보고, 노드의 기억(어느 하위 트리가 변하는 것을 품었는지)을 내려가는 길잡이로 쓰지 않는다.

## 고려할 점

- 영속 벡터의 선택지: Clojure `PersistentVector`(뒤만 쌈), Scala 2.13 `Vector`(앞뒤 모두 상각 O(1), 층마다 앞·뒤 배열), RRB 트리(Bagwell·Rompf; 이어 붙이기·중간 삽입 O(log n), 색인에 크기 표가 끼어 조금 느림),
  핑거 트리(Haskell `Data.Sequence`; 양끝 O(1), 색인 O(log n)이지만 상수가 큼). Elixir·Erlang은 연결 리스트라 앞이 싸고 뒤가 n이다. 어느 쪽이 싸야 하는지가 언어의 관용구를 정한다.
- 색인 `xs[i]`·`len`·자르기의 지금 비용(O(log32 n), O(1))을 잃지 않아야 한다. 기대 출력·perf(`crates/fung-cli/tests/perf.rs`)가 지키는 사례(스택처럼 쓰기, 꼬리 자르기)도 그대로 빨라야 한다.
- 큰 리스트 두 개의 이어 붙이기 `a + b`는 지금 `len(b)`에 비례한다. 앞에 붙이기만 고칠지, 일반 이어 붙이기까지 고칠지가 갈림길이다.
- 상수 접기는 값이 불변이라 안전하다. 다만 상수 하나가 하나의 `Rc`로 공유되므로, "참조가 하나면 제자리 수정"(`Rc::make_mut`)이 상수에는 닿지 않는다(복사가 한 번 생김). 지금 리스트도 같다.
- E0705는 "한 채널의 끝을 그 채널에 보내는" 순환만 막는다. 근본 해법(순환 회수)은 [07-rc-cycles.md](07-rc-cycles.md)와 겹친다. 여기서는 검사의 비용만 고친다.

## 선택지

**앞에 붙이기**

(가) 그대로 두고 매뉴얼 안내만. 비용 0. 관용구 하나가 함정으로 남는다.
(나) `Pvec`에 앞쪽 tail과 시작 오프셋을 둔다(Scala 2.12식 "앞으로도 자라는 트라이"). `[x] + acc`·`[a, b] + acc`가 상각 O(1)·O(len 왼쪽). 색인은 그대로 O(log32 n). 큰 + 큰은 여전히 오른쪽 길이에 비례.
(다) RRB 트리로 바꾼다. 앞에 붙이기와 큰 + 큰 모두 O(log n). 구현이 가장 크고(크기 표, 재균형), 색인이 조금 느려지며 지금의 기억(`Memo`)·`truncate`·`holds_mutable_in`을 다시 짜야 한다.

추천: **(나)**. 문제의 실제 모양(작은 왼쪽 + 큰 오른쪽)을 고치면서 바꾸는 곳이 `pvec.rs`·`list.rs`의 `concat`에 머문다. 큰 + 큰은 정할 것 1에서 따로 묻는다.

**상수 접기**

(가) `args`에 전용 상수 종류(`Const::Args` 같은 것)를 주고, `Const::Map`·`Const::Tuple`을 더해 `constant_list`를 리터럴 전반(`constant_value`)으로 넓힌다. 빈 리스트·빈 맵도 접는다.
(나) 리스트만 두고 빈 리스트 예외만 푼다.
추천: **(가)**. 한 상수 종류가 두 뜻인 것을 없애야 접기의 규칙이 "원소가 모두 상수인 리터럴"로 단순해진다. 태그 값(`Ok(1)`)도 접을지는 정할 것 2.

**`send` 훑기**

(가) 그대로(문서화된 한계).
(나) 큰 리스트·맵을 원소가 아니라 노드로 훑는다: 노드 기억이 `NO`인 하위 트리는 건너뛰고, `seen`에 **노드 주소**를 넣어 여러 보기가 같은 노드를 한 번만 본다. 보기의 경계가 자르는 잎만 원소 단위로 본다.
  비용이 "펼친 크기"에서 "변하는 것을 품은 노드 수 × 깊이 + 보기마다 경계 잎 둘"로 준다.
(다) E0705를 없애고 순환 회수에 맡긴다(07과 묶음).
추천: **(나)**. 지금 검사의 의미를 바꾸지 않고 비용만 영속 구조의 약속에 맞춘다. (다)는 07의 결론을 기다려야 한다.

## 정할 것

1. 큰 리스트 두 개의 이어 붙이기 `a + b`도 O(log n)으로 할까? 추천: 아니다(0.5.0 범위 밖, open-questions에 남김). 앞에 붙이기와 달리 흔한 누적 모양이 아니다.
2. 상수 접기에 태그 값(`Ok(1)`, `Some("x")`)과 중첩된 맵·튜플까지 넣을까? 추천: 넣는다(규칙이 "모든 원소가 상수인 리터럴"로 하나가 된다). 범위·`dur`는 이미 상수다.
3. 앞에 붙이기를 고친 뒤 매뉴얼 07장 237행의 안내를 지우고 "앞·뒤 모두 상각 O(1)"로 바꿀까? 추천: 바꾼다. 표현식 펼치기 `[x, ...acc]`는 이 문서 범위가 아니다(새 문법).

## 할 일

1. 시험 먼저(실패 확인):
   - `crates/fung-cli/tests/perf.rs`에 "prepend to a list"(`fold(0..20000, [], (acc, i) => [i] + acc)`, 0.3초 한도)와 "send many views of one vector"(위 재현, K = 4000) 사례.
   - `value/tests.rs`·`pvec.rs`의 모델 시험(`pvec_truncate_matches_the_model`과 같은 모양): 앞에 붙이기·뒤에 붙이기·자르기·`set`을 섞어 `Vec` 모델과 비교, 노드 기억이 앞쪽 변경 뒤에도 맞는지.
   - `fung-front` 단위 시험과 dump 기대 출력: `{x: 1}`·`(1, "a")`·`[[], [1]]`·`[]`가 `CONST` 하나. `args`가 여전히 실행 인자를 돌려주는 골든 시험(`tests/lang`).
   - `channel_tests.rs`: 보기 여럿에 숨긴 자기 끝도 E0705로 잡는지(의미가 그대로인지).
2. 코드: `pvec.rs`(앞쪽 tail·오프셋, `prefix`·`truncate`·`holds_mutable_in`의 오프셋 반영), `list.rs` `concat`(작은 왼쪽 + 큰 오른쪽 → 앞에 붙이기),
   `consts.rs`·`compiler.rs` 159행·`compiler/m4.rs`(`constant_value`), `bytecode.rs` `ConstKey`, `vm/link.rs` `constant`, `vm/channel.rs` `contains`(노드 단위 훑기; `pvec.rs`·`hamt.rs`에 노드를 내어 주는 반복자).
3. `FUNG_EXPECTED=bless`로 dump 기대 출력을 다시 쓰고 diff 검토. 매뉴얼에 `dump` 펜스가 있으면 `FUNG_UPDATE_MANUAL=1 cargo test -p fung-cli --test manual_build`.
4. 문서: 02(리스트의 비용 약속), 05 246행(`send` 비용), 09(영속 벡터 구조, 111행 근처), open-questions "알려진 한계"의 컬렉션 항목과 `send` 비용 항목 삭제(닫은 것은 `docs/notes/open-questions-closed.md`),
   매뉴얼 `manual/learn/07-collections.md` 237행, `manual/vm/internals.md`의 영속 벡터 설명. 학습용 설명은 `docs/notes/`에.
5. CHANGELOG `[Unreleased]`: 앞에 붙이기가 상각 O(1), 맵·튜플 상수 접기, `send` 검사 비용.
6. 측정: 바꾸기 전 바이너리를 복사해 두고 `scripts/bench.sh /tmp/fung-base target/release/fung`. `bench/list.fung`·`map.fung`·`chan.fung`이 느려지지 않아야 한다(색인·뒤에 붙이기의 회귀 확인).

## 끝의 기준

- `[i] + acc` 4만 번이 릴리스에서 0.05초 아래(지금 4.6초), perf 시험이 지킨다. `acc + [i]`·`s[..-1] + [x]`·꼬리 자르기 perf 사례가 그대로 통과한다.
- `fung dump bytecode`에서 상수만 든 맵·튜플·빈 리스트를 품은 리스트가 `CONST` 하나다. `args`는 전용 상수로 바뀌고 빈 `Const::List`를 `args`로 읽는 코드가 없다.
- 위 `send` 재현이 K = 4000에서 0.05초 아래이고, E0705 시험은 모두 그대로 통과한다.
- open-questions "알려진 한계"에서 세 항목이 빠지고(남긴 것은 정할 것 1의 결과만), 02·05·09·매뉴얼·CHANGELOG가 새 비용을 말한다. 커밋 전 검사(`CLAUDE.md`)가 모두 통과한다.
