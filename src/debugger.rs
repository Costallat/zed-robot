//! Debugging through RobotCode's debug adapter (`robotcode debug-launch`).

use zed_extension_api::{
    self as zed, serde_json, DebugAdapterBinary, DebugConfig, DebugRequest, DebugScenario,
    DebugTaskDefinition, Result, StartDebuggingRequestArguments,
    StartDebuggingRequestArgumentsRequest, TaskTemplate,
};

use crate::robotcode::{Options, RobotCode, SERVER_ID};
use zed_extension_api::settings::LspSettings;

/// Name of the debug adapter in `extension.toml` and in `debug.json`.
pub const ADAPTER_NAME: &str = "RobotCode";
/// Name of the locator that turns `robot` / `robotcode robot` tasks into
/// debug sessions.
pub const LOCATOR_NAME: &str = "robot";
/// Port debugpy listens on when Python debugging is on (RobotCode's default).
const DEBUGPY_PORT: u64 = 5678;
/// Seconds the run waits for Zed's Python debugger to attach.
const DEBUGPY_ATTACH_TIMEOUT: u64 = 120;

pub fn binary(
    robotcode: &mut RobotCode,
    config: DebugTaskDefinition,
    user_provided_path: Option<String>,
    worktree: &zed::Worktree,
) -> Result<DebugAdapterBinary> {
    let launch = robotcode.launch(worktree, None)?;

    let (command, mut arguments) = match user_provided_path {
        Some(path) => (path, Vec::new()),
        // The managed RobotCode runs on Python; keep its output unbuffered
        // so the debug console stays in sync.
        None if launch.managed_version.is_some() => {
            let mut args = vec!["-u".to_string()];
            args.extend(launch.prefix_args);
            (launch.command, args)
        }
        None => (launch.command, launch.prefix_args),
    };
    arguments.extend(["debug-launch".to_string(), "--stdio".to_string()]);

    let mut configuration: serde_json::Value =
        serde_json::from_str(&config.config).map_err(|e| format!("invalid debug config: {e}"))?;
    let request =
        request_kind(&configuration).unwrap_or(StartDebuggingRequestArgumentsRequest::Launch);
    let options = Options::from_settings(
        LspSettings::for_worktree(SERVER_ID, worktree)
            .ok()
            .and_then(|s| s.settings)
            .as_ref(),
    );
    fill_defaults(
        &mut configuration,
        &config.label,
        &worktree.root_path(),
        launch.python.as_deref(),
        options.debug_python,
    );

    Ok(DebugAdapterBinary {
        command: Some(command),
        arguments,
        envs: launch.env,
        cwd: Some(worktree.root_path()),
        connection: None,
        request_args: StartDebuggingRequestArguments {
            configuration: configuration.to_string(),
            request,
        },
    })
}

/// Fills in what RobotCode's launcher needs but a hand-written `debug.json`
/// entry usually leaves out.
///
/// With Python debugging (`attachPython`, or the `debug_python` setting),
/// RobotCode starts debugpy in the test run. Zed cannot attach to it on its
/// own (RobotCode announces it with a custom event Zed ignores), so the run
/// waits for the user to start the "Debugpy" attach configuration.
fn fill_defaults(
    config: &mut serde_json::Value,
    label: &str,
    root: &str,
    python: Option<&str>,
    debug_python: bool,
) {
    let Some(map) = config.as_object_mut() else {
        return;
    };
    let mut default = |key: &str, value: serde_json::Value| {
        map.entry(key.to_string()).or_insert(value);
    };
    default("request", "launch".into());
    default("name", label.into());
    default("cwd", root.into());
    // Robot Framework's console output goes to Zed's debug console.
    default("console", "internalConsole".into());
    default("outputMessages", true.into());
    default("outputLog", true.into());
    if let Some(python) = python {
        default("python", python.into());
    }
    if debug_python {
        default("attachPython", true.into());
    }

    if map.get("attachPython").and_then(|v| v.as_bool()) == Some(true) {
        map.entry("attachPythonPort").or_insert(DEBUGPY_PORT.into());
        map.entry("debuggerTimeout")
            .or_insert(DEBUGPY_ATTACH_TIMEOUT.into());
        let args = map
            .entry("debuggerArgs")
            .or_insert_with(|| serde_json::json!([]));
        if let Some(args) = args.as_array_mut() {
            let wait = "--debugpy-wait-for-client";
            if !args.iter().any(|a| a.as_str() == Some(wait)) {
                args.push(wait.into());
            }
        }
    }
}

