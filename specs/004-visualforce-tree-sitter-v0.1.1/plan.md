# Visualforce Tree-sitter v0.1.1 Update Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Update the Visualforce grammar to immutable upstream release `v0.1.1`, re-synchronize upstream queries while retaining explicit Zed adapters, and prove the new real-world syntax works.

**Architecture:** `extension.toml` remains the runtime source of the grammar pin, while `scripts/test-visualforce-integration.py` independently enforces the same revision and validates the language queries against committed fixtures. Upstream query files form the baseline; Zed-only captures and the local bracket query remain clearly scoped editor adaptations.

**Tech Stack:** Zed extension TOML, Tree-sitter Visualforce `b1f026749107d549e72b8cef841cfd3ae9cf8240`, Scheme query files, Python 3.10+ (`tomli` from `requirements-dev.txt` before Python 3.11), Tree-sitter CLI 0.26.10, Node.js/npm, Rust 2021.

**Verification prerequisite:** run `rtk python3 -m pip install -r requirements-dev.txt` before the documented Python commands.

## Global Constraints

- Pin `https://github.com/Damecek/tree-sitter-visualforce` at immutable commit `b1f026749107d549e72b8cef841cfd3ae9cf8240` (`v0.1.1`).
- Re-synchronize upstream `highlights.scm`, `injections.scm`, `indents.scm`, and `folds.scm`.
- Preserve XML declaration, doctype, and HTML punctuation highlighting as explicit Zed additions.
- Convert upstream `@indent.begin` and `@indent.end` captures to Zed `@indent` and `@end` captures.
- Preserve local `brackets.scm`; upstream does not publish a bracket query.
- Do not change Visualforce language-server runtime behavior or `.page`/`.component` associations.
- Prefix shell commands with `rtk`.
- End every commit message with `Co-Authored-By: codex <codex@openai.com>`.

---

### Task 1: Lock the v0.1.1 regression boundary

**Files:**
- Modify: `scripts/test-visualforce-integration.py`
- Modify: `scripts/fixtures/visualforce/CompletionProbe.page`
- Modify: `scripts/fixtures/visualforce/CompletionProbe.component`

**Interfaces:**
- Consumes: `extension.toml` grammar entry and the upstream repository URL.
- Produces: `GRAMMAR_REVISION = "b1f026749107d549e72b8cef841cfd3ae9cf8240"` plus page/component fixtures that exercise the release fixes.

- [x] **Step 1: Change the test-side grammar pin before the manifest pin**

In `scripts/test-visualforce-integration.py`, replace the revision constant with:

```python
GRAMMAR_REVISION = "b1f026749107d549e72b8cef841cfd3ae9cf8240"
```

- [x] **Step 2: Add v0.1.1 syntax to the page fixture**

Inside the existing `<apex:pageBlockTable>`, add a subscript expression and a quoted attribute containing literal message-format braces alongside a Visualforce expression:

```xml
                <apex:column value="{!accounts[0].Name}" />
                <apex:column>
                    <apex:outputText value="As of {0,date,dd/MM/yyyy}: {!accounts[0].Name}" />
                </apex:column>
```

Keep the incomplete `<apex:` completion probe unchanged so the existing expected recovery path remains covered.

- [x] **Step 3: Add formula-like literal text to the component fixture**

Before the existing `<style>` element, add markup that must not consume the following embedded elements:

```xml
    <pre><code>IF (VALUE(X) > 0, IMAGE("/asset?id=Y", "*", 1, 1), "")</code></pre>
```

- [x] **Step 4: Run the integration test and verify the pin mismatch fails first**

Run:

```bash
rtk python3 scripts/test-visualforce-integration.py
```

Expected: FAIL containing `Unexpected Visualforce grammar revision` and showing the old manifest revision `88d24e807898f294e9e7d575509378ba352ee297`.

- [x] **Step 5: Commit the regression boundary**

```bash
rtk git add scripts/test-visualforce-integration.py scripts/fixtures/visualforce/CompletionProbe.page scripts/fixtures/visualforce/CompletionProbe.component
rtk git commit -m $'Test Visualforce grammar v0.1.1 syntax\n\nCo-Authored-By: codex <codex@openai.com>'
```

---

### Task 2: Update the grammar and re-synchronize queries

**Files:**
- Modify: `extension.toml`
- Modify: `languages/visualforce/highlights.scm`
- Modify: `languages/visualforce/injections.scm`
- Modify: `languages/visualforce/indents.scm`
- Modify: `languages/visualforce/folds.scm`
- Verify unchanged: `languages/visualforce/brackets.scm`

**Interfaces:**
- Consumes: upstream query files from commit `b1f026749107d549e72b8cef841cfd3ae9cf8240`.
- Produces: a manifest pin that matches `GRAMMAR_REVISION` and Zed-compatible query files compiling against the new node types.

