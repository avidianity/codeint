# How to use the Code model and the Language plugin interface

## Write a built-in Language plugin

```rust
use codeint_model::language::{FileModel, Guest, LowerError, Token, TokenKind};
use codeint_model::token_text_hash;

pub struct Plain;

impl Guest for Plain {
    fn extensions() -> Vec<String> {
        vec!["txt".to_owned()]
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        let source_lines = u32::try_from(text.lines().count()).map_err(|_| LowerError::TooLarge)?;
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
- `tokens` lists every token of the file in source order, each with its kind, the hash of its text, and the 1-based line on which it starts.
  Leave out whitespace and comments; the core finds Clone groups from this stream.
- `kind` says whether a token is an identifier, a literal, or another token.
  Which tokens are identifiers and literals is a rule of the Language plugin, never of the core: a TypeScript keyword such as `type` is an identifier where the code uses it as a name.
- `text_hash` is the 64-bit FNV-1a hash of the token's text as written; `codeint_model::token_text_hash` computes it.
  An external WebAssembly Plugin computes the same hash itself.

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
