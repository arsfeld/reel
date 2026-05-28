-- Offline downloads: self-contained download items, their show/season groups,
-- and the offline progress-report queue (schema for the offline-downloads
-- feature). Downloads carry their own metadata snapshot and have no foreign key
-- to media_items, so they survive a library rebuild and render with no source
-- reachable.

CREATE TABLE IF NOT EXISTS downloads (
    media_item_id TEXT PRIMARY KEY NOT NULL,
    part_key TEXT NOT NULL,
    source_type TEXT NOT NULL,
    source_id TEXT NOT NULL,
    state TEXT NOT NULL,
    fail_reason TEXT,
    byte_count BIGINT NOT NULL DEFAULT 0,
    total_size BIGINT,
    validator TEXT,
    file_path TEXT,
    group_id TEXT,
    queue_order BIGINT NOT NULL DEFAULT 0,
    enqueued_at TEXT NOT NULL,
    completed_at TEXT,
    media_type TEXT NOT NULL,
    title TEXT NOT NULL,
    year INTEGER,
    parent_id TEXT,
    season_number INTEGER,
    episode_number INTEGER,
    poster_path TEXT
);

CREATE INDEX IF NOT EXISTS idx_downloads_state ON downloads(state);
CREATE INDEX IF NOT EXISTS idx_downloads_group ON downloads(group_id);
CREATE INDEX IF NOT EXISTS idx_downloads_completed_at ON downloads(completed_at);

CREATE TABLE IF NOT EXISTS download_groups (
    id TEXT PRIMARY KEY NOT NULL,
    scope TEXT NOT NULL,
    parent_media_id TEXT NOT NULL,
    title TEXT NOT NULL,
    snapshot_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS pending_sync (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    media_item_id TEXT NOT NULL,
    rating_key TEXT NOT NULL,
    position_ms BIGINT NOT NULL DEFAULT 0,
    duration_ms BIGINT NOT NULL DEFAULT 0,
    kind TEXT NOT NULL,
    offline_recorded_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pending_sync_recorded
    ON pending_sync(offline_recorded_at);
