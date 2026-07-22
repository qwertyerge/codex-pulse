# GitHub Repository Configuration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the public GitHub identity, Apache-2.0 licensing, contribution paths, issue and pull-request templates, repository settings, and default-branch verification for Codex Pulse.

**Architecture:** Keep durable repository policy in version-controlled files and enforce its structural invariants with one focused Vitest contract. Apply mutable GitHub metadata and merge settings through authenticated API calls, then read them back before and after the guarded squash merge so the default branch—not the feature checkout—is the final source of truth.

**Tech Stack:** Markdown, Apache License 2.0, GitHub Issue Forms YAML, JSON and Cargo manifest metadata, Vitest, `yaml`, pnpm, Cargo, GitHub CLI, GitHub REST API.

## Global Constraints

- Use the exact repository description: `Unofficial, local-first macOS desktop companion for monitoring active Codex tasks.`
- Use exactly these topics: `codex`, `openai-codex`, `macos`, `tauri`, `rust`, `vue`, `typescript`, `desktop-app`, `developer-tools`, `session-monitor`.
- Use SPDX license identifier `Apache-2.0`; do not add a `NOTICE` file or source-file headers.
- Keep the GitHub homepage empty until there is an independent site or normally distributable signed and notarized release.
- State that Codex Pulse is an independent community project not affiliated with or endorsed by OpenAI.
- Treat published macOS artifacts as experimental because they are not Developer ID signed or Apple notarized; keep source builds as the primary installation path.
- Use public GitHub Issues for security reports and require aggressive redaction; do not claim a private reporting channel exists.
- Never commit local Codex transcripts, tokens, signing material, unredacted `hooks.json` content, or user-specific paths.
- Keep Issues enabled; disable Projects, Wiki, and Discussions.
- Allow only squash merge and delete merged branches automatically; leave auto-merge and unrelated settings unchanged.
- Preserve `main` protection, required checks `Frontend` and `Rust`, read-only default Actions permissions, secret scanning, and push protection.
- Deliver through `codex/github-repository-config`, wait for both required checks, squash-merge, and verify the updated default branch.

---

### Task 1: Add a durable community-configuration contract

**Files:**
- Create: `src/__tests__/githubCommunity.spec.ts`
- Test: `src/__tests__/githubCommunity.spec.ts`

**Interfaces:**
- Consumes: repository-root Markdown, JSON, TOML, and `.github/ISSUE_TEMPLATE/*.yml` files.
- Produces: a Vitest contract that names every required file, metadata value, issue-form field ID, and public-identity statement used by Tasks 2–4.

- [ ] **Step 1: Create the failing contract test**

Create `src/__tests__/githubCommunity.spec.ts` with:

