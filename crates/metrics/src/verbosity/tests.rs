use codeint_model::language::FlaggedLine;
use codeint_model::{CodeModel, FileModel, RepoPath, SourceFile};

use super::{FlaggedLineFinding, Verbosity};

fn file(path: &str, source_lines: u32, flagged: &[(u32, &str)]) -> SourceFile {
    SourceFile {
        path: RepoPath::new(path).expect("a valid path"),
        model: FileModel {
            source_lines,
            flagged_lines: flagged
                .iter()
                .map(|&(line, rule)| FlaggedLine {
                    line,
                    rule: rule.to_owned(),
                })
                .collect(),
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
    };

    let verbosity = Verbosity::measure(&model);

    assert_eq!(verbosity.verbose_lines, 2);
    assert_eq!(verbosity.source_lines, 8);
    assert_eq!(verbosity.ratio(), 0.25);
    let finding = |path: &str, line, rule: &str| FlaggedLineFinding {
        path: RepoPath::new(path).expect("a valid path"),
        line,
        rule: rule.to_owned(),
    };
    assert_eq!(
        verbosity.flagged_lines,
        [
            finding("src/a.ts", 1, "rule-one"),
            finding("src/b.ts", 2, "rule-one"),
            finding("src/b.ts", 2, "rule-two"),
        ]
    );
}

#[test]
fn a_model_without_source_lines_has_zero_verbosity() {
    let verbosity = Verbosity::measure(&CodeModel::default());

    assert_eq!(verbosity.ratio(), 0.0);
}
