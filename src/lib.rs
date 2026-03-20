use std::ffi::OsString;
use std::path::Path;

use zed_extension_api as zed;
use zed_extension_api::serde_json::{self, json};

const EXTENSION_ID: &str = "salesforce-dx";
const APEX_LSP_ID: &str = "apex-lsp";
const APEX_LSP_MAIN_CLASS: &str = "apex.jorje.lsp.ApexLanguageServerLauncher";
const APEX_LSP_JAR_REL_PATH: &str = "vendor/apex-jorje-lsp.jar";
const APEX_LSP_DEFAULT_JVM_PROPERTIES: [(&str, &str); 3] = [
    ("debug.internal.errors", "true"),
    ("debug.completion.statistics", "false"),
    ("lwc.typegeneration.disabled", "true"),
];
const DEFAULT_JAVA_MAX_HEAP_MB: u64 = 2048;
const AER_BINARY_NAME: &str = "aer";
const AER_BACKEND_SETTING_KEY: &str = "backend";
const AER_DEBUG_ADAPTER_ID: &str = "aer";

#[derive(Debug, PartialEq)]
enum ApexLspBackend {
    Jorje,
    Aer,
}

struct SalesforceExtension;

impl zed::Extension for SalesforceExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        if language_server_id.as_ref() != APEX_LSP_ID {
            return Err(format!("Unknown language server id: {language_server_id}"));
        }

        let shell_env = worktree.shell_env();
        let lsp_settings = read_lsp_settings(worktree);

        match resolve_backend(&lsp_settings) {
            ApexLspBackend::Aer => build_aer_command(&lsp_settings, shell_env, worktree),
            ApexLspBackend::Jorje => {
                let jar_path = resolve_apex_lsp_jar_path()?;
                let (java_command, mut jvm_args) =
                    resolve_java_command(&lsp_settings, &shell_env, worktree);
                jvm_args.push("-cp".to_string());
                jvm_args.push(jar_path);
                jvm_args.push(APEX_LSP_MAIN_CLASS.to_string());
                Ok(zed::Command {
                    command: java_command,
                    args: jvm_args,
                    env: shell_env,
                })
            }
        }
    }

    fn get_dap_binary(
        &mut self,
        adapter_name: String,
        config: zed::DebugTaskDefinition,
        user_provided_debug_adapter_path: Option<String>,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::DebugAdapterBinary> {
        if adapter_name != AER_DEBUG_ADAPTER_ID {
            return Err(format!("Unknown debug adapter id: {adapter_name}"));
        }

        if config.tcp_connection.is_some() {
            return Err(
                "AER debug mode uses stdio transport; tcp_connection is not supported.".to_string(),
            );
        }

        let shell_env = worktree.shell_env();
        let lsp_settings = read_lsp_settings(worktree);
        let debug_config = parse_debug_task_config(&config.config)?;
        let aer_path = resolve_aer_debug_binary(
            user_provided_debug_adapter_path.as_deref(),
            debug_config.get("aerPath").and_then(|value| value.as_str()),
            &lsp_settings,
            &shell_env,
            worktree,
        )?;
        let request = resolve_debug_request_kind_from_value(debug_config.get("request"))?;
        let request_configuration = build_aer_debug_request_configuration(&debug_config)?;

        Ok(zed::DebugAdapterBinary {
            command: Some(aer_path),
            arguments: build_aer_debug_command_args(&debug_config)?,
            envs: shell_env,
            cwd: Some(resolve_aer_debug_cwd(&debug_config, worktree)),
            connection: None,
            request_args: zed::StartDebuggingRequestArguments {
                configuration: request_configuration.to_string(),
                request,
            },
        })
    }

    fn dap_request_kind(
        &mut self,
        adapter_name: String,
        config: serde_json::Value,
    ) -> zed::Result<zed::StartDebuggingRequestArgumentsRequest> {
        if adapter_name != AER_DEBUG_ADAPTER_ID {
            return Err(format!("Unknown debug adapter id: {adapter_name}"));
        }

        let config = config
            .as_object()
            .ok_or_else(|| "AER debug configuration must be a JSON object.".to_string())?;
        resolve_debug_request_kind_from_value(config.get("request"))
    }
}

zed::register_extension!(SalesforceExtension);

