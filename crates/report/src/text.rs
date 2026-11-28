use std::fmt::Write as _;

use codeint_model::{Metric, SourceSet};

use crate::{
    BoundaryFindingRow, BudgetFailure, CloneFamilyFinding, CloneGroupFinding, CloneLocation,
    CognitiveComplexityFinding, DependenciesSection, ErodedFunctionFinding, ExternalMetricFinding,
    FileDiagnosticEntry, Findings, FlaggedLineRow, HazardLineRow, ImportCycleFinding,
    ImportedDependencyRow, MeasuredSection, OrphanFileRow, PackageSection, RatchetFailure,
    ReferenceRangeEntry, Report, ScopeSummaryEntry, SkippedEntry, SourceSetSection,
    TestGapFindingRow, UnusedDependencyRow,
};

#[cfg(test)]
mod tests;

pub(crate) const TEXT_FINDINGS_LIMIT: usize = 5;

pub(crate) fn short_hash(commit: &str) -> &str {
    commit.get(..7).unwrap_or(commit)
}

pub(crate) fn plural<'a>(count: usize, one: &'a str, many: &'a str) -> &'a str {
    if count == 1 { one } else { many }
}

pub(crate) fn plural_u64<'a>(count: u64, one: &'a str, many: &'a str) -> &'a str {
    if count == 1 { one } else { many }
}

pub(crate) fn align_table(
    headers: &[&str],
    rows: &[Vec<String>],
    right_align: &[bool],
) -> Vec<String> {
    let mut widths: Vec<usize> = headers.iter().map(|header| header.len()).collect();
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            if let Some(width) = widths.get_mut(index) {
                *width = (*width).max(cell.len());
            }
        }
    }
    let mut lines = Vec::with_capacity(rows.len() + 1);
    lines.push(format_aligned_row(headers, &widths, right_align));
    for row in rows {
        let cells: Vec<&str> = row.iter().map(String::as_str).collect();
        lines.push(format_aligned_row(&cells, &widths, right_align));
    }
    lines
}

fn format_aligned_row(cells: &[&str], widths: &[usize], right_align: &[bool]) -> String {
    let mut line = String::from("  ");
    let last = cells.len().saturating_sub(1);
    for (index, cell) in cells.iter().enumerate() {
        if index > 0 {
            line.push_str("  ");
        }
        let width = widths.get(index).copied().unwrap_or(cell.len());
        let align_right = right_align.get(index).copied().unwrap_or(false);
        if align_right {
            let _ = write!(line, "{cell:>width$}");
        } else if index == last {
            line.push_str(cell);
        } else {
            let _ = write!(line, "{cell:<width$}");
        }
    }
    line.trim_end().to_owned()
}

pub(crate) fn push_bullet_block<T: std::fmt::Display>(
    blocks: &mut Vec<String>,
    heading: &str,
    items: &[T],
) {
    if items.is_empty() {
        return;
    }
    let mut lines = vec![format!("{heading}:")];
    lines.extend(items.iter().map(|item| format!("  - {item}")));
    blocks.push(lines.join("\n"));
}

pub(crate) fn push_notes_section(blocks: &mut Vec<String>, notes: Vec<String>) {
    if notes.is_empty() {
        return;
    }
    let mut out = Vec::with_capacity(notes.len() + 1);
    out.push("Notes".to_owned());
    for note in notes {
        out.push(format!("  {note}"));
    }
    blocks.push(out.join("\n"));
}

pub(crate) fn source_set_heading(source_set: SourceSet) -> &'static str {
    match source_set {
        SourceSet::Production => "Production",
        SourceSet::Test => "Test",
        SourceSet::Generated => "Generated",
        SourceSet::Vendored => "Vendored",
    }
}

pub(crate) fn format_eroded_function(finding: &ErodedFunctionFinding) -> String {
    let name = finding.name.as_deref().unwrap_or("<anonymous>");
    format!(
        "{}:{}  {}  cc={}  lines={}  mass={}",
        finding.path,
        finding.line,
        name,
        finding.cyclomatic_complexity,
        finding.source_lines,
        finding.function_mass.0
    )
}

