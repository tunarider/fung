package tests

import (
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func changelogSection(t *testing.T, file, version string) (string, error) {
	t.Helper()
	if runtime.GOOS == "windows" {
		t.Skip("the release scripts target Linux and macOS")
	}
	out, err := exec.Command("sh", "../scripts/changelog-section.sh", version, file).CombinedOutput()
	return string(out), err
}

func TestChangelogSectionExtractsOneVersion(t *testing.T) {
	f := filepath.Join(t.TempDir(), "CHANGELOG.md")
	os.WriteFile(f, []byte(`# Changelog

## [Unreleased]

- pending

## [0.2.0] - 2026-11-01

### Added
- two

## [0.1.0] - 2026-10-14

- one

[0.2.0]: https://example.com/0.2.0
`), 0o644)
	got, err := changelogSection(t, f, "v0.2.0")
	if err != nil {
		t.Fatalf("%v: %s", err, got)
	}
	if want := "### Added\n- two\n"; got != want {
		t.Fatalf("section = %q, want %q", got, want)
	}
	if _, err := changelogSection(t, f, "0.3.0"); err == nil {
		t.Fatal("a version with no section must fail the release")
	}
}

// The real changelog must have a section for the version in the manual,
// so a release tag never ships with empty notes.
func TestChangelogCoversTheManualVersion(t *testing.T) {
	page, err := os.ReadFile("../docs/index.html")
	if err != nil {
		t.Fatal(err)
	}
	_, rest, ok := strings.Cut(string(page), `<p class="side-foot">fung `)
	if !ok {
		t.Fatal("no version in the manual sidebar")
	}
	version, _, _ := strings.Cut(rest, " ")
	got, err := changelogSection(t, "../CHANGELOG.md", version)
	if err != nil || strings.TrimSpace(got) == "" {
		t.Fatalf("CHANGELOG.md has no notes for %s: %v %s", version, err, got)
	}
}
