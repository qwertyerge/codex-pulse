# GitHub Actions and Draft Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add reproducible pull-request validation and an Apple Silicon macOS draft-release pipeline, then prove the workflows on GitHub and protect `main` with the successful checks.

**Architecture:** Keep validation and publishing in separate workflows. `ci.yml` validates frontend work on Linux and Rust/Tauri work on an Apple Silicon macOS runner. `release.yml` accepts only an exact `app-v<tauri-version>` tag whose commit is contained in `origin/main`, repeats the tests, and uses `tauri-apps/tauri-action` to build one ad-hoc-signed ARM64 DMG into a draft GitHub Release.

**Tech Stack:** GitHub Actions, Node.js 24, pnpm 10.33.0, Vitest, Vue TypeScript build, stable Rust, Cargo, Tauri 2, GitHub CLI, GitHub GraphQL API.

## Global Constraints

- Preserve the existing branch-protection settings; only enable strict required checks named `Frontend` and `Rust` after both checks have succeeded on a pull request and the merged `main` commit.
- Do not add Apple certificates, notarization credentials, updater JSON, Intel builds, or universal binaries.
- Leave the release as a draft. Publishing it is outside this plan.
- Do not reuse local Codex data or add secrets to the repository.
- Use the exact current action majors approved in the design: `actions/checkout@v7`, `actions/setup-node@v7`, `pnpm/action-setup@v6`, `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`, and `tauri-apps/tauri-action@v1`.
- Treat GitHub-hosted execution as the source of truth. Local YAML tests prove the repository contract, not runner availability or release success.

---

### Task 1: Add failing workflow contract tests

**Files:**
- Create: `src/__tests__/githubWorkflows.spec.ts`
- Test: `src/__tests__/githubWorkflows.spec.ts`

- [ ] **Step 1: Create a static workflow contract test**

Create `src/__tests__/githubWorkflows.spec.ts` with two tests. The helper must assert that the requested file exists before reading it so the initial failure clearly identifies the missing workflow.

```ts
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
```

- [ ] **Step 2: Run the focused test and record RED**

Run:

```bash
pnpm exec vitest run src/__tests__/githubWorkflows.spec.ts
```

Expected: both tests fail at the existence assertion, reporting `ci.yml should exist` and `release.yml should exist`.

---

### Task 2: Implement the CI workflow

**Files:**
- Create: `.github/workflows/ci.yml`
- Test: `src/__tests__/githubWorkflows.spec.ts`

- [ ] **Step 1: Create `.github/workflows/ci.yml`**

```yaml
name: CI

on:
  pull_request:
    branches: [main]
  push:
    branches: [main]

permissions:
  contents: read

concurrency:
  group: ci-${{ github.workflow }}-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: true

jobs:
  frontend:
    name: Frontend
    runs-on: ubuntu-latest
    steps:
      - name: Check out repository
        uses: actions/checkout@v7
      - name: Install pnpm
        uses: pnpm/action-setup@v6
        with:
          version: 10.33.0
      - name: Set up Node.js
        uses: actions/setup-node@v7
        with:
          node-version: 24
          cache: pnpm
          cache-dependency-path: pnpm-lock.yaml
      - name: Install frontend dependencies
        run: pnpm install --frozen-lockfile
      - name: Test frontend
        run: pnpm test
      - name: Build frontend
        run: pnpm build

  rust:
    name: Rust
    runs-on: macos-15
    steps:
      - name: Check out repository
        uses: actions/checkout@v7
      - name: Set up Rust
        uses: dtolnay/rust-toolchain@stable
      - name: Cache Rust build
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: ./src-tauri -> target
      - name: Test Rust backend
        run: cargo test --manifest-path src-tauri/Cargo.toml
```

- [ ] **Step 2: Run the focused test and confirm only the release contract remains RED**

Run:

```bash
pnpm exec vitest run src/__tests__/githubWorkflows.spec.ts
```

Expected: the CI test passes; the release test fails because `release.yml` does not exist.

---

### Task 3: Implement the guarded draft-release workflow

**Files:**
- Create: `.github/workflows/release.yml`
- Test: `src/__tests__/githubWorkflows.spec.ts`

- [ ] **Step 1: Create `.github/workflows/release.yml`**

