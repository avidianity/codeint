use std::collections::{BTreeMap, BTreeSet};

use codeint_config::{
    Baseline, Boundaries as BoundariesConfig, BoundaryDirective, BoundaryRuleDecision,
    DuplicationConfig, FindingsShown,
};
use codeint_metrics::{
    Boundaries, BoundaryEdgeCheck, BoundaryPolicy, CognitiveComplexity, DependencyHygiene,
    Duplication, Erosion, Hazards, ImportCycles, MinimumCloneTokens, MinimumDistinctTokens,
    NearCloneConfig, OrphanFiles, TestGap, Verbosity,
};
use codeint_model::{
    CodeModel, Metric, MetricPlugin, MetricPluginError, Package, RepoPath, SourceSet,
    SourceSetModel, SourceSets,
};

use crate::baseline::BaselineScope;
use crate::findings::{
    CloneFamilyFinding, clustered_clone_group_ids, unlisted_clone_family_group_ids,
};
use crate::ownership::OwnershipOptions;
use crate::render::{
    Decimal, Findings, FindingsView, NO_IMPORTS_METRIC_NOTE, NOT_SHOWN_FOR_NEAR_CLONES,
    ReferenceRangeEntry, ReferenceRangeScope, TEST_GAP_NO_LANGUAGE_COVERAGE_NOTE,
    TEST_GAP_NO_REPO_COVERAGE_NOTE, TEST_GAP_NO_REPORT_NOTE,
};
use crate::sections::{
    BoundariesSection, CognitiveComplexityTotals, DependenciesSection,
    DevInProductionDependenciesSection, DuplicationSection, ErosionTotals, ExternalMetricSection,
    HazardsSection, ImportCyclesSection, MeasuredSection, MetricHeader, MetricRatioHeader,
    MetricSection, NearClonesSummary, OrphanFilesSection, PackageCognitiveComplexity,
    PackageDuplication, PackageErosion, PackageHazards, PackageImportCycles, PackageSection,
    PackageTestGap, PackageVerbosity, SourceSetSection, SourceSetSections, TestGapSection,
    UnlistedDependenciesSection, UnresolvedImportsSection, UnusedDependenciesSection,
    VerbosityTotals,
};

struct ConfigBoundaryPolicy<'config>(&'config BoundariesConfig);

impl BoundaryPolicy for ConfigBoundaryPolicy<'_> {
    fn zone_of(&self, path: &RepoPath) -> Option<&str> {
        self.0.zone_of(path)
    }

    fn has_public_files(&self, zone: &str) -> bool {
        self.0.has_public_files(zone)
    }

    fn is_public_file(&self, zone: &str, path: &RepoPath) -> bool {
        self.0.is_public_file(zone, path)
    }

    fn check_import(&self, source_zone: &str, target_zone: &str) -> BoundaryEdgeCheck {
        map_boundary_decision(self.0.check_import(source_zone, target_zone))
    }

    fn has_external_rules(&self) -> bool {
        self.0.has_external_rules()
    }

    fn check_external_import(&self, source_zone: &str, package: &str) -> BoundaryEdgeCheck {
        map_boundary_decision(self.0.check_external_import(source_zone, package))
    }

    fn strict_zones(&self) -> bool {
        self.0.strict_zones()
    }
}

fn map_boundary_decision(decision: BoundaryRuleDecision) -> BoundaryEdgeCheck {
    match decision {
        BoundaryRuleDecision::Allowed => BoundaryEdgeCheck::Allowed,
        BoundaryRuleDecision::Denied {
            directive: BoundaryDirective::Allow,
            reason,
        } => BoundaryEdgeCheck::DeniedByAllowRule { reason },
        BoundaryRuleDecision::Denied {
            directive: BoundaryDirective::Deny,
            reason,
        } => BoundaryEdgeCheck::DeniedByDenyRule { reason },
    }
}

