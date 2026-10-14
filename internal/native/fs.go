package native

import (
	"bufio"
	"errors"
	"fmt"
	"github.com/tunarider/fung/internal/nat"
	"io/fs"
	"os"
	"path/filepath"
	"sort"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

// fsModule builds _native.fs: the primitives fs.fung wraps (06). Every
// operation that can fail for external reasons returns Ok/Err with
// {kind, message}; wrong argument types are runtime errors.
func fsModule() *value.Module {
	m := value.NewModule("fs", "_native.fs")
	add := func(n *value.Native) { m.Set(n.Name, n) }
	add(nat.Def("read", fsReadFn, nat.Param("path")))
	add(nat.Def("write", fsWriteFn, nat.Param("path"), nat.Param("text")))
	add(nat.Def("append", fsAppendFn, nat.Param("path"), nat.Param("text")))
	add(nat.Def("list", fsListFn, nat.Param("dir"), nat.Opt("full", value.Bool(false))))
	add(nat.Def("glob", fsGlobFn, nat.Param("pattern")))
	add(nat.Def("walk", fsWalkFn, nat.Param("dir")))
	add(nat.Def("exists", fsExistsFn, nat.Param("path")))
	add(nat.Def("stat", fsStatFn, nat.Param("path")))
	add(nat.Def("mkdir", fsMkdirFn, nat.Param("path")))
	add(nat.Def("remove", fsRemoveFn, nat.Param("path")))
	add(nat.Def("remove_all", fsRemoveAllFn, nat.Param("path")))
	add(nat.Def("move", fsMoveFn, nat.Param("from"), nat.Param("to")))
	add(nat.Def("copy", fsCopyFn, nat.Param("from"), nat.Param("to")))
	add(nat.Def("open", fsOpenFn, nat.Param("path"), nat.Opt("mode", value.Intern("read"))))
	add(nat.Def("close", fsCloseFn, nat.Param("f")))
	add(nat.Def("write_line", fsWriteLineFn, nat.Param("f"), nat.Param("line")))
	add(nat.Def("temp_dir", fsTempDirFn))
	return m
}

// fsErr turns an OS error into Err({kind, message}) (06: 외부 요인 실패).
func fsErr(verb, path string, err error) value.Value {
	kind := "io"
	switch {
	case errors.Is(err, fs.ErrNotExist):
		kind = "not_found"
	case errors.Is(err, fs.ErrExist):
		kind = "exists"
	case errors.Is(err, fs.ErrPermission):
		kind = "permission"
	}
	return nat.ErrValue(kind, fmt.Sprintf("cannot %s `%s`: %s", verb, path, nat.Reason(err)))
}

func fsReadFn(_ value.Context, args []value.Value) (value.Value, error) {
	p, err := nat.NeedStr("path", args[0])
	if err != nil {
		return nil, err
	}
	b, rerr := os.ReadFile(p)
	if rerr != nil {
		return fsErr("read", p, rerr), nil
	}
	return nat.Ok(value.Str(string(b))), nil
}

func fsWriteFn(_ value.Context, args []value.Value) (value.Value, error) {
	s, err := nat.StrArgs(args, "path", "text")
	if err != nil {
		return nil, err
	}
	if werr := os.WriteFile(s[0], []byte(s[1]), 0o644); werr != nil {
		return fsErr("write", s[0], werr), nil
	}
	return nat.Ok(value.Nil{}), nil
}

func fsAppendFn(_ value.Context, args []value.Value) (value.Value, error) {
	s, err := nat.StrArgs(args, "path", "text")
	if err != nil {
		return nil, err
	}
	f, oerr := os.OpenFile(s[0], os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0o644)
	if oerr != nil {
		return fsErr("append to", s[0], oerr), nil
	}
	_, werr := f.WriteString(s[1])
	if cerr := f.Close(); werr == nil {
		werr = cerr
	}
	if werr != nil {
		return fsErr("append to", s[0], werr), nil
	}
	return nat.Ok(value.Nil{}), nil
}

func strListValue(xs []string) value.Value {
	out := make([]value.Value, len(xs))
	for i, x := range xs {
		out[i] = value.Str(x)
	}
	return nat.NewList(out)
}

func fsListFn(_ value.Context, args []value.Value) (value.Value, error) {
	dir, err := nat.NeedStr("dir", args[0])
	if err != nil {
		return nil, err
	}
	full, ok := args[1].(value.Bool)
	if !ok {
		return nil, diag.Errorf("E0403", diag.Pos{}, "full", "bool", args[1].TypeName())
	}
	entries, rerr := os.ReadDir(dir)
	if rerr != nil {
		return fsErr("list", dir, rerr), nil
	}
	names := make([]string, len(entries))
	for i, e := range entries {
		names[i] = e.Name()
		if full {
			names[i] = filepath.Join(dir, e.Name())
		}
	}
	return nat.Ok(strListValue(names)), nil
}

func fsGlobFn(_ value.Context, args []value.Value) (value.Value, error) {
	pattern, err := nat.NeedStr("pattern", args[0])
	if err != nil {
		return nil, err
	}
	matches, gerr := filepath.Glob(pattern)
	if gerr != nil {
		return nat.ErrValue("invalid", fmt.Sprintf("bad glob pattern `%s`: %s", pattern, gerr)), nil
	}
	sort.Strings(matches)
	return nat.Ok(strListValue(matches)), nil
}

// fsWalkFn lists every file below dir (not directories), depth first in
// name order, as paths that start with dir.
func fsWalkFn(_ value.Context, args []value.Value) (value.Value, error) {
	dir, err := nat.NeedStr("dir", args[0])
	if err != nil {
		return nil, err
	}
	var files []string
	werr := filepath.WalkDir(dir, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if !d.IsDir() {
			files = append(files, path)
		}
		return nil
	})
	if werr != nil {
		return fsErr("walk", dir, werr), nil
	}
	return nat.Ok(strListValue(files)), nil
}

