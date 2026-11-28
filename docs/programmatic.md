# Programmatic API

codeint provides Wave 1 client SDKs in three languages so tools and scripts can call codeint directly without parsing terminal text ([ADR 0038](./adr/0038-programmatic-sdks.md)):

- **JavaScript and TypeScript** (`codeint` on npm, `npm/codeint`): dual ESM and CommonJS package with TypeScript declarations, zero runtime dependencies, and per-platform native binary `optionalDependencies` (`@avidian/codeint-{os}-{arch}`).
- **Rust** (`codeint-cli` in `crates/cli`): in-process typed Rust library for workspace and Git dependencies (`publish = false` while [ADR 0006](./adr/0006-exact-dependency-pins.md) enforces exact workspace pins).
- **Dart** (`codeint` on pub.dev, `sdks/dart`): pure Dart package (`sdk: ^3.6.0`, zero Flutter dependency) with a 4-step SHA-256-verified binary resolver and `dart run codeint` CLI forwarding.

Every Wave 1 client SDK exposes all 18 non-interactive operations across 17 commands: `check`, `diff`, `trend`, `impact`, `trace`, `init` (with `format` and `adopt`), `guard`, `schema`, `config`, `report`, `doctor`, `agent status`, `agent install`, `churn`, `coupling`, `ratchet update`, `baseline update`, and `plugin test`.
`codeint mcp` and `codeint update` remain CLI-only.

## Uniform exit-code and failure semantics

All three client SDKs share the same failure model:

- **Gated operations** (`check`, `diff`, `doctor`, and `plugin test`) return a structured pass/fail outcome on both exit `0` (passed) and exit `1` (Budget, Ratchet, Regression, `doctor` blocking error, or `plugin test` failure).
  - TypeScript/JavaScript resolves to `GatedResult<TReport>` (`{ passed, exitCode, failures, report }`).
  - Rust returns `Ok(GatedOutcome<T>)` (`passed`, `failures`, `report`).
  - Dart resolves to `GatedOutcome<T>` (`passed`, `exitCode`, `failures`, `report`).
- **Operational errors** (exit `2`, such as an invalid configuration file, a missing folder, an unknown Git ref, or a directory outside Git) fail loudly:
  - TypeScript/JavaScript throws `CodeintError` with `message`, `exitCode`, `stderr`, and `cause`.
  - Rust returns `Err(CodeintError)` with `message()` and `remedy()`.
  - Dart throws `CodeintError` with `message`, `exitCode`, `stderr`, and `cause`.

## Operations and options

Every client SDK accepts full CLI option parity for every exposed operation.
See [docs/reference.md](./reference.md) for the exact semantics of each command and flag.