#[derive(Clone, Copy)]
pub(crate) struct SectionOptions<'a> {
    pub(crate) findings: FindingsShown,
    pub(crate) metric_plugins: &'a [&'a dyn MetricPlugin],
    pub(crate) boundaries: Option<&'a BoundariesConfig>,
    pub(crate) duplication: DuplicationConfig,
    pub(crate) orphan_files: Option<&'a OrphanFiles>,
    pub(crate) ownership: OwnershipOptions<'a>,
    pub(crate) has_imports: bool,
    pub(crate) has_hazard_rules: bool,
    pub(crate) disabled_hazard_rules: &'a BTreeSet<String>,
    pub(crate) coverage_reports_named: bool,
    pub(crate) per_language_section: bool,
    pub(crate) framework_profiles: &'a [String],
    pub(crate) framework_profiles_by_package: &'a BTreeMap<String, Vec<String>>,
    pub(crate) reference_range_note: Option<&'a str>,
    pub(crate) baseline: Option<&'a Baseline>,
}

impl<'a> SectionOptions<'a> {
    fn findings_view(&self, source_set: Option<SourceSet>) -> FindingsView<'a> {
        FindingsView {
            shown: self.findings,
            ownership: self.ownership,
            baseline: self.baseline.map(|baseline| BaselineScope {
                baseline,
                source_set,
            }),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct SectionModels<'a> {
    pub(crate) model: &'a SourceSetModel,
    pub(crate) import_model: Option<&'a SourceSetModel>,
    pub(crate) coverage_model: Option<&'a SourceSetModel>,
    pub(crate) hazards_model: Option<&'a SourceSetModel>,
}

pub(crate) struct MeasuredSectionModels {
    pub(crate) model: CodeModel,
    pub(crate) import_model: Option<CodeModel>,
    pub(crate) coverage_model: Option<CodeModel>,
    pub(crate) hazards_model: Option<CodeModel>,
}

pub(crate) struct Measured {
    pub(crate) erosion: Erosion,
    pub(crate) cognitive_complexity: CognitiveComplexity,
    pub(crate) duplication: Duplication,
    pub(crate) verbosity: Verbosity,
    pub(crate) import_cycles: Option<ImportCycles>,
    pub(crate) boundaries: Option<Boundaries>,
    pub(crate) test_gap: Option<TestGap>,
    pub(crate) hazards: Option<Hazards>,
}

pub(crate) fn near_clone_config(duplication_config: DuplicationConfig) -> Option<NearCloneConfig> {
    duplication_config.near_clones.then(|| {
        NearCloneConfig::new(duplication_config.min_similarity.ratio())
            .expect("validated min_similarity is in (0, 1]")
    })
}

impl Measured {
    pub(crate) fn new(
        models: SectionModels<'_>,
        boundaries: Option<&BoundariesConfig>,
        duplication_config: DuplicationConfig,
        disabled_hazard_rules: &BTreeSet<String>,
        precomputed_duplication: Option<Duplication>,
    ) -> Measured {
        let model = models.model;
        let near_clones = near_clone_config(duplication_config);
        let (
            duplication,
            (
                (erosion, cognitive_complexity),
                ((verbosity, import_cycles), (boundaries, (test_gap, hazards))),
            ),
        ) = rayon::join(
            || match precomputed_duplication {
                Some(duplication) => duplication,
                None => Duplication::measure_with_near_clones(
                    model,
                    MinimumCloneTokens::DEFAULT,
                    MinimumDistinctTokens::DEFAULT,
                    near_clones,
                ),
            },
            || {
                rayon::join(
                    || {
                        rayon::join(
                            || Erosion::measure(model),
                            || CognitiveComplexity::measure(model),
                        )
                    },
                    || {
                        rayon::join(
                            || {
                                rayon::join(
                                    || Verbosity::measure(model),
                                    || models.import_model.map(ImportCycles::measure),
                                )
                            },
                            || {
                                rayon::join(
                                    || {
                                        models.import_model.and_then(|imports| {
                                            boundaries.map(|config| {
                                                Boundaries::measure(
                                                    imports,
                                                    &ConfigBoundaryPolicy(config),
                                                )
                                            })
                                        })
                                    },
                                    || {
                                        rayon::join(
                                            || models.coverage_model.and_then(TestGap::measure),
                                            || {
                                                models.hazards_model.and_then(|hazards_model| {
                                                    Hazards::measure(
                                                        hazards_model,
                                                        disabled_hazard_rules,
                                                    )
                                                })
                                            },
                                        )
                                    },
                                )
                            },
                        )
                    },
                )
            },
        );
        Measured {
            erosion,
            cognitive_complexity,
            verbosity,
            duplication,
            import_cycles,
            boundaries,
            test_gap,
            hazards,
        }
    }
}

impl DependenciesSection {
    pub(crate) fn from_hygiene(
        hygiene: &DependencyHygiene,
        view: FindingsView<'_>,
    ) -> Option<DependenciesSection> {
        if !hygiene.has_activity() {
            return None;
        }
        Some(DependenciesSection {
            unused_dependencies: UnusedDependenciesSection {
                header: MetricRatioHeader::new(hygiene.unused_ratio(), Metric::UnusedDependencies),
                unused: hygiene.unused().len(),
                declared: hygiene.declared_count(),
                findings: view.map_baselined(Metric::UnusedDependencies.key(), hygiene.unused()),
            },
            unlisted_dependencies: UnlistedDependenciesSection {
                header: MetricRatioHeader::new(
                    hygiene.unlisted_ratio(),
                    Metric::UnlistedDependencies,
                ),
                unlisted: hygiene.unlisted().len(),
                imported: hygiene.imported_count(),
                findings: view
                    .map_baselined(Metric::UnlistedDependencies.key(), hygiene.unlisted()),
            },
            dev_in_production_dependencies: DevInProductionDependenciesSection {
                header: MetricRatioHeader::new(
                    hygiene.dev_in_production_ratio(),
                    Metric::DevInProductionDependencies,
                ),
                dev_in_production: hygiene.dev_in_production().len(),
                production_imported: hygiene.production_imported_count(),
                findings: view.map_baselined(
                    Metric::DevInProductionDependencies.key(),
                    hygiene.dev_in_production(),
                ),
            },
        })
    }
}

impl BoundariesSection {
    fn new(boundaries: &Boundaries, view: FindingsView<'_>) -> BoundariesSection {
        BoundariesSection {
            header: MetricRatioHeader::new(boundaries.ratio(), Metric::Boundaries),
            resolved_imports: boundaries.resolved_imports(),
            findings: view.map_baselined(Metric::Boundaries.key(), boundaries.findings()),
        }
    }
}

impl OrphanFilesSection {
    fn new(orphan_files: &OrphanFiles, view: FindingsView<'_>) -> OrphanFilesSection {
        let all_findings = orphan_files.findings();
        let findings = view.map_baselined(Metric::OrphanFiles.key(), all_findings);
        OrphanFilesSection {
            header: MetricRatioHeader::new(orphan_files.ratio(), Metric::OrphanFiles),
            orphan_files: all_findings.len(),
            evaluated_files: orphan_files.evaluated_files(),
            entry_points: orphan_files.entry_points().len(),
            unreferenced: orphan_files.unreferenced_count(),
            test_only: orphan_files.test_only_count(),
            findings,
        }
    }
}

impl TestGapSection {
    fn new(test_gap: &TestGap, view: FindingsView<'_>) -> TestGapSection {
        let totals = test_gap.totals();
        let findings = view.map_baselined(Metric::TestGap.key(), test_gap.findings());
        TestGapSection {
            header: MetricRatioHeader::new(totals.ratio(), Metric::TestGap),
            untested_function_mass: Decimal::rounded(totals.untested),
            total_function_mass: Decimal::rounded(totals.total),
            measured_functions: totals.measured_functions,
            findings,
        }
    }
}

impl HazardsSection {
    fn new(hazards: &Hazards, view: FindingsView<'_>) -> HazardsSection {
        HazardsSection {
            header: MetricRatioHeader::new(hazards.ratio(), Metric::Hazards),
            hazard_lines: hazards.hazard_lines(),
            source_lines: hazards.source_lines(),
            findings: view.map_baselined(Metric::Hazards.key(), hazards.findings()),
        }
    }
}

impl DuplicationSection {
    fn new(
        duplication: &Duplication,
        ranges: &ReferenceRangeScope,
        view: FindingsView<'_>,
        config: DuplicationConfig,
    ) -> DuplicationSection {
        let (reference_range, near_clones) = if config.near_clones {
            (
                ReferenceRangeEntry::NotShown(NOT_SHOWN_FOR_NEAR_CLONES.to_owned()),
                Some(NearClonesSummary {
                    min_similarity: config.min_similarity.ratio(),
                }),
            )
        } else {
            (
                ranges.entry(Metric::Duplication.key(), Metric::Duplication.version()),
                None,
            )
        };
        let (clone_groups, hidden_group_ids) =
            view.map_baselined_with_hidden(Metric::Duplication.key(), &duplication.clone_groups);
        let all_clone_families = view.ownership.apply(
            duplication
                .clone_families
                .iter()
                .map(CloneFamilyFinding::from)
                .filter(|family| {
                    family.clone_group_ids.is_empty()
                        || !family
                            .clone_group_ids
                            .iter()
                            .all(|id| hidden_group_ids.contains(id))
                })
                .collect(),
        );
        let all_clustered = clustered_clone_group_ids(&all_clone_families);
        let clone_families = Findings::worst_first(all_clone_families.into_iter(), view.shown);
        let unlisted_clone_family_group_ids = unlisted_clone_family_group_ids(
            &clone_groups.listed,
            &clone_families.listed,
            &all_clustered,
        );
        DuplicationSection {
            header: MetricHeader {
                ratio: Decimal::rounded(duplication.ratio()),
                metric_version: Metric::Duplication.version(),
                reference_range,
            },
            near_clones,
            clone_lines: duplication.clone_lines.count(),
            clone_groups,
            clone_families,
            unlisted_clone_family_group_ids,
        }
    }
}

impl UnresolvedImportsSection {
    fn new(import_cycles: &ImportCycles, view: FindingsView<'_>) -> UnresolvedImportsSection {
        let imports = view.map(import_cycles.unresolved_imports());
        UnresolvedImportsSection {
            count: imports.listed.len() + imports.left_out,
            imports,
        }
    }
}

impl SourceSetSections {
    fn new(
        source_sets: &SourceSets,
        import_source_sets: Option<&SourceSets>,
        coverage_source_sets: Option<&SourceSets>,
        hazards_source_sets: Option<&SourceSets>,
        ranges: &ReferenceRangeScope,
        options: SectionOptions<'_>,
        mut precomputed_duplication: Option<BTreeMap<SourceSet, Duplication>>,
    ) -> Result<(SourceSetSections, BTreeMap<SourceSet, Duplication>), MetricPluginError> {
        let mut sections = SourceSetSections::default();
        let mut measured_duplication = BTreeMap::new();
        for (source_set, model) in source_sets.iter() {
            if !model.files.is_empty() {
                let import_model = select_non_empty(import_source_sets, source_set);
                let coverage_model = select_coverage_model(source_set, model, coverage_source_sets);
                let hazards_model = select_non_empty(hazards_source_sets, source_set);
                let source_set_options = SectionOptions {
                    boundaries: options.boundaries.filter(|config| {
                        import_model.is_some() && config.applies_to_source_set(source_set)
                    }),
                    orphan_files: options
                        .orphan_files
                        .filter(|_| import_model.is_some() && source_set == SourceSet::Production),
                    ..options
                };
                let precomputed = precomputed_duplication
                    .as_mut()
                    .and_then(|map| map.remove(&source_set));
                let (section, duplication) = SourceSetSection::new(
                    source_set,
                    SectionModels {
                        model,
                        import_model,
                        coverage_model,
                        hazards_model,
                    },
                    ranges,
                    source_set_options,
                    precomputed,
                )?;
                *sections.slot(source_set) = Some(section);
                measured_duplication.insert(source_set, duplication);
            }
        }
        Ok((sections, measured_duplication))
    }
}

fn select_non_empty(
    source_sets: Option<&SourceSets>,
    source_set: SourceSet,
) -> Option<&SourceSetModel> {
    source_sets
        .map(|sets| sets.get(source_set))
        .filter(|model| !model.files.is_empty())
}

fn select_coverage_model<'a>(
    source_set: SourceSet,
    model: &'a SourceSetModel,
    coverage_source_sets: Option<&'a SourceSets>,
) -> Option<&'a SourceSetModel> {
    if source_set != SourceSet::Production {
        return None;
    }
    match coverage_source_sets {
        Some(sets) => select_non_empty(Some(sets), SourceSet::Production),
        None => Some(model),
    }
}

