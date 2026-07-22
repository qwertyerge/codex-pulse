import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const stylesheet = readFileSync(resolve(process.cwd(), "src/styles.css"), "utf8");

function rule(selector: string) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return stylesheet.match(new RegExp(`${escaped}\\s*\\{([^}]*)\\}`))?.[1] ?? "";
}

describe("theme control states", () => {
  it("keeps selected controls white on blue even while hovered", () => {
    const light = rule('.top-bar__theme-group button[aria-pressed="true"]:hover');
    const dark = rule(':root[data-theme="dark"] .top-bar__theme-group button[aria-pressed="true"]:hover');
    expect(light).toContain("color: #fff;");
    expect(light).toContain("background: #3478f6;");
    expect(dark).toContain("color: #fff;");
    expect(dark).toContain("background: #3478f6;");
  });

  it("applies scheme-specific hover surfaces only to unselected controls", () => {
    expect(rule('.top-bar__theme-group button[aria-pressed="false"]:hover'))
      .toContain("background: rgba(52, 120, 246, 0.14);");
    expect(rule(':root[data-theme="dark"] .top-bar__theme-group button[aria-pressed="false"]:hover'))
      .toContain("background: rgba(138, 194, 255, 0.18);");
    expect(stylesheet).not.toContain(".top-bar button:hover { background:");
  });

  it("limits locale hover styling to the trigger button", () => {
    expect(rule(".top-bar__locale > button:hover"))
      .toContain("background: rgba(255, 255, 255, 0.78);");
    expect(rule(':root[data-theme="dark"] .top-bar__locale > button:hover'))
      .toContain("background: rgba(47, 65, 98, 0.72);");
    expect(stylesheet).not.toContain(".top-bar__locale button:hover");
  });
});
