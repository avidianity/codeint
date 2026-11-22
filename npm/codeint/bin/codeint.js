#!/usr/bin/env node
"use strict";

const { spawnSync } = require("node:child_process");
const { resolveBinary } = require("../lib/binary.cjs");

let binaryPath;
try {
  binaryPath = resolveBinary();
} catch (error) {
  process.stderr.write(`codeint: ${error.message}\n`);
  process.exit(2);
}

const result = spawnSync(binaryPath, process.argv.slice(2), {
  stdio: "inherit",
});

if (result.error) {
  process.stderr.write(
    `codeint: failed to execute ${binaryPath}: ${result.error.message}\n`
  );
  process.exit(2);
}

if (result.signal) {
  process.kill(process.pid, result.signal);
}

process.exit(result.status ?? 2);
