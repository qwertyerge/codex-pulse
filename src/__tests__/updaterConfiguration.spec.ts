import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

function read(path: string) {
  return readFileSync(resolve(process.cwd(), path), "utf8");
}

describe("automatic updater configuration", () => {
  it("declares signed artifacts at the approved static endpoint", () => {
    const config = JSON.parse(read("src-tauri/tauri.conf.json")) as {
      version: string;
      bundle: { createUpdaterArtifacts?: boolean };
      plugins?: {
        updater?: {
          pubkey?: string;
          endpoints?: string[];
          windows?: { installMode?: string };
        };
      };
    };
    const updater = config.plugins?.updater;

    expect(config.version).toBe("0.3.2");
    expect(config.bundle.createUpdaterArtifacts).toBe(true);
    expect(updater?.endpoints).toEqual([
      "https://github.com/qwertyerge/codex-pulse/releases/latest/download/latest.json"
    ]);
    expect(updater?.windows).toEqual({ installMode: "passive" });
    expect(updater?.pubkey).toMatch(/^[A-Za-z0-9+/=]{100,}$/);
  });

  it("grants the precise updater surface to the main window", () => {
    const capability = JSON.parse(
      read("src-tauri/capabilities/default.json")
    ) as { permissions: string[] };

    expect(capability.permissions).toEqual([
      "core:default",
      "updater:allow-check",
      "updater:allow-download",
      "updater:allow-install",
      "dialog:allow-message",
      "process:allow-restart"
    ]);
    expect(capability.permissions).not.toContain("updater:default");
    expect(capability.permissions).not.toContain("dialog:default");
    expect(capability.permissions).not.toContain("process:default");
  });
});
