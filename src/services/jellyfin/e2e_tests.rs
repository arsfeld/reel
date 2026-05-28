use serde_json::json;

use crate::models::media::{MediaType, SourceType};
use crate::services::jellyfin::{
    api::JellyfinClient,
    fake_server::{self, FakeJellyfinServer},
    source::JellyfinSource,
};
use crate::services::media_source::{MediaSource, SourceError};

fn source_for(server: &FakeJellyfinServer) -> JellyfinSource {
    let client = JellyfinClient::new(
        server.url(),
        server.token(),
        server.user_id(),
        "test-device",
    );
    JellyfinSource::new(client, "Test Jellyfin".into())
}

#[tokio::test]
async fn jellyfin_source_is_trait_object_compatible() {
    let client = JellyfinClient::new("http://localhost:8096", "t", "u", "d");
    let source = JellyfinSource::new(client, "Test".into());
    let _boxed: Box<dyn MediaSource> = Box::new(source);
}

#[tokio::test]
async fn source_type_is_jellyfin() {
    let client = JellyfinClient::new("http://localhost:8096", "t", "u", "d");
    let source = JellyfinSource::new(client, "Test".into());
    assert_eq!(source.source_type(), SourceType::Jellyfin);
}

#[tokio::test]
async fn libraries_then_items_then_detail() {
    let server = FakeJellyfinServer::builder()
        .token("tok")
        .user("user1")
        .library("lib-movies", "Movies", "movies")
        .library("lib-music", "Music", "music")
        .item_in("lib-movies", fake_server::movie("m1", "Dune", 2021))
        .item_in("lib-movies", fake_server::movie("m2", "Arrival", 2016))
        .start()
        .await;

    let source = source_for(&server);

    // test_connection proves token + reachability.
    let name = source.test_connection().await.unwrap();
    assert_eq!(name, "Test Jellyfin");

    // Libraries: Music is filtered out by the converter.
    let libs = source.libraries().await.unwrap();
    assert_eq!(libs.len(), 1);
    assert_eq!(libs[0].title, "Movies");

    // Library items.
    let items = source.library_items("lib-movies").await.unwrap();
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|i| i.media_type == MediaType::Movie));
    assert!(
        items
            .iter()
            .all(|i| i.library_section_id == Some("lib-movies".to_string()))
    );

    // Detail for one movie.
    let detail = source.metadata("m1").await.unwrap();
    assert_eq!(detail.item.title, "Dune");
    assert_eq!(detail.item.year, Some(2021));
    assert_eq!(detail.item.source_type, SourceType::Jellyfin);
}

#[tokio::test]
async fn show_seasons_episodes() {
    let server = FakeJellyfinServer::builder()
        .token("tok")
        .user("user1")
        .library("lib-tv", "TV", "tvshows")
        .item_in("lib-tv", fake_server::series("s1", "Breaking Bad", 2008))
        .child_of("s1", fake_server::season("se1", "Season 1", 1, "s1"))
        .child_of(
            "se1",
            fake_server::episode("e1", "Pilot", 1, 1, "se1", "s1"),
        )
        .child_of(
            "se1",
            fake_server::episode("e2", "Cat's in the Bag", 1, 2, "se1", "s1"),
        )
        .start()
        .await;

    let source = source_for(&server);

    // children(series) → seasons.
    let seasons = source.children("s1").await.unwrap();
    assert_eq!(seasons.len(), 1);
    assert_eq!(seasons[0].title, "Season 1");
    assert_eq!(seasons[0].media_type, MediaType::Season);

    // children(season) → episodes.
    let episodes = source.children("se1").await.unwrap();
    assert_eq!(episodes.len(), 2);
    assert_eq!(episodes[0].title, "Pilot");
    assert_eq!(episodes[0].season_number, Some(1));
    assert_eq!(episodes[0].episode_number, Some(1));
}

#[tokio::test]
async fn resume_position_from_server() {
    // A movie ~120 min long, resume at 18:42 = 1122 seconds.
    let mut movie = fake_server::movie("m1", "Dune", 2021);
    movie["RunTimeTicks"] = json!(120_i64 * 60 * 10_000_000);
    movie["UserData"] = json!({
        "Played": false,
        "PlaybackPositionTicks": 1122_i64 * 10_000_000,
    });

    let server = FakeJellyfinServer::builder()
        .token("tok")
        .user("user1")
        .resume_item(movie)
        .start()
        .await;

    let source = source_for(&server);
    let items = source.continue_watching().await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].playback_position_ms, Some(1_122_000));
    // resume_position_secs backs up 10s from the server offset.
    assert_eq!(items[0].resume_position_secs(), Some(1122.0 - 10.0));
}

#[tokio::test]
async fn report_progress_hits_server() {
    let server = FakeJellyfinServer::builder()
        .token("tok")
        .user("user1")
        .start()
        .await;

    let source = source_for(&server);
    source
        .report_progress("m1", "playing", 60_000, 7_200_000)
        .await
        .unwrap();

    let calls = server.recorded_calls();
    let paths: Vec<&str> = calls.iter().map(|(p, _)| p.as_str()).collect();
    assert!(paths.contains(&"/Sessions/Playing"));
    assert!(paths.contains(&"/Sessions/Playing/Progress"));
    // Start precedes the first progress.
    let start_idx = paths
        .iter()
        .position(|p| *p == "/Sessions/Playing")
        .unwrap();
    let prog_idx = paths
        .iter()
        .position(|p| *p == "/Sessions/Playing/Progress")
        .unwrap();
    assert!(start_idx < prog_idx);
}

