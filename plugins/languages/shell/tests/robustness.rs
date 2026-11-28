use std::time::{Duration, Instant};

use codeint_lang_shell::{Shell, ShellDialect, detect_dialect};
use codeint_model::language::Function;
use codeint_model::{FileModel, LanguagePlugin, LowerError, RepoPath, SourceSet};

const NESTING_LIMIT: usize = 256;
const LARGE_INPUT_BOUND: Duration = Duration::from_secs(30);

fn try_lower(path: &str, src: &str) -> Result<FileModel, LowerError> {
    Shell.lower(&RepoPath::new(path).unwrap(), src, None)
}

fn lower(path: &str, src: &str) -> FileModel {
    try_lower(path, src).unwrap_or_else(|err| panic!("failed to lower {path}: {err}"))
}

fn function_named<'a>(model: &'a FileModel, name: &str) -> &'a Function {
    model
        .functions
        .iter()
        .find(|f| f.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no function named {name}"))
}

fn assert_shape(model: &FileModel, name: &str, expected: (u32, u32, u32)) {
    let f = function_named(model, name);
    assert_eq!((f.line, f.end_line, f.source_lines), expected, "{name}");
}

fn nest(open: &str, close: &str, inner: &str, depth: usize) -> String {
    format!(
        "#!/bin/sh\nset -eu\necho {}{inner}{}\n",
        open.repeat(depth),
        close.repeat(depth)
    )
}

fn assert_unparsable_for_nesting(src: &str, label: &str) {
    match try_lower("deep.sh", src) {
        Err(LowerError::Unparsable { message: reason }) => assert!(
            reason.contains("nested deeper than 256"),
            "{label}: unexpected reason {reason}"
        ),
        other => panic!("{label}: expected Unparsable, got {:?}", other.map(|_| ())),
    }
}

#[test]
fn command_substitution_nesting_at_the_limit_lowers_and_one_past_it_is_unparsable() {
    let at_limit = nest("$(echo ", ")", "x", NESTING_LIMIT);
    lower("at_limit.sh", &at_limit);
    assert_unparsable_for_nesting(
        &nest("$(echo ", ")", "x", NESTING_LIMIT + 1),
        "command substitution",
    );
}

#[test]
fn very_deep_nesting_is_unparsable_instead_of_overflowing_the_stack() {
    assert_unparsable_for_nesting(&nest("$(echo ", ")", "x", 8_000), "command substitution");
    assert_unparsable_for_nesting(&nest("<(cat ", ")", "x", 8_000), "process substitution");
    assert_unparsable_for_nesting(
        &nest("\"$(echo ", ")\"", "x", 8_000),
        "quoted command substitution",
    );
    assert_unparsable_for_nesting(
        &nest("${v:-$(echo ", ")}", "x", 8_000),
        "command substitution inside parameter expansion",
    );
    assert_unparsable_for_nesting(
        &nest("\"${v:-", "}\"", "x", 8_000),
        "quoted parameter expansion",
    );
}

#[test]
fn process_and_parameter_nesting_share_the_same_limit() {
    lower(
        "proc_at_limit.bash",
        &nest("<(cat ", ")", "x", NESTING_LIMIT).replace("#!/bin/sh", "#!/bin/bash"),
    );
    assert_unparsable_for_nesting(
        &nest("<(cat ", ")", "x", NESTING_LIMIT + 1),
        "process substitution",
    );
    assert_unparsable_for_nesting(
        &nest("${v:-$(echo ", ")}", "x", NESTING_LIMIT),
        "mixed nesting",
    );
}

fn sequential_functions(count: usize) -> String {
    let mut src = String::from("#!/bin/sh\nset -eu\n");
    for index in 0..count {
        src.push_str(&format!(
            "f{index}() {{ echo {index} alpha beta gamma delta epsilon zeta; }}\n"
        ));
    }
    src
}

fn nested_functions(depth: usize) -> String {
    let mut src = String::from("#!/bin/sh\nset -eu\n");
    for index in 0..depth {
        src.push_str(&format!("f{index}() {{\n"));
    }
    src.push_str("echo leaf\n");
    for _ in 0..depth {
        src.push_str("}\n");
    }
    src
}

fn assert_lowers_within_bound(
    path: &str,
    src: &str,
    label: &str,
    expected_functions: usize,
) -> FileModel {
    let started = Instant::now();
    let model = lower(path, src);
    let elapsed = started.elapsed();
    assert_eq!(model.functions.len(), expected_functions);
    assert!(
        elapsed < LARGE_INPUT_BOUND,
        "lowering {label} took {elapsed:?}"
    );
    model
}

