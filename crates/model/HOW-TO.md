# How to use the Code model and the Language plugin interface

## Write a built-in Language plugin

```rust
use codeint_model::language::{FileModel, Guest, LowerError, Manifest, WorkspaceError};

pub struct Plain;

impl Guest for Plain {
    fn extensions() -> Vec<String> {
        vec!["txt".to_owned()]
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        let source_lines = u32::try_from(text.lines().count()).map_err(|_| LowerError::TooLarge)?;
        Ok(FileModel {
            source_lines,
            functions: Vec::new(),
        })
    }

    fn workspace_manifests() -> Vec<String> {
        Vec::new()
    }

    fn workspace_packages(manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        Ok(Vec::new())
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
- `workspace_manifests` returns the file names, such as `package.json`, that describe workspace packages in the language's ecosystem.
  Return an empty list when the language has no workspaces.
- `workspace_packages` receives every file of the repository, at any depth, whose file name `workspace_manifests` lists, as a `Manifest` with its repository-relative path and its text.
  Return the root folder of each workspace package, never the repository root.
  An empty list means the repository is one package.
- Return a `WorkspaceError` that names the manifest and what is wrong with it when a manifest that decides the packages is invalid.
  Which manifest decides the packages, and how its patterns match folders, is a rule of the Language plugin, never of the core.

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
- `codeint_model::LowerError` is the core's error; it converts from the WIT `lower-error` and names the failure in its message.
- A `LowerError` never stops the run.
  Push a `LowerError::DeclarationOnly` file onto `CodeModel::declaration_only`, and every other failure onto `CodeModel::skipped` as a `SkippedFile` whose `reason` is the error's message.
  A skipped or declaration-only file adds no source lines and no file to the Metrics.

## Find workspace packages in the core

```rust
use codeint_model::{CodeModel, Manifest, Workspace};

let manifests = vec![Manifest {
    path: "package.json".to_owned(),
    text: tree_text_of("package.json"),
}];
let model = CodeModel {
    workspace: Workspace::new(plugins[0].workspace_packages(manifests)?),
    ..CodeModel::default()
};
for package in model.workspace.packages() {
    println!("{package}");
}
let package = model.workspace.package_of(&RepoPath::new("packages/app/src/main.ts")?);
```

- The host reads every file whose name a Language plugin lists in `workspace_manifests` and passes them to that plugin; the plugin reads no files itself.
- A `WorkspaceError` stops the run with exit code 2, because the package of every file depends on it.
- `Workspace::packages` yields `Package::Root` first, then each member package by root folder.
  `Package::Root` holds every file outside all member packages, and displays as `.`.
- `Workspace::package_of` gives the deepest member package whose root folder holds the file, or `Package::Root`.
- A `Workspace` without members is one package; `has_members` tells the report whether to list packages.
