package value

import "testing"

type fakeOpaque struct{ n int }

func (*fakeOpaque) TypeName() string  { return "widget" }
func (f *fakeOpaque) Display() string { return "<widget>" }

func TestOpaqueFormatAndIdentity(t *testing.T) {
	a, b := &fakeOpaque{1}, &fakeOpaque{1}
	if Format(a) != "<widget>" || FormatForPrint(a) != "<widget>" {
		t.Fatalf("format %s", Format(a))
	}
	if !Equal(a, a) || Equal(a, b) {
		t.Fatal("identity")
	}
	if _, err := Compare(a, b); err == nil || err.Code != "E0501" {
		t.Fatal("not ordered")
	}
	if Equal(a, Int(1)) {
		t.Fatal("never equal to other types")
	}
}
