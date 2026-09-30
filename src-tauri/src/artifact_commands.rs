use crate::{
    artifact::{self, ArtifactKind, ArtifactRevision, ArtifactState, ContentValidator},
    commands::{ActionReview, AppState, ConversationOperation},
    db, key_store,
    policy::{Action, ActionBinding, ActionCategory, ActionOrigin, Decision, Policy},
    provider::{HttpProvider, ProviderId},
    text_outputs,
};
use rusqlite::Connection;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

pub(crate) fn root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|path| path.join("output-workspaces"))
        .map_err(|_| "Output workspace is unavailable.".into())
}
pub(crate) fn kind(conn: &Connection, id: &str) -> Result<ArtifactKind, String> {
    match db::conversation(conn, id)?.output_type.as_str() {
        "agent" => Ok(ArtifactKind::Agent),
        "application" => Ok(ArtifactKind::Application),
        "document" => Ok(ArtifactKind::Document),
        "presentation" => Ok(ArtifactKind::Presentation),
        "image" => Ok(ArtifactKind::Image),
        "voice" => Ok(ArtifactKind::Voice),
        _ => Err("This output does not support that action.".into()),
    }
}
pub(crate) fn validate_content(kind: ArtifactKind, content: &Value) -> Result<(), String> {
    match kind {
        ArtifactKind::Agent => crate::agent::validate(kind, content),
        ArtifactKind::Application => crate::application::validate(kind, content),
        ArtifactKind::Document | ArtifactKind::Presentation => {
            text_outputs::validate(kind, content)
        }
        ArtifactKind::Image | ArtifactKind::Voice => crate::media::validate(kind, content),
    }
}
fn text_kind(conn: &Connection, id: &str) -> Result<ArtifactKind, String> {
    let kind = kind(conn, id)?;
    if !matches!(
        kind,
        ArtifactKind::Document | ArtifactKind::Presentation | ArtifactKind::Application
    ) {
        return Err("This output does not support a text action.".into());
    }
    Ok(kind)
}
// The arguments are the exact operation fields reconstructed at authorization.
#[allow(clippy::too_many_arguments)]
pub(crate) fn action(
    conn: &Connection,
    root: &Path,
    id: &str,
    kind: ArtifactKind,
    operation: &str,
    target: Option<u32>,
    content: Option<&Value>,
    validator: ContentValidator,
) -> Result<(Policy, Action), String> {
    if db::conversation(conn, id)?.output_type != kind.as_str() {
        return Err("This output does not support that action.".into());
    }
    let settings = db::settings(conn)?;
    let policy = Policy::from_settings(&settings.execution_behavior, &settings.approval_behavior)?;
    let message = db::messages(conn, id)?
        .into_iter()
        .rev()
        .find(|message| message.role == "user")
        .ok_or("Save a request before creating an output.")?;
    let base = artifact::load(root, id, kind, validator)?.map_or(0, |state| state.revision);
    let operation = match (operation, target, content) {
        ("generate", None, None) => format!("artifact:{}:generate:{base}", kind.as_str()),
        ("restore", Some(target), None) => {
            if !artifact::list_revisions(root, id, kind, validator)?
                .iter()
                .any(|revision| revision.revision == target)
            {
                return Err("This output version is unavailable.".into());
            }
            format!("artifact:{}:restore:{target}:{base}", kind.as_str())
        }
        ("edit", None, Some(content)) => {
            if base == 0 {
                return Err("Create this output before editing it.".into());
            }
            validator(kind, content)?;
            let bytes = serde_json::to_vec(content).map_err(|_| "Output edit is unreadable.")?;
            let digest = format!("{:x}", Sha256::digest(bytes));
            format!("artifact:{}:edit:{base}:{digest}", kind.as_str())
        }
        _ => return Err("This output action is unavailable.".into()),
    };
    Ok((
        policy,
        Action {
            binding: ActionBinding {
                conversation_id: id.into(),
                message_id: message.id,
                operation,
            },
            category: ActionCategory::ReversibleLocalWrite,
            origin: ActionOrigin::UserRequested,
        },
    ))
}

