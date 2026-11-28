use std::fmt::Write as _;

use codeint_config::{
    Boundaries as BoundariesConfig, BoundaryDirective, BoundaryExternalPolicy,
    BoundaryRuleDecision, GuardFormat,
};
use codeint_model::RepoPath;
use serde::Serialize;

use crate::{ReportError, render_text_report};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GuardReport {
    path: String,
    exists: bool,
    configured: bool,
    strict_zones: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_public_file: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    public_files: Vec<String>,
    allowed_targets: Vec<GuardAllowedTarget>,
    denied_targets: Vec<GuardDeniedTarget>,
    allowed_importers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    external_imports: Option<GuardExternalImports>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GuardExternalImports {
    directive: &'static str,
    patterns: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GuardAllowedTarget {
    zone: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    public_files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GuardDeniedTarget {
    zone: String,
    directive: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

impl GuardReport {
    pub fn evaluate(
        path: &RepoPath,
        exists: bool,
        boundaries: Option<&BoundariesConfig>,
    ) -> GuardReport {
        let Some(boundaries) = boundaries else {
            return GuardReport {
                path: path.as_str().to_owned(),
                exists,
                configured: false,
                strict_zones: false,
                zone: None,
                is_public_file: None,
                public_files: Vec::new(),
                allowed_targets: Vec::new(),
                denied_targets: Vec::new(),
                allowed_importers: Vec::new(),
                external_imports: None,
                note: Some("No boundaries are configured in the configuration file.".to_owned()),
            };
        };

        let strict_zones = boundaries.strict_zones();
        let Some(matched_zone) = boundaries.zone_of(path) else {
            let note = if strict_zones {
                "Path does not match any configured zone; strict_zones is enabled, so this file will be flagged as unzoned."
            } else {
                "Path does not match any configured zone; no boundary rules apply."
            };
            return GuardReport {
                path: path.as_str().to_owned(),
                exists,
                configured: true,
                strict_zones,
                zone: None,
                is_public_file: None,
                public_files: Vec::new(),
                allowed_targets: Vec::new(),
                denied_targets: Vec::new(),
                allowed_importers: Vec::new(),
                external_imports: None,
                note: Some(note.to_owned()),
            };
        };

        let public_files = boundaries.public_file_patterns(matched_zone).to_vec();
        let is_public_file = boundaries.is_public_file(matched_zone, path);
        let all_zones = boundaries.zone_names();
        let (allowed_targets, denied_targets) =
            collect_target_decisions(boundaries, matched_zone, &all_zones);
        let allowed_importers =
            collect_allowed_importers(boundaries, matched_zone, is_public_file, &all_zones);
        let note = (!is_public_file).then(|| {
            format!(
                "This file is private to zone \"{matched_zone}\"; other zones may only import files matching public_files."
            )
        });
        let external_imports = boundaries
            .external_policy_for(matched_zone)
            .map(guard_external_imports);

        GuardReport {
            path: path.as_str().to_owned(),
            exists,
            configured: true,
            strict_zones,
            zone: Some(matched_zone.to_owned()),
            is_public_file: Some(is_public_file),
            public_files,
            allowed_targets,
            denied_targets,
            allowed_importers,
            external_imports,
            note,
        }
    }

    pub fn render(&self, format: GuardFormat) -> Result<String, ReportError> {
        render_text_report(self, format.into(), GuardReport::render_text)
    }

    fn render_text(&self) -> String {
        let mut out = String::new();
        let status = if self.exists {
            "exists"
        } else {
            "planned, does not exist on disk"
        };
        let _ = writeln!(out, "path: {} ({status})", self.path);
        if !self.configured {
            let _ = writeln!(out, "boundaries: not configured");
            if let Some(note) = &self.note {
                let _ = write!(out, "note: {note}");
            }
            return out.trim_end_matches('\n').to_owned();
        }
        match &self.zone {
            Some(zone) => self.write_zone_details(&mut out, zone),
            None => {
                let _ = writeln!(out, "zone: none");
            }
        }
        if let Some(note) = &self.note {
            let _ = writeln!(out, "note: {note}");
        }
        out.trim_end_matches('\n').to_owned()
    }

    fn write_zone_details(&self, out: &mut String, zone: &str) {
        let _ = writeln!(out, "zone: {zone}");
        self.write_public_files(out);
        self.write_allowed_targets(out);
        self.write_denied_targets(out);
        self.write_allowed_importers(out);
        self.write_external_imports(out);
    }

    fn write_external_imports(&self, out: &mut String) {
        let Some(external) = &self.external_imports else {
            return;
        };
        let pattern_list = if external.patterns.is_empty() {
            "[] (built-ins only)".to_owned()
        } else {
            external.patterns.join(", ")
        };
        match &external.reason {
            Some(reason) => {
                let _ = writeln!(
                    out,
                    "external_imports: {} {pattern_list} ({reason})",
                    external.directive
                );
            }
            None => {
                let _ = writeln!(
                    out,
                    "external_imports: {} {pattern_list}",
                    external.directive
                );
            }
        }
    }

    fn write_public_files(&self, out: &mut String) {
        if self.public_files.is_empty() {
            return;
        }
        let visibility = if self.is_public_file == Some(true) {
            "this file is public"
        } else {
            "this file is private"
        };
        let _ = writeln!(
            out,
            "public_files: {} ({visibility})",
            self.public_files.join(", ")
        );
    }

    fn write_allowed_targets(&self, out: &mut String) {
        if self.allowed_targets.is_empty() {
            let _ = writeln!(out, "allowed_targets: none");
            return;
        }
        let _ = writeln!(out, "allowed_targets:");
        for target in &self.allowed_targets {
            let _ = writeln!(out, "  - {}", format_allowed_target(target));
        }
    }

    fn write_denied_targets(&self, out: &mut String) {
        if self.denied_targets.is_empty() {
            let _ = writeln!(out, "denied_targets: none");
            return;
        }
        let _ = writeln!(out, "denied_targets:");
        for target in &self.denied_targets {
            match &target.reason {
                Some(reason) => {
                    let _ = writeln!(out, "  - {} ({}: {reason})", target.zone, target.directive);
                }
                None => {
                    let _ = writeln!(out, "  - {} ({})", target.zone, target.directive);
                }
            }
        }
    }

    fn write_allowed_importers(&self, out: &mut String) {
        if self.allowed_importers.is_empty() {
            let _ = writeln!(out, "allowed_importers: none");
        } else {
            let _ = writeln!(
                out,
                "allowed_importers: {}",
                self.allowed_importers.join(", ")
            );
        }
    }
}

fn format_allowed_target(target: &GuardAllowedTarget) -> String {
    let mut details = Vec::new();
    if !target.public_files.is_empty() {
        details.push(format!("public_files: {}", target.public_files.join(", ")));
    }
    if let Some(reason) = &target.reason {
        details.push(format!("reason: {reason}"));
    }
    if details.is_empty() {
        target.zone.clone()
    } else {
        format!("{} ({})", target.zone, details.join("; "))
    }
}

fn directive_label(directive: BoundaryDirective) -> &'static str {
    match directive {
        BoundaryDirective::Allow => "allow",
        BoundaryDirective::Deny => "deny",
    }
}

fn collect_target_decisions(
    boundaries: &BoundariesConfig,
    matched_zone: &str,
    all_zones: &[&str],
) -> (Vec<GuardAllowedTarget>, Vec<GuardDeniedTarget>) {
    let mut allowed_targets = Vec::new();
    let mut denied_targets = Vec::new();
    for &target_zone in all_zones {
        match boundaries.check_import(matched_zone, target_zone) {
            BoundaryRuleDecision::Allowed => {
                let target_public_files = if target_zone == matched_zone {
                    Vec::new()
                } else {
                    boundaries.public_file_patterns(target_zone).to_vec()
                };
                let reason = boundaries
                    .allow_reason(matched_zone, target_zone)
                    .map(str::to_owned);
                allowed_targets.push(GuardAllowedTarget {
                    zone: target_zone.to_owned(),
                    public_files: target_public_files,
                    reason,
                });
            }
            BoundaryRuleDecision::Denied { directive, reason } => {
                denied_targets.push(GuardDeniedTarget {
                    zone: target_zone.to_owned(),
                    directive: directive_label(directive),
                    reason,
                });
            }
        }
    }
    (allowed_targets, denied_targets)
}

fn collect_allowed_importers(
    boundaries: &BoundariesConfig,
    matched_zone: &str,
    is_public_file: bool,
    all_zones: &[&str],
) -> Vec<String> {
    all_zones
        .iter()
        .copied()
        .filter(|&importer_zone| {
            importer_zone == matched_zone
                || (is_public_file
                    && matches!(
                        boundaries.check_import(importer_zone, matched_zone),
                        BoundaryRuleDecision::Allowed
                    ))
        })
        .map(str::to_owned)
        .collect()
}

fn guard_external_imports(policy: BoundaryExternalPolicy) -> GuardExternalImports {
    let directive = match policy.directive {
        BoundaryDirective::Allow => "allow_external",
        BoundaryDirective::Deny => "deny_external",
    };
    GuardExternalImports {
        directive,
        patterns: policy.patterns,
        reason: policy.reason,
    }
}
