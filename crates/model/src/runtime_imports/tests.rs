use crate::language::{DeclaredDep, ImportRequest, Resolution, SourceSet};
use crate::{
    FileModel, LanguagePlugin, LanguagePluginError, LowerError, Manifest, ReferenceRanges,
    RepoPath, SourceFile, WorkspaceError,
};

use super::{
    ImportResolution, ResolveError, RuntimeImport, choose_manifests, resolve_runtime_imports,
};

enum StubAnswer {
    Resolutions(Vec<Resolution>),
    MemoryLimit { plugin: String, memory_mb: u64 },
    NoImports,
}

struct Answering(StubAnswer);

impl Answering {
    fn of(resolutions: Vec<Resolution>) -> Answering {
        Answering(StubAnswer::Resolutions(resolutions))
    }

    fn assert_imports_enabled(&self, method: &str) {
        assert!(
            !matches!(self.0, StubAnswer::NoImports),
            "{method} must not be called when has_imports is false"
        );
    }
}

impl LanguagePlugin for Answering {
    fn declared_dependencies(
        &self,
        _manifests: Vec<Manifest>,
    ) -> Result<Vec<DeclaredDep>, LanguagePluginError> {
        self.assert_imports_enabled("declared_dependencies");
        Ok(Vec::new())
    }

