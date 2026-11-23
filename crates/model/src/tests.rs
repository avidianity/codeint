use super::language::{
    DeclaredDep, DepKind, FileModel, Guest, ImportRequest, LowerError, Manifest, ReferenceRange,
    Resolution, SourceSet, WorkspaceError,
};
use super::{
    CodeModel, ImportResolution, InvalidReferenceRange, LanguagePlugin, LanguagePluginError,
    ManifestUsedDep, Package, ReferenceRanges, RepoPath, RuntimeImport, SourceFile, Workspace,
    token_text_hash, validate_declared_dependencies, validate_entry_points,
};

fn path(text: &str) -> RepoPath {
    RepoPath::new(text).expect("a valid path")
}

#[test]
fn a_file_belongs_to_the_deepest_workspace_package_whose_root_folder_holds_it() {
    let workspace = Workspace::new([path("packages/app"), path("packages/app/plugins/chart")]);
    let package_of = |file: &str| workspace.package_of(&path(file)).to_string();

    assert_eq!(package_of("packages/app/src/main.ts"), "packages/app");
    assert_eq!(
        package_of("packages/app/plugins/chart/index.ts"),
        "packages/app/plugins/chart"
    );
    assert_eq!(package_of("packages/app2/index.ts"), ".");
    assert_eq!(package_of("packages/app.ts"), ".");
    assert_eq!(package_of("main.ts"), ".");
}

#[test]
fn the_packages_of_a_workspace_are_the_root_package_then_each_member_by_path() {
    let workspace = Workspace::new([path("packages/b"), path("packages/a"), path("packages/b")]);

    let packages: Vec<String> = workspace
        .packages()
        .map(|package| package.to_string())
        .collect();

    assert_eq!(packages, [".", "packages/a", "packages/b"]);
    assert!(workspace.has_members());
    assert!(!Workspace::default().has_members());
    assert_eq!(
        Workspace::default()
            .packages()
            .collect::<Vec<Package<'_>>>(),
        [Package::Root]
    );
}

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

    fn manifest_paths(repository_paths: Vec<String>) -> Vec<String> {
        repository_paths
    }

    fn workspace_packages(_manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        Ok(Vec::new())
    }

    fn resolve_imports(
        _repository_paths: Vec<String>,
        manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
    ) -> Vec<Resolution> {
        let reason = format!("{} manifests name no module", manifests.len());
        requests
            .iter()
            .map(|_| Resolution::Unresolved(reason.clone()))
            .collect()
    }

    fn declared_dependencies(_manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
        Vec::new()
    }

    fn entry_points(_repository_paths: Vec<String>, _manifests: Vec<Manifest>) -> Vec<String> {
        Vec::new()
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

#[test]
fn the_token_text_hash_is_64_bit_fnv_1a() {
    assert_eq!(token_text_hash(""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(token_text_hash("a"), 0xaf63_dc4c_8601_ec8c);
    assert_eq!(token_text_hash("foobar"), 0x8594_4171_f739_67e8);
}

#[test]
fn validate_entry_points_rejects_traversal_absolute_and_unknown_paths() {
    let repo_paths = vec![RepoPath::new("src/main.ts").expect("valid path")];
    assert_eq!(
        validate_entry_points(&repo_paths, vec!["src/main.ts".to_owned()])
            .expect("known repo path succeeds"),
        repo_paths
    );
    assert!(matches!(
        validate_entry_points(&repo_paths, vec!["../../../etc/passwd".to_owned()]),
        Err(LanguagePluginError::InvalidEntryPointPath { ref path }) if path == "../../../etc/passwd"
    ));
    assert!(matches!(
        validate_entry_points(&repo_paths, vec!["/etc/shadow".to_owned()]),
        Err(LanguagePluginError::InvalidEntryPointPath { ref path }) if path == "/etc/shadow"
    ));
    assert!(matches!(
        validate_entry_points(&repo_paths, vec!["src/missing.ts".to_owned()]),
        Err(LanguagePluginError::UnknownEntryPoint { ref path }) if path == "src/missing.ts"
    ));
}

#[test]
fn validate_declared_dependencies_rejects_invalid_package_roots_and_empty_names() {
    let valid = vec![
        DeclaredDep {
            package_root: String::new(),
            name: "serde".to_owned(),
            kind: DepKind::Runtime,
        },
        DeclaredDep {
            package_root: "packages/app".to_owned(),
            name: "@scope/pkg".to_owned(),
            kind: DepKind::Dev,
        },
    ];
    assert_eq!(
        validate_declared_dependencies(valid.clone()).expect("valid deps succeed"),
        valid
    );
    for bad_root in ["../escape", "/etc", "packages\\app", "packages/app/"] {
        let result = validate_declared_dependencies(vec![DeclaredDep {
            package_root: bad_root.to_owned(),
            name: "serde".to_owned(),
            kind: DepKind::Runtime,
        }]);
        assert!(
            matches!(
                result,
                Err(LanguagePluginError::InvalidDependencyPackageRoot { ref package_root })
                    if package_root == bad_root
            ),
            "expected invalid package root error for {bad_root:?}, got {result:?}"
        );
    }
    for bad_name in ["", "   "] {
        let result = validate_declared_dependencies(vec![DeclaredDep {
            package_root: String::new(),
            name: bad_name.to_owned(),
            kind: DepKind::Runtime,
        }]);
        assert!(
            matches!(result, Err(LanguagePluginError::EmptyDependencyName { .. })),
            "expected empty dependency name error for {bad_name:?}, got {result:?}"
        );
    }
}

#[test]
fn code_model_merge_sorts_and_deduplicates_manifest_used_dependencies() {
    let first = CodeModel {
        manifest_used_dependencies: vec![
            ManifestUsedDep {
                package_root: "packages/web".to_owned(),
                name: "vite".to_owned(),
            },
            ManifestUsedDep {
                package_root: String::new(),
                name: "vitest".to_owned(),
            },
        ],
        ..CodeModel::default()
    };
    let second = CodeModel {
        manifest_used_dependencies: vec![
            ManifestUsedDep {
                package_root: String::new(),
                name: "vitest".to_owned(),
            },
            ManifestUsedDep {
                package_root: String::new(),
                name: "eslint".to_owned(),
            },
        ],
        ..CodeModel::default()
    };

    let merged = CodeModel::merge([first, second]);

    assert_eq!(
        merged.manifest_used_dependencies,
        vec![
            ManifestUsedDep {
                package_root: String::new(),
                name: "eslint".to_owned(),
            },
            ManifestUsedDep {
                package_root: String::new(),
                name: "vitest".to_owned(),
            },
            ManifestUsedDep {
                package_root: "packages/web".to_owned(),
                name: "vite".to_owned(),
            },
        ]
    );
}