pub(crate) fn reconstruct_binding(
    conn: &Connection,
    root: &Path,
    binding: &ActionBinding,
    validator: ContentValidator,
) -> Result<ActionBinding, String> {
    let selected = kind(conn, &binding.conversation_id)?;
    let pieces = binding.operation.split(':').collect::<Vec<_>>();
    match pieces.as_slice() {
        ["artifact", kind, "generate", _] if *kind == selected.as_str() => Ok(action(
            conn,
            root,
            &binding.conversation_id,
            selected,
            "generate",
            None,
            None,
            validator,
        )?
        .1
        .binding),
        ["artifact", kind, "restore", target, _] if *kind == selected.as_str() => Ok(action(
            conn,
            root,
            &binding.conversation_id,
            selected,
            "restore",
            Some(
                target
                    .parse()
                    .map_err(|_| "This approval is unavailable.")?,
            ),
            None,
            validator,
        )?
        .1
        .binding),
        ["artifact", kind, "edit", base, digest] if *kind == selected.as_str() => {
            if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err("This approval is unavailable.".into());
            }
            let current = action(
                conn,
                root,
                &binding.conversation_id,
                selected,
                "generate",
                None,
                None,
                validator,
            )?
            .1
            .binding;
            let revision = artifact::load(root, &binding.conversation_id, selected, validator)?
                .ok_or("Output workspace could not be found.")?
                .revision;
            if base.parse::<u32>().ok() != Some(revision) {
                return Err("The output changed. Review the action again.".into());
            }
            Ok(ActionBinding {
                operation: format!("artifact:{kind}:edit:{revision}:{digest}"),
                ..current
            })
        }
        _ => Err("This approval is unavailable.".into()),
    }
}

