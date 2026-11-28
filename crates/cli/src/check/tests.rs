use std::fs;
use std::process::Command;
use std::sync::Barrier;
use std::sync::atomic::{AtomicUsize, Ordering};

use codeint_config::{Configuration, FindingsShown, ReportFormat};
use codeint_git::Repository;
use codeint_model::language::{
    DeclaredDep, FileModel, FlaggedLine, Guest, ImportRequest, LowerError, Manifest,
    ReferenceRange, Resolution, SourceSet, WorkspaceError,
};
use codeint_model::{LanguageFacts, LanguagePlugin, Package, ReferenceRanges, RepoPath};
use serde_json::Value;
use tempfile::TempDir;

use super::{CheckError, NamedLanguagePlugin, check_with, measure_snapshot};

struct Alpha;

struct AlphaTwo;

struct Beta;

struct DuplicateExtension;

struct Malformed;

struct ChecksFolderIsTest;

struct FlagsEveryLineInOnePackage;

const ALPHA: NamedLanguagePlugin<'static> = NamedLanguagePlugin {
    name: "alpha",
    plugin: &Alpha,
};

const ALPHA_TWO: NamedLanguagePlugin<'static> = NamedLanguagePlugin {
    name: "alpha-two",
    plugin: &AlphaTwo,
};

const BETA: NamedLanguagePlugin<'static> = NamedLanguagePlugin {
    name: "beta",
    plugin: &Beta,
};

const DUPLICATE_EXTENSION: NamedLanguagePlugin<'static> = NamedLanguagePlugin {
    name: "duplicate-extension",
    plugin: &DuplicateExtension,
};

const MALFORMED: NamedLanguagePlugin<'static> = NamedLanguagePlugin {
    name: "malformed",
    plugin: &Malformed,
};

const FLAGS_EVERY_LINE_IN_ONE_PACKAGE: NamedLanguagePlugin<'static> = NamedLanguagePlugin {
    name: "flags-every-line",
    plugin: &FlagsEveryLineInOnePackage,
};

const CHECKS_FOLDER_IS_TEST: NamedLanguagePlugin<'static> = NamedLanguagePlugin {
    name: "checks-folder-is-test",
    plugin: &ChecksFolderIsTest,
};

fn lines_of(text: &str) -> Result<FileModel, LowerError> {
    lines_in(SourceSet::Production, text)
}

fn lines_in(source_set: SourceSet, text: &str) -> Result<FileModel, LowerError> {
    Ok(FileModel {
        source_set,
        source_lines: u32::try_from(text.lines().count()).map_err(|_| LowerError::TooLarge)?,
        functions: Vec::new(),
        tokens: Vec::new(),
        flagged_lines: Vec::new(),
        hazard_lines: Vec::new(),
        runtime_imports: Vec::new(),
        program_start: false,
    })
}

macro_rules! every_import_is_external {
    () => {
        every_import_is_external!(imports_only);

        fn has_hazard_rules() -> bool {
            false
        }

        fn hazard_rules() -> Vec<String> {
            Vec::new()
        }

        fn has_imports() -> bool {
            true
        }
    };
    (imports_only) => {
        fn manifest_paths(_repository_paths: Vec<String>) -> Vec<String> {
            Vec::new()
        }

        fn resolve_imports(
            _repository_paths: Vec<String>,
            _manifests: Vec<Manifest>,
            requests: Vec<ImportRequest>,
            _active_profiles: Vec<String>,
        ) -> Vec<Resolution> {
            requests.iter().map(|_| Resolution::Builtin).collect()
        }

        fn declared_dependencies(_manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
            Vec::new()
        }

        fn entry_points(_repository_paths: Vec<String>, _manifests: Vec<Manifest>) -> Vec<String> {
            Vec::new()
        }

        codeint_model::empty_framework_profile_guest_methods!();
    };
}

fn erosion_range(low: f64, high: f64, source: &str) -> Vec<ReferenceRange> {
    vec![ReferenceRange {
        metric: "erosion".to_owned(),
        metric_version: 1,
        low,
        high,
        source: source.to_owned(),
    }]
}

macro_rules! without_workspaces {
    () => {
        fn workspace_packages(_manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
            Ok(Vec::new())
        }
    };
}

macro_rules! panic_on_import_calls {
    () => {
        fn manifest_paths(_repository_paths: Vec<String>) -> Vec<String> {
            panic!("manifest_paths must not be called when has_imports is false");
        }

        fn workspace_packages(_manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
            panic!("workspace_packages must not be called when has_imports is false");
        }

        fn resolve_imports(
            _repository_paths: Vec<String>,
            _manifests: Vec<Manifest>,
            _requests: Vec<ImportRequest>,
            _active_profiles: Vec<String>,
        ) -> Vec<Resolution> {
            panic!("resolve_imports must not be called when has_imports is false");
        }

        fn declared_dependencies(_manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
            panic!("declared_dependencies must not be called when has_imports is false");
        }

        fn entry_points(_repository_paths: Vec<String>, _manifests: Vec<Manifest>) -> Vec<String> {
            panic!("entry_points must not be called when has_imports is false");
        }

        codeint_model::empty_framework_profile_guest_methods!();
    };
}