pub(crate) fn format_complex_function(finding: &CognitiveComplexityFinding) -> String {
    let name = finding.name.as_deref().unwrap_or("<anonymous>");
    format!(
        "{}:{}  {}  cognitive={}  lines={}",
        finding.path, finding.line, name, finding.cognitive_complexity, finding.source_lines
    )
}

pub(crate) fn format_clone_group(finding: &CloneGroupFinding) -> String {
    let locations = format_clone_locations(&finding.locations);
    let sim = finding
        .similarity
        .map_or_else(String::new, |s| format!(", sim={:.0}%", s.0 * 100.0));
    format!("{locations}  ({} tokens{sim})", finding.tokens)
}

pub(crate) fn format_clone_family(finding: &CloneFamilyFinding) -> String {
    let locations = format_clone_locations(&finding.locations);
    format!(
        "{} locations ({} duplicated lines): {locations}",
        finding.locations.len(),
        finding.duplicated_lines
    )
}

fn format_clone_locations(locations: &[CloneLocation]) -> String {
    locations
        .iter()
        .map(|loc| format!("{}:{}-{}", loc.path, loc.start_line, loc.end_line))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_line_detail(path: &str, line: u32, detail: &str) -> String {
    format!("{path}:{line}  {detail}")
}

pub(crate) fn format_flagged_line(finding: &FlaggedLineRow) -> String {
    format_line_detail(&finding.path, finding.line, &finding.rule)
}

pub(crate) fn format_hazard_line(finding: &HazardLineRow) -> String {
    format_line_detail(&finding.path, finding.line, &finding.rule)
}

pub(crate) fn format_import_cycle(finding: &ImportCycleFinding) -> String {
    finding.modules.first().map_or_else(String::new, |first| {
        format!("{} -> {first}", finding.modules.join(" -> "))
    })
}

pub(crate) fn format_boundary_finding(finding: &BoundaryFindingRow) -> String {
    let target = finding
        .target
        .as_deref()
        .or(finding.external_package.as_deref())
        .or(finding.specifier.as_deref());
    match target {
        Some(target) => format!("{} -> {target}  ({})", finding.importer, finding.message),
        None => format!("{}  ({})", finding.importer, finding.message),
    }
}

pub(crate) fn format_orphan_file(finding: &OrphanFileRow) -> String {
    format!("{}  ({})", finding.path, finding.kind)
}

pub(crate) fn format_test_gap(finding: &TestGapFindingRow) -> String {
    let label = finding.name.as_deref().unwrap_or("<anonymous>");
    let pct = (finding.untested_share.0 * 100.0).round() as u32;
    format!(
        "{}:{}  {label}  untested={pct}%  mass={}",
        finding.path, finding.line, finding.function_mass.0
    )
}

pub(crate) fn format_external_finding(finding: &ExternalMetricFinding) -> String {
    format_line_detail(&finding.path, finding.line, &finding.message)
}

pub(crate) fn format_unused_dependency(finding: &UnusedDependencyRow) -> String {
    format!(
        "{}  {}  ({})",
        finding.manifest, finding.dependency, finding.kind
    )
}

pub(crate) fn format_imported_dependency(finding: &ImportedDependencyRow) -> String {
    if finding.importers.is_empty() {
        return format!("{}  {}", finding.manifest, finding.dependency);
    }
    let shown: Vec<&str> = finding
        .importers
        .iter()
        .take(3)
        .map(String::as_str)
        .collect();
    let suffix = if finding.importers.len() > 3 {
        format!(", +{} more", finding.importers.len() - 3)
    } else {
        String::new()
    };
    format!(
        "{}  {}  (imported by {}{suffix})",
        finding.manifest,
        finding.dependency,
        shown.join(", ")
    )
}

pub(crate) fn push_findings_lines<T>(
    blocks: &mut Vec<String>,
    heading: &str,
    findings: &Findings<T>,
    show_all: bool,
    format_item: fn(&T) -> String,
) {
    if findings.listed.is_empty() {
        return;
    }
    let total = findings.listed.len().saturating_add(findings.left_out);
    let limit = if show_all {
        findings.listed.len()
    } else {
        TEXT_FINDINGS_LIMIT.min(findings.listed.len())
    };
    let omitted = findings
        .listed
        .len()
        .saturating_sub(limit)
        .saturating_add(findings.left_out);
    let mut lines = Vec::with_capacity(limit + 2);
    lines.push(format!("{heading} ({total}):"));
    for item in findings.listed.iter().take(limit) {
        lines.push(format!("  {}", format_item(item)));
    }
    if omitted > 0 {
        if show_all {
            lines.push(format!("  ... {omitted} more"));
        } else {
            lines.push(format!(
                "  ... {omitted} more (pass --all to list every Finding)"
            ));
        }
    }
    blocks.push(lines.join("\n"));
}

pub(crate) fn render_check(report: &Report) -> String {
    let mut blocks = Vec::new();
    blocks.push(check_header_block(report));
    push_check_failure_blocks(&mut blocks, report);
    let show_reference = report.languages.is_empty();
    for (source_set, section) in report.measured.source_sets.iter() {
        if !should_show_source_set(section, report.show_all) {
            continue;
        }
        blocks.push(source_set_table_block(
            source_set,
            section,
            show_reference,
            &report.over_budget,
            &report.over_ratchet,
        ));
    }
    if let Some(dependencies) = &report.measured.dependencies {
        blocks.push(dependencies_table_block(
            dependencies,
            &report.over_budget,
            &report.over_ratchet,
        ));
    }
    if !report.languages.is_empty() {
        blocks.push(languages_summary_block(report));
    }
    if let Some(packages_block) = packages_summary_block(report) {
        blocks.push(packages_block);
    }
    push_check_findings_blocks(&mut blocks, report);
    push_notes_section(&mut blocks, collect_check_notes(report));
    blocks.join("\n\n")
}

fn check_header_block(report: &Report) -> String {
    let passed = report.over_budget.is_empty() && report.over_ratchet.is_empty();
    let status = if passed { "PASS" } else { "FAIL" };
    let mut details = Vec::new();
    if !report.over_budget.is_empty() {
        details.push(format!("{} over Budget", report.over_budget.len()));
    }
    if !report.over_ratchet.is_empty() {
        details.push(format!("{} over Ratchet", report.over_ratchet.len()));
    }
    if let Some(baseline) = &report.baseline {
        details.push(format!("{} hidden by Baseline", baseline.hidden_findings));
    }
    let files = report.measured.files;
    let lines = report.measured.source_lines;
    let summary = format!(
        "{files} {}, {lines} {}",
        plural(files, "file", "files"),
        plural_u64(lines, "source line", "source lines")
    );
    let first_line = if details.is_empty() {
        format!("codeint check: {status} ({summary})")
    } else {
        format!(
            "codeint check: {status} ({summary}; {})",
            details.join(", ")
        )
    };
    if report.measured.framework_profiles.is_empty() {
        return first_line;
    }
    format!(
        "{first_line}\nframework profiles: {}",
        report.measured.framework_profiles.join(", ")
    )
}

fn push_check_failure_blocks(blocks: &mut Vec<String>, report: &Report) {
    push_bullet_block(blocks, "Over Budget", &report.over_budget);
    push_bullet_block(blocks, "Over Ratchet", &report.over_ratchet);
}

fn should_show_source_set(section: &SourceSetSection, show_all: bool) -> bool {
    show_all || section.files > 0 || source_set_has_non_zero_metric(section)
}

fn source_set_has_non_zero_metric(section: &SourceSetSection) -> bool {
    section.erosion.header.ratio.0 > 0.0
        || section.cognitive_complexity.header.ratio.0 > 0.0
        || section.duplication.header.ratio.0 > 0.0
        || section.verbosity.header.ratio.0 > 0.0
        || section
            .import_cycles
            .as_ref()
            .is_some_and(|item| item.header.ratio.0 > 0.0)
        || section
            .boundaries
            .as_ref()
            .is_some_and(|item| item.header.ratio.0 > 0.0)
        || section
            .orphan_files
            .as_ref()
            .is_some_and(|item| item.header.ratio.0 > 0.0)
        || section
            .test_gap
            .as_ref()
            .is_some_and(|item| item.header.ratio.0 > 0.0)
        || section
            .hazards
            .as_ref()
            .is_some_and(|item| item.header.ratio.0 > 0.0)
        || section
            .external_metrics
            .values()
            .any(|item| item.header.ratio.0 > 0.0)
}

fn reference_cell(entry: &ReferenceRangeEntry) -> String {
    match entry {
        ReferenceRangeEntry::Published { low, high, .. } => format!("{}-{}", low.0, high.0),
        ReferenceRangeEntry::NotShown(_) => "-".to_owned(),
    }
}

fn metric_status(
    source_set: Option<&str>,
    metric_key: &str,
    baselined_hidden: usize,
    over_budget: &[BudgetFailure],
    over_ratchet: &[RatchetFailure],
) -> String {
    let mut reasons = Vec::new();
    if let Some(failure) = over_budget
        .iter()
        .find(|item| item.source_set.as_deref() == source_set && item.metric == metric_key)
    {
        reasons.push(format!("budget {}", failure.budget));
    }
    if let Some(failure) = over_ratchet
        .iter()
        .find(|item| item.source_set.as_deref() == source_set && item.metric == metric_key)
    {
        reasons.push(format!("ratchet {}", failure.ceiling.0));
    }
    if !reasons.is_empty() {
        return format!("FAIL ({})", reasons.join(", "));
    }
    if baselined_hidden > 0 {
        return format!("ok ({baselined_hidden} baselined)");
    }
    "ok".to_owned()
}

struct MetricRowSpec<'a> {
    name: &'a str,
    key: &'a str,
    ratio: f64,
    reference: Option<&'a ReferenceRangeEntry>,
    baselined_hidden: usize,
}

