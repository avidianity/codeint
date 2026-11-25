use std::collections::{BTreeMap, BTreeSet, HashSet};

use codeint_config::FindingsShown;
use codeint_model::Metric;

use super::types::{
    CloneFamilyChanges, DependenciesChanges, DuplicationChanges, ExternalMetricChanges,
    MetricChanges, SourceSetChanges,
};
use crate::measurement::{DependenciesMeasurement, MetricMeasurement, SourceSetMeasurement};
use crate::ownership::{OwnedFinding, OwnershipOptions};
use crate::{
    BoundaryFindingRow, CloneFamilyFinding, CloneGroupFinding, CognitiveComplexityFinding, Decimal,
    ErodedFunctionFinding, ExternalMetricFinding, Findings, FlaggedLineRow, HazardLineRow,
    ImportCycleFinding, ImportedDependencyRow, OrphanFileRow, UnusedDependencyRow,
    clustered_clone_group_ids, unlisted_clone_family_group_ids,
};

trait Identified {
    fn id(&self) -> &str;
}

trait FileAttributed {
    fn visit_files(&self, visit: &mut dyn FnMut(&str));
}

pub(super) fn source_set_changes(
    mut base: SourceSetMeasurement,
    mut head: SourceSetMeasurement,
    shown: FindingsShown,
    ownership: OwnershipOptions<'_>,
) -> SourceSetChanges {
    let keys: BTreeSet<String> = base
        .external_metrics
        .keys()
        .chain(head.external_metrics.keys())
        .cloned()
        .collect();
    let mut external_metrics = BTreeMap::new();
    for key in keys {
        let base_entry = base.external_metrics.remove(&key);
        let head_entry = head.external_metrics.remove(&key);
        let (name, description, metric_version) = match (&head_entry, &base_entry) {
            (Some(entry), _) | (None, Some(entry)) => (
                entry.name.clone(),
                entry.description.clone(),
                entry.metric_version,
            ),
            (None, None) => (key.clone(), String::new(), 1),
        };
        let base_measurement =
            base_entry.map_or_else(MetricMeasurement::default, |e| e.measurement);
        let head_measurement =
            head_entry.map_or_else(MetricMeasurement::default, |e| e.measurement);
        external_metrics.insert(
            key,
            ExternalMetricChanges {
                name,
                description,
                changes: classify(
                    metric_version,
                    base_measurement,
                    head_measurement,
                    shown,
                    ownership,
                ),
            },
        );
    }
    let import_cycles = classify_optional(
        Metric::ImportCycles.version(),
        base.import_cycles,
        head.import_cycles,
        shown,
        ownership,
    );
    let boundaries = classify_optional(
        Metric::Boundaries.version(),
        base.boundaries,
        head.boundaries,
        shown,
        ownership,
    );
    let orphan_files = classify_optional(
        Metric::OrphanFiles.version(),
        base.orphan_files,
        head.orphan_files,
        shown,
        ownership,
    );
    let hazards = classify_optional(
        Metric::Hazards.version(),
        base.hazards,
        head.hazards,
        shown,
        ownership,
    );
    let duplication_groups = classify(
        Metric::Duplication.version(),
        base.duplication,
        head.duplication,
        shown,
        ownership,
    );
    let base_families = ownership.apply(base.clone_families);
    let head_families = ownership.apply(head.clone_families);
    let clone_families = if base_families.is_empty() && head_families.is_empty() {
        None
    } else {
        let base_ids = ids(&base_families);
        let head_ids = ids(&head_families);
        let all_clustered = clustered_clone_group_ids(&head_families);
        let new = missing_from(head_families, &base_ids, shown);
        let resolved = missing_from(base_families, &head_ids, shown);
        let unlisted_clone_family_group_ids = unlisted_clone_family_group_ids(
            &duplication_groups.new.listed,
            &new.listed,
            &all_clustered,
        );
        Some(CloneFamilyChanges {
            unchanged: head_ids.intersection(&base_ids).count(),
            new,
            resolved,
            unlisted_clone_family_group_ids,
        })
    };
    SourceSetChanges {
        erosion: classify(
            Metric::Erosion.version(),
            base.erosion,
            head.erosion,
            shown,
            ownership,
        ),
        cognitive_complexity: classify(
            Metric::CognitiveComplexity.version(),
            base.cognitive_complexity,
            head.cognitive_complexity,
            shown,
            ownership,
        ),
        duplication: DuplicationChanges {
            groups: duplication_groups,
            clone_families,
        },
        verbosity: classify(
            Metric::Verbosity.version(),
            base.verbosity,
            head.verbosity,
            shown,
            ownership,
        ),
        import_cycles,
        boundaries,
        orphan_files,
        hazards,
        external_metrics,
    }
}