#[test]
fn forty_thousand_sequential_functions_lower_within_the_bound() {
    let src = sequential_functions(40_000);
    let model = assert_lowers_within_bound("many.sh", &src, "40,000 sequential functions", 40_001);
    assert_shape(&model, "f0", (3, 3, 1));
}

#[test]
fn two_thousand_nested_functions_lower_within_the_bound() {
    let src = nested_functions(2_000);
    let model = assert_lowers_within_bound("nested.sh", &src, "2,000 nested functions", 2_001);
    assert_shape(&model, "f0", (3, 4_003, 2));
    assert_eq!(function_named(&model, "f1999").source_lines, 3);
}

#[test]
fn heredoc_bodies_belong_to_the_function_that_starts_them() {
    let src = "#!/bin/sh\nset -eu\ng() { cat <<EOF; }\nbody one\nbody two\nEOF\necho done\n";
    let model = lower("g.sh", src);
    assert_shape(&model, "g", (3, 6, 4));
    assert_shape(&model, "<script>", (2, 7, 2));
}

#[test]
fn heredoc_at_the_end_of_the_script_extends_the_top_level_function() {
    let src = "#!/bin/sh\ncat <<EOF\na\nb\nEOF\n";
    let model = lower("tail.sh", src);
    assert_eq!(model.functions.len(), 1);
    let script = &model.functions[0];
    assert_eq!(script.name.as_deref(), Some("<script>"));
    assert_eq!(
        (script.line, script.end_line, script.source_lines),
        (2, 5, 4)
    );
}

#[test]
fn a_function_heredoc_does_not_stretch_the_top_level_range() {
    let src = "#!/bin/sh\necho a\ng() { cat <<EOF; }\nbody\nEOF\n";
    let model = lower("fn_last.sh", src);
    assert_shape(&model, "<script>", (2, 2, 1));
}

#[test]
fn a_hand_written_do_not_modify_sentence_is_not_a_generated_marker() {
    let prose = "#!/bin/sh\n# Please do not modify without asking the platform team.\necho ok\n";
    assert_eq!(lower("deploy.sh", prose).source_set, SourceSet::Production);
    for marker in [
        "# DO NOT MODIFY BY HAND",
        "# Do not edit this file",
        "# Code generated by tool. DO NOT EDIT.",
        "# This file is generated",
        "# @generated",
    ] {
        let src = format!("#!/bin/sh\n{marker}\necho ok\n");
        assert_eq!(
            lower("gen.sh", &src).source_set,
            SourceSet::Generated,
            "marker {marker}"
        );
    }
}

#[test]
fn busybox_shebang_selects_the_applet_dialect() {
    assert_eq!(
        detect_dialect("run.sh", "#!/bin/busybox sh\necho hi\n"),
        ShellDialect::PosixSh
    );
    assert_eq!(
        detect_dialect("run.bash", "#!/usr/bin/busybox ash\necho hi\n"),
        ShellDialect::PosixSh
    );
    assert_eq!(
        detect_dialect("run.bash", "#!/bin/busybox\necho hi\n"),
        ShellDialect::Bash
    );
    let model = lower(
        "busy.bash",
        "#!/bin/busybox sh\nset -eu\nif [[ -f a ]]; then :; fi\n",
    );
    assert!(
        model
            .hazard_lines
            .iter()
            .any(|h| h.rule == "posix-sh-bashism" && h.line == 3)
    );
}

#[test]
fn malformed_and_edge_case_case_patterns_lower_without_panicking() {
    let cases = [
        "#!/bin/sh\nf() {\n  case $1 in\n    ) echo empty ;;\n    a) echo a ;;\n    *) echo default ;;\n  esac\n}\n",
        "#!/bin/sh\nf() {\n  case $1 in\n    a)\n      case $2 in\n        b) echo b ;;\n      esac\n    ;;\n  esac\n}\n",
        "#!/bin/sh\ncase $1 in\n  a) echo unterminated\n",
    ];
    for (idx, src) in cases.iter().enumerate() {
        let model = lower(&format!("case_edge_{idx}.sh"), src);
        assert!(
            !model.functions.is_empty(),
            "case edge {idx} produced no functions"
        );
    }
    let model = lower("case_decisions.sh", cases[0]);
    assert_eq!(function_named(&model, "f").decision_points, 1);
}
