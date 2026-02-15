# zed-apex-lsp-extension

Salesforce Apex language support for the Zed editor, based on Salesforce's Apex Language Server (`apex-jorje-lsp.jar`) and Zed's extension model.

## Project Goal

Build a **Zed extension** that enables Apex development with a practical MVP:

- Apex files are recognized in Zed.
- Basic syntax highlighting works (comments vs code, keywords, strings, etc.).
- Apex Language Server (LSP) starts successfully.
- Core LSP features are available (at minimum diagnostics and completion if server/workspace supports them).

This repository currently focuses on **architecture and implementation planning** (not full implementation yet).

## Source Documentation and Inputs

Primary references for this architecture:

- Zed extension docs (development model):
  https://zed.dev/docs/extensions/developing-extensions
- Zed language extensions docs (grammar, `config.toml`, language servers):
  https://zed.dev/docs/extensions/languages
- Salesforce Apex VS Code extension source (upstream):
  https://github.com/forcedotcom/salesforcedx-vscode.git
- Key Salesforce Apex LSP bootstrap file (upstream path):
  `packages/salesforcedx-vscode-apex/src/languageServer.ts`
- Apex Language Server jar shipped by this extension:
  `vendor/apex-jorje-lsp.jar`
- Official Salesforce Apex Language Server docs:
  https://developer.salesforce.com/docs/platform/sfvscode-extensions/guide/apex-language-server.html
- Official Salesforce Java setup docs for Apex LSP runtime:
  https://developer.salesforce.com/docs/platform/sfvscode-extensions/guide/java-setup.html

## What We Learn from Existing Salesforce Implementation

From `languageServer.ts`, Salesforce VS Code extension starts the language server as:

- Java command from discovered JDK/JRE home (`<java_home>/bin/java`).
- Classpath set to `apex-jorje-lsp.jar`.
- Main class: `apex.jorje.lsp.ApexLanguageServerLauncher`.
- Additional JVM flags for diagnostics/telemetry/debug features.

That means this project should treat **`apex-jorje-lsp.jar` as the authoritative LSP server binary payload**, launched as a Java process.

From `requirements.ts` + Salesforce Java setup docs:

- Java path resolution strategy should support explicit config + env vars (`JAVA_HOME` / `JDK_HOME`).
- Java runtime check should verify executable presence and supported version.
- Apex LSP requires Java 11+; Salesforce recommends Java 21.

## Target Zed Extension Architecture

## 1) Extension package layout

Planned structure (high-level):

```text
.
├─ extension.toml
├─ Cargo.toml
├─ src/
│  └─ lib.rs
├─ languages/
│  └─ apex/
│     ├─ config.toml
│     ├─ highlights.scm
│     ├─ brackets.scm            (optional early)
│     ├─ textobjects.scm         (optional, later)
│     └─ injections.scm          (optional, later)
├─ (no `grammars/` dir required; Tree-sitter grammars are fetched from Git repos declared in `extension.toml`)
└─ vendor/
   ├─ apex-jorje-lsp.jar
   └─ apex-jorje-lsp.jar.sha256  (integrity check, optional but recommended)
```

## 2) Language registration in Zed

Implement `languages/apex/config.toml` with:

- `name = "Apex"`
- `grammar = "apex"` (Tree-sitter grammar name registered in `extension.toml`, see below)
- `path_suffixes = ["cls", "trigger", "apex"]`
- `line_comments = ["// "]`

This gives file association + comment behavior and is required before deeper LSP integration.

### Tree-sitter grammar (chosen implementation)

This extension uses the `tree-sitter-sfapex` grammar repository, pinned to a specific Git revision for deterministic builds:

- Repository: `https://github.com/aheber/tree-sitter-sfapex`
- Pinned commit (rev): `3597575a429766dd7ecce9f5bb97f6fec4419d5d`

Zed loads Tree-sitter grammars that are registered in `extension.toml`. Minimal registration for Apex:

```toml
# extension.toml
[grammars.apex]
repository = "https://github.com/aheber/tree-sitter-sfapex"
rev = "3597575a429766dd7ecce9f5bb97f6fec4419d5d"
```

`tree-sitter-sfapex` is a multi-grammar repository (it includes `apex`, `soql`, `sosl`, and `sflog` via `tree-sitter.json`).
If we decide to support these file types in Zed later, register them explicitly too:

```toml
# extension.toml
[grammars.soql]
repository = "https://github.com/aheber/tree-sitter-sfapex"
rev = "3597575a429766dd7ecce9f5bb97f6fec4419d5d"

[grammars.sosl]
repository = "https://github.com/aheber/tree-sitter-sfapex"
rev = "3597575a429766dd7ecce9f5bb97f6fec4419d5d"

[grammars.sflog]
repository = "https://github.com/aheber/tree-sitter-sfapex"
rev = "3597575a429766dd7ecce9f5bb97f6fec4419d5d"
```

Upstream file type mapping (from `tree-sitter.json`) is:

- `apex`: `.cls`, `.trigger`
- `soql`: `.soql`
- `sosl`: `.sosl`
- `sflog`: `.sflog`

For the Zed MVP we also associate `.apex` (Anonymous Apex) with the `apex` grammar via `path_suffixes`, even though it is not listed upstream.

For local grammar development (iterating on queries/grammar), Zed also supports `file://` repositories in `extension.toml`.

## 3) Syntax highlighting strategy

MVP highlighting should come from **Tree-sitter queries** (`highlights.scm`) for deterministic baseline:

- `@comment`, `@comment.doc`
- `@keyword`
- `@string`
- `@number`
- `@function`, `@type`, etc. where grammar supports it

Why Tree-sitter first:

- Semantic tokens in Zed are optional and can be off by default.
- We need a guaranteed baseline highlighting even before advanced LSP semantic token mapping.

Then optionally enable/validate semantic tokens (`semantic_tokens = "combined"` in user settings) to augment highlighting.

### Using upstream Tree-sitter highlight queries

We prefer to reuse as much as possible from `tree-sitter-sfapex` (grammar + queries), and only patch where Zed-specific needs require it.

`tree-sitter-sfapex` ships Tree-sitter query files that we can treat as the baseline source-of-truth (and they are easy to diff/update because we pin the commit):

- `apex/queries/highlights.scm`
- `soql/queries/highlights.scm`
- `sosl/queries/highlights.scm`

In Zed, queries live under `languages/<language>/` (e.g. `languages/apex/highlights.scm`). For the MVP, the intended workflow is:

- Copy the upstream `apex/queries/highlights.scm` into `languages/apex/highlights.scm` (verbatim first).
- Only then adjust captures/patterns when Zed rendering shows gaps or conflicts.

When we add `soql`/`sosl`/`sflog` as Zed languages, we should follow the same approach:

- Copy upstream queries into `languages/soql/highlights.scm`, `languages/sosl/highlights.scm`, etc.

If the pinned `rev` changes, re-sync the query files from upstream in the same PR so highlighting stays consistent with the grammar version.

## 4) LSP wiring in Zed

In `extension.toml`:

- Define language server entry (e.g. `[language_servers.apex-lsp]`).
- Map it to language `Apex`.

In Rust extension code (`src/lib.rs`):

- Implement `language_server_command(...) -> zed::Command`.
- Launch Java process similarly to Salesforce:
  - `command`: resolved Java executable path
  - `args`: `-cp <path-to-jar> apex.jorje.lsp.ApexLanguageServerLauncher`
  - plus safe JVM args (`-Xmx` optional for memory control)
- Set environment variables if needed.

## 5) Apex jar sourcing (decision)

For this project, we explicitly choose to **ship the jar inside the extension**:

- Default jar location: `vendor/apex-jorje-lsp.jar`
- Optional integrity file: `vendor/apex-jorje-lsp.jar.sha256`

Non-goal (for MVP): runtime download/caching. It adds failure modes (network/offline, trust, cache invalidation) without improving deterministic startup.

## 6) Java runtime acquisition strategy

Required capability in extension logic:

- Prefer explicit extension setting (e.g. `apex.java.home`).
- Fallback to `JDK_HOME`, then `JAVA_HOME`.
- Validate:
  - directory exists
  - `bin/java` present
  - `java -version` reports supported major (>=11)
- Provide clear error message in Zed logs/status when invalid.

Recommended docs for users should include Java 21 guidance (consistent with Salesforce recommendations).

## 7) Workspace assumptions and limitations

Salesforce Apex LSP behavior depends on project shape (`sfdx-project.json`, `packageDirectories`, and Salesforce DX metadata layout). For MVP, we need to be explicit about what we do (and do not) support so startup is deterministic.