fn measure_test_snapshot(
    files: &[(&str, &str)],
    plugins: &[NamedLanguagePlugin<'_>],
) -> Result<super::MeasuredSnapshot, CheckError> {
    let folder = repository(files);
    let repo = Repository::discover(folder.path()).expect("open repository");
    let tree = repo.working_tree().expect("read working tree");
    measure_snapshot(&tree, plugins, &Configuration::default())
}

macro_rules! stub_language_plugin {
    ($name:ident, [$($ext:expr),+ $(,)?], $ranges:expr) => {
        impl Guest for $name {
            fn extensions() -> Vec<String> {
                vec![$($ext.to_owned()),+]
            }

            fn lower(_path: String, text: String) -> Result<FileModel, LowerError> {
                lines_of(&text)
            }

            without_workspaces!();

            fn reference_ranges() -> Vec<ReferenceRange> {
                $ranges
            }

            every_import_is_external!();
        }
    };
}

stub_language_plugin!(
    Alpha,
    ["alpha"],
    erosion_range(0.12, 0.56, "Alpha panel of 40 repositories, Table 3")
);
stub_language_plugin!(AlphaTwo, ["alpha"], Vec::new());
stub_language_plugin!(DuplicateExtension, ["dup", "dup"], Vec::new());
stub_language_plugin!(Beta, ["beta"], Vec::new());
stub_language_plugin!(
    Malformed,
    ["malformed"],
    erosion_range(0.7, 0.2, "Reversed")
);

impl Guest for FlagsEveryLineInOnePackage {
    fn extensions() -> Vec<String> {
        vec!["flagged".to_owned()]
    }

    fn lower(_path: String, text: String) -> Result<FileModel, LowerError> {
        let mut model = lines_of(&text)?;
        model.flagged_lines = (1..=model.source_lines)
            .map(|line| FlaggedLine {
                line,
                rule: "every-line".to_owned(),
            })
            .collect();
        Ok(model)
    }

    fn workspace_packages(_manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        Ok(vec!["packages/one".to_owned()])
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        Vec::new()
    }

    every_import_is_external!();
}

impl Guest for ChecksFolderIsTest {
    fn extensions() -> Vec<String> {
        vec!["checked".to_owned()]
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        if path.starts_with("checks/") {
            lines_in(SourceSet::Test, &text)
        } else {
            lines_of(&text)
        }
    }

    without_workspaces!();

    fn reference_ranges() -> Vec<ReferenceRange> {
        Vec::new()
    }

    every_import_is_external!();
}

fn repository(files: &[(&str, &str)]) -> TempDir {
    let folder = tempfile::tempdir().expect("create a temporary folder");
    let status = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(folder.path())
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .status()
        .expect("run git");
    assert!(status.success(), "git init exited with {status}");
    for (path, text) in files {
        let full = folder.path().join(path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).expect("create the parent folder");
        }
        fs::write(full, text).expect("write the file");
    }
    folder
}

fn report(files: &[(&str, &str)], plugins: &[NamedLanguagePlugin<'_>]) -> String {
    let folder = repository(files);
    check_with(folder.path(), plugins, FindingsShown::All)
        .expect("the check succeeds")
        .render(ReportFormat::Toon)
        .expect("the report renders")
}

#[test]
fn a_reference_range_that_the_only_measured_language_supplies_is_shown_with_its_source_label() {
    let report = report(&[("main.alpha", "one\ntwo\n")], &[ALPHA, BETA]);

    assert!(
        report.starts_with(concat!(
            "files: 1\n",
            "source_lines: 2\n",
            "source_sets:\n",
            "  production:\n",
            "    files: 1\n",
            "    source_lines: 2\n",
            "    erosion:\n",
            "      ratio: 0\n",
            "      metric_version: 1\n",
            "      reference_range:\n",
            "        low: 0.12\n",
            "        high: 0.56\n",
            "        source: \"Alpha panel of 40 repositories, Table 3\"\n",
            "      eroded_function_mass: 0\n",
        )),
        "unexpected report: {report}"
    );
    let other_metrics = report
        .lines()
        .filter(|line| line.starts_with("      reference_range:"))
        .skip(1)
        .collect::<Vec<_>>();
    assert!(!other_metrics.is_empty(), "unexpected report: {report}");
    assert!(
        other_metrics
            .iter()
            .all(|line| *line
                == "      reference_range: no published Reference range for this language"),
        "unexpected report: {report}"
    );
}

#[test]
fn a_reference_range_of_one_language_is_never_shown_for_files_of_another_language() {
    let report = report(&[("main.beta", "one\n")], &[ALPHA, BETA]);

    assert!(
        report
            .contains("\n      reference_range: no published Reference range for this language\n"),
        "unexpected report: {report}"
    );
    assert!(
        !report.contains("Alpha panel"),
        "unexpected report: {report}"
    );
}

#[test]
fn no_reference_range_is_shown_next_to_the_total_of_several_languages() {
    let report = report(
        &[("main.alpha", "one\n"), ("main.beta", "one\n")],
        &[ALPHA, BETA],
    );
    let (total, _) = report
        .split_once("\nlanguages:\n")
        .expect("the report has a section per language");

    assert!(
        total.contains(
            "\n      reference_range: \"not shown, because the measured files are in more than one language\"\n"
        ),
        "unexpected report: {report}"
    );
    assert!(
        !total.contains("Alpha panel"),
        "unexpected report: {report}"
    );
}

fn language_section<'report>(report: &'report str, plugin: &str) -> Vec<&'report str> {
    let heading = format!("  {plugin}:");
    report
        .lines()
        .skip_while(|line| *line != "languages:")
        .skip_while(|line| *line != heading)
        .enumerate()
        .take_while(|(index, line)| *index == 0 || line.starts_with("    "))
        .map(|(_, line)| line)
        .collect()
}

#[test]
fn the_report_shows_the_metrics_of_each_language_with_the_reference_ranges_of_that_language() {
    let report = report(
        &[
            ("main.alpha", "one\n"),
            ("src/second.alpha", "one\ntwo\n"),
            ("main.beta", "one\ntwo\nthree\n"),
        ],
        &[ALPHA, BETA],
    );

    let alpha = language_section(&report, "alpha");
    let beta = language_section(&report, "beta");

    assert_eq!(
        alpha[..7],
        [
            "  alpha:",
            "    files: 2",
            "    source_lines: 3",
            "    source_sets:",
            "      production:",
            "        files: 2",
            "        source_lines: 3",
        ],
        "unexpected report: {report}"
    );
    assert!(
        alpha.contains(&"            source: \"Alpha panel of 40 repositories, Table 3\""),
        "unexpected report: {report}"
    );
    assert_eq!(
        beta[..3],
        ["  beta:", "    files: 1", "    source_lines: 3"],
        "unexpected report: {report}"
    );
    assert!(
        !beta.iter().any(|line| line.contains("Alpha panel")),
        "unexpected report: {report}"
    );
    assert!(
        report.starts_with("files: 3\nsource_lines: 6\n"),
        "unexpected report: {report}"
    );
}

#[test]
fn a_report_of_one_measured_language_has_no_section_per_language() {
    let report = report(&[("main.alpha", "one\n")], &[ALPHA, BETA]);

    assert!(
        !report.contains("\nlanguages:"),
        "unexpected report: {report}"
    );
}

#[test]
fn a_languages_entry_can_name_any_language_plugin_and_assigns_it_the_folders_it_lists() {
    let report = report(
        &[
            (
                "codeint.json",
                r#"{ "languages": [
                    { "plugin": "alpha", "include": ["web"] },
                    { "plugin": "beta", "include": ["tools"] }
                ] }"#,
            ),
            ("web/main.alpha", "one\n"),
            ("tools/main.alpha", "one\ntwo\n"),
            ("tools/build.beta", "one\ntwo\nthree\n"),
            ("web/build.beta", "one\ntwo\nthree\nfour\n"),
        ],
        &[ALPHA, BETA],
    );

    assert_eq!(
        language_section(&report, "alpha")[..3],
        ["  alpha:", "    files: 1", "    source_lines: 1"],
        "unexpected report: {report}"
    );
    assert_eq!(
        language_section(&report, "beta")[..3],
        ["  beta:", "    files: 1", "    source_lines: 3"],
        "unexpected report: {report}"
    );
}