- [x] **Step 1: Update the manifest pin**

Set the Visualforce grammar entry to:

```toml
[grammars.visualforce]
repository = "https://github.com/Damecek/tree-sitter-visualforce"
rev = "b1f026749107d549e72b8cef841cfd3ae9cf8240"
```

- [x] **Step 2: Re-synchronize highlights with explicit Zed additions**

Use upstream `queries/highlights.scm` verbatim as the baseline. Retain these additions in `languages/visualforce/highlights.scm` after the upstream tag-name captures and before upstream attribute captures:

```scheme
(doctype) @tag.doctype
(xml_declaration) @tag.doctype
```

Retain these additions after upstream entity highlighting and before expression punctuation:

```scheme
"=" @punctuation.delimiter.html

[
  "<"
  ">"
  "<!"
  "</"
  "/>"
] @punctuation.bracket.html
```

- [x] **Step 3: Re-synchronize injections and folds exactly**

Replace `languages/visualforce/injections.scm` and `languages/visualforce/folds.scm` with the respective upstream `v0.1.1` query content. The expected semantic result is unchanged; verify this with:

```bash
rtk git diff -- languages/visualforce/injections.scm languages/visualforce/folds.scm
```

Expected: no semantic diff, or formatting-only changes that exactly match upstream.

- [x] **Step 4: Re-synchronize indents through the Zed capture adapter**

Translate each upstream pattern into Zed captures, producing exactly:

```scheme
(element
  (start_tag)
  (end_tag) @end) @indent

(script_element
  (start_tag)
  (end_tag) @end) @indent

(style_element
  (start_tag)
  (end_tag) @end) @indent

(argument_list
  "("
  ")" @end) @indent
```

- [x] **Step 5: Verify the local bracket query is preserved**

Run:

```bash
rtk git diff -- languages/visualforce/brackets.scm
```

Expected: no output.

- [x] **Step 6: Run the focused integration test**

```bash
rtk python3 scripts/test-visualforce-integration.py
```

Expected: PASS ending with `Visualforce integration test passed` and no unexpected parse `ERROR` nodes or query compilation errors.

- [x] **Step 7: Commit the grammar and query update**

```bash
rtk git add extension.toml languages/visualforce
rtk git commit -m $'Update Visualforce Tree-sitter to v0.1.1\n\nCo-Authored-By: codex <codex@openai.com>'
```

---

### Task 3: Document and verify the release update

**Files:**
- Modify: `README.md`
- Modify: `specs/004-visualforce-tree-sitter-v0.1.1/plan.md`
- Verify: `.cache/tree-sitter-visualforce-integration/**`
- Verify: repository Rust workspace

**Interfaces:**
- Consumes: the completed manifest, fixtures, queries, and integration test from Tasks 1 and 2.
- Produces: current public documentation and an evidence-backed completed implementation plan.

- [x] **Step 1: Update the README version statement**

Replace the Visualforce grammar sentence with:

```markdown
The grammar is pinned at commit
`b1f026749107d549e72b8cef841cfd3ae9cf8240` (release `v0.1.1`).
```

- [x] **Step 2: Run upstream v0.1.1 grammar tests**

```bash
rtk git -C .cache/tree-sitter-visualforce-integration checkout --detach b1f026749107d549e72b8cef841cfd3ae9cf8240
rtk npm --prefix .cache/tree-sitter-visualforce-integration ci
rtk npm --prefix .cache/tree-sitter-visualforce-integration test
```

Expected: checkout reports detached `v0.1.1`; dependency installation succeeds; upstream corpus, fixture, query, harness, lint, format, and metadata checks all pass.

- [x] **Step 3: Run repository verification**

```bash
rtk python3 scripts/test-visualforce-integration.py
rtk cargo test
rtk git diff --check
```

Expected: the Visualforce integration test passes, all Rust tests pass, and `git diff --check` prints no errors.

- [x] **Step 4: Review scope and record completed checks**

```bash
rtk git status --short
rtk git diff --stat HEAD~2
rtk git diff HEAD~2 -- extension.toml languages/visualforce scripts/test-visualforce-integration.py scripts/fixtures/visualforce README.md
```

Expected: only the approved Visualforce grammar, query, fixture, test, README, and plan files differ; no Visualforce LSP runtime or file-association code changed. Mark each verified plan checkbox `[x]` only after its command succeeds.

- [x] **Step 5: Commit documentation and verification record**

```bash
rtk git add README.md specs/004-visualforce-tree-sitter-v0.1.1/plan.md
rtk git commit -m $'Document Visualforce Tree-sitter v0.1.1 verification\n\nCo-Authored-By: codex <codex@openai.com>'
```
