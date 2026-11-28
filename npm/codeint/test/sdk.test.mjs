import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import {
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
} from "../lib/index.mjs";

const binaryPath = process.env.CODEINT_BIN;

function git(cwd, args) {
  execFileSync("git", args, {
    cwd,
    stdio: "ignore",
    env: { ...process.env, GIT_CONFIG_GLOBAL: "/dev/null", GIT_CONFIG_NOSYSTEM: "1" },
  });
}

function withTempDir(prefix, fn) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
  return Promise.resolve()
    .then(() => fn(dir))
    .finally(() => {
      fs.rmSync(dir, { recursive: true, force: true });
    });
}

function makeRepo(prefix = "codeint-js-sdk-") {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
  git(dir, ["init", "-b", "main"]);
  git(dir, ["config", "user.email", "dev@example.com"]);
  git(dir, ["config", "user.name", "Dev"]);
  fs.mkdirSync(path.join(dir, "src"), { recursive: true });
  fs.writeFileSync(
    path.join(dir, "src/leaf.ts"),
    "export function value(x: boolean): boolean {\n  return x;\n}\n"
  );
  fs.writeFileSync(
    path.join(dir, "src/app.ts"),
    "import { value } from './leaf';\nexport function ok(x: boolean): boolean {\n  return value(x);\n}\n"
  );
  git(dir, ["add", "."]);
  git(dir, ["commit", "-m", "initial"]);
  return dir;
}

function makeRepoWithCommits(commitCount, prefix = "codeint-js-sdk-trend-") {
  const dir = makeRepo(prefix);
  for (let i = 2; i <= commitCount; i += 1) {
    fs.writeFileSync(
      path.join(dir, "src/leaf.ts"),
      `export function value(x: boolean): boolean {\n  return x && ${i} > 0;\n}\n`
    );
    git(dir, ["add", "src/leaf.ts"]);
    git(dir, ["commit", "-m", `commit ${i}`]);
  }
  return dir;
}

function withRepo(fn, prefix) {
  const repo = makeRepo(prefix);
  return Promise.resolve()
    .then(() => fn(repo))
    .finally(() => {
      fs.rmSync(repo, { recursive: true, force: true });
    });
}

test("check, diff, trend, report, ratchetUpdate, baselineUpdate, and operational exit 2", async () => {
  assert.ok(binaryPath, "CODEINT_BIN must point to built codeint binary");
  assert.equal(resolveBinary(), path.resolve(binaryPath));

  await withRepo(async (repo) => {
    const okCheck = await check({ cwd: repo, binaryPath });
    assert.equal(okCheck.passed, true);
    assert.equal(okCheck.exitCode, 0);
    assert.equal(okCheck.report.files, 2);
    assert.ok(okCheck.report.source_sets.production);
    assert.equal(typeof okCheck.report.source_sets.production.erosion.metric_version, "number");

    const savedReport = path.join(repo, "saved-report.json");
    await check({ cwd: repo, binaryPath, output: savedReport });
    const markdown = await report(savedReport, { cwd: repo, binaryPath, format: "markdown", all: true });
    assert.ok(markdown.includes("codeint"));
    fs.rmSync(savedReport);

    const ratchet = await ratchetUpdate({ cwd: repo, binaryPath, reset: false });
    assert.equal(ratchet.created, true);
    assert.ok(ratchet.total_entries >= 1);

    const baseline = await baselineUpdate({ cwd: repo, binaryPath, reset: false });
    assert.equal(baseline.created, true);

    fs.writeFileSync(
      path.join(repo, "codeint.jsonc"),
      JSON.stringify({ budgets: { production: { verbosity: 0 } } })
    );
    fs.writeFileSync(
      path.join(repo, "src/app.ts"),
      "import { value } from './leaf';\nexport function bad(x: boolean): boolean {\n  if (value(x)) {\n    return true;\n  } else {\n    return false;\n  }\n}\n"
    );

    const failedCheck = await check({ cwd: repo, binaryPath, ignoreBaseline: true, all: true });
    assert.equal(failedCheck.passed, false);
    assert.equal(failedCheck.exitCode, 1);
    assert.ok(failedCheck.failures.length >= 1);

    const cacheDir = path.join(repo, ".cache");
    const failedDiff = await diff("HEAD", { cwd: repo, binaryPath, resultCache: cacheDir, all: true });
    assert.equal(failedDiff.passed, false);
    assert.equal(failedDiff.exitCode, 1);
    assert.ok(failedDiff.failures.length >= 1);

    const history = await trend({ cwd: repo, binaryPath, resultCache: cacheDir });
    assert.equal(history.commits, 1);
    assert.ok(history.source_sets.production);
  });

  await withTempDir("codeint-non-git-", async (outside) => {
    await assert.rejects(
      () => check({ cwd: outside, binaryPath }),
      (err) => err instanceof CodeintError && err.exitCode === 2
    );
  });
});

