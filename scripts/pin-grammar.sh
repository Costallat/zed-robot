#!/usr/bin/env bash
# Points extension.toml at the latest commit that changed tree-sitter-robot/.
# Zed fetches the grammar from this repository at that commit, so run this
# after committing (and pushing) a grammar change, then commit extension.toml.
#
#   scripts/pin-grammar.sh          update extension.toml
#   scripts/pin-grammar.sh --check  only verify it is up to date (used by CI)
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
manifest="$root/extension.toml"
pinned="$(sed -n '/^\[grammars.robot\]/,/^\[/s/^rev = "\(.*\)"/\1/p' "$manifest")"

if ! git -C "$root" diff --quiet HEAD -- tree-sitter-robot; then
  echo "tree-sitter-robot/ has uncommitted changes; commit them first." >&2
  exit 1
fi

# The grammar at the pinned commit must be identical to the current one.
if [ -n "$pinned" ] && git -C "$root" cat-file -e "$pinned^{commit}" 2> /dev/null &&
  git -C "$root" diff --quiet "$pinned" HEAD -- tree-sitter-robot; then
  echo "ok    extension.toml grammar rev $pinned matches tree-sitter-robot/"
  exit 0
fi

if [ "${1:-}" = "--check" ]; then
  echo "FAIL  extension.toml points at grammar rev '$pinned', which differs from tree-sitter-robot/ in HEAD." >&2
  echo "      Run scripts/pin-grammar.sh and commit extension.toml." >&2
  exit 1
fi

latest="$(git -C "$root" log -1 --format=%H -- tree-sitter-robot)"
sed -i.bak "/^\[grammars.robot\]/,/^\[/s/^rev = \".*\"/rev = \"$latest\"/" "$manifest"
rm -f "$manifest.bak"
echo "extension.toml now pins the grammar at $latest"
echo "Push that commit before installing the extension: Zed fetches the grammar from GitHub."
