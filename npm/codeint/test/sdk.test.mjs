import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { CodeintError, check, diff, init, trend } from "../lib/index.mjs";

const require = createRequire(import.meta.url);
const cjsApi = require("../lib/index.cjs");
const binaryPath = process.env.CODEINT_BIN;

function git(cwd, args) {
  execFileSync("git", args, { cwd, stdio: "ignore" });
}

function makeRepo() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "codeint-js-sdk-"));
  git(dir, ["init", "-b", "main"]);
  git(dir, ["config", "user.email", "dev@example.com"]);
  git(dir, ["config", "user.name", "Dev"]);
  fs.mkdirSync(path.join(dir, "src"), { recursive: true });
  fs.writeFileSync(
    path.join(dir, "src/app.ts"),
    "export function ok(x: boolean): boolean {\n  return x;\n}\n"
  );
  git(dir, ["add", "."]);
  git(dir, ["commit", "-m", "initial"]);
  return dir;
}

test("ESM and CJS exports expose check, diff, trend, init, and handle exit 1 vs exit 2", async () => {
  assert.ok(binaryPath, "CODEINT_BIN must point to built codeint binary");
  assert.equal(typeof cjsApi.check, "function");
  assert.equal(typeof cjsApi.resolveBinary, "function");

  const repo = makeRepo();
  try {
    const initialized = await init({ cwd: repo, binaryPath });
    assert.ok(initialized.configPath.endsWith("codeint.json"));

    const okCheck = await check({ cwd: repo, binaryPath });
    assert.equal(okCheck.passed, true);
    assert.equal(okCheck.exitCode, 0);
    assert.equal(okCheck.report.files, 1);

    fs.writeFileSync(
      path.join(repo, "codeint.json"),
      JSON.stringify({ budgets: { production: { verbosity: 0 } } })
    );
    fs.writeFileSync(
      path.join(repo, "src/app.ts"),
      "export function bad(x: boolean): boolean {\n  if (x) {\n    return true;\n  } else {\n    return false;\n  }\n}\n"
    );

    const failedCheck = await check({ cwd: repo, binaryPath });
    assert.equal(failedCheck.passed, false);
    assert.equal(failedCheck.exitCode, 1);
    assert.ok(failedCheck.failures.length >= 1);

    const failedDiff = await diff("HEAD", { cwd: repo, binaryPath });
    assert.equal(failedDiff.passed, false);
    assert.equal(failedDiff.exitCode, 1);
    assert.ok(failedDiff.failures.length >= 1);

    const history = await trend({ cwd: repo, binaryPath });
    assert.equal(history.commits, 1);
    assert.ok(history.source_sets && typeof history.source_sets === "object");

    const outside = fs.mkdtempSync(path.join(os.tmpdir(), "codeint-non-git-"));
    try {
      await assert.rejects(
        () => check({ cwd: outside, binaryPath }),
        (err) => err instanceof CodeintError && err.exitCode === 2
      );
    } finally {
      fs.rmSync(outside, { recursive: true, force: true });
    }
  } finally {
    fs.rmSync(repo, { recursive: true, force: true });
  }
});