fn build_source_set_row(
    source_set_key: &str,
    spec: MetricRowSpec<'_>,
    show_reference: bool,
    over_budget: &[BudgetFailure],
    over_ratchet: &[RatchetFailure],
) -> Vec<String> {
    let status = metric_status(
        Some(source_set_key),
        spec.key,
        spec.baselined_hidden,
        over_budget,
        over_ratchet,
    );
    if show_reference {
        let reference = spec
            .reference
            .map_or_else(|| "-".to_owned(), reference_cell);
        vec![
            spec.name.to_owned(),
            spec.ratio.to_string(),
            reference,
            status,
        ]
    } else {
        vec![spec.name.to_owned(), spec.ratio.to_string(), status]
    }
}

fn source_set_table_block(
    source_set: SourceSet,
    section: &SourceSetSection,
    show_reference: bool,
    over_budget: &[BudgetFailure],
    over_ratchet: &[RatchetFailure],
) -> String {
    let key = source_set.key();
    let specs = collect_source_set_metric_specs(section);
    let rows: Vec<Vec<String>> = specs
        .into_iter()
        .map(|spec| build_source_set_row(key, spec, show_reference, over_budget, over_ratchet))
        .collect();
    let heading = format!(
        "{} ({} {}, {} {})",
        source_set_heading(source_set),
        section.files,
        plural(section.files, "file", "files"),
        section.source_lines,
        plural_u64(section.source_lines, "line", "lines")
    );
    let table_lines = if show_reference {
        align_table(
            &["Metric", "Ratio", "Reference", "Status"],
            &rows,
            &[false, true, false, false],
        )
    } else {
        align_table(&["Metric", "Ratio", "Status"], &rows, &[false, true, false])
    };
    let mut lines = Vec::with_capacity(table_lines.len() + 1);
    lines.push(heading);
    lines.extend(table_lines);
    lines.join("\n")
}

