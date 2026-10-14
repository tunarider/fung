package repl

import (
	"os"
	"strings"
)

// maxHistory bounds the history file (08): only the latest lines stay.
const maxHistory = 1000

// historyFile persists the editor's history one input line per line.
// Every operation fails silently: history is a convenience and must
// never stop the REPL.
type historyFile struct {
	path  string
	lines []string
}

// openHistory loads path, trimming (and rewriting) a file longer than
// maxHistory. It returns nil when the file cannot be written, so the
// REPL runs without history.
func openHistory(path string) *historyFile {
	h := &historyFile{path: path}
	if b, err := os.ReadFile(path); err == nil {
		h.lines = strings.Split(strings.TrimSuffix(string(b), "\n"), "\n")
		if len(h.lines) == 1 && h.lines[0] == "" {
			h.lines = nil
		}
	}
	if len(h.lines) > maxHistory {
		h.lines = h.lines[len(h.lines)-maxHistory:]
		if err := os.WriteFile(path, []byte(strings.Join(h.lines, "\n")+"\n"), 0o600); err != nil {
			return nil
		}
	}
	f, err := os.OpenFile(path, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0o600)
	if err != nil {
		return nil
	}
	f.Close()
	return h
}

// append records an accepted line, skipping empty lines and a repeat of
// the previous one (the same rule the editor applies in memory).
func (h *historyFile) append(line string) {
	if h == nil || line == "" || (len(h.lines) > 0 && h.lines[len(h.lines)-1] == line) {
		return
	}
	h.lines = append(h.lines, line)
	f, err := os.OpenFile(h.path, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0o600)
	if err != nil {
		return
	}
	f.WriteString(line + "\n")
	f.Close()
}