#[test]
fn an_invalid_reference_range_from_a_language_plugin_stops_the_check() {
    let folder = repository(&[("main.beta", "one\n")]);

    let result = check_with(folder.path(), &[BETA, MALFORMED], FindingsShown::All);

    assert!(
        matches!(result, Err(CheckError::ReferenceRange(_))),
        "unexpected result: {result:?}"
    );
}

fn source_set_sizes(report: &str) -> Vec<&str> {
    report
        .lines()
        .filter(|line| {
            let code = line.trim_start();
            match line.len() - code.len() {
                2 => true,
                4 => code.starts_with("files:") || code.starts_with("source_lines:"),
                _ => false,
            }
        })
        .collect()
}

#[test]
fn the_core_puts_each_file_in_the_source_set_that_its_language_plugin_names() {
    let report = report(
        &[
            ("checks/first.checked", "one\n"),
            ("checks/nested/second.checked", "one\ntwo\n"),
            ("src/main.checked", "one\ntwo\nthree\n"),
            ("src/main.test.checked", "one\ntwo\nthree\nfour\n"),
        ],
        &[CHECKS_FOLDER_IS_TEST],
    );

    assert_eq!(
        source_set_sizes(&report),
        [
            "  production:",
            "    files: 2",
            "    source_lines: 7",
            "  test:",
            "    files: 2",
            "    source_lines: 3",
        ],
        "unexpected report: {report}"
    );
}

#[test]
fn the_core_puts_a_file_under_a_vendor_or_third_party_folder_in_vendored_whatever_its_language_plugin_names()
 {
    let report = report(
        &[
            ("checks/vendor/first.checked", "one\n"),
            ("vendor/second.checked", "one\ntwo\n"),
            ("src/third_party/third.checked", "one\ntwo\nthree\n"),
            ("src/main.checked", "one\ntwo\nthree\nfour\n"),
        ],
        &[CHECKS_FOLDER_IS_TEST],
    );

    assert_eq!(
        source_set_sizes(&report),
        [
            "  production:",
            "    files: 1",
            "    source_lines: 4",
            "  vendored:",
            "    files: 3",
            "    source_lines: 6",
        ],
        "unexpected report: {report}"
    );
}

