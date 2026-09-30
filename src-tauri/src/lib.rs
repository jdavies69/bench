mod agent;
mod agent_commands;
mod application;
mod application_commands;
mod artifact;
mod artifact_commands;
mod attachment_commands;
mod attachment_extract;
mod attachments;
mod commands;
mod db;
mod grouping;
mod key_store;
mod media;
mod media_commands;
mod oauth;
mod output;
mod policy;
mod provider;
mod text_outputs;
mod tools;
mod updates;
mod usage;
mod website;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let path = app.path().app_data_dir()?.join("bench.sqlite3");
            let conn = db::open(&path).map_err(std::io::Error::other)?;
            app.manage(updates::UpdateState::default());
            app.manage(attachments::AttachmentState::default());
            app.manage(commands::AppState {
                db: std::sync::Mutex::new(conn),
                approvals: std::sync::Mutex::new(policy::ApprovalStore::default()),
                oauth_connection: std::sync::Mutex::new(None),
                active_operations: std::sync::Mutex::new(std::collections::HashSet::new()),
            });
            updates::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            attachment_commands::choose_attachments,
            attachment_commands::remove_staged_attachment,
            agent_commands::generate_agent_output,
            application_commands::load_application_values,
            application_commands::save_application_values,
            application_commands::evaluate_application,
            updates::app_update_status,
            updates::set_automatic_updates,
            updates::check_app_update,
            updates::install_app_update,
            artifact_commands::load_artifact,
            artifact_commands::generate_artifact,
            artifact_commands::list_artifact_revisions,
            artifact_commands::prepare_artifact_action,
            artifact_commands::restore_artifact_revision,
            artifact_commands::save_artifact_edits,
            artifact_commands::export_artifact,
            media_commands::generate_media_output,
            commands::load_snapshot,
            commands::connect_openrouter,
            commands::cancel_openrouter_connect,
            commands::open_openrouter_setup,
            commands::open_openrouter_usage,
            commands::open_openrouter_billing,
            commands::load_openrouter_usage,
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