```ts
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { parse } from "yaml";

interface IssueForm {
  name: string;
  description: string;
  title: string;
  labels: string[];
  body: Array<{ id?: string; type: string }>;
}

function readRoot(path: string) {
  const absolute = resolve(process.cwd(), path);
  expect(existsSync(absolute), `${path} should exist`).toBe(true);
  return readFileSync(absolute, "utf8");
}

function readIssueForm(name: string) {
  return parse(readRoot(`.github/ISSUE_TEMPLATE/${name}`)) as IssueForm;
}

describe("GitHub community configuration", () => {
  it("declares Apache-2.0 consistently", () => {
    const license = readRoot("LICENSE");
    const packageJson = JSON.parse(readRoot("package.json")) as {
      license: string;
      repository: { type: string; url: string };
    };
    const cargo = readRoot("src-tauri/Cargo.toml");

    expect(license).toContain("Apache License\n                           Version 2.0, January 2004");
    expect(license).toContain("http://www.apache.org/licenses/");
    expect(packageJson.license).toBe("Apache-2.0");
    expect(packageJson.repository).toEqual({
      type: "git",
      url: "https://github.com/qwertyerge/codex-pulse.git",
    });
    expect(cargo).toContain('license = "Apache-2.0"');
    expect(cargo).toContain('repository = "https://github.com/qwertyerge/codex-pulse"');
  });

  it("keeps the English and Chinese public identity aligned", () => {
    const english = readRoot("README.md");
    const chinese = readRoot("docs/README.zh-CN.md");

    for (const badge of ["actions/workflows/ci.yml/badge.svg", "github/v/release", "github/license"]) {
      expect(english).toContain(badge);
      expect(chinese).toContain(badge);
    }
    expect(english).toContain("independent community project");
    expect(english).toContain("not affiliated with or endorsed by OpenAI");
    expect(english).toContain("not Developer ID signed or Apple notarized");
    expect(english).toContain("## Build from Source");
    expect(english).toContain("## License");
    expect(chinese).toContain("独立社区项目");
    expect(chinese).toContain("与 OpenAI 无隶属关系，也未获得其认可");
    expect(chinese).toContain("未使用 Developer ID 签名，也未经过 Apple 公证");
    expect(chinese).toContain("## 从源码构建");
    expect(chinese).toContain("## 许可证");
  });

  it("provides contribution, security, issue, and pull-request guidance", () => {
    const contributing = readRoot("CONTRIBUTING.md");
    const security = readRoot("SECURITY.md");
    const pullRequest = readRoot(".github/pull_request_template.md");
    const config = parse(readRoot(".github/ISSUE_TEMPLATE/config.yml")) as {
      blank_issues_enabled: boolean;
    };
    const bug = readIssueForm("bug_report.yml");
    const feature = readIssueForm("feature_request.yml");

    expect(contributing).toContain("pnpm test");
    expect(contributing).toContain("cargo test --manifest-path src-tauri/Cargo.toml");
    expect(contributing).toContain("Do not include local Codex transcripts");
    expect(security).toContain("public GitHub issue");
    expect(security).toContain("does not currently offer a private reporting channel");
    expect(pullRequest).toContain("Privacy checklist");
    expect(config.blank_issues_enabled).toBe(false);
    expect(bug.labels).toEqual(["bug"]);
    expect(bug.body.map((item) => item.id).filter(Boolean)).toEqual(
      expect.arrayContaining([
        "version",
        "macos",
        "architecture",
        "problem",
        "steps",
        "expected",
        "actual",
        "logs",
        "privacy",
      ]),
    );
    expect(feature.labels).toEqual(["enhancement"]);
    expect(feature.body.map((item) => item.id).filter(Boolean)).toEqual(
      expect.arrayContaining(["problem", "outcome", "alternatives", "context", "privacy"]),
    );
  });
});
```

- [ ] **Step 2: Run the focused test and record RED**

Run:

```bash
pnpm exec vitest run src/__tests__/githubCommunity.spec.ts
```

Expected: all three tests fail at missing files or missing metadata; the first failure names `LICENSE should exist`.

- [ ] **Step 3: Commit the RED contract**

```bash
git add src/__tests__/githubCommunity.spec.ts
git commit -m "test: define GitHub community configuration"
```

---

### Task 2: Add Apache-2.0 licensing and manifest metadata

**Files:**
- Create: `LICENSE`
- Modify: `package.json`
- Modify: `src-tauri/Cargo.toml`
- Test: `src/__tests__/githubCommunity.spec.ts`

**Interfaces:**
- Consumes: the exact SPDX and repository values asserted by Task 1.
- Produces: a GitHub-detectable Apache-2.0 license and consistent npm/Cargo package metadata.

- [ ] **Step 1: Obtain the canonical license text**

Run:

```bash
gh api licenses/apache-2.0 --jq .body
```

Expected: the standard Apache License beginning with `Apache License` and `Version 2.0, January 2004`, ending with `limitations under the License.` Use the returned body unchanged as `LICENSE` via `apply_patch`.

- [ ] **Step 2: Add manifest metadata**

Add these top-level fields after `version` in `package.json`:

```json
"license": "Apache-2.0",
"repository": {
  "type": "git",
  "url": "https://github.com/qwertyerge/codex-pulse.git"
},
```

