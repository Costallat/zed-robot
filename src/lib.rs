use std::fs;

use zed_extension_api::{
    self as zed, serde_json, settings::LspSettings, LanguageServerId,
    LanguageServerInstallationStatus, Result,
};

const SERVER_ID: &str = "robotcode";
const ROBOTCODE_REPO: &str = "robotcodedev/robotcode";
const SERVER_ARGS: [&str; 2] = ["language-server", "--stdio"];
const VENV_DIRS: [&str; 3] = [".venv", "venv", "env"];
/// Path of RobotCode's launcher inside its VS Code package (`.vsix`). The
/// package bundles RobotCode and its pure-Python dependencies; it runs on any
/// interpreter that has Robot Framework installed.
const BUNDLED_LAUNCHER: &str = "extension/bundled/tool/robotcode";
/// Entries removed from the managed Robocop install: Robot Framework must come
/// from the project's own environment, and the console scripts are not needed.
fn is_excluded_from_robocop(name: &str) -> bool {
    name == "robot" || name == "bin" || name.starts_with("robotframework-")
}

/// Extension-specific options, read from `lsp.robotcode.settings`.
struct Options {
    /// Interpreter used to run the managed RobotCode.
    python: Option<String>,
    /// Install Robocop next to the managed RobotCode.
    install_robocop: bool,
}

impl Options {
    fn from_settings(settings: Option<&serde_json::Value>) -> Self {
        let get = |key: &str| settings.and_then(|s| s.get(key));
        Self {
            python: get("python").and_then(|v| v.as_str()).map(String::from),
            install_robocop: get("install_robocop")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
        }
    }
}

/// A virtualenv found at the worktree root.
struct Venv {
    python: String,
    robotcode: Option<String>,
}

#[derive(Default)]
struct RobotFrameworkExtension {
    /// Version and directory (relative to the extension's work dir) of the
    /// managed RobotCode, once downloaded.
    bundle: Option<(String, String)>,
}

impl RobotFrameworkExtension {
    /// Returns `(version, absolute launcher path)` of the managed RobotCode,
    /// downloading the latest release when it is not present yet.
    fn managed_robotcode(&mut self, id: &LanguageServerId) -> Result<(String, String)> {
        if let Some((version, dir)) = &self.bundle {
            if fs::metadata(format!("{dir}/{BUNDLED_LAUNCHER}")).is_ok() {
                return Ok((
                    version.clone(),
                    absolute(&format!("{dir}/{BUNDLED_LAUNCHER}"))?,
                ));
            }
        }

        zed::set_language_server_installation_status(
            id,
            &LanguageServerInstallationStatus::CheckingForUpdate,
        );
        let release = zed::latest_github_release(
            ROBOTCODE_REPO,
            zed::GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        );

        let (version, dir) = match release {
            Ok(release) => {
                let dir = format!("robotcode-{}", release.version);
                if fs::metadata(format!("{dir}/{BUNDLED_LAUNCHER}")).is_err() {
                    let asset = release
                        .assets
                        .iter()
                        .find(|asset| asset.name.ends_with(".vsix"))
                        .ok_or_else(|| {
                            format!("RobotCode {} has no .vsix release asset", release.version)
                        })?;
                    zed::set_language_server_installation_status(
                        id,
                        &LanguageServerInstallationStatus::Downloading,
                    );
                    zed::download_file(&asset.download_url, &dir, zed::DownloadedFileType::Zip)
                        .map_err(|err| format!("failed to download RobotCode: {err}"))?;
                    remove_other_versions("robotcode-", &dir);
                }
                (release.version, dir)
            }
            // Offline: fall back to a previously downloaded version.
            Err(err) => newest_dir("robotcode-")
                .filter(|dir| fs::metadata(format!("{dir}/{BUNDLED_LAUNCHER}")).is_ok())
                .map(|dir| (dir.trim_start_matches("robotcode-").to_string(), dir))
                .ok_or_else(|| format!("failed to look up the latest RobotCode release: {err}"))?,
        };

        zed::set_language_server_installation_status(id, &LanguageServerInstallationStatus::None);
        let launcher = absolute(&format!("{dir}/{BUNDLED_LAUNCHER}"))?;
        self.bundle = Some((version.clone(), dir));
        Ok((version, launcher))
    }

