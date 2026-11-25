use super::{
    PackageContext, ProfileDefinition, matches_workspace_subpath, no_fallback_imports,
    no_reference_ranges, no_virtual_modules,
};

const ROOT_ENTRIES: [&str; 3] = ["nest-cli.json", "src/main.ts", "src/app.module.ts"];
const APP_MARKERS: [&str; 2] = ["nest-cli.json", "src/main.ts"];

pub(super) const PROFILE: ProfileDefinition = ProfileDefinition {
    name: "nestjs",
    parents: &[],
    detect,
    entry_points,
    generated_paths,
    reference_ranges: no_reference_ranges,
    fallback_import: no_fallback_imports,
    is_virtual_module: no_virtual_modules,
};

fn is_peer_only_nest(ctx: &PackageContext<'_>) -> bool {
    ctx.is_peer_only_dep("@nestjs/core")
        || (ctx.is_peer_only_dep("@nestjs/common") && !ctx.has_runtime_dep("@nestjs/core"))
}

fn detect(ctx: &PackageContext<'_>) -> bool {
    if is_peer_only_nest(ctx) {
        return false;
    }
    if ctx.has_runtime_dep("@nestjs/core") {
        return ctx.has_any_file(&APP_MARKERS);
    }
    ctx.has_file("nest-cli.json") && ctx.has_any_extension(&["ts"])
}

fn entry_points(ctx: &PackageContext<'_>) -> Vec<String> {
    if is_peer_only_nest(ctx) {
        return Vec::new();
    }
    ctx.matching_files(|rel| {
        ROOT_ENTRIES.contains(&rel)
            || matches_workspace_subpath(rel, "apps", &["main.ts"])
            || matches_workspace_subpath(rel, "libs", &["index.ts"])
    })
}

fn generated_paths(ctx: &PackageContext<'_>) -> Vec<String> {
    ctx.matching_files(|rel| rel.starts_with("dist/"))
}
