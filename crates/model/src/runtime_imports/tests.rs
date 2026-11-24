use crate::language::{DeclaredDep, ImportRequest, Resolution, SourceSet};
use crate::{
    FileModel, InvalidReferenceRange, LanguagePlugin, LanguagePluginError, LowerError, Manifest,
    ReferenceRanges, RepoPath, SourceFile, WorkspaceError,
};

use super::{
    ImportResolution, ResolveError, RuntimeImport, choose_manifests, resolve_runtime_imports,
};

struct Answering(Vec<Resolution>);

impl LanguagePlugin for Answering {
    fn declared_dependencies(
        &self,
        _manifests: Vec<Manifest>,
    ) -> Result<Vec<DeclaredDep>, LanguagePluginError> {
        Ok(Vec::new())
    }

    fn entry_points(
        &self,
        _repository_paths: &[RepoPath],
        _manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, LanguagePluginError> {
        Ok(Vec::new())
    }

    fn extensions(&self) -> Vec<String> {
        vec!["ts".to_owned()]
    }

    fn lower(&self, _path: &RepoPath, _text: &str) -> Result<FileModel, LowerError> {
        Err(LowerError::TooLarge)
    }

    fn manifest_paths(&self, _repository_paths: &[RepoPath]) -> Vec<String> {
        vec!["package.json".to_owned()]
    }

    fn workspace_packages(
        &self,
        _manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, WorkspaceError> {
        Ok(Vec::new())
    }

    fn resolve_imports(
        &self,
        _repository_paths: &[RepoPath],
        _manifests: Vec<Manifest>,
        _requests: Vec<ImportRequest>,
    ) -> Vec<Resolution> {
        self.0.clone()
    }

    fn reference_ranges(&self) -> Result<ReferenceRanges, InvalidReferenceRange> {
        ReferenceRanges::new(Vec::new())
    }

    fn has_hazard_rules(&self) -> bool {
        false
    }

    fn hazard_rules(&self) -> Vec<String> {
        Vec::new()
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
            runtime_imports: specifiers.iter().map(|text| (*text).to_owned()).collect(),
            program_start: false,
        },
    }
}

#[test]
fn each_runtime_import_keeps_its_importer_specifier_and_resolution() {
    let plugin = Answering(vec![
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
    let plugin = Answering(vec![Resolution::External("react".to_owned())]);
    let a = importer("a.ts", &["react", "vue"]);

    let result = resolve_runtime_imports(&plugin, &[path("a.ts")], Vec::new(), &[&a]);

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

#[test]
fn a_plugin_that_resolves_to_an_empty_list_is_an_error() {
    let plugin = Answering(vec![Resolution::Resolved(Vec::new())]);
    let a = importer("a.ts", &["./empty"]);

    let result = resolve_runtime_imports(&plugin, &[path("a.ts")], Vec::new(), &[&a]);

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
    let plugin = Answering(vec![Resolution::Resolved(vec![
        "a.ts".to_owned(),
        "../etc/passwd".to_owned(),
    ])]);
    let a = importer("a.ts", &["./b"]);

    let result = resolve_runtime_imports(&plugin, &[path("a.ts")], Vec::new(), &[&a]);

    assert!(
        matches!(&result, Err(ResolveError::UnknownTarget { target, .. }) if target == "../etc/passwd"),
        "unexpected result: {result:?}"
    );
}

#[test]
fn the_manifests_a_plugin_chooses_must_be_files_of_the_snapshot() {
    let plugin = Answering(Vec::new());

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
