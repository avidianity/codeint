# doc-links

**Invariant**:
Every relative link in a Markdown file resolves to an existing file, and every heading anchor in a link exists in its target.

**Implemented by**:
lychee 0.24.2 with [lychee.toml](../../lychee.toml), offline, for relative links and heading anchors.

**Runs in**:
Hook (`links` in [lefthook.yml](../../lefthook.yml), on staged Markdown files) and CI (`repository` job, with the release that [the lychee action](../../.github/actions/lychee/action.yml) installs and verifies).

**Error text**:

```text
{file}:{line} links to {target}, which does not exist. Fix the link or restore the target.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/doc-links.md).
