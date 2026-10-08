# zed-robot

A [Zed](https://zed.dev) extension for [Robot Framework](https://robotframework.org/).

## Features

- **Syntax highlighting** with a tree-sitter grammar kept in this repository
  ([`tree-sitter-robot/`](tree-sitter-robot), based on
  [Hubro/tree-sitter-robot](https://github.com/Hubro/tree-sitter-robot)):
  sections, settings, test cases, tasks, keywords, `FOR`/`WHILE`/`IF`/`TRY`/`RETURN`,
  the Robot Framework 7 `VAR`, `GROUP` and keyword `[Setup]` syntax, nested
  variables (`${devices.${type}.ip}`), environment variables (`%{HOME}`) and
  inline Python `${{ ... }}` (highlighted as Python).
- **Language server**: [RobotCode](https://robotcode.io): completion, hover
  docs, go to definition, find references, rename, signature help, inlay hints,
  and diagnostics from Robot Framework's own analysis.
- **Linting**: [Robocop](https://robocop.dev) rules show up as diagnostics as you type (run by RobotCode).
- **Formatting**: Robocop's formatter (formerly *Robotidy*) through RobotCode,
  so *Format Document* and format-on-save work.
- **Debugging**: breakpoints, stepping through keywords and inspecting variables
  in Zed's debugger, through RobotCode's debug adapter.
- **Run buttons and tasks**: run or debug a single test, a whole suite, a
  folder or everything (see [Running and debugging tests](#running-and-debugging-tests)).
- **Snippets** for sections, tests, keywords and control structures (see [Snippets](#snippets)).
- **Editor structure**: code folding, outline panel and breadcrumbs, auto-indent,
  bracket matching, and function/class text objects for keywords, tests and sections.

Files with the `.robot` and `.resource` extensions are recognised.

## Setup

### What you need

- **Python 3.10 or newer with Robot Framework installed**, in the environment
  you run your tests with. Ideally that is a virtualenv in the project root
  (`.venv`, `venv` or `env`); the extension finds it automatically.
- Nothing else. The extension downloads and runs RobotCode and Robocop by itself (see below).

```sh
python3 -m venv .venv
.venv/bin/pip install robotframework   # plus your test libraries, e.g. robotframework-sshlibrary
```

### What the extension installs for you

When a Robot Framework file is opened and RobotCode is not installed in the
project, the extension:

1. downloads the latest [RobotCode release](https://github.com/robotcodedev/robotcode/releases)
   (the same package the VS Code extension uses) into its own folder,
2. runs it with the project's Python, so RobotCode sees your Robot Framework
   version and your libraries,
3. installs [Robocop](https://robocop.dev) for that Python with `pip` (or `uv`,
   for virtualenvs without pip) into the extension's own folder. Your project's
   environment is **not** modified.

Both are refreshed when a new RobotCode version is released. Everything lives
in Zed's extension work folder (on Linux,
`~/.local/share/zed/extensions/work/robot-framework/`). Delete that folder to
force a fresh install.

In a **dev container or remote project**, all of this happens inside the
container or remote machine, next to your code.

### How the language server is chosen

1. `lsp.robotcode.binary.path` in your Zed settings, if set.
2. `robotcode` installed in the project's virtualenv
   (`pip install "robotcode[all]"`, which includes Robocop).
3. `robotcode` on your `PATH`, only when the project has no virtualenv.
4. Otherwise, the RobotCode downloaded by the extension, run with the Python
   from `lsp.robotcode.settings.python`, the project's virtualenv, or
   `python3`/`python` on your `PATH`.

### Settings for the automatic setup

```jsonc
// settings.json or .zed/settings.json
{
  "lsp": {
    "robotcode": {
      "settings": {
        // Python used to run RobotCode (default: the project's virtualenv, then python3)
        "python": "/path/to/python",
        // Set to false to skip installing Robocop (no linting or formatting)
        "install_robocop": true,
        // Use a specific RobotCode release instead of the latest, e.g. to keep
        // a team on the same version (default: latest)
        "robotcode_version": "2.7.0",
        // Seconds RobotCode may spend importing one library before giving up
        // on it and all its keywords (default: 30; RobotCode's own is 10)
        "library_load_timeout": 30,
        // Turn on RobotCode's log (TRACE, DEBUG, INFO, WARNING, ERROR)
        "log_level": "INFO"
      }
    }
  }
}
```

### Building the extension from source

Only needed for *zed: install dev extension*. On the machine running the Zed
window (the host, not a dev container), install:

- [rustup](https://rustup.rs) (Rust installed another way, e.g. from your
  distro, is not supported by Zed). Zed adds the `wasm32-wasip2` target itself.
- A C compiler and linker (`cc`): `sudo apt install build-essential`
  (Debian/Ubuntu), `sudo dnf install gcc` (Fedora) or `sudo pacman -S base-devel` (Arch).

Then, in a **local** Zed window, run *zed: install dev extension* and pick this
folder. Build errors appear in Zed's log (*zed: open log*).

> **Updating from version 0.2 or older:** the grammar now comes from this
> repository instead of Hubro/tree-sitter-robot. Delete the `grammars/` folder
> in your local copy before reinstalling, or Zed stops with
> "grammar directory ... is not a git clone of ...".

## Project configuration: `robot.toml`

RobotCode does not see the shell you run your tests from: an
`export PYTHONPATH=...` or environment variables set before `robot` are
invisible to it. Without them it cannot find libraries and resources imported
through the Python path, and their keywords get no hover, go-to-definition or
documentation, and show up as `KeywordNotFound`.

Put that setup in a `robot.toml` file at the root of the project (the folder
opened in Zed). RobotCode reads it for analysis, and `robotcode robot` uses it
to run tests:

```toml
# robot.toml
python-path = [          # instead of export PYTHONPATH=...
    "libs",
    "libs/utils",
]

[env]                    # environment variables your libraries need at import time
TEST_ENV = "dev"

[variables]              # global variables, like robot --variable
BROWSER = "chrome"
```

- Paths are relative to the project root. List the **folders** that imports are
  searched in, not individual files. Imports written relative to the importing
  file (`Resource    ../keywords/common.robot`) already work without this.
- Do not list a virtualenv's `site-packages`; run RobotCode on that virtualenv
  instead (see [How the language server is chosen](#how-the-language-server-is-chosen)).
- After changing it, run *editor: restart language server*.

To see what RobotCode cannot resolve, run its analyzer in the project root:

```sh
robotcode analyze code path/to/file.robot | grep -iE "import|DataError"
```

For autocomplete and validation while editing `robot.toml`, add RobotCode's
schema as its first line (read by Zed's TOML language server):

```toml
#:schema https://www.robotcode.io/schemas/robot.toml.json
```

References:
[Configuration guide](https://robotcode.io/02_get_started/configuration) ·
[All `robot.toml` settings](https://robotcode.io/03_reference/config)
(e.g. [`python-path`](https://robotcode.io/03_reference/config#python-path),
[`env`](https://robotcode.io/03_reference/config#env)).

## Configuration

### Language server

Settings under `lsp.robotcode.settings.robotcode` are sent to RobotCode. They
use the names of RobotCode's VS Code settings, nested by their dots
(`robotcode.analysis.diagnosticMode` becomes `"robotcode": { "analysis": { "diagnosticMode": ... } }`),
for example:

```jsonc
{
  "lsp": {
    "robotcode": {
      // Optional: use a specific executable
      // "binary": { "path": "/path/to/venv/bin/robotcode", "arguments": ["language-server", "--stdio"] },
      "settings": {
        "robotcode": {
          "robocop": { "enabled": true },
          "analysis": { "diagnosticMode": "openFilesOnly" }
        }
      }
    }
  }
}
```

Project-level settings (paths, variables, environment) are better kept in
[`robot.toml`](#project-configuration-robottoml), so they also apply on the
command line and for the rest of your team.

### Linting and formatting rules

Robocop is configured in the project, through `pyproject.toml` or `robocop.toml`
([Robocop configuration](https://robocop.dev)):

```toml
[tool.robocop.lint]
select = ["DOC*", "NAME*"]
ignore = ["DOC03"]
configure = ["line-too-long.line_length=140"]

[tool.robocop.format]
space_count = 4
line_length = 120
```

To turn off format-on-save for Robot files only:

```jsonc
{
  "languages": {
    "Robot Framework": {
      "format_on_save": "off"
    }
  }
}
```

## Running and debugging tests

The extension comes with tasks (*task: spawn*) and run buttons in the gutter:

| Where | Task |
| --- | --- |
| Next to a test case or task | `robot: test <name>`: runs that test |
| Next to `*** Test Cases ***` / `*** Tasks ***` | `robot: suite <file>`: runs the file |
| Task list only | `robot: folder <dir>`, `robot: all tests`, `robot: dry run <file>` |

They run `robotcode robot`, which reads [`robot.toml`](#project-configuration-robottoml),
so `python-path`, `[env]` and `[variables]` apply exactly as in the editor.
This needs the RobotCode command line in the project's environment
(`pip install robotcode`), on the `PATH` of Zed's terminal (an activated
virtualenv, or `direnv`).

**Debugging.** Every task above can also run under the debugger: open the run
button's menu, or *debugger: start* and pick the task. Set breakpoints by
clicking next to the line numbers; the debugger stops on keyword calls and
shows the variables of the current test and keyword. Robot Framework's output
goes to the debug console.

For your own debug configurations, add them to `.zed/debug.json`. The
options (the same as RobotCode's VS Code launch configurations) are listed in
[`debug_adapter_schemas/RobotCode.json`](debug_adapter_schemas/RobotCode.json)
and completed while you type:

```jsonc
[
  {
    "label": "Debug smoke tests",
    "adapter": "RobotCode",
    "request": "launch",
    "target": "tests/smoke",       // file, folder or "." for everything
    "include": ["smoke"],          // like robot --include
    "variables": { "ENV": "dev" }, // like robot --variable
    "stopOnEntry": false
  }
]
```

To use your own `tasks.json`, give its Robot tasks the tags `robot-test` (the
test name is in `$ZED_CUSTOM_robot_test_name`) or `robot-suite`. Tasks whose
command is `robot`, `robotcode robot` or `python -m robot` can be debugged too.

## Snippets

Type the prefix and pick the snippet from the completion menu:

| Prefix | Inserts |
| --- | --- |
| `*** Settings`, `*** Variables`, `*** Test Cases`, `*** Tasks`, `*** Keywords` | a section with a first entry |
| `test`, `keyword` | a test case or keyword with documentation |
| `for`, `forrange`, `forenumerate`, `while` | loops |
| `if`, `ifelse`, `ifelseif` | conditionals |
| `try` | `TRY` / `EXCEPT` / `FINALLY` |
| `var`, `group`, `assign`, `setup` | `VAR`, `GROUP`, `${result}=    Keyword`, `[Setup]`/`[Teardown]` |

## Troubleshooting

### A keyword is "not found", or has no hover or go-to-definition

RobotCode reads keywords by importing each library, like Robot Framework
does when tests run. When an import fails, **all** keywords of that library
are missing, and the reason is reported on the `Library` / `Resource` line,
not on the keyword. Hover the underlined import, or open the problems panel
(*diagnostics: deploy*).

Common reasons, and what to do:

| Message on the import line | Cause | Fix |
| --- | --- | --- |
| `Resource file ... does not exist`, `No module named 'mylib'` | The folder holding it is only on the `PYTHONPATH` you export in the shell | Add the folder to `python-path` in [`robot.toml`](#project-configuration-robottoml) |
| `ModuleNotFoundError` for a package your library imports | The package is not installed in the Python RobotCode runs on | Install it in the project's virtualenv, or point `lsp.robotcode.settings.python` at the right interpreter |
| `KeyError`, or another error from your own code at import time | The library needs environment variables or files at import time | Add them under `[env]` in `robot.toml` |
| `Variable '${X}' not found` on `Library    MyLib    ${X}` | Library arguments come from variables only known when tests run | Add them under `[variables]` in `robot.toml` |
| `Loading library ... TimeoutError` | Importing took longer than allowed | Raise `library_load_timeout` (default 30 seconds) |
| `Imported library ... contains no keywords` | A dynamic library (`get_keyword_names`) whose keywords depend on runtime state | Nothing to fix in the editor: those keywords only exist while tests run |

Also check the keyword is visible to Robot Framework: functions whose names
start with `_` are skipped, and with `ROBOT_AUTO_KEYWORDS = False` only
functions marked with `@keyword` are keywords.

**Tasks that show what Robot Framework sees** (*task: spawn*; they need
`pip install robotcode` in the project's environment):

- `robot: check imports of <file>`: every unresolved import and keyword in
  the current file, with the reason.
- `robot: keywords in library file <selection>`: select a library path as
  written in the file (e.g. `../voice_libs/config_libs/ewi_voice.py`) and run
  it to list its keywords, or get the full Python traceback of the import
  error with file and line.
- `robot: keywords in library <selection>`: the same for a library imported
  by name (e.g. `SSHLibrary`).

All of them read `robot.toml`, so they see exactly what the editor sees.

**After editing a Python library**, RobotCode normally picks up the change
when the file is saved. If it keeps showing old keywords, delete the
`.robotcode_cache` folder in the project root and run *editor: restart
language server*. (Add `.robotcode_cache/` to `.gitignore`.)

### Other problems

- **Language server logs**: set `"log_level": "INFO"` (see
  [Settings for the automatic setup](#settings-for-the-automatic-setup)),
  restart the language server, and open *dev: open language server logs* →
  RobotCode. The *RPC Messages* view in the same panel shows each request and
  response.
- **No linting or formatting**: Robocop could not be installed (Zed's status
  bar shows "Robocop could not be installed" with the reason), or
  `install_robocop` is `false`. Installing `robotframework-robocop` in the
  project's environment also works.
- **Tasks fail with `robotcode: command not found`**: install the RobotCode
  command line in the project's environment (`pip install robotcode`) and make
  sure that environment is active in Zed's terminal.
- **Dev containers: the extension does not load at all**: check the Zed log
  (*zed: open log*) for `Failed to install extension ... on the remote ...
  Permission denied`. Files under `~/.local/share/zed` inside the container
  belong to another user (often `root`); fix with
  `sudo chown -R "$(id -u):$(id -g)" ~/.local/share/zed` in the container and
  reopen the window.

## Known limitations

- Item access written after the variable, `${list}[0]` or `${dict}[key]`, is
  highlighted as the variable followed by plain text. The `${list[0]}` form is
  fully supported.
- `BREAK` and `CONTINUE` inside an inline `IF` are parsed as keyword calls
  (they are still highlighted as keywords).

## Development

```sh
rustup target add wasm32-wasip2
cargo build --release --target wasm32-wasip2
cargo test
./scripts/check-queries.sh       # grammar + queries; needs tree-sitter-cli (npm i -g tree-sitter-cli)
./scripts/integration-test.sh    # RobotCode + Robocop + debugger end to end; needs python3, curl, unzip
```

CI (`.github/workflows/ci.yml`) runs all of the above on every push and pull
request, plus weekly to catch new RobotCode or Robocop releases.

### Changing the grammar

The grammar lives in [`tree-sitter-robot/`](tree-sitter-robot). Zed builds it
from GitHub at the commit pinned in `extension.toml`, so a change takes two commits:

1. Edit `tree-sitter-robot/grammar.js`, add a test to `tree-sitter-robot/test/corpus/`,
   then in `tree-sitter-robot/` run `tree-sitter generate` and `tree-sitter test`.
2. Commit (including the regenerated `src/`) and push.
3. Run `scripts/pin-grammar.sh`, commit `extension.toml` and push.

CI fails if `src/` is not regenerated or `extension.toml` points at an older grammar.

### Structure

- `extension.toml`: extension manifest (grammar, language server, debugger, capabilities)
- `src/lib.rs`: extension entry points
- `src/robotcode.rs`: finds, downloads and launches RobotCode; installs Robocop
- `src/debugger.rs`: debug adapter, `debug.json` handling, task-to-debug conversion
- `tree-sitter-robot/`: the grammar
- `languages/robot/`: language configuration, tree-sitter queries and tasks
- `snippets/`: snippets
- `debug_adapter_schemas/RobotCode.json`: options accepted in `debug.json`
- `tests/fixtures/`: sample files the queries are checked against
- `tests/integration/`: end-to-end check of RobotCode as the extension runs it
- `scripts/`: checks used by CI and the grammar pinning script

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
