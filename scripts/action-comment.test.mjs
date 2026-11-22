import assert from "node:assert/strict";
import test from "node:test";

import {
  STICKY_MARKER,
  buildCommentBody,
  findStickyCommentId,
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
