use std::collections::BTreeSet;

use codeint_model::{
    CodeModel, DeclaredDep, DepKind, DependencyManifest, FileModel, ImportResolution,
    ManifestUsedDep, RepoPath, RuntimeImport, SourceFile, SourceSet, Workspace,
};

use super::{DependencyHygiene, UnusedDependencyKind};

fn source_file(path: &str, source_set: SourceSet) -> SourceFile {
    SourceFile {
        path: RepoPath::new(path).expect("valid path"),
        model: FileModel {
            source_set,
            source_lines: 10,
            functions: Vec::new(),
            tokens: Vec::new(),
            flagged_lines: Vec::new(),
            runtime_imports: Vec::new(),
        },
    }
}

fn manifest(package_root: &str, path: &str) -> DependencyManifest {
    DependencyManifest {
        package_root: package_root.to_owned(),
        path: RepoPath::new(path).expect("valid path"),
    }
}

fn declared(package_root: &str, name: &str, kind: DepKind) -> DeclaredDep {
    DeclaredDep {
        package_root: package_root.to_owned(),
        name: name.to_owned(),
        kind,
    }
}

fn import_with(importer: &str, specifier: &str, resolution: ImportResolution) -> RuntimeImport {
    RuntimeImport {
        importer: RepoPath::new(importer).expect("valid path"),
        specifier: specifier.to_owned(),
        resolution,
    }
}

fn external(importer: &str, package: &str) -> RuntimeImport {
    import_with(
        importer,
        package,
        ImportResolution::External(package.to_owned()),
    )
}

fn model_with(
    files: Vec<SourceFile>,
    manifests: Vec<DependencyManifest>,
    declared_deps: Vec<DeclaredDep>,
    imports: Vec<RuntimeImport>,
) -> CodeModel {
    CodeModel {
        files,
        dependency_manifests: manifests,
        declared_dependencies: declared_deps,
        runtime_imports: imports,
        ..CodeModel::default()
    }
}

fn declared_list(entries: &[(&str, &str, DepKind)]) -> Vec<DeclaredDep> {
    entries
        .iter()
        .map(|&(root, name, kind)| DeclaredDep {
            package_root: root.to_owned(),
            name: name.to_owned(),
            kind,
        })
        .collect()
}

fn source_files(entries: &[(&str, SourceSet)]) -> Vec<SourceFile> {
    entries
        .iter()
        .map(|&(path, source_set)| source_file(path, source_set))
        .collect()
}

fn externals(entries: &[(&str, &str)]) -> Vec<RuntimeImport> {
    entries
        .iter()
        .map(|&(importer, pkg)| external(importer, pkg))
        .collect()
}

#[test]
fn detects_unused_unlisted_and_dev_in_production_while_ignoring_builtins_and_resolved() {
    let mut imports = externals(&[
        ("src/app.ts", "zod"),
        ("src/app.ts", "axios"),
        ("src/app.ts", "faker"),
        ("src/app.test.ts", "vitest"),
    ]);
    imports.extend([
        import_with("src/app.ts", "node:fs", ImportResolution::Builtin),
        import_with(
            "src/app.ts",
            "./helper",
            ImportResolution::Resolved(RepoPath::new("src/helper.ts").unwrap()),
        ),
        import_with(
            "src/app.ts",
            "./missing",
            ImportResolution::Unresolved {
                reason: "missing".to_owned(),
            },
        ),
    ]);
    let model = model_with(
        source_files(&[
            ("src/app.ts", SourceSet::Production),
            ("src/helper.ts", SourceSet::Production),
            ("src/app.test.ts", SourceSet::Test),
        ]),
        vec![manifest("", "package.json")],
        declared_list(&[
            ("", "zod", DepKind::Runtime),
            ("", "moment", DepKind::Runtime),
            ("", "vitest", DepKind::Dev),
            ("", "faker", DepKind::Dev),
            ("", "unused-dev", DepKind::Dev),
            ("", "react", DepKind::Optional),
        ]),
        imports,
    );

    let result = DependencyHygiene::measure(&model, &BTreeSet::new());

    assert!(result.has_manifests());
    assert_eq!(result.declared_count(), 5);
    assert_eq!(result.imported_count(), 4);
    assert_eq!(result.production_imported_count(), 3);
    assert_eq!(result.unused().len(), 2);
    assert_eq!(result.unused()[0].dependency, "moment");
    assert_eq!(result.unused()[0].kind, UnusedDependencyKind::Runtime);
    assert_eq!(result.unused()[1].dependency, "unused-dev");
    assert_eq!(result.unused()[1].kind, UnusedDependencyKind::Dev);
    assert_eq!(result.unlisted().len(), 1);
    assert_eq!(result.unlisted()[0].dependency, "axios");
    assert_eq!(result.dev_in_production().len(), 1);
    assert_eq!(result.dev_in_production()[0].dependency, "faker");
}

