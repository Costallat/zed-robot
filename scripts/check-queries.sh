#!/usr/bin/env bash
# Checks the grammar in tree-sitter-robot/ and every query in languages/robot:
#   - the committed parser (src/) is up to date with grammar.js,
#   - the grammar's corpus tests and highlight tests (test/highlight) pass,
#   - every query compiles against the grammar,
#   - the sample files in tests/fixtures parse without errors,
#   - the other queries capture what they should (tests/queries/check_captures.py),
#   - every snippet expands to code that parses (tests/queries/check_snippets.py),
#   - every highlight name is one Zed themes style (tests/queries/check_theme_names.py).
# Needs `tree-sitter` (tree-sitter-cli), node, python3 and a C compiler.
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

check "grammar corpus and highlight tests" tree-sitter test

fixtures=("$root"/tests/fixtures/*.robot "$root"/tests/fixtures/*.resource)
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

for script in check_captures check_snippets check_theme_names; do
  if ! python3 "$root/tests/queries/$script.py" > "$log" 2>&1; then
    grep -v "^ok" "$log" || true
    echo "FAIL  tests/queries/$script.py"
    status=1
  else
    echo "ok    tests/queries/$script.py ($(grep -c "^ok" "$log") checks)"
  fi
done

exit $status
