# Windows 0.3.0 Automated Acceptance Record

Date: 2026-07-27

Product version: `0.3.0`

Acceptance status: automated Windows gate complete; interactive Windows desktop acceptance pending

## Source and delivery identity

- Annotated tag: `0.3.0`
- Tag object: `2a38284b806a9c5beabead2297846f27cf9d0548`
- Peeled target and release source:
  `fed9a7a2affa6326e6390ef4ba0fa909fc032c76`
- Feature PR:
  [#10 feat: add Windows native MVP](https://github.com/qwertyerge/codex-pulse/pull/10)
- Release Guard repair PR:
  [#11 fix: make release authorization portable](https://github.com/qwertyerge/codex-pulse/pull/11)
- Draft Release:
  [Codex Pulse 0.3.0](https://github.com/qwertyerge/codex-pulse/releases/tag/untagged-6f8e63a5d7d582c231d1)

The Draft targets the exact tagged commit above. `package.json`,
`src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml` all declare `0.3.0`.

## Automated gate evidence

| Gate | Result | Evidence |
| --- | --- | --- |
| Local frontend regression | Passed: 19 files, 83 tests | `pnpm test` on the reviewed PR #11 tree |
| Local frontend production build | Passed | `pnpm build` on the reviewed PR #11 tree |
| Local Rust regression | Passed: 77 tests | `cargo test --manifest-path src-tauri/Cargo.toml` |
| Release Guard helper properties | Passed | Focused Vitest 5/5, `bash -n`, long-token single-line round trip, empty-token rejection, LF and executable-mode checks |
| Feature PR CI | Passed | [Run 30244759563](https://github.com/qwertyerge/codex-pulse/actions/runs/30244759563) |
| Feature main CI | Passed | [Run 30246534073](https://github.com/qwertyerge/codex-pulse/actions/runs/30246534073) |
| Guard repair PR CI | Passed | [Run 30249308548](https://github.com/qwertyerge/codex-pulse/actions/runs/30249308548) |
| Tagged source main CI | Passed | [Run 30249872417](https://github.com/qwertyerge/codex-pulse/actions/runs/30249872417) |
| Draft Release workflow | Passed | [Run 30250228503](https://github.com/qwertyerge/codex-pulse/actions/runs/30250228503) |
| Branch protection | Enforced | Strict required checks: `Frontend`, `Rust`, and `Rust (Windows)` |

The native Windows jobs proved the following on `windows-latest`:

- the frontend suite, including the executable Release Guard regression,
  passes under the Windows checkout and Git Bash environment;
- 79 Rust library tests and the `windows_hook_cli` integration test pass;
- the named-pipe hook CLI and Windows path/process fixtures execute natively;
- an x64 NSIS package is produced;
- the CI package verifier silently installs the package, invokes the installed
  `__hook` helper, verifies the PE GUI subsystem, and silently uninstalls it;
  and
- the Release workflow uploads the x64 NSIS package to the same Draft as the
  macOS ARM64 DMG.

The Release workflow's source Guard completed successfully before either
platform build. The prior failed run
[30247692588](https://github.com/qwertyerge/codex-pulse/actions/runs/30247692588)
created no release or assets; PR #11 repaired its GNU `base64` line-wrapping
failure before the tag was recreated on the reviewed main commit.

## Draft assets

Both assets were downloaded independently from the Draft into a task-specific
temporary directory before hashing.

| Asset | Bytes | SHA-256 |
| --- | ---: | --- |
| `Codex.Pulse_0.3.0_aarch64.dmg` | 4,629,132 | `04a85fadbe1858b2b406abbe4eb364f425a13cb281a569c2d6022c6138917fd6` |
| `Codex.Pulse_0.3.0_x64-setup.exe` | 3,064,396 | `537c2fe49c80de22baee1d1cb0c829e778c97286ac3945beb63d8bd63bef0439` |

The downloaded Windows asset identifies as a GUI PE executable and Nullsoft
Installer self-extracting archive.

## Interactive Windows acceptance

Hosted-runner evidence does not substitute for a user-controlled Windows 11
x64 desktop review. Every item below remains `pending-user-eyeball`:

| Interactive item | Status |
| --- | --- |
| Discover and render a real active native Codex task | `pending-user-eyeball` |
| Observe lifecycle-hook refresh latency with the 60-second fallback intact | `pending-user-eyeball` |
| Open an existing task through `codex://threads/<uuid>` | `pending-user-eyeball` |
| Open a native Windows project directory in Explorer | `pending-user-eyeball` |
| Check tray icon contrast and tray menu actions | `pending-user-eyeball` |
| Check transparent light and dark rendering | `pending-user-eyeball` |
| Exercise Pin and Unpin | `pending-user-eyeball` |
| Exercise close-to-hide | `pending-user-eyeball` |
| Confirm single-instance behavior | `pending-user-eyeball` |
| Confirm release app and `__hook` invocations do not flash a console | `pending-user-eyeball` |
| Check maximization on single- and multi-display setups | `pending-user-eyeball` |

## Scope and release limitations

- Supported MVP scope is Windows 11 x64 with native Codex and a local Windows
  `CODEX_HOME`. WSL discovery, path translation, hook bridging, and WSL GUI
  support are not included.
- The Windows installer uses per-user NSIS installation and the WebView2
  download bootstrapper when the runtime is missing.
- The Windows installer is unsigned. Browser downloads may trigger
  SmartScreen, and organizational security policy must not be bypassed.
- The GitHub Release is a Draft, is not a prerelease, and remains unpublished.
- Automated acceptance is complete. Final interactive Windows acceptance must
  not be reported complete until the pending items above are exercised on a
  user-controlled Windows 11 x64 desktop.
