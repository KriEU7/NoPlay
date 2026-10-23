use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::schedule::Window;

/// Config di partenza, la stessa di `config.example.json`.
const TEMPLATE: &str = include_str!("../config.example.json");

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

/// Cartella dell'eseguibile (all'avvio automatico la cartella di lavoro è un'altra).
pub fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(PathBuf::from)
}

/// `NOPLAY_CONFIG`, altrimenti `./config.json` se esiste, altrimenti accanto all'eseguibile.
pub fn config_path() -> PathBuf {
    if let Some(p) = std::env::var_os("NOPLAY_CONFIG") {
        return PathBuf::from(p);
    }
    let local = PathBuf::from("config.json");
    if local.exists() {
        return local;
    }
    exe_dir().map(|d| d.join("config.json")).unwrap_or(local)
}

/// Normalizza un nome di processo: minuscolo e senza estensione `.exe`.
pub fn normalize(name: &str) -> String {
    let n = name.trim().to_lowercase();
    n.strip_suffix(".exe").map(str::to_owned).unwrap_or(n)
}

impl Config {
    /// Carica la config; se manca ne crea una da `config.example.json` e si ferma
    /// finché l'utente non ha inserito token e ID Telegram.
    pub fn load() -> Result<Self> {
        let path = config_path();
        if !path.exists() {
            fs::write(&path, TEMPLATE).with_context(|| format!("impossibile creare {}", path.display()))?;
            anyhow::bail!(
                "config non trovata: ne ho creata una in {}. Inserisci bot_token e owner_ids, poi riavvia",
                path.display()
            );
        }
        let raw = fs::read_to_string(&path).with_context(|| format!("impossibile leggere {}", path.display()))?;
        let mut cfg: Config = serde_json::from_str(&raw).with_context(|| format!("{} non valido", path.display()))?;
        cfg.blocklist = cfg.blocklist.iter().map(|s| normalize(s)).collect();
        anyhow::ensure!(
            !cfg.bot_token.is_empty() && !cfg.bot_token.starts_with("INSERISCI"),
            "bot_token non impostato in {}",
            path.display()
        );
        anyhow::ensure!(!cfg.owner_ids.is_empty(), "owner_ids è vuoto in {}: nessuno potrebbe usare il bot", path.display());
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