| CLI command | TypeScript / JS | Rust (`codeint-cli`) | Dart (`CodeintClient`) | Gated | Key options |
| ----------- | --------------- | -------------------- | ---------------------- | ----- | ----------- |
| `check` | `check(options?)` | `check(folder, &CheckOptions)` | `check([CheckOptions])` | Yes | `path`, `all`, `package`, `owner`, `coverageReports` / `coverage_reports`, `ignoreBaseline` / `ignore_baseline` |
| `diff` | `diff(baseRef, options?)` | `diff(folder, base_ref, &DiffOptions)` | `diff(baseRef, [DiffOptions])` | Yes | `headRef` / `head_ref`, `all`, `package`, `owner`, `resultCache` / `result_cache` |
| `trend` | `trend(options?)` | `trend(folder, &TrendOptions)` | `trend([TrendOptions])` | No | `since`, `last` (Dart) / `range` (Rust), `resultCache` / `result_cache` |
| `impact` | `impact(options)` | `impact(folder, &ImpactOptions)` | `impact([ImpactOptions])` | No | `paths`, `changed`, `depth`, `package` |
| `trace` | `trace(from, to, options?)` | `trace(folder, from, to, &TraceOptions)` | `trace(from, to, [TraceOptions])` | No | `all`, `maxPaths` / `max_paths`, `package` |
| `init` | `init(options?)` | `init(folder, &InitOptions)` | `init([InitOptions])` | No | `format` (`jsonc`, `json`, `toml`, `yaml`), `adopt` |
| `guard` | `guard(targetPath, options?)` | `guard(folder, target)` | `guard(path)` | No | `targetPath` / `target` / `path` |
| `schema` | `schema(options?)` | `schema()` | `schema()` | No | `output` (TypeScript) |
| `config` | `config(options?)` | `resolved_config(folder, &ConfigOverrides)` | `config([ConfigOptions])` | No | `path`, `budgets`, `sourceSets` / `source_sets`, `pluginMemoryMb` / `plugin_memory_mb`, `pluginTimeoutMs` / `plugin_timeout_ms` |
| `report` | `report(input, options?)` | `rerender_report(folder, input, &RerenderReportOptions)` | `report(input, [ReportOptions])` | No | `format` (`toon`, `text`, `json`, `sarif`, `markdown`), `all`, `output` |
| `doctor` | `doctor(options?)` | `doctor(folder, &PluginHostConfig)` | `doctor([DoctorOptions])` | Yes | `path` (TypeScript, Dart), `plugin_host` (Rust) |
| `agent status` | `agentStatus(options?)` | `agent_status(config_root, client)` | `agentStatus([AgentStatusOptions])` | No | `client` (`claude-code`, `cursor`, `codex`), `configRoot` / `config_root` |
| `agent install` | `agentInstall(options?)` | `agent_install(config_root, client, dry_run)` | `agentInstall([AgentInstallOptions])` | No | `client`, `configRoot` / `config_root`, `dryRun` / `dry_run` |
| `churn` | `churn(options?)` | `churn(folder, &ChurnOptions)` | `churn([ChurnOptions])` | No | `since`, `commits` / `window`, `all` / `findings` |
| `coupling` | `coupling(options?)` | `coupling(folder, &CouplingOptions)` | `coupling([CouplingOptions])` | No | `since`, `commits`, `all`, `maxCommitFiles` / `max_commit_files`, `minCoChanges` / `min_co_changes`, `minRatio` / `min_ratio`, `includeTests` / `include_tests` |
| `ratchet update` | `ratchetUpdate(options?)` | `ratchet_update(folder, &RatchetUpdateOptions)` | `ratchetUpdate([RatchetUpdateOptions])` | No | `path`, `reset`, `coverageReports` / `coverage_reports` |
| `baseline update` | `baselineUpdate(options?)` | `baseline_update(folder, &BaselineUpdateOptions)` | `baselineUpdate([BaselineUpdateOptions])` | No | `path`, `reset`, `coverageReports` / `coverage_reports` |
| `plugin test` | `pluginTest(pluginPath, options?)` | `plugin_test(path, &PluginTestOptions)` | `pluginTest(wasmPath, [PluginTestOptions])` | Yes | `kind` (`language`, `metric`), `sources`, `pluginMemoryMb` / `limits.memory_mb`, `pluginTimeoutMs` / `limits.timeout_ms` |

Subprocess SDKs also accept common execution options:

- TypeScript/JavaScript accepts `cwd` and `binaryPath` on every options object, plus `output` on report-producing operations (`check`, `diff`, `trend`, `impact`, `trace`, `schema`, `report`, `churn`, `coupling`).
- Dart configures `cwd`, `binaryPath`, `allowDownload`, `releaseBaseUrl`, `cacheDir`, and `environment` on `CodeintClient`.
- Rust accepts `PluginHostConfig` on measurement options (`check`, `diff`, `trend`, `impact`, `trace`, `init`, `doctor`, `churn`, `coupling`, `ratchet_update`, `baseline_update`).

## Built-in Metrics, custom Metric plugins, and report JSON Schemas

Typed report models in TypeScript and Dart, and serialized JSON reports rendered by Rust (`Report::render`, `DiffReport::render`, `TrendReport::render`), expose all 12 built-in Metrics:

- Per Source set (`source_sets.<name>`): `erosion`, `cognitive_complexity`, `duplication`, `verbosity`, `import_cycles`, `boundaries`, `orphan_files`, `test_gap`, and `hazards`.
- Top-level Dependency hygiene (`dependencies`): `unused_dependencies`, `unlisted_dependencies`, and `dev_in_production_dependencies`.

External WebAssembly Metric plugins appear alongside built-in Metrics in an extensible custom-Metric map:

- TypeScript/JavaScript exposes custom Metric keys through `[customMetric: string]` index signatures on `SourceSetReport` (`ExternalMetricSection`), `SourceSetChanges` (`ExternalMetricChanges`), and `TrendRow` (`number`).
- Rust emits custom Metric plugin keys in serialized report output from `.render(...)` (such as `ReportFormat::Json` or `TrendFormat::Json`), matching the exported report JSON Schemas.
- Dart exposes `externalMetrics` getters on `SourceSetReport`, `SourceSetChanges`, and `TrendRow`.

codeint exports 16 JSON Schemas for all JSON report payloads from `codeint-report` (`all_report_schemas()`, `report_schema()`, `schema_for()`):

