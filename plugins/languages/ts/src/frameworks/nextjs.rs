use super::{
    PackageContext, ProfileDefinition, has_any_prefix, has_code_extension, is_declaration_file,
    multi_prefix_alias_fallback, no_reference_ranges, no_virtual_modules, split_file_name,
};

const NEXT_CONFIG_FILES: [&str; 4] = [
    "next.config.js",
    "next.config.mjs",
    "next.config.cjs",
    "next.config.ts",
];
const APP_COMPONENT_STEMS: [&str; 14] = [
    "page",
    "layout",
    "template",
    "loading",
    "error",
    "global-error",
    "not-found",
    "forbidden",
    "unauthorized",
    "default",
    "icon",
    "apple-icon",
    "opengraph-image",
    "twitter-image",
];
const APP_DATA_STEMS: [&str; 4] = ["route", "sitemap", "robots", "manifest"];
const ROOT_SCRIPT_STEMS: [&str; 4] = [
    "middleware",
    "proxy",
    "instrumentation",
    "instrumentation-client",
];
const JSX_EXTS: [&str; 4] = ["js", "jsx", "ts", "tsx"];
const SCRIPT_EXTS: [&str; 2] = ["js", "ts"];

pub(super) const PROFILE: ProfileDefinition = ProfileDefinition {
    name: "nextjs",
    parents: &["react"],
    detect,
    entry_points,
    generated_paths,
    reference_ranges: no_reference_ranges,
    fallback_import,
    is_virtual_module: no_virtual_modules,
};

fn strip_src_prefix(rel: &str) -> &str {
    rel.strip_prefix("src/").unwrap_or(rel)
}

fn is_app_router_entry(trimmed: &str) -> bool {
    if !trimmed.starts_with("app/") || is_declaration_file(trimmed) {
        return false;
    }
    let Some((stem, ext)) = split_file_name(trimmed) else {
        return false;
    };
    (APP_COMPONENT_STEMS.contains(&stem) && JSX_EXTS.contains(&ext))
        || (APP_DATA_STEMS.contains(&stem) && SCRIPT_EXTS.contains(&ext))
}

fn is_pages_router_entry(trimmed: &str) -> bool {
    trimmed.starts_with("pages/") && has_code_extension(trimmed, &JSX_EXTS)
}

fn is_root_convention(trimmed: &str) -> bool {
    if trimmed.contains('/') || is_declaration_file(trimmed) {
        return false;
    }
    let Some((stem, ext)) = split_file_name(trimmed) else {
        return false;
    };
    (ROOT_SCRIPT_STEMS.contains(&stem) && SCRIPT_EXTS.contains(&ext))
        || (stem == "mdx-components" && JSX_EXTS.contains(&ext))
}

fn has_router_marker(ctx: &PackageContext<'_>) -> bool {
    ctx.has_matching_file(|rel| {
        let trimmed = strip_src_prefix(rel);
        is_app_router_entry(trimmed) || is_pages_router_entry(trimmed)
    })
}

fn detect(ctx: &PackageContext<'_>) -> bool {
    if ctx.is_peer_only_dep("next") {
        return false;
    }
    if ctx.has_any_file(&NEXT_CONFIG_FILES) {
        return true;
    }
    ctx.has_runtime_dep("next") && has_router_marker(ctx)
}

fn entry_points(ctx: &PackageContext<'_>) -> Vec<String> {
    if ctx.is_peer_only_dep("next") {
        return Vec::new();
    }
    ctx.matching_files(|rel| {
        if NEXT_CONFIG_FILES.contains(&rel) {
            return true;
        }
        let trimmed = strip_src_prefix(rel);
        is_root_convention(trimmed)
            || is_app_router_entry(trimmed)
            || is_pages_router_entry(trimmed)
    })
}

fn generated_paths(ctx: &PackageContext<'_>) -> Vec<String> {
    ctx.matching_files(|rel| rel == "next-env.d.ts" || has_any_prefix(rel, &[".next/", "out/"]))
}

fn fallback_import(specifier: &str) -> Vec<String> {
    multi_prefix_alias_fallback(specifier, &["@/", "~/"], &["./src/", "./"])
}
