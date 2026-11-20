use std::fs;
use std::process::Command;

use codeint_config::{FindingsShown, ReportFormat};
use codeint_model::language::{
    FileModel, FlaggedLine, Guest, ImportRequest, LowerError, Manifest, ReferenceRange, Resolution,
    SourceSet, WorkspaceError,
};
use serde_json::Value;
use tempfile::TempDir;

use super::{CheckError, NamedLanguagePlugin, check_with};

struct Alpha;

struct Beta;

struct Malformed;

struct ChecksFolderIsTest;

struct FlagsEveryLineInOnePackage;

const ALPHA: NamedLanguagePlugin<'static> = NamedLanguagePlugin {
    name: "alpha",
    plugin: &Alpha,
};

const BETA: NamedLanguagePlugin<'static> = NamedLanguagePlugin {
    name: "beta",
    plugin: &Beta,
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
        runtime_imports: Vec::new(),
    })
}

macro_rules! every_import_is_external {
    () => {
        fn manifest_paths(_repository_paths: Vec<String>) -> Vec<String> {
            Vec::new()
        }

        fn resolve_imports(
            _repository_paths: Vec<String>,
            _manifests: Vec<Manifest>,
            requests: Vec<ImportRequest>,
        ) -> Vec<Resolution> {
            requests.iter().map(|_| Resolution::External).collect()
        }
    };
}

fn erosion_range(low: f64, high: f64, source: &str) -> Vec<ReferenceRange> {
    vec![ReferenceRange {
        metric: "erosion".to_owned(),
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

impl Guest for Alpha {
    fn extensions() -> Vec<String> {
        vec!["alpha".to_owned()]
    }

    fn lower(_path: String, text: String) -> Result<FileModel, LowerError> {
        lines_of(&text)
    }

    without_workspaces!();

    fn reference_ranges() -> Vec<ReferenceRange> {
        erosion_range(0.12, 0.56, "Alpha panel of 40 repositories, Table 3")
    }

    every_import_is_external!();
}

impl Guest for Beta {
    fn extensions() -> Vec<String> {
        vec!["beta".to_owned()]
    }

    fn lower(_path: String, text: String) -> Result<FileModel, LowerError> {
        lines_of(&text)
    }

    without_workspaces!();

    fn reference_ranges() -> Vec<ReferenceRange> {
        Vec::new()
    }

    every_import_is_external!();
}

impl Guest for Malformed {
    fn extensions() -> Vec<String> {
        vec!["malformed".to_owned()]
    }

    fn lower(_path: String, text: String) -> Result<FileModel, LowerError> {
        lines_of(&text)
    }

    without_workspaces!();

    fn reference_ranges() -> Vec<ReferenceRange> {
        erosion_range(0.7, 0.2, "Reversed")
    }

    every_import_is_external!();
}

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
        alpha.starts_with("codeint measured 2 files with 3 source lines.\n\n| Source set |"),
        "unexpected report: {report}"
    );
    assert!(
        alpha.contains("| Production | Verbosity | 0 |"),
        "unexpected report: {report}"
    );
    assert!(
        alpha.ends_with("| Production | `packages/one` | 1 | 1 | 0 | 0 | 0 | 0 |"),
        "unexpected report: {report}"
    );
    assert!(
        flagged.starts_with("codeint measured 1 file with 1 source line.\n\n| Source set |"),
        "unexpected report: {report}"
    );
    assert!(
        flagged.contains("| Production | Verbosity | 1 |"),
        "unexpected report: {report}"
    );
    assert!(
        flagged.ends_with("| Production | `packages/one` | 1 | 1 | 0 | 0 | 1 | 0 |"),
        "unexpected report: {report}"
    );
}
