use codeint_lang_shell::Shell;
use codeint_model::{LanguagePlugin, RepoPath};

fn assert_decisions(path: &str, src: &str, expected: &[(&str, u32)]) {
    let model = Shell
        .lower(&RepoPath::new(path).unwrap(), src, None)
        .unwrap_or_else(|err| panic!("expected {path} to lower cleanly: {err}"));
    let actual: Vec<(&str, u32)> = model
        .functions
        .iter()
        .map(|f| (f.name.as_deref().unwrap_or_default(), f.decision_points))
        .collect();
    assert_eq!(actual, expected);
}

macro_rules! decision_cases {
    ($($name:ident: $path:literal, $src:literal => $expected:expr;)+) => {
        $(
            #[test]
            fn $name() {
                assert_decisions($path, $src, $expected);
            }
        )+
    };
}

decision_cases! {
    case_branch_body_starts_a_command_so_its_keywords_count: "route.bash", "\
#!/usr/bin/env bash
route() {
    case \"$1\" in
        start) if [ -n \"$2\" ]; then echo go; fi ;;
        *) while read -r line; do echo \"$line\"; done ;;
    esac
}
"
        => &[("route", 3)];

    case_branch_body_can_define_a_function: "install.bash", "\
#!/usr/bin/env bash
case \"$1\" in
    install) function do_install() {
        if [ -d /opt/demo ]; then echo present; fi
    } ;;
esac
"
        => &[("<script>", 1), ("do_install", 1)];

    array_literal_words_are_not_reserved_words: "words.bash", "\
#!/usr/bin/env bash
words=( if then for while until select do done )
count() {
    local list=(case esac
        for while)
    echo \"${#list[@]}\"
}
"
        => &[("<script>", 0), ("count", 0)];

    arithmetic_for_header_is_arithmetic_not_a_heredoc: "shift.bash", "\
#!/usr/bin/env bash
shift_loop() {
    for (( i = 1; i < 64 && i > 0; i = i << 1 )); do
        echo \"$i\"
    done
}
"
        => &[("shift_loop", 2)];

    bare_brace_inside_parameter_expansion_does_not_nest: "strip.sh", "\
#!/bin/sh
strip() {
    tail=${1#*{}
    if [ -n \"$tail\" ]; then echo \"$tail\"; fi
}
"
        => &[("strip", 1)];

    zsh_closing_brace_ends_a_function_without_a_separator: "greet.zsh", "\
#!/usr/bin/env zsh
greet() { print -r -- \"hi $1\" }
check() { [[ -n $1 ]] && print ok }
"
        => &[("greet", 0), ("check", 1)];

    single_quote_inside_double_quoted_parameter_expansion_is_literal: "quotes.zsh", "\
#!/usr/bin/env zsh
count_quotes() {
    local singles=\"${1//[^']}\"
    if [[ -n $singles ]]; then print -r -- \"${#singles}\"; fi
}
"
        => &[("count_quotes", 1)];

    single_quotes_inside_double_quoted_bash_pattern_removal_still_quote: "rest.bash", "\
#!/usr/bin/env bash
rest=\"${payload%%'\"name\"'*}${payload#*'\"'}\"
if [ -n \"$rest\" ]; then echo \"$rest\"; fi
"
        => &[("<script>", 1)];

    case_pattern_parens_do_not_close_a_subshell_function_body: "probe.bash", "\
#!/usr/bin/env bash
probe() (
    case \"$1\" in
        6 | 7) echo low ;;
        (8) echo high ;;
        *) echo other ;;
    esac
    if [ -n \"$2\" ]; then echo \"$2\"; fi
)
"
        => &[("probe", 3)];

    closing_brace_right_after_a_compound_command_ends_the_function: "digits.sh", "\
#!/bin/sh
is_digit() { case \"$1\" in [0-9]) return 0 ;; *) return 1 ;; esac }
is_dir() { if [ -d \"$1\" ]; then return 0; fi }
after() { echo done; }
"
        => &[("is_digit", 1), ("is_dir", 1), ("after", 0)];

    dollar_double_paren_that_is_not_arithmetic_is_a_command_substitution: "summary.bash", "\
#!/usr/bin/env bash
summary() {
    local lines=$((git status --porcelain 2> /dev/null ||
        git status) | sort)
    if [ -n \"$lines\" ]; then echo \"$lines\"; fi
}
"
        => &[("summary", 2)];
}
