export type SourceSetName = "production" | "test" | "generated" | "vendored";
export type InitFormat = "jsonc" | "json" | "toml" | "yaml";
export type ReportRenderFormat = "toon" | "text" | "json" | "sarif" | "markdown";
export type AgentClient = "claude-code" | "cursor" | "codex";
export type PluginKind = "language" | "metric";

export interface CommonOptions {
  cwd?: string;
  binaryPath?: string;
}

export interface OutputOptions {
  output?: string;
}

export interface ResultCacheOptions {
  resultCache?: string;
}

export interface PluginLimitOptions {
  pluginMemoryMb?: number;
  pluginTimeoutMs?: number;
}

export interface CheckOptions extends CommonOptions, OutputOptions {
  path?: string;
  all?: boolean;
  package?: string;
  owner?: string;
  coverageReports?: string[];
  ignoreBaseline?: boolean;
}

export interface DiffOptions extends CommonOptions, OutputOptions, ResultCacheOptions {
  headRef?: string;
  all?: boolean;
  package?: string;
  owner?: string;
}

export interface TrendOptions extends CommonOptions, OutputOptions, ResultCacheOptions {
  since?: string;
  commits?: number;
}

export interface ImpactOptions extends CommonOptions, OutputOptions {
  paths?: string[];
  changed?: string;
  depth?: number;
  package?: string;
}

export interface TraceOptions extends CommonOptions, OutputOptions {
  all?: boolean;
  maxPaths?: number;
  package?: string;
}

export interface InitOptions extends CommonOptions {
  format?: InitFormat;
  adopt?: boolean;
}

export interface GuardOptions extends CommonOptions {}

export interface SchemaOptions extends CommonOptions, OutputOptions {}

export interface ConfigOptions extends CommonOptions, PluginLimitOptions {
  path?: string;
  budgets?: string[];
  sourceSets?: string[];
}

export interface ReportOptions extends CommonOptions, OutputOptions {
  format?: ReportRenderFormat;
  all?: boolean;
}

export interface DoctorOptions extends CommonOptions {
  path?: string;
}

export interface AgentStatusOptions extends CommonOptions {
  client?: AgentClient;
  configRoot?: string;
}

export interface AgentInstallOptions extends AgentStatusOptions {
  dryRun?: boolean;
}

export interface ChurnOptions extends CommonOptions, OutputOptions {
  since?: string;
  commits?: number;
  all?: boolean;
}

export interface CouplingOptions extends ChurnOptions {
  maxCommitFiles?: number;
  minCoChanges?: number;
  minRatio?: number;
  includeTests?: boolean;
}

export interface StateUpdateOptions extends CommonOptions {
  path?: string;
  reset?: boolean;
  coverageReports?: string[];
}

export type RatchetUpdateOptions = StateUpdateOptions;
export type BaselineUpdateOptions = StateUpdateOptions;

export interface PluginTestOptions extends CommonOptions, PluginLimitOptions {
  kind?: PluginKind;
  sources?: string[];
}

export interface ReferenceRange {
  low: number;
  high: number;
  source: string;
}

export type ReferenceRangeEntry = string | ReferenceRange;

export interface ErodedFunctionFinding {
  id: string;
  path: string;
  line: number;
  end_line: number;
  name?: string;
  cyclomatic_complexity: number;
  cognitive_complexity: number;
  source_lines: number;
  function_mass: number;
  owners?: string[];
}

export interface ErosionSection {
  ratio: number;
  metric_version: number;
  reference_range: ReferenceRangeEntry;
  eroded_function_mass: number;
  total_function_mass: number;
  findings: ErodedFunctionFinding[];
  findings_left_out?: number;
  baselined_findings?: number;
}

export interface CognitiveComplexityFinding {
  id: string;
  path: string;
  line: number;
  end_line: number;
  name?: string;
  cognitive_complexity: number;
  cyclomatic_complexity: number;
  source_lines: number;
  owners?: string[];
}

