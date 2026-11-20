# How to use the Code model and the Language plugin interface

## Write a built-in Language plugin

```rust
use codeint_model::language::{FileModel, Guest, LowerError, SourceSet, Token};

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
                    text: word.to_owned(),
                    line: number,
                })
            })
            .collect();
        Ok(FileModel {
            source_set,
            source_lines,
            functions: Vec::new(),
            tokens,
        })
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
- Set `source_set` of each `FileModel` with the rules of the Language plugin, such as the TypeScript `*.test.ts`, `*.spec.ts`, and `__tests__` rules.
  `SourceSet` is the closed set of four Source sets: Production, Test, Generated, and Vendored.
  Return Production for a file that no rule claims.
  Which files are Test is a rule of the Language plugin, never of the core.
- `tokens` lists every token of the file in source order, each with its text and the 1-based line on which it starts.
  Leave out whitespace and comments; the core finds Clone groups from this stream.

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
- Every Source set gets a `CodeModel`, empty when no file belongs to it.
- `iter` yields the Source sets in the order Production, Test, Generated, Vendored.
- A skipped or declaration-only file belongs to no Source set.
  Read `skipped` and `declaration_only` before the split; the split leaves them out.
