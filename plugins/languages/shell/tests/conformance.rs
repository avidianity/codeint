use std::collections::BTreeSet;

use codeint_lang_shell::{
    SHELL_FLAGGED_RULES, SHELL_HAZARD_RULES, Shell, ShellDialect, detect_dialect,
    flagged_line_rules, hazard_line_rules,
};
use codeint_model::{
    FrameworkProfileSelection, LanguagePlugin, LowerError, RepoPath, SourceSet, TokenKind,
};

fn repo_path(path: &str) -> RepoPath {
    RepoPath::new(path).unwrap()
}

fn lower_ok(path: &str, src: &str) -> codeint_model::FileModel {
    let model = Shell
        .lower(&repo_path(path), src)
        .unwrap_or_else(|err| panic!("expected {path} to lower cleanly: {err}"));
    let known_hazards: BTreeSet<String> = Shell.hazard_rules().into_iter().collect();
    let total_lines = src.lines().count().max(1) as u32;
    for h in &model.hazard_lines {
        assert!(
            known_hazards.contains(&h.rule),
            "unlisted hazard rule {:?} in {path}",
            h.rule
        );
        assert!(
            h.line >= 1 && h.line <= total_lines,
            "hazard line {} out of range 1..={total_lines} in {path}",
            h.line
        );
    }
    for pair in model.tokens.windows(2) {
        assert!(
            pair[0].line <= pair[1].line,
            "tokens out of line order in {path}: {} > {}",
            pair[0].line,
            pair[1].line
        );
    }
    model
}

#[test]
fn plugin_metadata_capabilities_and_no_imports_contract() {
    assert_eq!(Shell.extensions(), ["sh", "bash", "zsh"]);
    assert!(!Shell.has_imports());
    assert!(Shell.has_hazard_rules());
    let ranges: Vec<_> = Shell
        .reference_ranges()
        .unwrap()
        .iter()
        .map(|range| {
            (
                range.metric.clone(),
                range.metric_version,
                range.low,
                range.high,
                range.source.clone(),
            )
        })
        .collect();
    let source = "codeint panel of 50 Shell repositories, ADR 0031, codeint 0.6.0".to_owned();
    assert_eq!(
        ranges,
        [
            ("duplication".to_owned(), 1, 0.008, 0.751, source.clone()),
            ("erosion".to_owned(), 1, 0.416, 0.927, source.clone()),
            ("verbosity".to_owned(), 2, 0.000, 0.025, source),
        ]
    );
    assert!(Shell.known_framework_profiles().is_empty());
    let fp = Shell
        .resolve_framework_profiles("", &[], Vec::new(), FrameworkProfileSelection::Auto)
        .unwrap();
    assert!(fp.active_profiles.is_empty());

    assert_eq!(
        Shell.hazard_rules(),
        hazard_line_rules().map(str::to_owned).collect::<Vec<_>>()
    );
    assert_eq!(
        BTreeSet::from_iter(SHELL_HAZARD_RULES.iter().copied()).len(),
        10
    );
    assert_eq!(SHELL_HAZARD_RULES.len(), 10);

    assert_eq!(
        flagged_line_rules().collect::<Vec<_>>(),
        SHELL_FLAGGED_RULES
    );
    assert_eq!(SHELL_FLAGGED_RULES.len(), 4);

    let paths = vec![repo_path("scripts/deploy.sh")];
    assert!(Shell.manifest_paths(&paths).is_empty());
    assert!(Shell.workspace_packages(Vec::new()).unwrap().is_empty());
    assert!(
        Shell
            .resolve_imports(&paths, Vec::new(), Vec::new(), &[])
            .unwrap()
            .is_empty()
    );
    assert!(Shell.declared_dependencies(Vec::new()).unwrap().is_empty());
    assert!(Shell.entry_points(&paths, Vec::new()).unwrap().is_empty());

    for bad in ["notes.txt", "main.py", "Makefile", "README"] {
        let res = Shell.lower(&repo_path(bad), "echo hi\n");
        assert!(
            matches!(res, Err(LowerError::UnsupportedFile { .. })),
            "expected UnsupportedFile for {bad}, got {res:?}"
        );
    }
}

