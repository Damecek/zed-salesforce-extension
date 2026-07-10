# Visualforce Language Server Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build and publish the tested, inactive Visualforce language-server runtime seam using Salesforce's official pinned VSIX.

**Architecture:** A dedicated Rust module owns pinned artifact metadata, versioned cache validation/repair, command creation, and initialization options. A separate Python smoke test validates the real VSIX and LSP protocol without changing existing Apex tooling or manifest activation.

**Tech Stack:** Rust 2021, `zed_extension_api` 0.7, `sha2` 0.10, Python 3 standard library, Node.js, Cargo, GitHub CLI.

## Global Constraints

- Start from local commit `fe96ced`; preserve the two local commits ahead of `origin/main`.
- Use stable id `visualforce-language-server` and Salesforce release `v67.4.0`.
- Cache under `lsp/visualforce-language-server/v67.4.0/` and execute `extension/dist/visualforceServer.js` only after SHA-256 verification.
- Do not add a Visualforce grammar, language directory, manifest registration, HTML attachment, extension version change, release, or issue closure.
- Do not commit VSIX files, extracted Salesforce bundles, or caches.
- All commits end with `Co-Authored-By: codex <codex@openai.com>`.

---

### Task 1: Test the Visualforce runtime boundary

**Files:**
- Create: `src/visualforce.rs`
- Modify: `src/lib.rs`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

**Interfaces:**
- Produces: `VISUALFORCE_LSP_ID`, `visualforce_language_server_command`, and `initialization_options` for `src/lib.rs`.
- Produces: pure helpers for version path selection, file hashing, injected one-attempt cache repair, and `zed::Command` construction.

- [ ] Add failing Rust unit tests for the versioned bundle path, known SHA-256 calculation, valid-cache reuse, corrupt-cache replacement, persistent mismatch error text, command arguments/environment, and embedded-language initialization JSON.
- [ ] Run `rtk cargo test visualforce` and confirm failure because the implementation does not exist.
- [ ] Implement the minimum module behavior, add `sha2 = "0.10"`, and route the stable id from `src/lib.rs` without changing `extension.toml`.
- [ ] Run `rtk cargo test visualforce` and confirm every new test passes.
- [ ] Run `rtk cargo test` to confirm existing Rust tests remain green.

### Task 2: Test the official Visualforce server end to end

**Files:**
- Create: `scripts/fixtures/visualforce/CompletionProbe.page`
- Create: `scripts/test-visualforce-lsp-smoke.py`

**Interfaces:**
- Consumes: the pinned release URL and hashes from `src/visualforce.rs`.
- Produces: a standalone command with `--cache-dir`, `--vsix-url`, `--node`, and `--expect-corrupt-bundle-failure` controls.

- [ ] Add the fixture with `apex:page`, nested Visualforce tags, expressions, embedded CSS/JavaScript, and the literal completion probe `<apex:`.
- [ ] Write the smoke script's cache/download/hash/extraction functions and negative checksum assertion first; run negative-focused self-tests or the mode and observe the expected missing implementation/failure.
- [ ] Implement LSP framing for initialize, initialized, didOpen, deterministic completion, shutdown, exit, and clean termination.
- [ ] Run `rtk python3 scripts/test-visualforce-lsp-smoke.py` and require real completion output containing counts for all labels and `apex:*` labels.
- [ ] Run the same command a second time and require output proving the cached VSIX and extracted server were reused after hash validation.
- [ ] Run `rtk python3 scripts/test-visualforce-lsp-smoke.py --expect-corrupt-bundle-failure` and require an expected/actual hash mismatch assertion with exit status zero.

### Task 3: Document the inactive runtime honestly

**Files:**
- Modify: `README.md`
- Modify: `specs/002-visualforce-lsp-runtime/spec.md`

**Interfaces:**
- Consumes: verified runtime and smoke-test results from Tasks 1 and 2.
- Produces: concise public distribution, integrity, capability, inactivity, and later-integration documentation.

- [ ] Add the README section with the pinned official VSIX, why npm is excluded, versioned cache and extracted-bundle integrity checks, proven standalone capabilities, and intentional inactivity.
- [ ] Confirm the implementation note says what changed, why, and exact verification commands without claiming user-visible Visualforce support.
- [ ] Search the feature documentation for unfinished markers and replace any scoped placeholder with final language.

### Task 4: Verify, self-review, commit, and publish

**Files:**
- Inspect every changed file and the complete diff.
- No production file beyond the scoped runtime, wiring, dependency, docs, fixture, and smoke test may change.

**Interfaces:**
- Produces: focused commits and a GitHub pull request that references issue #19 without closing it.

- [ ] Run `rtk cargo fmt`, `rtk cargo fmt -- --check`, `rtk cargo check`, and `rtk cargo test`.
- [ ] Run the Visualforce smoke twice plus its corrupt-cache negative mode.
- [ ] Run existing Apex launch/smoke checks available in the repository and `rtk node scripts/test-lwc-wrapper-smoke.js` with the pinned LWC package installed.
- [ ] Parse `extension.toml` and all language TOML files with Python `tomllib`.
- [ ] Run `rtk git diff --check`, inspect `rtk git diff`, and prove no `languages/visualforce`, `[grammars.visualforce]`, HTML mapping/attachment, downloaded binary, version bump, or unrelated refactor exists.
- [ ] Apply the `superpowers:requesting-code-review` checklist inline against the base SHA; fix all critical and important findings and re-run affected verification.
- [ ] Create coherent commits with the required trailer, rerun fresh completion verification, and confirm `rtk git status --short` is empty.
- [ ] Push `feat/visualforce-lsp-runtime`, open a PR whose body says `Refs #19`, names `Damecek/tree-sitter-visualforce` as the remaining dependency, and does not publish or close anything.
