use zed_extension_api as zed;
use zed_extension_api::serde_json;

use std::collections::HashSet;

const EXTENSION_ID: &str = "salesforce-dx";
const APEX_LSP_ID: &str = "apex-lsp";
const APEX_LSP_BACKEND_SETTING: &str = "backend";
const APEX_LSP_BACKEND_JAVA: &str = "apex_jorje_java";
const APEX_LSP_BACKEND_NODE: &str = "apex_language_support";
const APEX_LSP_MAIN_CLASS: &str = "apex.jorje.lsp.ApexLanguageServerLauncher";
const APEX_LSP_JAR_REL_PATH: &str = "vendor/apex-jorje-lsp.jar";
const APEX_LANGUAGE_SUPPORT_ENTRY_REL_PATH: &str = "vendor/apex-language-support/index.js";
const SFDX_PROJECT_JSON: &str = "sfdx-project.json";
const DEFAULT_JAVA_MAX_HEAP_MB: u64 = 2048;
const APEX_LANGUAGE_SUPPORT_ENTRY_SETTING: &str = "apex_language_support_entry";
const APEX_LANGUAGE_SUPPORT_NODE_PATH_SETTING: &str = "apex_language_support_node_path";
const APEX_LANGUAGE_SUPPORT_NODE_HOME_SETTING: &str = "apex_language_support_node_home";
const APEX_LANGUAGE_SUPPORT_ARGS_SETTING: &str = "apex_language_support_args";
const APEX_LANGUAGE_SUPPORT_LOG_LEVEL_SETTING: &str = "apex_language_support_log_level";
const APEX_LANGUAGE_SUPPORT_EXTENSION_MODE_SETTING: &str = "apex_language_support_extension_mode";
const DEFAULT_APEX_LANGUAGE_SUPPORT_LOG_LEVEL: &str = "info";

const APEX_LSP_PROXY_REL_PATH: &str = "vendor/apex_lsp_proxy.py";
const APEX_NODE_LSP_PROXY_REL_PATH: &str = "vendor/apex_node_lsp_proxy.py";

// Apex LSP currently depends on LSP InitializeParams.rootPath. Zed sends rootUri but may leave
// rootPath null, which causes Apex LSP to crash when it tries to build its `.sfdx/tools/...` DB
// path. This proxy injects `rootPath` (from `--root-path`) into the initialize request.
const APEX_LSP_PROXY_PY: &str = r#"#!/usr/bin/env python3
import argparse
import json
import os
import subprocess
import sys
import threading
from urllib.parse import urlparse, unquote


def read_headers(stream):
    headers = {}
    while True:
        line = stream.readline()
        if not line:
            return None
        if line in (b"\r\n", b"\n"):
            return headers
        try:
            s = line.decode("ascii", errors="replace").strip()
        except Exception:
            continue
        if ":" in s:
            k, v = s.split(":", 1)
            headers[k.strip().lower()] = v.strip()


def read_message(stream):
    headers = read_headers(stream)
    if headers is None:
        return None, None, None
    if "content-length" not in headers:
        # invalid; bail so we don't desync the stream
        return None, None, None
    n = int(headers["content-length"])
    body = stream.read(n)
    return headers, body, n


def write_message(stream, body_bytes, headers=None):
    if headers is None:
        headers = {}
    # Always rewrite Content-Length to match the modified body
    out = []
    # Preserve content-type if present (optional)
    ct = headers.get("content-type")
    if ct:
        out.append(f"Content-Type: {ct}\r\n".encode("ascii"))
    out.append(f"Content-Length: {len(body_bytes)}\r\n\r\n".encode("ascii"))
    stream.write(b"".join(out))
    stream.write(body_bytes)
    stream.flush()


def derive_root_path(params, fallback_root_path):
    rp = params.get("rootPath")
    if isinstance(rp, str) and rp.strip():
        return rp
    ru = params.get("rootUri")
    if isinstance(ru, str) and ru.startswith("file:"):
        u = urlparse(ru)
        if u.path:
            return unquote(u.path)
    wfs = params.get("workspaceFolders")
    if isinstance(wfs, list) and wfs:
        uri = wfs[0].get("uri")
        if isinstance(uri, str) and uri.startswith("file:"):
            u = urlparse(uri)
            if u.path:
                return unquote(u.path)
    return fallback_root_path


