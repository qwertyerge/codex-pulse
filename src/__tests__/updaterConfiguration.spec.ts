import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const APPROVED_UPDATER_PUBLIC_KEY_SHA256 =
  "f914cff2593981637258bdfa0c35e64f8f7837d65dd4035b7393e4378a47db99";

function read(path: string) {
  return readFileSync(resolve(process.cwd(), path), "utf8");
}

function sha256(value: string) {
  return createHash("sha256").update(value, "utf8").digest("hex");
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
    expect(sha256(updater?.pubkey ?? "")).toBe(
      APPROVED_UPDATER_PUBLIC_KEY_SHA256
    );
  });

  it("rejects a different valid-looking updater identity", () => {
    const differentIdentity = "A".repeat(120);

    expect(sha256(differentIdentity)).not.toBe(
      APPROVED_UPDATER_PUBLIC_KEY_SHA256
    );
  });

  it("keeps the 0.4.1 release version aligned", () => {
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
      packageJson: "0.4.1",
      cargoToml: "0.4.1",
      cargoLock: "0.4.1",
      tauri: "0.4.1"
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

  it("pins the release action before GitHub API asset URLs were introduced", () => {
    const workflow = read(".github/workflows/release.yml");

    expect(workflow).toContain(
      "tauri-apps/tauri-action@84b9d35b5fc46c1e45415bdb6144030364f7ebc5"
    );
    expect(workflow).toContain("includeUpdaterJson: true");
    expect(workflow).not.toContain("tauri-apps/tauri-action@v1");
    expect(workflow).not.toContain("uploadUpdaterJson:");
    expect(workflow).not.toContain("uploadUpdaterSignatures:");
  });
});