export interface CognitiveComplexitySection {
  ratio: number;
  metric_version: number;
  reference_range: ReferenceRangeEntry;
  complex_cognitive_complexity: number;
  total_cognitive_complexity: number;
  findings: CognitiveComplexityFinding[];
  findings_left_out?: number;
  baselined_findings?: number;
}

export interface NearClonesSummary {
  min_similarity: number;
}

export interface CloneLocation {
  path: string;
  start_line: number;
  end_line: number;
}

export interface CloneGroupFinding {
  id: string;
  tokens: number;
  similarity?: number;
  locations: CloneLocation[];
  owners?: string[];
}

export interface CloneFamilyFinding {
  id: string;
  duplicated_lines: number;
  clone_group_ids: string[];
  locations: CloneLocation[];
  owners?: string[];
}

export interface DuplicationSection {
  ratio: number;
  metric_version: number;
  reference_range: ReferenceRangeEntry;
  near_clones?: NearClonesSummary;
  clone_lines: number;
  clone_groups: CloneGroupFinding[];
  findings_left_out?: number;
  baselined_findings?: number;
  clone_families?: CloneFamilyFinding[];
  clone_families_left_out?: number;
  unlisted_clone_family_group_ids?: string[];
}

export interface FlaggedLineFinding {
  id: string;
  path: string;
  line: number;
  rule: string;
  owners?: string[];
}

export interface VerbositySection {
  ratio: number;
  metric_version: number;
  reference_range: ReferenceRangeEntry;
  verbose_lines: number;
  flagged_lines: number;
  findings: FlaggedLineFinding[];
  findings_left_out?: number;
  baselined_findings?: number;
}

export interface ImportCycleFinding {
  id: string;
  modules: string[];
  owners?: string[];
}

export interface UnresolvedImportRow {
  importer: string;
  specifier: string;
  reason: string;
  owners?: string[];
}

export interface UnresolvedImportsSection {
  count: number;
  imports: UnresolvedImportRow[];
  imports_left_out?: number;
}

export interface ImportCyclesSection {
  ratio: number;
  metric_version: number;
  reference_range: ReferenceRangeEntry;
  modules_in_import_cycles: number;
  modules: number;
  findings: ImportCycleFinding[];
  findings_left_out?: number;
  baselined_findings?: number;
  unresolved_imports: UnresolvedImportsSection;
}

export type BoundaryFindingKind =
  | "zone_rule"
  | "external_package"
  | "public_files"
  | "unzoned_file";

export interface BoundaryFinding {
  id: string;
  kind: BoundaryFindingKind;
  importer: string;
  target?: string;
  external_package?: string;
  specifier?: string;
  source_zone?: string;
  target_zone?: string;
  reason?: string;
  message: string;
  owners?: string[];
}

export interface BoundariesSection {
  ratio: number;
  metric_version: number;
  resolved_imports: number;
  findings: BoundaryFinding[];
  findings_left_out?: number;
  baselined_findings?: number;
}

export interface OrphanFileFinding {
  id: string;
  path: string;
  kind: "unreferenced" | "test-only";
  test_importers?: string[];
  message: string;
  owners?: string[];
}

export interface OrphanFilesSection {
  ratio: number;
  metric_version: number;
  orphan_files: number;
  evaluated_files: number;
  entry_points: number;
  unreferenced: number;
  test_only: number;
  findings: OrphanFileFinding[];
  findings_left_out?: number;
  baselined_findings?: number;
}

export interface TestGapFinding {
  id: string;
  path: string;
  line: number;
  end_line: number;
  name?: string;
  function_mass: number;
  untested_share: number;
  owners?: string[];
}

export interface TestGapSection {
  ratio: number;
  metric_version: number;
  untested_function_mass: number;
  total_function_mass: number;
  measured_functions: number;
  findings: TestGapFinding[];
  findings_left_out?: number;
  baselined_findings?: number;
}

export interface HazardLineFinding {
  id: string;
  path: string;
  line: number;
  rule: string;
  owners?: string[];
}

