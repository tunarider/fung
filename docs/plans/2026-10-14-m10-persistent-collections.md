# M10 영속 컬렉션 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 리스트를 "작으면 평평한 배열, 크면 영속 벡터 위의 보기"로, 맵을 "작으면 지금 표현, 크면 순서 벡터 + HAMT"로 바꿔 누적과 꼬리 재귀를 선형으로 만든다. 언어의 의미는 바뀌지 않는다.

**Architecture:** 세 단계로 나눈다. (1) 표현은 그대로 둔 채 값 층의 API를 메서드·빌더로 바꾸고 모든 사용처를 옮긴다(동작 불변, 기준 벤치마크). (2) 제네릭 영속 벡터 `pvec[T]`와 HAMT를 독립된 자료구조로 만들고 참조 구현과 비교하는 성질 테스트로 검증한다. (3) `List`와 `Map`의 내부를 교체하고, 경계값을 벤치마크로 정하고, 문서와 매뉴얼을 갱신한다.

**Tech Stack:** Go 1.27 표준 라이브러리(`hash/maphash`, `math/bits`, `iter`, 제네릭). 테스트는 Go 단위·성질 테스트(`math/rand/v2`, 고정 시드), 벤치마크, 골든.

**Spec:** `docs/superpowers/specs/2026-10-14-persistent-collections-design.md`

## Global Constraints

- Go 표준 라이브러리만. Conventional Commits, 영어, 마지막 줄 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- 커밋 전: `gofmt -l .`(출력 없음), `go vet ./...`, `go test ./...`. 동시성이 걸린 Task 3·5는 `go test -race ./internal/value ./internal/vm`도.
- TDD: 테스트를 먼저 쓰고 실패를 본 뒤 구현한다.
- **의미 불변**: 골든·단위 테스트의 기대값을 바꾸지 않는다. 에러 번호·메시지 그대로. 맵 순서는 넣은 순서, `==`는 순서 무시.
- 내부 표현은 `internal/value` 밖으로 나가지 않는다. 밖은 `NewList`/`Len`/`At`/`All`/`Slice`/`Append`/`Concat`/`ToSlice`/`ListBuilder`와 `NewMap`/`Get`/`Len`/`Keys`/`All`/`Put`/`Remove`/`Merge`/`MapBuilder`만 쓴다.
- 튜플(`Tuple.Items`), 태그(`Tag.Args`)는 바꾸지 않는다.
- 경계값은 패키지 변수 `smallMax`(리스트), `smallMapMax`(맵)이고 Task 6에서 정한다. 그 전에는 32.

## Review Focus

1. **보기에 덧붙이기**: `ys = xs[0..40]; zs = ys + [1]` 뒤에 `xs[40]`이 그대로인가(앞쪽 조각에 덧붙여도 원본이 안 바뀜) → Task 3 성질 테스트.
2. **같은 값에서 갈라지는 덧붙이기**: `a = big + [1]; b = big + [2]`에서 `a[-1] == 1`, `b[-1] == 2` → Task 2·3.
3. **경계 넘나들기**: 31·32·33개에서 `put`/`remove`/`slice`가 표현을 바꿀 때 순서·내용이 그대로인가 → Task 3·5.
4. **기존 키 `put`의 순서**: `put(m, 첫 키, v)`가 그 키를 맨 뒤로 옮기지 않는가(02) → Task 5.
5. **해시 충돌**: 모든 키가 같은 해시여도 `get`/`put`/`remove`가 맞는가 → Task 4.

---

### Task 1: 값 층 API 전환 (표현 그대로)

**Files:**
- Modify: `internal/value/collections.go`(List·Map API, 빌더), `internal/value/{index,arith,compare,format}.go`
- Modify(사용처): `internal/nat/nat.go`, `internal/prelude/{prelude,collections,strings,io}.go`, `internal/native/{json,regex,fs,http,time,process}.go`, `internal/vm/{match,m4,call,…}.go` 등 컴파일러가 가리키는 곳 전부
- Create: `internal/value/list_api_test.go`, `internal/value/bench_test.go`

**Interfaces:**
- Produces(이후 모든 Task가 쓰는 API):
  ```go
  func NewList(items []Value) *List          // 소유권을 넘겨받는다
  func (l *List) Len() int
  func (l *List) At(i int) Value
  func (l *List) All() iter.Seq2[int, Value]
  func (l *List) Slice(lo, hi int) *List
  func (l *List) Append(v Value) *List
  func (l *List) Concat(o *List) *List
  func (l *List) ToSlice() []Value          // 새 슬라이스
  type ListBuilder struct{ items []Value }
  func (b *ListBuilder) Add(v Value)
  func (b *ListBuilder) List() *List

  func NewMap() *Map                          // 빈 맵
  func (m *Map) Get(k string) (Value, bool)
  func (m *Map) Len() int
  func (m *Map) Keys() []string               // 새 슬라이스, 넣은 순서
  func (m *Map) All() iter.Seq2[string, Value]
  func (m *Map) Put(k string, v Value) *Map
  func (m *Map) Remove(k string) *Map
  func (m *Map) Merge(o *Map) *Map
  type MapBuilder struct{ … }
  func NewMapBuilder() *MapBuilder
  func (b *MapBuilder) Set(k string, v Value)
  func (b *MapBuilder) Map() *Map
  ```
- `List.Items` 필드와 `Map.Set`은 없어진다. `nat.Items`(순회 대상 → `[]Value`)와 `nat.NewList`는 남기되 안에서 새 API를 쓴다.

- [ ] **Step 1: 실패하는 API 테스트**

`internal/value/list_api_test.go`:

