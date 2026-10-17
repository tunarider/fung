#!/bin/sh
# Check release archives the way a user receives them.
#
#   TARGETS="linux/amd64 darwin/arm64" scripts/check-release.sh 0.2.0 [dir]
#
# For every target in TARGETS (default: all four), dir (default: dist)
# must hold fung_<version>_<os>_<arch>.tar.gz and nothing else, and:
#   - the archive holds exactly fung, LICENSE and README.md in one folder;
#   - SHA256SUMS lists each archive once, with the right hash;
#   - a Linux fung is statically linked (no PT_INTERP, no NEEDED entries);
#   - a fung this machine can run prints "fung <version>" for `fung version`
#     and runs a program that spawns tasks (the Rust `conc` module).
set -eu

version=${1:?usage: check-release.sh <version> [dir]}
version=${version#v}
dir=${2:-dist}
targets=${TARGETS:-linux/amd64 linux/arm64 darwin/amd64 darwin/arm64}

fail() {
	echo "check-release: $*" >&2
	exit 1
}

case $(uname -s) in
Linux) host_os=linux ;;
Darwin) host_os=darwin ;;
*) host_os=other ;;
esac
case $(uname -m) in
x86_64 | amd64) host_arch=amd64 ;;
aarch64 | arm64) host_arch=arm64 ;;
*) host_arch=other ;;
esac

sha256() {
	if command -v sha256sum >/dev/null 2>&1; then
		sha256sum "$1" | cut -d' ' -f1
	else
		shasum -a 256 "$1" | cut -d' ' -f1
	fi
}

# The command prefix that runs a binary built for $1/$2 here, or nothing
# (exit 1) when this machine cannot run it.
runner() {
	[ "$1" = "$host_os" ] || return 1
	if [ "$2" = "$host_arch" ]; then
		echo env
		return 0
	fi
	# Apple silicon runs x86_64 binaries through Rosetta when it is installed.
	if [ "$1/$2" = darwin/amd64 ] && arch -x86_64 /usr/bin/true 2>/dev/null; then
		echo "arch -x86_64"
		return 0
	fi
	return 1
}

# No PT_INTERP (no dynamic loader) and no NEEDED (no shared libraries).
check_static() {
	if command -v readelf >/dev/null 2>&1; then
		if readelf -lW "$1" | grep -q 'INTERP'; then
			fail "$2: has a dynamic loader (PT_INTERP)"
		fi
		if readelf -dW "$1" | grep -q '(NEEDED)'; then
			readelf -dW "$1" | grep '(NEEDED)' >&2
			fail "$2: needs shared libraries"
		fi
	elif command -v file >/dev/null 2>&1; then
		case $(file -b "$1") in
		*"statically linked"* | *"static-pie linked"*) ;;
		*) fail "$2: not statically linked: $(file -b "$1")" ;;
		esac
	else
		fail "$2: need readelf or file to check static linking"
	fi
	echo "  static: yes"
}

[ -d "$dir" ] || fail "no directory $dir"
[ -f "$dir/SHA256SUMS" ] || fail "no $dir/SHA256SUMS"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

want=""
for t in $targets; do
	os=${t%/*}
	arch=${t#*/}
	name="fung_${version}_${os}_${arch}"
	want="$want $name.tar.gz"
	archive="$dir/$name.tar.gz"
	echo "$name.tar.gz"
	[ -f "$archive" ] || fail "missing $archive"

	# Contents: one folder named like the archive with three files.
	got=$(tar -tzf "$archive" | sed 's|/$||' | grep -vx "$name" | sort | tr '\n' ' ')
	expect="$name/LICENSE $name/README.md $name/fung "
	[ "$got" = "$expect" ] || fail "$name.tar.gz holds [$got], want [$expect]"

	# Checksum: exactly one line for this archive, and it matches.
	lines=$(grep -c " $name.tar.gz\$" "$dir/SHA256SUMS" || true)
	[ "$lines" = 1 ] || fail "SHA256SUMS has $lines lines for $name.tar.gz, want 1"
	listed=$(grep " $name.tar.gz\$" "$dir/SHA256SUMS" | cut -d' ' -f1)
	[ "$listed" = "$(sha256 "$archive")" ] || fail "SHA256SUMS hash for $name.tar.gz does not match"
	echo "  sha256: ok"

	mkdir "$tmp/$name"
	tar -xzf "$archive" -C "$tmp/$name"
	bin="$tmp/$name/$name/fung"
	[ -x "$bin" ] || fail "$name: fung is not executable"

	if [ "$os" = linux ]; then
		check_static "$bin" "$name"
	fi

	if run=$(runner "$os" "$arch"); then
		out=$($run "$bin" version 2>&1) || fail "$name: fung version failed: $out"
		[ "$out" = "fung $version" ] || fail "$name: fung version printed '$out', want 'fung $version'"
		out=$($run "$bin" -e 'import conc; print(conc.pmap([1, 2, 3], x => x * 2))' 2>&1) ||
			fail "$name: the conc.pmap program failed: $out"
		[ "$out" = "[2, 4, 6]" ] || fail "$name: conc.pmap printed '$out', want '[2, 4, 6]'"
		echo "  runs: fung $version"
	else
		echo "  runs: skipped (cannot run $os/$arch here)"
	fi
done

# Nothing else: no archive for a target that was not asked for, and no
# SHA256SUMS line for an archive that is not there.
for f in "$dir"/*.tar.gz; do
	case " $want " in
	*" ${f##*/} "*) ;;
	*) fail "unexpected archive $f" ;;
	esac
done
while read -r _ f; do
	case " $want " in
	*" $f "*) ;;
	*) fail "SHA256SUMS lists $f, which was not built" ;;
	esac
done <"$dir/SHA256SUMS"
echo "checked $dir: $(echo $want | wc -w | tr -d ' ') archives"
