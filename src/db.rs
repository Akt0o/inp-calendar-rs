//! Accès à la base SQLite : connexion, migrations et requêtes applicatives.

use std::path::Path;

use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

#[derive(Debug, Clone)]
pub struct CalendarState {
    pub ics_content: String,
    pub ics_hash: String,
    pub last_update_unix: i64,
}

#[derive(Debug, Clone)]
pub struct GuildNotification {
    pub channel_id: i64,
    pub role_id: i64,
}

pub async fn connect(path: &Path) -> Result<SqlitePool> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

pub async fn toggle_target_channel(pool: &SqlitePool, channel_id: i64) -> Result<bool> {
    let inserted = sqlx::query(
        "INSERT INTO target_channels (channel_id) VALUES (?) \
         ON CONFLICT(channel_id) DO NOTHING",
    )
    .bind(channel_id)
    .execute(pool)
    .await?
    .rows_affected()
        > 0;
    if inserted {
        return Ok(true);
    }
    sqlx::query("DELETE FROM target_channels WHERE channel_id = ?")
        .bind(channel_id)
        .execute(pool)
        .await?;
    Ok(false)
}

pub async fn list_target_channels(pool: &SqlitePool) -> Result<Vec<i64>> {
    let rows = sqlx::query("SELECT channel_id FROM target_channels ORDER BY channel_id")
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(|row| row.get(0)).collect())
}

pub async fn set_guild_notification(
    pool: &SqlitePool,
    guild_id: i64,
    channel_id: i64,
    role_id: i64,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO guild_notifications (guild_id, channel_id, role_id) VALUES (?, ?, ?) \
         ON CONFLICT(guild_id) DO UPDATE SET \
           channel_id = excluded.channel_id, \
           role_id = excluded.role_id",
    )
    .bind(guild_id)
    .bind(channel_id)
    .bind(role_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn clear_guild_notification(pool: &SqlitePool, guild_id: i64) -> Result<()> {
    sqlx::query("DELETE FROM guild_notifications WHERE guild_id = ?")
        .bind(guild_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_guild_notification(
    pool: &SqlitePool,
    guild_id: i64,
) -> Result<Option<GuildNotification>> {
    let row = sqlx::query("SELECT channel_id, role_id FROM guild_notifications WHERE guild_id = ?")
        .bind(guild_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|row| GuildNotification {
        channel_id: row.get(0),
        role_id: row.get(1),
    }))
}

pub async fn get_calendar_state(pool: &SqlitePool) -> Result<Option<CalendarState>> {
    let row = sqlx::query(
        "SELECT ics_content, ics_hash, last_update_unix FROM calendar_state WHERE id = 1",
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| CalendarState {
        ics_content: row.get(0),
        ics_hash: row.get(1),
        last_update_unix: row.get(2),
    }))
}

pub async fn set_calendar_state(
    pool: &SqlitePool,
    ics_content: &str,
    ics_hash: &str,
    last_update_unix: i64,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO calendar_state (id, ics_content, ics_hash, last_update_unix) VALUES (1, ?, ?, ?) \
         ON CONFLICT(id) DO UPDATE SET \
           ics_content = excluded.ics_content, \
           ics_hash = excluded.ics_hash, \
           last_update_unix = excluded.last_update_unix, \
           updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    )
    .bind(ics_content)
    .bind(ics_hash)
    .bind(last_update_unix)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::new().in_memory(true))
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn toggle_salon_ajoute_puis_retire() {
        let pool = test_pool().await;
        assert!(toggle_target_channel(&pool, 111).await.unwrap());
        assert!(!toggle_target_channel(&pool, 111).await.unwrap());
        assert!(list_target_channels(&pool).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn notifications_par_guilde() {
        let pool = test_pool().await;
        set_guild_notification(&pool, 1, 100, 200).await.unwrap();
        set_guild_notification(&pool, 2, 300, 400).await.unwrap();
        let notif = get_guild_notification(&pool, 1).await.unwrap().unwrap();
        assert_eq!((notif.channel_id, notif.role_id), (100, 200));
        set_guild_notification(&pool, 1, 500, 600).await.unwrap();
        let notif = get_guild_notification(&pool, 1).await.unwrap().unwrap();
        assert_eq!((notif.channel_id, notif.role_id), (500, 600));
        clear_guild_notification(&pool, 1).await.unwrap();
        assert!(get_guild_notification(&pool, 1).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn etat_calendrier_upsert() {
        let pool = test_pool().await;
        assert!(get_calendar_state(&pool).await.unwrap().is_none());
        set_calendar_state(&pool, "ics-1", "h1", 42).await.unwrap();
        set_calendar_state(&pool, "ics-2", "h2", 43).await.unwrap();
        let state = get_calendar_state(&pool).await.unwrap().unwrap();
        assert_eq!(
            (
                state.ics_content.as_str(),
                state.ics_hash.as_str(),
                state.last_update_unix
            ),
            ("ics-2", "h2", 43)
        );
    }
}
