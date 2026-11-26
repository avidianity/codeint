use std::collections::{BTreeMap, BTreeSet};

use codeint_config::{Baseline, FindingAnchor, FindingsShown, match_anchors};
use codeint_model::{Metric, SourceSet};

use super::types::{
    CloneFamilyChanges, DependenciesChanges, DuplicationChanges, ExternalMetricChanges,
    MetricChanges, SourceSetChanges,
};
use crate::baseline::{BaselineScope, BaselineWeight, BudgetedRatio, Identified};
use crate::measurement::{
    DependenciesMeasurement, MetricMeasurement, SourceSetMeasurement, rehydrate_anchors,
};
use crate::ownership::{OwnedFinding, OwnershipOptions};
use crate::{
    BoundaryFindingRow, CloneGroupFinding, CognitiveComplexityFinding, Decimal,
    ErodedFunctionFinding, ExternalMetricFinding, Findings, FlaggedLineRow, HazardLineRow,
    ImportCycleFinding, ImportedDependencyRow, OrphanFileRow, UnusedDependencyRow,
    clustered_clone_group_ids, unlisted_clone_family_group_ids,
};

trait FileAttributed {
    fn visit_files(&self, visit: &mut dyn FnMut(&str));
}

pub(super) fn source_set_changes(
    source_set: SourceSet,
    mut base: SourceSetMeasurement,
    mut head: SourceSetMeasurement,
    shown: FindingsShown,
    ownership: OwnershipOptions<'_>,
    baseline: Option<&Baseline>,
) -> SourceSetChanges {
    let baseline_scope = baseline.map(|baseline| BaselineScope {
        baseline,
        source_set: Some(source_set),
    });
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
        let changes = classify(
            &key,
            metric_version,
            base_measurement,
            head_measurement,
            shown,
            ownership,
            baseline_scope,
        );
        external_metrics.insert(
            key,
            ExternalMetricChanges {
                name,
                description,
                changes,
            },
        );
    }
    let import_cycles = classify_optional(
        Metric::ImportCycles.key(),
        Metric::ImportCycles.version(),
        base.import_cycles,
        head.import_cycles,
        shown,
        ownership,
        baseline_scope,
    );
    let boundaries = classify_optional(
        Metric::Boundaries.key(),
        Metric::Boundaries.version(),
        base.boundaries,
        head.boundaries,
        shown,
        ownership,
        baseline_scope,
    );
    let orphan_files = classify_optional(
        Metric::OrphanFiles.key(),
        Metric::OrphanFiles.version(),
        base.orphan_files,
        head.orphan_files,
        shown,
        ownership,
        baseline_scope,
    );
    let hazards = classify_optional(
        Metric::Hazards.key(),
        Metric::Hazards.version(),
        base.hazards,
        head.hazards,
        shown,
        ownership,
        baseline_scope,
    );
    let duplication_groups = classify(
        Metric::Duplication.key(),
        Metric::Duplication.version(),
        base.duplication,
        head.duplication,
        shown,
        ownership,
        baseline_scope,
    );
    let base_families = ownership.apply(rehydrate_anchors(
        base.clone_families,
        base.clone_family_anchors,
    ));
    let head_families = ownership.apply(rehydrate_anchors(
        head.clone_families,
        head.clone_family_anchors,
    ));
    let clone_families = if base_families.is_empty() && head_families.is_empty() {
        None
    } else {
        let all_clustered = clustered_clone_group_ids(&head_families);
        let (new_items, resolved_items, unchanged) = partition_diff(base_families, head_families);
        let new = Findings::worst_first(new_items.into_iter(), shown);
        let resolved = Findings::worst_first(resolved_items.into_iter(), shown);
        let unlisted_clone_family_group_ids = unlisted_clone_family_group_ids(
            &duplication_groups.new.listed,
            &new.listed,
            &all_clustered,
        );
        Some(CloneFamilyChanges {
            unchanged,
            new,
            resolved,
            unlisted_clone_family_group_ids,
        })
    };
    SourceSetChanges {
        erosion: classify(
            Metric::Erosion.key(),
            Metric::Erosion.version(),
            base.erosion,
            head.erosion,
            shown,
            ownership,
            baseline_scope,
        ),
        cognitive_complexity: classify(
            Metric::CognitiveComplexity.key(),
            Metric::CognitiveComplexity.version(),
            base.cognitive_complexity,
            head.cognitive_complexity,
            shown,
            ownership,
            baseline_scope,
        ),
        duplication: DuplicationChanges {
            groups: duplication_groups,
            clone_families,
        },
        verbosity: classify(
            Metric::Verbosity.key(),
            Metric::Verbosity.version(),
            base.verbosity,
            head.verbosity,
            shown,
            ownership,
            baseline_scope,
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
    baseline: Option<&Baseline>,
) -> Option<DependenciesChanges> {
    if base.is_none() && head.is_none() {
        return None;
    }
    let baseline_scope = baseline.map(|baseline| BaselineScope {
        baseline,
        source_set: None,
    });
    let base = base.unwrap_or_default();
    let head = head.unwrap_or_default();
    Some(DependenciesChanges {
        unused_dependencies: classify(
            Metric::UnusedDependencies.key(),
            Metric::UnusedDependencies.version(),
            base.unused_dependencies,
            head.unused_dependencies,
            shown,
            ownership,
            baseline_scope,
        ),
        unlisted_dependencies: classify(
            Metric::UnlistedDependencies.key(),
            Metric::UnlistedDependencies.version(),
            base.unlisted_dependencies,
            head.unlisted_dependencies,
            shown,
            ownership,
            baseline_scope,
        ),
        dev_in_production_dependencies: classify(
            Metric::DevInProductionDependencies.key(),
            Metric::DevInProductionDependencies.version(),
            base.dev_in_production_dependencies,
            head.dev_in_production_dependencies,
            shown,
            ownership,
            baseline_scope,
        ),
    })
}

fn classify<T: Identified + OwnedFinding + FileAttributed + BaselineWeight>(
    metric_key: &str,
    metric_version: u32,
    base: MetricMeasurement<T>,
    head: MetricMeasurement<T>,
    shown: FindingsShown,
    ownership: OwnershipOptions<'_>,
    baseline: Option<BaselineScope<'_>>,
) -> MetricChanges<T> {
    let base_ratio = Decimal::rounded(base.ratio);
    let head_ratio = Decimal::rounded(head.ratio);
    let base_numerator = Decimal::rounded(base.numerator);
    let head_numerator = Decimal::rounded(head.numerator);
    let base_file_numerators = rounded_file_numerators(base.file_numerators);
    let head_file_numerators = rounded_file_numerators(head.file_numerators);
    let base_findings = ownership.apply(rehydrate_anchors(
        base.findings_worst_first,
        base.finding_anchors,
    ));
    let head_findings_all = rehydrate_anchors(head.findings_worst_first, head.finding_anchors);
    let head_budgeted_ratio =
        match baseline.and_then(|scope| scope.retained_share(metric_key, &head_findings_all)) {
            Some(share) => BudgetedRatio {
                ratio: Decimal::rounded(head_ratio.0 * share).0,
                non_baselined: true,
            },
            None => BudgetedRatio::from(head_ratio.0),
        };
    let head_findings = ownership.apply(head_findings_all);
    let (new_items, resolved_items, unchanged) = partition_diff(base_findings, head_findings);
    let new_findings_by_file = count_findings_by_file(&new_items);
    MetricChanges {
        metric_version,
        base_ratio,
        head_ratio,
        head_budgeted_ratio,
        base_numerator,
        head_numerator,
        base_file_numerators,
        head_file_numerators,
        new_findings_by_file,
        unchanged,
        new: Findings::worst_first(new_items.into_iter(), shown),
        resolved: Findings::worst_first(resolved_items.into_iter(), shown),
    }
}

fn partition_diff<T: Identified>(base: Vec<T>, head: Vec<T>) -> (Vec<T>, Vec<T>, usize) {
    let base_anchors: Vec<FindingAnchor> = base.iter().map(Identified::anchor).collect();
    let head_anchors: Vec<FindingAnchor> = head.iter().map(Identified::anchor).collect();
    let matched = match_anchors(&base_anchors, &head_anchors);
    let unchanged = matched.matched_pairs.len();
    let base_matched = matched.matched_references(base.len());
    let head_matched = matched.matched_candidates(head.len());
    let new_items: Vec<T> = head
        .into_iter()
        .zip(head_matched)
        .filter_map(|(finding, is_matched)| (!is_matched).then_some(finding))
        .collect();
    let resolved_items: Vec<T> = base
        .into_iter()
        .zip(base_matched)
        .filter_map(|(finding, is_matched)| (!is_matched).then_some(finding))
        .collect();
    (new_items, resolved_items, unchanged)
}

fn classify_optional<T: Identified + OwnedFinding + FileAttributed + BaselineWeight>(
    metric_key: &str,
    metric_version: u32,
    base: Option<MetricMeasurement<T>>,
    head: Option<MetricMeasurement<T>>,
    shown: FindingsShown,
    ownership: OwnershipOptions<'_>,
    baseline: Option<BaselineScope<'_>>,
) -> Option<MetricChanges<T>> {
    match (base, head) {
        (None, None) => None,
        (base_metric, head_metric) => Some(classify(
            metric_key,
            metric_version,
            base_metric.unwrap_or_default(),
            head_metric.unwrap_or_default(),
            shown,
            ownership,
            baseline,
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
