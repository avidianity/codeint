import api, {
  CodeintError,
  agentInstall,
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
import type {
  AgentInstallReport,
  AgentStatusReport,
  BaselineUpdateReport,
  CheckResult,
  ChurnReport,
  ConfigReport,
  CouplingReport,
  DependenciesSection,
  DiffResult,
  DoctorResult,
  ExternalMetricSection,
  GuardReport,
  ImpactReport,
  InitResult,
  PluginTestResult,
  RatchetUpdateReport,
  SchemaReport,
  SourceSetReport,
  TraceReport,
  TrendReport,
} from "codeint";

function verifySourceSetMetrics(prod: SourceSetReport, deps: DependenciesSection): number[] {
  const custom = prod["custom_metric"] as ExternalMetricSection | undefined;
  return [
    prod.erosion.ratio,
    prod.erosion.metric_version,
    prod.cognitive_complexity.ratio,
    prod.duplication.ratio,
    prod.verbosity.ratio,
    prod.import_cycles ? prod.import_cycles.ratio : 0,
    prod.boundaries ? prod.boundaries.ratio : 0,
    prod.orphan_files ? prod.orphan_files.ratio : 0,
    prod.test_gap ? prod.test_gap.ratio : 0,
    prod.hazards ? prod.hazards.ratio : 0,
    custom ? custom.numerator : 0,
    deps.unused_dependencies.ratio,
    deps.unlisted_dependencies.ratio,
    deps.dev_in_production_dependencies.ratio,
  ];
}

async function verifyCoreTypes(): Promise<void> {
  const binary: string = resolveBinary();
  const checked: CheckResult = await check({
    cwd: ".",
    binaryPath: binary,
    path: "src",
    all: true,
    package: ".",
    owner: "@team",
    coverageReports: ["lcov.info"],
    ignoreBaseline: true,
  });
  void checked.report.scope_summaries;
  void checked.report.file_diagnostics;
  void checked.report.baseline;
  void checked.report.over_ratchet;

  const diffed: DiffResult = await diff("HEAD", {
    headRef: "HEAD",
    all: true,
    package: ".",
    owner: "@team",
    resultCache: ".cache",
  });
  void diffed.report.over_new_file_budget;
  void diffed.report.regressions;

  const history: TrendReport = await trend({ since: "2026-01-01", resultCache: ".cache" });
  const initialized: InitResult = await init({ format: "yaml", adopt: true });
  void [history.metric_versions, initialized.adopted];
}

async function verifyExtendedTypes(): Promise<void> {
  const blasted: ImpactReport = await impact({ paths: ["src/a.ts"], depth: 2, package: "." });
  const traced: TraceReport = await trace("src/a.ts", "src/b.ts", { all: true, maxPaths: 5 });
  const guarded: GuardReport = await guard("src/a.ts");
  const manifest: SchemaReport = await schema();
  const resolved: ConfigReport = await config({
    pluginMemoryMb: 128,
    pluginTimeoutMs: 5000,
    budgets: ["production.erosion=0.1"],
    sourceSets: ["test=tests/**"],
  });
  const rendered: string = await report("report.json", { format: "markdown", all: true });
  const diagnosed: DoctorResult = await doctor({ path: "." });
  const status: AgentStatusReport = await agentStatus({ client: "codex", configRoot: "/tmp" });
  const installed: AgentInstallReport = await agentInstall({
    client: "claude-code",
    dryRun: true,
    configRoot: "/tmp",
  });
  const churned: ChurnReport = await churn({ since: "30d", all: true });
  const coupled: CouplingReport = await coupling({
    commits: 10,
    maxCommitFiles: 50,
    minCoChanges: 2,
    minRatio: 0.5,
    includeTests: true,
  });
  const ratcheted: RatchetUpdateReport = await ratchetUpdate({ reset: true, coverageReports: [] });
  const baselined: BaselineUpdateReport = await baselineUpdate({ reset: false });
  const tested: PluginTestResult = await pluginTest("plugin.wasm", {
    kind: "language",
    sources: ["src/a.ts"],
    pluginMemoryMb: 64,
    pluginTimeoutMs: 1000,
  });
  const err = new CodeintError("failed", 2, "stderr");
  void [
    api.check,
    blasted,
    traced,
    guarded,
    manifest,
    resolved,
    rendered,
    diagnosed,
    status,
    installed,
    churned,
    coupled,
    ratcheted,
    baselined,
    tested,
    err.exitCode,
  ];
}

void [verifySourceSetMetrics, verifyCoreTypes, verifyExtendedTypes];
