use codeint_model::{LanguagePlugin, RepoPath};

use super::is_tooling_file;
use crate::TypeScriptAndJavaScript;

#[test]
fn classifies_js_ts_tooling_configs_rc_files_and_storybook_paths() {
    for path in [
        "vite.config.ts",
        "vitest.config.mts",
        "eslint.config.mjs",
        "prettier.config.mjs",
        "rollup.config.js",
        "webpack.config.js",
        "tsup.config.ts",
        "jest.config.ts",
        "playwright.config.ts",
        "svelte.config.js",
        "nuxt.config.ts",
        "tailwind.config.js",
        "postcss.config.js",
        "babel.config.js",
        "astro.config.mjs",
        "uno.config.ts",
        "next.config.mjs",
        "rspack.config.ts",
        ".eslintrc.js",
        ".eslintrc.cjs",
        ".prettierrc.mjs",
        ".babelrc.js",
        ".stylelintrc.cjs",
        ".commitlintrc.ts",
        ".storybook/main.ts",
        "packages/core/vite.config.ts",
        "packages/app/vite.config.ts",
        "packages/web/.prettierrc.cjs",
        "packages/web/.customrc.js",
        "packages/web/.storybook/main.ts",
        "packages/web/react-router.config.ts",
        "packages/web/sst.config.ts",
        "packages/web/orval.config.ts",
        "frontend/vite.config.ts",
        "frontend/panda.config.ts",
        "apps/admin/graphql.config.ts",
    ] {
        assert!(is_tooling_file(path), "expected tooling file: {path}");
    }
}

#[test]
fn preserves_nested_app_configs_and_runtime_modules_as_non_tooling() {
    for path in [
        "packages/core/src/app.config.ts",
        "packages/web/src/db.config.ts",
        "packages/web/app/db.config.ts",
        "app/app.config.ts",
        "app/auth.config.ts",
        "lib/env.config.ts",
        "src/app.config.ts",
        "src/vite.config.ts",
        "config/auth.config.ts",
        "server/stripe.config.ts",
        "src/.app.config.ts",
        "src/.arc.ts",
        "config.ts",
        ".src.ts",
        ".config.ts",
        "foo.config.",
        "src/index.ts",
    ] {
        assert!(!is_tooling_file(path), "unexpected tooling file: {path}");
    }
}

#[test]
fn lower_sets_is_tooling_file_on_file_model() {
    let cases = [
        ("packages/web/vite.config.ts", "export default {};\n", true),
        (
            "packages/web/src/app.config.ts",
            "export const config = {};\n",
            false,
        ),
    ];
    for (path, text, expected) in cases {
        let model = TypeScriptAndJavaScript
            .lower(&RepoPath::new(path).expect("valid path"), text, None)
            .expect("lower ts file");
        assert_eq!(model.is_tooling_file, expected, "{path}");
    }
}
