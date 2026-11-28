import '../errors.dart';
import '../json_parsing.dart';

const Set<String> _knownTrendRowKeys = <String>{
  'commit',
  'committed_on',
  'files',
  'erosion',
  'cognitive_complexity',
  'duplication',
  'verbosity',
  'import_cycles',
  'hazards',
};

typedef CommitManifestWarnings = ({String commit, List<String> warnings});
typedef ErodedChurnFile = ({
  String path,
  int commits,
  int erodedFunctions,
  double erodedFunctionMass,
  int maxCyclomaticComplexity,
});
typedef DuplicatedChurnFile = ({
  String path,
  int commits,
  int cloneLines,
  int cloneGroups,
});
typedef ErosionChurnSection = ({
  double ratio,
  double erodedFunctionMass,
  double totalFunctionMass,
  List<ErodedChurnFile> findings,
  int? findingsLeftOut,
});
typedef DuplicationChurnSection = ({
  double ratio,
  int cloneLines,
  List<DuplicatedChurnFile> findings,
  int? findingsLeftOut,
});
typedef SourceSetChurn = ({
  int files,
  ErosionChurnSection erosion,
  DuplicationChurnSection duplication,
});
typedef ChurnReport = ({
  String window,
  int commitsAnalyzed,
  bool shallow,
  String? warning,
  Map<String, SourceSetChurn> sourceSets,
});
typedef CoupledPair = ({
  String fileA,
  String fileB,
  int coChangeCount,
  int fileACommits,
  int fileBCommits,
  double couplingRatio,
  bool hasDirectImport,
});
typedef CouplingReport = ({
  String window,
  int commitsAnalyzed,
  int commitsExcludedByBulkLimit,
  int maxCommitFiles,
  int minCoChanges,
  double minRatio,
  List<String> sourceSets,
  bool shallow,
  String? warning,
  List<CoupledPair> pairs,
  int? pairsLeftOut,
});

class TrendReport extends JsonModel {
  const TrendReport.fromJson(super.json);

  int get commits => intOf('commits');
  Map<String, int> get metricVersions =>
      readOptionalIntMap(json, 'metric_versions');
  Map<String, List<TrendRow>> get sourceSets {
    final setsMap = requireJsonMap(json['source_sets'], 'source_sets');
    return setsMap.map(
      (key, _) =>
          MapEntry(key, readObjectList(setsMap, key, TrendRow.fromJson)),
    );
  }

  String? get testGapNote => optStr('test_gap_note');
  List<CommitManifestWarnings> get manifestWarnings => optList(
    'manifest_warnings',
    (m) => (
      commit: readString(m, 'commit'),
      warnings: readStringList(m, 'warnings'),
    ),
  );
}

class TrendRow extends JsonModel {
  const TrendRow.fromJson(super.json);

  String get commit => str('commit');
  String get committedOn => str('committed_on');
  int get files => intOf('files');
  double get erosion => dbl('erosion');
  double get cognitiveComplexity => dbl('cognitive_complexity');
  double get duplication => dbl('duplication');
  double get verbosity => dbl('verbosity');
  double? get importCycles => optDbl('import_cycles');
  double? get hazards => optDbl('hazards');

  Map<String, double> get externalMetrics {
    final external = <String, double>{};
    for (final entry in json.entries) {
      if (!_knownTrendRowKeys.contains(entry.key) && entry.value != null) {
        final raw = entry.value;
        if (raw is! num) {
          throw CodeintError('Expected number for trend metric "${entry.key}"');
        }
        external[entry.key] = raw.toDouble();
      }
    }
    return external;
  }
}

ChurnReport parseChurnReport(Map<String, Object?> j) => (
  window: readString(j, 'window'),
  commitsAnalyzed: readInt(j, 'commits_analyzed'),
  shallow: readBool(j, 'shallow'),
  warning: readOptionalString(j, 'warning'),
  sourceSets: readObjectMap(j, 'source_sets', _parseSourceSetChurn),
);

SourceSetChurn _parseSourceSetChurn(Map<String, Object?> j) {
  final er = requireJsonMap(j['erosion'], 'erosion');
  final dp = requireJsonMap(j['duplication'], 'duplication');
  return (
    files: readInt(j, 'files'),
    erosion: (
      ratio: readDouble(er, 'ratio'),
      erodedFunctionMass: readDouble(er, 'eroded_function_mass'),
      totalFunctionMass: readDouble(er, 'total_function_mass'),
      findingsLeftOut: readOptionalInt(er, 'findings_left_out'),
      findings: readObjectList(
        er,
        'findings',
        (m) => (
          path: readString(m, 'path'),
          commits: readInt(m, 'commits'),
          erodedFunctions: readInt(m, 'eroded_functions'),
          erodedFunctionMass: readDouble(m, 'eroded_function_mass'),
          maxCyclomaticComplexity: readInt(m, 'max_cyclomatic_complexity'),
        ),
      ),
    ),
    duplication: (
      ratio: readDouble(dp, 'ratio'),
      cloneLines: readInt(dp, 'clone_lines'),
      findingsLeftOut: readOptionalInt(dp, 'findings_left_out'),
      findings: readObjectList(
        dp,
        'findings',
        (m) => (
          path: readString(m, 'path'),
          commits: readInt(m, 'commits'),
          cloneLines: readInt(m, 'clone_lines'),
          cloneGroups: readInt(m, 'clone_groups'),
        ),
      ),
    ),
  );
}

CouplingReport parseCouplingReport(Map<String, Object?> j) => (
  window: readString(j, 'window'),
  commitsAnalyzed: readInt(j, 'commits_analyzed'),
  commitsExcludedByBulkLimit: readInt(j, 'commits_excluded_by_bulk_limit'),
  maxCommitFiles: readInt(j, 'max_commit_files'),
  minCoChanges: readInt(j, 'min_co_changes'),
  minRatio: readDouble(j, 'min_ratio'),
  sourceSets: readStringList(j, 'source_sets'),
  shallow: readBool(j, 'shallow'),
  warning: readOptionalString(j, 'warning'),
  pairsLeftOut: readOptionalInt(j, 'pairs_left_out'),
  pairs: readObjectList(
    j,
    'pairs',
    (m) => (
      fileA: readString(m, 'file_a'),
      fileB: readString(m, 'file_b'),
      coChangeCount: readInt(m, 'co_change_count'),
      fileACommits: readInt(m, 'file_a_commits'),
      fileBCommits: readInt(m, 'file_b_commits'),
      couplingRatio: readDouble(m, 'coupling_ratio'),
      hasDirectImport: readBool(m, 'has_direct_import'),
    ),
  ),
);
