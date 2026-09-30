use crate::{
    agent,
    artifact::{self, ArtifactKind, ArtifactState},
    artifact_commands,
    commands::{AppState, ConversationOperation},
    db, key_store,
    provider::{HttpProvider, ProviderId, ProviderMessage},
};
use tauri::{AppHandle, State};
#[tauri::command]
pub async fn generate_agent_output(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    approval_token: Option<String>,
) -> Result<ArtifactState, String> {
    let _guard = ConversationOperation::begin(&state.active_operations, &conversation_id)?;
    let root = artifact_commands::root(&app)?;
    let (policy, action, provider, model, context, count) = {
        let conn = state
            .db
            .lock()
            .map_err(|_| "Local database is unavailable.")?;
        if artifact_commands::kind(&conn, &conversation_id)? != ArtifactKind::Agent {
            return Err("This conversation is not an Agent task.".into());
        }
        let (policy, action) = artifact_commands::action(
            &conn,
            &root,
            &conversation_id,
            ArtifactKind::Agent,
            "generate",
            None,
            None,
            agent::validate,
        )?;
        let provider = ProviderId::parse(&db::settings(&conn)?.model_provider)?;
        let saved = db::messages(&conn, &conversation_id)?;
        let count = saved.iter().filter(|m| m.role == "user").count();
        let context = saved
            .into_iter()
            .filter(|m| matches!(m.role.as_str(), "user" | "assistant"))
            .map(|m| ProviderMessage {
                role: m.role,
                content: m.content,
            })
            .collect::<Vec<_>>();
        (
            policy,
            action,
            provider,
            db::model(&conn, provider)?,
            context,
            count,
        )
    };
    crate::commands::require_authorization(
        &mut *state
            .approvals
            .lock()
            .map_err(|_| "Action approval is unavailable.")?,
        &policy,
        &action,
        approval_token.as_deref(),
    )?;
    let previous = artifact::load(
        &root,
        &conversation_id,
        ArtifactKind::Agent,
        agent::validate,
    )?;
    let provider = HttpProvider::for_website(provider, key_store::active_key(provider)?, model);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(300),
        agent::generate(&provider, previous.as_ref(), &context, count),
    )
    .await
    .map_err(|_| "Agent reached its five-minute limit. Your saved report is unchanged.")??;
    {
        let conn = state
            .db
            .lock()
            .map_err(|_| "Local database is unavailable.")?;
        let (_, fresh) = artifact_commands::action(
            &conn,
            &root,
            &conversation_id,
            ArtifactKind::Agent,
            "generate",
            None,
            None,
            agent::validate,
        )?;
        if fresh.binding != action.binding {
            return Err("Agent task changed. Review the latest request before saving.".into());
        }
    }
    artifact::save(&root, &conversation_id, &result, agent::validate)
}