fn collect_source_set_metric_specs<'a>(section: &'a SourceSetSection) -> Vec<MetricRowSpec<'a>> {
    let mut specs = vec![
        MetricRowSpec {
            name: "Erosion",
            key: Metric::Erosion.key(),
            ratio: section.erosion.header.ratio.0,
            reference: Some(&section.erosion.header.reference_range),
            baselined_hidden: section.erosion.findings.baselined.hidden,
        },
        MetricRowSpec {
            name: "Cognitive complexity",
            key: Metric::CognitiveComplexity.key(),
            ratio: section.cognitive_complexity.header.ratio.0,
            reference: Some(&section.cognitive_complexity.header.reference_range),
            baselined_hidden: section.cognitive_complexity.findings.baselined.hidden,
        },
        MetricRowSpec {
            name: "Duplication",
            key: Metric::Duplication.key(),
            ratio: section.duplication.header.ratio.0,
            reference: Some(&section.duplication.header.reference_range),
            baselined_hidden: section.duplication.clone_groups.baselined.hidden,
        },
        MetricRowSpec {
            name: "Verbosity",
            key: Metric::Verbosity.key(),
            ratio: section.verbosity.header.ratio.0,
            reference: Some(&section.verbosity.header.reference_range),
            baselined_hidden: section.verbosity.findings.baselined.hidden,
        },
    ];
    push_optional_metric_specs(section, &mut specs);
    specs
}

