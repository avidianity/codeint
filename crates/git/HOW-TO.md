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
- `root` is the root of the working tree; read the repository configuration file there.
- A `Snapshot` lists the files to measure, sorted by path, and the Git submodules, which codeint never measures.
- The working tree lists tracked files and untracked files that Git does not ignore.
  It leaves out symbolic links, so codeint never follows one.

## Create a scratch repository

```rust
let repository = Repository::init(&empty_folder)?;
```

- `Repository::init` runs `git init` in an existing folder through the library, with no `git` binary, and returns the repository.
- The working tree lists untracked files, so a scratch repository needs no commit before `working_tree`.

## Read the merge-base of a base ref

```rust
let merge_base = repository.merge_base_with_head("main")?;
let base = repository.commit(merge_base)?;
let bytes = base.read(&base.files()[0])?;
```

- `merge_base_with_head` resolves the base ref to a commit, as `git rev-parse` does, and returns its best merge-base with HEAD.
  It fails with `GitError::UnknownRef` that names the ref when the ref names no commit.
- `commit` lists files from Git objects and reads each blob on demand through a thread-safe repository handle.
  It never checks out the commit and never writes the index.
- Call `parallel_reader().thread_local()` in each worker thread to stream blobs without buffering the tree in memory.
  Call `blob_id(path)` to key caches on a commit file's blob object id:

```rust
let reader = base.parallel_reader();
let local = reader.thread_local();
let blob_id = local.blob_id(path);
let bytes = local.read(path)?;
```

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

## Walk the main line of HEAD

```rust
use codeint_git::CommitRange;

let latest = repository.first_parent_history(CommitRange::Latest(20))?;
let since = repository.first_parent_history(CommitRange::CommittedSince { unix_seconds })?;
for HistoryCommit { commit, committed_at_unix_seconds } in latest {
    let snapshot = repository.commit(commit)?;
}
```

- `first_parent_history` follows the first parent of each commit from HEAD, so a merge commit is listed and the commits of the branch it merged are not.
- It returns the commits oldest first.
- `CommitRange::Latest` keeps at most that many commits; `CommitRange::CommittedSince` keeps commits whose committer time is at or after the Unix time, scanning up to 14 days earlier than the cutoff to tolerate out-of-order commit timestamps before stopping.
- In a shallow clone it stops at the shallow boundary instead of failing on a missing parent.
- It fails with `GitError::NoHistory` when HEAD names no commit yet, and with `GitError::History` when a commit object cannot be read.

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

- Take the folder from the `result_cache` field of `Command::Diff` or `Command::Trend`; `None` means codeint runs without a cache.
- `settings` is any `Serialize` value that holds everything, other than the commit, that changes the result: the codeint version, each Plugin and its version, and the effective `Configuration`.
  A different value names a different entry.
- An entry is a JSON file named `{commit}-{SHA-1 of the settings}.json` that also stores the commit and the settings text; `read` returns the result only when both match.
- `read` returns `None` for a missing, unreadable, or unparsable entry, so the caller measures again and `write` replaces it.
  A cache hit refreshes the entry's modification time.
- `write` writes a temporary file in the same folder and renames it over the entry.
  A reader never sees a partial entry, and racing runs leave one whole entry.
  After writing, `write` prunes entries untouched for more than 30 days.
  When more than 5,000 entries remain, it evicts the oldest-touched entries.
- A `write` failure must not fail the run: report it on standard error and continue.
- The result type must read back exactly what it wrote; floating-point numbers round-trip because the git crate enables the `float_roundtrip` feature of serde_json.
