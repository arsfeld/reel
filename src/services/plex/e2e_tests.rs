use crate::db::{init_db, media_repo::MediaRepo};
use crate::models::media::{MediaType, SourceType};
use crate::services::media_source::MediaSource;
use crate::services::plex::{
    api::PlexClient,
    fake_server::{self, FakePlexServer},
    source::PlexSource,
};
use rusqlite::Connection;

fn setup_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    init_db(&conn).unwrap();
    conn
}

#[tokio::test]
async fn full_movie_sync_pipeline() {
    let server = FakePlexServer::builder()
        .server_name("Home Server")
        .token("test-token")
        .library("1", "Movies", "movie")
        .item_in(
            "1",
            fake_server::movie_detailed(
                "100",
                "Dune",
                2021,
                &["Science Fiction", "Adventure"],
                155,
                8.0,
            ),
        )
        .item_in("1", fake_server::movie("101", "Arrival", 2016))
        .start()
        .await;

    let client = PlexClient::new(server.url(), server.token());
    let source = PlexSource::new(client, "Test".into());

    // Test connection
    let name = source.test_connection().await.unwrap();
    assert_eq!(name, "Home Server");

    // List libraries
    let libs = source.libraries().await.unwrap();
    assert_eq!(libs.len(), 1);
    assert_eq!(libs[0].title, "Movies");

    // Fetch library items
    let items = source.library_items("1").await.unwrap();
    assert_eq!(items.len(), 2);

    // Store in database
    let conn = setup_db();
    let repo = MediaRepo::new(&conn);
    for item in &items {
        repo.upsert(item).unwrap();
    }

    // Verify database contents
    let movies = repo.list_by_type(MediaType::Movie, 100, 0).unwrap();
    assert_eq!(movies.len(), 2);
    assert_eq!(movies[0].title, "Arrival");
    assert_eq!(movies[1].title, "Dune");
    assert_eq!(movies[1].year, Some(2021));
    assert_eq!(movies[1].runtime_minutes, Some(155));
    assert_eq!(movies[1].genres, vec!["Science Fiction", "Adventure"]);
    assert_eq!(movies[1].source_type, SourceType::Plex);
}

#[tokio::test]
async fn full_tv_hierarchy_sync() {
    let server = FakePlexServer::builder()
        .server_name("TV Server")
        .token("tv-token")
        .library("2", "TV Shows", "show")
        .item_in("2", fake_server::show("200", "Breaking Bad", 2008))
        .child_of("200", fake_server::season("201", "Season 1", 1, "200"))
        .child_of(
            "201",
            fake_server::episode("210", "Pilot", 1, 1, "201", "200"),
        )
        .child_of(
            "201",
            fake_server::episode("211", "Cat's in the Bag", 1, 2, "201", "200"),
        )
        .start()
        .await;

    let client = PlexClient::new(server.url(), server.token());
    let source = PlexSource::new(client, "Test".into());

    // Fetch show
    let shows = source.library_items("2").await.unwrap();
    assert_eq!(shows.len(), 1);
    assert_eq!(shows[0].title, "Breaking Bad");

    // Fetch seasons (children of show)
    let seasons = source.children("200").await.unwrap();
    assert_eq!(seasons.len(), 1);
    assert_eq!(seasons[0].title, "Season 1");

    // Fetch episodes (children of season)
    let episodes = source.children("201").await.unwrap();
    assert_eq!(episodes.len(), 2);
    assert_eq!(episodes[0].title, "Pilot");
    assert_eq!(episodes[0].season_number, Some(1));
    assert_eq!(episodes[0].episode_number, Some(1));

    // Store everything in DB
    let conn = setup_db();
    let repo = MediaRepo::new(&conn);
    for item in shows.iter().chain(seasons.iter()).chain(episodes.iter()) {
        repo.upsert(item).unwrap();
    }

    // Verify hierarchy
    assert_eq!(repo.count_by_type(MediaType::Show).unwrap(), 1);
    assert_eq!(repo.count_by_type(MediaType::Season).unwrap(), 1);
    assert_eq!(repo.count_by_type(MediaType::Episode).unwrap(), 2);

    // Verify parent relationships
    let db_seasons = repo.list_by_parent(&shows[0].id).unwrap();
    assert_eq!(db_seasons.len(), 1);

    let db_episodes = repo.list_by_parent(&seasons[0].id).unwrap();
    assert_eq!(db_episodes.len(), 2);
    assert_eq!(db_episodes[0].episode_number, Some(1));
    assert_eq!(db_episodes[1].episode_number, Some(2));
}

