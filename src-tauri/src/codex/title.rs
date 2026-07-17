use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use anyhow::Result;
use rusqlite::{Connection, OpenFlags, OptionalExtension};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadPath {
    pub thread_id: String,
    pub rollout_path: String,
}

/// Converts the small Markdown subset emitted in Codex thread titles to the
/// plain text that belongs in a compact native UI. In particular, a title such
/// as `[Investigate import](thread://...)` should read as `Investigate import`,
/// not expose the internal thread target.
pub fn display_title(raw: &str) -> String {
    let mut output = String::with_capacity(raw.len());
    let mut cursor = 0;

    while let Some(open_offset) = raw[cursor..].find('[') {
        let open = cursor + open_offset;
        output.push_str(&raw[cursor..open]);
        let label_start = open + 1;
        let Some(label_end_offset) = raw[label_start..].find("](") else {
            output.push('[');
            cursor = label_start;
            continue;
        };
        let label_end = label_start + label_end_offset;
        let target_start = label_end + 2;
        let Some(target_end_offset) = raw[target_start..].find(')') else {
            output.push_str(&raw[open..]);
            cursor = raw.len();
            break;
        };
        output.push_str(&raw[label_start..label_end]);
        cursor = target_start + target_end_offset + 1;
    }

    output.push_str(&raw[cursor..]);
    output
        .replace(['`', '*', '_', '~'], "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn lookup_title(database_path: &Path, thread_id: &str) -> Result<Option<String>> {
    if !database_path.exists() {
        return Ok(None);
    }

    let connection = Connection::open_with_flags(
        database_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let title = connection
        .query_row(
            "SELECT title FROM threads WHERE id = ?1 AND title <> ''",
            [thread_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    Ok(title.map(|title| display_title(&title)))
}

pub fn lookup_titles(
    database_path: &Path,
    thread_ids: &HashSet<String>,
) -> Result<HashMap<String, String>> {
    if !database_path.exists() || thread_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let connection = Connection::open_with_flags(
        database_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let mut statement =
        connection.prepare("SELECT title FROM threads WHERE id = ?1 AND title <> ''")?;
    let mut titles = HashMap::new();
    for thread_id in thread_ids {
        if let Some(title) = statement
            .query_row([thread_id], |row| row.get::<_, String>(0))
            .optional()?
        {
            titles.insert(thread_id.clone(), display_title(&title));
        }
    }
    Ok(titles)
}

pub fn recent_thread_paths(database_path: &Path, limit: usize) -> Result<Vec<ThreadPath>> {
    if !database_path.exists() {
        return Ok(Vec::new());
    }
    let connection = Connection::open_with_flags(
        database_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let mut statement = connection.prepare(
        "SELECT id, rollout_path FROM threads WHERE archived = 0 ORDER BY updated_at_ms DESC LIMIT ?1",
    )?;
    let paths = statement
        .query_map([limit as i64], |row| {
            Ok(ThreadPath {
                thread_id: row.get(0)?,
                rollout_path: row.get(1)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(anyhow::Error::from)?;
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::{display_title, lookup_title, lookup_titles};
    use std::collections::HashSet;

    #[test]
    fn returns_a_non_empty_title_from_the_codex_state_database() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("state_5.sqlite");
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection
            .execute(
                "CREATE TABLE threads (id TEXT PRIMARY KEY, title TEXT NOT NULL)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO threads (id, title) VALUES (?1, ?2)",
                [
                    "00000000-0000-4000-8000-000000000001",
                    "Useful session title",
                ],
            )
            .unwrap();
        drop(connection);

        assert_eq!(
            lookup_title(&database, "00000000-0000-4000-8000-000000000001").unwrap(),
            Some("Useful session title".into())
        );
    }

    #[test]
    fn treats_missing_or_empty_titles_as_no_title() {
        let temp = tempfile::tempdir().unwrap();
        assert_eq!(
            lookup_title(&temp.path().join("missing.sqlite"), "id").unwrap(),
            None
        );
    }

    #[test]
    fn reads_multiple_titles_through_one_database_query_plan() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("state_5.sqlite");
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection
            .execute(
                "CREATE TABLE threads (id TEXT PRIMARY KEY, title TEXT NOT NULL)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO threads (id, title) VALUES ('one', 'First'), ('two', 'Second')",
                [],
            )
            .unwrap();
        drop(connection);

        let titles = lookup_titles(&database, &HashSet::from(["one".into(), "two".into()]));

        assert_eq!(titles.unwrap().get("two"), Some(&"Second".to_owned()));
    }

    #[test]
    fn renders_markdown_links_as_a_plain_thread_title() {
        assert_eq!(
            display_title("[@落地 data-ingestion 契约](thread://019f6b90) **review**"),
            "@落地 data-ingestion 契约 review"
        );
    }
}
