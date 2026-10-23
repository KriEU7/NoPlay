use std::process::Command;

use anyhow::{bail, Context, Result};

fn run(program: &str, args: &[&str]) -> Result<()> {
    let out = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("impossibile eseguire {program}"))?;
    if !out.status.success() {
        bail!(
            "{program} ha fallito: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(())
}

/// Registra l'avvio automatico al login (visibile in Utilità di pianificazione).
#[cfg(windows)]
pub fn install() -> Result<String> {
    let exe = std::env::current_exe()?;
    let command = format!("\"{}\"", exe.display());
    run(
        "schtasks",
        &[
            "/Create", "/TN", "NoPlay", "/TR", &command, "/SC", "ONLOGON", "/RL", "LIMITED", "/F",
        ],
    )
    .context("serve un terminale avviato come amministratore")?;
    Ok("Avvio automatico installato (attività pianificata 'NoPlay').".into())
}

#[cfg(windows)]
pub fn uninstall() -> Result<String> {
    run("schtasks", &["/Delete", "/TN", "NoPlay", "/F"])?;
    Ok("Avvio automatico rimosso.".into())
}

#[cfg(target_os = "linux")]
fn unit_path() -> Result<std::path::PathBuf> {
    let home = std::env::var_os("HOME").context("HOME non impostata")?;
    Ok(std::path::PathBuf::from(home).join(".config/systemd/user/noplay.service"))
}

/// Installa e avvia un servizio systemd utente.
#[cfg(target_os = "linux")]
pub fn install() -> Result<String> {
    let exe = std::env::current_exe()?;
    let dir = exe.parent().context("percorso eseguibile non valido")?;
    let unit = format!(
        "[Unit]\nDescription=NoPlay\n\n[Service]\nExecStart={}\nWorkingDirectory={}\nRestart=on-failure\n\n[Install]\nWantedBy=default.target\n",
        exe.display(),
        dir.display()
    );
    let path = unit_path()?;
    std::fs::create_dir_all(path.parent().context("percorso unit non valido")?)?;
    std::fs::write(&path, unit)?;
    run("systemctl", &["--user", "daemon-reload"])?;
    run(
        "systemctl",
        &["--user", "enable", "--now", "noplay.service"],
    )?;
    Ok(format!(
        "Servizio systemd utente installato e avviato ({}).",
        path.display()
    ))
}

#[cfg(target_os = "linux")]
pub fn uninstall() -> Result<String> {
    run(
        "systemctl",
        &["--user", "disable", "--now", "noplay.service"],
    )?;
    std::fs::remove_file(unit_path()?)?;
    run("systemctl", &["--user", "daemon-reload"])?;
    Ok("Servizio systemd utente rimosso.".into())
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn install() -> Result<String> {
    bail!("avvio automatico non supportato su questo sistema")
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn uninstall() -> Result<String> {
    bail!("avvio automatico non supportato su questo sistema")
}
