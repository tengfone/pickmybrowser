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

pub fn dir() -> PathBuf {
    // Separate from the legacy WPF app (%AppData%\PickMyBrowser) so the two never clash.
    dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("com.tengfone.pickmybrowser")
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

    pub fn save(&self) {
        let _ = fs::create_dir_all(dir());
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let tmp = path().with_extension("json.tmp");
            if fs::write(&tmp, s).is_ok() {
                let _ = fs::rename(tmp, path());
            }
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

    /// Merge freshly detected browsers with saved order/enabled/hotkey state.
    pub fn merge_detected(&mut self, detected: Vec<Browser>) {
        let mut out: Vec<Browser> = Vec::new();
        for old in &self.browsers {
            if let Some(new) = detected.iter().find(|b| b.id == old.id) {
                let mut b = new.clone();
                b.enabled = old.enabled;
                out.push(b);
            }
        }
        for b in detected {
            if !out.iter().any(|o| o.id == b.id) {
                out.push(b);
            }
        }
        self.browsers = out;
    }
}
