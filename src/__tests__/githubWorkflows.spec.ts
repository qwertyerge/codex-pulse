import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

function readWorkflow(name: string) {
  const path = resolve(process.cwd(), ".github/workflows", name);
  expect(existsSync(path), `${name} should exist`).toBe(true);
  return readFileSync(path, "utf8");
}

describe("GitHub workflows", () => {
  it("validates frontend and Rust changes on pull requests and main", () => {
    const workflow = readWorkflow("ci.yml");

    expect(workflow).toContain("name: CI");
    expect(workflow).toContain("pull_request:");
    expect(workflow).toContain("push:");
    expect(workflow).toContain("branches: [main]");
    expect(workflow).toContain("permissions:\n  contents: read");
    expect(workflow).toContain("name: Frontend");
    expect(workflow).toContain("runs-on: ubuntu-latest");
    expect(workflow).toContain("name: Rust");
    expect(workflow).toContain("runs-on: macos-15");
    expect(workflow).toContain("pnpm test");
    expect(workflow).toContain("pnpm build");
    expect(workflow).toContain("cargo test --manifest-path src-tauri/Cargo.toml");
  });

  it("creates only a guarded ARM64 draft release from an app version tag", () => {
    const workflow = readWorkflow("release.yml");

    expect(workflow).toContain('      - "app-v*"');
    expect(workflow).toContain("permissions:\n  contents: write");
    expect(workflow).toContain('expected_tag="app-v${app_version}"');
    expect(workflow).toContain('git merge-base --is-ancestor "$GITHUB_SHA" origin/main');
    expect(workflow).toContain('APPLE_SIGNING_IDENTITY: "-"');
    expect(workflow).toContain("releaseDraft: true");
    expect(workflow).toContain("uploadUpdaterJson: false");
    expect(workflow).toContain('args: "--target aarch64-apple-darwin --bundles dmg"');
  });
});