pub(super) fn dependencies_changes(
    base: Option<DependenciesMeasurement>,
    head: Option<DependenciesMeasurement>,
    shown: FindingsShown,
    ownership: OwnershipOptions<'_>,
) -> Option<DependenciesChanges> {
    if base.is_none() && head.is_none() {
        return None;
    }
    let base = base.unwrap_or_default();
    let head = head.unwrap_or_default();
    Some(DependenciesChanges {
        unused_dependencies: classify(
            Metric::UnusedDependencies.version(),
            base.unused_dependencies,
            head.unused_dependencies,
            shown,
            ownership,
        ),
        unlisted_dependencies: classify(
            Metric::UnlistedDependencies.version(),
            base.unlisted_dependencies,
            head.unlisted_dependencies,
            shown,
            ownership,
        ),
        dev_in_production_dependencies: classify(
            Metric::DevInProductionDependencies.version(),
            base.dev_in_production_dependencies,
            head.dev_in_production_dependencies,
            shown,
            ownership,
        ),
    })
}

fn classify<T: Identified + OwnedFinding + FileAttributed>(
    metric_version: u32,
    base: MetricMeasurement<T>,
    head: MetricMeasurement<T>,
    shown: FindingsShown,
    ownership: OwnershipOptions<'_>,
) -> MetricChanges<T> {
    let base_findings = ownership.apply(base.findings_worst_first);
    let head_findings = ownership.apply(head.findings_worst_first);
    let base_ids = ids(&base_findings);
    let head_ids = ids(&head_findings);
    let new_items: Vec<T> = head_findings
        .into_iter()
        .filter(|finding| !base_ids.contains(finding.id()))
        .collect();
    let new_findings_by_file = count_findings_by_file(&new_items);
    MetricChanges {
        metric_version,
        base_ratio: Decimal::rounded(base.ratio),
        head_ratio: Decimal::rounded(head.ratio),
        base_numerator: Decimal::rounded(base.numerator),
        head_numerator: Decimal::rounded(head.numerator),
        base_file_numerators: rounded_file_numerators(base.file_numerators),
        head_file_numerators: rounded_file_numerators(head.file_numerators),
        new_findings_by_file,
        unchanged: head_ids.intersection(&base_ids).count(),
        new: Findings::worst_first(new_items.into_iter(), shown),
        resolved: missing_from(base_findings, &head_ids, shown),
    }
}

fn classify_optional<T: Identified + OwnedFinding + FileAttributed>(
    metric_version: u32,
    base: Option<MetricMeasurement<T>>,
    head: Option<MetricMeasurement<T>>,
    shown: FindingsShown,
    ownership: OwnershipOptions<'_>,
) -> Option<MetricChanges<T>> {
    match (base, head) {
        (None, None) => None,
        (base_metric, head_metric) => Some(classify(
            metric_version,
            base_metric.unwrap_or_default(),
            head_metric.unwrap_or_default(),
            shown,
            ownership,
        )),
    }
}

fn rounded_file_numerators(raw: BTreeMap<String, f64>) -> BTreeMap<String, Decimal> {
    raw.into_iter()
        .map(|(path, value)| (path, Decimal::rounded(value)))
        .collect()
}

fn count_findings_by_file<T: FileAttributed>(findings: &[T]) -> BTreeMap<String, usize> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for finding in findings {
        let mut unique: BTreeSet<String> = BTreeSet::new();
        finding.visit_files(&mut |path| {
            unique.insert(path.to_owned());
        });
        for path in unique {
            *counts.entry(path).or_insert(0) += 1;
        }
    }
    counts
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

impl Identified for ErodedFunctionFinding {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Identified for CognitiveComplexityFinding {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Identified for CloneGroupFinding {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Identified for CloneFamilyFinding {
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

impl Identified for ExternalMetricFinding {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Identified for BoundaryFindingRow {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Identified for UnusedDependencyRow {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Identified for ImportedDependencyRow {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Identified for OrphanFileRow {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Identified for HazardLineRow {
    fn id(&self) -> &str {
        &self.id
    }
}

macro_rules! impl_single_file_attributed {
    ($ty:ty, $field:ident) => {
        impl FileAttributed for $ty {
            fn visit_files(&self, visit: &mut dyn FnMut(&str)) {
                visit(self.$field.as_str());
            }
        }
    };
}

impl_single_file_attributed!(ErodedFunctionFinding, path);
impl_single_file_attributed!(CognitiveComplexityFinding, path);
impl_single_file_attributed!(FlaggedLineRow, path);
impl_single_file_attributed!(ExternalMetricFinding, path);
impl_single_file_attributed!(BoundaryFindingRow, importer);
impl_single_file_attributed!(UnusedDependencyRow, manifest);
impl_single_file_attributed!(OrphanFileRow, path);
impl_single_file_attributed!(HazardLineRow, path);

impl FileAttributed for CloneGroupFinding {
    fn visit_files(&self, visit: &mut dyn FnMut(&str)) {
        for location in &self.locations {
            visit(location.path.as_str());
        }
    }
}

impl FileAttributed for ImportCycleFinding {
    fn visit_files(&self, visit: &mut dyn FnMut(&str)) {
        for module in &self.modules {
            visit(module.as_str());
        }
    }
}

impl FileAttributed for ImportedDependencyRow {
    fn visit_files(&self, visit: &mut dyn FnMut(&str)) {
        for importer in &self.importers {
            visit(importer.as_str());
        }
    }
}