#[tokio::test]
async fn unauthorized_token_returns_auth_error() {
    let server = FakePlexServer::builder()
        .server_name("Secured")
        .token("correct-token")
        .start()
        .await;

    let client = PlexClient::new(server.url(), "wrong-token");
    let source = PlexSource::new(client, "Test".into());

    let result = source.test_connection().await;
    assert!(matches!(
        result,
        Err(crate::services::media_source::SourceError::Auth(_))
    ));
}

#[tokio::test]
async fn incremental_sync_picks_up_new_items() {
    let server = FakePlexServer::builder()
        .server_name("Growing Server")
        .token("token")
        .library("1", "Movies", "movie")
        .item_in("1", fake_server::movie("100", "Dune", 2021))
        .start()
        .await;

    let client = PlexClient::new(server.url(), server.token());
    let source = PlexSource::new(client, "Test".into());
    let conn = setup_db();
    let repo = MediaRepo::new(&conn);

    // Initial sync
    let items = source.library_items("1").await.unwrap();
    assert_eq!(items.len(), 1);
    for item in &items {
        repo.upsert(item).unwrap();
    }
    assert_eq!(repo.count_by_type(MediaType::Movie).unwrap(), 1);

    // Server gets a new movie
    server.add_item("1", fake_server::movie("102", "Arrival", 2016));

    // Re-sync picks it up
    let items = source.library_items("1").await.unwrap();
    assert_eq!(items.len(), 2);
    for item in &items {
        repo.upsert(item).unwrap();
    }
    assert_eq!(repo.count_by_type(MediaType::Movie).unwrap(), 2);
}

#[tokio::test]
async fn metadata_fetch_single_item_preserves_all_fields() {
    let server = FakePlexServer::builder()
        .server_name("Detail Server")
        .token("token")
        .library("1", "Movies", "movie")
        .item_in(
            "1",
            fake_server::movie_detailed(
                "100",
                "Dune",
                2021,
                &["Science Fiction", "Adventure"],
                155,
                8.0,
            ),
        )
        .start()
        .await;

    let client = PlexClient::new(server.url(), server.token());
    let source = PlexSource::new(client, "Test".into());

    let detail = source.metadata("100").await.unwrap();
    let item = &detail.item;
    assert_eq!(item.title, "Dune");
    assert_eq!(item.year, Some(2021));
    assert_eq!(item.runtime_minutes, Some(155));
    assert_eq!(item.genres, vec!["Science Fiction", "Adventure"]);
    assert!(item.poster_path.is_some());
    assert!(item.backdrop_path.is_some());
    assert!(item.file_path.is_some());
    assert_eq!(item.media_type, MediaType::Movie);
    assert_eq!(item.source_type, SourceType::Plex);
}