    fn entry_points(
        &self,
        _repository_paths: &[RepoPath],
        _manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, LanguagePluginError> {
        self.assert_imports_enabled("entry_points");
        Ok(Vec::new())
    }

    fn extensions(&self) -> Vec<String> {
        if matches!(self.0, StubAnswer::NoImports) {
            vec!["noimp".to_owned()]
        } else {
            vec!["ts".to_owned()]
        }
    }

    fn lower(&self, _path: &RepoPath, _text: &str) -> Result<FileModel, LowerError> {
        Err(LowerError::TooLarge)
    }

    fn manifest_paths(
        &self,
        _repository_paths: &[RepoPath],
    ) -> Result<Vec<String>, LanguagePluginError> {
        self.assert_imports_enabled("manifest_paths");
        Ok(vec!["package.json".to_owned()])
    }

    fn workspace_packages(
        &self,
        _manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, WorkspaceError> {
        self.assert_imports_enabled("workspace_packages");
        Ok(Vec::new())
    }

    fn resolve_imports(
        &self,
        _repository_paths: &[RepoPath],
        _manifests: Vec<Manifest>,
        _requests: Vec<ImportRequest>,
        _active_profiles: &[String],
    ) -> Result<Vec<Resolution>, LanguagePluginError> {
        match &self.0 {
            StubAnswer::Resolutions(resolutions) => Ok(resolutions.clone()),
            StubAnswer::MemoryLimit { plugin, memory_mb } => {
                Err(LanguagePluginError::MemoryLimitExceeded {
                    plugin: plugin.clone(),
                    memory_mb: *memory_mb,
                })
            }
            StubAnswer::NoImports => {
                panic!("resolve_imports must not be called when has_imports is false")
            }
        }
    }

    fn reference_ranges(&self) -> Result<ReferenceRanges, LanguagePluginError> {
        ReferenceRanges::new(Vec::new()).map_err(LanguagePluginError::InvalidReferenceRange)
    }

    crate::empty_framework_profile_plugin_methods!();

    fn has_hazard_rules(&self) -> bool {
        false
    }

    fn hazard_rules(&self) -> Vec<String> {
        Vec::new()
    }

    fn has_imports(&self) -> bool {
        !matches!(self.0, StubAnswer::NoImports)
    }
}

fn path(text: &str) -> RepoPath {
    RepoPath::new(text).expect("a valid path")
}

fn importer(name: &str, specifiers: &[&str]) -> SourceFile {
    SourceFile {
        path: path(name),
        model: FileModel {
            source_set: SourceSet::Production,
            source_lines: 1,
            functions: Vec::new(),
            tokens: Vec::new(),
            flagged_lines: Vec::new(),
            hazard_lines: Vec::new(),
            runtime_imports: specifiers.iter().map(|text| (*text).to_owned()).collect(),
            program_start: false,
        },
    }
}

#[test]
fn each_runtime_import_keeps_its_importer_specifier_and_resolution() {
    let plugin = Answering::of(vec![
        Resolution::Resolved(vec!["b.ts".to_owned(), "c.ts".to_owned()]),
        Resolution::External("react".to_owned()),
        Resolution::Builtin,
        Resolution::Unresolved("gone".to_owned()),
    ]);
    let a = importer("a.ts", &["./pkg", "react", "node:fs"]);
    let b = importer("b.ts", &["./gone"]);

    let imports = resolve_runtime_imports(
        &plugin,
        &[path("a.ts"), path("b.ts"), path("c.ts")],
        Vec::new(),
        &[&a, &b],
        &[],
    )
    .expect("the plugin answers every request");

    assert_eq!(
        imports,
        [
            RuntimeImport {
                importer: path("a.ts"),
                specifier: "./pkg".to_owned(),
                resolution: ImportResolution::Resolved(vec![path("b.ts"), path("c.ts")]),
            },
            RuntimeImport {
                importer: path("a.ts"),
                specifier: "react".to_owned(),
                resolution: ImportResolution::External("react".to_owned()),
            },
            RuntimeImport {
                importer: path("a.ts"),
                specifier: "node:fs".to_owned(),
                resolution: ImportResolution::Builtin,
            },
            RuntimeImport {
                importer: path("b.ts"),
                specifier: "./gone".to_owned(),
                resolution: ImportResolution::Unresolved {
                    reason: "gone".to_owned(),
                },
            },
        ]
    );
}

#[test]
fn a_plugin_that_answers_the_wrong_number_of_requests_is_an_error() {
    let plugin = Answering::of(vec![Resolution::External("react".to_owned())]);
    let a = importer("a.ts", &["react", "vue"]);

    let result = resolve_runtime_imports(&plugin, &[path("a.ts")], Vec::new(), &[&a], &[]);

    assert!(
        matches!(
            result,
            Err(ResolveError::WrongCount {
                asked: 2,
                answered: 1
            })
        ),
        "unexpected result: {result:?}"
    );
}

fn resolve_single_import(
    specifier: &str,
    resolution: Resolution,
) -> Result<Vec<RuntimeImport>, ResolveError> {
    let plugin = Answering::of(vec![resolution]);
    let a = importer("a.ts", &[specifier]);
    resolve_runtime_imports(&plugin, &[path("a.ts")], Vec::new(), &[&a], &[])
}

fn assert_rejects_external_package(package: &str) {
    let result = resolve_single_import("mystery", Resolution::External(package.to_owned()));
    assert!(
        matches!(
            &result,
            Err(ResolveError::EmptyExternalPackage { importer, specifier })
                if importer.as_str() == "a.ts" && specifier == "mystery"
        ),
        "invalid external package {package:?} must be rejected, got {result:?}"
    );
}

#[test]
fn a_plugin_that_resolves_to_an_empty_list_is_an_error() {
    let result = resolve_single_import("./empty", Resolution::Resolved(Vec::new()));

    assert!(
        matches!(
            &result,
            Err(ResolveError::EmptyResolution { importer, specifier })
                if importer.as_str() == "a.ts" && specifier == "./empty"
        ),
        "unexpected result: {result:?}"
    );
}

#[test]
fn a_plugin_that_resolves_to_a_file_outside_the_snapshot_is_an_error() {
    let plugin = Answering::of(vec![Resolution::Resolved(vec![
        "a.ts".to_owned(),
        "../etc/passwd".to_owned(),
    ])]);
    let a = importer("a.ts", &["./b"]);

    let result = resolve_runtime_imports(&plugin, &[path("a.ts")], Vec::new(), &[&a], &[]);

    assert!(
        matches!(&result, Err(ResolveError::UnknownTarget { target, .. }) if target == "../etc/passwd"),
        "unexpected result: {result:?}"
    );
}

#[test]
fn the_manifests_a_plugin_chooses_must_be_files_of_the_snapshot() {
    let plugin = Answering::of(Vec::new());

    let chosen = choose_manifests(&plugin, &[path("a.ts"), path("package.json")]);
    let missing = choose_manifests(&plugin, &[path("a.ts")]);

    assert!(
        matches!(&chosen, Ok(paths) if paths == &[path("package.json")]),
        "unexpected result: {chosen:?}"
    );
    assert!(
        matches!(&missing, Err(ResolveError::UnknownManifest { path }) if path == "package.json"),
        "unexpected result: {missing:?}"
    );
}

#[test]
fn no_imports_plugin_is_never_asked_for_manifests_or_import_resolution() {
    let plugin = Answering(StubAnswer::NoImports);
    let file = importer("a.noimp", &["./ignored"]);

    let manifests =
        choose_manifests(&plugin, &[path("a.noimp")]).expect("choose_manifests skips no-imports");
    assert!(manifests.is_empty());

    let resolved = resolve_runtime_imports(&plugin, &[path("a.noimp")], Vec::new(), &[&file], &[])
        .expect("resolve_runtime_imports skips no-imports");
    assert!(resolved.is_empty());
}

#[test]
fn resolve_runtime_imports_deduplicates_targets_preserving_first_seen_order() {
    let plugin = Answering::of(vec![Resolution::Resolved(vec![
        "src/b.fld".to_owned(),
        "src/b.fld".to_owned(),
        "src/a.fld".to_owned(),
        "src/b.fld".to_owned(),
    ])]);
    let repo = [path("src/main.fld"), path("src/a.fld"), path("src/b.fld")];
    let file = importer("src/main.fld", &["pkg"]);

    let resolved = resolve_runtime_imports(&plugin, &repo, Vec::new(), &[&file], &[])
        .expect("deduplicated multi-file resolution");

    assert_eq!(
        resolved,
        vec![RuntimeImport {
            importer: path("src/main.fld"),
            specifier: "pkg".to_owned(),
            resolution: ImportResolution::Resolved(vec![path("src/b.fld"), path("src/a.fld")]),
        }]
    );
}

#[test]
fn resolve_runtime_imports_propagates_plugin_memory_limit_errors() {
    let plugin = Answering(StubAnswer::MemoryLimit {
        plugin: "folder-import-lang".to_owned(),
        memory_mb: 1,
    });
    let file = importer("src/main.fld", &["pkg"]);
    let error =
        resolve_runtime_imports(&plugin, &[path("src/main.fld")], Vec::new(), &[&file], &[])
            .expect_err("memory limit error must propagate instead of becoming WrongCount");

    assert!(
        matches!(
            error,
            ResolveError::Plugin(LanguagePluginError::MemoryLimitExceeded {
                ref plugin,
                memory_mb: 1,
            }) if plugin == "folder-import-lang"
        ),
        "unexpected error: {error:?}"
    );
}

#[test]
fn a_plugin_that_resolves_to_an_empty_external_package_is_an_error() {
    for empty_package in ["", "   "] {
        assert_rejects_external_package(empty_package);
    }
}

#[test]
fn a_plugin_that_resolves_to_a_blank_invisible_or_control_external_package_is_an_error() {
    for invalid_package in [
        "",
        "   ",
        "\u{00a0}",
        "\u{200B}",
        "\u{FEFF}",
        "\0",
        "pkg\0name",
    ] {
        assert_rejects_external_package(invalid_package);
    }
}
