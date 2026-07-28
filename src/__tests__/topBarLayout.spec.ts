import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const stylesheet = readFileSync(resolve(process.cwd(), "src/styles.css"), "utf8");

function rule(selector: string) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return stylesheet.match(new RegExp(`${escaped}\\s*\\{([^}]*)\\}`))?.[1] ?? "";
}

describe("narrow TopBar layout", () => {
  it("protects controls and brand identity while allowing only the active count to shrink", () => {
    expect(rule(".top-bar__controls")).toContain("flex: 0 0 auto;");
    expect(rule(".top-bar__mark")).toContain("flex: 0 0 auto;");
    expect(rule(".top-bar__name")).toContain("flex: 0 0 auto;");
    expect(rule(".top-bar__count")).toContain("min-width: 0;");
    expect(rule(".top-bar__count")).toContain("flex: 0 1 auto;");
    expect(rule(".top-bar__count")).toContain("overflow: hidden;");
    expect(rule(".top-bar__count")).toContain("text-overflow: ellipsis;");
    expect(rule(".top-bar__count")).toContain("white-space: nowrap;");
    expect(rule(".top-bar .top-bar__update")).toContain("flex: 0 0 auto;");
    expect(rule(".top-bar .top-bar__update")).toContain("white-space: nowrap;");
  });

  it("keeps the controls anchored across the 360 pixel breakpoint", () => {
    const mediaStart = stylesheet.indexOf("@media (max-width: 360px)");
    const mediaEnd = stylesheet.indexOf("@keyframes pulse-dot");
    expect(mediaStart).toBeGreaterThanOrEqual(0);
    expect(mediaEnd).toBeGreaterThan(mediaStart);
    const narrowMedia = stylesheet.slice(mediaStart, mediaEnd);

    expect(narrowMedia).not.toContain(".pulse-shell { padding:");
    expect(narrowMedia).not.toContain(".top-bar {");
    expect(narrowMedia).not.toContain("\n  .top-bar__brand {");
    expect(narrowMedia).not.toContain(".top-bar button {");
    expect(narrowMedia).toContain(".top-bar--updating .top-bar__mark");
    expect(narrowMedia).toContain("display: none;");
    expect(narrowMedia).toContain(".top-bar--updating { gap: 6px;");
    expect(narrowMedia).toContain(
      ".top-bar--updating .top-bar__controls { gap: 3px;"
    );
  });
});