### Zed worktree model (important)

Zed starts language servers per *worktree* (a directory project or a single-file worktree). Practically, this means:

- Best supported: open the **project directory** as a Zed worktree (not just a standalone `.cls` file).
- If you open a **single file** outside a directory worktree, there is typically no `sfdx-project.json` at the worktree root.
  For the MVP, that means **syntax highlighting will work, but the Apex LSP will not be started** (see Root discovery below).

Zed also has a Restricted/Trusted worktree model:

- In Restricted mode, `.zed/settings.json` in the project is not applied, and language servers configured *by that project settings file* are not spawned.
- This extension is expected to provide its own language server command via `language_server_command` (installed extension code). Project settings may still matter for per-project configuration, but only after the worktree is trusted.

### Supported workspace profile (MVP)

- Primary target: an **SFDX project root** opened as the worktree root, containing `sfdx-project.json` at the top level.
- Apex source files are located according to Salesforce DX conventions, e.g. within one of the `packageDirectories` roots (commonly `force-app/main/default/...`).

Concrete examples of layouts we expect to work best:

- `sfdx-project.json`
- `force-app/main/default/classes/*.cls`
- `force-app/main/default/triggers/*.trigger`

Concrete examples that are *not* a primary MVP target (may partially work, but not guaranteed):

- “MDAPI-style” folders like `src/classes` without `sfdx-project.json`
- Standalone `.apex` scripts outside an SFDX project (highlighting should work; **LSP will not be started in MVP**)

If unsupported workspace is opened, extension should degrade gracefully:

- language mode + syntax highlighting still work
- LSP issues are surfaced with actionable diagnostics

### Explicit limitations (MVP)

- Multi-root setups: Zed can have multiple worktrees open. The MVP assumes Apex LSP is started independently per worktree and does not attempt cross-worktree indexing.
- Root discovery: the MVP requires `sfdx-project.json` to exist at the *worktree root*. If it is missing, the extension should not start the language server and should emit a clear error suggesting to open the SFDX project root folder.
- Org-dependent features (auth files, namespace from org, etc.) are out of scope. The VS Code implementation uses the Salesforce Core extension to derive org namespace and other context; we will not replicate that during MVP.
- Restricted worktrees: the MVP should not start any external process (including Java) until the worktree is trusted, aligning with Zed’s supply-chain safety posture.

### `sfdx-project.json` parsing (MVP)

For deterministic behavior, the extension should parse `sfdx-project.json` from the worktree root and use only a minimal set of keys:

- `packageDirectories`: determine which folder roots constitute “source packages” for Apex. This is used for:
  - validation and error messages (e.g. warn when a `.cls` is outside any package directory)
  - future: limit file watching/index scope if needed
- `namespace`: used only for user-facing messaging and (future) LSP UX parity behaviors; we do not assume org namespace access in MVP.
- `sourceApiVersion`: used for compatibility decisions that depend on API version (future). For MVP it is optional and can be logged for diagnostics.

If parsing fails (invalid JSON), the extension should not start LSP and should log an actionable parse error.


## MVP Progress Update (Part 1: without LSP)

What changed:

- Added a minimal Zed extension scaffold (`extension.toml`, `Cargo.toml`, `src/lib.rs`).
- Registered the Apex Tree-sitter grammar in `extension.toml` pinned to `tree-sitter-sfapex` commit `3597575a429766dd7ecce9f5bb97f6fec4419d5d`.
- Added Apex language configuration in `languages/apex/config.toml` with file suffixes `.cls`, `.trigger`, and `.apex`.
- Added baseline syntax highlighting query in `languages/apex/highlights.scm` sourced from upstream pinned grammar revision.
- Registered SOQL/SOSL/Salesforce Log grammars and added language configs + highlights for `.soql`, `.sosl`, and `.sflog` (and `.log`).

Why:

- This implements the first MVP slice (language recognition + deterministic baseline highlighting) while intentionally deferring language server startup to the next phase.

How to verify:

1. Run `cargo check` to validate Rust extension scaffold builds.
2. Validate TOML syntax (e.g. parse `extension.toml` and `languages/apex/config.toml`).
3. Install as a dev extension in Zed and open `.cls` / `.trigger` / `.apex` files.
4. Confirm Apex mode is selected and comments/keywords/strings are highlighted.