Add these fields after `description` in the `[package]` section of `src-tauri/Cargo.toml`:

```toml
license = "Apache-2.0"
repository = "https://github.com/qwertyerge/codex-pulse"
```

- [ ] **Step 3: Run the licensing contract and confirm partial GREEN**

Run:

```bash
pnpm exec vitest run src/__tests__/githubCommunity.spec.ts -t "declares Apache-2.0 consistently"
```

Expected: one test passes.

- [ ] **Step 4: Commit the licensing change**

```bash
git add LICENSE package.json src-tauri/Cargo.toml
git commit -m "docs: add Apache-2.0 license"
```

---

### Task 3: Add contribution, security, issue, and pull-request paths

**Files:**
- Create: `CONTRIBUTING.md`
- Create: `SECURITY.md`
- Create: `.github/ISSUE_TEMPLATE/config.yml`
- Create: `.github/ISSUE_TEMPLATE/bug_report.yml`
- Create: `.github/ISSUE_TEMPLATE/feature_request.yml`
- Create: `.github/pull_request_template.md`
- Test: `src/__tests__/githubCommunity.spec.ts`

**Interfaces:**
- Consumes: project commands and privacy boundaries from `AGENTS.md` and the approved design.
- Produces: GitHub-recognized contribution documents and structured forms with the field IDs asserted by Task 1.

- [ ] **Step 1: Add `CONTRIBUTING.md`**

Create `CONTRIBUTING.md` with:

````markdown
# Contributing to Codex Pulse

Thanks for helping improve Codex Pulse. Keep changes focused, testable, and safe for people who use local Codex data.

## Before You Start

- Search existing issues before opening a new one.
- Use an issue to discuss behavior changes that affect users or project scope.
- Keep each pull request focused on one coherent change.
- Do not push directly to `main`.

## Development Setup

You need macOS with the Tauri prerequisites, Node.js, pnpm, and a Rust toolchain.

```bash
pnpm install
pnpm tauri dev
```

Frontend code lives in `src/`; Rust and Tauri code lives in `src-tauri/src/`.

## Making a Change

1. Create a feature branch from the current `main` branch.
2. Follow the existing Vue Composition API and Rust module patterns.
3. Add behavior-focused Vitest or Rust coverage for regressions and behavior changes.
4. Use concise Conventional Commit-style subjects such as `feat:`, `fix:`, or `docs:`.
5. Keep `README.md` and `docs/README.zh-CN.md` aligned when public behavior or setup changes.

## Verification

Run the relevant focused test while iterating, then run the complete repository checks:

```bash
pnpm test
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
```

## Pull Requests

Explain the user-visible behavior, link related issues, and list the commands you ran. Include before-and-after screenshots for visual changes. A pull request must pass the required `Frontend` and `Rust` checks before it can merge.

## Privacy and Security

Do not include local Codex transcripts, tokens, signing material, unredacted `hooks.json` content, or user-specific paths in an issue, pull request, fixture, screenshot, or log.

Sanitize diagnostics to the smallest reproduction that still demonstrates the behavior. See [SECURITY.md](SECURITY.md) for vulnerability-reporting guidance.
````

- [ ] **Step 2: Add `SECURITY.md`**

Create `SECURITY.md` with:

```markdown
# Security Policy

## Supported Versions

Security fixes target the current `main` branch and the latest published release. Older releases do not receive a guaranteed security-fix backport.

## Reporting a Vulnerability

Open a [public GitHub issue](https://github.com/qwertyerge/codex-pulse/issues/new/choose) with a minimal, fully redacted reproduction. This project does not currently offer a private reporting channel.

Before submitting, remove all local Codex transcripts, tokens, credentials, signing material, complete `hooks.json` content, private repository names, and user-specific paths. Do not attach raw session data. Replace sensitive values with stable placeholders and include only the minimum sanitized diagnostic detail needed to reproduce the problem.

Describe the affected version, macOS version and architecture, impact, reproduction steps, and any mitigation you have already tested. A maintainer will triage the issue in public and may ask for a safer reduced reproduction.
```

