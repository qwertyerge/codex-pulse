import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { parse } from "yaml";

interface WorkflowStep {
  name?: string;
  uses?: string;
  run?: string;
  env?: Record<string, string>;
  with?: Record<string, unknown>;
}

interface WorkflowJob {
  name: string;
  "runs-on": string;
  steps: WorkflowStep[];
}

interface Workflow {
  name: string;
  on: Record<string, { branches?: string[]; tags?: string[] }>;
  permissions: Record<string, string>;
  jobs: Record<string, WorkflowJob>;
}

function readWorkflow(name: string) {
  const path = resolve(process.cwd(), ".github/workflows", name);
  expect(existsSync(path), `${name} should exist`).toBe(true);
  return parse(readFileSync(path, "utf8")) as Workflow;
}

function stepUsing(job: WorkflowJob, action: string) {
  const step = job.steps.find((candidate) => candidate.uses === action);
  expect(step, `${job.name} should use ${action}`).toBeDefined();
  return step!;
}

function stepNamed(job: WorkflowJob, name: string) {
  const step = job.steps.find((candidate) => candidate.name === name);
  expect(step, `${job.name} should contain ${name}`).toBeDefined();
  return step!;
}

describe("GitHub workflows", () => {
  it("validates frontend and Rust changes on pull requests and main", () => {
    const workflow = readWorkflow("ci.yml");

    expect(workflow.name).toBe("CI");
    expect(workflow.on).toEqual({
      pull_request: { branches: ["main"] },
      push: { branches: ["main"] },
    });
    expect(workflow.permissions).toEqual({ contents: "read" });
    expect(Object.keys(workflow.jobs)).toEqual([
      "frontend",
      "rust",
      "rust_windows",
    ]);

    const frontend = workflow.jobs.frontend;
    expect(frontend.name).toBe("Frontend");
    expect(frontend["runs-on"]).toBe("ubuntu-latest");
    expect(stepUsing(frontend, "actions/checkout@v7").with).toEqual({
      "persist-credentials": false,
    });
    expect(stepUsing(frontend, "pnpm/action-setup@v6").with).toEqual({
      version: "10.33.0",
    });
    expect(stepUsing(frontend, "actions/setup-node@v7").with).toMatchObject({
      "node-version": 24,
      cache: "pnpm",
    });
    expect(frontend.steps.map((step) => step.run).filter(Boolean)).toEqual([
      "pnpm install --frozen-lockfile",
      "pnpm test",
      "pnpm build",
    ]);

    const rust = workflow.jobs.rust;
    expect(rust.name).toBe("Rust");
    expect(rust["runs-on"]).toBe("macos-15");
    expect(stepUsing(rust, "actions/checkout@v7").with).toEqual({
      "persist-credentials": false,
    });
    stepUsing(rust, "dtolnay/rust-toolchain@stable");
    stepUsing(rust, "Swatinem/rust-cache@v2");
    expect(rust.steps.map((step) => step.run).filter(Boolean)).toEqual([
      "cargo test --manifest-path src-tauri/Cargo.toml",
    ]);

    const rustWindows = workflow.jobs.rust_windows;
    expect(rustWindows.name).toBe("Rust (Windows)");
    expect(rustWindows["runs-on"]).toBe("windows-latest");
    expect(stepUsing(rustWindows, "actions/checkout@v7").with).toEqual({
      "persist-credentials": false,
    });
    expect(stepUsing(rustWindows, "pnpm/action-setup@v6").with).toEqual({
      version: "10.33.0",
    });
    expect(
      stepUsing(rustWindows, "actions/setup-node@v7").with,
    ).toMatchObject({
      "node-version": 24,
      cache: "pnpm",
    });
    expect(
      stepUsing(rustWindows, "dtolnay/rust-toolchain@stable").with,
    ).toEqual({
      targets: "x86_64-pc-windows-msvc",
    });
    stepUsing(rustWindows, "Swatinem/rust-cache@v2");
    expect(rustWindows.steps.map((step) => step.run).filter(Boolean)).toEqual([
      "pnpm install --frozen-lockfile",
      "pnpm test",
      "pnpm build",
      "cargo test --manifest-path src-tauri/Cargo.toml",
      "pnpm tauri build --target x86_64-pc-windows-msvc --bundles nsis",
      "pwsh -NoProfile -File scripts/verify-windows-package.ps1 -BundleDirectory src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis",
    ]);
  });

  it("creates only a guarded ARM64 draft release from a SemVer tag", () => {
    const workflow = readWorkflow("release.yml");

    expect(workflow.name).toBe("Release");
    expect(workflow.on).toEqual({
      push: { tags: ["[0-9]*.[0-9]*.[0-9]*"] },
    });
    expect(workflow.permissions).toEqual({ contents: "write" });
    expect(Object.keys(workflow.jobs)).toEqual(["release"]);

    const release = workflow.jobs.release;
    expect(release.name).toBe("Release");
    expect(release["runs-on"]).toBe("macos-15");
    expect(stepUsing(release, "actions/checkout@v7").with).toEqual({
      "fetch-depth": 0,
      "persist-credentials": false,
    });

    const validation = stepNamed(release, "Validate release source");
    expect(validation.env).toEqual({
      GITHUB_TOKEN: "${{ secrets.GITHUB_TOKEN }}",
    });
    expect(validation.run).toContain("semver_pattern=");
    expect(validation.run).toContain(
      '[[ ! "$GITHUB_REF_NAME" =~ $semver_pattern ]]',
    );
    const semverPattern = validation.run?.match(/semver_pattern='([^']+)'/)?.[1];
    expect(semverPattern).toBeDefined();
    const semver = new RegExp(semverPattern!);
    expect(
      ["0.1.0", "1.2.3-alpha.1+build.5"].every((tag) => semver.test(tag)),
    ).toBe(true);
    expect(
      [
        "v0.1.0",
        "app-v0.1.0",
        "01.2.3",
        "1.02.3",
        "1.2.03",
        "1.2",
        "1.2.3-01",
      ].some((tag) => semver.test(tag)),
    ).toBe(false);
    expect(validation.run).toContain('expected_tag="$app_version"');
    expect(validation.run).not.toContain("app-v");
    expect(validation.run).toContain("AUTHORIZATION: basic $authorization");
    expect(validation.run).toContain(
      "fetch --no-tags origin main:refs/remotes/origin/main",
    );
    expect(validation.run).toContain(
      'git merge-base --is-ancestor "$GITHUB_SHA" origin/main',
    );

    expect(stepUsing(release, "pnpm/action-setup@v6").with).toEqual({
      version: "10.33.0",
    });
    expect(stepUsing(release, "actions/setup-node@v7").with).toMatchObject({
      "node-version": 24,
      cache: "pnpm",
    });
    expect(stepUsing(release, "dtolnay/rust-toolchain@stable").with).toEqual({
      targets: "aarch64-apple-darwin",
    });
    stepUsing(release, "Swatinem/rust-cache@v2");
    expect(release.steps.map((step) => step.run).filter(Boolean)).toEqual(
      expect.arrayContaining([
        "pnpm install --frozen-lockfile",
        "pnpm test",
        "cargo test --manifest-path src-tauri/Cargo.toml",
      ]),
    );

    const build = stepUsing(release, "tauri-apps/tauri-action@v1");
    expect(build.env).toEqual({
      GITHUB_TOKEN: "${{ secrets.GITHUB_TOKEN }}",
      APPLE_SIGNING_IDENTITY: "-",
    });
    expect(build.with).toMatchObject({
      tagName: "${{ github.ref_name }}",
      releaseName: "Codex Pulse __VERSION__",
      releaseCommitish: "${{ github.sha }}",
      generateReleaseNotes: true,
      releaseDraft: true,
      prerelease: false,
      uploadUpdaterJson: false,
      args: "--target aarch64-apple-darwin --bundles dmg",
    });
  });
});
