#!/bin/sh
# Build release archives for every supported platform.
#
#   scripts/build-release.sh 0.1.0 [outdir]
#
# Writes fung_<version>_<os>_<arch>.tar.gz and SHA256SUMS into outdir
# (default: dist). Each archive holds the fung binary, LICENSE and README.md.
# Works the same on a laptop and in GitHub Actions.
set -eu

version=${1:?usage: build-release.sh <version> [outdir]}
version=${version#v}
out=${2:-dist}
targets="linux/amd64 linux/arm64 darwin/amd64 darwin/arm64"

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
rm -rf "$out"
mkdir -p "$out"

for t in $targets; do
	os=${t%/*}
	arch=${t#*/}
	name="fung_${version}_${os}_${arch}"
	stage="$out/$name"
	mkdir -p "$stage"
	# CGO off: a static binary with no C library dependency.
	# -trimpath: no local paths in the binary; -s -w: no debug symbols.
	CGO_ENABLED=0 GOOS=$os GOARCH=$arch go build -trimpath \
		-ldflags "-s -w -X main.linkedVersion=$version" \
		-o "$stage/fung" ./cmd/fung
	cp LICENSE README.md "$stage/"
	tar -C "$out" -czf "$out/$name.tar.gz" "$name"
	rm -rf "$stage"
	echo "built $out/$name.tar.gz"
done

cd "$out"
if command -v sha256sum >/dev/null 2>&1; then
	sha256sum ./*.tar.gz | sed 's| \./| |' >SHA256SUMS
else
	shasum -a 256 ./*.tar.gz | sed 's| \./| |' >SHA256SUMS
fi
echo "wrote $out/SHA256SUMS"
