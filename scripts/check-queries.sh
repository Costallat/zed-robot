#!/usr/bin/env bash
# Checks the grammar in tree-sitter-robot/ and every query in languages/robot:
#   - the committed parser (src/) is up to date with grammar.js,
#   - the grammar's own corpus tests pass,
#   - every query compiles against the grammar,
#   - the sample files in tests/fixtures parse without errors.
# Needs `tree-sitter` (tree-sitter-cli), node and a C compiler.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
grammar="$root/tree-sitter-robot"
log="$(mktemp)"
trap 'rm -f "$log"' EXIT
status=0

check() {
  local label="$1"
  shift
  if "$@" > /dev/null 2> "$log"; then
    echo "ok    $label"
  else
    echo "FAIL  $label"
    grep -v "parser directories\|tree-sitter init-config\|configuration file to indicate\|language grammars" "$log" || true
    status=1
  fi
}

cd "$grammar"

check "parser is generated from grammar.js" tree-sitter generate
if ! git -C "$root" diff --quiet -- tree-sitter-robot/src; then
  echo "FAIL  tree-sitter-robot/src is out of date: run 'tree-sitter generate' in tree-sitter-robot/ and commit the result"
  git -C "$root" diff --stat -- tree-sitter-robot/src
  status=1
fi

check "grammar corpus tests" tree-sitter test

fixtures=("$root"/tests/fixtures/*.robot)
for query in "$root"/languages/robot/*.scm; do
  check "query $(basename "$query")" tree-sitter query "$query" "${fixtures[@]}"
done

if tree-sitter parse --quiet "${fixtures[@]}" > /dev/null 2>&1; then
  echo "ok    fixtures parse without errors"
else
  echo "FAIL  fixtures contain parse errors:"
  tree-sitter parse --quiet "${fixtures[@]}" 2> /dev/null || true
  status=1
fi

exit $status
