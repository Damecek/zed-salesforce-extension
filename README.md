# zed-salesforce-extension

Salesforce DX language support for the Zed editor. Apex is backed by [`aer`](https://github.com/octoberswimmer/aer-dist/) (default) or Salesforce's Java-based Apex Language Server (`apex-jorje-lsp.jar`, opt-in). The extension scope covers additional Salesforce languages (SOQL/SOSL/logs today, LWC/Aura/Visualforce next).

## Project Goal

Build a **Zed extension** that enables Salesforce DX development with a practical MVP:

- Salesforce source files are recognized in Zed (current: Apex + SOQL/SOSL/logs).
- Basic syntax highlighting works (comments vs code, keywords, strings, etc.).
- Apex Language Server (LSP) starts successfully for Apex files.
- Core LSP features are available where the active backend supports them (at minimum diagnostics and completion).

This repository is in **MVP bootstrap implementation**: core language support and Apex LSP startup wiring are in place, with validation/operational hardening still in progress.

## Source Documentation and Inputs

Primary references for this architecture:

- Zed extension docs (development model):
  https://zed.dev/docs/extensions/developing-extensions
- Zed language extensions docs (grammar, `config.toml`, language servers):
  https://zed.dev/docs/extensions/languages
- Salesforce VS Code extension source (upstream):
  https://github.com/forcedotcom/salesforcedx-vscode.git
- Key Apex LSP bootstrap file (upstream path):
  `packages/salesforcedx-vscode-apex/src/languageServer.ts`
- Official Salesforce Apex Language Server docs:
  https://developer.salesforce.com/docs/platform/sfvscode-extensions/guide/apex-language-server.html
- Official Salesforce Java setup docs for Apex runtime:
  https://developer.salesforce.com/docs/platform/sfvscode-extensions/guide/java-setup.html
- Language Server Protocol (LSP) specification:
  https://github.com/microsoft/language-server-protocol

## Apex LSP backends

The extension supports two Apex language server backends, selectable via `lsp.apex-lsp.settings.backend`:

- `aer` (default) — [`aer`](https://github.com/octoberswimmer/aer-dist/) is a fast, modern, native Apex language server distributed as a single binary. No JVM required. Source paths are auto-discovered from `sfdx-project.json` `packageDirectories`.
- `jorje` — Salesforce's official Java-based `apex-jorje-lsp.jar` (downloaded and cached on first launch). Requires a Java 11+ runtime on the system.

To opt into jorje, set:

```json
{
  "lsp": {
    "apex-lsp": {
      "settings": { "backend": "jorje" }
    }
  }
}
```

## Apex formatting with Prettier

The Apex language declaration sets `prettier_parser_name = "apex"`, so Zed's
built-in Prettier picks up the right parser. Users still need to enable
Prettier and provide [`prettier-plugin-apex`](https://github.com/dangmai/prettier-plugin-apex).

Recommended `.zed/settings.json` for an SFDX project:

```json
{
  "languages": {
    "Apex": {
      "format_on_save": "on",
      "formatter": "prettier",
      "prettier": {
        "allowed": true,
        "parser": "apex",
        "plugins": ["prettier-plugin-apex"]
      }
    }
  }
}
```

If the project has its own `package.json` / `.prettierrc`, Zed will use the
project-local Prettier and config (so the `plugins` and `options` keys above
are ignored, but `allowed` and `parser` still apply). Anonymous Apex blocks
should use `"parser": "apex-anonymous"`.

Alternative — invoke Prettier as an external formatter (no bundled Prettier
needed, calls `npx`):

```json
{
  "languages": {
    "Apex": {
      "format_on_save": "on",
      "formatter": {
        "external": {
          "command": "npx",
          "arguments": [
            "prettier",
            "--plugin=prettier-plugin-apex",
            "--parser", "apex",
            "--stdin-filepath", "{buffer_path}"
          ]
        }
      }
    }
  }
}
```

## What We Learn from Existing Salesforce Implementation

The legacy reference for the jorje backend comes from Salesforce's VS Code extension. From `languageServer.ts`, Salesforce VS Code extension starts the language server as:

- Java command from discovered JDK/JRE home (`<java_home>/bin/java`).
- Classpath set to `apex-jorje-lsp.jar`.
- Main class: `apex.jorje.lsp.ApexLanguageServerLauncher`.
- Additional JVM flags for diagnostics/telemetry/debug features.

That means when the jorje backend is selected, this project treats **`apex-jorje-lsp.jar` as the authoritative LSP server binary payload**, launched as a Java process.

From `requirements.ts` + Salesforce Java setup docs:

- Java path resolution strategy should support explicit config + env vars (`JAVA_HOME` / `JDK_HOME`).
- Java runtime check should verify executable presence and supported version.
- The Apex LSP backend requires Java 11+; Salesforce recommends Java 21.

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
└─ (no `grammars/` dir required; Tree-sitter grammars are fetched from Git repos declared in `extension.toml`)
```

## 2) Language registration in Zed

Implement `languages/apex/config.toml` with:

- `name = "Apex"`
- `grammar = "apex"` (Tree-sitter grammar name registered in `extension.toml`, see below)
- `path_suffixes = ["cls", "trigger", "apex"]`
- `line_comments = ["// "]`

This gives Apex file association + comment behavior and is required before deeper LSP integration.

### Tree-sitter grammar (chosen implementation)

This extension uses the `tree-sitter-sfapex` grammar repository, pinned to a specific Git revision for deterministic builds:

- Repository: `https://github.com/aheber/tree-sitter-sfapex`
- Pinned commit (rev): `3597575a429766dd7ecce9f5bb97f6fec4419d5d`

Zed loads Tree-sitter grammars that are registered in `extension.toml`. Apex is one registered language in a broader Salesforce DX extension.

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

Then optionally enable/validate semantic tokens in your Zed settings to augment highlighting.

### Semantic tokens compatibility notes (`off` / `combined` / `full`)

- `off`: only Tree-sitter query highlighting is used. This is the most deterministic mode and the safest troubleshooting baseline.
- `combined`: Tree-sitter baseline + semantic token overlays when available. This is usually the best mode for Apex.
- `full`: semantic token rendering is prioritized. This can look better when server token coverage is strong, but may reduce fallback consistency if token classes are incomplete.

Note: semantic token mode is an editor/user setting. The extension cannot force this value globally for users.

Optional toggle in `.zed/settings.json` while evaluating Apex color behavior:

```json
{
  "languages": {
    "Apex": {
      "semantic_tokens": "combined"
    }
  }
}
```

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

- Define language server entry for Apex (e.g. `[language_servers.apex-lsp]`).
- Map it to language `Apex`.

In Rust extension code (`src/lib.rs`):

- Implement `language_server_command(...) -> zed::Command`.
- Branch on the configured backend (`aer` default, `jorje` opt-in).
- For `aer`: resolve the binary via `lsp.apex-lsp.binary.path` → `lsp.apex-lsp.settings.aer_path` → `worktree.which("aer")`, then launch with `aer lsp [--path <pkg-dir>]...`.
- For `jorje`: launch a Java process similarly to Salesforce VS Code:
  - `command`: resolved Java executable path
  - `args`: `-cp <path-to-jar> apex.jorje.lsp.ApexLanguageServerLauncher`
  - plus safe JVM args (`-Xmx` optional for memory control)
- Set environment variables if needed.

## 5) Apex jar sourcing (jorje backend only)

When the jorje backend is selected, the extension runtime **downloads and caches the jar on first use** inside the extension work directory.

- Canonical runtime source: the pinned upstream jar URL from `forcedotcom/salesforcedx-vscode`
- Cached runtime jar path: `<extension-workdir>/lsp/apex-lsp/apex-jorje-lsp.jar`
- The local smoke test (`scripts/test-lsp-launch.sh`) downloads the same jar into `.cache/apex-lsp/` (gitignored) and verifies its SHA-256 before each run

## 6) Java runtime acquisition strategy (jorje backend only)

Only relevant when `backend` is set to `jorje`. Required capability in extension logic:

- Prefer explicit Zed LSP setting for this server (e.g. `lsp.apex-lsp.binary.path` pointing to a `java` executable).
- Support explicit Java home via `lsp.apex-lsp.settings.java_home` (resolved as `<java_home>/bin/java`).
- Fallback to `JDK_HOME`, then `JAVA_HOME`.
- Auto-install the Apex jar on first launch and reuse the cached copy on subsequent launches.
- Validate:
  - directory exists
  - `bin/java` present
  - `java -version` reports supported major (>=11)
- Provide clear error message in Zed logs/status when invalid.

Heap behavior:

- default is preset by the extension to `-Xmx2048m` (no user config needed)
- optional override: `lsp.apex-lsp.settings.java_max_heap_mb`
- advanced override: `lsp.apex-lsp.binary.arguments` with explicit `-Xmx...` (takes precedence)

Default Apex LSP JVM properties:

- `-Ddebug.internal.errors=true`
- `-Ddebug.completion.statistics=false`
- `-Dlwc.typegeneration.disabled=true`
- advanced override: `lsp.apex-lsp.binary.arguments` with explicit `-D...` for the same property name (takes precedence)

Example `.zed/settings.json` (jorje backend):

```json
{
  "lsp": {
    "apex-lsp": {
      "settings": {
        "backend": "jorje",
        "java_home": "/Library/Java/JavaVirtualMachines/temurin-21.jdk/Contents/Home",
        "java_max_heap_mb": 2048
      }
    }
  }
}
```

Recommended docs for users on the jorje backend should include Java 21 guidance (consistent with Salesforce recommendations).

Example `.zed/settings.json` (aer backend, default — only needed if `aer` is not on `PATH`):

```json
{
  "lsp": {
    "apex-lsp": {
      "settings": {
        "aer_path": "/usr/local/bin/aer"
      }
    }
  }
}
```

## 7) Workspace assumptions and limitations

Salesforce DX language tooling behavior depends on project shape (`sfdx-project.json`, `packageDirectories`, and Salesforce DX metadata layout). For MVP, we need to be explicit about what we do (and do not) support so startup is deterministic.

### Zed worktree model (important)

Zed starts language servers per *worktree* (a directory project or a single-file worktree). Practically, this means:

- Best supported: open the **project directory** as a Zed worktree (not just a standalone `.cls` file).
- If you open a **single file** outside a directory worktree, there is typically no `sfdx-project.json` at the worktree root.
  For MVP, syntax highlighting still works; Apex LSP behavior depends on the workspace root supplied by Zed.

Zed also has a Restricted/Trusted worktree model:

- In Restricted mode, `.zed/settings.json` in the project is not applied, and language servers configured *by that project settings file* are not spawned.
- This extension is expected to provide its own language server command via `language_server_command` (installed extension code). Project settings may still matter for per-project configuration, but only after the worktree is trusted.

### Supported workspace profile (MVP)

- Primary target: an **SFDX project root** opened as the worktree root, containing `sfdx-project.json` at the top level.
- Also observed to work: a monorepo-style worktree where an SFDX project is nested in a subdirectory. This is based on the current Apex LSP behavior with the workspace root supplied by Zed; the extension does not implement explicit nested-root discovery or rewrite `rootPath`.
- Apex and related Salesforce DX source files are located according to Salesforce DX conventions, e.g. within one of the `packageDirectories` roots (commonly `force-app/main/default/...`).

Concrete examples of layouts we expect to work best:

- `sfdx-project.json`
- `force-app/main/default/classes/*.cls`
- `force-app/main/default/triggers/*.trigger`

Concrete examples that are *not* a primary MVP target (may partially work, but not guaranteed):

- “MDAPI-style” folders like `src/classes` without `sfdx-project.json`
- Standalone `.apex` scripts outside an SFDX project (highlighting should work; LSP start behavior depends on Apex server tolerance when no SFDX layout is present)

If unsupported workspace is opened, extension should degrade gracefully:

- language mode + syntax highlighting still work
- LSP issues are surfaced with actionable diagnostics

### Explicit limitations (MVP)

- Multi-root setups: Zed can have multiple worktrees open. The MVP assumes language servers are started independently per worktree and does not attempt cross-worktree indexing.
- Missing SFDX marker: the extension should not throw a fatal startup error solely because `sfdx-project.json` is absent at worktree root.
- No explicit nested-root rewrite: the extension does not scan for nested `sfdx-project.json` files and does not override the workspace root that Zed passes to the language server.
- Org-dependent features (auth files, namespace from org, etc.) are out of scope. The VS Code implementation uses the Salesforce Core extension to derive org namespace and other context; we will not replicate that during MVP.
- Restricted worktrees: the MVP should not start any external process (including Java) until the worktree is trusted, aligning with Zed’s supply-chain safety posture.

### `sfdx-project.json` parsing (future work)

The current MVP does not parse `sfdx-project.json` in extension runtime code yet.

When this is implemented, the intended minimal surface is:

- `packageDirectories`: determine which folder roots constitute Salesforce DX source packages. This is used for:
  - validation and error messages (e.g. warn when a `.cls` is outside any package directory)
  - future: limit file watching/index scope if needed
- `namespace`: used only for user-facing messaging and (future) LSP UX parity behaviors; we do not assume org namespace access in MVP.
- `sourceApiVersion`: used for compatibility decisions that depend on API version (future). For MVP it is optional and can be logged for diagnostics.

If this parsing is added later and parsing fails (invalid JSON), the extension should not start LSP and should log an actionable parse error.


## MVP Progress Update

What changed:

- Added a minimal Zed extension scaffold (`extension.toml`, `Cargo.toml`, `src/lib.rs`).
- Registered the Apex Tree-sitter grammar in `extension.toml` pinned to `tree-sitter-sfapex` commit `3597575a429766dd7ecce9f5bb97f6fec4419d5d`.
- Added Apex language configuration in `languages/apex/config.toml` with file suffixes `.cls`, `.trigger`, and `.apex`.
- Added baseline syntax highlighting query in `languages/apex/highlights.scm` sourced from upstream pinned grammar revision.
- Registered SOQL/SOSL/Salesforce Log grammars and added language configs + highlights for `.soql`, `.sosl`, and `.sflog` (and `.log`) as part of broader Salesforce DX language support.
- Implemented Apex LSP launch command wiring in `src/lib.rs` (Java resolution + managed jar download/cache).
- Removed the temporary Python stdio proxy; Apex LSP is now launched directly through Java.
- Added deterministic smoke test automation:
  - `scripts/test-lsp-launch.sh`
  - `scripts/lsp_smoke.py`
  - fixture workspace at `scripts/fixtures/sfdx-minimal/`

Why:

- This establishes a deterministic MVP baseline: file/language recognition, baseline highlighting, and automated Apex LSP startup handshake verification.
- This removes an obsolete runtime dependency on Python while keeping automated startup and completion verification.

How to verify:

1. Run `cargo check` to validate Rust extension scaffold builds.
2. Run `./scripts/test-lsp-launch.sh` to verify Java resolution, download/cache the pinned Apex LSP jar (with SHA-256 check), and exercise Apex LSP completion against both a fixture SFDX workspace root and a nested SFDX workspace inside a monorepo-style root.
3. Install as a dev extension in Zed and open `.cls` / `.trigger` / `.apex` files.
4. Confirm Apex mode is selected, comments/keywords/strings are highlighted, and Apex LSP starts when opening an SFDX project root. On first launch the extension should download the jar into its work directory, then reuse the cached copy on later launches. Optionally also verify the currently tested nested monorepo scenario.

## MVP Scope (Phase 1)

## Must have

- [x] Zed extension skeleton (`extension.toml`, Rust entrypoint).
- [x] Apex language registration (`languages/apex/config.toml`) as part of Salesforce DX language coverage.
- [x] Tree-sitter grammar registration for Salesforce DX languages (Apex/SOQL/SOSL/SF log).
- [x] Basic `highlights.scm` with comments/code distinction and common token classes.
- [x] LSP process launch path for Apex jar + Java.
- [x] Startup validation and meaningful failure logs.
- [x] Basic manual test instructions.

## Nice to have (still close to MVP)

- [x] Configurable Java home and heap size.
- [x] Semantic tokens compatibility notes (`off` / `combined` / `full`).

## Out of MVP (next phases)

- Advanced code lenses, language-specific commands (Apex/LWC/Aura/VF), log tooling.
- Org-specific indexing and search.
- Embedded SOQL enhancements.
- Deep index lifecycle controls (restart/reset UX parity).

## Suggested Implementation Milestones

1. **Scaffold extension** and install as dev extension in Zed.
2. **Language + grammar integration** and verify highlighting.
3. **Java + jar launcher** in `language_server_command`.
4. **Open sample Salesforce DX project** and validate LSP handshake/startup.
5. **Stabilize diagnostics + completion** on representative files.
6. **Document known constraints** and troubleshooting.

## Testing Strategy (including AI-agent-friendly automation)

## A) Fast static checks

- Validate TOML files parse (`extension.toml`, `languages/apex/config.toml`).
- Validate query files syntax (`highlights.scm` etc.) where tooling is available.
- Lint Rust extension code (`cargo check`, `cargo clippy` if configured).

## B) Deterministic process-level tests

`scripts/test-lsp-launch.sh` provides deterministic process-level validation:

1. Resolves Java path (setting/env simulation).
2. Downloads the pinned Apex LSP jar into `.cache/apex-lsp/` if missing and validates its SHA-256 checksum.
3. Runs short-lived Apex LSP launch smoke tests (jorje backend):
   - start the process
   - perform a minimal LSP handshake over stdio
   - open a fixture trigger file and request completion at `System.`
   - assert completion returns one or more items
   - terminate/cleanup

Run:

```bash
./scripts/test-lsp-launch.sh
```

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
- Completion works in a direct SFDX project root. The current smoke test also covers one monorepo-style parent worktree scenario with a nested SFDX project.
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

- Default Apex LSP engine: [`aer`](https://github.com/octoberswimmer/aer-dist/) (native binary, no JVM).
- Opt-in alternative: Salesforce `apex-jorje-lsp.jar` (selected via `backend = "jorje"`); requires Java 11+ (Java 21 recommended).
- Editor integration: Zed extension with Rust `language_server_command` launcher; jorje backend additionally manages jar download/cache.
- Highlighting baseline: Tree-sitter (`highlights.scm`), semantic tokens optional enhancement.
- MVP goal: reliable startup + baseline coding ergonomics across Salesforce DX languages before advanced features.

## SFDX CLI Task Templates

This repository includes a `tasks.json` file with pre-configured Salesforce CLI (`sf`) task templates for common operations (deploy, retrieve, org management, etc.).

Zed extensions currently have no mechanism to register commands or bundle task templates (unlike VS Code's `contributes.commands`). As a workaround, you can manually copy the file into your project's Zed configuration directory:

```bash
cp tasks.json <your-sfdx-project>/.zed/tasks.json
```

The tasks will then appear in Zed's task picker (cmd/ctrl+shift+t). All tasks use `$ZED_WORKTREE_ROOT` as the working directory, so they work from any file within the project.

## Notes for Next Contributors

- Keep MVP tightly scoped to reliability first.
- Avoid overfitting to VS Code-specific UX patterns not available in Zed.
- Preserve a clear separation between:
  - language metadata/highlighting (works offline, no org required)
  - language server runtime (depends on Java + project assumptions)
