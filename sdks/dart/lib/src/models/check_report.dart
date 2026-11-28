import '../json_parsing.dart';
import 'common_models.dart';

const Set<String> _knownSourceSetKeys = <String>{
  'files',
  'source_lines',
  'erosion',
  'cognitive_complexity',
  'duplication',
  'verbosity',
  'import_cycles',
  'boundaries',
  'orphan_files',
  'test_gap',
  'hazards',
  'packages',
};

typedef NearClonesSummary = ({double minSimilarity});
typedef PackageErosion = ({
  double ratio,
  double erodedFunctionMass,
  double totalFunctionMass,
});
typedef PackageCognitiveComplexity = ({
  double ratio,
  int complexCognitiveComplexity,
  int totalCognitiveComplexity,
});
typedef PackageDuplication = ({double ratio, int cloneLines});
typedef PackageVerbosity = ({double ratio, int verboseLines});
typedef PackageImportCycles = ({double ratio, int modulesInImportCycles});
typedef PackageTestGap = ({
  double ratio,
  double untestedFunctionMass,
  double totalFunctionMass,
});
typedef PackageHazards = ({double ratio, int hazardLines, int sourceLines});

class MetricSection<F> extends JsonModel {
  const MetricSection.fromJson(super.json, this._parseFinding);

  final F Function(Map<String, Object?>) _parseFinding;

  double get ratio => dbl('ratio');
  int get metricVersion => intOf('metric_version');
  ReferenceRange get referenceRange =>
      ReferenceRange.fromJson(json['reference_range']);
  int? get findingsLeftOut => optInt('findings_left_out');
  int? get baselinedFindings => optInt('baselined_findings');
  List<F> get findings => list('findings', _parseFinding);
}

typedef ErosionSection = MetricSection<ErodedFunctionFinding>;
typedef CognitiveComplexitySection = MetricSection<CognitiveComplexityFinding>;
typedef VerbositySection = MetricSection<FlaggedLineFinding>;
typedef ImportCyclesSection = MetricSection<ImportCycleFinding>;
typedef BoundariesSection = MetricSection<BoundaryFinding>;
typedef OrphanFilesSection = MetricSection<OrphanFileFinding>;
typedef TestGapSection = MetricSection<TestGapFinding>;
typedef HazardsSection = MetricSection<HazardLineFinding>;
typedef ExternalMetricSummary = MetricSection<ExternalMetricFinding>;
typedef UnusedDependenciesSection = MetricSection<UnusedDependencyFinding>;
typedef UnlistedDependenciesSection = MetricSection<ImportedDependencyFinding>;
typedef DevInProductionDependenciesSection
    = MetricSection<ImportedDependencyFinding>;

extension ErosionSectionFields on ErosionSection {
  double get erodedFunctionMass => dbl('eroded_function_mass');
  double get totalFunctionMass => dbl('total_function_mass');
}

extension CognitiveComplexitySectionFields on CognitiveComplexitySection {
  int get complexCognitiveComplexity => intOf('complex_cognitive_complexity');
  int get totalCognitiveComplexity => intOf('total_cognitive_complexity');
}

extension LineRuleSectionFields on MetricSection<LineRuleFinding> {
  int get verboseLines => intOf('verbose_lines');
  int get flaggedLines => intOf('flagged_lines');
  int get hazardLines => intOf('hazard_lines');
  int get sourceLines => intOf('source_lines');
}

extension ImportCyclesSectionFields on ImportCyclesSection {
  int get modulesInImportCycles => intOf('modules_in_import_cycles');
  int get modules => intOf('modules');
  UnresolvedImportsSection get unresolvedImports =>
      obj('unresolved_imports', parseUnresolvedImports);
}

extension BoundariesSectionFields on BoundariesSection {
  int get resolvedImports => intOf('resolved_imports');
}

extension OrphanFilesSectionFields on OrphanFilesSection {
  int get orphanFiles => intOf('orphan_files');
  int get evaluatedFiles => intOf('evaluated_files');
  int get entryPoints => intOf('entry_points');
  int get unreferenced => intOf('unreferenced');
  int get testOnly => intOf('test_only');
}