```yaml
name: Release

on:
  push:
    tags:
      - "app-v*"

permissions:
  contents: write

concurrency:
  group: release-${{ github.ref_name }}
  cancel-in-progress: false

jobs:
  release:
    name: Release
    runs-on: macos-15
    steps:
      - name: Check out repository
        uses: actions/checkout@v7
        with:
          fetch-depth: 0

      - name: Validate release source
        shell: bash
        run: |
          app_version="$(jq -r '.version' src-tauri/tauri.conf.json)"
          expected_tag="app-v${app_version}"

          if [[ "$GITHUB_REF_NAME" != "$expected_tag" ]]; then
            echo "Tag $GITHUB_REF_NAME does not match $expected_tag" >&2
            exit 1
          fi

          git fetch --no-tags origin main
          if ! git merge-base --is-ancestor "$GITHUB_SHA" origin/main; then
            echo "Tagged commit $GITHUB_SHA is not contained in origin/main" >&2
            exit 1
          fi

      - name: Install pnpm
        uses: pnpm/action-setup@v6
        with:
          version: 10.33.0
      - name: Set up Node.js
        uses: actions/setup-node@v7
        with:
          node-version: 24
          cache: pnpm
          cache-dependency-path: pnpm-lock.yaml
      - name: Set up Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: aarch64-apple-darwin
      - name: Cache Rust build
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: ./src-tauri -> target
      - name: Install frontend dependencies
        run: pnpm install --frozen-lockfile
      - name: Test frontend
        run: pnpm test
      - name: Test Rust backend
        run: cargo test --manifest-path src-tauri/Cargo.toml

      - name: Build draft release
        uses: tauri-apps/tauri-action@v1
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          APPLE_SIGNING_IDENTITY: "-"
        with:
          tagName: ${{ github.ref_name }}
          releaseName: "Codex Pulse v__VERSION__"
          releaseCommitish: ${{ github.sha }}
          generateReleaseNotes: true
          releaseDraft: true
          prerelease: false
          uploadUpdaterJson: false
          args: "--target aarch64-apple-darwin --bundles dmg"
```

- [ ] **Step 2: Run the focused workflow contract test and record GREEN**

Run:

```bash
pnpm exec vitest run src/__tests__/githubWorkflows.spec.ts
```

Expected: 1 file and 2 tests pass.

- [ ] **Step 3: Run repository-level verification**

Run:

```bash
pnpm test
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
git diff --check
```

Expected: all frontend tests pass, the production frontend build succeeds, all Rust tests pass, and `git diff --check` produces no output.

- [ ] **Step 4: Review and commit the implementation**

Inspect:

```bash
git status --short
git diff -- .github/workflows/ci.yml .github/workflows/release.yml src/__tests__/githubWorkflows.spec.ts
```

Commit only the workflow and workflow-test files:

```bash
git add .github/workflows/ci.yml .github/workflows/release.yml src/__tests__/githubWorkflows.spec.ts
git commit -m "ci: add validation and draft release workflows"
```

---

### Task 4: Open the pull request and prove both CI entry points

**Files:**
- No repository file changes expected.

- [ ] **Step 1: Push the feature branch and open a pull request**

Run:

```bash
git push -u origin codex/github-actions-release
gh pr create \
  --base main \
  --head codex/github-actions-release \
  --title "ci: add GitHub Actions release flow" \
  --body-file /tmp/codex-pulse-actions-pr.md
```

The PR body must summarize CI, the guarded draft-release path, ad-hoc/not-notarized scope, and the exact local verification commands.

- [ ] **Step 2: Wait for pull-request CI and inspect evidence**

Run:

```bash
gh pr checks --watch
gh pr checks
```

Expected: `Frontend` and `Rust` both complete successfully. If either fails, inspect `gh run view <run-id> --log-failed`, fix through a new RED/GREEN cycle, and repeat.

- [ ] **Step 3: Ask for explicit merge approval**

Report the PR URL, commits, changed files, local verification, and GitHub check results through AskHuman. Do not merge until the user approves.

- [ ] **Step 4: Merge and verify the `main` push workflow**

After approval, merge the PR, fetch the exact merged SHA, then find and watch the `CI` run triggered by the push to `main`:

```bash
gh pr merge <pr-number> --merge
git fetch origin main
merge_sha="$(git rev-parse origin/main)"
gh run list --workflow CI --branch main --event push --limit 10
gh run watch <main-ci-run-id> --exit-status
```

Expected: the run has `headSha == merge_sha`, event `push`, conclusion `success`, and successful jobs named `Frontend` and `Rust`.

---

### Task 5: Require the proven checks on `main`

**Files:**
- No repository file changes expected.

- [ ] **Step 1: Snapshot the current branch-protection rule**

