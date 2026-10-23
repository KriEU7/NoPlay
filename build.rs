fn main() {
    // Impostato dal workflow CI con lo stesso tag che userà per la release.
    // In sviluppo locale vale "dev": l'auto-update resta disattivato (vedi update.rs).
    let tag = std::env::var("NOPLAY_RELEASE_TAG").unwrap_or_else(|_| "dev".to_string());
    println!("cargo:rustc-env=NOPLAY_RELEASE_TAG={tag}");
    println!("cargo:rerun-if-env-changed=NOPLAY_RELEASE_TAG");
}
