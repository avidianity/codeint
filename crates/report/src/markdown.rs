use crate::ownership::OwnedFinding;
use crate::{
    BoundaryFindingRow, CloneGroupFinding, CloneLocation, DuplicationSection,
    ErodedFunctionFinding, ErosionSection, ExternalMetricFinding, Findings, FlaggedLineRow,
    ImportCycleFinding, ImportCyclesSection, MeasuredSection, PackageSection, ReferenceRangeEntry,
    Report, SourceSetSection, UnresolvedImportRow, VerbositySection,
};

#[cfg(test)]
mod tests;

const TITLE: &str = "# codeint report";
const METRIC_COLUMNS: [&str; 5] = [
    "Source set",
    "Metric",
    "Value",
    "Behind the value",
    "Reference range",
];
pub(crate) const ERODED_FUNCTION_COLUMNS: [&str; 4] = [
    "Function",
    "Cyclomatic complexity",
    "Source lines",
    "Function mass",
];
const PACKAGE_COLUMNS: [&str; 8] = [
    "Source set",
    "Package",
    "Files",
    "Source lines",
    "Erosion",
    "Duplication",
    "Verbosity",
    "Import cycles",
];
pub(crate) const FLAGGED_LINE_COLUMNS: [&str; 2] = ["Flagged line", "Rule"];
pub(crate) const EXTERNAL_FINDING_COLUMNS: [&str; 2] = ["Location", "Message"];
pub(crate) const BOUNDARY_FINDING_COLUMNS: [&str; 6] = [
    "Importer",
    "Target",
    "Source zone",
    "Target zone",
    "Rule",
    "Reason",
];
const UNRESOLVED_IMPORT_COLUMNS: [&str; 3] = ["Importer", "Specifier", "Reason"];

pub(crate) struct Noun {
    one: &'static str,
    many: &'static str,
}

const FILE: Noun = Noun {
    one: "file",
    many: "files",
};
const SOURCE_LINE: Noun = Noun {
    one: "source line",
    many: "source lines",
};
const CLONE_LINE: Noun = Noun {
    one: "clone line",
    many: "clone lines",
};
const VERBOSE_LINE: Noun = Noun {
    one: "verbose line",
    many: "verbose lines",
};
pub(crate) const FLAGGED_LINE: Noun = Noun {
    one: "Flagged line",
    many: "Flagged lines",
};
const MODULE: Noun = Noun {
    one: "module",
    many: "modules",
};
pub(crate) const ERODED_FUNCTION: Noun = Noun {
    one: "eroded function",
    many: "eroded functions",
};
pub(crate) const CLONE_GROUP: Noun = Noun {
    one: "Clone group",
    many: "Clone groups",
};
pub(crate) const IMPORT_CYCLE: Noun = Noun {
    one: "Import cycle",
    many: "Import cycles",
};
pub(crate) const FINDING: Noun = Noun {
    one: "Finding",
    many: "Findings",
};
const UNRESOLVED_IMPORT: Noun = Noun {
    one: "unresolved import",
    many: "unresolved imports",
};

impl Noun {
    fn counted(&self, count: impl Into<u64>) -> String {
        let count = count.into();
        format!("{count} {}", self.for_count(count))
    }

    fn for_count(&self, count: u64) -> &'static str {
        if count == 1 { self.one } else { self.many }
    }
}

pub(crate) fn render(report: &Report) -> String {
    let sections = report.measured.source_sets.named();
    let mut blocks = vec![TITLE.to_string(), measured_summary(&report.measured)];
    blocks.extend(metric_table(&sections));
    if !report.over_budget.is_empty() {
        blocks.push("## Over Budget".to_string());
        blocks.push(list(report.over_budget.iter().map(sentence_item)));
    }
    if let Some(packages) = package_table(&sections) {
        blocks.push("## Workspace packages".to_string());
        blocks.push(packages);
    }
    for (plugin, language) in &report.languages {
        let language_sections = language.source_sets.named();
        blocks.push(format!("## Language plugin {}", code(plugin)));
        blocks.push(measured_summary(language));
        blocks.extend(metric_table(&language_sections));
        blocks.extend(package_table(&language_sections));
    }
    for (name, section) in &sections {
        let findings = findings_blocks(section);
        if !findings.is_empty() {
            blocks.push(format!("## {name} Findings"));
            blocks.extend(findings);
        }
    }
    let not_measured = files_not_measured(report);
    if !not_measured.is_empty() {
        blocks.push("## Files not measured".to_string());
        blocks.push(not_measured.join("\n"));
    }
    blocks.join("\n\n")
}

