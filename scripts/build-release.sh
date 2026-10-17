#!/bin/sh
# Build release archives of the Rust fung.
#
#   scripts/build-release.sh 0.2.0 [outdir]
#   TARGETS="darwin/arm64" scripts/build-release.sh 0.2.0   # just one
#
# Writes fung_<version>_<os>_<arch>.tar.gz and SHA256SUMS into outdir
# (default: dist). Each archive holds the fung binary, LICENSE and README.md.
# TARGETS (default: all four) picks the platforms:
#
#   linux/amd64   x86_64-unknown-linux-musl    static, runs on any Linux
#   linux/arm64   aarch64-unknown-linux-musl   static, runs on any Linux
#   darwin/amd64  x86_64-apple-darwin
#   darwin/arm64  aarch64-apple-darwin
#
# Each target needs its standard library (`rustup target add <triple>`),
# and a musl target needs a C compiler for musl: `ring` compiles C and
# assembly. On a machine of the same CPU, musl-gcc (Debian/Ubuntu package
# musl-tools) is used unless CC_<triple> is set; cross builds set CC_<triple>
# themselves. The release workflow runs this once per platform runner.
set -eu

version=${1:?usage: build-release.sh <version> [outdir]}
version=${version#v}
out=${2:-dist}
targets=${TARGETS:-linux/amd64 linux/arm64 darwin/amd64 darwin/arm64}

fail() {
	echo "build-release: $*" >&2
	exit 1
}

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

case $(uname -m) in
x86_64 | amd64) host_arch=amd64 ;;
aarch64 | arm64) host_arch=arm64 ;;
*) host_arch=other ;;
esac

triple() {
	case $1 in
	linux/amd64) echo x86_64-unknown-linux-musl ;;
	linux/arm64) echo aarch64-unknown-linux-musl ;;
	darwin/amd64) echo x86_64-apple-darwin ;;
	darwin/arm64) echo aarch64-apple-darwin ;;
	*) fail "unknown target $1 (want linux|darwin / amd64|arm64)" ;;
	esac
}

# Check every target before building any, so a missing toolchain fails
# fast instead of after the first builds.
installed=$(rustup target list --installed 2>/dev/null || true)
for t in $targets; do
	tr=$(triple "$t")
	if [ -n "$installed" ] && ! echo "$installed" | grep -qx "$tr"; then
		fail "Rust target $tr is not installed: rustup target add $tr"
	fi
done

# outdir is emptied first; refuse when it holds anything this script did
# not write, so a mistyped path cannot wipe unrelated files.
if [ -e "$out" ]; then
	[ -d "$out" ] || fail "$out exists and is not a directory"
	for f in "$out"/* "$out"/.[!.]* "$out"/..?*; do
		[ -e "$f" ] || [ -L "$f" ] || continue
		case ${f##*/} in
		fung_* | SHA256SUMS) ;;
		*) fail "$out holds $f, which is not a release file; pick an empty or new outdir" ;;
		esac
	done
	rm -rf "$out"
fi
mkdir -p "$out"

# The version `fung version` prints (crates/fung-cli/src/version.rs reads
# it with option_env!; cargo rebuilds when it changes).
export FUNG_BUILD_VERSION="$version"
# No debug symbols (Go's -s -w).
export CARGO_PROFILE_RELEASE_STRIP=symbols
# No local paths in the binary (Go's -trimpath): the checkout and the
# crate sources under CARGO_HOME. With --target, the flags reach only the
# target's code, not build scripts. CARGO_ENCODED_RUSTFLAGS separates
# flags with 0x1f, so a path with spaces stays one flag; cargo then
# ignores RUSTFLAGS, so flags already set there are carried over.
cargo_home=${CARGO_HOME:-$HOME/.cargo}
sep=$(printf '\037')
flags=${CARGO_ENCODED_RUSTFLAGS:-}
if [ -z "$flags" ] && [ -n "${RUSTFLAGS:-}" ]; then
	set -f # split on spaces, but do not expand globs
	for f in $RUSTFLAGS; do
		flags="${flags:+$flags$sep}$f"
	done
	set +f
fi
flags="${flags:+$flags$sep}--remap-path-prefix=$root=/fung$sep--remap-path-prefix=$cargo_home=/cargo"
export CARGO_ENCODED_RUSTFLAGS="$flags"
unset RUSTFLAGS
# macOS tar would otherwise add AppleDouble (._*) files and xattrs.
export COPYFILE_DISABLE=1

for t in $targets; do
	os=${t%/*}
	arch=${t#*/}
	tr=$(triple "$t")
	name="fung_${version}_${os}_${arch}"
	stage="$out/$name"

	if [ "$os" = linux ] && [ "$arch" = "$host_arch" ]; then
		var=CC_$(echo "$tr" | tr - _)
		if eval "[ -z \"\${$var:-}\" ]"; then
			command -v musl-gcc >/dev/null 2>&1 ||
				fail "$tr needs musl-gcc (apt-get install musl-tools) or $var"
			eval "export $var=musl-gcc"
		fi
	fi

	cargo build --release --locked -p fung-cli --target "$tr"
	mkdir -p "$stage"
	cp "${CARGO_TARGET_DIR:-target}/$tr/release/fung" LICENSE README.md "$stage/"
	tar --no-xattrs -C "$out" -czf "$out/$name.tar.gz" "$name"
	rm -rf "$stage"
	echo "built $out/$name.tar.gz"
done

sh scripts/sha256sums.sh "$out"
