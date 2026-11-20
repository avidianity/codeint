#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use codeint_config::{Budgets, FindingsShown, ReportFormat};
use codeint_metrics::{
    CloneGroup, Duplication, ErodedFunction, Erosion, FlaggedLineFinding, Fragment, ImportCycle,
    ImportCycles, MinimumCloneTokens, UnresolvedImport, Verbosity,
};
use codeint_model::{
    CodeModel, Metric, Package, ReferenceRange, ReferenceRanges, RepoPath, SkippedFile, SourceSet,
    SourceSets,
};
use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

mod diff;
mod markdown;
mod sarif;
#[cfg(test)]
mod tests;

pub use diff::{BaseCommit, DiffReport, Head, HeadCommit};

const DECIMAL_SCALE: f64 = 10_000.0;
const NOT_PUBLISHED_FOR_THIS_LANGUAGE: &str = "no published Reference range for this language";
const NOT_SHOWN_FOR_SEVERAL_LANGUAGES: &str =
    "not shown, because the measured files are in more than one language";

#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    #[error("cannot render the report as TOON")]
    Toon(#[source] toon_format::ToonError),
    #[error("cannot render the report as JSON")]
    Json(#[source] serde_json::Error),
    #[error("cannot render the report as SARIF")]
    Sarif(#[source] serde_json::Error),
}

#[derive(Debug, Serialize)]
pub struct Report {
    #[serde(flatten)]
    measured: MeasuredSection,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    languages: BTreeMap<String, MeasuredSection>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    skipped: Vec<SkippedEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    declaration_only: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    submodules: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    over_budget: Vec<BudgetFailure>,
}

#[derive(Debug, Serialize)]
pub struct BudgetFailure {
    source_set: &'static str,
    metric: &'static str,
    ratio: Decimal,
    budget: f64,
}

pub struct MeasuredLanguage {
    pub plugin: String,
    pub model: CodeModel,
    pub reference_ranges: ReferenceRanges,
}

#[derive(Debug, Serialize)]
struct MeasuredSection {
    files: usize,
    source_lines: u64,
    source_sets: SourceSetSections,
}

enum ReferenceRangeScope {
    OneLanguage(ReferenceRanges),
    SeveralLanguages,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum ReferenceRangeEntry {
    Published {
        low: Decimal,
        high: Decimal,
        source: String,
    },
    NotShown(&'static str),
}

#[derive(Debug, Default, Serialize)]
struct SourceSetSections {
    #[serde(skip_serializing_if = "Option::is_none")]
    production: Option<SourceSetSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    test: Option<SourceSetSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generated: Option<SourceSetSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vendored: Option<SourceSetSection>,
}

#[derive(Debug, Serialize)]
struct SourceSetSection {
    files: usize,
    source_lines: u64,
    erosion: ErosionSection,
    duplication: DuplicationSection,
    verbosity: VerbositySection,
    import_cycles: ImportCyclesSection,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    packages: Vec<PackageSection>,
}

#[derive(Debug, Serialize)]
struct PackageSection {
    path: String,
    files: usize,
    source_lines: u64,
    erosion: PackageErosion,
    duplication: PackageDuplication,
    verbosity: PackageVerbosity,
    import_cycles: PackageImportCycles,
}

#[derive(Debug, Serialize)]
struct PackageErosion {
    ratio: Decimal,
    eroded_function_mass: Decimal,
    total_function_mass: Decimal,
}

#[derive(Debug, Serialize)]
struct PackageDuplication {
    ratio: Decimal,
    clone_lines: u64,
}

#[derive(Debug, Serialize)]
struct PackageVerbosity {
    ratio: Decimal,
    verbose_lines: u64,
}

#[derive(Debug, Serialize)]
struct PackageImportCycles {
    ratio: Decimal,
    modules_in_import_cycles: usize,
}

#[derive(Debug, Serialize)]
struct ErosionSection {
    ratio: Decimal,
    reference_range: ReferenceRangeEntry,
    eroded_function_mass: Decimal,
    total_function_mass: Decimal,
    #[serde(flatten)]
    findings: Findings<ErodedFunctionFinding>,
}

trait FindingKind: Serialize {
    const LIST_KEY: &'static str;
    const LEFT_OUT_KEY: &'static str = "findings_left_out";
}

#[derive(Debug)]
struct Findings<T> {
    listed: Vec<T>,
    left_out: usize,
}

#[derive(Debug, Serialize)]
struct ErodedFunctionFinding {
    id: String,
    path: String,
    line: u32,
    cyclomatic_complexity: u32,
    source_lines: u32,
    function_mass: Decimal,
}

#[derive(Debug, Serialize)]
struct DuplicationSection {
    ratio: Decimal,
    reference_range: ReferenceRangeEntry,
    clone_lines: u64,
    #[serde(flatten)]
    clone_groups: Findings<CloneGroupFinding>,
}

#[derive(Debug, Serialize)]
struct CloneGroupFinding {
    id: String,
    tokens: usize,
    locations: Vec<CloneLocation>,
}

#[derive(Debug, Serialize)]
struct CloneLocation {
    path: String,
    start_line: u32,
    end_line: u32,
}

#[derive(Debug, Serialize)]
struct VerbositySection {
    ratio: Decimal,
    reference_range: ReferenceRangeEntry,
    verbose_lines: u64,
    flagged_lines: u64,
    clone_lines: u64,
    #[serde(flatten)]
    findings: Findings<FlaggedLineRow>,
}

#[derive(Debug, Serialize)]
struct FlaggedLineRow {
    id: String,
    path: String,
    line: u32,
    rule: String,
}

#[derive(Debug, Serialize)]
struct ImportCyclesSection {
    ratio: Decimal,
    reference_range: ReferenceRangeEntry,
    modules_in_import_cycles: usize,
    modules: usize,
    #[serde(flatten)]
    findings: Findings<ImportCycleFinding>,
    unresolved_imports: UnresolvedImportsSection,
}

#[derive(Debug, Serialize)]
struct UnresolvedImportsSection {
    count: usize,
    #[serde(flatten)]
    imports: Findings<UnresolvedImportRow>,
}

#[derive(Debug, Serialize)]
struct UnresolvedImportRow {
    importer: String,
    specifier: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct ImportCycleFinding {
    id: String,
    modules: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(transparent)]
struct Decimal(f64);

#[derive(Debug, Serialize)]
struct SkippedEntry {
    path: String,
    reason: String,
}

impl std::fmt::Display for BudgetFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "the {} {} ratio {} is over its Budget {}",
            self.source_set, self.metric, self.ratio.0, self.budget
        )
    }
}

impl Decimal {
    fn rounded(value: f64) -> Decimal {
        let rounded = (value * DECIMAL_SCALE).round() / DECIMAL_SCALE;
        let without_negative_zero = if rounded == 0.0 { 0.0 } else { rounded };
        Decimal(without_negative_zero)
    }
}

impl ReferenceRangeScope {
    fn entry(&self, metric: &str) -> ReferenceRangeEntry {
        match self {
            ReferenceRangeScope::OneLanguage(ranges) => ranges.for_metric(metric).map_or(
                ReferenceRangeEntry::NotShown(NOT_PUBLISHED_FOR_THIS_LANGUAGE),
                ReferenceRangeEntry::from,
            ),
            ReferenceRangeScope::SeveralLanguages => {
                ReferenceRangeEntry::NotShown(NOT_SHOWN_FOR_SEVERAL_LANGUAGES)
            }
        }
    }
}

impl From<&ReferenceRange> for ReferenceRangeEntry {
    fn from(range: &ReferenceRange) -> ReferenceRangeEntry {
        ReferenceRangeEntry::Published {
            low: Decimal::rounded(range.low),
            high: Decimal::rounded(range.high),
            source: range.source.clone(),
        }
    }
}

impl<T> Findings<T> {
    fn worst_first(
        worst_first: impl ExactSizeIterator<Item = T>,
        shown: FindingsShown,
    ) -> Findings<T> {
        let total = worst_first.len();
        let listed: Vec<T> = match shown {
            FindingsShown::WorstPerMetric(limit) => worst_first.take(limit).collect(),
            FindingsShown::All => worst_first.collect(),
        };
        Findings {
            left_out: total - listed.len(),
            listed,
        }
    }
}

impl<T: FindingKind> Serialize for Findings<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry(T::LIST_KEY, &self.listed)?;
        if self.left_out > 0 {
            map.serialize_entry(T::LEFT_OUT_KEY, &self.left_out)?;
        }
        map.end()
    }
}

