use super::{
    PackageContext, ProfileDefinition, empty_paths, no_reference_ranges, prefix_alias_fallback,
};

const SVELTE_CONFIG_FILES: [&str; 4] = [
    "svelte.config.js",
    "svelte.config.mjs",
    "svelte.config.cjs",
    "svelte.config.ts",
];
const VITE_CONFIG_FILES: [&str; 3] = ["vite.config.js", "vite.config.mjs", "vite.config.ts"];

fn is_svelte_library(ctx: &PackageContext<'_>) -> bool {
    (ctx.has_top_level_field("exports") || ctx.has_top_level_field("svelte"))
        && !ctx.has_file("index.html")
}

fn detect(ctx: &PackageContext<'_>) -> bool {
    if ctx.has_runtime_or_dev_dep("@sveltejs/kit") {
        return false;
    }
    if ctx.has_runtime_or_dev_dep("svelte") {
        return true;
    }
    ctx.has_any_file(&SVELTE_CONFIG_FILES) && ctx.has_any_extension(&["svelte"])
}

fn entry_points(ctx: &PackageContext<'_>) -> Vec<String> {
    let include_main = !is_svelte_library(ctx);
    ctx.matching_files(|rel| {
        SVELTE_CONFIG_FILES.contains(&rel)
            || VITE_CONFIG_FILES.contains(&rel)
            || (include_main && matches!(rel, "src/main.js" | "src/main.ts"))
    })
}

fn fallback_import(specifier: &str) -> Vec<String> {
    if specifier == "$lib" {
        return vec!["./src/lib/index".to_owned()];
    }
    prefix_alias_fallback(specifier, "$lib/", "./src/lib/")
}

pub(super) const PROFILE: ProfileDefinition = ProfileDefinition {
    name: "svelte",
    parent: None,
    detect,
    entry_points,
    generated_paths: empty_paths,
    reference_ranges: no_reference_ranges,
    fallback_import,
};
