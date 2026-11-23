use std::fmt::Write;

use codeint_config::ImpactFormat;
use codeint_metrics::{ImpactAnalysis, ImpactDependent, ImpactSummary, ImpactTarget};
use codeint_model::SourceSet;
use serde::Serialize;

use crate::ReportError;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ChangedBase {
    #[serde(rename = "ref")]
    pub git_ref: String,
    pub merge_base: String,
}

#[derive(Debug, Serialize)]
pub struct ImpactReport {
    targets: Vec<ImpactTargetRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    changed_base: Option<ChangedBase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    depth_limit: Option<usize>,
    summary: ImpactSummaryRow,
    production: SourceSetImpact,
    test: SourceSetImpact,
    #[serde(skip_serializing_if = "SourceSetImpact::is_empty")]
    generated: SourceSetImpact,
    #[serde(skip_serializing_if = "SourceSetImpact::is_empty")]
    vendored: SourceSetImpact,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    affected_packages: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ImpactTargetRow {
    path: String,
    source_set: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    package: Option<String>,
    fan_in: usize,
    fan_out: usize,
    transitive_import_weight: usize,
}

#[derive(Debug, Serialize)]
struct ImpactSummaryRow {
    direct_dependents: usize,
    transitive_dependents: usize,
    total_dependents: usize,
    fan_in: usize,
    fan_out: usize,
    transitive_import_weight: usize,
}

#[derive(Debug, Default, Serialize)]
struct SourceSetImpact {
    direct: Vec<ImpactDependentRow>,
    transitive: Vec<ImpactDependentRow>,
}

impl SourceSetImpact {
    fn is_empty(&self) -> bool {
        self.direct.is_empty() && self.transitive.is_empty()
    }
}

#[derive(Debug, Serialize)]
struct ImpactDependentRow {
    path: String,
    distance: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    package: Option<String>,
}

impl ImpactReport {
    pub fn from_analysis(
        analysis: ImpactAnalysis,
        changed_base: Option<ChangedBase>,
    ) -> ImpactReport {
        let mut production = SourceSetImpact::default();
        let mut test = SourceSetImpact::default();
        let mut generated = SourceSetImpact::default();
        let mut vendored = SourceSetImpact::default();

        for dependent in analysis.dependents {
            let bucket = match dependent.source_set {
                SourceSet::Production => &mut production,
                SourceSet::Test => &mut test,
                SourceSet::Generated => &mut generated,
                SourceSet::Vendored => &mut vendored,
            };
            let row = ImpactDependentRow::from_dependent(dependent);
            if row.distance == 1 {
                bucket.direct.push(row);
            } else {
                bucket.transitive.push(row);
            }
        }

        ImpactReport {
            targets: analysis
                .targets
                .into_iter()
                .map(ImpactTargetRow::from_target)
                .collect(),
            changed_base,
            depth_limit: analysis.depth_limit,
            summary: ImpactSummaryRow::from_summary(analysis.summary),
            production,
            test,
            generated,
            vendored,
            affected_packages: analysis.affected_packages,
        }
    }

