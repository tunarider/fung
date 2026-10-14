package bytecode

import "testing"

func TestImportOpcode(t *testing.T) {
	if !OpImport.HasArg() || OpImport.String() != "IMPORT" {
		t.Fatalf("IMPORT: hasArg=%v name=%s", OpImport.HasArg(), OpImport.String())
	}
	in := Encode(OpImport, 7)
	if in.Op() != OpImport || in.Arg() != 7 {
		t.Fatal("round trip")
	}
}