def pump_stdin_to_java(java_proc, root_path):
    while True:
        headers, body, _ = read_message(sys.stdin.buffer)
        if headers is None:
            try:
                java_proc.stdin.close()
            except Exception:
                pass
            return

        try:
            msg = json.loads(body.decode("utf-8", errors="strict"))
        except Exception:
            # Forward raw message if we can't parse JSON (shouldn't happen)
            write_message(java_proc.stdin, body, headers=headers)
            continue

        if msg.get("method") == "initialize" and isinstance(msg.get("params"), dict):
            params = msg["params"]
            params["rootPath"] = derive_root_path(params, root_path)
            msg["params"] = params
            body = json.dumps(msg, separators=(",", ":"), ensure_ascii=False).encode("utf-8")

        write_message(java_proc.stdin, body, headers=headers)


def pump_java_to_stdout(java_proc):
    while True:
        headers, body, _ = read_message(java_proc.stdout)
        if headers is None:
            return
        write_message(sys.stdout.buffer, body, headers=headers)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root-path", required=True)
    ap.add_argument("--java-cmd", required=True)
    ap.add_argument("java_args", nargs=argparse.REMAINDER)
    args = ap.parse_args()

    if args.java_args and args.java_args[0] == "--":
        args.java_args = args.java_args[1:]

    java_proc = subprocess.Popen(
        [args.java_cmd] + args.java_args,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=None,  # inherit
    )

    t1 = threading.Thread(target=pump_stdin_to_java, args=(java_proc, args.root_path), daemon=True)
    t2 = threading.Thread(target=pump_java_to_stdout, args=(java_proc,), daemon=True)
    t1.start()
    t2.start()

    return java_proc.wait()


if __name__ == "__main__":
    sys.exit(main())
"#;

const APEX_NODE_LSP_PROXY_PY: &str = r#"#!/usr/bin/env python3
import argparse
import json
import subprocess
import sys


def read_headers(stream):
    headers = {}
    while True:
        line = stream.readline()
        if not line:
            return None
        if line in (b"\r\n", b"\n"):
            return headers
        try:
            s = line.decode("ascii", errors="replace").strip()
        except Exception:
            continue
        if ":" in s:
            k, v = s.split(":", 1)
            headers[k.strip().lower()] = v.strip()


def read_message(stream):
    headers = read_headers(stream)
    if headers is None:
        return None, None, None
    if "content-length" not in headers:
        return None, None, None
    n = int(headers["content-length"])
    body = stream.read(n)
    return headers, body, n


def write_message(stream, body_bytes, headers=None):
    if headers is None:
        headers = {}
    out = []
    ct = headers.get("content-type")
    if ct:
        out.append(f"Content-Type: {ct}\r\n".encode("ascii"))
    out.append(f"Content-Length: {len(body_bytes)}\r\n\r\n".encode("ascii"))
    stream.write(b"".join(out))
    stream.write(body_bytes)
    stream.flush()


def patch_initialize(msg, log_level, extension_mode):
    if msg.get("method") != "initialize" or not isinstance(msg.get("params"), dict):
        return msg

    params = msg["params"]
    init_opts = params.get("initializationOptions")
    if not isinstance(init_opts, dict):
        init_opts = {}

    if log_level and not init_opts.get("logLevel"):
        init_opts["logLevel"] = log_level
    if extension_mode and not init_opts.get("extensionMode"):
        init_opts["extensionMode"] = extension_mode

    params["initializationOptions"] = init_opts
    msg["params"] = params
    return msg


def normalize_diagnostic_response(msg, pending_requests):
    if not isinstance(msg, dict):
        return msg
    if "id" not in msg or "result" not in msg:
        return msg

    req_method = pending_requests.pop(msg.get("id"), None)
    if req_method != "textDocument/diagnostic":
        return msg

    result = msg.get("result")
    if isinstance(result, list):
        msg["result"] = {"kind": "full", "items": result}
    return msg


