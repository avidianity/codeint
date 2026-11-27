use codeint_model::language::ReferenceRange;

use super::{
    PackageContext, ProfileDefinition, at_src_fallback, detect_vue_major, no_virtual_modules,
    vue_entry_points, vue_reference_ranges,
};

const VUE2_REFERENCE_SOURCE: &str =
    "codeint stratified panel of 50 Vue 2 repositories, ADR 0035, codeint 0.8.0";

const CONFIG_FILES: [&str; 3] = ["vue.config.js", "vue.config.cjs", "vue.config.mjs"];
const VUE2_TOOLING: [&str; 3] = [
    "vue-template-compiler",
    "@vitejs/plugin-vue2",
    "@vue/cli-service",
];

pub(super) const PROFILE: ProfileDefinition = ProfileDefinition {
    name: "vue2",
    parents: &[],
    detect,
    entry_points,
    generated_paths,
    reference_ranges,
    fallback_import: at_src_fallback,
    is_virtual_module: no_virtual_modules,
};

fn detect(ctx: &PackageContext<'_>) -> bool {
    detect_vue_major(ctx, 2, &VUE2_TOOLING)
        .unwrap_or_else(|| ctx.has_any_file(&CONFIG_FILES) && ctx.has_any_extension(&["vue"]))
}

fn entry_points(ctx: &PackageContext<'_>) -> Vec<String> {
    vue_entry_points(ctx, &CONFIG_FILES)
}

fn generated_paths(ctx: &PackageContext<'_>) -> Vec<String> {
    ctx.matching_files(|rel| rel.starts_with("dist/"))
}

fn reference_ranges(ctx: &PackageContext<'_>) -> Option<Vec<ReferenceRange>> {
    vue_reference_ranges(
        ctx,
        VUE2_REFERENCE_SOURCE,
        (0.025, 0.479),
        (0.033, 0.406),
        (0.000, 0.003),
        (0.023, 0.267),
        (0.000, 0.128),
    )
}
