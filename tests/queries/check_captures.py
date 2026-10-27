"""Checks what the queries in languages/robot capture on small Robot files:
outline, injections, run buttons, auto-close scopes, folds, indents and
text objects. (Highlights have their own tests in
tree-sitter-robot/test/highlight, run by `tree-sitter test`.)

    python3 tests/queries/check_captures.py

Needs the tree-sitter CLI and runs it in tree-sitter-robot/. Exits non-zero
if a check fails.
"""

import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
QUERIES = ROOT / "languages" / "robot"
CAPTURE = re.compile(r"capture: (?:\d+ - )?([\w.]+), start: \((\d+), \d+\), end: \((\d+), \d+\)(?:, text: `(.*)`)?$")
failures = 0


def captures(query, source):
    """(capture name, text) pairs, plus (name, first row, last row) for spans."""
    with tempfile.NamedTemporaryFile("w", suffix=".robot", delete=False, encoding="utf-8") as f:
        f.write(source)
    try:
        out = subprocess.run(
            ["tree-sitter", "query", str(QUERIES / query), f.name],
            cwd=ROOT / "tree-sitter-robot", capture_output=True, text=True, check=True,
        ).stdout
    finally:
        Path(f.name).unlink()
    found = []
    for line in out.splitlines():
        m = CAPTURE.search(line.strip())
        if m:
            name, start, end, text = m.groups()
            found.append((name, text, int(start), int(end)))
    return found


def check(name, ok, detail=""):
    global failures
    print(f"{'ok  ' if ok else 'FAIL'}  {name}" + ("" if ok else f"\n      {detail}"))
    failures += not ok


def expect(query, source, present=(), absent=(), rows=()):
    """`present` / `absent`: (capture, text) pairs; `rows`: (capture, first, last)."""
    found = captures(query, source)
    pairs = {(name, text) for name, text, _, _ in found}
    spans = {(name, start, end) for name, _, start, end in found}
    for item in present:
        check(f"{query}: {item[0]} `{item[1]}`", item in pairs, f"captured: {sorted(pairs, key=str)}")
    for item in absent:
        check(f"{query}: no {item[0]} `{item[1]}`", item not in pairs, f"captured: {sorted(pairs, key=str)}")
    for item in rows:
        check(f"{query}: {item[0]} on rows {item[1]}-{item[2]}", item in spans, f"captured: {sorted(spans, key=str)}")


TEST = "*** Test Cases ***\nT\n"

# Outline: sections, tests, keywords, variables and imports (also translated).
expect("outline.scm",
       "*** Settings ***\nLibrary    SSHLibrary    timeout=1\nResource    common.resource\n"
       "Variables    vars.py\nDocumentation    not an import\n\n"
       "*** Variables ***\n${HOST}    example.com\n\n"
       "*** Test Cases ***\nLogin Works\n    No Operation\n\n"
       "*** Keywords ***\nLog In\n    No Operation\n",
       present=[("name", "*** Settings ***"), ("name", "SSHLibrary"), ("context", "Library"),
                ("name", "common.resource"), ("name", "vars.py"), ("name", "${HOST}"),
                ("name", "Login Works"), ("name", "Log In")],
       absent=[("name", "not an import"), ("name", "timeout=1")])
expect("outline.scm",
       "Language: Brazilian Portuguese\n\n*** Configurações ***\nBiblioteca    Collections\n",
       present=[("context", "Biblioteca"), ("name", "Collections")])

# Python in ${{ }} and in conditions written with $name, not with ${name}.
expect("injections.scm",
       TEST + "    Log    ${{ len($items) + 1 }}\n"
       "    IF    $count > 3 and $name != 'x'\n        No Operation\n"
       "    ELSE IF    ${count} == 2\n        No Operation\n    END\n"
       "    WHILE    $i < 3    limit=5\n        BREAK\n    END\n"
       "    IF    $done    RETURN\n",
       present=[("injection.content", " len($items) + 1 "),
                ("injection.content", "$count > 3 and $name != 'x'"),
                ("injection.content", "$i < 3"), ("injection.content", "$done")],
       absent=[("injection.content", "${count} == 2"), ("injection.content", "limit=5"),
               ("injection.content", "$items")])

# Run buttons on each test and on the section header, nowhere else.
expect("runnables.scm",
       "*** Test Cases ***\nFirst Test\n    No Operation\nSecond [smoke]\n    No Operation\n\n"
       "*** Keywords ***\nNot A Test\n    No Operation\n",
       present=[("run", "First Test"), ("robot_test_name", "Second [smoke]"),
                ("run", "*** Test Cases ***")],
       absent=[("run", "Not A Test"), ("run", "*** Keywords ***")])
expect("runnables.scm", "*** Tasks ***\nProcess\n    No Operation\n",
       present=[("run", "Process"), ("run", "*** Tasks ***")])

# Scopes for config.toml's `not_in`: no auto-closed quotes inside text or comments.
expect("overrides.scm",
       TEST + "    Log    don't    ${{ 'a' }}    # it's\n",
       present=[("string", "don't"), ("string", "'a'"), ("comment", "    # it's")])

# Folds and indents: sections, tests, keywords and every block with END.
expect("folds.scm",
       TEST + "    FOR    ${i}    IN RANGE    3\n        IF    $i\n            Log    x\n        END\n    END\n",
       rows=[("fold", 0, 7), ("fold", 1, 7), ("fold", 2, 6), ("fold", 3, 5)])
expect("indents.scm",
       TEST + "    TRY\n        Fail    x\n    EXCEPT\n        No Operation\n    END\n",
       present=[("end", "END")])

# Text objects: keywords and tests are functions, sections are classes.
expect("textobjects.scm",
       "*** Keywords ***\nLog In\n    No Operation\n",
       present=[("class.around", None)], rows=[("function.around", 1, 3), ("function.inside", 2, 3)])

# The code src/labels.rs wraps completion items and symbols in, so they get
# the colors below in the completion menu and symbol search.
expect("highlights.scm", TEST + "    Should Be Equal\n", present=[("function.call", "Should Be Equal")])
expect("highlights.scm", TEST + "    Log    ${user}\n", present=[("variable", "${user}")])
expect("highlights.scm", TEST + "    Log    timeout=x\n",
       present=[("property", "timeout"), ("operator", "=")])
expect("highlights.scm", "*** Settings ***\n", present=[("title", "*** Settings ***")])
expect("highlights.scm", "*** Keywords ***\nLog In\n", present=[("function", "Log In")])
expect("highlights.scm", "*** Test Cases ***\nLogin Works\n", present=[("function", "Login Works")])
expect("highlights.scm", "*** Variables ***\n${HOST}    x\n", present=[("variable", "${HOST}")])

sys.exit(1 if failures else 0)