def pump_stdin_to_node(node_proc, log_level, extension_mode, pending_requests):
    while True:
        headers, body, _ = read_message(sys.stdin.buffer)
        if headers is None:
            try:
                node_proc.stdin.close()
            except Exception:
                pass
            return

        try:
            msg = json.loads(body.decode("utf-8", errors="strict"))
            msg = patch_initialize(msg, log_level, extension_mode)
            if "id" in msg and isinstance(msg.get("method"), str):
                pending_requests[msg["id"]] = msg["method"]
            body = json.dumps(msg, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
        except Exception:
            pass

        write_message(node_proc.stdin, body, headers=headers)


def pump_node_to_stdout(node_proc, pending_requests):
    while True:
        headers, body, _ = read_message(node_proc.stdout)
        if headers is None:
            return
        try:
            msg = json.loads(body.decode("utf-8", errors="strict"))
            msg = normalize_diagnostic_response(msg, pending_requests)
            body = json.dumps(msg, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
        except Exception:
            pass
        write_message(sys.stdout.buffer, body, headers=headers)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--node-cmd", required=True)
    ap.add_argument("--log-level", default="")
    ap.add_argument("--extension-mode", default="")
    ap.add_argument("node_args", nargs=argparse.REMAINDER)
    args = ap.parse_args()

    node_args = args.node_args
    if node_args and node_args[0] == "--":
        node_args = node_args[1:]

    node_proc = subprocess.Popen(
        [args.node_cmd] + node_args,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=None,  # inherit
    )

    import threading
    pending_requests = {}

    t1 = threading.Thread(
        target=pump_stdin_to_node,
        args=(node_proc, args.log_level, args.extension_mode, pending_requests),
        daemon=True,
    )
    t2 = threading.Thread(target=pump_node_to_stdout, args=(node_proc, pending_requests), daemon=True)
    t1.start()
    t2.start()

    return node_proc.wait()


if __name__ == "__main__":
    sys.exit(main())
"#;

struct SalesforceExtension {
    warned_missing_sfdx: HashSet<u64>,
}

enum ApexLspBackend {
    ApexJorjeJava,
    ApexLanguageSupport,
}

impl zed::Extension for SalesforceExtension {
    fn new() -> Self {
        Self {
            warned_missing_sfdx: HashSet::new(),
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        if language_server_id.as_ref() != APEX_LSP_ID {
            return Err(format!("Unknown language server id: {language_server_id}"));
        }

        // MVP guardrail: Apex LSP expects an SFDX workspace root.
        // If missing, we intentionally skip starting the server.
        if worktree.read_text_file(SFDX_PROJECT_JSON).is_err() {
            let id = worktree.id();
            if self.warned_missing_sfdx.insert(id) {
                eprintln!(
                    "[salesforce] Apex LSP not started: `{SFDX_PROJECT_JSON}` not found at worktree root ({}). Open the Salesforce DX project root folder to enable LSP.",
                    worktree.root_path()
                );
            }
            return Err(format!(
                "Apex LSP skipped (missing `{SFDX_PROJECT_JSON}` at worktree root)."
            ));
        }

        let shell_env = worktree.shell_env();
        let lsp_settings = zed::settings::LspSettings::for_worktree(APEX_LSP_ID, worktree)
            .unwrap_or_default();

        match resolve_backend(&lsp_settings) {
            ApexLspBackend::ApexJorjeJava => {
                let jar_path = resolve_apex_lsp_jar_path()?;

                let (java_command, mut jvm_args) =
                    resolve_java_command(&lsp_settings, &shell_env, worktree);
                jvm_args.push("-cp".to_string());
                jvm_args.push(jar_path);
                jvm_args.push(APEX_LSP_MAIN_CLASS.to_string());

                // Apex jorje LSP relies on InitializeParams.rootPath; Zed may only send rootUri.
                // We run a tiny stdio proxy that injects rootPath based on the worktree root.
                let (proxy_cmd, proxy_args) =
                    ensure_and_build_proxy_command(worktree, &shell_env, &java_command, &jvm_args)?;

                Ok(zed::Command {
                    command: proxy_cmd,
                    args: proxy_args,
                    env: shell_env,
                })
            }
            ApexLspBackend::ApexLanguageSupport => {
                build_apex_language_support_command(&lsp_settings, &shell_env, worktree)
            }
        }
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
    let installed_dir = resolve_extension_installed_dir()?;
    Ok(installed_dir
        .join(APEX_LSP_JAR_REL_PATH)
        .to_string_lossy()
        .into_owned())
}

fn resolve_apex_language_support_entry_path(
    lsp_settings: &zed::settings::LspSettings,
) -> zed::Result<String> {
    if let Some(entry) = setting_string(lsp_settings, APEX_LANGUAGE_SUPPORT_ENTRY_SETTING) {
        return Ok(entry);
    }

    let installed_dir = resolve_extension_installed_dir()?;
    Ok(installed_dir
        .join(APEX_LANGUAGE_SUPPORT_ENTRY_REL_PATH)
        .to_string_lossy()
        .into_owned())
}

fn resolve_extension_installed_dir() -> zed::Result<std::path::PathBuf> {
    let work_dir = std::env::current_dir().map_err(|err| err.to_string())?;

    work_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|extensions_dir| extensions_dir.join("installed").join(EXTENSION_ID))
        .ok_or_else(|| format!("Could not derive extension installed directory from {}", work_dir.display()))
}

fn resolve_backend(lsp_settings: &zed::settings::LspSettings) -> ApexLspBackend {
    let Some(raw_backend) = setting_string(lsp_settings, APEX_LSP_BACKEND_SETTING) else {
        return ApexLspBackend::ApexJorjeJava;
    };
    let normalized = raw_backend.to_ascii_lowercase();

    match normalized.as_str() {
        APEX_LSP_BACKEND_JAVA | "java" | "jorje" => ApexLspBackend::ApexJorjeJava,
        APEX_LSP_BACKEND_NODE | "node" | "nodejs" => ApexLspBackend::ApexLanguageSupport,
        _ => {
            eprintln!(
                "[salesforce] Unsupported apex LSP backend `{raw_backend}`. Falling back to `{APEX_LSP_BACKEND_JAVA}`."
            );
            ApexLspBackend::ApexJorjeJava
        }
    }
}

fn build_apex_language_support_command(
    lsp_settings: &zed::settings::LspSettings,
    shell_env: &zed::EnvVars,
    worktree: &zed::Worktree,
) -> zed::Result<zed::Command> {
    let entry = resolve_apex_language_support_entry_path(lsp_settings)?;
    let node_command = resolve_node_command(lsp_settings, shell_env, worktree);

    let mut args = Vec::new();
    args.push(entry);

    let mut extra_args =
        setting_string_vec(lsp_settings, APEX_LANGUAGE_SUPPORT_ARGS_SETTING).unwrap_or_default();
    if !extra_args.iter().any(|arg| arg == "--stdio") {
        args.push("--stdio".to_string());
    }
    args.append(&mut extra_args);

    let log_level = setting_string(lsp_settings, APEX_LANGUAGE_SUPPORT_LOG_LEVEL_SETTING)
        .unwrap_or_else(|| DEFAULT_APEX_LANGUAGE_SUPPORT_LOG_LEVEL.to_string());
    let extension_mode = setting_string(lsp_settings, APEX_LANGUAGE_SUPPORT_EXTENSION_MODE_SETTING);
    let (proxy_cmd, proxy_args) =
        ensure_and_build_apex_node_proxy_command(worktree, &node_command, &args, &log_level, extension_mode.as_deref())?;

    Ok(zed::Command {
        command: proxy_cmd,
        args: proxy_args,
        env: shell_env.clone(),
    })
}

fn ensure_and_build_apex_node_proxy_command(
    worktree: &zed::Worktree,
    node_command: &str,
    node_args: &[String],
    log_level: &str,
    extension_mode: Option<&str>,
) -> zed::Result<(String, Vec<String>)> {
    let work_dir = std::env::current_dir().map_err(|err| err.to_string())?;
    let proxy_path = work_dir.join(APEX_NODE_LSP_PROXY_REL_PATH);

    std::fs::create_dir_all(
        proxy_path
            .parent()
            .ok_or_else(|| "Invalid node proxy path".to_string())?,
    )
    .map_err(|err| err.to_string())?;

    std::fs::write(&proxy_path, APEX_NODE_LSP_PROXY_PY.as_bytes()).map_err(|err| err.to_string())?;
    let _ = zed::make_file_executable(APEX_NODE_LSP_PROXY_REL_PATH);

    let python = worktree
        .which("python3")
        .or_else(|| worktree.which("python"))
        .unwrap_or_else(|| "python3".to_string());

    let mut args = Vec::new();
    args.push(proxy_path.to_string_lossy().into_owned());
    args.push("--node-cmd".to_string());
    args.push(node_command.to_string());
    args.push("--log-level".to_string());
    args.push(log_level.to_string());
    if let Some(mode) = extension_mode {
        if !mode.trim().is_empty() {
            args.push("--extension-mode".to_string());
            args.push(mode.trim().to_string());
        }
    }
    args.push("--".to_string());
    args.extend(node_args.iter().cloned());

    Ok((python, args))
}

fn resolve_node_command(
    lsp_settings: &zed::settings::LspSettings,
    shell_env: &zed::EnvVars,
    worktree: &zed::Worktree,
) -> String {
    if let Some(path) = setting_string(lsp_settings, APEX_LANGUAGE_SUPPORT_NODE_PATH_SETTING) {
        return path;
    }

    if let Some(node_home) = setting_string(lsp_settings, APEX_LANGUAGE_SUPPORT_NODE_HOME_SETTING)
        .or_else(|| env_var(shell_env, "NODE_HOME"))
    {
        let cmd = std::path::Path::new(&node_home).join("bin").join("node");
        return cmd.to_string_lossy().into_owned();
    }

    if let Some(path) = worktree.which("node") {
        return path;
    }

    "node".to_string()
}

fn ensure_and_build_proxy_command(
    worktree: &zed::Worktree,
    _shell_env: &zed::EnvVars,
    java_command: &str,
    java_args: &[String],
) -> zed::Result<(String, Vec<String>)> {
    let work_dir = std::env::current_dir().map_err(|err| err.to_string())?;
    let proxy_path = work_dir.join(APEX_LSP_PROXY_REL_PATH);

    std::fs::create_dir_all(
        proxy_path
            .parent()
            .ok_or_else(|| "Invalid proxy path".to_string())?,
    )
    .map_err(|err| err.to_string())?;

    // Write/overwrite; it's tiny and avoids version skew.
    std::fs::write(&proxy_path, APEX_LSP_PROXY_PY.as_bytes()).map_err(|err| err.to_string())?;

    // Make it executable for easier debugging, but we still invoke via python.
    let _ = zed::make_file_executable(APEX_LSP_PROXY_REL_PATH);

    let python = worktree
        .which("python3")
        .or_else(|| worktree.which("python"))
        .unwrap_or_else(|| "python3".to_string());

    let root_path = worktree.root_path();

    let mut args = Vec::new();
    args.push(proxy_path.to_string_lossy().into_owned());
    args.push("--root-path".to_string());
    args.push(root_path);
    args.push("--java-cmd".to_string());
    args.push(java_command.to_string());
    args.push("--".to_string());
    args.extend(java_args.iter().cloned());

    Ok((python, args))
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

fn setting_string(lsp_settings: &zed::settings::LspSettings, key: &str) -> Option<String> {
    setting_value(lsp_settings, key)
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn setting_string_vec(lsp_settings: &zed::settings::LspSettings, key: &str) -> Option<Vec<String>> {
    value_to_string_vec(setting_value(lsp_settings, key)?)
}

fn value_to_u64(value: &serde_json::Value) -> Option<u64> {
    match value {
        serde_json::Value::Number(number) => number.as_u64(),
        serde_json::Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

fn value_to_string_vec(value: &serde_json::Value) -> Option<Vec<String>> {
    match value {
        serde_json::Value::Array(items) => {
            let parsed: Vec<String> = items
                .iter()
                .filter_map(|item| item.as_str())
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect();
            Some(parsed)
        }
        serde_json::Value::String(item) => {
            let text = item.trim();
            if text.is_empty() {
                Some(Vec::new())
            } else {
                Some(vec![text.to_string()])
            }
        }
        _ => None,
    }
}
