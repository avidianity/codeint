import 'dart:convert';
import 'dart:io';

import 'package:codeint/codeint.dart';
import 'package:test/test.dart';

import 'test_helpers.dart';

void main() {
  final binaryPath = resolveTestCodeintBinary();

  group('CodeintClient 18-operation offline parity', () {
    late Directory repo;
    late CodeintClient client;

    setUp(() async {
      repo = await createSampleRepo();
      client = CodeintClient(
        cwd: repo.path,
        binaryPath: binaryPath,
        allowDownload: false,
      );
    });

    tearDown(() async {
      if (await repo.exists()) {
        await repo.delete(recursive: true);
      }
    });

    test('check passes with exit 0 and fails gated with exit 1 on over-budget',
        () async {
      final passResult = await client.check(const CheckOptions(all: true));
      expect(passResult.passed, isTrue);
      expect(passResult.exitCode, equals(0));
      expect(passResult.failures, isEmpty);
      expect(passResult.report.files, greaterThanOrEqualTo(2));
      expect(passResult.report.sourceSets.containsKey('production'), isTrue);

      await writeRepoFile(
        repo,
        'codeint.json',
        '{"budgets":{"production":{"verbosity":0}}}\n',
      );
      await writeRepoFile(
        repo,
        'src/verbose.ts',
        'export function checkFlag(v: boolean): boolean {\n'
        '  return v ? true : false;\n'
        '}\n',
      );

      final failResult = await client.check(
        const CheckOptions(ignoreBaseline: true, all: true),
      );
      expect(failResult.passed, isFalse);
      expect(failResult.exitCode, equals(1));
      expect(failResult.failures, isNotEmpty);
      expect(failResult.report.overBudget, isNotEmpty);
    });

    test('diff passes and returns gated failure on regression', () async {
      final cacheDir = await createTempDir('diff_cache');
      addTearDown(() => cacheDir.delete(recursive: true));

      final passDiff = await client.diff(
        'HEAD',
        DiffOptions(all: true, resultCache: cacheDir.path),
      );
      expect(passDiff.passed, isTrue);
      expect(passDiff.exitCode, equals(0));
      expect(passDiff.report.base.ref, equals('HEAD'));
      expect(passDiff.report.head.isWorkingTree, isTrue);

      await writeRepoFile(
        repo,
        'src/verbose.ts',
        'export function checkFlag(v: boolean): boolean {\n'
        '  return v ? true : false;\n'
        '}\n',
      );
      await writeRepoFile(
        repo,
        'codeint.json',
        '{"budgets":{"production":{"verbosity":0}}}\n',
      );

      final failDiff = await client.diff('HEAD');
      expect(failDiff.passed, isFalse);
      expect(failDiff.exitCode, equals(1));
      expect(failDiff.failures, isNotEmpty);
    });

    test('trend supports default, since, last, and validates options', () async {
      await writeRepoFile(
        repo,
        'src/math.ts',
        'export function add(a: number, b: number): number {\n'
        '  return a + b + 0;\n'
        '}\n',
      );
      await runGit(repo, const <String>['add', '.']);
      await runGit(repo, const <String>['commit', '-m', 'second']);

      final fullTrend = await client.trend();
      expect(fullTrend.commits, equals(2));
      expect(fullTrend.sourceSets['production'], hasLength(2));

      final lastOne = await client.trend(const TrendOptions(last: 1));
      expect(lastOne.commits, equals(1));
      expect(lastOne.sourceSets['production'], hasLength(1));

      await expectLater(
        client.trend(const TrendOptions(last: 0)),
        throwsA(isA<CodeintError>()),
      );
      await expectLater(
        client.trend(const TrendOptions(since: '2026-01-01', last: 2)),
        throwsA(isA<CodeintError>()),
      );
    });

    test('impact and trace traverse import graph', () async {
      final impactReport = await client.impact(
        const ImpactOptions(paths: <String>['src/math.ts'], depth: 2),
      );
      expect(impactReport.targets, hasLength(1));
      expect(impactReport.summary.directDependents, equals(1));
      expect(impactReport.production.direct.first.path, equals('src/index.ts'));

      final traceReport = await client.trace(
        'src/index.ts',
        'src/math.ts',
        const TraceOptions(all: true, maxPaths: 5),
      );
      expect(traceReport.connected, isTrue);
      expect(traceReport.shortestDistance, equals(1));
      expect(traceReport.paths.first, equals(<String>['src/index.ts', 'src/math.ts']));
    });

    test('guard, schema, config, report, and doctor work offline', () async {
      final guardReport = await client.guard('src/index.ts');
      expect(guardReport.path, equals('src/index.ts'));
      expect(guardReport.exists, isTrue);

      final schemaReport = await client.schema();
      expect(schemaReport.name, equals('codeint'));
      expect(schemaReport.version, equals(codeintVersion));
      expect(schemaReport.metrics, isNotEmpty);

      final configReport = await client.config(
        const ConfigOptions(
          pluginMemoryMb: 256,
          pluginTimeoutMs: 5000,
          budgets: <String>['production.erosion=0.2'],
        ),
      );
      expect(configReport.configFilePresent, isTrue);
      expect(configReport.settings, isNotEmpty);

      final checkJson = await Process.run(
        binaryPath,
        const <String>['check', '--format', 'json'],
        workingDirectory: repo.path,
      );
      final savedReport = await writeRepoFile(
        repo,
        'saved-check.json',
        checkJson.stdout.toString(),
      );
      final rendered = await client.report(
        savedReport.path,
        const ReportOptions(format: ReportRenderFormat.markdown, all: true),
      );
      expect(rendered, contains('Production'));

      final doctorOk = await client.doctor();
      expect(doctorOk.passed, isTrue);
      expect(doctorOk.exitCode, equals(0));
      expect(doctorOk.report.checks, isNotEmpty);

      await writeRepoFile(repo, 'codeint.json', '{ invalid json');
      final doctorBad = await client.doctor();
      expect(doctorBad.passed, isFalse);
      expect(doctorBad.exitCode, equals(1));
      expect(doctorBad.report.status, equals('error'));
    });

    test('agentStatus and agentInstall operate in isolated configRoot', () async {
      final agentHome = await createTempDir('agent_home');
      addTearDown(() => agentHome.delete(recursive: true));

      final statusReport = await client.agentStatus(
        AgentStatusOptions(
          client: AgentClient.codex,
          configRoot: agentHome.path,
        ),
      );
      expect(statusReport.agents, hasLength(1));
      expect(statusReport.agents.first.client, equals('codex'));

      final installReport = await client.agentInstall(
        AgentInstallOptions(
          client: AgentClient.codex,
          dryRun: true,
          configRoot: agentHome.path,
        ),
      );
      expect(installReport.dryRun, isTrue);
      expect(installReport.results, hasLength(1));
      expect(installReport.results.first.written, isFalse);
    });

    test('churn, coupling, ratchetUpdate, baselineUpdate, and pluginTest work',
        () async {
      final churnReport = await client.churn(const ChurnOptions(commits: 5));
      expect(churnReport.commitsAnalyzed, greaterThanOrEqualTo(1));
      expect(churnReport.sourceSets.containsKey('production'), isTrue);

      final couplingReport = await client.coupling(
        const CouplingOptions(
          commits: 5,
          maxCommitFiles: 20,
          minCoChanges: 1,
          minRatio: 0.1,
          includeTests: true,
        ),
      );
      expect(couplingReport.maxCommitFiles, equals(20));
      expect(couplingReport.minCoChanges, equals(1));

      final ratchetReport = await client.ratchetUpdate(
        const RatchetUpdateOptions(reset: true),
      );
      expect(ratchetReport.created, isTrue);
      expect(ratchetReport.totalEntries, greaterThan(0));

      final baselineReport = await client.baselineUpdate(
        const BaselineUpdateOptions(reset: true),
      );
      expect(baselineReport.created, isTrue);

      final invalidWasm = await writeRepoFile(repo, 'bad.wasm', 'not-wasm');
      final pluginOutcome = await client.pluginTest(
        invalidWasm.path,
        const PluginTestOptions(
          kind: PluginKind.language,
          pluginMemoryMb: 128,
          pluginTimeoutMs: 2000,
        ),
      );
      expect(pluginOutcome.passed, isFalse);
      expect(pluginOutcome.exitCode, equals(1));
      expect(pluginOutcome.report.passed, isFalse);
      expect(pluginOutcome.report.summary.failed, greaterThan(0));
    });

    test('operational exit 2 throws CodeintError with exitCode and stderr',
        () async {
      await expectLater(
        client.diff('refs/heads/does-not-exist-anywhere'),
        throwsA(
          isA<CodeintError>()
              .having((e) => e.exitCode, 'exitCode', equals(2))
              .having((e) => e.stderr, 'stderr', isNotEmpty),
        ),
      );
    });
  });

  group('init across all 4 formats and adopt', () {
    for (final format in InitFormat.values) {
      test('init writes ${format.cliValue} starter config', () async {
        final repo = await createSampleRepo(includeConfig: false);
        addTearDown(() => repo.delete(recursive: true));

        final client = CodeintClient(
          cwd: repo.path,
          binaryPath: binaryPath,
          allowDownload: false,
        );
        final result = await client.init(InitOptions(format: format));
        expect(result.configPath, endsWith('codeint.${format.cliValue}'));
        expect(result.adopted, isNull);
        expect(File(result.configPath).existsSync(), isTrue);
      });
    }

    test('init with adopt writes config, ratchet, and baseline', () async {
      final repo = await createSampleRepo(includeConfig: false);
      addTearDown(() => repo.delete(recursive: true));

      final client = CodeintClient(
        cwd: repo.path,
        binaryPath: binaryPath,
        allowDownload: false,
      );
      final result = await client.init(
        const InitOptions(format: InitFormat.yaml, adopt: true),
      );
      expect(result.configPath, endsWith('codeint.yaml'));
      expect(result.adopted, isNotNull);
      expect(result.adopted!.ratchet, endsWith('ratchet.json'));
      expect(result.adopted!.baseline, endsWith('baseline.json'));
      expect(result.adopted!.ratchetEntries, greaterThan(0));
      expect(jsonDecode(File(result.adopted!.ratchet).readAsStringSync()), isMap);
    });
  });
}
