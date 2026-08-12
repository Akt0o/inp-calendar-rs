//! Démarrage du bot, branchement Discord et arrêt propre sur signal système.

mod app;
mod config;
mod db;
mod handlers;
mod services;
mod util;

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use serenity::all::*;
use serenity::async_trait;
use tracing_subscriber::EnvFilter;

use app::App;

struct Handler {
    app: Arc<App>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        self.app
            .bot_user_id
            .store(ready.user.id.get(), Ordering::Relaxed);
        if let Ok(Some(state)) = db::get_calendar_state(&self.app.pool).await {
            self.app
                .last_update_unix
                .store(state.last_update_unix, Ordering::Relaxed);
        }
        match Command::set_global_commands(&ctx.http, handlers::commands::definitions()).await {
            Ok(commands) => {
                tracing::info!(user = %ready.user.name, commands = commands.len(), "bot Discord prêt")
            }
            Err(error) => tracing::error!(%error, "enregistrement des commandes impossible"),
        }
        if !self.app.loop_started.swap(true, Ordering::AcqRel) {
            tokio::spawn(services::update::run_loop(
                ctx.http.clone(),
                self.app.clone(),
            ));
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(command) => {
                handlers::commands::handle(&ctx, self.app.clone(), &command).await
            }
            Interaction::Component(component) => {
                handlers::components::handle_component(&ctx, self.app.clone(), &component).await
            }
            Interaction::Modal(modal) => {
                handlers::components::handle_modal(&ctx, self.app.clone(), &modal).await
            }
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    let config = config::Config::from_env()?;
    tokio::fs::create_dir_all(&config.data_dir).await?;
    let pool = db::connect(&config.data_dir.join("calendar.db")).await?;
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("inp-calendar-bot/0.1")
        .build()?;
    let token = config.token.clone();
    let app = App::new(config, pool, http_client);
    let mut client = Client::builder(&token, GatewayIntents::GUILDS)
        .event_handler(Handler { app })
        .await?;
    let manager = client.shard_manager.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        tracing::info!("arrêt demandé");
        manager.shutdown_all().await;
    });
    client.start().await?;
    Ok(())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut terminate =
            signal(SignalKind::terminate()).expect("gestionnaire SIGTERM indisponible");
        tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c()
        .await
        .expect("gestionnaire Ctrl-C indisponible");
}
