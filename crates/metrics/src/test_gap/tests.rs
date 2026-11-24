use codeint_model::language::Function;
use codeint_model::{
    CodeModel, FunctionCoverage, LanguageFacts, Package, SourceFile, SourceSet, Token, TokenKind,
    Workspace, token_text_hash,
};

use super::TestGap;
use crate::test_support::{empty_source_file, repo_path};

fn function_with_coverage(
    name: Option<&str>,
    line: u32,
    end_line: u32,
    decision_points: u32,
    source_lines: u32,
    coverage: Option<FunctionCoverage>,
) -> Function {
    Function {
        line,
        end_line,
        decision_points,
        source_lines,
        name: name.map(str::to_owned),
        cognitive_complexity: 0,
        coverage,
    }
}

fn token(line: u32, text: &str) -> Token {
    Token {
        kind: TokenKind::Other,
        text_hash: token_text_hash(text),
        line,
    }
}

fn covered_model(files: Vec<SourceFile>, workspace: Workspace) -> CodeModel {
    CodeModel {
        files,
        workspace,
        languages: vec![LanguageFacts {
            language: "typescript".to_owned(),
            has_coverage: true,
            has_hazard_rules: false,
            has_imports: true,
        }],
        ..CodeModel::default()
    }
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn test_gap_is_absent_when_no_language_has_coverage_or_only_non_production_files_exist() {
    let mut prod = empty_source_file("src/app.ts", SourceSet::Production, 10);
    prod.model
        .functions
        .push(function_with_coverage(Some("run"), 1, 4, 0, 4, None));
    let uncovered = CodeModel {
        files: vec![prod],
        languages: vec![LanguageFacts {
            language: "typescript".to_owned(),
            has_coverage: false,
            has_hazard_rules: false,
            has_imports: true,
        }],
        ..CodeModel::default()
    };
    assert_eq!(TestGap::measure(&uncovered), None);

    let mut test_only = empty_source_file("src/app.test.ts", SourceSet::Test, 10);
    test_only.model.functions.push(function_with_coverage(
        Some("itWorks"),
        1,
        4,
        0,
        4,
        Some(FunctionCoverage {
            listed_lines: 4,
            executed_lines: 0,
        }),
    ));
    let non_prod_model = covered_model(vec![test_only], Workspace::default());
    assert_eq!(TestGap::measure(&non_prod_model), None);
}

#[test]
fn test_gap_weighs_by_function_mass_handles_zero_listed_and_uncovered_files_and_sorts_findings() {
    let mut covered_file = empty_source_file("src/core.ts", SourceSet::Production, 30);
    covered_file.model.functions = vec![
        function_with_coverage(
            Some("fullyTested"),
            1,
            4,
            1,
            4,
            Some(FunctionCoverage {
                listed_lines: 4,
                executed_lines: 4,
            }),
        ),
        function_with_coverage(
            Some("halfTestedHeavy"),
            6,
            14,
            3,
            9,
            Some(FunctionCoverage {
                listed_lines: 4,
                executed_lines: 2,
            }),
        ),
        function_with_coverage(
            Some("notExecutable"),
            16,
            20,
            5,
            9,
            Some(FunctionCoverage {
                listed_lines: 0,
                executed_lines: 0,
            }),
        ),
    ];

    let mut uncovered_file = empty_source_file("src/unloaded.ts", SourceSet::Production, 16);
    uncovered_file.model.functions = vec![function_with_coverage(None, 1, 16, 1, 16, None)];

    let mut ignored_test_file = empty_source_file("src/core.test.ts", SourceSet::Test, 16);
    ignored_test_file.model.functions =
        vec![function_with_coverage(Some("testFn"), 1, 16, 10, 16, None)];

    let model = covered_model(
        vec![covered_file, uncovered_file, ignored_test_file],
        Workspace::default(),
    );

    let measured = TestGap::measure(&model).expect("Test gap should be measured");

    let fully_tested_mass = 2.0 * 4.0_f64.sqrt();
    let half_tested_mass = 4.0 * 9.0_f64.sqrt();
    let unloaded_mass = 2.0 * 16.0_f64.sqrt();
    let expected_total = fully_tested_mass + half_tested_mass + unloaded_mass;
    let expected_untested = half_tested_mass * 0.5 + unloaded_mass;

    assert_eq!(measured.measured_functions(), 3);
    assert_close(measured.total_mass(), expected_total);
    assert_close(measured.untested_mass(), expected_untested);
    assert_close(measured.ratio(), expected_untested / expected_total);

    let findings = measured.findings();
    assert_eq!(findings.len(), 2);

    assert_eq!(findings[0].path, repo_path("src/unloaded.ts"));
    assert_eq!(findings[0].name, None);
    assert_eq!(findings[0].line, 1);
    assert_close(findings[0].function_mass, unloaded_mass);
    assert_close(findings[0].untested_share, 1.0);
    assert_close(findings[0].untested_mass, unloaded_mass);
    assert_eq!(findings[0].listed_lines, None);
    assert_eq!(findings[0].executed_lines, None);

    assert_eq!(findings[1].path, repo_path("src/core.ts"));
    assert_eq!(findings[1].name.as_deref(), Some("halfTestedHeavy"));
    assert_eq!(findings[1].line, 6);
    assert_close(findings[1].function_mass, half_tested_mass);
    assert_close(findings[1].untested_share, 0.5);
    assert_close(findings[1].untested_mass, half_tested_mass * 0.5);
    assert_eq!(findings[1].listed_lines, Some(4));
    assert_eq!(findings[1].executed_lines, Some(2));
}

#[test]
fn finding_id_stays_stable_when_function_moves_lines_and_by_package_sums_to_total() {
    let workspace = Workspace::new([repo_path("packages/api"), repo_path("packages/web")]);
    let mut api_file = empty_source_file("packages/api/src/handler.ts", SourceSet::Production, 10);
    api_file.model.functions.push(function_with_coverage(
        Some("handle"),
        2,
        5,
        0,
        4,
        Some(FunctionCoverage {
            listed_lines: 4,
            executed_lines: 1,
        }),
    ));
    api_file.model.tokens = vec![token(2, "fn"), token(3, "return")];

    let mut web_file = empty_source_file("packages/web/src/view.ts", SourceSet::Production, 10);
    web_file.model.functions.push(function_with_coverage(
        Some("render"),
        1,
        4,
        1,
        4,
        Some(FunctionCoverage {
            listed_lines: 4,
            executed_lines: 4,
        }),
    ));

    let model = covered_model(vec![api_file.clone(), web_file], workspace.clone());
    let measured = TestGap::measure(&model).expect("Test gap should be measured");

    let by_package = measured.by_package(&model);
    let api_root = repo_path("packages/api");
    let web_root = repo_path("packages/web");
    let api_masses = by_package[&Package::Member(&api_root)];
    let web_masses = by_package[&Package::Member(&web_root)];
    assert_eq!(api_masses.files, 1);
    assert_eq!(web_masses.files, 1);
    assert_close(api_masses.ratio(), 0.75);
    assert_close(web_masses.ratio(), 0.0);
    assert_close(api_masses.total + web_masses.total, measured.total_mass());
    assert_close(
        api_masses.untested + web_masses.untested,
        measured.untested_mass(),
    );

    api_file.model.functions[0].line = 20;
    api_file.model.functions[0].end_line = 23;
    api_file.model.tokens = vec![token(20, "fn"), token(21, "return")];
    let shifted_model = covered_model(vec![api_file], workspace);
    let shifted = TestGap::measure(&shifted_model).expect("Test gap should be measured");
    assert_eq!(measured.findings()[0].id, shifted.findings()[0].id);
}
