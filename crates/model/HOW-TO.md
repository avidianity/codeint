# How to use the Code model and the Language plugin interface

## Write a built-in Language plugin

```rust
use codeint_model::language::{
    DeclaredDep, FileModel, FrameworkProfileResult, FrameworkProfileSelection, ImportRequest,
    Manifest, ReferenceRange, Resolution, SourceSet, Token, TokenKind,
};
use codeint_model::scan::find_manifests;
use codeint_model::{
    InvalidReferenceRange, LanguagePlugin, LanguagePluginError, LowerError, ReferenceRanges,
    RepoPath, WorkspaceError, empty_framework_profile_result, token_text_hash,
    validate_workspace_packages,
};

pub struct Plain;

impl LanguagePlugin for Plain {
    fn extensions(&self) -> Vec<String> {
        vec!["txt".to_owned()]
    }

    fn lower(&self, path: &RepoPath, text: &str) -> Result<FileModel, LowerError> {
        let source_lines = u32::try_from(text.lines().count()).map_err(|_| LowerError::TooLarge)?;
        let source_set = if path.as_str().starts_with("drafts/") {
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
            hazard_lines: Vec::new(),
            runtime_imports: Vec::new(),
            program_start: false,
        })
    }

    fn manifest_paths(
        &self,
        repository_paths: &[RepoPath],
    ) -> Result<Vec<String>, LanguagePluginError> {
        Ok(find_manifests(repository_paths, "plain.toml"))
    }

    fn workspace_packages(
        &self,
        _manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, WorkspaceError> {
        validate_workspace_packages(Vec::new())
    }

    fn resolve_imports(
        &self,
        _repository_paths: &[RepoPath],
        _manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
        _active_profiles: &[String],
    ) -> Result<Vec<Resolution>, LanguagePluginError> {
        Ok(requests
            .iter()
            .map(|request| Resolution::External(request.specifier.clone()))
            .collect())
    }

    fn declared_dependencies(
        &self,
        _manifests: Vec<Manifest>,
    ) -> Result<Vec<DeclaredDep>, LanguagePluginError> {
        Ok(Vec::new())
    }

    fn entry_points(
        &self,
        _repository_paths: &[RepoPath],
        _manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, LanguagePluginError> {
        Ok(Vec::new())
    }

    fn reference_ranges(&self) -> Result<ReferenceRanges, LanguagePluginError> {
        ReferenceRanges::new(vec![ReferenceRange {
            metric: "erosion".to_owned(),
            metric_version: 1,
            low: 0.12,
            high: 0.56,
            source: "Plain text panel of 40 repositories, Table 3".to_owned(),
        }])
        .map_err(LanguagePluginError::InvalidReferenceRange)
    }

    fn known_framework_profiles(&self) -> Vec<String> {
        Vec::new()
    }

    fn resolve_framework_profiles(
        &self,
        _package_root: &str,
        _files: &[RepoPath],
        _manifests: Vec<Manifest>,
        _selection: FrameworkProfileSelection,
    ) -> Result<FrameworkProfileResult, LanguagePluginError> {
        Ok(empty_framework_profile_result())
    }

    fn has_hazard_rules(&self) -> bool {
        false
    }

    fn hazard_rules(&self) -> Vec<String> {
        Vec::new()
    }

    fn has_imports(&self) -> bool {
        true
    }
}
```

- The interface is defined once, in [wit/language.wit](../../wit/language.wit), as the versioned package `codeint:plugin@0.4.0`.
- `codeint_model::language` holds the Rust types that wit-bindgen generates from that file.
  Never write a second Rust copy of a WIT type.
