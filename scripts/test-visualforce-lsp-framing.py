#!/usr/bin/env python3
"""Regression tests for shared smoke-test JSON-RPC framing."""

import importlib.util
import io
import json
import os
import subprocess
import tempfile
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

    def test_visualforce_wrapper_normalizes_missing_configuration_entries(self):
        wrapper = Path(__file__).with_name("visualforce-language-server-wrapper.js")
        fake_server_source = r"""
'use strict';

let buffer = Buffer.alloc(0);

function writeMessage(message) {
  const body = Buffer.from(JSON.stringify(message), 'utf8');
  process.stdout.write(`Content-Length: ${body.length}\r\n\r\n`);
  process.stdout.write(body);
}

process.stdin.on('data', (chunk) => {
  buffer = Buffer.concat([buffer, chunk]);
  while (true) {
    const headerEnd = buffer.indexOf('\r\n\r\n');
    if (headerEnd === -1) return;
    const headers = buffer.subarray(0, headerEnd).toString('ascii');
    const length = Number(headers.match(/Content-Length:\s*(\d+)/i)[1]);
    const bodyStart = headerEnd + 4;
    const bodyEnd = bodyStart + length;
    if (buffer.length < bodyEnd) return;
    const message = JSON.parse(buffer.subarray(bodyStart, bodyEnd).toString('utf8'));
    buffer = buffer.subarray(bodyEnd);

    if (message.method === 'initialize') {
      writeMessage({ jsonrpc: '2.0', id: message.id, result: { capabilities: {} } });
    } else if (message.method === 'initialized') {
      writeMessage({
        jsonrpc: '2.0',
        id: 99,
        method: 'workspace/configuration',
        params: { items: [{ section: 'css' }, { section: 'html' }] },
      });
    } else if (message.id === 99) {
      writeMessage({
        jsonrpc: '2.0',
        method: 'test/configurationResult',
        params: message.result,
      });
    }
  }
});
"""
        with tempfile.TemporaryDirectory(prefix="visualforce-wrapper-") as temp_dir:
            fake_server = Path(temp_dir) / "fake-server.js"
            fake_server.write_text(fake_server_source, encoding="utf-8")
            proc = subprocess.Popen(
                ["node", str(wrapper), str(fake_server), "--stdio"],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            try:
                protocol.write_message(
                    proc.stdin,
                    {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}},
                )
                self.assertEqual(
                    protocol.read_message(proc.stdout, 2),
                    {"jsonrpc": "2.0", "id": 1, "result": {"capabilities": {}}},
                )
                protocol.write_message(
                    proc.stdin,
                    {"jsonrpc": "2.0", "method": "initialized", "params": {}},
                )
                configuration_request = protocol.read_message(proc.stdout, 2)
                self.assertEqual(configuration_request["method"], "workspace/configuration")
                protocol.write_message(
                    proc.stdin,
                    {
                        "jsonrpc": "2.0",
                        "id": configuration_request["id"],
                        "result": [None, {"validProperties": ["custom-property"]}],
                    },
                )
                forwarded = protocol.read_message(proc.stdout, 2)
                self.assertEqual(forwarded["method"], "test/configurationResult")
                self.assertEqual(
                    forwarded["params"],
                    [{}, {"validProperties": ["custom-property"]}],
                )
            finally:
                proc.kill()
                proc.wait(timeout=2)
                proc.stdin.close()
                proc.stdout.close()
                proc.stderr.close()

    @staticmethod
    def frame(payload):
        body = json.dumps(payload, separators=(",", ":")).encode("utf-8")
        return f"Content-Length: {len(body)}\r\n\r\n".encode("ascii") + body


if __name__ == "__main__":
    unittest.main()
