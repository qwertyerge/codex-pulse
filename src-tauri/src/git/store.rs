use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};

use crate::git::resolver::RepositoryRecord;

const SCHEMA_VERSION: i64 = 1;

pub struct GitCacheStore {
    path: PathBuf,
    connection: Connection,
}

impl GitCacheStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("could not create Git cache directory: {}", parent.display())
            })?;
        }

        let connection = Connection::open(path)
            .with_context(|| format!("could not open Git cache: {}", path.display()))?;
        let version =
            connection.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))?;
        if version > SCHEMA_VERSION {
            bail!(
                "Git cache schema version {version} is newer than supported version {SCHEMA_VERSION}"
            );
        }

        connection.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS repositories (
              repository_key TEXT PRIMARY KEY NOT NULL,
              primary_checkout_path TEXT NOT NULL,
              project_name TEXT NOT NULL,
              default_branch TEXT,
              default_upstream TEXT,
              remote_url TEXT,
              updated_at_ms INTEGER NOT NULL
            );
            PRAGMA user_version = 1;
            ",
        )?;

        Ok(Self {
            path: path.to_owned(),
            connection,
        })
    }

    pub fn load(&self, repository_key: &str) -> Result<Option<RepositoryRecord>> {
        self.connection
            .query_row(
                "
                SELECT repository_key, primary_checkout_path, project_name, default_branch,
                       default_upstream, remote_url, updated_at_ms
                FROM repositories
                WHERE repository_key = ?1
                ",
                params![repository_key],
                |row| {
                    Ok(RepositoryRecord {
                        repository_key: row.get(0)?,
                        primary_checkout_path: row.get(1)?,
                        project_name: row.get(2)?,
                        default_branch: row.get(3)?,
                        default_upstream: row.get(4)?,
                        remote_url: row.get(5)?,
                        updated_at_ms: row.get(6)?,
                    })
                },
            )
            .optional()
            .context("could not load Git repository metadata")
    }

    pub fn upsert(&self, record: &RepositoryRecord) -> Result<()> {
        self.connection
            .execute(
                "
                INSERT INTO repositories (
                  repository_key, primary_checkout_path, project_name, default_branch,
                  default_upstream, remote_url, updated_at_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ON CONFLICT(repository_key) DO UPDATE SET
                  primary_checkout_path = excluded.primary_checkout_path,
                  project_name = excluded.project_name,
                  default_branch = excluded.default_branch,
                  default_upstream = excluded.default_upstream,
                  remote_url = excluded.remote_url,
                  updated_at_ms = excluded.updated_at_ms
                ",
                params![
                    record.repository_key,
                    record.primary_checkout_path,
                    record.project_name,
                    record.default_branch,
                    record.default_upstream,
                    record.remote_url,
                    record.updated_at_ms,
                ],
            )
            .context("could not persist Git repository metadata")?;

        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::GitCacheStore;
    use crate::git::resolver::RepositoryRecord;

    fn repository_record() -> RepositoryRecord {
        RepositoryRecord {
            repository_key: "common-dir".into(),
            primary_checkout_path: "/src/project".into(),
            project_name: "project".into(),
            default_branch: Some("trunk".into()),
            default_upstream: Some("company/trunk".into()),
            remote_url: Some("https://example.com/acme/project.git".into()),
            updated_at_ms: 100,
        }
    }

    #[test]
    fn creates_version_one_schema_and_reopens_records() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("git-cache.sqlite3");
        let store = GitCacheStore::open(&path).unwrap();
        let record = repository_record();

        store.upsert(&record).unwrap();
        drop(store);

        let reopened = GitCacheStore::open(&path).unwrap();
        assert_eq!(reopened.load("common-dir").unwrap(), Some(record));
        assert_eq!(
            reopened
                .connection
                .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn a_successful_null_value_replaces_old_optional_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let store = GitCacheStore::open(&temp.path().join("git-cache.sqlite3")).unwrap();
        let mut record = repository_record();
        store.upsert(&record).unwrap();

        record.default_upstream = None;
        record.remote_url = None;
        record.updated_at_ms = 200;
        store.upsert(&record).unwrap();

        assert_eq!(store.load("common-dir").unwrap(), Some(record));
    }

    #[test]
    fn rejects_a_schema_version_newer_than_supported() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("git-cache.sqlite3");
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute_batch("PRAGMA user_version = 2;")
            .unwrap();
        drop(connection);

        let error = GitCacheStore::open(&path).err().unwrap();

        assert!(error.to_string().contains("newer than supported"));
    }
}
