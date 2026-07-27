# Git Support Design

## Goal

Enrich each active Codex session with local Git repository identity and branch
context. A session card should show the primary checkout's project name, the
current worktree branch, and a hover card containing the primary checkout's
default branch and its tracking remote URL.

The feature must preserve the current project-link action: clicking the project
name opens the session's original working directory, even when the visible
project name comes from a different primary worktree.

## Confirmed Product Semantics

- "Primary checkout" means Git's primary worktree, not the session's linked
  worktree and not whichever worktree currently has the repository's default
  branch checked out.
- The primary worktree's currently checked-out local branch is the default
  branch for this feature.
- The default upstream is that local branch's configured tracking upstream. The
  remote repository address comes from that upstream's remote; the
  implementation must not assume the names `main` or `origin`.
- A Git worktree's current branch is read from that worktree. Detached HEAD is
  displayed as the localized equivalent of "No branch".
- A non-Git working directory retains the existing directory-basename link and
  has no branch label or Git hover card.
- Missing default branch, upstream, or remote information does not discard other
  Git information. Missing hover-card values are displayed as the localized
  equivalent of "Not configured".
- Remote URLs are sanitized before they enter persistence or the frontend.
  HTTPS URL userinfo is removed; an SSH address such as
  `git@host.example:owner/repository.git` is preserved.

## Architecture

Codex session discovery remains responsible only for parsing Codex state:
JSONL and SQLite sources flow through `SessionRegistry` into a basic list of
`SessionSnapshot` values.

A new Rust Git boundary enriches those snapshots during the existing
`spawn_blocking` reconciliation:

1. `GitCommandRunner` executes bounded, read-only local Git commands without a
   shell.
2. `GitRepositoryResolver` maps a session working directory to its Git
   worktree, primary worktree, current branch, default branch, default
   upstream, and remote URL.
3. `GitCacheStore` persists last-known-good repository metadata in an
   independent SQLite database.

Repository work is deduplicated within a reconciliation. Multiple active
sessions in the same repository share the stable repository result, while each
distinct worktree keeps its own current branch result. Git commands never run
from the WebView invoke path or the synchronous `get_snapshot` command.

The Codex scanner, Git resolver, persistence store, and Vue presentation remain
separate units with explicit models between them.

## Git Resolution

The resolver uses the Git executable on `PATH` and passes arguments directly to
`std::process::Command`. It does not construct shell strings.

For each distinct session worktree, the resolver:

1. Runs Git from the session working directory to identify the repository,
   canonical common Git directory, and current worktree.
2. Reads the current worktree's symbolic `HEAD`. A missing symbolic branch with
   a valid repository identity is detached HEAD, not a resolution failure.
3. Reads `git worktree list --porcelain` and selects Git's primary worktree.
4. Reads the primary worktree entry's branch as the feature's default branch.
5. Resolves that local branch's configured tracking upstream.
6. Derives the upstream remote and reads its configured URL.
7. Sanitizes the URL before returning or persisting it.

The Git environment sets `GIT_OPTIONAL_LOCKS=0` and
`GIT_TERMINAL_PROMPT=0`. Resolution performs no fetch, network request, hook,
or repository mutation.

Each Git child process has a two-second timeout and is killed when the timeout
expires. One repository's timeout or invalid state cannot fail the surrounding
Codex session scan.

## Data Model

The Rust and TypeScript session contracts add an optional Git context:

```text
SessionSnapshot
  git?: SessionGitContext

SessionGitContext
  projectName: string
  primaryCheckoutPath: string
  branch?: string
  defaultBranch?: string
  defaultUpstream?: string
  remoteUrl?: string
```

`git` being absent means the working directory is not a Git repository, or
repository identity could not be established for this reconciliation.

`git` being present while `branch` is absent means the session worktree is in
detached HEAD state. This distinction prevents non-Git directories from being
mislabelled as detached.

`defaultUpstream` remains part of the model and cache so the remote URL's
provenance is explicit. The first UI does not give it a separate display row.

The visible project name is `git.projectName` when Git context exists and the
existing working-directory basename otherwise. The project-link event always
emits the unmodified `session.cwd`.

## Persistence

Git metadata uses a dedicated `git-cache.sqlite3` in the Codex Pulse user data
directory. It remains separate from `config.json`, which continues to contain
only user settings.

SQLite schema versions use `PRAGMA user_version`. Version 1 contains:

```sql
CREATE TABLE repositories (
  repository_key TEXT PRIMARY KEY NOT NULL,
  primary_checkout_path TEXT NOT NULL,
  project_name TEXT NOT NULL,
  default_branch TEXT,
  default_upstream TEXT,
  remote_url TEXT,
  updated_at_ms INTEGER NOT NULL
);
```

