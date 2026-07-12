#!/usr/bin/env python3
"""Small shared JSON-RPC transport helpers for repository LSP smoke tests."""

import json
import os
import select
import time
from pathlib import Path
from urllib.parse import quote


_READ_BUFFERS = {}


def write_message(stream, payload):
    body = json.dumps(payload, separators=(",", ":")).encode("utf-8")
    stream.write(f"Content-Length: {len(body)}\r\n\r\n".encode("ascii"))
    stream.write(body)
    stream.flush()


def read_message(stream, timeout_seconds):
    buffer = _READ_BUFFERS.setdefault(stream, bytearray())
    deadline = time.monotonic() + timeout_seconds

    while True:
        message = _decode_frame(buffer)
        if message is not None:
            return message

        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return None
        ready, _, _ = select.select([stream], [], [], remaining)
        if not ready:
            return None
        chunk = os.read(stream.fileno(), 64 * 1024)
        if not chunk:
            _READ_BUFFERS.pop(stream, None)
            return None
        buffer.extend(chunk)


def file_uri(path):
    return "file://" + quote(str(Path(path).resolve()), safe="/")


def _decode_frame(buffer):
    separator = b"\r\n\r\n"
    header_end = buffer.find(separator)
    if header_end < 0:
        separator = b"\n\n"
        header_end = buffer.find(separator)
    if header_end < 0:
        return None

    headers = {}
    for line in bytes(buffer[:header_end]).decode("ascii", errors="replace").splitlines():
        if ":" in line:
            key, value = line.split(":", 1)
            headers[key.strip().lower()] = value.strip()

    content_length = headers.get("content-length")
    if content_length is None:
        raise RuntimeError(f"LSP response omitted Content-Length: {headers}")
    length = int(content_length)
    body_start = header_end + len(separator)
    body_end = body_start + length
    if len(buffer) < body_end:
        return None

    body = bytes(buffer[body_start:body_end])
    del buffer[:body_end]
    return json.loads(body.decode("utf-8"))
