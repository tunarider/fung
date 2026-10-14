#!/bin/sh
# Print one version's notes from a Keep a Changelog file.
#
#   scripts/changelog-section.sh 0.1.0 [CHANGELOG.md]
#
# Prints the lines between "## [0.1.0]" and the next "## " heading (or the
# link list at the end), without surrounding blank lines. Fails when the
# version has no section or the section is empty, so a release never goes
# out without notes.
set -eu

version=${1:?usage: changelog-section.sh <version> [file]}
version=${version#v}
file=${2:-CHANGELOG.md}

notes=$(awk -v v="$version" '
	index($0, "## [" v "]") == 1 { on = 1; next }
	on && (/^## / || /^\[[^]]+\]: /) { exit }
	on { print }
' "$file" | sed -e '/./,$!d')
# $(...) drops trailing newlines; leading blank lines were dropped by sed.
[ -n "$notes" ] || {
	echo "changelog-section: no notes for $version in $file" >&2
	exit 1
}
printf '%s\n' "$notes"