fn push_spec<'a>(
    specs: &mut Vec<MetricRowSpec<'a>>,
    name: &'a str,
    key: &'a str,
    ratio: f64,
    reference: Option<&'a ReferenceRangeEntry>,
    baselined_hidden: usize,
) {
    specs.push(MetricRowSpec {
        name,
        key,
        ratio,
        reference,
        baselined_hidden,
    });
}

fn push_optional_metric_specs<'a>(
    section: &'a SourceSetSection,
    specs: &mut Vec<MetricRowSpec<'a>>,
) {
    if let Some(item) = &section.import_cycles {
        push_spec(
            specs,
            "Import cycles",
            Metric::ImportCycles.key(),
            item.header.ratio.0,
            Some(&item.header.reference_range),
            item.findings.baselined.hidden,
        );
    }
    if let Some(item) = &section.boundaries {
        push_spec(
            specs,
            "Boundaries",
            Metric::Boundaries.key(),
            item.header.ratio.0,
            None,
            item.findings.baselined.hidden,
        );
    }
    if let Some(item) = &section.orphan_files {
        push_spec(
            specs,
            "Orphan files",
            Metric::OrphanFiles.key(),
            item.header.ratio.0,
            None,
            item.findings.baselined.hidden,
        );
    }
    if let Some(item) = &section.test_gap {
        push_spec(
            specs,
            "Test gap",
            Metric::TestGap.key(),
            item.header.ratio.0,
            None,
            item.findings.baselined.hidden,
        );
    }
    if let Some(item) = &section.hazards {
        push_spec(
            specs,
            "Hazards",
            Metric::Hazards.key(),
            item.header.ratio.0,
            None,
            item.findings.baselined.hidden,
        );
    }
    for (key, item) in &section.external_metrics {
        push_spec(
            specs,
            &item.name,
            key.as_str(),
            item.header.ratio.0,
            Some(&item.header.reference_range),
            item.findings.baselined.hidden,
        );
    }
}

