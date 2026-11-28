use std::collections::{BTreeMap, HashSet};

use super::language::{
    DeclaredDep, DepKind, FileHazards, FileModel, FrameworkProfileResult, Guest, HazardLine,
    ImportRequest, LowerError, Manifest, ManifestAnalysis, PackageFrameworkProfiles,
    ReferenceRange, Resolution, ScopeAnalysis, SourceSet,
};
use super::{
    CodeModel, FunctionCoverage, ImportResolution, InvalidReferenceRange, LanguageFacts,
    LanguagePlugin, LanguagePluginError, ManifestUsedDep, Package, ReferenceRanges, RepoPath,
    RuntimeImport, SourceFile, Workspace, empty_file_model, empty_framework_profile_result,
    empty_manifest_analysis, language, token_text_hash, validate_declared_dependencies,
    validate_entry_points, validate_flagged_lines, validate_framework_profile_result,
    validate_manifest_analysis, validate_scope_analysis,
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

fn production_file_model(text: &str) -> Result<FileModel, LowerError> {
    let source_lines = u32::try_from(text.lines().count()).map_err(|_| LowerError::TooLarge)?;
    Ok(FileModel {
        source_lines,
        ..empty_file_model(SourceSet::Production)
    })
}

struct MeasuredLanguage;

impl Guest for MeasuredLanguage {
    fn manifest_paths(repository_paths: Vec<String>) -> Vec<String> {
        repository_paths
    }

    fn extensions() -> Vec<String> {
        vec!["measured".to_owned()]
    }

    fn lower(
        _path: String,
        text: String,
        _dialect: Option<String>,
    ) -> Result<FileModel, LowerError> {
        production_file_model(&text)
    }

    fn analyze_manifests(_manifests: Vec<Manifest>) -> ManifestAnalysis {
        empty_manifest_analysis()
    }

    fn resolve_imports(
        _repository_paths: Vec<String>,
        manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
        _package_profiles: Vec<PackageFrameworkProfiles>,
    ) -> Vec<Resolution> {
        let reason = format!("{} manifests name no module", manifests.len());
        requests
            .iter()
            .map(|_| Resolution::Unresolved(reason.clone()))
            .collect()
    }

    fn entry_points(_repository_paths: Vec<String>, _manifests: Vec<Manifest>) -> Vec<String> {
        Vec::new()
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        vec![range("erosion", 0.12, 0.56, "Measured panel, Table 1")]
    }

    crate::empty_framework_profile_guest_methods!();

    fn has_hazard_rules() -> bool {
        false
    }

    fn hazard_rules() -> Vec<String> {
        Vec::new()
    }

    fn has_imports() -> bool {
        true
    }

    crate::empty_v05_guest_methods!();
}

fn range(metric: &str, low: f64, high: f64, source: &str) -> ReferenceRange {
    ReferenceRange {
        metric: metric.to_owned(),
        metric_version: 1,
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
            source_lines: 1,
            ..empty_file_model(source_set)
        },
    }
}

