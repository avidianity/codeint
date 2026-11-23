use codeint_model::{CodeModel, DeclaredDep, DepKind, DependencyManifest, RepoPath, Workspace};

use super::ToolingPaths;

fn assert_root_paths(paths: &[&str], expected: bool) {
    let tooling = ToolingPaths::default();
    for &path in paths {
        let repo_path = RepoPath::new(path).expect("valid repo path");
        assert_eq!(
            tooling.is_non_runtime_tooling_path(&repo_path),
            expected,
            "unexpected repo-root classification for {path}"
        );
    }
}

fn assert_package_paths(cases: &[(&str, &str)], expected: bool) {
    for &(path, package_root) in cases {
        let tooling = ToolingPaths::from_roots([package_root]);
        let repo_path = RepoPath::new(path).expect("valid repo path");
        assert_eq!(
            tooling.is_non_runtime_tooling_path(&repo_path),
            expected,
            "unexpected package-root classification for {path} (package_root={package_root})"
        );
    }
}

fn assert_paths_with_roots(roots: &[&str], paths: &[&str], expected: bool) {
    let tooling = ToolingPaths::from_roots(roots.iter().copied());
    for &path in paths {
        let repo_path = RepoPath::new(path).expect("valid repo path");
        assert_eq!(
            tooling.is_non_runtime_tooling_path(&repo_path),
            expected,
            "unexpected multi-root classification for {path} (roots={roots:?})"
        );
    }
}

#[test]
fn matches_non_runtime_tooling_directories_at_repo_root_package_root_and_dot_dirs() {
    assert_root_paths(
        &[
            "scripts/release.ts",
            "scripts/action-comment.mjs",
            "scripts/melos_packages.dart",
            "tools/codegen.rs",
            "examples/demo/main.dart",
            "examples/celery/make_celery.py",
            "example/main.dart",
            "examples/demo/lib/main.dart",
            "benchmarks/router.ts",
            "bench/parse.rs",
            "docs/conf.py",
            ".changeset/changelog.js",
            ".github/scripts/check.mjs",
            ".opencode/plugins/no-hook-bypass.js",
            ".claude/hooks/verify.js",
            ".agents/scripts/run.ts",
            ".vscode/extensions.ts",
            ".husky/check.mjs",
            ".storybook/main.ts",
        ],
        true,
    );
    assert_package_paths(
        &[
            ("packages/core/scripts/gen.ts", "packages/core"),
            ("packages/core/example/main.dart", "packages/core"),
            ("pkgs/foo/tool/jnigen.dart", "pkgs/foo"),
            ("examples/demo/lib/main.dart", "examples/demo"),
            ("crates/foo/benches/parse.rs", "crates/foo"),
            ("packages/app/docs/guide.ts", "packages/app"),
            ("packages/web/.storybook/main.ts", "packages/web"),
        ],
        true,
    );
}

#[test]
fn matches_build_lint_test_and_doc_config_filenames_at_repo_or_package_root() {
    assert_root_paths(
        &[
            "vite.config.ts",
            "vitest.config.ts",
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
            ".eslintrc.js",
            ".eslintrc.cjs",
            ".prettierrc.mjs",
            ".babelrc.js",
            ".stylelintrc.cjs",
            ".php-cs-fixer.dist.php",
            ".php-cs-fixer.php",
            ".commitlintrc.ts",
            "conf.py",
            "docs/conf.py",
            "noxfile.py",
            "fabfile.py",
            "conftest.py",
            "tests/conftest.py",
            "src/tests/conftest.py",
            "setup.py",
        ],
        true,
    );
    assert_package_paths(
        &[
            ("packages/core/vite.config.ts", "packages/core"),
            ("packages/web/.prettierrc.cjs", "packages/web"),
            ("packages/py/setup.py", "packages/py"),
        ],
        true,
    );
}

