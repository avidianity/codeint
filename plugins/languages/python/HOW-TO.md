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
- Relative imports (`.foo`, `..bar`) and repository-root or `src/`-layout package imports resolve to repository `.py` files, while standard-library and third-party modules are classified as `External`.

## Reference ranges

- `reference_ranges` returns the 10th to 90th percentile bands from the 20-repository pre-2023 Python panel recorded in [ADR 0013](../../../docs/adr/0013-python-reference-ranges.md).