impl SourceSetSection {
    fn new(
        source_set: SourceSet,
        models: SectionModels<'_>,
        ranges: &ReferenceRangeScope,
        options: SectionOptions<'_>,
        precomputed_duplication: Option<Duplication>,
    ) -> Result<(SourceSetSection, Duplication), MetricPluginError> {
        let model = models.model;
        let view = options.findings_view(Some(source_set));
        let effective_import_model = models.import_model.filter(|_| options.has_imports);
        let effective_hazards_model = models.hazards_model.filter(|_| options.has_hazard_rules);
        let boundaries_config = effective_import_model
            .is_some()
            .then_some(options.boundaries)
            .flatten();
        let measured = Measured::new(
            SectionModels {
                model,
                import_model: effective_import_model,
                coverage_model: models.coverage_model,
                hazards_model: effective_hazards_model,
            },
            boundaries_config,
            options.duplication,
            options.disabled_hazard_rules,
            precomputed_duplication,
        );
        let boundaries = measured
            .boundaries
            .as_ref()
            .map(|evaluated| BoundariesSection::new(evaluated, view));
        let orphan_files = effective_import_model
            .is_some()
            .then_some(options.orphan_files)
            .flatten()
            .map(|measured| OrphanFilesSection::new(measured, view));
        let test_gap = measured
            .test_gap
            .as_ref()
            .map(|evaluated| TestGapSection::new(evaluated, view));
        let hazards = measured
            .hazards
            .as_ref()
            .map(|evaluated| HazardsSection::new(evaluated, view));
        let mut external_metrics = BTreeMap::new();
        let external_model = (!options.metric_plugins.is_empty()).then(|| model.to_code_model());
        for plugin in options.metric_plugins {
            if let Some(plugin_model) = external_model.as_ref() {
                let result = plugin.measure(plugin_model)?;
                external_metrics.insert(
                    plugin.key().to_owned(),
                    ExternalMetricSection {
                        name: plugin.name().to_owned(),
                        description: plugin.description().to_owned(),
                        header: MetricHeader::new(
                            result.ratio,
                            ranges,
                            plugin.key(),
                            plugin.version(),
                        ),
                        numerator: Decimal::rounded(result.numerator),
                        denominator: Decimal::rounded(result.denominator),
                        findings: view.map_baselined(plugin.key(), &result.findings),
                    },
                );
            }
        }
        let section = SourceSetSection {
            files: model.files.len(),
            source_lines: source_lines(&model.files),
            erosion: MetricSection {
                header: MetricHeader::new(
                    measured.erosion.ratio(),
                    ranges,
                    Metric::Erosion.key(),
                    Metric::Erosion.version(),
                ),
                totals: ErosionTotals {
                    eroded_function_mass: Decimal::rounded(measured.erosion.eroded_function_mass()),
                    total_function_mass: Decimal::rounded(measured.erosion.total_function_mass()),
                },
                findings: view
                    .map_baselined(Metric::Erosion.key(), measured.erosion.eroded_functions()),
            },
            cognitive_complexity: MetricSection {
                header: MetricHeader::new(
                    measured.cognitive_complexity.ratio(),
                    ranges,
                    Metric::CognitiveComplexity.key(),
                    Metric::CognitiveComplexity.version(),
                ),
                totals: CognitiveComplexityTotals {
                    complex_cognitive_complexity: measured
                        .cognitive_complexity
                        .complex_cognitive_complexity(),
                    total_cognitive_complexity: measured
                        .cognitive_complexity
                        .total_cognitive_complexity(),
                },
                findings: view.map_baselined(
                    Metric::CognitiveComplexity.key(),
                    measured.cognitive_complexity.complex_functions(),
                ),
            },
            duplication: DuplicationSection::new(
                &measured.duplication,
                ranges,
                view,
                options.duplication,
            ),
            verbosity: MetricSection {
                header: MetricHeader::new(
                    measured.verbosity.ratio(),
                    ranges,
                    Metric::Verbosity.key(),
                    Metric::Verbosity.version(),
                ),
                totals: VerbosityTotals {
                    verbose_lines: measured.verbosity.verbose_lines(),
                    flagged_lines: measured.verbosity.flagged_line_count(),
                },
                findings: view
                    .map_baselined(Metric::Verbosity.key(), measured.verbosity.flagged_lines()),
            },
            import_cycles: measured
                .import_cycles
                .as_ref()
                .map(|cycles| ImportCyclesSection {
                    header: MetricHeader::new(
                        cycles.ratio(),
                        ranges,
                        Metric::ImportCycles.key(),
                        Metric::ImportCycles.version(),
                    ),
                    modules_in_import_cycles: cycles.modules_in_import_cycles(),
                    modules: cycles.modules(),
                    findings: view.map_baselined(Metric::ImportCycles.key(), cycles.cycles()),
                    unresolved_imports: UnresolvedImportsSection::new(cycles, view),
                }),
            boundaries,
            orphan_files,
            test_gap,
            hazards,
            external_metrics,
            packages: package_sections(
                SectionModels {
                    model,
                    import_model: effective_import_model,
                    coverage_model: models.coverage_model,
                    hazards_model: effective_hazards_model,
                },
                &measured,
                options.framework_profiles_by_package,
            ),
        };
        Ok((section, measured.duplication))
    }
}