func fsExistsFn(_ value.Context, args []value.Value) (value.Value, error) {
	p, err := nat.NeedStr("path", args[0])
	if err != nil {
		return nil, err
	}
	_, serr := os.Stat(p)
	return value.Bool(serr == nil), nil
}

func fsStatFn(_ value.Context, args []value.Value) (value.Value, error) {
	p, err := nat.NeedStr("path", args[0])
	if err != nil {
		return nil, err
	}
	info, serr := os.Stat(p)
	if serr != nil {
		return fsErr("stat", p, serr), nil
	}
	m := value.NewMapBuilder()
	m.Set("name", value.Str(info.Name()))
	m.Set("size", value.Int(info.Size()))
	m.Set("is_dir", value.Bool(info.IsDir()))
	return nat.Ok(m.Map()), nil
}

func fsMkdirFn(_ value.Context, args []value.Value) (value.Value, error) {
	p, err := nat.NeedStr("path", args[0])
	if err != nil {
		return nil, err
	}
	if merr := os.MkdirAll(p, 0o755); merr != nil {
		return fsErr("create directory", p, merr), nil
	}
	return nat.Ok(value.Nil{}), nil
}

// fsRemoveFn removes one file or an empty directory; remove_all is the
// recursive version, kept separate so `rm -rf` is never the default.
func fsRemoveFn(_ value.Context, args []value.Value) (value.Value, error) {
	p, err := nat.NeedStr("path", args[0])
	if err != nil {
		return nil, err
	}
	if rerr := os.Remove(p); rerr != nil {
		return fsErr("remove", p, rerr), nil
	}
	return nat.Ok(value.Nil{}), nil
}

func fsRemoveAllFn(_ value.Context, args []value.Value) (value.Value, error) {
	p, err := nat.NeedStr("path", args[0])
	if err != nil {
		return nil, err
	}
	if rerr := os.RemoveAll(p); rerr != nil {
		return fsErr("remove", p, rerr), nil
	}
	return nat.Ok(value.Nil{}), nil
}