#[test]
fn preserves_monorepo_packages_named_like_tooling_dirs_and_nested_runtime_modules() {
    assert_package_paths(
        &[
            ("packages/tools/src/index.ts", "packages/tools"),
            ("crates/tools/src/lib.rs", "crates/tools"),
            ("apps/docs/src/app.tsx", "apps/docs"),
            ("packages/docs/src/index.ts", "packages/docs"),
            ("crates/bench/src/lib.rs", "crates/bench"),
            ("packages/scripts/src/cli.ts", "packages/scripts"),
            ("packages/example/src/main.ts", "packages/example"),
            ("crates/config/src/lib.rs", "crates/config"),
        ],
        false,
    );
    assert_root_paths(
        &[
            "app/docs/page.tsx",
            "pages/docs/index.tsx",
            "routes/docs/+page.svelte",
            "app/tools/calc.tsx",
            "app/Tools/Parser.php",
            "app/Docs/Renderer.php",
            "myapp/tools/parser.py",
            "myapp/docs/views.py",
            "src/config.rs",
            "src/config/loader.ts",
            "src/tools.rs",
            "src/tools/helper.ts",
            "src/scripts/runner.ts",
            "src/examples/sample.ts",
            "src/docs/renderer.ts",
            "lib/src/tools_helper.dart",
            "lib/src/examples/x.dart",
            "lib/src/scripts/bar.dart",
        ],
        false,
    );
}

#[test]
fn preserves_nested_config_and_runtime_filenames_below_repo_and_package_roots() {
    assert_package_paths(
        &[("packages/core/src/app.config.ts", "packages/core")],
        false,
    );
    assert_root_paths(
        &[
            "app/app.config.ts",
            "src/app.config.ts",
            "config/auth.config.ts",
            "server/stripe.config.ts",
            "config/app.config.php",
            "app/settings.config.py",
            "myapp/conf.py",
            "django_app/core/conf.py",
            "myapp/billing/setup.py",
            "src/conf.py",
            "src/setup.py",
            "src/noxfile.py",
            "src/fabfile.py",
            "src/.app.config.ts",
            "src/.arc.ts",
            "src/.crc.rs",
            "app_config.rs",
            "config.ts",
            "config.rs",
            ".src.ts",
            ".config.ts",
            "foo.config.",
        ],
        false,
    );
}

#[test]
fn matches_nested_example_and_tool_subpackages_while_preserving_tooling_named_packages() {
    assert_paths_with_roots(
        &["pkgs/foo", "pkgs/foo/example"],
        &["pkgs/foo/example/lib/main.dart"],
        true,
    );
    assert_paths_with_roots(
        &["packages/sdk", "packages/sdk/examples/demo"],
        &["packages/sdk/examples/demo/src/index.ts"],
        true,
    );
    assert_paths_with_roots(
        &["pkgs/foo", "pkgs/foo/tool/gen"],
        &["pkgs/foo/tool/gen/bin/main.dart"],
        true,
    );
    assert_paths_with_roots(
        &["packages/tools", "packages/sdk"],
        &["packages/tools/src/index.ts"],
        false,
    );
    assert_paths_with_roots(
        &["pkgs/foo", "pkgs/foo/sub"],
        &["pkgs/foo/sub/vite.config.ts"],
        true,
    );
    assert_paths_with_roots(
        &["pkgs/foo", "pkgs/foo/sub"],
        &["pkgs/foo/sub/src/app.config.ts"],
        false,
    );
    assert_paths_with_roots(&["pkgs/foo"], &["pkgs/foo/sub/vite.config.ts"], false);
}

#[test]
fn from_model_combines_workspace_manifests_and_declared_dependency_roots() {
    let model = CodeModel {
        workspace: Workspace::new([RepoPath::new("packages/ws").expect("valid root")]),
        dependency_manifests: vec![DependencyManifest {
            package_root: "frontend".to_owned(),
            path: RepoPath::new("frontend/package.json").expect("valid manifest"),
        }],
        declared_dependencies: vec![DeclaredDep {
            package_root: "services/py".to_owned(),
            name: "pytest".to_owned(),
            kind: DepKind::Dev,
        }],
        ..CodeModel::default()
    };
    let tooling = ToolingPaths::from_model(&model);
    for expected_tooling in [
        "packages/ws/vite.config.ts",
        "frontend/vite.config.ts",
        "frontend/scripts/release.ts",
        "services/py/setup.py",
    ] {
        let path = RepoPath::new(expected_tooling).expect("valid path");
        assert!(
            tooling.is_non_runtime_tooling_path(&path),
            "expected tooling path: {expected_tooling}"
        );
    }
    let runtime_path = RepoPath::new("frontend/src/index.ts").expect("valid path");
    assert!(!tooling.is_non_runtime_tooling_path(&runtime_path));
}
