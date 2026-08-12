//! Mise à jour périodique et matérialisation atomique des fichiers servis par Discord.

use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use anyhow::Result;
use chrono::{Duration, Utc};
use serenity::all::Http;
use sha2::{Digest, Sha256};

use crate::app::App;
use crate::services::{calendar, ics, images, publisher};
use crate::util::dates;

struct RunningGuard<'a>(&'a std::sync::atomic::AtomicBool);
impl Drop for RunningGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

pub async fn run_once(http: Arc<Http>, app: Arc<App>) -> Result<()> {
    if app.update_running.swap(true, Ordering::AcqRel) {
        tracing::warn!("mise à jour déjà en cours");
        return Ok(());
    }
    let _guard = RunningGuard(&app.update_running);
    let content = calendar::fetch_ics_with_retry(&app.client, &app.config).await?;
    let new_events = ics::parse_ics(&content)?;
    let old_state = crate::db::get_calendar_state(&app.pool).await?;
    if let Some(state) = &old_state {
        tracing::debug!(previous_hash = %state.ics_hash, "état calendrier chargé");
    }
    let old_events = old_state
        .as_ref()
        .map(|state| ics::parse_ics(&state.ics_content))
        .transpose()?
        .unwrap_or_default();
    let today = dates::today_paris();
    let changed = old_state.is_some() && !ics::compare_future(&old_events, &new_events, today);
    let report = changed
        .then(|| ics::compare_changes(&old_events, &new_events, today))
        .flatten();
    let unix = Utc::now().timestamp();
    let hash = format!("{:x}", Sha256::digest(content.as_bytes()));
    crate::db::set_calendar_state(&app.pool, &content, &hash, unix).await?;
    tokio::fs::create_dir_all(&app.config.data_dir).await?;
    atomic_write(&app.config.data_dir.join("current.ics"), content.as_bytes()).await?;
    let schedule = app.config.data_dir.join("schedule.png");
    let day = app.config.data_dir.join("day_schedule.png");
    let events = new_events.clone();
    let schedule_copy = schedule.clone();
    tokio::task::spawn_blocking(move || {
        images::render_week(
            &events,
            dates::monday_of_week(today + Duration::days(2)),
            &schedule_copy,
        )
    })
    .await??;
    let events = new_events;
    let day_copy = day.clone();
    tokio::task::spawn_blocking(move || {
        images::render_day(&images::events_for_date(&events, today), today, &day_copy)
    })
    .await??;
    app.last_update_unix.store(unix, Ordering::Relaxed);
    let image_url = publisher::upload_image(&http, &app, &schedule).await?;
    *app.current_image_url.lock().await = Some(image_url.clone());
    publisher::publish_all(http, app.clone(), changed, report, image_url).await;
    Ok(())
}

async fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("file")
    ));
    tokio::fs::write(&tmp, content).await?;
    tokio::fs::rename(tmp, path).await?;
    Ok(())
}

pub async fn run_loop(http: Arc<Http>, app: Arc<App>) {
    loop {
        if let Err(error) = run_once(http.clone(), app.clone()).await {
            tracing::error!(%error, "mise à jour du calendrier échouée");
        }
        tokio::time::sleep(app.config.update_interval).await;
    }
}
