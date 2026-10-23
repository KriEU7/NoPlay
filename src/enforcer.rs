use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use sysinfo::{ProcessesToUpdate, System};
use teloxide::{prelude::*, types::ChatId};
use tokio::sync::Mutex;

use crate::{config::Config, schedule};

pub struct AppState {
    pub config: Mutex<Config>,
    /// Finché è nel futuro, i giochi sono temporaneamente permessi.
    pub paused_until: Mutex<Option<Instant>>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self { config: Mutex::new(config), paused_until: Mutex::new(None) }
    }

    pub async fn is_paused(&self) -> bool {
        matches!(*self.paused_until.lock().await, Some(t) if t > Instant::now())
    }
}

/// Elenca i processi in esecuzione che sono nella blocklist: (pid, nome).
pub fn running_blocked(sys: &System, cfg: &Config) -> Vec<(u32, String)> {
    sys.processes()
        .iter()
        .filter_map(|(pid, p)| {
            let name = p.name().to_string_lossy().into_owned();
            cfg.is_blocked(&name).then(|| (pid.as_u32(), name))
        })
        .collect()
}

/// Chiude tutti i giochi in blocklist; restituisce i nomi chiusi.
pub fn kill_blocked(sys: &mut System, cfg: &Config) -> Vec<String> {
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let mut killed = Vec::new();
    for (pid, name) in running_blocked(sys, cfg) {
        if let Some(p) = sys.process(sysinfo::Pid::from_u32(pid)) {
            if p.kill() {
                killed.push(name);
            }
        }
    }
    killed.sort();
    killed.dedup();
    killed
}

/// Ciclo di controllo in background: chiude i giochi e avvisa i proprietari.
pub async fn run(bot: Bot, state: Arc<AppState>) {
    let mut sys = System::new();
    loop {
        let (enforce, interval, owners, in_window) = {
            let c = state.config.lock().await;
            (c.enforce, c.poll_interval_seconds.max(1), c.owner_ids.clone(), schedule::allowed_now(&c.schedule))
        };

        if enforce && !in_window && !state.is_paused().await {
            let killed = {
                let c = state.config.lock().await;
                kill_blocked(&mut sys, &c)
            };
            if !killed.is_empty() {
                log::info!("chiusi: {killed:?}");
                let text = format!("🚫 Gioco chiuso: {}", killed.join(", "));
                for id in owners {
                    let _ = bot.send_message(ChatId(id as i64), &text).await;
                }
            }
        }

        tokio::time::sleep(Duration::from_secs(interval)).await;
    }
}