fn read_lsp_settings(worktree: &zed::Worktree) -> zed::settings::LspSettings {
    zed::settings::LspSettings::for_worktree(APEX_LSP_ID, worktree).unwrap_or_default()
}

fn resolve_apex_lsp_jar_path() -> zed::Result<String> {
    // Important: the extension runs in a WASI sandbox. It cannot reliably stat/read files outside
    // its work directory, but it *can* pass absolute host paths to child processes (Java), which
    // will resolve them on the host filesystem. So we avoid `std::fs::metadata` checks here.
    //
    // Zed layout:
    // - work dir:      .../extensions/work/<id>
    // - install dir:   .../extensions/installed/<id>  (for dev extensions this is a symlink to the repo)
    let work_dir = std::env::current_dir().map_err(|err| err.to_string())?;

    // .../extensions/work/<id> -> .../extensions/installed/<id>
    let installed_dir = work_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|extensions_dir| extensions_dir.join("installed").join(EXTENSION_ID))
        .ok_or_else(|| {
            format!(
                "Could not derive extension installed directory from {}",
                work_dir.display()
            )
        })?;

    Ok(installed_dir
        .join(APEX_LSP_JAR_REL_PATH)
        .to_string_lossy()
        .into_owned())
}

fn resolve_backend(lsp_settings: &zed::settings::LspSettings) -> ApexLspBackend {
    match setting_value(lsp_settings, AER_BACKEND_SETTING_KEY)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("aer") => ApexLspBackend::Aer,
        _ => ApexLspBackend::Jorje,
    }
}

fn build_aer_command(
    lsp_settings: &zed::settings::LspSettings,
    shell_env: zed::EnvVars,
    worktree: &zed::Worktree,
) -> zed::Result<zed::Command> {
    let aer_path = resolve_aer_binary(lsp_settings, &shell_env, worktree)?;
    let source_args = resolve_aer_source_args(lsp_settings, worktree);
    let mut args = vec!["lsp".to_string()];
    args.extend(source_args);
    Ok(zed::Command {
        command: aer_path,
        args,
        env: shell_env,
    })
}

fn parse_debug_task_config(
    config: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(config).map_err(|err| format!("Invalid AER debug config: {err}"))?;
    parsed
        .as_object()
        .cloned()
        .ok_or_else(|| "AER debug configuration must be a JSON object.".to_string())
}

fn resolve_debug_request_kind_from_value(
    value: Option<&serde_json::Value>,
) -> Result<zed::StartDebuggingRequestArgumentsRequest, String> {
    match value.and_then(|value| value.as_str()) {
        Some(request) if request.eq_ignore_ascii_case("launch") => {
            Ok(zed::StartDebuggingRequestArgumentsRequest::Launch)
        }
        Some(request) => Err(format!(
            "Unsupported AER debug request '{request}'. Only 'launch' is supported."
        )),
        None => Err("AER debug configuration must include request = 'launch'.".to_string()),
    }
}

