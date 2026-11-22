export interface CommonOptions {
  cwd?: string;
  binaryPath?: string;
}

export interface CheckOptions extends CommonOptions {
  path?: string;
  all?: boolean;
}

export interface DiffOptions extends CommonOptions {
  headRef?: string;
  all?: boolean;
}

export interface TrendOptions extends CommonOptions {
  since?: string;
}

export interface InitOptions extends CommonOptions {}

export interface ReferenceRange {
  low: number;
  high: number;
  source: string;
}

export interface MetricSummary {
  ratio: number;
  reference_range?: ReferenceRange;
  [key: string]: unknown;
}

export interface SourceSetReport {
  files: number;
  source_lines: number;
  erosion: MetricSummary;
  duplication: MetricSummary;
  verbosity: MetricSummary;
  import_cycles: MetricSummary;
  [key: string]: unknown;
}

export interface CheckReport {
  files: number;
  source_lines: number;
  source_sets: Partial<Record<"production" | "test" | "generated" | "vendored", SourceSetReport>>;
  skipped?: Array<{ path: string; reason: string }>;
  over_budget?: Array<Record<string, unknown>>;
  [key: string]: unknown;
}

export interface DiffReport {
  base: { ref: string; merge_base: string };
  head: string | { ref: string; commit: string };
  source_sets: Record<string, unknown>;
  over_budget?: Array<Record<string, unknown>>;
  regressions?: Array<Record<string, unknown>>;
  [key: string]: unknown;
}

export interface TrendReport {
  commits: number;
  source_sets: Record<string, unknown>;
  [key: string]: unknown;
}

export interface CheckResult {
  passed: boolean;
  exitCode: 0 | 1;
  failures: string[];
  report: CheckReport;
}

export interface DiffResult {
  passed: boolean;
  exitCode: 0 | 1;
  failures: string[];
  report: DiffReport;
}

export interface InitResult {
  configPath: string;
}

export declare class CodeintError extends Error {
  readonly exitCode: number;
  readonly stderr: string;
  constructor(message: string, exitCode: number, stderr: string);
}

export declare function resolveBinary(): string;
export declare function check(options?: CheckOptions): Promise<CheckResult>;
export declare function diff(baseRef: string, options?: DiffOptions): Promise<DiffResult>;
export declare function trend(options?: TrendOptions): Promise<TrendReport>;
export declare function init(options?: InitOptions): Promise<InitResult>;

declare const api: {
  CodeintError: typeof CodeintError;
  check: typeof check;
  diff: typeof diff;
  trend: typeof trend;
  init: typeof init;
  resolveBinary: typeof resolveBinary;
};

export default api;
