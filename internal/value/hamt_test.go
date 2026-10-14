package value

import (
	"math/rand/v2"
	"strconv"
	"testing"
)

// checkHamt drives random set/del against a Go map and re-reads saved
// versions to check that updates never change an older trie.
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
	hashKey = func(string) uint32 { return 7 } // every key collides
	checkHamt(t, 5000, 1000)
	hashKey = func(s string) uint32 { return uint32(len(s)) * 0x9e3779b9 } // few distinct hashes
	checkHamt(t, 20000, 5000)
}

func TestHamtEmpty(t *testing.T) {
	var n *hnode
	if _, ok := n.get(hashKey("a"), "a"); ok {
		t.Fatal("empty get")
	}
	if m, ok := n.del(0, hashKey("a"), "a"); ok || m != nil {
		t.Fatal("empty del")
	}
	one := n.set(0, hashKey("a"), "a", 1)
	if m, ok := one.del(0, hashKey("a"), "a"); !ok || m != nil {
		t.Fatal("deleting the last key leaves an empty trie")
	}
}
