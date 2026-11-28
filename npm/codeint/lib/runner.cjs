"use strict";

const { spawn } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");
const { resolveBinary } = require("./binary.cjs");

class CodeintError extends Error {
  constructor(message, exitCode, stderr, options) {
    super(message, options);
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

function buildSpawnEnv(options, cwd) {
  if (!options.resultCache) {
    return process.env;
  }
  return {
    ...process.env,
    CODEINT_CACHE_DIR: path.resolve(cwd, options.resultCache),
  };
}

function runCommand(args, options) {
  const opts = options || {};
  const binary = resolveBinary(opts.binaryPath);
  const cwd = opts.cwd || process.cwd();
  const env = buildSpawnEnv(opts, cwd);
  return new Promise((resolve, reject) => {
    const child = spawn(binary, args, {
      cwd,
      env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    const outChunks = [];
    const errChunks = [];
    child.stdout.on("data", (chunk) => outChunks.push(chunk));
    child.stderr.on("data", (chunk) => errChunks.push(chunk));
    child.on("error", (err) => {
      reject(new CodeintError(`failed to execute ${binary}: ${err.message}`, 2, "", { cause: err }));
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

function operationalError(result) {
  const message = result.stderr.trim() || `codeint exited with code ${result.exitCode}`;
  return new CodeintError(message, result.exitCode, result.stderr);
}

function readOutputPayload(result, options) {
  const opts = options || {};
  if (!opts.output) {
    return result.stdout;
  }
  const cwd = opts.cwd || process.cwd();
  const outputPath = path.resolve(cwd, opts.output);
  try {
    return fs.readFileSync(outputPath, "utf8");
  } catch (error) {
    throw new CodeintError(
      `failed to read output file ${outputPath}: ${error.message}`,
      result.exitCode,
      result.stderr,
      { cause: error }
    );
  }
}

function parseJsonReport(payload, stderr, exitCode) {
  try {
    return JSON.parse(payload);
  } catch (error) {
    throw new CodeintError(
      `codeint returned invalid JSON (${error.message})`,
      exitCode,
      stderr,
      { cause: error }
    );
  }
}

async function runGatedCommand(args, options) {
  const result = await runCommand(args, options);
  if (result.exitCode !== 0 && result.exitCode !== 1) {
    throw operationalError(result);
  }
  const payload = readOutputPayload(result, options);
  return {
    passed: result.exitCode === 0,
    exitCode: result.exitCode,
    failures: result.exitCode === 1 ? parseFailureLines(result.stderr) : [],
    report: parseJsonReport(payload, result.stderr, result.exitCode),
  };
}

async function runJsonCommand(args, options) {
  const result = await runCommand(args, options);
  if (result.exitCode !== 0) {
    throw operationalError(result);
  }
  const payload = readOutputPayload(result, options);
  return parseJsonReport(payload, result.stderr, result.exitCode);
}

async function runTextCommand(args, options) {
  const result = await runCommand(args, options);
  if (result.exitCode !== 0) {
    throw operationalError(result);
  }
  return readOutputPayload(result, options);
}

const ARG_HANDLERS = {
  flag(args, flag, value) {
    if (value) {
      args.push(flag);
    }
  },
  opt(args, flag, value) {
    if (value !== undefined && value !== null) {
      args.push(flag, String(value));
    }
  },
  rep(args, flag, values) {
    if (!Array.isArray(values)) {
      return;
    }
    for (const value of values) {
      args.push(flag, String(value));
    }
  },
  pos(args, _flag, value) {
    if (value) {
      args.push(String(value));
    }
  },
  spread(args, _flag, values) {
    if (Array.isArray(values)) {
      args.push(...values);
    }
  },
};

function buildArgs(baseArgs, options, specs) {
  const opts = options || {};
  const args = [...baseArgs];
  for (const [kind, flag, key] of specs) {
    ARG_HANDLERS[kind](args, flag, opts[key]);
  }
  return args;
}

module.exports = {
  CodeintError,
  buildArgs,
  operationalError,
  runCommand,
  runGatedCommand,
  runJsonCommand,
  runTextCommand,
};
