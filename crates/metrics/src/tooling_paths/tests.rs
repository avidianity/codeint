use super::is_non_runtime_tooling_path;

#[test]
fn matches_non_runtime_tooling_directories_at_root_and_in_workspace_packages() {
    for path in [
        "scripts/release.ts",
        "scripts/action-comment.mjs",
        "scripts/melos_packages.dart",
        "packages/web/scripts/gen.ts",
        "pkgs/foo/tool/jnigen.dart",
        "tools/codegen.rs",
        "examples/demo/main.dart",
        "examples/celery/make_celery.py",
        "example/main.dart",
        "examples/demo/lib/main.dart",
        "benchmarks/router.ts",
        "bench/parse.rs",
        "docs/conf.py",
        "packages/app/docs/guide.ts",
        ".changeset/changelog.js",
        ".github/scripts/check.mjs",
        ".opencode/plugins/no-hook-bypass.js",
        ".claude/hooks/verify.js",
        ".agents/scripts/run.ts",
        ".vscode/extensions.ts",
        ".husky/check.mjs",
        ".storybook/main.ts",
    ] {
        assert!(
            is_non_runtime_tooling_path(path),
            "expected tooling path to match: {path}"
        );
    }
}

#[test]
fn matches_build_lint_test_and_doc_config_filenames() {
    for path in [
        "vite.config.ts",
        "vitest.config.mts",
        "eslint.config.js",
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
        "packages/web/vite.config.ts",
        ".eslintrc.js",
        ".eslintrc.cjs",
        ".prettierrc.mjs",
        ".babelrc.js",
        ".stylelintrc.cjs",
        ".php-cs-fixer.dist.php",
        ".php-cs-fixer.php",
        ".commitlintrc.ts",
        "conf.py",
        "noxfile.py",
        "fabfile.py",
        "conftest.py",
        "tests/conftest.py",
        "src/tests/conftest.py",
        "setup.py",
        "packages/sdk/setup.py",
    ] {
        assert!(
            is_non_runtime_tooling_path(path),
            "expected config filename to match: {path}"
        );
    }
}

#[test]
fn preserves_runtime_application_source_paths_under_src_and_lib() {
    for path in [
        "src/config.rs",
        "src/config/loader.ts",
        "src/app.config.ts",
        "src/tools.rs",
        "src/tools/helper.ts",
        "src/scripts/runner.ts",
        "src/examples/sample.ts",
        "src/docs/renderer.ts",
        "src/conf.py",
        "src/setup.py",
        "crates/config/src/lib.rs",
        "lib/src/tools_helper.dart",
        "lib/src/examples/x.dart",
        "lib/src/scripts/bar.dart",
        "app_config.rs",
        "config.ts",
        "config.rs",
        ".src.ts",
        ".config.ts",
        "foo.config.",
    ] {
        assert!(
            !is_non_runtime_tooling_path(path),
            "expected production path not to match: {path}"
        );
    }
}