fn json_report(files: &[(&str, &str)], plugins: &[NamedLanguagePlugin<'_>]) -> Value {
    let folder = repository(files);
    let rendered = check_with(folder.path(), plugins, FindingsShown::All)
        .expect("the check succeeds")
        .render(ReportFormat::Json)
        .expect("the report renders");
    serde_json::from_str(&rendered).expect("the report is JSON")
}

fn report_with_a_production_verbosity_budget(budget: &str) -> Value {
    let configuration =
        format!(r#"{{ "budgets": {{ "production": {{ "verbosity": {budget} }} }} }}"#);
    json_report(
        &[
            ("codeint.json", &configuration),
            ("main.alpha", "one\n"),
            ("main.flagged", "one\n"),
        ],
        &[ALPHA, FLAGS_EVERY_LINE_IN_ONE_PACKAGE],
    )
}

#[test]
fn a_budget_applies_to_the_total_of_every_language_not_to_each_language() {
    let within = report_with_a_production_verbosity_budget("0.6");
    assert_eq!(
        within["languages"]["flags-every-line"]["source_sets"]["production"]["verbosity"]["ratio"],
        1.0,
        "unexpected report: {within}"
    );
    assert_eq!(
        within["source_sets"]["production"]["verbosity"]["ratio"], 0.5,
        "unexpected report: {within}"
    );
    assert!(
        within.get("over_budget").is_none(),
        "unexpected report: {within}"
    );

    let over = report_with_a_production_verbosity_budget("0.4");
    assert_eq!(
        over["over_budget"],
        serde_json::json!([
            { "source_set": "production", "metric": "verbosity", "ratio": 0.5, "budget": 0.4 }
        ]),
        "unexpected report: {over}"
    );
}

fn package_paths(source_sets: &Value) -> Vec<&str> {
    source_sets["production"]["packages"]
        .as_array()
        .map(|packages| {
            packages
                .iter()
                .filter_map(|package| package["path"].as_str())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn each_language_section_lists_the_workspace_packages_like_the_total() {
    let report = json_report(
        &[
            ("packages/one/main.alpha", "one\n"),
            ("main.alpha", "one\ntwo\n"),
            ("packages/one/main.flagged", "one\n"),
        ],
        &[ALPHA, FLAGS_EVERY_LINE_IN_ONE_PACKAGE],
    );

    assert_eq!(
        package_paths(&report["source_sets"]),
        [".", "packages/one"],
        "unexpected report: {report}"
    );
    for plugin in ["alpha", "flags-every-line"] {
        assert_eq!(
            package_paths(&report["languages"][plugin]["source_sets"]),
            [".", "packages/one"],
            "{plugin}: unexpected report: {report}"
        );
    }
    assert_eq!(
        report["languages"]["flags-every-line"]["source_sets"]["production"]["packages"][1]["files"],
        1,
        "unexpected report: {report}"
    );
}

#[test]
fn the_markdown_report_shows_the_metrics_and_packages_of_each_language_after_the_total() {
    let folder = repository(&[
        ("packages/one/main.alpha", "one\n"),
        ("main.alpha", "one\ntwo\n"),
        ("packages/one/main.flagged", "one\n"),
    ]);
    let report = check_with(
        folder.path(),
        &[ALPHA, FLAGS_EVERY_LINE_IN_ONE_PACKAGE],
        FindingsShown::All,
    )
    .expect("the check succeeds")
    .render(ReportFormat::Markdown)
    .expect("the report renders");

    let (_, alpha) = report
        .split_once("\n\n## Language plugin `alpha`\n\n")
        .expect("the report has a section for alpha");
    let (alpha, flagged) = alpha
        .split_once("\n\n## Language plugin `flags-every-line`\n\n")
        .expect("the report has a section for flags-every-line after alpha");
    let (flagged, _) = flagged
        .split_once("\n\n## Production Findings\n\n")
        .expect("the Findings follow the sections per language");

    assert!(
        alpha.starts_with(
            "codeint measured 2 files with 3 source lines.\n\nTest gap is not measured because no Coverage report was named.\n\n| Source set |"
        ),
        "unexpected report: {report}"
    );
    assert!(
        alpha.contains("| Production | Verbosity | 0 |"),
        "unexpected report: {report}"
    );
    assert!(
        alpha.ends_with("| Production | `packages/one` | 1 | 1 | 0 | 0 | 0 | 0 | 0 |"),
        "unexpected report: {report}"
    );
    assert!(
        flagged.starts_with(
            "codeint measured 1 file with 1 source line.\n\nTest gap is not measured because no Coverage report was named.\n\n| Source set |"
        ),
        "unexpected report: {report}"
    );
    assert!(
        flagged.contains("| Production | Verbosity | 1 |"),
        "unexpected report: {report}"
    );
    assert!(
        flagged.ends_with("| Production | `packages/one` | 1 | 1 | 0 | 0 | 0 | 1 | 0 |"),
        "unexpected report: {report}"
    );
}

#[test]
fn the_sarif_log_lists_each_finding_once_when_the_report_has_a_section_per_language() {
    let folder = repository(&[("main.alpha", "one\n"), ("main.flagged", "one\ntwo\n")]);
    let report = check_with(
        folder.path(),
        &[ALPHA, FLAGS_EVERY_LINE_IN_ONE_PACKAGE],
        FindingsShown::All,
    )
    .expect("the check succeeds");
    let json: Value = serde_json::from_str(
        &report
            .render(ReportFormat::Json)
            .expect("the report renders"),
    )
    .expect("the report is JSON");
    assert!(json.get("languages").is_some(), "unexpected report: {json}");

    let log: Value = serde_json::from_str(
        &report
            .render(ReportFormat::Sarif)
            .expect("the SARIF log renders"),
    )
    .expect("the SARIF log is JSON");
    let lines: Vec<u64> = log["runs"][0]["results"]
        .as_array()
        .expect("a list of results")
        .iter()
        .filter_map(|result| {
            result["locations"][0]["physicalLocation"]["region"]["startLine"].as_u64()
        })
        .collect();

    assert_eq!(lines, [1, 2], "unexpected SARIF log: {log}");
}

struct ParallelProofPlugin {
    arrivals: AtomicUsize,
    barrier: Barrier,
    expected: usize,
    fail_entry_points: bool,
}

impl ParallelProofPlugin {
    fn new(expected: usize) -> ParallelProofPlugin {
        ParallelProofPlugin {
            arrivals: AtomicUsize::new(0),
            barrier: Barrier::new(expected),
            expected,
            fail_entry_points: false,
        }
    }

    fn failing_entry_points() -> ParallelProofPlugin {
        ParallelProofPlugin {
            arrivals: AtomicUsize::new(0),
            barrier: Barrier::new(1),
            expected: 0,
            fail_entry_points: true,
        }
    }
}

impl LanguagePlugin for ParallelProofPlugin {
    codeint_model::empty_framework_profile_plugin_methods!();

    fn extensions(&self) -> Vec<String> {
        vec!["parallel".to_owned()]
    }

    fn lower(&self, path: &RepoPath, text: &str) -> Result<FileModel, codeint_model::LowerError> {
        if self.arrivals.fetch_add(1, Ordering::SeqCst) < self.expected {
            self.barrier.wait();
        }
        if path.as_str().starts_with("decl_") {
            return Err(codeint_model::LowerError::DeclarationOnly);
        }
        if path.as_str().starts_with("bad_") {
            return Err(codeint_model::LowerError::Unparsable {
                message: format!("syntax error in {path}"),
            });
        }
        let mut model = lines_of(text).map_err(codeint_model::LowerError::from)?;
        model.flagged_lines.push(FlaggedLine {
            line: 1,
            rule: "parallel-marker".to_owned(),
        });
        Ok(model)
    }

    fn manifest_paths(
        &self,
        _repository_paths: &[RepoPath],
    ) -> Result<Vec<String>, codeint_model::LanguagePluginError> {
        Ok(Vec::new())
    }

    fn workspace_packages(
        &self,
        _manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, codeint_model::WorkspaceError> {
        Ok(Vec::new())
    }

    fn resolve_imports(
        &self,
        _repository_paths: &[RepoPath],
        _manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
        _active_profiles: &[String],
    ) -> Result<Vec<Resolution>, codeint_model::LanguagePluginError> {
        Ok(requests.iter().map(|_| Resolution::Builtin).collect())
    }

    fn declared_dependencies(
        &self,
        _manifests: Vec<Manifest>,
    ) -> Result<Vec<DeclaredDep>, codeint_model::LanguagePluginError> {
        Ok(Vec::new())
    }

    fn entry_points(
        &self,
        _repository_paths: &[RepoPath],
        _manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, codeint_model::LanguagePluginError> {
        if self.fail_entry_points {
            return Err(codeint_model::LanguagePluginError::InvalidEntryPointPath {
                path: "../outside.parallel".to_owned(),
            });
        }
        Ok(Vec::new())
    }

    fn reference_ranges(&self) -> Result<ReferenceRanges, codeint_model::LanguagePluginError> {
        Ok(ReferenceRanges::default())
    }

    fn has_hazard_rules(&self) -> bool {
        false
    }

    fn hazard_rules(&self) -> Vec<String> {
        Vec::new()
    }

    fn has_imports(&self) -> bool {
        true
    }
}

#[test]
fn check_lowers_files_in_parallel_and_preserves_deterministic_order() {
    let plugin = ParallelProofPlugin::new(2);
    let named = [NamedLanguagePlugin {
        name: "parallel",
        plugin: &plugin,
    }];
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .expect("build two-thread rayon pool");
    let report = pool.install(|| {
        json_report(
            &[
                ("z_file.parallel", "one\n"),
                ("a_file.parallel", "one\n"),
                ("m_file.parallel", "one\n"),
                ("decl_b.parallel", "one\n"),
                ("decl_a.parallel", "one\n"),
                ("bad_b.parallel", "one\n"),
                ("bad_a.parallel", "one\n"),
            ],
            &named,
        )
    });

    assert_eq!(report["files"], 3, "unexpected report: {report}");
    let finding_files: Vec<&str> = report["source_sets"]["production"]["verbosity"]["findings"]
        .as_array()
        .expect("verbosity findings array")
        .iter()
        .filter_map(|finding| finding["path"].as_str())
        .collect();
    assert_eq!(
        finding_files,
        ["a_file.parallel", "m_file.parallel", "z_file.parallel"],
        "unexpected findings order: {report}"
    );
    let skipped_files: Vec<&str> = report["skipped"]
        .as_array()
        .expect("skipped array")
        .iter()
        .filter_map(|skipped| skipped["path"].as_str())
        .collect();
    assert_eq!(
        skipped_files,
        ["bad_a.parallel", "bad_b.parallel"],
        "unexpected skipped order: {report}"
    );
    let declaration_only: Vec<&str> = report["declaration_only"]
        .as_array()
        .expect("declaration_only array")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(
        declaration_only,
        ["decl_a.parallel", "decl_b.parallel"],
        "unexpected declaration_only order: {report}"
    );
}

#[test]
fn two_language_plugins_claiming_one_extension_without_languages_stops_with_an_error() {
    let folder = repository(&[("main.alpha", "one\n")]);

    let error = check_with(folder.path(), &[ALPHA_TWO, ALPHA], FindingsShown::All)
        .expect_err("two plugins claiming .alpha without languages must fail");

    assert!(
        matches!(error, CheckError::ExtensionConflict { .. }),
        "unexpected error: {error:?}"
    );
    let message = error.to_string();
    let alpha_pos = message.find("alpha").expect("error names the first plugin");
    let alpha_two_pos = message
        .find("alpha-two")
        .expect("error names the second plugin");
    assert!(
        alpha_pos < alpha_two_pos,
        "expected deterministic sorted plugin order, got: {message}"
    );
    assert!(
        message.contains(".alpha") && message.contains("languages"),
        "expected extension and languages setting in error, got: {message}"
    );
}

#[test]
fn two_language_plugins_claiming_one_extension_in_overlapping_languages_directories_stops_with_an_error()
 {
    let folder = repository(&[
        (
            "codeint.json",
            r#"{ "languages": [
                { "plugin": "alpha-two", "include": ["src"] },
                { "plugin": "alpha", "include": ["src/sub"] }
            ] }"#,
        ),
        ("src/sub/main.alpha", "one\n"),
    ]);

    let error = check_with(folder.path(), &[ALPHA_TWO, ALPHA], FindingsShown::All)
        .expect_err("overlapping languages entries claiming .alpha must fail");

    assert!(
        matches!(error, CheckError::ExtensionConflict { .. }),
        "unexpected error: {error:?}"
    );
    let message = error.to_string();
    assert!(
        message.contains("alpha")
            && message.contains("alpha-two")
            && message.contains(".alpha")
            && message.contains("src/sub/main.alpha")
            && message.contains("languages"),
        "unexpected error message: {message}"
    );
}

#[test]
fn assigning_separate_directories_in_languages_resolves_an_extension_conflict() {
    let report = json_report(
        &[
            (
                "codeint.json",
                r#"{ "languages": [
                    { "plugin": "alpha", "include": ["web"] },
                    { "plugin": "alpha-two", "include": ["tools"] }
                ] }"#,
            ),
            ("web/main.alpha", "one\n"),
            ("tools/main.alpha", "one\ntwo\n"),
        ],
        &[ALPHA, ALPHA_TWO],
    );

    assert_eq!(report["files"], 2, "unexpected report: {report}");
    assert_eq!(
        report["languages"]["alpha"]["files"], 1,
        "unexpected report: {report}"
    );
    assert_eq!(
        report["languages"]["alpha"]["source_lines"], 1,
        "unexpected report: {report}"
    );
    assert_eq!(
        report["languages"]["alpha-two"]["files"], 1,
        "unexpected report: {report}"
    );
    assert_eq!(
        report["languages"]["alpha-two"]["source_lines"], 2,
        "unexpected report: {report}"
    );
}

#[test]
fn excluding_a_nested_directory_from_one_plugin_resolves_an_extension_conflict() {
    let report = json_report(
        &[
            (
                "codeint.json",
                r#"{ "languages": [
                    { "plugin": "alpha", "include": ["src"], "exclude": ["src/legacy"] },
                    { "plugin": "alpha-two", "include": ["src/legacy"] }
                ] }"#,
            ),
            ("src/app.alpha", "one\n"),
            ("src/legacy/old.alpha", "one\ntwo\nthree\n"),
        ],
        &[ALPHA, ALPHA_TWO],
    );

    assert_eq!(report["files"], 2, "unexpected report: {report}");
    assert_eq!(
        report["languages"]["alpha"]["files"], 1,
        "unexpected report: {report}"
    );
    assert_eq!(
        report["languages"]["alpha"]["source_lines"], 1,
        "unexpected report: {report}"
    );
    assert_eq!(
        report["languages"]["alpha-two"]["files"], 1,
        "unexpected report: {report}"
    );
    assert_eq!(
        report["languages"]["alpha-two"]["source_lines"], 3,
        "unexpected report: {report}"
    );
}

#[test]
fn a_single_plugin_listing_one_extension_twice_does_not_conflict_with_itself() {
    let report = report(&[("main.dup", "one\n")], &[DUPLICATE_EXTENSION]);

    assert!(
        report.starts_with("files: 1\nsource_lines: 1\n"),
        "unexpected report: {report}"
    );
}

#[test]
fn an_invalid_entry_point_from_a_language_plugin_stops_the_check() {
    let folder = repository(&[("main.parallel", "one\n")]);
    let plugin = ParallelProofPlugin::failing_entry_points();
    let named = NamedLanguagePlugin {
        name: "bad-entry",
        plugin: &plugin,
    };

    let result = check_with(folder.path(), &[named], FindingsShown::All);

    assert!(
        matches!(
            result,
            Err(CheckError::LanguagePlugin(ref scoped))
                if matches!(
                    scoped.error,
                    codeint_model::LanguagePluginError::InvalidEntryPointPath { .. }
                )
        ),
        "unexpected result: {result:?}"
    );
}

#[test]
fn extension_conflict_error_and_remedy_name_codeint_jsonc_when_active() {
    let folder = repository(&[
        (
            "codeint.jsonc",
            r#"{
                // overlapping language globs
                "languages": [
                    { "plugin": "alpha-two", "include": ["src"] },
                    { "plugin": "alpha", "include": ["src/sub"] },
                ],
            }"#,
        ),
        ("src/sub/main.alpha", "one\n"),
    ]);

    let error = check_with(folder.path(), &[ALPHA_TWO, ALPHA], FindingsShown::All)
        .expect_err("overlapping languages entries claiming .alpha must fail");

    let message = error.to_string();
    assert!(
        message.contains("codeint.jsonc")
            && !message
                .replace("codeint.jsonc", "")
                .contains("codeint.json"),
        "expected codeint.jsonc in error message, got: {message}"
    );
    let remedy = crate::remedy::remedy(&crate::CliError::Check(error));
    assert!(
        remedy.contains("codeint.jsonc")
            && !remedy.replace("codeint.jsonc", "").contains("codeint.json"),
        "expected codeint.jsonc in remedy, got: {remedy}"
    );
}

