use codeint_model::language::ReferenceRange;

use super::{
    PackageContext, ProfileDefinition, at_src_fallback, detect_vue_major, no_virtual_modules,
    vue_entry_points, vue_reference_ranges,
};

const VUE3_REFERENCE_SOURCE: &str =
    "codeint stratified panel of 50 Vue 3 repositories, ADR 0035, codeint 0.8.0";

const CONFIG_FILES: [&str; 3] = ["vite.config.js", "vite.config.mjs", "vite.config.ts"];
const GENERATED_FILES: [&str; 4] = [
    "auto-imports.d.ts",
    "components.d.ts",
    "src/auto-imports.d.ts",
    "src/components.d.ts",
];
const VUE3_TOOLING: [&str; 2] = ["@vue/compiler-sfc", "@vitejs/plugin-vue"];

pub(super) const PROFILE: ProfileDefinition = ProfileDefinition {
    name: "vue3",
    parents: &[],
    detect,
    entry_points,
    generated_paths,
    reference_ranges,
    fallback_import: at_src_fallback,
    is_virtual_module: no_virtual_modules,
};

fn detect(ctx: &PackageContext<'_>) -> bool {
    detect_vue_major(ctx, 3, &VUE3_TOOLING).unwrap_or_else(|| {
        ctx.has_runtime_or_dev_dep("@vitejs/plugin-vue") && ctx.has_any_extension(&["vue"])
    })
}

fn entry_points(ctx: &PackageContext<'_>) -> Vec<String> {
    vue_entry_points(ctx, &CONFIG_FILES)
}

fn generated_paths(ctx: &PackageContext<'_>) -> Vec<String> {
    ctx.matching_files(|rel| rel.starts_with("dist/") || GENERATED_FILES.contains(&rel))
}

fn reference_ranges(ctx: &PackageContext<'_>) -> Option<Vec<ReferenceRange>> {
    vue_reference_ranges(
        ctx,
        VUE3_REFERENCE_SOURCE,
        (0.042, 0.438),
        (0.022, 0.297),
        (0.000, 0.002),
        (0.021, 0.297),
        (0.000, 0.325),
    )
}
