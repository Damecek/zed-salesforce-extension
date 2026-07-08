#!/usr/bin/env python3
"""Parse Salesforce language fixtures with the grammar revision pinned by extension.toml."""

from __future__ import annotations

import argparse
import subprocess
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path


REPO_URL = "https://github.com/aheber/tree-sitter-sfapex"
TREE_SITTER_CLI = "tree-sitter-cli@0.24.0"


@dataclass(frozen=True)
class Fixture:
    language: str
    path: str
    expect_error: bool = False
    note: str = ""


@dataclass(frozen=True)
class QueryCheck:
    language: str
    query_path: str
    source_path: str
    required_output: tuple[str, ...] = ()


FIXTURES = [
    Fixture("apex", "scripts/fixtures/apex-summer26/Summer26ConstructsTest.cls"),
    Fixture(
        "apex",
        "scripts/fixtures/known-gaps/apex-inline-soql-formula.cls",
        expect_error=True,
        note="SOQL FORMULA() in WHERE is not supported by tree-sitter-sfapex yet.",
    ),
    Fixture(
        "soql",
        "scripts/fixtures/known-gaps/soql-formula-where.soql",
        expect_error=True,
        note="SOQL FORMULA() in WHERE is not supported by tree-sitter-sfapex yet.",
    ),
]

QUERY_CHECKS = [
    QueryCheck(
        "apex",
        "languages/apex/highlights.scm",
        "scripts/fixtures/apex-summer26/Summer26ConstructsTest.cls",
        (
            "text: `IntegrationTest`",
            "text: `TearDown`",
            "capture: string",
        ),
    ),
    QueryCheck(
        "apex",
        "languages/apex/runnables.scm",
        "scripts/fixtures/apex-summer26/Summer26ConstructsTest.cls",
        (
            "text: `Summer26ConstructsTest`",
            "text: `multilineStringTemplateAndUserModeDml`",
        ),
    ),
]


def run(args: list[str], cwd: Path | None = None, check: bool = True) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        args,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if check and completed.returncode != 0:
        print(completed.stdout, end="")
        print(completed.stderr, end="", file=sys.stderr)
        raise SystemExit(completed.returncode)
    return completed


def grammar_rev(repo_root: Path) -> str:
    with (repo_root / "extension.toml").open("rb") as extension_toml:
        config = tomllib.load(extension_toml)
    return config["grammars"]["apex"]["rev"]


def ensure_grammar(repo_root: Path, rev: str) -> Path:
    grammar_root = repo_root / ".cache" / "tree-sitter-sfapex"
    if not (grammar_root / ".git").is_dir():
        grammar_root.parent.mkdir(parents=True, exist_ok=True)
        run(["git", "clone", "--filter=blob:none", "--no-checkout", REPO_URL, str(grammar_root)])

    run(["git", "fetch", "--depth", "1", "origin", rev], cwd=grammar_root)
    run(["git", "checkout", "--detach", rev], cwd=grammar_root)
    return grammar_root


def parse_fixture(repo_root: Path, grammar_root: Path, fixture: Fixture) -> bool:
    source_path = repo_root / fixture.path
    parser_dir = grammar_root / fixture.language
    completed = run(
        ["npx", "--yes", TREE_SITTER_CLI, "parse", str(source_path)],
        cwd=parser_dir,
        check=False,
    )
    parsed_cleanly = completed.returncode == 0 and "ERROR" not in completed.stdout

    if fixture.expect_error:
        if parsed_cleanly:
            print(f"unexpected pass: {fixture.path}")
            print(f"  {fixture.note}")
            return False
        print(f"known gap confirmed: {fixture.path}")
        print(f"  {fixture.note}")
        return True

    if parsed_cleanly:
        print(f"parsed cleanly: {fixture.path}")
        return True

    print(f"parse failed: {fixture.path}")
    print(completed.stdout, end="")
    print(completed.stderr, end="", file=sys.stderr)
    return False


def query_fixture(repo_root: Path, grammar_root: Path, check: QueryCheck) -> bool:
    source_path = repo_root / check.source_path
    query_path = repo_root / check.query_path
    parser_dir = grammar_root / check.language
    completed = run(
        ["npx", "--yes", TREE_SITTER_CLI, "query", str(query_path), str(source_path)],
        cwd=parser_dir,
        check=False,
    )

    missing = [snippet for snippet in check.required_output if snippet not in completed.stdout]
    if completed.returncode == 0 and not missing:
        print(f"query valid: {check.query_path} on {check.source_path}")
        return True

    print(f"query failed: {check.query_path} on {check.source_path}")
    if missing:
        print("missing expected query output:")
        for snippet in missing:
            print(f"  {snippet}")
    print(completed.stdout, end="")
    print(completed.stderr, end="", file=sys.stderr)
    return False


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="Repository root. Defaults to the parent of scripts/.",
    )
    args = parser.parse_args()

    repo_root = args.repo_root.resolve()
    rev = grammar_rev(repo_root)
    grammar_root = ensure_grammar(repo_root, rev)

    ok = True
    for fixture in FIXTURES:
        ok = parse_fixture(repo_root, grammar_root, fixture) and ok
    for fixture in QUERY_CHECKS:
        ok = query_fixture(repo_root, grammar_root, fixture) and ok

    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