    pub fn render(&self, format: ImpactFormat) -> Result<String, ReportError> {
        match format {
            ImpactFormat::Toon => toon_format::encode_default(self).map_err(ReportError::Toon),
            ImpactFormat::Json => serde_json::to_string_pretty(self).map_err(ReportError::Json),
            ImpactFormat::Text => Ok(render_text(self)),
        }
    }
}

impl ImpactTargetRow {
    fn from_target(target: ImpactTarget) -> ImpactTargetRow {
        ImpactTargetRow {
            path: target.path.to_string(),
            source_set: source_set_key(target.source_set),
            package: target.package,
            fan_in: target.fan_in,
            fan_out: target.fan_out,
            transitive_import_weight: target.transitive_import_weight,
        }
    }
}

impl ImpactSummaryRow {
    fn from_summary(summary: ImpactSummary) -> ImpactSummaryRow {
        ImpactSummaryRow {
            direct_dependents: summary.direct_dependents,
            transitive_dependents: summary.transitive_dependents,
            total_dependents: summary.total_dependents,
            fan_in: summary.fan_in,
            fan_out: summary.fan_out,
            transitive_import_weight: summary.transitive_import_weight,
        }
    }
}

impl ImpactDependentRow {
    fn from_dependent(dependent: ImpactDependent) -> ImpactDependentRow {
        ImpactDependentRow {
            path: dependent.path.to_string(),
            distance: dependent.distance,
            package: dependent.package,
        }
    }
}

fn source_set_key(source_set: SourceSet) -> &'static str {
    match source_set {
        SourceSet::Production => "production",
        SourceSet::Test => "test",
        SourceSet::Generated => "generated",
        SourceSet::Vendored => "vendored",
    }
}

fn render_text(report: &ImpactReport) -> String {
    let mut out = String::new();
    if let Some(base) = &report.changed_base {
        let _ = writeln!(
            out,
            "Changed base: {} (merge-base {})",
            base.git_ref, base.merge_base
        );
    }
    if let Some(depth) = report.depth_limit {
        let _ = writeln!(out, "Depth limit: {depth}");
    }
    let _ = writeln!(out, "Targets ({}):", report.targets.len());
    if report.targets.is_empty() {
        let _ = writeln!(out, "  (none)");
    } else {
        for target in &report.targets {
            let pkg = target
                .package
                .as_deref()
                .map(|p| format!(" [{p}]"))
                .unwrap_or_default();
            let _ = writeln!(
                out,
                "  {} ({}){} fan-in={} fan-out={} weight={}",
                target.path,
                target.source_set,
                pkg,
                target.fan_in,
                target.fan_out,
                target.transitive_import_weight
            );
        }
    }
    let _ = writeln!(
        out,
        "Summary: {} direct, {} transitive, {} total dependents (fan-in={}, fan-out={}, weight={})",
        report.summary.direct_dependents,
        report.summary.transitive_dependents,
        report.summary.total_dependents,
        report.summary.fan_in,
        report.summary.fan_out,
        report.summary.transitive_import_weight
    );
    write_source_set_section(&mut out, "Production dependents", &report.production);
    write_source_set_section(&mut out, "Test dependents", &report.test);
    if !report.generated.is_empty() {
        write_source_set_section(&mut out, "Generated dependents", &report.generated);
    }
    if !report.vendored.is_empty() {
        write_source_set_section(&mut out, "Vendored dependents", &report.vendored);
    }
    if !report.affected_packages.is_empty() {
        let _ = writeln!(
            out,
            "Affected Workspace packages ({}):",
            report.affected_packages.len()
        );
        for pkg in &report.affected_packages {
            let _ = writeln!(out, "  {pkg}");
        }
    }
    out.trim_end().to_owned()
}

fn write_source_set_section(out: &mut String, heading: &str, section: &SourceSetImpact) {
    let total = section.direct.len() + section.transitive.len();
    let _ = writeln!(out, "{heading} ({total}):");
    if total == 0 {
        let _ = writeln!(out, "  (none)");
        return;
    }
    if !section.direct.is_empty() {
        let _ = writeln!(out, "  Direct (distance 1):");
        for dep in &section.direct {
            write_dependent_line(out, dep);
        }
    }
    if !section.transitive.is_empty() {
        let _ = writeln!(out, "  Transitive (distance >= 2):");
        for dep in &section.transitive {
            write_dependent_line(out, dep);
        }
    }
}

fn write_dependent_line(out: &mut String, dep: &ImpactDependentRow) {
    let pkg = dep
        .package
        .as_deref()
        .map(|p| format!(" [{p}]"))
        .unwrap_or_default();
    let _ = writeln!(out, "    {} (distance {}){}", dep.path, dep.distance, pkg);
}