test("init supports jsonc, json, toml, yaml, and adopt without hardcoded fallback", async () => {
  const cases = [
    ["jsonc", "codeint.jsonc"],
    ["json", "codeint.json"],
    ["toml", "codeint.toml"],
    ["yaml", "codeint.yaml"],
  ];
  for (const [format, expectedName] of cases) {
    await withRepo(async (repo) => {
      const initialized = await init({ cwd: repo, binaryPath, format });
      assert.ok(initialized.configPath.endsWith(expectedName));
      assert.equal(fs.existsSync(initialized.configPath), true);
      assert.equal(initialized.adopted, undefined);
    });
  }

  for (const prefix of ["codeint-js-sdk-", "codeint-dir (123 copy)-"]) {
    await withRepo(async (repo) => {
      const adopted = await init({ cwd: repo, binaryPath, format: "yaml", adopt: true });
      assert.ok(adopted.configPath.endsWith("codeint.yaml"));
      assert.ok(adopted.adopted);
      assert.ok(adopted.adopted.ratchetPath.endsWith("ratchet.json"));
      assert.ok(adopted.adopted.baselinePath.endsWith("baseline.json"));
      assert.equal(fs.existsSync(adopted.adopted.ratchetPath), true);
      assert.equal(fs.existsSync(adopted.adopted.baselinePath), true);
      if (prefix.includes("(123 copy)")) {
        assert.ok(adopted.adopted.ratchetPath.includes("(123 copy)"));
        assert.ok(adopted.adopted.baselinePath.includes("(123 copy)"));
      }
    }, prefix);
  }
});

test("CodeintError preserves cause on direct and wrapped errors", async () => {
  const root = new Error("root failure");
  const wrapped = new CodeintError("wrapped failure", 2, "stderr", { cause: root });
  assert.equal(wrapped.cause, root);

  await withTempDir("codeint-missing-bin-", async (dir) => {
    await assert.rejects(
      () => check({ cwd: dir, binaryPath: path.join(dir, "missing-codeint") }),
      (err) => err instanceof CodeintError && err.exitCode === 2 && err.cause instanceof Error
    );
  });
});

