# CI and GitHub Actions

Run `codeint diff` on every pull request so Software entropy Regressions, Budget failures, Ratchet violations, boundary violations, orphan files, and dependency hygiene problems are caught before merge.

## One-line GitHub Action (recommended)

Use the reusable `avidianity/codeint` action to install codeint, diff against the pull-request base, and maintain one updating sticky pull-request summary comment:

```yaml
name: codeint

on:
  pull_request:

permissions:
  contents: read
  pull-requests: write

jobs:
  codeint:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
        with:
          fetch-depth: 0
      - uses: avidianity/codeint@main
```

### Action inputs

| Input | Default | Description |
| ----- | ------- | ----------- |
| `base-ref` | `origin/<github.base_ref>` | Base Git ref to diff against. |
| `path` | `.` | Directory to measure inside the repository. |
| `version` | `latest` | Release tag (for example `v0.6.0`) or `latest`. |
| `comment` | `true` | Post or update one sticky pull-request summary comment. |
| `upload-sarif` | `false` | Upload `codeint.sarif` to GitHub code scanning (requires `security-events: write`). |
| `fail-on-regression` | `true` | Fail the step when `codeint diff` exits `1`. |
| `token` | `${{ github.token }}` | Token used to post or update the pull-request comment. |

## Single-scan multi-format workflow with `codeint report`

To emit Markdown, SARIF, and terminal output in one CI run without analyzing the repository twice, save `codeint diff --format json` once and re-render it losslessly with `codeint report`:

```yaml
name: codeint

on:
  pull_request:

permissions:
  contents: read
  security-events: write

jobs:
  codeint:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
        with:
          fetch-depth: 0
      - name: Install codeint
        run: |
          curl -fsSL https://github.com/avidianity/codeint/releases/latest/download/install.sh | bash
          echo "$HOME/.local/bin" >> "$GITHUB_PATH"
      - name: Measure pull request once
        id: measure
        env:
          BASE_REF: ${{ github.base_ref }}
        run: |
          set +e
          codeint diff "origin/$BASE_REF" --format json --output codeint-diff.json
          echo "exit_code=$?" >> "$GITHUB_OUTPUT"
      - name: Render Markdown summary and SARIF
        if: always() && hashFiles('codeint-diff.json') != ''
        run: |
          codeint report codeint-diff.json --format markdown >> "$GITHUB_STEP_SUMMARY"
          codeint report codeint-diff.json --format sarif --output codeint.sarif
      - name: Upload SARIF
        if: always() && hashFiles('codeint.sarif') != ''
        uses: github/codeql-action/upload-sarif@1bd86f2fdb4b86187fa4059f16f31a3202acc9ea
        with:
          sarif_file: codeint.sarif
      - name: Enforce exit code
        if: steps.measure.outputs.exit_code != '0'
        run: exit "${{ steps.measure.outputs.exit_code }}"
```

## Scoping by `CODEOWNERS` and Workspace packages

- Pass `--owner <pattern>` to `codeint check` or `codeint diff` to report only Findings owned by a specific team or person from your repository's `CODEOWNERS` file:

  ```sh
  codeint diff origin/main --owner @org/payments
  ```

- Pass `--package <path>` in a monorepo to scope `check`, `diff`, `impact`, or `trace` to one Workspace package (whole-repository `.codeint/ratchet.json` enforcement is skipped with a `ratchet_note` on `--package` runs):

  ```sh
  codeint diff origin/main --package packages/billing
  ```

- Commit `.codeint/ratchet.json` (created and tightened with `codeint ratchet update`) so whole-repository `codeint check` and `codeint diff` runs fail in CI if any Metric rises above its recorded ceiling or if a Metric version upgrade needs re-baselining.

- Pair `codeint diff` in CI with local `codeint guard <path>` checks (or the `guard` MCP tool) so architectural boundary rules are checked before code is even written.

## Notes

- `fetch-depth: 0` fetches full branch history so `codeint diff` can compute the Git merge-base.
- Grant `pull-requests: write` for sticky pull-request comments, and `security-events: write` when uploading SARIF.
- To tune Budgets, Regression limits, Ratchet paths, or boundaries, edit `codeint.jsonc`, `codeint.json`, `.codeint/config.jsonc`, or `.codeint/config.json`; see [Configuration](./configuration.md).
