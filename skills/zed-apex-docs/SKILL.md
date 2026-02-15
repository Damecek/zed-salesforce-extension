---
name: zed-apex-docs
description: Use when working on this repository (Zed Apex LSP extension) and you need up-to-date or authoritative information about Zed extension APIs/docs, language server integration, the Language Server Protocol, Salesforce Apex Language Server behavior, or the Salesforce VS Code Apex extension reference implementation. This skill provides a repeatable research workflow using Context7 and a small set of authoritative web sources, plus local repo reference paths.
---

# Zed Apex Docs Research Workflow

## Context7 (preferred for up-to-date docs/source)

Use Context7 first for anything that could change or when you need exact API shapes.

Resolved Context7 `libraryId` values (use these directly with `query-docs`):
- `/microsoft/language-server-protocol`
- `/zed-industries/zed`
- `/websites/zed_dev`
- `/forcedotcom/salesforcedx-vscode`

If a library ID fails to query (not found / renamed), re-run `resolve-library-id` and update the ID list above.

Suggested query patterns:
- Zed extension API usage: “`zed_extension_api` language server command example”, “register language server in extension.toml”
- Zed languages: “languages config.toml schema”, “highlights.scm tree-sitter queries”
- LSP spec: “initialize params capabilities”, “textDocument/completion request response”, “diagnostics publishDiagnostics”
- Salesforce VS Code: “apex language server start command”, “java requirements resolution order”

## Authoritative Web Sources (use when Context7 is insufficient)

Use `web.run` to fetch these pages (prefer primary docs; avoid blogs unless necessary):
- `https://docs.rs/zed_extension_api/latest/zed_extension_api/` (API reference)
- `https://github.com/aheber/tree-sitter-sfapex/tree/main` (grammar, highlights guidance)
- `https://developer.salesforce.com/docs/platform/sfvscode-extensions/guide/apex-language-server.html` (Apex LS behavior and requirements)

When quoting, keep excerpts short; prefer paraphrase + citation.

## Local Repo “Source of Truth” Files

Prefer local mirrors when available (stable, works offline):
- Apex VS Code reference implementation:
  - `third_party/salesforcedx-vscode-apex/src/languageServer.ts`
  - `third_party/salesforcedx-vscode-apex/src/requirements.ts`
  - `third_party/salesforcedx-vscode-apex/src/languageUtils/apexLanguageConfiguration.ts`
- Vendored jar:
  - `vendor/apex-jorje-lsp.jar`
  - `vendor/apex-jorje-lsp.jar.sha256`

## Output Expectations (when reporting findings)

When you answer a question based on research:
1. State the concrete conclusion (exact API names/fields/commands).
2. Cite the source(s): Context7 snippet reference (if available) or web citations.
3. Note any version constraints/assumptions (Zed version, Java version, Apex LS version).
