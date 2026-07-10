#!/usr/bin/env python3
"""Smoke-test Salesforce's pinned Visualforce language server over stdio."""

import argparse
import hashlib
import json
import os
import select
import shutil
import subprocess
import sys
import tempfile
import urllib.request
import zipfile
from pathlib import Path
from urllib.parse import quote


RELEASE = "v67.4.0"
VSIX_NAME = "salesforcedx-vscode-visualforce-67.4.0.vsix"
VSIX_URL = (
    "https://github.com/forcedotcom/salesforcedx-vscode/releases/download/"
    f"{RELEASE}/{VSIX_NAME}"
)
VSIX_SHA256 = "6232bb3dc3bdfe2c491601b9c96c488fb52941c2ff62bcc125230e4dceacbb0c"
SERVER_REL_PATH = Path("extension/dist/visualforceServer.js")
SERVER_SHA256 = "37f6808e5e4bd360f7c7f219fd2d71cc8d7ce22688b271c1a4ae5020bd85bb3f"
COMPLETION_MARKER = "<!-- VISUALFORCE_COMPLETION_PROBE -->"
COMPLETION_PREFIX = "<apex:"


class IntegrityError(RuntimeError):
    def __init__(self, path, expected, actual):
        self.path = Path(path)
        self.expected = expected
        self.actual = actual
        super().__init__(
            f"SHA-256 verification failed for {self.path}: "
            f"expected {expected}; actual {actual}"
        )