fn source_lines(files: &[codeint_model::SourceFile]) -> u64 {
    files
        .iter()
        .map(|file| u64::from(file.model.source_lines))
        .sum()
}

fn sorted_unique_profiles(profiles: &[String]) -> Vec<String> {
    let unique: BTreeSet<String> = profiles.iter().cloned().collect();
    unique.into_iter().collect()
}

fn package_sections<'a>(
    models: SectionModels<'a>,
    measured: &Measured,
    framework_profiles_by_package: &BTreeMap<String, Vec<String>>,
) -> Vec<PackageSection> {
    let model = models.model;
    if !model.workspace.has_members() {
        return Vec::new();
    }
    let mut files: BTreeMap<Package<'a>, usize> = BTreeMap::new();
    for file in &model.files {
        *files
            .entry(model.workspace.package_of(&file.path))
            .or_default() += 1;
    }
    let mut cognitive = measured.cognitive_complexity.by_package(model);
    let mut duplication = measured.duplication.by_package(model);
    let mut verbosity = measured.verbosity.by_package(model);
    let mut import_cycles = models
        .import_model
        .zip(measured.import_cycles.as_ref())
        .map(|(imports, cycles)| cycles.by_package(imports));
    let mut test_gap = models
        .coverage_model
        .zip(measured.test_gap.as_ref())
        .map(|(coverage, gap)| gap.by_package(coverage));
    let mut hazards = models
        .hazards_model
        .zip(measured.hazards.as_ref())
        .map(|(hazards_model, hazards)| hazards.by_package(hazards_model));
    measured
        .erosion
        .by_package(model)
        .into_iter()
        .map(|(package, masses)| {
            let package_files = files.get(&package).copied().unwrap_or_default();
            let cognitive_totals = cognitive.remove(&package).unwrap_or_default();
            let clone_lines = duplication.remove(&package).unwrap_or_default();
            let verbose_lines = verbosity.remove(&package).unwrap_or_default();
            let package_import_cycles = import_cycles.as_mut().and_then(|by_package| {
                let modules = by_package.remove(&package).unwrap_or_default();
                (modules.modules > 0 || package_files == 0).then(|| PackageImportCycles {
                    ratio: Decimal::rounded(modules.ratio()),
                    modules_in_import_cycles: modules.modules_in_import_cycles,
                })
            });
            let package_test_gap = test_gap.as_mut().and_then(|by_package| {
                let gap = by_package.remove(&package).unwrap_or_default();
                (gap.files > 0).then(|| PackageTestGap {
                    ratio: Decimal::rounded(gap.ratio()),
                    untested_function_mass: Decimal::rounded(gap.untested),
                    total_function_mass: Decimal::rounded(gap.total),
                })
            });
            let package_hazards = hazards.as_mut().and_then(|by_package| {
                by_package.remove(&package).map(|lines| PackageHazards {
                    ratio: Decimal::rounded(lines.ratio()),
                    hazard_lines: lines.hazard_lines,
                    source_lines: lines.source_lines,
                })
            });
            let package_path = package.to_string();
            let package_framework_profiles = framework_profiles_by_package
                .get(&package_path)
                .map(Vec::as_slice)
                .map(sorted_unique_profiles)
                .unwrap_or_default();
            PackageSection {
                path: package_path,
                files: package_files,
                source_lines: clone_lines.source_lines,
                framework_profiles: package_framework_profiles,
                erosion: PackageErosion {
                    ratio: Decimal::rounded(masses.ratio()),
                    eroded_function_mass: Decimal::rounded(masses.eroded),
                    total_function_mass: Decimal::rounded(masses.total),
                },
                cognitive_complexity: PackageCognitiveComplexity {
                    ratio: Decimal::rounded(cognitive_totals.ratio()),
                    complex_cognitive_complexity: cognitive_totals.complex,
                    total_cognitive_complexity: cognitive_totals.total,
                },
                duplication: PackageDuplication {
                    ratio: Decimal::rounded(clone_lines.ratio()),
                    clone_lines: clone_lines.lines,
                },
                verbosity: PackageVerbosity {
                    ratio: Decimal::rounded(verbose_lines.ratio()),
                    verbose_lines: verbose_lines.lines,
                },
                import_cycles: package_import_cycles,
                test_gap: package_test_gap,
                hazards: package_hazards,
            }
        })
        .collect()
}

