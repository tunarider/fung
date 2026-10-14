package value

import "strings"

// MatchesType reports whether v satisfies a union of type names (07):
// "any" always, "num" for int or float, a tag name for a tagged value
// (M4), otherwise v.TypeName() == name. A nil list means unchecked.
func MatchesType(v Value, alts []string) bool {
	if alts == nil {
		return true
	}
	tn := v.TypeName()
	for _, a := range alts {
		switch a {
		case "any":
			return true
		case "num":
			if tn == "int" || tn == "float" {
				return true
			}
		default:
			if a == tn {
				return true
			}
		}
	}
	return false
}

// TypeText renders a union for messages: "int | nil".
func TypeText(alts []string) string { return strings.Join(alts, " | ") }