    /// Installs Robocop for `python` into a directory owned by the extension
    /// and returns its absolute path. Robot Framework itself is left out so the
    /// project's own version is used.
    fn managed_robocop(
        &self,
        id: &LanguageServerId,
        python: &str,
        robotcode_version: &str,
        worktree: &zed::Worktree,
        env: &[(String, String)],
    ) -> Result<String> {
        let prefix = format!("robocop-{robotcode_version}-");
        let dir = format!("{prefix}{}", sanitize(python));
        let target = absolute(&dir)?;
        if fs::metadata(format!("{dir}/robocop")).is_ok() {
            return Ok(target);
        }

        zed::set_language_server_installation_status(
            id,
            &LanguageServerInstallationStatus::Downloading,
        );
        fs::remove_dir_all(&dir).ok();

        let pip_args = [
            "install",
            "--disable-pip-version-check",
            "--quiet",
            "--target",
            &target,
            "robotframework-robocop",
        ];
        let mut result = zed::process::Command::new(python)
            .args(["-m", "pip"])
            .args(pip_args)
            .envs(env.iter().cloned())
            .output();
        // Virtualenvs created by uv have no pip; use uv itself when available.
        if !succeeded(&result) {
            if let Some(uv) = worktree.which("uv") {
                result = zed::process::Command::new(uv)
                    .args(["pip", pip_args[0], "--python", python])
                    .args(pip_args[3..].iter().copied())
                    .envs(env.iter().cloned())
                    .output();
            }
        }

        zed::set_language_server_installation_status(id, &LanguageServerInstallationStatus::None);
        match result {
            Ok(output) if output.status == Some(0) => {}
            Ok(output) => {
                fs::remove_dir_all(&dir).ok();
                return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
            }
            Err(err) => {
                fs::remove_dir_all(&dir).ok();
                return Err(err);
            }
        }

        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if is_excluded_from_robocop(&name) {
                let path = entry.path();
                fs::remove_dir_all(&path)
                    .or_else(|_| fs::remove_file(&path))
                    .ok();
            }
        }
        remove_other_versions("robocop-", &dir);
        Ok(target)
    }
}

impl zed::Extension for RobotFrameworkExtension {
    fn new() -> Self {
        Self::default()
    }

    /// Picks how to launch RobotCode, in order of preference:
    /// 1. `lsp.robotcode.binary.path` from the user's settings,
    /// 2. `robotcode` installed in the project's virtualenv,
    /// 3. `robotcode` on the `$PATH` (only when the project has no virtualenv),
    /// 4. RobotCode downloaded by the extension, run with the project's Python,
    ///    plus Robocop installed by the extension.
    fn language_server_command(
        &mut self,
        id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let settings = LspSettings::for_worktree(SERVER_ID, worktree).unwrap_or_default();
        let options = Options::from_settings(settings.settings.as_ref());
        let binary = settings.binary.unwrap_or(zed::settings::CommandSettings {
            path: None,
            arguments: None,
            env: None,
        });
        let server_args = || {
            SERVER_ARGS
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        };

        let mut env = worktree.shell_env();
        if let Some(extra) = binary.env {
            env.extend(extra);
        }

        if let Some(path) = binary.path {
            let args = binary.arguments.unwrap_or_else(server_args);
            return Ok(zed::Command {
                command: path,
                args,
                env,
            });
        }

        let venv = find_venv(worktree);
        let installed = match &venv {
            Some(venv) => venv.robotcode.clone(),
            None => worktree.which("robotcode"),
        };
        if let Some(robotcode) = installed {
            let args = binary.arguments.unwrap_or_else(server_args);
            return Ok(zed::Command {
                command: robotcode,
                args,
                env,
            });
        }

        let python = options
            .python
            .or(venv.map(|venv| venv.python))
            .or_else(|| worktree.which("python3"))
            .or_else(|| worktree.which("python"))
            .ok_or(
                "No Python interpreter found for RobotCode. Create a virtualenv in the project \
                 (with Robot Framework installed) or set `lsp.robotcode.settings.python`.",
            )?;

        let (version, launcher) = self.managed_robotcode(id)?;

        if options.install_robocop {
            match self.managed_robocop(id, &python, &version, worktree, &env) {
                Ok(dir) => prepend_python_path(&mut env, &dir),
                // Linting and formatting are optional; keep the language server running.
                Err(err) => eprintln!("robot-framework: could not install Robocop: {err}"),
            }
        }

        let mut args = vec![launcher];
        args.extend(binary.arguments.unwrap_or_else(server_args));
        Ok(zed::Command {
            command: python,
            args,
            env,
        })
    }

