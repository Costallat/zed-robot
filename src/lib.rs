mod debugger;
mod robotcode;

use zed_extension_api::{
    self as zed, serde_json, settings::LspSettings, DebugAdapterBinary, DebugConfig, DebugRequest,
    DebugScenario, DebugTaskDefinition, LanguageServerId, Result,
    StartDebuggingRequestArgumentsRequest, TaskTemplate,
};

use robotcode::{RobotCode, SERVER_ID};

const SERVER_ARGS: [&str; 2] = ["language-server", "--stdio"];

#[derive(Default)]
struct RobotFrameworkExtension {
    robotcode: RobotCode,
}

impl zed::Extension for RobotFrameworkExtension {
    fn new() -> Self {
        Self::default()
    }

    fn language_server_command(
        &mut self,
        id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let launch = self.robotcode.launch(worktree, Some(id))?;
        let user_args = LspSettings::for_worktree(SERVER_ID, worktree)
            .ok()
            .and_then(|s| s.binary)
            .and_then(|b| b.arguments);

        let mut args = launch.prefix_args;
        args.extend(user_args.unwrap_or_else(|| SERVER_ARGS.map(String::from).to_vec()));
        Ok(zed::Command {
            command: launch.command,
            args,
            env: launch.env,
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

    fn get_dap_binary(
        &mut self,
        adapter_name: String,
        config: DebugTaskDefinition,
        user_provided_debug_adapter_path: Option<String>,
        worktree: &zed::Worktree,
    ) -> Result<DebugAdapterBinary> {
        if adapter_name != debugger::ADAPTER_NAME {
            return Err(format!("unknown debug adapter `{adapter_name}`"));
        }
        debugger::binary(
            &mut self.robotcode,
            config,
            user_provided_debug_adapter_path,
            worktree,
        )
    }

    fn dap_request_kind(
        &mut self,
        _adapter_name: String,
        config: serde_json::Value,
    ) -> Result<StartDebuggingRequestArgumentsRequest> {
        debugger::request_kind(&config)
    }

    fn dap_config_to_scenario(&mut self, config: DebugConfig) -> Result<DebugScenario> {
        debugger::config_to_scenario(config)
    }

    fn dap_locator_create_scenario(
        &mut self,
        locator_name: String,
        build_task: TaskTemplate,
        resolved_label: String,
        debug_adapter_name: String,
    ) -> Option<DebugScenario> {
        if locator_name != debugger::LOCATOR_NAME {
            return None;
        }
        debugger::locator_scenario(build_task, resolved_label, debug_adapter_name)
    }

    fn run_dap_locator(
        &mut self,
        _locator_name: String,
        _build_task: TaskTemplate,
    ) -> Result<DebugRequest> {
        // Scenarios from the locator never have a build step.
        Err("the Robot Framework locator has no build step".into())
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
}
