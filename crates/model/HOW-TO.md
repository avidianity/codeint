# How to use the Code model and the Language plugin interface

## Write a built-in Language plugin

```rust
use codeint_model::language::{
    FileModel, Guest, ImportRequest, LowerError, Manifest, ReferenceRange, Resolution, SourceSet,
    Token, TokenKind, WorkspaceError,
};
use codeint_model::token_text_hash;

pub struct Plain;

impl Guest for Plain {
    fn extensions() -> Vec<String> {
        vec!["txt".to_owned()]
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        let source_lines = u32::try_from(text.lines().count()).map_err(|_| LowerError::TooLarge)?;
        let source_set = if path.starts_with("drafts/") {
            SourceSet::Test
        } else {
            SourceSet::Production
        };
        let tokens = text
            .lines()
            .zip(1..)
            .flat_map(|(line, number)| {
                line.split_whitespace().map(move |word| Token {
                    kind: TokenKind::Other,
                    text_hash: token_text_hash(word),
                    line: number,
                })
            })
            .collect();
        Ok(FileModel {
            source_set,
            source_lines,
            functions: Vec::new(),
            tokens,
            flagged_lines: Vec::new(),
            runtime_imports: Vec::new(),
        })
    }

    fn manifest_paths(repository_paths: Vec<String>) -> Vec<String> {
        repository_paths
            .into_iter()
            .filter(|path| path.ends_with("/plain.toml") || path == "plain.toml")
            .collect()
    }

    fn workspace_packages(manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        Ok(Vec::new())
    }

    fn resolve_imports(
        _repository_paths: Vec<String>,
        _manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
    ) -> Vec<Resolution> {
        requests.iter().map(|_| Resolution::External).collect()
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        vec![ReferenceRange {
            metric: "erosion".to_owned(),
            low: 0.12,
            high: 0.56,
            source: "Plain text panel of 40 repositories, Table 3".to_owned(),
        }]
    }
}
```

- The interface is defined once, in [wit/language.wit](../../wit/language.wit), as the versioned package `codeint:plugin@0.1.0`.
- `codeint_model::language` holds the Rust types that wit-bindgen generates from that file.
  Never write a second Rust copy of a WIT type.
- A built-in Language plugin implements the generated `Guest` trait, so it compiles against the same interface as an external WebAssembly Plugin.
- `extensions` returns each file extension without its leading dot.
- `lower` receives a repository-relative path with forward slashes and the whole file as text.
- Return `LowerError::UnsupportedFile` for a file that no rule of the Language plugin covers.
- Return `LowerError::Unparsable` with the parser's message, and its line and column when the parser gives them, for a file that does not parse.
- Return `LowerError::DeclarationOnly` for a file that only declares types, such as a TypeScript `.d.ts`, `.d.mts`, or `.d.cts` file.
  Which files are declaration-only is a rule of the Language plugin, never of the core.
- `workspace_packages` receives each file that `manifest_paths` chose, as a `Manifest` with its repository-relative path and its text.
  Ignore the manifests that do not describe workspace packages, such as a `tsconfig.json` file.
  Return the root folder of each workspace package, never the repository root.
  Return an empty list when the language has no workspaces.
  An empty list means the repository is one package.
- Return a `WorkspaceError` that names the manifest and what is wrong with it when a manifest that decides the packages is invalid.
  Which manifest decides the packages, and how its patterns match folders, is a rule of the Language plugin, never of the core.
- Set `source_set` of each `FileModel` with the rules of the Language plugin, such as the TypeScript `*.test.ts`, `*.spec.ts`, and `__tests__` rules.
  `SourceSet` is the closed set of four Source sets: Production, Test, Generated, and Vendored.
  Return Production for a file that no rule claims.
  Which files are Test is a rule of the Language plugin, never of the core.
  Which files are Vendored is a rule of the core, because it is the same in every language.
  A generated marker comment is a Generated rule of the Language plugin, because only the plugin knows what a comment is in its language.
  Build output folders and very long lines are Generated rules of the core, because they read only the path and the text.
- `reference_ranges` returns the Reference ranges measured in the plugin's language, at most one per Metric, each with a `source` label that names where it was published.
  `metric` is the Metric's key in the report: `erosion`, `verbosity`, `duplication`, or `import_cycles`.
  `low` and `high` are ratios with `0 <= low <= high <= 1`.
  Return no entry for a Metric that has no published Reference range in this language; never borrow a range measured in another language.
