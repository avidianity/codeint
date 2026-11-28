import assert from "node:assert/strict";
import test from "node:test";

import {
  STICKY_MARKER,
  buildCommentBody,
  findStickyCommentId,
  parseExitCode,
} from "./action-comment.mjs";

test("buildCommentBody includes sticky marker and status", () => {
  const ok = buildCommentBody("| Metric | Base | Head |", 0);
  assert.ok(ok.startsWith(STICKY_MARKER));
  assert.ok(ok.includes("Passed (no Budget or Regression failures)"));
  assert.ok(ok.includes("| Metric | Base | Head |"));

  const failed = buildCommentBody("regression details", 1);
  assert.ok(failed.includes("Failed (Budget or Regression exceeded)"));
});

test("buildCommentBody truncates oversized reports cleanly", () => {
  const huge = "x".repeat(70000);
  const body = buildCommentBody(huge, 0);
  assert.ok(body.length < 61000);
  assert.ok(body.includes("Report truncated"));
});

test("findStickyCommentId finds existing marker or returns null", () => {
  assert.equal(
    findStickyCommentId([
      { id: 10, body: "unrelated comment" },
      { id: 42, body: `${STICKY_MARKER}\nold report` },
    ]),
    42
  );
  assert.equal(findStickyCommentId([{ id: 10, body: "plain" }]), null);
});

test("parseExitCode rejects empty or non-numeric values instead of treating them as 0", () => {
  assert.equal(parseExitCode("0"), 0);
  assert.equal(parseExitCode("1"), 1);
  assert.equal(parseExitCode("137"), 137);
  assert.equal(parseExitCode(""), null);
  assert.equal(parseExitCode("   "), null);
  assert.equal(parseExitCode(undefined), null);
  assert.equal(parseExitCode("abc"), null);
  assert.equal(parseExitCode("-1"), null);

  const missingBody = buildCommentBody("partial report", parseExitCode(""));
  assert.ok(missingBody.includes("Error (missing or invalid exit code)"));
  assert.ok(!missingBody.includes("Passed"));

  const crashBody = buildCommentBody("partial report", parseExitCode("137"));
  assert.ok(crashBody.includes("Error (exit code 137)"));
});
