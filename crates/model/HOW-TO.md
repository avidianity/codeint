# How to use the Code model and the Language plugin interface

## Write a built-in Language plugin

```rust
use codeint_model::language::{FileModel, Guest, ImportRequest, LowerError, Resolution};

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
            runtime_imports: Vec::new(),
        })
    }

    fn resolve_imports(_repository_paths: Vec<String>, requests: Vec<ImportRequest>) -> Vec<Resolution> {
        requests.iter().map(|_| Resolution::External).collect()
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
- `runtime_imports` lists the specifier of each import that runs when the file runs, in source order; leave out type-only imports.
- `resolve_imports` receives every file path of the repository snapshot, because a Plugin cannot read the file system.
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
- After lowering, call `resolve_runtime_imports(plugin, repository_paths, &importers)` once per Language plugin with the files it lowered.
  It returns one `RuntimeImport` per runtime import for `CodeModel::runtime_imports`, and a `ResolveError` when the plugin answers the wrong number of requests or names a file outside the snapshot.
- `codeint_model::LowerError` is the core's error; it converts from the WIT `lower-error` and names the failure in its message.
- A `LowerError` never stops the run.
  Push a `LowerError::DeclarationOnly` file onto `CodeModel::declaration_only`, and every other failure onto `CodeModel::skipped` as a `SkippedFile` whose `reason` is the error's message.
  A skipped or declaration-only file adds no source lines and no file to the Metrics.