#[test]
fn measure_snapshot_populates_per_language_coverage_and_hazard_rule_statements() {
    struct WithHazardRules;

    impl Guest for WithHazardRules {
        fn extensions() -> Vec<String> {
            vec!["haz".to_owned()]
        }

        fn lower(_path: String, text: String) -> Result<FileModel, LowerError> {
            lines_of(&text)
        }

        without_workspaces!();

        fn reference_ranges() -> Vec<ReferenceRange> {
            Vec::new()
        }

        every_import_is_external!(imports_only);

        fn has_hazard_rules() -> bool {
            true
        }

        fn hazard_rules() -> Vec<String> {
            vec![
                "delete-without-where".to_owned(),
                "leading-wildcard-like".to_owned(),
            ]
        }

        fn has_imports() -> bool {
            true
        }
    }

    const WITH_HAZARD_RULES: NamedLanguagePlugin<'static> = NamedLanguagePlugin {
        name: "hazard-lang",
        plugin: &WithHazardRules,
    };

    let snapshot = measure_test_snapshot(
        &[("main.alpha", "one\n"), ("query.haz", "two\n")],
        &[ALPHA, WITH_HAZARD_RULES],
    )
    .expect("measure snapshot succeeds");

    let facts_by_plugin: Vec<(&str, &[LanguageFacts])> = snapshot
        .languages
        .iter()
        .map(|language| {
            (
                language.plugin.as_str(),
                language.model.languages.as_slice(),
            )
        })
        .collect();
    assert_eq!(
        facts_by_plugin,
        vec![
            (
                "alpha",
                &[LanguageFacts {
                    language: "alpha".to_owned(),
                    has_coverage: false,
                    has_hazard_rules: false,
                    has_imports: true,
                }][..]
            ),
            (
                "hazard-lang",
                &[LanguageFacts {
                    language: "hazard-lang".to_owned(),
                    has_coverage: false,
                    has_hazard_rules: true,
                    has_imports: true,
                }][..]
            ),
        ]
    );
}