#[tokio::test]
async fn playing_start_precedes_first_progress() {
    let server = FakeJellyfinServer::builder()
        .token("tok")
        .user("user1")
        .start()
        .await;

    let source = source_for(&server);

    // First report: playing → START + Progress.
    source
        .report_progress("m1", "playing", 60_000, 7_200_000)
        .await
        .unwrap();
    // Second report: paused → Progress only (no new START).
    source
        .report_progress("m1", "paused", 70_000, 7_200_000)
        .await
        .unwrap();
    // Terminal: stopped → Stopped.
    source
        .report_progress("m1", "stopped", 80_000, 7_200_000)
        .await
        .unwrap();

    let calls = server.recorded_calls();
    let paths: Vec<&str> = calls.iter().map(|(p, _)| p.as_str()).collect();

    // Exactly one START, before any progress.
    let starts: Vec<usize> = paths
        .iter()
        .enumerate()
        .filter(|(_, p)| **p == "/Sessions/Playing")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(starts.len(), 1, "exactly one start call");

    let progress: Vec<usize> = paths
        .iter()
        .enumerate()
        .filter(|(_, p)| **p == "/Sessions/Playing/Progress")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(progress.len(), 2, "two progress calls");
    assert!(starts[0] < progress[0], "start before first progress");

    let stopped: Vec<usize> = paths
        .iter()
        .enumerate()
        .filter(|(_, p)| **p == "/Sessions/Playing/Stopped")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(stopped.len(), 1, "one stopped call");
    assert!(stopped[0] > progress[1], "stopped after last progress");
}

#[tokio::test]
async fn scrobble_marks_played() {
    let server = FakeJellyfinServer::builder()
        .token("tok")
        .user("user1")
        .start()
        .await;

    let source = source_for(&server);
    source.scrobble("m1").await.unwrap();

    let calls = server.recorded_calls();
    let played = calls
        .iter()
        .find(|(p, _)| p == "/UserPlayedItems/m1")
        .expect("played call recorded");
    assert_eq!(played.1["played"], json!(true));
}

#[tokio::test]
async fn unscrobble_marks_unplayed() {
    let server = FakeJellyfinServer::builder()
        .token("tok")
        .user("user1")
        .start()
        .await;

    let source = source_for(&server);
    source.unscrobble("m1").await.unwrap();

    let calls = server.recorded_calls();
    let played = calls
        .iter()
        .find(|(p, _)| p == "/UserPlayedItems/m1")
        .expect("unplayed call recorded");
    assert_eq!(played.1["played"], json!(false));
}

#[tokio::test]
async fn skip_markers_present_and_absent() {
    let server = FakeJellyfinServer::builder()
        .token("tok")
        .user("user1")
        .segment(
            "with-intro",
            json!({"Id": "seg1", "Type": "Intro", "StartTicks": 0, "EndTicks": 90_i64 * 10_000_000}),
        )
        .segment(
            "with-intro",
            json!({"Id": "seg2", "Type": "Outro", "StartTicks": 3000_i64 * 10_000_000, "EndTicks": 3100_i64 * 10_000_000}),
        )
        .start()
        .await;

    let source = source_for(&server);

    // Item with segments yields markers.
    let markers = source.skip_markers("with-intro", 3600.0).await.unwrap();
    assert_eq!(markers.intro_end_secs, 90.0);
    assert_eq!(markers.credits_start_secs, 3000.0);

    // Item with no segments (404) degrades to NotSupported, no crash.
    let result = source.skip_markers("no-segments", 3600.0).await;
    assert!(matches!(result, Err(SourceError::NotSupported(_))));
}

#[tokio::test]
async fn collections_absent_degrades() {
    let server = FakeJellyfinServer::builder()
        .token("tok")
        .user("user1")
        .library("lib-movies", "Movies", "movies")
        .item_in("lib-movies", fake_server::movie("m1", "Dune", 2021))
        .start()
        .await;

    let source = source_for(&server);
    // No box sets in the library → empty Vec, NOT an error (R13).
    let collections = source.collections("lib-movies").await.unwrap();
    assert!(collections.is_empty());
}

#[tokio::test]
async fn collections_present_are_browsable() {
    let server = FakeJellyfinServer::builder()
        .token("tok")
        .user("user1")
        .library("lib-movies", "Movies", "movies")
        .item_in("lib-movies", fake_server::box_set("bs1", "The Trilogy"))
        .item_in("bs1", fake_server::movie("m1", "Part One", 2021))
        .start()
        .await;

    let source = source_for(&server);
    let collections = source.collections("lib-movies").await.unwrap();
    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0].media_type, MediaType::Collection);

    let items = source.collection_items("bs1").await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "Part One");
}

#[tokio::test]
async fn hubs_from_latest_and_next_up() {
    let server = FakeJellyfinServer::builder()
        .token("tok")
        .user("user1")
        .latest_item(fake_server::movie("m1", "Dune", 2021))
        .next_up_item(fake_server::episode("e1", "Pilot", 1, 1, "se1", "s1"))
        .start()
        .await;

    let source = source_for(&server);
    let hubs = source.hubs().await.unwrap();
    assert_eq!(hubs.len(), 2);
    assert!(hubs.iter().any(|h| h.title == "Latest"));
    assert!(hubs.iter().any(|h| h.title == "Next Up"));
}

#[tokio::test]
async fn unauthorized_token_returns_auth_error() {
    let server = FakeJellyfinServer::builder()
        .token("correct")
        .user("user1")
        .start()
        .await;

    let client = JellyfinClient::new(server.url(), "wrong", server.user_id(), "device");
    let source = JellyfinSource::new(client, "Test".into());

    let result = source.test_connection().await;
    assert!(matches!(result, Err(SourceError::Auth(_))));
}
