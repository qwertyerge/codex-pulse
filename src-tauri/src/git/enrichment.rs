use std::{collections::HashMap, path::Path};

use crate::{
    git::{
        resolver::{GitMetadataSource, RepositoryRecord},
        store::GitCacheStore,
    },
    model::{SessionGitContext, SessionSnapshot},
};

pub struct GitSessionEnricher<S> {
    source: S,
    store: Option<GitCacheStore>,
}

impl<S: GitMetadataSource> GitSessionEnricher<S> {
    pub fn new(source: S, store: Option<GitCacheStore>) -> Self {
        Self { source, store }
    }

    pub fn enrich(&mut self, sessions: &mut [SessionSnapshot], now_ms: i64) {
        let mut records: HashMap<String, Option<RepositoryRecord>> = HashMap::new();

        for session in sessions {
            session.git = None;
            let identity = match self.source.resolve_worktree(Path::new(&session.cwd)) {
                Ok(Some(identity)) => identity,
                Ok(None) | Err(_) => continue,
            };
            let record = records
                .entry(identity.repository_key.clone())
                .or_insert_with(|| {
                    let cached = self
                        .store
                        .as_ref()
                        .and_then(|store| store.load(&identity.repository_key).ok())
                        .flatten();

                    match self
                        .source
                        .resolve_repository(Path::new(&session.cwd), &identity, now_ms)
                    {
                        Ok(record) => {
                            if let Some(store) = self.store.as_ref() {
                                let _ = store.upsert(&record);
                            }
                            Some(record)
                        }
                        Err(_) => cached,
                    }
                });
            let Some(record) = record.as_ref() else {
                continue;
            };

            session.git = Some(SessionGitContext {
                project_name: record.project_name.clone(),
                primary_checkout_path: record.primary_checkout_path.clone(),
                branch: identity.branch,
                default_branch: record.default_branch.clone(),
                default_upstream: record.default_upstream.clone(),
                remote_url: record.remote_url.clone(),
            });
        }
    }

    #[cfg(test)]
    fn source(&self) -> &S {
        &self.source
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, path::Path};

    use super::GitSessionEnricher;
    use crate::{
        git::{
            resolver::{GitMetadataSource, RepositoryRecord, WorktreeIdentity},
            store::GitCacheStore,
        },
        model::{SessionGitContext, SessionSnapshot},
    };

    struct FakeSource {
        repository_calls: Cell<usize>,
        fail_repository: bool,
    }

    impl FakeSource {
        fn repository_call_count(&self) -> usize {
            self.repository_calls.get()
        }
    }

    impl GitMetadataSource for FakeSource {
        fn resolve_worktree(&self, cwd: &Path) -> anyhow::Result<Option<WorktreeIdentity>> {
            let branch = match cwd.to_str().unwrap() {
                "/repo/worktree-a" => Some("feature/a"),
                "/repo/worktree-b" => Some("feature/b"),
                "/plain" => return Ok(None),
                other => anyhow::bail!("unexpected fixture path: {other}"),
            };
            Ok(Some(WorktreeIdentity {
                repository_key: "repo".into(),
                branch: branch.map(str::to_owned),
            }))
        }

        fn resolve_repository(
            &self,
            _cwd: &Path,
            identity: &WorktreeIdentity,
            now_ms: i64,
        ) -> anyhow::Result<RepositoryRecord> {
            self.repository_calls.set(self.repository_calls.get() + 1);
            if self.fail_repository {
                anyhow::bail!("stable metadata unavailable");
            }
            Ok(RepositoryRecord {
                repository_key: identity.repository_key.clone(),
                primary_checkout_path: "/src/project".into(),
                project_name: "project".into(),
                default_branch: Some("trunk".into()),
                default_upstream: Some("company/trunk".into()),
                remote_url: Some("https://example.com/acme/project.git".into()),
                updated_at_ms: now_ms,
            })
        }
    }

    fn session(cwd: &str) -> SessionSnapshot {
        SessionSnapshot {
            thread_id: cwd.into(),
            title: "Task".into(),
            cwd: cwd.into(),
            git: None,
            session_created_at_ms: 1_000,
            current_run_started_at_ms: 2_000,
            recent_event: None,
            last_user_message: None,
        }
    }

    #[test]
    fn deduplicates_repository_metadata_and_keeps_worktree_branches() {
        let source = FakeSource {
            repository_calls: Cell::new(0),
            fail_repository: false,
        };
        let mut sessions = vec![
            session("/repo/worktree-a"),
            session("/repo/worktree-b"),
            session("/plain"),
        ];
        sessions[2].git = Some(SessionGitContext {
            project_name: "stale".into(),
            primary_checkout_path: "/stale".into(),
            branch: Some("stale".into()),
            default_branch: Some("stale".into()),
            default_upstream: Some("stale/stale".into()),
            remote_url: Some("https://example.com/stale.git".into()),
        });
        let mut enricher = GitSessionEnricher::new(source, None);

        enricher.enrich(&mut sessions, 500);

        assert_eq!(enricher.source().repository_call_count(), 1);
        assert_eq!(
            sessions[0].git.as_ref().unwrap().branch.as_deref(),
            Some("feature/a")
        );
        assert_eq!(
            sessions[1].git.as_ref().unwrap().branch.as_deref(),
            Some("feature/b")
        );
        assert!(sessions[2].git.is_none());
    }

    #[test]
    fn retains_last_known_good_cache_when_live_repository_resolution_fails() {
        let temp = tempfile::tempdir().unwrap();
        let cache_path = temp.path().join("git-cache.sqlite3");
        let store = GitCacheStore::open(&cache_path).unwrap();
        let original_record = RepositoryRecord {
            repository_key: "repo".into(),
            primary_checkout_path: "/src/project".into(),
            project_name: "project".into(),
            default_branch: Some("trunk".into()),
            default_upstream: Some("company/trunk".into()),
            remote_url: Some("https://example.com/acme/project.git".into()),
            updated_at_ms: 100,
        };
        store.upsert(&original_record).unwrap();
        let source = FakeSource {
            repository_calls: Cell::new(0),
            fail_repository: true,
        };
        let mut sessions = vec![session("/repo/worktree-a")];
        let mut enricher = GitSessionEnricher::new(source, Some(store));

        enricher.enrich(&mut sessions, 600);

        let git = sessions[0].git.as_ref().unwrap();
        assert_eq!(git.project_name, "project");
        assert_eq!(git.branch.as_deref(), Some("feature/a"));
        assert_eq!(
            git.remote_url.as_deref(),
            Some("https://example.com/acme/project.git")
        );
        drop(enricher);
        let store_reopened = GitCacheStore::open(&cache_path).unwrap();
        assert_eq!(store_reopened.load("repo").unwrap(), Some(original_record));
    }
}
