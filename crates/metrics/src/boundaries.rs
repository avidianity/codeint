use codeint_model::{CodeModel, ImportResolution, RepoPath, token_text_hash};

use crate::FindingId;
use crate::finding_id::FindingIds;

const BOUNDARIES_METRIC_KEY: &str = "boundaries";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoundaryEdgeCheck {
    Allowed,
    DeniedByAllowRule { reason: Option<String> },
    DeniedByDenyRule { reason: Option<String> },
}

pub trait BoundaryPolicy {
    fn zone_of(&self, path: &RepoPath) -> Option<&str>;

    fn has_public_files(&self, zone: &str) -> bool;

    fn is_public_file(&self, zone: &str, path: &RepoPath) -> bool;

    fn check_import(&self, source_zone: &str, target_zone: &str) -> BoundaryEdgeCheck;

    fn strict_zones(&self) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoundaryFindingKind {
    ZoneRule,
    PublicFiles,
    UnzonedFile,
}

impl BoundaryFindingKind {
    pub fn key(self) -> &'static str {
        match self {
            BoundaryFindingKind::ZoneRule => "zone_rule",
            BoundaryFindingKind::PublicFiles => "public_files",
            BoundaryFindingKind::UnzonedFile => "unzoned_file",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundaryFinding {
    pub id: FindingId,
    pub kind: BoundaryFindingKind,
    pub importer: RepoPath,
    pub target: Option<RepoPath>,
    pub specifier: Option<String>,
    pub source_zone: Option<String>,
    pub target_zone: Option<String>,
    pub reason: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Boundaries {
    resolved_imports: usize,
    findings: Vec<BoundaryFinding>,
}

struct PendingFinding {
    kind: BoundaryFindingKind,
    importer: RepoPath,
    target: Option<RepoPath>,
    specifier: Option<String>,
    source_zone: Option<String>,
    target_zone: Option<String>,
    reason: Option<String>,
    message: String,
}

impl Boundaries {
    pub fn measure(model: &CodeModel, policy: &dyn BoundaryPolicy) -> Boundaries {
        let mut resolved_imports = 0usize;
        let mut pending = Vec::new();

        for runtime_import in &model.runtime_imports {
            let ImportResolution::Resolved(target) = &runtime_import.resolution else {
                continue;
            };
            resolved_imports += 1;
            let (Some(source_zone), Some(target_zone)) = (
                policy.zone_of(&runtime_import.importer),
                policy.zone_of(target),
            ) else {
                continue;
            };
            if source_zone == target_zone {
                continue;
            }
            match policy.check_import(source_zone, target_zone) {
                BoundaryEdgeCheck::DeniedByDenyRule { reason } => {
                    pending.push(PendingFinding {
                        kind: BoundaryFindingKind::ZoneRule,
                        importer: runtime_import.importer.clone(),
                        target: Some(target.clone()),
                        specifier: Some(runtime_import.specifier.clone()),
                        source_zone: Some(source_zone.to_owned()),
                        target_zone: Some(target_zone.to_owned()),
                        message: format_zone_rule_message(
                            source_zone,
                            target_zone,
                            "is denied from importing",
                            reason.as_deref(),
                        ),
                        reason,
                    });
                }
                BoundaryEdgeCheck::DeniedByAllowRule { reason } => {
                    pending.push(PendingFinding {
                        kind: BoundaryFindingKind::ZoneRule,
                        importer: runtime_import.importer.clone(),
                        target: Some(target.clone()),
                        specifier: Some(runtime_import.specifier.clone()),
                        source_zone: Some(source_zone.to_owned()),
                        target_zone: Some(target_zone.to_owned()),
                        message: format_zone_rule_message(
                            source_zone,
                            target_zone,
                            "is not allowed to import",
                            reason.as_deref(),
                        ),
                        reason,
                    });
                }
                BoundaryEdgeCheck::Allowed => {
                    if policy.has_public_files(target_zone)
                        && !policy.is_public_file(target_zone, target)
                    {
                        pending.push(PendingFinding {
                            kind: BoundaryFindingKind::PublicFiles,
                            importer: runtime_import.importer.clone(),
                            target: Some(target.clone()),
                            specifier: Some(runtime_import.specifier.clone()),
                            source_zone: Some(source_zone.to_owned()),
                            target_zone: Some(target_zone.to_owned()),
                            reason: None,
                            message: format!(
                                "zone {source_zone:?} deep-imports non-public file {target} of zone {target_zone:?}"
                            ),
                        });
                    }
                }
            }
        }

        if policy.strict_zones() {
            for file in &model.files {
                if policy.zone_of(&file.path).is_none() {
                    pending.push(PendingFinding {
                        kind: BoundaryFindingKind::UnzonedFile,
                        importer: file.path.clone(),
                        target: None,
                        specifier: None,
                        source_zone: None,
                        target_zone: None,
                        reason: None,
                        message: format!(
                            "file {} does not belong to any configured zone",
                            file.path
                        ),
                    });
                }
            }
        }

        pending.sort_by(|left, right| {
            left.importer
                .cmp(&right.importer)
                .then_with(|| left.target.cmp(&right.target))
                .then_with(|| left.kind.cmp(&right.kind))
                .then_with(|| left.specifier.cmp(&right.specifier))
        });

        let mut ids = FindingIds::for_key(BOUNDARIES_METRIC_KEY);
        let findings = pending
            .into_iter()
            .map(|item| {
                let kind_hash = token_text_hash(item.kind.key());
                let id = match &item.target {
                    Some(target) => ids.next_in_source_order([&item.importer, target], [kind_hash]),
                    None => ids.next_in_source_order([&item.importer], [kind_hash]),
                };
                BoundaryFinding {
                    id,
                    kind: item.kind,
                    importer: item.importer,
                    target: item.target,
                    specifier: item.specifier,
                    source_zone: item.source_zone,
                    target_zone: item.target_zone,
                    reason: item.reason,
                    message: item.message,
                }
            })
            .collect();

        Boundaries {
            resolved_imports,
            findings,
        }
    }

    pub fn ratio(&self) -> f64 {
        if self.findings.is_empty() {
            0.0
        } else if self.resolved_imports == 0 {
            1.0
        } else {
            (self.findings.len() as f64 / self.resolved_imports as f64).min(1.0)
        }
    }

    pub fn resolved_imports(&self) -> usize {
        self.resolved_imports
    }

    pub fn findings(&self) -> &[BoundaryFinding] {
        &self.findings
    }
}

fn format_zone_rule_message(
    source_zone: &str,
    target_zone: &str,
    verb: &str,
    reason: Option<&str>,
) -> String {
    match reason {
        Some(reason) => format!("zone {source_zone:?} {verb} zone {target_zone:?}: {reason}"),
        None => format!("zone {source_zone:?} {verb} zone {target_zone:?}"),
    }
}
