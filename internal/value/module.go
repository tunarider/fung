package value

import (
	"bufio"
	"io"
	"sync"
)

// Module is an immutable record of a module's public names (04: 모듈은
// 값). Members keep export order for listings and dumps.
type Module struct {
	Name    string // what the user wrote or the file stem: fs, rename
	Key     string // runtime key: stdlib name, _native.x, or the absolute file path
	names   []string
	members map[string]Value
}

func NewModule(name, key string) *Module {
	return &Module{Name: name, Key: key, members: map[string]Value{}}
}

// Set adds or replaces a member, keeping first-insertion order.
func (m *Module) Set(name string, v Value) {
	if _, ok := m.members[name]; !ok {
		m.names = append(m.names, name)
	}
	m.members[name] = v
}

func (m *Module) Get(name string) (Value, bool) { v, ok := m.members[name]; return v, ok }
func (m *Module) Names() []string               { return m.names }
func (*Module) TypeName() string                { return "module" }

// File is an open file handle (02, 06): a stateful resource. Reader is
// set for 'read, Writer for 'write/'append; Closed makes every operation
// Err({kind: 'closed}).
type File struct {
	Path   string
	Reader *bufio.Reader
	Writer *bufio.Writer
	Closer io.Closer
	Closed bool
	Mu     sync.Mutex // one operation at a time: tasks may share a handle (06)
}

func (*File) TypeName() string { return "file" }
