# How to use the Plugin SDK (`codeint-plugin-sdk`)

## Implement a Language plugin (`wasm32-wasip2`, `no_std`, zero extra tools)

```rust
#![forbid(unsafe_code)]
#![cfg_attr(all(target_arch = "wasm32", target_os = "wasi", target_env = "p2"), no_std)]

use codeint_plugin_sdk::language::{FileModel, LanguagePlugin, LowerError, SourceSet};
use codeint_plugin_sdk::prelude::*;

struct MyLanguage;

impl LanguagePlugin for MyLanguage {
    fn extensions() -> Vec<String> {
        vec!["mylang".to_string()]
    }

    fn lower(
        path: String,
        text: String,
        _dialect: Option<String>,
    ) -> Result<FileModel, LowerError> {
        if !path.ends_with(".mylang") {
            return Err(LowerError::UnsupportedFile(format!(
                "{path} does not end with .mylang"
            )));
        }
        Ok(FileModel {
            source_lines: text.lines().filter(|line| !line.trim().is_empty()).count() as u32,
            ..FileModel::empty(SourceSet::Production)
        })
    }
}

codeint_plugin_sdk::export_language_plugin!(MyLanguage);
```

## Implement a Metric plugin

```rust
#![forbid(unsafe_code)]
#![cfg_attr(all(target_arch = "wasm32", target_os = "wasi", target_env = "p2"), no_std)]

use codeint_plugin_sdk::metric::{CodeModel, Measurement, MetricPlugin};
use codeint_plugin_sdk::prelude::*;

struct MyMetric;

impl MetricPlugin for MyMetric {
    fn key() -> String {
        "my_metric".to_string()
    }

    fn name() -> String {
        "My metric".to_string()
    }

    fn description() -> String {
        "Share of files matching a custom rule".to_string()
    }

    fn measure(model: CodeModel) -> Measurement {
        let total = model.files.len() as f64;
        Measurement::from_counts(0.0, total, Vec::new())
    }
}

codeint_plugin_sdk::export_metric_plugin!(MyMetric);
```

## Rules and Cargo features

- `codeint-plugin-sdk` wraps the `codeint:plugin@0.5.0` WIT contract (`wit/language.wit` and `wit/metric.wit`) behind safe `LanguagePlugin` and `MetricPlugin` traits with sensible defaults.
- Default features (`default-allocator`, `panic-handler`) supply a freeing `dlmalloc::GlobalDlmalloc` allocator, panic handler, `cabi_realloc`, and C `mem*` shims on `wasm32-wasip2` with zero WASI imports.
- Enable `features = ["std"]` (with `default-features = false`) only when targeting `wasm32-unknown-unknown` plus `wasm-tools component new`; enabling the SDK `std` feature on `wasm32-wasip2` fails at compile time, because WASI imports violate codeint's empty-`Linker` sandbox.
- The SDK cannot see whether your own crate uses `std`: with `default-features = false` and no `no_std` attribute, a `wasm32-wasip2` build compiles and imports `wasi:*` interfaces, and codeint rejects that component at load time.
- `codeint_plugin_sdk::fnv1a_64` and `codeint_plugin_sdk::Fnv1a64` match codeint's 64-bit FNV-1a token hash.
- `unsafe` inside `codeint-plugin-sdk` is strictly isolated to `src/runtime.rs` per [ADR 0027](../../docs/adr/0027-plugin-sdk-guest-runtime-unsafe.md).
- Native host unit tests (`cargo test`) run with full `std` because the `wasm32-wasip2` `no_std` attribute and guest shims compile out on non-WebAssembly targets.
- Plugin crates keep `#![forbid(unsafe_code)]` at their root; the export macros compile under it in both build paths.
