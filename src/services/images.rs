//! Rendu PNG des vues journalière et hebdomadaire.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use chrono::{Duration, NaiveDate, Timelike};
use image::{Rgb, RgbImage};
use imageproc::drawing::{
    draw_filled_rect_mut, draw_hollow_rect_mut, draw_line_segment_mut, draw_text_mut,
};
use imageproc::rect::Rect;

use crate::services::ics::{organize_events, Event};
use crate::util::{dates, fonts};

const DAY_COLORS: [Rgb<u8>; 6] = [
    rgb(0x3498db),
    rgb(0xe74c3c),
    rgb(0x2ecc71),
    rgb(0xf39c12),
    rgb(0x9b59b6),
    rgb(0x1abc9c),
];
const WEEK_COLORS: [Rgb<u8>; 15] = [
    rgb(0x3498db),
    rgb(0xe74c3c),
    rgb(0x2ecc71),
    rgb(0xf39c12),
    rgb(0x9b59b6),
    rgb(0x1abc9c),
    rgb(0xe67e22),
    rgb(0x34495e),
    rgb(0x16a085),
    rgb(0xd35400),
    rgb(0xc0392b),
    rgb(0x8e44ad),
    rgb(0x2980b9),
    rgb(0x27ae60),
    rgb(0xf1c40f),
];

const fn rgb(value: u32) -> Rgb<u8> {
    Rgb([
        ((value >> 16) & 255) as u8,
        ((value >> 8) & 255) as u8,
        (value & 255) as u8,
    ])
}

pub fn events_for_date(events: &[Event], date: NaiveDate) -> Vec<Event> {
    let mut selected: Vec<Event> = events
        .iter()
        .filter(|event| event.date() == date)
        .cloned()
        .collect();
    selected.sort_by_key(|event| event.start);
    selected
}

fn details(event: &Event) -> String {
    event
        .description
        .lines()
        .nth(3)
        .filter(|line| !line.trim().is_empty())
        .map(|line| format!("{} - {}", event.summary, line.trim()))
        .unwrap_or_else(|| event.summary.clone())
}

pub fn render_day(events: &[Event], target: NaiveDate, path: &Path) -> Result<()> {
    let groups = organize_events(events);
    let width = 800u32;
    let height = (80 + groups.len() as u32 * 100 + 40).max(400);
    let mut image = RgbImage::from_pixel(width, height, rgb(0xffffff));
    let font = fonts::font();
    draw_filled_rect_mut(&mut image, Rect::at(0, 0).of_size(width, 80), rgb(0x2c3e50));
    let title = dates::format_full_date_fr(target);
    let title_x = ((width as f32 - fonts::text_width(&font, 32.0, &title)) / 2.0).max(0.0) as i32;
    draw_text_mut(&mut image, rgb(0xffffff), title_x, 22, 32.0, &font, &title);
    if groups.is_empty() {
        let text = "Aucun événement prévu ce jour";
        let x = ((width as f32 - fonts::text_width(&font, 18.0, text)) / 2.0) as i32;
        draw_text_mut(&mut image, rgb(0x7f8c8d), x, 150, 18.0, &font, text);
    }
    for (group_index, group) in groups.iter().enumerate() {
        let y = 100 + group_index as i32 * 100;
        let cell_width = (width as i32 - 40) / group.len() as i32;
        for (index, event) in group.iter().enumerate() {
            let x = 20 + index as i32 * cell_width;
            let rect = Rect::at(x, y).of_size((cell_width - 5) as u32, 90);
            draw_filled_rect_mut(&mut image, rect, DAY_COLORS[group_index % DAY_COLORS.len()]);
            draw_hollow_rect_mut(&mut image, rect, rgb(0x2c3e50));
            let time = if event.all_day {
                "Journée".to_string()
            } else {
                format!(
                    "{} - {}",
                    event.start.format("%H:%M"),
                    event.end.format("%H:%M")
                )
            };
            draw_text_mut(&mut image, rgb(0xffffff), x + 12, y + 8, 21.0, &font, &time);
            let max = (cell_width - 25).max(20) as f32;
            let title = fonts::truncate_to_width(&font, 16.0, &details(event), max);
            draw_text_mut(
                &mut image,
                rgb(0xffffff),
                x + 12,
                y + 38,
                16.0,
                &font,
                &title,
            );
            if !event.location.is_empty() {
                let location = fonts::truncate_to_width(&font, 13.0, &event.location, max);
                draw_text_mut(
                    &mut image,
                    rgb(0xecf0f1),
                    x + 12,
                    y + 65,
                    13.0,
                    &font,
                    &location,
                );
            }
        }
    }
    image.save(path)?;
    Ok(())
}

