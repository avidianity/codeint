use codeint_model::{CodeModel, ImportResolution, RepoPath, token_text_hash};

use crate::FindingId;
use crate::finding_id::FindingIds;

#[cfg(test)]
mod tests;

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
        let (resolved_imports, mut pending) = evaluate_imports(model, policy);
        collect_unzoned_files(model, policy, &mut pending);
        let findings = assign_finding_ids(pending);
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

fn evaluate_imports(
    model: &CodeModel,
    policy: &dyn BoundaryPolicy,
) -> (usize, Vec<PendingFinding>) {
    let mut resolved_imports = 0usize;
    let mut pending = Vec::new();
    for runtime_import in &model.runtime_imports {
        let ImportResolution::Resolved(targets) = &runtime_import.resolution else {
            continue;
        };
        for target in targets {
            resolved_imports += 1;
            if let Some(finding) = check_import_edge(
                &runtime_import.importer,
                target,
                &runtime_import.specifier,
                policy,
            ) {
                pending.push(finding);
            }
        }
    }
    (resolved_imports, pending)
}

fn check_import_edge(
    importer: &RepoPath,
    target: &RepoPath,
    specifier: &str,
    policy: &dyn BoundaryPolicy,
) -> Option<PendingFinding> {
    let (Some(source_zone), Some(target_zone)) = (policy.zone_of(importer), policy.zone_of(target))
    else {
        return None;
    };
    if source_zone == target_zone {
        return None;
    }
    match policy.check_import(source_zone, target_zone) {
        BoundaryEdgeCheck::DeniedByDenyRule { reason } => Some(zone_rule_finding(
            importer,
            target,
            specifier,
            source_zone,
            target_zone,
            "is denied from importing",
            reason,
        )),
        BoundaryEdgeCheck::DeniedByAllowRule { reason } => Some(zone_rule_finding(
            importer,
            target,
            specifier,
            source_zone,
            target_zone,
            "is not allowed to import",
            reason,
        )),
        BoundaryEdgeCheck::Allowed => check_public_files_edge(
            importer,
            target,
            specifier,
            source_zone,
            target_zone,
            policy,
        ),
    }
}

fn zone_rule_finding(
    importer: &RepoPath,
    target: &RepoPath,
    specifier: &str,
    source_zone: &str,
    target_zone: &str,
    verb: &str,
    reason: Option<String>,
) -> PendingFinding {
    PendingFinding {
        kind: BoundaryFindingKind::ZoneRule,
        importer: importer.clone(),
        target: Some(target.clone()),
        specifier: Some(specifier.to_owned()),
        source_zone: Some(source_zone.to_owned()),
        target_zone: Some(target_zone.to_owned()),
        message: format_zone_rule_message(source_zone, target_zone, verb, reason.as_deref()),
        reason,
    }
}

fn check_public_files_edge(
    importer: &RepoPath,
    target: &RepoPath,
    specifier: &str,
    source_zone: &str,
    target_zone: &str,
    policy: &dyn BoundaryPolicy,
) -> Option<PendingFinding> {
    if !policy.has_public_files(target_zone) || policy.is_public_file(target_zone, target) {
        return None;
    }
    Some(PendingFinding {
        kind: BoundaryFindingKind::PublicFiles,
        importer: importer.clone(),
        target: Some(target.clone()),
        specifier: Some(specifier.to_owned()),
        source_zone: Some(source_zone.to_owned()),
        target_zone: Some(target_zone.to_owned()),
        reason: None,
        message: format!(
            "zone {source_zone:?} deep-imports non-public file {target} of zone {target_zone:?}"
        ),
    })
}

fn collect_unzoned_files(
    model: &CodeModel,
    policy: &dyn BoundaryPolicy,
    pending: &mut Vec<PendingFinding>,
) {
    if !policy.strict_zones() {
        return;
    }
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
                message: format!("file {} does not belong to any configured zone", file.path),
            });
        }
    }
}

fn assign_finding_ids(mut pending: Vec<PendingFinding>) -> Vec<BoundaryFinding> {
    pending.sort_by(|left, right| {
        left.importer
            .cmp(&right.importer)
            .then_with(|| left.target.cmp(&right.target))
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.specifier.cmp(&right.specifier))
    });
    let mut ids = FindingIds::for_key(BOUNDARIES_METRIC_KEY);
    pending
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
        .collect()
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
