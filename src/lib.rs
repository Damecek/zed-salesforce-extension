use std::path::Path;
use zed_extension_api as zed;
use zed_extension_api::serde_json;
use zed_extension_api::{DownloadedFileType, LanguageServerInstallationStatus};

const APEX_LSP_ID: &str = "apex-language-server";
const LWC_LSP_ID: &str = "lwc-language-server";
const LWC_LSP_PACKAGE_NAME: &str = "@salesforce/lwc-language-server";
const LWC_LSP_PACKAGE_VERSION: &str = "4.12.13";
const LWC_LSP_WRAPPER_REL_PATH: &str = "scripts/lwc-language-server-wrapper.js";
const LWC_LSP_WRAPPER_SOURCE: &str = include_str!("../scripts/lwc-language-server-wrapper.js");
const LWC_LSP_UPSTREAM_SERVER_ENV: &str = "ZED_SALESFORCE_LWC_UPSTREAM_SERVER_PATH";
const APEX_LSP_MAIN_CLASS: &str = "apex.jorje.lsp.ApexLanguageServerLauncher";
const APEX_LSP_JAR_CACHE_REL_PATH: &str = "lsp/apex-language-server/apex-jorje-lsp.jar";
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
        match language_server_id.as_ref() {
            APEX_LSP_ID => apex_language_server_command(language_server_id, worktree),
            LWC_LSP_ID => lwc_language_server_command(language_server_id, worktree),
            _ => Err(format!("Unknown language server id: {language_server_id}")),
        }
    }

    fn language_server_initialization_options(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<serde_json::Value>> {
        if language_server_id.as_ref() != APEX_LSP_ID {
            return Ok(None);
        }
        let lsp_settings =
            zed::settings::LspSettings::for_worktree(APEX_LSP_ID, worktree).unwrap_or_default();
        match resolve_backend(&lsp_settings) {
            ApexLspBackend::Jorje => Ok(Some(serde_json::json!({
                "enableSynchronizedInitJobs": true,
                "enableSemanticErrors": false,
                "enableCompletionStatistics": false,
            }))),
            ApexLspBackend::Aer => Ok(None),
        }
    }
}

zed::register_extension!(SalesforceExtension);

fn apex_language_server_command(
    language_server_id: &zed::LanguageServerId,
    worktree: &zed::Worktree,
) -> zed::Result<zed::Command> {
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

fn lwc_language_server_command(
    language_server_id: &zed::LanguageServerId,
    worktree: &zed::Worktree,
) -> zed::Result<zed::Command> {
    let node = zed::node_binary_path()?;
    let server_path = ensure_lwc_language_server(language_server_id)?;
    let wrapper_path = ensure_lwc_language_server_wrapper(language_server_id)?;
    let mut env = worktree.shell_env();
    env.push((LWC_LSP_UPSTREAM_SERVER_ENV.to_string(), server_path));

    Ok(zed::Command {
        command: node,
        args: vec![wrapper_path, "--stdio".to_string()],
        env,
    })
}

fn ensure_lwc_language_server(language_server_id: &zed::LanguageServerId) -> zed::Result<String> {
    let installed_version = zed::npm_package_installed_version(LWC_LSP_PACKAGE_NAME)?;
    if installed_version.as_deref() != Some(LWC_LSP_PACKAGE_VERSION) {
        zed::set_language_server_installation_status(
            language_server_id,
            &LanguageServerInstallationStatus::Downloading,
        );
        if let Err(err) = zed::npm_install_package(LWC_LSP_PACKAGE_NAME, LWC_LSP_PACKAGE_VERSION) {
            let message = format!(
                "Failed to install {LWC_LSP_PACKAGE_NAME}@{LWC_LSP_PACKAGE_VERSION}: {err}"
            );
            zed::set_language_server_installation_status(
                language_server_id,
                &LanguageServerInstallationStatus::Failed(message.clone()),
            );
            return Err(message);
        }
    }

    let work_dir = std::env::current_dir()
        .map_err(|err| format!("Could not get extension work directory: {err}"))?;
    let server_path = work_dir
        .join("node_modules")
        .join("@salesforce")
        .join("lwc-language-server")
        .join("bin")
        .join("lwc-language-server.js");

    if !server_path.is_file() {
        let message = format!(
            "{LWC_LSP_PACKAGE_NAME}@{LWC_LSP_PACKAGE_VERSION} is installed but {} was not found",
            server_path.display()
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

    Ok(server_path.to_string_lossy().into_owned())
}

fn ensure_lwc_language_server_wrapper(
    language_server_id: &zed::LanguageServerId,
) -> zed::Result<String> {
    let work_dir = std::env::current_dir()
        .map_err(|err| format!("Could not get extension work directory: {err}"))?;
    let wrapper_path = work_dir.join(LWC_LSP_WRAPPER_REL_PATH);

    if let Some(parent) = wrapper_path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            let message = format!(
                "Could not create LWC language server wrapper directory {}: {err}",
                parent.display()
            );
            zed::set_language_server_installation_status(
                language_server_id,
                &LanguageServerInstallationStatus::Failed(message.clone()),
            );
            message
        })?;
    }

    let should_write = match std::fs::read_to_string(&wrapper_path) {
        Ok(existing) => existing != LWC_LSP_WRAPPER_SOURCE,
        Err(_) => true,
    };

    if should_write {
        std::fs::write(&wrapper_path, LWC_LSP_WRAPPER_SOURCE).map_err(|err| {
            let message = format!(
                "Could not write LWC language server wrapper {}: {err}",
                wrapper_path.display()
            );
            zed::set_language_server_installation_status(
                language_server_id,
                &LanguageServerInstallationStatus::Failed(message.clone()),
            );
            message
        })?;
    }

    if !wrapper_path.is_file() {
        let message = format!(
            "LWC language server wrapper {} was not created",
            wrapper_path.display()
        );
        zed::set_language_server_installation_status(
            language_server_id,
            &LanguageServerInstallationStatus::Failed(message.clone()),
        );
        return Err(message);
    }

    Ok(wrapper_path.to_string_lossy().into_owned())
}

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
        Some("jorje") => ApexLspBackend::Jorje,
        _ => ApexLspBackend::Aer,
    }
}

