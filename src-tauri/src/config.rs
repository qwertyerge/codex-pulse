use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::model::ThemeMode;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub schema_version: u32,
    pub always_on_top: bool,
    pub window_visible: bool,
    pub launch_at_login: bool,
    pub monitoring_enabled: bool,
    pub locale: String,
    pub theme: ThemeMode,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            always_on_top: false,
            window_visible: true,
            launch_at_login: false,
            monitoring_enabled: false,
            locale: "system".into(),
            theme: ThemeMode::System,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn for_user() -> Self {
        let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
        Self::new(base.join("CodexPulse/config.json"))
    }

    pub fn load(&self) -> Result<AppConfig> {
        if !self.path.exists() {
            return Ok(AppConfig::default());
        }
        Ok(serde_json::from_slice(&fs::read(&self.path)?).unwrap_or_default())
    }

    pub fn save(&self, config: &AppConfig) -> Result<()> {
        ensure_parent(&self.path)?;
        let temporary = self.path.with_extension("tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(config)?)?;
        fs::rename(temporary, &self.path)?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, ConfigStore};
    use crate::model::ThemeMode;

    #[test]
    fn persists_the_pin_state_across_loads() {
        let temp = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(temp.path().join("config.json"));
        let config = AppConfig {
            always_on_top: true,
            ..AppConfig::default()
        };

        store.save(&config).unwrap();
        assert!(store.load().unwrap().always_on_top);
    }

    #[test]
    fn missing_config_uses_safe_defaults() {
        let temp = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(temp.path().join("missing.json"));

        assert_eq!(store.load().unwrap(), AppConfig::default());
    }

    #[test]
    fn persists_an_explicit_theme_choice() {
        let temp = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(temp.path().join("config.json"));
        let config = AppConfig {
            theme: ThemeMode::Dark,
            ..AppConfig::default()
        };

        store.save(&config).unwrap();

        assert_eq!(store.load().unwrap().theme, ThemeMode::Dark);
        assert_eq!(AppConfig::default().theme, ThemeMode::System);
    }
}
