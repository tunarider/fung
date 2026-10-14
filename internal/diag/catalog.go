package diag

import "fmt"

// catalog maps error numbers to message templates. Numbers never change
// once assigned (golden tests and users depend on them); messages may be
// reworded. Categories: E01 syntax, E02 names/scope, E03 modules,
// E04 calls/annotations, E05 runtime operations, E06 patterns/results,
// E07 concurrency, E08 limits/environment, W01 warnings.
var catalog = map[string]string{
	// E01xx syntax (lexer, parser)
	"E0101": "unexpected character %q",
	"E0102": "malformed number %q",
	"E0103": "unknown escape \\%s in string",
	"E0104": "unterminated string",
	"E0105": "malformed duration %q",
	"E0110": "unexpected %s",
	"E0111": "comparison operators cannot be chained",
	"E0112": "expected %s, found %s",
	"E0113": "duplicate key `%s` in map literal",
	"E0114": "test must be at the top level",
	// E02xx names and scope
	"E0204": "undefined name `%s`",
	"E0205": "cannot reassign `%s` here: it was defined outside this block",
	"E0206": "`%s` is not visible here: it was defined inside a block",
	// E0207 (`used before it is assigned`) was retired on 2026-10-13: functions
	// copy variables when created, so a global can no longer be read unset.
	// The number is not reused.
	"E0208": "local function `%s` is defined later in this block",
	"E0209": "cannot reassign `%s`: it is a function or an imported name",
	"E0210": "function `%s` is defined twice",
	"E0211": "return outside a function",
	"E0212": "unknown type `%s`",
	"E0213": "return inside a for body",
	"E0214": "`?` outside a function",
	"E0215": "`_` can only be a call argument or a pattern",
	"E0216": "alternatives of a pattern must bind the same names",
	"E0217": "type `%s` is defined twice",
	"E0218": "type `%s` refers to itself",
	"E0219": "type aliases must be at the top level",
	"E0220": "`?` inside a for body",
	"E0221": "name `%s` is bound twice in one pattern",
	// E03xx modules
	"E0301": "cannot find module `%s`",
	"E0302": "circular import: %s",
	"E0303": "module `%s` has no member `%s`",
	"E0304": "`%s` is private to module `%s`",
	"E0305": "`%s` is not a valid module name; use `as`",
	"E0306": "only import, fn, type, and assignment are allowed at the top level of an imported module",
	"E0307": "cannot import the entry file",
	"E0308": "cannot shadow module `%s`",
	"E0309": "name `%s` is already bound to a module",
	"E0310": "`%s` is internal; import `%s` instead",
	"E0311": "import must be at the top level",
	// E04xx calls
	"E0401": "cannot call a value of type %s",
	"E0402": "%s takes %d argument(s), got %d",
	"E0403": "argument `%s` must be %s, got %s",
	"E0404": "return value must be %s, got %s",
	"E0405": "argument `%s` given twice",
	"E0406": "%s has no argument named `%s`",
	"E0407": "missing argument `%s`",
	// E05xx runtime operations
	"E0501": "cannot %s %s and %s",
	"E0502": "division by zero",
	"E0503": "integer overflow",
	"E0504": "result is not a finite number",
	"E0505": "expected bool, got %s",
	"E0506": "cannot %s %s",
	"E0507": "index %d is out of range (length %d)",
	"E0508": "key `%s` not found",
	"E0509": "cannot index a value of type %s",
	"E0511": "cannot slice a value of type %s",
	"E0512": "cannot test membership in a value of type %s",
	"E0513": "cannot convert %s to %s",
	"E0514": "cannot use a value of type %s as a map key",
	"E0515": "invalid pattern: %s",
	// E06xx patterns and results
	"E0601": "no pattern matched %s",
	"E0602": "value %s does not match the pattern",
	"E0603": "unhandled Err: %s",
	"E0604": "`%s` on nil",
	"E0605": "`%s` on a value of type %s (not Ok or Err)",
	// E07xx concurrency (05)
	"E0701": "task cancelled",
	"E0702": "deadlock: all tasks are waiting",
	"E0703": "atom swapped inside its own swap",
	// E08xx limits and environment
	"E0801": "internal error: %v",
	"E0802": "expression nesting too deep (limit %d)",
	"E0803": "%s",
	"E0804": "stack overflow: call depth exceeded %d",
	"E0805": "not supported yet: %s",
	"E0806": "too many %s (limit %d)",
	"E0807": "assertion failed: %s",
	"E0808": "`%s` has no value: the statement that assigned it failed",
	// W01xx warnings (check and test only, 08)
	"W0101": "unused import `%s`",
	"W0102": "unused variable `%s`",
	"W0103": "`%s` shadows a prelude function",
	"W0104": "`%s` is assigned as the last statement of a block used as a value, so the value is nil",
}

// hints are optional one-line suggestions shown after the position.
var hints = map[string]string{
	"E0111": "split it with `and`: a < b and b < c",
	"E0205": "a block cannot change an outer variable; build the value with an expression instead, e.g. `x = if … ;;` or `fold`",
	"E0206": "assign the block's value instead: `x = if … ;;`",
	"E0208": "local functions can only call themselves; move both functions to the top level",
	"E0211": "use `exit()` to stop a script early",
	"E0804": "make the recursion a tail call or use `fold`",
	"E0213": "`for` builds a list; use `find`, `all`/`any`, or `fold` instead",
	"E0214": "use `!` to stop the script on Err",
	"E0220": "`for` builds a list and cannot return early; handle the Err with `match`, or use `all`/`any`/`fold`",
	"E0508": "use get(m, key, default) or ?? for a default",
	"E0221": "use a guard to compare the two positions: `[a, b] when a == b`",
	"E0301": "standard library modules are imported by name (`import fs`); your own files by path (`import \"./lib/x\"`)",
	"E0304": "names starting with `_` are not exported",
	"E0306": "move the statement into a function, or into the entry file",
	"W0102": "prefix it with `_` if it is intentional",
	"W0104": "put the value last: end the block with an expression, e.g. the assigned name on its own line",
}

// labels are short notes printed next to the underline in excerpts.
var labels = map[string]string{
	"E0204": "not found in this scope",
}

// Label returns the excerpt note for a code, or "".
func Label(code string) string { return labels[code] }

// Message renders the catalog template for a code.
func Message(code string, args ...any) string {
	tmpl, ok := catalog[code]
	if !ok {
		return fmt.Sprintf("unknown error %s %v", code, args)
	}
	return fmt.Sprintf(tmpl, args...)
}

// Hint returns the help line for a code, or "".
func Hint(code string) string { return hints[code] }
