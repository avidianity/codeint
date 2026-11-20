use super::language::{
    FileModel, Guest, ImportRequest, LowerError, ReferenceRange, Resolution, SourceSet,
};
use super::{
    CodeModel, ImportResolution, InvalidReferenceRange, LanguagePlugin, ReferenceRanges, RepoPath,
    RuntimeImport, SourceFile,
};

struct MeasuredLanguage;

impl Guest for MeasuredLanguage {
    fn extensions() -> Vec<String> {
        vec!["measured".to_owned()]
    }

    fn lower(_path: String, text: String) -> Result<FileModel, LowerError> {
        Ok(FileModel {
            source_set: SourceSet::Production,
            source_lines: u32::try_from(text.lines().count()).map_err(|_| LowerError::TooLarge)?,
            functions: Vec::new(),
            tokens: Vec::new(),
            flagged_lines: Vec::new(),
            runtime_imports: Vec::new(),
        })
    }

    fn resolve_imports(
        _repository_paths: Vec<String>,
        requests: Vec<ImportRequest>,
    ) -> Vec<Resolution> {
        requests.iter().map(|_| Resolution::External).collect()
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        vec![range("erosion", 0.12, 0.56, "Measured panel, Table 1")]
    }
}

fn range(metric: &str, low: f64, high: f64, source: &str) -> ReferenceRange {
    ReferenceRange {
        metric: metric.to_owned(),
        low,
        high,
        source: source.to_owned(),
    }
}

#[test]
fn a_reference_range_that_a_plugin_supplies_comes_back_for_its_metric_with_its_source_label() {
    let plugin: &dyn LanguagePlugin = &MeasuredLanguage;

    let ranges = plugin.reference_ranges().expect("valid Reference ranges");

    let erosion = ranges.for_metric("erosion").expect("an Erosion range");
    assert_eq!(
        (erosion.low, erosion.high, erosion.source.as_str()),
        (0.12, 0.56, "Measured panel, Table 1")
    );
    assert!(ranges.for_metric("verbosity").is_none());
    assert!(ranges.for_metric("Erosion").is_none());
}

#[test]
fn two_reference_ranges_for_one_metric_are_invalid() {
    let result = ReferenceRanges::new(vec![
        range("erosion", 0.1, 0.2, "First"),
        range("erosion", 0.3, 0.4, "Second"),
    ]);

    assert!(
        matches!(&result, Err(InvalidReferenceRange::Duplicate { metric }) if metric == "erosion"),
        "unexpected result: {result:?}"
    );
}

#[test]
fn a_reference_range_must_run_from_a_lower_to_a_higher_ratio_between_0_and_1() {
    for (low, high) in [
        (0.5, 0.4),
        (-0.1, 0.4),
        (0.1, 1.1),
        (f64::NAN, 0.4),
        (0.1, f64::NAN),
        (0.1, f64::INFINITY),
    ] {
        let result = ReferenceRanges::new(vec![range("verbosity", low, high, "Source")]);

        assert!(
            matches!(result, Err(InvalidReferenceRange::Bounds { .. })),
            "{low} to {high}: unexpected result: {result:?}"
        );
    }
    for (low, high) in [(0.0, 1.0), (0.3, 0.3)] {
        assert!(ReferenceRanges::new(vec![range("verbosity", low, high, "Source")]).is_ok());
    }
}

#[test]
fn a_repository_path_is_relative_and_uses_forward_slashes() {
    for valid in ["main.ts", "src/app/main.ts", ".github/ci.js"] {
        assert!(RepoPath::new(valid).is_ok(), "{valid} should be valid");
    }
    for invalid in [
        "",
        "/src/main.ts",
        "src\\main.ts",
        "src//main.ts",
        "src/",
        "./main.ts",
        "src/../main.ts",
    ] {
        assert!(
            RepoPath::new(invalid).is_err(),
            "{invalid} should be invalid"
        );
    }
}

#[test]
fn the_extension_is_the_text_after_the_last_dot_of_the_file_name() {
    let extension_of = |path: &str| {
        RepoPath::new(path)
            .expect("a valid path")
            .extension()
            .map(str::to_owned)
    };
    assert_eq!(extension_of("src/main.ts"), Some("ts".to_owned()));
    assert_eq!(extension_of("types/index.d.ts"), Some("ts".to_owned()));
    assert_eq!(extension_of("v1.2/Makefile"), None);
    assert_eq!(extension_of("src/.eslintrc"), None);
}

fn file_in(path: &str, source_set: SourceSet) -> SourceFile {
    SourceFile {
        path: RepoPath::new(path).expect("a valid path"),
        model: FileModel {
            source_set,
            source_lines: 1,
            functions: Vec::new(),
            tokens: Vec::new(),
            flagged_lines: Vec::new(),
            runtime_imports: Vec::new(),
        },
    }
}

fn import_of(importer: &str, target: &str) -> RuntimeImport {
    RuntimeImport {
        importer: RepoPath::new(importer).expect("a valid path"),
        specifier: format!("./{target}"),
        resolution: ImportResolution::Resolved(RepoPath::new(target).expect("a valid path")),
    }
}

#[test]
fn each_runtime_import_moves_into_the_source_set_of_the_file_that_imports() {
    let model = CodeModel {
        files: vec![
            file_in("main.ts", SourceSet::Production),
            file_in("main.test.ts", SourceSet::Test),
        ],
        runtime_imports: vec![
            import_of("main.ts", "main.test.ts"),
            import_of("main.test.ts", "main.ts"),
            import_of("main.test.ts", "main.test.ts"),
        ],
        ..CodeModel::default()
    };

    let source_sets = model.into_source_sets();

    assert_eq!(
        source_sets.get(SourceSet::Production).runtime_imports,
        [import_of("main.ts", "main.test.ts")]
    );
    assert_eq!(
        source_sets.get(SourceSet::Test).runtime_imports,
        [
            import_of("main.test.ts", "main.ts"),
            import_of("main.test.ts", "main.test.ts"),
        ]
    );
    assert!(
        source_sets
            .get(SourceSet::Generated)
            .runtime_imports
            .is_empty()
    );
}