fn import_of(importer: &str, target: &str) -> RuntimeImport {
    RuntimeImport {
        importer: RepoPath::new(importer).expect("a valid path"),
        specifier: format!("./{target}"),
        resolution: ImportResolution::Resolved(vec![RepoPath::new(target).expect("a valid path")]),
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

#[test]
fn function_coverage_distinguishes_absent_from_zero_executed_lines() {
    let absent = super::language::Function {
        line: 1,
        end_line: 5,
        decision_points: 1,
        source_lines: 5,
        name: Some("uncovered".to_owned()),
        cognitive_complexity: 1,
        coverage: None,
    };
    let zero_executed = super::language::Function {
        line: 1,
        end_line: 5,
        decision_points: 1,
        source_lines: 5,
        name: Some("uncovered".to_owned()),
        cognitive_complexity: 1,
        coverage: Some(FunctionCoverage {
            listed_lines: 5,
            executed_lines: 0,
        }),
    };

    assert_ne!(absent, zero_executed);
    assert_eq!(absent.coverage, None);
    assert_eq!(
        zero_executed.coverage,
        Some(FunctionCoverage {
            listed_lines: 5,
            executed_lines: 0,
        })
    );
}

#[test]
fn code_model_merge_sorts_languages_and_combines_language_facts() {
    let first = CodeModel {
        languages: vec![
            LanguageFacts {
                language: "typescript-javascript".to_owned(),
                has_coverage: true,
                has_hazard_rules: false,
                has_imports: true,
            },
            LanguageFacts {
                language: "rust".to_owned(),
                has_coverage: false,
                has_hazard_rules: true,
                has_imports: false,
            },
        ],
        ..CodeModel::default()
    };
    let second = CodeModel {
        languages: vec![
            LanguageFacts {
                language: "python".to_owned(),
                has_coverage: false,
                has_hazard_rules: false,
                has_imports: false,
            },
            LanguageFacts {
                language: "rust".to_owned(),
                has_coverage: true,
                has_hazard_rules: false,
                has_imports: true,
            },
            LanguageFacts {
                language: "typescript-javascript".to_owned(),
                has_coverage: false,
                has_hazard_rules: true,
                has_imports: false,
            },
        ],
        ..CodeModel::default()
    };

    let merged = CodeModel::merge([first, second]);

    assert_eq!(
        merged.languages,
        vec![
            LanguageFacts {
                language: "python".to_owned(),
                has_coverage: false,
                has_hazard_rules: false,
                has_imports: false,
            },
            LanguageFacts {
                language: "rust".to_owned(),
                has_coverage: true,
                has_hazard_rules: true,
                has_imports: true,
            },
            LanguageFacts {
                language: "typescript-javascript".to_owned(),
                has_coverage: true,
                has_hazard_rules: true,
                has_imports: true,
            },
        ]
    );
}

#[test]
fn into_source_sets_preserves_language_facts_in_every_source_set() {
    let expected = vec![
        LanguageFacts {
            language: "rust".to_owned(),
            has_coverage: true,
            has_hazard_rules: false,
            has_imports: true,
        },
        LanguageFacts {
            language: "sql".to_owned(),
            has_coverage: false,
            has_hazard_rules: true,
            has_imports: false,
        },
    ];
    let model = CodeModel {
        files: vec![file_in("src/main.rs", SourceSet::Production)],
        languages: expected.clone(),
        ..CodeModel::default()
    };

    let source_sets = model.into_source_sets();

    for source_set in [
        SourceSet::Production,
        SourceSet::Test,
        SourceSet::Generated,
        SourceSet::Vendored,
    ] {
        assert_eq!(source_sets.get(source_set).source_set, source_set);
        assert_eq!(source_sets.get(source_set).languages, expected);
    }
}

#[test]
fn source_sets_merge_preserves_files_and_imports() {
    let prod = path("src/app.rs");
    let test = path("tests/app_test.rs");
    let first = CodeModel {
        files: vec![
            file_in("src/app.rs", SourceSet::Production),
            file_in("tests/app_test.rs", SourceSet::Test),
        ],
        runtime_imports: vec![
            RuntimeImport {
                importer: prod.clone(),
                specifier: "./util".to_owned(),
                resolution: ImportResolution::External("util".to_owned()),
            },
            RuntimeImport {
                importer: test.clone(),
                specifier: "../src/app".to_owned(),
                resolution: ImportResolution::Resolved(vec![prod.clone()]),
            },
        ],
        ..CodeModel::default()
    };
    let first_sets = first.into_source_sets();
    let projected = first_sets.get(SourceSet::Production);
    assert_eq!(projected.source_set, SourceSet::Production);
    assert_eq!(projected.files.len(), 1);
    assert_eq!(projected.runtime_imports.len(), 1);
    assert_eq!(projected.runtime_imports[0].importer, prod);

    let second = CodeModel {
        files: vec![file_in("src/b.rs", SourceSet::Production)],
        ..CodeModel::default()
    };
    let merged = crate::SourceSets::merge([first_sets, second.into_source_sets()]);
    assert_eq!(merged.get(SourceSet::Production).files.len(), 2);
    assert_eq!(merged.get(SourceSet::Test).files.len(), 1);
    assert_eq!(merged.get(SourceSet::Production).runtime_imports.len(), 1);
    assert_eq!(
        merged.get(SourceSet::Production).runtime_imports[0].importer,
        prod
    );
}

#[test]
fn language_plugin_trait_forwards_hazard_rules_and_has_imports_exports() {
    let plugin: &dyn LanguagePlugin = &MeasuredLanguage;

    assert!(!plugin.has_hazard_rules());
    assert!(plugin.hazard_rules().is_empty());
    assert!(plugin.has_imports());
}

#[test]
fn hazard_line_out_of_range_error_names_plugin_file_and_line() {
    let error = LanguagePluginError::HazardLineOutOfRange {
        plugin: "hazard-lang".to_owned(),
        path: path("queries/bad.haz"),
        line: 5,
        line_count: 3,
    };
    let message = error.to_string();

    assert!(message.contains("hazard-lang"), "message: {message}");
    assert!(message.contains("queries/bad.haz"), "message: {message}");
    assert!(message.contains("line 5"), "message: {message}");
    assert!(message.contains("3 lines"), "message: {message}");
}

#[test]
fn unlisted_hazard_rule_error_names_plugin_file_and_rule() {
    let error = LanguagePluginError::UnlistedHazardRule {
        plugin: "custom-sql".to_owned(),
        path: path("queries/users.sql"),
        rule: "unlisted-rule".to_owned(),
    };

    let message = error.to_string();

    assert!(
        message.contains("custom-sql")
            && message.contains("queries/users.sql")
            && message.contains("unlisted-rule"),
        "unexpected error message: {message}"
    );
}

#[test]
fn validate_framework_profile_result_rejects_unlisted_profiles_unknown_paths_and_invalid_ranges() {
    let known = vec!["react".to_owned()];
    let files = vec![path("src/App.tsx"), path("src/gen/routes.ts")];
    let valid = FrameworkProfileResult {
        active_profiles: vec!["react".to_owned()],
        entry_points: vec!["src/App.tsx".to_owned()],
        generated_paths: vec!["src/gen/routes.ts".to_owned()],
        reference_ranges: vec![range("erosion", 0.1, 0.5, "React panel")],
        reference_range_note: Some("react profile".to_owned()),
    };
    assert!(validate_framework_profile_result(&known, &files, valid).is_ok());

    let unlisted = FrameworkProfileResult {
        active_profiles: vec!["svelte".to_owned()],
        ..empty_framework_profile_result()
    };
    assert!(matches!(
        validate_framework_profile_result(&known, &files, unlisted),
        Err(LanguagePluginError::UnlistedFrameworkProfile { ref profile }) if profile == "svelte"
    ));

    let bad_entry = FrameworkProfileResult {
        entry_points: vec!["src/missing.tsx".to_owned()],
        ..empty_framework_profile_result()
    };
    assert!(matches!(
        validate_framework_profile_result(&known, &files, bad_entry),
        Err(LanguagePluginError::UnknownEntryPoint { ref path }) if path == "src/missing.tsx"
    ));

    let bad_generated = FrameworkProfileResult {
        generated_paths: vec!["../escape.ts".to_owned()],
        ..empty_framework_profile_result()
    };
    assert!(matches!(
        validate_framework_profile_result(&known, &files, bad_generated),
        Err(LanguagePluginError::InvalidGeneratedPath { ref path }) if path == "../escape.ts"
    ));

    let bad_range = FrameworkProfileResult {
        reference_ranges: vec![range("erosion", 0.8, 0.2, "Reversed")],
        ..empty_framework_profile_result()
    };
    assert!(matches!(
        validate_framework_profile_result(&known, &files, bad_range),
        Err(LanguagePluginError::InvalidFrameworkReferenceRange(
            InvalidReferenceRange::Bounds { .. }
        ))
    ));
}

#[test]
fn repo_path_as_ref_and_find_manifests_accept_borrowed_repo_paths() {
    let paths = [
        path("package.json"),
        path("packages/app/package.json"),
        path("packages/app/src/index.ts"),
    ];
    assert_eq!(paths[0].as_ref(), "package.json");
    assert_eq!(
        crate::scan::find_manifests(&paths, "package.json"),
        vec![
            "package.json".to_owned(),
            "packages/app/package.json".to_owned()
        ]
    );
}

#[test]
fn validate_workspace_packages_converts_valid_roots_and_rejects_invalid_paths() {
    let valid = crate::validate_workspace_packages(vec!["packages/app".to_owned()])
        .expect("valid workspace package");
    assert_eq!(valid, vec![path("packages/app")]);
    assert!(crate::validate_workspace_packages(vec!["../escape".to_owned()]).is_err());
}

#[test]
fn validate_flagged_lines_rejects_unlisted_rule_and_out_of_range_line() {
    let path = RepoPath::new("src/app.ts").expect("valid path");
    let listed: HashSet<String> = ["redundant-pass".to_owned()].into_iter().collect();
    assert!(
        validate_flagged_lines(
            "probe",
            &path,
            3,
            &[language::FlaggedLine {
                line: 2,
                rule: "redundant-pass".to_owned(),
            }],
            &listed,
        )
        .is_ok()
    );
    assert!(matches!(
        validate_flagged_lines(
            "probe",
            &path,
            3,
            &[language::FlaggedLine {
                line: 2,
                rule: "secret-rule".to_owned(),
            }],
            &listed,
        ),
        Err(LanguagePluginError::UnlistedFlaggedLineRule { ref rule, .. }) if rule == "secret-rule"
    ));
    assert!(matches!(
        validate_flagged_lines(
            "probe",
            &path,
            3,
            &[language::FlaggedLine {
                line: 4,
                rule: "redundant-pass".to_owned(),
            }],
            &listed,
        ),
        Err(LanguagePluginError::FlaggedLineOutOfRange { line: 4, .. })
    ));
}

#[test]
fn validate_manifest_analysis_rejects_duplicate_invalid_manifests() {
    let manifests = vec![Manifest {
        path: "package.json".to_owned(),
        text: "{bad".to_owned(),
    }];
    let analysis = ManifestAnalysis {
        invalid_manifests: vec![
            language::WorkspaceError {
                path: "package.json".to_owned(),
                message: "first".to_owned(),
            },
            language::WorkspaceError {
                path: "package.json".to_owned(),
                message: "second".to_owned(),
            },
        ],
        ..empty_manifest_analysis()
    };
    assert!(matches!(
        validate_manifest_analysis(&manifests, analysis),
        Err(LanguagePluginError::DuplicateInvalidManifest { ref path }) if path == "package.json"
    ));
}

#[test]
fn validate_manifest_analysis_rejects_unknown_dependency_manifest_and_accepts_known() {
    let manifests = vec![Manifest {
        path: "package.json".to_owned(),
        text: "{}".to_owned(),
    }];
    let unknown = ManifestAnalysis {
        dependency_manifests: vec![crate::DependencyManifestOwner {
            package_root: "packages/a".to_owned(),
            path: "packages/a/package.json".to_owned(),
        }],
        ..empty_manifest_analysis()
    };
    assert!(matches!(
        validate_manifest_analysis(&manifests, unknown),
        Err(LanguagePluginError::UnknownDependencyManifest { ref path })
            if path == "packages/a/package.json"
    ));

    let valid = ManifestAnalysis {
        dependency_manifests: vec![crate::DependencyManifestOwner {
            package_root: String::new(),
            path: "package.json".to_owned(),
        }],
        ..empty_manifest_analysis()
    };
    assert!(validate_manifest_analysis(&manifests, valid.clone()).is_ok());

    let overlapping = ManifestAnalysis {
        invalid_manifests: vec![language::WorkspaceError {
            path: "package.json".to_owned(),
            message: "is not valid JSON".to_owned(),
        }],
        ..valid
    };
    assert!(matches!(
        validate_manifest_analysis(&manifests, overlapping),
        Err(LanguagePluginError::InvalidAndDependencyManifest { ref path })
            if path == "package.json"
    ));
}

#[test]
fn code_model_merge_sorts_and_deduplicates_root_dependency_inheritance() {
    let first = CodeModel {
        root_dependency_inheritance: vec!["packages/b".to_owned(), "packages/a".to_owned()],
        ..CodeModel::default()
    };
    let second = CodeModel {
        root_dependency_inheritance: vec!["packages/a".to_owned(), "packages/c".to_owned()],
        ..CodeModel::default()
    };
    let merged = CodeModel::merge([first, second]);
    assert_eq!(
        merged.root_dependency_inheritance,
        vec!["packages/a", "packages/b", "packages/c"]
    );
}

fn scope_with_hazard(path: &str, line: u32) -> ScopeAnalysis {
    ScopeAnalysis {
        extra_hazards: vec![FileHazards {
            path: path.to_owned(),
            hazard_lines: vec![HazardLine {
                line,
                rule: "cross-file-risk".to_owned(),
            }],
        }],
        summaries: Vec::new(),
    }
}

#[test]
fn validate_scope_analysis_rejects_unknown_path_and_out_of_bounds_line() {
    let artifact_paths = ["src/a.hst"];
    let listed: HashSet<String> = ["cross-file-risk".to_owned()].into_iter().collect();
    let mut line_counts = BTreeMap::new();
    line_counts.insert("src/a.hst".to_owned(), 5);

    assert!(matches!(
        validate_scope_analysis(
            "probe",
            &artifact_paths,
            &listed,
            None,
            scope_with_hazard("not/in/artifacts.hst", 1),
        ),
        Err(LanguagePluginError::UnknownScopeHazardPath { ref path, .. })
            if path == "not/in/artifacts.hst"
    ));

    assert!(matches!(
        validate_scope_analysis(
            "probe",
            &artifact_paths,
            &listed,
            Some(&line_counts),
            scope_with_hazard("src/a.hst", 4_000_000_000),
        ),
        Err(LanguagePluginError::HazardLineOutOfRange {
            line: 4_000_000_000,
            ..
        })
    ));
}

#[test]
fn validate_file_diagnostics_rejects_invalid_lines_and_blank_fields() {
    let path = RepoPath::new("src/a.sql").expect("valid path");
    assert!(
        crate::validate_file_diagnostics(
            "sql",
            &path,
            3,
            &[crate::FileDiagnostic {
                line: 2,
                kind: "rejected-statement".to_owned(),
                message: "could not parse".to_owned(),
            }],
        )
        .is_ok()
    );
    assert!(matches!(
        crate::validate_file_diagnostics(
            "sql",
            &path,
            3,
            &[crate::FileDiagnostic {
                line: 0,
                kind: "note".to_owned(),
                message: "bad".to_owned(),
            }],
        ),
        Err(LanguagePluginError::DiagnosticLineOutOfRange { line: 0, .. })
    ));
    assert!(matches!(
        crate::validate_file_diagnostics(
            "sql",
            &path,
            3,
            &[crate::FileDiagnostic {
                line: 4,
                kind: "note".to_owned(),
                message: "bad".to_owned(),
            }],
        ),
        Err(LanguagePluginError::DiagnosticLineOutOfRange { line: 4, .. })
    ));
    assert!(matches!(
        crate::validate_file_diagnostics(
            "sql",
            &path,
            3,
            &[crate::FileDiagnostic {
                line: 1,
                kind: "   ".to_owned(),
                message: "bad".to_owned(),
            }],
        ),
        Err(LanguagePluginError::InvalidDiagnostic { .. })
    ));
    assert!(matches!(
        crate::validate_file_diagnostics(
            "sql",
            &path,
            3,
            &[crate::FileDiagnostic {
                line: 1,
                kind: "note".to_owned(),
                message: "   ".to_owned(),
            }],
        ),
        Err(LanguagePluginError::InvalidDiagnostic { .. })
    ));
}
