// Package golden parses and checks golden test files: fung scripts whose
// expected output and expected error are written in comments.
//
//	print(1 + 2)
//	# expect: 3
//
//	print(fiels)
//	# expect-error: E0204
//
// Every `# expect:` line contributes one line of expected standard output,
// in file order. At most one `# expect-error:` line names the error number
// that must appear on standard error; without it the script must succeed
// with nothing on standard error.
package golden

import (
	"bufio"
	"bytes"
	"fmt"
	"regexp"
	"strconv"
	"strings"

	"github.com/tunarider/fung/internal/diag"
)

// Case is what a golden file expects.
type Case struct {
	Path      string
	Stdout    string   // expected standard output, newline-terminated lines
	ErrorCode string   // expected error number such as "E0204", or "" for success
	Args      []string // `# args: a b`: script arguments
	Stdin     string   // `# stdin: line` lines, each newline-terminated
	ExitCode  int      // `# expect-exit: n` for a successful run (default 0)
}

// Result is what actually happened when the script ran.
type Result struct {
	Stdout   string
	Stderr   string
	ExitCode int
}

const (
	expectPrefix      = "# expect:"
	expectErrorPrefix = "# expect-error:"
	expectExitPrefix  = "# expect-exit:"
	argsPrefix        = "# args:"
	stdinPrefix       = "# stdin:"
)

var errorCodeRE = regexp.MustCompile(`^E\d{4}$`)

// Parse reads the expectations out of a golden file's source.
func Parse(path string, src []byte) (Case, error) {
	c := Case{Path: path}
	var out strings.Builder
	sc := bufio.NewScanner(bytes.NewReader(src))
	lineNo := 0
	for sc.Scan() {
		lineNo++
		line := strings.TrimSpace(sc.Text())
		switch {
		case strings.HasPrefix(line, expectErrorPrefix):
			code := strings.TrimSpace(strings.TrimPrefix(line, expectErrorPrefix))
			if !errorCodeRE.MatchString(code) {
				return c, fmt.Errorf("%s:%d: malformed error code %q (want like E0204)", path, lineNo, code)
			}
			if c.ErrorCode != "" {
				return c, fmt.Errorf("%s:%d: second expect-error line (already expecting %s)", path, lineNo, c.ErrorCode)
			}
			c.ErrorCode = code
		case strings.HasPrefix(line, expectExitPrefix):
			n, err := strconv.Atoi(strings.TrimSpace(strings.TrimPrefix(line, expectExitPrefix)))
			if err != nil {
				return c, fmt.Errorf("%s:%d: malformed exit code in %q", path, lineNo, line)
			}
			c.ExitCode = n
		case strings.HasPrefix(line, argsPrefix):
			c.Args = append(c.Args, strings.Fields(strings.TrimPrefix(line, argsPrefix))...)
		case strings.HasPrefix(line, stdinPrefix):
			rest := strings.TrimPrefix(strings.TrimPrefix(line, stdinPrefix), " ")
			c.Stdin += rest + "\n"
		case strings.HasPrefix(line, expectPrefix):
			rest := strings.TrimPrefix(line, expectPrefix)
			rest = strings.TrimPrefix(rest, " ")
			out.WriteString(rest)
			out.WriteByte('\n')
		}
	}
	if err := sc.Err(); err != nil {
		return c, fmt.Errorf("%s: %w", path, err)
	}
	if c.ErrorCode != "" && c.ExitCode != 0 {
		return c, fmt.Errorf("%s: expect-exit and expect-error cannot be combined", path)
	}
	c.Stdout = out.String()
	return c, nil
}

// Check compares an actual run against the expectations and returns a
// descriptive error on the first mismatch.
func Check(c Case, r Result) error {
	if r.ExitCode == diag.ExitUsage && c.ExitCode != diag.ExitUsage {
		return fmt.Errorf("fung could not start (exit %d)\nstderr:\n%s", r.ExitCode, r.Stderr)
	}
	if r.Stdout != c.Stdout {
		return fmt.Errorf("stdout mismatch\nwant:\n%s\ngot:\n%s", c.Stdout, r.Stdout)
	}
	if c.ErrorCode == "" {
		if r.ExitCode != c.ExitCode {
			return fmt.Errorf("exit code %d, want %d\nstderr:\n%s", r.ExitCode, c.ExitCode, r.Stderr)
		}
		if r.Stderr != "" {
			return fmt.Errorf("unexpected stderr output:\n%s", r.Stderr)
		}
		return nil
	}
	if r.ExitCode == diag.ExitOK {
		return fmt.Errorf("expected error %s but the script succeeded", c.ErrorCode)
	}
	if !strings.Contains(r.Stderr, "error["+c.ErrorCode+"]") {
		return fmt.Errorf("expected error %s not reported\nstderr:\n%s", c.ErrorCode, r.Stderr)
	}
	return nil
}
