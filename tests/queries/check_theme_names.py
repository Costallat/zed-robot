"""Checks that every highlight name the extension uses is one Zed's themes
style, so a typo does not silently leave code uncolored:

  - captures in languages/robot/highlights.scm,
  - highlight names of the literal spans in src/labels.rs,
  - the `style` lists in languages/robot/semantic_token_rules.json (at least
    one name of each list).

Zed looks a name up by its longest known prefix (`function.call` falls back
to `function`), so a name is fine when it or one of its prefixes is known.

    python3 tests/queries/check_theme_names.py
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# The `syntax` keys of Zed's bundled themes (One, Ayu, Gruvbox).
THEME_NAMES = {
    "attribute", "boolean", "comment", "comment.doc", "constant", "constructor",
    "embedded", "emphasis", "emphasis.strong", "enum", "function", "hint",
    "keyword", "label", "link_text", "link_uri", "namespace", "number",
    "operator", "predictive", "preproc", "primary", "property", "punctuation",
    "punctuation.bracket", "punctuation.delimiter", "punctuation.list_marker",
    "punctuation.special", "string", "string.escape", "string.regex",
    "string.special", "string.special.symbol", "tag", "text.literal", "title",
    "type", "variable", "variable.special", "variant",
}

failures = 0


def known(name):
    parts = name.split(".")
    return any(".".join(parts[:i]) in THEME_NAMES for i in range(len(parts), 0, -1))


def check(what, names):
    global failures
    unknown = sorted(n for n in names if not known(n))
    print(f"{'ok  ' if not unknown else 'FAIL'}  {what}" + (f": unknown {unknown}" if unknown else ""))
    failures += bool(unknown)


highlights = (ROOT / "languages" / "robot" / "highlights.scm").read_text(encoding="utf-8")
highlights = re.sub(r";[^\n]*", "", highlights)
check("highlights.scm captures", set(re.findall(r"@([\w.]+)", highlights)))

labels = (ROOT / "src" / "labels.rs").read_text(encoding="utf-8")
labels = labels.split("#[cfg(test)]")[0]
check("src/labels.rs literal highlights",
      set(re.findall(r'literal\([^,]+,\s*"([\w.]+)"\)', labels))
      | set(re.findall(r'Some\("([\w.]+)"\.into\(\)\)', labels)))

rules = (ROOT / "languages" / "robot" / "semantic_token_rules.json").read_text(encoding="utf-8")
rules = json.loads(re.sub(r"(?m)^\s*//.*$", "", rules))
unresolved = [r["token_type"] for r in rules if not any(known(s) for s in r["style"])]
print(f"{'ok  ' if not unresolved else 'FAIL'}  semantic_token_rules.json styles"
      + (f": no known style for {unresolved}" if unresolved else ""))
failures += bool(unresolved)

sys.exit(1 if failures else 0)
