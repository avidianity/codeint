import '../json_parsing.dart';

typedef BaselineInfo = ({String path, int hiddenFindings});
typedef SkippedFile = ({String path, String reason});
typedef FileDiagnostic = ({String path, int line, String kind, String message});
typedef ScopeSummary = ({
  String language,
  String? package,
  String? dialect,
  String kind,
  Map<String, String> fields,
});
typedef BudgetFailure = ({
  String? sourceSet,
  String metric,
  double ratio,
  double budget,
  bool? nonBaselined,
});
typedef NewFileBudgetFailure = ({
  String sourceSet,
  String metric,
  double ratio,
  double budget,
  int files,
});
typedef RatchetFailure = ({
  String? sourceSet,
  String metric,
  double ratio,
  double ceiling,
});
typedef RegressionEntry = ({
  String? sourceSet,
  String? file,
  String metric,
  String rule,
  double base,
  double head,
  int newFindings,
  double ratioRiseLimit,
});
typedef CloneLocation = ({String path, int startLine, int endLine});
typedef UnresolvedImportRow = ({
  String importer,
  String specifier,
  String reason,
  List<String> owners,
});
typedef UnresolvedImportsSection = ({
  int count,
  List<UnresolvedImportRow> imports,
  int? importsLeftOut,
});
typedef UnusedDependencyFinding = ({
  String id,
  String package,
  String manifest,
  String dependency,
  String kind,
  List<String> owners,
});
typedef ImportedDependencyFinding = ({
  String id,
  String package,
  String manifest,
  String dependency,
  List<String> importers,
  List<String> owners,
});
typedef ErodedFunctionFinding = ({
  String id,
  String path,
  int line,
  int endLine,
  String? name,
  int cyclomaticComplexity,
  int cognitiveComplexity,
  int sourceLines,
  double functionMass,
  List<String> owners,
});
typedef CognitiveComplexityFinding = ({
  String id,
  String path,
  int line,
  int endLine,
  String? name,
  int cognitiveComplexity,
  int cyclomaticComplexity,
  int sourceLines,
  List<String> owners,
});
typedef CloneGroupFinding = ({
  String id,
  int tokens,
  double? similarity,
  List<CloneLocation> locations,
  List<String> owners,
});
typedef CloneFamilyFinding = ({
  String id,
  int duplicatedLines,
  List<String> cloneGroupIds,
  List<CloneLocation> locations,
  List<String> owners,
});
typedef LineRuleFinding = ({
  String id,
  String path,
  int line,
  String rule,
  List<String> owners,
});
typedef FlaggedLineFinding = LineRuleFinding;
typedef HazardLineFinding = LineRuleFinding;
typedef ImportCycleFinding = ({
  String id,
  List<String> modules,
  List<String> owners,
});
typedef BoundaryFinding = ({
  String id,
  String kind,
  String importer,
  String? target,
  String? externalPackage,
  String? specifier,
  String? sourceZone,
  String? targetZone,
  String? reason,
  String message,
  List<String> owners,
});
typedef OrphanFileFinding = ({
  String id,
  String path,
  String kind,
  List<String> testImporters,
  String message,
  List<String> owners,
});
typedef TestGapFinding = ({
  String id,
  String path,
  int line,
  int endLine,
  String? name,
  double functionMass,
  double untestedShare,
  List<String> owners,
});
typedef ExternalMetricFinding = ({
  String id,
  String path,
  int line,
  int? endLine,
  String message,
  List<String> owners,
});

class ReferenceRange {
  const ReferenceRange({this.low, this.high, this.source, this.note});

  factory ReferenceRange.fromJson(Object? value) {
    if (value is String) {
      return ReferenceRange(note: value);
    }
    final map = requireJsonMap(value, 'reference_range');
    return ReferenceRange(
      low: readDouble(map, 'low'),
      high: readDouble(map, 'high'),
      source: readString(map, 'source'),
    );
  }

  final double? low;
  final double? high;
  final String? source;
  final String? note;

  bool get isRange => low != null && high != null && source != null;
}

