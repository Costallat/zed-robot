//! Debugging through RobotCode's debug adapter (`robotcode debug-launch`).

use zed_extension_api::{
    self as zed, serde_json, DebugAdapterBinary, DebugConfig, DebugRequest, DebugScenario,
    DebugTaskDefinition, Result, StartDebuggingRequestArguments,
    StartDebuggingRequestArgumentsRequest, TaskTemplate,
};

use crate::robotcode::RobotCode;

/// Name of the debug adapter in `extension.toml` and in `debug.json`.
pub const ADAPTER_NAME: &str = "RobotCode";
/// Name of the locator that turns `robot` / `robotcode robot` tasks into
/// debug sessions.
pub const LOCATOR_NAME: &str = "robot";

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
    fill_defaults(
        &mut configuration,
        &config.label,
        &worktree.root_path(),
        launch.python.as_deref(),
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
fn fill_defaults(config: &mut serde_json::Value, label: &str, root: &str, python: Option<&str>) {
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
            &args[index + 1..]
        }
        p if p.starts_with("python") => match args {
            [flag, module, rest @ ..] if flag == "-m" && module == "robot" => rest,
            _ => return None,
        },
        _ => return None,
    };
    // Tasks quote arguments for the shell; the debugger passes them directly.
    Some(rest.iter().map(|a| unquote(a).to_string()).collect())
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
    fn fills_launch_defaults_without_overriding() {
        let mut config =
            serde_json::json!({ "target": "a.robot", "console": "integratedTerminal" });
        fill_defaults(
            &mut config,
            "Run a",
            "/project",
            Some("/project/.venv/bin/python"),
        );
        assert_eq!(config["request"], "launch");
        assert_eq!(config["cwd"], "/project");
        assert_eq!(config["console"], "integratedTerminal");
        assert_eq!(config["python"], "/project/.venv/bin/python");
        assert_eq!(config["name"], "Run a");
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
