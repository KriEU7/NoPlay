use chrono::{Datelike, Local, Timelike};
use serde::{Deserialize, Serialize};

const DAYS: [&str; 7] = ["lun", "mar", "mer", "gio", "ven", "sab", "dom"];

/// Fascia oraria in cui i giochi sono permessi. Orari in minuti dalla mezzanotte, ora locale.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Window {
    /// 0 = lunedì … 6 = domenica
    pub days: Vec<u8>,
    pub start: u16,
    pub end: u16,
}

impl Window {
    pub fn contains(&self, weekday: u8, minute: u16) -> bool {
        self.days.contains(&weekday) && minute >= self.start && minute < self.end
    }

    pub fn describe(&self) -> String {
        let days = if self.days.len() == 7 {
            "tutti i giorni".to_string()
        } else {
            self.days.iter().map(|d| DAYS[*d as usize]).collect::<Vec<_>>().join(",")
        };
        format!("{days} {}-{}", fmt_time(self.start), fmt_time(self.end))
    }
}

fn fmt_time(m: u16) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// True se adesso (ora locale) cade in almeno una fascia.
pub fn allowed_now(windows: &[Window]) -> bool {
    let now = Local::now();
    let weekday = now.weekday().num_days_from_monday() as u8;
    let minute = (now.hour() * 60 + now.minute()) as u16;
    windows.iter().any(|w| w.contains(weekday, minute))
}

fn day_index(s: &str) -> Result<u8, String> {
    DAYS.iter()
        .position(|d| *d == s)
        .map(|i| i as u8)
        .ok_or_else(|| format!("giorno non valido: '{s}' (usa lun mar mer gio ven sab dom)"))
}

/// Accetta "tutti", "lun-ven", "sab,dom", "ven-lun" (anche a cavallo della settimana).
pub fn parse_days(input: &str) -> Result<Vec<u8>, String> {
    let input = input.trim().to_lowercase();
    if input == "tutti" {
        return Ok((0..7).collect());
    }
    let mut days = Vec::new();
    for part in input.split(',') {
        match part.split_once('-') {
            Some((a, b)) => {
                let (a, b) = (day_index(a)?, day_index(b)?);
                if a <= b {
                    days.extend(a..=b);
                } else {
                    days.extend(a..7);
                    days.extend(0..=b);
                }
            }
            None => days.push(day_index(part)?),
        }
    }
    days.sort_unstable();
    days.dedup();
    Ok(days)
}

fn parse_time(s: &str) -> Result<u16, String> {
    let bad = || format!("orario non valido: '{s}' (usa HH:MM)");
    let (h, m) = s.trim().split_once(':').ok_or_else(bad)?;
    let (h, m): (u16, u16) = (h.parse().map_err(|_| bad())?, m.parse().map_err(|_| bad())?);
    if m < 60 && (h < 24 || (h == 24 && m == 0)) {
        Ok(h * 60 + m)
    } else {
        Err(bad())
    }
}

/// Costruisce una fascia da "lun-ven" e "17:00-19:30".
pub fn parse_window(days: &str, range: &str) -> Result<Window, String> {
    let (start, end) = range
        .split_once('-')
        .ok_or_else(|| format!("fascia non valida: '{range}' (usa HH:MM-HH:MM)"))?;
    let (start, end) = (parse_time(start)?, parse_time(end)?);
    if end <= start {
        return Err("la fine deve essere dopo l'inizio (per orari a cavallo di mezzanotte usa due fasce)".into());
    }
    Ok(Window { days: parse_days(days)?, start, end })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_ranges() {
        assert_eq!(parse_days("lun-ven").unwrap(), vec![0, 1, 2, 3, 4]);
        assert_eq!(parse_days("sab,dom").unwrap(), vec![5, 6]);
        assert_eq!(parse_days("sab-lun").unwrap(), vec![0, 5, 6]);
        assert_eq!(parse_days("tutti").unwrap().len(), 7);
        assert!(parse_days("xyz").is_err());
    }

    #[test]
    fn window_contains() {
        let w = parse_window("lun-ven", "17:00-19:30").unwrap();
        assert!(w.contains(0, 17 * 60));
        assert!(w.contains(4, 19 * 60 + 29));
        assert!(!w.contains(4, 19 * 60 + 30));
        assert!(!w.contains(5, 18 * 60));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse_window("lun", "19:00-17:00").is_err());
        assert!(parse_window("lun", "25:00-26:00").is_err());
        assert!(parse_window("lun", "1700-1900").is_err());
    }
}
