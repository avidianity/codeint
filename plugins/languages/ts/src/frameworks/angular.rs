use super::{
    PackageContext, ProfileDefinition, has_any_prefix, matches_workspace_subpath,
    no_fallback_imports, no_reference_ranges, no_virtual_modules,
};

const ROOT_ENTRIES: [&str; 7] = [
    "angular.json",
    "src/main.ts",
    "src/main.server.ts",
    "src/server.ts",
    "src/app/app.config.ts",
    "src/app/app.routes.ts",
    "src/app/app.module.ts",
];
const PROJECT_ENTRIES: [&str; 3] = ["main.ts", "main.server.ts", "public-api.ts"];
const APP_MARKERS: [&str; 3] = ["angular.json", "project.json", "src/main.ts"];

pub(super) const PROFILE: ProfileDefinition = ProfileDefinition {
    name: "angular",
    parents: &[],
    detect,
    entry_points,
    generated_paths,
    reference_ranges: no_reference_ranges,
    fallback_import: no_fallback_imports,
    is_virtual_module: no_virtual_modules,
};

fn detect(ctx: &PackageContext<'_>) -> bool {
    if ctx.is_peer_only_dep("@angular/core") {
        return false;
    }
    if ctx.has_runtime_dep("@angular/core") {
        return ctx.has_any_file(&APP_MARKERS);
    }
    ctx.has_file("angular.json") && ctx.has_any_extension(&["ts"])
}

fn entry_points(ctx: &PackageContext<'_>) -> Vec<String> {
    if ctx.is_peer_only_dep("@angular/core") {
        return Vec::new();
    }
    ctx.matching_files(|rel| {
        ROOT_ENTRIES.contains(&rel) || matches_workspace_subpath(rel, "projects", &PROJECT_ENTRIES)
    })
}

fn generated_paths(ctx: &PackageContext<'_>) -> Vec<String> {
    ctx.matching_files(|rel| has_any_prefix(rel, &[".angular/", "dist/"]))
}
