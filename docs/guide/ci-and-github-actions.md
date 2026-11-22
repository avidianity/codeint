# CI and GitHub Actions

Run `codeint diff` on every pull request so Software entropy Regressions are caught before merge.

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
| `version` | `latest` | Release tag (for example `v0.4.0`) or `latest`. |
| `comment` | `true` | Post or update one sticky pull-request summary comment. |
| `upload-sarif` | `false` | Upload `codeint.sarif` to GitHub code scanning (requires `security-events: write`). |
| `fail-on-regression` | `true` | Fail the step when `codeint diff` exits `1`. |
| `token` | `${{ github.token }}` | Token used to post or update the pull-request comment. |

## Manual workflow with SARIF upload

If you prefer calling the CLI directly without the composite action:

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
      - name: Diff against pull request base
        env:
          BASE_REF: ${{ github.base_ref }}
        run: codeint diff "origin/$BASE_REF" --format sarif --output codeint.sarif
      - name: Upload SARIF
        if: always() && hashFiles('codeint.sarif') != ''
        uses: github/codeql-action/upload-sarif@1bd86f2fdb4b86187fa4059f16f31a3202acc9ea
        with:
          sarif_file: codeint.sarif
```

## Notes

- `fetch-depth: 0` fetches full branch history so `codeint diff` can compute the Git merge-base.
- Grant `pull-requests: write` for sticky pull-request comments, and `security-events: write` when `upload-sarif: true`.
- To tune Budgets or Regression tolerances, edit `codeint.json`; see [Configuration](./configuration.md).