fn build_aer_debug_command_args(
    config: &serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<String>, String> {
    let request_args = debug_args_or_default(config)?;
    let mut args = vec!["test".to_string(), "--debug".to_string()];

    if let Some(timeout) = config.get("timeout").and_then(value_to_u64) {
        args.push("--timeout".to_string());
        args.push(timeout.to_string());
    }

    args.extend(request_args);
    Ok(args)
}

fn build_aer_debug_request_configuration(
    config: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let args = debug_args_or_default(config)?;
    let stop_on_entry = config
        .get("stopOnEntry")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    let mut request = serde_json::Map::new();
    request.insert("request".to_string(), json!("launch"));
    request.insert("args".to_string(), json!(args));
    request.insert("stopOnEntry".to_string(), json!(stop_on_entry));

    if let Some(timeout) = config.get("timeout").and_then(value_to_u64) {
        request.insert("timeout".to_string(), json!(timeout));
    }

    Ok(serde_json::Value::Object(request))
}

fn debug_args_or_default(
    config: &serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<String>, String> {
    let args = string_array(config.get("args"), "args")?;
    if args.is_empty() {
        Ok(vec![".".to_string()])
    } else {
        Ok(args)
    }
}

fn resolve_aer_debug_cwd(
    config: &serde_json::Map<String, serde_json::Value>,
    worktree: &zed::Worktree,
) -> String {
    config
        .get("cwd")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| worktree.root_path())
}

fn resolve_aer_debug_binary(
    user_provided_debug_adapter_path: Option<&str>,
    debug_config_aer_path: Option<&str>,
    lsp_settings: &zed::settings::LspSettings,
    shell_env: &zed::EnvVars,
    worktree: &zed::Worktree,
) -> zed::Result<String> {
    if let Some(path) = trim_non_empty(user_provided_debug_adapter_path) {
        return Ok(path.to_string());
    }

    if let Some(path) = trim_non_empty(debug_config_aer_path) {
        return Ok(path.to_string());
    }

    let include_binary_path = resolve_backend(lsp_settings) == ApexLspBackend::Aer;
    if let Some(path) = aer_path_from_lsp_settings(lsp_settings, include_binary_path) {
        return Ok(path);
    }

    if let Some(path) = worktree.which(AER_BINARY_NAME) {
        return Ok(path);
    }

    if let Some(path) = find_in_shell_path(AER_BINARY_NAME, shell_env) {
        return Ok(path);
    }

    Err(
        "aer binary not found. Configure dap.aer.binary, set 'aerPath' in .zed/debug.json, \
         or set 'lsp.apex-lsp.settings.aer_path'."
            .to_string(),
    )
}

fn resolve_aer_source_args(
    lsp_settings: &zed::settings::LspSettings,
    worktree: &zed::Worktree,
) -> Vec<String> {
    // Priority 1: explicit user override via settings.aer_source_paths (JSON array)
    if let Some(paths) = setting_value(lsp_settings, "aer_source_paths").and_then(|v| v.as_array())
    {
        let explicit: Vec<String> = paths
            .iter()
            .filter_map(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .flat_map(|p| ["--path".to_string(), p])
            .collect();
        if !explicit.is_empty() {
            return explicit;
        }
    }
    // Priority 2: autodiscover from sfdx-project.json
    aer_source_args_from_sfdx(worktree).unwrap_or_default()
}

fn aer_source_args_from_sfdx(worktree: &zed::Worktree) -> Option<Vec<String>> {
    let json = worktree.read_text_file("sfdx-project.json").ok()?;
    let root: serde_json::Value = serde_json::from_str(&json).ok()?;
    let dirs = root.get("packageDirectories")?.as_array()?;
    let root_path = worktree.root_path();
    let args: Vec<String> = dirs
        .iter()
        .filter_map(|d| d.get("path")?.as_str())
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .flat_map(|p| {
            let abs = normalize_source_path(&root_path, &p);
            ["--path".to_string(), abs]
        })
        .collect();
    if args.is_empty() {
        None
    } else {
        Some(args)
    }
}

fn resolve_aer_binary(
    lsp_settings: &zed::settings::LspSettings,
    shell_env: &zed::EnvVars,
    worktree: &zed::Worktree,
) -> zed::Result<String> {
    if let Some(path) = aer_path_from_lsp_settings(lsp_settings, true) {
        return Ok(path);
    }

    if let Some(path) = worktree.which(AER_BINARY_NAME) {
        return Ok(path);
    }

    if let Some(path) = find_in_shell_path(AER_BINARY_NAME, shell_env) {
        return Ok(path);
    }

    Err(format!(
        "aer binary not found. Install from https://github.com/octoberswimmer/aer-dist/ \
         or set 'aer_path' in lsp.apex-lsp.settings"
    ))
}

fn aer_path_from_lsp_settings(
    lsp_settings: &zed::settings::LspSettings,
    include_binary_path: bool,
) -> Option<String> {
    if include_binary_path {
        if let Some(binary) = &lsp_settings.binary {
            if let Some(path) = trim_non_empty(binary.path.as_deref()) {
                return Some(path.to_string());
            }
        }
    }

    trim_non_empty(
        setting_value(lsp_settings, "aer_path")
            .and_then(|v| v.as_str())
            .map(str::trim),
    )
    .map(ToOwned::to_owned)
}

fn trim_non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn string_array(value: Option<&serde_json::Value>, key: &str) -> Result<Vec<String>, String> {
    match value {
        None => Ok(Vec::new()),
        Some(serde_json::Value::Array(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| format!("AER debug config '{key}' entries must be strings."))
            })
            .collect(),
        Some(_) => Err(format!(
            "AER debug config '{key}' must be an array of strings."
        )),
    }
}

fn find_in_shell_path(name: &str, shell_env: &zed::EnvVars) -> Option<String> {
    let path_var = env_var(shell_env, "PATH")?;
    let path_var = OsString::from(path_var);
    let path_entries = std::env::split_paths(&path_var);
    if path_entries
        .into_iter()
        .any(|dir| !dir.as_os_str().is_empty())
    {
        return Some(name.to_string());
    }
    None
}

fn normalize_source_path(root_path: &str, package_path: &str) -> String {
    let package_path = Path::new(package_path);
    if package_path.is_absolute() {
        return package_path.to_string_lossy().into_owned();
    }

    Path::new(root_path)
        .join(package_path)
        .to_string_lossy()
        .into_owned()
}

fn resolve_java_command(
    lsp_settings: &zed::settings::LspSettings,
    shell_env: &zed::EnvVars,
    worktree: &zed::Worktree,
) -> (String, Vec<String>) {
    let mut extra_args = Vec::new();
    if let Some(binary) = &lsp_settings.binary {
        if let Some(args) = &binary.arguments {
            extra_args.extend(args.iter().cloned());
        }
        apply_default_apex_lsp_jvm_args(&mut extra_args);
        if let Some(path) = &binary.path {
            apply_heap_setting(lsp_settings, &mut extra_args);
            return (path.clone(), extra_args);
        }
    }

    apply_default_apex_lsp_jvm_args(&mut extra_args);

    if let Some(java_home) = java_home_from_settings(lsp_settings)
        .or_else(|| env_var(shell_env, "JDK_HOME"))
        .or_else(|| env_var(shell_env, "JAVA_HOME"))
    {
        let cmd = std::path::Path::new(&java_home).join("bin").join("java");
        apply_heap_setting(lsp_settings, &mut extra_args);
        return (cmd.to_string_lossy().into_owned(), extra_args);
    }

    if let Some(path) = worktree.which("java") {
        apply_heap_setting(lsp_settings, &mut extra_args);
        return (path, extra_args);
    }

    apply_heap_setting(lsp_settings, &mut extra_args);
    ("java".to_string(), extra_args)
}

fn env_var(env: &zed::EnvVars, key: &str) -> Option<String> {
    env.iter()
        .find_map(|(k, v)| if k == key { Some(v.clone()) } else { None })
}

fn java_home_from_settings(lsp_settings: &zed::settings::LspSettings) -> Option<String> {
    setting_value(lsp_settings, "java_home")
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn apply_heap_setting(lsp_settings: &zed::settings::LspSettings, args: &mut Vec<String>) {
    if has_heap_arg(args) {
        return;
    }

    let heap = setting_value(lsp_settings, "java_max_heap_mb")
        .and_then(value_to_u64)
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_JAVA_MAX_HEAP_MB);

    args.push(format!("-Xmx{heap}m"));
}

fn apply_default_apex_lsp_jvm_args(args: &mut Vec<String>) {
    for (name, value) in APEX_LSP_DEFAULT_JVM_PROPERTIES {
        if has_system_property_arg(args, name) {
            continue;
        }
        args.push(format!("-D{name}={value}"));
    }
}

fn has_heap_arg(args: &[String]) -> bool {
    args.iter().any(|arg| arg.starts_with("-Xmx"))
}

fn has_system_property_arg(args: &[String], key: &str) -> bool {
    let prefix = format!("-D{key}");
    args.iter()
        .any(|arg| arg == &prefix || arg.starts_with(&format!("{prefix}=")))
}

fn setting_value<'a>(
    lsp_settings: &'a zed::settings::LspSettings,
    key: &str,
) -> Option<&'a serde_json::Value> {
    let settings = lsp_settings.settings.as_ref()?;
    settings.as_object()?.get(key)
}