extension TestGapSectionFields on TestGapSection {
  double get untestedFunctionMass => dbl('untested_function_mass');
  double get totalFunctionMass => dbl('total_function_mass');
  int get measuredFunctions => intOf('measured_functions');
}

extension ExternalMetricSummaryFields on ExternalMetricSummary {
  double get numerator => dbl('numerator');
  double get denominator => dbl('denominator');
}

extension UnusedDependenciesSectionFields on UnusedDependenciesSection {
  int get unused => intOf('unused');
  int get declared => intOf('declared');
}

extension ImportedDependenciesSectionFields
    on MetricSection<ImportedDependencyFinding> {
  int get unlisted => intOf('unlisted');
  int get imported => intOf('imported');
  int get devInProduction => intOf('dev_in_production');
  int get productionImported => intOf('production_imported');
}

class DuplicationSection extends JsonModel {
  const DuplicationSection.fromJson(super.json);

  double get ratio => dbl('ratio');
  int get metricVersion => intOf('metric_version');
  ReferenceRange get referenceRange =>
      ReferenceRange.fromJson(json['reference_range']);
  NearClonesSummary? get nearClones => optObj(
        'near_clones',
        (m) => (minSimilarity: readDouble(m, 'min_similarity')),
      );
  int get cloneLines => intOf('clone_lines');
  List<CloneGroupFinding> get cloneGroups =>
      list('clone_groups', parseCloneGroupFinding);
  int? get findingsLeftOut => optInt('findings_left_out');
  int? get baselinedFindings => optInt('baselined_findings');
  List<CloneFamilyFinding> get cloneFamilies =>
      optList('clone_families', parseCloneFamilyFinding);
  int? get cloneFamiliesLeftOut => optInt('clone_families_left_out');
  List<String> get unlistedCloneFamilyGroupIds =>
      optStrs('unlisted_clone_family_group_ids');
}

class DependenciesSection extends JsonModel {
  const DependenciesSection.fromJson(super.json);

  UnusedDependenciesSection get unusedDependencies => MetricSection.fromJson(
        requireJsonMap(json['unused_dependencies'], 'unused_dependencies'),
        parseUnusedDependencyFinding,
      );
  UnlistedDependenciesSection get unlistedDependencies =>
      MetricSection.fromJson(
        requireJsonMap(json['unlisted_dependencies'], 'unlisted_dependencies'),
        parseImportedDependencyFinding,
      );
  DevInProductionDependenciesSection get devInProductionDependencies =>
      MetricSection.fromJson(
        requireJsonMap(
          json['dev_in_production_dependencies'],
          'dev_in_production_dependencies',
        ),
        parseImportedDependencyFinding,
      );
}

class PackageSection extends JsonModel {
  const PackageSection.fromJson(super.json);

  String get path => str('path');
  int get files => intOf('files');
  int get sourceLines => intOf('source_lines');
  List<String> get frameworkProfiles => optStrs('framework_profiles');
  PackageErosion get erosion => obj(
        'erosion',
        (m) => (
          ratio: readDouble(m, 'ratio'),
          erodedFunctionMass: readDouble(m, 'eroded_function_mass'),
          totalFunctionMass: readDouble(m, 'total_function_mass'),
        ),
      );
  PackageCognitiveComplexity get cognitiveComplexity => obj(
        'cognitive_complexity',
        (m) => (
          ratio: readDouble(m, 'ratio'),
          complexCognitiveComplexity: readInt(m, 'complex_cognitive_complexity'),
          totalCognitiveComplexity: readInt(m, 'total_cognitive_complexity'),
        ),
      );
  PackageDuplication get duplication => obj(
        'duplication',
        (m) => (
          ratio: readDouble(m, 'ratio'),
          cloneLines: readInt(m, 'clone_lines'),
        ),
      );
  PackageVerbosity get verbosity => obj(
        'verbosity',
        (m) => (
          ratio: readDouble(m, 'ratio'),
          verboseLines: readInt(m, 'verbose_lines'),
        ),
      );
  PackageImportCycles? get importCycles => optObj(
        'import_cycles',
        (m) => (
          ratio: readDouble(m, 'ratio'),
          modulesInImportCycles: readInt(m, 'modules_in_import_cycles'),
        ),
      );
  PackageTestGap? get testGap => optObj(
        'test_gap',
        (m) => (
          ratio: readDouble(m, 'ratio'),
          untestedFunctionMass: readDouble(m, 'untested_function_mass'),
          totalFunctionMass: readDouble(m, 'total_function_mass'),
        ),
      );
  PackageHazards? get hazards => optObj(
        'hazards',
        (m) => (
          ratio: readDouble(m, 'ratio'),
          hazardLines: readInt(m, 'hazard_lines'),
          sourceLines: readInt(m, 'source_lines'),
        ),
      );
}