pub fn request_kind(config: &serde_json::Value) -> Result<StartDebuggingRequestArgumentsRequest> {
    match config.get("request").and_then(|r| r.as_str()) {
        Some("launch") => Ok(StartDebuggingRequestArgumentsRequest::Launch),
        Some("attach") => Ok(StartDebuggingRequestArgumentsRequest::Attach),
        Some(other) => Err(format!("unknown debug request `{other}`")),
        None => Err("debug config has no `request` (\"launch\")".into()),
    }
}

/// Builds a scenario from Zed's "new debug session" dialog: the program is the
/// suite, file or folder to run.
pub fn config_to_scenario(config: DebugConfig) -> Result<DebugScenario> {
    let DebugRequest::Launch(launch) = config.request else {
        return Err("RobotCode can only launch Robot Framework runs, not attach to them".into());
    };
    let mut scenario = serde_json::json!({
        "request": "launch",
        "target": launch.program,
        "args": launch.args,
        "env": env_object(launch.envs),
    });
    if let Some(cwd) = launch.cwd {
        scenario["cwd"] = cwd.into();
    }
    if let Some(stop) = config.stop_on_entry {
        scenario["stopOnEntry"] = stop.into();
    }
    Ok(DebugScenario {
        label: config.label,
        adapter: config.adapter,
        build: None,
        config: scenario.to_string(),
        tcp_connection: None,
    })
}

/// Turns a task that runs Robot Framework (`robot ...`, `robotcode robot ...`,
/// `python -m robot ...`) into a debug scenario with the same arguments, so
/// run buttons and tasks can also be debugged.
pub fn locator_scenario(
    task: TaskTemplate,
    label: String,
    adapter: String,
) -> Option<DebugScenario> {
    if adapter != ADAPTER_NAME {
        return None;
    }
    let robot_args = robot_arguments(&task.command, &task.args)?;
    let mut config = serde_json::json!({
        "request": "launch",
        "args": robot_args,
        "env": env_object(task.env),
    });
    if let Some(cwd) = task.cwd {
        config["cwd"] = cwd.into();
    }
    Some(DebugScenario {
        label,
        adapter,
        build: None,
        config: config.to_string(),
        tcp_connection: None,
    })
}

/// Returns the arguments meant for Robot Framework if `command args` runs it.
fn robot_arguments(command: &str, args: &[String]) -> Option<Vec<String>> {
    let program = command.rsplit(['/', '\\']).next().unwrap_or(command);
    let program = program.trim_end_matches(".exe");
    let rest = match program {
        "robot" => args,
        "robotcode" => {
            // Skip robotcode's own options (e.g. `-p profile`) up to the subcommand.
            let index = args.iter().position(|a| a == "robot" || a == "run")?;
            let rest = &args[index + 1..];
            return Some(longname_to_test(unquoted(rest)));
        }
        p if p.starts_with("python") => match args {
            [flag, module, rest @ ..] if flag == "-m" && module == "robot" => rest,
            _ => return None,
        },
        _ => return None,
    };
    Some(unquoted(rest))
}

/// Tasks quote arguments for the shell; the debugger passes them directly.
fn unquoted(args: &[String]) -> Vec<String> {
    args.iter().map(|a| unquote(a).to_string()).collect()
}

/// `--by-longname` is an option of `robotcode robot`, which the debugger does
/// not run: it passes the arguments to Robot Framework itself. Turn it into
/// Robot Framework's `--test`, escaping the pattern characters so the name
/// still matches exactly.
fn longname_to_test(args: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len());
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--by-longname" || arg == "-bl" {
            if let Some(name) = args.next() {
                out.push("--test".to_string());
                out.push(escape_test_pattern(&name));
            }
        } else {
            out.push(arg);
        }
    }
    out
}

/// Escapes `*`, `?` and `[` so Robot Framework's `--test` matches them literally.
fn escape_test_pattern(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        match c {
            '*' | '?' | '[' => {
                out.push('[');
                out.push(c);
                out.push(']');
            }
            _ => out.push(c),
        }
    }
    out
}