- [ ] **Step 3: Add the issue-form selector configuration**

Create `.github/ISSUE_TEMPLATE/config.yml`:

```yaml
blank_issues_enabled: false
contact_links: []
```

- [ ] **Step 4: Add the bug report form**

Create `.github/ISSUE_TEMPLATE/bug_report.yml` with:

```yaml
name: Bug report
description: Report reproducible incorrect behavior in Codex Pulse
title: "[Bug]: "
labels: [bug]
assignees: []
body:
  - type: input
    id: version
    attributes:
      label: Codex Pulse version
      placeholder: 0.1.0 or commit SHA
    validations:
      required: true
  - type: input
    id: macos
    attributes:
      label: macOS version
      placeholder: macOS 26.0
    validations:
      required: true
  - type: dropdown
    id: architecture
    attributes:
      label: Mac architecture
      options:
        - Apple Silicon (arm64)
        - Intel (x86_64)
    validations:
      required: true
  - type: textarea
    id: problem
    attributes:
      label: Problem summary
      description: Describe the incorrect behavior and its impact.
    validations:
      required: true
  - type: textarea
    id: steps
    attributes:
      label: Reproduction steps
      placeholder: |
        1. Start Codex Pulse...
        2. Perform...
        3. Observe...
    validations:
      required: true
  - type: textarea
    id: expected
    attributes:
      label: Expected behavior
    validations:
      required: true
  - type: textarea
    id: actual
    attributes:
      label: Actual behavior
    validations:
      required: true
  - type: textarea
    id: logs
    attributes:
      label: Sanitized logs
      description: Optional. Never paste transcripts, tokens, credentials, complete hooks.json content, or private paths.
      render: shell
  - type: checkboxes
    id: privacy
    attributes:
      label: Privacy confirmation
      options:
        - label: I removed local Codex transcripts, tokens, credentials, signing material, unredacted hooks.json content, and user-specific paths.
          required: true
```

- [ ] **Step 5: Add the feature request form**

Create `.github/ISSUE_TEMPLATE/feature_request.yml` with:

```yaml
name: Feature request
description: Propose an improvement to Codex Pulse
title: "[Feature]: "
labels: [enhancement]
assignees: []
body:
  - type: textarea
    id: problem
    attributes:
      label: User problem
      description: What is difficult today, and who is affected?
    validations:
      required: true
  - type: textarea
    id: outcome
    attributes:
      label: Desired outcome
      description: Describe the behavior or result you want, without prescribing unnecessary implementation details.
    validations:
      required: true
  - type: textarea
    id: alternatives
    attributes:
      label: Alternatives considered
      description: What workarounds or other approaches have you tried?
  - type: textarea
    id: context
    attributes:
      label: Additional context
      description: Add sanitized examples or screenshots if they materially clarify the request.
  - type: checkboxes
    id: privacy
    attributes:
      label: Privacy confirmation
      options:
        - label: I removed local Codex transcripts, tokens, credentials, signing material, unredacted hooks.json content, and user-specific paths.
          required: true
```

- [ ] **Step 6: Add the pull-request template**

Create `.github/pull_request_template.md` with:

```markdown
## Summary

- What changed?
- What user-visible behavior does it affect?

## Related issue

Closes #

## Verification

- [ ] Focused tests
- [ ] `pnpm test`
- [ ] `pnpm build`
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml`

List commands and relevant results:

## Visual evidence

Add before-and-after screenshots for visual changes, or explain why none are needed.

## Documentation

- [ ] Public behavior and setup documentation are updated.
- [ ] `README.md` and `docs/README.zh-CN.md` remain aligned where applicable.

## Privacy checklist