export interface HazardsSection {
  ratio: number;
  metric_version: number;
  hazard_lines: number;
  source_lines: number;
  findings: HazardLineFinding[];
  findings_left_out?: number;
  baselined_findings?: number;
}

export interface ExternalMetricFinding {
  id: string;
  path: string;
  line: number;
  end_line?: number;
  message: string;
  owners?: string[];
}

export interface ExternalMetricSection {
  ratio: number;
  metric_version: number;
  reference_range: ReferenceRangeEntry;
  numerator: number;
  denominator: number;
  findings: ExternalMetricFinding[];
  findings_left_out?: number;
  baselined_findings?: number;
}

export interface PackageErosion {
  ratio: number;
  eroded_function_mass: number;
  total_function_mass: number;
}

export interface PackageCognitiveComplexity {
  ratio: number;
  complex_cognitive_complexity: number;
  total_cognitive_complexity: number;
}

export interface PackageDuplication {
  ratio: number;
  clone_lines: number;
}

export interface PackageVerbosity {
  ratio: number;
  verbose_lines: number;
}

export interface PackageImportCycles {
  ratio: number;
  modules_in_import_cycles: number;
}

export interface PackageTestGap {
  ratio: number;
  untested_function_mass: number;
  total_function_mass: number;
}

export interface PackageHazards {
  ratio: number;
  hazard_lines: number;
  source_lines: number;
}

export interface PackageSection {
  path: string;
  files: number;
  source_lines: number;
  framework_profiles?: string[];
  erosion: PackageErosion;
  cognitive_complexity: PackageCognitiveComplexity;
  duplication: PackageDuplication;
  verbosity: PackageVerbosity;
  import_cycles?: PackageImportCycles;
  test_gap?: PackageTestGap;
  hazards?: PackageHazards;
}

export interface SourceSetReport {
  files: number;
  source_lines: number;
  erosion: ErosionSection;
  cognitive_complexity: CognitiveComplexitySection;
  duplication: DuplicationSection;
  verbosity: VerbositySection;
  import_cycles?: ImportCyclesSection;
  boundaries?: BoundariesSection;
  orphan_files?: OrphanFilesSection;
  test_gap?: TestGapSection;
  hazards?: HazardsSection;
  packages?: PackageSection[];
  [customMetric: string]:
    | number
    | ErosionSection
    | CognitiveComplexitySection
    | DuplicationSection
    | VerbositySection
    | ImportCyclesSection
    | BoundariesSection
    | OrphanFilesSection
    | TestGapSection
    | HazardsSection
    | PackageSection[]
    | ExternalMetricSection
    | undefined;
}

export interface UnusedDependencyFinding {
  id: string;
  package: string;
  manifest: string;
  dependency: string;
  kind: string;
  owners?: string[];
}

export interface ImportedDependencyFinding {
  id: string;
  package: string;
  manifest: string;
  dependency: string;
  importers: string[];
  owners?: string[];
}

export interface UnusedDependenciesSection {
  ratio: number;
  metric_version: number;
  unused: number;
  declared: number;
  findings: UnusedDependencyFinding[];
  findings_left_out?: number;
  baselined_findings?: number;
}

export interface UnlistedDependenciesSection {
  ratio: number;
  metric_version: number;
  unlisted: number;
  imported: number;
  findings: ImportedDependencyFinding[];
  findings_left_out?: number;
  baselined_findings?: number;
}

export interface DevInProductionDependenciesSection {
  ratio: number;
  metric_version: number;
  dev_in_production: number;
  production_imported: number;
  findings: ImportedDependencyFinding[];
  findings_left_out?: number;
  baselined_findings?: number;
}

export interface DependenciesSection {
  unused_dependencies: UnusedDependenciesSection;
  unlisted_dependencies: UnlistedDependenciesSection;
  dev_in_production_dependencies: DevInProductionDependenciesSection;
}

