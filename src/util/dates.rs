use chrono::{Datelike, Duration, NaiveDate, Utc, Weekday};
use chrono_tz::Europe::Paris;

pub const DAYS: [&str; 7] = [
    "Lundi", "Mardi", "Mercredi", "Jeudi", "Vendredi", "Samedi", "Dimanche",
];

const MONTHS_FULL: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];

const MONTHS_SHORT: [&str; 12] = [
    "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.",
    "déc.",
];

pub fn month_full(index: u32) -> &'static str {
    MONTHS_FULL[(index - 1) as usize]
}

pub fn month_short(index: u32) -> &'static str {
    MONTHS_SHORT[(index - 1) as usize]
}

pub fn day_name(index: usize) -> &'static str {
    DAYS[index]
}

pub fn today_paris() -> NaiveDate {
    Utc::now().with_timezone(&Paris).date_naive()
}

pub fn format_full_date_fr(date: NaiveDate) -> String {
    format!(
        "{} {} {} {}",
        day_name(weekday_index(date)).to_lowercase(),
        date.day(),
        month_full(date.month()),
        date.year()
    )
}

pub fn week_title(start: NaiveDate, end: NaiveDate) -> String {
    format!(
        "Semaine {} - {} {} a {} {} {}",
        start.iso_week().week(),
        start.day(),
        month_short(start.month()),
        end.day(),
        month_short(end.month()),
        end.year()
    )
}

pub fn weekday_index(date: NaiveDate) -> usize {
    date.weekday().num_days_from_monday() as usize
}

pub fn monday_of_week(date: NaiveDate) -> NaiveDate {
    date - Duration::days(weekday_index(date) as i64)
}

/// Parse une saisie libre de date : formats usuels et expressions relatives en français.
///
/// Formats acceptés :
/// - dates complètes : `12/08/2026`, `2026-08-12`, `12 août 2026` ;
/// - jour et mois : `12 août` (année courante) ;
/// - noms de jours : `lundi` (prochaine occurrence, aujourd'hui compris) ;
/// - mots-clés relatifs : `aujourd'hui`, `demain`, `hier`, `cette semaine`, `semaine prochaine` ;
/// - numéros de semaine : `semaine 33`, `2026-W33`, `W33` (année courante).
pub fn parse_date_input(input: &str) -> Option<NaiveDate> {
    parse_date_input_on(input, today_paris())
}

pub fn parse_date_input_on(input: &str, today: NaiveDate) -> Option<NaiveDate> {
    let raw = input.trim().to_lowercase();
    if raw.is_empty() {
        return None;
    }

    match raw.as_str() {
        "aujourd'hui" => return Some(today),
        "demain" => return Some(today + Duration::days(1)),
        "hier" => return Some(today - Duration::days(1)),
        "cette semaine" => return Some(monday_of_week(today)),
        "la semaine prochaine" | "semaine prochaine" => {
            return Some(monday_of_week(today) + Duration::days(7));
        }
        _ => {}
    }

    if let Some(rest) = raw.strip_prefix("semaine ") {
        if let Some(date) = iso_week_date(today.year(), rest.trim()) {
            return Some(date);
        }
    }
    if let Some(rest) = raw.strip_prefix("w") {
        if let Some(date) = iso_week_date(today.year(), rest.trim()) {
            return Some(date);
        }
    }
    if let Some((year, week)) = raw.split_once("-w") {
        if let (Ok(year), Ok(week)) = (year.trim().parse::<i32>(), week.trim().parse::<u32>()) {
            if let Some(date) = NaiveDate::from_isoywd_opt(year, week, Weekday::Mon) {
                return Some(date);
            }
        }
    }

    if let Some(index) = DAYS.iter().position(|day| day.to_lowercase() == raw) {
        let delta = (index as i64 - weekday_index(today) as i64).rem_euclid(7);
        return Some(today + Duration::days(delta));
    }

    if let Ok(date) = NaiveDate::parse_from_str(&raw, "%d/%m/%y") {
        return Some(date);
    }
    const FULL_FORMATS: [&str; 5] = ["%Y-%m-%d", "%d/%m/%Y", "%d-%m-%Y", "%Y/%m/%d", "%d.%m.%Y"];
    for format in FULL_FORMATS {
        if let Ok(date) = NaiveDate::parse_from_str(&raw, format) {
            return Some(date);
        }
    }

    parse_spelled_date(&raw, today)
}

fn iso_week_date(year: i32, week: &str) -> Option<NaiveDate> {
    let week = week.trim().parse::<u32>().ok()?;
    NaiveDate::from_isoywd_opt(year, week, Weekday::Mon)
}

