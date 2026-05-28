-- Baseline schema for reel-ng (equivalent to the legacy hand-rolled v3).
--
-- Every statement is idempotent (IF NOT EXISTS) on purpose: existing users
-- already have this schema at the legacy integer `schema_version = 3` but no
-- diesel tracking table, so on the first diesel run this migration runs and
-- must be a no-op against their database while still recording itself as
-- applied. Against a fresh database it creates everything.

CREATE TABLE IF NOT EXISTS sources (
    id TEXT PRIMARY KEY NOT NULL,
    source_type TEXT NOT NULL,
    name TEXT NOT NULL,
    config TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    last_synced_at TEXT
);

CREATE TABLE IF NOT EXISTS media_items (
    id TEXT PRIMARY KEY NOT NULL,
    source_type TEXT NOT NULL,
    source_id TEXT NOT NULL,
    external_id TEXT NOT NULL,
    media_type TEXT NOT NULL,
    title TEXT NOT NULL,
    year INTEGER,
    overview TEXT,
    content_rating TEXT,
    rating DOUBLE,
    runtime_minutes INTEGER,
    poster_path TEXT,
    backdrop_path TEXT,
    genres TEXT NOT NULL DEFAULT '[]',
    parent_id TEXT,
    season_number INTEGER,
    episode_number INTEGER,
    air_date TEXT,
    file_path TEXT,
    video_resolution TEXT,
    hdr TEXT,
    added_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_media_items_media_type
    ON media_items(media_type);
CREATE INDEX IF NOT EXISTS idx_media_items_parent_id
    ON media_items(parent_id);
CREATE INDEX IF NOT EXISTS idx_media_items_source
    ON media_items(source_type, source_id);
CREATE INDEX IF NOT EXISTS idx_media_items_added_at
    ON media_items(added_at DESC);

CREATE TABLE IF NOT EXISTS watch_progress (
    media_item_id TEXT PRIMARY KEY NOT NULL,
    position_seconds DOUBLE NOT NULL DEFAULT 0.0,
    duration_seconds DOUBLE NOT NULL DEFAULT 0.0,
    watched INTEGER NOT NULL DEFAULT 0,
    last_watched_at TEXT NOT NULL,
    FOREIGN KEY (media_item_id) REFERENCES media_items(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_watch_progress_last_watched
    ON watch_progress(last_watched_at DESC);

-- Retire the legacy integer version table; diesel tracks applied migrations
-- in __diesel_schema_migrations from here on.
DROP TABLE IF EXISTS schema_version;