impl FindingKind for ErodedFunctionFinding {
    const LIST_KEY: &'static str = "findings";
}

impl FindingKind for CloneGroupFinding {
    const LIST_KEY: &'static str = "clone_groups";
}

impl FindingKind for FlaggedLineRow {
    const LIST_KEY: &'static str = "findings";
}

impl FindingKind for ImportCycleFinding {
    const LIST_KEY: &'static str = "findings";
}

impl FindingKind for UnresolvedImportRow {
    const LIST_KEY: &'static str = "imports";
    const LEFT_OUT_KEY: &'static str = "imports_left_out";
}

impl SourceSetSections {
    fn new(
        source_sets: &SourceSets,
        ranges: &ReferenceRangeScope,
        findings: FindingsShown,
    ) -> SourceSetSections {
        let mut sections = SourceSetSections::default();
        for (source_set, model) in source_sets.iter() {
            if !model.files.is_empty() {
                *sections.slot(source_set) = Some(SourceSetSection::new(model, ranges, findings));
            }
        }
        sections
    }

    fn iter(&self) -> impl Iterator<Item = (SourceSet, &SourceSetSection)> {
        [
            (SourceSet::Production, &self.production),
            (SourceSet::Test, &self.test),
            (SourceSet::Generated, &self.generated),
            (SourceSet::Vendored, &self.vendored),
        ]
        .into_iter()
        .filter_map(|(source_set, section)| Some((source_set, section.as_ref()?)))
    }