fn metric_table(sections: &[(&str, &SourceSetSection)]) -> Option<String> {
    if sections.is_empty() {
        return None;
    }
    Some(table(
        &METRIC_COLUMNS,
        sections
            .iter()
            .flat_map(|(name, section)| metric_rows(name, section))
            .collect(),
    ))
}

fn package_table(sections: &[(&str, &SourceSetSection)]) -> Option<String> {
    let package_rows: Vec<Vec<String>> = sections
        .iter()
        .flat_map(|(name, section)| {
            section
                .packages
                .iter()
                .map(|package| package_row(name, package))
        })
        .collect();
    if package_rows.is_empty() {
        return None;
    }
    Some(table(&PACKAGE_COLUMNS, package_rows))
}

pub(crate) fn sentence_item(failure: &impl std::fmt::Display) -> String {
    let sentence = failure.to_string();
    let mut characters = sentence.chars();
    let capitalized: String = characters
        .next()
        .map(|first| first.to_uppercase().chain(characters).collect())
        .unwrap_or_default();
    format!("{}.", prose(&capitalized))
}

fn package_row(source_set: &str, package: &PackageSection) -> Vec<String> {
    vec![
        source_set.to_string(),
        code(&package.path),
        package.files.to_string(),
        package.source_lines.to_string(),
        package.erosion.ratio.0.to_string(),
        package.duplication.ratio.0.to_string(),
        package.verbosity.ratio.0.to_string(),
        package.import_cycles.ratio.0.to_string(),
    ]
}

fn measured_summary(measured: &MeasuredSection) -> String {
    let sections = measured.source_sets.named();
    let total = files_and_lines(measured.files, measured.source_lines);
    if sections.len() < 2 {
        return format!("codeint measured {total}.");
    }
    let per_source_set: Vec<String> = sections
        .iter()
        .map(|(name, section)| {
            format!(
                "{name} has {}",
                files_and_lines(section.files, section.source_lines)
            )
        })
        .collect();
    format!("codeint measured {total}: {}.", per_source_set.join(", "))
}

fn files_and_lines(files: usize, source_lines: u64) -> String {
    format!(
        "{} with {}",
        FILE.counted(files as u64),
        SOURCE_LINE.counted(source_lines)
    )
}

fn metric_rows(source_set: &str, section: &SourceSetSection) -> Vec<Vec<String>> {
    let row = |metric: &str, value: f64, behind: String, range: &ReferenceRangeEntry| {
        vec![
            source_set.to_string(),
            metric.to_string(),
            value.to_string(),
            behind,
            reference_range(range),
        ]
    };
    let ErosionSection {
        ratio,
        reference_range: erosion_range,
        eroded_function_mass,
        total_function_mass,
        ..
    } = &section.erosion;
    let DuplicationSection {
        ratio: duplication_ratio,
        reference_range: duplication_range,
        clone_lines,
        ..
    } = &section.duplication;
    let VerbositySection {
        ratio: verbosity_ratio,
        reference_range: verbosity_range,
        verbose_lines,
        flagged_lines,
        clone_lines: verbosity_clone_lines,
        ..
    } = &section.verbosity;
    let import_cycles = &section.import_cycles;
    let mut rows = vec![
        row(
            "Erosion",
            ratio.0,
            format!(
                "{} of {} Function mass in eroded functions",
                eroded_function_mass.0, total_function_mass.0
            ),
            erosion_range,
        ),
        row(
            "Duplication",
            duplication_ratio.0,
            format!(
                "{} of {}",
                CLONE_LINE.counted(*clone_lines),
                SOURCE_LINE.counted(section.source_lines)
            ),
            duplication_range,
        ),
        row(
            "Verbosity",
            verbosity_ratio.0,
            format!(
                "{} of {}: {}, {}",
                VERBOSE_LINE.counted(*verbose_lines),
                SOURCE_LINE.counted(section.source_lines),
                FLAGGED_LINE.counted(*flagged_lines),
                CLONE_LINE.counted(*verbosity_clone_lines)
            ),
            verbosity_range,
        ),
        row(
            "Import cycles",
            import_cycles.ratio.0,
            import_cycles_behind_the_value(import_cycles),
            &import_cycles.reference_range,
        ),
    ];
    if let Some(boundaries) = &section.boundaries {
        let total_findings = boundaries
            .findings
            .listed
            .len()
            .saturating_add(boundaries.findings.left_out);
        rows.push(vec![
            source_set.to_string(),
            "Boundaries".to_string(),
            boundaries.ratio.0.to_string(),
            format!(
                "{} across {} resolved imports",
                FINDING.counted(total_findings as u64),
                boundaries.resolved_imports
            ),
            "not applicable".to_string(),
        ]);
    }
    for external in section.external_metrics.values() {
        rows.push(row(
            &external.name,
            external.ratio.0,
            format!("{} of {}", external.numerator.0, external.denominator.0),
            &external.reference_range,
        ));
    }
    rows
}

