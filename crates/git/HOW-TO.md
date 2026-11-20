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

## Cache a result per commit

```rust
use codeint_git::ResultCache;

let cache = ResultCache::in_folder(result_cache_folder);
let measurement = match cache.read(merge_base, &settings) {
    Some(cached) => cached,
    None => {
        let measured = measure(&repository.commit(merge_base)?);
        cache.write(merge_base, &settings, &measured)?;
        measured
    }
};
```

- Take the folder from `Command::Diff::result_cache`; `None` means codeint runs without a cache.
- `settings` is any `Serialize` value that holds everything, other than the commit, that changes the result: the codeint version, each Plugin and its version, and the effective `Configuration`.
  A different value names a different entry.
- An entry is a JSON file named `{commit}-{SHA-1 of the settings}.json` that also stores the commit and the settings text; `read` returns the result only when both match.
- `read` returns `None` for a missing, unreadable, or unparsable entry, so the caller measures again and `write` replaces the entry.
- `write` writes a temporary file in the same folder and renames it over the entry, so a reader never sees half an entry, and two runs that race leave one whole entry.
- A `write` failure must not fail the run: report it on standard error and continue.
- The result type must read back exactly what it wrote; floating-point numbers round-trip because the git crate enables the `float_roundtrip` feature of serde_json.
