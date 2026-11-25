use super::{
    PackageContext, ProfileDefinition, has_any_prefix, has_code_extension, is_declaration_file,
    no_reference_ranges, split_file_name, svelte_lib_fallback,
};

const SVELTE_CONFIG_FILES: [&str; 4] = [
    "svelte.config.js",
    "svelte.config.mjs",
    "svelte.config.cjs",
    "svelte.config.ts",
];
const VITE_CONFIG_FILES: [&str; 4] = [
    "vite.config.js",
    "vite.config.mjs",
    "vite.config.cjs",
    "vite.config.ts",
];
const APP_WIDE_ENTRIES: [&str; 15] = [
    "src/app.html",
    "src/hooks.js",
    "src/hooks.ts",
    "src/hooks.server.js",
    "src/hooks.server.ts",
    "src/hooks.client.js",
    "src/hooks.client.ts",
    "src/instrumentation.server.js",
    "src/instrumentation.server.ts",
    "src/params.js",
    "src/params.ts",
    "src/service-worker.js",
    "src/service-worker.ts",
    "src/service-worker/index.js",
    "src/service-worker/index.ts",
];

pub(super) const PROFILE: ProfileDefinition = ProfileDefinition {
    name: "sveltekit",
    parents: &["svelte"],
    detect,
    entry_points,
    generated_paths,
    reference_ranges: no_reference_ranges,
    fallback_import: svelte_lib_fallback,
    is_virtual_module,
};

fn is_route_entry(rel: &str) -> bool {
    if !rel.starts_with("src/routes/") || is_declaration_file(rel) {
        return false;
    }
    let Some((stem, ext)) = split_file_name(rel) else {
        return false;
    };
    let base_stem = stem.split_once('@').map_or(stem, |(head, _)| head);
    (ext == "svelte" && matches!(base_stem, "+page" | "+layout" | "+error"))
        || (matches!(ext, "js" | "ts")
            && matches!(
                base_stem,
                "+page" | "+page.server" | "+layout" | "+layout.server" | "+server"
            ))
}

fn is_params_entry(rel: &str) -> bool {
    rel.strip_prefix("src/params/")
        .is_some_and(|tail| !tail.contains('/') && has_code_extension(tail, &["js", "ts"]))
}

fn detect(ctx: &PackageContext<'_>) -> bool {
    if ctx.is_peer_only_dep_any("@sveltejs/kit") {
        return false;
    }
    if ctx.has_runtime_or_dev_dep("@sveltejs/kit") {
        return ctx.has_any_file(&SVELTE_CONFIG_FILES) || ctx.has_matching_file(is_route_entry);
    }
    ctx.has_matching_file(is_route_entry)
}

fn entry_points(ctx: &PackageContext<'_>) -> Vec<String> {
    if ctx.is_peer_only_dep_any("@sveltejs/kit") {
        return Vec::new();
    }
    ctx.matching_files(|rel| {
        SVELTE_CONFIG_FILES.contains(&rel)
            || VITE_CONFIG_FILES.contains(&rel)
            || APP_WIDE_ENTRIES.contains(&rel)
            || is_route_entry(rel)
            || is_params_entry(rel)
    })
}

fn generated_paths(ctx: &PackageContext<'_>) -> Vec<String> {
    ctx.matching_files(|rel| has_any_prefix(rel, &[".svelte-kit/", "build/"]))
}

fn is_virtual_module(clean: &str) -> bool {
    matches!(clean, "$app" | "$service-worker")
        || ["$app/", "$env/"].iter().any(|prefix| {
            clean
                .strip_prefix(prefix)
                .is_some_and(|rest| !rest.is_empty())
        })
}
