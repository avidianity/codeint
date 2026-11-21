#!/usr/bin/env node
"use strict";

const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const PLATFORMS = {
  "darwin-arm64": { pkg: "@avidian/codeint-darwin-arm64", bin: "bin/codeint" },
  "darwin-x64": { pkg: "@avidian/codeint-darwin-x64", bin: "bin/codeint" },
  "linux-arm64": { pkg: "@avidian/codeint-linux-arm64", bin: "bin/codeint" },
  "linux-x64": { pkg: "@avidian/codeint-linux-x64", bin: "bin/codeint" },
  "win32-arm64": { pkg: "@avidian/codeint-win32-arm64", bin: "bin/codeint.exe" },
  "win32-x64": { pkg: "@avidian/codeint-win32-x64", bin: "bin/codeint.exe" },
};

const platformKey = `${process.platform}-${process.arch}`;
const target = PLATFORMS[platformKey];

if (!target) {
  const supported = Object.keys(PLATFORMS).join(", ");
  process.stderr.write(
    `codeint: unsupported platform "${platformKey}". Supported platforms: ${supported}.\n`
  );
  process.exit(2);
}

let packageJsonPath;
try {
  packageJsonPath = require.resolve(`${target.pkg}/package.json`);
} catch (error) {
  process.stderr.write(
    `codeint: missing platform package "${target.pkg}" for ${platformKey} (${error.message}). Reinstall codeint with optional dependencies enabled.\n`
  );
  process.exit(2);
}

const binaryPath = path.join(path.dirname(packageJsonPath), target.bin);
if (!fs.existsSync(binaryPath)) {
  process.stderr.write(
    `codeint: native binary not found at ${binaryPath} (package "${target.pkg}").\n`
  );
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
