#!/bin/sh
# Write dir/SHA256SUMS for every release archive in dir.
#
#   scripts/sha256sums.sh [dir]
#
# One "<hash>  <file>" line per fung_*.tar.gz, the format `sha256sum -c`
# and install.sh read. The release workflow builds each platform on its own
# runner and runs this once over all the archives.
set -eu

dir=${1:-dist}
cd "$dir"
set -- ./*.tar.gz
[ -f "$1" ] || {
	echo "sha256sums: no .tar.gz in $dir" >&2
	exit 1
}
if command -v sha256sum >/dev/null 2>&1; then
	sha256sum "$@" | sed 's| \./| |' >SHA256SUMS
else
	shasum -a 256 "$@" | sed 's| \./| |' >SHA256SUMS
fi
echo "wrote $dir/SHA256SUMS"
