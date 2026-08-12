//! État partagé entre les interactions Discord et la boucle de mise à jour.

use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64};
use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::Mutex;

use crate::config::Config;

pub struct App {
    pub config: Config,
    pub pool: SqlitePool,
    pub client: reqwest::Client,
    pub bot_user_id: AtomicU64,
    pub last_update_unix: AtomicI64,
    pub current_image_url: Mutex<Option<String>>,
    pub update_running: AtomicBool,
    pub loop_started: AtomicBool,
    pub image_writes: Mutex<()>,
}

impl App {
    pub fn new(config: Config, pool: SqlitePool, client: reqwest::Client) -> Arc<Self> {
        Arc::new(Self {
            config,
            pool,
            client,
            bot_user_id: AtomicU64::new(0),
            last_update_unix: AtomicI64::new(0),
            current_image_url: Mutex::new(None),
            update_running: AtomicBool::new(false),
            loop_started: AtomicBool::new(false),
            image_writes: Mutex::new(()),
        })
    }
}