fn import_cycles_behind_the_value(section: &ImportCyclesSection) -> String {
    let modules = section.modules as u64;
    let in_cycles = format!(
        "{} of {modules} {} in an Import cycle",
        section.modules_in_import_cycles,
        MODULE.for_count(modules)
    );
    match section.unresolved_imports.count {
        0 => in_cycles,
        unresolved => format!(
            "{in_cycles}, {}",
            UNRESOLVED_IMPORT.counted(unresolved as u64)
        ),
    }
}

fn reference_range(range: &ReferenceRangeEntry) -> String {
    match range {
        ReferenceRangeEntry::Published { low, high, source } => {
            format!("{} to {}, source: {}", low.0, high.0, prose(source))
        }
        ReferenceRangeEntry::NotShown(reason) => prose(reason),
    }
}

fn findings_blocks(section: &SourceSetSection) -> Vec<String> {
    let mut blocks = Vec::new();
    push_findings(
        &mut blocks,
        "Erosion",
        &section.erosion.findings,
        &ERODED_FUNCTION,
        |listed| findings_table(&ERODED_FUNCTION_COLUMNS, listed, eroded_function_row),
    );
    push_findings(
        &mut blocks,
        "Duplication",
        &section.duplication.clone_groups,
        &CLONE_GROUP,
        |listed| list(listed.iter().map(clone_group_item)),
    );
    push_findings(
        &mut blocks,
        "Verbosity",
        &section.verbosity.findings,
        &FLAGGED_LINE,
        |listed| findings_table(&FLAGGED_LINE_COLUMNS, listed, flagged_line_row),
    );
    push_findings(
        &mut blocks,
        "Import cycles",
        &section.import_cycles.findings,
        &IMPORT_CYCLE,
        |listed| list(listed.iter().map(import_cycle_item)),
    );
    push_findings(
        &mut blocks,
        "Unresolved imports",
        &section.import_cycles.unresolved_imports.imports,
        &UNRESOLVED_IMPORT,
        |listed| findings_table(&UNRESOLVED_IMPORT_COLUMNS, listed, unresolved_import_row),
    );
    if let Some(boundaries) = &section.boundaries {
        push_findings(
            &mut blocks,
            "Boundaries",
            &boundaries.findings,
            &FINDING,
            |listed| findings_table(&BOUNDARY_FINDING_COLUMNS, listed, boundary_finding_row),
        );
    }
    for external in section.external_metrics.values() {
        push_findings(
            &mut blocks,
            &external.name,
            &external.findings,
            &FINDING,
            |listed| findings_table(&EXTERNAL_FINDING_COLUMNS, listed, external_finding_row),
        );
    }
    blocks
}

pub(crate) fn push_findings<T>(
    blocks: &mut Vec<String>,
    heading: &str,
    findings: &Findings<T>,
    noun: &Noun,
    listed_block: impl Fn(&[T]) -> String,
) {
    if findings.listed.is_empty() && findings.left_out == 0 {
        return;
    }
    blocks.push(format!("### {heading}"));
    if !findings.listed.is_empty() {
        blocks.push(listed_block(&findings.listed));
    }
    if findings.left_out > 0 {
        let left_out = findings.left_out as u64;
        blocks.push(format!(
            "{left_out} more {} not listed. Pass `--all` to list every one.",
            noun.for_count(left_out)
        ));
    }
}

pub(crate) fn findings_table<T: OwnedFinding>(
    columns: &[&str],
    listed: &[T],
    row: impl Fn(&T) -> Vec<String>,
) -> String {
    let show_owners = listed.iter().any(|item| !item.owners().is_empty());
    let mut headers: Vec<&str> = columns.to_vec();
    if show_owners {
        headers.push("Owners");
    }
    let rows = listed
        .iter()
        .map(|item| {
            let mut cells = row(item);
            if show_owners {
                cells.push(format_owners(item.owners()));
            }
            cells
        })
        .collect();
    table(&headers, rows)
}

