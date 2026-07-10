# Visualforce Language Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `.page` and `.component` files work end to end as Visualforce in Zed using the released public grammar and the existing verified LSP runtime.

**Architecture:** Pin the immutable grammar commit in the Zed manifest, add one dedicated Visualforce language definition with upstream-derived queries, and register the existing runtime only for that language. A standalone integration test validates manifest wiring, file associations, parser behavior, and all shipped queries before real LSP and editor validation.

**Tech Stack:** Zed extension manifest/language configuration, Tree-sitter Visualforce `88d24e807898f294e9e7d575509378ba352ee297`, Python 3 standard library, Node.js, Rust 2021.

## Global Constraints

- Continue on `feat/visualforce-lsp-runtime` and update PR #20 without rewriting existing commits.
- Pin `https://github.com/Damecek/tree-sitter-visualforce` at `88d24e807898f294e9e7d575509378ba352ee297`.
- Register `visualforce-language-server` only for `Visualforce`; never attach it to HTML.
- Recognize exactly `.page` and `.component` as Visualforce source suffixes.
- Reuse the existing VSIX runtime and hashes without npm or source-build fallback.
- Do not publish a release or bump `extension.toml` version.
- End every commit with `Co-Authored-By: codex <codex@openai.com>`.

---

### Task 1: Write the failing integration gate

**Files:**
- Create: `scripts/test-visualforce-integration.py`
- Create later in Task 2: `scripts/fixtures/visualforce/CompletionProbe.component`

**Interfaces:**
- Consumes: `extension.toml`, `languages/visualforce/**`, local fixtures, and an overrideable `.cache/tree-sitter-visualforce-integration` checkout.
- Produces: one deterministic command that validates manifest wiring, file associations, parsing, and query compilation.

- [x] Implement assertions for grammar repository/revision, server registration/language id, config suffixes, fixture presence, clean parsing, and all query files.
- [x] Run `rtk python3 scripts/test-visualforce-integration.py` and confirm it fails because `[grammars.visualforce]` and the language directory do not exist.

### Task 2: Add the Visualforce language and server registration

**Files:**
- Modify: `extension.toml`
- Create: `languages/visualforce/config.toml`
- Create: `languages/visualforce/highlights.scm`
- Create: `languages/visualforce/injections.scm`
- Create: `languages/visualforce/indents.scm`
- Create: `languages/visualforce/folds.scm`
- Create: `languages/visualforce/brackets.scm`
- Create: `scripts/fixtures/visualforce/CompletionProbe.component`

**Interfaces:**
- Produces: Zed language name `Visualforce`, grammar name `visualforce`, suffixes `page`/`component`, and one manifest server binding to protocol id `visualforce`.

- [x] Add the grammar and server manifest records with the exact pinned values.
- [x] Add HTML-compatible language ergonomics and upstream Visualforce highlight/injection/indent/fold queries.
- [x] Add a component fixture containing nested `apex:*`/`c:*` markup, `{!...}` expressions, CSS, and JavaScript.
- [x] Run `rtk python3 scripts/test-visualforce-integration.py` and confirm manifest, parser, and query checks pass.

### Task 3: Document active support

**Files:**
- Modify: `README.md`
- Modify: `specs/002-visualforce-lsp-runtime/spec.md`
- Modify: `specs/003-visualforce-language-integration/plan.md`

**Interfaces:**
- Consumes: the implemented manifest/language behavior and verified test commands.
- Produces: accurate user-facing setup, architecture, verification, and capability statements.

- [x] Update the project summary and Visualforce section to state that `.page`/`.component` support is active.
- [x] Preserve the official VSIX URL and both hashes, add the pinned grammar revision, and document the integration smoke command.
- [x] Replace the old remaining-dependency wording with the exact completed integration.

### Task 4: Verify and publish the completed integration

**Files:**
- Inspect: every file changed since `93b7f74` and the complete PR diff.
- Modify: PR #20 title/body after local verification passes.

**Interfaces:**
- Produces: fresh build/test evidence, pushed commits, and a PR that closes issue #19 only after grammar and runtime are both wired.

- [x] Run formatting, host and wasm checks, Rust tests, the new integration test, Visualforce LSP smoke twice, and its corruption mode.
- [x] Run existing Apex, LWC, and Tree-sitter regression tests plus TOML validation.
- [x] Confirm the server is attached only to `Visualforce`, both suffixes resolve through the new language, no binary/cache is tracked, and the version is unchanged.
- [x] Load/build the development extension in Zed where possible and gather language/LSP evidence from the fixture.
- [x] Apply the code-review checklist inline, commit with required trailers, push, and update PR #20 to `Closes #19` without publishing a release.
