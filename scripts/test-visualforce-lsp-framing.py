#!/usr/bin/env python3
"""Regression tests for shared smoke-test JSON-RPC framing."""

import importlib.util
import io
import json
import os
import unittest
from pathlib import Path

import lsp_test_protocol as protocol


def load_script(name):
    path = Path(__file__).with_name(name)
    spec = importlib.util.spec_from_file_location(path.stem, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class FramingTests(unittest.TestCase):
    def test_read_message_consumes_second_frame_already_in_transport_buffer(self):
        first = {"jsonrpc": "2.0", "id": 1, "result": {}}
        second = {"jsonrpc": "2.0", "id": 2, "result": {"items": []}}
        read_fd, write_fd = os.pipe()
        stream = os.fdopen(read_fd, "rb")
        try:
            os.write(write_fd, self.frame(first) + self.frame(second))

            self.assertEqual(protocol.read_message(stream, 0.05), first)
            self.assertEqual(protocol.read_message(stream, 0.05), second)
        finally:
            os.close(write_fd)
            stream.close()

    def test_write_message_uses_compact_content_length_frame(self):
        stream = io.BytesIO()

        protocol.write_message(stream, {"jsonrpc": "2.0", "id": 1})

        self.assertEqual(
            stream.getvalue(),
            b'Content-Length: 24\r\n\r\n{"jsonrpc":"2.0","id":1}',
        )

    def test_apex_and_visualforce_clients_use_shared_protocol_helpers(self):
        apex = load_script("lsp_smoke.py")
        visualforce = load_script("test-visualforce-lsp-smoke.py")

        for client in (apex, visualforce):
            self.assertIs(client.read_message, protocol.read_message)
            self.assertIs(client.write_message, protocol.write_message)
            self.assertIs(client.file_uri, protocol.file_uri)

    @staticmethod
    def frame(payload):
        body = json.dumps(payload, separators=(",", ":")).encode("utf-8")
        return f"Content-Length: {len(body)}\r\n\r\n".encode("ascii") + body


if __name__ == "__main__":
    unittest.main()