func fsMoveFn(_ value.Context, args []value.Value) (value.Value, error) {
	s, err := nat.StrArgs(args, "from", "to")
	if err != nil {
		return nil, err
	}
	if rerr := os.Rename(s[0], s[1]); rerr != nil {
		return fsErr("move", s[0], rerr), nil
	}
	return nat.Ok(value.Nil{}), nil
}

func fsCopyFn(_ value.Context, args []value.Value) (value.Value, error) {
	s, err := nat.StrArgs(args, "from", "to")
	if err != nil {
		return nil, err
	}
	info, serr := os.Stat(s[0])
	if serr != nil {
		return fsErr("copy", s[0], serr), nil
	}
	b, rerr := os.ReadFile(s[0])
	if rerr != nil {
		return fsErr("copy", s[0], rerr), nil
	}
	if werr := os.WriteFile(s[1], b, info.Mode().Perm()); werr != nil {
		return fsErr("copy to", s[1], werr), nil
	}
	return nat.Ok(value.Nil{}), nil
}

func fsOpenFn(_ value.Context, args []value.Value) (value.Value, error) {
	p, err := nat.NeedStr("path", args[0])
	if err != nil {
		return nil, err
	}
	mode, isSym := args[1].(value.Sym)
	if !isSym {
		return nil, diag.Errorf("E0403", diag.Pos{}, "mode", "'read | 'write | 'append", args[1].TypeName())
	}
	var f *os.File
	var oerr error
	h := &value.File{Path: p}
	switch mode.Name() {
	case "read":
		f, oerr = os.Open(p)
		if oerr == nil {
			h.Reader = bufio.NewReader(f)
		}
	case "write":
		f, oerr = os.Create(p)
		if oerr == nil {
			h.Writer = bufio.NewWriter(f)
		}
	case "append":
		f, oerr = os.OpenFile(p, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0o644)
		if oerr == nil {
			h.Writer = bufio.NewWriter(f)
		}
	default:
		return nil, diag.Errorf("E0403", diag.Pos{}, "mode", "'read | 'write | 'append", "'"+mode.Name())
	}
	if oerr != nil {
		return fsErr("open", p, oerr), nil
	}
	h.Closer = f
	return nat.Ok(h), nil
}

func needFile(v value.Value) (*value.File, error) {
	f, ok := v.(*value.File)
	if !ok {
		return nil, diag.Errorf("E0403", diag.Pos{}, "f", "file", v.TypeName())
	}
	return f, nil
}

func closedErr(f *value.File) value.Value {
	return nat.ErrValue("closed", "file `"+f.Path+"` is closed")
}

func fsCloseFn(_ value.Context, args []value.Value) (value.Value, error) {
	f, err := needFile(args[0])
	if err != nil {
		return nil, err
	}
	f.Mu.Lock()
	defer f.Mu.Unlock()
	if f.Closed {
		return closedErr(f), nil
	}
	f.Closed = true
	var cerr error
	if f.Writer != nil {
		cerr = f.Writer.Flush()
	}
	if e := f.Closer.Close(); cerr == nil {
		cerr = e
	}
	if cerr != nil {
		return fsErr("close", f.Path, cerr), nil
	}
	return nat.Ok(value.Nil{}), nil
}

func fsWriteLineFn(_ value.Context, args []value.Value) (value.Value, error) {
	f, err := needFile(args[0])
	if err != nil {
		return nil, err
	}
	line, err := nat.NeedStr("line", args[1])
	if err != nil {
		return nil, err
	}
	f.Mu.Lock()
	defer f.Mu.Unlock()
	if f.Closed {
		return closedErr(f), nil
	}
	if f.Writer == nil {
		return nat.ErrValue("invalid", "file `"+f.Path+"` is not open for writing"), nil
	}
	if _, werr := f.Writer.WriteString(line + "\n"); werr != nil {
		return fsErr("write", f.Path, werr), nil
	}
	return nat.Ok(value.Nil{}), nil
}

func fsTempDirFn(_ value.Context, _ []value.Value) (value.Value, error) {
	dir, err := os.MkdirTemp("", "fung-")
	if err != nil {
		return fsErr("create temporary directory", "", err), nil
	}
	return nat.Ok(value.Str(dir)), nil
}