fn format_owners(owners: &[String]) -> String {
    if owners.is_empty() {
        "-".to_owned()
    } else {
        owners
            .iter()
            .map(String::as_str)
            .map(code)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn with_owners_suffix(text: String, owners: &[String]) -> String {
    if owners.is_empty() {
        text
    } else {
        format!("{text} (owners: {})", format_owners(owners))
    }
}

pub(crate) fn eroded_function_row(function: &ErodedFunctionFinding) -> Vec<String> {
    vec![
        code(&format!("{}:{}", function.path, function.line)),
        function.cyclomatic_complexity.to_string(),
        function.source_lines.to_string(),
        function.function_mass.0.to_string(),
    ]
}

pub(crate) fn clone_group_item(group: &CloneGroupFinding) -> String {
    let locations: Vec<String> = group.locations.iter().map(clone_location).collect();
    with_owners_suffix(
        format!("{} tokens: {}", group.tokens, locations.join(", ")),
        &group.owners,
    )
}

fn clone_location(location: &CloneLocation) -> String {
    if location.start_line == location.end_line {
        code(&format!("{}:{}", location.path, location.start_line))
    } else {
        code(&format!(
            "{}:{}-{}",
            location.path, location.start_line, location.end_line
        ))
    }
}

pub(crate) fn flagged_line_row(row: &FlaggedLineRow) -> Vec<String> {
    vec![
        code(&format!("{}:{}", row.path, row.line)),
        prose(&row.rule),
    ]
}

pub(crate) fn external_finding_row(finding: &ExternalMetricFinding) -> Vec<String> {
    let location = match finding.end_line {
        Some(end_line) if end_line != finding.line => {
            format!("{}:{}-{}", finding.path, finding.line, end_line)
        }
        _ => format!("{}:{}", finding.path, finding.line),
    };
    vec![code(&location), prose(&finding.message)]
}

pub(crate) fn import_cycle_item(cycle: &ImportCycleFinding) -> String {
    let modules: Vec<String> = cycle.modules.iter().map(|module| code(module)).collect();
    with_owners_suffix(modules.join(", "), &cycle.owners)
}

fn unresolved_import_row(row: &UnresolvedImportRow) -> Vec<String> {
    vec![
        code(&row.importer),
        code(&row.specifier),
        prose(&row.reason),
    ]
}

pub(crate) fn boundary_finding_row(row: &BoundaryFindingRow) -> Vec<String> {
    vec![
        code(&row.importer),
        row.target.as_deref().map_or_else(|| "-".to_owned(), code),
        row.source_zone
            .as_deref()
            .map_or_else(|| "-".to_owned(), prose),
        row.target_zone
            .as_deref()
            .map_or_else(|| "-".to_owned(), prose),
        prose(&row.kind),
        prose(row.reason.as_deref().unwrap_or(&row.message)),
    ]
}

fn files_not_measured(report: &Report) -> Vec<String> {
    let skipped = report
        .skipped
        .iter()
        .map(|entry| (entry.path.as_str(), prose(&entry.reason)));
    let declaration_only = report
        .declaration_only
        .iter()
        .map(|path| (path.as_str(), "declaration only".to_string()));
    let submodules = report
        .submodules
        .iter()
        .map(|path| (path.as_str(), "Git submodule".to_string()));
    skipped
        .chain(declaration_only)
        .chain(submodules)
        .map(|(path, why)| format!("- {}: {why}", code(path)))
        .collect()
}

pub(crate) fn list(items: impl Iterator<Item = String>) -> String {
    items
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn table(columns: &[&str], rows: Vec<Vec<String>>) -> String {
    let header = table_row(columns.iter().map(|column| column.to_string()));
    let delimiter = table_row(columns.iter().map(|column| "-".repeat(column.len())));
    let body = rows.into_iter().map(table_row);
    [header, delimiter]
        .into_iter()
        .chain(body)
        .collect::<Vec<_>>()
        .join("\n")
}

fn table_row(cells: impl IntoIterator<Item = String>) -> String {
    let cells: Vec<String> = cells
        .into_iter()
        .map(|cell| cell.replace('|', "\\|"))
        .collect();
    format!("| {} |", cells.join(" | "))
}

pub(crate) fn code(text: &str) -> String {
    let text = on_one_line(text);
    let longest_backtick_run = text
        .split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or(0);
    let fence = "`".repeat(longest_backtick_run + 1);
    let padding = if text.starts_with('`') || text.ends_with('`') {
        " "
    } else {
        ""
    };
    format!("{fence}{padding}{text}{padding}{fence}")
}

pub(crate) fn prose(text: &str) -> String {
    on_one_line(text)
        .chars()
        .fold(String::new(), |mut escaped, character| {
            if matches!(
                character,
                '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '&' | '~'
            ) {
                escaped.push('\\');
            }
            escaped.push(character);
            escaped
        })
}

fn on_one_line(text: &str) -> String {
    text.split(['\r', '\n'])
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
