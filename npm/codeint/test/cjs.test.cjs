"use strict";

const assert = require("node:assert/strict");
const test = require("node:test");
const api = require("../lib/index.cjs");

const EXPECTED_EXPORTS = [
  "CodeintError",
  "agentInstall",
  "agentStatus",
  "baselineUpdate",
  "check",
  "churn",
  "config",
  "coupling",
  "diff",
  "doctor",
  "guard",
  "impact",
  "init",
  "pluginTest",
  "ratchetUpdate",
  "report",
  "resolveBinary",
  "schema",
  "trace",
  "trend",
];

test("CommonJS require exports all 18 operations, CodeintError, and resolveBinary", async () => {
  const binaryPath = process.env.CODEINT_BIN;
  assert.ok(binaryPath, "CODEINT_BIN must point to built codeint binary");
  for (const name of EXPECTED_EXPORTS) {
    assert.equal(typeof api[name], "function", `CJS export ${name} must be a function`);
  }
  assert.equal(api.resolveBinary(), binaryPath);
  const manifest = await api.schema();
  assert.equal(manifest.name, "codeint");
  assert.ok(Array.isArray(manifest.metrics));
  assert.equal(manifest.metrics.length, 11);
});
