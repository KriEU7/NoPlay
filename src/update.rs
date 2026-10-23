//! Auto-update dalle release GitHub del progetto.
//!
//! L'indirizzo del repository è fisso nel codice, non in `config.json` e non
//! comandabile da Telegram: nessun messaggio o file di configurazione può
//! far scaricare l'aggiornamento da un posto diverso. Ogni archivio scaricato
//! viene verificato con il checksum pubblicato nella stessa release prima di
//! sostituire l'eseguibile.

use std::{fs, sync::Arc, time::Duration};

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use teloxide::{prelude::*, types::ChatId};

use crate::enforcer::AppState;

const REPO_OWNER: &str = "KriEU7";
const REPO_NAME: &str = "NoPlay";

/// Tag della release con cui questo binario è stato compilato (vedi build.rs).
/// "dev" per una build locale: in quel caso l'auto-update non fa nulla.
pub const CURRENT_TAG: &str = env!("NOPLAY_RELEASE_TAG");

fn asset_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "noplay-windows-x64.zip"
    } else if cfg!(target_os = "macos") {
        "noplay-macos-arm64.tar.gz"
    } else {
        "noplay-linux-x64.tar.gz"
    }
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(concat!("NoPlay-updater/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(30))
        .build()?)
}

async fn latest_release(client: &reqwest::Client) -> Result<Release> {
    let url = format!("https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/releases/latest");
    let resp = client.get(url).send().await?.error_for_status()?;
    Ok(resp.json().await?)
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Controlla se è uscita una release più recente; se sì la scarica, verifica
/// il checksum, sostituisce l'eseguibile e lo rilancia. Ritorna il tag nuovo
/// se ha aggiornato, altrimenti `None`. Il processo corrente va chiuso subito
/// dopo da chi chiama, perché quello nuovo è già partito al suo posto.
pub async fn check_and_apply() -> Result<Option<String>> {
    if CURRENT_TAG == "dev" {
        return Ok(None);
    }
    let client = client()?;
    let release = latest_release(&client)
        .await
        .context("controllo aggiornamenti su GitHub")?;
    if release.tag_name == CURRENT_TAG {
        return Ok(None);
    }

    let asset = release
        .assets
        .iter()
        .find(|a| a.name == asset_name())
        .with_context(|| {
            format!(
                "release {} senza pacchetto per questa piattaforma",
                release.tag_name
            )
        })?;
    let sums = release
        .assets
        .iter()
        .find(|a| a.name == "SHA256SUMS.txt")
        .context("release senza SHA256SUMS.txt, aggiornamento annullato")?;

    let archive = client
        .get(&asset.browser_download_url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    let sums_text = client
        .get(&sums.browser_download_url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let expected = sums_text
        .lines()
        .find_map(|l| {
            l.split_once("  ")
                .filter(|(_, name)| *name == asset.name)
                .map(|(h, _)| h)
        })
        .with_context(|| format!("checksum di {} non trovato in SHA256SUMS.txt", asset.name))?;
    let actual = to_hex(&Sha256::digest(&archive));
    anyhow::ensure!(
        actual.eq_ignore_ascii_case(expected),
        "checksum non corrispondente per {}, aggiornamento annullato",
        asset.name
    );

    let new_binary = extract_binary(&archive)?;
    replace_and_relaunch(&new_binary)?;
    Ok(Some(release.tag_name))
}

#[cfg(windows)]
fn extract_binary(archive: &[u8]) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive))?;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        if file.name().ends_with("noplay.exe") {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            return Ok(buf);
        }
    }
    bail!("noplay.exe non trovato nell'archivio scaricato")
}

#[cfg(not(windows))]
fn extract_binary(archive: &[u8]) -> Result<Vec<u8>> {
    use std::io::Read;
    let gz = flate2::read::GzDecoder::new(archive);
    let mut tar = tar::Archive::new(gz);
    for entry in tar.entries()? {
        let mut entry = entry?;
        let is_bin = entry
            .path()?
            .file_name()
            .map(|n| n == "noplay")
            .unwrap_or(false);
        if is_bin {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            return Ok(buf);
        }
    }
    bail!("binario 'noplay' non trovato nell'archivio scaricato")
}

/// Scrive il nuovo binario al posto di quello in esecuzione e lo rilancia.
/// Il processo attuale continua a girare dalla sua copia finché non esce.
fn replace_and_relaunch(new_binary: &[u8]) -> Result<()> {
    let current = std::env::current_exe().context("percorso dell'eseguibile non trovato")?;
    let tmp = current.with_extension("new");
    fs::write(&tmp, new_binary).with_context(|| format!("scrittura di {}", tmp.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&tmp)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&tmp, perms)?;
        // Su Unix si può sovrascrivere il file del binario in esecuzione: chi è già
        // partito tiene la vecchia copia finché non esce, chi parte dopo prende la nuova.
        fs::rename(&tmp, &current)?;
    }
    #[cfg(windows)]
    {
        // Su Windows un eseguibile in esecuzione si può rinominare ma non sovrascrivere:
        // lo spostiamo da parte e mettiamo il nuovo al suo posto.
        let old = current.with_extension("old.exe");
        let _ = fs::remove_file(&old);
        fs::rename(&current, &old)?;
        fs::rename(&tmp, &current)?;
    }

    std::process::Command::new(&current)
        .args(std::env::args().skip(1))
        .spawn()
        .context("avvio del nuovo eseguibile")?;
    Ok(())
}

/// Ciclo in background: controlla periodicamente le release su GitHub. Se ne
/// trova una più recente la installa e chiude questo processo (quello nuovo,
/// già partito al suo posto, riprende da lì); altrimenti riprova più tardi.
pub async fn run_periodic(bot: Bot, state: Arc<AppState>) {
    loop {
        let (enabled, hours, owners) = {
            let c = state.config.lock().await;
            (
                c.auto_update,
                c.update_check_interval_hours.max(1),
                c.owner_ids.clone(),
            )
        };

        if enabled {
            match check_and_apply().await {
                Ok(Some(tag)) => {
                    log::info!("aggiornato a {tag}, riavvio");
                    for id in owners {
                        let _ = bot
                            .send_message(
                                ChatId(id as i64),
                                format!("🔄 NoPlay aggiornato a {tag}, riavvio..."),
                            )
                            .await;
                    }
                    std::process::exit(0);
                }
                Ok(None) => {}
                Err(e) => log::warn!("controllo aggiornamenti fallito: {e:#}"),
            }
        }

        tokio::time::sleep(Duration::from_secs(hours * 3600)).await;
    }
}