- `tokens` lists every token of the file in source order, each with its kind, the hash of its text, and the 1-based line on which it starts.
  Leave out whitespace and comments; the core finds Clone groups from this stream.
- `kind` says whether a token is an identifier, a literal, or another token.
  Which tokens are identifiers and literals is a rule of the Language plugin, never of the core: a TypeScript keyword such as `type` is an identifier where the code uses it as a name.
- `text_hash` is the 64-bit FNV-1a hash of the token's text as written; `codeint_model::token_text_hash` computes it.
  An external WebAssembly Plugin computes the same hash itself.
- `flagged_lines` lists each Flagged line as `FlaggedLine { line, rule }`, with `line` counted from 1 and `rule` the stable identifier of the Flagged-line rule, such as `else-after-return`.
  A line that matches two rules appears once per rule.
  The Flagged-line rules live in the Language plugin; the core never inspects code to find them.
- `runtime_imports` lists the specifier of each import that runs when the file runs, in source order; leave out type-only imports.
- `manifest_paths` receives every file path of the repository snapshot and returns the paths of the files that finding workspace packages and resolving imports need to read, such as each `package.json` and `tsconfig.json` file.
  Name only paths from `repository_paths`; which files matter is a rule of the Language plugin, never of the core.
  It is the one way a Language plugin asks the host for files, so the host reads each file once.
- `resolve_imports` receives every file path of the repository snapshot and each file that `manifest_paths` chose as a `Manifest` with its text, because a Plugin cannot read the file system.
  Return one `Resolution` per request, in request order: `Resolved` with a path from `repository_paths`, `External` for a package or built-in module, or `Unresolved` with the reason.

## Lower files in the core

```rust
use codeint_model::{LanguagePlugin, RepoPath};

let plugins: [&dyn LanguagePlugin; 1] = [&Plain];
let path = RepoPath::new("notes/todo.txt")?;
let model = plugins[0].lower(&path, "first\nsecond\n")?;
assert_eq!(model.source_lines, 2);
```

- The core holds Language plugins as `&dyn LanguagePlugin`.
  Every `Guest` is a `LanguagePlugin`; the WebAssembly Plugin host implements `LanguagePlugin` itself.
- The core picks the Language plugin of a file from the extensions that the plugins declare, never from its own list.
- Before lowering, call `choose_manifests(plugin, repository_paths)` once per Language plugin and read each path it returns, once, as `Manifest { path, text }`.
  Pass those manifests to `workspace_packages`, and after lowering to `resolve_runtime_imports(plugin, repository_paths, manifests, &importers)` with the files the plugin lowered.
  `choose_manifests` fails with `ResolveError::UnknownManifest` when the plugin names a path outside the snapshot.
  `resolve_runtime_imports` returns one `RuntimeImport` per runtime import for `CodeModel::runtime_imports`, and a `ResolveError` when the plugin answers the wrong number of requests or names a file outside the snapshot.
- `codeint_model::LowerError` is the core's error; it converts from the WIT `lower-error` and names the failure in its message.
- A `LowerError` never stops the run.
  Push a `LowerError::DeclarationOnly` file onto `CodeModel::declaration_only`, and every other failure onto `CodeModel::skipped` as a `SkippedFile` whose `reason` is the error's message.
  A skipped or declaration-only file adds no source lines and no file to the Metrics.
- Push a file that is not valid UTF-8 onto `CodeModel::skipped` without lowering it, with a `reason` that starts with `not valid UTF-8`.
- Hand the text to `lower` with its line endings as written; a CRLF line ending counts like LF in every line a Language plugin reports.
- Pass each Git submodule of the working tree to `Report::summarize`, beside the Code models; no Language plugin measures a submodule's files, so a submodule belongs to no Code model.

## Find workspace packages in the core

```rust
use codeint_model::{CodeModel, Manifest, Workspace, choose_manifests};

let manifests: Vec<Manifest> = choose_manifests(plugins[0], tree.files())?
    .into_iter()
    .map(|path| Manifest {
        text: tree_text_of(&path),
        path: path.to_string(),
    })
    .collect();
let model = CodeModel {
    workspace: Workspace::new(plugins[0].workspace_packages(manifests.clone())?),
    ..CodeModel::default()
};
for package in model.workspace.packages() {
    println!("{package}");
}
let package = model.workspace.package_of(&RepoPath::new("packages/app/src/main.ts")?);
```

