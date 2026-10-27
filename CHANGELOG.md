# Changelog

All notable changes to the Robot Framework extension for Zed. Versions are
the ones in `extension.toml`; see [Releasing](README.md#releasing).

## Unreleased

## 0.5.0 - 2026-10-27

### Added

- Escape sequences (`\n`, `\${not_a_variable}`, `\#`, `\x41`, ...) are
  highlighted, in a grammar node of their own.
- Named arguments: the name in `timeout=10s`, `level=INFO` or a dictionary
  item `name=bob` is highlighted.
- Python highlighting in `IF`, `ELSE IF` and `WHILE` conditions written with
  `$name` variables (`IF    $count > 3`).
- `Library`, `Resource` and `Variables` imports (also translated) in the
  outline panel.
- Completion items and symbols are colored like the code, and keyword
  completions show their library.

### Fixed

- `${result} =    Keyword    args`: the keyword after an assignment is
  parsed and highlighted as a keyword call (it was plain text). Assignments to
  `@{list}`, `&{dict}`, `${dict}[key]` and several variables at once parse too.
- Text in a `*** Comments ***` section no longer needs `#` to parse.
- A `$`, `@`, `&` or `%` at the end of an argument (`prompt=$`) no longer
  swallows the line break and the line after it.
- `${{ ... }}` with parentheses inside was injected as Python more than once.
- Offline, the newest downloaded RobotCode is picked by version number
  (`v2.10.0` after `v2.9.0`), not alphabetically.
- Quotes typed inside an argument or a comment (`don't`) are no longer
  auto-closed.
- The run button next to a test runs exactly that test, also when its name
  contains `*`, `?` or `[...]`.
- `robot: open log.html` / `report.html` find the results in `robot.toml`'s
  `output-dir`, and work on Windows.

### Tests

- Highlight tests (`tree-sitter-robot/test/highlight/`), checks of the other
  queries' captures and of every snippet (`tests/queries/`), the shipped tasks
  run end to end (`tests/integration/tasks.py`), more fixtures (`.resource`,
  `*** Tasks ***`, `*** Comments ***`, translated files) and a Rust test of
  debugging the run button's task.
- Completion items checked against the format `labels.rs` reads; debugging
  one test from the run button end to end; Robocop-formatted samples must
  still parse; highlight names must exist in Zed's themes; incremental parsing
  fuzzed; Robot Framework's own test suites parsed against a baseline;
  translations checked against the latest Robot Framework; the tasks run on
  macOS and Windows (PowerShell 5.1 and 7) in CI.

### Documentation

- `robot.toml` schema for every file of a project through `.taplo.toml`.
- Release steps and this changelog; pushing a `v*` tag publishes the GitHub
  release with the version's changelog notes.

## 0.4.1 - 2026-10-23

- Renato Foot maintains the extension; repository moved to
  `Costallat/zed-robot`.
- README: RobotCode settings described in Zed terms; `code_lens` belongs in
  user settings.

## 0.4.0 - 2026-10-13

- Semantic highlighting from RobotCode (built-in keywords, library prefixes,
  BDD prefixes, embedded and named arguments) with `semantic_tokens`.
- Python debugging of keyword libraries (`attachPython` / `debug_python`).
- `diagnostic_mode` setting to report problems for the whole workspace.
- The project's Python is checked (Robot Framework installed, Python 3.10 or
  newer) before the downloaded RobotCode starts.
- Tasks to open `log.html` / `report.html`, or serve them for dev containers.
- Grammar: item access (`${list}[0]`), `Given`/`When`/`Then` prefixes,
  `BREAK`/`CONTINUE` in inline `IF`, translated section headers and settings.

## 0.3.2 - 2026-10-08

- Use the activated virtualenv (`VIRTUAL_ENV`) for RobotCode.

## 0.3.1 - 2026-10-08

- Library import timeout raised to 30 seconds (`library_load_timeout`), and
  a `log_level` setting.
- Tasks that show why keywords from a library are missing.
- README troubleshooting for unresolved imports.

## 0.3.0 - 2026-10-05

- The grammar lives in this repository (`tree-sitter-robot/`): nested and
  environment variables, `VAR`, `GROUP`, keyword `[Setup]`.
- Debugging through RobotCode's debug adapter, also from tasks and run buttons.
- Tasks for a test, a suite, a folder, all tests and a dry run.
- Snippets.
- `robotcode_version` setting; Robocop install failures show in the status bar.
- Fix: RobotCode published no diagnostics, because of Zed's default language ID.

## 0.2.0 - 2026-10-02

- RobotCode is downloaded and Robocop installed automatically when the
  project does not have them.
- `python` and `install_robocop` settings; CI.

## 0.1.0 - 2026-10-01

- RobotCode language server with Robocop linting and formatting.
- Refreshed highlights, brackets, outline, text objects, runnables, folds and
  indents; `.resource` files.

## 0.0.1 - 2025-11-26

- First version: syntax highlighting with Hubro/tree-sitter-robot.
