pub mod infrastructure;

use infrastructure::pg;
use tauri::Manager;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet])
        .setup(|app| {
            // Persistence is best-effort at startup (ADR 0003): when Postgres
            // or Docker is down the app still opens — the pool is managed in
            // state only once migrations succeed.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                dotenvy::dotenv().ok();
                let Some(url) = pg::database_url_from_env() else {
                    eprintln!(
                        "[disputatio] no database config (set DATABASE_URL, or POSTGRES_PASSWORD in .env) — running without persistence"
                    );
                    return;
                };
                match pg::create_pool(&url).await {
                    Ok(pool) => {
                        if let Err(e) = pg::run_migrations(&pool).await {
                            eprintln!("[disputatio] migrations failed: {e}");
                        }
                        handle.manage(pg::Db(pool));
                    }
                    Err(e) => eprintln!(
                        "[disputatio] Postgres unreachable — running without persistence: {e}"
                    ),
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
