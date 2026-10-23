//! Avvisi mostrati alla persona davanti al PC (messagebox di sistema su
//! Windows, best-effort altrove).

/// Da usare dentro al runtime async (es. il ciclo di enforcement): non blocca
/// il bot mentre il messaggio è a schermo.
pub fn show_ban_message(text: &str) {
    let text = text.to_string();

    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONSTOP;
        tokio::task::spawn_blocking(move || windows_message_box(&text, MB_ICONSTOP));
    }

    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("notify-send")
            .arg("NoPlay")
            .arg(&text)
            .spawn();
    }
}

/// Messagebox sincrona, bloccante finché l'utente non la chiude. Da chiamare
/// solo fuori dal runtime async (es. dal thread della tray) o già dentro un
/// `spawn_blocking`.
#[cfg(windows)]
pub fn windows_message_box(text: &str, icon_flag: u32) {
    use std::{ffi::OsStr, iter::once, os::windows::ffi::OsStrExt};

    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_OK, MB_SETFOREGROUND, MB_TOPMOST,
    };

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(once(0)).collect()
    }

    let msg = wide(text);
    let caption = wide("NoPlay");
    // SAFETY: entrambe le stringhe sono terminate da nul e vive per tutta la chiamata.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            msg.as_ptr(),
            caption.as_ptr(),
            MB_OK | icon_flag | MB_TOPMOST | MB_SETFOREGROUND,
        );
    }
}