`repository_key` is the canonical common Git directory. Repository metadata is
upserted atomically after successful resolution. A successful result containing
an absent optional field may replace a previously populated value because that
represents a real configuration change. A timeout, command error, invalid
output, or empty failed result must not overwrite the last-known-good row.

The database stores only stable repository-level information. It does not store
thread IDs, session IDs, raw session working directories, or worktree current
branches.

Each reconciliation reads the current worktree branch live. When repository
identity succeeds but a later stable-field query fails, the resolver may
combine the live worktree result with the last-known-good repository row.

If the cache database cannot be opened or migrated, Git enrichment continues
with in-memory results for that reconciliation. Cache failure must not prevent
Codex Pulse from starting or displaying sessions.

## Session Card Presentation

The second card line becomes a focused `ProjectIdentity` presentation unit:

- The project-name link remains the leading element.
- Git sessions add a visually subdued branch label immediately after the link.
  The label contains a branch icon and either the current branch name or the
  localized "No branch" fallback.
- Non-Git sessions do not render the branch label.
- Both names truncate safely in narrow windows without displacing the session
  action or timer layout.

The project-name link no longer has the full working directory in its `title`
attribute. The old native full-path tooltip is removed.

Hovering the project link, or focusing it with a keyboard, opens a read-only
floating card:

- Card title: project name.
- Row 1: localized "Default branch" and the resolved value, or localized
  "Not configured".
- Row 2: localized "Remote repository" and the sanitized URL, or localized
  "Not configured".

The hover card uses a body `Teleport` and fixed positioning based on the
project link's bounding rectangle. It flips above the anchor near the bottom of
the window so the scrollable session list cannot clip it. The card has no
interactive controls and does not intercept pointer events. It uses tooltip
semantics and is associated with the focused project link.

Light and dark appearances, reduced motion, and all existing locales (English,
Simplified Chinese, French, and German) receive corresponding styles and copy.

## Error Handling and Degradation

- A non-Git directory is a normal `git: None` result.
- Detached HEAD is a normal Git result with `branch: None`.
- Missing default branch, upstream, or remote URL produces a partial Git result
  whose unavailable popup values display "Not configured".
- A Git timeout, missing executable, malformed output, or inaccessible
  repository degrades only that repository's enrichment.
- A stable-field query failure may use a last-known-good SQLite row after the
  repository key has been established.
- SQLite failure disables persistence for the affected run without affecting
  the base session snapshot.
- Raw remote URLs containing HTTP(S) userinfo never enter logs, SQLite,
  serialized snapshots, or the WebView.

No toast, dialog, new global monitoring state, or network retry is introduced.

## Testing

Implementation follows red-green-refactor.

Rust coverage uses temporary real Git repositories for:

- a normal primary checkout and local branch;
- a linked worktree resolving back to the primary worktree;
- a current linked-worktree branch distinct from the primary default branch;
- a tracking upstream whose remote is not named `origin`;
- detached HEAD;
- a non-Git directory;
- missing upstream and remote configuration.

A controllable command-runner test double covers timeouts, process failures,
malformed output, and repository-level deduplication. URL tests prove that
HTTP(S) userinfo is removed while ordinary HTTPS and SSH URLs are preserved.

SQLite tests cover schema creation, versioning, atomic upsert, successful NULL
replacement, last-known-good retention after resolution failure, and reading
the cache through a fresh store instance.

Registry/enrichment tests prove that:

- multiple sessions sharing a repository reuse stable resolution;
- distinct worktrees retain distinct current branches;
- non-Git and failed Git enrichment do not remove or fail a session snapshot.

Vue tests cover:

- removal of the full-path `title`;
- Git project name and exact original-cwd click payload;
- branch icon and current branch;
- detached "No branch";
- absence of Git decoration for non-Git sessions;
- hover and keyboard-focus hover-card visibility;
- default branch, remote URL, and "Not configured" values;
- narrow-window truncation and top/bottom placement contracts;
- English, Simplified Chinese, French, and German copy.

Verification runs focused tests first, followed by:

```bash
pnpm test
pnpm build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

Desktop visual verification separately covers light and dark appearances,
hover-card placement for a card near the bottom of the list, an ordinary
branch, detached HEAD, and a non-Git working directory. Automated verification
and manual visual verification are reported as distinct evidence.

## Out of Scope

- Fetching or contacting a remote repository.
- Assuming or configuring `origin`, `main`, or any other branch or remote name.
- Changing branches, upstreams, remotes, worktrees, or repository files.
- Showing commit hashes, dirty status, ahead/behind counts, tags, or submodules.
- Persisting active session snapshots or current worktree branches.
- Changing the Open Codex Task action.
- Displaying raw working-directory paths in the new hover card.
