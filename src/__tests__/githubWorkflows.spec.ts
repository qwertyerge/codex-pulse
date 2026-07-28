import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { describe, expect, it } from "vitest";
import { parse } from "yaml";

interface WorkflowStep {
  name?: string;
  uses?: string;
  shell?: string;
  run?: string;
  env?: Record<string, string>;
  with?: Record<string, unknown>;
}

interface WorkflowJob {
  name: string;
  "runs-on": string;
  needs?: string | string[];
  env?: Record<string, string>;
  strategy?: {
    "fail-fast": boolean;
    "max-parallel"?: number;
    matrix: {
      include: Array<{
        label: string;
        platform: string;
        target: string;
        bundles: string;
        "apple-signing-identity": string;
      }>;
    };
  };
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

function runBasicAuthHelper(token?: string) {
  const env = { ...process.env };
  if (token === undefined) {
    delete env.GITHUB_TOKEN;
  } else {
    env.GITHUB_TOKEN = token;
  }

  return spawnSync("bash", ["scripts/github-basic-auth.sh"], {
    cwd: process.cwd(),
    encoding: "utf8",
    env,
  });
}

describe("GitHub Basic Auth helper", () => {
  it("keeps the Bash helper on LF line endings for Windows checkouts", () => {
    const result = spawnSync(
      "git",
      ["check-attr", "eol", "--", "scripts/github-basic-auth.sh"],
      {
        cwd: process.cwd(),
        encoding: "utf8",
      },
    );

    expect(result.status).toBe(0);
    expect(result.error).toBeUndefined();
    expect(result.stderr).toBe("");
    expect(result.stdout.trim()).toBe(
      "scripts/github-basic-auth.sh: eol: lf",
    );
  });

  it("encodes a long token on one line and round trips the exact credential", () => {
    const token = `synthetic-${"a".repeat(256)}`;

    const result = runBasicAuthHelper(token);

    expect(result.status).toBe(0);
    expect(result.error).toBeUndefined();
    expect(result.stderr).toBe("");
    expect(result.stdout).not.toMatch(/[\r\n]/);
    expect(Buffer.from(result.stdout, "base64").toString("utf8")).toBe(
      `x-access-token:${token}`,
    );
    expect(result.stdout).not.toContain(token);
  });

  it("rejects missing and empty tokens without printing environment secrets", () => {
    const secretMarker = `private-${"z".repeat(64)}`;
    for (const token of [undefined, ""]) {
      const env: NodeJS.ProcessEnv = {
        ...process.env,
        RELEASE_TEST_SECRET: secretMarker,
      };
      if (token === undefined) {
        delete env.GITHUB_TOKEN;
      } else {
        env.GITHUB_TOKEN = token;
      }

      const result = spawnSync("bash", ["scripts/github-basic-auth.sh"], {
        cwd: process.cwd(),
        encoding: "utf8",
        env,
      });

      expect(result.status).not.toBe(0);
      expect(result.error).toBeUndefined();
      expect(result.stdout).toBe("");
      expect(result.stderr).toContain("GITHUB_TOKEN is required");
      expect(`${result.stdout}${result.stderr}`).not.toContain(secretMarker);
    }
  });
});

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