```go
package value

import "testing"

func ints(xs ...int) []Value {
	out := make([]Value, len(xs))
	for i, x := range xs {
		out[i] = Int(x)
	}
	return out
}

func TestListAPI(t *testing.T) {
	a := NewList(ints(1, 2, 3))
	if a.Len() != 3 || a.At(0) != Int(1) || a.At(2) != Int(3) {
		t.Fatal("Len/At")
	}
	b := a.Append(Int(4))
	if a.Len() != 3 || b.Len() != 4 || b.At(3) != Int(4) {
		t.Fatal("Append must not change the original")
	}
	s := b.Slice(1, 3)
	if Format(s) != "[2, 3]" || Format(b) != "[1, 2, 3, 4]" {
		t.Fatalf("Slice: %s %s", Format(s), Format(b))
	}
	if Format(a.Concat(NewList(ints(9)))) != "[1, 2, 3, 9]" {
		t.Fatal("Concat")
	}
	got := []Value{}
	for i, v := range b.All() {
		if Int(i+1) != v {
			t.Fatalf("All index %d value %v", i, v)
		}
		got = append(got, v)
	}
	if len(got) != 4 {
		t.Fatal("All length")
	}
	raw := b.ToSlice()
	raw[0] = Int(99)
	if b.At(0) != Int(1) {
		t.Fatal("ToSlice must copy")
	}
	var lb ListBuilder
	lb.Add(Int(7))
	lb.Add(Int(8))
	if Format(lb.List()) != "[7, 8]" {
		t.Fatal("ListBuilder")
	}
}

func TestMapAPI(t *testing.T) {
	mb := NewMapBuilder()
	mb.Set("b", Int(1))
	mb.Set("a", Int(2))
	mb.Set("b", Int(3)) // replace keeps position
	m := mb.Map()
	if Format(m) != "{b: 3, a: 2}" || m.Len() != 2 {
		t.Fatalf("builder: %s", Format(m))
	}
	m2 := m.Put("c", Int(4)).Put("b", Int(5))
	if Format(m2) != "{b: 5, a: 2, c: 4}" || Format(m) != "{b: 3, a: 2}" {
		t.Fatalf("Put: %s / %s", Format(m2), Format(m))
	}
	if Format(m2.Remove("a")) != "{b: 5, c: 4}" || Format(m2.Remove("zz")) != Format(m2) {
		t.Fatal("Remove")
	}
	o := NewMapBuilder()
	o.Set("a", Int(0))
	o.Set("d", Int(1))
	if Format(m.Merge(o.Map())) != "{b: 3, a: 0, d: 1}" {
		t.Fatal("Merge")
	}
	keys := m2.Keys()
	keys[0] = "zz"
	if m2.Keys()[0] != "b" {
		t.Fatal("Keys must copy")
	}
	n := 0
	for k, v := range m2.All() {
		if got, _ := m2.Get(k); got != v {
			t.Fatal("All")
		}
		n++
	}
	if n != 3 || NewMap().Len() != 0 {
		t.Fatal("All count / empty")
	}
}
```

- [ ] **Step 2: 실패 확인**

Run: `go test ./internal/value -run 'TestListAPI|TestMapAPI' 2>&1 | head -5`
Expected: 컴파일 실패(`undefined: NewList` 등)

- [ ] **Step 3: API 구현(평평한 표현)**

`collections.go`의 `List`·`Map`을 다음으로 바꾼다(필드는 소문자, 표현은 지금과 같다).

```go
// List is an ordered sequence (02). Immutable: every operation returns a
// new list. The representation is private to this package (M10).
type List struct{ flat []Value }

func NewList(items []Value) *List {
	if items == nil {
		items = []Value{}
	}
	return &List{flat: items}
}
func (l *List) Len() int        { return len(l.flat) }
func (l *List) At(i int) Value  { return l.flat[i] }
func (l *List) ToSlice() []Value { return append([]Value(nil), l.flat...) }
func (l *List) All() iter.Seq2[int, Value] {
	return func(yield func(int, Value) bool) {
		for i, v := range l.flat {
			if !yield(i, v) {
				return
			}
		}
	}
}
func (l *List) Slice(lo, hi int) *List  { return NewList(append([]Value(nil), l.flat[lo:hi]...)) }
func (l *List) Append(v Value) *List {
	out := make([]Value, len(l.flat)+1)
	copy(out, l.flat)
	out[len(l.flat)] = v
	return NewList(out)
}
func (l *List) Concat(o *List) *List {
	out := make([]Value, 0, len(l.flat)+len(o.flat))
	return NewList(append(append(out, l.flat...), o.flat...))
}

// ListBuilder collects items in place before the list is shared.
type ListBuilder struct{ items []Value }

func (b *ListBuilder) Add(v Value) { b.items = append(b.items, v) }
func (b *ListBuilder) List() *List { l := NewList(b.items); b.items = nil; return l }
```

맵은 `keys []string; vals map[string]Value`를 유지하고 `Set`을 지운다. `Put`은 지금 코드 그대로, `Remove`·`Merge`·`All`·`Keys`(복사)를 더하고 `MapBuilder`를 만든다:

```go
type MapBuilder struct{ m *Map }

func NewMapBuilder() *MapBuilder { return &MapBuilder{m: NewMap()} }
func (b *MapBuilder) Set(k string, v Value) {
	if _, ok := b.m.vals[k]; !ok {
		b.m.keys = append(b.m.keys, k)
	}
	b.m.vals[k] = v
}
func (b *MapBuilder) Map() *Map { m := b.m; b.m = nil; return m }
```

`Remove`는 키를 뺀 사본을, `Merge`는 `o`의 항목을 차례로 `Put`한 결과를 돌려준다(이 단계에서는 O(n)이어도 된다).

`index.go`·`arith.go`·`compare.go`·`format.go`의 `x.Items`는 `x.flat`(같은 패키지)이 아니라 **공개 API**로 고친다: `Len`/`At`/`Slice`/`Concat`/`All`. `equalItems`·`compareItems`는 `*List`·`*Tuple` 둘 다 받도록 `func(i int) Value`와 길이를 받는 형태로 바꾼다. 다음 Task에서 표현이 바뀌어도 이 파일들을 다시 열지 않기 위해서다.

- [ ] **Step 4: 사용처 이전**

`go build ./... 2>&1 | head -60`이 가리키는 곳을 아래 규칙으로 옮긴다.

| 지금 | 바꾼 뒤 |
|---|---|
| `&value.List{Items: xs}` | `value.NewList(xs)` |
| `&value.List{}` | `value.NewList(nil)` |
| `len(l.Items)` | `l.Len()` |
| `l.Items[i]` | `l.At(i)` |
| `for _, v := range l.Items` | `for _, v := range l.All()` |
| `l.Items[a:b]`(복사해 새 리스트) | `l.Slice(a, b)` |
| `l.Items`를 슬라이스로 넘김 | `l.ToSlice()` |
| `m := value.NewMap(); m.Set(k, v)…; return m` | `b := value.NewMapBuilder(); b.Set(k, v)…; return b.Map()` |

특히:
- `vm/match.go`의 `...rest`: `list.Slice(len(p.Kids), list.Len())`. 앞부분 비교는 `list.At(i)`로.
- `vm/call.go`의 `...rest` 매개변수: `value.NewList(append([]value.Value(nil), positional[fixed:]...))`.
- `prelude/collections.go`의 `group_by`: 지금은 그룹 리스트를 맵에 넣었다 꺼내며 덧붙인다. 키 순서 `[]string`과 `map[string][]value.Value`로 모은 뒤 마지막에 `MapBuilder`로 만든다.
- `nat.Items`: 리스트는 `x.ToSlice()`.

- [ ] **Step 5: 기준 벤치마크**

