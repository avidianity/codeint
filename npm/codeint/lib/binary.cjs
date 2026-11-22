"use strict";

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

function resolveBinary() {
  const platformKey = `${process.platform}-${process.arch}`;
  const target = PLATFORMS[platformKey];
  if (!target) {
    const supported = Object.keys(PLATFORMS).join(", ");
    throw new Error(
      `unsupported platform "${platformKey}". Supported platforms: ${supported}.`
    );
  }
  let packageJsonPath;
  try {
    packageJsonPath = require.resolve(`${target.pkg}/package.json`);
  } catch (error) {
    throw new Error(
      `missing platform package "${target.pkg}" for ${platformKey} (${error.message}). Reinstall codeint with optional dependencies enabled.`
    );
  }
  const binaryPath = path.join(path.dirname(packageJsonPath), target.bin);
  if (!fs.existsSync(binaryPath)) {
    throw new Error(
      `native binary not found at ${binaryPath} (package "${target.pkg}").`
    );
  }
  return binaryPath;
}

module.exports = { resolveBinary };