fn dependencies_table_block(
    dependencies: &DependenciesSection,
    over_budget: &[BudgetFailure],
    over_ratchet: &[RatchetFailure],
) -> String {
    let rows = vec![
        vec![
            "Unused dependencies".to_owned(),
            dependencies.unused_dependencies.header.ratio.0.to_string(),
            format!(
                "{}/{}",
                dependencies.unused_dependencies.unused, dependencies.unused_dependencies.declared
            ),
            metric_status(
                None,
                Metric::UnusedDependencies.key(),
                dependencies.unused_dependencies.findings.baselined.hidden,
                over_budget,
                over_ratchet,
            ),
        ],
        vec![
            "Unlisted dependencies".to_owned(),
            dependencies
                .unlisted_dependencies
                .header
                .ratio
                .0
                .to_string(),
            format!(
                "{}/{}",
                dependencies.unlisted_dependencies.unlisted,
                dependencies.unlisted_dependencies.imported
            ),
            metric_status(
                None,
                Metric::UnlistedDependencies.key(),
                dependencies.unlisted_dependencies.findings.baselined.hidden,
                over_budget,
                over_ratchet,
            ),
        ],
        vec![
            "Dev dependencies in production".to_owned(),
            dependencies
                .dev_in_production_dependencies
                .header
                .ratio
                .0
                .to_string(),
            format!(
                "{}/{}",
                dependencies
                    .dev_in_production_dependencies
                    .dev_in_production,
                dependencies
                    .dev_in_production_dependencies
                    .production_imported
            ),
            metric_status(
                None,
                Metric::DevInProductionDependencies.key(),
                dependencies
                    .dev_in_production_dependencies
                    .findings
                    .baselined
                    .hidden,
                over_budget,
                over_ratchet,
            ),
        ],
    ];
    let mut lines = vec!["Dependencies".to_owned()];
    lines.extend(align_table(
        &["Metric", "Ratio", "Count", "Status"],
        &rows,
        &[false, true, true, false],
    ));
    lines.join("\n")
}

fn languages_summary_block(report: &Report) -> String {
    let mut lines = vec!["Languages".to_owned()];
    for (plugin, section) in &report.languages {
        lines.push(format!("  {plugin}: {}", compact_language_line(section)));
    }
    lines.join("\n")
}

fn compact_language_line(section: &MeasuredSection) -> String {
    let base = format!(
        "{} {}, {} {}",
        section.files,
        plural(section.files, "file", "files"),
        section.source_lines,
        plural_u64(section.source_lines, "line", "lines")
    );
    let Some(prod) = section.source_sets.production() else {
        return base;
    };
    let mut metrics = vec![
        format!("erosion {}", prod.erosion.header.ratio.0),
        format!("cognitive {}", prod.cognitive_complexity.header.ratio.0),
        format!("duplication {}", prod.duplication.header.ratio.0),
        format!("verbosity {}", prod.verbosity.header.ratio.0),
    ];
    if let Some(cycles) = &prod.import_cycles {
        metrics.push(format!("cycles {}", cycles.header.ratio.0));
    }
    if let Some(hazards) = &prod.hazards {
        metrics.push(format!("hazards {}", hazards.header.ratio.0));
    }
    format!("{base} (production: {})", metrics.join(", "))
}

fn packages_summary_block(report: &Report) -> Option<String> {
    let lines: Vec<String> = report
        .measured
        .source_sets
        .iter()
        .flat_map(|(source_set, section)| {
            section
                .packages
                .iter()
                .filter_map(move |pkg| format_package_summary_line(source_set, pkg, report))
        })
        .collect();
    if lines.is_empty() {
        return None;
    }
    let mut out = vec!["Workspace packages".to_owned()];
    out.extend(lines);
    Some(out.join("\n"))
}

fn format_package_summary_line(
    source_set: SourceSet,
    pkg: &PackageSection,
    report: &Report,
) -> Option<String> {
    let failing_optional = failing_optional_package_metrics(
        source_set,
        pkg,
        &report.over_budget,
        &report.over_ratchet,
    );
    let failing = !failing_optional.is_empty()
        || package_fails_core_limit(source_set, pkg, &report.over_budget, &report.over_ratchet);
    if !report.show_all && !failing {
        return None;
    }
    let badge = if failing { "FAIL" } else { "ok" };
    let mut line = format!(
        "  {} ({}) [{}]: erosion={} cognitive={} duplication={} verbosity={}",
        pkg.path,
        source_set.key(),
        badge,
        pkg.erosion.ratio.0,
        pkg.cognitive_complexity.ratio.0,
        pkg.duplication.ratio.0,
        pkg.verbosity.ratio.0
    );
    for (metric_key, ratio) in failing_optional {
        let _ = write!(line, " {metric_key}={ratio}");
    }
    Some(line)
}