`internal/value/bench_test.go`(이후 Task가 같은 벤치마크로 비교한다):

```go
package value

import (
	"strconv"
	"testing"
)

func benchKeys(n int) []string {
	ks := make([]string, n)
	for i := range ks {
		ks[i] = "k" + strconv.Itoa(i)
	}
	return ks
}

func BenchmarkMapPutFold20k(b *testing.B) {
	ks := benchKeys(20000)
	for b.Loop() {
		m := NewMap()
		for i, k := range ks {
			m = m.Put(k, Int(i))
		}
	}
}

func BenchmarkMapPutExisting20k(b *testing.B) { // word-count shape: many updates of existing keys
	ks := benchKeys(20000)
	for b.Loop() {
		m := NewMap()
		for i, k := range ks {
			m = m.Put(k, Int(i))
		}
		for i, k := range ks {
			m = m.Put(k, Int(i+1))
		}
	}
}

func BenchmarkListAppend20k(b *testing.B) {
	for b.Loop() {
		l := NewList(nil)
		for i := range 20000 {
			l = l.Append(Int(i))
		}
	}
}

func BenchmarkListRest20k(b *testing.B) {
	items := make([]Value, 20000)
	for i := range items {
		items[i] = Int(i)
	}
	full := NewList(items)
	for b.Loop() {
		l := full
		for l.Len() > 0 {
			l = l.Slice(1, l.Len())
		}
	}
}

func BenchmarkSmallMapGet(b *testing.B) {
	for _, n := range []int{4, 16, 32} {
		mb := NewMapBuilder()
		ks := benchKeys(n)
		for i, k := range ks {
			mb.Set(k, Int(i))
		}
		m := mb.Map()
		b.Run(strconv.Itoa(n), func(b *testing.B) {
			for b.Loop() {
				for _, k := range ks {
					m.Get(k)
				}
			}
		})
	}
}

func BenchmarkSmallMapPut(b *testing.B) {
	for _, n := range []int{4, 16, 32} {
		ks := benchKeys(n)
		b.Run(strconv.Itoa(n), func(b *testing.B) {
			for b.Loop() {
				m := NewMap()
				for i, k := range ks {
					m = m.Put(k, Int(i))
				}
			}
		})
	}
}

func BenchmarkSmallListAt(b *testing.B) {
	for _, n := range []int{4, 16, 32} {
		l := NewList(make([]Value, n))
		b.Run(strconv.Itoa(n), func(b *testing.B) {
			for b.Loop() {
				for i := range n {
					l.At(i)
				}
			}
		})
	}
}
```

`BenchmarkMapPutFold20k` 등 큰 것은 지금 수 초가 걸리므로 기준은 `-benchtime=1x`로 한 번만 잰다. 결과를 장부(`progress.md`)에 그대로 남긴다:

```bash
go test ./internal/value -run '^$' -bench . -benchtime=1x 2>&1 | tee /tmp/bench-before-large.txt
go test ./internal/value -run '^$' -bench 'Small' -count=5 2>&1 | tee /tmp/bench-before-small.txt
```

- [ ] **Step 6: 전체 확인과 커밋**

Run: `gofmt -l . && go vet ./... && go test ./...`
Expected: 전부 통과(동작 불변). 골든 기대값을 고치지 않는다.

```bash
git add internal
git commit -m "refactor(value): hide list and map representations behind methods and builders

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: 영속 벡터 `pvec[T]`

**Files:**
- Create: `internal/value/pvec.go`, `internal/value/pvec_test.go`

**Interfaces:**
- Produces(패키지 내부):
  ```go
  type pvec[T any] struct{ cnt int; shift uint; root *pnode[T]; tail []T }
  func pvecOf[T any](items []T) *pvec[T]     // 일괄 구성 O(n)
  func (v *pvec[T]) at(i int) T
  func (v *pvec[T]) push(x T) *pvec[T]       // 원본 불변
  func (v *pvec[T]) set(i int, x T) *pvec[T] // 원본 불변
  ```

- [ ] **Step 1: 실패하는 성질 테스트**

`internal/value/pvec_test.go`:

```go
package value

import (
	"math/rand/v2"
	"testing"
)

// pvec against a plain slice: every version stays readable and unchanged.
func TestPvecMatchesSlice(t *testing.T) {
	r := rand.New(rand.NewPCG(1, 2))
	type version struct {
		v   *pvec[int]
		ref []int
	}
	cur := version{v: pvecOf[int](nil), ref: nil}
	history := []version{cur}
	for step := 0; step < 40000; step++ {
		switch op := r.IntN(10); {
		case op < 7 || len(cur.ref) == 0:
			x := r.Int()
			cur = version{cur.v.push(x), append(append([]int(nil), cur.ref...), x)}
		default:
			i := r.IntN(len(cur.ref))
			x := r.Int()
			ref := append([]int(nil), cur.ref...)
			ref[i] = x
			cur = version{cur.v.set(i, x), ref}
		}
		if step%997 == 0 {
			history = append(history, cur)
		}
		if step%4999 == 0 {
			// fork from an old version: its future must not see cur's pushes
			old := history[r.IntN(len(history))]
			f := old.v.push(-1)
			if f.at(f.cnt-1) != -1 || old.v.cnt != len(old.ref) {
				t.Fatal("fork changed its parent")
			}
		}
	}
	for _, h := range append(history, cur) {
		if h.v.cnt != len(h.ref) {
			t.Fatalf("len %d want %d", h.v.cnt, len(h.ref))
		}
		for i, x := range h.ref {
			if h.v.at(i) != x {
				t.Fatalf("at(%d) = %d want %d (len %d)", i, h.v.at(i), x, len(h.ref))
			}
		}
	}
}

func TestPvecOfBoundaries(t *testing.T) {
	for _, n := range []int{0, 1, 31, 32, 33, 64, 1023, 1024, 1025, 1056, 32768, 32800} {
		items := make([]int, n)
		for i := range items {
			items[i] = i * 3
		}
		v := pvecOf(items)
		if v.cnt != n {
			t.Fatalf("n=%d cnt=%d", n, v.cnt)
		}
		for i := range n {
			if v.at(i) != i*3 {
				t.Fatalf("n=%d at(%d)=%d", n, i, v.at(i))
			}
		}
		w := v.push(-1)
		if w.at(n) != -1 || v.cnt != n {
			t.Fatalf("n=%d push after pvecOf", n)
		}
	}
}
```

- [ ] **Step 2: 실패 확인**

Run: `go test ./internal/value -run TestPvec 2>&1 | head -3`
Expected: `undefined: pvecOf`

- [ ] **Step 3: 구현**

`internal/value/pvec.go`:

```go
package value