def sha256_file(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify_file(path, expected):
    path = Path(path)
    if not path.is_file():
        raise IntegrityError(path, expected, "missing")
    actual = sha256_file(path)
    if actual != expected:
        raise IntegrityError(path, expected, actual)
    return actual


def download_once(url, destination):
    destination = Path(destination)
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = destination.with_suffix(destination.suffix + ".part")
    temporary.unlink(missing_ok=True)
    try:
        with urllib.request.urlopen(url, timeout=60) as response:
            with temporary.open("wb") as output:
                shutil.copyfileobj(response, output)
        verify_file(temporary, VSIX_SHA256)
        temporary.replace(destination)
    finally:
        temporary.unlink(missing_ok=True)


def ensure_vsix(url, archive_path):
    archive_path = Path(archive_path)
    if archive_path.is_file():
        try:
            verify_file(archive_path, VSIX_SHA256)
            print(f"Reused verified cached VSIX: {archive_path}")
            return archive_path
        except IntegrityError as error:
            print(f"Discarding invalid cached VSIX: {error}")
            archive_path.unlink()

    print(f"Downloading official Visualforce VSIX {RELEASE}: {url}")
    download_once(url, archive_path)
    print(f"Downloaded and verified VSIX SHA-256 {VSIX_SHA256}")
    return archive_path


def safe_extract_zip(archive_path, destination):
    destination = Path(destination)
    destination_root = destination.resolve()
    with zipfile.ZipFile(archive_path) as archive:
        for member in archive.infolist():
            member_path = (destination / member.filename).resolve()
            if not member_path.is_relative_to(destination_root):
                raise RuntimeError(f"Refusing unsafe VSIX path: {member.filename}")
        archive.extractall(destination)


def extract_verified_server(archive_path, version_dir):
    version_dir = Path(version_dir)
    temporary = version_dir.with_name(f"{version_dir.name}.extracting-{os.getpid()}")
    shutil.rmtree(temporary, ignore_errors=True)
    temporary.mkdir(parents=True)
    try:
        safe_extract_zip(archive_path, temporary)
        temporary_server = temporary / SERVER_REL_PATH
        verify_file(temporary_server, SERVER_SHA256)
        shutil.rmtree(version_dir, ignore_errors=True)
        temporary.replace(version_dir)
    finally:
        shutil.rmtree(temporary, ignore_errors=True)
    return version_dir / SERVER_REL_PATH


def ensure_server(archive_path, version_dir):
    server_path = Path(version_dir) / SERVER_REL_PATH
    if server_path.is_file():
        try:
            verify_file(server_path, SERVER_SHA256)
            print(f"Reused verified extracted server: {server_path}")
            return server_path
        except IntegrityError as error:
            print(f"Refreshing invalid extracted server: {error}")

    server_path = extract_verified_server(archive_path, version_dir)
    print(f"Extracted and verified server SHA-256 {SERVER_SHA256}")
    return server_path


def file_uri(path):
    return "file://" + quote(str(Path(path).resolve()), safe="/")


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
        raise RuntimeError(f"LSP response omitted Content-Length: {headers}")
    length = int(content_length)
    body = stream.read(length)
    if len(body) != length:
        raise RuntimeError(
            f"LSP response body ended early: expected {length} bytes, received {len(body)}"
        )
    return json.loads(body.decode("utf-8"))


def respond_to_server_request(proc, message):
    if "id" not in message or "method" not in message:
        return False
    if message["method"] == "workspace/configuration":
        items = message.get("params", {}).get("items", [])
        result = [None] * len(items)
    else:
        result = None
    write_message(
        proc.stdin,
        {"jsonrpc": "2.0", "id": message["id"], "result": result},
    )
    return True


def wait_for_response(proc, response_id, timeout_seconds, max_messages=300):
    for _ in range(max_messages):
        message = read_message(proc.stdout, timeout_seconds)
        if message is None:
            return None
        if message.get("id") == response_id and "method" not in message:
            return message
        respond_to_server_request(proc, message)
    raise RuntimeError(f"Too many LSP messages while waiting for response {response_id}")


def completion_position(text):
    lines = text.splitlines()
    for index, line in enumerate(lines[:-1]):
        if COMPLETION_MARKER in line:
            probe_line = lines[index + 1]
            column = probe_line.find(COMPLETION_PREFIX)
            if column < 0:
                break
            return {"line": index + 1, "character": column + len(COMPLETION_PREFIX)}
    raise RuntimeError("Visualforce completion marker or probe prefix is missing")


def completion_items(result):
    if isinstance(result, list):
        return result
    if isinstance(result, dict) and isinstance(result.get("items"), list):
        return result["items"]
    return []


def run_lsp_smoke(node, server_path, fixture_path, timeout_seconds):
    workspace = fixture_path.parent.resolve()
    stderr_file = tempfile.TemporaryFile()
    proc = subprocess.Popen(
        [node, str(server_path), "--stdio"],
        cwd=workspace,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=stderr_file,
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
                    "workspaceFolders": [
                        {"uri": file_uri(workspace), "name": workspace.name}
                    ],
                    "capabilities": {
                        "workspace": {"configuration": True},
                        "textDocument": {"completion": {}},
                    },
                    "clientInfo": {
                        "name": "zed-salesforce-visualforce-smoke",
                        "version": "0.1",
                    },
                    "initializationOptions": {
                        "embeddedLanguages": {"css": True, "javascript": True}
                    },
                },
            },
        )
        initialize = wait_for_response(proc, 1, timeout_seconds)
        if initialize is None or "result" not in initialize:
            raise RuntimeError(f"Unexpected initialize response: {initialize}")

        write_message(proc.stdin, {"jsonrpc": "2.0", "method": "initialized", "params": {}})
        text = fixture_path.read_text(encoding="utf-8")
        write_message(
            proc.stdin,
            {
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": file_uri(fixture_path),
                        "languageId": "visualforce",
                        "version": 1,
                        "text": text,
                    }
                },
            },
        )
        write_message(
            proc.stdin,
            {
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/completion",
                "params": {
                    "textDocument": {"uri": file_uri(fixture_path)},
                    "position": completion_position(text),
                    "context": {"triggerKind": 1},
                },
            },
        )
        completion = wait_for_response(proc, 2, timeout_seconds)
        if completion is None or "result" not in completion:
            raise RuntimeError(f"Unexpected completion response: {completion}")
        items = completion_items(completion["result"])
        labels = [item.get("label", "") for item in items if isinstance(item, dict)]
        apex_labels = sorted({label for label in labels if label.startswith("apex:")})
        if not apex_labels:
            raise RuntimeError(
                f"Completion returned {len(items)} items but no apex:* labels"
            )

        write_message(
            proc.stdin,
            {"jsonrpc": "2.0", "id": 3, "method": "shutdown", "params": None},
        )
        shutdown = wait_for_response(proc, 3, timeout_seconds)
        if shutdown is None or "result" not in shutdown:
            raise RuntimeError(f"Unexpected shutdown response: {shutdown}")
        write_message(proc.stdin, {"jsonrpc": "2.0", "method": "exit", "params": None})
        proc.stdin.close()
        return_code = proc.wait(timeout=timeout_seconds)
        if return_code != 0:
            raise RuntimeError(f"Visualforce LSP exited with status {return_code}")
        return len(items), apex_labels
    except Exception:
        proc.kill()
        proc.wait(timeout=5)
        raise
    finally:
        stderr_file.seek(0)
        stderr = stderr_file.read().decode("utf-8", errors="replace")
        stderr_file.close()
        if stderr:
            print(stderr, file=sys.stderr, end="")


