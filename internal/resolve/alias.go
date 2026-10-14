package resolve

import (
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/tree"
)

// declareAliases registers the file's type aliases before anything else
// is resolved, so functions above an alias can use it (07). Duplicates
// are E0217; aliases are dropped from the core tree.
func (r *resolver) declareAliases(root *tree.Node) {
	for _, k := range root.Kids {
		if k.Head != "typealias" {
			continue
		}
		a := k.Lit.(*tree.TypeAlias)
		if _, dup := r.aliases[a.Name]; dup {
			r.errs = append(r.errs, diag.Errorf("E0217", k.Pos, a.Name).Span(runeLen(a.Name)))
			continue
		}
		r.aliases[a.Name] = a.Alts
		r.aliasPos[a.Name] = k.Pos
	}
	for name := range r.aliases {
		r.expand(name, nil, r.aliasPos[name])
	}
}

// expand returns the union behind a type list with aliases replaced,
// recursively; a cycle is E0218 and an unknown name E0212.
func (r *resolver) expandTypes(alts []string, pos diag.Pos) []string {
	var out []string
	for _, a := range alts {
		out = append(out, r.expand(a, nil, pos)...)
	}
	return out
}

func (r *resolver) expand(name string, visiting []string, pos diag.Pos) []string {
	for _, v := range visiting {
		if v == name {
			if !r.reported[name] {
				r.reported[name] = true
				r.errs = append(r.errs, diag.Errorf("E0218", pos, name).Span(runeLen(name)))
			}
			return nil
		}
	}
	if alts, ok := r.aliases[name]; ok {
		var out []string
		for _, a := range alts {
			out = append(out, r.expand(a, append(visiting, name), pos)...)
		}
		return out
	}
	if builtinTypes[name] || isTagName(name) {
		return []string{name}
	}
	if !r.reported[name] {
		r.reported[name] = true
		r.errs = append(r.errs, diag.Errorf("E0212", pos, name).Span(runeLen(name)))
	}
	return nil
}