pub fn render_week(events: &[Event], start: NaiveDate, path: &Path) -> Result<()> {
    let end = start + Duration::days(6);
    let selected: Vec<Event> = events
        .iter()
        .filter(|event| event.date() >= start && event.date() <= end)
        .cloned()
        .collect();
    let mut earliest = 8u32;
    let mut latest = 20u32;
    for event in selected.iter().filter(|event| !event.all_day) {
        earliest = earliest.min(event.start.hour());
        latest = latest.max((event.end.hour() + u32::from(event.end.minute() > 0)).min(24));
    }
    let width = 1840u32;
    let height = 100 + (latest - earliest) * 60 + 30;
    let mut image = RgbImage::from_pixel(width, height, rgb(0xf8f9fa));
    let font = fonts::font();
    draw_filled_rect_mut(
        &mut image,
        Rect::at(0, 0).of_size(width, 100),
        rgb(0x2c3e50),
    );
    let title = dates::week_title(start, end);
    let x = ((width as f32 - fonts::text_width(&font, 28.0, &title)) / 2.0) as i32;
    draw_text_mut(&mut image, rgb(0xffffff), x, 17, 28.0, &font, &title);
    for day in 0..7 {
        let day_x = 75 + day * 250;
        draw_filled_rect_mut(
            &mut image,
            Rect::at(day_x, 65).of_size(235, 35),
            rgb(0x34495e),
        );
        let date = start + Duration::days(day as i64);
        let label = format!("{} {}", dates::day_name(day as usize), date.format("%-d"));
        draw_text_mut(
            &mut image,
            rgb(0xffffff),
            day_x + 55,
            70,
            18.0,
            &font,
            &label,
        );
    }
    for hour in earliest..latest {
        let y = 115 + (hour - earliest) * 60;
        draw_text_mut(
            &mut image,
            rgb(0x7f8c8d),
            15,
            y as i32 + 18,
            14.0,
            &font,
            &format!("{hour:02}:00"),
        );
        draw_line_segment_mut(
            &mut image,
            (60.0, y as f32),
            (1825.0, y as f32),
            rgb(0xdee2e6),
        );
    }
    for day in 0..=7 {
        let x = 60 + day * 250;
        draw_line_segment_mut(
            &mut image,
            (x as f32, 100.0),
            (x as f32, (height - 15) as f32),
            rgb(0xdee2e6),
        );
    }
    let mut colors: HashMap<String, usize> = HashMap::new();
    for day in 0..7 {
        let date = start + Duration::days(day);
        let day_events = events_for_date(&selected, date);
        for group in organize_events(&day_events) {
            for (slot, event) in group.iter().enumerate() {
                if event.all_day {
                    continue;
                }
                let event_width = 220.0 / group.len() as f32;
                let event_x = 60.0 + day as f32 * 250.0 + 15.0 + slot as f32 * event_width;
                let start_offset =
                    (event.start.hour() - earliest) as f32 + event.start.minute() as f32 / 60.0;
                let duration = (event.end - event.start).num_minutes().max(10) as f32 / 60.0;
                let event_y = 115.0 + start_offset * 60.0;
                let event_height = (duration * 60.0 - 5.0).max(18.0);
                let next_color = colors.len() % WEEK_COLORS.len();
                let color_index = *colors.entry(event.summary.clone()).or_insert(next_color);
                let rect = Rect::at(event_x as i32, event_y as i32)
                    .of_size((event_width - 5.0).max(8.0) as u32, event_height as u32);
                draw_filled_rect_mut(&mut image, rect, WEEK_COLORS[color_index]);
                draw_hollow_rect_mut(&mut image, rect, rgb(0x2c3e50));
                let max_width = (event_width - 13.0).max(8.0);
                let time = fonts::truncate_to_width(
                    &font,
                    12.0,
                    &format!(
                        "{} - {}",
                        event.start.format("%H:%M"),
                        event.end.format("%H:%M")
                    ),
                    max_width,
                );
                draw_text_mut(
                    &mut image,
                    rgb(0xffffff),
                    event_x as i32 + 4,
                    event_y as i32 + 5,
                    12.0,
                    &font,
                    &time,
                );
                let lines = wrap_text(
                    &font,
                    13.0,
                    &details(event),
                    max_width,
                    ((event_height - 25.0) / 16.0).max(0.0) as usize,
                );
                for (line, text) in lines.iter().enumerate() {
                    draw_text_mut(
                        &mut image,
                        rgb(0xffffff),
                        event_x as i32 + 4,
                        event_y as i32 + 23 + line as i32 * 16,
                        13.0,
                        &font,
                        text,
                    );
                }
            }
        }
    }
    image.save(path)?;
    Ok(())
}

fn wrap_text(
    font: &ab_glyph::FontArc,
    size: f32,
    text: &str,
    width: f32,
    max_lines: usize,
) -> Vec<String> {
    if max_lines == 0 {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };
        if fonts::text_width(font, size, &candidate) <= width {
            current = candidate;
        } else {
            if !current.is_empty() {
                lines.push(current);
            }
            current = fonts::truncate_to_width(font, size, word, width);
            if lines.len() + 1 >= max_lines {
                break;
            }
        }
    }
    if lines.len() < max_lines && !current.is_empty() {
        lines.push(current);
    }
    lines.truncate(max_lines);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;
    #[test]
    fn images_ont_les_dimensions_attendues() {
        let dir = tempfile::tempdir().unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
        let day = dir.path().join("day.png");
        render_day(&[], date, &day).unwrap();
        let day_image = image::open(day).unwrap();
        assert_eq!(day_image.width(), 800);
        assert_eq!(day_image.get_pixel(0, 0).0[..3], [0x2c, 0x3e, 0x50]);
        let week = dir.path().join("week.png");
        render_week(&[], dates::monday_of_week(date), &week).unwrap();
        assert_eq!(image::open(week).unwrap().dimensions(), (1840, 850));
    }
}