// pvec is a persistent vector (Clojure's PersistentVector): a 32-way
// trie of leaves plus a tail of up to 32 items. Updates copy the path
// they change and share the rest, so old versions stay valid (M10).
const (
	pvBits  = 5
	pvWidth = 1 << pvBits
	pvMask  = pvWidth - 1
)

type pnode[T any] struct {
	kids []*pnode[T] // internal node
	vals []T         // leaf (len pvWidth)
}

type pvec[T any] struct {
	cnt   int
	shift uint
	root  *pnode[T]
	tail  []T
}

func (v *pvec[T]) tailOff() int { return v.cnt - len(v.tail) }

func (v *pvec[T]) leafFor(i int) []T {
	if i >= v.tailOff() {
		return v.tail
	}
	n := v.root
	for level := v.shift; level > 0; level -= pvBits {
		n = n.kids[(i>>level)&pvMask]
	}
	return n.vals
}

func (v *pvec[T]) at(i int) T { return v.leafFor(i)[i&pvMask] }

// pvecOf builds a vector from items in O(n): full 32-chunks become
// leaves, the last chunk the tail, then levels are grouped bottom-up.
func pvecOf[T any](items []T) *pvec[T] {
	n := len(items)
	if n == 0 {
		return &pvec[T]{shift: pvBits, root: &pnode[T]{}}
	}
	tailLen := n % pvWidth
	if tailLen == 0 {
		tailLen = pvWidth
	}
	off := n - tailLen
	tail := append([]T(nil), items[off:]...)
	var nodes []*pnode[T]
	for i := 0; i < off; i += pvWidth {
		nodes = append(nodes, &pnode[T]{vals: append([]T(nil), items[i:i+pvWidth]...)})
	}
	shift := uint(pvBits)
	for len(nodes) > pvWidth {
		var up []*pnode[T]
		for i := 0; i < len(nodes); i += pvWidth {
			up = append(up, &pnode[T]{kids: nodes[i:min(i+pvWidth, len(nodes))]})
		}
		nodes = up
		shift += pvBits
	}
	return &pvec[T]{cnt: n, shift: shift, root: &pnode[T]{kids: nodes}, tail: tail}
}

func newPath[T any](level uint, node *pnode[T]) *pnode[T] {
	if level == 0 {
		return node
	}
	return &pnode[T]{kids: []*pnode[T]{newPath(level-pvBits, node)}}
}

// pushTail inserts a full tail leaf at the end of the trie, copying the
// path from parent down (cnt is the count including that tail).
func (v *pvec[T]) pushTail(level uint, parent *pnode[T], leaf *pnode[T]) *pnode[T] {
	sub := ((v.cnt - 1) >> level) & pvMask
	kids := make([]*pnode[T], len(parent.kids), max(len(parent.kids), sub+1))
	copy(kids, parent.kids)
	var child *pnode[T]
	if level == pvBits {
		child = leaf
	} else if sub < len(parent.kids) {
		child = v.pushTail(level-pvBits, parent.kids[sub], leaf)
	} else {
		child = newPath(level-pvBits, leaf)
	}
	if sub < len(kids) {
		kids[sub] = child
	} else {
		kids = append(kids, child)
	}
	return &pnode[T]{kids: kids}
}

func (v *pvec[T]) push(x T) *pvec[T] {
	if len(v.tail) < pvWidth {
		tail := make([]T, len(v.tail)+1)
		copy(tail, v.tail)
		tail[len(v.tail)] = x
		return &pvec[T]{cnt: v.cnt + 1, shift: v.shift, root: v.root, tail: tail}
	}
	leaf := &pnode[T]{vals: v.tail}
	shift := v.shift
	var root *pnode[T]
	if (v.cnt >> pvBits) > (1 << v.shift) { // root is full: grow a level
		root = &pnode[T]{kids: []*pnode[T]{v.root, newPath(v.shift, leaf)}}
		shift += pvBits
	} else {
		root = v.pushTail(v.shift, v.root, leaf)
	}
	return &pvec[T]{cnt: v.cnt + 1, shift: shift, root: root, tail: []T{x}}
}

func (v *pvec[T]) set(i int, x T) *pvec[T] {
	if i >= v.tailOff() {
		tail := append([]T(nil), v.tail...)
		tail[i&pvMask] = x
		return &pvec[T]{cnt: v.cnt, shift: v.shift, root: v.root, tail: tail}
	}
	return &pvec[T]{cnt: v.cnt, shift: v.shift, root: assocPath(v.shift, v.root, i, x), tail: v.tail}
}

func assocPath[T any](level uint, n *pnode[T], i int, x T) *pnode[T] {
	if level == 0 {
		vals := append([]T(nil), n.vals...)
		vals[i&pvMask] = x
		return &pnode[T]{vals: vals}
	}
	kids := append([]*pnode[T](nil), n.kids...)
	sub := (i >> level) & pvMask
	kids[sub] = assocPath(level-pvBits, kids[sub], i, x)
	return &pnode[T]{kids: kids}
}
```

`pushTail`의 `cnt - 1`은 Clojure와 같이 "꼬리를 포함한 지금 개수"를 쓴다(`push`가 부를 때 `v.cnt`는 아직 꼬리가 찬 상태의 개수다). 루트가 꽉 찼는지는 `(cnt >> 5) > (1 << shift)`.

- [ ] **Step 4: 통과 확인**

Run: `go test ./internal/value -run TestPvec -count=1`
Expected: `ok`. 실패하면 Clojure의 `PersistentVector.pushTail`/`cons`와 대조한다(공식은 위와 같다).

- [ ] **Step 5: 커밋**

```bash
git add internal/value/pvec.go internal/value/pvec_test.go
git commit -m "feat(value): add a persistent vector

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: 리스트 표현 교체 (작은 배열 / 보기)

**Files:**
- Modify: `internal/value/collections.go`(List), Create: `internal/value/list.go`(List를 이 파일로 옮긴다), `internal/value/list_test.go`

**Interfaces:**
- Consumes: `pvec[Value]`, `pvecOf`, `push`, `at` (Task 2).
- Produces: Task 1의 `List` API 그대로, 내부만 바뀐다. 패키지 변수 `smallMax = 32`.

- [ ] **Step 1: 실패하는 성질 테스트**

`internal/value/list_test.go`: 무작위 연산(`Append`, `Slice`, `Concat`, `At`, `Len`, `All`)을 `List`와 참조 `[]Value`에 똑같이 적용한다. 매 연산 뒤 결과를 비교하고, 기록해 둔 옛 버전 30개를 다시 읽어 바뀌지 않았는지 본다. 크기는 0~5,000 사이를 오가게 하고 31·32·33을 자주 지나가게 한다(작은 크기에서 덧붙이기가 많도록). Review Focus 1·2를 명시적 사례로도 넣는다:

```go
func TestListViewAppendKeepsOriginal(t *testing.T) {
	xs := NewList(nil)
	for i := range 100 {
		xs = xs.Append(Int(i))
	}
	ys := xs.Slice(0, 40)
	zs := ys.Append(Int(-1))
	if xs.At(40) != Int(40) || zs.At(40) != Int(-1) || zs.Len() != 41 {
		t.Fatal("appending to a front slice changed the original")
	}
	a, b := xs.Append(Int(1)), xs.Append(Int(2))
	if a.At(100) != Int(1) || b.At(100) != Int(2) || xs.Len() != 100 {
		t.Fatal("forked appends interfere")
	}
}

func TestListRestIsCheap(t *testing.T) {
	xs := NewList(nil)
	for i := range 100000 {
		xs = xs.Append(Int(i))
	}
	for i := 0; xs.Len() > 0; i++ {
		if xs.At(0) != Int(i) {
			t.Fatalf("rest %d: %v", i, xs.At(0))
		}
		xs = xs.Slice(1, xs.Len())
	}
}
```

`TestListRestIsCheap`은 지금 구현(복사)으로는 100,000²/2번 복사라 수십 초가 걸려 `-timeout 60s`에서 실패한다.

- [ ] **Step 2: 실패 확인**

Run: `go test ./internal/value -run 'TestList' -timeout 60s 2>&1 | tail -3`
Expected: `TestListRestIsCheap` 시간 초과(성질 테스트는 표현과 무관하게 통과할 수 있다. 성질 테스트는 교체 뒤에도 같은 결과를 보장하는 안전망이다)

- [ ] **Step 3: 구현**

`internal/value/list.go`:

```go
package value

import "iter"

// smallMax is the largest list kept as a plain slice; bigger lists live
// in a persistent vector viewed through [lo, hi) (M10, tuned in Task 6).
var smallMax = 32

type List struct {
	flat   []Value     // small (vec == nil)
	vec    *pvec[Value] // big
	lo, hi int
}

func NewList(items []Value) *List {
	if items == nil {
		items = []Value{}
	}
	if len(items) <= smallMax {
		return &List{flat: items}
	}
	v := pvecOf(items)
	return &List{vec: v, lo: 0, hi: v.cnt}
}

func (l *List) Len() int {
	if l.vec == nil {
		return len(l.flat)
	}
	return l.hi - l.lo
}

func (l *List) At(i int) Value {
	if l.vec == nil {
		return l.flat[i]
	}
	return l.vec.at(l.lo + i)
}

func (l *List) All() iter.Seq2[int, Value] {
	return func(yield func(int, Value) bool) {
		if l.vec == nil {
			for i, v := range l.flat {
				if !yield(i, v) {
					return
				}
			}
			return
		}
		for i := l.lo; i < l.hi; {
			leaf := l.vec.leafFor(i) // walk a leaf at a time
			for j := i & pvMask; j < len(leaf) && i < l.hi; j, i = j+1, i+1 {
				if !yield(i-l.lo, leaf[j]) {
					return
				}
			}
		}
	}
}

func (l *List) ToSlice() []Value {
	out := make([]Value, 0, l.Len())
	for _, v := range l.All() {
		out = append(out, v)
	}
	return out
}

// Slice is a view on the same vector; a small result, or one much
// smaller than the vector it would pin, is copied out instead.
func (l *List) Slice(lo, hi int) *List {
	n := hi - lo
	if l.vec == nil || n <= smallMax || n*4 < l.vec.cnt {
		return NewList(l.sliceCopy(lo, hi))
	}
	return &List{vec: l.vec, lo: l.lo + lo, hi: l.lo + hi}
}

// sliceCopy copies items [lo, hi) into a new slice.
func (l *List) sliceCopy(lo, hi int) []Value {
	out := make([]Value, 0, hi-lo)
	for i := lo; i < hi; i++ {
		out = append(out, l.At(i))
	}
	return out
}

func (l *List) Append(x Value) *List {
	switch {
	case l.vec == nil && len(l.flat) < smallMax:
		out := make([]Value, len(l.flat)+1)
		copy(out, l.flat)
		out[len(l.flat)] = x
		return &List{flat: out}
	case l.vec == nil:
		return NewList(append(append(make([]Value, 0, len(l.flat)+1), l.flat...), x))
	case l.hi == l.vec.cnt: // the view reaches the end: grow the vector
		return &List{vec: l.vec.push(x), lo: l.lo, hi: l.hi + 1}
	default: // a front slice: its own vector, so the original is untouched
		return NewList(append(l.ToSlice(), x))
	}
}

func (l *List) Concat(o *List) *List {
	if o.Len() == 0 {
		return l
	}
	if l.Len() == 0 {
		return o
	}
	if l.Len()+o.Len() <= smallMax || l.vec == nil {
		return NewList(append(l.ToSlice(), o.ToSlice()...))
	}
	out := l
	for _, v := range o.All() {
		out = out.Append(v)
	}
	return out
}
```

`ListBuilder`는 Task 1 그대로(`NewList`가 크기에 따라 표현을 고른다).

`Concat`에서 `l`이 작고 `o`가 크면(`[x] + big`) 둘을 펼쳐 새로 만든다(O(n+m), 스펙의 "앞에 붙이기는 O(n)").

- [ ] **Step 4: 통과 확인(경쟁 검사 포함)**

Run: `go test ./internal/value -count=1 && go test -race ./internal/value -run 'TestList' && go test ./...`
Expected: 전부 통과. 골든 기대값 불변.

- [ ] **Step 5: 커밋**

