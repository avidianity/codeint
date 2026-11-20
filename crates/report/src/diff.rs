use codeint_config::{FindingsShown, ReportFormat};
use codeint_model::{CodeModel, SourceSet};
use std::collections::HashSet;

use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

use crate::sarif::{ListedFindings, SarifLog};
use crate::{
    CloneGroupFinding, Decimal, ErodedFunctionFinding, Findings, FlaggedLineRow,
    ImportCycleFinding, Measured, ReportError,
};

mod markdown;

const WORKING_TREE: &str = "working tree";

pub struct BaseCommit {
    pub git_ref: String,
    pub merge_base: String,
}

pub enum Head {
    WorkingTree,
    Commit(HeadCommit),
}

pub struct HeadCommit {
    pub git_ref: String,
    pub commit: String,
}

#[derive(Debug, Serialize)]
pub struct DiffReport {
    base: BaseEntry,
    head: HeadEntry,
    source_sets: SourceSetChangesSections,
}

#[derive(Debug, Serialize)]
struct BaseEntry {
    #[serde(rename = "ref")]
    git_ref: String,
    merge_base: String,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum HeadEntry {
    WorkingTree(&'static str),
    Commit {
        #[serde(rename = "ref")]
        git_ref: String,
        commit: String,
    },
}

#[derive(Debug, Default, Serialize)]
struct SourceSetChangesSections {
    #[serde(skip_serializing_if = "Option::is_none")]
    production: Option<SourceSetChanges>,
    #[serde(skip_serializing_if = "Option::is_none")]
    test: Option<SourceSetChanges>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generated: Option<SourceSetChanges>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vendored: Option<SourceSetChanges>,
}

#[derive(Debug, Serialize)]
struct SourceSetChanges {
    erosion: MetricChanges<ErodedFunctionFinding>,
    duplication: MetricChanges<CloneGroupFinding>,
    verbosity: MetricChanges<FlaggedLineRow>,
    import_cycles: MetricChanges<ImportCycleFinding>,
}

#[derive(Debug)]
struct MetricChanges<T> {
    base_ratio: Decimal,
    head_ratio: Decimal,
    new: Findings<T>,
    resolved: Findings<T>,
    unchanged: usize,
}

trait Identified {
    fn id(&self) -> &str;
}

struct Side<T> {
    ratio: f64,
    findings_worst_first: Vec<T>,
}

impl DiffReport {
    pub fn compare(
        base: BaseCommit,
        head: Head,
        base_model: CodeModel,
        head_model: CodeModel,
        findings: FindingsShown,
    ) -> DiffReport {
        let mut source_sets = SourceSetChangesSections::default();
        let base_sets = base_model.into_source_sets();
        let head_sets = head_model.into_source_sets();
        for ((source_set, base), (_, head)) in base_sets.iter().zip(head_sets.iter()) {
            if !base.files.is_empty() || !head.files.is_empty() {
                *source_sets.slot(source_set) = Some(source_set_changes(base, head, findings));
            }
        }
        DiffReport {
            base: BaseEntry {
                git_ref: base.git_ref,
                merge_base: base.merge_base,
            },
            head: head_entry(head),
            source_sets,
        }
    }

    pub fn render(&self, format: ReportFormat) -> Result<String, ReportError> {
        match format {
            ReportFormat::Toon => toon_format::encode_default(self).map_err(ReportError::Toon),
            ReportFormat::Json => serde_json::to_string_pretty(self).map_err(ReportError::Json),
            ReportFormat::Sarif => serde_json::to_string_pretty(&self.new_findings_as_sarif())
                .map_err(ReportError::Sarif),
            ReportFormat::Markdown => Ok(markdown::render(self)),
        }
    }
}

fn head_entry(head: Head) -> HeadEntry {
    match head {
        Head::WorkingTree => HeadEntry::WorkingTree(WORKING_TREE),
        Head::Commit(HeadCommit { git_ref, commit }) => HeadEntry::Commit { git_ref, commit },
    }
}

impl DiffReport {
    fn new_findings_as_sarif(&self) -> SarifLog {
        SarifLog::listing(self.source_sets.iter().map(|(source_set, changes)| {
            (
                source_set,
                ListedFindings {
                    eroded_functions: &changes.erosion.new.listed,
                    clone_groups: &changes.duplication.new.listed,
                    flagged_lines: &changes.verbosity.new.listed,
                    import_cycles: &changes.import_cycles.new.listed,
                },
            )
        }))
    }
}

impl<T: Serialize> Serialize for MetricChanges<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("base_ratio", &self.base_ratio)?;
        map.serialize_entry("head_ratio", &self.head_ratio)?;
        map.serialize_entry("new", &self.new.listed)?;
        if self.new.left_out > 0 {
            map.serialize_entry("new_left_out", &self.new.left_out)?;
        }
        map.serialize_entry("resolved", &self.resolved.listed)?;
        if self.resolved.left_out > 0 {
            map.serialize_entry("resolved_left_out", &self.resolved.left_out)?;
        }
        map.serialize_entry("unchanged", &self.unchanged)?;
        map.end()
    }
}

