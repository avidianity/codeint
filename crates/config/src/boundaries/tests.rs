use std::collections::BTreeMap;

use codeint_model::RepoPath;

use super::{
    Boundaries, BoundariesFile, BoundaryDirective, BoundaryRuleDecision, BoundaryRuleFile,
    BoundaryZoneFile,
};

fn repo_path(text: &str) -> RepoPath {
    RepoPath::new(text).expect("valid repo path")
}

fn to_strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

fn zone(patterns: &[&str]) -> BoundaryZoneFile {
    BoundaryZoneFile {
        patterns: to_strings(patterns),
        public_files: Vec::new(),
    }
}

fn base_rule(from: &str, reason: Option<&str>) -> BoundaryRuleFile {
    BoundaryRuleFile {
        from: from.to_owned(),
        allow: None,
        deny: None,
        allow_external: None,
        deny_external: None,
        reason: reason.map(str::to_owned),
    }
}

fn rule_allow(from: &str, allow: &[&str], reason: Option<&str>) -> BoundaryRuleFile {
    BoundaryRuleFile {
        allow: Some(to_strings(allow)),
        ..base_rule(from, reason)
    }
}

fn rule_deny(from: &str, deny: &[&str], reason: Option<&str>) -> BoundaryRuleFile {
    BoundaryRuleFile {
        deny: Some(to_strings(deny)),
        ..base_rule(from, reason)
    }
}

fn rule_deny_external(from: &str, deny: &[&str], reason: Option<&str>) -> BoundaryRuleFile {
    BoundaryRuleFile {
        deny_external: Some(to_strings(deny)),
        ..base_rule(from, reason)
    }
}

#[test]
fn combined_globset_preserves_zone_specificity_and_lexicographical_tie_breaking() {
    let mut zones = BTreeMap::new();
    zones.insert("broad".to_owned(), zone(&["src/**"]));
    zones.insert("alpha".to_owned(), zone(&["src/shared/**"]));
    zones.insert("beta".to_owned(), zone(&["src/shared/**"]));
    zones.insert("specific".to_owned(), zone(&["src/shared/core/**"]));

    let boundaries = Boundaries::from_file(BoundariesFile {
        zones,
        ..Default::default()
    })
    .expect("compile boundaries");

    assert_eq!(boundaries.zone_of(&repo_path("README.md")), None);
    assert_eq!(boundaries.zone_of(&repo_path("src/app.ts")), Some("broad"));
    assert_eq!(
        boundaries.zone_of(&repo_path("src/shared/util.ts")),
        Some("alpha")
    );
    assert_eq!(
        boundaries.zone_of(&repo_path("src/shared/core/id.ts")),
        Some("specific")
    );
}

#[test]
fn indexed_rules_preserve_specific_over_wildcard_precedence_and_reasons_at_scale() {
    let mut zones = BTreeMap::new();
    let mut rules = Vec::new();
    for idx in 0..80usize {
        let name = format!("zone_{idx:02}");
        zones.insert(name.clone(), zone(&[&format!("packages/{name}/**")]));
        if idx > 0 {
            let prev = format!("zone_{:02}", idx - 1);
            rules.push(rule_allow(
                &name,
                &[&prev],
                Some(&format!("{name} allows {prev}")),
            ));
        }
    }
    rules.push(rule_deny(
        "*",
        &["zone_79"],
        Some("wildcard denies zone_79"),
    ));
    rules.push(rule_deny_external(
        "*",
        &["left-pad"],
        Some("wildcard external deny"),
    ));
    rules.push(rule_deny_external(
        "zone_01",
        &["lodash*"],
        Some("zone_01 external deny"),
    ));

    let boundaries = Boundaries::from_file(BoundariesFile {
        zones,
        rules,
        ..Default::default()
    })
    .expect("compile large boundary index");

    assert_eq!(
        boundaries.zone_of(&repo_path("packages/zone_42/src/lib.ts")),
        Some("zone_42")
    );
    assert_eq!(
        boundaries.check_import("zone_42", "zone_41"),
        BoundaryRuleDecision::Allowed
    );
    assert_eq!(
        boundaries.allow_reason("zone_42", "zone_41"),
        Some("zone_42 allows zone_41")
    );
    assert_eq!(
        boundaries.check_import("zone_42", "zone_10"),
        BoundaryRuleDecision::Denied {
            directive: BoundaryDirective::Allow,
            reason: Some("zone_42 allows zone_41".to_owned()),
        }
    );
    assert_eq!(
        boundaries.check_import("zone_00", "zone_79"),
        BoundaryRuleDecision::Denied {
            directive: BoundaryDirective::Deny,
            reason: Some("wildcard denies zone_79".to_owned()),
        }
    );
    assert_eq!(
        boundaries.check_import("zone_00", "zone_01"),
        BoundaryRuleDecision::Allowed
    );
    assert_eq!(
        boundaries.check_external_import("zone_01", "lodash-es"),
        BoundaryRuleDecision::Denied {
            directive: BoundaryDirective::Deny,
            reason: Some("zone_01 external deny".to_owned()),
        }
    );
    assert_eq!(
        boundaries.check_external_import("zone_01", "left-pad"),
        BoundaryRuleDecision::Allowed
    );
    assert_eq!(
        boundaries.check_external_import("zone_02", "left-pad"),
        BoundaryRuleDecision::Denied {
            directive: BoundaryDirective::Deny,
            reason: Some("wildcard external deny".to_owned()),
        }
    );
}
