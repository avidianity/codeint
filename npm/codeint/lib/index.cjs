"use strict";

const { spawn } = require("node:child_process");
const path = require("node:path");
const { resolveBinary } = require("./binary.cjs");

class CodeintError extends Error {
  constructor(message, exitCode, stderr) {
    super(message);
    this.name = "CodeintError";
    this.exitCode = exitCode;
    this.stderr = stderr;
  }
}

function parseFailureLines(stderr) {
  return stderr
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
    .map((line) => line.replace(/^codeint:\s*/, ""));
}

function runCommand(args, options) {
  const opts = options || {};
  const binary = opts.binaryPath ? path.resolve(opts.binaryPath) : resolveBinary();
  const cwd = opts.cwd || process.cwd();
  return new Promise((resolve, reject) => {
    const child = spawn(binary, args, {
      cwd,
      stdio: ["ignore", "pipe", "pipe"],
    });
    const outChunks = [];
    const errChunks = [];
    child.stdout.on("data", (chunk) => outChunks.push(chunk));
    child.stderr.on("data", (chunk) => errChunks.push(chunk));
    child.on("error", (err) => {
      reject(new CodeintError(`failed to execute ${binary}: ${err.message}`, 2, ""));
    });
    child.on("close", (code) => {
      resolve({
        exitCode: code ?? 2,
        stdout: Buffer.concat(outChunks).toString("utf8"),
        stderr: Buffer.concat(errChunks).toString("utf8"),
      });
    });
  });
}

function parseJsonReport(stdout, stderr, exitCode) {
  try {
    return JSON.parse(stdout);
  } catch (error) {
    throw new CodeintError(
      `codeint returned invalid JSON (${error.message})`,
      exitCode,
      stderr
    );
  }
}

function assertGateExit(result) {
  if (result.exitCode === 0 || result.exitCode === 1) {
    return;
  }
  const message = result.stderr.trim() || `codeint exited with code ${result.exitCode}`;
  throw new CodeintError(message, result.exitCode, result.stderr);
}

async function runGatedReport(args, opts) {
  const result = await runCommand(args, opts);
  assertGateExit(result);
  return {
    passed: result.exitCode === 0,
    exitCode: result.exitCode,
    failures: result.exitCode === 1 ? parseFailureLines(result.stderr) : [],
    report: parseJsonReport(result.stdout, result.stderr, result.exitCode),
  };
}

async function check(options) {
  const opts = options || {};
  const args = ["check", "--format", "json"];
  if (opts.all) {
    args.push("--all");
  }
  if (opts.path) {
    args.push(opts.path);
  }
  return runGatedReport(args, opts);
}

async function diff(baseRef, options) {
  const opts = options || {};
  const args = ["diff", baseRef, "--format", "json"];
  if (opts.headRef) {
    args.push("--head", opts.headRef);
  }
  if (opts.all) {
    args.push("--all");
  }
  return runGatedReport(args, opts);
}

async function trend(options) {
  const opts = options || {};
  const args = ["trend", "--format", "json"];
  if (opts.since) {
    args.push("--since", opts.since);
  }
  const result = await runCommand(args, opts);
  if (result.exitCode !== 0) {
    const message = result.stderr.trim() || `codeint exited with code ${result.exitCode}`;
    throw new CodeintError(message, result.exitCode, result.stderr);
  }
  return parseJsonReport(result.stdout, result.stderr, result.exitCode);
}

async function init(options) {
  const opts = options || {};
  const result = await runCommand(["init"], opts);
  if (result.exitCode !== 0) {
    const message = result.stderr.trim() || `codeint exited with code ${result.exitCode}`;
    throw new CodeintError(message, result.exitCode, result.stderr);
  }
  const raw = result.stdout.trim().replace(/^Wrote\s+/, "");
  const configPath = raw || path.join(opts.cwd || process.cwd(), "codeint.json");
  return { configPath };
}

module.exports = {
  CodeintError,
  check,
  diff,
  trend,
  init,
  resolveBinary,
};
