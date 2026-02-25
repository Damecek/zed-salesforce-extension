use zed_extension_api as zed;
use zed_extension_api::serde_json;

const EXTENSION_ID: &str = "salesforce-dx";
const APEX_LSP_ID: &str = "apex-lsp";
const APEX_LSP_MAIN_CLASS: &str = "apex.jorje.lsp.ApexLanguageServerLauncher";
const APEX_LSP_JAR_REL_PATH: &str = "vendor/apex-jorje-lsp.jar";
const DEFAULT_JAVA_MAX_HEAP_MB: u64 = 2048;
const SFDX_DISCOVERY_MAX_DEPTH: u8 = 3;

const APEX_LSP_PROXY_REL_PATH: &str = "vendor/apex_lsp_proxy.py";

// Apex LSP currently depends on LSP InitializeParams.rootPath. Zed sends rootUri but may leave
// rootPath null, which causes Apex LSP to crash when it tries to build its `.sfdx/tools/...` DB
// path. This proxy injects `rootPath` (from `--root-path`) into the initialize request and
// attempts to auto-discover an SFDX project root in nested folders.
const APEX_LSP_PROXY_PY: &str = r#"#!/usr/bin/env python3
import argparse
import json
import os
import subprocess
import sys
import threading
from collections import deque
from urllib.parse import urlparse, unquote

SFDX_PROJECT_JSON = "sfdx-project.json"
MAX_SFDX_SCAN_DEPTH = __SFDX_DISCOVERY_MAX_DEPTH__
SKIP_DIR_NAMES = {
    ".git",
    ".hg",
    ".svn",
    ".idea",
    ".vscode",
    ".zed",
    "node_modules",
    "dist",
    "build",
    "target",
}


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
    candidates = []

    rp = params.get("rootPath")
    if isinstance(rp, str) and rp.strip():
        candidates.append(rp)

    ru = params.get("rootUri")
    if isinstance(ru, str) and ru.startswith("file:"):
        u = urlparse(ru)
        if u.path:
            candidates.append(unquote(u.path))

    wfs = params.get("workspaceFolders")
    if isinstance(wfs, list):
        for wf in wfs:
            if not isinstance(wf, dict):
                continue
            uri = wf.get("uri")
            if isinstance(uri, str) and uri.startswith("file:"):
                u = urlparse(uri)
                if u.path:
                    candidates.append(unquote(u.path))

    if isinstance(fallback_root_path, str) and fallback_root_path.strip():
        candidates.append(fallback_root_path)

    seen = set()
    for candidate in candidates:
        normalized = os.path.abspath(candidate)
        if normalized in seen:
            continue
        seen.add(normalized)
        found = find_sfdx_project_root(normalized)
        if found:
            return found

    if candidates:
        fallback = os.path.abspath(candidates[0])
        if os.path.isfile(fallback):
            return os.path.dirname(fallback)
        return fallback
    return fallback_root_path


def find_sfdx_project_root(start_path):
    if not start_path:
        return None

    start_path = os.path.abspath(start_path)
    if os.path.isfile(start_path):
        start_path = os.path.dirname(start_path)
    if not os.path.isdir(start_path):
        return None

    queue = deque([(start_path, 0)])
    seen = set()
    while queue:
        current, depth = queue.popleft()
        if current in seen:
            continue
        seen.add(current)

        marker = os.path.join(current, SFDX_PROJECT_JSON)
        if os.path.isfile(marker):
            return current

        if depth >= MAX_SFDX_SCAN_DEPTH:
            continue

        try:
            entries = list(os.scandir(current))
        except OSError:
            continue

        for entry in entries:
            if not entry.is_dir(follow_symlinks=False):
                continue
            if entry.name in SKIP_DIR_NAMES:
                continue
            queue.append((entry.path, depth + 1))

    return None


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

        // Apex LSP relies on InitializeParams.rootPath; Zed may only send rootUri. We run a tiny
        // stdio proxy that injects rootPath and can resolve nested SFDX roots.
        let (proxy_cmd, proxy_args) = ensure_and_build_proxy_command(worktree, &shell_env, &java_command, &jvm_args)?;

        Ok(zed::Command {
            command: proxy_cmd,
            args: proxy_args,
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
    let proxy_contents = APEX_LSP_PROXY_PY.replace(
        "__SFDX_DISCOVERY_MAX_DEPTH__",
        &SFDX_DISCOVERY_MAX_DEPTH.to_string(),
    );
    std::fs::write(&proxy_path, proxy_contents.as_bytes()).map_err(|err| err.to_string())?;

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

fn value_to_u64(value: &serde_json::Value) -> Option<u64> {
    match value {
        serde_json::Value::Number(number) => number.as_u64(),
        serde_json::Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}
