use std::collections::HashSet;

use codeint_model::{CodeModel, ImportResolution, Metric, RepoPath, token_text_hash};

use crate::FindingId;
use crate::finding_id::{FindingContext, FindingIds};

#[cfg(test)]
mod tests;

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

    fn has_external_rules(&self) -> bool;

    fn check_external_import(&self, source_zone: &str, package: &str) -> BoundaryEdgeCheck;

    fn strict_zones(&self) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoundaryFindingKind {
    ZoneRule,
    ExternalPackage,
    PublicFiles,
    UnzonedFile,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundaryFinding {
    pub id: FindingId,
    pub context: FindingContext,
    pub kind: BoundaryFindingKind,
    pub importer: RepoPath,
    pub target: Option<RepoPath>,
    pub external_package: Option<String>,
    pub specifier: Option<String>,
    pub source_zone: Option<String>,
    pub target_zone: Option<String>,
    pub reason: Option<String>,
    pub message: String,
}

impl BoundaryFindingKind {
    pub fn key(self) -> &'static str {
        match self {
            BoundaryFindingKind::ZoneRule => "zone_rule",
            BoundaryFindingKind::ExternalPackage => "external_package",
            BoundaryFindingKind::PublicFiles => "public_files",
            BoundaryFindingKind::UnzonedFile => "unzoned_file",
        }
    }
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
    external_package: Option<String>,
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
    let check_external = policy.has_external_rules();
    for runtime_import in &model.runtime_imports {
        match &runtime_import.resolution {
            ImportResolution::Resolved(targets) => {
                resolved_imports += evaluate_resolved_targets(
                    &runtime_import.importer,
                    targets,
                    &runtime_import.specifier,
                    policy,
                    &mut pending,
                );
            }
            ImportResolution::External(package) if check_external => {
                resolved_imports += 1;
                if let Some(finding) = check_external_import_edge(
                    &runtime_import.importer,
                    package,
                    &runtime_import.specifier,
                    policy,
                ) {
                    pending.push(finding);
                }
            }
            ImportResolution::External(_)
            | ImportResolution::Builtin
            | ImportResolution::Unresolved { .. } => {}
        }
    }
    (resolved_imports, pending)
}

fn evaluate_resolved_targets(
    importer: &RepoPath,
    targets: &[RepoPath],
    specifier: &str,
    policy: &dyn BoundaryPolicy,
    pending: &mut Vec<PendingFinding>,
) -> usize {
    let mut counted = 0usize;
    let mut seen_targets = HashSet::with_capacity(targets.len());
    for target in targets {
        if !seen_targets.insert(target) {
            continue;
        }
        counted += 1;
        if let Some(finding) = check_import_edge(importer, target, specifier, policy) {
            pending.push(finding);
        }
    }
    counted
}

fn check_external_import_edge(
    importer: &RepoPath,
    package: &str,
    specifier: &str,
    policy: &dyn BoundaryPolicy,
) -> Option<PendingFinding> {
    let source_zone = policy.zone_of(importer)?;
    match policy.check_external_import(source_zone, package) {
        BoundaryEdgeCheck::Allowed => None,
        BoundaryEdgeCheck::DeniedByDenyRule { reason } => Some(external_package_finding(
            importer,
            package,
            specifier,
            source_zone,
            "is denied from importing external package",
            reason,
        )),
        BoundaryEdgeCheck::DeniedByAllowRule { reason } => Some(external_package_finding(
            importer,
            package,
            specifier,
            source_zone,
            "is not allowed to import external package",
            reason,
        )),
    }
}

fn external_package_finding(
    importer: &RepoPath,
    package: &str,
    specifier: &str,
    source_zone: &str,
    verb: &str,
    reason: Option<String>,
) -> PendingFinding {
    PendingFinding {
        kind: BoundaryFindingKind::ExternalPackage,
        importer: importer.clone(),
        target: None,
        external_package: Some(package.to_owned()),
        specifier: Some(specifier.to_owned()),
        source_zone: Some(source_zone.to_owned()),
        target_zone: None,
        message: format_external_rule_message(source_zone, package, verb, reason.as_deref()),
        reason,
    }
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
        external_package: None,
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
        external_package: None,
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
                external_package: None,
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
            .then_with(|| left.external_package.cmp(&right.external_package))
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.specifier.cmp(&right.specifier))
    });
    let mut ids = FindingIds::new(Metric::Boundaries);
    pending
        .into_iter()
        .map(|item| {
            let kind_hash = token_text_hash(item.kind.key());
            let (id, content_hash) = match (&item.target, &item.external_package) {
                (Some(target), _) => {
                    ids.next_with_content_hash([&item.importer, target], [kind_hash])
                }
                (None, Some(package)) => ids.next_with_content_hash(
                    [&item.importer],
                    [kind_hash, token_text_hash(package)],
                ),
                (None, None) => ids.next_with_content_hash([&item.importer], [kind_hash]),
            };
            BoundaryFinding {
                id,
                context: FindingContext::content_only(content_hash),
                kind: item.kind,
                importer: item.importer,
                target: item.target,
                external_package: item.external_package,
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

fn format_external_rule_message(
    source_zone: &str,
    package: &str,
    verb: &str,
    reason: Option<&str>,
) -> String {
    match reason {
        Some(reason) => format!("zone {source_zone:?} {verb} {package:?}: {reason}"),
        None => format!("zone {source_zone:?} {verb} {package:?}"),
    }
}
