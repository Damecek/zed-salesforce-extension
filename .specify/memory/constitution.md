<!--
Sync Impact Report
- Version change: 1.0.0 -> 1.0.1
- Clarified Architecture Constraints: Apex LSP backend is now `aer` by default,
  with `jorje` as opt-in; jar-sourcing scope narrowed to the jorje backend.
- Templates requiring updates:
  - .specify/templates/plan-template.md ✅ aligned
  - .specify/templates/spec-template.md ✅ aligned
  - .specify/templates/tasks-template.md ✅ aligned
- Follow-up TODOs: none
-->

# Zed Salesforce Extension Constitution

## Core Principles

### I. Incremental Reliability

Every change MUST be testable in isolation and MUST NOT regress existing
functionality. Prefer a working subset over a broad but fragile feature set.
New features are added only when the current baseline is stable and verified.
Complexity MUST be justified against a simpler alternative.

### II. Zed-Native Design

The extension MUST follow Zed extension patterns, APIs, and conventions as
the primary design constraint. Do not replicate VS Code-specific UX patterns
that have no Zed equivalent. When Zed's API does not support a desired
capability, use the closest idiomatic mechanism (e.g., task templates for CLI
commands) rather than inventing non-standard workarounds.

### III. Deterministic Baseline

Tree-sitter highlighting MUST provide a usable editing experience without any
LSP or network dependency. Language recognition, syntax coloring, bracket
matching, and code outline MUST work offline and without an authenticated
Salesforce org. LSP features (diagnostics, completion, hover) are additive
enhancements that degrade gracefully when unavailable.

### IV. Source-Format First

The extension targets modern Salesforce DX source-format projects
(`sfdx-project.json` + `packageDirectories`). MDAPI-format and legacy project
layouts are not primary targets. Features SHOULD assume source-format
conventions but MUST NOT crash or block startup when the project layout is
unexpected.

### V. Automation-Ready Testing

Every testable behavior MUST have a repeatable verification path that both
humans and AI agents can execute. Prefer scripted smoke tests and CLI-based
validation over manual GUI-only checks. Log-based assertions are acceptable
when GUI assertion tooling is unavailable.

## Architecture Constraints

- **Runtime stack**: Rust (WASI target) for the Zed extension host. Apex LSP
  has two selectable backends: `aer` (default, native binary, no JVM) and
  `jorje` (opt-in, requires Java 11+; Java 21 recommended).
- **Grammar source**: `tree-sitter-sfapex` pinned to a specific Git revision.
  Query files (`highlights.scm`, etc.) MUST stay in sync with the pinned
  grammar version.
- **LSP jar sourcing (jorje backend only)**: Managed download-and-cache on
  first launch from a pinned upstream URL
  (`forcedotcom/salesforcedx-vscode`). The local smoke test downloads the
  same jar into a gitignored cache and verifies a pinned SHA-256 checksum
  before launch. The default `aer` backend is a user-installed native
  binary and does not use this mechanism.
- **Supported languages**: Apex (LSP-backed), SOQL, SOSL, Salesforce Log
  (Tree-sitter only for now). LWC/Aura/Visualforce are planned expansions.
- **Extension API**: `zed_extension_api` 0.7. Declarative features (task
  templates, language configs) are preferred over Rust code when both options
  exist.

## Development Workflow

- Keep README architecture sections up to date with each significant decision.
- Include a concise "what changed / why / how to verify" note with MVP-phase
  changes.
- Do not introduce unrelated refactors during focused feature work.
- Use SpecKit artifacts (`spec.md`, `plan.md`, `tasks.md`) to formalize
  non-trivial features before implementation.
- Commit after each logical unit of work; prefer small, reviewable diffs.

## Governance

This constitution is the authoritative source for project-level engineering
decisions. All contributions MUST be consistent with these principles.
Amendments require:

1. A written rationale explaining what changed and why.
2. A version bump following semantic versioning (MAJOR for principle
   removal/redefinition, MINOR for additions, PATCH for clarifications).
3. A sync impact check against SpecKit templates and AGENTS.md.

Use `AGENTS.md` for runtime contributor guidance that complements (but does
not override) this constitution.

**Version**: 1.0.1 | **Ratified**: 2026-04-17 | **Last Amended**: 2026-05-06
