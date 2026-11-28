"use strict";

const { resolveBinary } = require("./binary.cjs");
const { init } = require("./init.cjs");
const {
  CodeintError,
  buildArgs,
  runGatedCommand,
  runJsonCommand,
  runTextCommand,
} = require("./runner.cjs");

const REPORT_SCOPE_SPECS = [
  ["opt", "--package", "package"],
  ["opt", "--owner", "owner"],
  ["opt", "--output", "output"],
  ["flag", "--all", "all"],
];

const HISTORY_SPECS = [
  ["opt", "--since", "since"],
  ["opt", "--commits", "commits"],
  ["opt", "--output", "output"],
  ["flag", "--all", "all"],
];

const PLUGIN_LIMIT_SPECS = [
  ["opt", "--plugin-memory-mb", "pluginMemoryMb"],
  ["opt", "--plugin-timeout-ms", "pluginTimeoutMs"],
];

const AGENT_SPECS = [
  ["opt", "--client", "client"],
  ["opt", "--config-root", "configRoot"],
];

const STATE_UPDATE_SPECS = [
  ["flag", "--reset", "reset"],
  ["rep", "--coverage-report", "coverageReports"],
  ["pos", "", "path"],
];

const CHECK_SPECS = [
  ...REPORT_SCOPE_SPECS,
  ["rep", "--coverage-report", "coverageReports"],
  ["flag", "--ignore-baseline", "ignoreBaseline"],
  ["pos", "", "path"],
];

const DIFF_SPECS = [["opt", "--head", "headRef"], ...REPORT_SCOPE_SPECS];

const TREND_SPECS = [
  ["opt", "--since", "since"],
  ["opt", "--output", "output"],
];

const IMPACT_SPECS = [
  ["opt", "--changed", "changed"],
  ["opt", "--depth", "depth"],
  ["opt", "--package", "package"],
  ["opt", "--output", "output"],
  ["spread", "", "paths"],
];

const TRACE_SPECS = [
  ["flag", "--all", "all"],
  ["opt", "--max-paths", "maxPaths"],
  ["opt", "--package", "package"],
  ["opt", "--output", "output"],
];

const CONFIG_SPECS = [
  ...PLUGIN_LIMIT_SPECS,
  ["rep", "--budget", "budgets"],
  ["rep", "--source-set", "sourceSets"],
  ["pos", "", "path"],
];

const REPORT_SPECS = [
  ["flag", "--all", "all"],
  ["opt", "--format", "format"],
  ["opt", "--output", "output"],
];

const COUPLING_SPECS = [
  ...HISTORY_SPECS,
  ["opt", "--max-commit-files", "maxCommitFiles"],
  ["opt", "--min-co-changes", "minCoChanges"],
  ["opt", "--min-ratio", "minRatio"],
  ["flag", "--include-tests", "includeTests"],
];

const PLUGIN_TEST_SPECS = [
  ["opt", "--kind", "kind"],
  ["rep", "--source", "sources"],
  ...PLUGIN_LIMIT_SPECS,
];

function check(options) {
  return runGatedCommand(buildArgs(["check", "--format", "json"], options, CHECK_SPECS), options);
}

function diff(baseRef, options) {
  return runGatedCommand(
    buildArgs(["diff", baseRef, "--format", "json"], options, DIFF_SPECS),
    options
  );
}

function trend(options) {
  return runJsonCommand(buildArgs(["trend", "--format", "json"], options, TREND_SPECS), options);
}

function impact(options) {
  return runJsonCommand(buildArgs(["impact", "--format", "json"], options, IMPACT_SPECS), options);
}

function trace(from, to, options) {
  return runJsonCommand(
    buildArgs(["trace", from, to, "--format", "json"], options, TRACE_SPECS),
    options
  );
}

function guard(targetPath, options) {
  return runJsonCommand(["guard", targetPath, "--format", "json"], options);
}

function schema(options) {
  return runJsonCommand(
    buildArgs(["schema", "--format", "json"], options, [["opt", "--output", "output"]]),
    options
  );
}

function config(options) {
  return runJsonCommand(
    buildArgs(["config", "--resolved", "--format", "json"], options, CONFIG_SPECS),
    options
  );
}

function report(input, options) {
  return runTextCommand(buildArgs(["report", input], options, REPORT_SPECS), options);
}

function doctor(options) {
  return runGatedCommand(
    buildArgs(["doctor", "--format", "json"], options, [["pos", "", "path"]]),
    options
  );
}

function agentStatus(options) {
  return runJsonCommand(
    buildArgs(["agent", "status", "--format", "json"], options, AGENT_SPECS),
    options
  );
}

function agentInstall(options) {
  return runJsonCommand(
    buildArgs(
      ["agent", "install", "--format", "json"],
      options,
      [["flag", "--dry-run", "dryRun"], ...AGENT_SPECS]
    ),
    options
  );
}

function churn(options) {
  return runJsonCommand(buildArgs(["churn", "--format", "json"], options, HISTORY_SPECS), options);
}

function coupling(options) {
  return runJsonCommand(
    buildArgs(["coupling", "--format", "json"], options, COUPLING_SPECS),
    options
  );
}

function ratchetUpdate(options) {
  return runJsonCommand(
    buildArgs(["ratchet", "update", "--format", "json"], options, STATE_UPDATE_SPECS),
    options
  );
}

function baselineUpdate(options) {
  return runJsonCommand(
    buildArgs(["baseline", "update", "--format", "json"], options, STATE_UPDATE_SPECS),
    options
  );
}

function pluginTest(pluginPath, options) {
  return runGatedCommand(
    buildArgs(["plugin", "test", pluginPath, "--format", "json"], options, PLUGIN_TEST_SPECS),
    options
  );
}

module.exports = {
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
};
