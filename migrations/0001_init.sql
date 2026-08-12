-- Schéma initial du bot.
-- La base sert de source de vérité pour les salons cibles, les notifications par guilde
-- et l'état du calendrier.

CREATE TABLE IF NOT EXISTS target_channels (
    channel_id INTEGER PRIMARY KEY,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS guild_notifications (
    guild_id INTEGER PRIMARY KEY,
    channel_id INTEGER NOT NULL,
    role_id INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS calendar_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    ics_content TEXT NOT NULL,
    ics_hash TEXT NOT NULL,
    last_update_unix INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