#[tauri::command]
pub fn load_artifact(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Option<ArtifactState>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "Local database is unavailable.")?;
    let selected = match kind(&conn, &conversation_id) {
        Ok(kind) => kind,
        Err(_) => return Ok(None),
    };
    artifact::load(&root(&app)?, &conversation_id, selected, validate_content)
}
#[tauri::command]
pub fn list_artifact_revisions(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<ArtifactRevision>, String> {
    let selected = kind(
        &*state
            .db
            .lock()
            .map_err(|_| "Local database is unavailable.")?,
        &conversation_id,
    )?;
    artifact::list_revisions(&root(&app)?, &conversation_id, selected, validate_content)
}
#[tauri::command]
pub fn prepare_artifact_action(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    operation: String,
    target_revision: Option<u32>,
    content: Option<Value>,
) -> Result<Option<ActionReview>, String> {
    let (policy, action) = {
        let conn = state
            .db
            .lock()
            .map_err(|_| "Local database is unavailable.")?;
        action(
            &conn,
            &root(&app)?,
            &conversation_id,
            kind(&conn, &conversation_id)?,
            &operation,
            target_revision,
            content.as_ref(),
            validate_content,
        )?
    };
    if policy.evaluate(&action) == Decision::Allow {
        return Ok(None);
    }
    let is_agent = action.binding.operation.starts_with("artifact:agent:");
    let review = state
        .approvals
        .lock()
        .map_err(|_| "Action approval is unavailable.")?
        .register(action)?;
    let title = match operation.as_str() {
        "restore" => format!("Restore version {}?", target_revision.unwrap()),
        "edit" => "Save these edits?".into(),
        _ => "Create or revise this output?".into(),
    };
    Ok(Some(ActionReview {
        id: review.id,
        conversation_id,
        title,
        detail: if is_agent {
            "Up to four model rounds may read this saved conversation, stage and inspect a local report, then save its final version. No web research, shell commands or external changes. This bounds calls, not a dollar cost.".into()
        } else {
            "Your current version and saved request will be kept.".into()
        },
    }))
}
#[tauri::command]
pub async fn generate_artifact(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    approval_token: Option<String>,
) -> Result<ArtifactState, String> {
    let _operation = ConversationOperation::begin(&state.active_operations, &conversation_id)?;
    let root = root(&app)?;
    let (selected, requests, provider, model, policy, action) = {
        let conn = state
            .db
            .lock()
            .map_err(|_| "Local database is unavailable.")?;
        let selected = text_kind(&conn, &conversation_id)?;
        let (policy, action) = action(
            &conn,
            &root,
            &conversation_id,
            selected,
            "generate",
            None,
            None,
            text_outputs::validate,
        )?;
        let provider = ProviderId::parse(&db::settings(&conn)?.model_provider)?;
        let requests = db::messages(&conn, &conversation_id)?
            .into_iter()
            .filter(|message| message.role == "user")
            .map(|message| message.content)
            .collect::<Vec<_>>();
        (
            selected,
            requests,
            provider,
            db::model(&conn, provider)?,
            policy,
            action,
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
    let previous = artifact::load(&root, &conversation_id, selected, text_outputs::validate)?;
    if previous
        .as_ref()
        .is_some_and(|state| state.request_count >= requests.len())
    {
        return Err("This output is already up to date. Send a new request to revise it.".into());
    }
    let provider = HttpProvider::for_website(provider, key_store::active_key(provider)?, model);
    let result = text_outputs::generate(&provider, selected, previous.as_ref(), &requests).await?;
    artifact::save(&root, &conversation_id, &result, text_outputs::validate)
}
#[tauri::command]
pub fn restore_artifact_revision(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    revision: u32,
    approval_token: Option<String>,
) -> Result<ArtifactState, String> {
    let _operation = ConversationOperation::begin(&state.active_operations, &conversation_id)?;
    let root = root(&app)?;
    let (selected, policy, action) = {
        let conn = state
            .db
            .lock()
            .map_err(|_| "Local database is unavailable.")?;
        let selected = kind(&conn, &conversation_id)?;
        let (policy, action) = action(
            &conn,
            &root,
            &conversation_id,
            selected,
            "restore",
            Some(revision),
            None,
            validate_content,
        )?;
        (selected, policy, action)
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
    artifact::restore_revision(
        &root,
        &conversation_id,
        selected,
        revision,
        validate_content,
    )
}
#[tauri::command]
pub fn save_artifact_edits(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    content: Value,
    approval_token: Option<String>,
) -> Result<ArtifactState, String> {
    let _operation = ConversationOperation::begin(&state.active_operations, &conversation_id)?;
    let root = root(&app)?;
    let (selected, policy, action) = {
        let conn = state
            .db
            .lock()
            .map_err(|_| "Local database is unavailable.")?;
        let selected = text_kind(&conn, &conversation_id)?;
        let (policy, action) = action(
            &conn,
            &root,
            &conversation_id,
            selected,
            "edit",
            None,
            Some(&content),
            text_outputs::validate,
        )?;
        (selected, policy, action)
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
    let mut current = artifact::load(&root, &conversation_id, selected, text_outputs::validate)?
        .ok_or("Output workspace could not be found.")?;
    current.content = content;
    artifact::save(&root, &conversation_id, &current, text_outputs::validate)
}

pub(crate) fn write_selected(path: &Path, bytes: &[u8], overwrite: bool) -> Result<(), String> {
    if bytes.len() > 30 * 1024 * 1024 {
        return Err("Export is too large.".into());
    }
    let parent = path
        .parent()
        .ok_or("Choose a file in an existing folder.")?;
    for ancestor in parent.ancestors() {
        let metadata =
            fs::symlink_metadata(ancestor).map_err(|_| "Choose an existing export folder.")?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err("Choose an export folder without symbolic links.".into());
        }
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err("Choose a regular file destination, not a symbolic link.".into())
        }
        Ok(_) if !overwrite => {
            return Err("That export file already exists. Choose another name.".into())
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("Could not inspect the export destination.".into()),
    }
    let staging = parent.join(format!(".bench-export-{}", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
            .map_err(|_| "Could not save this export.")?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| "Could not save this export.")?;
        if overwrite {
            fs::rename(&staging, path).map_err(|_| "Could not replace this export.")?;
        } else {
            fs::hard_link(&staging, path)
                .map_err(|_| "Export destination changed. Choose another file.")?;
            fs::remove_file(&staging).map_err(|_| "Could not finish saving the export.")?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(staging);
    }
    result
}

#[tauri::command]
pub async fn export_artifact(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    format: String,
) -> Result<bool, String> {
    let _operation = ConversationOperation::begin(&state.active_operations, &conversation_id)?;
    let (selected, policy) = {
        let conn = state
            .db
            .lock()
            .map_err(|_| "Local database is unavailable.")?;
        let settings = db::settings(&conn)?;
        (
            kind(&conn, &conversation_id)?,
            Policy::from_settings(&settings.execution_behavior, &settings.approval_behavior)?,
        )
    };
    let current = artifact::load(&root(&app)?, &conversation_id, selected, validate_content)?
        .ok_or("Create this output before exporting it.")?;
    let (extension, bytes) = match (selected, format.as_str()) {
        (ArtifactKind::Image | ArtifactKind::Voice, "binary") => {
            crate::media::export_bytes(&current)?
        }
        (ArtifactKind::Document | ArtifactKind::Presentation, "text") => (
            if selected == ArtifactKind::Document {
                "md".to_owned()
            } else {
                "json".to_owned()
            },
            text_outputs::export_text(&current)?.into_bytes(),
        ),
        (ArtifactKind::Agent, "text") => (
            "md".to_owned(),
            crate::agent::export_text(&current.content)?.into_bytes(),
        ),
        (ArtifactKind::Agent, "html") => (
            "html".to_owned(),
            crate::agent::export_html(&current.content)?.into_bytes(),
        ),
        (ArtifactKind::Application, "html") => (
            "html".to_owned(),
            crate::application::export_html(&current.content)?.into_bytes(),
        ),
        (ArtifactKind::Document | ArtifactKind::Presentation, "html") => (
            "html".to_owned(),
            text_outputs::export_html(&current)?.into_bytes(),
        ),
        _ => return Err("Choose a supported export format.".into()),
    };
    native_export(
        &app,
        &policy,
        &conversation_id,
        &current,
        &extension,
        &bytes,
    )
    .await
}
pub(crate) async fn native_export(
    app: &AppHandle,
    policy: &Policy,
    conversation_id: &str,
    current: &ArtifactState,
    extension: &str,
    bytes: &[u8],
) -> Result<bool, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Export output")
        .set_file_name(format!("Bench-{}.{}", current.kind.as_str(), extension))
        .add_filter("Output", &[extension])
        .save_file(move |path| {
            let _ = tx.send(path);
        });
    let Some(destination) = rx.await.map_err(|_| "Export dialog was interrupted.")? else {
        return Ok(false);
    };
    let path = destination
        .into_path()
        .map_err(|_| "Choose a local export file.")?;
    let action = Action {
        binding: ActionBinding {
            conversation_id: conversation_id.into(),
            message_id: String::new(),
            operation: format!(
                "artifact:export:{}:{}:{}",
                current.kind.as_str(),
                current.revision,
                path.to_string_lossy()
            ),
        },
        category: if path.exists() {
            ActionCategory::Destructive
        } else {
            ActionCategory::ReversibleLocalWrite
        },
        origin: ActionOrigin::UserRequested,
    };
    let overwrite = fs::symlink_metadata(&path).is_ok();
    // Selecting a destination is an explicit native UI action. When policy
    // requires review (and for every replacement), require exact confirmation.
    if overwrite || policy.evaluate(&action) == Decision::ReviewRequired {
        let (tx, rx) = tokio::sync::oneshot::channel();
        app.dialog()
            .message(format!(
                "{} version {} to {}?",
                if overwrite { "Replace with" } else { "Export" },
                current.revision,
                path.display()
            ))
            .title("Confirm export")
            .buttons(MessageDialogButtons::OkCancel)
            .show(move |accepted| {
                let _ = tx.send(accepted);
            });
        if !rx
            .await
            .map_err(|_| "Export confirmation was interrupted.")?
        {
            return Ok(false);
        }
    }
    write_selected(&path, bytes, overwrite)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::{ApprovalStore, Decision};

    fn fixture() -> (Connection, db::Conversation, PathBuf) {
        let mut conn = Connection::open_in_memory().unwrap();
        db::initialize(&conn).unwrap();
        let conversation =
            db::create_user_message(&mut conn, None, "Draft a brief", "document").unwrap();
        db::update_settings(&conn, "discuss", "always").unwrap();
        let root =
            std::env::temp_dir().join(format!("bench-artifact-action-{}", uuid::Uuid::new_v4()));
        (conn, conversation, root)
    }
    fn saved(root: &Path, id: &str, count: usize) -> ArtifactState {
        artifact::save(
            root,
            id,
            &ArtifactState {
                version: 1,
                kind: ArtifactKind::Document,
                revision: 1,
                request_count: count,
                content: serde_json::json!({"title":"Draft","markdown":"Saved content"}),
            },
            text_outputs::validate,
        )
        .unwrap()
    }

    #[test]
    fn agent_grant_is_exact_one_use_and_stale_saved_request_is_rejected() {
        let (mut conn, _, root) = fixture();
        let task =
            db::create_user_message(&mut conn, None, "Synthesize this conversation", "agent")
                .unwrap();
        let (policy, first) = action(
            &conn,
            &root,
            &task.id,
            ArtifactKind::Agent,
            "generate",
            None,
            None,
            crate::agent::validate,
        )
        .unwrap();
        let mut approvals = ApprovalStore::default();
        assert_eq!(
            approvals.authorize(&policy, &first, None).unwrap(),
            Decision::ReviewRequired
        );
        let review = approvals.register(first.clone()).unwrap();
        let token = approvals.approve(&review.id, &first.binding).unwrap();
        assert_eq!(
            approvals.authorize(&policy, &first, Some(&token)).unwrap(),
            Decision::Allow
        );
        assert!(approvals.authorize(&policy, &first, Some(&token)).is_err());
        let review = approvals.register(first.clone()).unwrap();
        db::create_user_message(&mut conn, Some(&task.id), "Use the updated facts", "agent")
            .unwrap();
        let (_, changed) = action(
            &conn,
            &root,
            &task.id,
            ArtifactKind::Agent,
            "generate",
            None,
            None,
            crate::agent::validate,
        )
        .unwrap();
        assert_ne!(changed.binding, first.binding);
        assert!(approvals.approve(&review.id, &changed.binding).is_err());
    }

    #[test]
    fn policy_binds_saved_request_output_and_revision_and_grants_are_one_use() {
        let (mut conn, conversation, root) = fixture();
        let mut approvals = ApprovalStore::default();
        let (policy, first) = action(
            &conn,
            &root,
            &conversation.id,
            ArtifactKind::Document,
            "generate",
            None,
            None,
            text_outputs::validate,
        )
        .unwrap();
        assert_eq!(
            approvals.authorize(&policy, &first, None).unwrap(),
            Decision::ReviewRequired
        );
        assert!(action(
            &conn,
            &root,
            &conversation.id,
            ArtifactKind::Presentation,
            "generate",
            None,
            None,
            text_outputs::validate
        )
        .is_err());
        let review = approvals.register(first.clone()).unwrap();
        let binding =
            reconstruct_binding(&conn, &root, &review.binding, text_outputs::validate).unwrap();
        let token = approvals.approve(&review.id, &binding).unwrap();
        assert!(crate::commands::require_authorization(
            &mut approvals,
            &policy,
            &first,
            Some(&token)
        )
        .is_ok());
        assert!(crate::commands::require_authorization(
            &mut approvals,
            &policy,
            &first,
            Some(&token)
        )
        .is_err());
        let review = approvals.register(first.clone()).unwrap();
        saved(&root, &conversation.id, 1);
        let changed =
            reconstruct_binding(&conn, &root, &review.binding, text_outputs::validate).unwrap();
        assert!(approvals.approve(&review.id, &changed).is_err());
        let (_, second) = action(
            &conn,
            &root,
            &conversation.id,
            ArtifactKind::Document,
            "generate",
            None,
            None,
            text_outputs::validate,
        )
        .unwrap();
        let review = approvals.register(second).unwrap();
        db::create_user_message(&mut conn, Some(&conversation.id), "Revise it", "document")
            .unwrap();
        let changed =
            reconstruct_binding(&conn, &root, &review.binding, text_outputs::validate).unwrap();
        assert!(approvals.approve(&review.id, &changed).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn edit_approval_binds_exact_content_and_preserves_pending_request_count_after_reload() {
        let (conn, conversation, root) = fixture();
        let current = saved(&root, &conversation.id, 1);
        let content = serde_json::json!({"title":"Edited","markdown":"Updated locally"});
        let (policy, edit) = action(
            &conn,
            &root,
            &conversation.id,
            ArtifactKind::Document,
            "edit",
            None,
            Some(&content),
            text_outputs::validate,
        )
        .unwrap();
        let mut approvals = ApprovalStore::default();
        let review = approvals.register(edit.clone()).unwrap();
        let fresh =
            reconstruct_binding(&conn, &root, &review.binding, text_outputs::validate).unwrap();
        let token = approvals.approve(&review.id, &fresh).unwrap();
        let altered = serde_json::json!({"title":"Changed","markdown":"Updated locally"});
        let (_, altered) = action(
            &conn,
            &root,
            &conversation.id,
            ArtifactKind::Document,
            "edit",
            None,
            Some(&altered),
            text_outputs::validate,
        )
        .unwrap();
        assert!(crate::commands::require_authorization(
            &mut approvals,
            &policy,
            &altered,
            Some(&token)
        )
        .is_err());
        let review = approvals.register(edit.clone()).unwrap();
        let token = approvals.approve(&review.id, &edit.binding).unwrap();
        crate::commands::require_authorization(&mut approvals, &policy, &edit, Some(&token))
            .unwrap();
        let mut edited = current;
        edited.content = content.clone();
        let edited =
            artifact::save(&root, &conversation.id, &edited, text_outputs::validate).unwrap();
        assert_eq!(edited.revision, 2);
        assert_eq!(edited.request_count, 1);
        let reloaded = artifact::load(
            &root,
            &conversation.id,
            ArtifactKind::Document,
            text_outputs::validate,
        )
        .unwrap()
        .unwrap();
        assert_eq!(reloaded.content, content);
        assert_eq!(reloaded.request_count, 1);
        assert!(reconstruct_binding(&conn, &root, &edit.binding, text_outputs::validate).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn exports_write_exact_bytes_and_preserve_existing_files_without_confirmation() {
        let root = std::env::temp_dir().join(format!("bench-export-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let path = root.join("Draft.md");
        write_selected(&path, b"# Draft\n\nBody", false).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"# Draft\n\nBody");
        assert!(write_selected(&path, b"replacement", false).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"# Draft\n\nBody");
        write_selected(&path, b"replacement", true).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"replacement");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        assert!(write_selected(&path, &vec![b'x'; 30 * 1024 * 1024 + 1], true).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"replacement");
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn export_rejects_symlink_destination_and_ancestors_without_touching_target() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!("bench-export-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let secret = root.join("original");
        fs::write(&secret, "keep").unwrap();
        let destination = root.join("linked.md");
        symlink(&secret, &destination).unwrap();
        assert!(write_selected(&destination, b"replace", true).is_err());
        assert_eq!(fs::read_to_string(&secret).unwrap(), "keep");
        let folder = root.join("folder");
        fs::create_dir(&folder).unwrap();
        symlink(&folder, root.join("alias")).unwrap();
        assert!(write_selected(&root.join("alias/new.md"), b"write", false).is_err());
        assert!(!folder.join("new.md").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
