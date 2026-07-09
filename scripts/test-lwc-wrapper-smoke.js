#!/usr/bin/env node
'use strict';

const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawn } = require('child_process');

const repoRoot = path.resolve(__dirname, '..');
const wrapperPath = path.join(repoRoot, 'scripts', 'lwc-language-server-wrapper.js');
const packageDir =
  process.env.LWC_LSP_PACKAGE_DIR ||
  path.join(repoRoot, 'node_modules', '@salesforce', 'lwc-language-server');
const upstreamServerPath = path.join(packageDir, 'bin', 'lwc-language-server.js');
const providedWorkspaceRoot = process.env.LWC_WRAPPER_SMOKE_WORKSPACE
  ? path.resolve(process.env.LWC_WRAPPER_SMOKE_WORKSPACE)
  : null;

if (!fs.existsSync(upstreamServerPath)) {
  console.error(`Missing upstream server at ${upstreamServerPath}`);
  console.error('Set LWC_LSP_PACKAGE_DIR to an installed @salesforce/lwc-language-server package.');
  process.exit(1);
}

const workspaceRoot = providedWorkspaceRoot || fs.mkdtempSync(path.join(os.tmpdir(), 'zed-lwc-wrapper-smoke-'));
if (!providedWorkspaceRoot) {
  const lwcDir = path.join(workspaceRoot, 'force-app', 'main', 'default', 'lwc', 'hello');
  fs.mkdirSync(lwcDir, { recursive: true });
  fs.writeFileSync(
    path.join(workspaceRoot, 'sfdx-project.json'),
    JSON.stringify({ packageDirectories: [{ path: 'force-app', default: true }], sourceApiVersion: '64.0' }, null, 2)
  );
  fs.writeFileSync(path.join(lwcDir, 'hello.html'), '<template><p>Hello</p></template>\n');
  fs.writeFileSync(path.join(lwcDir, 'hello.js'), "import { LightningElement } from 'lwc';\nexport default class Hello extends LightningElement {}\n");
  fs.writeFileSync(
    path.join(lwcDir, 'hello.js-meta.xml'),
    '<?xml version="1.0" encoding="UTF-8"?><LightningComponentBundle xmlns="http://soap.sforce.com/2006/04/metadata"><apiVersion>64.0</apiVersion><isExposed>false</isExposed></LightningComponentBundle>\n'
  );
}

const child = spawn(process.execPath, [wrapperPath, '--stdio'], {
  cwd: workspaceRoot,
  env: {
    ...process.env,
    ZED_SALESFORCE_LWC_UPSTREAM_SERVER_PATH: upstreamServerPath,
  },
  stdio: ['pipe', 'pipe', 'pipe'],
});

let stdout = Buffer.alloc(0);
let stderr = '';
let nextId = 1;
let initialized = false;

function send(message) {
  const json = JSON.stringify(message);
  child.stdin.write(`Content-Length: ${Buffer.byteLength(json, 'utf8')}\r\n\r\n${json}`);
}

function parseMessages() {
  while (true) {
    const headerEnd = stdout.indexOf('\r\n\r\n');
    if (headerEnd === -1) {
      return;
    }
    const header = stdout.slice(0, headerEnd).toString('utf8');
    const match = /Content-Length: (\d+)/i.exec(header);
    if (!match) {
      throw new Error(`Invalid LSP header: ${header}`);
    }
    const length = Number(match[1]);
    const bodyStart = headerEnd + 4;
    const bodyEnd = bodyStart + length;
    if (stdout.length < bodyEnd) {
      return;
    }
    const body = stdout.slice(bodyStart, bodyEnd).toString('utf8');
    stdout = stdout.slice(bodyEnd);
    handleMessage(JSON.parse(body));
  }
}

function handleMessage(message) {
  if (message.id === 1 && Object.prototype.hasOwnProperty.call(message, 'result')) {
    initialized = true;
    send({ jsonrpc: '2.0', method: 'initialized', params: {} });
    send({ jsonrpc: '2.0', id: nextId++, method: 'shutdown' });
  } else if (message.id === 1 && Object.prototype.hasOwnProperty.call(message, 'error')) {
    throw new Error(`LSP initialize failed: ${JSON.stringify(message.error)}`);
  } else if (message.id === 2) {
    send({ jsonrpc: '2.0', method: 'exit' });
  }
}

function assertNoWorkspaceWrites() {
  const forbidden = [
    '.vscode/settings.json',
    'core.code-workspace',
  ];
  const created = forbidden.filter((relativePath) => fs.existsSync(path.join(workspaceRoot, relativePath)));
  if (created.length > 0) {
    throw new Error(`Wrapper allowed upstream workspace writes: ${created.join(', ')}`);
  }
}

function assertHelpfulDefaultsRan() {
  const expected = ['.sfdx/indexes/lwc/custom-components.json'];
  const missing = expected.filter((relativePath) => !fs.existsSync(path.join(workspaceRoot, relativePath)));
  if (missing.length > 0) {
    throw new Error(`Wrapper did not allow expected LWC intelligence files: ${missing.join(', ')}`);
  }
}

const timeout = setTimeout(() => {
  child.kill('SIGKILL');
  console.error(stderr);
  console.error(`Timed out waiting for LSP initialize response from workspace ${workspaceRoot}`);
  process.exit(1);
}, 10000);

child.stdout.on('data', (chunk) => {
  stdout = Buffer.concat([stdout, chunk]);
  parseMessages();
});

child.stderr.on('data', (chunk) => {
  stderr += chunk.toString('utf8');
});

child.on('exit', (code, signal) => {
  clearTimeout(timeout);
  try {
    if (!initialized) {
      throw new Error(`LSP did not initialize. Exit code=${code} signal=${signal} stderr=${stderr}`);
    }
    assertNoWorkspaceWrites();
    if (!providedWorkspaceRoot) {
      assertHelpfulDefaultsRan();
    }
    if (!providedWorkspaceRoot) {
      fs.rmSync(workspaceRoot, { recursive: true, force: true });
    }
    console.log('LWC wrapper smoke test passed: no blocked workspace files were created.');
  } catch (error) {
    console.error(error.message);
    console.error(`Workspace kept for inspection: ${workspaceRoot}`);
    if (stderr) {
      console.error(stderr);
    }
    process.exit(1);
  }
});

send({
  jsonrpc: '2.0',
  id: nextId++,
  method: 'initialize',
  params: {
    processId: process.pid,
    rootUri: `file://${workspaceRoot}`,
    workspaceFolders: [{ uri: `file://${workspaceRoot}`, name: path.basename(workspaceRoot) }],
    capabilities: {},
    initializationOptions: {},
  },
});
