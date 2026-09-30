//! Native media generation; artifact load/history/restore/export use the shared
//! artifact commands. Exact Rust action grants apply before any provider call.
use rusqlite::Connection;
use tauri::{AppHandle, State};

use crate::{
    artifact::{self, ArtifactKind, ArtifactState},
    artifact_commands,
    commands::{AppState, ConversationOperation},
    db, key_store,
    media::{self, OpenRouterMedia},
    policy::Decision,
    provider::{HttpProvider, ProviderId},
};

fn context(conn: &Connection, id: &str) -> Result<(ArtifactKind, Vec<String>, String), String> {
    let conversation = db::conversation(conn, id)?;
    let kind = match conversation.output_type.as_str() {
        "image" => ArtifactKind::Image,
        "voice" => ArtifactKind::Voice,
        _ => return Err("This output does not support media generation.".into()),
    };
    if db::settings(conn)?.model_provider != "openrouter" {
        return Err("Select OpenRouter in Settings to create Image and Voice outputs.".into());
    }
    let requests = db::model_messages(conn, id)?
        .into_iter()
        .filter(|message| message.role == "user")
        .map(|message| message.content)
        .collect();
    Ok((kind, requests, db::model(conn, ProviderId::OpenRouter)?))
}

#[tauri::command]
pub async fn generate_media_output(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    approval_token: Option<String>,
) -> Result<ArtifactState, String> {
    let _operation = ConversationOperation::begin(&state.active_operations, &conversation_id)?;
    let root = artifact_commands::root(&app)?;
    let (kind, requests, model, policy, action) = {
        let conn = state
            .db
            .lock()
            .map_err(|_| "Local workspace is unavailable.")?;
        let (kind, requests, model) = context(&conn, &conversation_id)?;
        let (policy, action) = artifact_commands::action(
            &conn,
            &root,
            &conversation_id,
            kind,
            "generate",
            None,
            None,
            media::validate,
        )?;
        (kind, requests, model, policy, action)
    };
    let previous = artifact::load(&root, &conversation_id, kind, media::validate)?;
    if previous
        .as_ref()
        .is_some_and(|state| state.request_count >= requests.len())
    {
        return Err("This output is already up to date. Send a new request to revise it.".into());
    }
    {
        let mut approvals = state
            .approvals
            .lock()
            .map_err(|_| "Action approval is unavailable.")?;
        if approvals.authorize(&policy, &action, approval_token.as_deref())? != Decision::Allow {
            return Err("Review this media action before continuing.".into());
        }
    }
    let key = key_store::active_key(ProviderId::OpenRouter)?;
    let images = crate::attachments::conversation_images(
        &*state
            .db
            .lock()
            .map_err(|_| "Local database is unavailable.")?,
        &conversation_id,
    )?;
    let media_provider = OpenRouterMedia::new(key.clone())?.with_images(images.clone());
    let text_provider =
        HttpProvider::new(ProviderId::OpenRouter, key, model).with_images(images)?;
    let next = media::generate(
        &media_provider,
        &text_provider,
        kind,
        previous.as_ref(),
        &requests,
    )
    .await?;
    artifact::save(&root, &conversation_id, &next, media::validate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_context_uses_persisted_output_requests_and_openrouter_text_model() {
        let mut conn = Connection::open_in_memory().unwrap();
        db::initialize(&conn).unwrap();
        db::update_model(&conn, ProviderId::OpenRouter, "configured/text-model").unwrap();
        for (output, kind) in [
            ("image", ArtifactKind::Image),
            ("voice", ArtifactKind::Voice),
        ] {
            let conversation =
                db::create_user_message(&mut conn, None, "Create a welcome", output).unwrap();
            let (actual_kind, requests, model) = context(&conn, &conversation.id).unwrap();
            assert_eq!(actual_kind, kind);
            assert_eq!(requests, ["Create a welcome"]);
            assert_eq!(model, "configured/text-model");
            db::select_provider(&conn, ProviderId::OpenAI).unwrap();
            assert!(context(&conn, &conversation.id)
                .unwrap_err()
                .contains("Select OpenRouter"));
            db::select_provider(&conn, ProviderId::OpenRouter).unwrap();
        }
        let conversation = db::create_user_message(&mut conn, None, "Hello", "chat").unwrap();
        assert!(context(&conn, &conversation.id).is_err());
    }
}
