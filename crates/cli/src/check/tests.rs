use std::fs;
use std::process::Command;

use codeint_config::FindingsShown;
use codeint_model::LanguagePlugin;
use codeint_model::language::{FileModel, Guest, LowerError, ReferenceRange};
use tempfile::TempDir;

use super::{CheckError, check_with};

struct Alpha;

struct Beta;

struct Malformed;

fn lines_of(text: &str) -> Result<FileModel, LowerError> {
    Ok(FileModel {
        source_lines: u32::try_from(text.lines().count()).map_err(|_| LowerError::TooLarge)?,
        functions: Vec::new(),
        tokens: Vec::new(),
    })
}

fn erosion_range(low: f64, high: f64, source: &str) -> Vec<ReferenceRange> {
    vec![ReferenceRange {
        metric: "erosion".to_owned(),
        low,
        high,
        source: source.to_owned(),
    }]
}

impl Guest for Alpha {
    fn extensions() -> Vec<String> {
        vec!["alpha".to_owned()]
    }

    fn lower(_path: String, text: String) -> Result<FileModel, LowerError> {
        lines_of(&text)
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        erosion_range(0.12, 0.56, "Alpha panel of 40 repositories, Table 3")
    }
}

impl Guest for Beta {
    fn extensions() -> Vec<String> {
        vec!["beta".to_owned()]
    }

    fn lower(_path: String, text: String) -> Result<FileModel, LowerError> {
        lines_of(&text)
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        Vec::new()
    }
}

impl Guest for Malformed {
    fn extensions() -> Vec<String> {
        vec!["malformed".to_owned()]
    }

    fn lower(_path: String, text: String) -> Result<FileModel, LowerError> {
        lines_of(&text)
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        erosion_range(0.7, 0.2, "Reversed")
    }
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
        fs::write(folder.path().join(path), text).expect("write the file");
    }
    folder
}

fn report(files: &[(&str, &str)], plugins: &[&dyn LanguagePlugin]) -> String {
    let folder = repository(files);
    check_with(folder.path(), plugins, FindingsShown::All).expect("the check succeeds")
}

#[test]
fn a_reference_range_that_the_only_measured_language_supplies_is_shown_with_its_source_label() {
    let report = report(&[("main.alpha", "one\ntwo\n")], &[&Alpha, &Beta]);

    assert_eq!(
        report,
        concat!(
            "files: 1\n",
            "source_lines: 2\n",
            "erosion:\n",
            "  ratio: 0\n",
            "  reference_range:\n",
            "    low: 0.12\n",
            "    high: 0.56\n",
            "    source: \"Alpha panel of 40 repositories, Table 3\"\n",
            "  eroded_function_mass: 0\n",
            "  total_function_mass: 0\n",
            "  findings: []\n",
            "duplication:\n",
            "  ratio: 0\n",
            "  reference_range: no published Reference range for this language\n",
            "  clone_lines: 0\n",
            "  clone_groups: []",
        )
    );
}

#[test]
fn a_reference_range_of_one_language_is_never_shown_for_files_of_another_language() {
    let report = report(&[("main.beta", "one\n")], &[&Alpha, &Beta]);

    assert!(
        report.contains("\n  reference_range: no published Reference range for this language\n"),
        "unexpected report: {report}"
    );
    assert!(
        !report.contains("Alpha panel"),
        "unexpected report: {report}"
    );
}

#[test]
fn no_reference_range_is_shown_when_the_measured_files_are_in_several_languages() {
    let report = report(
        &[("main.alpha", "one\n"), ("main.beta", "one\n")],
        &[&Alpha, &Beta],
    );

    assert!(
        report.contains(
            "\n  reference_range: \"not shown, because the measured files are in more than one language\"\n"
        ),
        "unexpected report: {report}"
    );
    assert!(
        !report.contains("Alpha panel"),
        "unexpected report: {report}"
    );
}

#[test]
fn an_invalid_reference_range_from_a_language_plugin_stops_the_check() {
    let folder = repository(&[("main.beta", "one\n")]);

    let result = check_with(folder.path(), &[&Beta, &Malformed], FindingsShown::All);

    assert!(
        matches!(result, Err(CheckError::ReferenceRange(_))),
        "unexpected result: {result:?}"
    );
}
