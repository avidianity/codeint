import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const STICKY_MARKER = "<!-- codeint-pr-comment -->";
const MAX_BODY_CHARS = 60000;

export function buildCommentBody(markdown, exitCode) {
  const status =
    exitCode === 0
      ? "Passed (no Budget or Regression failures)"
      : exitCode === 1
        ? "Failed (Budget or Regression exceeded)"
        : `Error (exit code ${exitCode})`;
  const trimmed = markdown.trim();
  const content =
    trimmed.length > MAX_BODY_CHARS
      ? `${trimmed.slice(0, MAX_BODY_CHARS)}\n\n_(Report truncated to fit GitHub comment size limits.)_`
      : trimmed;
  return `${STICKY_MARKER}\n## codeint Software entropy diff\n\n**Status:** ${status}\n\n${content}\n`;
}

export function findStickyCommentId(comments) {
  const existing = comments.find(
    (item) => typeof item?.body === "string" && item.body.includes(STICKY_MARKER)
  );
  return existing ? existing.id : null;
}

async function githubRequest(url, token, options = {}) {
  const response = await fetch(url, {
    ...options,
    headers: {
      Accept: "application/vnd.github+json",
      Authorization: `Bearer ${token}`,
      "User-Agent": "codeint-action",
      "X-GitHub-Api-Version": "2022-11-28",
      ...options.headers,
    },
  });
  if (!response.ok) {
    throw new Error(`GitHub API ${response.status} for ${url}`);
  }
  return response.json();
}

async function runFromEnv() {
  const token = process.env.GITHUB_TOKEN;
  const repo = process.env.GITHUB_REPOSITORY;
  const prNumber = process.env.PR_NUMBER;
  const reportFile = process.env.REPORT_FILE;
  const exitCode = Number(process.env.DIFF_EXIT_CODE ?? "0");
  if (!token || !repo || !prNumber || !reportFile || !fs.existsSync(reportFile)) {
    return;
  }
  const markdown = fs.readFileSync(reportFile, "utf8");
  const body = buildCommentBody(markdown, exitCode);
  const apiBase = process.env.GITHUB_API_URL || "https://api.github.com";
  const listUrl = `${apiBase}/repos/${repo}/issues/${prNumber}/comments?per_page=100`;
  const comments = await githubRequest(listUrl, token);
  const existingId = findStickyCommentId(comments);
  if (existingId) {
    await githubRequest(`${apiBase}/repos/${repo}/issues/comments/${existingId}`, token, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ body }),
    });
  } else {
    await githubRequest(`${apiBase}/repos/${repo}/issues/${prNumber}/comments`, token, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ body }),
    });
  }
}

const isEntrypoint =
  process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (isEntrypoint) {
  await runFromEnv();
}
