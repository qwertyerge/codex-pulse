import { describe, expect, it } from "vitest";
import { projectName } from "../lib/projectName";

describe("projectName", () => {
  it.each([
    ["/workspace/codex-pulse", "codex-pulse"],
    ["/workspace/codex-pulse/", "codex-pulse"],
    ["C:\\workspace\\codex-pulse", "codex-pulse"],
    ["C:\\workspace\\codex-pulse\\", "codex-pulse"],
    ["/", "/"],
    ["C:\\", "C:"]
  ])("derives the display label from %s", (cwd, expected) => {
    expect(projectName(cwd)).toBe(expected);
  });
});