test("impact, trace, guard, schema, config, doctor, churn, and coupling", async () => {
  await withRepo(async (repo) => {
    const blasted = await impact({ cwd: repo, binaryPath, paths: ["src/leaf.ts"], depth: 2 });
    assert.equal(blasted.targets.length, 1);
    assert.equal(blasted.summary.direct_dependents, 1);

    const traced = await trace("src/app.ts", "src/leaf.ts", { cwd: repo, binaryPath, all: true, maxPaths: 5 });
    assert.equal(traced.connected, true);
    assert.equal(traced.shortest_distance, 1);

    const guarded = await guard("src/app.ts", { cwd: repo, binaryPath });
    assert.equal(guarded.path, "src/app.ts");
    assert.equal(guarded.exists, true);

    const manifest = await schema({ cwd: repo, binaryPath });
    assert.equal(manifest.name, "codeint");
    assert.equal(manifest.metrics.length, 11);

    const resolved = await config({
      cwd: repo,
      binaryPath,
      pluginMemoryMb: 128,
      pluginTimeoutMs: 5000,
      budgets: ["production.erosion=0.2"],
    });
    assert.equal(resolved.config_file_present, false);
    assert.ok(resolved.settings.some((entry) => entry.key === "plugin_limits.memory_mb"));

    const healthyDoctor = await doctor({ cwd: repo, binaryPath });
    assert.equal(healthyDoctor.passed, true);
    assert.equal(healthyDoctor.exitCode, 0);
    assert.equal(healthyDoctor.report.summary.errors, 0);

    const churnReport = await churn({ cwd: repo, binaryPath, commits: 5, all: true });
    assert.equal(churnReport.commits_analyzed, 1);

    const couplingReport = await coupling({
      cwd: repo,
      binaryPath,
      commits: 5,
      maxCommitFiles: 50,
      minCoChanges: 1,
      minRatio: 0.1,
      includeTests: true,
    });
    assert.equal(couplingReport.commits_analyzed, 1);
    assert.equal(couplingReport.min_co_changes, 1);
  });

  await withTempDir("codeint-doctor-fail-", async (outside) => {
    fs.writeFileSync(path.join(outside, "codeint.json"), "{ invalid json");
    const brokenDoctor = await doctor({ cwd: outside, binaryPath });
    assert.equal(brokenDoctor.passed, false);
    assert.equal(brokenDoctor.exitCode, 1);
    assert.ok(brokenDoctor.report.summary.errors >= 1);
  });
});

test("agentStatus, agentInstall, and pluginTest gated exit 1 vs exit 2", async () => {
  await withTempDir("codeint-agent-home-", async (configRoot) => {
    const status = await agentStatus({ binaryPath, configRoot, client: "codex" });
    assert.equal(status.agents.length, 1);
    assert.equal(status.agents[0].client, "codex");

    const preview = await agentInstall({ binaryPath, configRoot, client: "codex", dryRun: true });
    assert.equal(preview.dry_run, true);
    assert.equal(preview.results.length, 1);
    assert.equal(preview.results[0].written, false);
  });

  await withTempDir("codeint-plugin-test-", async (dir) => {
    const emptyWasm = path.join(dir, "empty.wasm");
    fs.writeFileSync(emptyWasm, Buffer.from([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]));
    const failedPlugin = await pluginTest(emptyWasm, {
      binaryPath,
      kind: "language",
      pluginMemoryMb: 64,
      pluginTimeoutMs: 2000,
    });
    assert.equal(failedPlugin.passed, false);
    assert.equal(failedPlugin.exitCode, 1);
    assert.equal(failedPlugin.report.kind, "language");
    assert.ok(failedPlugin.report.summary.failed >= 1);

    await assert.rejects(
      () => pluginTest(path.join(dir, "missing.wasm"), { binaryPath }),
      (err) => err instanceof CodeintError && err.exitCode === 2
    );
  });
});

test("trend forwards commits option across below, at, and above 20 and rejects invalid combinations", async () => {
  const repo = makeRepoWithCommits(25);
  try {
    const five = await trend({ cwd: repo, binaryPath, commits: 5 });
    assert.equal(five.commits, 5);
    assert.equal(five.source_sets.production.length, 5);

    const twenty = await trend({ cwd: repo, binaryPath, commits: 20 });
    assert.equal(twenty.commits, 20);
    assert.equal(twenty.source_sets.production.length, 20);

    const twentyFive = await trend({ cwd: repo, binaryPath, commits: 25 });
    assert.equal(twentyFive.commits, 25);
    assert.equal(twentyFive.source_sets.production.length, 25);

    await assert.rejects(
      () => trend({ cwd: repo, binaryPath, commits: 0 }),
      (err) => err instanceof CodeintError && err.exitCode === 2
    );

    await assert.rejects(
      () => trend({ cwd: repo, binaryPath, since: "2020-01-01", commits: 5 }),
      (err) => err instanceof CodeintError && err.exitCode === 2
    );
  } finally {
    fs.rmSync(repo, { recursive: true, force: true });
  }
});