export interface MeasuredSection {
  files: number;
  source_lines: number;
  framework_profiles?: string[];
  reference_range_note?: string;
  import_metrics_note?: string;
  source_sets: Partial<Record<SourceSetName, SourceSetReport>>;
  test_gap_note?: string;
  dependencies?: DependenciesSection;
}

export interface BaselineSummary {
  path: string;
  hidden_findings: number;
}

export interface SkippedFile {
  path: string;
  reason: string;
}

export interface FileDiagnosticEntry {
  path: string;
  line: number;
  kind: string;
  message: string;
}

export interface ScopeSummaryEntry {
  language: string;
  package?: string;
  dialect?: string;
  kind: string;
  fields?: Record<string, string>;
}

export interface BudgetFailure {
  source_set?: string;
  metric: string;
  ratio: number;
  budget: number;
  non_baselined?: boolean;
}

export interface RatchetFailure {
  source_set?: string;
  metric: string;
  ratio: number;
  ceiling: number;
}

export interface CheckReport extends MeasuredSection {
  coverage_unmatched_note?: string;
  ratchet_note?: string;
  manifest_warnings?: string[];
  baseline?: BaselineSummary;
  languages?: Record<string, MeasuredSection>;
  skipped?: SkippedFile[];
  file_diagnostics?: FileDiagnosticEntry[];
  scope_summaries?: ScopeSummaryEntry[];
  declaration_only?: string[];
  submodules?: string[];
  over_budget?: BudgetFailure[];
  over_ratchet?: RatchetFailure[];
}

export interface MetricChanges<TFinding> {
  base_ratio: number;
  head_ratio: number;
  metric_version: number;
  new: TFinding[];
  new_left_out?: number;
  resolved: TFinding[];
  resolved_left_out?: number;
  unchanged: number;
}

export interface CloneFamilyChanges {
  new: CloneFamilyFinding[];
  new_left_out?: number;
  resolved: CloneFamilyFinding[];
  resolved_left_out?: number;
  unchanged: number;
  unlisted_clone_family_group_ids?: string[];
}

export interface DuplicationChanges extends MetricChanges<CloneGroupFinding> {
  clone_families?: CloneFamilyChanges;
}

export type ExternalMetricChanges = MetricChanges<ExternalMetricFinding>;

export interface SourceSetChanges {
  erosion: MetricChanges<ErodedFunctionFinding>;
  cognitive_complexity: MetricChanges<CognitiveComplexityFinding>;
  duplication: DuplicationChanges;
  verbosity: MetricChanges<FlaggedLineFinding>;
  import_cycles?: MetricChanges<ImportCycleFinding>;
  boundaries?: MetricChanges<BoundaryFinding>;
  orphan_files?: MetricChanges<OrphanFileFinding>;
  hazards?: MetricChanges<HazardLineFinding>;
  [customMetric: string]:
    | MetricChanges<ErodedFunctionFinding>
    | MetricChanges<CognitiveComplexityFinding>
    | DuplicationChanges
    | MetricChanges<FlaggedLineFinding>
    | MetricChanges<ImportCycleFinding>
    | MetricChanges<BoundaryFinding>
    | MetricChanges<OrphanFileFinding>
    | MetricChanges<HazardLineFinding>
    | ExternalMetricChanges
    | undefined;
}

export interface DependenciesChanges {
  unused_dependencies: MetricChanges<UnusedDependencyFinding>;
  unlisted_dependencies: MetricChanges<ImportedDependencyFinding>;
  dev_in_production_dependencies: MetricChanges<ImportedDependencyFinding>;
}

export interface NewFileBudgetFailure {
  source_set: string;
  metric: string;
  ratio: number;
  budget: number;
  files: number;
}

export type RegressionRule =
  | "ratio_rise"
  | "new_findings"
  | "numerator_rise"
  | "per_file_numerator_rise"
  | "per_file_new_findings";

export interface Regression {
  source_set?: string;
  file?: string;
  metric: string;
  rule: RegressionRule;
  base: number;
  head: number;
  new_findings: number;
  ratio_rise_limit: number;
}