    fn over_budget(&self, budgets: &Budgets) -> Vec<BudgetFailure> {
        self.iter()
            .flat_map(|(source_set, section)| {
                [
                    (Metric::Erosion, &section.erosion.ratio),
                    (Metric::Duplication, &section.duplication.ratio),
                    (Metric::Verbosity, &section.verbosity.ratio),
                    (Metric::ImportCycles, &section.import_cycles.ratio),
                ]
                .into_iter()
                .filter_map(move |(metric, ratio)| {
                    let budget = budgets.budget(source_set, metric)?;
                    (!budget.admits(ratio.0)).then(|| BudgetFailure {
                        source_set: source_set.key(),
                        metric: metric.key(),
                        ratio: Decimal(ratio.0),
                        budget: budget.ratio(),
                    })
                })
            })
            .collect()
    }

    fn slot(&mut self, source_set: SourceSet) -> &mut Option<SourceSetSection> {
        match source_set {
            SourceSet::Production => &mut self.production,
            SourceSet::Test => &mut self.test,
            SourceSet::Generated => &mut self.generated,
            SourceSet::Vendored => &mut self.vendored,
        }
    }
}

impl SourceSetSection {
    fn new(
        model: &CodeModel,
        ranges: &ReferenceRangeScope,
        findings: FindingsShown,
    ) -> SourceSetSection {
        let measured = Measured::new(model);
        SourceSetSection {
            files: model.files.len(),
            source_lines: source_lines(model),
            erosion: ErosionSection::new(&measured.erosion, ranges, findings),
            duplication: DuplicationSection::new(&measured.duplication, ranges, findings),
            verbosity: VerbositySection::new(&measured.verbosity, ranges, findings),
            import_cycles: ImportCyclesSection::new(&measured.import_cycles, ranges, findings),
            packages: package_sections(model, &measured),
        }
    }
}

fn source_lines(model: &CodeModel) -> u64 {
    model
        .files
        .iter()
        .map(|file| u64::from(file.model.source_lines))
        .sum()
}

impl ErosionSection {
    fn new(
        erosion: &Erosion,
        ranges: &ReferenceRangeScope,
        shown: FindingsShown,
    ) -> ErosionSection {
        ErosionSection {
            ratio: Decimal::rounded(erosion.ratio()),
            reference_range: ranges.entry("erosion"),
            eroded_function_mass: Decimal::rounded(erosion.eroded_function_mass()),
            total_function_mass: Decimal::rounded(erosion.total_function_mass()),
            findings: Findings::worst_first(
                erosion
                    .eroded_functions()
                    .iter()
                    .map(ErodedFunctionFinding::new),
                shown,
            ),
        }
    }
}

impl ErodedFunctionFinding {
    fn new(function: &ErodedFunction) -> ErodedFunctionFinding {
        ErodedFunctionFinding {
            id: function.id.to_string(),
            path: function.path.to_string(),
            line: function.line,
            cyclomatic_complexity: function.cyclomatic_complexity,
            source_lines: function.source_lines,
            function_mass: Decimal::rounded(function.function_mass),
        }
    }
}