const TS_JS_RANGE: &str = "codeint stratified panel of 100 TypeScript and JavaScript repositories, ADR 0035, codeint 0.8.0";
const VUE2_RANGE: &str =
    "codeint stratified panel of 50 Vue 2 repositories, ADR 0035, codeint 0.8.0";
const VUE3_RANGE: &str =
    "codeint stratified panel of 50 Vue 3 repositories, ADR 0035, codeint 0.8.0";
const REACT_RANGE: &str =
    "codeint stratified panel of 50 React repositories, ADR 0035, codeint 0.8.0";

fn package_reference_range_source(repo: &std::path::Path, pkg: &str) -> String {
    let report = super::check(
        repo,
        super::CheckScope {
            findings: FindingsShown::All,
            package: Some(pkg),
            owner: None,
            coverage_reports: &[],
            ignore_baseline: false,
        },
        &codeint_config::PluginHostConfig::default(),
    )
    .expect("package check succeeds");
    let rendered = report.render(ReportFormat::Json).expect("render json");
    let parsed: Value = serde_json::from_str(&rendered).expect("parse json");
    parsed["source_sets"]["production"]["erosion"]["reference_range"]["source"]
        .as_str()
        .expect("reference range source")
        .to_owned()
}

fn assert_package_ranges(repo: &std::path::Path, cases: &[(&str, &str)]) {
    for &(pkg, expected) in cases {
        assert_eq!(package_reference_range_source(repo, pkg), expected, "{pkg}");
    }
}