#[tokio::test]
async fn metadata_not_found_returns_error() {
    let server = FakePlexServer::builder()
        .server_name("Empty Server")
        .token("token")
        .start()
        .await;

    let client = PlexClient::new(server.url(), server.token());
    let source = PlexSource::new(client, "Test".into());

    let result = source.metadata("999").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn mixed_library_types_filters_unsupported() {
    let server = FakePlexServer::builder()
        .server_name("Mixed Server")
        .token("token")
        .library("1", "Movies", "movie")
        .library("2", "TV Shows", "show")
        .library("3", "Music", "artist")
        .library("4", "Photos", "photo")
        .start()
        .await;

    let client = PlexClient::new(server.url(), server.token());
    let source = PlexSource::new(client, "Test".into());

    let libs = source.libraries().await.unwrap();
    assert_eq!(libs.len(), 2);
    assert!(libs.iter().any(|l| l.title == "Movies"));
    assert!(libs.iter().any(|l| l.title == "TV Shows"));
}

#[tokio::test]
async fn playback_url_points_to_server() {
    let server = FakePlexServer::builder()
        .server_name("Play Server")
        .token("play-token")
        .library("1", "Movies", "movie")
        .item_in("1", fake_server::movie("100", "Dune", 2021))
        .start()
        .await;

    let client = PlexClient::new(server.url(), server.token());
    let source = PlexSource::new(client, "Test".into());

    let items = source.library_items("1").await.unwrap();
    let part_key = items[0].file_path.as_ref().unwrap();
    let url = source.playback_url(part_key);

    assert!(url.starts_with(server.url()));
    assert!(url.contains(part_key));
    assert!(url.contains("X-Plex-Token=play-token"));
    assert!(url.contains("download=1"));
}

#[tokio::test]
async fn upsert_updates_existing_items_on_resync() {
    let server = FakePlexServer::builder()
        .server_name("Update Server")
        .token("token")
        .library("1", "Movies", "movie")
        .item_in("1", fake_server::movie("100", "Dune", 2021))
        .start()
        .await;

    let client = PlexClient::new(server.url(), server.token());
    let source = PlexSource::new(client, "Test".into());
    let conn = setup_db();
    let repo = MediaRepo::new(&conn);

    // Initial sync
    let items = source.library_items("1").await.unwrap();
    for item in &items {
        repo.upsert(item).unwrap();
    }

    // Server updates the movie (simulate metadata refresh)
    server.remove_item("100");
    server.add_item(
        "1",
        fake_server::movie_detailed("100", "Dune: Part One", 2021, &["Sci-Fi"], 155, 8.5),
    );

    // Re-sync
    let items = source.library_items("1").await.unwrap();
    for item in &items {
        repo.upsert(item).unwrap();
    }

    // Should update, not duplicate
    assert_eq!(repo.count_by_type(MediaType::Movie).unwrap(), 1);
    let movie = repo.list_by_type(MediaType::Movie, 1, 0).unwrap();
    assert_eq!(movie[0].title, "Dune: Part One");
    assert_eq!(movie[0].genres, vec!["Sci-Fi"]);
}

#[tokio::test]
async fn empty_library_returns_no_items() {
    let server = FakePlexServer::builder()
        .server_name("Empty Library Server")
        .token("token")
        .library("1", "Movies", "movie")
        .start()
        .await;

    let client = PlexClient::new(server.url(), server.token());
    let source = PlexSource::new(client, "Test".into());

    let items = source.library_items("1").await.unwrap();
    assert!(items.is_empty());
}

#[tokio::test]
async fn multiple_libraries_independent() {
    let server = FakePlexServer::builder()
        .server_name("Multi Server")
        .token("token")
        .library("1", "Movies", "movie")
        .library("2", "TV Shows", "show")
        .item_in("1", fake_server::movie("100", "Dune", 2021))
        .item_in("1", fake_server::movie("101", "Arrival", 2016))
        .item_in("2", fake_server::show("200", "Breaking Bad", 2008))
        .start()
        .await;

    let client = PlexClient::new(server.url(), server.token());
    let source = PlexSource::new(client, "Test".into());

    let movies = source.library_items("1").await.unwrap();
    assert_eq!(movies.len(), 2);

    let shows = source.library_items("2").await.unwrap();
    assert_eq!(shows.len(), 1);
    assert_eq!(shows[0].title, "Breaking Bad");
}