fn package_metric_fails_limit(
    source_set: SourceSet,
    metric_key: &str,
    ratio: f64,
    over_budget: &[BudgetFailure],
    over_ratchet: &[RatchetFailure],
) -> bool {
    let key = source_set.key();
    over_budget.iter().any(|failure| {
        failure.source_set.as_deref() == Some(key)
            && failure.metric == metric_key
            && ratio > failure.budget
    }) || over_ratchet.iter().any(|failure| {
        failure.source_set.as_deref() == Some(key)
            && failure.metric == metric_key
            && ratio > failure.ceiling.0
    })
}

fn package_fails_core_limit(
    source_set: SourceSet,
    pkg: &PackageSection,
    over_budget: &[BudgetFailure],
    over_ratchet: &[RatchetFailure],
) -> bool {
    [
        (Metric::Erosion.key(), pkg.erosion.ratio.0),
        (
            Metric::CognitiveComplexity.key(),
            pkg.cognitive_complexity.ratio.0,
        ),
        (Metric::Duplication.key(), pkg.duplication.ratio.0),
        (Metric::Verbosity.key(), pkg.verbosity.ratio.0),
    ]
    .into_iter()
    .any(|(metric_key, ratio)| {
        package_metric_fails_limit(source_set, metric_key, ratio, over_budget, over_ratchet)
    })
}

fn failing_optional_package_metrics(
    source_set: SourceSet,
    pkg: &PackageSection,
    over_budget: &[BudgetFailure],
    over_ratchet: &[RatchetFailure],
) -> Vec<(&'static str, f64)> {
    [
        (
            Metric::ImportCycles.key(),
            pkg.import_cycles.as_ref().map(|item| item.ratio.0),
        ),
        (
            Metric::TestGap.key(),
            pkg.test_gap.as_ref().map(|item| item.ratio.0),
        ),
        (
            Metric::Hazards.key(),
            pkg.hazards.as_ref().map(|item| item.ratio.0),
        ),
    ]
    .into_iter()
    .filter_map(|(metric_key, ratio)| {
        let ratio = ratio?;
        package_metric_fails_limit(source_set, metric_key, ratio, over_budget, over_ratchet)
            .then_some((metric_key, ratio))
    })
    .collect()
}

fn push_check_findings_blocks(blocks: &mut Vec<String>, report: &Report) {
    for (source_set, section) in report.measured.source_sets.iter() {
        let name = source_set_heading(source_set);
        push_findings_lines(
            blocks,
            &format!("{name} Erosion findings"),
            &section.erosion.findings,
            report.show_all,
            format_eroded_function,
        );
        push_findings_lines(
            blocks,
            &format!("{name} Cognitive complexity findings"),
            &section.cognitive_complexity.findings,
            report.show_all,
            format_complex_function,
        );
        push_findings_lines(
            blocks,
            &format!("{name} Clone families"),
            &section.duplication.clone_families,
            report.show_all,
            format_clone_family,
        );
        push_findings_lines(
            blocks,
            &format!("{name} Duplication findings"),
            &section.duplication.clone_groups,
            report.show_all,
            format_clone_group,
        );
        push_findings_lines(
            blocks,
            &format!("{name} Verbosity findings"),
            &section.verbosity.findings,
            report.show_all,
            format_flagged_line,
        );
        push_optional_source_set_findings(blocks, name, section, report.show_all);
    }
    if let Some(dependencies) = &report.measured.dependencies {
        push_findings_lines(
            blocks,
            "Unused dependencies findings",
            &dependencies.unused_dependencies.findings,
            report.show_all,
            format_unused_dependency,
        );
        push_findings_lines(
            blocks,
            "Unlisted dependencies findings",
            &dependencies.unlisted_dependencies.findings,
            report.show_all,
            format_imported_dependency,
        );
        push_findings_lines(
            blocks,
            "Dev dependencies in production findings",
            &dependencies.dev_in_production_dependencies.findings,
            report.show_all,
            format_imported_dependency,
        );
    }
}

