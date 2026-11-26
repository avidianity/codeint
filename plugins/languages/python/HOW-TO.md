# How to change the rules of the Python Language plugin

## Overview

`plugins/languages/python` (`codeint-lang-python`, plugin name `python`) is a built-in Language plugin that lowers `.py` files into the language-neutral Code model using a pure-Rust `#![forbid(unsafe_code)]` indentation-aware lexer and structural analyzer targeting the modern Python 3 superset.

## Source sets and workspaces

- `src/source_set.rs` classifies a `.py` file as `Generated` when its filename ends with `_pb2.py` or `_pb2_grpc.py` or when a leading comment contains a generated-file marker, `Test` when its path is under `test/` or `tests/` or its filename is `conftest.py`, starts with `test_`, or ends with `_test.py`, and `Production` otherwise (`vendor/` and `third_party/` are settled by the core).
- `src/workspaces.rs` reads `[tool.uv.workspace].members` and `exclude` from the root `pyproject.toml` to report Workspace packages.

## Functions, tokens, and Flagged lines

- `src/functions.rs` extracts `def`, `async def`, and `lambda` functions and counts decision points (`if`, `elif`, `for`, `while`, `except`, non-wildcard `case` arms, `and`, and `or`).
- `src/imports.rs` marks `import` and `from ... import` statements as `TokenKind::ImportOrExport`.
- `src/rules.rs` implements five zero-false-positive structural Flagged-line rules: `conditional-yields-boolean`, `else-after-return`, `except-only-reraises`, `if-else-returns-boolean`, and `trailing-bare-return`.

## Imports and module resolution

- `src/imports.rs` and `src/resolution.rs` extract runtime imports while ignoring imports inside `if TYPE_CHECKING:` (or `if typing.TYPE_CHECKING:`) guard blocks and `from __future__ import` directives.
- Relative imports (`.foo`, `..bar`, and `from . import Symbol`) and repository-root or `src/`-layout package imports resolve to repository `.py` files, standard-library top-level modules are classified as `Builtin`, and third-party modules are classified as `External` with PEP 503 normalized package names.
- `src/entry_points.rs` discovers top-level package `__init__.py` and `__main__.py` entry points (excluding non-package tooling/test/docs folders) plus `[project.scripts]`, `[project.gui-scripts]`, `[project.entry-points]`, and Poetry script entry points from `pyproject.toml`.
- `src/dependencies.rs` parses runtime, dev, and optional dependencies from `pyproject.toml` (`[project]`, `[dependency-groups]`, `[tool.uv]`, `[tool.pdm]`, and `[tool.poetry]`), `requirements*.txt`, `setup.cfg`, and `Pipfile` per Workspace package using PEP 503 normalization while excluding local workspace package names.

## Framework profiles

- `src/frameworks/` implements isolated per-framework profiles for `django` (`django.rs`), `fastapi` (`fastapi.rs`), and `flask` (`flask.rs`) without sibling cross-imports, keeping per-file lowering framework-blind.
- Each profile requires a package-local manifest signal paired with local convention files, contributes framework entry points, framework-generated Source set paths (`**/migrations/[0-9][0-9][0-9][0-9]_*.py` and `staticfiles/` for Django; `instance/` for Flask), and profile-gated fallback import roots in `src/resolution.rs`.

## Reference ranges

- `reference_ranges` returns the 10th to 90th percentile bands from the stratified 50-repository current-codebase Python panel under [ADR 0028](../../../docs/adr/0028-stratified-reference-panels.md).