- [ ] No local Codex transcripts are included.
- [ ] No tokens, credentials, or signing material are included.
- [ ] No unredacted `hooks.json` content or user-specific paths are included.
- [ ] Screenshots and logs are sanitized.
```

- [ ] **Step 7: Validate YAML and the focused contract**

Run:

```bash
pnpm exec vitest run src/__tests__/githubCommunity.spec.ts -t "provides contribution"
```

Expected: one test passes and all three YAML files parse without errors.

- [ ] **Step 8: Commit the community files**

```bash
git add CONTRIBUTING.md SECURITY.md .github/ISSUE_TEMPLATE .github/pull_request_template.md
git commit -m "docs: add contribution and issue guidance"
```

---

### Task 4: Align the bilingual public README identity

**Files:**
- Modify: `README.md`
- Modify: `docs/README.zh-CN.md`
- Test: `src/__tests__/githubCommunity.spec.ts`

**Interfaces:**
- Consumes: current bilingual feature descriptions and the distribution limits already documented in both READMEs.
- Produces: aligned English and Chinese landing pages with badges, unofficial-project disclosure, source-first build instructions, and license links.

- [ ] **Step 1: Add the shared badges**

Insert these badges beneath each language-navigation link:

```markdown
[![CI](https://github.com/qwertyerge/codex-pulse/actions/workflows/ci.yml/badge.svg)](https://github.com/qwertyerge/codex-pulse/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/qwertyerge/codex-pulse)](https://github.com/qwertyerge/codex-pulse/releases/latest)
[![License](https://img.shields.io/github/license/qwertyerge/codex-pulse)](LICENSE)
```

For the Chinese README, make the License badge link `../LICENSE`.

- [ ] **Step 2: Add the English identity and release callouts**

Place these callouts after the opening description:

```markdown
> [!IMPORTANT]
> Codex Pulse is an independent community project. It is not affiliated with or endorsed by OpenAI.

> [!WARNING]
> Published macOS artifacts are experimental. They are not Developer ID signed or Apple notarized, so normal Gatekeeper installation is not yet supported. Build from source for the current supported path.
```

- [ ] **Step 3: Add the Chinese identity and release callouts**

Place these callouts after the opening description:

```markdown
> [!IMPORTANT]
> Codex Pulse 是独立社区项目，与 OpenAI 无隶属关系，也未获得其认可。

> [!WARNING]
> 当前发布的 macOS 构建属于实验性产物，未使用 Developer ID 签名，也未经过 Apple 公证，因此尚不支持常规的 Gatekeeper 安装流程。当前推荐从源码构建。
```

- [ ] **Step 4: Make source builds the primary installation path**

Add this English block before the existing development commands:

````markdown
## Build from Source

```bash
git clone https://github.com/qwertyerge/codex-pulse.git
cd codex-pulse
pnpm install --frozen-lockfile
pnpm tauri build
```

The macOS app and DMG are written under `src-tauri/target/release/bundle/`.
````

Add the aligned Chinese block:

````markdown
## 从源码构建

```bash
git clone https://github.com/qwertyerge/codex-pulse.git
cd codex-pulse
pnpm install --frozen-lockfile
pnpm tauri build
```

macOS 应用和 DMG 会生成在 `src-tauri/target/release/bundle/` 下。
````

Keep `pnpm tauri dev`, `pnpm test`, and Cargo testing in their development sections.

- [ ] **Step 5: Add aligned license sections**

Append:

```markdown
## License

Licensed under the [Apache License 2.0](LICENSE).
```

and:

```markdown
## 许可证

本项目采用 [Apache License 2.0](../LICENSE) 许可证。
```

- [ ] **Step 6: Run the full community contract**

Run:

```bash
pnpm exec vitest run src/__tests__/githubCommunity.spec.ts
```

Expected: three tests pass.

- [ ] **Step 7: Commit the public identity update**

```bash
git add README.md docs/README.zh-CN.md
git commit -m "docs: complete repository public identity"
```

---

### Task 5: Verify, publish, configure, merge, and audit GitHub

**Files:**
- Verify: all files changed in Tasks 1–4
- Remote state: `qwertyerge/codex-pulse` metadata, topics, feature switches, merge settings, pull request, `main`, and community profile

**Interfaces:**
- Consumes: the green local repository contract and commits from Tasks 1–4.
- Produces: a merged default branch plus API evidence for the approved GitHub settings and recognized community files.

- [ ] **Step 1: Run complete local verification**

Run:

```bash
pnpm test
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
git diff --check origin/main...HEAD
```

Expected: all Vitest tests pass, the Vue TypeScript build succeeds, all Rust tests pass, and `git diff --check` emits no output.

- [ ] **Step 2: Verify branch scope and push**

Run:

```bash
git status --short --branch
git log --oneline origin/main..HEAD
git push -u origin codex/github-repository-config
```

Expected: the worktree is clean, the log contains only the approved design, plan, tests, license, community, and README commits, and the branch push succeeds.

- [ ] **Step 3: Create the pull request**

Run `gh pr create` with title `docs: complete GitHub repository configuration`. The body must summarize Apache-2.0 licensing, public identity, contribution paths, and repository settings, and list the exact local verification commands from Step 1. Capture the PR number and URL.

- [ ] **Step 4: Apply the approved repository metadata and switches**

Run:

```bash
gh api --method PATCH repos/qwertyerge/codex-pulse \
  -f description='Unofficial, local-first macOS desktop companion for monitoring active Codex tasks.' \
  -F has_issues=true \
  -F has_projects=false \
  -F has_wiki=false \
  -F has_discussions=false \
  -F allow_squash_merge=true \
  -F allow_merge_commit=false \
  -F allow_rebase_merge=false \
  -F delete_branch_on_merge=true

gh api --method PUT repos/qwertyerge/codex-pulse/topics \
  -f 'names[]=codex' \
  -f 'names[]=openai-codex' \
  -f 'names[]=macos' \
  -f 'names[]=tauri' \
  -f 'names[]=rust' \
  -f 'names[]=vue' \
  -f 'names[]=typescript' \
  -f 'names[]=desktop-app' \
  -f 'names[]=developer-tools' \
  -f 'names[]=session-monitor'
```

Expected: both calls return HTTP success and the repository payload reflects the approved values.

- [ ] **Step 5: Read back settings that must not drift**

Read the repository, branch-protection, and Actions-permission APIs. Assert the exact description and topics; feature and merge booleans; `Frontend` and `Rust` required checks; PR requirement; admin enforcement; force-push and deletion disabled; Actions `default_workflow_permissions` equal to `read`; secret scanning and push protection still enabled.

- [ ] **Step 6: Wait for required pull-request checks**

Run:

```bash
gh pr checks --watch --interval 10
```

Expected: `Frontend` and `Rust` both complete successfully. If either fails, inspect the failing job and repair only the in-scope cause before continuing.

- [ ] **Step 7: Squash-merge and delete the remote branch**

Run:

```bash
gh pr merge --squash --delete-branch
gh pr view --json state,mergedAt,mergeCommit,url
git fetch origin main --prune
```

Expected: PR state is `MERGED`, `mergedAt` and `mergeCommit` are populated, `origin/main` advances, and the remote feature branch is absent.

- [ ] **Step 8: Audit the updated default branch**

Run:

```bash
gh repo view qwertyerge/codex-pulse --json description,homepageUrl,repositoryTopics,licenseInfo,defaultBranchRef,url
gh api repos/qwertyerge/codex-pulse/community/profile
gh api repos/qwertyerge/codex-pulse/branches/main/protection
gh api repos/qwertyerge/codex-pulse/actions/permissions/workflow
```

Expected: `licenseInfo.spdxId` is `Apache-2.0`; homepage is empty; all ten topics are present; the community profile recognizes the license, contribution guide, security policy, issue templates, pull-request template, and README; branch protection and Actions permissions match Step 5. If GitHub indexing lags, retry the read-only repository and community-profile calls without changing state.

- [ ] **Step 9: Report completion evidence**

Report the merged PR URL and merge commit, local test/build counts, CI results, exact GitHub metadata/settings, community-profile recognition, and the unchanged release limitation: the macOS artifact remains experimental, not Developer ID signed, and not Apple notarized.
