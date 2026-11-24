use codeint_model::{
    CodeModel, DependencyManifest, FileModel, ImportResolution, RepoPath, RuntimeImport,
    SourceFile, SourceSet, Workspace,
};

use super::{OrphanFileKind, OrphanFiles};

fn repo_path(text: &str) -> RepoPath {
    RepoPath::new(text).expect("valid repo path")
}

fn source_file(name: &str, source_set: SourceSet) -> SourceFile {
    SourceFile {
        path: repo_path(name),
        model: FileModel {
            source_set,
            source_lines: 5,
            functions: Vec::new(),
            tokens: Vec::new(),
            flagged_lines: Vec::new(),
            hazard_lines: Vec::new(),
            runtime_imports: Vec::new(),
            program_start: false,
        },
    }
}

fn build_model(
    files: &[(&str, SourceSet)],
    edges: &[(&str, &str)],
    workspace_members: &[&str],
) -> CodeModel {
    CodeModel {
        files: files
            .iter()
            .map(|(name, set)| source_file(name, *set))
            .collect(),
        runtime_imports: edges
            .iter()
            .map(|(from, to)| RuntimeImport {
                importer: repo_path(from),
                specifier: format!("./{to}"),
                resolution: ImportResolution::Resolved(vec![repo_path(to)]),
            })
            .collect(),
        workspace: Workspace::new(workspace_members.iter().copied().map(repo_path)),
        ..CodeModel::default()
    }
}

fn production_model(files: &[&str], edges: &[(&str, &str)]) -> CodeModel {
    workspace_production_model(files, edges, &[])
}

fn workspace_production_model(
    files: &[&str],
    edges: &[(&str, &str)],
    workspace_members: &[&str],
) -> CodeModel {
    let entries: Vec<(&str, SourceSet)> = files
        .iter()
        .map(|name| (*name, SourceSet::Production))
        .collect();
    build_model(&entries, edges, workspace_members)
}

fn orphan_paths(measured: &OrphanFiles) -> Vec<&str> {
    measured
        .findings()
        .iter()
        .map(|finding| finding.path.as_str())
        .collect()
}