- The host reads each file that `choose_manifests` names for a Language plugin and passes them to that plugin; the plugin reads no files itself.
  The same manifests go to `workspace_packages` and to `resolve_runtime_imports`, so the host reads each file once.
- The host decodes a manifest that is not valid UTF-8 lossily, so its folder can still be a package.
- A `WorkspaceError` stops the run with exit code 2, because the package of every file depends on it.
- `Workspace::packages` yields `Package::Root` first, then each member package by root folder.
  `Package::Root` holds every file outside all member packages, and displays as `.`.
- `Workspace::package_of` gives the deepest member package whose root folder holds the file, or `Package::Root`.
- A `Workspace` without members is one package; `has_members` tells the report whether to list packages.

## Settle the Source set of a lowered file

```rust
use codeint_model::settle_source_set;

let mut model = plugins[0].lower(&path, text)?;
model.source_set = settle_source_set(&path, text, model.source_set);
```

- Call `settle_source_set` on every file that a Language plugin lowers, before the file joins the `CodeModel`, with the same text the plugin lowered.
- A file with a folder named exactly `vendor` or `third_party`, at any depth, is Vendored, whatever Source set the Language plugin names.
  Vendored wins over every Generated rule, because a vendored `dist` folder is third-party code first.
- Otherwise a file with a folder named exactly `dist`, `build`, `out`, or `coverage`, at any depth, is Generated.
- Otherwise a file whose non-blank lines average more than 250 characters is Generated, such as a minified bundle.
  Line length counts characters, not bytes, and a CRLF line ending counts like LF.
- The file name never counts: `src/vendor.ts` and `src/dist.ts` keep the Source set that the Language plugin names.
- Every other file keeps the Source set that the Language plugin names.
- Source set overrides from `codeint.json` apply to the result of `settle_source_set`, at its one call site, and win over every rule; [the configuration HOW-TO](../config/HOW-TO.md#apply-source-set-overrides) shows the call.

## Split the Code model by Source set

```rust
use codeint_model::{CodeModel, SourceSet};

let source_sets = model.into_source_sets();
let test_model: &CodeModel = source_sets.get(SourceSet::Test);
for (source_set, files) in source_sets.iter() {
    println!("{source_set:?} has {} files", files.files.len());
}
```

- `into_source_sets` moves each file into the `CodeModel` of its Source set.
- It moves each `RuntimeImport` into the `CodeModel` of its importer's Source set, so an import of a file in another Source set adds no edge there.
- Every Source set gets a `CodeModel`, empty when no file belongs to it.
- Every Source set's `CodeModel` keeps the whole `workspace`, so each Source set splits into the same workspace packages.
- `iter` yields the Source sets in the order Production, Test, Generated, Vendored.
- A skipped or declaration-only file belongs to no Source set.
  Read `skipped` and `declaration_only` before the split; the split leaves them out.

## Show Reference ranges only for the language they were measured in

```rust
use codeint_model::LanguagePlugin;

let ranges = plugins[0].reference_ranges()?;
if let Some(erosion) = ranges.for_metric("erosion") {
    println!("{} to {} ({})", erosion.low, erosion.high, erosion.source);
}
```

- `LanguagePlugin::reference_ranges` returns `ReferenceRanges`, which looks a range up by Metric key.
  A new Metric joins by calling `for_metric` with its own key; nothing else changes.
- It fails with `InvalidReferenceRange` when the plugin supplies two ranges for one Metric, or a range that does not run from a lower to a higher ratio between 0 and 1.
  That failure stops the run with exit code 2.
- Show a plugin's ranges only next to values that this plugin alone measured.
  When the measured files come from several Language plugins, show no Reference range.
- Keep one `CodeModel` per Language plugin and hand each to `codeint_report::Report::summarize` as a `MeasuredLanguage` with the plugin's name and ranges.
  The report applies this rule: the total shows a plugin's ranges only when that plugin alone measured files, and each per-language section shows its own plugin's ranges.

## Merge the Code models of several Language plugins

```rust
use codeint_model::CodeModel;

let total = CodeModel::merge([typescript_model, python_model]);
```

- `merge` joins the files, skipped files, declaration-only files, runtime imports, and workspace packages of every model.
- Give each Language plugin's `CodeModel` the whole `workspace` that every plugin found, so each per-language section lists the same workspace packages as the total.
- It orders files, skipped files, and declaration-only files by path, so the total does not depend on the order of the plugins.