## MVP Scope (Phase 1)

## Must have

- [x] Zed extension skeleton (`extension.toml`, Rust entrypoint).
- [x] Apex language registration (`languages/apex/config.toml`).
- [x] Tree-sitter grammar registration for Apex.
- [x] Basic `highlights.scm` with comments/code distinction and common token classes.
- [ ] LSP process launch path for Apex jar + Java.
- [ ] Startup validation and meaningful failure logs.
- [ ] Basic manual test instructions.

## Nice to have (still close to MVP)

- [ ] Configurable Java home and heap size.
- [ ] Semantic tokens compatibility notes (`off` / `combined` / `full`).

## Out of MVP (next phases)

- Advanced code lenses, Apex-specific commands, log tooling.
- Org-specific indexing and search.
- Embedded SOQL enhancements.
- Deep index lifecycle controls (restart/reset UX parity).

## Suggested Implementation Milestones

1. **Scaffold extension** and install as dev extension in Zed.
2. **Language + grammar integration** and verify highlighting.
3. **Java + jar launcher** in `language_server_command`.
4. **Open sample Apex project** and validate LSP handshake/startup.
5. **Stabilize diagnostics + completion** on representative files.
6. **Document known constraints** and troubleshooting.

## Testing Strategy (including AI-agent-friendly automation)

## A) Fast static checks

- Validate TOML files parse (`extension.toml`, `languages/apex/config.toml`).
- Validate query files syntax (`highlights.scm` etc.) where tooling is available.
- Lint Rust extension code (`cargo check`, `cargo clippy` if configured).

## B) Deterministic process-level tests

Create an automated script (future `scripts/test-lsp-launch.sh`) that:

1. Resolves Java path (setting/env simulation).
2. Verifies jar existence, and if `vendor/apex-jorje-lsp.jar.sha256` exists, validates the checksum.
3. Runs a short-lived Apex LSP launch smoke test:
   - start the process
   - perform a minimal LSP handshake over stdio (`initialize` -> expect a valid response -> `shutdown`/`exit`)
   - terminate/cleanup

This can be run by both humans and AI agents in CI/local.

## C) Integration smoke test in headless style

Use fixture workspace with minimal SFDX structure:

- `sfdx-project.json`
- one `.cls` file
- one `.trigger` file

Planned automated check:

- Start Zed with extension enabled in foreground/logging mode.
- Open fixture file.
- Assert logs show language server command startup and no fatal errors.

Even if full GUI assertion is hard, log-based validation is practical for agents.

## D) Manual acceptance checklist

- Open Apex file: syntax colors clearly distinguish comments, keywords, strings.
- Open `.soql` / `.sosl` / `.sflog` files: basic highlighting works.
- LSP starts without configuration surprises on Java 21.
- At least one of: diagnostics / completion / go-to-definition works.
- Failure mode with invalid Java path yields clear instruction.

## Relevant External Implementations to Study Further

- Salesforce Apex VS Code extension startup and settings handling:
  - upstream repo: `forcedotcom/salesforcedx-vscode`
  - `packages/salesforcedx-vscode-apex/src/languageServer.ts`
  - `packages/salesforcedx-vscode-apex/src/requirements.ts`
  - `packages/salesforcedx-vscode-apex/src/languageUtils/apexLanguageConfiguration.ts`
- Zed extension language/LSP model docs:
  - https://zed.dev/docs/extensions/languages
- LSP protocol reference (for capability alignment):
  - https://microsoft.github.io/language-server-protocol/

## Architecture Decisions (Current)

- Primary LSP engine: Salesforce `apex-jorje-lsp.jar`.
- Runtime: Java (11+ required, Java 21 recommended).
- Editor integration: Zed extension with Rust `language_server_command` launcher.
- Highlighting baseline: Tree-sitter (`highlights.scm`), semantic tokens optional enhancement.
- MVP goal: reliable startup + baseline coding ergonomics before advanced Salesforce features.

## Notes for Next Contributors

- Keep MVP tightly scoped to reliability first.
- Avoid overfitting to VS Code-specific UX patterns not available in Zed.
- Preserve a clear separation between:
  - language metadata/highlighting (works offline, no org required)
  - language server runtime (depends on Java + project assumptions)