#[test]
fn matches_longest_package_root_prefix_in_nested_monorepo_without_prefix_collisions() {
    let model = model_with(
        source_files(&[
            ("scripts/build.ts", SourceSet::Production),
            ("packages/api/src/index.ts", SourceSet::Production),
            ("packages/api/sub/src/worker.ts", SourceSet::Production),
            ("packages/api-v2/src/index.ts", SourceSet::Production),
        ]),
        vec![
            manifest("", "package.json"),
            manifest("packages/api", "packages/api/package.json"),
            manifest("packages/api/sub", "packages/api/sub/package.json"),
        ],
        declared_list(&[
            ("", "chalk", DepKind::Runtime),
            ("packages/api", "fastify", DepKind::Runtime),
            ("packages/api/sub", "bullmq", DepKind::Runtime),
        ]),
        externals(&[
            ("scripts/build.ts", "chalk"),
            ("packages/api/src/index.ts", "fastify"),
            ("packages/api/sub/src/worker.ts", "bullmq"),
            ("packages/api-v2/src/index.ts", "chalk"),
            ("packages/api/src/index.ts", "chalk"),
        ]),
    );

    let result = DependencyHygiene::measure(&model, &BTreeSet::new());

    assert!(result.unused().is_empty());
    assert_eq!(result.unlisted().len(), 1);
    assert_eq!(result.unlisted()[0].package_root, "packages/api");
    assert_eq!(result.unlisted()[0].dependency, "chalk");
    assert_eq!(
        result.unlisted()[0].manifest.as_str(),
        "packages/api/package.json"
    );
}

#[test]
fn produces_no_findings_when_no_dependency_manifests_exist() {
    let model = model_with(
        vec![source_file("src/app.ts", SourceSet::Production)],
        Vec::new(),
        Vec::new(),
        vec![external("src/app.ts", "lodash")],
    );

    let result = DependencyHygiene::measure(&model, &BTreeSet::new());

    assert!(!result.has_manifests());
    assert!(!result.has_activity());
}

#[test]
fn empty_manifest_still_flags_unlisted_external_imports() {
    let model = model_with(
        vec![source_file("src/app.ts", SourceSet::Production)],
        vec![manifest("", "package.json")],
        Vec::new(),
        vec![external("src/app.ts", "lodash")],
    );

    let result = DependencyHygiene::measure(&model, &BTreeSet::new());

    assert!(result.has_manifests());
    let unlisted: Vec<&str> = result
        .unlisted()
        .iter()
        .map(|f| f.dependency.as_str())
        .collect();
    assert_eq!(unlisted, ["lodash"]);
}

#[test]
fn handles_generated_vendored_optional_and_ignore_allowlist() {
    let model = model_with(
        source_files(&[
            ("src/app.ts", SourceSet::Production),
            ("src/gen.g.dart", SourceSet::Generated),
            ("vendor/lib.ts", SourceSet::Vendored),
        ]),
        vec![manifest("", "package.json")],
        declared_list(&[
            ("", "codegen-runtime", DepKind::Runtime),
            ("", "ignored-unused", DepKind::Runtime),
            ("", "ignored-dev", DepKind::Dev),
            ("", "optional-peer", DepKind::Optional),
            ("", "both-runtime-and-dev", DepKind::Runtime),
            ("", "both-runtime-and-dev", DepKind::Dev),
        ]),
        externals(&[
            ("src/app.ts", "optional-peer"),
            ("src/app.ts", "both-runtime-and-dev"),
            ("src/app.ts", "ignored-dev"),
            ("src/app.ts", "ignored-unlisted"),
            ("src/gen.g.dart", "codegen-runtime"),
            ("src/gen.g.dart", "gen-only-unlisted"),
            ("vendor/lib.ts", "vendor-unlisted"),
        ]),
    );
    let ignored: BTreeSet<String> = ["ignored-unused", "ignored-dev", "ignored-unlisted"]
        .into_iter()
        .map(str::to_owned)
        .collect();

    let result = DependencyHygiene::measure(&model, &ignored);

    assert!(result.unused().is_empty());
    assert!(result.unlisted().is_empty());
    assert!(result.dev_in_production().is_empty());
}

