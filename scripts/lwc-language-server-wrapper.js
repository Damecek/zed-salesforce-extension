#!/usr/bin/env node
'use strict';

const fs = require('fs');
const path = require('path');
const { createRequire } = require('module');

const UPSTREAM_SERVER_ENV = 'ZED_SALESFORCE_LWC_UPSTREAM_SERVER_PATH';

function takeArgValue(flag) {
  const index = process.argv.indexOf(flag);
  if (index === -1) {
    return null;
  }
  const value = process.argv[index + 1];
  process.argv.splice(index, 2);
  return value || null;
}

function findPackageRoot(startPath) {
  let current = fs.statSync(startPath).isDirectory() ? startPath : path.dirname(startPath);
  while (true) {
    const packageJsonPath = path.join(current, 'package.json');
    if (fs.existsSync(packageJsonPath)) {
      const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf8'));
      if (packageJson.name === '@salesforce/lwc-language-server') {
        return current;
      }
    }

    const parent = path.dirname(current);
    if (parent === current) {
      throw new Error(`Could not find @salesforce/lwc-language-server package root from ${startPath}`);
    }
    current = parent;
  }
}

function patchMethod(prototype, methodName, replacement, label) {
  if (!prototype || typeof prototype[methodName] !== 'function') {
    throw new Error(`Cannot patch ${label}.${methodName}; upstream API shape changed`);
  }
  prototype[methodName] = replacement;
}

function applyWorkspaceWritePatches(packageRoot, requireFromPackage) {
  const common = requireFromPackage('@salesforce/lightning-lsp-common');
  patchMethod(
    common.BaseWorkspaceContext && common.BaseWorkspaceContext.prototype,
    'writeSettings',
    async function writeSettingsNoop() {},
    'BaseWorkspaceContext'
  );
  patchMethod(
    common.BaseWorkspaceContext && common.BaseWorkspaceContext.prototype,
    'writeSettingsJson',
    async function writeSettingsJsonNoop() {},
    'BaseWorkspaceContext'
  );
  patchMethod(
    common.BaseWorkspaceContext && common.BaseWorkspaceContext.prototype,
    'writeCodeWorkspace',
    async function writeCodeWorkspaceNoop() {},
    'BaseWorkspaceContext'
  );

  const context = require(path.join(packageRoot, 'lib', 'context', 'lwc-context.js'));
  if (context.LWCWorkspaceContext) {
    context.LWCWorkspaceContext.prototype.writeSettings = async function writeSettingsNoop() {};
  }
}

function resolveUpstreamServerPath() {
  const argvPath = takeArgValue('--upstream-server-path');
  const configuredPath = argvPath || process.env[UPSTREAM_SERVER_ENV];
  if (!configuredPath) {
    throw new Error(
      `Missing upstream LWC language server path. Set ${UPSTREAM_SERVER_ENV} or pass --upstream-server-path.`
    );
  }

  const serverPath = path.resolve(configuredPath);
  if (!fs.existsSync(serverPath)) {
    throw new Error(`Upstream LWC language server entrypoint does not exist: ${serverPath}`);
  }
  return serverPath;
}

const upstreamServerPath = resolveUpstreamServerPath();
const packageRoot = findPackageRoot(upstreamServerPath);
const requireFromPackage = createRequire(path.join(packageRoot, 'package.json'));
applyWorkspaceWritePatches(packageRoot, requireFromPackage);
require(upstreamServerPath);
