# Apex LSP Provenance

This extension vendors two Apex backend payloads:

- `vendor/apex-jorje-lsp.jar`
- SHA-256 (hex) in `vendor/apex-jorje-lsp.jar.sha256`
- `vendor/apex-language-support/index.js`
- SHA-256 (hex) in `vendor/apex-language-support/index.js.sha256`

## Source: Java backend (`apex-jorje-lsp.jar`)

The jar was copied from the Salesforce VS Code extensions monorepo (Apex package):

- Source repo: https://github.com/forcedotcom/salesforcedx-vscode
- Source path: `packages/salesforcedx-vscode-apex/jars/apex-jorje-lsp.jar`
- Source commit (mirror import): `67dc27932e0ce43b93abe00878a2f966d0eb16a3`
- Imported on: 2026-02-14

## Source: Node backend (`apex-language-support`)

The Node entrypoint is bundled from the Salesforce Apex language support monorepo:

- Source repo: https://github.com/forcedotcom/apex-language-support
- Source package: `packages/apex-ls-node`
- Source commit: `87ebdb86e21b42b453a04bfad33e365f65970ca1`
- Build command sequence:
  - `npm install`
  - `npm run compile`
  - `npm run bundle --workspace=@salesforce/apex-ls-node`
- Vendored artifact: `packages/apex-ls-node/dist/index.js`
- Imported on: 2026-02-19

## Licensing

See `vendor/LICENSE.salesforcedx-vscode-apex.txt`.
See `vendor/LICENSE.apex-language-support.txt`.