fn push_optional_source_set_findings(
    blocks: &mut Vec<String>,
    name: &str,
    section: &SourceSetSection,
    show_all: bool,
) {
    if let Some(cycles) = &section.import_cycles {
        push_findings_lines(
            blocks,
            &format!("{name} Import cycles findings"),
            &cycles.findings,
            show_all,
            format_import_cycle,
        );
    }
    if let Some(boundaries) = &section.boundaries {
        push_findings_lines(
            blocks,
            &format!("{name} Boundaries findings"),
            &boundaries.findings,
            show_all,
            format_boundary_finding,
        );
    }
    if let Some(orphans) = &section.orphan_files {
        push_findings_lines(
            blocks,
            &format!("{name} Orphan files findings"),
            &orphans.findings,
            show_all,
            format_orphan_file,
        );
    }
    if let Some(test_gap) = &section.test_gap {
        push_findings_lines(
            blocks,
            &format!("{name} Test gap findings"),
            &test_gap.findings,
            show_all,
            format_test_gap,
        );
    }
    if let Some(hazards) = &section.hazards {
        push_findings_lines(
            blocks,
            &format!("{name} Hazards findings"),
            &hazards.findings,
            show_all,
            format_hazard_line,
        );
    }
    for external in section.external_metrics.values() {
        push_findings_lines(
            blocks,
            &format!("{name} {} findings", external.name),
            &external.findings,
            show_all,
            format_external_finding,
        );
    }
}

fn collect_check_notes(report: &Report) -> Vec<String> {
    let mut lines: Vec<String> = [
        report.measured.reference_range_note.as_deref(),
        report.measured.import_metrics_note.as_deref(),
        report.measured.test_gap_note.as_deref(),
        report.coverage_unmatched_note.as_deref(),
        report.ratchet_note.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(str::to_owned)
    .collect();
    for warning in &report.manifest_warnings {
        lines.push(format!("warning: {warning}"));
    }
    push_capped_note(
        &mut lines,
        "skipped",
        &report.skipped,
        report.show_all,
        |e: &SkippedEntry| format!("{} ({})", e.path, e.reason),
    );
    push_capped_note(
        &mut lines,
        "file diagnostics",
        &report.file_diagnostics,
        report.show_all,
        |e: &FileDiagnosticEntry| format!("{}:{} [{}] {}", e.path, e.line, e.kind, e.message),
    );
    push_scope_summary_notes(&mut lines, &report.scope_summaries);
    if !report.declaration_only.is_empty() {
        lines.push(format!(
            "declaration-only: {} {}",
            report.declaration_only.len(),
            plural(report.declaration_only.len(), "file", "files")
        ));
    }
    if !report.submodules.is_empty() {
        lines.push(format!("submodules: {}", report.submodules.join(", ")));
    }
    lines
}

fn push_capped_note<T>(
    lines: &mut Vec<String>,
    label: &str,
    items: &[T],
    show_all: bool,
    format_item: fn(&T) -> String,
) {
    if items.is_empty() {
        return;
    }
    let limit = if show_all {
        items.len()
    } else {
        TEXT_FINDINGS_LIMIT.min(items.len())
    };
    let listed = items
        .iter()
        .take(limit)
        .map(format_item)
        .collect::<Vec<_>>()
        .join(", ");
    let extra = items.len().saturating_sub(limit);
    if extra > 0 {
        lines.push(format!(
            "{label} ({}): {listed}, +{extra} more",
            items.len()
        ));
    } else {
        lines.push(format!("{label} ({}): {listed}", items.len()));
    }
}

fn push_scope_summary_notes(lines: &mut Vec<String>, summaries: &[ScopeSummaryEntry]) {
    if summaries.is_empty() {
        return;
    }
    let listed = summaries
        .iter()
        .map(|summary| {
            let scope = summary.package.as_deref().unwrap_or("root");
            let dialect = summary
                .dialect
                .as_deref()
                .map(|name| format!(" {name}"))
                .unwrap_or_default();
            let details = summary
                .fields
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join(", ");
            if details.is_empty() {
                format!("{} {scope}{dialect} {}", summary.language, summary.kind)
            } else {
                format!(
                    "{} {scope}{dialect} {} ({details})",
                    summary.language, summary.kind
                )
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    lines.push(format!("scope summaries: {listed}"));
}
