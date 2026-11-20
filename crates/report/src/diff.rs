use codeint_config::{FindingsShown, ReportFormat};
use codeint_model::{CodeModel, SourceSet};
use serde::{Serialize, Serializer};

use crate::{
    CloneGroupFinding, Decimal, ErodedFunctionFinding, Findings, FlaggedLineRow,
    ImportCycleFinding, Measured, ReportError,
};

const WORKING_TREE: &str = "working tree";

pub struct BaseCommit {
    pub git_ref: String,
    pub merge_base: String,
}

#[derive(Debug, Serialize)]
pub struct DiffReport {
    base: BaseEntry,
    head: &'static str,
    source_sets: SourceSetChangesSections,
}

#[derive(Debug, Serialize)]
struct BaseEntry {
    #[serde(rename = "ref")]
    git_ref: String,
    merge_base: String,
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
        base_model: CodeModel,
        head_model: CodeModel,
        findings: FindingsShown,
    ) -> DiffReport {
        let mut source_sets = SourceSetChangesSections::default();
        let base_sets = base_model.into_source_sets();
        let head_sets = head_model.into_source_sets();
        for ((source_set, base), (_, head)) in base_sets.iter().zip(head_sets.iter()) {
            *source_sets.slot(source_set) = Some(source_set_changes(base, head, findings));
        }
        DiffReport {
            base: BaseEntry {
                git_ref: base.git_ref,
                merge_base: base.merge_base,
            },
            head: WORKING_TREE,
            source_sets,
        }
    }

    pub fn render(&self, format: ReportFormat) -> Result<String, ReportError> {
        let _ = format;
        todo!()
    }
}

impl<T: Serialize> Serialize for MetricChanges<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let _ = (
            serializer,
            &self.base_ratio,
            &self.head_ratio,
            &self.new,
            &self.resolved,
            self.unchanged,
        );
        todo!()
    }
}

fn classify<T: Identified>(base: Side<T>, head: Side<T>, shown: FindingsShown) -> MetricChanges<T> {
    let _ = (
        base.ratio,
        base.findings_worst_first.iter().map(Identified::id).count(),
        head.ratio,
        head.findings_worst_first,
        shown,
    );
    todo!()
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
    let _ = measured;
    todo!()
}

fn duplication_side(measured: &Measured) -> Side<CloneGroupFinding> {
    let _ = measured;
    todo!()
}

fn verbosity_side(measured: &Measured) -> Side<FlaggedLineRow> {
    let _ = measured;
    todo!()
}

fn import_cycles_side(measured: &Measured) -> Side<ImportCycleFinding> {
    let _ = measured;
    todo!()
}

impl SourceSetChangesSections {
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
