use codeint_model::language::{FileModel, Function, SourceSet, Token, TokenKind};
use codeint_model::{Package, RepoPath, SourceFile, SourceSetModel, Workspace};

use super::CognitiveComplexity;

fn file(path: &str, functions: Vec<Function>, tokens: Vec<Token>) -> SourceFile {
    SourceFile {
        path: RepoPath::new(path).expect("valid path"),
        model: FileModel {
            source_set: SourceSet::Production,
            source_lines: 40,
            functions,
            tokens,
            flagged_lines: Vec::new(),
            hazard_lines: Vec::new(),
            runtime_imports: Vec::new(),
            program_start: false,
            is_tooling_file: false,
            diagnostics: Vec::new(),
            scope_artifact: None,
        },
    }
}

fn token(line: u32, text_hash: u64) -> Token {
    Token {
        kind: TokenKind::Other,
        text_hash,
        line,
    }
}

#[test]
fn flags_only_functions_over_15_and_sorts_worst_first() {
    let model = SourceSetModel {
        files: vec![file(
            "src/logic.ts",
            vec![
                Function {
                    line: 1,
                    end_line: 10,
                    decision_points: 15,
                    source_lines: 10,
                    name: Some("flatAtThreshold".to_owned()),
                    cognitive_complexity: 15,
                    coverage: None,
                },
                Function {
                    line: 12,
                    end_line: 25,
                    decision_points: 6,
                    source_lines: 12,
                    name: Some("nestedModerate".to_owned()),
                    cognitive_complexity: 16,
                    coverage: None,
                },
                Function {
                    line: 27,
                    end_line: 40,
                    decision_points: 8,
                    source_lines: 14,
                    name: None,
                    cognitive_complexity: 24,
                    coverage: None,
                },
            ],
            vec![token(1, 10), token(12, 20), token(27, 30)],
        )],
        ..SourceSetModel::default()
    };

    let metric = CognitiveComplexity::measure(&model);

    assert_eq!(metric.complex_cognitive_complexity(), 40);
    assert_eq!(metric.total_cognitive_complexity(), 55);
    assert!((metric.ratio() - (40.0 / 55.0)).abs() < 1e-12);
    let findings = metric.complex_functions();
    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0].name, None);
    assert_eq!(findings[0].cognitive_complexity, 24);
    assert_eq!(findings[0].line, 27);
    assert_eq!(findings[0].end_line, 40);
    assert_eq!(findings[1].name.as_deref(), Some("nestedModerate"));
    assert_eq!(findings[1].cognitive_complexity, 16);
}

#[test]
fn finding_id_stays_stable_when_function_moves_lines_and_by_package_sums_to_total() {
    let workspace = Workspace::new(vec![
        RepoPath::new("packages/a").expect("valid"),
        RepoPath::new("packages/b").expect("valid"),
    ]);
    let model_a = SourceSetModel {
        files: vec![
            file(
                "packages/a/src/mod.ts",
                vec![Function {
                    line: 1,
                    end_line: 10,
                    decision_points: 5,
                    source_lines: 10,
                    name: Some("deep".to_owned()),
                    cognitive_complexity: 20,
                    coverage: None,
                }],
                vec![token(2, 111), token(3, 222)],
            ),
            file(
                "packages/b/src/mod.ts",
                vec![Function {
                    line: 1,
                    end_line: 5,
                    decision_points: 2,
                    source_lines: 5,
                    name: Some("simple".to_owned()),
                    cognitive_complexity: 5,
                    coverage: None,
                }],
                vec![token(1, 333)],
            ),
        ],
        workspace: workspace.clone(),
        ..SourceSetModel::default()
    };
    let model_shifted = SourceSetModel {
        files: vec![file(
            "packages/a/src/mod.ts",
            vec![Function {
                line: 20,
                end_line: 29,
                decision_points: 5,
                source_lines: 10,
                name: Some("deep".to_owned()),
                cognitive_complexity: 20,
                coverage: None,
            }],
            vec![token(21, 111), token(22, 222)],
        )],
        workspace,
        ..SourceSetModel::default()
    };

    let measured_a = CognitiveComplexity::measure(&model_a);
    let measured_shifted = CognitiveComplexity::measure(&model_shifted);
    assert_eq!(
        measured_a.complex_functions()[0].id,
        measured_shifted.complex_functions()[0].id
    );

    let by_pkg = measured_a.by_package(&model_a);
    let pkg_a_root = RepoPath::new("packages/a").expect("valid");
    let pkg_b_root = RepoPath::new("packages/b").expect("valid");
    assert_eq!(by_pkg[&Package::Member(&pkg_a_root)].complex, 20);
    assert_eq!(by_pkg[&Package::Member(&pkg_a_root)].total, 20);
    assert_eq!(by_pkg[&Package::Member(&pkg_b_root)].complex, 0);
    assert_eq!(by_pkg[&Package::Member(&pkg_b_root)].total, 5);
    assert_eq!(by_pkg[&Package::Root].total, 0);
}
