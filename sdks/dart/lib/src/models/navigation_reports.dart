import '../errors.dart';
import '../json_parsing.dart';

typedef ChangedBase = ({String ref, String mergeBase});
typedef ImpactTarget = ({
  String path,
  String sourceSet,
  String? package,
  int fanIn,
  int fanOut,
  int transitiveImportWeight,
});
typedef ImpactSummary = ({
  int directDependents,
  int transitiveDependents,
  int totalDependents,
  int fanIn,
  int fanOut,
  int transitiveImportWeight,
});
typedef ImpactDependent = ({String path, int distance, String? package});
typedef SourceSetImpact = ({
  List<ImpactDependent> direct,
  List<ImpactDependent> transitive,
});
typedef TraceMultiFileImport = ({
  String importer,
  String specifier,
  List<String> targets,
});
typedef GuardAllowedTarget = ({
  String zone,
  List<String> publicFiles,
  String? reason,
});
typedef GuardDeniedTarget = ({String zone, String directive, String? reason});
typedef GuardExternalImports = ({
  String directive,
  List<String> patterns,
  String? reason,
});

SourceSetImpact _parseSourceSetImpact(Map<String, Object?> m) {
  ImpactDependent parseDep(Map<String, Object?> row) => (
        path: readString(row, 'path'),
        distance: readInt(row, 'distance'),
        package: readOptionalString(row, 'package'),
      );
  return (
    direct: readObjectList(m, 'direct', parseDep),
    transitive: readObjectList(m, 'transitive', parseDep),
  );
}

class ImpactReport extends JsonModel {
  const ImpactReport.fromJson(super.json);

  List<ImpactTarget> get targets => list(
        'targets',
        (m) => (
          path: readString(m, 'path'),
          sourceSet: readString(m, 'source_set'),
          package: readOptionalString(m, 'package'),
          fanIn: readInt(m, 'fan_in'),
          fanOut: readInt(m, 'fan_out'),
          transitiveImportWeight: readInt(m, 'transitive_import_weight'),
        ),
      );
  ChangedBase? get changedBase => optObj(
        'changed_base',
        (m) => (ref: readString(m, 'ref'), mergeBase: readString(m, 'merge_base')),
      );
  int? get depthLimit => optInt('depth_limit');
  ImpactSummary get summary => obj(
        'summary',
        (m) => (
          directDependents: readInt(m, 'direct_dependents'),
          transitiveDependents: readInt(m, 'transitive_dependents'),
          totalDependents: readInt(m, 'total_dependents'),
          fanIn: readInt(m, 'fan_in'),
          fanOut: readInt(m, 'fan_out'),
          transitiveImportWeight: readInt(m, 'transitive_import_weight'),
        ),
      );
  SourceSetImpact get production => obj('production', _parseSourceSetImpact);
  SourceSetImpact get test => obj('test', _parseSourceSetImpact);
  SourceSetImpact? get generated => optObj('generated', _parseSourceSetImpact);
  SourceSetImpact? get vendored => optObj('vendored', _parseSourceSetImpact);
  List<String> get affectedPackages => optStrs('affected_packages');
}

class TraceReport extends JsonModel {
  const TraceReport.fromJson(super.json);

  String get from => str('from');
  String get to => str('to');
  String? get fromEntryPoint => optStr('from_entry_point');
  String? get toEntryPoint => optStr('to_entry_point');
  bool get connected => flag('connected');
  int? get shortestDistance => optInt('shortest_distance');
  List<List<String>> get paths {
    final rawPaths = readTyped<List<Object?>>(json, 'paths', 'path list');
    return rawPaths.map((entry) {
      if (entry is! List) {
        throw const CodeintError('Expected string path list in trace report');
      }
      return entry.map((step) => '$step').toList(growable: false);
    }).toList(growable: false);
  }

  List<TraceMultiFileImport> get multiFileImports => optList(
        'multi_file_imports',
        (m) => (
          importer: readString(m, 'importer'),
          specifier: readString(m, 'specifier'),
          targets: readStringList(m, 'targets'),
        ),
      );
  bool get truncated => flag('truncated');
}

class GuardReport extends JsonModel {
  const GuardReport.fromJson(super.json);

  String get path => str('path');
  bool get exists => flag('exists');
  bool get configured => flag('configured');
  bool get strictZones => flag('strict_zones');
  String? get zone => optStr('zone');
  bool? get isPublicFile => optFlag('is_public_file');
  List<String> get publicFiles => optStrs('public_files');
  List<GuardAllowedTarget> get allowedTargets => list(
        'allowed_targets',
        (m) => (
          zone: readString(m, 'zone'),
          publicFiles: readOptionalStringList(m, 'public_files'),
          reason: readOptionalString(m, 'reason'),
        ),
      );
  List<GuardDeniedTarget> get deniedTargets => list(
        'denied_targets',
        (m) => (
          zone: readString(m, 'zone'),
          directive: readString(m, 'directive'),
          reason: readOptionalString(m, 'reason'),
        ),
      );
  List<String> get allowedImporters => strs('allowed_importers');
  GuardExternalImports? get externalImports => optObj(
        'external_imports',
        (m) => (
          directive: readString(m, 'directive'),
          patterns: readStringList(m, 'patterns'),
          reason: readOptionalString(m, 'reason'),
        ),
      );
  String? get note => optStr('note');
}