Run:

```bash
gh api graphql -f query='query { repository(owner: "qwertyerge", name: "codex-pulse") { branchProtectionRules(first: 20) { nodes { id pattern requiresApprovingReviews requiredApprovingReviewCount requiresConversationResolution requiresStatusChecks requiresStrictStatusChecks requiredStatusCheckContexts isAdminEnforced allowsForcePushes allowsDeletions } } } }'
```

Record the `main` rule ID and confirm the unrelated settings still match the pre-change snapshot.

- [ ] **Step 2: Add only the successful required checks**

Run with the actual rule ID:

```bash
gh api graphql \
  -F ruleId='<main-rule-id>' \
  -f query='mutation($ruleId: ID!) { updateBranchProtectionRule(input: { branchProtectionRuleId: $ruleId, requiresStatusChecks: true, requiresStrictStatusChecks: true, requiredStatusCheckContexts: ["Frontend", "Rust"] }) { branchProtectionRule { requiresStatusChecks requiresStrictStatusChecks requiredStatusCheckContexts } } }'
```

Expected: status checks are enabled, strict mode is true, and the only required contexts are `Frontend` and `Rust`.

- [ ] **Step 3: Verify the complete protection rule did not drift**

Repeat the snapshot query from Step 1. Compare all returned non-status fields with the before snapshot. If any unrelated field changed, stop and ask before altering it.

---

### Task 6: Trigger and verify the first draft release

**Files:**
- No repository file changes expected.

- [ ] **Step 1: Verify the release target and absence of collisions**

Run:

```bash
git fetch origin main --tags
test "$(git show origin/main:src-tauri/tauri.conf.json | jq -r '.version')" = "0.1.0"
test -z "$(git tag --list app-v0.1.0)"
if gh release view app-v0.1.0 >/dev/null 2>&1; then exit 1; fi
```

If the tag or release already exists, stop and ask; do not overwrite or delete it.

- [ ] **Step 2: Create and push the annotated release tag**

Run:

```bash
git tag -a app-v0.1.0 origin/main -m "Codex Pulse v0.1.0"
git push origin refs/tags/app-v0.1.0
```

This is the externally visible release trigger; do it only after the successful `main` CI and branch-protection verification.

- [ ] **Step 3: Monitor the exact release workflow**

Find the `Release` workflow run with event `push`, branch/tag `app-v0.1.0`, and `headSha == origin/main`, then run:

```bash
gh run watch <release-run-id> --exit-status
gh run view <release-run-id> --json databaseId,event,headBranch,headSha,status,conclusion,url,jobs
```

Expected: conclusion `success`; its only release job succeeds. On failure, inspect the failed logs and do not repoint or recreate the tag without asking.

- [ ] **Step 4: Inspect the draft release and download its asset**

Run:

```bash
gh release view app-v0.1.0 --json tagName,name,isDraft,isPrerelease,targetCommitish,url,assets
release_check_dir="$(mktemp -d)"
gh release download app-v0.1.0 --dir "$release_check_dir" --pattern '*.dmg'
find "$release_check_dir" -maxdepth 1 -type f -name '*.dmg'
shasum -a 256 "$release_check_dir"/*.dmg
stat -f '%N %z bytes' "$release_check_dir"/*.dmg
```

Expected: one draft, non-prerelease release targeted at the merged commit with one ARM64 DMG asset. Record the asset name, byte size, and SHA-256.

- [ ] **Step 5: Mount the DMG and verify its app signature**

Use an explicit temporary mount directory and guarantee detach on exit:

```bash
release_mount_dir="$(mktemp -d)"
hdiutil attach "$release_check_dir"/*.dmg -mountpoint "$release_mount_dir" -nobrowse
codesign --verify --deep --strict "$release_mount_dir/Codex Pulse.app"
codesign -dv --verbose=4 "$release_mount_dir/Codex Pulse.app" 2>&1
hdiutil detach "$release_mount_dir"
```

Expected: strict deep verification succeeds and signature detail identifies an ad-hoc signature. If attachment succeeds but later verification fails, detach before investigation.

- [ ] **Step 6: Hand off the verified draft release**

Report through AskHuman:

- PR and merge commit
- successful pull-request and `main` CI runs
- exact required status-check configuration
- successful release run
- draft release URL
- DMG asset name, size, and SHA-256
- `codesign --verify --deep --strict` result
- explicit caveat that the artifact is ad-hoc signed, not Developer ID signed, and not notarized

Leave the release in draft state and do not publish it.

