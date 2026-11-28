import 'enums.dart';

const String codeintVersion = '0.8.0';

class BinaryResolverOptions {
  const BinaryResolverOptions({
    this.binaryPath,
    this.allowDownload = true,
    this.releaseBaseUrl,
    this.cacheDir,
    this.expectedVersion = codeintVersion,
    this.environment,
  });

  final String? binaryPath;
  final bool allowDownload;
  final String? releaseBaseUrl;
  final String? cacheDir;
  final String expectedVersion;
  final Map<String, String>? environment;
}

class CommonOptions {
  const CommonOptions({
    this.cwd,
    this.binaryPath,
    this.allowDownload = true,
    this.releaseBaseUrl,
    this.cacheDir,
    this.environment,
  });

  final String? cwd;
  final String? binaryPath;
  final bool allowDownload;
  final String? releaseBaseUrl;
  final String? cacheDir;
  final Map<String, String>? environment;

  BinaryResolverOptions toResolverOptions() => BinaryResolverOptions(
        binaryPath: binaryPath,
        allowDownload: allowDownload,
        releaseBaseUrl: releaseBaseUrl,
        cacheDir: cacheDir,
        environment: environment,
      );
}

class CheckOptions {
  const CheckOptions({
    this.path,
    this.all = false,
    this.package,
    this.owner,
    this.coverageReports = const <String>[],
    this.ignoreBaseline = false,
  });

  final String? path;
  final bool all;
  final String? package;
  final String? owner;
  final List<String> coverageReports;
  final bool ignoreBaseline;
}

class DiffOptions {
  const DiffOptions({
    this.headRef,
    this.all = false,
    this.package,
    this.owner,
    this.resultCache,
  });

  final String? headRef;
  final bool all;
  final String? package;
  final String? owner;
  final String? resultCache;
}

class TrendOptions {
  const TrendOptions({
    this.since,
    this.last,
    this.resultCache,
  });

  final String? since;
  final int? last;
  final String? resultCache;
}

class ImpactOptions {
  const ImpactOptions({
    this.paths = const <String>[],
    this.changed,
    this.depth,
    this.package,
  });

  final List<String> paths;
  final String? changed;
  final int? depth;
  final String? package;
}

class TraceOptions {
  const TraceOptions({
    this.all = false,
    this.maxPaths,
    this.package,
  });

  final bool all;
  final int? maxPaths;
  final String? package;
}

class InitOptions {
  const InitOptions({
    this.format = InitFormat.jsonc,
    this.adopt = false,
  });

  final InitFormat format;
  final bool adopt;
}

class ConfigOptions {
  const ConfigOptions({
    this.path,
    this.pluginMemoryMb,
    this.pluginTimeoutMs,
    this.budgets = const <String>[],
    this.sourceSets = const <String>[],
  });

  final String? path;
  final int? pluginMemoryMb;
  final int? pluginTimeoutMs;
  final List<String> budgets;
  final List<String> sourceSets;
}

class ReportOptions {
  const ReportOptions({
    this.format,
    this.all = false,
    this.output,
  });

  final ReportRenderFormat? format;
  final bool all;
  final String? output;
}

class DoctorOptions {
  const DoctorOptions({this.path});

  final String? path;
}

class AgentStatusOptions {
  const AgentStatusOptions({this.client, this.configRoot});

  final AgentClient? client;
  final String? configRoot;
}

class AgentInstallOptions {
  const AgentInstallOptions({
    this.client,
    this.dryRun = false,
    this.configRoot,
  });

  final AgentClient? client;
  final bool dryRun;
  final String? configRoot;
}

class ChurnOptions {
  const ChurnOptions({
    this.since,
    this.commits,
    this.all = false,
  });

  final String? since;
  final int? commits;
  final bool all;
}

class CouplingOptions {
  const CouplingOptions({
    this.since,
    this.commits,
    this.all = false,
    this.maxCommitFiles,
    this.minCoChanges,
    this.minRatio,
    this.includeTests = false,
  });

  final String? since;
  final int? commits;
  final bool all;
  final int? maxCommitFiles;
  final int? minCoChanges;
  final double? minRatio;
  final bool includeTests;
}

class StateUpdateOptions {
  const StateUpdateOptions({
    this.path,
    this.reset = false,
    this.coverageReports = const <String>[],
  });

  final String? path;
  final bool reset;
  final List<String> coverageReports;
}

typedef RatchetUpdateOptions = StateUpdateOptions;
typedef BaselineUpdateOptions = StateUpdateOptions;

class PluginTestOptions {
  const PluginTestOptions({
    this.kind = PluginKind.language,
    this.sources = const <String>[],
    this.pluginMemoryMb,
    this.pluginTimeoutMs,
  });

  final PluginKind kind;
  final List<String> sources;
  final int? pluginMemoryMb;
  final int? pluginTimeoutMs;
}
