use crate::{
    application,
    artifact::{self, ArtifactKind},
    artifact_commands,
    commands::AppState,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use tauri::{AppHandle, State};
fn table(conn: &Connection) -> Result<(), String> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS application_values(conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE, revision INTEGER NOT NULL, values_json TEXT NOT NULL, PRIMARY KEY(conversation_id,revision));").map_err(|_|"Application values are unavailable.".to_owned())
}
pub fn load_values(
    conn: &Connection,
    id: &str,
    revision: u32,
    content: &Value,
) -> Result<Value, String> {
    table(conn)?;
    let app = application::parse(content)?;
    let saved: Option<String> = conn
        .query_row(
            "SELECT values_json FROM application_values WHERE conversation_id=?1 AND revision=?2",
            params![id, revision],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| "Application values are unavailable.")?;
    match saved {
        None => Ok(application::defaults(&app)),
        Some(raw) => application::values(
            &app,
            &serde_json::from_str(&raw).map_err(|_| "Saved application values are invalid.")?,
        ),
    }
}
pub fn save_values(
    conn: &Connection,
    id: &str,
    revision: u32,
    content: &Value,
    input: &Value,
) -> Result<Value, String> {
    let app = application::parse(content)?;
    let valid = application::values(&app, input)?;
    table(conn)?;
    conn.execute("INSERT INTO application_values(conversation_id,revision,values_json) VALUES(?1,?2,?3) ON CONFLICT(conversation_id,revision) DO UPDATE SET values_json=excluded.values_json",params![id,revision,valid.to_string()]).map_err(|_|"Could not save application values.")?;
    Ok(valid)
}
#[tauri::command]
pub fn load_application_values(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Value, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "Local database is unavailable.")?;
    if artifact_commands::kind(&conn, &conversation_id)? != ArtifactKind::Application {
        return Err("This conversation is not an Application.".into());
    }
    let saved = artifact::load(
        &artifact_commands::root(&app)?,
        &conversation_id,
        ArtifactKind::Application,
        application::validate,
    )?
    .ok_or("Create an Application first.")?;
    load_values(&conn, &conversation_id, saved.revision, &saved.content)
}
#[tauri::command]
pub fn save_application_values(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    revision: u32,
    values: Value,
) -> Result<Value, String> {
    let _guard =
        crate::commands::ConversationOperation::begin(&state.active_operations, &conversation_id)?;
    let conn = state
        .db
        .lock()
        .map_err(|_| "Local database is unavailable.")?;
    if artifact_commands::kind(&conn, &conversation_id)? != ArtifactKind::Application {
        return Err("This conversation is not an Application.".into());
    }
    let saved = artifact::load(
        &artifact_commands::root(&app)?,
        &conversation_id,
        ArtifactKind::Application,
        application::validate,
    )?
    .ok_or("Create an Application first.")?;
    if saved.revision != revision {
        return Err("Application changed. Reload before saving values.".into());
    }
    save_values(&conn, &conversation_id, revision, &saved.content, &values)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn values_persist_per_revision_and_reject_invalid_writes() {
        let path =
            std::env::temp_dir().join(format!("bench-app-values-{}.sqlite", uuid::Uuid::new_v4()));
        let content = serde_json::json!({"title":"Form","description":"","fields":[{"id":"x","label":"X","type":"number","default":1}],"outputs":[]});
        let id;
        {
            let mut conn = Connection::open(&path).unwrap();
            crate::db::initialize(&conn).unwrap();
            id = crate::db::create_user_message(&mut conn, None, "form", "application")
                .unwrap()
                .id;
            assert_eq!(load_values(&conn, &id, 1, &content).unwrap()["x"], 1);
            save_values(&conn, &id, 1, &content, &serde_json::json!({"x":9})).unwrap();
            assert!(
                save_values(&conn, &id, 1, &content, &serde_json::json!({"x":"wrong"})).is_err()
            );
        }
        let conn = Connection::open(&path).unwrap();
        assert_eq!(load_values(&conn, &id, 1, &content).unwrap()["x"], 9);
        assert_eq!(load_values(&conn, &id, 2, &content).unwrap()["x"], 1);
        drop(conn);
        std::fs::remove_file(path).unwrap();
    }
}
#[tauri::command]
pub fn evaluate_application(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    revision: u32,
    values: Value,
) -> Result<Vec<Option<f64>>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "Local database is unavailable.")?;
    if artifact_commands::kind(&conn, &conversation_id)? != ArtifactKind::Application {
        return Err("This conversation is not an Application.".into());
    }
    let saved = artifact::load(
        &artifact_commands::root(&app)?,
        &conversation_id,
        ArtifactKind::Application,
        application::validate,
    )?
    .ok_or("Create an Application first.")?;
    if saved.revision != revision {
        return Err("Application changed. Reload before calculating.".into());
    }
    let schema = application::parse(&saved.content)?;
    let values = application::values(&schema, &values)?;
    Ok(schema
        .outputs
        .iter()
        .map(|o| application::evaluate(&o.expression, &values).ok())
        .collect())
}
