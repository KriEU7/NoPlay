//! Icona nella tray di Windows (solo Windows): un accesso rapido per chi sta
//! davanti al PC. Non serve per far funzionare il blocco, che il bot Telegram
//! gestisce comunque da solo; è solo una comodità in più, ben visibile.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use teloxide::prelude::*;
use tokio::runtime::Handle;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, TranslateMessage, MB_ICONINFORMATION, MSG,
};

use crate::{enforcer::AppState, ui::windows_message_box};

fn icon() -> Icon {
    const SIZE: u32 = 32;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for _ in 0..(SIZE * SIZE) {
        rgba.extend_from_slice(&[0xE0, 0x30, 0x30, 0xFF]); // quadrato rosso pieno
    }
    Icon::from_rgba(rgba, SIZE, SIZE).expect("icona di base non valida")
}

fn build_tray() -> tray_icon::Result<TrayIcon> {
    let status = MenuItem::with_id("status", "NoPlay — in esecuzione", false, None);
    let allow15 = MenuItem::with_id("allow15", "Permetti giochi 15 minuti", true, None);
    let lock = MenuItem::with_id("lock", "Blocca lo schermo", true, None);
    let exit = MenuItem::with_id("exit", "Esci", true, None);

    let menu = Menu::new();
    menu.append(&status).ok();
    menu.append(&PredefinedMenuItem::separator()).ok();
    menu.append(&allow15).ok();
    menu.append(&lock).ok();
    menu.append(&PredefinedMenuItem::separator()).ok();
    menu.append(&exit).ok();

    TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("NoPlay")
        .with_icon(icon())
        .build()
}

/// Avvia l'icona nella tray su un thread dedicato, con il proprio message
/// loop Win32 (richiesto da Windows per ricevere i click sull'icona/menu).
pub fn spawn(bot: Bot, state: Arc<AppState>) {
    let handle = Handle::current();
    std::thread::spawn(move || run(bot, state, handle));
}

fn run(bot: Bot, state: Arc<AppState>, handle: Handle) {
    let _tray = match build_tray() {
        Ok(t) => t,
        Err(e) => {
            log::warn!("icona nella tray non disponibile: {e}");
            return;
        }
    };

    loop {
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        // SAFETY: `msg` resta valido per tutta la chiamata; GetMessageW blocca
        // finché non arriva un messaggio (o WM_QUIT, gestito sotto).
        let ret = unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) };
        if ret <= 0 {
            break;
        }
        // SAFETY: `msg` è stato appena riempito dalla chiamata sopra.
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        if let Ok(event) = MenuEvent::receiver().try_recv() {
            handle_menu_event(&event, &bot, &state, &handle);
        }
    }
}

fn handle_menu_event(event: &MenuEvent, bot: &Bot, state: &Arc<AppState>, handle: &Handle) {
    if event.id() == "exit" {
        std::process::exit(0);
    } else if event.id() == "lock" {
        if let Err(e) = crate::lock_screen() {
            log::warn!("blocco schermo dalla tray fallito: {e}");
        }
    } else if event.id() == "allow15" {
        *state.paused_until.blocking_lock() = Some(Instant::now() + Duration::from_secs(15 * 60));
        let owners = state.config.blocking_lock().owner_ids.clone();
        let bot = bot.clone();
        handle.spawn(async move {
            for id in owners {
                let _ = bot
                    .send_message(
                        ChatId(id as i64),
                        "🎮 Giochi permessi per 15 minuti (dalla tray del PC).",
                    )
                    .await;
            }
        });
        windows_message_box("Giochi permessi per 15 minuti.", MB_ICONINFORMATION);
    }
}