#[test]
fn dialect_detection_prefers_shebang_and_falls_back_to_extension() {
    let cases = [
        ("run.sh", "#!/bin/sh\necho hi\n", ShellDialect::PosixSh),
        (
            "run.sh",
            "#!/usr/bin/env sh\necho hi\n",
            ShellDialect::PosixSh,
        ),
        ("run.sh", "#!/bin/dash\necho hi\n", ShellDialect::PosixSh),
        ("run.sh", "#!/bin/ash\necho hi\n", ShellDialect::PosixSh),
        ("run.sh", "#!/bin/bash\necho hi\n", ShellDialect::Bash),
        (
            "run.sh",
            "#!/usr/bin/env bash\necho hi\n",
            ShellDialect::Bash,
        ),
        (
            "run.sh",
            "#!/usr/bin/env -S bash -euo pipefail\necho hi\n",
            ShellDialect::Bash,
        ),
        (
            "run.sh",
            "#!/usr/local/bin/bash5\necho hi\n",
            ShellDialect::Bash,
        ),
        ("run.sh", "#!/bin/zsh\necho hi\n", ShellDialect::Zsh),
        ("run.sh", "#!/usr/bin/env zsh\necho hi\n", ShellDialect::Zsh),
        ("run.bash", "#!/bin/sh\necho hi\n", ShellDialect::PosixSh),
        (
            "run.zsh",
            "#!/usr/bin/env bash\necho hi\n",
            ShellDialect::Bash,
        ),
        (
            "run.sh",
            "\u{feff}#!/bin/bash\r\necho hi\r\n",
            ShellDialect::Bash,
        ),
        ("plain.sh", "echo hi\n", ShellDialect::PosixSh),
        ("plain.bash", "echo hi\n", ShellDialect::Bash),
        ("plain.zsh", "echo hi\n", ShellDialect::Zsh),
    ];
    for (path, src, expected) in cases {
        assert_eq!(
            detect_dialect(path, src),
            expected,
            "dialect mismatch for {path} with {src:?}"
        );
    }
}

#[test]
fn lexer_handles_quotes_heredocs_comments_continuations_utf8_and_errors() {
    let empty = lower_ok(
        "empty.sh",
        "#!/bin/sh\n# only comments\n\n   # indented comment\n",
    );
    assert_eq!(empty.source_lines, 0);
    assert!(empty.functions.is_empty());
    assert!(empty.hazard_lines.is_empty());
    assert!(empty.flagged_lines.is_empty());

    let rich = "\
#!/usr/bin/env bash
set -euo pipefail

# Leading comment excluded from source_lines
single='literal $UNQUOTED `not_backtick` # not a comment'
ansi=$'line1\\n\\'quoted\\'\\t\\\\'
nested=\"hello \\\"$(echo \"inner $(printf '%s' \"café 🚀\")\")\\\"\"
case_sub=\"$(case \"$single\" in foo) echo 1 ;; *) echo 2 ;; esac)\"
hashes=\"$# ${#single} foo#bar\"
joined=alpha\\
beta

cat <<EOF
# inside heredoc
if [ $? -eq 0 ]; then cat file | grep x; fi
EOF

cat <<- 'INDENTED'
\tindented body
\tINDENTED

read -r line <<< \"$nested\"
printf '%s %s %s %s\\n' \"$ansi\" \"$case_sub\" \"$hashes\" \"$joined\"
";
    let model = lower_ok("rich.bash", rich);
    assert!(model.source_lines >= 12);
    assert!(
        model.flagged_lines.is_empty(),
        "got {:?}",
        model.flagged_lines
    );
    assert!(
        model.hazard_lines.is_empty(),
        "got {:?}",
        model.hazard_lines
    );
    assert!(model.runtime_imports.is_empty());

    for (name, bad) in [
        ("unclosed_single.sh", "echo 'unterminated\n"),
        ("unclosed_double.sh", "echo \"unterminated\n"),
        ("unclosed_ansi.sh", "echo $'unterminated\\'\n"),
        ("unclosed_backtick.sh", "echo `date\n"),
        ("unclosed_cmdsub.sh", "echo $(date\n"),
        ("unclosed_param.sh", "echo ${VAR\n"),
        ("unclosed_arith.sh", "echo $((1 + 2\n"),
        ("unclosed_heredoc.sh", "cat <<EOF\nhello\n"),
    ] {
        let res = Shell.lower(&repo_path(name), bad);
        assert!(
            matches!(res, Err(LowerError::Unparsable { .. })),
            "expected Unparsable for {name}, got {res:?}"
        );
    }
}

fn assert_fn_shapes(model: &codeint_model::FileModel, expected: &[(&str, u32, u32, u32)]) {
    assert_eq!(model.functions.len(), expected.len());
    for (actual, &(name, source_lines, decisions, cognitive)) in
        model.functions.iter().zip(expected)
    {
        assert_eq!(actual.name.as_deref(), Some(name));
        assert_eq!(actual.source_lines, source_lines, "source_lines for {name}");
        assert_eq!(actual.decision_points, decisions, "decisions for {name}");
        assert_eq!(
            actual.cognitive_complexity, cognitive,
            "cognitive for {name}"
        );
    }
}

