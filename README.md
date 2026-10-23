# NoPlay

Parental control trasparente scritto in Rust: chiude i videogiochi sul PC e si gestisce da Telegram.

NoPlay gira **in chiaro** (nessuna modalità nascosta), risponde **solo** agli ID Telegram dei proprietari
indicati in `config.json` e **non esegue comandi shell arbitrari**: ogni azione è un comando specifico.

## Setup

1. Crea un bot con [@BotFather](https://t.me/BotFather) e copia il token.
2. Scopri il tuo ID Telegram (es. con [@userinfobot](https://t.me/userinfobot)).
3. Avvia `noplay` (o `cargo run --release`): al primo avvio crea da solo `config.json` (accanto all'eseguibile, o nella cartella corrente se c'è già) con una blocklist e delle fasce di esempio, e si ferma.
4. Apri `config.json`, inserisci `bot_token` e il tuo ID in `owner_ids`, e riavvia.

`config.json` è ignorato da git: non committare mai il token. Si può usare un altro percorso con `NOPLAY_CONFIG`.

## Comandi Telegram

| Comando | Cosa fa |
|---|---|
| `/status` | blocco attivo/pausa e giochi aperti |
| `/kill` | chiude subito i giochi aperti |
| `/enforce on\|off` | attiva o disattiva il blocco |
| `/allow <minuti>` | permette i giochi per un tempo limitato |
| `/add <nome>` / `/remove <nome>` | modifica la blocklist (nome processo, senza `.exe`) |
| `/list` | mostra la blocklist |
| `/schedule` | mostra le fasce in cui i giochi sono permessi |
| `/schedule add <giorni> <HH:MM-HH:MM>` | aggiunge una fascia, es. `add lun-ven 17:00-19:00`, `add sab,dom 10:00-12:30` |
| `/schedule del <n>` / `/schedule clear` | rimuove una fascia / tutte le fasce |
| `/lock` | blocca lo schermo (come Veyon: serve la password di Windows per rientrare) |
| `/update` | controlla e installa subito un aggiornamento dalle release di GitHub |
| `/help` | elenco comandi |

Mentre il blocco è attivo, ogni `poll_interval_seconds` il programma chiude i processi in blocklist e avvisa i proprietari su Telegram.

## Fasce orarie

I giochi sono permessi solo dentro le fasce del `schedule` (ora locale del PC); fuori fascia vengono chiusi.
Senza fasce, i giochi sono sempre bloccati. Le fasce si gestiscono da Telegram con `/schedule` e vengono
salvate in `config.json`. Giorni: `lun mar mer gio ven sab dom`, intervalli (`lun-ven`), liste (`sab,dom`) o `tutti`.
`/allow <minuti>` resta come eccezione una tantum, e `/enforce off` disattiva tutto.

## Esecuzione in background

- Su Windows la build release parte **senza finestra di console**; il processo `noplay.exe` resta ben visibile in Gestione attività.
- `noplay install` registra l'avvio automatico al login (Windows: attività pianificata `NoPlay`, serve un terminale come amministratore; Linux: servizio systemd utente). `noplay uninstall` lo rimuove.
- Ad ogni avvio il bot ti scrive su Telegram "NoPlay avviato su <nome PC>".
- Il `config.json` viene cercato nella cartella corrente e poi accanto all'eseguibile.
- Gli errori di avvio (config mancante, token sbagliato) finiscono in `noplay.log` accanto all'eseguibile.

Chiunque abbia i permessi può terminare il processo: NoPlay non si protegge da questo. Per rendere la cosa robusta usa il sistema operativo, ad esempio un account utente standard per chi viene controllato e l'installazione dall'account amministratore.

## Quando chiude un gioco

Oltre ad avvisare i proprietari su Telegram, NoPlay mostra sullo schermo del PC un messaggio
(testo configurabile in `ban_message`, su Windows è un messagebox di sistema) per far capire
a chi stava giocando perché il gioco si è chiuso.

## Blocco schermo

`/lock` usa il blocco nativo del sistema operativo (su Windows, `LockWorkStation`, la stessa funzione
di Blocca schermo di Windows), non un lucchetto su misura: richiede la password dell'account per
rientrare, esattamente come fa Veyon. Non c'è un overlay che disabilita Task Manager, Alt+Tab o
Ctrl+Alt+Canc: quel genere di blocco è la stessa tecnica usata dai ransomware "screen locker", e qui
non c'è.

## Auto-update

NoPlay controlla da solo le [release GitHub del progetto](https://github.com/KriEU7/NoPlay/releases)
(indirizzo fisso nel codice: non è configurabile da `config.json` né da Telegram, quindi nessun
messaggio può reindirizzarlo altrove) e, se trova una versione più recente, la scarica, verifica il
checksum SHA-256 pubblicato nella stessa release e si riavvia da solo. Succede ogni
`update_check_interval_hours` (6 di default) e si può anche forzare subito con `/update`.
Per disattivarlo metti `"auto_update": false` in `config.json`. Richiede una build scaricata da una
release (le build locali con `cargo build` hanno l'auto-update disattivato).

## Icona nella tray (Windows)

Su Windows compare un'icona nella tray con un menu rapido: permetti i giochi per 15 minuti, blocca lo
schermo, esci. È solo una comodità per chi è davanti al PC: il bot Telegram funziona comunque anche
senza, e l'icona resta sempre visibile (niente "nascondi dalla tray").

## Note

- Il matching è sul nome esatto del processo (minuscolo, senza `.exe`), per non chiudere programmi sbagliati.
