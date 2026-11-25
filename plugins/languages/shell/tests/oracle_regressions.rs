use codeint_lang_shell::Shell;
use codeint_model::{FileModel, LanguagePlugin, RepoPath};

fn lower_ok(path: &str, src: &str) -> FileModel {
    Shell
        .lower(&RepoPath::new(path).unwrap(), src)
        .unwrap_or_else(|err| panic!("expected {path} to lower cleanly: {err}"))
}

fn decisions_by_name(model: &FileModel) -> Vec<(String, u32)> {
    model
        .functions
        .iter()
        .map(|f| (f.name.clone().unwrap_or_default(), f.decision_points))
        .collect()
}

fn named(entries: &[(&str, u32)]) -> Vec<(String, u32)> {
    entries
        .iter()
        .map(|&(name, decisions)| (name.to_owned(), decisions))
        .collect()
}

#[test]
fn case_branch_body_starts_a_command_so_its_keywords_count() {
    let src = "\
#!/usr/bin/env bash
route() {
    case \"$1\" in
        start) if [ -n \"$2\" ]; then echo go; fi ;;
        *) while read -r line; do echo \"$line\"; done ;;
    esac
}
";
    let model = lower_ok("route.bash", src);
    assert_eq!(decisions_by_name(&model), named(&[("route", 3)]));
}

#[test]
fn case_branch_body_can_define_a_function() {
    let src = "\
#!/usr/bin/env bash
case \"$1\" in
    install) function do_install() {
        if [ -d /opt/demo ]; then echo present; fi
    } ;;
esac
";
    let model = lower_ok("install.bash", src);
    assert_eq!(
        decisions_by_name(&model),
        named(&[("<script>", 1), ("do_install", 1)])
    );
}

#[test]
fn array_literal_words_are_not_reserved_words() {
    let src = "\
#!/usr/bin/env bash
words=( if then for while until select do done )
count() {
    local list=(case esac
        for while)
    echo \"${#list[@]}\"
}
";
    let model = lower_ok("words.bash", src);
    assert_eq!(
        decisions_by_name(&model),
        named(&[("<script>", 0), ("count", 0)])
    );
}

#[test]
fn arithmetic_for_header_is_arithmetic_not_a_heredoc() {
    let src = "\
#!/usr/bin/env bash
shift_loop() {
    for (( i = 1; i < 64 && i > 0; i = i << 1 )); do
        echo \"$i\"
    done
}
";
    let model = lower_ok("shift.bash", src);
    assert_eq!(decisions_by_name(&model), named(&[("shift_loop", 2)]));
}

#[test]
fn bare_brace_inside_parameter_expansion_does_not_nest() {
    let src = "\
#!/bin/sh
strip() {
    tail=${1#*{}
    if [ -n \"$tail\" ]; then echo \"$tail\"; fi
}
";
    let model = lower_ok("strip.sh", src);
    assert_eq!(decisions_by_name(&model), named(&[("strip", 1)]));
}

#[test]
fn zsh_closing_brace_ends_a_function_without_a_separator() {
    let src = "\
#!/usr/bin/env zsh
greet() { print -r -- \"hi $1\" }
check() { [[ -n $1 ]] && print ok }
";
    let model = lower_ok("greet.zsh", src);
    assert_eq!(
        decisions_by_name(&model),
        named(&[("greet", 0), ("check", 1)])
    );
}

#[test]
fn single_quote_inside_double_quoted_parameter_expansion_is_literal() {
    let src = "\
#!/usr/bin/env zsh
count_quotes() {
    local singles=\"${1//[^']}\"
    if [[ -n $singles ]]; then print -r -- \"${#singles}\"; fi
}
";
    let model = lower_ok("quotes.zsh", src);
    assert_eq!(decisions_by_name(&model), named(&[("count_quotes", 1)]));
}

#[test]
fn single_quotes_inside_double_quoted_bash_pattern_removal_still_quote() {
    let src = "\
#!/usr/bin/env bash
rest=\"${payload%%'\"name\"'*}${payload#*'\"'}\"
if [ -n \"$rest\" ]; then echo \"$rest\"; fi
";
    let model = lower_ok("rest.bash", src);
    assert_eq!(decisions_by_name(&model), named(&[("<script>", 1)]));
}

#[test]
fn case_pattern_parens_do_not_close_a_subshell_function_body() {
    let src = "\
#!/usr/bin/env bash
probe() (
    case \"$1\" in
        6 | 7) echo low ;;
        (8) echo high ;;
        *) echo other ;;
    esac
    if [ -n \"$2\" ]; then echo \"$2\"; fi
)
";
    let model = lower_ok("probe.bash", src);
    assert_eq!(decisions_by_name(&model), named(&[("probe", 3)]));
}

#[test]
fn closing_brace_right_after_a_compound_command_ends_the_function() {
    let src = "\
#!/bin/sh
is_digit() { case \"$1\" in [0-9]) return 0 ;; *) return 1 ;; esac }
is_dir() { if [ -d \"$1\" ]; then return 0; fi }
after() { echo done; }
";
    let model = lower_ok("digits.sh", src);
    assert_eq!(
        decisions_by_name(&model),
        named(&[("is_digit", 1), ("is_dir", 1), ("after", 0)])
    );
}

#[test]
fn dollar_double_paren_that_is_not_arithmetic_is_a_command_substitution() {
    let src = "\
#!/usr/bin/env bash
summary() {
    local lines=$((git status --porcelain 2> /dev/null ||
        git status) | sort)
    if [ -n \"$lines\" ]; then echo \"$lines\"; fi
}
";
    let model = lower_ok("summary.bash", src);
    assert_eq!(decisions_by_name(&model), named(&[("summary", 2)]));
}
