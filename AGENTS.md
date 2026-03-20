# AGENTS.md

Guidance for AI/human contributors working in this repository.

## Mission Context

This repository defines a **Zed extension for Salesforce DX languages**. The current LSP backend is Salesforce's Java-based Apex Language Server (`apex-jorje-lsp.jar`) for Apex support.

Current project stage is architecture-first:

- prioritize documentation, design decisions, and implementation sequencing
- avoid speculative complexity before MVP startup is stable

## Primary Objective (MVP)

Deliver a minimal but functional Salesforce DX experience in Zed:

1. Salesforce DX file recognition (current MVP: `.cls`, `.trigger`, `.apex`, `.soql`, `.sosl`, `.sflog`/`.log`)
2. Baseline syntax highlighting (especially comment vs code distinction)
3. Apex LSP process starts successfully through Java for Apex files
4. Basic LSP functionality works (diagnostics/completion at minimum where supported by the active backend)

## Core References

Use these as authoritative sources:

- Zed extension development docs:  
  https://zed.dev/docs/extensions/developing-extensions
- Zed language/LSP docs:  
  https://zed.dev/docs/extensions/languages
- In addition to the URLs above, you may use the Context7 documentation server for Zed docs/source:
  - `websites/zed_dev`
  - `zed-industries/zed`
- Apex Language Server jar currently shipped by this extension:
  - `vendor/apex-jorje-lsp.jar`
  - `vendor/apex-jorje-lsp.jar.sha256`
- Salesforce VS Code Apex extension reference implementation (upstream):
  - https://github.com/forcedotcom/salesforcedx-vscode/tree/main/packages/salesforcedx-vscode-apex/src
- Salesforce Apex Language Server docs:
  - https://developer.salesforce.com/docs/platform/sfvscode-extensions/guide/apex-language-server.html
  - https://developer.salesforce.com/docs/platform/sfvscode-extensions/guide/java-setup.html

## Engineering Principles

- Keep implementation incremental and testable.
- Prefer deterministic startup over feature breadth.
- Preserve clear fallback/error messages for Java/jar misconfiguration.
- Favor Tree-sitter highlighting as baseline; treat semantic tokens as additive.
- Document all assumptions, especially workspace assumptions around `sfdx-project.json`.
- Keep language scope explicit: Apex is the first LSP-backed language, while LWC/Aura/Visualforce are planned expansion areas.

## Expected Implementation Shape

When code implementation begins, contributors should generally work toward:

- `extension.toml` with language server registration
- `src/lib.rs` implementing `language_server_command`
- `languages/apex/config.toml`
- `languages/apex/highlights.scm`
- `languages/soql|sosl|sflog/*` baseline language metadata/highlighting
- jar sourcing strategy (managed download with vendored provenance)

## Java + Jar Runtime Expectations

- Java major version must be >= 11 (recommend 21).
- Java resolution order should be: extension setting -> `JDK_HOME` -> `JAVA_HOME`.
- Verify `bin/java` exists and is executable.
- Launch main class `apex.jorje.lsp.ApexLanguageServerLauncher` with classpath to `apex-jorje-lsp.jar`.

## Testing Expectations

Prefer automation an AI agent can run repeatedly:

1. Static/config checks (`cargo check`, TOML validity, query syntax when tooling exists)
2. Scripted LSP launch smoke test (Java + jar + startup/teardown)
3. Fixture workspace integration check (SFDX-like project)
4. Log-based assertions for startup success/failure diagnostics

If a test cannot be executed in current environment, state exact limitation and provide a fallback check.

## Change Discipline

- Keep README architecture sections up to date with each significant decision.
- For MVP changes, include a concise "what changed / why / how to verify" note.
- Do not introduce unrelated refactors during MVP bootstrap tasks.

## Non-goals at this stage

- Full parity with Salesforce VS Code extension UX
- Advanced language-specific commands/tooling beyond baseline LSP
- Large-scale optimization before reliability is proven
