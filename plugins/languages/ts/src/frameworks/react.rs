use codeint_model::language::ReferenceRange;

use super::{
    PackageContext, ProfileDefinition, empty_paths, extension_reference_ranges,
    no_fallback_imports, no_virtual_modules,
};

const REACT_REFERENCE_SOURCE: &str =
    "codeint stratified panel of 50 React repositories, ADR 0035, codeint 0.8.0";

const MAIN_ENTRIES: [&str; 4] = ["src/main.tsx", "src/main.jsx", "src/main.ts", "src/main.js"];
const INDEX_ENTRIES: [&str; 4] = [
    "src/index.tsx",
    "src/index.jsx",
    "src/index.ts",
    "src/index.js",
];
const JSX_ENTRIES: [&str; 4] = [
    "src/main.tsx",
    "src/main.jsx",
    "src/index.tsx",
    "src/index.jsx",
];
const HTML_ENTRIES: [&str; 2] = ["index.html", "public/index.html"];

pub(super) const PROFILE: ProfileDefinition = ProfileDefinition {
    name: "react",
    parents: &[],
    detect,
    entry_points,
    generated_paths: empty_paths,
    reference_ranges,
    fallback_import: no_fallback_imports,
    is_virtual_module: no_virtual_modules,
};

fn has_renderer(ctx: &PackageContext<'_>) -> bool {
    ctx.has_runtime_dep("react-dom") || ctx.has_runtime_dep("react-native")
}

fn is_react_app(ctx: &PackageContext<'_>) -> bool {
    !ctx.is_peer_only_dep("react")
        && (has_renderer(ctx) || ctx.has_any_file(&HTML_ENTRIES) || ctx.has_any_file(&MAIN_ENTRIES))
}

fn detect(ctx: &PackageContext<'_>) -> bool {
    if ctx.has_runtime_dep("react") {
        return ctx.has_any_extension(&["jsx", "tsx"])
            || has_renderer(ctx)
            || ctx.has_any_file(&JSX_ENTRIES);
    }
    !ctx.is_peer_only_dep("react")
        && ctx.has_any_file(&HTML_ENTRIES)
        && ctx.has_any_file(&MAIN_ENTRIES)
        && ctx.has_any_extension(&["jsx", "tsx"])
}

fn entry_points(ctx: &PackageContext<'_>) -> Vec<String> {
    if !is_react_app(ctx) {
        return Vec::new();
    }
    ctx.matching_files(|rel| MAIN_ENTRIES.contains(&rel) || INDEX_ENTRIES.contains(&rel))
}

fn reference_ranges(ctx: &PackageContext<'_>) -> Option<Vec<ReferenceRange>> {
    extension_reference_ranges(
        ctx,
        &["jsx", "tsx"],
        REACT_REFERENCE_SOURCE,
        [
            ("erosion", 1, 0.054, 0.566),
            ("cognitive_complexity", 1, 0.038, 0.399),
            ("verbosity", 2, 0.000, 0.001),
            ("duplication", 1, 0.026, 0.315),
            ("import_cycles", 1, 0.000, 0.202),
        ],
    )
}
