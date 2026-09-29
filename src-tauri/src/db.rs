use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;
use uuid::Uuid;

use crate::output;

pub const MISC_ID: &str = "miscellaneous";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub is_system: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub output_type: String,
    pub output_selection: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

/// Product-facing tool history excludes debugging payloads and source JSON.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolActivity {
    pub id: String,
    pub user_message_id: Option<String>,
    pub tool_name: String,
    pub status: String,
    pub summary: String,
}

pub struct ToolExecutionRecord<'a> {
    pub tool_name: &'a str,
    pub status: &'a str,
    pub summary: &'a str,
    pub detail_json: &'a str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub execution_behavior: String,
    pub approval_behavior: String,
    pub model_provider: String,
    pub sidebar_collapsed: bool,
    pub web_search_backend: String,
    pub web_search_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub outputs: Vec<crate::output::OutputDefinition>,
    pub projects: Vec<Project>,
    pub conversations: Vec<Conversation>,
    pub settings: Settings,
    pub providers: Vec<crate::provider::ProviderStatus>,
    pub web_search_key_source: String,
    pub web_search_status: String,
}

pub fn open(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    initialize(&conn)?;
    Ok(conn)
}

pub fn initialize(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         CREATE TABLE IF NOT EXISTS projects (
           id TEXT PRIMARY KEY, name TEXT NOT NULL UNIQUE,
           is_system INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS conversations (
           id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id),
           title TEXT NOT NULL, output_type TEXT NOT NULL DEFAULT 'auto',
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
           updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS messages (
           id TEXT PRIMARY KEY, conversation_id TEXT NOT NULL REFERENCES conversations(id),
           role TEXT NOT NULL CHECK(role IN ('user', 'assistant')),
           content TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE INDEX IF NOT EXISTS messages_conversation_idx ON messages(conversation_id, created_at);
         CREATE TABLE IF NOT EXISTS tool_executions (
           id TEXT PRIMARY KEY,
           conversation_id TEXT NOT NULL REFERENCES conversations(id),
           user_message_id TEXT REFERENCES messages(id),
           tool_name TEXT NOT NULL,
           status TEXT NOT NULL,
           summary TEXT NOT NULL,
           detail_json TEXT NOT NULL,
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE INDEX IF NOT EXISTS tool_executions_conversation_idx ON tool_executions(conversation_id, created_at);
         CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT OR IGNORE INTO projects(id, name, is_system) VALUES ('miscellaneous', 'Miscellaneous', 1);
         INSERT OR IGNORE INTO settings(key, value) VALUES ('execution_behavior', 'balanced');
         INSERT OR IGNORE INTO settings(key, value) VALUES ('approval_behavior', 'important');
         INSERT OR IGNORE INTO settings(key, value) VALUES ('model_provider', 'openrouter');
         INSERT OR IGNORE INTO settings(key, value) VALUES ('model_openrouter', 'openrouter/auto');
         INSERT OR IGNORE INTO settings(key, value) VALUES ('model_openai', 'gpt-5.4-mini');
         INSERT OR IGNORE INTO settings(key, value) VALUES ('model_anthropic', 'claude-sonnet-5-5');
         INSERT OR IGNORE INTO settings(key, value) VALUES ('model_xai', 'grok-4.7');
         INSERT OR IGNORE INTO settings(key, value) VALUES ('web_search_backend', 'auto');
         INSERT OR IGNORE INTO settings(key, value) VALUES ('web_search_url', '');",
    )
    .map_err(|e| e.to_string())?;
    // Legacy backend choices are inert. Keep their URL/Keychain credential so
    // migration cannot destroy user data, while defaulting to the OpenRouter
    // hosted tool selected for this milestone.
    conn.execute(
        "UPDATE settings SET value = 'auto' WHERE key = 'web_search_backend' AND value IN ('brave', 'searxng')",
        [],
    )
    .map_err(|e| e.to_string())?;
    // Legacy executions remain unassociated; guessing a request would invent
    // history. New executions validate their exact saved user message.
    ensure_column(
        conn,
        "tool_executions",
        "user_message_id",
        "TEXT REFERENCES messages(id)",
    )?;
    ensure_column(
        conn,
        "conversations",
        "output_selection",
        "TEXT NOT NULL DEFAULT 'auto'",
    )?;
    ensure_column(
        conn,
        "conversations",
        "project_manually_assigned",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    conn.execute(
        "UPDATE conversations SET output_type = 'chat' WHERE output_type = 'auto'",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO settings(key, value) VALUES ('sidebar_collapsed', 'false')",
        [],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn ensure_column(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|e| e.to_string())?;
    let exists = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .any(|name| name.is_ok_and(|name| name == column));
    if !exists {
        conn.execute_batch(&format!(
            "ALTER TABLE {table} ADD COLUMN {column} {definition}"
        ))
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn projects(conn: &Connection) -> Result<Vec<Project>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name, is_system FROM projects ORDER BY is_system, name COLLATE NOCASE")
        .map_err(|e| e.to_string())?;
    let result = stmt
        .query_map([], |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                is_system: row.get::<_, i64>(2)? != 0,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string());
    result
}

pub fn conversations(conn: &Connection) -> Result<Vec<Conversation>, String> {
    let mut stmt = conn
        .prepare("SELECT id, project_id, title, output_type, updated_at, output_selection FROM conversations ORDER BY updated_at DESC, rowid DESC")
        .map_err(|e| e.to_string())?;
    let result = stmt
        .query_map([], |row| {
            Ok(Conversation {
                id: row.get(0)?,
                project_id: row.get(1)?,
                title: row.get(2)?,
                output_type: row.get(3)?,
                updated_at: row.get(4)?,
                output_selection: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string());
    result
}

pub fn settings(conn: &Connection) -> Result<Settings, String> {
    let read = |key: &str| -> Result<String, String> {
        conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .map_err(|e| e.to_string())
    };
    Ok(Settings {
        execution_behavior: read("execution_behavior")?,
        approval_behavior: read("approval_behavior")?,
        model_provider: read("model_provider")?,
        sidebar_collapsed: read("sidebar_collapsed")? == "true",
        web_search_backend: read("web_search_backend")?,
        web_search_url: read("web_search_url")?,
    })
}

pub fn set_web_search(
    conn: &Connection,
    backend: &str,
    searxng_url: Option<&str>,
) -> Result<Settings, String> {
    if !matches!(backend, "auto" | "off") {
        return Err("Choose a valid web search connection.".into());
    }
    let existing = settings(conn)?;
    let url = searxng_url.unwrap_or(&existing.web_search_url);
    conn.execute(
        "UPDATE settings SET value = CASE key WHEN 'web_search_backend' THEN ?1 ELSE ?2 END
         WHERE key IN ('web_search_backend', 'web_search_url')",
        params![backend, url],
    )
    .map_err(|e| e.to_string())?;
    settings(conn)
}

pub fn model(conn: &Connection, provider: crate::provider::ProviderId) -> Result<String, String> {
    let key = format!("model_{}", provider.as_str());
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
        row.get(0)
    })
    .optional()
    .map(|value: Option<String>| value.unwrap_or_else(|| provider.default_model().into()))
    .map_err(|error| error.to_string())
}

pub fn select_provider(
    conn: &Connection,
    provider: crate::provider::ProviderId,
) -> Result<Settings, String> {
    conn.execute(
        "UPDATE settings SET value = ?1 WHERE key = 'model_provider'",
        [provider.as_str()],
    )
    .map_err(|error| error.to_string())?;
    settings(conn)
}

pub fn update_model(
    conn: &Connection,
    provider: crate::provider::ProviderId,
    model: &str,
) -> Result<(), String> {
    let model = model.trim();
    if model.is_empty()
        || model.len() > 128
        || !model
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "/-_.:~".contains(character))
    {
        return Err("Enter a valid model ID (up to 128 characters).".into());
    }
    let key = format!("model_{}", provider.as_str());
    conn.execute(
        "UPDATE settings SET value = ?1 WHERE key = ?2",
        params![model, key],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn create_user_message(
    conn: &mut Connection,
    conversation_id: Option<&str>,
    content: &str,
    output_type: &str,
) -> Result<Conversation, String> {
    let content = content.trim();
    if content.is_empty() {
        return Err("Write a message before sending.".into());
    }
    if content.chars().count() > 20_000 {
        return Err("Message is too long (20,000 character limit).".into());
    }
    let selection = output::OutputType::parse_selection(output_type)?;
    let effective = selection.unwrap_or_else(|| output::OutputType::infer(content));
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let id = if let Some(id) = conversation_id {
        let existing: Option<(String, bool, String)> = tx
            .query_row(
                "SELECT project_id, project_manually_assigned, output_type FROM conversations WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get::<_, i64>(1)? != 0, r.get(2)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        let (project_id, manually_assigned, prior_output) =
            existing.ok_or("Conversation not found.")?;
        let route = if selection.is_some() || effective != output::OutputType::Chat {
            effective.as_str()
        } else {
            &prior_output
        };
        tx.execute(
            "UPDATE conversations SET output_type = ?1, output_selection = ?2 WHERE id = ?3",
            params![route, output_type, id],
        )
        .map_err(|e| e.to_string())?;
        if project_id == MISC_ID && !manually_assigned {
            let previous = messages(&tx, id)?
                .into_iter()
                .filter(|message| message.role == "user")
                .map(|message| message.content)
                .collect::<Vec<_>>();
            let context = previous
                .into_iter()
                .chain(std::iter::once(content.to_owned()))
                .collect::<Vec<_>>()
                .join("\n");
            let project = crate::grouping::classify(&tx, &context)?;
            if project != MISC_ID {
                tx.execute(
                    "UPDATE conversations SET project_id = ?1 WHERE id = ?2",
                    params![project, id],
                )
                .map_err(|e| e.to_string())?;
            }
        }
        id.to_owned()
    } else {
        let id = Uuid::new_v4().to_string();
        let project = crate::grouping::classify(&tx, content)?;
        let title = content
            .lines()
            .next()
            .unwrap_or(content)
            .chars()
            .take(54)
            .collect::<String>();
        tx.execute(
            "INSERT INTO conversations(id, project_id, title, output_type, output_selection) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, project, title, effective.as_str(), output_type],
        )
        .map_err(|e| e.to_string())?;
        id
    };
    tx.execute(
        "INSERT INTO messages(id, conversation_id, role, content) VALUES (?1, ?2, 'user', ?3)",
        params![Uuid::new_v4().to_string(), id, content],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE conversations SET updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
        [&id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    conversation(conn, &id)
}

pub fn conversation(conn: &Connection, id: &str) -> Result<Conversation, String> {
    conn.query_row(
        "SELECT id, project_id, title, output_type, updated_at, output_selection FROM conversations WHERE id = ?1",
        [id],
        |row| {
            Ok(Conversation {
                id: row.get(0)?,
                project_id: row.get(1)?,
                title: row.get(2)?,
                output_type: row.get(3)?,
                updated_at: row.get(4)?,
                output_selection: row.get(5)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

pub fn messages(conn: &Connection, conversation_id: &str) -> Result<Vec<Message>, String> {
    let mut stmt = conn
        .prepare("SELECT id, conversation_id, role, content, created_at FROM messages WHERE conversation_id = ?1 ORDER BY rowid")
        .map_err(|e| e.to_string())?;
    let result = stmt
        .query_map([conversation_id], |row| {
            Ok(Message {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string());
    result
}

pub fn save_assistant(
    conn: &Connection,
    conversation_id: &str,
    content: &str,
) -> Result<(), String> {
    if content.trim().is_empty() {
        return Err("The response was empty.".into());
    }
    conn.execute_batch("SAVEPOINT save_assistant")
        .map_err(|e| e.to_string())?;
    let result = (|| {
        let updated = conn
            .execute(
                "UPDATE conversations SET updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
                [conversation_id],
            )
            .map_err(|e| e.to_string())?;
        if updated == 0 {
            return Err("Conversation not found.".into());
        }
        conn.execute(
            "INSERT INTO messages(id, conversation_id, role, content) VALUES (?1, ?2, 'assistant', ?3)",
            params![Uuid::new_v4().to_string(), conversation_id, content],
        ).map_err(|e| e.to_string())?;
        Ok(())
    })();
    match result {
        Ok(()) => conn
            .execute_batch("RELEASE SAVEPOINT save_assistant")
            .map_err(|e| e.to_string())?,
        Err(error) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO SAVEPOINT save_assistant; RELEASE SAVEPOINT save_assistant",
            );
            return Err(error);
        }
    }
    Ok(())
}

#[allow(dead_code)] // Retained for migration tests of pre-association search records.
pub fn record_tool_execution(
    conn: &Connection,
    conversation_id: &str,
    tool_name: &str,
    status: &str,
    summary: &str,
    detail_json: &str,
) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO tool_executions(id, conversation_id, tool_name, status, summary, detail_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, conversation_id, tool_name, status, summary, detail_json],
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

pub fn record_tool_execution_for_message(
    conn: &Connection,
    conversation_id: &str,
    user_message_id: &str,
    record: &ToolExecutionRecord<'_>,
) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    let changed = conn.execute(
        "INSERT INTO tool_executions(id, conversation_id, user_message_id, tool_name, status, summary, detail_json)
         SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7
         WHERE EXISTS (SELECT 1 FROM messages WHERE id = ?3 AND conversation_id = ?2 AND role = 'user')",
        params![id, conversation_id, user_message_id, record.tool_name, record.status, record.summary, record.detail_json],
    ).map_err(|_| "Could not save search activity. Your request is still saved.".to_owned())?;
    if changed == 0 {
        return Err("This search request is unavailable.".into());
    }
    Ok(id)
}

pub fn load_tool_activity(
    conn: &Connection,
    conversation_id: &str,
) -> Result<Vec<ToolActivity>, String> {
    let mut statement = conn
        .prepare(
            "SELECT id, user_message_id, tool_name, status, summary FROM tool_executions
         WHERE conversation_id = ?1 ORDER BY created_at, rowid",
        )
        .map_err(|_| "Could not load search activity.".to_owned())?;
    let activity = statement
        .query_map([conversation_id], |row| {
            Ok(ToolActivity {
                id: row.get(0)?,
                user_message_id: row.get(1)?,
                tool_name: row.get(2)?,
                status: row.get(3)?,
                summary: row.get(4)?,
            })
        })
        .map_err(|_| "Could not load search activity.".to_owned())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "Could not load search activity.".to_owned());
    activity
}

pub fn move_conversation(
    conn: &Connection,
    conversation_id: &str,
    project_id: &str,
) -> Result<(), String> {
    let changed = conn
        .execute(
            "UPDATE conversations SET project_id = ?1, project_manually_assigned = 1 WHERE id = ?2",
            params![project_id, conversation_id],
        )
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("Conversation not found.".into());
    }
    Ok(())
}

pub fn create_project(conn: &Connection, name: &str) -> Result<Project, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 48 {
        return Err("Project name must be 1–48 characters.".into());
    }
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO projects(id, name) VALUES (?1, ?2)",
        params![id, name],
    )
    .map_err(|e| e.to_string())?;
    Ok(Project {
        id,
        name: name.to_owned(),
        is_system: false,
    })
}

pub fn update_settings(
    conn: &Connection,
    execution: &str,
    approval: &str,
) -> Result<Settings, String> {
    if !matches!(execution, "discuss" | "balanced" | "just_do_it")
        || !matches!(approval, "always" | "important" | "autonomous")
    {
        return Err("Invalid settings selection.".into());
    }
    conn.execute(
        "UPDATE settings SET value = ?1 WHERE key = 'execution_behavior'",
        [execution],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE settings SET value = ?1 WHERE key = 'approval_behavior'",
        [approval],
    )
    .map_err(|e| e.to_string())?;
    settings(conn)
}

pub fn set_sidebar_collapsed(conn: &Connection, collapsed: bool) -> Result<Settings, String> {
    conn.execute(
        "UPDATE settings SET value = ?1 WHERE key = 'sidebar_collapsed'",
        [if collapsed { "true" } else { "false" }],
    )
    .map_err(|e| e.to_string())?;
    settings(conn)
}

pub fn search(conn: &Connection, query: &str) -> Result<Vec<Conversation>, String> {
    let pattern = format!("%{}%", query.trim().replace('%', "\\%").replace('_', "\\_"));
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT c.id, c.project_id, c.title, c.output_type, c.updated_at, c.output_selection
         FROM conversations c LEFT JOIN messages m ON m.conversation_id = c.id
         WHERE c.title LIKE ?1 ESCAPE '\\' OR m.content LIKE ?1 ESCAPE '\\'
         ORDER BY c.updated_at DESC LIMIT 50",
        )
        .map_err(|e| e.to_string())?;
    let result = stmt
        .query_map([pattern], |row| {
            Ok(Conversation {
                id: row.get(0)?,
                project_id: row.get(1)?,
                title: row.get(2)?,
                output_type: row.get(3)?,
                updated_at: row.get(4)?,
                output_selection: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string());
    result
}

pub fn project_by_name(conn: &Connection, name: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT id FROM projects WHERE name = ?1 COLLATE NOCASE",
        [name],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_activity_survives_reopen_without_exposing_debug_payload() {
        let path = std::env::temp_dir().join(format!("bench-tools-{}.sqlite3", Uuid::new_v4()));
        let (conversation_id, message_id, first_id, second_id) = {
            let mut conn = open(&path).unwrap();
            let chat = create_user_message(&mut conn, None, "Weather this week", "chat").unwrap();
            let user = messages(&conn, &chat.id).unwrap().remove(0);
            let first = record_tool_execution_for_message(
                &conn,
                &chat.id,
                &user.id,
                &ToolExecutionRecord {
                    tool_name: "web_search",
                    status: "failed",
                    summary: "Web search unavailable",
                    detail_json: "{\"debugOnly\":\"private metadata\"}",
                },
            )
            .unwrap();
            let second = record_tool_execution_for_message(
                &conn,
                &chat.id,
                &user.id,
                &ToolExecutionRecord {
                    tool_name: "web_search",
                    status: "completed",
                    summary: "Searched the web",
                    detail_json: "{\"query\":\"Weather this week\"}",
                },
            )
            .unwrap();
            (chat.id, user.id, first, second)
        };
        let conn = open(&path).unwrap();
        let history = load_tool_activity(&conn, &conversation_id).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].id, first_id);
        assert_eq!(history[1].id, second_id);
        assert!(history
            .iter()
            .all(|activity| activity.user_message_id.as_deref() == Some(&message_id)));
        assert_eq!(history[1].summary, "Searched the web");
        let serialized = serde_json::to_value(&history).unwrap();
        assert!(serialized[0].get("detailJson").is_none());
        assert!(!serialized.to_string().contains("private metadata"));
        assert!(load_tool_activity(&conn, "another-conversation")
            .unwrap()
            .is_empty());
        drop(conn);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn tool_activity_rejects_missing_assistant_and_other_conversation_messages() {
        let mut conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        let first = create_user_message(&mut conn, None, "First", "chat").unwrap();
        save_assistant(&conn, &first.id, "Response").unwrap();
        let assistant = messages(&conn, &first.id)
            .unwrap()
            .into_iter()
            .find(|message| message.role == "assistant")
            .unwrap();
        let other = create_user_message(&mut conn, None, "Other", "chat").unwrap();
        let other_user = messages(&conn, &other.id).unwrap().remove(0);
        let record = ToolExecutionRecord {
            tool_name: "web_search",
            status: "completed",
            summary: "Searched the web",
            detail_json: "{}",
        };
        for invalid in ["missing", &assistant.id, &other_user.id] {
            assert!(record_tool_execution_for_message(&conn, &first.id, invalid, &record).is_err());
        }
        assert!(load_tool_activity(&conn, &first.id).unwrap().is_empty());
    }

    #[test]
    fn legacy_tool_activity_migrates_without_inventing_message_association() {
        let mut conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        let chat = create_user_message(&mut conn, None, "Existing request", "chat").unwrap();
        conn.execute_batch(
            "DROP TABLE tool_executions;
            CREATE TABLE tool_executions (
                id TEXT PRIMARY KEY, conversation_id TEXT NOT NULL REFERENCES conversations(id),
                tool_name TEXT NOT NULL, status TEXT NOT NULL, summary TEXT NOT NULL,
                detail_json TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );",
        )
        .unwrap();
        let id = record_tool_execution(
            &conn,
            &chat.id,
            "web_search",
            "completed",
            "Searched the web",
            "{}",
        )
        .unwrap();
        initialize(&conn).unwrap();
        initialize(&conn).unwrap();
        let history = load_tool_activity(&conn, &chat.id).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].id, id);
        assert_eq!(history[0].user_message_id, None);
        assert_eq!(messages(&conn, &chat.id).unwrap().len(), 1);
    }

    #[test]
    fn persists_message_and_settings() {
        let mut db = Connection::open_in_memory().unwrap();
        initialize(&db).unwrap();
        let chat = create_user_message(&mut db, None, "Hello Bench", "auto").unwrap();
        assert_eq!(chat.project_id, MISC_ID);
        assert_eq!(chat.output_type, "chat");
        assert_eq!(chat.output_selection, "auto");
        assert_eq!(messages(&db, &chat.id).unwrap().len(), 1);
        save_assistant(&db, &chat.id, "Hello").unwrap();
        assert_eq!(messages(&db, &chat.id).unwrap().len(), 2);
        assert_eq!(
            update_settings(&db, "discuss", "always")
                .unwrap()
                .approval_behavior,
            "always"
        );
    }

    #[test]
    fn survives_database_reopen() {
        let path = std::env::temp_dir().join(format!("bench-test-{}.sqlite3", Uuid::new_v4()));
        let chat_id = {
            let mut conn = open(&path).unwrap();
            let chat =
                create_user_message(&mut conn, None, "Draft a note for Sunday Capital", "auto")
                    .unwrap();
            update_settings(&conn, "just_do_it", "autonomous").unwrap();
            select_provider(&conn, crate::provider::ProviderId::Anthropic).unwrap();
            update_model(
                &conn,
                crate::provider::ProviderId::Anthropic,
                "claude-sonnet-5-5",
            )
            .unwrap();
            set_web_search(&conn, "off", None).unwrap();
            chat.id
        };
        let conn = open(&path).unwrap();
        assert_eq!(
            messages(&conn, &chat_id).unwrap()[0].content,
            "Draft a note for Sunday Capital"
        );
        assert_eq!(settings(&conn).unwrap().execution_behavior, "just_do_it");
        assert_eq!(settings(&conn).unwrap().model_provider, "anthropic");
        assert_eq!(settings(&conn).unwrap().web_search_backend, "off");
        assert_eq!(settings(&conn).unwrap().web_search_url, "");
        assert_eq!(
            model(&conn, crate::provider::ProviderId::Anthropic).unwrap(),
            "claude-sonnet-5-5"
        );
        assert_eq!(projects(&conn).unwrap().len(), 2);
        drop(conn);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn web_search_selection_is_validated() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        assert!(set_web_search(&conn, "unknown", None).is_err());
        assert_eq!(settings(&conn).unwrap().web_search_backend, "auto");
        set_web_search(&conn, "off", None).unwrap();
        assert_eq!(settings(&conn).unwrap().web_search_backend, "off");
        assert!(set_web_search(&conn, "brave", None).is_err());
    }

    #[test]
    fn legacy_search_selection_migrates_without_erasing_url() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn.execute(
            "UPDATE settings SET value = 'searxng' WHERE key = 'web_search_backend'",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE settings SET value = 'https://search.example/search' WHERE key = 'web_search_url'",
            [],
        ).unwrap();
        initialize(&conn).unwrap();
        let selected = settings(&conn).unwrap();
        assert_eq!(selected.web_search_backend, "auto");
        assert_eq!(selected.web_search_url, "https://search.example/search");
    }

    #[test]
    fn confident_match_and_manual_move_survive_reopen() {
        let path = std::env::temp_dir().join(format!("bench-test-{}.sqlite3", Uuid::new_v4()));
        let (chat_id, project_id) = {
            let mut conn = open(&path).unwrap();
            let project = create_project(&conn, "Laundros").unwrap();
            let chat = create_user_message(&mut conn, None, "How should I structure this?", "auto")
                .unwrap();
            assert_eq!(chat.project_id, MISC_ID);
            let chat = create_user_message(&mut conn, Some(&chat.id), "It is for Laundros", "auto")
                .unwrap();
            assert_eq!(chat.project_id, project.id);
            move_conversation(&conn, &chat.id, MISC_ID).unwrap();
            create_user_message(&mut conn, Some(&chat.id), "More about Laundros", "auto").unwrap();
            assert_eq!(conversation(&conn, &chat.id).unwrap().project_id, MISC_ID);
            set_sidebar_collapsed(&conn, true).unwrap();
            (chat.id, project.id)
        };
        let conn = open(&path).unwrap();
        assert_eq!(conversation(&conn, &chat_id).unwrap().project_id, MISC_ID);
        assert!(settings(&conn).unwrap().sidebar_collapsed);
        assert!(projects(&conn)
            .unwrap()
            .iter()
            .any(|project| project.id == project_id));
        drop(conn);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn output_override_and_migration() {
        let mut conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        let inferred = create_user_message(
            &mut conn,
            None,
            "Build a simple website for Laundros",
            "auto",
        )
        .unwrap();
        assert_eq!(inferred.output_type, "website");
        let forced = create_user_message(&mut conn, None, "Build a website", "chat").unwrap();
        assert_eq!(forced.output_type, "chat");
        assert_eq!(forced.output_selection, "chat");
        let routed = create_user_message(
            &mut conn,
            Some(&forced.id),
            "Build a website for Laundros",
            "auto",
        )
        .unwrap();
        assert_eq!(routed.output_type, "website");
        let revised = create_user_message(
            &mut conn,
            Some(&forced.id),
            "Make the header smaller",
            "auto",
        )
        .unwrap();
        assert_eq!(revised.output_type, "website");
        conn.execute(
            "UPDATE conversations SET output_type = 'auto' WHERE id = ?1",
            [&forced.id],
        )
        .unwrap();
        initialize(&conn).unwrap();
        assert_eq!(conversation(&conn, &forced.id).unwrap().output_type, "chat");
    }

    #[test]
    fn failed_write_keeps_conversation_atomic() {
        let mut conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn.execute_batch("CREATE TRIGGER fail_messages BEFORE INSERT ON messages BEGIN SELECT RAISE(FAIL, 'disk write blocked'); END;").unwrap();
        assert!(create_user_message(&mut conn, None, "Keep this prompt", "chat").is_err());
        assert!(conversations(&conn).unwrap().is_empty());
        conn.execute_batch("DROP TRIGGER fail_messages").unwrap();
        let chat = create_user_message(&mut conn, None, "Keep this prompt", "chat").unwrap();
        conn.execute_batch("CREATE TRIGGER fail_assistant BEFORE INSERT ON messages WHEN NEW.role = 'assistant' BEGIN SELECT RAISE(FAIL, 'disk write blocked'); END;").unwrap();
        assert!(save_assistant(&conn, &chat.id, "Partial response").is_err());
        assert_eq!(messages(&conn, &chat.id).unwrap().len(), 1);
    }

    #[test]
    fn migrates_existing_conversations_and_records_tool_metadata() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE projects (id TEXT PRIMARY KEY, name TEXT NOT NULL UNIQUE, is_system INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
             CREATE TABLE conversations (id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id), title TEXT NOT NULL, output_type TEXT NOT NULL DEFAULT 'auto', created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
             INSERT INTO projects(id, name, is_system) VALUES ('miscellaneous', 'Miscellaneous', 1);
             INSERT INTO conversations(id, project_id, title) VALUES ('old-chat', 'miscellaneous', 'Older chat');"
        ).unwrap();
        initialize(&conn).unwrap();
        let chat = conversation(&conn, "old-chat").unwrap();
        assert_eq!(chat.output_type, "chat");
        assert_eq!(chat.output_selection, "auto");
        let id = record_tool_execution(
            &conn,
            &chat.id,
            "web_search",
            "completed",
            "Searched the web",
            "{\"query\":\"weather\"}",
        )
        .unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tool_executions WHERE id = ?1 AND conversation_id = ?2",
                params![id, chat.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        create_user_message(&mut conn, Some(&chat.id), "Follow up", "auto").unwrap();
    }
}
