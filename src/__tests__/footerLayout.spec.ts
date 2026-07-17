import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const stylesheet = readFileSync(resolve(process.cwd(), "src/styles.css"), "utf8");

function rule(selector: string) {
  const match = stylesheet.match(new RegExp(`${selector.replace(/[.*+?^${}()|[\\]\\]/g, "\\$&")}\\s*\\{([^}]*)\\}`));
  return match?.[1] ?? "";
}

describe("footer layout", () => {
  it("ends the scroll viewport above the floating footer stack", () => {
    expect(rule(".session-list")).toContain("margin-bottom: var(--footer-stack-reserve);");
  });

  it("reserves only the currently visible footer height", () => {
    expect(rule(".pulse-shell")).toContain("--footer-stack-reserve: 48px;");
    expect(rule(".pulse-shell--background-refresh")).toContain("--footer-stack-reserve: 72px;");
  });

  it("hides the native scrollbar without reserving space and styles the localizable list end", () => {
    expect(rule(".session-list")).not.toContain("scrollbar-gutter");
    expect(rule(".session-list::-webkit-scrollbar")).toContain("display: none;");
    expect(rule(".session-list__end")).toContain("height: 1px;");
    expect(stylesheet).not.toContain('content: "END"');
  });

  it("aligns cards with the footer and leaves END a separator gap", () => {
    expect(rule(".session-list")).toContain("padding: 0;");
    const endMarker = rule(".session-list__end");
    expect(endMarker).toContain("height: 1px;");
    expect(endMarker).toContain("linear-gradient");
    expect(endMarker).toContain("transparent calc(50% - 22px)");
    expect(endMarker).toContain("transparent calc(50% + 22px)");
    expect(endMarker).not.toContain("border-top");
  });

  it("uses one strong glass surface behind the whole footer stack", () => {
    const footerStack = rule(".footer-stack");
    expect(footerStack).toContain("backdrop-filter: blur(32px)");
    expect(footerStack).toContain("padding:");
  });

  it("stretches the bottom-anchored footer only while a background event is visible", () => {
    expect(rule(".footer-stack")).toContain("max-height: 48px;");
    expect(rule(".footer-stack")).toContain("transition: max-height");
    expect(rule(".footer-stack--with-event")).toContain("max-height: 72px;");
    expect(stylesheet).toContain(".footer-status-enter-from");
    expect(stylesheet).toContain(".footer-status-leave-to");
  });
});