fn env_object(env: Vec<(String, String)>) -> serde_json::Value {
    env.into_iter()
        .map(|(key, value)| (key, serde_json::Value::String(value)))
        .collect::<serde_json::Map<_, _>>()
        .into()
}

fn unquote(arg: &str) -> &str {
    for quote in ['"', '\''] {
        if let Some(inner) = arg.strip_prefix(quote).and_then(|a| a.strip_suffix(quote)) {
            return inner;
        }
    }
    arg
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn recognises_robot_commands() {
        let args = strings(&["--test", "\"My Test\"", "\"suite.robot\""]);
        assert_eq!(
            robot_arguments("robot", &args),
            Some(strings(&["--test", "My Test", "suite.robot"]))
        );
        assert_eq!(
            robot_arguments(
                "/x/.venv/bin/robotcode",
                &strings(&["-p", "dev", "robot", "a.robot"])
            ),
            Some(strings(&["a.robot"]))
        );
        assert_eq!(
            robot_arguments("python3", &strings(&["-m", "robot", "a.robot"])),
            Some(strings(&["a.robot"]))
        );
        assert_eq!(robot_arguments("python3", &strings(&["script.py"])), None);
        assert_eq!(robot_arguments("cargo", &strings(&["test"])), None);
        assert_eq!(robot_arguments("robotcode", &strings(&["analyze"])), None);
    }

    #[test]
    fn exact_test_names_become_escaped_test_patterns() {
        let args = strings(&[
            "robot",
            "--name",
            "Robot",
            "--by-longname",
            "\"Robot.Login [smoke] * works?\"",
            "\"a.robot\"",
        ]);
        assert_eq!(
            robot_arguments("robotcode", &args),
            Some(strings(&[
                "--name",
                "Robot",
                "--test",
                "Robot.Login [[]smoke] [*] works[?]",
                "a.robot"
            ]))
        );
    }

    #[test]
    fn fills_launch_defaults_without_overriding() {
        let mut config =
            serde_json::json!({ "target": "a.robot", "console": "integratedTerminal" });
        fill_defaults(
            &mut config,
            "Run a",
            "/project",
            Some("/project/.venv/bin/python"),
            false,
        );
        assert_eq!(config["request"], "launch");
        assert_eq!(config["cwd"], "/project");
        assert_eq!(config["console"], "integratedTerminal");
        assert_eq!(config["python"], "/project/.venv/bin/python");
        assert_eq!(config["name"], "Run a");
        assert!(config.get("attachPython").is_none());
        assert!(config.get("debuggerArgs").is_none());
    }

    #[test]
    fn python_debugging_waits_for_the_python_debugger() {
        // Turned on by the `debug_python` setting...
        let mut config = serde_json::json!({ "target": "a.robot" });
        fill_defaults(&mut config, "Run a", "/p", None, true);
        assert_eq!(config["attachPython"], true);
        assert_eq!(config["attachPythonPort"], 5678);
        assert_eq!(config["debuggerTimeout"], 120);
        assert_eq!(
            config["debuggerArgs"],
            serde_json::json!(["--debugpy-wait-for-client"])
        );

        // ...or in debug.json, keeping the user's own values.
        let mut config = serde_json::json!({
            "attachPython": true,
            "attachPythonPort": 6000,
            "debuggerArgs": ["--debugpy-wait-for-client", "--output-timestamps"]
        });
        fill_defaults(&mut config, "Run a", "/p", None, false);
        assert_eq!(config["attachPythonPort"], 6000);
        assert_eq!(
            config["debuggerArgs"],
            serde_json::json!(["--debugpy-wait-for-client", "--output-timestamps"])
        );

        // debug.json can opt out even when the setting is on.
        let mut config = serde_json::json!({ "attachPython": false });
        fill_defaults(&mut config, "Run a", "/p", None, true);
        assert_eq!(config["attachPython"], false);
        assert!(config.get("debuggerArgs").is_none());
    }

    #[test]
    fn request_kind_requires_request() {
        assert!(request_kind(&serde_json::json!({})).is_err());
        assert!(matches!(
            request_kind(&serde_json::json!({ "request": "launch" })),
            Ok(StartDebuggingRequestArgumentsRequest::Launch)
        ));
    }
}
