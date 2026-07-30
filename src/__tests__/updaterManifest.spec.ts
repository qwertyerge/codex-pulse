import {
  mkdtempSync,
  rmSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { afterEach, describe, expect, it } from "vitest";

const directories: string[] = [];

type PlatformName = "darwin-aarch64" | "windows-x86_64";
type ManifestField = "url" | "signature";
interface ManifestFixture {
  version: string;
  notes: string;
  pub_date: string;
  platforms: Record<
    PlatformName,
    { url: string; signature: string }
  >;
}

function runManifest(manifest: unknown, version = "0.4.0") {
  const directory = mkdtempSync(join(tmpdir(), "codex-pulse-updater-"));
  directories.push(directory);
  const path = join(directory, "latest.json");
  writeFileSync(path, JSON.stringify(manifest));
  return spawnSync(
    process.execPath,
    [
      resolve(process.cwd(), "scripts/verify-updater-manifest.mjs"),
      path,
      version
    ],
    { encoding: "utf8" }
  );
}

function validManifest(): ManifestFixture {
  return {
    version: "0.4.0",
    notes: "Synthetic test fixture",
    pub_date: "2026-07-28T00:00:00Z",
    platforms: {
      "darwin-aarch64": {
        url:
          "https://github.com/qwertyerge/codex-pulse/releases/download/0.4.0/Codex.Pulse.app.tar.gz",
        signature: "mac-signature"
      },
      "windows-x86_64": {
        url:
          "https://github.com/qwertyerge/codex-pulse/releases/download/0.4.0/Codex.Pulse-setup.exe",
        signature: "windows-signature"
      }
    }
  };
}

afterEach(() => {
  for (const directory of directories.splice(0)) {
    rmSync(directory, { recursive: true, force: true });
  }
});

describe("updater manifest validator", () => {
  it("accepts the exact version and both signed platforms", () => {
    const result = runManifest(validManifest());

    expect(result.status).toBe(0);
    expect(result.stdout).toContain(
      "Validated updater manifest 0.4.0 for darwin-aarch64, windows-x86_64"
    );
    expect(result.stderr).toBe("");
  });

  it("rejects a version that does not match the tag", () => {
    const result = runManifest(validManifest(), "0.4.1");

    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain(
      "version 0.4.0 does not match tag 0.4.1"
    );
  });

  it.each([
    ["darwin-aarch64", "url"],
    ["darwin-aarch64", "signature"],
    ["windows-x86_64", "url"],
    ["windows-x86_64", "signature"]
  ] satisfies ReadonlyArray<readonly [PlatformName, ManifestField]>)(
    "rejects missing %s %s",
    (platform, field) => {
      const manifest = validManifest();
      manifest.platforms[platform][field] = "";
      const result = runManifest(manifest);

      expect(result.status).not.toBe(0);
      expect(result.stderr).toContain(`${platform}.${field} must be non-empty`);
    }
  );

  it("rejects GitHub API asset URLs that consume anonymous API quota", () => {
    const manifest = validManifest();
    manifest.platforms["windows-x86_64"].url =
      "https://api.github.com/repos/qwertyerge/codex-pulse/releases/assets/495045117";

    const result = runManifest(manifest);

    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain(
      "windows-x86_64.url must use the public GitHub release download URL"
    );
  });

  it("rejects a release download URL for a different tag", () => {
    const manifest = validManifest();
    manifest.platforms["windows-x86_64"].url =
      "https://github.com/qwertyerge/codex-pulse/releases/download/0.4.1/Codex.Pulse-setup.exe";

    const result = runManifest(manifest);

    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain(
      "windows-x86_64.url must use the public GitHub release download URL"
    );
  });
});
