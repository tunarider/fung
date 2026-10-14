package bytecode

import (
	"fmt"
	"strings"

	"github.com/tunarider/fung/internal/value"
)

// Disassemble renders a Proto and its child functions for
// `fung dump bytecode`:
//
//	0000  1:7   CONST        0    ; 1
func Disassemble(p *Proto) string {
	var b strings.Builder
	disasm(p, &b)
	return b.String()
}

func disasm(p *Proto, b *strings.Builder) {
	name := p.Name
	if name == "" {
		name = "<lambda>"
	}
	fmt.Fprintf(b, "== %s ==\n", name)
	for i, in := range p.Code {
		pos := fmt.Sprintf("%d:%d", p.Pos[i].Line, p.Pos[i].Col)
		if !in.Op().HasArg() {
			fmt.Fprintf(b, "%04d  %-5s %s\n", i, pos, in.Op())
			continue
		}
		fmt.Fprintf(b, "%04d  %-5s %-12s %d", i, pos, in.Op(), in.Arg())
		switch in.Op() {
		case OpConst:
			fmt.Fprintf(b, "    ; %s", value.Format(p.Consts[in.Arg()]))
		case OpLoadGlobal, OpStoreGlobal:
			if in.Arg() < len(p.GlobalNames) {
				fmt.Fprintf(b, "    ; %s", p.GlobalNames[in.Arg()])
			}
		case OpCall, OpTailCall:
			np, nn := DecodeCall(in.Arg())
			fmt.Fprintf(b, "    ; %d positional, %d named", np, nn)
		case OpClosure:
			if in.Arg() < len(p.Protos) {
				child := p.Protos[in.Arg()].Name
				if child == "" {
					child = "<lambda>"
				}
				fmt.Fprintf(b, "    ; %s", child)
			}
		case OpCheckType:
			slot, ti := in.Arg()&0xFFF, in.Arg()>>12
			if slot < len(p.Params) && ti < len(p.Types) {
				fmt.Fprintf(b, "    ; %s: %s", p.Params[slot].Name, value.TypeText(p.Types[ti]))
			}
		case OpCheckReturn:
			if in.Arg() < len(p.Types) {
				fmt.Fprintf(b, "    ; -> %s", value.TypeText(p.Types[in.Arg()]))
			}
		case OpTag:
			fmt.Fprintf(b, "    ; %d args", in.Arg())
		case OpSlice:
			var parts []string
			if in.Arg()&1 != 0 {
				parts = append(parts, "start")
			}
			if in.Arg()&2 != 0 {
				parts = append(parts, "end")
			}
			fmt.Fprintf(b, "    ; %s", strings.Join(parts, ", "))
		case OpField, OpImport:
			fmt.Fprintf(b, "    ; %s", value.FormatForPrint(p.Consts[in.Arg()]))
		case OpTest:
			fmt.Fprintf(b, "    ; %s", value.Format(p.Consts[in.Arg()]))
		case OpAssert:
			if in.Arg() < len(p.Asserts) {
				fmt.Fprintf(b, "    ; %s", p.Asserts[in.Arg()].Text)
			}
		case OpMatch, OpMatchOrFail:
			if in.Arg() < len(p.Patterns) {
				fmt.Fprintf(b, "    ; %s", p.Patterns[in.Arg()])
			}
		case OpRange:
			if in.Arg() == 1 {
				b.WriteString("    ; inclusive")
			}
		}
		b.WriteByte('\n')
	}
	for _, child := range p.Protos {
		b.WriteByte('\n')
		disasm(child, b)
	}
}
