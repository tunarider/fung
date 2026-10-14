package native

import (
	"regexp"
	"sync"
	"unicode/utf8"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/nat"
	"github.com/tunarider/fung/internal/value"
)

// regexModule builds _native.regex (06): Go RE2 over fung strings. Match
// positions are rune indexes (02: 문자열은 글자 단위), not byte offsets.
func regexModule() *value.Module {
	m := value.NewModule("regex", "_native.regex")
	add := func(n *value.Native) { m.Set(n.Name, n) }
	add(nat.Def("is_match", reMatchFn, nat.Param("s"), nat.Param("pat")))
	add(nat.Def("find", reFindFn, nat.Param("s"), nat.Param("pat")))
	add(nat.Def("find_all", reFindAllFn, nat.Param("s"), nat.Param("pat")))
	add(nat.Def("replace", reReplaceFn, nat.Param("s"), nat.Param("pat"), nat.Param("repl")))
	add(nat.Def("split", reSplitFn, nat.Param("s"), nat.Param("pat")))
	return m
}

// maxCachedPatterns bounds the compiled-pattern cache; past it the cache
// is emptied rather than evicted one by one (patterns are few and small).
const maxCachedPatterns = 256

var (
	patternMu    sync.Mutex
	patternCache = map[string]*regexp.Regexp{}
)

// compilePattern returns the compiled pattern, cached. A bad pattern is
// a code error (E0515): patterns are written in the script.
func compilePattern(pat string) (*regexp.Regexp, error) {
	patternMu.Lock()
	defer patternMu.Unlock()
	if re, ok := patternCache[pat]; ok {
		return re, nil
	}
	re, err := regexp.Compile(pat)
	if err != nil {
		return nil, diag.Errorf("E0515", diag.Pos{}, err.Error())
	}
	if len(patternCache) >= maxCachedPatterns {
		patternCache = map[string]*regexp.Regexp{}
	}
	patternCache[pat] = re
	return re, nil
}

// subjectAndPattern reads the (s, pat) arguments every function takes.
func subjectAndPattern(args []value.Value) (string, *regexp.Regexp, error) {
	s, err := nat.NeedStr("s", args[0])
	if err != nil {
		return "", nil, err
	}
	pat, err := nat.NeedStr("pat", args[1])
	if err != nil {
		return "", nil, err
	}
	re, err := compilePattern(pat)
	return s, re, err
}

// runeCursor converts byte offsets of s to rune indexes. Offsets that
// arrive in non-decreasing order (match after match) cost only the text
// between them, so find_all stays linear in the subject.
type runeCursor struct {
	s string
	b int // bytes counted so far
	r int // runes in s[:b]
}

func (c *runeCursor) index(byteOff int) int {
	if byteOff < c.b {
		c.b, c.r = 0, 0
	}
	c.r += utf8.RuneCountInString(c.s[c.b:byteOff])
	c.b = byteOff
	return c.r
}

// matchValue builds {text, start, end, groups, named} from submatch
// byte offsets (pairs, -1 for a group that did not take part).
func matchValue(s string, re *regexp.Regexp, loc []int, cur *runeCursor) value.Value {
	m := value.NewMapBuilder()
	m.Set("text", value.Str(s[loc[0]:loc[1]]))
	m.Set("start", value.Int(cur.index(loc[0])))
	m.Set("end", value.Int(cur.index(loc[1])))
	groups := []value.Value{}
	named := value.NewMapBuilder()
	names := re.SubexpNames()
	for i := 1; i < len(loc)/2; i++ {
		var g value.Value = value.Nil{}
		if loc[2*i] >= 0 {
			g = value.Str(s[loc[2*i]:loc[2*i+1]])
		}
		groups = append(groups, g)
		if names[i] != "" {
			named.Set(names[i], g)
		}
	}
	m.Set("groups", value.NewList(groups))
	m.Set("named", named.Map())
	return m.Map()
}

func reMatchFn(_ value.Context, args []value.Value) (value.Value, error) {
	s, re, err := subjectAndPattern(args)
	if err != nil {
		return nil, err
	}
	return value.Bool(re.MatchString(s)), nil
}

func reFindFn(_ value.Context, args []value.Value) (value.Value, error) {
	s, re, err := subjectAndPattern(args)
	if err != nil {
		return nil, err
	}
	loc := re.FindStringSubmatchIndex(s)
	if loc == nil {
		return value.Nil{}, nil
	}
	return matchValue(s, re, loc, &runeCursor{s: s}), nil
}

func reFindAllFn(_ value.Context, args []value.Value) (value.Value, error) {
	s, re, err := subjectAndPattern(args)
	if err != nil {
		return nil, err
	}
	all := re.FindAllStringSubmatchIndex(s, -1)
	out := make([]value.Value, len(all))
	cur := &runeCursor{s: s}
	for i, loc := range all {
		out[i] = matchValue(s, re, loc, cur)
	}
	return nat.NewList(out), nil
}

func reReplaceFn(_ value.Context, args []value.Value) (value.Value, error) {
	s, re, err := subjectAndPattern(args)
	if err != nil {
		return nil, err
	}
	repl, err := nat.NeedStr("repl", args[2])
	if err != nil {
		return nil, err
	}
	return value.Str(re.ReplaceAllString(s, repl)), nil
}

func reSplitFn(_ value.Context, args []value.Value) (value.Value, error) {
	s, re, err := subjectAndPattern(args)
	if err != nil {
		return nil, err
	}
	return strListValue(re.Split(s, -1)), nil
}
