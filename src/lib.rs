use zed_extension_api::{self as zed, serde_json, settings::LspSettings, LanguageServerId, Result};

const SERVER_ID: &str = "robotcode";
const DEFAULT_ARGS: [&str; 2] = ["language-server", "--stdio"];
const VENV_DIRS: [&str; 3] = [".venv", "venv", "env"];

struct RobotFrameworkExtension;

impl RobotFrameworkExtension {
    /// Finds a way to launch RobotCode, in order of preference:
    /// 1. `lsp.robotcode.binary.path` from the user's settings,
    /// 2. a `robotcode` executable on the worktree's `$PATH`,
    /// 3. the Python interpreter of a virtualenv at the worktree root,
    /// 4. a `python3`/`python` interpreter on the `$PATH`.
    fn server_command(
        &self,
        settings: &LspSettings,
        worktree: &zed::Worktree,
    ) -> Result<(String, Vec<String>)> {
        let default_args = || {
            DEFAULT_ARGS
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        };
        let module_args = || {
            ["-m", "robotcode"]
                .into_iter()
                .chain(DEFAULT_ARGS)
                .map(String::from)
                .collect::<Vec<_>>()
        };
        let user_args = settings.binary.as_ref().and_then(|b| b.arguments.clone());

        if let Some(path) = settings.binary.as_ref().and_then(|b| b.path.clone()) {
            return Ok((path, user_args.unwrap_or_else(default_args)));
        }

        if let Some(path) = worktree.which("robotcode") {
            return Ok((path, user_args.unwrap_or_else(default_args)));
        }

        if let Some(python) = venv_python(worktree) {
            return Ok((python, user_args.unwrap_or_else(module_args)));
        }

        if let Some(python) = worktree
            .which("python3")
            .or_else(|| worktree.which("python"))
        {
            return Ok((python, user_args.unwrap_or_else(module_args)));
        }

        Err(
            "RobotCode language server not found. Install it in your project's Python \
             environment with `pip install \"robotcode[all]\"` (includes Robocop for \
             linting and formatting), or set `lsp.robotcode.binary.path` in your Zed settings."
                .into(),
        )
    }
}

/// Returns the interpreter of a virtualenv located at the worktree root, if any.
fn venv_python(worktree: &zed::Worktree) -> Option<String> {
    let (os, _) = zed::current_platform();
    let root = worktree.root_path();
    VENV_DIRS.iter().find_map(|dir| {
        worktree.read_text_file(&format!("{dir}/pyvenv.cfg")).ok()?;
        Some(match os {
            zed::Os::Windows => format!("{root}\\{dir}\\Scripts\\python.exe"),
            _ => format!("{root}/{dir}/bin/python"),
        })
    })
}

/// RobotCode asks for dotted sections such as `robotcode.robocop`. Expose every
/// nested object under its dotted path as well, so the lookup works whether the
/// client resolves sections by path or by exact key.
fn add_dotted_sections(value: &serde_json::Value) -> serde_json::Value {
    fn collect(
        prefix: &str,
        value: &serde_json::Value,
        out: &mut serde_json::Map<String, serde_json::Value>,
    ) {
        if let serde_json::Value::Object(map) = value {
            for (key, child) in map {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                if child.is_object() {
                    out.entry(path.clone()).or_insert_with(|| child.clone());
                    collect(&path, child, out);
                }
            }
        }
    }

    let mut out = match value {
        serde_json::Value::Object(map) => map.clone(),
        _ => serde_json::Map::new(),
    };
    collect("", value, &mut out);
    serde_json::Value::Object(out)
}

impl zed::Extension for RobotFrameworkExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let settings = LspSettings::for_worktree(SERVER_ID, worktree).unwrap_or_default();
        let (command, args) = self.server_command(&settings, worktree)?;

        let mut env = worktree.shell_env();
        if let Some(extra) = settings.binary.and_then(|b| b.env) {
            env.extend(extra);
        }

        Ok(zed::Command { command, args, env })
    }

    fn language_server_initialization_options(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<serde_json::Value>> {
        Ok(LspSettings::for_worktree(SERVER_ID, worktree)
            .ok()
            .and_then(|s| s.initialization_options))
    }

    fn language_server_workspace_configuration(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<serde_json::Value>> {
        let settings = LspSettings::for_worktree(SERVER_ID, worktree)
            .ok()
            .and_then(|s| s.settings)
            .unwrap_or_else(|| serde_json::json!({ "robotcode": {} }));
        Ok(Some(add_dotted_sections(&settings)))
    }
}

zed::register_extension!(RobotFrameworkExtension);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotted_sections_mirror_nested_objects() {
        let input = serde_json::json!({
            "robotcode": { "robocop": { "enabled": false }, "analysis": { "cache": { "saveLocation": "x" } } }
        });
        let out = add_dotted_sections(&input);
        assert_eq!(out["robotcode"]["robocop"]["enabled"], false);
        assert_eq!(out["robotcode.robocop"]["enabled"], false);
        assert_eq!(out["robotcode.analysis.cache"]["saveLocation"], "x");
    }
}
