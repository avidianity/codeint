# CI and GitHub Actions

Run `codeint diff` on every pull request so Software entropy Regressions are caught before merge.

## GitHub Actions workflow

Add `.github/workflows/codeint.yml` to diff against the pull request base branch and upload SARIF findings to GitHub code scanning:

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
- `if: always() && hashFiles('codeint.sarif') != ''` ensures SARIF annotations still upload when `codeint diff` exits `1` on a Budget or Regression failure.
- To tune Budgets or Regression tolerances, edit `codeint.json`; see [Configuration](./configuration.md).
