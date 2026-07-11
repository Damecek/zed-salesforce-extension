# Visualforce Language Server Runtime Design

## Status and scope

This design covers the first, runtime-only phase of GitHub issue #19. That phase deliberately omitted grammar and language registration. The follow-up integration in `specs/003-visualforce-language-integration/` now makes the verified runtime user-visible through the real public grammar without attaching it to HTML or publishing a release.

## Distribution decision

Use Salesforce's official Visualforce VSIX from release `v67.4.0`:

- URL: `https://github.com/forcedotcom/salesforcedx-vscode/releases/download/v67.4.0/salesforcedx-vscode-visualforce-67.4.0.vsix`
- VSIX SHA-256: `6232bb3dc3bdfe2c491601b9c96c488fb52941c2ff62bcc125230e4dceacbb0c`
- server entry point: `extension/dist/visualforceServer.js`
- server SHA-256: `37f6808e5e4bd360f7c7f219fd2d71cc8d7ce22688b271c1a4ae5020bd85bb3f`

The internal upstream npm packages are not public, so runtime npm installation cannot reproduce Salesforce's published server. Building the Salesforce monorepo at runtime would be slow, fragile, and outside the extension host's responsibility. The official VSIX is therefore the pinned distribution artifact.

## Approaches considered

1. **Dedicated Rust runtime module backed by the official VSIX (selected).** This keeps constants, cache verification, cache repair, command construction, and initialization data behind one narrow boundary. Pure helpers can be unit-tested without invoking Zed host functions.
2. **Add the logic directly to `src/lib.rs`.** This would use fewer files, but would enlarge the existing mixed Apex/LWC module and make staged Visualforce support harder to review independently.
3. **Install or build upstream npm packages.** This is not viable because the required Salesforce Visualforce packages are unpublished and a monorepo source build is not a stable runtime distribution strategy.

## Runtime architecture

`src/visualforce.rs` owns the stable server id `visualforce-language-server`, pinned artifact metadata, and the deterministic cache path `lsp/visualforce-language-server/v67.4.0/extension/dist/visualforceServer.js` relative to Zed's extension work directory.

On launch, the module reports `CheckingForUpdate` and hashes an existing bundle. A matching bundle is reused without download. A missing or mismatched bundle causes exactly one repair attempt: remove only `lsp/visualforce-language-server/v67.4.0`, report `Downloading`, and ask `zed::download_file` to extract the pinned VSIX as `DownloadedFileType::Zip` into that version directory. The extracted JavaScript is hashed again before execution. A second mismatch fails with an error containing the expected and actual hashes. Failures are reported through `LanguageServerInstallationStatus::Failed`; success clears the status with `None`.

The command builder uses `zed::node_binary_path()`, a materialized bundled protocol shim, the verified JavaScript path, `--stdio`, and the worktree shell environment. The shim preserves `languageId = "visualforce"` for Visualforce-specific completion while mirroring document lifecycle notifications to an internal `html` shadow URI for the embedded CSS/JavaScript validation that v67.4.0 otherwise suppresses. Only shadow diagnostics are mapped back to the real URI. Initialization options are exactly:

```json
{"embeddedLanguages":{"css":true,"javascript":true}}
```

`src/lib.rs` recognizes the stable server id and delegates to this module. The completed follow-up integration registers it only for `Visualforce`; installation behavior remains isolated in this module.

## Testing design

Rust unit tests cover deterministic path selection, SHA-256 verification, valid-cache reuse, corrupt-cache repair with one downloader call, post-download mismatch errors, wrapper materialization, command construction, and initialization options. Tests inject a small download closure so they exercise real filesystem behavior while avoiding Zed host calls.

`scripts/test-visualforce-lsp-smoke.py` independently verifies the pinned VSIX hash, extracts it into an ignored or caller-supplied cache, verifies the server hash, launches the shim and `visualforceServer.js --stdio`, performs initialize/initialized/open/completion/shutdown/exit, and requires at least one completion label beginning with `apex:`. `scripts/test-visualforce-lsp-diagnostics.py` opens a valid document and changes it to invalid embedded CSS/JavaScript, requiring both supported diagnostics on the real Visualforce URI. The cache and URL are overrideable. A checksum-negative mode corrupts the extracted bundle and asserts deterministic expected/actual hash failure. Running the normal smoke twice proves validated cache reuse.

The fixture is a realistic `.page` file containing nested `apex:*` tags, `{!...}` expressions, CSS, JavaScript, and a marked completion probe.

## What changed / why / how to verify

- **What:** add a pinned, integrity-checked Visualforce LSP runtime seam plus unit and standalone protocol tests.
- **Why:** Salesforce distributes the runnable server in its official VSIX rather than public npm packages. The separately developed Visualforce grammar is now integrated by the follow-up phase.
- **How:** run `rtk cargo test`, then run `rtk python3 scripts/test-visualforce-lsp-smoke.py` twice, `rtk python3 scripts/test-visualforce-lsp-smoke.py --expect-corrupt-bundle-failure`, and `rtk python3 scripts/test-visualforce-lsp-diagnostics.py`. Complete verification also includes formatting, compilation, existing Apex/LWC smoke tests, TOML parsing, and diff checks.

## Completed integration dependency

The initial follow-up phase pinned `https://github.com/Damecek/tree-sitter-visualforce`
at v0.1.0 (`88d24e807898f294e9e7d575509378ba352ee297`), added
`languages/visualforce/**`, and registered `visualforce-language-server` only for
`Visualforce`; that chronology remains in
`specs/003-visualforce-language-integration/spec.md`. The delivered integration
now pins v0.1.1 (`b1f026749107d549e72b8cef841cfd3ae9cf8240`) as recorded in
`specs/004-visualforce-tree-sitter-v0.1.1/spec.md`.
