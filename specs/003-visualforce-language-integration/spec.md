# Visualforce Language Integration Design

## Goal

Turn the verified but inactive Visualforce language-server runtime into end-to-end Visualforce support in Zed now that the public Tree-sitter grammar exists. `.page` and `.component` files must be recognized as `Visualforce`, receive structural highlighting and embedded CSS/JavaScript injections, and start only `visualforce-language-server`.

## Pinned grammar

Use `https://github.com/Damecek/tree-sitter-visualforce` at full commit `88d24e807898f294e9e7d575509378ba352ee297`. The annotated `v0.1.0` tag dereferences to this commit and upstream CI is green. Pinning the immutable commit matches the existing grammar policy in `extension.toml`.

## Approaches considered

1. **Pin the grammar commit and add a dedicated Zed language (selected).** This gives deterministic parsing, Visualforce expression nodes, embedded-language injections, and narrow server activation.
2. **Track `main` or only the tag.** This is easier to read but weaker than a full immutable revision in a Zed extension manifest.
3. **Associate Visualforce files with HTML.** This is no longer justified because the real grammar exists and HTML cannot represent `{!...}` expressions structurally.

## Zed integration

Add `[grammars.visualforce]` and `[language_servers.visualforce-language-server]` to `extension.toml`. The server entry lists only `languages = ["Visualforce"]` and maps `"Visualforce" = "visualforce"`; it is never attached to HTML.

Add `languages/visualforce/` with:

- `config.toml`: `name = "Visualforce"`, `grammar = "visualforce"`, suffixes `page` and `component`, HTML-style comments/brackets/wrapping, four-space indentation, and completion query support for namespace separators.
- `highlights.scm`: upstream Visualforce captures plus Zed-compatible markup punctuation captures.
- `injections.scm`: upstream JavaScript and CSS injections for script/style blocks and matching inline attributes.
- `indents.scm` and `folds.scm`: upstream structural queries.
- `brackets.scm`: Zed HTML-compatible markup/rainbow delimiter behavior against the inherited HTML node structure.

The existing Rust module remains the sole owner of installation, integrity verification, command construction, and initialization options.

## Test strategy

Create `scripts/test-visualforce-integration.py` before manifest or language files. Its red state must report the missing grammar/server registration. Its green state must:

1. parse `extension.toml` and assert the exact repository/revision, language server, language list, and protocol id;
2. parse `languages/visualforce/config.toml` and assert both suffixes;
3. fetch or reuse the pinned grammar in an ignored overrideable cache;
4. parse the `.page` completion fixture and a realistic `.component` fixture without parser errors;
5. compile every shipped Visualforce query and assert representative highlight captures;
6. leave the existing real VSIX/LSP smoke test responsible for initialize, completion, shutdown, cache reuse, and corruption behavior.

Final verification also covers Rust host/wasm builds, all unit tests, Apex and LWC smoke tests, existing grammar checks, TOML validation, diff scope, and a development-extension check in Zed when the local editor environment permits it.

## Documentation and publication

Rewrite the README section from inactive groundwork to active Visualforce support, while retaining the pinned VSIX/hashes and test commands. Update the existing PR so it describes end-to-end support and changes `Refs #19` to `Closes #19`; do not publish an extension release or bump the extension version in this change.
