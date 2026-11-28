"use strict";

const {
  CodeintError,
  buildArgs,
  operationalError,
  runCommand,
} = require("./runner.cjs");

const WROTE_PREFIX = "Wrote ";
const COUNTED_WROTE_PATTERN = /^Wrote\s+(.+?)\s+\((\d+)\s+/;
const INIT_SPECS = [
  ["flag", "--adopt", "adopt"],
  ["opt", "--format", "format"],
];

function parseCountedLine(line, stderr, exitCode) {
  const match = COUNTED_WROTE_PATTERN.exec(line);
  if (!match) {
    throw new CodeintError(
      `codeint init --adopt returned unexpected output: ${line}`,
      exitCode,
      stderr
    );
  }
  return {
    filePath: match[1].trim(),
    count: Number.parseInt(match[2], 10),
  };
}

function parseInitOutput(stdout, adopt, stderr, exitCode) {
  const wroteLines = stdout
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.startsWith(WROTE_PREFIX));
  if (wroteLines.length === 0) {
    throw new CodeintError(
      "codeint init did not report a written configuration path",
      exitCode,
      stderr
    );
  }
  const configPath = wroteLines[0].slice(WROTE_PREFIX.length).trim();
  if (!adopt) {
    return { configPath };
  }
  if (wroteLines.length < 3) {
    throw new CodeintError(
      "codeint init --adopt did not report configuration, ratchet, and baseline paths",
      exitCode,
      stderr
    );
  }
  const ratchet = parseCountedLine(wroteLines[1], stderr, exitCode);
  const baseline = parseCountedLine(wroteLines[2], stderr, exitCode);
  return {
    configPath,
    adopted: {
      configPath,
      ratchetPath: ratchet.filePath,
      ratchetEntries: ratchet.count,
      baselinePath: baseline.filePath,
      baselinedFindings: baseline.count,
    },
  };
}

async function init(options) {
  const opts = options || {};
  const args = buildArgs(["init"], opts, INIT_SPECS);
  const result = await runCommand(args, opts);
  if (result.exitCode !== 0) {
    throw operationalError(result);
  }
  return parseInitOutput(result.stdout, Boolean(opts.adopt), result.stderr, result.exitCode);
}

module.exports = { init };
