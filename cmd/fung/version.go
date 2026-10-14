package main

import (
	"runtime/debug"
	"strings"
)

// linkedVersion is set by release builds:
//
//	go build -ldflags "-X main.linkedVersion=0.1.0" ./cmd/fung
var linkedVersion string

// version is the toolchain version reported by `fung version`.
var version = resolveVersion(linkedVersion, moduleVersion())

// moduleVersion is the version `go install module@version` records in
// the binary; local builds report "(devel)".
func moduleVersion() string {
	if info, ok := debug.ReadBuildInfo(); ok {
		return info.Main.Version
	}
	return ""
}

// resolveVersion picks the link-time version, then the module version,
// then a development marker, and drops the tag's leading "v".
func resolveVersion(linked, module string) string {
	for _, v := range []string{linked, module} {
		if v != "" && v != "(devel)" {
			return strings.TrimPrefix(v, "v")
		}
	}
	return "0.0.0-dev"
}