#[test]
fn detects_live_unreferenced_and_test_only_production_files() {
    let model = build_model(
        &[
            ("src/index.ts", SourceSet::Production),
            ("src/live.ts", SourceSet::Production),
            ("src/deep.ts", SourceSet::Production),
            ("src/dead.ts", SourceSet::Production),
            ("src/test_only.ts", SourceSet::Production),
            ("src/test_only_dep.ts", SourceSet::Production),
            ("src/test_only.test.ts", SourceSet::Test),
            ("src/live.test.ts", SourceSet::Test),
        ],
        &[
            ("src/index.ts", "src/live.ts"),
            ("src/live.ts", "src/deep.ts"),
            ("src/test_only.test.ts", "src/test_only.ts"),
            ("src/test_only.ts", "src/test_only_dep.ts"),
            ("src/live.test.ts", "src/live.ts"),
        ],
        &[],
    );

    let measured = OrphanFiles::measure(&model, &[repo_path("src/index.ts")])
        .expect("entry point activates orphan-file detection");

    assert_eq!(measured.evaluated_files(), 6);
    assert_eq!(measured.entry_points(), &[repo_path("src/index.ts")]);
    assert_eq!(measured.unreferenced_count(), 1);
    assert_eq!(measured.test_only_count(), 2);
    assert_eq!(measured.findings().len(), 3);
    assert!((measured.ratio() - 0.5).abs() < 1e-9);

    let summary: Vec<(&str, OrphanFileKind, Vec<&str>)> = measured
        .findings()
        .iter()
        .map(|f| {
            (
                f.path.as_str(),
                f.kind,
                f.test_importers.iter().map(RepoPath::as_str).collect(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            ("src/dead.ts", OrphanFileKind::Unreferenced, vec![]),
            (
                "src/test_only.ts",
                OrphanFileKind::TestOnly,
                vec!["src/test_only.test.ts"],
            ),
            (
                "src/test_only_dep.ts",
                OrphanFileKind::TestOnly,
                vec!["src/test_only.test.ts"],
            ),
        ]
    );
    assert!(
        measured.findings()[1]
            .message
            .contains("only imported by tests")
    );
}

#[test]
fn reachable_import_cycle_is_live_while_unreachable_import_cycle_is_orphan() {
    let model = production_model(
        &[
            "src/main.ts",
            "src/live_a.ts",
            "src/live_b.ts",
            "src/dead_a.ts",
            "src/dead_b.ts",
        ],
        &[
            ("src/main.ts", "src/live_a.ts"),
            ("src/live_a.ts", "src/live_b.ts"),
            ("src/live_b.ts", "src/live_a.ts"),
            ("src/dead_a.ts", "src/dead_b.ts"),
            ("src/dead_b.ts", "src/dead_a.ts"),
        ],
    );

    let measured = OrphanFiles::measure(&model, &[repo_path("src/main.ts")])
        .expect("entry point activates detection");

    assert_eq!(
        orphan_paths(&measured),
        vec!["src/dead_a.ts", "src/dead_b.ts"]
    );
    assert_eq!(measured.unreferenced_count(), 2);
    assert_eq!(measured.test_only_count(), 0);
}

#[test]
fn stays_silently_inactive_when_zero_entry_points_exist() {
    let model = build_model(
        &[
            ("src/a.ts", SourceSet::Production),
            ("src/b.ts", SourceSet::Production),
            ("src/a.test.ts", SourceSet::Test),
        ],
        &[("src/a.test.ts", "src/a.ts")],
        &[],
    );

    assert!(OrphanFiles::measure(&model, &[]).is_none());
    assert!(OrphanFiles::measure(&model, &[repo_path("src/missing.ts")]).is_none());
    assert!(OrphanFiles::measure(&model, &[repo_path("src/a.test.ts")]).is_none());
}

#[test]
fn excludes_generated_and_vendored_source_sets() {
    let model = build_model(
        &[
            ("src/index.ts", SourceSet::Production),
            ("src/orphan.ts", SourceSet::Production),
            ("src/schema.g.ts", SourceSet::Generated),
            ("vendor/lib.ts", SourceSet::Vendored),
        ],
        &[],
        &[],
    );

    let measured = OrphanFiles::measure(&model, &[repo_path("src/index.ts")])
        .expect("entry point activates detection");

    assert_eq!(measured.evaluated_files(), 2);
    assert_eq!(orphan_paths(&measured), vec!["src/orphan.ts"]);
}

#[test]
fn generated_and_vendored_entry_points_do_not_activate_orphan_detection() {
    let single = build_model(
        &[
            ("app/main.ts", SourceSet::Production),
            ("app/util.ts", SourceSet::Production),
            ("app/types.g.ts", SourceSet::Generated),
            ("vendor/pkg.ts", SourceSet::Vendored),
        ],
        &[],
        &[],
    );
    for candidates in [
        vec![repo_path("app/types.g.ts")],
        vec![repo_path("vendor/pkg.ts")],
        vec![repo_path("app/types.g.ts"), repo_path("vendor/pkg.ts")],
    ] {
        assert!(OrphanFiles::measure(&single, &candidates).is_none());
    }

    let workspace = build_model(
        &[
            ("mods/web/main.ts", SourceSet::Production),
            ("mods/web/unused.ts", SourceSet::Production),
            ("mods/gen/service.ts", SourceSet::Production),
            ("mods/gen/router.g.ts", SourceSet::Generated),
            ("mods/gen/vendor/entry.ts", SourceSet::Vendored),
        ],
        &[],
        &["mods/web", "mods/gen"],
    );
    let measured = OrphanFiles::measure(
        &workspace,
        &[
            repo_path("mods/web/main.ts"),
            repo_path("mods/gen/router.g.ts"),
            repo_path("mods/gen/vendor/entry.ts"),
        ],
    )
    .expect("production root activates detection");

    assert_eq!(
        (
            measured.entry_points(),
            measured.evaluated_files(),
            orphan_paths(&measured),
        ),
        (
            &[repo_path("mods/web/main.ts")][..],
            2,
            vec!["mods/web/unused.ts"],
        )
    );
}

#[test]
fn traverses_cross_package_imports_and_skips_unrooted_isolated_packages() {
    let model = build_model(
        &[
            ("packages/app/src/index.ts", SourceSet::Production),
            ("packages/lib/src/used.ts", SourceSet::Production),
            ("packages/lib/src/unused.ts", SourceSet::Production),
            ("packages/site/pages/home.ts", SourceSet::Production),
            ("packages/site/pages/about.ts", SourceSet::Production),
        ],
        &[("packages/app/src/index.ts", "packages/lib/src/used.ts")],
        &["packages/app", "packages/lib", "packages/site"],
    );

    let measured = OrphanFiles::measure(&model, &[repo_path("packages/app/src/index.ts")])
        .expect("entry point in app activates detection");

    assert_eq!(measured.evaluated_files(), 3);
    assert_eq!(orphan_paths(&measured), vec!["packages/lib/src/unused.ts"]);

    let lib_only =
        OrphanFiles::measure_filtered(&model, &[repo_path("packages/app/src/index.ts")], |path| {
            path.as_str().starts_with("packages/lib/")
        })
        .expect("lib package stays active via cross-package import");
    assert_eq!(lib_only.evaluated_files(), 2);
    assert_eq!(lib_only.findings().len(), 1);
    assert_eq!(
        lib_only.findings()[0].path.as_str(),
        "packages/lib/src/unused.ts"
    );
}

#[test]
fn assigns_stable_finding_ids_and_merges_across_languages() {
    let ts_model = build_model(
        &[
            ("web/index.ts", SourceSet::Production),
            ("web/dead.ts", SourceSet::Production),
        ],
        &[],
        &[],
    );
    let py_model = build_model(
        &[
            ("api/main.py", SourceSet::Production),
            ("api/dead.py", SourceSet::Production),
        ],
        &[],
        &[],
    );

    let first_ts = OrphanFiles::measure(&ts_model, &[repo_path("web/index.ts")]).expect("ts");
    let second_ts = OrphanFiles::measure(&ts_model, &[repo_path("web/index.ts")]).expect("ts");
    assert_eq!(first_ts.findings()[0].id, second_ts.findings()[0].id);

    let py = OrphanFiles::measure(&py_model, &[repo_path("api/main.py")]).expect("py");
    assert_ne!(first_ts.findings()[0].id, py.findings()[0].id);

    let merged = OrphanFiles::merge([first_ts, py]).expect("merged");
    assert_eq!(merged.evaluated_files(), 4);
    assert_eq!(merged.findings().len(), 2);
    assert_eq!(merged.findings()[0].path.as_str(), "api/dead.py");
    assert_eq!(merged.findings()[1].path.as_str(), "web/dead.ts");
    assert!((merged.ratio() - 0.5).abs() < 1e-9);
}

#[test]
fn excludes_non_runtime_tooling_files_from_orphan_evaluation_while_keeping_runtime_orphans() {
    let model = workspace_production_model(
        &[
            "packages/tools/src/index.ts",
            "packages/tools/src/dead.ts",
            "packages/tools/app/app.config.ts",
            "packages/tools/scripts/release.ts",
            "packages/tools/vite.config.ts",
            "scripts/release.ts",
            "docs/conf.py",
            "examples/demo.dart",
            "vite.config.ts",
            ".opencode/plugins/no-hook-bypass.js",
        ],
        &[],
        &["packages/tools"],
    );

    let measured = OrphanFiles::measure(&model, &[repo_path("packages/tools/src/index.ts")])
        .expect("production root activates detection");

    assert_eq!(measured.evaluated_files(), 3);
    assert_eq!(
        orphan_paths(&measured),
        vec![
            "packages/tools/app/app.config.ts",
            "packages/tools/src/dead.ts"
        ]
    );
}

fn dep_manifest(package_root: &str, path: &str) -> DependencyManifest {
    DependencyManifest {
        package_root: package_root.to_owned(),
        path: repo_path(path),
    }
}

fn assert_tooling_entry_ignored(
    model: &CodeModel,
    runtime_entry: &str,
    tooling_entries: &[&str],
    expected_dead: &str,
) {
    let tooling_paths: Vec<RepoPath> = tooling_entries.iter().copied().map(repo_path).collect();
    assert!(OrphanFiles::measure(model, &tooling_paths).is_none());

    let mut all_entries = vec![repo_path(runtime_entry)];
    all_entries.extend(tooling_paths);
    let measured =
        OrphanFiles::measure(model, &all_entries).expect("runtime entry point activates detection");
    assert_eq!(measured.entry_points(), &[repo_path(runtime_entry)]);
    assert_eq!(measured.evaluated_files(), 2);
    assert_eq!(orphan_paths(&measured), vec![expected_dead]);
}

#[test]
fn tooling_entry_points_do_not_activate_orphan_detection_or_keep_dead_runtime_files_live() {
    let dart_model = production_model(
        &[
            "lib/main.dart",
            "lib/dead.dart",
            "examples/demo/lib/main.dart",
            "scripts/cli.ts",
        ],
        &[("examples/demo/lib/main.dart", "lib/dead.dart")],
    );
    assert_tooling_entry_ignored(
        &dart_model,
        "lib/main.dart",
        &["examples/demo/lib/main.dart", "scripts/cli.ts"],
        "lib/dead.dart",
    );

    let workspace = workspace_production_model(
        &[
            "packages/core/src/index.ts",
            "packages/core/src/dead.ts",
            "packages/core/scripts/cli.ts",
        ],
        &[("packages/core/scripts/cli.ts", "packages/core/src/dead.ts")],
        &["packages/core"],
    );
    assert_tooling_entry_ignored(
        &workspace,
        "packages/core/src/index.ts",
        &["packages/core/scripts/cli.ts"],
        "packages/core/src/dead.ts",
    );
}

#[test]
fn excludes_non_workspace_subdirectory_manifest_tooling_files_from_orphans() {
    let mut model = production_model(
        &[
            "frontend/src/index.ts",
            "frontend/src/dead.ts",
            "frontend/vite.config.ts",
            "frontend/scripts/release.ts",
            "pkgs/foo/lib/main.dart",
            "pkgs/foo/lib/dead.dart",
            "pkgs/foo/tool/jnigen.dart",
        ],
        &[],
    );
    model.dependency_manifests = vec![
        dep_manifest("frontend", "frontend/package.json"),
        dep_manifest("pkgs/foo", "pkgs/foo/pubspec.yaml"),
    ];

    let measured = OrphanFiles::measure(
        &model,
        &[
            repo_path("frontend/src/index.ts"),
            repo_path("pkgs/foo/lib/main.dart"),
        ],
    )
    .expect("runtime entry points activate detection");

    assert_eq!(measured.evaluated_files(), 4);
    assert_eq!(
        orphan_paths(&measured),
        vec!["frontend/src/dead.ts", "pkgs/foo/lib/dead.dart"]
    );
}

#[test]
fn excludes_nested_example_subpackage_from_orphan_roots_and_evaluated_files() {
    let mut model = production_model(
        &[
            "pkgs/foo/lib/main.dart",
            "pkgs/foo/lib/dead.dart",
            "pkgs/foo/example/lib/main.dart",
        ],
        &[("pkgs/foo/example/lib/main.dart", "pkgs/foo/lib/dead.dart")],
    );
    model.dependency_manifests = vec![
        dep_manifest("pkgs/foo", "pkgs/foo/pubspec.yaml"),
        dep_manifest("pkgs/foo/example", "pkgs/foo/example/pubspec.yaml"),
    ];

    assert_tooling_entry_ignored(
        &model,
        "pkgs/foo/lib/main.dart",
        &["pkgs/foo/example/lib/main.dart"],
        "pkgs/foo/lib/dead.dart",
    );
}
