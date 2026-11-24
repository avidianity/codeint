use std::collections::{BTreeMap, BTreeSet};

use codeint_model::{CodeModel, ImportResolution, RepoPath, RuntimeImport, SourceFile, SourceSet};

use super::{Boundaries, BoundaryEdgeCheck, BoundaryFindingKind, BoundaryPolicy};
use crate::test_support::{empty_source_file, repo_path as path};

fn production_file(name: &str) -> SourceFile {
    empty_source_file(name, SourceSet::Production, 1)
}

struct StubPolicy {
    zones: BTreeMap<String, String>,
    public_files: BTreeMap<String, BTreeSet<String>>,
    decisions: BTreeMap<(String, String), BoundaryEdgeCheck>,
}

impl StubPolicy {
    fn new(zones: &[(&str, &str)]) -> StubPolicy {
        StubPolicy {
            zones: zones
                .iter()
                .map(|&(file, zone)| (file.to_owned(), zone.to_owned()))
                .collect(),
            public_files: BTreeMap::new(),
            decisions: BTreeMap::new(),
        }
    }

    fn with_decision(
        mut self,
        source_zone: &str,
        target_zone: &str,
        decision: BoundaryEdgeCheck,
    ) -> StubPolicy {
        self.decisions
            .insert((source_zone.to_owned(), target_zone.to_owned()), decision);
        self
    }

    fn with_public_files(mut self, zone: &str, files: &[&str]) -> StubPolicy {
        self.public_files.insert(
            zone.to_owned(),
            files.iter().map(|file| (*file).to_owned()).collect(),
        );
        self
    }
}

impl BoundaryPolicy for StubPolicy {
    fn zone_of(&self, target: &RepoPath) -> Option<&str> {
        self.zones.get(target.as_str()).map(String::as_str)
    }

    fn has_public_files(&self, zone: &str) -> bool {
        self.public_files.contains_key(zone)
    }

    fn is_public_file(&self, zone: &str, target: &RepoPath) -> bool {
        self.public_files
            .get(zone)
            .is_none_or(|allowed| allowed.contains(target.as_str()))
    }

    fn check_import(&self, source_zone: &str, target_zone: &str) -> BoundaryEdgeCheck {
        self.decisions
            .get(&(source_zone.to_owned(), target_zone.to_owned()))
            .cloned()
            .unwrap_or(BoundaryEdgeCheck::Allowed)
    }

    fn strict_zones(&self) -> bool {
        false
    }
}

fn model_with_multi_target_import(
    files: &[&str],
    importer: &str,
    specifier: &str,
    targets: &[&str],
) -> CodeModel {
    CodeModel {
        files: files.iter().copied().map(production_file).collect(),
        runtime_imports: vec![RuntimeImport {
            importer: path(importer),
            specifier: specifier.to_owned(),
            resolution: ImportResolution::Resolved(targets.iter().copied().map(path).collect()),
        }],
        ..CodeModel::default()
    }
}

#[test]
fn multi_file_import_flags_only_targets_on_the_forbidden_side() {
    let model = model_with_multi_target_import(
        &["ui/page.fld", "shared/public.fld", "shared/secret.fld"],
        "ui/page.fld",
        "shared",
        &["shared/public.fld", "shared/secret.fld"],
    );
    let policy = StubPolicy::new(&[
        ("ui/page.fld", "ui"),
        ("shared/public.fld", "public"),
        ("shared/secret.fld", "secret"),
    ])
    .with_decision(
        "ui",
        "secret",
        BoundaryEdgeCheck::DeniedByDenyRule {
            reason: Some("UI must not depend on secret files".to_owned()),
        },
    );

    let measured = Boundaries::measure(&model, &policy);

    assert_eq!(measured.resolved_imports(), 2);
    assert_eq!(measured.ratio(), 0.5);
    let findings = measured.findings();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].kind, BoundaryFindingKind::ZoneRule);
    assert_eq!(findings[0].importer, path("ui/page.fld"));
    assert_eq!(findings[0].target, Some(path("shared/secret.fld")));
    assert_eq!(findings[0].specifier.as_deref(), Some("shared"));
    assert_eq!(findings[0].source_zone.as_deref(), Some("ui"));
    assert_eq!(findings[0].target_zone.as_deref(), Some("secret"));
    assert_eq!(
        findings[0].reason.as_deref(),
        Some("UI must not depend on secret files")
    );
}

#[test]
fn multi_file_import_with_public_files_flags_only_non_public_targets() {
    let model = model_with_multi_target_import(
        &["ui/page.fld", "shared/public.fld", "shared/secret.fld"],
        "ui/page.fld",
        "shared",
        &["shared/public.fld", "shared/secret.fld"],
    );
    let policy = StubPolicy::new(&[
        ("ui/page.fld", "ui"),
        ("shared/public.fld", "shared"),
        ("shared/secret.fld", "shared"),
    ])
    .with_public_files("shared", &["shared/public.fld"]);

    let measured = Boundaries::measure(&model, &policy);

    assert_eq!(measured.resolved_imports(), 2);
    let findings = measured.findings();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].kind, BoundaryFindingKind::PublicFiles);
    assert_eq!(findings[0].importer, path("ui/page.fld"));
    assert_eq!(findings[0].target, Some(path("shared/secret.fld")));
    assert_eq!(findings[0].specifier.as_deref(), Some("shared"));
    assert_eq!(findings[0].source_zone.as_deref(), Some("ui"));
    assert_eq!(findings[0].target_zone.as_deref(), Some("shared"));
}

#[test]
fn multi_file_import_to_multiple_forbidden_targets_emits_distinct_stable_findings() {
    let policy = StubPolicy::new(&[
        ("ui/page.fld", "ui"),
        ("secret/a.fld", "secret"),
        ("secret/b.fld", "secret"),
    ])
    .with_decision(
        "ui",
        "secret",
        BoundaryEdgeCheck::DeniedByAllowRule {
            reason: Some("UI may import public modules only".to_owned()),
        },
    );
    let reversed = model_with_multi_target_import(
        &["ui/page.fld", "secret/a.fld", "secret/b.fld"],
        "ui/page.fld",
        "secret",
        &["secret/b.fld", "secret/a.fld"],
    );
    let ordered = model_with_multi_target_import(
        &["ui/page.fld", "secret/a.fld", "secret/b.fld"],
        "ui/page.fld",
        "secret",
        &["secret/a.fld", "secret/b.fld"],
    );

    let from_reversed = Boundaries::measure(&reversed, &policy);
    let from_ordered = Boundaries::measure(&ordered, &policy);

    assert_eq!(from_reversed.resolved_imports(), 2);
    assert_eq!(from_reversed.ratio(), 1.0);
    assert_eq!(from_reversed.findings(), from_ordered.findings());
    let findings = from_reversed.findings();
    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0].importer, path("ui/page.fld"));
    assert_eq!(findings[0].target, Some(path("secret/a.fld")));
    assert_eq!(findings[1].importer, path("ui/page.fld"));
    assert_eq!(findings[1].target, Some(path("secret/b.fld")));
    assert_ne!(findings[0].id, findings[1].id);
}