```bash
git add internal/value
git commit -m "feat(value): back big lists with a persistent vector and slice views

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: HAMT

**Files:**
- Create: `internal/value/hamt.go`, `internal/value/hamt_test.go`

**Interfaces:**
- Produces(패키지 내부):
  ```go
  var hashKey = func(s string) uint32 // maphash; 테스트가 바꿔 끼운다
  type hnode struct{ bitmap uint32; slots []hslot; coll []hslot }
  func (n *hnode) get(h uint32, key string) (int, bool)
  func (n *hnode) set(shift uint, h uint32, key string, pos int) *hnode   // nil 노드에서도 동작, 원본 불변
  func (n *hnode) del(shift uint, h uint32, key string) (*hnode, bool)    // 빈 노드면 nil
  ```

- [ ] **Step 1: 실패하는 테스트**

`internal/value/hamt_test.go`: (a) 무작위 `set`/`del`/`get`을 Go 맵 참조와 비교(키 5만 개 규모, 옛 버전 불변 확인), (b) `hashKey`를 `func(string) uint32 { return 7 }`로 바꿔(모든 키 충돌) 같은 테스트를 1,000개 규모로, (c) 해시 하위 비트만 같은 키들(`func(s) uint32 { return uint32(len(s)) }`)로 깊이를 시험. 테스트는 끝나면 `hashKey`를 원래대로 되돌린다.

```go
func checkHamt(t *testing.T, ops int, keyspace int) {
	t.Helper()
	r := rand.New(rand.NewPCG(3, 4))
	var root *hnode
	ref := map[string]int{}
	type ver struct {
		n   *hnode
		ref map[string]int
	}
	var history []ver
	for step := 0; step < ops; step++ {
		k := "k" + strconv.Itoa(r.IntN(keyspace))
		h := hashKey(k)
		if r.IntN(4) == 0 {
			var ok bool
			root, ok = root.del(0, h, k)
			_, had := ref[k]
			if ok != had {
				t.Fatalf("del %s: %v want %v", k, ok, had)
			}
			delete(ref, k)
		} else {
			root = root.set(0, h, k, step)
			ref[k] = step
		}
		if step%(ops/20+1) == 0 {
			cp := map[string]int{}
			for a, b := range ref {
				cp[a] = b
			}
			history = append(history, ver{root, cp})
		}
	}
	history = append(history, ver{root, ref})
	for _, v := range history {
		for i := 0; i < keyspace; i++ {
			k := "k" + strconv.Itoa(i)
			got, ok := v.n.get(hashKey(k), k)
			want, has := v.ref[k]
			if ok != has || (ok && got != want) {
				t.Fatalf("get %s = %d,%v want %d,%v", k, got, ok, want, has)
			}
		}
	}
}

func TestHamt(t *testing.T) { checkHamt(t, 200000, 50000) }

func TestHamtCollisions(t *testing.T) {
	old := hashKey
	defer func() { hashKey = old }()
	hashKey = func(string) uint32 { return 7 }
	checkHamt(t, 5000, 1000)
	hashKey = func(s string) uint32 { return uint32(len(s)) * 0x9e3779b9 }
	checkHamt(t, 20000, 5000)
}
```

- [ ] **Step 2: 실패 확인**

Run: `go test ./internal/value -run TestHamt 2>&1 | head -3`
Expected: `undefined: hnode`

- [ ] **Step 3: 구현**

`internal/value/hamt.go`:

```go
package value

import (
	"hash/maphash"
	"math/bits"
)

// HAMT (hash array mapped trie) from string keys to positions: 32-way
// nodes indexed by 5 bits of the key's hash, a bitmap of present slots
// and a compact slot array (M10). Updates copy the path they change.

var hashSeed = maphash.MakeSeed()

var hashKey = func(s string) uint32 {
	h := maphash.String(hashSeed, s)
	return uint32(h) ^ uint32(h>>32)
}

const hamtMaxShift = 30 // levels use bits 0..29 (6 levels); below that, collision lists

type hslot struct {
	key  string
	hash uint32
	pos  int
	sub  *hnode // non-nil: a child node instead of an entry
}

type hnode struct {
	bitmap uint32
	slots  []hslot
	coll   []hslot // collision node: entries whose hashes are identical
}

func (n *hnode) get(h uint32, key string) (int, bool) {
	for shift := uint(0); n != nil; shift += pvBits {
		if n.coll != nil {
			for _, s := range n.coll {
				if s.key == key {
					return s.pos, true
				}
			}
			return 0, false
		}
		bit := uint32(1) << ((h >> shift) & pvMask)
		if n.bitmap&bit == 0 {
			return 0, false
		}
		s := n.slots[bits.OnesCount32(n.bitmap&(bit-1))]
		if s.sub == nil {
			return s.pos, s.key == key
		}
		n = s.sub
	}
	return 0, false
}

func (n *hnode) set(shift uint, h uint32, key string, pos int) *hnode {
	if n == nil {
		n = &hnode{}
	}
	if n.coll != nil {
		coll := append([]hslot(nil), n.coll...)
		for i := range coll {
			if coll[i].key == key {
				coll[i].pos = pos
				return &hnode{coll: coll}
			}
		}
		return &hnode{coll: append(coll, hslot{key: key, hash: h, pos: pos})}
	}
	bit := uint32(1) << ((h >> shift) & pvMask)
	i := bits.OnesCount32(n.bitmap & (bit - 1))
	if n.bitmap&bit == 0 {
		slots := make([]hslot, len(n.slots)+1)
		copy(slots, n.slots[:i])
		slots[i] = hslot{key: key, hash: h, pos: pos}
		copy(slots[i+1:], n.slots[i:])
		return &hnode{bitmap: n.bitmap | bit, slots: slots}
	}
	slots := append([]hslot(nil), n.slots...)
	s := slots[i]
	switch {
	case s.sub != nil:
		slots[i].sub = s.sub.set(shift+pvBits, h, key, pos)
	case s.key == key:
		slots[i].pos = pos
	default:
		slots[i] = hslot{sub: pair(shift+pvBits, s, hslot{key: key, hash: h, pos: pos})}
	}
	return &hnode{bitmap: n.bitmap, slots: slots}
}

// pair makes the smallest subtree holding two entries whose hashes agree
// on the bits above shift.
func pair(shift uint, a, b hslot) *hnode {
	if shift > hamtMaxShift { // every hash bit is used: a true collision
		return &hnode{coll: []hslot{a, b}}
	}
	ia, ib := (a.hash>>shift)&pvMask, (b.hash>>shift)&pvMask
	if ia == ib {
		return &hnode{bitmap: 1 << ia, slots: []hslot{{sub: pair(shift+pvBits, a, b)}}}
	}
	if ia > ib {
		a, b, ia, ib = b, a, ib, ia
	}
	return &hnode{bitmap: 1<<ia | 1<<ib, slots: []hslot{a, b}}
}

func (n *hnode) del(shift uint, h uint32, key string) (*hnode, bool) {
	if n == nil {
		return nil, false
	}
	if n.coll != nil {
		for i, s := range n.coll {
			if s.key == key {
				coll := append(append([]hslot(nil), n.coll[:i]...), n.coll[i+1:]...)
				if len(coll) == 0 {
					return nil, true
				}
				return &hnode{coll: coll}, true
			}
		}
		return n, false
	}
	bit := uint32(1) << ((h >> shift) & pvMask)
	if n.bitmap&bit == 0 {
		return n, false
	}
	i := bits.OnesCount32(n.bitmap & (bit - 1))
	s := n.slots[i]
	if s.sub != nil {
		sub, ok := s.sub.del(shift+pvBits, h, key)
		if !ok {
			return n, false
		}
		slots := append([]hslot(nil), n.slots...)
		if sub == nil {
			return removeSlot(n, i, bit), true
		}
		slots[i].sub = sub
		return &hnode{bitmap: n.bitmap, slots: slots}, true
	}
	if s.key != key {
		return n, false
	}
	return removeSlot(n, i, bit), true
}