#[test]
fn merges_independent_language_models_deterministically_with_stable_ids() {
    let ts = model_with(
        vec![source_file("web/src/index.ts", SourceSet::Production)],
        vec![manifest("web", "web/package.json")],
        vec![declared("web", "lodash", DepKind::Runtime)],
        vec![external("web/src/index.ts", "axios")],
    );
    let rust = model_with(
        vec![source_file("crates/api/src/lib.rs", SourceSet::Production)],
        vec![manifest("crates/api", "crates/api/Cargo.toml")],
        vec![declared("crates/api", "serde", DepKind::Runtime)],
        vec![external("crates/api/src/lib.rs", "reqwest")],
    );

    let first = DependencyHygiene::merge([
        DependencyHygiene::measure(&ts, &BTreeSet::new()),
        DependencyHygiene::measure(&rust, &BTreeSet::new()),
    ]);
    let second = DependencyHygiene::merge([
        DependencyHygiene::measure(&rust, &BTreeSet::new()),
        DependencyHygiene::measure(&ts, &BTreeSet::new()),
    ]);

    assert_eq!(first, second);
    assert_eq!(first.declared_count(), 2);
    assert_eq!(first.imported_count(), 2);
    assert_eq!(first.unused()[0].manifest.as_str(), "crates/api/Cargo.toml");
    assert_eq!(first.unused()[1].manifest.as_str(), "web/package.json");
}

#[test]
fn manifest_used_dependencies_credit_usage_without_hiding_dev_in_production_or_declared_count() {
    let mut model = model_with(
        source_files(&[("src/index.ts", SourceSet::Production)]),
        vec![manifest("", "package.json")],
        declared_list(&[
            ("", "zod", DepKind::Runtime),
            ("", "vitest", DepKind::Dev),
            ("", "eslint", DepKind::Dev),
            ("", "unused-dev", DepKind::Dev),
        ]),
        externals(&[("src/index.ts", "zod"), ("src/index.ts", "vitest")]),
    );
    model.manifest_used_dependencies = vec![
        ManifestUsedDep {
            package_root: String::new(),
            name: "vitest".to_owned(),
        },
        ManifestUsedDep {
            package_root: String::new(),
            name: "eslint".to_owned(),
        },
    ];

    let result = DependencyHygiene::measure(&model, &BTreeSet::new());

    assert_eq!(result.declared_count(), 4);
    assert_eq!(result.imported_count(), 2);
    assert_eq!(result.production_imported_count(), 2);
    assert_eq!(result.unused().len(), 1);
    assert_eq!(result.unused()[0].dependency, "unused-dev");
    assert!(result.unlisted().is_empty());
    assert_eq!(result.dev_in_production().len(), 1);
    assert_eq!(result.dev_in_production()[0].dependency, "vitest");
}

