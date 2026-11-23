use codeint_model::{
    CodeModel, FileModel, ImportResolution, RepoPath, RuntimeImport, SourceFile, SourceSet,
    Workspace,
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
            runtime_imports: Vec::new(),
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
                resolution: ImportResolution::Resolved(repo_path(to)),
            })
            .collect(),
        workspace: Workspace::new(workspace_members.iter().copied().map(repo_path)),
        ..CodeModel::default()
    }
}

fn production_model(files: &[&str], edges: &[(&str, &str)]) -> CodeModel {
    let entries: Vec<(&str, SourceSet)> = files
        .iter()
        .map(|name| (*name, SourceSet::Production))
        .collect();
    build_model(&entries, edges, &[])
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