- [schema/codeint-report.schema.json](../schema/codeint-report.schema.json) for `codeint check`.
- 15 command schemas under `schema/reports/`:
  [diff.schema.json](../schema/reports/diff.schema.json), [trend.schema.json](../schema/reports/trend.schema.json), [impact.schema.json](../schema/reports/impact.schema.json), [trace.schema.json](../schema/reports/trace.schema.json), [guard.schema.json](../schema/reports/guard.schema.json), [schema.schema.json](../schema/reports/schema.schema.json), [config.schema.json](../schema/reports/config.schema.json), [doctor.schema.json](../schema/reports/doctor.schema.json), [agent-status.schema.json](../schema/reports/agent-status.schema.json), [agent-install.schema.json](../schema/reports/agent-install.schema.json), [churn.schema.json](../schema/reports/churn.schema.json), [coupling.schema.json](../schema/reports/coupling.schema.json), [ratchet-update.schema.json](../schema/reports/ratchet-update.schema.json), [baseline-update.schema.json](../schema/reports/baseline-update.schema.json), and [plugin-test.schema.json](../schema/reports/plugin-test.schema.json).

## JavaScript and TypeScript (`codeint`)

Install the `codeint` npm package, which ships dual ESM and CommonJS modules, TypeScript declarations, and the native platform binary with zero runtime dependencies:

```sh
npm install --save-dev codeint
```

The package exports all 18 operations (`check`, `diff`, `trend`, `impact`, `trace`, `init`, `guard`, `schema`, `config`, `report`, `doctor`, `agentStatus`, `agentInstall`, `churn`, `coupling`, `ratchetUpdate`, `baselineUpdate`, and `pluginTest`), plus `CodeintError` and `resolveBinary`.

```ts
import {
  CodeintError,
  agentStatus,
  baselineUpdate,
  check,
  churn,
  config,
  coupling,
  diff,
  doctor,
  guard,
  impact,
  init,
  pluginTest,
  ratchetUpdate,
  report,
  resolveBinary,
  schema,
  trace,
  trend,
} from "codeint";

try {
  const checked = await check({
    cwd: process.cwd(),
    all: false,
    coverageReports: ["coverage/lcov.info"],
  });
  if (!checked.passed) {
    console.error("Budget or Ratchet failures:", checked.failures);
  }
  console.log("Production erosion:", checked.report.source_sets.production?.erosion.ratio);

  const diffed = await diff("origin/main", { cwd: process.cwd(), package: "packages/web" });
  if (!diffed.passed) {
    console.error("Regressions:", diffed.failures);
  }

  const diagnosed = await doctor({ cwd: process.cwd() });
  if (!diagnosed.passed) {
    console.error("Doctor failures:", diagnosed.failures);
  }

  const tested = await pluginTest("dist/plugin.wasm", {
    cwd: process.cwd(),
    kind: "language",
    sources: ["fixtures/sample.mini"],
  });
  if (!tested.passed) {
    console.error("Plugin failures:", tested.failures);
  }

  const history = await trend({ cwd: process.cwd(), since: "2026-01-01" });
  const blastRadius = await impact({ cwd: process.cwd(), paths: ["src/domain/order.ts"] });
  const importPaths = await trace("src/api/index.ts", "src/domain/order.ts", { cwd: process.cwd() });
  const boundary = await guard("src/domain/order.ts", { cwd: process.cwd() });
  const manifest = await schema({ cwd: process.cwd() });
  const resolved = await config({ cwd: process.cwd() });
  const markdown = await report("check.json", { cwd: process.cwd(), format: "markdown" });
  const agents = await agentStatus({ cwd: process.cwd() });
  const hotFiles = await churn({ cwd: process.cwd(), commits: 50 });
  const coupled = await coupling({ cwd: process.cwd(), minCoChanges: 3 });
  const ratchet = await ratchetUpdate({ cwd: process.cwd() });
  const baseline = await baselineUpdate({ cwd: process.cwd() });
  const created = await init({ cwd: process.cwd(), format: "jsonc", adopt: false });

  console.log({
    commits: history.commits,
    dependents: blastRadius.summary.total_dependents,
    connected: importPaths.connected,
    boundaryExists: boundary.exists,
    cliVersion: manifest.version,
    configFile: resolved.config_file,
    markdownLength: markdown.length,
    agents: agents.agents.length,
    churnCommits: hotFiles.commits_analyzed,
    coupledPairs: coupled.pairs.length,
    ratchetEntries: ratchet.total_entries,
    baselinedFindings: baseline.total_findings,
    configPath: created.configPath,
    binary: resolveBinary(),
  });
} catch (error) {
  if (error instanceof CodeintError) {
    console.error(`Operational error (exit ${error.exitCode}):`, error.message);
    console.error(error.stderr);
  }
  throw error;
}
```

