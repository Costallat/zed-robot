#!/usr/bin/env bash
# Parses a large set of real Robot Framework files (by default Robot
# Framework's own acceptance tests, atest/) and fails when more of them have
# parse errors than tests/real-world-baseline.txt allows, so grammar changes
# cannot quietly break real code.
#
#   scripts/check-real-world.sh [DIR]   (default: clone robotframework/robotframework)
#
# Needs git, tree-sitter (tree-sitter-cli) and a C compiler. When fewer files
# fail than the baseline, lower the number in the baseline file.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
baseline_file="$root/tests/real-world-baseline.txt"
baseline="$(grep -v '^#' "$baseline_file" | head -n 1)"

dir="${1:-}"
if [ -z "$dir" ]; then
  dir="$root/target/robotframework"
  rm -rf "$dir"
  git clone --quiet --depth 1 https://github.com/robotframework/robotframework.git "$dir"
  dir="$dir/atest"
fi

list="$(mktemp)"
trap 'rm -f "$list"' EXIT
find "$dir" -type f \( -name '*.robot' -o -name '*.resource' \) | sort > "$list"
total="$(wc -l < "$list")"

cd "$root/tree-sitter-robot"
# --quiet prints only files with errors; tree-sitter exits non-zero when any has one.
failed="$(tree-sitter parse --quiet --paths "$list" 2> /dev/null | grep -c "(ERROR\|(MISSING" || true)"

echo "$failed of $total files have parse errors (baseline: $baseline)"
if [ "$failed" -gt "$baseline" ]; then
  echo "FAIL  more files fail to parse than before. Files with errors:"
  tree-sitter parse --quiet --paths "$list" 2> /dev/null | grep "(ERROR\|(MISSING" | head -n 50
  exit 1
fi
if [ "$failed" -lt "$baseline" ]; then
  echo "note  fewer files fail than the baseline: set $baseline_file to $failed"
fi
echo "ok    real-world files parse at least as well as before"
