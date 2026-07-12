#!/usr/bin/env node
'use strict';

const { spawn } = require('child_process');

const upstreamServer = process.argv[2];
if (!upstreamServer) {
  throw new Error('Missing upstream Visualforce language server path');
}

const upstream = spawn(process.execPath, [upstreamServer, ...process.argv.slice(3)], {
  stdio: ['pipe', 'pipe', 'pipe'],
});
const originalToShadow = new Map();
const shadowToOriginal = new Map();

function writeMessage(stream, message) {
  const body = Buffer.from(JSON.stringify(message), 'utf8');
  stream.write(`Content-Length: ${body.length}\r\n\r\n`);
  stream.write(body);
}

function readMessages(stream, onMessage) {
  let buffer = Buffer.alloc(0);
  stream.on('data', (chunk) => {
    buffer = Buffer.concat([buffer, chunk]);
    while (true) {
      const headerEnd = buffer.indexOf('\r\n\r\n');
      if (headerEnd === -1) {
        return;
      }
      const headers = buffer.subarray(0, headerEnd).toString('ascii');
      const lengthMatch = headers.match(/(?:^|\r\n)Content-Length:\s*(\d+)/i);
      if (!lengthMatch) {
        throw new Error(`LSP frame omitted Content-Length: ${headers}`);
      }
      const bodyStart = headerEnd + 4;
      const bodyEnd = bodyStart + Number(lengthMatch[1]);
      if (buffer.length < bodyEnd) {
        return;
      }
      const message = JSON.parse(buffer.subarray(bodyStart, bodyEnd).toString('utf8'));
      buffer = buffer.subarray(bodyEnd);
      onMessage(message);
    }
  });
}

function diagnosticShadowUri(uri) {
  return `${uri}${uri.includes('?') ? '&' : '?'}zed-visualforce-diagnostics`;
}

function forwardClientMessage(message) {
  const method = message.method;
  const document = message.params && message.params.textDocument;
  const uri = document && document.uri;

  if (method === 'textDocument/didOpen' && document.languageId === 'visualforce') {
    const shadowUri = diagnosticShadowUri(uri);
    originalToShadow.set(uri, shadowUri);
    shadowToOriginal.set(shadowUri, uri);
    writeMessage(upstream.stdin, message);
    writeMessage(upstream.stdin, {
      ...message,
      params: {
        ...message.params,
        textDocument: { ...document, uri: shadowUri, languageId: 'html' },
      },
    });
    return;
  }

  const shadowUri = uri && originalToShadow.get(uri);
  if (
    shadowUri &&
    (method === 'textDocument/didChange' || method === 'textDocument/didClose')
  ) {
    writeMessage(upstream.stdin, message);
    writeMessage(upstream.stdin, {
      ...message,
      params: { ...message.params, textDocument: { ...document, uri: shadowUri } },
    });
    return;
  }

  writeMessage(upstream.stdin, message);
}

function forwardServerMessage(message) {
  if (message.method === 'textDocument/publishDiagnostics') {
    const uri = message.params && message.params.uri;
    const originalUri = shadowToOriginal.get(uri);
    if (originalUri) {
      writeMessage(process.stdout, {
        ...message,
        params: { ...message.params, uri: originalUri },
      });
      return;
    }
    if (originalToShadow.has(uri)) {
      return;
    }
  }
  writeMessage(process.stdout, message);
}

readMessages(process.stdin, forwardClientMessage);
readMessages(upstream.stdout, forwardServerMessage);
upstream.stderr.pipe(process.stderr);
process.stdin.on('end', () => upstream.stdin.end());
upstream.on('error', (error) => {
  console.error(`Could not launch Visualforce language server: ${error.message}`);
  process.exitCode = 1;
});
upstream.on('exit', (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
  } else {
    process.exitCode = code === null ? 1 : code;
  }
});
