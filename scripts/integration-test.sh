#!/usr/bin/env bash
# Recreates the extension's automatic setup and checks it end to end:
#   1. a virtualenv with only Robot Framework (like a user's project),
#   2. the RobotCode release package (.vsix) the extension downloads,
#   3. Robocop installed with `pip --target`, without Robot Framework,
#   4. tests/integration/smoke.py against that setup.
#
#   scripts/integration-test.sh [ROBOTCODE_VERSION]   (default: latest release)
#
# Needs python3 (3.10+), curl and unzip. Set GITHUB_TOKEN to avoid API rate limits.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
work="${INTEGRATION_WORKDIR:-$root/target/integration}"
rm -rf "$work"
mkdir -p "$work/project"

auth=()
if [ -n "${GITHUB_TOKEN:-}" ]; then
  auth=(-H "Authorization: Bearer $GITHUB_TOKEN")
fi

version="${1:-}"
if [ -z "$version" ]; then
  version="$(curl -fsSL "${auth[@]}" https://api.github.com/repos/robotcodedev/robotcode/releases/latest |
    python3 -c 'import json, sys; print(json.load(sys.stdin)["tag_name"])')"
fi
version="v${version#v}"
echo "RobotCode $version"

curl -fsSL -o "$work/robotcode.vsix" \
  "https://github.com/robotcodedev/robotcode/releases/download/$version/robotcode-${version#v}.vsix"
unzip -q "$work/robotcode.vsix" -d "$work/robotcode"
launcher="$work/robotcode/extension/bundled/tool/robotcode"

python3 -m venv "$work/project/.venv"
python="$work/project/.venv/bin/python"
"$python" -m pip install --quiet --disable-pip-version-check robotframework

robocop="$work/robocop"
"$python" -m pip install --quiet --disable-pip-version-check --target "$robocop" robotframework-robocop
rm -rf "$robocop/robot" "$robocop"/robotframework-* "$robocop/bin"

PYTHONPATH="$robocop" "$python" "$root/tests/integration/smoke.py" "$python" "$launcher" "$work/project"
