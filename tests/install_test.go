package tests

import (
	"archive/tar"
	"compress/gzip"
	"crypto/sha256"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

// fakeRelease writes what a GitHub release holds for this platform into
// dir/v<version>/: an archive whose "fung" is a shell script, and SHA256SUMS.
func fakeRelease(t *testing.T, dir, version string, corrupt bool) {
	t.Helper()
	name := fmt.Sprintf("fung_%s_%s_%s", version, runtime.GOOS, runtime.GOARCH)
	rel := filepath.Join(dir, "v"+version)
	if err := os.MkdirAll(rel, 0o755); err != nil {
		t.Fatal(err)
	}
	f, err := os.Create(filepath.Join(rel, name+".tar.gz"))
	if err != nil {
		t.Fatal(err)
	}
	gz := gzip.NewWriter(f)
	tw := tar.NewWriter(gz)
	body := "#!/bin/sh\necho fung " + version + "\n"
	for _, e := range []struct {
		name string
		mode int64
		body string
	}{{name + "/fung", 0o755, body}, {name + "/LICENSE", 0o644, "MIT\n"}} {
		if err := tw.WriteHeader(&tar.Header{Name: e.name, Mode: e.mode, Size: int64(len(e.body))}); err != nil {
			t.Fatal(err)
		}
		tw.Write([]byte(e.body))
	}
	tw.Close()
	gz.Close()
	f.Close()
	data, _ := os.ReadFile(f.Name())
	sum := sha256.Sum256(data)
	if corrupt {
		sum[0] ^= 0xff
	}
	line := fmt.Sprintf("%x  %s.tar.gz\n", sum, name)
	if err := os.WriteFile(filepath.Join(rel, "SHA256SUMS"), []byte(line), 0o644); err != nil {
		t.Fatal(err)
	}
}

func runInstall(t *testing.T, env ...string) (string, error) {
	t.Helper()
	if runtime.GOOS == "windows" {
		t.Skip("install.sh targets Linux and macOS")
	}
	if _, err := exec.LookPath("curl"); err != nil {
		t.Skip("curl not found")
	}
	cmd := exec.Command("sh", "../install.sh")
	cmd.Env = append(os.Environ(), env...)
	out, err := cmd.CombinedOutput()
	return string(out), err
}

func TestInstallScriptInstallsAndVerifies(t *testing.T) {
	rel, bin := t.TempDir(), t.TempDir()
	fakeRelease(t, rel, "9.9.9", false)
	out, err := runInstall(t,
		"FUNG_VERSION=9.9.9",
		"FUNG_INSTALL_DIR="+bin,
		"FUNG_DOWNLOAD_BASE=file://"+rel)
	if err != nil {
		t.Fatalf("install failed: %v\n%s", err, out)
	}
	got, err := exec.Command(filepath.Join(bin, "fung")).Output()
	if err != nil || strings.TrimSpace(string(got)) != "fung 9.9.9" {
		t.Fatalf("installed fung printed %q (%v)\n%s", got, err, out)
	}
	if !strings.Contains(out, "PATH") {
		t.Errorf("a directory not on PATH should be pointed out:\n%s", out)
	}
}

func TestInstallScriptRejectsBadChecksum(t *testing.T) {
	rel, bin := t.TempDir(), t.TempDir()
	fakeRelease(t, rel, "9.9.9", true)
	out, err := runInstall(t,
		"FUNG_VERSION=v9.9.9",
		"FUNG_INSTALL_DIR="+bin,
		"FUNG_DOWNLOAD_BASE=file://"+rel)
	if err == nil {
		t.Fatalf("install succeeded with a bad checksum:\n%s", out)
	}
	if !strings.Contains(out, "checksum") {
		t.Errorf("the error should name the checksum:\n%s", out)
	}
	if _, err := os.Stat(filepath.Join(bin, "fung")); err == nil {
		t.Fatal("a binary with a bad checksum was installed")
	}
}
