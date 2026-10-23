# NoPlay

Parental control trasparente scritto in Rust: chiude i videogiochi sul PC e si gestisce da Telegram.

NoPlay gira **in chiaro** (nessuna modalità nascosta), risponde **solo** agli ID Telegram dei proprietari
indicati in `config.json` e **non esegue comandi shell arbitrari**: ogni azione è un comando specifico.

## Setup

1. Crea un bot con [@BotFather](https://t.me/BotFather) e copia il token.
2. Scopri il tuo ID Telegram (es. con [@userinfobot](https://t.me/userinfobot)).
3. `cp config.example.json config.json` e inserisci token e `owner_ids`.
4. `cargo run --release`

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
| `/lock` | blocca lo schermo |
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

## Note

- Il matching è sul nome esatto del processo (minuscolo, senza `.exe`), per non chiudere programmi sbagliati.
