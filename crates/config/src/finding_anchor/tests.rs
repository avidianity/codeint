use super::{FindingAnchor, match_anchors};

fn anchor(
    id: &str,
    content: &str,
    before: Option<&str>,
    after: Option<&str>,
    line: u32,
) -> FindingAnchor {
    FindingAnchor {
        id: id.to_owned(),
        content: Some(content.to_owned()),
        before: before.map(str::to_owned),
        after: after.map(str::to_owned),
        line: Some(line),
    }
}

fn assert_pairs(refs: &[FindingAnchor], cands: &[FindingAnchor], expected: &[(usize, usize)]) {
    assert_eq!(match_anchors(refs, cands).matched_pairs, expected);
}

#[test]
fn single_and_multi_occurrence_context_anchor_matching_rules() {
    assert_pairs(
        &[anchor("id0", "same", Some("b1"), Some("a1"), 2)],
        &[anchor("id0", "same", Some("b2"), Some("a2"), 2)],
        &[(0, 0)],
    );
    assert_pairs(
        &[anchor("occ0", "del", Some("sel1"), None, 2)],
        &[
            anchor("occ0", "del", None, Some("sel1"), 1),
            anchor("occ1", "del", Some("sel1"), None, 3),
        ],
        &[(0, 1)],
    );
    assert_pairs(
        &[anchor("occ0", "del", Some("sel1"), None, 2)],
        &[
            anchor("occ0", "del", Some("sel1"), Some("del"), 2),
            anchor("occ1", "del", Some("del"), None, 3),
        ],
        &[(0, 0)],
    );
    assert_pairs(
        &[anchor("occ0", "del", None, Some("sel1"), 1)],
        &[
            anchor("occ0", "del", None, Some("del"), 1),
            anchor("occ1", "del", Some("del"), Some("sel1"), 2),
        ],
        &[(0, 1)],
    );
    assert_pairs(
        &[FindingAnchor::id_only("legacy-id")],
        &[
            anchor("other-id", "c1", Some("x"), Some("y"), 1),
            anchor("legacy-id", "c2", Some("p"), Some("q"), 5),
        ],
        &[(0, 1)],
    );
    assert_pairs(
        &[
            anchor("r0", "same", Some("a"), Some("b"), 2),
            anchor("r1", "same", Some("a"), Some("c"), 5),
        ],
        &[
            anchor("c0", "same", Some("a"), Some("x"), 5),
            anchor("c1", "same", Some("y"), Some("c"), 6),
        ],
        &[(0, 0), (1, 1)],
    );
}

#[test]
fn large_duplicate_groups_match_with_bucketed_fallback() {
    let refs: Vec<FindingAnchor> = (1..=70)
        .map(|line| {
            anchor(
                &format!("r{line}"),
                "dup",
                Some(&format!("b{line}")),
                Some(&format!("a{line}")),
                line,
            )
        })
        .collect();
    let mut cands = vec![anchor("new0", "dup", None, Some("b1"), 1)];
    for line in 1..=70 {
        cands.push(anchor(
            &format!("c{line}"),
            "dup",
            Some(&format!("b{line}")),
            Some(&format!("a{line}")),
            line + 1,
        ));
    }

    let matched = match_anchors(&refs, &cands);
    assert_eq!(matched.matched_pairs.len(), 70);
    assert!(!matched.matched_candidates(cands.len())[0]);
}
