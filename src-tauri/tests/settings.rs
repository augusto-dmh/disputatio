//! Settings round-trip on a throwaway Postgres 16 (ADR 0009 testcontainers):
//! the `app_settings` key/value table through the domain port — vault path and
//! per-track position overrides (specs/queue-slice task 4, design.md keys).

use std::collections::BTreeMap;

use disputatio_lib::application::settings::SettingsService;
use disputatio_lib::domain::settings::{Settings, SettingsError, SettingsStore, TRACKS};
use disputatio_lib::infrastructure::pg;
use disputatio_lib::infrastructure::settings_pg::PgSettingsStore;
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::postgres::Postgres;

/// Starts Postgres, applies migration 0001, returns a ready store (plus the
/// container, which must stay alive for the connection to keep working).
async fn store_on_fresh_postgres() -> (PgSettingsStore, testcontainers::ContainerAsync<Postgres>) {
    let container = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container (docker daemon required, ADR 0003)");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("container port mapped to host");
    let pool = pg::create_pool(&format!(
        "postgres://postgres:postgres@localhost:{port}/postgres"
    ))
    .await
    .expect("connect sqlx pool");
    pg::run_migrations(&pool)
        .await
        .expect("migration 0001 applies cleanly");
    (PgSettingsStore(pool), container)
}

#[tokio::test]
async fn settings_round_trip_vault_path_and_track_positions() {
    let (store, _container) = store_on_fresh_postgres().await;
    let service = SettingsService::new(store);

    // Fresh database: the aggregate is empty.
    assert_eq!(
        service.get().await.expect("get on empty db"),
        Settings::default()
    );

    // Vault path: set, then overwritten in place — one row, latest wins.
    service
        .set_vault_path("  /home/augusto/vault  ")
        .await
        .expect("set vault path");
    service
        .set_vault_path("/home/augusto/other-vault")
        .await
        .expect("overwrite vault path");
    let settings = service.get().await.expect("get after vault path");
    assert_eq!(
        settings.vault_path.as_deref(),
        Some("/home/augusto/other-vault")
    );

    // Position overrides: one per track, keyed by track name (not raw key).
    for (track, position) in [
        (
            "fundamentos-enterprise",
            "courses/fundamentos-enterprise/classes/class-030.md",
        ),
        (
            "system-design",
            "courses/system-design/classes/class-008.md",
        ),
    ] {
        service
            .set_track_position(track, position)
            .await
            .expect("set track position");
    }
    let settings = service.get().await.expect("get after positions");
    assert_eq!(
        settings.track_positions,
        BTreeMap::from([
            (
                "fundamentos-enterprise".to_string(),
                "courses/fundamentos-enterprise/classes/class-030.md".to_string()
            ),
            (
                "system-design".to_string(),
                "courses/system-design/classes/class-008.md".to_string()
            ),
        ])
    );

    // Storage shows the upserted rows only — no duplicates from re-sets.
    let stored = service.into_store().load_all().await.expect("load_all");
    assert_eq!(stored.len(), 3, "vault path + 2 track positions");
}

#[tokio::test]
async fn blank_position_clears_the_override() {
    let (store, _container) = store_on_fresh_postgres().await;
    let service = SettingsService::new(store);

    service
        .set_track_position("videos", "videos/some-video.mp4")
        .await
        .expect("set videos position");
    service
        .set_track_position("videos", "   ")
        .await
        .expect("blank clears the override");

    let settings = service.get().await.expect("get after clear");
    assert!(settings.track_positions.is_empty(), "override removed");
}

#[tokio::test]
async fn unknown_tracks_and_blank_paths_are_rejected() {
    let (store, _container) = store_on_fresh_postgres().await;
    let service = SettingsService::new(store);

    for track in ["nonexistent", "Fundamentos-Enterprise"] {
        let error = service
            .set_track_position(track, "chunks/any.md")
            .await
            .expect_err("unknown track must be rejected");
        assert!(matches!(error, SettingsError::InvalidInput(_)));
    }
    for known in TRACKS {
        service
            .set_track_position(known, "chunks/any.md")
            .await
            .expect("known tracks are accepted");
    }

    let error = service
        .set_vault_path("   ")
        .await
        .expect_err("blank vault path must be rejected");
    assert!(matches!(error, SettingsError::InvalidInput(_)));
}

#[tokio::test]
async fn get_ignores_keys_this_version_does_not_know() {
    let (store, _container) = store_on_fresh_postgres().await;

    store
        .put("future_setting", "value")
        .await
        .expect("put raw key");
    store
        .put("track_position:future-track", "x")
        .await
        .expect("put raw position key");

    let service = SettingsService::new(store);
    let settings = service.get().await.expect("get with unknown keys");
    assert_eq!(settings, Settings::default(), "unknown keys are skipped");
    // ...but stay in storage for future versions.
    assert_eq!(
        service
            .into_store()
            .load_all()
            .await
            .expect("load_all")
            .len(),
        2
    );
}