CommonJS callers can import the same functions with `require("codeint")`.
Pass `binaryPath` in any options object to override the native executable path.

## Rust (`codeint-cli`)

`codeint-cli` is an in-process Rust library crate in this workspace (`publish = false` while [ADR 0006](./adr/0006-exact-dependency-pins.md) enforces exact workspace dependency pins).
Within the workspace or from a path or Git dependency on `crates/cli`, import the typed library functions directly:

```rust
use std::path::Path;

use codeint_cli::{
    BaselineUpdateOptions, CheckOptions, ChurnOptions, ConfigOverrides, CouplingOptions,
    DiffOptions, ImpactOptions, InitFormat, InitOptions, PluginHostConfig, PluginKind,
    PluginTestOptions, RatchetUpdateOptions, ReportFormat, ReportRenderFormat,
    RerenderReportOptions, TraceOptions, TrendOptions, agent_install, agent_status,
    baseline_update, check, churn, coupling, diff, doctor, guard, impact, init, plugin_test,
    ratchet_update, rerender_report, resolved_config, schema, trace, trend,
};

fn run(folder: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let checked = check(folder, &CheckOptions::default())?;
    if !checked.passed {
        for failure in &checked.failures {
            eprintln!("{failure}");
        }
    }
    let _json = checked.report.render(ReportFormat::Json)?;

    let _diffed = diff(folder, "origin/main", &DiffOptions::default())?;
    let _history = trend(folder, &TrendOptions::default())?;
    let _blast_radius = impact(
        folder,
        &ImpactOptions {
            paths: vec!["src/domain/order.ts".to_string()],
            ..ImpactOptions::default()
        },
    )?;
    let _paths = trace(
        folder,
        "src/api/index.ts",
        "src/domain/order.ts",
        &TraceOptions::default(),
    )?;
    let _boundary = guard(folder, Path::new("src/domain/order.ts"))?;
    let _manifest = schema();
    let _resolved = resolved_config(folder, &ConfigOverrides::default())?;
    let _markdown = rerender_report(
        folder,
        Path::new("check.json"),
        &RerenderReportOptions {
            format: Some(ReportRenderFormat::Markdown),
            all: false,
        },
    )?;
    let _diagnosed = doctor(folder, &PluginHostConfig::default())?;
    let _agents = agent_status(folder, None)?;
    let _installed = agent_install(folder, None, true)?;
    let _hot_files = churn(folder, &ChurnOptions::default())?;
    let _coupled = coupling(folder, &CouplingOptions::default())?;
    let _ratchet = ratchet_update(folder, &RatchetUpdateOptions::default())?;
    let _baseline = baseline_update(folder, &BaselineUpdateOptions::default())?;
    let _tested = plugin_test(
        Path::new("dist/plugin.wasm"),
        &PluginTestOptions::new(PluginKind::Language),
    )?;
    let _created = init(
        folder,
        &InitOptions {
            format: InitFormat::Jsonc,
            adopt: false,
            ..InitOptions::default()
        },
    )?;
    Ok(())
}
```

Operational errors return `Err(CodeintError)`, which exposes `message()` and `remedy()`.
Use `SavedReport::from_json` from `codeint_cli` to re-render a JSON report in memory without reading from disk.
See [crates/cli/HOW-TO.md](../crates/cli/HOW-TO.md) for internal call contracts.

## Dart (`codeint`)

Install the pure Dart `codeint` package from pub.dev (`sdk: ^3.6.0`, zero Flutter dependency):

```sh
dart pub add --dev codeint
```

Run the CLI directly through Dart with `dart run codeint`:

```sh
dart run codeint check
dart run codeint diff origin/main
```

`dart run codeint` resolves the native binary, forwards all arguments and stdio directly to it, and exits with the binary's exact exit code (`0`, `1`, or `2`).

### Programmatic usage in Dart

`CodeintClient` exposes all 18 operations with typed option classes and typed report models:

