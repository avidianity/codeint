# How to use the Code model and the Language plugin interface

## Write a built-in Language plugin

```rust
use codeint_model::language::{FileModel, Guest, LowerError, ReferenceRange, SourceSet, Token};

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
            flagged_lines: Vec::new(),
        })
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
- Set `source_set` of each `FileModel` with the rules of the Language plugin, such as the TypeScript `*.test.ts`, `*.spec.ts`, and `__tests__` rules.
  `SourceSet` is the closed set of four Source sets: Production, Test, Generated, and Vendored.
  Return Production for a file that no rule claims.
  Which files are Test is a rule of the Language plugin, never of the core.
  Which files are Vendored is a rule of the core, because it is the same in every language.
- `reference_ranges` returns the Reference ranges measured in the plugin's language, at most one per Metric, each with a `source` label that names where it was published.
  `metric` is the Metric's key in the report: `erosion`, `verbosity`, `duplication`, or `import_cycles`.
  `low` and `high` are ratios with `0 <= low <= high <= 1`.
  Return no entry for a Metric that has no published Reference range in this language; never borrow a range measured in another language.
- `tokens` lists every token of the file in source order, each with its text and the 1-based line on which it starts.
  Leave out whitespace and comments; the core finds Clone groups from this stream.
- `flagged_lines` lists each Flagged line as `FlaggedLine { line, rule }`, with `line` counted from 1 and `rule` the stable identifier of the Flagged-line rule, such as `else-after-return`.
  A line that matches two rules appears once per rule.
  The Flagged-line rules live in the Language plugin; the core never inspects code to find them.

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

## Settle the Source set of a lowered file

```rust
use codeint_model::settle_source_set;

let mut model = plugins[0].lower(&path, text)?;
model.source_set = settle_source_set(&path, model.source_set);
```

- Call `settle_source_set` on every file that a Language plugin lowers, before the file joins the `CodeModel`.
- A file with a folder named exactly `vendor` or `third_party`, at any depth, is Vendored, whatever Source set the Language plugin names.
- The file name never counts: `src/vendor.ts` keeps the Source set that the Language plugin names.
- Every other file keeps the Source set that the Language plugin names.
- Source set overrides from `codeint.json` apply after `settle_source_set` and win over it; [the configuration HOW-TO](../config/HOW-TO.md#apply-source-set-overrides) shows the call.

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
