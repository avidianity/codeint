use super::{always_read_pages, placeholders_in};

#[test]
fn always_read_pages_come_from_the_always_read_column() {
    let rules = "| Task | Always read | Read when |\n| --- | --- | --- |\n| Plan | `docs/agents/planning.md` | `docs/agents/issue-tracker.md` |\n| Code | `docs/agents/engineering.md`, `docs/agents/stack.md` | |\n";
    assert_eq!(
        always_read_pages(rules),
        vec![
            "docs/agents/engineering.md".to_string(),
            "docs/agents/planning.md".to_string(),
            "docs/agents/stack.md".to_string(),
        ]
    );
}

#[test]
fn placeholder_in_prose_is_found() {
    let found = placeholders_in("Allowed scopes: <scope list>.\n");
    assert_eq!(found, vec![(1, "scope list".to_string())]);
}

#[test]
fn placeholder_in_inline_code_is_found() {
    let found = placeholders_in("Run `<baseline test command>` first.\n");
    assert_eq!(found, vec![(1, "baseline test command".to_string())]);
}

#[test]
fn template_fence_curly_metavariables_and_autolinks_are_not_placeholders() {
    let text = "```template\n<main branch>\n```\nUse `{type}/{slug}`.\nSee <https://example.com> and <Capital>.\n";
    assert!(placeholders_in(text).is_empty());
}

#[test]
fn plain_fence_still_counts() {
    let text = "```text\n<main branch>\n```\n";
    assert_eq!(placeholders_in(text), vec![(2, "main branch".to_string())]);
}
