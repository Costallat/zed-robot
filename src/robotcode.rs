//! Finding, downloading and launching RobotCode, and installing Robocop.
//!
//! Shared by the language server and the debug adapter, which are both
//! RobotCode subcommands (`language-server`, `debug-launch`).

use std::fs;

use zed_extension_api::{
    self as zed, serde_json, settings::LspSettings, LanguageServerId,
    LanguageServerInstallationStatus, Result,
};

/// Name of the language server in `extension.toml`, and the key of its
/// settings (`lsp.robotcode`), which also hold the options below.
pub const SERVER_ID: &str = "robotcode";
const ROBOTCODE_REPO: &str = "robotcodedev/robotcode";
const VENV_DIRS: [&str; 3] = [".venv", "venv", "env"];
/// Path of RobotCode's launcher inside its VS Code package (`.vsix`). The
/// package bundles RobotCode and its pure-Python dependencies; it runs on any
/// interpreter that has Robot Framework installed.
const BUNDLED_LAUNCHER: &str = "extension/bundled/tool/robotcode";
/// Seconds RobotCode may spend importing one library or variable file before
/// giving up on it (and on all its keywords). RobotCode's own default is 10,
/// which libraries doing real work at import time (SSH, parsers, config
/// loading) can exceed on a cold start.
const DEFAULT_LIBRARY_LOAD_TIMEOUT: u64 = 30;
const LOAD_TIMEOUT_VAR: &str = "ROBOTCODE_LOAD_LIBRARY_TIMEOUT";

/// Entries removed from the managed Robocop install: Robot Framework must come
/// from the project's own environment, and the console scripts are not needed.
fn is_excluded_from_robocop(name: &str) -> bool {
    name == "robot" || name == "bin" || name.starts_with("robotframework-")
}

/// Extension-specific options, read from `lsp.robotcode.settings`.
pub struct Options {
    /// Interpreter used to run the managed RobotCode.
    pub python: Option<String>,
    /// Install Robocop next to the managed RobotCode.
    pub install_robocop: bool,
    /// RobotCode release to download (e.g. "2.7.0"); latest when unset.
    pub robotcode_version: Option<String>,
    /// Seconds allowed for importing one library (see
    /// `DEFAULT_LIBRARY_LOAD_TIMEOUT`); `None` keeps an existing
    /// `ROBOTCODE_LOAD_LIBRARY_TIMEOUT`, else uses the default.
    pub library_load_timeout: Option<u64>,
    /// RobotCode log level (TRACE, DEBUG, INFO, ...); logging is off when unset.
    pub log_level: Option<String>,
}

impl Options {
    pub fn from_settings(settings: Option<&serde_json::Value>) -> Self {
        let get = |key: &str| settings.and_then(|s| s.get(key));
        Self {
            python: get("python").and_then(|v| v.as_str()).map(String::from),
            install_robocop: get("install_robocop")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            robotcode_version: get("robotcode_version")
                .and_then(|v| v.as_str())
                .map(|v| v.trim_start_matches('v').to_string())
                .filter(|v| !v.is_empty()),
            library_load_timeout: get("library_load_timeout").and_then(|v| v.as_u64()),
            log_level: get("log_level")
                .and_then(|v| v.as_str())
                .map(|v| v.to_uppercase())
                .filter(|v| !v.is_empty()),
        }
    }

    /// Adds the environment variables these options map to.
    fn apply_env(&self, env: &mut Vec<(String, String)>) {
        let has = |env: &Vec<(String, String)>, key: &str| env.iter().any(|(k, _)| k == key);
        match self.library_load_timeout {
            Some(seconds) => set_env(env, LOAD_TIMEOUT_VAR, &seconds.to_string()),
            None if !has(env, LOAD_TIMEOUT_VAR) => set_env(
                env,
                LOAD_TIMEOUT_VAR,
                &DEFAULT_LIBRARY_LOAD_TIMEOUT.to_string(),
            ),
            None => {}
        }
        if let Some(level) = &self.log_level {
            set_env(env, "ROBOTCODE_LOG", "1");
            set_env(env, "ROBOTCODE_LOG_LEVEL", level);
        }
    }
}

