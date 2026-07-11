#!/usr/bin/env python3
"""Verify diagnostics from the pinned Visualforce server through its Zed shim."""

import argparse
import json
import os
import subprocess
import tempfile
from pathlib import Path

from lsp_test_protocol import file_uri, read_message, write_message

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 compatibility
    import tomli as tomllib


VALID_PROBE_TEXT = """<apex:page>
    <style>
        .valid { color: red; }
    </style>
    <script>
        const valid = 1;
    </script>
</apex:page>
"""
INVALID_PROBE_TEXT = """<apex:page>
    <style>
        .broken { color: ; }
    </style>
    <script>
        const broken = ;
    </script>
</apex:page>
"""


def wait_for_response(proc, response_id, timeout_seconds):
    while True:
        message = read_message(proc.stdout, timeout_seconds)
        if message is None:
            raise RuntimeError(f"Timed out waiting for response {response_id}")
        if message.get("id") == response_id and "method" not in message:
            return message


def wait_for_diagnostics(proc, document_uri, timeout_seconds):
    while True:
        message = read_message(proc.stdout, timeout_seconds)
        if message is None:
            raise RuntimeError("Timed out waiting for textDocument/publishDiagnostics")
        if message.get("method") != "textDocument/publishDiagnostics":
            continue
        params = message.get("params", {})
        if params.get("uri") == document_uri:
            return params.get("diagnostics", [])


def collect_diagnostics(node, wrapper_path, server_path, language_id, timeout_seconds):
    with tempfile.TemporaryDirectory(prefix="visualforce-diagnostics-") as temp_dir:
        workspace = Path(temp_dir)
        document = workspace / "DiagnosticsProbe.page"
        document.write_text(VALID_PROBE_TEXT, encoding="utf-8")
        document_uri = file_uri(document)
        proc = subprocess.Popen(
            [node, str(wrapper_path), str(server_path), "--stdio"],
            cwd=workspace,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env={**os.environ, "LC_ALL": "C"},
        )
        try:
            write_message(
                proc.stdin,
                {
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize",
                    "params": {
                        "processId": os.getpid(),
                        "rootPath": str(workspace),
                        "rootUri": file_uri(workspace),
                        "capabilities": {},
                        "initializationOptions": {
                            "embeddedLanguages": {"css": True, "javascript": True}
                        },
                    },
                },
            )
            initialize = wait_for_response(proc, 1, timeout_seconds)
            if "result" not in initialize:
                raise RuntimeError(f"Unexpected initialize response: {initialize}")
            write_message(
                proc.stdin,
                {"jsonrpc": "2.0", "method": "initialized", "params": {}},
            )
            write_message(
                proc.stdin,
                {
                    "jsonrpc": "2.0",
                    "method": "textDocument/didOpen",
                    "params": {
                        "textDocument": {
                            "uri": document_uri,
                            "languageId": language_id,
                            "version": 1,
                            "text": VALID_PROBE_TEXT,
                        }
                    },
                },
            )
            opened = wait_for_diagnostics(proc, document_uri, timeout_seconds)
            write_message(
                proc.stdin,
                {
                    "jsonrpc": "2.0",
                    "method": "textDocument/didChange",
                    "params": {
                        "textDocument": {"uri": document_uri, "version": 2},
                        "contentChanges": [{"text": INVALID_PROBE_TEXT}],
                    },
                },
            )
            changed = wait_for_diagnostics(proc, document_uri, timeout_seconds)
            write_message(
                proc.stdin,
                {"jsonrpc": "2.0", "id": 2, "method": "shutdown", "params": None},
            )
            wait_for_response(proc, 2, timeout_seconds)
            write_message(
                proc.stdin,
                {"jsonrpc": "2.0", "method": "exit", "params": None},
            )
            proc.stdin.close()
            proc.wait(timeout=timeout_seconds)
            return {"didOpen": opened, "didChange": changed}
        except Exception:
            proc.kill()
            proc.wait(timeout=5)
            raise


def main():
    repo_root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser()
    parser.add_argument("--node", default="node")
    parser.add_argument(
        "--wrapper",
        type=Path,
        default=repo_root / "scripts/visualforce-language-server-wrapper.js",
    )
    parser.add_argument(
        "--server",
        type=Path,
        default=(
            repo_root
            / ".cache/visualforce-language-server/v67.4.0/extension/dist/visualforceServer.js"
        ),
    )
    parser.add_argument("--timeout-seconds", type=int, default=5)
    args = parser.parse_args()

    if not args.server.is_file():
        raise SystemExit(f"Pinned Visualforce server is missing: {args.server}")
    if not args.wrapper.is_file():
        raise SystemExit(f"Visualforce diagnostic shim is missing: {args.wrapper}")

    with (repo_root / "extension.toml").open("rb") as stream:
        manifest = tomllib.load(stream)
    mapped_language_id = manifest["language_servers"]["visualforce-language-server"][
        "language_ids"
    ]["Visualforce"]

    results = collect_diagnostics(
        args.node,
        args.wrapper.resolve(),
        args.server.resolve(),
        mapped_language_id,
        args.timeout_seconds,
    )
    print(json.dumps(results, indent=2, sort_keys=True))

    if results["didOpen"]:
        raise SystemExit("The valid didOpen probe unexpectedly produced diagnostics")

    diagnostics = results["didChange"]
    if not any(item.get("source") == "css" for item in diagnostics):
        raise SystemExit("The shim did not preserve the expected CSS diagnostic")
    if not any("Expression expected" in item.get("message", "") for item in diagnostics):
        raise SystemExit("The shim did not preserve the expected JavaScript diagnostic")

    print(
        f"Visualforce diagnostic smoke passed with manifest language id {mapped_language_id!r}: "
        f"{len(diagnostics)} diagnostics after didChange."
    )


if __name__ == "__main__":
    main()