fn js_workspace_hoisting_model() -> CodeModel {
    let mut js_model = model_with(
        source_files(&[
            ("packages/app/src/index.ts", SourceSet::Production),
            ("packages/app/src/index.test.ts", SourceSet::Test),
            ("examples/demo/src/index.ts", SourceSet::Production),
        ]),
        vec![
            manifest("", "package.json"),
            manifest("packages/app", "packages/app/package.json"),
            manifest("examples/demo", "examples/demo/package.json"),
        ],
        declared_list(&[
            ("", "zod", DepKind::Runtime),
            ("", "vitest", DepKind::Dev),
            ("", "faker", DepKind::Dev),
            ("", "unused-root", DepKind::Dev),
            ("packages/app", "react", DepKind::Runtime),
        ]),
        externals(&[
            ("packages/app/src/index.ts", "react"),
            ("packages/app/src/index.ts", "zod"),
            ("packages/app/src/index.ts", "faker"),
            ("packages/app/src/index.ts", "axios"),
            ("packages/app/src/index.test.ts", "vitest"),
            ("examples/demo/src/index.ts", "zod"),
        ]),
    );
    js_model.workspace = Workspace::new([RepoPath::new("packages/app").expect("valid root")]);
    js_model
}

#[test]
fn js_workspace_hoisting_credits_root_and_preserves_dev_in_production_and_isolation() {
    let js_model = js_workspace_hoisting_model();

    let result = DependencyHygiene::measure(&js_model, &BTreeSet::new());

    assert_eq!(result.unused().len(), 1);
    assert_eq!(result.unused()[0].package_root, "");
    assert_eq!(result.unused()[0].dependency, "unused-root");

    let unlisted: Vec<(&str, &str)> = result
        .unlisted()
        .iter()
        .map(|f| (f.package_root.as_str(), f.dependency.as_str()))
        .collect();
    assert_eq!(
        unlisted,
        vec![("examples/demo", "zod"), ("packages/app", "axios")]
    );

    assert_eq!(result.dev_in_production().len(), 1);
    assert_eq!(result.dev_in_production()[0].package_root, "packages/app");
    assert_eq!(result.dev_in_production()[0].dependency, "faker");

    let mut rust_model = model_with(
        source_files(&[("crates/app/src/lib.rs", SourceSet::Production)]),
        vec![
            manifest("", "Cargo.toml"),
            manifest("crates/app", "crates/app/Cargo.toml"),
        ],
        declared_list(&[("", "serde", DepKind::Runtime)]),
        externals(&[("crates/app/src/lib.rs", "serde")]),
    );
    rust_model.workspace = Workspace::new([RepoPath::new("crates/app").expect("valid root")]);

    let rust_result = DependencyHygiene::measure(&rust_model, &BTreeSet::new());
    assert_eq!(rust_result.unused().len(), 1);
    assert_eq!(rust_result.unlisted().len(), 1);
    assert_eq!(rust_result.unlisted()[0].dependency, "serde");
}

#[test]
fn measure_filtered_preserves_workspace_hoisting_for_member_and_root_scopes() {
    let model = js_workspace_hoisting_model();

    let member = DependencyHygiene::measure_filtered(&model, &BTreeSet::new(), |root| {
        root == "packages/app"
    });
    assert!(member.has_manifests());
    assert_eq!(member.declared_count(), 1);
    assert!(member.unused().is_empty());
    assert_eq!(member.imported_count(), 5);
    assert_eq!(member.production_imported_count(), 4);
    assert_eq!(member.unlisted().len(), 1);
    assert_eq!(member.unlisted()[0].package_root, "packages/app");
    assert_eq!(member.unlisted()[0].dependency, "axios");
    assert_eq!(member.dev_in_production().len(), 1);
    assert_eq!(member.dev_in_production()[0].package_root, "packages/app");
    assert_eq!(member.dev_in_production()[0].dependency, "faker");

    let root_only =
        DependencyHygiene::measure_filtered(&model, &BTreeSet::new(), |root| root.is_empty());
    assert!(root_only.has_manifests());
    assert_eq!(root_only.declared_count(), 4);
    assert_eq!(root_only.unused().len(), 1);
    assert_eq!(root_only.unused()[0].package_root, "");
    assert_eq!(root_only.unused()[0].dependency, "unused-root");
    assert_eq!(root_only.imported_count(), 0);
    assert_eq!(root_only.production_imported_count(), 0);
    assert!(root_only.unlisted().is_empty());
    assert!(root_only.dev_in_production().is_empty());

    let missing = DependencyHygiene::measure_filtered(&model, &BTreeSet::new(), |root| {
        root == "packages/other"
    });
    assert!(!missing.has_manifests());
    assert!(!missing.has_activity());
}
