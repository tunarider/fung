// Package extras holds the built-in extras (06: 네 층): the modules
// written in fung that ship inside the binary, and the registry that
// bundles them with the native bridges they build on. Only cmd/fung
// assembles this with the core; nothing under internal/ imports it.
package extras

import (
	"embed"

	"github.com/tunarider/fung/internal/frontend"
	"github.com/tunarider/fung/internal/native"
)

//go:embed *.fung
var files embed.FS

// Source returns the fung source of a built-in module by name
// (prelude, fs, path, cli, …).
func Source(name string) (string, bool) {
	b, err := files.ReadFile(name + ".fung")
	if err != nil {
		return "", false
	}
	return string(b), true
}

// Default is the built-in library: the embedded fung modules plus the Go
// bridges they import. The concurrency bridge is the VM's; the command
// adds it.
func Default() frontend.Registry {
	return frontend.Registry{Stdlib: Source, Natives: native.Bridges()}
}
