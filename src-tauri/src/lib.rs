mod commands;
mod db;
mod grouping;
mod key_store;
mod output;
mod policy;
mod provider;
mod tools;
mod website;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let path = app.path().app_data_dir()?.join("bench.sqlite3");
            let conn = db::open(&path).map_err(std::io::Error::other)?;
            app.manage(commands::AppState {
                db: std::sync::Mutex::new(conn),
                approvals: std::sync::Mutex::new(policy::ApprovalStore::default()),
                active_operations: std::sync::Mutex::new(std::collections::HashSet::new()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::load_snapshot,
            commands::open_openrouter_setup,
            commands::open_openrouter_usage,
            commands::open_openrouter_billing,
            commands::save_provider_key,
            commands::remove_provider_key,
            commands::select_provider,
            commands::update_model,
            commands::save_web_search_key,
            commands::remove_web_search_key,
            commands::configure_web_search,
            commands::load_messages,
            commands::load_tool_activity,
            commands::create_user_message,
            commands::create_project,
            commands::move_conversation,
            commands::update_settings,
            commands::set_sidebar_collapsed,
            commands::search_conversations,
            commands::stream_response,
            commands::load_website,
            commands::generate_website,
            commands::list_website_revisions,
            commands::restore_website_revision,
            commands::prepare_website_action,
            commands::approve_action,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
