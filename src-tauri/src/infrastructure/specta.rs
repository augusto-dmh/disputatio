//! tauri-specta bindings emission (ADR 0009): one builder registering every
//! IPC command; typed TS bindings are generated to `../src/bindings.ts` and
//! committed. Kept in infrastructure per the design.md module map — the RC's
//! blast radius stays at the presentation edge.
//!
//! Call it fully qualified (`infrastructure::specta::...`): importing the
//! module name `specta` into a scope shadows the specta crate and breaks
//! `#[specta::specta]` annotations there.

use tauri_specta::{collect_commands, Builder};

/// The command registry: every IPC command is registered exactly once, here.
pub fn builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        crate::greet,
        crate::presentation::commands::reindex,
        crate::presentation::settings::get_settings,
        crate::presentation::settings::set_settings,
    ])
}

/// Export the committed TS bindings. Runs on debug builds (`tauri dev`) and
/// via the test below (`cargo test`), never in release.
pub fn export_bindings(builder: &mut Builder<tauri::Wry>) {
    #[cfg(debug_assertions)]
    builder
        .export(
            specta_typescript::Typescript::default(),
            "../src/bindings.ts",
        )
        .expect("failed to export typescript bindings");
}

/// Regenerates `../src/bindings.ts` so `cargo test` refreshes the committed
/// bindings after any command signature change (ADR 0009: no hand-written
/// types — drift is impossible).
#[test]
fn export_typescript_bindings() {
    let mut builder = builder();
    export_bindings(&mut builder);
    assert!(std::path::Path::new("../src/bindings.ts").exists());
}