fn classify<T: Identified>(base: Side<T>, head: Side<T>, shown: FindingsShown) -> MetricChanges<T> {
    let base_ids = ids(&base.findings_worst_first);
    let head_ids = ids(&head.findings_worst_first);
    MetricChanges {
        base_ratio: Decimal::rounded(base.ratio),
        head_ratio: Decimal::rounded(head.ratio),
        unchanged: head_ids.intersection(&base_ids).count(),
        new: missing_from(head.findings_worst_first, &base_ids, shown),
        resolved: missing_from(base.findings_worst_first, &head_ids, shown),
    }
}

fn ids<T: Identified>(findings: &[T]) -> HashSet<String> {
    findings
        .iter()
        .map(|finding| finding.id().to_owned())
        .collect()
}

fn missing_from<T: Identified>(
    findings_worst_first: Vec<T>,
    other_side: &HashSet<String>,
    shown: FindingsShown,
) -> Findings<T> {
    let missing: Vec<T> = findings_worst_first
        .into_iter()
        .filter(|finding| !other_side.contains(finding.id()))
        .collect();
    Findings::worst_first(missing.into_iter(), shown)
}

fn source_set_changes(
    base: &CodeModel,
    head: &CodeModel,
    shown: FindingsShown,
) -> SourceSetChanges {
    let base = Measured::new(base);
    let head = Measured::new(head);
    SourceSetChanges {
        erosion: classify(erosion_side(&base), erosion_side(&head), shown),
        duplication: classify(duplication_side(&base), duplication_side(&head), shown),
        verbosity: classify(verbosity_side(&base), verbosity_side(&head), shown),
        import_cycles: classify(import_cycles_side(&base), import_cycles_side(&head), shown),
    }
}

fn erosion_side(measured: &Measured) -> Side<ErodedFunctionFinding> {
    Side {
        ratio: measured.erosion.ratio(),
        findings_worst_first: measured
            .erosion
            .eroded_functions()
            .iter()
            .map(ErodedFunctionFinding::new)
            .collect(),
    }
}

fn duplication_side(measured: &Measured) -> Side<CloneGroupFinding> {
    Side {
        ratio: measured.duplication.ratio(),
        findings_worst_first: measured
            .duplication
            .clone_groups
            .iter()
            .map(CloneGroupFinding::new)
            .collect(),
    }
}

fn verbosity_side(measured: &Measured) -> Side<FlaggedLineRow> {
    Side {
        ratio: measured.verbosity.ratio(),
        findings_worst_first: measured
            .verbosity
            .flagged_lines()
            .iter()
            .map(FlaggedLineRow::from)
            .collect(),
    }
}

fn import_cycles_side(measured: &Measured) -> Side<ImportCycleFinding> {
    Side {
        ratio: measured.import_cycles.ratio(),
        findings_worst_first: measured
            .import_cycles
            .cycles()
            .iter()
            .map(ImportCycleFinding::new)
            .collect(),
    }
}

impl SourceSetChangesSections {
    fn iter(&self) -> impl Iterator<Item = (SourceSet, &SourceSetChanges)> {
        [
            (SourceSet::Production, &self.production),
            (SourceSet::Test, &self.test),
            (SourceSet::Generated, &self.generated),
            (SourceSet::Vendored, &self.vendored),
        ]
        .into_iter()
        .filter_map(|(source_set, changes)| Some((source_set, changes.as_ref()?)))
    }

    fn named(&self) -> Vec<(&'static str, &SourceSetChanges)> {
        self.iter()
            .map(|(source_set, changes)| (source_set_name(source_set), changes))
            .collect()
    }

    fn slot(&mut self, source_set: SourceSet) -> &mut Option<SourceSetChanges> {
        match source_set {
            SourceSet::Production => &mut self.production,
            SourceSet::Test => &mut self.test,
            SourceSet::Generated => &mut self.generated,
            SourceSet::Vendored => &mut self.vendored,
        }
    }
}

impl Identified for ErodedFunctionFinding {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Identified for CloneGroupFinding {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Identified for FlaggedLineRow {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Identified for ImportCycleFinding {
    fn id(&self) -> &str {
        &self.id
    }
}

fn source_set_name(source_set: SourceSet) -> &'static str {
    match source_set {
        SourceSet::Production => "Production",
        SourceSet::Test => "Test",
        SourceSet::Generated => "Generated",
        SourceSet::Vendored => "Vendored",
    }
}