test("binary resolution failures throw and asynchronously reject with CodeintError exitCode 2 and cause", async () => {
  await withTempDir("codeint-bin-res-", async (dir) => {
    const missingOverride = path.join(dir, "missing-codeint");
    assert.throws(
      () => resolveBinary(missingOverride),
      (err) => err instanceof CodeintError && err.exitCode === 2 && err.cause instanceof Error
    );

    const overridePromise = check({ cwd: dir, binaryPath: missingOverride });
    assert.ok(overridePromise instanceof Promise);
    await assert.rejects(
      () => overridePromise,
      (err) => err instanceof CodeintError && err.exitCode === 2 && err.cause instanceof Error
    );

    const savedEnv = process.env.CODEINT_BIN;
    delete process.env.CODEINT_BIN;
    try {
      assert.throws(
        () => resolveBinary(),
        (err) => err instanceof CodeintError && err.exitCode === 2 && err.cause instanceof Error
      );

      for (const invoke of [
        () => check({ cwd: dir }),
        () => init({ cwd: dir }),
        () => trend({ cwd: dir }),
        () => report("missing.json", { cwd: dir }),
      ]) {
        const promise = invoke();
        assert.ok(promise instanceof Promise);
        await assert.rejects(
          () => promise,
          (err) => err instanceof CodeintError && err.exitCode === 2 && err.cause instanceof Error
        );
      }

      const originalPlatform = Object.getOwnPropertyDescriptor(process, "platform");
      Object.defineProperty(process, "platform", { value: "freebsd", configurable: true });
      try {
        assert.throws(
          () => resolveBinary(),
          (err) => err instanceof CodeintError && err.exitCode === 2
        );

        const unsupportedPromise = check({ cwd: dir });
        assert.ok(unsupportedPromise instanceof Promise);
        await assert.rejects(
          () => unsupportedPromise,
          (err) => err instanceof CodeintError && err.exitCode === 2
        );
      } finally {
        Object.defineProperty(process, "platform", originalPlatform);
      }
    } finally {
      if (savedEnv !== undefined) {
        process.env.CODEINT_BIN = savedEnv;
      }
    }

    const platformPkg = `@avidian/codeint-${process.platform}-${process.arch}`;
    const fakeNodeModules = path.join(dir, "node_modules");
    const fakePkgDir = path.join(fakeNodeModules, ...platformPkg.split("/"));
    fs.mkdirSync(fakePkgDir, { recursive: true });
    fs.writeFileSync(
      path.join(fakePkgDir, "package.json"),
      JSON.stringify({ name: platformPkg, version: "0.9.0" })
    );

    const sdkEntry = path.resolve(path.dirname(new URL(import.meta.url).pathname), "../lib/index.cjs");
    const childEnv = { ...process.env, NODE_PATH: fakeNodeModules };
    delete childEnv.CODEINT_BIN;
    const probeScript = [
      `const assert = require("node:assert/strict");`,
      `const { CodeintError, check, resolveBinary } = require(${JSON.stringify(sdkEntry)});`,
      `assert.throws(() => resolveBinary(), (err) => err instanceof CodeintError && err.exitCode === 2 && err.cause instanceof Error);`,
      `const p = check({ cwd: ${JSON.stringify(dir)} });`,
      `assert.ok(p instanceof Promise);`,
      `p.then(() => { process.exit(1); }, (err) => { assert.ok(err instanceof CodeintError && err.exitCode === 2 && err.cause instanceof Error); });`,
    ].join("\n");
    execFileSync(process.execPath, ["-e", probeScript], { env: childEnv, stdio: "pipe" });
  });
});