- A built-in Language plugin implements `LanguagePlugin` directly and borrows `&RepoPath`, `&str`, and `&[RepoPath]` to avoid per-file clones.
- External WebAssembly Plugins implement the generated `Guest` trait, which bridges into `LanguagePlugin` through a blanket implementation.
- `RepoPath` implements `AsRef<str>`, so shared path helpers accept `&[RepoPath]` and `&[String]` without cloning paths.
- Call `codeint_model::scan::find_manifests<P: AsRef<str>>` to match a manifest filename across any `&[P]` slice, and `codeint_model::scan::paths_with_suffix` to collect paths ending with a suffix into a `HashSet<&str>`.
- Call `validate_workspace_packages` inside `LanguagePlugin::workspace_packages` to convert raw package root strings into validated `Vec<RepoPath>` values.
- Use `impl_builtin_language_plugin!` to wire a built-in Language plugin's `LanguagePlugin` implementation without repeating validation, framework-profile delegation, or empty default methods.
- Use `empty_framework_profile_plugin_methods!` in test stubs that implement `LanguagePlugin` directly without Framework profiles.
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
- `reference_ranges` returns the Reference ranges measured in the plugin's language, at most one per Metric, each tagged with its positive integer `metric-version` and a `source` label that names where it was published.
  `metric` is the Metric's key in the report: `erosion`, `verbosity`, `duplication`, or `import_cycles`.
  `low` and `high` are ratios with `0 <= low <= high <= 1`.
  Return no entry for a Metric that has no published Reference range in this language; never borrow a range measured in another language.
- `known_framework_profiles` returns the stable names of Framework profiles this Language plugin defines, and `resolve_framework_profiles` resolves active profiles, convention entry points, generated paths, framework Reference ranges, and an optional Reference range note for one Workspace package.
- `has_hazard_rules` states whether this Language plugin defines Hazard line rules, and `hazard_rules` returns their stable names (empty when `has_hazard_rules` is `false`).
  The five built-in Language plugins return `false` and an empty list.
- `has_imports` states whether the language has runtime imports, manifests, and entry points.
  When `has_imports` returns `false`, codeint never calls `manifest_paths`, `workspace_packages`, `resolve_imports`, `declared_dependencies`, or `entry_points`.
  The five built-in Language plugins return `true`.
- Each `Function` has `line` and `end-line`, the lines where it starts and ends, counted from 1; `end-line` equals `line` for a function on one line.
  The core reads the tokens from `line` to `end-line` as the content of the function's Finding identity.
  Each `Function` also carries `coverage: Option<FunctionCoverage>` with `listed_lines` and `executed_lines`.
  A Language plugin always sets `coverage` to `None`; codeint fills coverage facts from Coverage reports before Metrics run.
- `tokens` lists every token of the file in source order, each with its kind, the hash of its text, and the 1-based line on which it starts.
  Leave out whitespace and comments; the core finds Clone groups from this stream.
- `kind` says whether a token is an identifier, a literal, another token, or a token of an import or export declaration.
  Which tokens are identifiers and literals is a rule of the Language plugin, never of the core: a TypeScript keyword such as `type` is an identifier where the code uses it as a name.
  Give `TokenKind::ImportOrExport` to every token of a declaration that only imports or exports names, so clone detection skips it.
  Which declarations count is a rule of the Language plugin, never of the core.
- `text_hash` is the 64-bit FNV-1a hash of the token's text as written; `codeint_model::token_text_hash` computes it.
  `codeint_model::StableHasher` is the same hash over any bytes, and writes each `u64` as little-endian bytes, for values that must match on every platform.
  An external WebAssembly Plugin computes the same hash itself.
- `flagged_lines` lists each Flagged line as `FlaggedLine { line, rule }`, with `line` counted from 1 and `rule` the stable identifier of the Flagged-line rule, such as `else-after-return`.
  A line that matches two rules appears once per rule.
  The Flagged-line rules live in the Language plugin; the core never inspects code to find them.
- `hazard_lines` lists each Hazard line as `HazardLine { line, rule }`, with `line` counted from 1 and `rule` a stable rule name listed by `hazard_rules`.
  A `HazardLine` never holds source text.
  A line that matches two rules appears once per rule.
  Return an empty list when `has_hazard_rules` returns `false`; the five built-in Language plugins always return an empty list.
- `runtime_imports` lists the specifier of each import that runs when the file runs, in source order; leave out type-only imports.
- `program_start` is `false` unless the source text marks the file as a program start, such as a Go `package main` file.
  The five built-in Language plugins always set `program_start` to `false`.
- `manifest_paths` receives every file path of the repository snapshot and returns the paths of the files that finding workspace packages and resolving imports need to read, such as each `package.json` and `tsconfig.json` file.
  Name only paths from `repository_paths`; which files matter is a rule of the Language plugin, never of the core.
  It is the one way a Language plugin asks the host for files, so the host reads each file once.
