use crate::language::{ImportRequest, Manifest, Resolution, SourceSet};
use crate::{
    FileModel, InvalidReferenceRange, LanguagePlugin, LowerError, ReferenceRanges, RepoPath,
    SourceFile,
};

use super::{ImportResolution, ResolveError, RuntimeImport, resolve_runtime_imports};

struct Answering(Vec<Resolution>);

impl LanguagePlugin for Answering {
    fn extensions(&self) -> Vec<String> {
        vec!["ts".to_owned()]
    }

    fn lower(&self, _path: &RepoPath, _text: &str) -> Result<FileModel, LowerError> {
        Err(LowerError::TooLarge)
    }

    fn manifest_paths(&self, _repository_paths: &[RepoPath]) -> Vec<String> {
        Vec::new()
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
        },
    }
}

#[test]
fn each_runtime_import_keeps_its_importer_specifier_and_resolution() {
    let plugin = Answering(vec![
        Resolution::Resolved("b.ts".to_owned()),
        Resolution::External,
        Resolution::Unresolved("gone".to_owned()),
    ]);
    let a = importer("a.ts", &["./b", "react"]);
    let b = importer("b.ts", &["./gone"]);

    let imports = resolve_runtime_imports(
        &plugin,
        &[path("a.ts"), path("b.ts")],
        Vec::new(),
        &[&a, &b],
    )
    .expect("the plugin answers every request");

    assert_eq!(
        imports,
        [
            RuntimeImport {
                importer: path("a.ts"),
                specifier: "./b".to_owned(),
                resolution: ImportResolution::Resolved(path("b.ts")),
            },
            RuntimeImport {
                importer: path("a.ts"),
                specifier: "react".to_owned(),
                resolution: ImportResolution::External,
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
    let plugin = Answering(vec![Resolution::External]);
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
fn a_plugin_that_resolves_to_a_file_outside_the_snapshot_is_an_error() {
    let plugin = Answering(vec![Resolution::Resolved("../etc/passwd".to_owned())]);
    let a = importer("a.ts", &["./b"]);

    let result = resolve_runtime_imports(&plugin, &[path("a.ts")], Vec::new(), &[&a]);

    assert!(
        matches!(&result, Err(ResolveError::UnknownTarget { target, .. }) if target == "../etc/passwd"),
        "unexpected result: {result:?}"
    );
}
