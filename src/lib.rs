use std::ffi::OsString;
use std::path::Path;
use zed_extension_api as zed;
use zed_extension_api::serde_json;
use zed_extension_api::{DownloadedFileType, LanguageServerInstallationStatus};

const APEX_LSP_ID: &str = "apex-lsp";
const APEX_LSP_MAIN_CLASS: &str = "apex.jorje.lsp.ApexLanguageServerLauncher";
const APEX_LSP_JAR_CACHE_REL_PATH: &str = "lsp/apex-lsp/apex-jorje-lsp.jar";
const APEX_LSP_JAR_DOWNLOAD_URL: &str = "https://raw.githubusercontent.com/forcedotcom/salesforcedx-vscode/67dc27932e0ce43b93abe00878a2f966d0eb16a3/packages/salesforcedx-vscode-apex/jars/apex-jorje-lsp.jar";
const APEX_LSP_DEFAULT_JVM_PROPERTIES: [(&str, &str); 3] = [
    ("debug.internal.errors", "true"),
    ("debug.completion.statistics", "false"),
    ("lwc.typegeneration.disabled", "true"),
];
const DEFAULT_JAVA_MAX_HEAP_MB: u64 = 2048;
const AER_BINARY_NAME: &str = "aer";
const AER_BACKEND_SETTING_KEY: &str = "backend";

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
        let lsp_settings =
            zed::settings::LspSettings::for_worktree(APEX_LSP_ID, worktree).unwrap_or_default();

        match resolve_backend(&lsp_settings) {
            ApexLspBackend::Aer => build_aer_command(&lsp_settings, shell_env, worktree),
            ApexLspBackend::Jorje => {
                let jar_path = ensure_apex_lsp_jar(language_server_id)?;
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
}

zed::register_extension!(SalesforceExtension);

fn ensure_apex_lsp_jar(language_server_id: &zed::LanguageServerId) -> zed::Result<String> {
    let work_dir = std::env::current_dir().map_err(|err| err.to_string())?;
    let jar_rel_path = std::path::Path::new(APEX_LSP_JAR_CACHE_REL_PATH);
    let jar_abs_path = work_dir.join(jar_rel_path);

    if jar_abs_path.is_file() {
        zed::set_language_server_installation_status(
            language_server_id,
            &LanguageServerInstallationStatus::None,
        );
        return Ok(jar_abs_path.to_string_lossy().into_owned());
    }

    if let Some(parent) = jar_abs_path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Could not create Apex Language Server cache directory {}: {err}",
                parent.display()
            )
        })?;
    }

    zed::set_language_server_installation_status(
        language_server_id,
        &LanguageServerInstallationStatus::Downloading,
    );

    if let Err(err) = zed::download_file(
        APEX_LSP_JAR_DOWNLOAD_URL,
        APEX_LSP_JAR_CACHE_REL_PATH,
        DownloadedFileType::Uncompressed,
    ) {
        let message = format!(
            "Failed to download Apex Language Server jar from {APEX_LSP_JAR_DOWNLOAD_URL}: {err}"
        );
        zed::set_language_server_installation_status(
            language_server_id,
            &LanguageServerInstallationStatus::Failed(message.clone()),
        );
        return Err(message);
    }

    if !jar_abs_path.is_file() {
        let message = format!(
            "Apex Language Server download completed but {} was not created",
            jar_abs_path.display()
        );
        zed::set_language_server_installation_status(
            language_server_id,
            &LanguageServerInstallationStatus::Failed(message.clone()),
        );
        return Err(message);
    }

    zed::set_language_server_installation_status(
        language_server_id,
        &LanguageServerInstallationStatus::None,
    );

    Ok(jar_abs_path.to_string_lossy().into_owned())
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
    // Priority 1: lsp.apex-lsp.binary.path
    if let Some(binary) = &lsp_settings.binary {
        if let Some(path) = &binary.path {
            return Ok(path.clone());
        }
    }
    // Priority 2: lsp.apex-lsp.settings.aer_path
    if let Some(aer_path) = setting_value(lsp_settings, "aer_path")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        return Ok(aer_path);
    }
    // Priority 3: worktree.which (may use restricted PATH in worktree-shell)
    if let Some(path) = worktree.which(AER_BINARY_NAME) {
        return Ok(path);
    }
    // Priority 4: rely on the full shell PATH when worktree.which() misses user dirs.
    // worktree.which() may miss binaries in user dirs like ~/.cargo/bin because it
    // uses the worktree-shell PATH (a Zed limitation). shell_env has the full PATH,
    // and the host process resolver can search it when we return a bare command name.
    if let Some(path) = find_in_shell_path(AER_BINARY_NAME, shell_env) {
        return Ok(path);
    }
    Err(format!(
        "aer binary not found. Install from https://github.com/octoberswimmer/aer-dist/ \
         or set 'aer_path' in lsp.apex-lsp.settings"
    ))
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
    use super::{find_in_shell_path, normalize_source_path};

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
}
