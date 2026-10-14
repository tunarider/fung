package lexer

import (
	"fmt"
	"time"

	"github.com/tunarider/fung/internal/diag"
)

// Kind classifies a token.
type Kind int

const (
	EOF Kind = iota
	Newline

	// Literals and names
	Int
	Float
	String    // "..." with escapes and ${} parts
	RawString // `...`
	Duration
	Symbol // 'name
	Name   // starts with lowercase, _, or a caseless letter
	Tag    // starts with an uppercase letter

	// Keywords (01. 예약어)
	KwFn
	KwIf
	KwElse
	KwThen
	KwMatch
	KwWhen
	KwFor
	KwIn
	KwNot
	KwAnd
	KwOr
	KwReturn
	KwImport
	KwAs
	KwType
	KwTest
	KwAssert
	KwTrue
	KwFalse
	KwNil

	// Punctuation and operators
	LParen
	RParen
	LBracket
	RBracket
	LBrace
	RBrace
	Comma
	Colon
	Semi
	DoubleSemi
	Dot
	DotDot
	DotDotEq
	Ellipsis
	FatArrow // =>
	Arrow    // ->
	Assign   // =
	Eq       // ==
	Ne       // !=
	Lt
	Gt
	Le
	Ge
	Plus
	Minus
	Star
	StarStar
	Slash
	SlashSlash
	Percent
	Pipe     // |>
	Bar      // |
	Coalesce // ??
	Question // ?
	Bang     // !
)

var kindNames = map[Kind]string{
	EOF: "end of file", Newline: "newline",
	Int: "integer", Float: "float", String: "string", RawString: "raw string",
	Duration: "duration", Symbol: "symbol", Name: "name", Tag: "tag",
	KwFn: "fn", KwIf: "if", KwElse: "else", KwThen: "then", KwMatch: "match", KwWhen: "when",
	KwFor: "for", KwIn: "in", KwNot: "not", KwAnd: "and", KwOr: "or", KwReturn: "return",
	KwImport: "import", KwAs: "as", KwType: "type", KwTest: "test", KwAssert: "assert",
	KwTrue: "true", KwFalse: "false", KwNil: "nil",
	LParen: "(", RParen: ")", LBracket: "[", RBracket: "]", LBrace: "{", RBrace: "}",
	Comma: ",", Colon: ":", Semi: ";", DoubleSemi: ";;", Dot: ".", DotDot: "..", DotDotEq: "..=",
	Ellipsis: "...", FatArrow: "=>", Arrow: "->", Assign: "=", Eq: "==", Ne: "!=",
	Lt: "<", Gt: ">", Le: "<=", Ge: ">=", Plus: "+", Minus: "-", Star: "*", StarStar: "**",
	Slash: "/", SlashSlash: "//", Percent: "%", Pipe: "|>", Bar: "|", Coalesce: "??",
	Question: "?", Bang: "!",
}

// IsKeyword reports a reserved word. Where only a name can appear (after
// `.`, as a map key) the parser accepts keywords as plain names (01).
func (k Kind) IsKeyword() bool { return k >= KwFn && k <= KwNil }

func (k Kind) String() string {
	if s, ok := kindNames[k]; ok {
		return s
	}
	return fmt.Sprintf("Kind(%d)", int(k))
}

// Keywords returns the reserved words and their kinds.
func Keywords() map[string]Kind {
	return map[string]Kind{
		"fn": KwFn, "if": KwIf, "else": KwElse, "then": KwThen, "match": KwMatch, "when": KwWhen,
		"for": KwFor, "in": KwIn, "not": KwNot, "and": KwAnd, "or": KwOr, "return": KwReturn,
		"import": KwImport, "as": KwAs, "type": KwType, "test": KwTest, "assert": KwAssert,
		"true": KwTrue, "false": KwFalse, "nil": KwNil,
	}
}

// Token is one lexical unit. Text is the source slice (for names, tags,
// symbols it is the identifier without any sigil). Int, Float, Dur and
// Parts carry the decoded value of literals.
type Token struct {
	Kind  Kind
	Text  string
	Pos   diag.Pos
	Off   int // byte offset of the token's first byte in the source
	End   int // byte offset just past the token (assert/dbg show source spans, 08)
	Int   int64
	Float float64
	Dur   time.Duration
	Parts []Part
}

// Part is one piece of a "..." string: literal text, or the source of an
// interpolated ${...} expression that the parser parses later.
type Part struct {
	IsCode bool
	Text   string
	Pos    diag.Pos
	Off    int // byte offset of Text in the whole source (LexAt adds it to spans)
}

// Comment is trivia kept aside for the formatter.
type Comment struct {
	Pos  diag.Pos
	Text string
}

func (t Token) String() string {
	switch t.Kind {
	case Name, Tag, Symbol, Int, Float, Duration, String, RawString:
		return fmt.Sprintf("%v(%s)", t.Kind, t.Text)
	}
	return t.Kind.String()
}
