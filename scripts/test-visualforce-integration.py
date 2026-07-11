#!/usr/bin/env python3
"""Validate Visualforce grammar, language, and server integration for Zed."""

import importlib.util
import os
import re
import subprocess
import sys
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 compatibility
    import tomli as tomllib


MINIMUM_PYTHON = (3, 10)
GRAMMAR_REPOSITORY = "https://github.com/Damecek/tree-sitter-visualforce"
GRAMMAR_REVISION = "b1f026749107d549e72b8cef841cfd3ae9cf8240"
TREE_SITTER_CLI = "tree-sitter-cli@0.26.10"
LANGUAGE_FILES = (
    "brackets.scm",
    "config.toml",
    "folds.scm",
    "highlights.scm",
    "indents.scm",
    "injections.scm",
)


def require_supported_python(version_info=sys.version_info):
    if tuple(version_info[:2]) < MINIMUM_PYTHON:
        raise RuntimeError("Visualforce integration tests require Python 3.10 or newer")


def run(command, cwd=None):
    completed = subprocess.run(
        command,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode != 0:
        detail = completed.stdout + completed.stderr
        raise RuntimeError(
            f"Command failed with exit {completed.returncode}: {' '.join(command)}\n{detail}"
        )
    return completed.stdout


def load_toml(path):
    with Path(path).open("rb") as stream:
        return tomllib.load(stream)


def assert_manifest(repo_root):
    manifest = load_toml(repo_root / "extension.toml")
    grammar = manifest.get("grammars", {}).get("visualforce")
    if grammar is None:
        raise RuntimeError("extension.toml is missing [grammars.visualforce]")
    if grammar.get("repository") != GRAMMAR_REPOSITORY:
        raise RuntimeError(f"Unexpected Visualforce grammar repository: {grammar}")
    if grammar.get("rev") != GRAMMAR_REVISION:
        raise RuntimeError(f"Unexpected Visualforce grammar revision: {grammar}")

    server = manifest.get("language_servers", {}).get("visualforce-language-server")
    if server is None:
        raise RuntimeError(
            "extension.toml is missing [language_servers.visualforce-language-server]"
        )
    if server.get("languages") != ["Visualforce"]:
        raise RuntimeError(f"Visualforce server must serve only Visualforce: {server}")
    language_ids = server.get("language_ids", {})
    if language_ids != {"Visualforce": "visualforce"}:
        raise RuntimeError(f"Unexpected Visualforce language id mapping: {language_ids}")


def assert_language_definition(repo_root):
    language_dir = repo_root / "languages" / "visualforce"
    missing = [name for name in LANGUAGE_FILES if not (language_dir / name).is_file()]
    if missing:
        raise RuntimeError(f"Visualforce language files are missing: {', '.join(missing)}")

    config = load_toml(language_dir / "config.toml")
    if config.get("name") != "Visualforce" or config.get("grammar") != "visualforce":
        raise RuntimeError(f"Unexpected Visualforce language identity: {config}")
    suffixes = config.get("path_suffixes", [])
    if set(suffixes) != {"page", "component"} or len(suffixes) != 2:
        raise RuntimeError(f"Visualforce suffixes must be page/component: {suffixes}")

    indents = (language_dir / "indents.scm").read_text(encoding="utf-8")
    if "@indent.begin" in indents or "@indent.end" in indents:
        raise RuntimeError(
            "Visualforce indents use upstream captures; Zed requires @indent with @end"
        )
    if "@indent" not in indents or "@end" not in indents:
        raise RuntimeError("Visualforce indents must contain Zed @indent and @end captures")
    return language_dir


def assert_visualforce_artifact_identity(repo_root):
    smoke_path = repo_root / "scripts" / "test-visualforce-lsp-smoke.py"
    spec = importlib.util.spec_from_file_location("visualforce_lsp_smoke", smoke_path)
    smoke = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(smoke)

    rust_source = (repo_root / "src" / "visualforce.rs").read_text(encoding="utf-8")
    expected = {
        "VISUALFORCE_LSP_RELEASE": smoke.RELEASE,
        "VISUALFORCE_LSP_BUNDLE_SHA256": smoke.SERVER_SHA256,
        "VISUALFORCE_LSP_DOWNLOAD_URL": smoke.VSIX_URL,
        "VISUALFORCE_LSP_CACHE_REL_PATH": (
            f"lsp/visualforce-language-server/{smoke.RELEASE}"
        ),
        "VISUALFORCE_LSP_SERVER_REL_PATH": smoke.SERVER_REL_PATH.as_posix(),
    }
    for constant, expected_value in expected.items():
        match = re.search(
            rf'const\s+{constant}:\s*&str\s*=\s*"([^"]+)"\s*;',
            rust_source,
            re.MULTILINE,
        )
        if match is None:
            raise RuntimeError(f"src/visualforce.rs is missing string constant {constant}")
        actual_value = match.group(1)
        if actual_value != expected_value:
            raise RuntimeError(
                f"Visualforce artifact identity drift for {constant}: "
                f"runtime={actual_value!r}; smoke={expected_value!r}"
            )


def ensure_grammar(repo_root):
    cache_dir = Path(
        os.environ.get(
            "VISUALFORCE_GRAMMAR_CACHE_DIR",
            repo_root / ".cache" / "tree-sitter-visualforce-integration",
        )
    ).resolve()
    if not (cache_dir / ".git").is_dir():
        cache_dir.parent.mkdir(parents=True, exist_ok=True)
        run(
            [
                "git",
                "clone",
                "--filter=blob:none",
                "--no-checkout",
                GRAMMAR_REPOSITORY,
                str(cache_dir),
            ]
        )
    run(["git", "fetch", "--depth", "1", "origin", GRAMMAR_REVISION], cwd=cache_dir)
    run(["git", "checkout", "--detach", GRAMMAR_REVISION], cwd=cache_dir)
    actual_revision = run(["git", "rev-parse", "HEAD"], cwd=cache_dir).strip()
    if actual_revision != GRAMMAR_REVISION:
        raise RuntimeError(
            f"Grammar checkout mismatch: expected {GRAMMAR_REVISION}; actual {actual_revision}"
        )
    return cache_dir


def assert_fixture_parses(grammar_dir, fixture):
    completed = subprocess.run(
        ["npx", "--yes", TREE_SITTER_CLI, "parse", str(fixture)],
        cwd=grammar_dir,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    output = completed.stdout + completed.stderr
    if "(ERROR" in output:
        raise RuntimeError(f"Visualforce parse contains ERROR nodes for {fixture}:\n{output}")
    expected_completion_recovery = (
        fixture.name == "CompletionProbe.page" and '(MISSING \">\"' in output
    )
    if completed.returncode != 0 and not expected_completion_recovery:
        raise RuntimeError(
            f"Visualforce parse failed with exit {completed.returncode} for {fixture}:\n{output}"
        )


def assert_queries_compile(grammar_dir, language_dir, fixtures):
    capture_output = ""
    for query_name in LANGUAGE_FILES:
        if not query_name.endswith(".scm"):
            continue
        query_path = language_dir / query_name
        for fixture in fixtures:
            capture_output += run(
                [
                    "npx",
                    "--yes",
                    TREE_SITTER_CLI,
                    "query",
                    str(query_path),
                    str(fixture),
                ],
                cwd=grammar_dir,
            )

    for capture in (" - tag.builtin,", " - punctuation.special,", " - function,"):
        if capture not in capture_output:
            raise RuntimeError(f"Visualforce query output is missing representative {capture!r}")


def main():
    require_supported_python()
    repo_root = Path(__file__).resolve().parent.parent
    assert_manifest(repo_root)
    language_dir = assert_language_definition(repo_root)
    assert_visualforce_artifact_identity(repo_root)

    fixtures = (
        repo_root / "scripts" / "fixtures" / "visualforce" / "CompletionProbe.page",
        repo_root
        / "scripts"
        / "fixtures"
        / "visualforce"
        / "CompletionProbe.component",
    )
    missing_fixtures = [str(path) for path in fixtures if not path.is_file()]
    if missing_fixtures:
        raise RuntimeError(f"Visualforce fixtures are missing: {', '.join(missing_fixtures)}")

    grammar_dir = ensure_grammar(repo_root)
    for fixture in fixtures:
        assert_fixture_parses(grammar_dir, fixture)
    assert_queries_compile(grammar_dir, language_dir, fixtures)

    print(
        "Visualforce integration test passed: manifest registration, page/component "
        "associations, runtime artifact identity, pinned grammar parsing, and all "
        "language queries are valid."
    )


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        raise SystemExit(str(error)) from error