func removeSlot(n *hnode, i int, bit uint32) *hnode {
	if len(n.slots) == 1 {
		return nil
	}
	slots := append(append([]hslot(nil), n.slots[:i]...), n.slots[i+1:]...)
	return &hnode{bitmap: n.bitmap &^ bit, slots: slots}
}
```

충돌 노드는 해시 32비트를 다 쓴 뒤(`shift > 30`)에만 만든다. 그래서 충돌 노드에 들어오는 키는 언제나 해시 전체가 같다. `get`의 `for` 조건이 `n != nil`이므로 빈 HAMT(`nil`)에서도 동작한다. 레벨은 shift 0,5,…,30(7개)이고 `shift > 30`이면 충돌 목록이다.

- [ ] **Step 4: 통과 확인과 커밋**

Run: `go test ./internal/value -run TestHamt -count=1`
Expected: `ok`

```bash
git add internal/value/hamt.go internal/value/hamt_test.go
git commit -m "feat(value): add a HAMT index from keys to positions

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: 맵 표현 교체 (작은 맵 / 순서 벡터 + HAMT)

**Files:**
- Create: `internal/value/omap.go`, `internal/value/omap_test.go`
- Modify: `internal/value/collections.go`(Map)

**Interfaces:**
- Consumes: `pvec[entry]`, `hnode` (Task 2·4).
- Produces: Task 1의 `Map` API 그대로. 패키지 변수 `smallMapMax = 32`.

- [ ] **Step 1: 실패하는 테스트**

`internal/value/omap_test.go`:
- 성질 테스트: 무작위 `Put`(새 키·기존 키)/`Remove`/`Merge`를 `Map`과 참조(키 순서 `[]string` + Go 맵, 02의 규칙대로: 기존 키 `put`은 자리 유지, `remove` 뒤 다시 넣으면 맨 뒤)에 적용하고 `Format`(순서 포함), `Len`, `Get`, `Keys`, `Equal`을 비교한다. 옛 버전 불변 확인. 크기가 31·32·33을 자주 오가게 한다.
- Review Focus 4:
  ```go
  func TestMapPutExistingKeepsPosition(t *testing.T) {
	m := NewMap()
	for i := range 100 {
		m = m.Put("k"+strconv.Itoa(i), Int(i))
	}
	m = m.Put("k0", Int(-1))
	if m.Keys()[0] != "k0" || mustGet(m, "k0") != Int(-1) {
		t.Fatal("put on an existing key moved it")
	}
	m = m.Remove("k0").Put("k0", Int(1))
	if ks := m.Keys(); ks[len(ks)-1] != "k0" {
		t.Fatal("a removed key comes back at the end")
	}
  }
  ```
- 시간: `TestMapPutFoldIsCheap` — 10만 키를 `Put`으로 쌓고 다시 전부 갱신. 지금 구현으로는 시간 초과.

- [ ] **Step 2: 실패 확인**

Run: `go test ./internal/value -run 'TestMap|TestOmap' -timeout 60s 2>&1 | tail -3`
Expected: `TestMapPutFoldIsCheap` 시간 초과

- [ ] **Step 3: 구현**

`internal/value/omap.go`:

```go
package value

// omap is a big map (M10): entries in insertion order in a persistent
// vector, and a HAMT from key to entry position. Removal leaves a dead
// entry; when dead entries outnumber live ones the map is rebuilt.
type entry struct {
	key  string
	val  Value
	live bool
}

type omap struct {
	ents       *pvec[entry]
	idx        *hnode
	live, dead int
}

func omapOf(keys []string, vals func(string) Value) *omap {
	es := make([]entry, len(keys))
	var idx *hnode
	for i, k := range keys {
		es[i] = entry{key: k, val: vals(k), live: true}
		idx = idx.set(0, hashKey(k), k, i)
	}
	return &omap{ents: pvecOf(es), idx: idx, live: len(keys)}
}

func (o *omap) get(k string) (Value, bool) {
	pos, ok := o.idx.get(hashKey(k), k)
	if !ok {
		return nil, false
	}
	return o.ents.at(pos).val, true
}

func (o *omap) put(k string, v Value) *omap {
	h := hashKey(k)
	if pos, ok := o.idx.get(h, k); ok {
		return &omap{ents: o.ents.set(pos, entry{key: k, val: v, live: true}), idx: o.idx, live: o.live, dead: o.dead}
	}
	pos := o.ents.cnt
	return &omap{ents: o.ents.push(entry{key: k, val: v, live: true}), idx: o.idx.set(0, h, k, pos), live: o.live + 1, dead: o.dead}
}

func (o *omap) remove(k string) *omap {
	h := hashKey(k)
	pos, ok := o.idx.get(h, k)
	if !ok {
		return o
	}
	idx, _ := o.idx.del(0, h, k)
	out := &omap{ents: o.ents.set(pos, entry{}), idx: idx, live: o.live - 1, dead: o.dead + 1}
	if out.dead > out.live {
		return out.compact()
	}
	return out
}

func (o *omap) each(yield func(string, Value) bool) {
	for i := 0; i < o.ents.cnt; i++ {
		if e := o.ents.at(i); e.live && !yield(e.key, e.val) {
			return
		}
	}
}

func (o *omap) compact() *omap {
	var keys []string
	vals := map[string]Value{}
	o.each(func(k string, v Value) bool { keys = append(keys, k); vals[k] = v; return true })
	return omapOf(keys, func(k string) Value { return vals[k] })
}
```

`each`는 리스트의 `All`처럼 잎 단위로 도는 최적화를 해도 된다(`ents.leafFor`).

`Map`은 `{keys []string; vals map[string]Value; big *omap}`이 된다. `big != nil`이면 큰 표현.
- `Get`/`Len`/`Keys`/`All`은 표현에 따라 나눈다.
- `Put`: 작은 맵에서 결과가 `smallMapMax`를 넘으면 `omapOf`로 바꾼 뒤 넣는다. 아니면 지금처럼 복사한다.
- `Remove`: 큰 맵에서 결과의 `live`가 `smallMapMax` 이하면 작은 표현으로 바꾼다.
- `Merge(o)`: `o.All()`을 차례로 `Put`한다(`m`이 작고 결과가 크면 첫 경계에서 큰 표현으로 바뀐다).
- `MapBuilder.Map()`: 개수가 `smallMapMax`를 넘으면 `omapOf`.
- `compare.go`의 맵 `==`는 이미 `Len`+`All`+`Get`으로 바뀌어 있으니(Task 1) 손대지 않는다.

