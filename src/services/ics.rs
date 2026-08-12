//! Analyse et comparaison des calendriers ICS.
//!
//! La comparaison se fait sur des signatures `summary||start||end` des événements du jour
//! courant au futur, le dernier jour du nouveau calendrier étant exclu des deux côtés : le
//! fichier ADE déborde toujours d'une journée glissante, ce qui fausserait la comparaison sinon.

use std::collections::{BTreeSet, HashMap};
use std::str::FromStr;

use anyhow::{anyhow, Result};
use chrono::{Duration, NaiveDate, NaiveDateTime};
use chrono_tz::{Europe::Paris, Tz, UTC};
use icalendar::{Calendar, CalendarComponent, Component, Property};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub summary: String,
    pub location: String,
    pub description: String,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub all_day: bool,
}

impl Event {
    pub fn date(&self) -> NaiveDate {
        self.start.date()
    }
}

#[derive(Debug, Clone)]
pub struct EmbedField {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct ChangeReport {
    pub title: String,
    pub description: String,
    pub color: u32,
    pub fields: Vec<EmbedField>,
    pub footer: String,
}

pub fn parse_ics(content: &str) -> Result<Vec<Event>> {
    let calendar = Calendar::from_str(content).map_err(|e| anyhow!("ICS invalide : {e}"))?;
    let mut events = Vec::new();
    for component in &calendar.components {
        let CalendarComponent::Event(event) = component else {
            continue;
        };
        let properties = event.properties();
        let Some(start_prop) = properties.get("DTSTART") else {
            continue;
        };
        let (start, all_day) = parse_datetime_property(start_prop)?;
        let end = properties
            .get("DTEND")
            .and_then(|p| parse_datetime_property(p).ok())
            .map(|(dt, _)| dt)
            .unwrap_or_else(|| start + Duration::days(1));
        events.push(Event {
            summary: event
                .property_value("SUMMARY")
                .unwrap_or("No Title")
                .to_string(),
            location: event.property_value("LOCATION").unwrap_or("").to_string(),
            description: event
                .property_value("DESCRIPTION")
                .unwrap_or("")
                .to_string(),
            start,
            end,
            all_day,
        });
    }
    Ok(events)
}

/// Convertit la propriété DTSTART/DTEND d'un événement en date locale Paris.
///
/// Le fuseau est résolu dans cet ordre : suffixe `Z` (UTC), paramètre `TZID`, sinon le fuseau
/// est supposé être Europe/Paris (fichiers ADE sans fuseau explicite).
fn parse_datetime_property(prop: &Property) -> Result<(NaiveDateTime, bool)> {
    let raw = prop.value();
    if raw.len() == 8 && raw.chars().all(|c| c.is_ascii_digit()) {
        let date = chrono::NaiveDate::parse_from_str(raw, "%Y%m%d")
            .map_err(|e| anyhow!("date ICS invalide '{raw}' : {e}"))?;
        return Ok((date.and_hms_opt(0, 0, 0).unwrap(), true));
    }
    let naive = chrono::NaiveDateTime::parse_from_str(raw.trim_end_matches('Z'), "%Y%m%dT%H%M%S")
        .map_err(|e| anyhow!("date-heure ICS invalide '{raw}' : {e}"))?;
    let local = if raw.ends_with('Z') {
        naive.and_utc().with_timezone(&Paris).naive_local()
    } else {
        let tzid = prop.params().get("TZID").map(|p| p.value());
        resolve_timezone(tzid)
            .and_then(|tz| localize(naive, tz))
            .unwrap_or(naive)
    };
    Ok((local, false))
}

fn resolve_timezone(tzid: Option<&str>) -> Option<Tz> {
    let tzid = tzid?.trim_matches('"');
    Tz::from_str(tzid)
        .ok()
        .or_else(|| Tz::from_str(tzid.to_lowercase().replace(' ', "_").as_str()).ok())
}

fn localize(naive: NaiveDateTime, tz: Tz) -> Option<NaiveDateTime> {
    match tz {
        UTC => Some(naive.and_utc().with_timezone(&Paris).naive_local()),
        tz => naive
            .and_local_timezone(tz)
            .single()
            .map(|dt| dt.with_timezone(&Paris).naive_local()),
    }
}

fn signature(event: &Event) -> String {
    format!(
        "{}||{}||{}",
        event.summary,
        event.start.format("%Y-%m-%d %H:%M:%S"),
        event.end.format("%Y-%m-%d %H:%M:%S")
    )
}

fn is_today_or_future(event: &Event, today: NaiveDate) -> bool {
    event.date() >= today
}

fn get_last_event_date(events: &[Event]) -> Option<NaiveDate> {
    events.iter().map(Event::date).max()
}

fn exclude_last_day(events: Vec<Event>, last_day: Option<NaiveDate>) -> Vec<Event> {
    match last_day {
        Some(day) => events.into_iter().filter(|e| e.date() != day).collect(),
        None => events,
    }
}

/// Découpe les événements en groupes partageant le même début (créneaux parallèles).
///
/// Pré-condition : les événements sont triés par heure de début.
pub fn organize_events(events: &[Event]) -> Vec<Vec<&Event>> {
    let mut groups: Vec<Vec<&Event>> = Vec::new();
    for event in events {
        if let Some(last) = groups.last_mut() {
            if last[0].start == event.start {
                last.push(event);
                continue;
            }
        }
        groups.push(vec![event]);
    }
    groups
}

pub fn compare_future(old: &[Event], new: &[Event], today: NaiveDate) -> bool {
    let old_future: Vec<Event> = old
        .iter()
        .filter(|e| is_today_or_future(e, today))
        .cloned()
        .collect();
    let new_future: Vec<Event> = new
        .iter()
        .filter(|e| is_today_or_future(e, today))
        .cloned()
        .collect();
    let last_day = get_last_event_date(&new_future);
    let old_sigs: BTreeSet<String> = exclude_last_day(old_future, last_day)
        .iter()
        .map(signature)
        .collect();
    let new_sigs: BTreeSet<String> = exclude_last_day(new_future, last_day)
        .iter()
        .map(signature)
        .collect();
    old_sigs == new_sigs
}

pub fn compare_changes(old: &[Event], new: &[Event], today: NaiveDate) -> Option<ChangeReport> {
    let old_future: Vec<Event> = old
        .iter()
        .filter(|e| is_today_or_future(e, today))
        .cloned()
        .collect();
    let new_future: Vec<Event> = new
        .iter()
        .filter(|e| is_today_or_future(e, today))
        .cloned()
        .collect();
    let last_day = get_last_event_date(&new_future);
    let old = exclude_last_day(old_future, last_day);
    let new = exclude_last_day(new_future, last_day);

    let old_map: HashMap<String, &Event> =
        old.iter().map(|e| (signature(e).to_string(), e)).collect();
    let new_map: HashMap<String, &Event> =
        new.iter().map(|e| (signature(e).to_string(), e)).collect();
    let old_sigs: BTreeSet<&String> = old_map.keys().collect();
    let new_sigs: BTreeSet<&String> = new_map.keys().collect();

    let added: Vec<&String> = new_sigs.difference(&old_sigs).cloned().collect();
    let deleted: Vec<&String> = old_sigs.difference(&new_sigs).cloned().collect();
    if added.is_empty() && deleted.is_empty() {
        return None;
    }

    let color = match (added.is_empty(), deleted.is_empty()) {
        (false, true) => 0x2ecc71, // ajouts uniquement : vert
        (true, false) => 0xe74c3c, // suppressions uniquement : rouge
        _ => 0xe67e22,             // les deux : orange
    };

    let mut fields = Vec::new();
    if !added.is_empty() {
        fields.push(EmbedField {
            name: format!("🟢 {} NOUVEAU(X) ÉVÉNEMENT(S)", added.len()),
            value: list_events_embed(&new_map, &added),
        });
    }
    if !deleted.is_empty() {
        fields.push(EmbedField {
            name: format!("🔴 {} ÉVÉNEMENT(S) SUPPRIMÉ(S)", deleted.len()),
            value: list_events_embed(&old_map, &deleted),
        });
    }

    Some(ChangeReport {
        title: "📅 Changement d'emploi du temps !".to_string(),
        description: format!(
            "**Ancienne version** : {} événements\n**Nouvelle version** : {} événements",
            old.len(),
            new.len()
        ),
        color,
        fields,
        footer: format!(
            "Résumé : +{} ajouté(s), -{} supprimé(s)",
            added.len(),
            deleted.len()
        ),
    })
}

/// Formate la liste des événements d'un champ d'embed, avec troncature à 1024 caractères
/// (marge de 60 caractères puis note d'événements omis).
fn list_events_embed(events: &HashMap<String, &Event>, sigs: &[&String]) -> String {
    let mut sigs_sorted = sigs.to_vec();
    sigs_sorted.sort();
    let items: Vec<String> = sigs_sorted
        .iter()
        .filter_map(|sig| events.get(*sig))
        .map(|event| format_item(event))
        .collect();

    let mut result = String::new();
    let mut omitted = 0usize;
    let item_count = items.len();
    for (index, item) in items.into_iter().enumerate() {
        if result.chars().count() + item.chars().count() + 60 > 1024 {
            omitted = item_count - index;
            break;
        }
        result.push_str(&item);
        result.push('\n');
    }
    if omitted > 0 {
        result.push_str(&format!("\n*... et {omitted} autre(s) événement(s)*"));
    }
    result
}

fn format_item(event: &Event) -> String {
    let date_str = event.start.format("%d/%m");
    let time_str = if event.all_day {
        "Journée".to_string()
    } else {
        format!(
            "{}-{}",
            event.start.format("%H:%M"),
            event.end.format("%H:%M")
        )
    };
    let title = truncate_chars(&event.summary, 45);
    format!("`{date_str} {time_str}` {title}")
}

pub fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let mut out: String = text.chars().take(max).collect();
        out.push_str("...");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn event(summary: &str, start: NaiveDateTime, end: NaiveDateTime) -> Event {
        Event {
            summary: summary.to_string(),
            location: String::new(),
            description: String::new(),
            start,
            end,
            all_day: false,
        }
    }

