use zed_extension_api as zed;
use zed_extension_api::serde_json;

const EXTENSION_ID: &str = "salesforce-dx";
const APEX_LSP_ID: &str = "apex-lsp";
const APEX_LSP_MAIN_CLASS: &str = "apex.jorje.lsp.ApexLanguageServerLauncher";
const APEX_LSP_JAR_REL_PATH: &str = "vendor/apex-jorje-lsp.jar";
const DEFAULT_JAVA_MAX_HEAP_MB: u64 = 2048;

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

        let jar_path = resolve_apex_lsp_jar_path()?;

        let shell_env = worktree.shell_env();
        let lsp_settings = zed::settings::LspSettings::for_worktree(APEX_LSP_ID, worktree)
            .unwrap_or_default();

        let (java_command, mut jvm_args) = resolve_java_command(&lsp_settings, &shell_env, worktree);
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

zed::register_extension!(SalesforceExtension);

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
        .ok_or_else(|| format!("Could not derive extension installed directory from {}", work_dir.display()))?;

    Ok(installed_dir
        .join(APEX_LSP_JAR_REL_PATH)
        .to_string_lossy()
        .into_owned())
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
        if let Some(path) = &binary.path {
            apply_heap_setting(lsp_settings, &mut extra_args);
            return (path.clone(), extra_args);
        }
    }

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

fn has_heap_arg(args: &[String]) -> bool {
    args.iter().any(|arg| arg.starts_with("-Xmx"))
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