export interface DiffBaseEntry {
  ref: string;
  merge_base: string;
}

export type DiffHeadEntry = string | { ref: string; commit: string };

export interface DiffManifestWarnings {
  base?: string[];
  head?: string[];
}

export interface DiffReport {
  base: DiffBaseEntry;
  head: DiffHeadEntry;
  test_gap_note?: string;
  ratchet_note?: string;
  manifest_warnings?: DiffManifestWarnings;
  source_sets: Partial<Record<SourceSetName, SourceSetChanges>>;
  dependencies?: DependenciesChanges;
  over_budget?: BudgetFailure[];
  over_new_file_budget?: NewFileBudgetFailure[];
  over_ratchet?: RatchetFailure[];
  regressions?: Regression[];
}

export interface CommitManifestWarnings {
  commit: string;
  warnings: string[];
}

export interface TrendRow {
  commit: string;
  committed_on: string;
  files: number;
  erosion: number;
  cognitive_complexity: number;
  duplication: number;
  verbosity: number;
  import_cycles?: number;
  hazards?: number;
  [customMetric: string]: string | number | undefined;
}

export interface TrendReport {
  commits: number;
  metric_versions?: Record<string, number>;
  source_sets: Partial<Record<SourceSetName, TrendRow[]>>;
  test_gap_note?: string;
  manifest_warnings?: CommitManifestWarnings[];
}

export interface ImpactTargetRow {
  path: string;
  source_set: SourceSetName;
  package?: string;
  fan_in: number;
  fan_out: number;
  transitive_import_weight: number;
}

export interface ImpactSummaryRow {
  direct_dependents: number;
  transitive_dependents: number;
  total_dependents: number;
  fan_in: number;
  fan_out: number;
  transitive_import_weight: number;
}

export interface ImpactDependentRow {
  path: string;
  distance: number;
  package?: string;
}

export interface SourceSetImpact {
  direct: ImpactDependentRow[];
  transitive: ImpactDependentRow[];
}

export interface ImpactReport {
  targets: ImpactTargetRow[];
  changed_base?: DiffBaseEntry;
  depth_limit?: number;
  summary: ImpactSummaryRow;
  production: SourceSetImpact;
  test: SourceSetImpact;
  generated?: SourceSetImpact;
  vendored?: SourceSetImpact;
  affected_packages?: string[];
}

export interface TraceMultiFileImport {
  importer: string;
  specifier: string;
  targets: string[];
}

export interface TraceReport {
  from: string;
  to: string;
  from_entry_point?: string;
  to_entry_point?: string;
  connected: boolean;
  shortest_distance: number | null;
  paths: string[][];
  multi_file_imports?: TraceMultiFileImport[];
  truncated: boolean;
}

export interface AdoptedFiles {
  configPath: string;
  ratchetPath: string;
  ratchetEntries: number;
  baselinePath: string;
  baselinedFindings: number;
}

export interface InitResult {
  configPath: string;
  adopted?: AdoptedFiles;
}

export interface GuardAllowedTarget {
  zone: string;
  public_files?: string[];
  reason?: string;
}

export interface GuardDeniedTarget {
  zone: string;
  directive: string;
  reason?: string;
}

export interface GuardExternalImports {
  directive: string;
  patterns: string[];
  reason?: string;
}

export interface GuardReport {
  path: string;
  exists: boolean;
  configured: boolean;
  strict_zones: boolean;
  zone?: string;
  is_public_file?: boolean;
  public_files?: string[];
  allowed_targets: GuardAllowedTarget[];
  denied_targets: GuardDeniedTarget[];
  allowed_importers: string[];
  external_imports?: GuardExternalImports;
  note?: string;
}

export interface CliArgumentSchema {
  name: string;
  value_name: string;
  required: boolean;
  description?: string;
}

export interface CliFlagSchema {
  short?: string;
  long?: string;
  value_name?: string;
  takes_value: boolean;
  required: boolean;
  default_value?: string;
  possible_values?: string[];
  description?: string;
}

