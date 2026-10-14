package frontend

import (
	"testing"

	"github.com/tunarider/fung/internal/value"
)

var fakePrelude = map[string]value.Value{"print": &value.Native{Name: "print"}}

func TestCompileStopsAtRequestedStage(t *testing.T) {
	b, errs := Compile("t.fung", "print(1)", fakePrelude, StageAST)
	if len(errs) != 0 || b.Surface == nil || b.Core != nil || b.Proto != nil {
		t.Fatalf("ast: %+v %v", b, errs)
	}
	b, errs = Compile("t.fung", "print(1)", fakePrelude, StageCore)
	if len(errs) != 0 || b.Core == nil || b.Proto != nil {
		t.Fatalf("core: %+v %v", b, errs)
	}
	if got := b.Core.String(); got != "(do (call (prelude print) 1))" {
		t.Fatalf("core = %s", got)
	}
	b, errs = Compile("t.fung", "print(1)", fakePrelude, StageAll)
	if len(errs) != 0 || b.Proto == nil || b.Proto.Name != "<main>" {
		t.Fatalf("all: %+v %v", b, errs)
	}
}

func TestCompileReturnsFirstSyntaxErrorAndAllNameErrors(t *testing.T) {
	_, errs := Compile("t.fung", "1 +", fakePrelude, StageAll)
	if len(errs) != 1 || errs[0].Code != "E0110" {
		t.Fatalf("errs = %v", errs)
	}
	_, errs = Compile("t.fung", "print(a + b)", fakePrelude, StageAll)
	if len(errs) != 2 || errs[0].Code != "E0204" || errs[1].Code != "E0204" {
		t.Fatalf("errs = %v", errs)
	}
}
