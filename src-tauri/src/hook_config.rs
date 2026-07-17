use std::{fs, path::Path};

use anyhow::{Context, Result};
use serde_json::{json, Map, Value};

const EVENTS: [&str; 5] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "Stop",
];
const STATUS_MESSAGE: &str = "Refreshing Codex Pulse";

pub fn install(codex_home: &Path, command: &str) -> Result<()> {
    let path = codex_home.join("hooks.json");
    let mut document = read_document(&path)?;
    let hooks = document
        .as_object_mut()
        .context("Codex hooks.json must contain an object")?
        .entry("hooks")
        .or_insert_with(|| json!({}));
    let hooks = hooks
        .as_object_mut()
        .context("Codex hooks.json 'hooks' must contain an object")?;

    for event in EVENTS {
        let groups = hooks.entry(event).or_insert_with(|| json!([]));
        let groups = groups
            .as_array_mut()
            .with_context(|| format!("Codex hook '{event}' must contain an array"))?;
        if !groups.iter().any(|group| group_has_command(group, command)) {
            groups.push(json!({
                "hooks": [{
                    "type": "command",
                    "command": command,
                    "timeout": 2,
                    "statusMessage": STATUS_MESSAGE
                }]
            }));
        }
    }
    write_document(&path, &document)
}

pub fn is_installed(codex_home: &Path) -> bool {
    let path = codex_home.join("hooks.json");
    let Ok(document) = read_document(&path) else {
        return false;
    };
    EVENTS.into_iter().all(|event| {
        document
            .pointer(&format!("/hooks/{event}"))
            .and_then(Value::as_array)
            .is_some_and(|groups| groups.iter().any(group_has_codex_pulse_hook))
    })
}

fn group_has_command(group: &Value, command: &str) -> bool {
    group
        .get("hooks")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                item.get("command").and_then(Value::as_str) == Some(command)
                    && item.get("statusMessage").and_then(Value::as_str) == Some(STATUS_MESSAGE)
            })
        })
}

fn group_has_codex_pulse_hook(group: &Value) -> bool {
    group
        .get("hooks")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                item.get("statusMessage").and_then(Value::as_str) == Some(STATUS_MESSAGE)
                    && item
                        .get("command")
                        .and_then(Value::as_str)
                        .is_some_and(|command| command.contains("__hook"))
            })
        })
}

fn read_document(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(Value::Object(Map::new()));
    }
    serde_json::from_slice(&fs::read(path)?)
        .with_context(|| format!("Could not parse {}", path.display()))
}

fn write_document(path: &Path, document: &Value) -> Result<()> {
    let parent = path
        .parent()
        .context("Codex hooks.json has no parent directory")?;
    fs::create_dir_all(parent)?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(document)?)?;
    fs::rename(temporary, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{install, is_installed, STATUS_MESSAGE};

    #[test]
    fn merges_pulse_hooks_without_replacing_existing_groups() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("hooks.json");
        std::fs::write(
            &path,
            r#"{"hooks":{"PostToolUse":[{"matcher":"Write","hooks":[{"type":"command","command":"other"}]}]}}"#,
        )
        .unwrap();

        install(
            temp.path(),
            "\"/Applications/Codex Pulse.app/Contents/MacOS/CodexPulse\" __hook",
        )
        .unwrap();
        install(
            temp.path(),
            "\"/Applications/Codex Pulse.app/Contents/MacOS/CodexPulse\" __hook",
        )
        .unwrap();

        let document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(
            document["hooks"]["PostToolUse"].as_array().unwrap().len(),
            2
        );
        assert!(is_installed(temp.path()));
        assert_eq!(
            document["hooks"]["SessionStart"][0]["hooks"][0]["statusMessage"],
            STATUS_MESSAGE
        );
    }
}
