#!/usr/bin/env python3
import argparse
import json
import os
import select
import subprocess
import sys
from pathlib import Path
from urllib.parse import quote


def write_message(stream, payload):
    body = json.dumps(payload, separators=(",", ":")).encode("utf-8")
    stream.write(f"Content-Length: {len(body)}\r\n\r\n".encode("ascii"))
    stream.write(body)
    stream.flush()


def wait_readable(stream, timeout_seconds):
    ready, _, _ = select.select([stream], [], [], timeout_seconds)
    return bool(ready)


def read_message(stream, timeout_seconds):
    headers = {}
    while True:
        if not wait_readable(stream, timeout_seconds):
            return None
        line = stream.readline()
        if not line:
            return None
        if line in (b"\r\n", b"\n"):
            break
        decoded = line.decode("ascii", errors="replace").strip()
        if ":" in decoded:
            key, value = decoded.split(":", 1)
            headers[key.strip().lower()] = value.strip()

    content_length = headers.get("content-length")
    if content_length is None:
        return None

    length = int(content_length)
    if not wait_readable(stream, timeout_seconds):
        return None
    body = stream.read(length)
    if not body:
        return None
    return json.loads(body.decode("utf-8", errors="strict"))


def file_uri(path):
    return "file://" + quote(str(path), safe="/")


def wait_for_response(stream, response_id, timeout_seconds, max_messages=200):
    for _ in range(max_messages):
        message = read_message(stream, timeout_seconds)
        if message is None:
            return None
        if message.get("id") == response_id:
            return message
    return None


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--java", required=True)
    parser.add_argument("--jar", required=True)
    parser.add_argument("--workspace", required=True)
    parser.add_argument("--timeout-seconds", type=int, default=20)
    args = parser.parse_args()

    workspace = Path(args.workspace).resolve()
    if not workspace.exists():
        raise SystemExit(f"Workspace does not exist: {workspace}")

    java_cmd = [
        args.java,
        "-cp",
        str(Path(args.jar).resolve()),
        "apex.jorje.lsp.ApexLanguageServerLauncher",
    ]

    env = os.environ.copy()
    env.setdefault("LC_ALL", "C")

    proc = subprocess.Popen(
        java_cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=False,
        env=env,
    )

    try:
        initialize = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": None,
                "rootPath": str(workspace),
                "rootUri": file_uri(workspace),
                "workspaceFolders": [
                    {"uri": file_uri(workspace), "name": workspace.name}
                ],
                "capabilities": {},
                "clientInfo": {"name": "zed-salesforce-smoke", "version": "0.1"},
            },
        }
        write_message(proc.stdin, initialize)

        response = wait_for_response(proc.stdout, 1, args.timeout_seconds)
        if response is None:
            stderr = proc.stderr.read().decode("utf-8", errors="replace")
            raise RuntimeError(f"No initialize response from LSP process.\n{stderr}")
        if response.get("id") != 1 or "result" not in response:
            raise RuntimeError(f"Unexpected initialize response: {response}")

        write_message(
            proc.stdin,
            {"jsonrpc": "2.0", "id": 2, "method": "shutdown", "params": None},
        )
        shutdown_response = wait_for_response(proc.stdout, 2, args.timeout_seconds)
        if shutdown_response is None or shutdown_response.get("id") != 2:
            raise RuntimeError(f"Unexpected shutdown response: {shutdown_response}")

        write_message(proc.stdin, {"jsonrpc": "2.0", "method": "exit", "params": None})
        proc.stdin.close()
        proc.wait(timeout=args.timeout_seconds)
    except Exception as exc:
        proc.kill()
        proc.wait(timeout=5)
        raise SystemExit(str(exc))

    print("Apex LSP smoke test passed: initialize/shutdown handshake succeeded.")


if __name__ == "__main__":
    main()
