# Codex Pulse GitHub Actions and Release Design

## Context

Codex Pulse currently has no `.github` directory, Actions workflows, tags,
GitHub Releases, Actions secrets, or Actions variables. The public repository
allows GitHub Actions, while `main` already requires pull requests, enforces the
rule for administrators, and disallows force-pushes and deletion. It does not
yet require any status checks.

The application is a macOS-focused Tauri 2 project. Version `0.1.0` is defined
in `src-tauri/tauri.conf.json`. The current machine has no valid Apple signing
identity and the repository has no Apple credentials. The first automated
release therefore cannot be notarized.

## Goals

- Run frontend and Rust validation for pull requests and pushes to `main`.
- Make those two checks required by the existing `main` branch protection.
- Create an Apple Silicon macOS Draft Release when an `app-v*` tag is pushed.
- Reject tags whose version differs from the Tauri application version.
- Reject release tags that do not point to history already contained in
  `origin/main`.
- Produce an explicitly ad-hoc-signed DMG without storing Apple credentials.
- Verify the first remote release artifact before handing it off for manual
  publication.

## Non-Goals

- Intel macOS, Windows, or Linux release artifacts.
- Developer ID signing, Apple notarization, or App Store distribution.
- Automatic publication of a Draft Release.
- Tauri updater configuration or `latest.json` generation.
- Automatic version mutation or changelog commits from the workflow.
- A release branch or a manual `workflow_dispatch` release path.

## Workflow Architecture

### Continuous Integration

Create `.github/workflows/ci.yml` with two independent jobs.

`Frontend` runs on `ubuntu-latest` for pull requests targeting `main` and for
pushes to `main`. It uses Node.js 24 and pnpm 10.33.0, installs dependencies
with the frozen lockfile, runs `pnpm test`, and runs `pnpm build`.

`Rust` runs on the ARM64 `macos-15` image and executes
`cargo test --manifest-path src-tauri/Cargo.toml`. A macOS runner preserves the
project's real platform boundary: the application enables Tauri macOS private
APIs and the liquid-glass plugin, so a Linux-only Rust build would not be the
authoritative native check.

The workflow has only `contents: read`. A concurrency group based on workflow
and ref cancels superseded runs on the same pull request or branch.

### Tag-Driven Draft Release

Create `.github/workflows/release.yml`, triggered only by pushed tags matching
`app-v*`. The release job runs on ARM64 `macos-15`, has `contents: write`, and
uses a concurrency group based on the immutable tag name without cancelling a
release already in progress.

Checkout uses full history. Before dependencies are installed, a guard step:

1. reads the version from `src-tauri/tauri.conf.json`;
2. requires the tag to equal `app-v<version>` exactly; and
3. fetches `origin/main` and requires the tagged commit to be an ancestor of
   that remote branch.

The workflow then installs the same Node.js, pnpm, and Rust toolchains as CI,
runs `pnpm test` and the Rust test suite, and invokes
`tauri-apps/tauri-action@v1`. The action receives the triggering tag, names the
release `Codex Pulse v__VERSION__`, generates release notes, creates a Draft,
disables updater JSON, and builds only the `aarch64-apple-darwin` DMG.

`APPLE_SIGNING_IDENTITY` is set to the pseudo-identity `-`, which tells Tauri to
apply an ad-hoc signature to the bundle. No repository secret is read.

## Action Versions and Supply Boundary

Use current stable major tags:

- `actions/checkout@v7`
- `actions/setup-node@v7`
- `pnpm/action-setup@v6`
- `dtolnay/rust-toolchain@stable`
- `Swatinem/rust-cache@v2`
- `tauri-apps/tauri-action@v1`

The repository currently permits all public actions and does not require SHA
pinning. Major tags keep the workflow on the supported line, but they are a
deliberate mutable trust boundary: a tag can resolve newer action code without a
repository change. Pinning actions to immutable commit SHAs remains out of scope
for this release setup.

## Branch Protection

Adding a workflow does not itself create a merge gate. After the workflow has
run successfully on its pull request and again on the merged `main`, update the
existing branch protection to require the `Frontend` and `Rust` status contexts
with strict branch freshness.

The update must preserve all existing settings: pull-request-only integration,
administrator enforcement, no force-push, no deletion, and the current review
requirements.

## Testing Strategy

Add `src/__tests__/githubWorkflows.spec.ts`. The repository already uses static
source-contract tests for configuration boundaries, so this test will read the
two workflow files without adding a YAML dependency.

The test will first fail because the files do not exist. It will then assert
the high-value contracts:

- CI triggers, read-only permission, runner split, frozen installation, and
  exact frontend/Rust commands;
- release tag trigger, write permission, full checkout, version and ancestry
  guards, test commands, ad-hoc identity, Draft state, updater exclusion, and
  Apple Silicon DMG arguments.

After GREEN, run the complete frontend suite, frontend production build, Rust
suite, and `git diff --check`. The GitHub pull request is the authoritative YAML
syntax and hosted-runner integration test; both jobs and CodeRabbit must reach
a successful terminal state before merge.

## First Release Procedure

After the workflow pull request is merged and the `main` CI run passes:

1. apply the required checks to branch protection;
2. create annotated tag `app-v0.1.0` at the verified `origin/main` commit;
3. push only that tag;
4. follow the Release workflow to a terminal state;
5. inspect the Draft Release and confirm the expected DMG asset;
6. download the DMG to a temporary directory;
7. record its size and SHA-256;
8. mount it and run `codesign --verify --deep --strict` on the contained app;
9. detach the image and leave the Release in Draft state.

The handoff must state that ad-hoc signing is not notarization. Users who
download the eventual public release may still need to allow the application
from macOS Privacy & Security.

## Failure Handling

- A mismatched tag or non-main commit fails before any release API call.
- Test or build failure leaves no public release.
- If Tauri Action creates a partial Draft before a later failure, keep it
  private for inspection; do not publish or silently replace assets.
- Do not bypass a failed required check or weaken branch protection to merge.
- Do not create Apple signing secrets as part of this scope.

## References

- Tauri GitHub pipeline guide: https://v2.tauri.app/distribute/pipelines/github/
- Tauri macOS signing guide: https://v2.tauri.app/distribute/sign/macos/
- GitHub workflow permissions: https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax
- GitHub release management: https://docs.github.com/en/repositories/releasing-projects-on-github/managing-releases-in-a-repository
