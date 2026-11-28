import 'dart:convert';

import 'binary_resolver.dart';
import 'enums.dart';
import 'errors.dart';
import 'models/check_report.dart';
import 'models/diff_report.dart';
import 'models/history_reports.dart';
import 'models/navigation_reports.dart';
import 'models/tooling_reports.dart';
import 'options.dart';
import 'process_runner.dart';

class CodeintClient {
  const CodeintClient({
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

  CommonOptions get _common => CommonOptions(
        cwd: cwd,
        binaryPath: binaryPath,
        allowDownload: allowDownload,
        releaseBaseUrl: releaseBaseUrl,
        cacheDir: cacheDir,
        environment: environment,
      );

  Future<String> ensureBinary([BinaryResolverOptions? options]) =>
      ensureBinaryResolved(options ?? _common.toResolverOptions());

  Future<GatedOutcome<CheckReport>> check([
    CheckOptions options = const CheckOptions(),
  ]) {
    final args = <String>['check'];
    if (options.path != null) {
      args.add(options.path!);
    }
    _addRepeated(args, '--coverage-report', options.coverageReports);
    if (options.ignoreBaseline) {
      args.add('--ignore-baseline');
    }
    _appendReportArgs(
      args,
      package: options.package,
      owner: options.owner,
      all: options.all,
    );
    return runGatedJsonCommand(args, _common, CheckReport.fromJson);
  }

  Future<GatedOutcome<DiffReport>> diff(
    String baseRef, [
    DiffOptions options = const DiffOptions(),
  ]) {
    final args = <String>['diff', baseRef];
    _addOpt(args, '--head', options.headRef);
    _appendReportArgs(
      args,
      package: options.package,
      owner: options.owner,
      all: options.all,
    );
    return runGatedJsonCommand(
      args,
      _common,
      DiffReport.fromJson,
      extraEnv: _resultCacheEnv(options.resultCache),
    );
  }

  Future<TrendReport> trend([
    TrendOptions options = const TrendOptions(),
  ]) async {
    if (options.last != null && options.last! <= 0) {
      throw const CodeintError('TrendOptions.last must be at least 1');
    }
    if (options.since != null && options.last != null) {
      throw const CodeintError(
        'TrendOptions.since and TrendOptions.last cannot be set together',
      );
    }
    final args = <String>['trend'];
    _addOpt(args, '--since', options.since);
    args.addAll(const <String>['--format', 'json']);
    final report = await runJsonCommand(
      args,
      _common,
      TrendReport.fromJson,
      extraEnv: _resultCacheEnv(options.resultCache),
    );
    return options.last == null ? report : report.limitLast(options.last!);
  }

  Future<ImpactReport> impact([
    ImpactOptions options = const ImpactOptions(),
  ]) {
    final args = <String>['impact', ...options.paths];
    _addOpt(args, '--changed', options.changed);
    _addOpt(args, '--depth', options.depth);
    _addOpt(args, '--package', options.package);
    args.addAll(const <String>['--format', 'json']);
    return runJsonCommand(args, _common, ImpactReport.fromJson);
  }

  Future<TraceReport> trace(
    String from,
    String to, [
    TraceOptions options = const TraceOptions(),
  ]) {
    final args = <String>['trace', from, to];
    if (options.all) {
      args.add('--all');
    }
    _addOpt(args, '--max-paths', options.maxPaths);
    _addOpt(args, '--package', options.package);
    args.addAll(const <String>['--format', 'json']);
    return runJsonCommand(args, _common, TraceReport.fromJson);
  }

  Future<InitResult> init([
    InitOptions options = const InitOptions(),
  ]) async {
    final args = <String>['init', '--format', options.format.cliValue];
    if (options.adopt) {
      args.add('--adopt');
    }
    final stdout = await runTextCommand(args, _common);
    return _parseInitOutput(stdout, options.adopt);
  }

  Future<GuardReport> guard(String path) => runJsonCommand(
        <String>['guard', path, '--format', 'json'],
        _common,
        GuardReport.fromJson,
      );

  Future<SchemaReport> schema() => runJsonCommand(
        const <String>['schema', '--format', 'json'],
        _common,
        SchemaReport.fromJson,
      );

  Future<ConfigReport> config([
    ConfigOptions options = const ConfigOptions(),
  ]) {
    final args = <String>['config'];
    if (options.path != null) {
      args.add(options.path!);
    }
    args.addAll(const <String>['--resolved', '--format', 'json']);
    _addOpt(args, '--plugin-memory-mb', options.pluginMemoryMb);
    _addOpt(args, '--plugin-timeout-ms', options.pluginTimeoutMs);
    _addRepeated(args, '--budget', options.budgets);
    _addRepeated(args, '--source-set', options.sourceSets);
    return runJsonCommand(args, _common, parseConfigReport);
  }

  Future<String> report(
    String input, [
    ReportOptions options = const ReportOptions(),
  ]) {
    final args = <String>['report', input];
    if (options.all) {
      args.add('--all');
    }
    _addOpt(args, '--format', options.format?.cliValue);
    _addOpt(args, '--output', options.output);
    return runTextCommand(args, _common);
  }

  Future<GatedOutcome<DoctorReport>> doctor([
    DoctorOptions options = const DoctorOptions(),
  ]) {
    final args = <String>['doctor'];
    if (options.path != null) {
      args.add(options.path!);
    }
    args.addAll(const <String>['--format', 'json']);
    return runGatedJsonCommand(args, _common, parseDoctorReport);
  }

  Future<AgentStatusReport> agentStatus([
    AgentStatusOptions options = const AgentStatusOptions(),
  ]) {
    final args = <String>['agent', 'status'];
    _appendAgentArgs(
      args,
      client: options.client,
      configRoot: options.configRoot,
    );
    return runJsonCommand(args, _common, parseAgentStatusReport);
  }

  Future<AgentInstallReport> agentInstall([
    AgentInstallOptions options = const AgentInstallOptions(),
  ]) {
    final args = <String>['agent', 'install'];
    if (options.dryRun) {
      args.add('--dry-run');
    }
    _appendAgentArgs(
      args,
      client: options.client,
      configRoot: options.configRoot,
    );
    return runJsonCommand(args, _common, parseAgentInstallReport);
  }

  Future<ChurnReport> churn([
    ChurnOptions options = const ChurnOptions(),
  ]) {
    final args = <String>['churn'];
    _appendHistoryArgs(
      args,
      since: options.since,
      commits: options.commits,
      all: options.all,
    );
    return runJsonCommand(args, _common, parseChurnReport);
  }

  Future<CouplingReport> coupling([
    CouplingOptions options = const CouplingOptions(),
  ]) {
    final args = <String>['coupling'];
    _appendHistoryArgs(
      args,
      since: options.since,
      commits: options.commits,
      all: options.all,
    );
    _addOpt(args, '--max-commit-files', options.maxCommitFiles);
    _addOpt(args, '--min-co-changes', options.minCoChanges);
    _addOpt(args, '--min-ratio', options.minRatio);
    if (options.includeTests) {
      args.add('--include-tests');
    }
    return runJsonCommand(args, _common, parseCouplingReport);
  }

  Future<RatchetUpdateReport> ratchetUpdate([
    RatchetUpdateOptions options = const RatchetUpdateOptions(),
  ]) =>
      runJsonCommand(
        _stateUpdateArgs('ratchet', options),
        _common,
        parseRatchetUpdateReport,
      );

  Future<BaselineUpdateReport> baselineUpdate([
    BaselineUpdateOptions options = const BaselineUpdateOptions(),
  ]) =>
      runJsonCommand(
        _stateUpdateArgs('baseline', options),
        _common,
        parseBaselineUpdateReport,
      );

  Future<GatedOutcome<PluginTestReport>> pluginTest(
    String wasmPath, [
    PluginTestOptions options = const PluginTestOptions(),
  ]) {
    final args = <String>[
      'plugin',
      'test',
      wasmPath,
      '--kind',
      options.kind.cliValue,
      '--format',
      'json',
    ];
    _addRepeated(args, '--source', options.sources);
    _addOpt(args, '--plugin-memory-mb', options.pluginMemoryMb);
    _addOpt(args, '--plugin-timeout-ms', options.pluginTimeoutMs);
    return runGatedJsonCommand(args, _common, parsePluginTestReport);
  }
}

Future<String> ensureBinaryResolved([
  BinaryResolverOptions options = const BinaryResolverOptions(),
]) =>
    ensureBinary(options);

void _addOpt(List<String> args, String flag, Object? value) {
  if (value != null) {
    args.addAll(<String>[flag, '$value']);
  }
}

void _addRepeated(List<String> args, String flag, List<String> values) {
  for (final value in values) {
    args.addAll(<String>[flag, value]);
  }
}

void _appendReportArgs(
  List<String> args, {
  String? package,
  String? owner,
  required bool all,
}) {
  _addOpt(args, '--package', package);
  _addOpt(args, '--owner', owner);
  if (all) {
    args.add('--all');
  }
  args.addAll(const <String>['--format', 'json']);
}

void _appendAgentArgs(
  List<String> args, {
  AgentClient? client,
  String? configRoot,
}) {
  _addOpt(args, '--client', client?.cliValue);
  _addOpt(args, '--config-root', configRoot);
  args.addAll(const <String>['--format', 'json']);
}

void _appendHistoryArgs(
  List<String> args, {
  String? since,
  int? commits,
  required bool all,
}) {
  _addOpt(args, '--since', since);
  _addOpt(args, '--commits', commits);
  if (all) {
    args.add('--all');
  }
  args.addAll(const <String>['--format', 'json']);
}

List<String> _stateUpdateArgs(String command, StateUpdateOptions options) {
  final args = <String>[command, 'update'];
  if (options.path != null) {
    args.add(options.path!);
  }
  if (options.reset) {
    args.add('--reset');
  }
  _addRepeated(args, '--coverage-report', options.coverageReports);
  args.addAll(const <String>['--format', 'json']);
  return args;
}

Map<String, String>? _resultCacheEnv(String? resultCache) =>
    resultCache == null ? null : <String, String>{'CODEINT_CACHE_DIR': resultCache};

InitResult _parseInitOutput(String stdout, bool adopt) {
  final lines = const LineSplitter().convert(stdout);
  final wroteLines = lines
      .map((line) => line.trim())
      .where((line) => line.startsWith('Wrote '))
      .toList(growable: false);
  if (wroteLines.isEmpty) {
    throw CodeintError('Unexpected codeint init output: $stdout');
  }
  final configPath = wroteLines.first.substring('Wrote '.length).trim();
  if (!adopt) {
    return InitResult(configPath: configPath, stdout: stdout);
  }
  return InitResult(
    configPath: configPath,
    adopted: _parseAdoptedFiles(configPath, wroteLines),
    stdout: stdout,
  );
}

AdoptedFiles _parseAdoptedFiles(String configPath, List<String> wroteLines) {
  String ratchetPath = '';
  int ratchetEntries = 0;
  String baselinePath = '';
  int baselinedFindings = 0;
  final ratchetPattern = RegExp(r'^Wrote (.+) \((\d+) Metric ceilings\)$');
  final baselinePattern = RegExp(r'^Wrote (.+) \((\d+) existing Findings\)$');
  for (final line in wroteLines.skip(1)) {
    final ratchetMatch = ratchetPattern.firstMatch(line);
    if (ratchetMatch != null) {
      ratchetPath = ratchetMatch.group(1)!.trim();
      ratchetEntries = int.parse(ratchetMatch.group(2)!);
      continue;
    }
    final baselineMatch = baselinePattern.firstMatch(line);
    if (baselineMatch != null) {
      baselinePath = baselineMatch.group(1)!.trim();
      baselinedFindings = int.parse(baselineMatch.group(2)!);
    }
  }
  if (ratchetPath.isEmpty || baselinePath.isEmpty) {
    throw CodeintError(
      'Unexpected codeint init --adopt output: missing ratchet or baseline summary',
    );
  }
  return AdoptedFiles(
    configuration: configPath,
    ratchet: ratchetPath,
    ratchetEntries: ratchetEntries,
    baseline: baselinePath,
    baselinedFindings: baselinedFindings,
  );
}