```dart
import 'package:codeint/codeint.dart';

Future<void> main() async {
  const client = CodeintClient(cwd: '.');

  try {
    final checked = await client.check(
      const CheckOptions(
        all: false,
        coverageReports: <String>['coverage/lcov.info'],
      ),
    );
    if (!checked.passed) {
      print('Budget or Ratchet failures: ${checked.failures.join(', ')}');
    }
    final production = checked.report.sourceSets['production'];
    print('Production erosion: ${production?.erosion.ratio}');
    print('Custom metrics: ${production?.externalMetrics.keys.join(', ')}');

    final diffed = await client.diff('origin/main');
    if (!diffed.passed) {
      print('Regressions: ${diffed.failures.join(', ')}');
    }

    final diagnosed = await client.doctor();
    if (!diagnosed.passed) {
      print('Doctor failures: ${diagnosed.failures.join(', ')}');
    }

    final tested = await client.pluginTest(
      'dist/plugin.wasm',
      const PluginTestOptions(
        kind: PluginKind.language,
        sources: <String>['fixtures/sample.mini'],
      ),
    );
    if (!tested.passed) {
      print('Plugin failures: ${tested.failures.join(', ')}');
    }

    final history = await client.trend(const TrendOptions(since: '2026-01-01'));
    final blastRadius = await client.impact(
      const ImpactOptions(paths: <String>['lib/domain/order.dart']),
    );
    final importPaths = await client.trace('lib/api/index.dart', 'lib/domain/order.dart');
    final boundary = await client.guard('lib/domain/order.dart');
    final manifest = await client.schema();
    final resolved = await client.config();
    final markdown = await client.report(
      'check.json',
      const ReportOptions(format: ReportRenderFormat.markdown),
    );
    final agents = await client.agentStatus();
    final hotFiles = await client.churn(const ChurnOptions(commits: 50));
    final coupled = await client.coupling(const CouplingOptions(minCoChanges: 3));
    final ratchet = await client.ratchetUpdate();
    final baseline = await client.baselineUpdate();
    final created = await client.init(
      const InitOptions(format: InitFormat.jsonc, adopt: false),
    );

    print(
      'commits=${history.commits} dependents=${blastRadius.summary.totalDependents} '
      'connected=${importPaths.connected} boundaryExists=${boundary.exists} '
      'version=${manifest.version} config=${resolved.configFile} '
      'markdownLen=${markdown.length} agents=${agents.agents.length} '
      'churn=${hotFiles.commitsAnalyzed} pairs=${coupled.pairs.length} '
      'ratchet=${ratchet.totalEntries} baseline=${baseline.totalFindings} '
      'init=${created.configPath}',
    );
  } on CodeintError catch (error) {
    print('Operational error (exit ${error.exitCode}): ${error.message}');
    if (error.stderr.isNotEmpty) {
      print(error.stderr);
    }
    rethrow;
  }
}
```

### Binary resolution and air-gapped CI in Dart

Because pub.dev caps package archives at 100 MB and does not support per-platform optional dependencies, `ensureBinary()` resolves the native `codeint` executable in four deterministic steps:

1. **Explicit `binaryPath`**: uses `CodeintClient(binaryPath: ...)` or `BinaryResolverOptions(binaryPath: ...)` when set.
2. **`CODEINT_BIN` environment variable**: uses the executable path in `CODEINT_BIN` when set.
3. **Version-matching `PATH`**: searches `PATH` for `codeint` (or `codeint.exe` on Windows) whose `codeint --version` matches `codeintVersion`.
4. **Verified GitHub Release cache**: downloads the platform archive and `SHA256SUMS` from `https://github.com/avidianity/codeint/releases/download/v<codeintVersion>` (or `releaseBaseUrl` / `CODEINT_RELEASE_BASE_URL`), verifies the SHA-256 digest before extracting, rejects unsafe archive paths and links, and installs atomically into the local user cache (`cacheDir`, `CODEINT_CACHE_DIR`, `%LOCALAPPDATA%\codeint`, `$XDG_CACHE_HOME/codeint`, or `~/.cache/codeint`).

For offline or air-gapped CI environments:

- Set `allowDownload: false` on `CodeintClient` or `BinaryResolverOptions` so resolution fails fast with `CodeintError` instead of attempting a network download.
- Point `CODEINT_BIN` (or `binaryPath`) at a pre-installed `codeint` binary, or pre-warm the verified cache during an online image-build step with `dart run codeint:install` or `await ensureBinary()` (`await client.ensureBinary()`):

```sh
dart run codeint:install
```

```dart
import 'package:codeint/codeint.dart';

Future<void> main() async {
  // Pre-warm during image build (online):
  final cachedPath = await ensureBinary();
  print('Cached binary: $cachedPath');

  // Run strictly offline in air-gapped CI:
  const offlineClient = CodeintClient(
    cwd: '.',
    allowDownload: false,
  );
  final checked = await offlineClient.check();
  print('Passed: ${checked.passed}');
}
```