def run_corrupt_bundle_check(archive_path, version_dir):
    server_path = ensure_server(archive_path, version_dir)
    server_path.write_bytes(b"deliberately corrupted Visualforce server")
    try:
        verify_file(server_path, SERVER_SHA256)
    except IntegrityError as error:
        if error.expected != SERVER_SHA256 or error.actual in (SERVER_SHA256, "missing"):
            raise RuntimeError(f"Unexpected corruption result: {error}") from error
        print(f"Corrupt-bundle negative check passed: {error}")
    else:
        raise RuntimeError("Corrupt-bundle negative check did not detect the changed server")

    ensure_server(archive_path, version_dir)
    verify_file(server_path, SERVER_SHA256)
    print("Corrupt extracted cache was restored from the verified cached VSIX.")


def main():
    repo_root = Path(__file__).resolve().parent.parent
    default_cache = Path(
        os.environ.get(
            "VISUALFORCE_LSP_CACHE_DIR",
            repo_root / ".cache" / "visualforce-language-server",
        )
    )
    parser = argparse.ArgumentParser()
    parser.add_argument("--cache-dir", type=Path, default=default_cache)
    parser.add_argument(
        "--vsix-url",
        default=os.environ.get("VISUALFORCE_LSP_VSIX_URL", VSIX_URL),
    )
    parser.add_argument(
        "--node",
        default=os.environ.get("VISUALFORCE_LSP_NODE", shutil.which("node")),
    )
    parser.add_argument("--timeout-seconds", type=int, default=20)
    parser.add_argument("--expect-corrupt-bundle-failure", action="store_true")
    args = parser.parse_args()

    if not args.node:
        raise SystemExit("Node.js was not found; pass --node or set VISUALFORCE_LSP_NODE")

    cache_root = args.cache_dir.resolve()
    archive_path = cache_root / "downloads" / VSIX_NAME
    version_dir = cache_root / RELEASE
    fixture_path = repo_root / "scripts" / "fixtures" / "visualforce" / "CompletionProbe.page"
    if not fixture_path.is_file():
        raise SystemExit(f"Visualforce fixture is missing: {fixture_path}")

    try:
        archive_path = ensure_vsix(args.vsix_url, archive_path)
        if args.expect_corrupt_bundle_failure:
            run_corrupt_bundle_check(archive_path, version_dir)
            return
        server_path = ensure_server(archive_path, version_dir)
        item_count, apex_labels = run_lsp_smoke(
            args.node, server_path, fixture_path, args.timeout_seconds
        )
    except Exception as error:
        raise SystemExit(str(error)) from error

    sample = ", ".join(apex_labels[:5])
    print(
        "Visualforce LSP smoke test passed: "
        f"{item_count} completion items; {len(apex_labels)} unique apex:* labels "
        f"(sample: {sample})."
    )


if __name__ == "__main__":
    main()
