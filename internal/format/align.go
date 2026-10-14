package format

import (
	"strings"
	"unicode/utf8"
)

// align pads consecutive lines so match arms share the `=>` column
// (pattern width max + 1) and trailing comments start in one column
// (code width max + 2; emit adds the two spaces). A run is broken by a
// blank line, a comment line, a line without the feature, or a different
// indent (spec: 정렬). Arms go first so comment columns see final code.
func align(rs []rendered) {
	// An arm whose body sits on deeper lines, or ends with a closing line
	// at the arm's own indent (`;;`, `;;)`), still belongs to the run.
	alignRuns(rs,
		func(r *rendered, runIndent int) bool {
			return !r.blank && r.code != "" && (r.indent > runIndent || (r.indent == runIndent && r.closing))
		},
		func(r *rendered) bool { return r.arrow >= 0 },
		func(r *rendered) int { return r.arrow },
		func(r *rendered, width int) {
			at := byteAt(r.code, r.arrow)
			r.code = r.code[:at] + strings.Repeat(" ", width-r.arrow) + r.code[at:]
			r.arrow = width
		})
	alignRuns(rs,
		func(*rendered, int) bool { return false },
		func(r *rendered) bool { return r.code != "" && r.comment != "" },
		func(r *rendered) int { return utf8.RuneCountInString(r.code) },
		func(r *rendered, width int) {
			r.code += strings.Repeat(" ", width-utf8.RuneCountInString(r.code))
		})
}

// alignRuns pads every run of lines that has the feature at one indent;
// through says which non-member lines may sit inside a run without
// ending it.
func alignRuns(rs []rendered, through func(*rendered, int) bool, has func(*rendered) bool, width func(*rendered) int, pad func(*rendered, int)) {
	for i := 0; i < len(rs); {
		if !has(&rs[i]) {
			i++
			continue
		}
		indent := rs[i].indent
		var members []int
		j := i
		for ; j < len(rs); j++ {
			switch {
			case has(&rs[j]) && rs[j].indent == indent:
				members = append(members, j)
			case through(&rs[j], indent):
			default:
				goto done
			}
		}
	done:
		max := 0
		for _, k := range members {
			if w := width(&rs[k]); w > max {
				max = w
			}
		}
		for _, k := range members {
			pad(&rs[k], max)
		}
		// Lines passed through may start runs of their own (an inner
		// match), so continue right after this run's first member.
		i++
	}
}

// byteAt converts a rune index in s to a byte offset.
func byteAt(s string, runes int) int {
	off := 0
	for i := 0; i < runes; i++ {
		_, n := utf8.DecodeRuneInString(s[off:])
		off += n
	}
	return off
}
