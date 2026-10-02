# zed-robot

A [Zed](https://zed.dev) extension for [Robot Framework](https://robotframework.org/).

## Features

- **Syntax highlighting** with [tree-sitter-robot](https://github.com/Hubro/tree-sitter-robot):
  sections, settings, test cases, tasks, keywords, variables (including
  `${dict}[key]` access), inline Python `${{ ... }}` (highlighted as Python),
  `FOR`/`WHILE`/`IF`/`TRY`/`RETURN`, and the Robot Framework 7 `VAR` and `GROUP` syntax.
- **Language server**: [RobotCode](https://robotcode.io): completion, hover
  docs, go to definition, find references, rename, signature help, inlay hints,
  and diagnostics from Robot Framework's own analysis.
- **Linting**: [Robocop](https://robocop.dev) rules show up as diagnostics as you type (run by RobotCode).
- **Formatting**: Robocop's formatter (formerly *Robotidy*) through RobotCode,
  so *Format Document* and format-on-save work.
- **Editor structure**: code folding, outline panel and breadcrumbs, auto-indent,
  bracket matching, and function/class text objects for keywords, tests and sections.
- **Run buttons**: a run indicator next to each test case or task (see [Running tests](#running-tests)).

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
        "install_robocop": true
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

## Running tests

Test cases and tasks get a run button in the gutter. They are tagged
`robot-test`, and the test name is exposed as `$ZED_CUSTOM_robot_test_name`.
Add tasks like these to your `tasks.json` (*zed: open tasks*):

```json
[
  {
    "label": "robot: $ZED_CUSTOM_robot_test_name",
    "command": "robot",
    "args": ["--test", "\"$ZED_CUSTOM_robot_test_name\"", "\"$ZED_FILE\""],
    "tags": ["robot-test"]
  },
  {
    "label": "robot: current file",
    "command": "robot",
    "args": ["\"$ZED_FILE\""]
  }
]
```

## Troubleshooting

- **No hover or go-to-definition for some keywords**: an import could not be
  resolved. Check the problems panel for errors on the `Library`/`Resource`
  lines, or run `robotcode analyze code` as shown above, then fix `robot.toml`.
- **Language server logs**: *dev: open language server logs* → RobotCode.
  RobotCode logs very little by default; enable more with:

  ```jsonc
  { "lsp": { "robotcode": { "binary": { "env": { "ROBOTCODE_LOG": "1", "ROBOTCODE_LOG_LEVEL": "INFO" } } } } }
  ```

  The *RPC Messages* view in the same panel shows each request and response.
- **No linting or formatting**: Robocop could not be installed (check Zed's log
  for `could not install Robocop`), or `install_robocop` is `false`. Installing
  `robotframework-robocop` in the project's environment also works.

## Known limitations

The tree-sitter grammar does not yet model some syntax, so the lines below show
as parse errors and can break highlighting around them. Code intelligence and
diagnostics from RobotCode are not affected.

- Nested variables, e.g. `${devices.${name}.ip}` or `${PREFIX_${index}}`.
- The `VAR` and `GROUP` statements and keyword-level `[Setup]`. `VAR`/`GROUP`/`END`
  are highlighted by name, but `GROUP` blocks cannot be folded.

## Development

```sh
rustup target add wasm32-wasip2
cargo build --release --target wasm32-wasip2
cargo test
./scripts/check-queries.sh   # needs tree-sitter-cli (npm i -g tree-sitter-cli)
```

CI (`.github/workflows/ci.yml`) runs formatting, clippy, unit tests and the
WebAssembly build, and checks every tree-sitter query against the grammar
revision pinned in `extension.toml`.

### Structure

- `extension.toml`: extension manifest (grammar, language server, capabilities)
- `src/lib.rs`: finds, installs and launches RobotCode and Robocop
- `languages/robot/`: language configuration and tree-sitter queries
  (`highlights`, `folds`, `indents`, `brackets`, `outline`, `textobjects`,
  `injections`, `runnables`)
- `tests/fixtures/`: sample files the queries are checked against
- `scripts/check-queries.sh`: query check used by CI

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
