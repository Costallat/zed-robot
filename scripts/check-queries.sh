#!/usr/bin/env bash
# Checks every tree-sitter query in languages/robot against the grammar
# revision pinned in extension.toml. Needs `tree-sitter` (tree-sitter-cli),
# git and a C compiler.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
manifest="$root/extension.toml"
repo="$(sed -n '/^\[grammars.robot\]/,/^\[/s/^repository = "\(.*\)"/\1/p' "$manifest")"
rev="$(sed -n '/^\[grammars.robot\]/,/^\[/s/^rev = "\(.*\)"/\1/p' "$manifest")"

grammar="$root/target/grammar-robot"
if [ "$(git -C "$grammar" rev-parse HEAD 2>/dev/null)" != "$rev" ]; then
  rm -rf "$grammar"
  git init -q "$grammar"
  git -C "$grammar" fetch -q --depth 1 "$repo" "$rev"
  git -C "$grammar" checkout -q FETCH_HEAD
fi

fixtures=("$root"/tests/fixtures/*.robot)
status=0
cd "$grammar"
for query in "$root"/languages/robot/*.scm; do
  if tree-sitter query "$query" "${fixtures[@]}" > /dev/null 2> "$grammar/query.log"; then
    echo "ok    $(basename "$query")"
  else
    echo "FAIL  $(basename "$query")"
    cat "$grammar/query.log"
    status=1
  fi
done

if tree-sitter parse --quiet "${fixtures[@]}" > /dev/null; then
  echo "ok    fixtures parse without errors"
else
  echo "FAIL  fixtures contain parse errors"
  status=1
fi
exit $status
