# Git Support Final Fix Report

Date: 2026-07-27

Commit: this report is included in `fix: enforce git metadata boundaries`.

## Resolver boundary

Root cause:

- `remote_url` was gated only by the presence of the primary worktree branch,
  so `branch.<default>.remote` could supply a URL even when
  `branch.<default>.merge` did not configure a tracking upstream.
- `git remote get-url <name>` returns the literal remote name when a remote
  section exists without `remote.<name>.url`; the resolver accepted that
  fallback as a repository URL.
- `for-each-ref %(upstream:short)` omits a configured branch remote/merge pair
  when the referenced remote section is absent, which discarded useful
  `default_upstream` metadata.

RED:

```text
cargo test --manifest-path src-tauri/Cargo.toml git::resolver::tests::
```

The three real-repository regressions failed for the intended reasons:

- remote URL leaked when `branch.trunk.remote` existed without
  `branch.trunk.merge`;
- the configured `missing/trunk` upstream was returned as `None` when the
  remote did not exist;
- a remote section without a URL produced `Some("company")`.

GREEN:

- A tracking upstream now requires both `branch.<default>.remote` and
  `branch.<default>.merge`.
- A configured remote/merge pair remains available as `default_upstream` even
  when its remote section is missing.
- Remote resolution runs only for a configured tracking upstream and only
  after `remote.<tracking>.url` is confirmed to exist.
- Valid remotes still flow through `remote get-url`, preserving Git URL
  rewrites, and then through URL sanitization.
- Every missing-field regression also asserts that the default branch,
  primary checkout path, and project name remain available so cache/UI
  degradation can display `Not configured` rather than discard Git context.

```text
cargo test --manifest-path src-tauri/Cargo.toml git::resolver::tests::
8 passed; 0 failed
```

## Timeout cleanup boundary

Root cause:

- The timeout path passed `Child` by mutable reference to a bounded kill/reap
  helper. If the child was still unreaped after 100 ms, returning dropped the
  `Child` and detached both pipe-reader handles, so eventual wait and reader
  cleanup were no longer owned by any component.

RED:

```text
cargo test --manifest-path src-tauri/Cargo.toml \
  git::command::tests::timeout_returns_promptly_while_a_background_reaper_finishes_cleanup \
  -- --exact
```

The regression failed to compile because the runner had no controllable reap
window and no way to observe completed background cleanup.

GREEN:

- Timeout/error cleanup now consumes the `Child`.
- After the 100 ms synchronous reap window, an unreaped child and both reader
  handles move together to a dedicated background reaper.
- The reaper performs the final `wait` and joins both pipe readers while the
  caller retains a bounded timeout return.
- The regression forces the background path with a zero-length test reap
  window, verifies the timeout returns within 250 ms, verifies the delayed
  completion marker never runs, and observes reaper completion after
  `wait` plus both reader joins through a two-second bounded channel.

```text
cargo test --manifest-path src-tauri/Cargo.toml git::command::tests::
5 passed; 0 failed
```

The focused command suite also retains the large stdout/stderr and nonzero
exit-status contracts.

## Full verification

```text
cargo test --manifest-path src-tauri/Cargo.toml
72 passed; 0 failed

cargo fmt --check --manifest-path src-tauri/Cargo.toml
exit 0

pnpm test
19 files passed; 79 tests passed

pnpm build
vue-tsc --noEmit and vite build passed

git diff --check
exit 0
```

## Findings status

- Addressed: resolver tracking-upstream and configured-remote URL boundary.
- Addressed: eventual child reap and reader cleanup after bounded timeout.
- Open in this final fix scope: none.
- Manual visual verification remains a separate pre-existing acceptance item;
  these fixes do not change UI layout or styling.
