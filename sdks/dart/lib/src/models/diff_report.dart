import '../json_parsing.dart';
import 'common_models.dart';

const Set<String> _knownDiffMetricKeys = <String>{
  'erosion',
  'cognitive_complexity',
  'duplication',
  'verbosity',
  'import_cycles',
  'boundaries',
  'orphan_files',
  'hazards',
};

typedef DiffBase = ({String ref, String mergeBase});
typedef DiffManifestWarnings = ({List<String> base, List<String> head});

class DiffHead {
  const DiffHead({this.workingTreeLabel, this.ref, this.commit});

  factory DiffHead.fromJson(Object? value) {
    if (value is String) {
      return DiffHead(workingTreeLabel: value);
    }
    final map = requireJsonMap(value, 'head');
    return DiffHead(
      ref: readString(map, 'ref'),
      commit: readString(map, 'commit'),
    );
  }

  final String? workingTreeLabel;
  final String? ref;
  final String? commit;

  bool get isWorkingTree => workingTreeLabel != null;
}

class MetricChanges<T> extends JsonModel {
  const MetricChanges.fromJson(super.json, this._parseFinding);

  final T Function(Map<String, Object?>) _parseFinding;

  double get baseRatio => dbl('base_ratio');
  double get headRatio => dbl('head_ratio');
  int get metricVersion => intOf('metric_version');
  List<T> get added => list('new', _parseFinding);
  int? get newLeftOut => optInt('new_left_out');
  List<T> get resolved => list('resolved', _parseFinding);
  int? get resolvedLeftOut => optInt('resolved_left_out');
  int get unchanged => intOf('unchanged');
}

class CloneFamilyChanges extends JsonModel {
  const CloneFamilyChanges.fromJson(super.json);

  List<CloneFamilyFinding> get added => list('new', parseCloneFamilyFinding);
  int? get newLeftOut => optInt('new_left_out');
  List<CloneFamilyFinding> get resolved =>
      list('resolved', parseCloneFamilyFinding);
  int? get resolvedLeftOut => optInt('resolved_left_out');
  int get unchanged => intOf('unchanged');
  List<String> get unlistedCloneFamilyGroupIds =>
      optStrs('unlisted_clone_family_group_ids');
}

class DuplicationChanges extends MetricChanges<CloneGroupFinding> {
  const DuplicationChanges.fromJson(Map<String, Object?> json)
      : super.fromJson(json, parseCloneGroupFinding);

  CloneFamilyChanges? get cloneFamilies =>
      optObj('clone_families', CloneFamilyChanges.fromJson);
}

class SourceSetChanges extends JsonModel {
  const SourceSetChanges.fromJson(super.json);

  MetricChanges<ErodedFunctionFinding> get erosion =>
      _required('erosion', parseErodedFunctionFinding);
  MetricChanges<CognitiveComplexityFinding> get cognitiveComplexity =>
      _required('cognitive_complexity', parseCognitiveComplexityFinding);
  DuplicationChanges get duplication =>
      obj('duplication', DuplicationChanges.fromJson);
  MetricChanges<FlaggedLineFinding> get verbosity =>
      _required('verbosity', parseLineRuleFinding);
  MetricChanges<ImportCycleFinding>? get importCycles =>
      _optional('import_cycles', parseImportCycleFinding);
  MetricChanges<BoundaryFinding>? get boundaries =>
      _optional('boundaries', parseBoundaryFinding);
  MetricChanges<OrphanFileFinding>? get orphanFiles =>
      _optional('orphan_files', parseOrphanFileFinding);
  MetricChanges<HazardLineFinding>? get hazards =>
      _optional('hazards', parseLineRuleFinding);

  Map<String, MetricChanges<ExternalMetricFinding>> get externalMetrics =>
      readUnknownEntries(
        json,
        _knownDiffMetricKeys,
        (m) => MetricChanges.fromJson(m, parseExternalMetricFinding),
      );

  MetricChanges<T> _required<T>(
    String key,
    T Function(Map<String, Object?>) parse,
  ) =>
      obj(key, (m) => MetricChanges.fromJson(m, parse));

  MetricChanges<T>? _optional<T>(
    String key,
    T Function(Map<String, Object?>) parse,
  ) =>
      optObj(key, (m) => MetricChanges.fromJson(m, parse));
}

class DependenciesChanges extends JsonModel {
  const DependenciesChanges.fromJson(super.json);

  MetricChanges<UnusedDependencyFinding> get unusedDependencies =>
      MetricChanges.fromJson(
        requireJsonMap(json['unused_dependencies'], 'unused_dependencies'),
        parseUnusedDependencyFinding,
      );
  MetricChanges<ImportedDependencyFinding> get unlistedDependencies =>
      MetricChanges.fromJson(
        requireJsonMap(json['unlisted_dependencies'], 'unlisted_dependencies'),
        parseImportedDependencyFinding,
      );
  MetricChanges<ImportedDependencyFinding> get devInProductionDependencies =>
      MetricChanges.fromJson(
        requireJsonMap(
          json['dev_in_production_dependencies'],
          'dev_in_production_dependencies',
        ),
        parseImportedDependencyFinding,
      );
}

class DiffReport extends JsonModel {
  const DiffReport.fromJson(super.json);

  DiffBase get base => obj(
        'base',
        (m) => (
          ref: readString(m, 'ref'),
          mergeBase: readString(m, 'merge_base'),
        ),
      );
  DiffHead get head => DiffHead.fromJson(json['head']);
  String? get testGapNote => optStr('test_gap_note');
  String? get ratchetNote => optStr('ratchet_note');
  DiffManifestWarnings? get manifestWarnings => optObj(
        'manifest_warnings',
        (m) => (
          base: readOptionalStringList(m, 'base'),
          head: readOptionalStringList(m, 'head'),
        ),
      );
  Map<String, SourceSetChanges> get sourceSets =>
      readObjectMap(json, 'source_sets', SourceSetChanges.fromJson);
  DependenciesChanges? get dependencies =>
      optObj('dependencies', DependenciesChanges.fromJson);
  List<BudgetFailure> get overBudget =>
      optList('over_budget', parseBudgetFailure);
  List<NewFileBudgetFailure> get overNewFileBudget =>
      optList('over_new_file_budget', parseNewFileBudgetFailure);
  List<RatchetFailure> get overRatchet =>
      optList('over_ratchet', parseRatchetFailure);
  List<RegressionEntry> get regressions =>
      optList('regressions', parseRegressionEntry);
}
