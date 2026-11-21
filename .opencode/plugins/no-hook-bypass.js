import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const guardPath = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../../.claude/hooks/no-hook-bypass.sh",
);

function extractPatchPaths(patchText) {
  const paths = [];
  for (const line of String(patchText).split(/\r?\n/)) {
    const match = line.match(
      /^\*\*\* (?:(?:Add|Update|Delete) File|Move to):\s*(.+)$/,
    );
    if (match) {
      paths.push(match[1].trim());
    }
  }
  return paths;
}

function runGuard(toolInput) {
  const result = spawnSync("bash", [guardPath], {
    input: JSON.stringify({ tool_input: toolInput }),
    encoding: "utf8",
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    const message =
      (result.stderr || "").trim() ||
      "Blocked: this command skips Git hooks. Fix the hook failure and commit again; see docs/checks/no-hook-bypass.md.";
    throw new Error(message);
  }
}

export function checkToolCall(tool, args) {
  const toolArgs = args && typeof args === "object" ? args : {};
  if (tool === "bash") {
    runGuard({ command: String(toolArgs.command || "") });
    return;
  }
  if (tool === "write" || tool === "edit") {
    runGuard({ file_path: String(toolArgs.filePath || toolArgs.path || "") });
    return;
  }
  if (tool === "apply_patch") {
    for (const filePath of extractPatchPaths(
      toolArgs.patchText || toolArgs.patch || "",
    )) {
      runGuard({ file_path: filePath });
    }
  }
}

export default {
  id: "no-hook-bypass",
  async setup(ctx) {
    await ctx.tool.hook("execute.before", (event) => {
      const args = event?.args || event?.input || {};
      checkToolCall(event?.tool, args);
    });
  },
};
