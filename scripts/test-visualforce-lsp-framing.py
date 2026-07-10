#!/usr/bin/env python3
"""Regression tests for Visualforce smoke-test JSON-RPC framing."""

import importlib.util
import json
import os
import unittest
from pathlib import Path


SCRIPT_PATH = Path(__file__).with_name("test-visualforce-lsp-smoke.py")
SPEC = importlib.util.spec_from_file_location("visualforce_lsp_smoke", SCRIPT_PATH)
SMOKE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SMOKE)


class FramingTests(unittest.TestCase):
    def test_read_message_consumes_a_frame_already_in_python_buffer(self):
        payload = {"jsonrpc": "2.0", "id": 1, "result": {}}
        body = json.dumps(payload, separators=(",", ":")).encode("utf-8")
        read_fd, write_fd = os.pipe()
        stream = os.fdopen(read_fd, "rb")
        try:
            os.write(write_fd, f"Content-Length: {len(body)}\r\n\r\n".encode("ascii") + body)

            self.assertEqual(SMOKE.read_message(stream, 0.05), payload)
        finally:
            os.close(write_fd)
            stream.close()


if __name__ == "__main__":
    unittest.main()