fn build_aer_command(
    lsp_settings: &zed::settings::LspSettings,
    shell_env: zed::EnvVars,
    worktree: &zed::Worktree,
) -> zed::Result<zed::Command> {
    let aer_path = resolve_aer_binary(lsp_settings, worktree)?;
    let source_args = resolve_aer_source_args(lsp_settings, worktree)?;
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
) -> zed::Result<Vec<String>> {
    // Priority 1: explicit user override via settings.aer_source_paths (JSON array)
    if let Some(paths) = setting_value(lsp_settings, "aer_source_paths").and_then(|v| v.as_array())
    {
        let explicit: Vec<String> = paths
            .iter()
            .filter_map(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !explicit.is_empty() {
            return Ok(explicit);
        }
    }
    // Priority 2: autodiscover from sfdx-project.json
    aer_source_args_from_sfdx(worktree)
}

fn aer_source_args_from_sfdx(worktree: &zed::Worktree) -> zed::Result<Vec<String>> {
    // Missing sfdx-project.json is fine — extension still serves Apex highlighting
    // and a single-file LSP session. Only surface an error when the file exists
    // but is malformed enough that we can't trust autodiscovery.
    let Ok(json) = worktree.read_text_file("sfdx-project.json") else {
        return Ok(Vec::new());
    };
    let root: serde_json::Value = serde_json::from_str(&json).map_err(|err| {
        format!(
            "sfdx-project.json at the worktree root is not valid JSON: {err}. \
             Fix the file or override package paths via lsp.apex-language-server.settings.aer_source_paths."
        )
    })?;
    let Some(dirs) = root.get("packageDirectories").and_then(|v| v.as_array()) else {
        return Ok(Vec::new());
    };
    let root_path = worktree.root_path();
    let args: Vec<String> = dirs
        .iter()
        .filter_map(|d| d.get("path")?.as_str())
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .map(|p| normalize_source_path(&root_path, &p))
        .collect();
    Ok(args)
}

fn resolve_aer_binary(
    lsp_settings: &zed::settings::LspSettings,
    worktree: &zed::Worktree,
) -> zed::Result<String> {
    // Priority 1: lsp.apex-language-server.binary.path
    if let Some(binary) = &lsp_settings.binary {
        if let Some(path) = &binary.path {
            return Ok(path.clone());
        }
    }
    // Priority 2: lsp.apex-language-server.settings.aer_path
    if let Some(aer_path) = setting_value(lsp_settings, "aer_path")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        return Ok(aer_path);
    }
    // Priority 3: worktree.which
    if let Some(path) = worktree.which(AER_BINARY_NAME) {
        return Ok(path);
    }
    Err(format!(
        "aer binary not found. Install from https://github.com/octoberswimmer/aer-dist/ \
         or set 'aer_path' in lsp.apex-language-server.settings"
    ))
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
    use super::normalize_source_path;

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
}
