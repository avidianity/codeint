use super::{
    EVIDENCE_SECTIONS, commit_message_problems, declares_no_behavior_change,
    evidence_bundle_problems, without_git_comments,
};

fn types() -> Vec<String> {
    ["feat", "fix", "chore"]
        .iter()
        .map(|kind| (*kind).to_string())
        .collect()
}

fn scopes() -> Vec<String> {
    ["auth", "checks"]
        .iter()
        .map(|scope| (*scope).to_string())
        .collect()
}

#[test]
fn conventional_message_passes() {
    assert!(
        commit_message_problems(
            "fix(auth): refuse expired session tokens\n",
            &types(),
            &scopes()
        )
        .is_empty()
    );
    assert!(
        commit_message_problems(
            "feat!: drop the old flag\n\nCloses #4\n",
            &types(),
            &scopes()
        )
        .is_empty()
    );
}

#[test]
fn co_author_trailer_fails() {
    let message =
        "fix(auth): refuse expired tokens\n\nCo-Authored-By: Someone <someone@example.com>\n";
    assert_eq!(
        commit_message_problems(message, &types(), &scopes()).len(),
        1
    );
}

#[test]
fn subject_without_type_fails() {
    assert_eq!(
        commit_message_problems("refuse expired tokens\n", &types(), &scopes()).len(),
        1
    );
}

#[test]
fn unknown_scope_and_skip_marker_fail() {
    assert_eq!(
        commit_message_problems("fix(nope): x\n", &types(), &scopes()).len(),
        1
    );
    assert_eq!(
        commit_message_problems("fix: x [skip ci]\n", &types(), &scopes()).len(),
        1
    );
}

#[test]
fn git_merge_subject_and_comment_lines_are_accepted() {
    let message =
        "Merge remote-tracking branch 'origin/main' into feat/x\n# Please enter a commit message\n";
    assert!(commit_message_problems(message, &types(), &scopes()).is_empty());
}

#[test]
fn body_with_only_a_summary_fails_every_section() {
    let problems = evidence_bundle_problems("This change fixes the thing.\n");
    assert_eq!(problems.len(), EVIDENCE_SECTIONS.len());
}

#[test]
fn filled_template_with_not_applicable_passes() {
    let body = "## Summary\n\nFix.\n\n## Tier\n\nTier 1 from the tier check.\n\n## Task\n\n- [x] Done\n\n## Fail-first test\n\nNo behavior change: docs only.\n\n## Checks\n\nAll passed.\n\n## Safety proof\n\nNot applicable: Tier 0 or 1.\n\n## Runtime evidence\n\nNot applicable, no user-visible change.\n\n## Gaps\n\nNone.\n";
    assert!(evidence_bundle_problems(body).is_empty());
}

#[test]
fn unfilled_template_placeholder_fails() {
    let body = "## Tier\n\nTier <n> from the tier check.\n## Task\nx\n## Fail-first test\nx\n## Checks\nx\n## Safety proof\nx\n## Runtime evidence\nx\n## Gaps\nx\n";
    assert_eq!(evidence_bundle_problems(body).len(), 1);
}

#[test]
fn no_behavior_change_needs_a_reason() {
    assert!(declares_no_behavior_change(
        "No behavior change: rename only."
    ));
    assert!(!declares_no_behavior_change("No behavior change:"));
}

#[test]
fn a_stored_commit_keeps_its_comment_lines() {
    let stored =
        "#wip whatever I like\n\nfeat: looks fine\n\n#Generated with Claude Code\n#[skip ci]\n";
    let problems = commit_message_problems(stored, &types(), &scopes());
    assert_eq!(problems.len(), 3, "{problems:?}");
}

#[test]
fn the_hook_message_drops_comment_lines_and_everything_after_the_scissors() {
    let edited = "fix(checks): refuse an empty path\n# Please enter the commit message\n# ------------------------ >8 ------------------------\nCo-Authored-By: Bot <bot@example.invalid>\n";
    let cleaned = without_git_comments(edited);
    assert!(commit_message_problems(&cleaned, &types(), &scopes()).is_empty());
}

#[test]
fn not_applicable_without_a_reason_fails() {
    let sections: String = EVIDENCE_SECTIONS
        .iter()
        .map(|section| format!("## {section}\n\nNot applicable\n\n"))
        .collect();
    assert_eq!(
        evidence_bundle_problems(&sections).len(),
        EVIDENCE_SECTIONS.len()
    );
}

#[test]
fn a_no_behavior_change_line_inside_an_html_comment_does_not_count() {
    assert!(!declares_no_behavior_change(
        "<!-- No behavior change: hidden from the rendered body -->"
    ));
}
