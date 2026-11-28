# Release instructions

This page applies when you prepare, dry-run, or publish a release of codeint.

## Mandatory: boundaries

- Never publish a package, create a release, or push a tag without an Owner Decision for that exact action.
  See [security.md](./security.md#mandatory-forbidden-actions).
- Never create or edit a repository-root `CHANGELOG.md` file.
  Repository release notes live in the pull request body and the GitHub Release only.
  Published pub.dev packages (`sdks/dart/CHANGELOG.md` and any future `sdks/*/CHANGELOG.md`) keep a package changelog updated automatically by `cargo xtask prepare-release`; never edit those files by hand.
- Keep the exact same SemVer version in every manifest and SDK version file:
  - `Cargo.toml` (`[workspace.package].version`) and `Cargo.lock`
  - `npm/codeint/package.json` (`version` and all six `optionalDependencies`)
  - all six `npm/platforms/*/package.json` (`version`)
  - `sdks/dart/pubspec.yaml` (`version`)
  - `sdks/dart/lib/src/options.dart` (`codeintVersion`)
  - `sdks/dart/CHANGELOG.md` (`## {version}` heading)
  - any future `sdks/*/pubspec.yaml` (`version`) and its sibling `CHANGELOG.md` (`## {version}` heading)

`cargo xtask version-sync` checks that every manifest, `codeintVersion`, and sibling SDK changelog agrees, and `cargo xtask merged-result` runs it on every pull request.

## One-time pub.dev bootstrap (Owner only)

Before automated tag publishing can publish the Dart `codeint` package to pub.dev, the Owner completes one manual bootstrap:

1. Run `dart pub publish` once locally from `sdks/dart` to register the `codeint` package on pub.dev.
2. In the pub.dev package admin settings for `codeint`, enable GitHub Actions OIDC trusted publishing for repository `avidianity/codeint` with tag pattern `v{{version}}`.

## Dry run on a branch

Run a dry run before you prepare a version or change the release workflow.

1. Draft the release notes and validate the manifests without changing files:

   ```sh
   cargo xtask prepare-release --version {version} --dry-run
   ```

2. Trigger the `release` workflow manually (`workflow_dispatch`) from the branch to build all six target archives, checksums, attestations, npm tarballs, and Dart dry-run validation without publishing:

   ```sh
   gh workflow run release.yml --ref {branch}
   ```

Manual (`workflow_dispatch`) runs execute `verify`, `build`, `checksums-and-attestations`, `stage-npm`, and `stage-dart`.
They skip `publish-github-release`, `publish-npm`, and `publish-pub`.

## Step 1: version pull request

When the Owner asks for version `{version}`:

1. Start a branch `chore/release-{version}` from updated `main`.
2. Update every manifest, SDK version file, sibling SDK changelog, and `Cargo.lock`, and draft the release notes from Conventional Commit subjects since the last `v*` tag:

   ```sh
   cargo xtask prepare-release --version {version}
   ```

3. Verify manifest agreement and whole-repository checks:

   ```sh
   cargo xtask version-sync --tag v{version}
   cargo xtask merged-result
   ```

4. Commit the manifest updates, `sdks/dart/lib/src/options.dart`, `sdks/dart/CHANGELOG.md`, and `Cargo.lock` with `chore(release): prepare v{version}`.
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
   - `verify` runs `cargo xtask version-sync --tag v{version}` and refuses to publish when the tag, manifests, `codeintVersion`, or SDK changelogs disagree.
   - `verify` drafts `RELEASE_NOTES.md` from Conventional Commit subjects since the previous `v*` tag.
   - `build` compiles and verifies all six target archives.
   - `checksums-and-attestations` writes `SHA256SUMS` and generates build provenance attestations.
   - `stage-npm` packs the six `@avidian/codeint-{os}-{arch}` platform tarballs and the `codeint` wrapper tarball.
   - `stage-dart` runs `dart pub get`, `dart analyze --fatal-infos`, and `dart pub publish --dry-run` in `sdks/dart` on both `workflow_dispatch` and `v*` tags, gating all publish jobs.
   - `publish-github-release` creates the GitHub Release for `v{version}` with the drafted notes, the six archives, `SHA256SUMS`, `install.sh`, and `install.ps1`.
   - `publish-npm` publishes the six `@avidian/codeint-{os}-{arch}` platform packages first, then the `codeint` wrapper, using GitHub Actions OIDC Trusted Publishing (`id-token: write`, no `NPM_TOKEN` secret) with `--provenance --access public`.
   - `publish-pub` runs on `v*` tags after `publish-github-release` (so the release archives and `SHA256SUMS` are live before Dart users install the new version), using GitHub Actions OIDC trusted publishing (`id-token: write`, no pub.dev token secret) to run `dart pub get`, `dart analyze --fatal-infos`, `dart pub publish --dry-run`, and `dart pub publish --force` in `sdks/dart`.
