use codeint_model::language::FlaggedLine;
use codeint_model::{CodeModel, FileModel, RepoPath, SourceFile, SourceSet};

use super::Verbosity;
use crate::CloneLines;

fn file(path: &str, source_lines: u32, flagged: &[(u32, &str)]) -> SourceFile {
    SourceFile {
        path: RepoPath::new(path).expect("a valid path"),
        model: FileModel {
            source_set: SourceSet::Production,
            source_lines,
            functions: Vec::new(),
            tokens: Vec::new(),
            flagged_lines: flagged
                .iter()
                .map(|&(line, rule)| FlaggedLine {
                    line,
                    rule: rule.to_owned(),
                })
                .collect(),
            runtime_imports: Vec::new(),
            program_start: false,
        },
    }
}

#[test]
fn a_line_that_matches_two_rules_counts_once_and_lists_both_rules() {
    let model = CodeModel {
        files: vec![
            file("src/b.ts", 6, &[(2, "rule-two"), (2, "rule-one")]),
            file("src/a.ts", 2, &[(1, "rule-one")]),
        ],
        ..CodeModel::default()
    };

    let verbosity = Verbosity::measure(&model, &CloneLines::default());

    assert_eq!(verbosity.verbose_lines(), 2);
    assert_eq!(verbosity.source_lines(), 8);
    assert_eq!(verbosity.ratio(), 0.25);
    let listed: Vec<(&str, u32, &str)> = verbosity
        .flagged_lines()
        .iter()
        .map(|finding| (finding.path.as_str(), finding.line, finding.rule.as_str()))
        .collect();
    assert_eq!(
        listed,
        [
            ("src/a.ts", 1, "rule-one"),
            ("src/b.ts", 2, "rule-one"),
            ("src/b.ts", 2, "rule-two"),
        ]
    );
}

#[test]
fn a_model_without_source_lines_has_zero_verbosity() {
    let verbosity = Verbosity::measure(&CodeModel::default(), &CloneLines::default());

    assert_eq!(verbosity.ratio(), 0.0);
}
