package prelude

import (
	"testing"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/nat"
	"github.com/tunarider/fung/internal/value"
)

type ctx = nat.FakeContext

func TestPrintJoinsArgsWithSpaces(t *testing.T) {
	c := &ctx{}
	print := Test()["print"].(*value.Native)
	v, err := print.Fn(c, []value.Value{value.NewList([]value.Value{value.Int(1), value.Float(2), value.Bool(true)})})
	if err != nil || v != (value.Nil{}) {
		t.Fatalf("v = %v, err = %v", v, err)
	}
	if got := c.Out.String(); got != "1 2.0 true\n" {
		t.Fatalf("out = %q", got)
	}
	c.Out.Reset()
	print.Fn(c, []value.Value{value.NewList(nil)})
	if got := c.Out.String(); got != "\n" {
		t.Fatalf("print() out = %q", got)
	}
}

func TestFailReturnsE0803(t *testing.T) {
	fail := Test()["fail"].(*value.Native)
	_, err := fail.Fn(&ctx{}, []value.Value{value.NewList([]value.Value{value.Str("port"), value.Int(1)})})
	d, ok := err.(*diag.Error)
	if !ok || d.Code != "E0803" || d.Args[0] != "port 1" {
		t.Fatalf("err = %v", err)
	}
	if _, err := fail.Fn(&ctx{}, []value.Value{value.NewList(nil)}); err == nil || err.(*diag.Error).Args[0] != "fail" {
		t.Fatalf("fail() = %v", err)
	}
}

func TestAtExitRegistersFunctionsOnly(t *testing.T) {
	atExit := Test()["at_exit"].(*value.Native)
	if _, err := atExit.Fn(&ctx{}, []value.Value{value.Int(1)}); err == nil || err.(*diag.Error).Code != "E0403" {
		t.Fatalf("at_exit(1) err = %v", err)
	}
	if v, err := atExit.Fn(&ctx{}, []value.Value{&value.Native{Name: "h"}}); err != nil || v != (value.Nil{}) {
		t.Fatalf("at_exit(fn) = %v %v", v, err)
	}
}
