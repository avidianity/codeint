use super::{
    PackageContext, ProfileDefinition, has_any_prefix, has_code_extension,
    multi_prefix_alias_fallback, no_reference_ranges,
};

const NUXT_CONFIG_FILES: [&str; 4] = [
    "nuxt.config.ts",
    "nuxt.config.js",
    "nuxt.config.mjs",
    "nuxt.config.cjs",
];
const CLIENT_ROOT_FILES: [&str; 6] = [
    "app.vue",
    "error.vue",
    "app.config.ts",
    "app.config.js",
    "router.options.ts",
    "router.options.js",
];
const CLIENT_SCRIPT_DIRS: [&str; 4] = ["middleware/", "plugins/", "composables/", "utils/"];
const SERVER_AND_SHARED_DIRS: [&str; 7] = [
    "server/api/",
    "server/routes/",
    "server/middleware/",
    "server/plugins/",
    "server/utils/",
    "modules/",
    "shared/utils/",
];
const SCRIPT_EXTS: [&str; 2] = ["js", "ts"];
const PAGE_EXTS: [&str; 3] = ["vue", "js", "ts"];

pub(super) const PROFILE: ProfileDefinition = ProfileDefinition {
    name: "nuxt",
    parents: &["vue2", "vue3"],
    detect,
    entry_points,
    generated_paths,
    reference_ranges: no_reference_ranges,
    fallback_import,
    is_virtual_module,
};

fn strip_app_prefix(rel: &str) -> &str {
    rel.strip_prefix("app/").unwrap_or(rel)
}

fn is_nuxt_app_marker(rel: &str) -> bool {
    let trimmed = strip_app_prefix(rel);
    matches!(trimmed, "app.vue" | "error.vue")
        || trimmed.starts_with("pages/")
        || has_any_prefix(rel, &["server/api/", "server/routes/"])
}

fn is_client_entry(trimmed: &str) -> bool {
    CLIENT_ROOT_FILES.contains(&trimmed)
        || (trimmed.starts_with("pages/") && has_code_extension(trimmed, &PAGE_EXTS))
        || (has_any_prefix(trimmed, &["layouts/", "components/"])
            && has_code_extension(trimmed, &["vue"]))
        || (has_any_prefix(trimmed, &CLIENT_SCRIPT_DIRS)
            && has_code_extension(trimmed, &SCRIPT_EXTS))
}

fn detect(ctx: &PackageContext<'_>) -> bool {
    if ctx.is_peer_only_dep_any("nuxt") {
        return false;
    }
    if ctx.has_any_file(&NUXT_CONFIG_FILES) {
        return true;
    }
    ctx.has_runtime_or_dev_dep("nuxt") && ctx.has_matching_file(is_nuxt_app_marker)
}

fn entry_points(ctx: &PackageContext<'_>) -> Vec<String> {
    if ctx.is_peer_only_dep_any("nuxt") {
        return Vec::new();
    }
    ctx.matching_files(|rel| {
        NUXT_CONFIG_FILES.contains(&rel)
            || (has_any_prefix(rel, &SERVER_AND_SHARED_DIRS)
                && has_code_extension(rel, &SCRIPT_EXTS))
            || is_client_entry(strip_app_prefix(rel))
    })
}

fn generated_paths(ctx: &PackageContext<'_>) -> Vec<String> {
    ctx.matching_files(|rel| has_any_prefix(rel, &[".nuxt/", ".output/", "dist/"]))
}

fn fallback_import(specifier: &str) -> Vec<String> {
    if specifier.starts_with("~~/") || specifier.starts_with("@@/") {
        return multi_prefix_alias_fallback(specifier, &["~~/", "@@/"], &["./"]);
    }
    if specifier.starts_with("#shared/") {
        return multi_prefix_alias_fallback(specifier, &["#shared/"], &["./shared/"]);
    }
    multi_prefix_alias_fallback(specifier, &["~/", "@/"], &["./app/", "./"])
}

fn is_virtual_module(clean: &str) -> bool {
    matches!(clean, "#imports" | "#components" | "#app" | "#head")
        || ["#app/", "#build/"].iter().any(|prefix| {
            clean
                .strip_prefix(prefix)
                .is_some_and(|rest| !rest.is_empty())
        })
}
