import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

function read(path: string) {
  return readFileSync(resolve(process.cwd(), path), "utf8");
}

function packageVersionFromCargoToml(contents: string) {
  const packageStart = contents.indexOf("[package]");
  expect(
    packageStart,
    "Cargo.toml should contain [package]"
  ).toBeGreaterThanOrEqual(0);
  const nextSection = contents.indexOf(
    "\n[",
    packageStart + "[package]".length
  );
  const packageSection = contents.slice(
    packageStart,
    nextSection === -1 ? contents.length : nextSection
  );
  const version = packageSection.match(/^version = "([^"]+)"$/m);
  expect(
    version,
    "Cargo.toml [package] should declare version"
  ).not.toBeNull();
  return version![1];
}

function codexPulseVersionFromCargoLock(contents: string) {
  const packageBlock = contents
    .split("[[package]]")
    .find((candidate) => /^name = "codex-pulse"$/m.test(candidate));
  expect(
    packageBlock,
    "Cargo.lock should contain codex-pulse"
  ).toBeDefined();
  const version = packageBlock!.match(/^version = "([^"]+)"$/m);
  expect(
    version,
    "Cargo.lock codex-pulse should declare version"
  ).not.toBeNull();
  return version![1];
}

describe("automatic updater configuration", () => {
  it("declares signed artifacts at the approved static endpoint", () => {
    const config = JSON.parse(read("src-tauri/tauri.conf.json")) as {
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

    expect(config.bundle.createUpdaterArtifacts).toBe(true);
    expect(updater?.endpoints).toEqual([
      "https://github.com/qwertyerge/codex-pulse/releases/latest/download/latest.json"
    ]);
    expect(updater?.windows).toEqual({ installMode: "passive" });
    expect(updater?.pubkey).toMatch(/^[A-Za-z0-9+/=]{100,}$/);
  });

  it("keeps the bootstrap release version aligned", () => {
    const packageJson = JSON.parse(read("package.json")) as { version: string };
    const tauri = JSON.parse(read("src-tauri/tauri.conf.json")) as {
      version: string;
    };

    expect({
      packageJson: packageJson.version,
      cargoToml: packageVersionFromCargoToml(read("src-tauri/Cargo.toml")),
      cargoLock: codexPulseVersionFromCargoLock(read("src-tauri/Cargo.lock")),
      tauri: tauri.version
    }).toEqual({
      packageJson: "0.4.0",
      cargoToml: "0.4.0",
      cargoLock: "0.4.0",
      tauri: "0.4.0"
    });
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
