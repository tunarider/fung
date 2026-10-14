#!/bin/sh
# Install fung from a GitHub release.
#
#   curl -fsSL https://raw.githubusercontent.com/tunarider/fung/main/install.sh | sh
#
# Environment:
#   FUNG_VERSION        version to install, e.g. 0.1.0 (default: latest release)
#   FUNG_INSTALL_DIR    where to put the binary (default: ~/.local/bin)
#   FUNG_DOWNLOAD_BASE  release download base (default: GitHub; for testing)
#
# The archive is checked against the release's SHA256SUMS before anything
# is installed. Supported: Linux and macOS on amd64 and arm64.
set -eu

repo="tunarider/fung"
base=${FUNG_DOWNLOAD_BASE:-https://github.com/$repo/releases/download}
dir=${FUNG_INSTALL_DIR:-$HOME/.local/bin}

fail() {
	echo "fung install: $*" >&2
	exit 1
}

command -v curl >/dev/null 2>&1 || fail "curl is required"
command -v tar >/dev/null 2>&1 || fail "tar is required"

case $(uname -s) in
Linux) os=linux ;;
Darwin) os=darwin ;;
*) fail "unsupported OS $(uname -s) (Linux and macOS only)" ;;
esac
case $(uname -m) in
x86_64 | amd64) arch=amd64 ;;
aarch64 | arm64) arch=arm64 ;;
*) fail "unsupported CPU $(uname -m) (amd64 and arm64 only)" ;;
esac

version=${FUNG_VERSION:-}
if [ -z "$version" ]; then
	# The latest-release page redirects to .../tag/v<version>; no API token needed.
	url=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$repo/releases/latest") ||
		fail "cannot reach GitHub to find the latest release"
	version=${url##*/}
	case $version in
	v[0-9]*) ;;
	*) fail "no release found at https://github.com/$repo/releases" ;;
	esac
fi
version=${version#v}

name="fung_${version}_${os}_${arch}"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

echo "downloading fung $version for $os/$arch"
curl -fsSL -o "$tmp/$name.tar.gz" "$base/v$version/$name.tar.gz" ||
	fail "download failed: $base/v$version/$name.tar.gz"
curl -fsSL -o "$tmp/SHA256SUMS" "$base/v$version/SHA256SUMS" ||
	fail "download failed: $base/v$version/SHA256SUMS"

want=$(awk -v f="$name.tar.gz" '$2 == f { print $1 }' "$tmp/SHA256SUMS")
[ -n "$want" ] || fail "$name.tar.gz is not listed in SHA256SUMS"
if command -v sha256sum >/dev/null 2>&1; then
	got=$(sha256sum "$tmp/$name.tar.gz" | awk '{ print $1 }')
else
	got=$(shasum -a 256 "$tmp/$name.tar.gz" | awk '{ print $1 }')
fi
[ "$got" = "$want" ] || fail "checksum mismatch for $name.tar.gz (want $want, got $got); nothing installed"

tar -xzf "$tmp/$name.tar.gz" -C "$tmp"
mkdir -p "$dir"
# Copy then rename so a running fung is never left half-written.
cp "$tmp/$name/fung" "$dir/.fung.new"
chmod 755 "$dir/.fung.new"
mv -f "$dir/.fung.new" "$dir/fung"
echo "installed $dir/fung"

case ":$PATH:" in
*":$dir:"*) "$dir/fung" version ;;
*)
	echo
	echo "$dir is not on your PATH. Add this line to your shell profile:"
	echo "  export PATH=\"$dir:\$PATH\""
	;;
esac
