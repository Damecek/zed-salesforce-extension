# Visualforce Tree-sitter v0.1.1 Update Design

## Goal

Update the Visualforce Tree-sitter integration from upstream release `v0.1.0`
(`88d24e807898f294e9e7d575509378ba352ee297`) to `v0.1.1`
(`b1f026749107d549e72b8cef841cfd3ae9cf8240`). The update must retain Zed's
language-specific behavior while covering the parser fixes introduced by the
new release.

## Approach

Pin the immutable `v0.1.1` commit in both `extension.toml` and the Visualforce
integration test. Re-synchronize every query that upstream publishes:
`highlights.scm`, `injections.scm`, `indents.scm`, and `folds.scm`.

Keep the upstream query structure as the baseline, with only these explicit
Zed adaptations:

- retain XML declaration, doctype, and HTML punctuation highlighting;
- translate upstream indentation captures from `@indent.begin` and
  `@indent.end` to Zed's `@indent` and `@end` captures;
- retain the local `brackets.scm`, because upstream does not publish one.

No Visualforce language-server runtime or file-association behavior changes.

## Regression Coverage

Extend the committed Visualforce fixtures with representative `v0.1.1`
syntax:

- subscript expressions such as `records[index].Name`;
- literal message-format braces mixed with Visualforce expressions in quoted
  attributes;
- formula-like literal text followed by embedded or ordinary markup.

The integration test must check out the exact pinned grammar, parse both page
and component fixtures without unexpected `ERROR` nodes, compile every shipped
query, and assert representative captures from the synchronized queries.

## Verification

Run the Visualforce integration test against the new commit, run the upstream
grammar test suite at `v0.1.1`, run the repository Rust tests, and finish with
`git diff --check` plus a focused diff review. Any incompatibility in a query
or fixture is fixed in the smallest relevant Visualforce file; unrelated
refactoring is out of scope.

## Delivery Note

- **What changed:** the Visualforce grammar pin, synchronized queries, and
  targeted regression fixtures.
- **Why:** `v0.1.1` adds real-world parsing fixes for subscripts, literal
  braces, and formula-like content.
- **How to verify:** follow the commands recorded in the implementation plan
  and the final verification report.
