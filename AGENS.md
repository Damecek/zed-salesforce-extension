# AGENS.md – pracovní zadání projektu

Tento dokument shrnuje účel repozitáře a slouží jako stručné zadání pro další implementační kroky.

## Úkol

Vybudovat extension pro editor **Zed**, která přidá podporu jazyka **Salesforce Apex** přes **LSP server** (`apex-jorje-lsp.jar`).

## Výchozí dokumentace

- Zed extension development: https://zed.dev/docs/extensions/developing-extensions
- Zed language extensions: https://zed.dev/docs/extensions/languages
- Salesforce Apex Language Server: https://developer.salesforce.com/docs/platform/sfvscode-extensions/guide/apex-language-server.html
- Referenční implementace (VS Code):
  - https://github.com/forcedotcom/salesforcedx-vscode
  - `packages/salesforcedx-vscode-apex/src/languageServer.ts`
  - `packages/salesforcedx-vscode-apex/jars/apex-jorje-lsp.jar`

## Architektonická východiska

1. Zed extension je Rust/WASM (`zed_extension_api`) + `extension.toml`.
2. Apex language support v Zed bude kombinovat:
   - Tree-sitter vrstvu (syntax highlighting + comments),
   - LSP vrstvu (diagnostics/completion/definition).
3. Apex LSP runtime je Java proces spouštěný příkazem typu:
   - `java -cp <jar> apex.jorje.lsp.ApexLanguageServerLauncher`

## MVP (první funkční verze)

- korektní mapování Apex souborů (`.cls`, `.trigger`),
- základní syntax highlighting (komentář vs. kód minimálně),
- úspěšné spuštění Apex LSP serveru v Zed,
- základní LSP funkce: diagnostics + completion + definition (dle podpory serveru),
- provozní dokumentace (Java verze, troubleshooting, logy).

## Testovatelnost pro AI agenta

Doporučený přístup:

1. Automatický smoke test startu Java LSP procesu.
2. LSP JSON-RPC handshake test (`initialize`, open document, completion request).
3. Integrační ověření v Zed dev extension režimu + kontrola logu (`zed: open log`).

## Poznámka

Detailní architektura, rizika, roadmapa a implementační plán jsou v `README.md`.