fn parse_spelled_date(raw: &str, today: NaiveDate) -> Option<NaiveDate> {
    let parts: Vec<&str> = raw.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    let day = parts[0].parse::<u32>().ok()?;
    let month = find_month(parts[1])?;
    let year = parts
        .get(2)
        .and_then(|y| y.parse::<i32>().ok())
        .unwrap_or(today.year());
    NaiveDate::from_ymd_opt(year, month, day)
}

fn find_month(token: &str) -> Option<u32> {
    let token = fold_accents(token.trim_end_matches('.'));
    for (index, full) in MONTHS_FULL.iter().enumerate() {
        if fold_accents(full) == token {
            return Some((index + 1) as u32);
        }
    }
    for (index, short) in MONTHS_SHORT.iter().enumerate() {
        if fold_accents(short.trim_end_matches('.')) == token {
            return Some((index + 1) as u32);
        }
    }
    None
}

fn fold_accents(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|character| match character {
            'à' | 'â' | 'ä' => 'a',
            'ç' => 'c',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn reference() -> NaiveDate {
        date(2026, 8, 12)
    }

    #[test]
    fn format_full_date_fr_suit_la_locale() {
        assert_eq!(format_full_date_fr(date(2026, 8, 10)), "lundi 10 août 2026");
        assert_eq!(
            format_full_date_fr(date(2026, 2, 5)),
            "jeudi 5 février 2026"
        );
    }

    #[test]
    fn week_title_suit_le_format_attendu() {
        let start = date(2026, 8, 10);
        assert_eq!(
            week_title(start, date(2026, 8, 16)),
            "Semaine 33 - 10 août a 16 août 2026"
        );
    }

    #[test]
    fn monday_of_week_retourne_le_lundi() {
        assert_eq!(monday_of_week(date(2026, 8, 13)), date(2026, 8, 10));
        assert_eq!(monday_of_week(date(2026, 8, 10)), date(2026, 8, 10));
        assert_eq!(monday_of_week(date(2026, 8, 9)), date(2026, 8, 3));
    }

    #[test]
    fn parse_formats_complets() {
        assert_eq!(
            parse_date_input_on("2026-08-12", reference()),
            Some(date(2026, 8, 12))
        );
        assert_eq!(
            parse_date_input_on("12/08/2026", reference()),
            Some(date(2026, 8, 12))
        );
        assert_eq!(
            parse_date_input_on("12-08-2026", reference()),
            Some(date(2026, 8, 12))
        );
        assert_eq!(
            parse_date_input_on("12/08/26", reference()),
            Some(date(2026, 8, 12))
        );
    }

    #[test]
    fn parse_semaine_iso() {
        assert_eq!(
            parse_date_input_on("2026-W33", reference()),
            Some(date(2026, 8, 10))
        );
        assert_eq!(
            parse_date_input_on("semaine 33", reference()),
            Some(date(2026, 8, 10))
        );
        assert_eq!(
            parse_date_input_on("w33", reference()),
            Some(date(2026, 8, 10))
        );
    }

    #[test]
    fn parse_nom_de_jour() {
        assert_eq!(
            parse_date_input_on("lundi", reference()),
            Some(date(2026, 8, 17))
        );
        assert_eq!(
            parse_date_input_on("samedi", reference()),
            Some(date(2026, 8, 15))
        );
        assert_eq!(
            parse_date_input_on("mercredi", reference()),
            Some(date(2026, 8, 12))
        );
    }

    #[test]
    fn parse_mots_cles_relatifs() {
        assert_eq!(
            parse_date_input_on("aujourd'hui", reference()),
            Some(reference())
        );
        assert_eq!(
            parse_date_input_on("demain", reference()),
            Some(date(2026, 8, 13))
        );
        assert_eq!(
            parse_date_input_on("hier", reference()),
            Some(date(2026, 8, 11))
        );
        assert_eq!(
            parse_date_input_on("cette semaine", reference()),
            Some(date(2026, 8, 10))
        );
        assert_eq!(
            parse_date_input_on("semaine prochaine", reference()),
            Some(date(2026, 8, 17))
        );
    }

    #[test]
    fn parse_date_avec_mois_en_lettres() {
        assert_eq!(
            parse_date_input_on("12 août 2026", reference()),
            Some(date(2026, 8, 12))
        );
        assert_eq!(
            parse_date_input_on("12 aout 2026", reference()),
            Some(date(2026, 8, 12))
        );
        assert_eq!(
            parse_date_input_on("12 août", reference()),
            Some(date(2026, 8, 12))
        );
    }

    #[test]
    fn parse_invalide_retourne_none() {
        assert_eq!(parse_date_input_on("", reference()), None);
        assert_eq!(parse_date_input_on("pas une date", reference()), None);
        assert_eq!(parse_date_input_on("32/13/2026", reference()), None);
    }
}