BaselineInfo parseBaselineInfo(Map<String, Object?> j) => (
      path: readString(j, 'path'),
      hiddenFindings: readInt(j, 'hidden_findings'),
    );

SkippedFile parseSkippedFile(Map<String, Object?> j) => (
      path: readString(j, 'path'),
      reason: readString(j, 'reason'),
    );

FileDiagnostic parseFileDiagnostic(Map<String, Object?> j) => (
      path: readString(j, 'path'),
      line: readInt(j, 'line'),
      kind: readString(j, 'kind'),
      message: readString(j, 'message'),
    );

ScopeSummary parseScopeSummary(Map<String, Object?> j) => (
      language: readString(j, 'language'),
      package: readOptionalString(j, 'package'),
      dialect: readOptionalString(j, 'dialect'),
      kind: readString(j, 'kind'),
      fields: readOptionalStringMap(j, 'fields'),
    );

BudgetFailure parseBudgetFailure(Map<String, Object?> j) => (
      sourceSet: readOptionalString(j, 'source_set'),
      metric: readString(j, 'metric'),
      ratio: readDouble(j, 'ratio'),
      budget: readDouble(j, 'budget'),
      nonBaselined: readOptionalBool(j, 'non_baselined'),
    );

NewFileBudgetFailure parseNewFileBudgetFailure(Map<String, Object?> j) => (
      sourceSet: readString(j, 'source_set'),
      metric: readString(j, 'metric'),
      ratio: readDouble(j, 'ratio'),
      budget: readDouble(j, 'budget'),
      files: readInt(j, 'files'),
    );

RatchetFailure parseRatchetFailure(Map<String, Object?> j) => (
      sourceSet: readOptionalString(j, 'source_set'),
      metric: readString(j, 'metric'),
      ratio: readDouble(j, 'ratio'),
      ceiling: readDouble(j, 'ceiling'),
    );

RegressionEntry parseRegressionEntry(Map<String, Object?> j) => (
      sourceSet: readOptionalString(j, 'source_set'),
      file: readOptionalString(j, 'file'),
      metric: readString(j, 'metric'),
      rule: readString(j, 'rule'),
      base: readDouble(j, 'base'),
      head: readDouble(j, 'head'),
      newFindings: readInt(j, 'new_findings'),
      ratioRiseLimit: readDouble(j, 'ratio_rise_limit'),
    );

CloneLocation parseCloneLocation(Map<String, Object?> j) => (
      path: readString(j, 'path'),
      startLine: readInt(j, 'start_line'),
      endLine: readInt(j, 'end_line'),
    );

UnresolvedImportsSection parseUnresolvedImports(Map<String, Object?> j) => (
      count: readInt(j, 'count'),
      imports: readObjectList(
        j,
        'imports',
        (row) => (
          importer: readString(row, 'importer'),
          specifier: readString(row, 'specifier'),
          reason: readString(row, 'reason'),
          owners: readOptionalStringList(row, 'owners'),
        ),
      ),
      importsLeftOut: readOptionalInt(j, 'imports_left_out'),
    );

UnusedDependencyFinding parseUnusedDependencyFinding(Map<String, Object?> j) =>
    (
      id: readString(j, 'id'),
      package: readString(j, 'package'),
      manifest: readString(j, 'manifest'),
      dependency: readString(j, 'dependency'),
      kind: readString(j, 'kind'),
      owners: readOptionalStringList(j, 'owners'),
    );

ImportedDependencyFinding parseImportedDependencyFinding(
  Map<String, Object?> j,
) =>
    (
      id: readString(j, 'id'),
      package: readString(j, 'package'),
      manifest: readString(j, 'manifest'),
      dependency: readString(j, 'dependency'),
      importers: readStringList(j, 'importers'),
      owners: readOptionalStringList(j, 'owners'),
    );

