use codeint_model::language::{FileModel, Function, SourceSet, Token, TokenKind};
use codeint_model::{RepoPath, SourceFile, SourceSetModel, Workspace};

use super::Erosion;

fn model_with(functions: Vec<Function>) -> SourceSetModel {
    let path = RepoPath::new("src/demo.ts").expect("valid path");
    SourceSetModel {
        files: vec![SourceFile {
            path,
            model: FileModel {
                source_set: SourceSet::Production,
                source_lines: 30,
                functions,
                tokens: vec![Token {
                    kind: TokenKind::Other,
                    text_hash: 1,
                    line: 1,
                }],
                flagged_lines: Vec::new(),
                hazard_lines: Vec::new(),
                runtime_imports: Vec::new(),
                program_start: false,
                is_tooling_file: false,
                diagnostics: Vec::new(),
                scope_artifact: None,
            },
        }],
        workspace: Workspace::default(),
        ..SourceSetModel::default()
    }
}

#[test]
fn eroded_functions_preserve_name_line_range_and_cognitive_complexity() {
    let model = model_with(vec![
        Function {
            line: 1,
            end_line: 20,
            decision_points: 12,
            source_lines: 18,
            name: Some("routeRequest".to_owned()),
            cognitive_complexity: 22,
            coverage: None,
        },
        Function {
            line: 22,
            end_line: 35,
            decision_points: 11,
            source_lines: 14,
            name: None,
            cognitive_complexity: 16,
            coverage: None,
        },
    ]);

    let erosion = Erosion::measure(&model);
    let findings = erosion.eroded_functions();

    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0].name.as_deref(), Some("routeRequest"));
    assert_eq!(findings[0].line, 1);
    assert_eq!(findings[0].end_line, 20);
    assert_eq!(findings[0].cyclomatic_complexity, 13);
    assert_eq!(findings[0].cognitive_complexity, 22);
    assert_eq!(findings[1].name, None);
    assert_eq!(findings[1].line, 22);
    assert_eq!(findings[1].end_line, 35);
    assert_eq!(findings[1].cyclomatic_complexity, 12);
    assert_eq!(findings[1].cognitive_complexity, 16);
}