    fn dt(y: i32, m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(h, min, 0)
            .unwrap()
    }

    #[test]
    fn parse_ics_avec_tzid() {
        let content = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\n\
                       DTSTART;TZID=Europe/Paris:20260812T090000\n\
                       DTEND;TZID=Europe/Paris:20260812T100000\n\
                       SUMMARY:Cours de maths\n\
                       END:VEVENT\nEND:VCALENDAR";
        let events = parse_ics(content).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].summary, "Cours de maths");
        assert_eq!(events[0].start, dt(2026, 8, 12, 9, 0));
        assert!(!events[0].all_day);
    }

    #[test]
    fn parse_ics_avec_date_seule() {
        let content = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\n\
                       DTSTART;VALUE=DATE:20260812\n\
                       DTEND;VALUE=DATE:20260813\n\
                       SUMMARY:Vacances\n\
                       END:VEVENT\nEND:VCALENDAR";
        let events = parse_ics(content).unwrap();
        assert!(events[0].all_day);
        assert_eq!(events[0].date(), date(2026, 8, 12));
    }

    #[test]
    fn parse_ics_date_utc() {
        let content = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\n\
                       DTSTART:20260812T070000Z\n\
                       DTEND:20260812T080000Z\n\
                       SUMMARY:En UTC\n\
                       END:VEVENT\nEND:VCALENDAR";
        let events = parse_ics(content).unwrap();
        assert_eq!(events[0].start, dt(2026, 8, 12, 9, 0));
    }

    #[test]
    fn compare_future_egal() {
        let today = date(2026, 8, 12);
        let a = [event("A", dt(2026, 8, 12, 9, 0), dt(2026, 8, 12, 10, 0))];
        let b = [event("A", dt(2026, 8, 12, 9, 0), dt(2026, 8, 12, 10, 0))];
        assert!(compare_future(&a, &b, today));
    }

    #[test]
    fn compare_future_detecte_l_ajout() {
        let today = date(2026, 8, 12);
        let old = [event("A", dt(2026, 8, 12, 9, 0), dt(2026, 8, 12, 10, 0))];
        let new = [
            event("A", dt(2026, 8, 12, 9, 0), dt(2026, 8, 12, 10, 0)),
            event("B", dt(2026, 8, 13, 9, 0), dt(2026, 8, 13, 10, 0)),
            event("Fin", dt(2026, 8, 14, 9, 0), dt(2026, 8, 14, 10, 0)),
        ];
        let old = [
            old[0].clone(),
            event("Fin", dt(2026, 8, 14, 9, 0), dt(2026, 8, 14, 10, 0)),
        ];
        assert!(!compare_future(&old, &new, today));
    }

    #[test]
    fn compare_future_ignore_le_dernier_jour() {
        let today = date(2026, 8, 12);
        let old = [event("A", dt(2026, 8, 20, 9, 0), dt(2026, 8, 20, 10, 0))];
        let new = [
            event("A", dt(2026, 8, 20, 9, 0), dt(2026, 8, 20, 10, 0)),
            event("B", dt(2026, 8, 21, 9, 0), dt(2026, 8, 21, 10, 0)),
        ];
        assert!(compare_future(&old, &new, today));
    }

    #[test]
    fn compare_changes_ajout_unique() {
        let today = date(2026, 8, 12);
        let old = [
            event("A", dt(2026, 8, 12, 9, 0), dt(2026, 8, 12, 10, 0)),
            event("Fin", dt(2026, 8, 13, 9, 0), dt(2026, 8, 13, 10, 0)),
        ];
        let new = [
            event("A", dt(2026, 8, 12, 9, 0), dt(2026, 8, 12, 10, 0)),
            event("Cours B", dt(2026, 8, 12, 14, 0), dt(2026, 8, 12, 15, 0)),
            event("Fin", dt(2026, 8, 13, 9, 0), dt(2026, 8, 13, 10, 0)),
        ];
        let report = compare_changes(&old, &new, today).unwrap();
        assert_eq!(report.color, 0x2ecc71);
        assert_eq!(report.fields.len(), 1);
        assert!(report.fields[0].name.contains("NOUVEAU"));
        assert!(report.fields[0].value.contains("Cours"));
        assert_eq!(report.footer, "Résumé : +1 ajouté(s), -0 supprimé(s)");
    }

    #[test]
    fn compare_changes_suppression_unique() {
        let today = date(2026, 8, 12);
        let old = [event("A", dt(2026, 8, 12, 9, 0), dt(2026, 8, 12, 10, 0))];
        let new = [];
        let report = compare_changes(&old, &new, today).unwrap();
        assert_eq!(report.color, 0xe74c3c);
        assert!(report.fields[0].name.contains("SUPPRIMÉ"));
    }

    #[test]
    fn compare_changes_mixte() {
        let today = date(2026, 8, 12);
        let old = [
            event("Ancien", dt(2026, 8, 12, 9, 0), dt(2026, 8, 12, 10, 0)),
            event("Fin", dt(2026, 8, 13, 9, 0), dt(2026, 8, 13, 10, 0)),
        ];
        let new = [
            event("Nouveau", dt(2026, 8, 12, 11, 0), dt(2026, 8, 12, 12, 0)),
            event("Fin", dt(2026, 8, 13, 9, 0), dt(2026, 8, 13, 10, 0)),
        ];
        let report = compare_changes(&old, &new, today).unwrap();
        assert_eq!(report.color, 0xe67e22);
        assert_eq!(report.fields.len(), 2);
    }

    #[test]
    fn compare_changes_sans_changement_renvoie_none() {
        let today = date(2026, 8, 12);
        let old = [event("A", dt(2026, 8, 12, 9, 0), dt(2026, 8, 12, 10, 0))];
        let new = [event("A", dt(2026, 8, 12, 9, 0), dt(2026, 8, 12, 10, 0))];
        assert!(compare_changes(&old, &new, today).is_none());
    }

    #[test]
    fn embed_value_cap_a_1024_caracteres() {
        let today = date(2026, 8, 12);
        let mut new = Vec::new();
        for i in 0..60 {
            let summary = format!("Cours interminable numéro {i} pour dépasser la limite du champ");
            new.push(event(
                &summary,
                dt(2026, 8, 12, 9, 0),
                dt(2026, 8, 12, 10, 0),
            ));
        }
        new.push(event("Fin", dt(2026, 8, 13, 9, 0), dt(2026, 8, 13, 10, 0)));
        let report = compare_changes(&[], &new, today).unwrap();
        let value = &report.fields[0].value;
        assert!(value.chars().count() <= 1024);
        assert!(value.contains("autre(s) événement(s)"));
    }

    #[test]
    fn truncate_chars_ajoute_points() {
        assert_eq!(truncate_chars("courte", 45), "courte");
        assert_eq!(truncate_chars("xy", 1), "x...");
    }

    #[test]
    fn organize_events_groupe_les_creneaux_parallèles() {
        let events = vec![
            event("A", dt(2026, 8, 12, 9, 0), dt(2026, 8, 12, 10, 0)),
            event("B", dt(2026, 8, 12, 9, 0), dt(2026, 8, 12, 10, 0)),
            event("C", dt(2026, 8, 12, 10, 0), dt(2026, 8, 12, 11, 0)),
        ];
        let groups = organize_events(&events);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].len(), 2);
        assert_eq!(groups[1].len(), 1);
    }

    #[test]
    fn parse_ics_avec_lignes_description() {
        let content = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\n\
                       DTSTART;TZID=Europe/Paris:20260812T090000\n\
                       DTEND;TZID=Europe/Paris:20260812T100000\n\
                       SUMMARY:Cours\n\
                       DESCRIPTION:ligne1\\nligne2\\nligne3\\nligne4\\nligne5\n\
                       END:VEVENT\nEND:VCALENDAR";
        let events = parse_ics(content).unwrap();
        assert_eq!(events[0].description.lines().nth(3), Some("ligne4"));
    }
}