    fn language_server_initialization_options(
        &mut self,
        _id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<serde_json::Value>> {
        Ok(LspSettings::for_worktree(SERVER_ID, worktree)
            .ok()
            .and_then(|s| s.initialization_options))
    }

    fn language_server_workspace_configuration(
        &mut self,
        _id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<serde_json::Value>> {
        let settings = LspSettings::for_worktree(SERVER_ID, worktree)
            .ok()
            .and_then(|s| s.settings)
            .unwrap_or_else(|| serde_json::json!({ "robotcode": {} }));
        Ok(Some(add_dotted_sections(&settings)))
    }
}

/// Finds a virtualenv at the worktree root.
fn find_venv(worktree: &zed::Worktree) -> Option<Venv> {
    let (os, _) = zed::current_platform();
    let root = worktree.root_path();
    VENV_DIRS.iter().find_map(|dir| {
        worktree.read_text_file(&format!("{dir}/pyvenv.cfg")).ok()?;
        Some(match os {
            // On Windows the `robotcode` entry point is an .exe that cannot be
            // detected from here; the managed RobotCode runs on the venv instead.
            zed::Os::Windows => Venv {
                python: format!("{root}\\{dir}\\Scripts\\python.exe"),
                robotcode: None,
            },
            _ => Venv {
                python: format!("{root}/{dir}/bin/python"),
                robotcode: worktree
                    .read_text_file(&format!("{dir}/bin/robotcode"))
                    .ok()
                    .map(|_| format!("{root}/{dir}/bin/robotcode")),
            },
        })
    })
}

fn succeeded(result: &Result<zed::process::Output>) -> bool {
    matches!(result, Ok(output) if output.status == Some(0))
}

fn prepend_python_path(env: &mut Vec<(String, String)>, dir: &str) {
    let separator = match zed::current_platform().0 {
        zed::Os::Windows => ";",
        _ => ":",
    };
    match env.iter_mut().find(|(key, _)| key == "PYTHONPATH") {
        Some((_, value)) if !value.is_empty() => *value = format!("{dir}{separator}{value}"),
        Some((_, value)) => *value = dir.to_string(),
        None => env.push(("PYTHONPATH".into(), dir.to_string())),
    }
}

fn absolute(path: &str) -> Result<String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    Ok(cwd.join(path).to_string_lossy().to_string())
}

/// Turns an interpreter path into a directory-name-safe key.
fn sanitize(path: &str) -> String {
    path.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

fn newest_dir(prefix: &str) -> Option<String> {
    let mut dirs: Vec<String> = fs::read_dir(".")
        .ok()?
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.starts_with(prefix))
        .collect();
    dirs.sort();
    dirs.pop()
}

/// Removes directories starting with `prefix` that belong to another version
/// than `keep`.
fn remove_other_versions(prefix: &str, keep: &str) {
    let keep_version = keep
        .trim_start_matches(prefix)
        .split('-')
        .next()
        .unwrap_or("");
    let Ok(entries) = fs::read_dir(".") else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(rest) = name.strip_prefix(prefix) else {
            continue;
        };
        if rest.split('-').next() != Some(keep_version) {
            fs::remove_dir_all(entry.path()).ok();
        }
    }
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

    #[test]
    fn options_default_to_installing_robocop() {
        let options = Options::from_settings(None);
        assert!(options.python.is_none());
        assert!(options.install_robocop);

        let settings =
            serde_json::json!({ "python": "/usr/bin/python3", "install_robocop": false });
        let options = Options::from_settings(Some(&settings));
        assert_eq!(options.python.as_deref(), Some("/usr/bin/python3"));
        assert!(!options.install_robocop);
    }

    #[test]
    fn robocop_cleanup_keeps_robocop_itself() {
        assert!(is_excluded_from_robocop("robot"));
        assert!(is_excluded_from_robocop("robotframework-7.5.dist-info"));
        assert!(!is_excluded_from_robocop(
            "robotframework_robocop-9.1.0.dist-info"
        ));
        assert!(!is_excluded_from_robocop("robocop"));
    }

    #[test]
    fn sanitize_makes_directory_names() {
        assert_eq!(sanitize("/work/.venv/bin/python"), "work__venv_bin_python");
        assert_eq!(sanitize("C:\\py\\python.exe"), "C__py_python_exe");
    }
}