  it("creates guarded macOS and Windows draft artifacts from a SemVer tag", () => {
    const workflow = readWorkflow("release.yml");

    expect(workflow.name).toBe("Release");
    expect(workflow.on).toEqual({
      push: { tags: ["[0-9]*.[0-9]*.[0-9]*"] },
    });
    expect(workflow.permissions).toEqual({ contents: "write" });
    expect(Object.keys(workflow.jobs)).toEqual([
      "guard",
      "release",
      "verify_updater_manifest",
    ]);

    const guard = workflow.jobs.guard;
    expect(guard.name).toBe("Guard release source");
    expect(guard["runs-on"]).toBe("ubuntu-latest");
    expect(stepUsing(guard, "actions/checkout@v7").with).toEqual({
      "fetch-depth": 0,
      "persist-credentials": false,
    });

    const validation = stepNamed(guard, "Validate release source");
    expect(guard.steps.filter((step) => step.run)).toEqual([validation]);
    expect(validation.shell).toBe("bash");
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
    expect(validation.run).toContain(
      'package_version="$(jq -r \'.version\' package.json)"',
    );
    expect(validation.run).toContain(
      'tauri_version="$(jq -r \'.version\' src-tauri/tauri.conf.json)"',
    );
    expect(validation.run).toContain(
      String.raw`cargo_version="$(sed -n '/^\[package\]/,/^\[/s/^version = \"\([^\"]*\)\"/\1/p' src-tauri/Cargo.toml | head -n 1)"`,
    );
    for (const version of [
      "package_version",
      "tauri_version",
      "cargo_version",
    ]) {
      expect(validation.run).toContain(
        `"$${version}" != "$GITHUB_REF_NAME"`,
      );
    }
    expect(validation.run).not.toContain("app-v");
    expect(validation.run).toContain(
      'authorization="$(scripts/github-basic-auth.sh)"',
    );
    expect(validation.run).not.toMatch(
      /printf[^\n]*GITHUB_TOKEN[^\n]*\|\s*base64/,
    );
    expect(validation.run).toContain("AUTHORIZATION: basic $authorization");
    expect(validation.run).toContain(
      "fetch --no-tags origin main:refs/remotes/origin/main",
    );
    expect(validation.run).toContain(
      'git merge-base --is-ancestor "$GITHUB_SHA" origin/main',
    );
    expect(
      validation.run!.indexOf('"$cargo_version" != "$GITHUB_REF_NAME"'),
    ).toBeLessThan(validation.run!.indexOf("fetch --no-tags origin main"));
    expect(validation.run!.match(/extraheader/g)).toHaveLength(1);
    expect(validation.run).not.toContain("git config");

    const release = workflow.jobs.release;
    expect(release.name).toBe("Release (${{ matrix.label }})");
    expect(release["runs-on"]).toBe("${{ matrix.platform }}");
    expect(release.needs).toBe("guard");
    expect(release.strategy).toEqual({
      "fail-fast": false,
      "max-parallel": 1,
      matrix: {
        include: [
          {
            label: "macOS ARM64",
            platform: "macos-15",
            target: "aarch64-apple-darwin",
            bundles: "dmg",
            "apple-signing-identity": "-",
          },
          {
            label: "Windows x64",
            platform: "windows-latest",
            target: "x86_64-pc-windows-msvc",
            bundles: "nsis",
            "apple-signing-identity": "",
          },
        ],
      },
    });
    expect(stepUsing(release, "actions/checkout@v7").with).toEqual({
      "persist-credentials": false,
    });

    expect(stepUsing(release, "pnpm/action-setup@v6").with).toEqual({
      version: "10.33.0",
    });
    expect(stepUsing(release, "actions/setup-node@v7").with).toMatchObject({
      "node-version": 24,
      cache: "pnpm",
    });
    expect(stepUsing(release, "dtolnay/rust-toolchain@stable").with).toEqual({
      targets: "${{ matrix.target }}",
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
      APPLE_SIGNING_IDENTITY: "${{ matrix.apple-signing-identity }}",
      TAURI_SIGNING_PRIVATE_KEY:
        "${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}",
      TAURI_SIGNING_PRIVATE_KEY_PASSWORD:
        "${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}",
    });
    expect(build.with).toMatchObject({
      tagName: "${{ github.ref_name }}",
      releaseName: "Codex Pulse __VERSION__",
      releaseCommitish: "${{ github.sha }}",
      generateReleaseNotes: true,
      releaseDraft: true,
      prerelease: false,
      uploadUpdaterJson: true,
      uploadUpdaterSignatures: true,
      updaterJsonPreferNsis: true,
      args: "--target ${{ matrix.target }} --bundles ${{ matrix.bundles }}",
    });
    expect(release.strategy?.["max-parallel"]).toBe(1);

    const verification = workflow.jobs.verify_updater_manifest;
    expect(verification.name).toBe("Verify updater manifest");
    expect(verification["runs-on"]).toBe("ubuntu-latest");
    expect(verification.needs).toBe("release");
    expect(verification.env).toEqual({
      GH_TOKEN: "${{ secrets.GITHUB_TOKEN }}",
    });
    expect(stepUsing(verification, "actions/checkout@v7").with).toEqual({
      "persist-credentials": false,
    });

    const download = stepNamed(verification, "Download updater manifest");
    expect(download.shell).toBe("bash");
    expect(download.run).toContain(
      'mkdir -p "$RUNNER_TEMP/updater-manifest"',
    );
    expect(download.run).toContain(
      'gh release download "$GITHUB_REF_NAME"',
    );
    expect(download.run).toContain("--pattern latest.json");
    expect(download.run).toContain(
      '--dir "$RUNNER_TEMP/updater-manifest"',
    );
    expect(download.run).toContain("--clobber");

    const validateManifest = stepNamed(
      verification,
      "Validate updater manifest",
    );
    expect(validateManifest.run).toBe(
      'node scripts/verify-updater-manifest.mjs "$RUNNER_TEMP/updater-manifest/latest.json" "$GITHUB_REF_NAME"',
    );
  });
});
