package value

import (
	"sync"
	"unicode"
)

// Sym is an interned symbol ('ok). Equal symbols share one id, so
// comparison is an integer compare (02). The table is process-wide and
// guarded for the tasks of M7.
type Sym int32

var symbols = struct {
	sync.RWMutex
	ids   map[string]Sym
	names []string
}{ids: map[string]Sym{}}

// Intern returns the symbol for a name, creating it on first use.
func Intern(name string) Sym {
	symbols.RLock()
	id, ok := symbols.ids[name]
	symbols.RUnlock()
	if ok {
		return id
	}
	symbols.Lock()
	defer symbols.Unlock()
	if id, ok := symbols.ids[name]; ok {
		return id
	}
	id = Sym(len(symbols.names))
	symbols.names = append(symbols.names, name)
	symbols.ids[name] = id
	return id
}

// Name returns the symbol's text without the quote.
func (s Sym) Name() string {
	symbols.RLock()
	defer symbols.RUnlock()
	return symbols.names[s]
}

func (Sym) TypeName() string { return "sym" }

// IsName reports whether s is a fung name (01): a lowercase letter or `_`
// followed by letters, digits and `_`. Symbols and module stems use it.
func IsName(s string) bool {
	for i, r := range s {
		switch {
		case i == 0 && (r == '_' || (unicode.IsLetter(r) && !unicode.IsUpper(r))):
		case i > 0 && (r == '_' || unicode.IsLetter(r) || unicode.IsDigit(r)):
		default:
			return false
		}
	}
	return s != ""
}