impl DuplicationSection {
    fn new(
        duplication: &Duplication,
        ranges: &ReferenceRangeScope,
        shown: FindingsShown,
    ) -> DuplicationSection {
        DuplicationSection {
            ratio: Decimal::rounded(duplication.ratio()),
            reference_range: ranges.entry("duplication"),
            clone_lines: duplication.clone_lines.count(),
            clone_groups: Findings::worst_first(
                duplication.clone_groups.iter().map(CloneGroupFinding::new),
                shown,
            ),
        }
    }
}

impl CloneGroupFinding {
    fn new(group: &CloneGroup) -> CloneGroupFinding {
        CloneGroupFinding {
            id: group.id.to_string(),
            tokens: group.tokens,
            locations: group.fragments.iter().map(CloneLocation::new).collect(),
        }
    }
}

impl CloneLocation {
    fn new(fragment: &Fragment) -> CloneLocation {
        CloneLocation {
            path: fragment.path.to_string(),
            start_line: fragment.start_line,
            end_line: fragment.end_line,
        }
    }
}

impl VerbositySection {
    fn new(
        verbosity: &Verbosity,
        ranges: &ReferenceRangeScope,
        shown: FindingsShown,
    ) -> VerbositySection {
        VerbositySection {
            ratio: Decimal::rounded(verbosity.ratio()),
            reference_range: ranges.entry("verbosity"),
            verbose_lines: verbosity.verbose_lines(),
            flagged_lines: verbosity.flagged_line_count(),
            clone_lines: verbosity.clone_line_count(),
            findings: Findings::worst_first(
                verbosity.flagged_lines().iter().map(FlaggedLineRow::from),
                shown,
            ),
        }
    }
}

impl From<&FlaggedLineFinding> for FlaggedLineRow {
    fn from(finding: &FlaggedLineFinding) -> FlaggedLineRow {
        FlaggedLineRow {
            id: finding.id.to_string(),
            path: finding.path.to_string(),
            line: finding.line,
            rule: finding.rule.clone(),
        }
    }
}

impl ImportCyclesSection {
    fn new(
        import_cycles: &ImportCycles,
        ranges: &ReferenceRangeScope,
        shown: FindingsShown,
    ) -> ImportCyclesSection {
        ImportCyclesSection {
            ratio: Decimal::rounded(import_cycles.ratio()),
            reference_range: ranges.entry("import_cycles"),
            modules_in_import_cycles: import_cycles.modules_in_import_cycles(),
            modules: import_cycles.modules(),
            findings: Findings::worst_first(
                import_cycles.cycles().iter().map(ImportCycleFinding::new),
                shown,
            ),
            unresolved_imports: UnresolvedImportsSection::new(import_cycles, shown),
        }
    }
}

impl ImportCycleFinding {
    fn new(cycle: &ImportCycle) -> ImportCycleFinding {
        ImportCycleFinding {
            id: cycle.id().to_string(),
            modules: cycle.modules().iter().map(RepoPath::to_string).collect(),
        }
    }
}

impl UnresolvedImportsSection {
    fn new(import_cycles: &ImportCycles, shown: FindingsShown) -> UnresolvedImportsSection {
        let unresolved = import_cycles.unresolved_imports();
        UnresolvedImportsSection {
            count: unresolved.len(),
            imports: Findings::worst_first(unresolved.iter().map(UnresolvedImportRow::from), shown),
        }
    }
}

impl From<&UnresolvedImport> for UnresolvedImportRow {
    fn from(import: &UnresolvedImport) -> UnresolvedImportRow {
        UnresolvedImportRow {
            importer: import.importer.to_string(),
            specifier: import.specifier.clone(),
            reason: import.reason.clone(),
        }
    }
}

impl From<&SkippedFile> for SkippedEntry {
    fn from(file: &SkippedFile) -> SkippedEntry {
        SkippedEntry {
            path: file.path.to_string(),
            reason: file.reason.clone(),
        }
    }
}

struct Measured {
    erosion: Erosion,
    duplication: Duplication,
    verbosity: Verbosity,
    import_cycles: ImportCycles,
}

impl Measured {
    fn new(model: &CodeModel) -> Measured {
        let duplication = Duplication::measure(model, MinimumCloneTokens::PROVISIONAL_DEFAULT);
        Measured {
            erosion: Erosion::measure(model),
            verbosity: Verbosity::measure(model, &duplication.clone_lines),
            duplication,
            import_cycles: ImportCycles::measure(model),
        }
    }
}