export interface CliCommandSchema {
  name: string;
  description?: string;
  arguments: CliArgumentSchema[];
  flags: CliFlagSchema[];
  subcommands?: CliCommandSchema[];
}

export interface MetricSchema {
  key: string;
  name: string;
  metric_version: number;
  short_description: string;
  description: string;
  unit: string;
  default_ratio_rise_limit?: number;
  default_new_findings_rule?: string;
}

export interface LanguageSchemaEntry {
  name: string;
  extensions: string[];
  hazard_rules?: string[];
  framework_profiles?: string[];
}

export interface OutputFormatSchema {
  name: string;
  commands: string[];
}

export interface McpToolSchema {
  name: string;
  description: string;
  inputSchema: Record<string, unknown>;
}

export interface ExitCodeSchema {
  code: number;
  name: string;
  description: string;
}

export interface SchemaReport {
  name: string;
  version: string;
  description: string;
  commands: CliCommandSchema[];
  metrics: MetricSchema[];
  configuration_keys: string[];
  source_sets: string[];
  languages: LanguageSchemaEntry[];
  output_formats: OutputFormatSchema[];
  mcp_tools: McpToolSchema[];
  exit_codes: ExitCodeSchema[];
}

export interface ResolvedSetting {
  key: string;
  value: unknown;
  source: "default" | "file" | "cli";
}

export interface ConfigReport {
  config_file: string;
  config_file_present: boolean;
  settings: ResolvedSetting[];
}

export type DoctorStatus = "pass" | "warn" | "error";

export interface DoctorSummary {
  passed: number;
  warnings: number;
  errors: number;
}

export interface DoctorCheck {
  id: string;
  status: DoctorStatus;
  summary: string;
  detail?: string;
  remedy?: string;
}

export interface DoctorReport {
  version: string;
  path: string;
  status: DoctorStatus;
  summary: DoctorSummary;
  checks: DoctorCheck[];
}

export interface AgentStatusEntry {
  client: string;
  name: string;
  config_path: string;
  detected: boolean;
  config_exists: boolean;
  registered: boolean;
}

export interface AgentStatusReport {
  config_root: string;
  agents: AgentStatusEntry[];
}

export interface AgentInstallEntry {
  client: string;
  name: string;
  config_path: string;
  action: "create" | "update" | "unchanged";
  written: boolean;
  preview?: string;
}

export interface AgentInstallReport {
  config_root: string;
  dry_run: boolean;
  results: AgentInstallEntry[];
  note?: string;
}

export interface ErodedChurnFile {
  path: string;
  commits: number;
  eroded_functions: number;
  eroded_function_mass: number;
  max_cyclomatic_complexity: number;
}

export interface ErosionChurnSection {
  ratio: number;
  eroded_function_mass: number;
  total_function_mass: number;
  findings: ErodedChurnFile[];
  findings_left_out?: number;
}

export interface DuplicatedChurnFile {
  path: string;
  commits: number;
  clone_lines: number;
  clone_groups: number;
}

export interface DuplicationChurnSection {
  ratio: number;
  clone_lines: number;
  findings: DuplicatedChurnFile[];
  findings_left_out?: number;
}

export interface SourceSetChurnSection {
  files: number;
  erosion: ErosionChurnSection;
  duplication: DuplicationChurnSection;
}

export interface ChurnReport {
  window: string;
  commits_analyzed: number;
  shallow: boolean;
  warning?: string;
  source_sets: Partial<Record<SourceSetName, SourceSetChurnSection>>;
}

export interface CoupledPairRow {
  file_a: string;
  file_b: string;
  co_change_count: number;
  file_a_commits: number;
  file_b_commits: number;
  coupling_ratio: number;
  has_direct_import: boolean;
}

export interface CouplingReport {
  window: string;
  commits_analyzed: number;
  commits_excluded_by_bulk_limit: number;
  max_commit_files: number;
  min_co_changes: number;
  min_ratio: number;
  source_sets: SourceSetName[];
  shallow: boolean;
  warning?: string;
  pairs: CoupledPairRow[];
  pairs_left_out?: number;
}

