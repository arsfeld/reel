// @generated automatically by Diesel CLI.

diesel::table! {
    media_items (id) {
        id -> Text,
        source_type -> Text,
        source_id -> Text,
        external_id -> Text,
        media_type -> Text,
        title -> Text,
        year -> Nullable<Integer>,
        overview -> Nullable<Text>,
        content_rating -> Nullable<Text>,
        rating -> Nullable<Double>,
        runtime_minutes -> Nullable<Integer>,
        poster_path -> Nullable<Text>,
        backdrop_path -> Nullable<Text>,
        genres -> Text,
        parent_id -> Nullable<Text>,
        season_number -> Nullable<Integer>,
        episode_number -> Nullable<Integer>,
        air_date -> Nullable<Text>,
        file_path -> Nullable<Text>,
        video_resolution -> Nullable<Text>,
        hdr -> Nullable<Text>,
        added_at -> Text,
        updated_at -> Text,
    }
}

diesel::table! {
    sources (id) {
        id -> Text,
        source_type -> Text,
        name -> Text,
        config -> Text,
        enabled -> Integer,
        last_synced_at -> Nullable<Text>,
    }
}

diesel::table! {
    watch_progress (media_item_id) {
        media_item_id -> Text,
        position_seconds -> Double,
        duration_seconds -> Double,
        watched -> Integer,
        last_watched_at -> Text,
    }
}

diesel::table! {
    downloads (media_item_id) {
        media_item_id -> Text,
        part_key -> Text,
        source_type -> Text,
        source_id -> Text,
        state -> Text,
        fail_reason -> Nullable<Text>,
        byte_count -> BigInt,
        total_size -> Nullable<BigInt>,
        validator -> Nullable<Text>,
        file_path -> Nullable<Text>,
        group_id -> Nullable<Text>,
        queue_order -> BigInt,
        enqueued_at -> Text,
        completed_at -> Nullable<Text>,
        media_type -> Text,
        title -> Text,
        year -> Nullable<Integer>,
        parent_id -> Nullable<Text>,
        season_number -> Nullable<Integer>,
        episode_number -> Nullable<Integer>,
        poster_path -> Nullable<Text>,
    }
}

diesel::table! {
    download_groups (id) {
        id -> Text,
        scope -> Text,
        parent_media_id -> Text,
        title -> Text,
        snapshot_at -> Text,
    }
}

diesel::table! {
    pending_sync (id) {
        id -> BigInt,
        media_item_id -> Text,
        rating_key -> Text,
        position_ms -> BigInt,
        duration_ms -> BigInt,
        kind -> Text,
        offline_recorded_at -> Text,
    }
}

diesel::joinable!(watch_progress -> media_items (media_item_id));

diesel::allow_tables_to_appear_in_same_query!(
    media_items,
    sources,
    watch_progress,
    downloads,
    download_groups,
    pending_sync,
);
