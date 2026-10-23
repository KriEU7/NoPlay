// In release su Windows gira senza finestra di console (resta visibile in Gestione attività).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod autostart;
mod config;
mod enforcer;
mod schedule;
#[cfg(windows)]
mod tray;
mod ui;
mod update;

use std::{
    io::Write,
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
    #[command(
        description = "[add <giorni> <HH:MM-HH:MM> | del <n> | clear]: fasce in cui i giochi sono permessi"
    )]
    Schedule(String),
    #[command(description = "blocca lo schermo del PC (richiede la password per rientrare)")]
    Lock,
    #[command(description = "controlla e installa un aggiornamento dalle release di GitHub")]
    Update,
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
                    format!(
                        "in pausa ancora {} min",
                        (t - Instant::now()).as_secs() / 60 + 1
                    )
                }
                _ => "nessuna pausa".into(),
            };
            let games = if open.is_empty() {
                "nessuno".to_string()
            } else {
                open.iter()
                    .map(|(_, n)| n.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let window = if schedule::allowed_now(&cfg.schedule) {
                "giochi permessi ora"
            } else {
                "fuori fascia"
            };
            format!(
                "Blocco: {}\nFascia: {window}\nPausa: {paused}\nGiochi aperti: {games}",
                if cfg.enforce { "attivo" } else { "disattivato" }
            )
        }
        Command::Kill => {
            let cfg = state.config.lock().await.clone();
            let killed = enforcer::kill_blocked(&mut System::new(), &cfg);
            if killed.is_empty() {
                "Nessun gioco aperto.".into()
            } else {
                format!("Chiusi: {}", killed.join(", "))
            }
        }
        Command::Enforce(arg) => match arg.trim().to_lowercase().as_str() {
            "on" | "off" => {
                let on = arg.trim().eq_ignore_ascii_case("on");
                let mut cfg = state.config.lock().await;
                cfg.enforce = on;
                save_reply(
                    &cfg,
                    if on {
                        "Blocco attivato."
                    } else {
                        "Blocco disattivato."
                    },
                )
            }
            _ => "Uso: /enforce on|off".into(),
        },
        Command::Allow(minutes) => {
            *state.paused_until.lock().await =
                Some(Instant::now() + Duration::from_secs(minutes * 60));
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
        Command::Schedule(args) => {
            let mut cfg = state.config.lock().await;
            let parts: Vec<&str> = args.split_whitespace().collect();
            match parts.as_slice() {
                [] | ["list"] => schedule_list(&cfg),
                ["add", days, range] => match schedule::parse_window(days, range) {
                    Ok(w) => {
                        let text = format!("Aggiunta fascia: {}", w.describe());
                        cfg.schedule.push(w);
                        save_reply(&cfg, &text)
                    }
                    Err(e) => e,
                },
                ["del", n] => match n.parse::<usize>() {
                    Ok(n) if (1..=cfg.schedule.len()).contains(&n) => {
                        let w = cfg.schedule.remove(n - 1);
                        save_reply(&cfg, &format!("Rimossa fascia: {}", w.describe()))
                    }
                    _ => "Numero di fascia non valido, vedi /schedule".into(),
                },
                ["clear"] => {
                    cfg.schedule.clear();
                    save_reply(&cfg, "Fasce rimosse: i giochi sono sempre bloccati.")
                }
                _ => "Uso:\n/schedule\n/schedule add lun-ven 17:00-19:00\n/schedule add sab,dom 10:00-12:30\n/schedule del 1\n/schedule clear".into(),
            }
        }
        Command::Lock => match lock_screen() {
            Ok(()) => "Schermo bloccato (serve la password per rientrare).".into(),
            Err(e) => format!("Impossibile bloccare lo schermo: {e}"),
        },
        Command::Update => match update::check_and_apply().await {
            Ok(Some(tag)) => {
                bot.send_message(msg.chat.id, format!("🔄 Aggiornato a {tag}, riavvio..."))
                    .await?;
                tokio::time::sleep(Duration::from_millis(300)).await;
                std::process::exit(0);
            }
            Ok(None) => "Nessun aggiornamento disponibile.".into(),
            Err(e) => format!("Controllo aggiornamenti fallito: {e:#}"),
        },
    };

    bot.send_message(msg.chat.id, reply).await?;
    Ok(())
}

fn schedule_list(cfg: &Config) -> String {
    if cfg.schedule.is_empty() {
        return "Nessuna fascia: i giochi sono sempre bloccati.".into();
    }
    let lines: Vec<String> = cfg
        .schedule
        .iter()
        .enumerate()
        .map(|(i, w)| format!("{}. {}", i + 1, w.describe()))
        .collect();
    format!("Giochi permessi in queste fasce:\n{}", lines.join("\n"))
}

fn save_reply(cfg: &Config, ok: &str) -> String {
    match cfg.save() {
        Ok(()) => ok.to_string(),
        Err(e) => format!("{ok} (ma non sono riuscito a salvare la config: {e})"),
    }
}

/// Scrive un messaggio per chi ha lanciato il programma da terminale.
/// Senza console (Windows release) si aggancia a quella del processo padre.
fn say(msg: &str) {
    #[cfg(all(windows, not(debug_assertions)))]
    {
        extern "system" {
            fn AttachConsole(process_id: u32) -> i32;
        }
        // SAFETY: chiamata Win32 senza puntatori; fallisce innocuamente se non c'è un padre con console.
        if unsafe { AttachConsole(u32::MAX) } != 0 {
            if let Ok(mut con) = std::fs::OpenOptions::new().write(true).open("CONOUT$") {
                let _ = writeln!(con, "{msg}");
            }
        }
    }
    #[cfg(not(all(windows, not(debug_assertions))))]
    println!("{msg}");
}

/// Senza console gli errori andrebbero persi: li accodiamo a `noplay.log` accanto all'eseguibile.
fn log_error(msg: &str) {
    let path = config::exe_dir().unwrap_or_default().join("noplay.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{msg}");
    }
}

#[tokio::main]
async fn main() {
    let result = match std::env::args().nth(1).as_deref() {
        Some("install") => autostart::install().map(|m| say(&m)),
        Some("uninstall") => autostart::uninstall().map(|m| say(&m)),
        _ => run().await,
    };
    if let Err(e) = result {
        let msg = format!("errore: {e:#}");
        log_error(&msg);
        say(&msg);
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    pretty_env_logger::init();
    let cfg = Config::load()?;
    let bot = Bot::new(cfg.bot_token.clone());
    let owners = cfg.owner_ids.clone();
    let state = Arc::new(AppState::new(cfg));

    log::info!("NoPlay avviato (visibile, nessuna modalità nascosta)");
    let host = System::host_name().unwrap_or_else(|| "PC".into());
    for id in owners {
        let _ = bot
            .send_message(ChatId(id as i64), format!("✅ NoPlay avviato su {host}"))
            .await;
    }
    tokio::spawn(enforcer::run(bot.clone(), state.clone()));
    tokio::spawn(update::run_periodic(bot.clone(), state.clone()));
    #[cfg(windows)]
    tray::spawn(bot.clone(), state.clone());

    let handler = Update::filter_message()
        .filter_command::<Command>()
        .endpoint(answer);
    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .build()
        .dispatch()
        .await;
    Ok(())
}