ErodedFunctionFinding parseErodedFunctionFinding(Map<String, Object?> j) => (
      id: readString(j, 'id'),
      path: readString(j, 'path'),
      line: readInt(j, 'line'),
      endLine: readInt(j, 'end_line'),
      name: readOptionalString(j, 'name'),
      cyclomaticComplexity: readInt(j, 'cyclomatic_complexity'),
      cognitiveComplexity: readInt(j, 'cognitive_complexity'),
      sourceLines: readInt(j, 'source_lines'),
      functionMass: readDouble(j, 'function_mass'),
      owners: readOptionalStringList(j, 'owners'),
    );

CognitiveComplexityFinding parseCognitiveComplexityFinding(
  Map<String, Object?> j,
) =>
    (
      id: readString(j, 'id'),
      path: readString(j, 'path'),
      line: readInt(j, 'line'),
      endLine: readInt(j, 'end_line'),
      name: readOptionalString(j, 'name'),
      cognitiveComplexity: readInt(j, 'cognitive_complexity'),
      cyclomaticComplexity: readInt(j, 'cyclomatic_complexity'),
      sourceLines: readInt(j, 'source_lines'),
      owners: readOptionalStringList(j, 'owners'),
    );

CloneGroupFinding parseCloneGroupFinding(Map<String, Object?> j) => (
      id: readString(j, 'id'),
      tokens: readInt(j, 'tokens'),
      similarity: readOptionalDouble(j, 'similarity'),
      locations: readObjectList(j, 'locations', parseCloneLocation),
      owners: readOptionalStringList(j, 'owners'),
    );

CloneFamilyFinding parseCloneFamilyFinding(Map<String, Object?> j) => (
      id: readString(j, 'id'),
      duplicatedLines: readInt(j, 'duplicated_lines'),
      cloneGroupIds: readStringList(j, 'clone_group_ids'),
      locations: readObjectList(j, 'locations', parseCloneLocation),
      owners: readOptionalStringList(j, 'owners'),
    );

LineRuleFinding parseLineRuleFinding(Map<String, Object?> j) => (
      id: readString(j, 'id'),
      path: readString(j, 'path'),
      line: readInt(j, 'line'),
      rule: readString(j, 'rule'),
      owners: readOptionalStringList(j, 'owners'),
    );

ImportCycleFinding parseImportCycleFinding(Map<String, Object?> j) => (
      id: readString(j, 'id'),
      modules: readStringList(j, 'modules'),
      owners: readOptionalStringList(j, 'owners'),
    );

BoundaryFinding parseBoundaryFinding(Map<String, Object?> j) => (
      id: readString(j, 'id'),
      kind: readString(j, 'kind'),
      importer: readString(j, 'importer'),
      target: readOptionalString(j, 'target'),
      externalPackage: readOptionalString(j, 'external_package'),
      specifier: readOptionalString(j, 'specifier'),
      sourceZone: readOptionalString(j, 'source_zone'),
      targetZone: readOptionalString(j, 'target_zone'),
      reason: readOptionalString(j, 'reason'),
      message: readString(j, 'message'),
      owners: readOptionalStringList(j, 'owners'),
    );

OrphanFileFinding parseOrphanFileFinding(Map<String, Object?> j) => (
      id: readString(j, 'id'),
      path: readString(j, 'path'),
      kind: readString(j, 'kind'),
      testImporters: readOptionalStringList(j, 'test_importers'),
      message: readString(j, 'message'),
      owners: readOptionalStringList(j, 'owners'),
    );

TestGapFinding parseTestGapFinding(Map<String, Object?> j) => (
      id: readString(j, 'id'),
      path: readString(j, 'path'),
      line: readInt(j, 'line'),
      endLine: readInt(j, 'end_line'),
      name: readOptionalString(j, 'name'),
      functionMass: readDouble(j, 'function_mass'),
      untestedShare: readDouble(j, 'untested_share'),
      owners: readOptionalStringList(j, 'owners'),
    );

ExternalMetricFinding parseExternalMetricFinding(Map<String, Object?> j) => (
      id: readString(j, 'id'),
      path: readString(j, 'path'),
      line: readInt(j, 'line'),
      endLine: readOptionalInt(j, 'end_line'),
      message: readString(j, 'message'),
      owners: readOptionalStringList(j, 'owners'),
    );
