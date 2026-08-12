//! Récupération du flux ICS auprès du serveur d'emploi du temps.

use anyhow::{bail, Result};
use reqwest::StatusCode;
use tokio::time::{sleep, Duration};

use crate::config::Config;

const MAX_ATTEMPTS: u32 = 3;
const BASE_DELAY_SECS: u64 = 5;

pub async fn fetch_ics_with_retry(client: &reqwest::Client, config: &Config) -> Result<String> {
    let mut last_error: Option<anyhow::Error> = None;
    for attempt in 0..MAX_ATTEMPTS {
        match fetch_ics(client, config).await {
            Ok(content) => return Ok(content),
            Err(error) => {
                last_error = Some(error);
                if attempt + 1 < MAX_ATTEMPTS {
                    let delay = Duration::from_secs(BASE_DELAY_SECS * 2u64.pow(attempt));
                    tracing::warn!(delay = ?delay, "échec de récupération du calendrier, nouvelle tentative");
                    sleep(delay).await;
                }
            }
        }
    }
    Err(last_error.expect("au moins une tentative"))
}

pub async fn fetch_ics(client: &reqwest::Client, config: &Config) -> Result<String> {
    let response = client
        .get(&config.calendar_url)
        .basic_auth(&config.calendar_username, Some(&config.calendar_password))
        .send()
        .await?;
    if response.status() == StatusCode::UNAUTHORIZED {
        bail!("authentification refusée par le serveur (identifiants invalides ?)");
    }
    let response = response.error_for_status()?;
    let content = response.text().await?;
    if !content.trim_start().starts_with("BEGIN:VCALENDAR") {
        bail!("la réponse du serveur ne contient pas un calendrier ICS");
    }
    Ok(content)
}