fn set_env(env: &mut Vec<(String, String)>, key: &str, value: &str) {
    match env.iter_mut().find(|(k, _)| k == key) {
        Some((_, v)) => *v = value.to_string(),
        None => env.push((key.to_string(), value.to_string())),
    }
}

/// How to start RobotCode: `command` followed by `prefix_args`, then the
/// RobotCode subcommand.
pub struct Launch {
    pub command: String,
    pub prefix_args: Vec<String>,
    /// The project's Python interpreter, when known.
    pub python: Option<String>,
    /// Version of the RobotCode downloaded by the extension, if that is the
    /// one being used.
    pub managed_version: Option<String>,
    pub env: Vec<(String, String)>,
}

/// A virtualenv found at the worktree root.
struct Venv {
    python: String,
    robotcode: Option<String>,
}

#[derive(Default)]
pub struct RobotCode {
    /// Version and directory (relative to the extension's work dir) of the
    /// managed RobotCode, once downloaded.
    bundle: Option<(String, String)>,
}

impl RobotCode {
    /// Picks how to launch RobotCode, in order of preference:
    /// 1. `lsp.robotcode.binary.path` from the user's settings,
    /// 2. `robotcode` installed in the project's virtualenv (the activated
    ///    `VIRTUAL_ENV`, else `.venv`/`venv`/`env` at the worktree root),
    /// 3. `robotcode` on the `$PATH` (only when the project has no virtualenv),
    /// 4. RobotCode downloaded by the extension, run with the project's Python.
    ///
    /// `status` is the language server whose installation progress is shown
    /// in Zed's status bar, if any.
    pub fn launch(
        &mut self,
        worktree: &zed::Worktree,
        status: Option<&LanguageServerId>,
    ) -> Result<Launch> {
        let settings = LspSettings::for_worktree(SERVER_ID, worktree).unwrap_or_default();
        let options = Options::from_settings(settings.settings.as_ref());
        let binary = settings.binary;

        let mut env = worktree.shell_env();
        if let Some(extra) = binary.as_ref().and_then(|b| b.env.clone()) {
            env.extend(extra);
        }
        options.apply_env(&mut env);

        let venv = find_venv(worktree, &env);
        let python = options
            .python
            .clone()
            .or_else(|| venv.as_ref().map(|venv| venv.python.clone()));

        let installed = binary
            .as_ref()
            .and_then(|b| b.path.clone())
            .or_else(|| match &venv {
                Some(venv) => venv.robotcode.clone(),
                None => worktree.which("robotcode"),
            });
        if let Some(command) = installed {
            return Ok(Launch {
                command,
                prefix_args: Vec::new(),
                python,
                managed_version: None,
                env,
            });
        }

        let python = python
            .or_else(|| worktree.which("python3"))
            .or_else(|| worktree.which("python"))
            .ok_or(
                "No Python interpreter found for RobotCode. Create a virtualenv in the project \
                 (with Robot Framework installed) or set `lsp.robotcode.settings.python`.",
            )?;

        let (version, launcher) =
            self.managed_robotcode(options.robotcode_version.as_deref(), status)?;

        if options.install_robocop {
            match managed_robocop(&python, &version, worktree, &env, status) {
                Ok(dir) => prepend_python_path(&mut env, &dir),
                // Linting and formatting are optional; keep RobotCode running
                // but say why they are missing.
                Err(err) => {
                    let message = format!(
                        "Robocop could not be installed, so linting and formatting are off. \
                         Install `robotframework-robocop` in the project's environment, or set \
                         `lsp.robotcode.settings.install_robocop` to false. Error: {err}"
                    );
                    eprintln!("robot-framework: {message}");
                    if let Some(id) = status {
                        zed::set_language_server_installation_status(
                            id,
                            &LanguageServerInstallationStatus::Failed(message),
                        );
                    }
                }
            }
        }

        Ok(Launch {
            command: python.clone(),
            prefix_args: vec![launcher],
            python: Some(python),
            managed_version: Some(version),
            env,
        })
    }