- [ ] **Step 4: 통과 확인(경쟁 검사 포함)**

Run: `go test ./internal/value -count=1 && go test -race ./internal/value ./internal/vm && go test ./...`
Expected: 전부 통과

- [ ] **Step 5: 커밋**

```bash
git add internal/value
git commit -m "feat(value): back big maps with an ordered vector and a HAMT index

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: 경계값 정하기와 성능 고정

**Files:**
- Create: `internal/value/threshold_test.go`(벤치마크 보조), `tests/perf_test.go`
- Modify: `internal/value/list.go`·`collections.go`(정한 값)

- [ ] **Step 1: 실패하는 성능 회귀 테스트**

`tests/perf_test.go`(골든과 같은 `fungBinary`를 쓴다):

```go
package tests

import (
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"
)

// TestCollectionsStayLinear: the accumulation and tail-recursion shapes
// that were quadratic before M10 must finish 20,000 elements well under a
// second (they took 0.7–18 s with full copies).
func TestCollectionsStayLinear(t *testing.T) {
	cases := map[string]string{
		"map put fold":  `m = fold(0..20000, {}, (acc, i) => put(acc, str(i), i))` + "\nprint(len(m))",
		"map put again": `m = fold(0..20000, {}, (acc, i) => put(acc, str(i % 5000), i))` + "\nprint(len(m))",
		"list append":   `xs = fold(0..20000, [], (acc, i) => acc + [i])` + "\nprint(len(xs))",
		"rest pattern":  "fn count(xs, acc) = match xs:\n  [] => acc\n  [_, ...rest] => count(rest, acc + 1)\n;;\nprint(count(map(0..20000, x => x), 0))",
		"slice tail":    "fn go(ys, acc) = len(ys) == 0 then acc else go(ys[1..], acc + 1)\nprint(go(map(0..20000, x => x), 0))",
	}
	dir := t.TempDir()
	for name, src := range cases {
		f := filepath.Join(dir, "p.fung")
		os.WriteFile(f, []byte(src+"\n"), 0o644)
		start := time.Now()
		out, err := exec.Command(fungBinary, f).CombinedOutput()
		took := time.Since(start)
		if err != nil {
			t.Fatalf("%s: %v\n%s", name, err, out)
		}
		if took > time.Second {
			t.Errorf("%s took %s", name, took)
		}
	}
}
```

Run: `go test ./tests -run TestCollectionsStayLinear` — Task 3·6을 마친 뒤라 통과해야 한다. **확인용으로 한 번** Task 1 커밋에서 빌드한 바이너리로 돌려 실패(수 초)하는 것을 본다: `git worktree add /tmp/m10-base <Task1 커밋>` 후 그쪽에서 `go test ./tests -run TestCollectionsStayLinear` 실행. 확인 뒤 `git worktree remove /tmp/m10-base`.

- [ ] **Step 2: 경계값 비교**

`internal/value/threshold_test.go`에 `smallMax`·`smallMapMax`를 바꿔 끼우며 `BenchmarkSmall*`와 큰 벤치마크를 도는 도우미를 둔다(`b.Run(fmt.Sprintf("max=%d", m), …)`, 끝나면 원래대로). 16·32·64를 잰다:

```bash
go test ./internal/value -run '^$' -bench 'Threshold' -count=5 | tee /tmp/bench-threshold.txt
```

판단 규칙: (1) 작은 컬렉션(4·16·32개) 읽기·생성이 Task 1 기준 대비 10% 이내인 값 중, (2) 큰 벤치마크가 가장 좋은 값. 리스트와 맵을 따로 고른다. 결과 표와 고른 값을 장부에 적고 코드의 기본값을 바꾼다.

- [ ] **Step 3: 전후 측정 기록**

```bash
go test ./internal/value -run '^$' -bench . -benchtime=1x | tee /tmp/bench-after-large.txt
go test ./internal/value -run '^$' -bench 'Small' -count=5 | tee /tmp/bench-after-small.txt
```

Task 1 기준과 함께 장부에 표로 남긴다(노트와 매뉴얼이 이 숫자를 쓴다). 스펙 목표(네 패턴 0.1초 이하 — fung 스크립트 기준은 `tests/perf_test.go`의 각 시간을 `-v`로 출력해 확인)를 확인한다.

- [ ] **Step 4: 커밋**

```bash
git add internal/value tests/perf_test.go
git commit -m "test: pin linear collection costs and tune the small-collection thresholds

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: 노트, 매뉴얼, 상태

**Files:**
- Create: `docs/notes/M10-persistent-collections.md`
- Modify: `docs/internals.html`, `docs/syntax.html`, `docs/design/09-implementation-plan.md`, `docs/design/open-questions.md`, `CLAUDE.md`

- [ ] **Step 1: 노트** — 스펙의 "문서" 절 그대로: 통째 복사가 왜 n²인지(전후 표), 구조 공유와 길 복사, 영속 벡터(5비트·tail·`pushTail`), 보기와 1/4 규칙, HAMT(비트맵·`OnesCount32`·충돌 목록), 순서 문제와 항목 벡터 + 색인, 지운 자리 정리, 작은 표현과 정해진 경계값의 근거(Task 6 표), 구현 중 걸린 것.

- [ ] **Step 2: 매뉴얼 내부 구조** — `internals.html`의 "값의 표현"에서 `*List`·`*Map` 설명을 고치고 새 절 `<h2 id="collections">컬렉션의 내부</h2>`를 VM 절 뒤에 넣는다. 사이드바 목차에 추가. 내용은 스펙의 매뉴얼 항목 전부와 Task 6의 실측 표. 그림은 `<pre class="plain">`의 글자 그림(구조 공유, 순서 벡터 + 색인).

- [ ] **Step 3: 매뉴얼 문법 쪽 성능 안내** — `syntax.html`의 `for` 절 뒤나 "값과 타입"의 맵·리스트 항목에 짧은 안내: 누적·꼬리 재귀는 선형, 문자열 누적은 `join`, `[x] + acc`는 O(n).

- [ ] **Step 4: 상태 문서** — 09 마일스톤 표의 M10을 "완료"로, open-questions의 누적 비용 항목을 닫고, CLAUDE.md 현재 상태에 M10 줄.

- [ ] **Step 5: 검사와 커밋** — 매뉴얼 예제 검사기(문서 예제가 `fung check`·`fmt --check`·실행을 통과)와 링크 검사를 돌린다. `gofmt -l . && go vet ./... && go test ./...`.

```bash
git add docs CLAUDE.md
git commit -m "docs: add M10 notes and the collections section of the manual

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```
