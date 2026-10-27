#!/usr/bin/env bash
# Checks that Cargo.toml and extension.toml carry the same version and that
# CHANGELOG.md has a section for it.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
version_of() { sed -n 's/^version = "\(.*\)"$/\1/p' "$1" | head -n 1; }

extension="$(version_of "$root/extension.toml")"
cargo="$(version_of "$root/Cargo.toml")"
status=0

if [[ -z "$extension" || "$extension" != "$cargo" ]]; then
  echo "FAIL  extension.toml has version '$extension', Cargo.toml '$cargo': make them the same"
  status=1
else
  echo "ok    extension.toml and Cargo.toml are both at $extension"
fi

if grep -q "^## ${extension//./\\.} " "$root/CHANGELOG.md"; then
  echo "ok    CHANGELOG.md has a section for $extension"
else
  echo "FAIL  CHANGELOG.md has no '## $extension - <date>' section: move the Unreleased notes under it"
  status=1
fi

exit $status