class SourceSetReport extends JsonModel {
  const SourceSetReport.fromJson(super.json);

  int get files => intOf('files');
  int get sourceLines => intOf('source_lines');
  ErosionSection get erosion =>
      _required('erosion', parseErodedFunctionFinding);
  CognitiveComplexitySection get cognitiveComplexity =>
      _required('cognitive_complexity', parseCognitiveComplexityFinding);
  DuplicationSection get duplication =>
      obj('duplication', DuplicationSection.fromJson);
  VerbositySection get verbosity =>
      _required('verbosity', parseLineRuleFinding);
  ImportCyclesSection? get importCycles =>
      _optional('import_cycles', parseImportCycleFinding);
  BoundariesSection? get boundaries =>
      _optional('boundaries', parseBoundaryFinding);
  OrphanFilesSection? get orphanFiles =>
      _optional('orphan_files', parseOrphanFileFinding);
  TestGapSection? get testGap => _optional('test_gap', parseTestGapFinding);
  HazardsSection? get hazards => _optional('hazards', parseLineRuleFinding);
  List<PackageSection> get packages =>
      optList('packages', PackageSection.fromJson);

  Map<String, ExternalMetricSummary> get externalMetrics => readUnknownEntries(
        json,
        _knownSourceSetKeys,
        (m) => MetricSection.fromJson(m, parseExternalMetricFinding),
      );

  MetricSection<F> _required<F>(
    String key,
    F Function(Map<String, Object?>) parse,
  ) =>
      MetricSection.fromJson(requireJsonMap(json[key], key), parse);

  MetricSection<F>? _optional<F>(
    String key,
    F Function(Map<String, Object?>) parse,
  ) =>
      optObj(key, (m) => MetricSection.fromJson(m, parse));
}

class MeasuredSection extends JsonModel {
  const MeasuredSection.fromJson(super.json);

  int get files => intOf('files');
  int get sourceLines => intOf('source_lines');
  List<String> get frameworkProfiles => optStrs('framework_profiles');
  String? get referenceRangeNote => optStr('reference_range_note');
  String? get importMetricsNote => optStr('import_metrics_note');
  Map<String, SourceSetReport> get sourceSets =>
      readObjectMap(json, 'source_sets', SourceSetReport.fromJson);
  String? get testGapNote => optStr('test_gap_note');
  DependenciesSection? get dependencies =>
      optObj('dependencies', DependenciesSection.fromJson);
}

class CheckReport extends MeasuredSection {
  const CheckReport.fromJson(super.json) : super.fromJson();

  String? get coverageUnmatchedNote => optStr('coverage_unmatched_note');
  String? get ratchetNote => optStr('ratchet_note');
  List<String> get manifestWarnings => optStrs('manifest_warnings');
  BaselineInfo? get baseline => optObj('baseline', parseBaselineInfo);
  Map<String, MeasuredSection> get languages =>
      readOptionalObjectMap(json, 'languages', MeasuredSection.fromJson);
  List<SkippedFile> get skipped => optList('skipped', parseSkippedFile);
  List<FileDiagnostic> get fileDiagnostics =>
      optList('file_diagnostics', parseFileDiagnostic);
  List<ScopeSummary> get scopeSummaries =>
      optList('scope_summaries', parseScopeSummary);
  List<String> get declarationOnly => optStrs('declaration_only');
  List<String> get submodules => optStrs('submodules');
  List<BudgetFailure> get overBudget =>
      optList('over_budget', parseBudgetFailure);
  List<RatchetFailure> get overRatchet =>
      optList('over_ratchet', parseRatchetFailure);
}