export interface RatchetUpdateReport {
  path: string;
  created: boolean;
  reset: boolean;
  replaced_invalid?: boolean;
  tightened: number;
  loosened: number;
  refreshed: number;
  added: number;
  unchanged: number;
  removed: number;
  total_entries: number;
}

export interface BaselineUpdateReport {
  path: string;
  created: boolean;
  reset: boolean;
  replaced_invalid?: boolean;
  added: number;
  removed: number;
  refreshed: number;
  skipped_new: number;
  total_findings: number;
}

export interface PluginTestSummary {
  passed: number;
  failed: number;
  skipped: number;
}

export interface PluginTestCheck {
  id: string;
  status: "pass" | "fail" | "skipped";
  message: string;
  hint?: string;
}

export interface PluginTestReport {
  plugin: string;
  kind: PluginKind;
  passed: boolean;
  summary: PluginTestSummary;
  checks: PluginTestCheck[];
}

export interface GatedResult<TReport> {
  passed: boolean;
  exitCode: 0 | 1;
  failures: string[];
  report: TReport;
}

export type CheckResult = GatedResult<CheckReport>;
export type DiffResult = GatedResult<DiffReport>;
export type DoctorResult = GatedResult<DoctorReport>;
export type PluginTestResult = GatedResult<PluginTestReport>;

export declare class CodeintError extends Error {
  readonly exitCode: number;
  readonly stderr: string;
  constructor(message: string, exitCode: number, stderr: string, options?: ErrorOptions);
}

export declare function resolveBinary(binaryPath?: string): string;
export declare function check(options?: CheckOptions): Promise<CheckResult>;
export declare function diff(baseRef: string, options?: DiffOptions): Promise<DiffResult>;
export declare function trend(options?: TrendOptions): Promise<TrendReport>;
export declare function impact(options: ImpactOptions): Promise<ImpactReport>;
export declare function trace(from: string, to: string, options?: TraceOptions): Promise<TraceReport>;
export declare function init(options?: InitOptions): Promise<InitResult>;
export declare function guard(targetPath: string, options?: GuardOptions): Promise<GuardReport>;
export declare function schema(options?: SchemaOptions): Promise<SchemaReport>;
export declare function config(options?: ConfigOptions): Promise<ConfigReport>;
export declare function report(input: string, options?: ReportOptions): Promise<string>;
export declare function doctor(options?: DoctorOptions): Promise<DoctorResult>;
export declare function agentStatus(options?: AgentStatusOptions): Promise<AgentStatusReport>;
export declare function agentInstall(options?: AgentInstallOptions): Promise<AgentInstallReport>;
export declare function churn(options?: ChurnOptions): Promise<ChurnReport>;
export declare function coupling(options?: CouplingOptions): Promise<CouplingReport>;
export declare function ratchetUpdate(options?: RatchetUpdateOptions): Promise<RatchetUpdateReport>;
export declare function baselineUpdate(options?: BaselineUpdateOptions): Promise<BaselineUpdateReport>;
export declare function pluginTest(pluginPath: string, options?: PluginTestOptions): Promise<PluginTestResult>;

declare const api: {
  CodeintError: typeof CodeintError;
  agentInstall: typeof agentInstall;
  agentStatus: typeof agentStatus;
  baselineUpdate: typeof baselineUpdate;
  check: typeof check;
  churn: typeof churn;
  config: typeof config;
  coupling: typeof coupling;
  diff: typeof diff;
  doctor: typeof doctor;
  guard: typeof guard;
  impact: typeof impact;
  init: typeof init;
  pluginTest: typeof pluginTest;
  ratchetUpdate: typeof ratchetUpdate;
  report: typeof report;
  resolveBinary: typeof resolveBinary;
  schema: typeof schema;
  trace: typeof trace;
  trend: typeof trend;
};

export default api;
