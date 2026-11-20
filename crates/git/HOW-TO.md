# How to read files from Git

## Read the working tree

```rust
use codeint_git::Repository;

let repository = Repository::discover(&folder)?;
let tree = repository.working_tree()?;
for path in tree.files() {
    let bytes = tree.read(path)?;
}
```

- `Repository::discover` finds the repository that contains the folder, at any depth, and fails on a bare repository.
- `root` is the root of the working tree; read `codeint.json` there.
- A `Snapshot` lists the files to measure, sorted by path, and the Git submodules, which codeint never measures.
- The working tree lists tracked files and untracked files that Git does not ignore.
  It leaves out symbolic links, so codeint never follows one.

## Read the merge-base of a base ref

```rust
let merge_base = repository.merge_base_with_head("main")?;
let base = repository.commit(merge_base)?;
let bytes = base.read(&base.files()[0])?;
```

- `merge_base_with_head` resolves the base ref to a commit, as `git rev-parse` does, and returns its best merge-base with HEAD.
  It fails with `GitError::UnknownRef` that names the ref when the ref names no commit.
- `commit` lists the files of a commit from Git objects and reads each blob on demand; it never checks out the commit and never writes the index.
- A commit `Snapshot` follows the same rules as the working tree: regular and executable files are files, symbolic links are left out, and submodules are listed.
- Measure both `Snapshot` values with the same code; the caller never knows which kind it holds.
- `CommitId` displays as the full hexadecimal object id; use it to name the commit in a report or a cache key.

## Read the merge-base of two refs

```rust
let merge_base = repository.merge_base("main", "feature")?;
let head = repository.commit(repository.resolve("feature")?)?;
```

- `resolve` turns a ref into a `CommitId`, as `git rev-parse` does, and peels a tag to its commit.
  It fails with `GitError::UnknownRef` that names the ref when the ref names no commit.
- `merge_base` resolves both refs and returns their best merge-base; HEAD plays no part.
- Both `merge_base` and `merge_base_with_head` fail with `GitError::NoMergeBase` when the two sides share no commit; the error names both refs, and the head ref is `HEAD` for `merge_base_with_head`.
