use ab_glyph::{Font, FontArc, PxScale, ScaleFont};

/// Police DejaVu Sans empaquetée dans le binaire, pour un rendu identique sur toutes les
/// machines sans dépendre des polices du système.
pub static FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/DejaVuSans.ttf");

pub fn font() -> FontArc {
    FontArc::try_from_slice(FONT_BYTES).expect("la police empaquetée est invalide")
}

pub fn text_width(font: &FontArc, size: f32, text: &str) -> f32 {
    let scaled = font.as_scaled(PxScale::from(size));
    text.chars()
        .map(|c| scaled.h_advance(scaled.glyph_id(c)))
        .sum()
}

pub fn truncate_to_width(font: &FontArc, size: f32, text: &str, max_width: f32) -> String {
    if text_width(font, size, text) <= max_width {
        return text.to_string();
    }
    let mut kept = String::new();
    for c in text.chars() {
        let candidate = format!("{kept}{c}");
        if text_width(font, size, &format!("{candidate}...")) > max_width {
            break;
        }
        kept = candidate;
    }
    format!("{kept}...")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn police_chargee() {
        let f = font();
        assert!(text_width(&f, 16.0, "abc") > 0.0);
    }

    #[test]
    fn truncation_respecte_la_largeur() {
        let f = font();
        let long = "un texte très long qui ne rentre pas dans la boîte";
        let short = truncate_to_width(&f, 14.0, long, 60.0);
        assert!(text_width(&f, 14.0, &short) <= 60.0);
        assert!(short.ends_with("..."));
        assert!(short.len() < long.len());
    }

    #[test]
    fn texte_court_non_trongue() {
        let f = font();
        assert_eq!(truncate_to_width(&f, 14.0, "court", 200.0), "court");
    }
}
