"""Expands every snippet in snippets/ with its default values, as accepting it
without typing would, and checks that the grammar parses the result without
errors and finds what the snippet is meant to insert.

    python3 tests/queries/check_snippets.py

Needs the tree-sitter CLI. Exits non-zero if a check fails.
"""

import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TEST = "*** Test Cases ***\nT\n"

# Prefix -> (text placed before the snippet, indentation of its lines, node it must produce).
CONTEXT = {
    "*** Settings": ("", "", "settings_section"),
    "*** Variables": ("", "", "variable_definition"),
    "*** Test Cases": ("", "", "test_case_definition"),
    "*** Tasks": ("", "", "test_case_definition"),
    "*** Keywords": ("", "", "keyword_definition"),
    "test": ("*** Test Cases ***\n", "", "test_case_definition"),
    "keyword": ("*** Keywords ***\n", "", "keyword_definition"),
    "for": (TEST, "    ", "for_statement"),
    "forrange": (TEST, "    ", "in_range"),
    "forenumerate": (TEST, "    ", "in_enumerate"),
    "while": (TEST, "    ", "while_statement"),
    "if": (TEST, "    ", "if_statement"),
    "ifelse": (TEST, "    ", "else_statement"),
    "ifelseif": (TEST, "    ", "elseif_statement"),
    "try": (TEST, "    ", "finally_statement"),
    "var": (TEST, "    ", "var_statement"),
    "group": (TEST, "    ", "group_statement"),
    "setup": (TEST, "    ", "test_case_setting"),
    "assign": (TEST, "    ", "keyword_invocation"),
}


def expand(body):
    """Replaces tabstops and placeholders with their defaults (the first
    choice of `${1|a,b|}`); a bare `$0` / `$1` becomes a keyword call."""
    out, i = [], 0
    while i < len(body):
        if body.startswith("\\$", i):
            out.append("$")
            i += 2
        elif body.startswith("${", i) and re.match(r"\$\{\d+[:|]", body[i:]):
            depth, j = 1, i + 2
            while depth:
                if body.startswith("\\$", j):
                    j += 2
                    continue
                depth += {"{": 1, "}": -1}.get(body[j], 0)
                j += 1
            inner = body[i + 2:j - 1]
            number, sep, rest = re.match(r"(\d+)([:|])(.*)", inner, re.S).groups()
            out.append(rest.rstrip("|").split(",")[0] if sep == "|" else expand(rest))
            i = j
        elif m := re.match(r"\$\d+", body[i:]):
            out.append("No Operation")
            i += len(m.group())
        else:
            out.append(body[i])
            i += 1
    return "".join(out)


def parse(source):
    with tempfile.NamedTemporaryFile("w", suffix=".robot", delete=False, encoding="utf-8") as f:
        f.write(source)
    try:
        return subprocess.run(["tree-sitter", "parse", f.name], cwd=ROOT / "tree-sitter-robot",
                              capture_output=True, text=True).stdout
    finally:
        Path(f.name).unlink()


failures = 0
for path in sorted((ROOT / "snippets").glob("*.json")):
    for name, snippet in json.loads(path.read_text(encoding="utf-8")).items():
        prefix = snippet["prefix"]
        if prefix not in CONTEXT:
            print(f"FAIL  snippet `{prefix}` has no entry in CONTEXT in {Path(__file__).name}")
            failures += 1
            continue
        before, indent, node = CONTEXT[prefix]
        # A tabstop alone on its line is where typing continues: leave it empty.
        lines = ["" if re.fullmatch(r"\$\d+", line) else line for line in snippet["body"]]
        body = "\n".join(indent + line if line else line for line in expand("\n".join(lines)).split("\n"))
        tree = parse(before + body + "\n")
        problems = [p for p in ("ERROR", "MISSING") if p in tree]
        ok = not problems and f"({node} " in tree
        print(f"{'ok  ' if ok else 'FAIL'}  snippet `{prefix}` parses as {node}")
        if not ok:
            print("      " + (before + body).replace("\n", "\n      "))
            print("      " + tree.replace("\n", "\n      "))
        failures += not ok

sys.exit(1 if failures else 0)
