mod config;
mod enforcer;

use std::{
    process::Command as Proc,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Result;
use sysinfo::{ProcessesToUpdate, System};
use teloxide::{prelude::*, utils::command::BotCommands};

use config::{normalize, Config};
use enforcer::AppState;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Comandi disponibili:")]
enum Command {
    #[command(description = "mostra questo aiuto")]
    Help,
    #[command(description = "stato: blocco attivo/pausa e giochi aperti")]
    Status,
    #[command(description = "chiudi subito i giochi aperti")]
    Kill,
    #[command(description = "on|off: attiva o disattiva il blocco")]
    Enforce(String),
    #[command(description = "<minuti>: permetti i giochi per un tempo limitato")]
    Allow(u64),
    #[command(description = "<nome>: aggiungi un processo alla blocklist")]
    Add(String),
    #[command(description = "<nome>: togli un processo dalla blocklist")]
    Remove(String),
    #[command(description = "mostra la blocklist")]
    List,
    #[command(description = "blocca lo schermo del PC")]
    Lock,
}

fn lock_screen() -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = Proc::new("rundll32.exe");
        c.args(["user32.dll,LockWorkStation"]);
        c
    };
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = Proc::new("pmset");
        c.arg("displaysleepnow");
        c
    };
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut cmd = {
        let mut c = Proc::new("loginctl");
        c.arg("lock-session");
        c
    };
    cmd.status().map(|_| ())
}

async fn answer(bot: Bot, msg: Message, cmd: Command, state: Arc<AppState>) -> ResponseResult<()> {
    // Solo i proprietari configurati possono usare il bot; gli altri vengono ignorati.
    let uid = msg.from.as_ref().map(|u| u.id.0).unwrap_or(0);
    if !state.config.lock().await.owner_ids.contains(&uid) {
        log::warn!("accesso negato all'utente {uid}");
        return Ok(());
    }

    let reply = match cmd {
        Command::Help => Command::descriptions().to_string(),
        Command::Status => {
            let cfg = state.config.lock().await.clone();
            let mut sys = System::new();
            sys.refresh_processes(ProcessesToUpdate::All, true);
            let open = enforcer::running_blocked(&sys, &cfg);
            let paused = match *state.paused_until.lock().await {
                Some(t) if t > Instant::now() => {
                    format!("in pausa ancora {} min", (t - Instant::now()).as_secs() / 60 + 1)
                }
                _ => "nessuna pausa".into(),
            };
            let games = if open.is_empty() {
                "nessuno".to_string()
            } else {
                open.iter().map(|(_, n)| n.as_str()).collect::<Vec<_>>().join(", ")
            };
            format!(
                "Blocco: {}\nPausa: {paused}\nGiochi aperti: {games}",
                if cfg.enforce { "attivo" } else { "disattivato" }
            )
        }
        Command::Kill => {
            let cfg = state.config.lock().await.clone();
            let killed = enforcer::kill_blocked(&mut System::new(), &cfg);
            if killed.is_empty() { "Nessun gioco aperto.".into() } else { format!("Chiusi: {}", killed.join(", ")) }
        }
        Command::Enforce(arg) => match arg.trim().to_lowercase().as_str() {
            "on" | "off" => {
                let on = arg.trim().eq_ignore_ascii_case("on");
                let mut cfg = state.config.lock().await;
                cfg.enforce = on;
                save_reply(&cfg, if on { "Blocco attivato." } else { "Blocco disattivato." })
            }
            _ => "Uso: /enforce on|off".into(),
        },
        Command::Allow(minutes) => {
            *state.paused_until.lock().await = Some(Instant::now() + Duration::from_secs(minutes * 60));
            format!("Giochi permessi per {minutes} minuti, poi il blocco riparte.")
        }
        Command::Add(name) => {
            let n = normalize(&name);
            if n.is_empty() {
                "Uso: /add <nome processo>".into()
            } else {
                let mut cfg = state.config.lock().await;
                if !cfg.blocklist.contains(&n) {
                    cfg.blocklist.push(n.clone());
                }
                save_reply(&cfg, &format!("Aggiunto: {n}"))
            }
        }
        Command::Remove(name) => {
            let n = normalize(&name);
            let mut cfg = state.config.lock().await;
            let before = cfg.blocklist.len();
            cfg.blocklist.retain(|b| *b != n);
            if cfg.blocklist.len() == before {
                format!("{n} non era in lista.")
            } else {
                save_reply(&cfg, &format!("Rimosso: {n}"))
            }
        }
        Command::List => state.config.lock().await.blocklist.join("\n"),
        Command::Lock => match lock_screen() {
            Ok(()) => "Schermo bloccato.".into(),
            Err(e) => format!("Impossibile bloccare lo schermo: {e}"),
        },
    };

    bot.send_message(msg.chat.id, reply).await?;
    Ok(())
}

fn save_reply(cfg: &Config, ok: &str) -> String {
    match cfg.save() {
        Ok(()) => ok.to_string(),
        Err(e) => format!("{ok} (ma non sono riuscito a salvare la config: {e})"),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();
    let cfg = Config::load()?;
    let bot = Bot::new(cfg.bot_token.clone());
    let state = Arc::new(AppState::new(cfg));

    log::info!("NoPlay avviato (visibile, nessuna modalità nascosta)");
    tokio::spawn(enforcer::run(bot.clone(), state.clone()));

    let handler = Update::filter_message().filter_command::<Command>().endpoint(answer);
    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .build()
        .dispatch()
        .await;
    Ok(())
}