    /// Returns `(version, absolute launcher path)` of the managed RobotCode,
    /// downloading the requested (or latest) release when it is not present.
    fn managed_robotcode(
        &mut self,
        pinned: Option<&str>,
        status: Option<&LanguageServerId>,
    ) -> Result<(String, String)> {
        if let Some((version, dir)) = &self.bundle {
            let wanted = pinned.is_none_or(|pin| version.trim_start_matches('v') == pin);
            if wanted && fs::metadata(format!("{dir}/{BUNDLED_LAUNCHER}")).is_ok() {
                return Ok((
                    version.clone(),
                    absolute(&format!("{dir}/{BUNDLED_LAUNCHER}"))?,
                ));
            }
        }

        set_status(status, LanguageServerInstallationStatus::CheckingForUpdate);
        let release = match pinned {
            Some(version) => {
                zed::github_release_by_tag_name(ROBOTCODE_REPO, &format!("v{version}"))
            }
            None => zed::latest_github_release(
                ROBOTCODE_REPO,
                zed::GithubReleaseOptions {
                    require_assets: true,
                    pre_release: false,
                },
            ),
        };

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
                    set_status(status, LanguageServerInstallationStatus::Downloading);
                    zed::download_file(&asset.download_url, &dir, zed::DownloadedFileType::Zip)
                        .map_err(|err| format!("failed to download RobotCode: {err}"))?;
                    remove_other_versions("robotcode-", &dir);
                }
                (release.version, dir)
            }
            // Offline: fall back to a previously downloaded version.
            Err(err) => newest_dir("robotcode-")
                .filter(|dir| fs::metadata(format!("{dir}/{BUNDLED_LAUNCHER}")).is_ok())
                .filter(|dir| pinned.is_none_or(|pin| dir.trim_start_matches("robotcode-v") == pin))
                .map(|dir| (dir.trim_start_matches("robotcode-").to_string(), dir))
                .ok_or_else(|| match pinned {
                    Some(pin) => format!("failed to find RobotCode release v{pin}: {err}"),
                    None => format!("failed to look up the latest RobotCode release: {err}"),
                })?,
        };

        set_status(status, LanguageServerInstallationStatus::None);
        let launcher = absolute(&format!("{dir}/{BUNDLED_LAUNCHER}"))?;
        self.bundle = Some((version.clone(), dir));
        Ok((version, launcher))
    }
}