#[test]
fn react_reference_ranges_select_adr_0020_and_preserve_vue_and_hybrid_precedence() {
    let ts_plugin = [codeint_plugin_host::built_in_language_plugin(
        codeint_plugin_host::TYPESCRIPT_JAVASCRIPT_PLUGIN_NAME,
    )
    .expect("built-in typescript-javascript plugin")];

    type RangeCase<'a> = (&'a str, &'a [(&'a str, &'a str)], &'a str);
    let cases: &[RangeCase<'_>] = &[
        (
            "react-tsx",
            &[
                ("package.json", r#"{"dependencies":{"react":"^18.3.1"}}"#),
                (
                    "src/App.tsx",
                    "export function App() { return <div>hello</div>; }\n",
                ),
            ],
            REACT_RANGE,
        ),
        (
            "react-jsx",
            &[
                ("package.json", r#"{"dependencies":{"react":"^18.3.1"}}"#),
                (
                    "src/App.jsx",
                    "export function App() { return <div>hi</div>; }\n",
                ),
            ],
            REACT_RANGE,
        ),
        (
            "vue-with-tsx",
            &[
                ("package.json", r#"{"dependencies":{"vue":"^3.4.0"}}"#),
                ("src/App.vue", "<script setup>\nconst x = 1;\n</script>\n"),
                ("src/Icon.tsx", "export const Icon = () => <span />;\n"),
            ],
            VUE3_RANGE,
        ),
        (
            "hybrid-vue-react",
            &[
                (
                    "package.json",
                    r#"{"dependencies":{"react":"^18.3.1","vue":"^3.4.0"}}"#,
                ),
                ("src/App.vue", "<script setup>\nconst x = 1;\n</script>\n"),
                (
                    "src/Widget.tsx",
                    "export function Widget() { return <div />; }\n",
                ),
            ],
            TS_JS_RANGE,
        ),
        (
            "hybrid-unparsed-vue",
            &[
                (
                    "package.json",
                    r#"{"dependencies":{"react":"^18.3.1","vue":"latest"}}"#,
                ),
                ("src/App.vue", "<script setup>\nconst x = 1;\n</script>\n"),
                ("src/Widget.tsx", "export const Widget = () => <div />;\n"),
            ],
            TS_JS_RANGE,
        ),
        (
            "plain-ts-with-react",
            &[
                ("package.json", r#"{"dependencies":{"react":"^18.3.1"}}"#),
                ("src/util.ts", "export const answer = 42;\n"),
            ],
            TS_JS_RANGE,
        ),
    ];
    for &(label, files, expected) in cases {
        let report = json_report(files, &ts_plugin);
        assert_eq!(
            report["source_sets"]["production"]["erosion"]["reference_range"]["source"], expected,
            "{label}"
        );
    }

    let monorepo = repository(&[
        (
            "package.json",
            r#"{"private":true,"workspaces":["packages/*"]}"#,
        ),
        (
            "packages/react-app/package.json",
            r#"{"name":"react-app","dependencies":{"react":"^18.3.1"}}"#,
        ),
        (
            "packages/react-app/src/App.tsx",
            "export const App = () => <main />;\n",
        ),
        (
            "packages/vue-app/package.json",
            r#"{"name":"vue-app","dependencies":{"vue":"^3.4.0"}}"#,
        ),
        (
            "packages/vue-app/src/App.vue",
            "<script setup>\nconst x = 1;\n</script>\n",
        ),
        ("packages/api/package.json", r#"{"name":"api"}"#),
        ("packages/api/src/index.ts", "export const ok = true;\n"),
    ]);
    assert_package_ranges(
        monorepo.path(),
        &[
            ("packages/react-app", REACT_RANGE),
            ("packages/vue-app", VUE3_RANGE),
            ("packages/api", TS_JS_RANGE),
        ],
    );
}

#[test]
fn no_imports_language_plugin_never_has_import_manifest_or_entry_point_functions_called() {
    struct NoImports;

    impl Guest for NoImports {
        fn extensions() -> Vec<String> {
            vec!["noimp".to_owned()]
        }

        fn lower(_path: String, text: String) -> Result<FileModel, LowerError> {
            let mut model = lines_of(&text)?;
            model.runtime_imports.push("./never-resolved".to_owned());
            model.program_start = true;
            Ok(model)
        }

        panic_on_import_calls!();

        fn reference_ranges() -> Vec<ReferenceRange> {
            Vec::new()
        }

        fn has_hazard_rules() -> bool {
            false
        }

        fn hazard_rules() -> Vec<String> {
            Vec::new()
        }

        fn has_imports() -> bool {
            false
        }
    }

    const NO_IMPORTS: NamedLanguagePlugin<'static> = NamedLanguagePlugin {
        name: "no-imports",
        plugin: &NoImports,
    };

    let snapshot = measure_test_snapshot(
        &[("main.alpha", "one\n"), ("query.noimp", "two\n")],
        &[ALPHA, NO_IMPORTS],
    )
    .expect("measure snapshot skips import, manifest, and entry point calls");

    let no_imports_lang = snapshot
        .languages
        .iter()
        .find(|language| language.plugin == "no-imports")
        .expect("no-imports language measured");
    assert_eq!(no_imports_lang.model.files.len(), 1);
    assert!(!no_imports_lang.model.files[0].model.program_start);
    assert!(no_imports_lang.model.runtime_imports.is_empty());
    assert!(no_imports_lang.model.declared_dependencies.is_empty());
    assert!(no_imports_lang.model.dependency_manifests.is_empty());
    assert!(no_imports_lang.model.manifest_used_dependencies.is_empty());
    assert_eq!(
        no_imports_lang.model.languages,
        vec![LanguageFacts {
            language: "no-imports".to_owned(),
            has_coverage: false,
            has_hazard_rules: false,
            has_imports: false,
        }]
    );
}

#[test]
fn package_scoped_reference_ranges_prefer_member_manifests_over_root() {
    let vue_script = "<script setup>\nconst x = 1;\n</script>\n";
    let tsx_main = "export const App = () => <main />;\n";
    let monorepo = repository(&[
        (
            "package.json",
            r#"{"private":true,"workspaces":["packages/*"],"dependencies":{"vue":"^3.4.0"},"devDependencies":{"react":"^18.3.1"}}"#,
        ),
        (
            "packages/legacy/package.json",
            r#"{"name":"legacy","dependencies":{"vue":"^2.7.16"}}"#,
        ),
        (
            "packages/legacy/src/App.vue",
            "<script>\nexport default {};\n</script>\n",
        ),
        (
            "packages/vue-jsx/package.json",
            r#"{"name":"vue-jsx","dependencies":{"vue":"^3.4.0"}}"#,
        ),
        ("packages/vue-jsx/src/App.vue", vue_script),
        (
            "packages/vue-jsx/src/Icon.tsx",
            "export const Icon = () => <span />;\n",
        ),
        (
            "packages/hybrid/package.json",
            r#"{"name":"hybrid","dependencies":{"vue":"^3.4.0","react":"^18.3.1"}}"#,
        ),
        ("packages/hybrid/src/App.vue", vue_script),
        (
            "packages/hybrid/src/Widget.tsx",
            "export const Widget = () => <div />;\n",
        ),
        (
            "packages/hoisted-react/package.json",
            r#"{"name":"hoisted-react"}"#,
        ),
        ("packages/hoisted-react/src/App.tsx", tsx_main),
        (
            "packages/hoisted-vue/package.json",
            r#"{"name":"hoisted-vue"}"#,
        ),
        ("packages/hoisted-vue/src/App.vue", vue_script),
        (
            "packages/local-signal-react/package.json",
            r#"{"name":"local-signal-react"}"#,
        ),
        ("packages/local-signal-react/index.html", "<html></html>\n"),
        ("packages/local-signal-react/src/main.tsx", tsx_main),
        (
            "packages/local-signal-vue2/package.json",
            r#"{"name":"local-signal-vue2"}"#,
        ),
        (
            "packages/local-signal-vue2/vue.config.js",
            "module.exports = {};\n",
        ),
        (
            "packages/local-signal-vue2/src/App.vue",
            "<script>\nexport default {};\n</script>\n",
        ),
        ("packages/plain-api/package.json", r#"{"name":"plain-api"}"#),
        (
            "packages/plain-api/src/index.ts",
            "export const ready = true;\n",
        ),
    ]);
    assert_package_ranges(
        monorepo.path(),
        &[
            ("packages/legacy", VUE2_RANGE),
            ("packages/vue-jsx", VUE3_RANGE),
            ("packages/hybrid", TS_JS_RANGE),
            ("packages/hoisted-react", TS_JS_RANGE),
            ("packages/hoisted-vue", TS_JS_RANGE),
            ("packages/local-signal-react", REACT_RANGE),
            ("packages/local-signal-vue2", VUE2_RANGE),
            ("packages/plain-api", TS_JS_RANGE),
        ],
    );
}

#[test]
fn lowering_clears_language_plugin_coverage_and_unmeasured_plugins_emit_no_language_facts() {
    struct SpoofingCoveragePlugin;

    impl Guest for SpoofingCoveragePlugin {
        every_import_is_external!();
        without_workspaces!();

        fn lower(_path: String, text: String) -> Result<FileModel, LowerError> {
            let functions = vec![codeint_model::language::Function {
                line: 1,
                end_line: 1,
                decision_points: 1,
                source_lines: 1,
                name: Some("spoofed".to_owned()),
                cognitive_complexity: 0,
                coverage: Some(codeint_model::language::FunctionCoverage {
                    listed_lines: 10,
                    executed_lines: 10,
                }),
            }];
            Ok(FileModel {
                functions,
                ..lines_of(&text)?
            })
        }

        fn extensions() -> Vec<String> {
            vec![String::from("cov")]
        }

        fn reference_ranges() -> Vec<ReferenceRange> {
            Vec::new()
        }
    }

    const SPOOF: NamedLanguagePlugin<'static> = NamedLanguagePlugin {
        name: "spoof-cov",
        plugin: &SpoofingCoveragePlugin,
    };

    let snapshot = measure_test_snapshot(&[("src/main.cov", "fn spoofed() {}\n")], &[ALPHA, SPOOF])
        .expect("measure snapshot");

    let alpha = snapshot
        .languages
        .iter()
        .find(|language| language.plugin == "alpha")
        .expect("alpha entry");
    assert!(alpha.model.files.is_empty());
    assert!(alpha.model.languages.is_empty());

    let spoof = snapshot
        .languages
        .iter()
        .find(|language| language.plugin == "spoof-cov")
        .expect("spoof-cov entry");
    assert_eq!(spoof.model.languages.len(), 1);
    assert_eq!(spoof.model.files[0].model.functions[0].coverage, None);

    let mut filtered = spoof.model.clone();
    super::filter_model_to_package(
        &mut filtered,
        &super::SelectedPackage::Member(RepoPath::new("packages/other").expect("path")),
    );
    assert!(filtered.files.is_empty());
    assert!(filtered.languages.is_empty());
}

struct PickyManifests;

fn assert_no_bad_manifest(manifests: &[Manifest]) {
    assert!(
        manifests
            .iter()
            .all(|manifest| manifest.text.trim() != "bad")
    );
}

fn empty_after_checking<Item>(manifests: &[Manifest]) -> Vec<Item> {
    assert_no_bad_manifest(manifests);
    Vec::new()
}

fn fatal_picky_error(manifest_text: &str) -> codeint_model::WorkspaceError {
    match measure_test_snapshot(
        &[("a/manifest.picky", manifest_text), ("a/a.picky", "x\n")],
        &[PICKY_MANIFESTS],
    ) {
        Err(CheckError::Workspace(error)) => error,
        other => panic!("expected a fatal workspace error, got {:?}", other.err()),
    }
}

const PICKY_MANIFESTS: NamedLanguagePlugin<'static> = NamedLanguagePlugin {
    name: "picky",
    plugin: &PickyManifests,
};

impl Guest for PickyManifests {
    fn extensions() -> Vec<String> {
        vec!["picky".to_owned()]
    }

    fn lower(_path: String, text: String) -> Result<FileModel, LowerError> {
        lines_of(&text)
    }

    fn manifest_paths(repository_paths: Vec<String>) -> Vec<String> {
        repository_paths
            .into_iter()
            .filter(|path| path.ends_with("manifest.picky"))
            .collect()
    }

    fn workspace_packages(manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        for manifest in &manifests {
            match manifest.text.trim() {
                "bad" => {
                    return Err(WorkspaceError {
                        path: manifest.path.clone(),
                        message: "bad manifest".to_owned(),
                    });
                }
                "unnamed" => {
                    return Err(WorkspaceError {
                        path: "elsewhere/manifest.picky".to_owned(),
                        message: "names a path that was not passed".to_owned(),
                    });
                }
                "escape" => return Ok(vec!["../escape".to_owned()]),
                _ => {}
            }
        }
        Ok(manifests
            .iter()
            .filter_map(|manifest| manifest.path.strip_suffix("/manifest.picky"))
            .map(str::to_owned)
            .collect())
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        Vec::new()
    }

    fn resolve_imports(
        _repository_paths: Vec<String>,
        manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
        _active_profiles: Vec<String>,
    ) -> Vec<Resolution> {
        assert_no_bad_manifest(&manifests);
        requests.iter().map(|_| Resolution::Builtin).collect()
    }

    fn declared_dependencies(manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
        empty_after_checking(&manifests)
    }

    fn entry_points(_repository_paths: Vec<String>, manifests: Vec<Manifest>) -> Vec<String> {
        empty_after_checking(&manifests)
    }

    fn has_hazard_rules() -> bool {
        false
    }

    fn hazard_rules() -> Vec<String> {
        Vec::new()
    }

    fn has_imports() -> bool {
        true
    }

    codeint_model::empty_framework_profile_guest_methods!();
}

#[test]
fn invalid_manifest_is_dropped_with_a_warning_and_the_rest_is_measured() {
    let snapshot = measure_test_snapshot(
        &[
            ("good/manifest.picky", "ok"),
            ("good/a.picky", "one\n"),
            ("broken/manifest.picky", "bad"),
            ("broken/b.picky", "two\ntwo\n"),
        ],
        &[PICKY_MANIFESTS],
    )
    .expect("an invalid manifest does not stop the measurement");

    assert_eq!(snapshot.manifest_warnings.len(), 1);
    assert!(
        snapshot.manifest_warnings[0].contains("broken/manifest.picky")
            && snapshot.manifest_warnings[0].contains("bad manifest"),
        "{:?}",
        snapshot.manifest_warnings
    );
    let picky = &snapshot.languages[0];
    assert_eq!(picky.model.files.len(), 4);
    let broken = RepoPath::new("broken/b.picky").expect("path");
    assert_eq!(picky.model.workspace.package_of(&broken), Package::Root);
    let members: Vec<&str> = picky
        .model
        .workspace
        .member_roots()
        .map(RepoPath::as_str)
        .collect();
    assert_eq!(members, ["good"]);
}

#[test]
fn every_invalid_manifest_is_dropped_with_its_own_warning() {
    let snapshot = measure_test_snapshot(
        &[
            ("one/manifest.picky", "bad"),
            ("two/manifest.picky", "bad"),
            ("two/a.picky", "x\n"),
        ],
        &[PICKY_MANIFESTS],
    )
    .expect("invalid manifests do not stop the measurement");

    assert_eq!(snapshot.manifest_warnings.len(), 2);
    assert!(snapshot.manifest_warnings[0].contains("one/manifest.picky"));
    assert!(snapshot.manifest_warnings[1].contains("two/manifest.picky"));
}

#[test]
fn workspace_error_naming_a_path_that_was_not_passed_stays_fatal() {
    let error = fatal_picky_error("unnamed");

    assert!(matches!(
        &error,
        codeint_model::WorkspaceError::Manifest { path, .. } if path == "elsewhere/manifest.picky"
    ));
}

#[test]
fn workspace_error_that_is_not_a_manifest_error_stays_fatal() {
    assert!(matches!(
        fatal_picky_error("escape"),
        codeint_model::WorkspaceError::InvalidPackageRoot(_)
    ));
}
