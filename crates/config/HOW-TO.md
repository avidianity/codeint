# How to read the configuration

## Read codeint.json

```rust
use codeint_config::read_configuration;

let tree = WorkingTree::discover(&folder)?;
let configuration = read_configuration(tree.root())?;
```

- `read_configuration` reads `codeint.json` at the root of the repository, so pass the root of the working tree, never the folder the user named.
- It returns `Configuration::default()` when the file is absent; codeint runs with defaults.
- Every other failure is a `ConfigurationFileError`, which stops the run with exit code 2.
  Its message names the key with its path, such as `source_sets.test[1]`, and the expected JSON type.
- Only this crate reads `codeint.json`; every other crate receives the typed `Configuration`.

## Apply Source set overrides

```rust
use codeint_model::settle_source_set;

let mut model = plugins[0].lower(&path, text)?;
model.source_set = configuration
    .source_set_overrides
    .source_set(&path, settle_source_set(&path, text, model.source_set));
```

- Apply the overrides once per lowered file, last, on the Source set that the Language plugin and the core rules settled.
  An override wins over every rule.
- `source_set` returns the settled Source set unchanged when no override glob matches the path.
- Globs match the repository-relative path with forward slashes: `*` matches within one folder, `**` matches across folders.
- When globs of several Source sets match one file, the first of Vendored, Generated, Test, and Production wins.

## Add a setting

- Add a field to `ConfigurationFile` in `src/codeint_json.rs`, with `#[serde(default)]` and a `#[schemars(description = ...)]` that tells the user what it does.
- Convert it into a typed field of `Configuration`; never hand the raw JSON to another crate.
- Run `cargo test -p codeint-config`.
  The schema test rewrites `schema/codeint.schema.json` when it drifts from the types and fails once; review and commit the rewritten file.
- The shape check reads the generated schema to name the bad key and the expected type.
  When a new type makes schemars write a keyword the shape check does not understand, `the_shape_check_understands_every_keyword_of_the_configuration_schema` fails; teach `src/codeint_json/shape.rs` that keyword first.
