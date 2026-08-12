use std::path::PathBuf;
use std::time::Duration;

const DEFAULT_IMG_TARGET_CHANNEL: u64 = 1424712703498387537;
const DEFAULT_UPDATE_INTERVAL_MINUTES: u64 = 30;

#[derive(Debug, Clone)]
pub struct Config {
    pub token: String,
    pub calendar_username: String,
    pub calendar_password: String,
    pub calendar_url: String,
    pub img_target_channel: u64,
    pub update_interval: Duration,
    pub data_dir: PathBuf,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let token = env_var("TOKEN")?;
        let calendar_username = env_var("CALENDAR_USERNAME")?;
        let calendar_password = env_var("CALENDAR_PASSWORD")?;
        let calendar_url = env_var("CALENDAR_URL")?;
        let img_target_channel = std::env::var("IMG_TARGET_CHANNEL")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(DEFAULT_IMG_TARGET_CHANNEL);
        let minutes: u64 = std::env::var("UPDATE_INTERVAL_MINUTES")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(DEFAULT_UPDATE_INTERVAL_MINUTES)
            .max(1);
        let data_dir = std::env::var("DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("data"));

        Ok(Self {
            token,
            calendar_username,
            calendar_password,
            calendar_url,
            img_target_channel,
            update_interval: Duration::from_secs(minutes * 60),
            data_dir,
        })
    }
}

fn env_var(name: &str) -> anyhow::Result<String> {
    let value = std::env::var(name)
        .map_err(|_| anyhow::anyhow!("variable d'environnement manquante : {name}"))?;
    let value = value.trim().to_string();
    if value.is_empty() {
        anyhow::bail!("variable d'environnement manquante : {name}");
    }
    Ok(value)
}