- `resolve_imports` receives every file path of the repository snapshot and each file that `manifest_paths` chose as a `Manifest` with its text, because a Plugin cannot read the file system.
  Return one `Resolution` per request, in request order: `Resolved` with a non-empty list of paths from `repository_paths`, `External` with the canonical third-party package name, `Builtin` for a standard-library or runtime built-in module, or `Unresolved` with the reason.
- `declared_dependencies` extracts external dependencies declared in `manifests`, each with its `package-root`, normalized `name`, and `kind` (`Runtime`, `Dev`, or `Optional`).
- `entry_points` discovers standard entry-point source files from `repository_paths` and `manifests`, naming only paths from `repository_paths`.

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
  Pass those manifests to `workspace_packages`, and after lowering to `resolve_runtime_imports(plugin, repository_paths, manifests, &importers, active_profiles)` with the files the plugin lowered and the active Framework profile names.
  `choose_manifests` fails with `ResolveError::UnknownManifest` when the plugin names a path outside the snapshot.
  `resolve_runtime_imports` returns one `RuntimeImport` per runtime import for `CodeModel::runtime_imports`, and a `ResolveError` when the plugin answers the wrong number of requests, returns an empty `Resolved` target list, returns an empty, blank/whitespace-only, invisible, or control-character `External` package name, or names a file outside the snapshot.
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
- A `WorkspaceError::Manifest` that names one passed manifest makes the host skip that manifest with a warning and call `workspace_packages` again without it.
  Any other `WorkspaceError`, including a plugin trap, stops the run with exit code 2, because the package of every file depends on it.
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
- Source set overrides from `codeint.jsonc` or `codeint.json` apply to the result of `settle_source_set`, at its one call site, and win over every rule; [the configuration HOW-TO](../config/HOW-TO.md#apply-source-set-overrides) shows the call.
- Built-in Language plugins share `has_generated_header_marker`, `has_generated_header_marker_with_extra`, `has_any_ascii_marker`, `contains_ascii_ignore_case`, `has_folder_named`, and `path_file_name` from `codeint_model` to classify generated header comments and directory segments without duplicating marker or path checks.

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
- Every Source set's `CodeModel` keeps the whole `workspace` and `languages`, so each Source set splits into the same workspace packages and carries the same per-language facts.
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

- `merge` joins the files, skipped files, declaration-only files, runtime imports, declared dependencies, dependency manifests, manifest-used dependencies, workspace packages, and per-language facts (`languages`) of every model.
- Give each Language plugin's `CodeModel` the whole `workspace` that every plugin found, so each per-language section lists the same workspace packages as the total.
- It orders files, skipped files, declaration-only files, and `languages` deterministically by path or language name, so the total does not depend on the order of the plugins.

## Read built-in Metric metadata from the descriptor registry

```rust
use codeint_model::{Metric, STRUCTURAL_REPORT_KEYS, is_reserved_report_key};

for metric in Metric::ALL {
    let descriptor = metric.descriptor();
    println!("{} ({}, v{}): {}", descriptor.display_name, descriptor.key, descriptor.version, descriptor.numerator_field);
}
assert_eq!(Metric::from_key("orphan_files"), Some(Metric::OrphanFiles));
assert!(is_reserved_report_key(STRUCTURAL_REPORT_KEYS[0]));
```

- `Metric` enumerates all twelve built-in Metrics, backed by one ordered table (`Metric::DESCRIPTORS`) and `Metric::ALL`.
- Each `MetricDescriptor` carries nine fields: machine key, Glossary display name, positive integer Metric version (starting at `1`), kind (`MetricKind::SoftwareEntropy` or `MetricKind::Hazards`), Source-set scope (`MetricScope`), whether `codeint diff` supports Regression checks, default Regression rule (`NewFindingsRule`), required capability (`MetricCapability`), and report numerator field name.
- Call `Metric::built_in_versions()` for the sorted map of all built-in Metric versions, or `Metric::versions_with_plugins(&metric_plugins)` to include loaded external Metric plugins.
- `STRUCTURAL_REPORT_KEYS` lists the three structural JSON report keys (`files`, `source_lines`, `packages`), and `is_reserved_report_key` checks both `Metric::BUILT_IN_KEYS` and `STRUCTURAL_REPORT_KEYS`.
- Read built-in keys, default Regression rules, scopes, capabilities, and numerator field names from `Metric` instead of keeping duplicate partial tables in downstream crates.