impl MeasuredSection {
    pub(crate) fn new(
        models: MeasuredSectionModels,
        hygiene: &DependencyHygiene,
        ranges: &ReferenceRangeScope,
        options: SectionOptions<'_>,
        precomputed_duplication: Option<BTreeMap<SourceSet, Duplication>>,
    ) -> Result<(MeasuredSection, BTreeMap<SourceSet, Duplication>), MetricPluginError> {
        let files = models.model.files.len();
        let lines = source_lines(&models.model.files);
        let view = options.findings_view(None);
        let dependencies = options
            .has_imports
            .then(|| DependenciesSection::from_hygiene(hygiene, view))
            .flatten();
        let import_metrics_note = (!options.has_imports).then(|| NO_IMPORTS_METRIC_NOTE.to_owned());
        let section_options = if options.has_imports {
            options
        } else {
            SectionOptions {
                boundaries: None,
                orphan_files: None,
                ..options
            }
        };
        let source_sets = models.model.into_source_sets();
        let separate_import_source_sets = models.import_model.map(CodeModel::into_source_sets);
        let import_source_sets = if options.has_imports {
            Some(separate_import_source_sets.as_ref().unwrap_or(&source_sets))
        } else {
            None
        };
        let separate_coverage_source_sets = models.coverage_model.map(CodeModel::into_source_sets);
        let separate_hazards_source_sets = models.hazards_model.map(CodeModel::into_source_sets);
        let hazards_source_sets = if options.has_hazard_rules {
            Some(
                separate_hazards_source_sets
                    .as_ref()
                    .unwrap_or(&source_sets),
            )
        } else {
            None
        };
        let (source_sets, measured_duplication) = SourceSetSections::new(
            &source_sets,
            import_source_sets,
            separate_coverage_source_sets.as_ref(),
            hazards_source_sets,
            ranges,
            section_options,
            precomputed_duplication,
        )?;
        let test_gap_note = source_sets
            .production()
            .filter(|prod| prod.test_gap.is_none())
            .map(|_| {
                if !options.coverage_reports_named {
                    TEST_GAP_NO_REPORT_NOTE.to_owned()
                } else if options.per_language_section {
                    TEST_GAP_NO_LANGUAGE_COVERAGE_NOTE.to_owned()
                } else {
                    TEST_GAP_NO_REPO_COVERAGE_NOTE.to_owned()
                }
            });
        Ok((
            MeasuredSection {
                files,
                source_lines: lines,
                framework_profiles: sorted_unique_profiles(options.framework_profiles),
                reference_range_note: options.reference_range_note.map(str::to_owned),
                import_metrics_note,
                source_sets,
                test_gap_note,
                dependencies,
            },
            measured_duplication,
        ))
    }
}
