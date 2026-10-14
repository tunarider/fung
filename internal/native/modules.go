package native

import "github.com/tunarider/fung/internal/value"

// Bridges returns the native layer (06): the hidden `_native.*` modules
// the fung extras build on, keyed by import name. Only embedded extras
// may import them. The concurrency bridge comes from the VM.
func Bridges() map[string]*value.Module {
	return map[string]*value.Module{
		"_native.fs":      fsModule(),
		"_native.path":    pathModule(),
		"_native.process": processModule(),
		"_native.time":    timeModule(),
		"_native.json":    jsonModule(),
		"_native.regex":   regexModule(),
		"_native.http":    httpModule(),
	}
}
