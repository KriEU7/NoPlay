use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::schedule::Window;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub bot_token: String,
    /// Solo questi ID Telegram possono comandare il bot.
    pub owner_ids: Vec<u64>,
    /// Nomi dei processi (senza `.exe`, minuscoli) considerati videogiochi.
    pub blocklist: Vec<String>,
    #[serde(default = "default_poll")]
    pub poll_interval_seconds: u64,
    #[serde(default = "default_true")]
    pub enforce: bool,
    /// Fasce in cui i giochi sono permessi. Se vuota, i giochi sono sempre bloccati.
    #[serde(default)]
    pub schedule: Vec<Window>,
}

fn default_poll() -> u64 {
    5
}

fn default_true() -> bool {
    true
}

pub fn config_path() -> PathBuf {
    std::env::var_os("NOPLAY_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("config.json"))
}

/// Normalizza un nome di processo: minuscolo e senza estensione `.exe`.
pub fn normalize(name: &str) -> String {
    let n = name.trim().to_lowercase();
    n.strip_suffix(".exe").map(str::to_owned).unwrap_or(n)
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path();
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("impossibile leggere {} (copia config.example.json)", path.display()))?;
        let mut cfg: Config = serde_json::from_str(&raw).context("config.json non valido")?;
        cfg.blocklist = cfg.blocklist.iter().map(|s| normalize(s)).collect();
        anyhow::ensure!(!cfg.owner_ids.is_empty(), "owner_ids è vuoto: nessuno potrebbe usare il bot");
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        fs::write(config_path(), serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn is_blocked(&self, process_name: &str) -> bool {
        let n = normalize(process_name);
        self.blocklist.contains(&n)
    }
}
