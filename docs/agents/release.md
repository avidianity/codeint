# Release instructions

This page applies when you prepare, dry-run, or publish a release of codeint.

## Mandatory: boundaries

- Never publish a package, create a release, or push a tag without an Owner Decision for that exact action.
  See [security.md](./security.md#mandatory-forbidden-actions).
- Never create or edit a `CHANGELOG.md` file.
  Release notes live in the pull request body and the GitHub Release only.
- Keep the exact same SemVer version in every manifest:
  - `Cargo.toml` (`[workspace.package].version`)
  - `npm/codeint/package.json` (`version` and all six `optionalDependencies`)
  - all six `npm/platforms/*/package.json` (`version`)

`cargo xtask version-sync` checks that every manifest agrees, and `cargo xtask merged-result` runs it on every pull request.

## Dry run on a branch

Run a dry run before you prepare a version or change the release workflow.

1. Draft the release notes and validate the manifests without changing files:

   ```sh
   cargo xtask prepare-release --version {version} --dry-run
   ```

2. Trigger the `release` workflow manually (`workflow_dispatch`) from the branch to build all six target archives, checksums, attestations, and npm tarballs without publishing:

   ```sh
   gh workflow run release.yml --ref {branch}
   ```

Manual (`workflow_dispatch`) runs execute `verify`, `build`, `checksums-and-attestations`, and `stage-npm`.
They skip `publish-github-release` and `publish-npm`.

## Step 1: version pull request

When the Owner asks for version `{version}`:

1. Start a branch `chore/release-{version}` from updated `main`.
2. Update every manifest and draft the release notes from Conventional Commit subjects since the last `v*` tag:

   ```sh
   cargo xtask prepare-release --version {version}
   ```

3. Verify manifest agreement and whole-repository checks:

   ```sh
   cargo xtask version-sync --tag v{version}
   cargo xtask merged-result
   ```

4. Commit the manifest updates and `Cargo.lock` with `chore(release): prepare v{version}`.
5. Open a pull request to `main` that includes the drafted release notes and the [Evidence bundle](./review-and-merge.md#mandatory-evidence-bundle).
6. Wait for all required CI checks to pass and merge the pull request.

## Step 2: tag-triggered publishing

After the version pull request merges into `main`:

1. Ask the Owner for an explicit Decision to push the tag `v{version}` on the merged commit.
2. Wait for approval.
   Do not create or push the tag before the Owner approves that exact version.
3. Fetch `main`, create the tag on `origin/main`, and push it:

   ```sh
   git fetch origin main
   git tag v{version} origin/main
   git push origin v{version}
   ```

4. Pushing `v{version}` starts `.github/workflows/release.yml`:
   - `verify` runs `cargo xtask version-sync --tag v{version}` and refuses to publish when the tag and manifests disagree.
   - `verify` drafts `RELEASE_NOTES.md` from Conventional Commit subjects since the previous `v*` tag.
   - `build` compiles and verifies all six target archives.
   - `checksums-and-attestations` writes `SHA256SUMS` and generates build provenance attestations.
   - `stage-npm` packs the six `@avidian/codeint-{os}-{arch}` platform tarballs and the `codeint` wrapper tarball.
   - `publish-github-release` creates the GitHub Release for `v{version}` with the drafted notes, the six archives, `SHA256SUMS`, `install.sh`, and `install.ps1`.
   - `publish-npm` publishes the six `@avidian/codeint-{os}-{arch}` platform packages first, then the `codeint` wrapper, with `--provenance --access public`.
