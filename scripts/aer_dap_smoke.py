#!/usr/bin/env python3
import json
import os
import select
import shutil
import subprocess
import sys
import time
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
WORKSPACE = REPO_ROOT / "scripts" / "fixtures" / "sfdx-minimal"


def encode_dap(message: dict) -> bytes:
    payload = json.dumps(message).encode("utf-8")
    return f"Content-Length: {len(payload)}\r\n\r\n".encode("ascii") + payload


def send(stream, message: dict) -> None:
    stream.write(encode_dap(message))
    stream.flush()


def collect_output(stream, timeout: float) -> str:
    deadline = time.time() + timeout
    chunks: list[bytes] = []

    while time.time() < deadline:
        remaining = max(0.0, deadline - time.time())
        readable, _, _ = select.select([stream], [], [], min(0.25, remaining))
        if not readable:
            continue
        chunk = os.read(stream.fileno(), 4096)
        if not chunk:
            break
        chunks.append(chunk)

    return b"".join(chunks).decode("utf-8", errors="replace")


def expect_contains(output: str, needle: str, label: str) -> None:
    if needle not in output:
        raise RuntimeError(f"Missing {label}: {needle}\n--- output ---\n{output}")


def main() -> int:
    if not shutil.which("aer"):
        print("aer binary not found on PATH", file=sys.stderr)
        return 1

    process = subprocess.Popen(
        ["aer", "test", "--debug", "."],
        cwd=WORKSPACE,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )

    assert process.stdin is not None
    assert process.stdout is not None

    try:
        send(
            process.stdin,
            {
                "seq": 1,
                "type": "request",
                "command": "initialize",
                "arguments": {
                    "adapterID": "aer",
                    "clientID": "smoke",
                    "clientName": "smoke",
                    "linesStartAt1": True,
                    "columnsStartAt1": True,
                    "pathFormat": "path",
                },
            },
        )

        output = collect_output(process.stdout, timeout=2)
        expect_contains(output, '"command":"initialize"', "initialize response")
        expect_contains(output, '"event":"initialized"', "initialized event")

        send(
            process.stdin,
            {
                "seq": 2,
                "type": "request",
                "command": "launch",
                "arguments": {
                    "request": "launch",
                    "args": ["."],
                    "stopOnEntry": False,
                },
            },
        )

        output = collect_output(process.stdout, timeout=5)
        expect_contains(output, '"command":"launch"', "launch response")
        expect_contains(output, '"success":true', "successful launch response")

        send(
            process.stdin,
            {
                "seq": 3,
                "type": "request",
                "command": "configurationDone",
                "arguments": {},
            },
        )

        output = collect_output(process.stdout, timeout=2)
        expect_contains(output, '"command":"configurationDone"', "configurationDone response")

        print("AER DAP smoke test passed")
        return 0
    finally:
        process.terminate()
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            process.kill()


if __name__ == "__main__":
    sys.exit(main())
