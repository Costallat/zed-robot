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

## Requirements

The language server, linter and formatter are Python tools. Install them into
the same Python environment as your Robot Framework libraries, so RobotCode can
resolve your keywords:

```sh
pip install "robotcode[all]"   # RobotCode, plus Robocop for linting and formatting
```

The extension starts the language server by looking, in this order, for:

1. `lsp.robotcode.binary.path` in your Zed settings,
2. a `robotcode` executable on your `PATH` (e.g. an activated virtualenv or `direnv`),
3. a virtualenv in the project root (`.venv`, `venv` or `env`), via `python -m robotcode`,
4. `python3` / `python` on your `PATH`, via `python -m robotcode`.

## Configuration

### Language server

Settings under `lsp.robotcode.settings` are sent to RobotCode. They use the
same names as the [RobotCode VS Code settings](https://robotcode.io/03_reference/config),
for example:

```jsonc
// settings.json
{
  "lsp": {
    "robotcode": {
      // Optional: use a specific executable
      // "binary": { "path": "/path/to/venv/bin/robotcode", "arguments": ["language-server", "--stdio"] },
      "settings": {
        "robotcode": {
          "robot": {
            "pythonPath": ["./lib"],
            "variables": { "ENV": "dev" }
          },
          "robocop": {
            "enabled": true
          },
          "analysis": {
            "diagnosticMode": "openFilesOnly"
          }
        }
      }
    }
  }
}
```

RobotCode also reads project settings from `robot.toml` / `pyproject.toml`.

### Linting and formatting rules

Robocop is configured in the project, through `pyproject.toml` or `robocop.toml`:

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

## Known limitations

The tree-sitter grammar does not yet model some newer syntax, such as the
`VAR` and `GROUP` statements or keyword-level `[Setup]`. This extension
highlights `VAR`/`GROUP`/`END` by name, but those blocks cannot be folded, and
keyword `[Setup]` lines show up as parse errors in the syntax tree. Code
intelligence and diagnostics from RobotCode are not affected.

## Development

```sh
rustup target add wasm32-wasip2
cargo build --release --target wasm32-wasip2
cargo test
```

Then in Zed run *zed: install dev extension* and pick this directory.

### Structure

- `extension.toml`: extension manifest (grammar and language server)
- `src/lib.rs`: finds and launches the RobotCode language server
- `languages/robot/`: language configuration and tree-sitter queries
  (`highlights`, `folds`, `indents`, `brackets`, `outline`, `textobjects`,
  `injections`, `runnables`)

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
