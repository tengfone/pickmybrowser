use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

use crate::browsers::Browser;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    /// Glob-ish host/path pattern, e.g. `*.figma.com` or `github.com/acme/*`.
    pub pattern: String,
    pub browser_id: String,
    #[serde(default)]
    pub profile_id: Option<String>,
    #[serde(default = "yes")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recent {
    pub url: String,
    pub at: u64,
    #[serde(default)]
    pub browser_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// "system" | "dark" | "light"
    pub theme: String,
    /// "cursor" | "center"
    pub position: String,
    pub store_recents: bool,
    pub recent_limit: usize,
    pub strip_tracking: bool,
    pub warn_lookalike: bool,
    /// Rule countdown in ms; 0 = open immediately.
    pub rule_countdown_ms: u64,
    pub show_tray: bool,
    pub onboarded: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            position: "cursor".into(),
            store_recents: true,
            recent_limit: 20,
            strip_tracking: true,
            warn_lookalike: true,
            rule_countdown_ms: 1200,
            show_tray: true,
            onboarded: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub browsers: Vec<Browser>,
    pub rules: Vec<Rule>,
    pub recents: Vec<Recent>,
    pub settings: Settings,
}

fn yes() -> bool {
    true
}

/// E2E mode only exists in `--features wdio` builds; release builds ignore `PMB_E2E`.
pub fn is_e2e() -> bool {
    cfg!(feature = "wdio") && std::env::var_os("PMB_E2E").is_some_and(|v| !v.is_empty())
}

pub fn dir() -> PathBuf {
    // E2E builds keep their state in a throwaway folder chosen by the test runner.
    if is_e2e()
        && let Some(d) = std::env::var_os("PMB_CONFIG_DIR")
    {
        return PathBuf::from(d);
    }
    // Separate from the legacy WPF app (%AppData%\PickMyBrowser) so the two never clash.
    dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(crate::cleanup::IDENTIFIER)
}

fn path() -> PathBuf {
    dir().join("config.json")
}

impl Config {
    pub fn load() -> Self {
        fs::read_to_string(path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Writes via a temp file + rename so a crash mid-write can't corrupt the config.
    pub fn save(&self) {
        let Ok(json) = serde_json::to_string_pretty(self) else { return };
        let _ = fs::create_dir_all(dir());
        let tmp = path().with_extension("json.tmp");
        if fs::write(&tmp, json).is_ok() {
            let _ = fs::rename(tmp, path());
        }
    }

    pub fn push_recent(&mut self, url: &str, browser_id: Option<String>) {
        if !self.settings.store_recents {
            return;
        }
        self.recents.retain(|r| r.url != url);
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.recents.insert(0, Recent { url: url.into(), at, browser_id });
        self.recents.truncate(self.settings.recent_limit.max(1));
    }

    /// Merge freshly detected browsers with the saved list: keep the user's order and
    /// enabled flags, drop browsers that are gone, append new ones at the end.
    pub fn merge_detected(&mut self, detected: Vec<Browser>) {
        let mut merged: Vec<Browser> = Vec::new();
        for saved in &self.browsers {
            if let Some(found) = detected.iter().find(|b| b.id == saved.id) {
                let mut browser = found.clone();
                browser.enabled = saved.enabled;
                merged.push(browser);
            }
        }
        for browser in detected {
            if !merged.iter().any(|m| m.id == browser.id) {
                merged.push(browser);
            }
        }
        self.browsers = merged;
    }
}
