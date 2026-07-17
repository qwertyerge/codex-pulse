use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};
#[derive(Deserialize)]
struct Entry {
    id: String,
    thread_name: Option<String>,
}
pub fn lookup_thread_names(home: &Path, ids: &HashSet<String>) -> HashMap<String, String> {
    let Ok(file) = File::open(home.join("session_index.jsonl")) else {
        return HashMap::new();
    };
    BufReader::new(file)
        .lines()
        .filter_map(Result::ok)
        .filter_map(|line| serde_json::from_str::<Entry>(&line).ok())
        .filter_map(|entry| {
            entry
                .thread_name
                .filter(|name| !name.trim().is_empty())
                .map(|name| (entry.id, name))
        })
        .filter(|(id, _)| ids.contains(id))
        .collect()
}