#[test]
fn functions_top_level_script_decisions_and_cognitive_complexity() {
    let flat_script = "\
#!/bin/sh
set -eu
if [ -f \"a\" ] && [ -f \"b\" ]; then
    for item in one two; do
        while [ -n \"$item\" ]; do
            break
        done
    done
elif [ -f \"c\" ]; then
    echo \"c\"
else
    echo \"d\"
fi
";
    let flat = lower_ok("flat.sh", flat_script);
    assert_fn_shapes(&flat, &[("<script>", 12, 5, 9)]);
    assert_eq!(
        (flat.functions[0].line, flat.functions[0].end_line),
        (2, 13)
    );

    let library_only = "\
#!/usr/bin/env bash
greet() {
    case \"$1\" in
        hi|hello)
            echo \"hey\"
            ;;
        bye)
            echo \"later\"
            ;;
        *)
            echo \"unknown\"
            ;;
    esac
}

function recurse {
    if [ \"$1\" -gt 0 ]; then
        recurse \"$(( $1 - 1 ))\"
    fi
}

function with_parens() {
    until false; do
        echo \"once\"
        break
    done
}
";
    let lib = lower_ok("lib.bash", library_only);
    assert_fn_shapes(
        &lib,
        &[
            ("greet", 13, 2, 1),
            ("recurse", 5, 1, 2),
            ("with_parens", 6, 1, 1),
        ],
    );
    assert!(
        lib.hazard_lines.is_empty(),
        "library-only file should not trigger missing-strict-mode, got {:?}",
        lib.hazard_lines
    );

    let mixed = "#!/bin/sh\nset -eu\nhelper() {\n    echo \"helper\"\n}\nhelper\necho \"done\"\n";
    let mix = lower_ok("mixed.sh", mixed);
    assert_fn_shapes(&mix, &[("<script>", 3, 0, 0), ("helper", 3, 0, 0)]);
}

#[test]
fn nested_functions_and_duplication_token_normalization() {
    let nested_src = "\
#!/bin/sh
set -eu
outer() {
    inner() {
        if [ -f \"x\" ]; then
            echo \"inner\"
        fi
    }
    if [ -f \"y\" ]; then
        inner
    fi
}
";
    let nested = lower_ok("nested.sh", nested_src);
    assert_fn_shapes(
        &nested,
        &[
            ("<script>", 1, 0, 0),
            ("outer", 5, 1, 1),
            ("inner", 5, 1, 1),
        ],
    );

    let clone_a = lower_ok(
        "a.sh",
        "#!/bin/sh\nset -eu\nif [ -f \"$alpha\" ]; then\n  cp \"$alpha\" \"out_a\"\nfi\n",
    );
    let clone_b = lower_ok(
        "b.sh",
        "#!/bin/sh\nset -eu\nif [ -f \"$beta\" ]; then\n  cp \"$beta\" \"out_b\"\nfi\n",
    );
    let shape_a: Vec<TokenKind> = clone_a.tokens.iter().map(|t| t.kind).collect();
    let shape_b: Vec<TokenKind> = clone_b.tokens.iter().map(|t| t.kind).collect();
    assert_eq!(shape_a, shape_b);
    assert!(shape_a.contains(&TokenKind::Identifier));
    assert!(shape_a.contains(&TokenKind::Literal));
    assert!(shape_a.contains(&TokenKind::Other));
}

#[test]
fn source_set_classification_covers_production_test_generated_and_vendored() {
    let plain = "#!/bin/sh\nset -eu\necho ok\n";
    assert_eq!(
        lower_ok("scripts/deploy.sh", plain).source_set,
        SourceSet::Production
    );

    for test_path in [
        "test/run.sh",
        "tests/smoke.bash",
        "spec/unit.zsh",
        "scripts/deploy_test.sh",
        "scripts/test_deploy.sh",
        "scripts/helper_test.bash",
        "scripts/test_helper.zsh",
    ] {
        assert_eq!(
            lower_ok(test_path, plain).source_set,
            SourceSet::Test,
            "expected Test for {test_path}"
        );
    }

    for gen_path in [
        "configure",
        "build-aux/ltmain.sh",
        "scripts/configure.sh",
        "config.sub",
        "config.guess",
    ] {
        assert_eq!(
            lower_ok(gen_path, plain).source_set,
            SourceSet::Generated,
            "expected Generated for {gen_path}"
        );
    }

    let autoconf = "#!/bin/sh\n# Generated by GNU Autoconf 2.71\necho ok\n";
    assert_eq!(
        lower_ok("scripts/setup.sh", autoconf).source_set,
        SourceSet::Generated
    );
    let do_not_edit = "#!/bin/bash\n# DO NOT EDIT - auto-generated\nset -euo pipefail\necho ok\n";
    assert_eq!(
        lower_ok("scripts/env.bash", do_not_edit).source_set,
        SourceSet::Generated
    );

    assert_eq!(
        lower_ok("vendor/lib.sh", plain).source_set,
        SourceSet::Vendored
    );
    assert_eq!(
        lower_ok("third_party/tool.sh", plain).source_set,
        SourceSet::Vendored
    );
}
