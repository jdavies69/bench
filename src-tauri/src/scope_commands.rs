//! Tauri commands for project scope. `reconcile_scope` is the only one that
//! contacts a provider, and its reply can file suggestions and nothing else.
use std::collections::HashMap;

use rusqlite::Connection;
use tauri::{AppHandle, Manager, State};

use crate::{
    artifact::{self, ArtifactKind},
    artifact_commands,
    commands::{AppState, ConversationOperation},
    db, key_store,
    provider::{HttpProvider, ProviderId},
    scope::{self, ItemUpdate, OutputEvidence, ScopeView},
    website,
};

fn database<'a>(
    state: &'a State<'_, AppState>,
) -> Result<std::sync::MutexGuard<'a, Connection>, String> {
    state
        .db
        .lock()
        .map_err(|_| "Local database is unavailable.".into())
}

#[tauri::command]
pub fn load_scope(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Option<ScopeView>, String> {
    scope::load(&*database(&state)?, &project_id)
}

#[tauri::command]
pub fn set_scope_outcome(
    state: State<'_, AppState>,
    project_id: String,
    outcome: String,
) -> Result<ScopeView, String> {
    scope::set_outcome(&*database(&state)?, &project_id, &outcome)
}

#[tauri::command]
pub fn set_scope_auto_update(
    state: State<'_, AppState>,
    project_id: String,
    enabled: bool,
) -> Result<ScopeView, String> {
    scope::set_auto_update(&*database(&state)?, &project_id, enabled)
}

#[tauri::command]
pub fn add_scope_item(
    state: State<'_, AppState>,
    project_id: String,
    title: String,
    bucket: String,
) -> Result<ScopeView, String> {
    scope::add_item(&*database(&state)?, &project_id, &title, &bucket)
}

#[tauri::command]
pub fn update_scope_item(
    state: State<'_, AppState>,
    item_id: String,
    title: Option<String>,
    done_when: Option<String>,
    status: Option<String>,
    bucket: Option<String>,
) -> Result<ScopeView, String> {
    scope::update_item(
        &*database(&state)?,
        &item_id,
        &ItemUpdate {
            title,
            done_when,
            status,
            bucket,
        },
    )
}

#[tauri::command]
pub fn resolve_scope_change(
    state: State<'_, AppState>,
    change_id: String,
    decision: String,
) -> Result<ScopeView, String> {
    scope::resolve_change(&*database(&state)?, &change_id, &decision)
}

#[tauri::command]
pub fn scope_session_prompt(state: State<'_, AppState>, item_id: String) -> Result<String, String> {
    scope::session_prompt(&*database(&state)?, &item_id)
}

fn artifact_kind(output_type: &str) -> Option<ArtifactKind> {
    Some(match output_type {
        "agent" => ArtifactKind::Agent,
        "application" => ArtifactKind::Application,
        "document" => ArtifactKind::Document,
        "presentation" => ArtifactKind::Presentation,
        "image" => ArtifactKind::Image,
        "voice" => ArtifactKind::Voice,
        _ => return None,
    })
}

/// Reads what Bench itself saved for each active output. A missing or
/// unreadable output is simply absent evidence; it never fails the update.
fn saved_outputs(app: &AppHandle, active: &[(String, String)]) -> HashMap<String, OutputEvidence> {
    let website_root = app
        .path()
        .app_data_dir()
        .ok()
        .map(|path| path.join("website-workspaces"));
    let artifact_root = artifact_commands::root(app).ok();
    let mut found = HashMap::new();
    for (conversation_id, output_type) in active {
        let evidence = if output_type == "website" {
            website_root
                .as_ref()
                .and_then(|root| website::load(root, conversation_id).ok().flatten())
                .map(|site| OutputEvidence {
                    revision: site.revision,
                    request_count: site.request_count,
                })
        } else {
            artifact_root
                .as_ref()
                .zip(artifact_kind(output_type))
                .and_then(|(root, kind)| {
                    artifact::load(
                        root,
                        conversation_id,
                        kind,
                        artifact_commands::validate_content,
                    )
                    .ok()
                    .flatten()
                })
                .map(|saved| OutputEvidence {
                    revision: saved.revision,
                    request_count: saved.request_count,
                })
        };
        if let Some(evidence) = evidence {
            found.insert(conversation_id.clone(), evidence);
        }
    }
    found
}

/// Reviews new project activity against the scope. Without `force` this is the
/// automatic loop: it makes no request when the project has it turned off,
/// when nothing changed, or when earlier suggestions still wait for review.
#[tauri::command]
pub async fn reconcile_scope(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    force: bool,
) -> Result<ScopeView, String> {
    let _operation =
        ConversationOperation::begin(&state.active_operations, &format!("scope:{project_id}"))
            .map_err(|_| "Scope is already updating.".to_owned())?;
    let active = scope::active_outputs(&*database(&state)?, &project_id)?;
    let evidence = saved_outputs(&app, &active);
    let (pending, provider, model) = {
        let conn = database(&state)?;
        let Some(pending) = scope::prepare(&conn, &project_id, force, &evidence)? else {
            return scope::load(&conn, &project_id)?
                .ok_or_else(|| "Set an outcome for this project first.".into());
        };
        let provider = ProviderId::parse(&db::settings(&conn)?.model_provider)?;
        let model = db::model(&conn, provider)?;
        (pending, provider, model)
    };
    let proposals = match key_store::active_key(provider) {
        Ok(key) => {
            let provider = HttpProvider::new(provider, key, model);
            scope::propose(&provider, pending.messages.clone()).await
        }
        Err(error) => Err(error),
    };
    let conn = database(&state)?;
    match proposals {
        Ok(proposals) => {
            scope::apply(&conn, &pending, proposals)?;
        }
        Err(error) => {
            scope::mark_failed(&conn, &project_id)?;
            return Err(error);
        }
    }
    scope::load(&conn, &project_id)?.ok_or_else(|| "Set an outcome for this project first.".into())
}
