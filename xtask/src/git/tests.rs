use super::parse_unified_diff;

#[test]
fn an_added_line_that_starts_with_two_plus_signs_is_kept_with_its_number() {
    let diff = "diff --git a/a.js b/a.js\n--- a/a.js\n+++ b/a.js\n@@ -3,2 +3,3 @@\n-old\n--i;\n+++i;\n+// added comment\n+x\n";
    let parsed = parse_unified_diff(diff);
    assert_eq!(
        parsed.added,
        vec![
            (3, "++i;".to_string()),
            (4, "// added comment".to_string()),
            (5, "x".to_string()),
        ]
    );
    assert_eq!(
        parsed.removed,
        vec![(3, "old".to_string()), (4, "-i;".to_string())]
    );
}