/// Installs Robocop for `python` into a directory owned by the extension and
/// returns its absolute path. Robot Framework itself is left out so the
/// project's own version is used.
fn managed_robocop(
    python: &str,
    robotcode_version: &str,
    worktree: &zed::Worktree,
    env: &[(String, String)],
    status: Option<&LanguageServerId>,
) -> Result<String> {
    let prefix = format!("robocop-{robotcode_version}-");
    let dir = format!("{prefix}{}", sanitize(python));
    let target = absolute(&dir)?;
    if fs::metadata(format!("{dir}/robocop")).is_ok() {
        return Ok(target);
    }

    set_status(status, LanguageServerInstallationStatus::Downloading);
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

    set_status(status, LanguageServerInstallationStatus::None);
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

fn set_status(id: Option<&LanguageServerId>, status: LanguageServerInstallationStatus) {
    if let Some(id) = id {
        zed::set_language_server_installation_status(id, &status);
    }
}

/// Finds the project's virtualenv: the one activated in the project's shell
/// (`VIRTUAL_ENV`, e.g. set by direnv or an activated shell), else one at the
/// worktree root.
fn find_venv(worktree: &zed::Worktree, env: &[(String, String)]) -> Option<Venv> {
    let (os, _) = zed::current_platform();
    let virtual_env = env
        .iter()
        .find(|(key, _)| key == "VIRTUAL_ENV")
        .map(|(_, value)| value.as_str());
    if let Some(venv) =
        virtual_env.and_then(|dir| active_venv(dir, os, |name| worktree.which(name)))
    {
        return Some(venv);
    }

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

/// Describes the activated virtualenv at `dir`. Its `robotcode` is used when
/// the shell's `PATH` resolves `robotcode` inside it, as activation does.
fn active_venv(dir: &str, os: zed::Os, which: impl Fn(&str) -> Option<String>) -> Option<Venv> {
    let dir = dir.trim_end_matches(['/', '\\']);
    if dir.is_empty() {
        return None;
    }
    let python = match os {
        zed::Os::Windows => format!("{dir}\\Scripts\\python.exe"),
        _ => format!("{dir}/bin/python"),
    };
    let robotcode = which("robotcode").filter(|path| path.starts_with(dir));
    Some(Venv { python, robotcode })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_have_sensible_defaults() {
        let options = Options::from_settings(None);
        assert!(options.python.is_none());
        assert!(options.install_robocop);
        assert!(options.robotcode_version.is_none());

        let settings = serde_json::json!({
            "python": "/usr/bin/python3",
            "install_robocop": false,
            "robotcode_version": "v2.7.0",
        });
        let options = Options::from_settings(Some(&settings));
        assert_eq!(options.python.as_deref(), Some("/usr/bin/python3"));
        assert!(!options.install_robocop);
        assert_eq!(options.robotcode_version.as_deref(), Some("2.7.0"));
    }

    #[test]
    fn library_timeout_and_logging_env() {
        let env_of = |settings: serde_json::Value, mut env: Vec<(String, String)>| {
            Options::from_settings(Some(&settings)).apply_env(&mut env);
            env
        };
        let get = |env: &Vec<(String, String)>, key: &str| {
            env.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
        };

        let env = env_of(serde_json::json!({}), Vec::new());
        assert_eq!(get(&env, LOAD_TIMEOUT_VAR).as_deref(), Some("30"));
        assert_eq!(get(&env, "ROBOTCODE_LOG"), None);

        // An existing environment variable wins over the default...
        let env = env_of(
            serde_json::json!({}),
            vec![(LOAD_TIMEOUT_VAR.into(), "5".into())],
        );
        assert_eq!(get(&env, LOAD_TIMEOUT_VAR).as_deref(), Some("5"));

        // ...and the setting wins over both.
        let env = env_of(
            serde_json::json!({ "library_load_timeout": 60, "log_level": "info" }),
            vec![(LOAD_TIMEOUT_VAR.into(), "5".into())],
        );
        assert_eq!(get(&env, LOAD_TIMEOUT_VAR).as_deref(), Some("60"));
        assert_eq!(get(&env, "ROBOTCODE_LOG").as_deref(), Some("1"));
        assert_eq!(get(&env, "ROBOTCODE_LOG_LEVEL").as_deref(), Some("INFO"));
    }

    #[test]
    fn activated_virtualenv() {
        let which_in = |found: Option<&'static str>| move |_: &str| found.map(String::from);

        let venv = active_venv(
            "/home/me/venvs/robot/",
            zed::Os::Linux,
            which_in(Some("/home/me/venvs/robot/bin/robotcode")),
        )
        .unwrap();
        assert_eq!(venv.python, "/home/me/venvs/robot/bin/python");
        assert_eq!(
            venv.robotcode.as_deref(),
            Some("/home/me/venvs/robot/bin/robotcode")
        );

        // A robotcode outside the venv (e.g. ~/.local/bin) is not the venv's.
        let venv = active_venv(
            "/home/me/venvs/robot",
            zed::Os::Linux,
            which_in(Some("/home/me/.local/bin/robotcode")),
        )
        .unwrap();
        assert_eq!(venv.robotcode, None);

        let venv = active_venv("C:\\venvs\\robot", zed::Os::Windows, which_in(None)).unwrap();
        assert_eq!(venv.python, "C:\\venvs\\robot\\Scripts\\python.exe");

        assert!(active_venv("", zed::Os::Linux, which_in(None)).is_none());
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
