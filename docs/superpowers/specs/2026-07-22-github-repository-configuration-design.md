# GitHub Repository Configuration Design

## Goal

Make `qwertyerge/codex-pulse` a clear, conventional open-source repository without overstating the readiness of its unsigned macOS release. The repository should communicate what Codex Pulse does, how it relates to OpenAI, how people can contribute, and which GitHub collaboration paths are actively maintained.

This work changes repository metadata, documentation, contribution templates, and GitHub settings. It does not change application behavior, release signing, notarization, or runtime data handling.

## Public Identity and License

The repository description will be:

> Unofficial, local-first macOS desktop companion for monitoring active Codex tasks.

The repository topics will be:

- `codex`
- `openai-codex`
- `macos`
- `tauri`
- `rust`
- `vue`
- `typescript`
- `desktop-app`
- `developer-tools`
- `session-monitor`

The GitHub homepage field will remain empty because the project has no independent website and the current macOS release is not Developer ID signed or notarized. The README may link to the release page, but it must identify the release as experimental and keep source builds as the primary installation path.

Codex Pulse will use the Apache License 2.0. The repository will contain the standard Apache License 2.0 text in `LICENSE`. `package.json` and `src-tauri/Cargo.toml` will declare the SPDX identifier `Apache-2.0` and the repository URL.

## README Changes

`README.md` and `docs/README.zh-CN.md` will remain reciprocal English and Simplified Chinese entry points. Both will add:

- CI, release, and Apache-2.0 license badges;
- a prominent statement that Codex Pulse is an independent community project and is not affiliated with or endorsed by OpenAI;
- a release-status note explaining that published macOS artifacts are experimental, unsigned with a Developer ID certificate, and not notarized;
- a source-build-first installation path;
- a License section linking to `LICENSE`.

The language-specific text may be idiomatic rather than sentence-for-sentence identical, but the behavioral, security, and distribution claims must stay aligned.

## Contribution and Support Files

`CONTRIBUTING.md` will define the supported development environment, branch and pull-request workflow, Conventional Commit-style subjects, required verification commands, screenshot expectations for visual changes, and documentation synchronization expectations.

It will also state the privacy boundary: contributors must not submit local Codex transcripts, tokens, signing materials, unredacted `hooks.json` contents, or user-specific paths.

`SECURITY.md` will cover the current `main` branch and latest release only. Per the selected policy, reports will use public GitHub Issues. The document must require aggressive redaction and explicitly prohibit publishing transcripts, credentials, tokens, signing material, or private paths. It will make clear that no private reporting channel is currently offered.

Two structured issue forms will be added:

- a bug report form collecting application version, macOS version and architecture, reproduction steps, expected and actual behavior, verification of redaction, and optional sanitized logs;
- a feature request form collecting the user problem, proposed outcome, alternatives, and relevant context.

Blank issues will be disabled. A pull-request template will request a linked issue, user-visible behavior summary, verification evidence, screenshots for UI changes, documentation impact, and a privacy check.

This scope intentionally excludes a Code of Conduct, CODEOWNERS, Funding configuration, Dependabot configuration, automatic labels, and other governance automation.

## GitHub Settings

GitHub repository settings will be updated as follows:

- Issues enabled;
- Projects disabled;
- Wiki disabled;
- Discussions disabled;
- squash merge enabled;
- merge commits disabled;
- rebase merges disabled;
- merged branches deleted automatically.

Auto-merge and other repository settings not listed above will remain unchanged.

The existing `main` branch protection remains authoritative: changes require a pull request, `Frontend` and `Rust` status checks must pass, administrators are included, and force-pushes and branch deletion are disabled. GitHub Actions will retain read-only default workflow permissions. Existing secret scanning and push protection settings will remain enabled.

## Delivery Sequence

Work will occur on `codex/github-repository-config`, never directly on `main`.

1. Add the license, manifests, bilingual README updates, contribution guidance, security policy, issue forms, and pull-request template.
2. Validate YAML parsing, Markdown links, package metadata, frontend tests, Rust tests, and the frontend build.
3. Commit and push the branch, then create a pull request.
4. Apply the approved GitHub metadata and repository settings.
5. Wait for the required `Frontend` and `Rust` checks, then squash-merge the pull request and delete its remote branch.
6. Read the repository and community-profile APIs from the updated default branch to confirm the resulting state.
7. Report any distinction between local validation, CI, merged repository state, and release distribution readiness.

## Failure Handling

GitHub changes will be made through authenticated API or CLI calls and immediately read back. If a requested field is unsupported or the current token lacks permission, work will stop at the affected setting. The exact response will be recorded and no broader permission, weaker policy, or substitute workflow will be chosen without approval.

If a community-profile file is not recognized, its path and content will be checked against GitHub's accepted conventions before any rename or scope change. Existing branch protection will not be weakened to unblock the pull request.

## Acceptance Criteria

- GitHub reports Apache-2.0 as the repository license.
- The approved description and all ten topics are visible through the repository API.
- README files disclose the unofficial status and experimental, unnotarized release state consistently.
- GitHub recognizes the contribution guide, security policy, issue forms, pull-request template, and license.
- Issues are enabled; Projects, Wiki, and Discussions are disabled.
- Squash is the only enabled merge method and merged branches are deleted automatically.
- Existing `main` protection, required checks, Actions permissions, secret scanning, and push protection remain intact.
- Frontend tests, Rust tests, frontend build, static YAML validation, and Markdown-link checks pass locally.
- The pull request from `codex/github-repository-config` passes both required checks and is squash-merged into `main`.