fn value_to_u64(value: &serde_json::Value) -> Option<u64> {
    match value {
        serde_json::Value::Number(number) => number.as_u64(),
        serde_json::Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        aer_path_from_lsp_settings, build_aer_debug_command_args,
        build_aer_debug_request_configuration, debug_args_or_default, find_in_shell_path,
        normalize_source_path, parse_debug_task_config, resolve_backend,
        resolve_debug_request_kind_from_value, ApexLspBackend,
    };
    use zed_extension_api as zed;
    use zed_extension_api::serde_json::{self, json};

    #[test]
    fn normalize_source_path_joins_relative_package_directory() {
        assert_eq!(
            normalize_source_path("/workspace/project", "force-app/main/default"),
            "/workspace/project/force-app/main/default"
        );
    }

    #[test]
    fn normalize_source_path_preserves_absolute_package_directory() {
        assert_eq!(
            normalize_source_path("/workspace/project", "/tmp/force-app"),
            "/tmp/force-app"
        );
    }

    #[test]
    fn find_in_shell_path_returns_bare_command_when_path_has_entries() {
        let shell_env = vec![("PATH".to_string(), "/usr/local/bin:/usr/bin".to_string())];

        assert_eq!(
            find_in_shell_path("aer", &shell_env),
            Some("aer".to_string())
        );
    }

    #[test]
    fn find_in_shell_path_returns_none_when_path_missing() {
        let shell_env = Vec::new();

        assert_eq!(find_in_shell_path("aer", &shell_env), None);
    }

    #[test]
    fn parse_debug_task_config_requires_json_object() {
        let err = parse_debug_task_config("[]").unwrap_err();
        assert!(err.contains("JSON object"));
    }

    #[test]
    fn resolve_debug_request_kind_accepts_launch() {
        let request = resolve_debug_request_kind_from_value(Some(&json!("launch"))).unwrap();
        assert_eq!(request, zed::StartDebuggingRequestArgumentsRequest::Launch);
    }

    #[test]
    fn resolve_debug_request_kind_rejects_attach() {
        let err = resolve_debug_request_kind_from_value(Some(&json!("attach"))).unwrap_err();
        assert!(err.contains("Only 'launch'"));
    }

    #[test]
    fn debug_command_defaults_to_current_directory() {
        let config = serde_json::Map::new();
        assert_eq!(
            build_aer_debug_command_args(&config).unwrap(),
            vec!["test", "--debug", "."]
        );
    }

    #[test]
    fn debug_command_includes_timeout_and_args() {
        let config =
            parse_debug_task_config(r#"{"request":"launch","args":["force-app"],"timeout":45}"#)
                .unwrap();
        assert_eq!(
            build_aer_debug_command_args(&config).unwrap(),
            vec!["test", "--debug", "--timeout", "45", "force-app"]
        );
    }

    #[test]
    fn debug_request_configuration_filters_to_adapter_fields() {
        let config = parse_debug_task_config(
            r#"{"label":"Debug tests","adapter":"aer","request":"launch","args":["force-app"],"stopOnEntry":true,"aerPath":"/tmp/aer","cwd":"/tmp/work"}"#,
        )
        .unwrap();
        assert_eq!(
            build_aer_debug_request_configuration(&config).unwrap(),
            json!({
                "request": "launch",
                "args": ["force-app"],
                "stopOnEntry": true
            })
        );
    }

    #[test]
    fn debug_args_reject_non_string_entries() {
        let config = parse_debug_task_config(r#"{"args":[42]}"#).unwrap();
        let err = debug_args_or_default(&config).unwrap_err();
        assert!(err.contains("entries must be strings"));
    }

    #[test]
    fn resolve_backend_prefers_aer_setting() {
        let settings = zed::settings::LspSettings {
            binary: None,
            initialization_options: None,
            settings: Some(json!({"backend":"aer"})),
        };

        assert_eq!(resolve_backend(&settings), ApexLspBackend::Aer);
    }

    #[test]
    fn aer_path_from_lsp_settings_reads_binary_path_when_enabled() {
        let settings = zed::settings::LspSettings {
            binary: Some(zed::settings::CommandSettings {
                path: Some("/tmp/aer".to_string()),
                arguments: None,
                env: None,
            }),
            initialization_options: None,
            settings: Some(json!({})),
        };

        assert_eq!(
            aer_path_from_lsp_settings(&settings, true),
            Some("/tmp/aer".to_string())
        );
        assert_eq!(aer_path_from_lsp_settings(&settings, false), None);
    }
}
