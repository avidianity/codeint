use crate::{
    CodeModel, DeclaredDep, DepKind, DependencyManifest, RepoPath, SourceFile, SourceSet,
    UniversalToolingPaths, Workspace, empty_file_model,
};

fn assert_root_paths(paths: &[&str], expected: bool) {
    let tooling = UniversalToolingPaths::default();
    for &path in paths {
        let repo_path = RepoPath::new(path).expect("valid repo path");
        assert_eq!(
            tooling.is_universal_tooling_path(&repo_path),
            expected,
            "unexpected repo-root classification for {path}"
        );
    }
}

fn assert_package_paths(cases: &[(&str, &str)], expected: bool) {
    for &(path, package_root) in cases {
        let tooling = UniversalToolingPaths::from_roots([package_root]);
        let repo_path = RepoPath::new(path).expect("valid repo path");
        assert_eq!(
            tooling.is_universal_tooling_path(&repo_path),
            expected,
            "unexpected package-root classification for {path} (package_root={package_root})"
        );
    }
}

fn assert_paths_with_roots(cases: &[(&str, &[&str])], expected: bool) {
    for &(path, package_roots) in cases {
        let tooling = UniversalToolingPaths::from_roots(package_roots.iter().copied());
        let repo_path = RepoPath::new(path).expect("valid repo path");
        assert_eq!(
            tooling.is_universal_tooling_path(&repo_path),
            expected,
            "unexpected multi-root classification for {path} (package_roots={package_roots:?})"
        );
    }
}

#[test]
fn matches_only_canonical_universal_tooling_folders_at_repo_and_package_roots() {
    assert_root_paths(
        &[
            ".github/scripts/check.mjs",
            ".github/workflows/ci.ts",
            "scripts/release.ts",
            "scripts/action-comment.mjs",
            "docs/conf.py",
            "examples/demo/main.dart",
            "examples/celery/make_celery.py",
            "tools/codegen.rs",
            "benches/parse.rs",
        ],
        true,
    );
    assert_package_paths(
        &[
            ("packages/core/scripts/gen.ts", "packages/core"),
            ("packages/app/docs/guide.ts", "packages/app"),
            ("packages/sdk/examples/demo/src/index.ts", "packages/sdk"),
            ("packages/core/tools/gen.ts", "packages/core"),
            ("crates/foo/benches/parse.rs", "crates/foo"),
            ("packages/web/.github/check.ts", "packages/web"),
        ],
        true,
    );
    assert_paths_with_roots(
        &[(
            "packages/sdk/examples/demo/src/index.ts",
            &["packages/sdk", "packages/sdk/examples/demo"],
        )],
        true,
    );
}

#[test]
fn rejects_language_specific_filenames_and_non_canonical_folders_in_core() {
    assert_root_paths(
        &[
            "vite.config.ts",
            "eslint.config.js",
            ".eslintrc.js",
            ".php-cs-fixer.dist.php",
            ".storybook/main.ts",
            "conftest.py",
            "setup.py",
            "conf.py",
            "noxfile.py",
            "fabfile.py",
            "example/main.dart",
            "tool/jnigen.dart",
            "bench/parse.rs",
            "benchmarks/router.ts",
            ".changeset/changelog.js",
            ".opencode/plugins/guard.js",
            ".claude/hooks/verify.js",
            ".agents/scripts/run.ts",
            ".vscode/extensions.ts",
            ".husky/check.mjs",
        ],
        false,
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
            ("packages/scripts/src/cli.ts", "packages/scripts"),
            ("packages/examples/src/main.ts", "packages/examples"),
            ("crates/benches/src/lib.rs", "crates/benches"),
        ],
        false,
    );
    assert_paths_with_roots(
        &[(
            "packages/tools/src/index.ts",
            &["packages/tools", "packages/sdk"],
        )],
        false,
    );
    assert_root_paths(
        &[
            "app/docs/page.tsx",
            "pages/docs/index.tsx",
            "app/tools/calc.tsx",
            "src/tools/helper.ts",
            "src/scripts/runner.ts",
            "src/examples/sample.ts",
            "src/docs/renderer.ts",
            "src/benches/run.rs",
        ],
        false,
    );
}

#[test]
fn code_model_is_tooling_file_combines_plugin_flag_and_universal_folders() {
    let mut plugin_marked = empty_file_model(SourceSet::Production);
    plugin_marked.is_tooling_file = true;
    let model = CodeModel {
        files: vec![
            SourceFile {
                path: RepoPath::new("frontend/vite.config.ts").expect("valid path"),
                model: plugin_marked,
            },
            SourceFile {
                path: RepoPath::new("frontend/scripts/release.ts").expect("valid path"),
                model: empty_file_model(SourceSet::Production),
            },
            SourceFile {
                path: RepoPath::new("packages/ws/benches/run.rs").expect("valid path"),
                model: empty_file_model(SourceSet::Production),
            },
            SourceFile {
                path: RepoPath::new("services/py/tools/check.py").expect("valid path"),
                model: empty_file_model(SourceSet::Production),
            },
            SourceFile {
                path: RepoPath::new("frontend/src/index.ts").expect("valid path"),
                model: empty_file_model(SourceSet::Production),
            },
        ],
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

    assert!(model.is_tooling_file(&model.files[0]));
    assert!(model.is_tooling_file(&model.files[1]));
    assert!(model.is_tooling_file(&model.files[2]));
    assert!(model.is_tooling_file(&model.files[3]));
    assert!(!model.is_tooling_file(&model.files[4]));

    assert!(model.is_tooling_path(&model.files[0].path));
    assert!(model.is_tooling_path(&model.files[1].path));
    assert!(!model.is_tooling_path(&model.files[4].path));
}
