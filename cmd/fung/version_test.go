package main

import "testing"

func TestResolveVersion(t *testing.T) {
	cases := []struct {
		name, linked, module, want string
	}{
		{"release build sets it at link time", "0.1.0", "", "0.1.0"},
		{"a leading v from the tag is dropped", "v0.1.0", "", "0.1.0"},
		{"link-time value wins over the module version", "0.2.0", "v0.1.0", "0.2.0"},
		{"go install records the module version", "", "v0.1.0", "0.1.0"},
		{"a local build has no module version", "", "(devel)", "0.0.0-dev"},
		{"nothing known", "", "", "0.0.0-dev"},
	}
	for _, c := range cases {
		if got := resolveVersion(c.linked, c.module); got != c.want {
			t.Errorf("%s: resolveVersion(%q, %q) = %q, want %q", c.name, c.linked, c.module, got, c.want)
		}
	}
}