fn package_sections(model: &CodeModel, measured: &Measured) -> Vec<PackageSection> {
    if !model.workspace.has_members() {
        return Vec::new();
    }
    let mut files: BTreeMap<Package<'_>, usize> = BTreeMap::new();
    for file in &model.files {
        *files
            .entry(model.workspace.package_of(&file.path))
            .or_default() += 1;
    }
    let mut duplication = measured.duplication.by_package(model);
    let mut verbosity = measured.verbosity.by_package(model);
    let mut import_cycles = measured.import_cycles.by_package(model);
    measured
        .erosion
        .by_package(model)
        .into_iter()
        .map(|(package, masses)| {
            let clone_lines = duplication.remove(&package).unwrap_or_default();
            let verbose_lines = verbosity.remove(&package).unwrap_or_default();
            let modules = import_cycles.remove(&package).unwrap_or_default();
            PackageSection {
                path: package.to_string(),
                files: files.get(&package).copied().unwrap_or_default(),
                source_lines: clone_lines.source_lines,
                erosion: PackageErosion {
                    ratio: Decimal::rounded(masses.ratio()),
                    eroded_function_mass: Decimal::rounded(masses.eroded),
                    total_function_mass: Decimal::rounded(masses.total),
                },
                duplication: PackageDuplication {
                    ratio: Decimal::rounded(clone_lines.ratio()),
                    clone_lines: clone_lines.lines,
                },
                verbosity: PackageVerbosity {
                    ratio: Decimal::rounded(verbose_lines.ratio()),
                    verbose_lines: verbose_lines.lines,
                },
                import_cycles: PackageImportCycles {
                    ratio: Decimal::rounded(modules.ratio()),
                    modules_in_import_cycles: modules.modules_in_import_cycles,
                },
            }
        })
        .collect()
}

impl MeasuredSection {
    fn new(
        model: CodeModel,
        ranges: &ReferenceRangeScope,
        findings: FindingsShown,
    ) -> MeasuredSection {
        MeasuredSection {
            files: model.files.len(),
            source_lines: source_lines(&model),
            source_sets: SourceSetSections::new(&model.into_source_sets(), ranges, findings),
        }
    }
}

impl Report {
    pub fn summarize(
        languages: Vec<MeasuredLanguage>,
        submodules: Vec<RepoPath>,
        findings: FindingsShown,
        budgets: &Budgets,
    ) -> Report {
        let (measured, unmeasured): (Vec<MeasuredLanguage>, Vec<MeasuredLanguage>) = languages
            .into_iter()
            .partition(|language| !language.model.files.is_empty());
        let (total_ranges, sections_per_language) = match measured.as_slice() {
            [] => (
                ReferenceRangeScope::OneLanguage(ReferenceRanges::default()),
                BTreeMap::new(),
            ),
            [only] => (
                ReferenceRangeScope::OneLanguage(only.reference_ranges.clone()),
                BTreeMap::new(),
            ),
            _ => (
                ReferenceRangeScope::SeveralLanguages,
                measured
                    .iter()
                    .map(|language| {
                        (
                            language.plugin.clone(),
                            MeasuredSection::new(
                                language.model.clone(),
                                &ReferenceRangeScope::OneLanguage(
                                    language.reference_ranges.clone(),
                                ),
                                findings,
                            ),
                        )
                    })
                    .collect(),
            ),
        };
        let model = CodeModel::merge(
            measured
                .into_iter()
                .chain(unmeasured)
                .map(|language| language.model),
        );
        let skipped = model.skipped.iter().map(SkippedEntry::from).collect();
        let declaration_only = model
            .declaration_only
            .iter()
            .map(RepoPath::to_string)
            .collect();
        let measured = MeasuredSection::new(model, &total_ranges, findings);
        Report {
            over_budget: measured.source_sets.over_budget(budgets),
            skipped,
            declaration_only,
            submodules: submodules.iter().map(RepoPath::to_string).collect(),
            measured,
            languages: sections_per_language,
        }
    }

    pub fn over_budget(&self) -> &[BudgetFailure] {
        &self.over_budget
    }

    pub fn render(&self, format: ReportFormat) -> Result<String, ReportError> {
        match format {
            ReportFormat::Toon => toon_format::encode_default(self).map_err(ReportError::Toon),
            ReportFormat::Json => serde_json::to_string_pretty(self).map_err(ReportError::Json),
            ReportFormat::Sarif => serde_json::to_string_pretty(&sarif::SarifLog::new(self))
                .map_err(ReportError::Sarif),
            ReportFormat::Markdown => Ok(markdown::render(self)),
        }
    }
}
