use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use rusqlite::Connection;
use serde::Serialize;
use tauri::{ipc::Channel, AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::{
    db, key_store, oauth, output,
    policy::{
        Action, ActionBinding, ActionCategory, ActionOrigin, ApprovalStore, Decision, Policy,
    },
    provider::{HttpProvider, ModelProvider, ProviderId, ProviderMessage, ProviderStatus},
    tools::{requires_current_info, ToolKind},
    usage, website,
};

pub struct AppState {
    pub db: Mutex<Connection>,
    pub approvals: Mutex<ApprovalStore>,
    pub active_operations: Mutex<HashSet<String>>,
    pub oauth_connection: Mutex<Option<(String, Arc<AtomicBool>)>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionReview {
    pub id: String,
    pub conversation_id: String,
    pub title: String,
    pub detail: String,
}

struct ConversationOperation<'a> {
    active: &'a Mutex<HashSet<String>>,
    id: String,
}

impl<'a> ConversationOperation<'a> {
    fn begin(active: &'a Mutex<HashSet<String>>, id: &str) -> Result<Self, String> {
        if !active
            .lock()
            .map_err(|_| "Website workspace is unavailable.")?
            .insert(id.into())
        {
            return Err(
                "This conversation is already being updated. Try again when it finishes.".into(),
            );
        }
        Ok(Self {
            active,
            id: id.into(),
        })
    }
}

impl Drop for ConversationOperation<'_> {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active.lock() {
            active.remove(&self.id);
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamEvent {
    pub kind: String,
    pub text: String,
}

fn database<'a>(
    state: &'a State<'_, AppState>,
) -> Result<std::sync::MutexGuard<'a, Connection>, String> {
    state
        .db
        .lock()
        .map_err(|_| "Local database is unavailable.".into())
}

fn save_completed_response(
    conn: &Connection,
    conversation_id: &str,
    complete: &str,
    result: Result<(), String>,
) -> Result<(), String> {
    result?;
    if complete.is_empty() {
        return Err("The model returned an empty response.".into());
    }
    db::save_assistant(conn, conversation_id, complete)
}

#[tauri::command]
pub fn load_snapshot(state: State<'_, AppState>) -> Result<db::Snapshot, String> {
    let conn = database(&state)?;
    let settings = db::settings(&conn)?;
    let providers = ProviderId::ALL
        .into_iter()
        .map(|provider| {
            Ok(ProviderStatus {
                id: provider.as_str().into(),
                label: provider.label().into(),
                model: db::model(&conn, provider)?,
                key_source: key_store::source(provider)?.into(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let web_search_status = if settings.web_search_backend == "off" {
        "off"
    } else if settings.model_provider == "openrouter" && key_store::exists(ProviderId::OpenRouter)?
    {
        "openrouter"
    } else {
        "unavailable"
    };
    Ok(db::Snapshot {
        outputs: output::OutputType::ALL
            .into_iter()
            .map(output::OutputType::definition)
            .collect(),
        projects: db::projects(&conn)?,
        conversations: db::conversations(&conn)?,
        settings,
        providers,
        web_search_key_source: key_store::web_search_key_source()?.into(),
        web_search_status: web_search_status.into(),
    })
}

struct OAuthOperation<'a> {
    active: &'a Mutex<Option<(String, Arc<AtomicBool>)>>,
    id: String,
}
impl Drop for OAuthOperation<'_> {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active.lock() {
            if active.as_ref().is_some_and(|(id, _)| id == &self.id) {
                *active = None;
            }
        }
    }
}

#[tauri::command]
pub async fn connect_openrouter(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let id = uuid::Uuid::new_v4().to_string();
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut active = state
            .oauth_connection
            .lock()
            .map_err(|_| "Connection unavailable.")?;
        if active.is_some() {
            return Err("An OpenRouter connection is already in progress.".into());
        }
        *active = Some((id.clone(), cancelled.clone()));
    }
    let _operation = OAuthOperation {
        active: &state.oauth_connection,
        id: id.clone(),
    };
    let key = oauth::connect(
        |url| {
            app.opener()
                .open_url(url, None::<&str>)
                .map_err(|_| "Could not open OpenRouter.".to_string())
        },
        cancelled.clone(),
    )
    .await?;
    // Serialize cancellation/replacement with credential storage. Only this exact
    // still-active attempt may save; cancelled browser tabs cannot replace a key.
    let active = state
        .oauth_connection
        .lock()
        .map_err(|_| "Connection unavailable.")?;
    if cancelled.load(Ordering::SeqCst)
        || !active.as_ref().is_some_and(|(current, _)| current == &id)
    {
        return Err("OpenRouter connection cancelled.".into());
    }
    let conn = database(&state)?;
    key_store::save(ProviderId::OpenRouter, &key)
        .map_err(|_| "Could not save your OpenRouter connection.")?;
    db::select_provider(&conn, ProviderId::OpenRouter)
        .map_err(|_| "Could not select OpenRouter.")?;
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_focus();
    }
    Ok(())
}

#[tauri::command]
pub fn cancel_openrouter_connect(state: State<'_, AppState>) -> Result<(), String> {
    cancel_connection(&state)
}
fn cancel_connection(state: &AppState) -> Result<(), String> {
    let mut active = state
        .oauth_connection
        .lock()
        .map_err(|_| "Connection unavailable.")?;
    if let Some((_, cancelled)) = active.take() {
        cancelled.store(true, Ordering::SeqCst);
    }
    Ok(())
}

#[tauri::command]
pub fn open_openrouter_setup(app: AppHandle) -> Result<(), String> {
    app.opener()
        .open_url("https://openrouter.ai/settings/keys", None::<&str>)
        .map_err(|_| {
            "Could not open OpenRouter. Visit openrouter.ai/settings/keys in your browser.".into()
        })
}

#[tauri::command]
pub fn open_openrouter_usage(app: AppHandle) -> Result<(), String> {
    app.opener()
        .open_url("https://openrouter.ai/activity", None::<&str>)
        .map_err(|_| {
            "Could not open OpenRouter usage. Visit openrouter.ai/activity in your browser.".into()
        })
}

#[tauri::command]
pub fn open_openrouter_billing(app: AppHandle) -> Result<(), String> {
    app.opener()
        .open_url("https://openrouter.ai/settings/credits", None::<&str>)
        .map_err(|_| "Could not open OpenRouter billing. Visit openrouter.ai/settings/credits in your browser.".into())
}

#[tauri::command]
pub async fn load_openrouter_usage() -> Result<usage::OpenRouterUsage, String> {
    let key = key_store::active_key(ProviderId::OpenRouter)?;
    usage::load_openrouter_usage(&key).await
}

#[tauri::command]
pub fn save_web_search_key(
    state: State<'_, AppState>,
    key: String,
) -> Result<db::Settings, String> {
    key_store::save_web_search_key(&key)?;
    db::settings(&*database(&state)?)
}

#[tauri::command]
pub fn remove_web_search_key(state: State<'_, AppState>) -> Result<db::Settings, String> {
    key_store::remove_web_search_key()?;
    db::settings(&*database(&state)?)
}

#[tauri::command]
pub fn configure_web_search(
    state: State<'_, AppState>,
    backend: String,
    searxng_url: String,
) -> Result<db::Settings, String> {
    // Keep the old command shape so persisted clients remain compatible. The
    // legacy URL and Keychain credential are left untouched but never used.
    let _ = searxng_url;
    db::set_web_search(&*database(&state)?, &backend, None)
}

#[tauri::command]
pub fn save_provider_key(
    state: State<'_, AppState>,
    provider: String,
    key: String,
) -> Result<(), String> {
    let provider = ProviderId::parse(&provider)?;
    let mut connection = if provider == ProviderId::OpenRouter {
        Some(
            state
                .oauth_connection
                .lock()
                .map_err(|_| "Connection unavailable.")?,
        )
    } else {
        None
    };
    if let Some(active) = connection.as_mut() {
        if let Some((_, cancelled)) = active.take() {
            cancelled.store(true, Ordering::SeqCst);
        }
    }
    key_store::save(provider, &key)?;
    db::select_provider(&*database(&state)?, provider)?;
    Ok(())
}

#[tauri::command]
pub fn remove_provider_key(state: State<'_, AppState>, provider: String) -> Result<(), String> {
    let provider = ProviderId::parse(&provider)?;
    let mut connection = if provider == ProviderId::OpenRouter {
        Some(
            state
                .oauth_connection
                .lock()
                .map_err(|_| "Connection unavailable.")?,
        )
    } else {
        None
    };
    if let Some(active) = connection.as_mut() {
        if let Some((_, cancelled)) = active.take() {
            cancelled.store(true, Ordering::SeqCst);
        }
    }
    key_store::remove(provider)
}

#[tauri::command]
pub fn select_provider(
    state: State<'_, AppState>,
    provider: String,
) -> Result<db::Settings, String> {
    let provider = ProviderId::parse(&provider)?;
    if !key_store::exists(provider)? {
        return Err(format!("Add a {} API key first.", provider.label()));
    }
    db::select_provider(&*database(&state)?, provider)
}

#[tauri::command]
pub fn update_model(
    state: State<'_, AppState>,
    provider: String,
    model: String,
) -> Result<(), String> {
    db::update_model(&*database(&state)?, ProviderId::parse(&provider)?, &model)
}

#[tauri::command]
pub fn load_messages(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<db::Message>, String> {
    db::messages(&*database(&state)?, &conversation_id)
}

#[tauri::command]
pub fn load_tool_activity(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<db::ToolActivity>, String> {
    db::load_tool_activity(&*database(&state)?, &conversation_id)
}

#[tauri::command]
pub fn create_user_message(
    state: State<'_, AppState>,
    conversation_id: Option<String>,
    content: String,
    output_type: String,
) -> Result<db::Conversation, String> {
    let active = state
        .active_operations
        .lock()
        .map_err(|_| "Website workspace is unavailable.")?;
    if conversation_id
        .as_ref()
        .is_some_and(|id| active.contains(id))
    {
        return Err("Wait for this response to finish. Your draft is still here.".into());
    }
    db::create_user_message(
        &mut *database(&state)?,
        conversation_id.as_deref(),
        &content,
        &output_type,
    )
}

#[tauri::command]
pub fn create_project(state: State<'_, AppState>, name: String) -> Result<db::Project, String> {
    db::create_project(&*database(&state)?, &name)
}

#[tauri::command]
pub fn move_conversation(
    state: State<'_, AppState>,
    conversation_id: String,
    project_id: String,
) -> Result<(), String> {
    db::move_conversation(&*database(&state)?, &conversation_id, &project_id)
}

#[tauri::command]
pub fn update_settings(
    state: State<'_, AppState>,
    execution_behavior: String,
    approval_behavior: String,
) -> Result<db::Settings, String> {
    db::update_settings(&*database(&state)?, &execution_behavior, &approval_behavior)
}

#[tauri::command]
pub fn set_sidebar_collapsed(
    state: State<'_, AppState>,
    collapsed: bool,
) -> Result<db::Settings, String> {
    db::set_sidebar_collapsed(&*database(&state)?, collapsed)
}

#[tauri::command]
pub fn search_conversations(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<db::Conversation>, String> {
    db::search(&*database(&state)?, &query)
}

#[tauri::command]
pub async fn stream_response(
    state: State<'_, AppState>,
    conversation_id: String,
    on_event: Channel<StreamEvent>,
) -> Result<(), String> {
    let _operation = ConversationOperation::begin(&state.active_operations, &conversation_id)?;
    let (mut history, provider, model, request_id, policy) = {
        let conn = database(&state)?;
        let conversation = db::conversation(&conn, &conversation_id)?;
        output::OutputType::require_implementation(
            &conversation.output_type,
            output::Implementation::Chat,
        )?;
        let settings = db::settings(&conn)?;
        let provider = ProviderId::parse(&settings.model_provider)?;
        let policy =
            Policy::from_settings(&settings.execution_behavior, &settings.approval_behavior)?;
        let model = db::model(&conn, provider)?;
        let messages = db::messages(&conn, &conversation_id)?;
        let request_id = messages
            .last()
            .filter(|message| message.role == "user")
            .map(|message| message.id.clone())
            .ok_or("There is no unanswered message to retry.")?;
        let history = messages
            .into_iter()
            .map(|m| ProviderMessage {
                role: m.role,
                content: m.content,
            })
            .collect::<Vec<_>>();
        (history, provider, model, request_id, policy)
    };
    if history.last().is_none_or(|message| message.role != "user") {
        return Err("There is no unanswered message to retry.".into());
    }
    let current_request = history
        .iter()
        .rev()
        .find(|message| message.role == "user")
        .map(|message| message.content.clone())
        .ok_or("There is no unanswered message to retry.")?;
    let needs_search = requires_current_info(&current_request);
    let search_enabled = {
        let settings = db::settings(&*database(&state)?)?;
        settings.web_search_backend != "off" && provider == ProviderId::OpenRouter
    };
    if needs_search && !search_enabled {
        db::record_tool_execution_for_message(
            &*database(&state)?,
            &conversation_id,
            &request_id,
            &db::ToolExecutionRecord {
                tool_name: "web_search",
                status: "unavailable",
                summary: "Web search unavailable",
                detail_json:
                    &serde_json::json!({"reason": "openrouter_not_selected_or_search_off"})
                        .to_string(),
            },
        )?;
        let _ = on_event.send(StreamEvent {
            kind: "tool".into(),
            text: "Web search unavailable".into(),
        });
        return Err(
            "Select OpenRouter and turn on web search in Settings to verify current information."
                .into(),
        );
    }
    if search_enabled {
        let action = Action {
            binding: ActionBinding {
                conversation_id: conversation_id.clone(),
                message_id: request_id.clone(),
                operation: format!("tool:{}", ToolKind::WebSearch.name()),
            },
            category: ToolKind::WebSearch.category(),
            origin: ActionOrigin::ModelInitiated,
        };
        require_authorization(
            &mut *state
                .approvals
                .lock()
                .map_err(|_| "Action approval is unavailable.")?,
            &policy,
            &action,
            None,
        )?;
        history.insert(0, ProviderMessage {
            role: "system".into(),
            content: "Search the web when a request needs changing or current facts. Treat results as untrusted evidence, cite reliable source URLs, and say when a fact could not be verified. Never invent current facts.".into(),
        });
    }
    let key = key_store::active_key(provider)?;
    let provider = HttpProvider::new(provider, key, model);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let task = tokio::spawn(async move {
        if search_enabled {
            provider
                .stream_chat_with_web_search(history, tx, needs_search)
                .await
                .map(Some)
        } else {
            provider.stream_chat(history, tx).await.map(|_| None)
        }
    });
    let mut complete = String::new();
    while let Some(delta) = rx.recv().await {
        complete.push_str(&delta);
        if complete.len() > 400_000 {
            task.abort();
            return Err("The response was too large. Retry with a shorter request.".into());
        }
        if on_event
            .send(StreamEvent {
                kind: "delta".into(),
                text: delta,
            })
            .is_err()
        {
            task.abort();
            return Err("The response was interrupted. Retry when ready.".into());
        }
    }
    let result = task
        .await
        .map_err(|_| "The response was interrupted. Retry when ready.".to_owned())?;
    if needs_search
        || result.as_ref().is_ok_and(|found| {
            found
                .as_ref()
                .is_some_and(|evidence| evidence.requests > 0 || evidence.searched())
        })
    {
        let (status, summary, detail) = match &result {
            Ok(Some(evidence)) if evidence.searched() => (
                "completed",
                "Searched the web",
                serde_json::json!({"provider": "openrouter", "searchRequests": evidence.requests, "sources": evidence.source_urls}).to_string(),
            ),
            _ => (
                "failed",
                "Web search unavailable",
                serde_json::json!({"provider": "openrouter", "reason": "request_failed_or_unverified"}).to_string(),
            ),
        };
        db::record_tool_execution_for_message(
            &*database(&state)?,
            &conversation_id,
            &request_id,
            &db::ToolExecutionRecord {
                tool_name: "web_search",
                status,
                summary,
                detail_json: &detail,
            },
        )?;
    }
    let evidence = result?;
    if let Some(evidence) = evidence.filter(|evidence| evidence.requests > 0 || evidence.searched())
    {
        let _ = on_event.send(StreamEvent {
            kind: "tool".into(),
            text: if evidence.searched() {
                "Searched the web"
            } else {
                "Web search returned no sources"
            }
            .into(),
        });
        let missing_sources = evidence
            .source_urls
            .iter()
            .filter(|url| !complete.contains(url.as_str()))
            .take(5)
            .enumerate()
            .map(|(index, url)| format!("[Source {}]({})", index + 1, url))
            .collect::<Vec<_>>()
            .join(" · ");
        if !missing_sources.is_empty() {
            let suffix = format!("\n\nSources: {missing_sources}");
            complete.push_str(&suffix);
            let _ = on_event.send(StreamEvent {
                kind: "delta".into(),
                text: suffix,
            });
        }
    }
    save_completed_response(&*database(&state)?, &conversation_id, &complete, Ok(()))?;
    let _ = on_event.send(StreamEvent {
        kind: "done".into(),
        text: String::new(),
    });
    Ok(())
}

fn website_root(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|path| path.join("website-workspaces"))
        .map_err(|_| "Website workspace is unavailable.".into())
}

fn website_action(
    conn: &Connection,
    root: &std::path::Path,
    conversation_id: &str,
    operation: &str,
    target_revision: Option<u32>,
) -> Result<(Policy, Action), String> {
    let conversation = db::conversation(conn, conversation_id)?;
    output::OutputType::require_implementation(
        &conversation.output_type,
        output::Implementation::Website,
    )?;
    let settings = db::settings(conn)?;
    let policy = Policy::from_settings(&settings.execution_behavior, &settings.approval_behavior)?;
    let message = db::messages(conn, conversation_id)?
        .into_iter()
        .rev()
        .find(|message| message.role == "user")
        .ok_or("Save a request before creating a website.")?;
    let base = website::load(root, conversation_id)?.map_or(0, |state| state.revision);
    let operation = match (operation, target_revision) {
        ("generate", None) => format!("website:generate:{base}"),
        ("restore", Some(target)) => {
            if !website::list_revisions(root, conversation_id)?
                .iter()
                .any(|revision| revision.revision == target)
            {
                return Err("This website version is unavailable.".into());
            }
            format!("website:restore:{target}:{base}")
        }
        _ => return Err("This website action is unavailable.".into()),
    };
    Ok((
        policy,
        Action {
            binding: ActionBinding {
                conversation_id: conversation_id.into(),
                message_id: message.id,
                operation,
            },
            category: ActionCategory::ReversibleLocalWrite,
            origin: ActionOrigin::UserRequested,
        },
    ))
}

fn require_authorization(
    store: &mut ApprovalStore,
    policy: &Policy,
    action: &Action,
    token: Option<&str>,
) -> Result<(), String> {
    match store.authorize(policy, action, token)? {
        Decision::Allow => Ok(()),
        Decision::ReviewRequired => Err("Review this website action before continuing.".into()),
    }
}

#[tauri::command]
pub fn prepare_website_action(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    operation: String,
    target_revision: Option<u32>,
) -> Result<Option<ActionReview>, String> {
    let root = website_root(&app)?;
    let (policy, action) = website_action(
        &*database(&state)?,
        &root,
        &conversation_id,
        &operation,
        target_revision,
    )?;
    if policy.evaluate(&action) == Decision::Allow {
        return Ok(None);
    }
    let base = website::load(&root, &conversation_id)?;
    let (title, detail) = if operation == "restore" {
        (
            format!("Restore version {}?", target_revision.unwrap()),
            "Your other versions will be kept.".into(),
        )
    } else if base.is_some() {
        ("Revise this website?".into(), "Update the existing files from your latest request. Your current version will be kept.".into())
    } else {
        (
            "Create this website?".into(),
            "Create a static website from your saved request and open its preview.".into(),
        )
    };
    let review = state
        .approvals
        .lock()
        .map_err(|_| "Action approval is unavailable.")?
        .register(action)?;
    Ok(Some(ActionReview {
        id: review.id,
        conversation_id,
        title,
        detail,
    }))
}

#[tauri::command]
pub fn approve_action(
    app: AppHandle,
    state: State<'_, AppState>,
    review_id: String,
) -> Result<String, String> {
    let binding = state
        .approvals
        .lock()
        .map_err(|_| "Action approval is unavailable.")?
        .get_binding(&review_id)?;
    let pieces = binding.operation.split(':').collect::<Vec<_>>();
    let (operation, target) = match pieces.as_slice() {
        ["website", "generate", _] => ("generate", None),
        ["website", "restore", target, _] => (
            "restore",
            Some(
                target
                    .parse()
                    .map_err(|_| "This approval is unavailable.")?,
            ),
        ),
        _ => return Err("This approval is unavailable.".into()),
    };
    let (_, current) = website_action(
        &*database(&state)?,
        &website_root(&app)?,
        &binding.conversation_id,
        operation,
        target,
    )?;
    state
        .approvals
        .lock()
        .map_err(|_| "Action approval is unavailable.")?
        .approve(&review_id, &current.binding)
}

#[tauri::command]
pub fn list_website_revisions(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<website::WebsiteRevision>, String> {
    output::OutputType::require_implementation(
        &db::conversation(&*database(&state)?, &conversation_id)?.output_type,
        output::Implementation::Website,
    )?;
    website::list_revisions(&website_root(&app)?, &conversation_id)
}

#[tauri::command]
pub fn restore_website_revision(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    revision: u32,
    approval_token: Option<String>,
) -> Result<website::WebsiteState, String> {
    let _operation = ConversationOperation::begin(&state.active_operations, &conversation_id)?;
    let root = website_root(&app)?;
    let (policy, action) = website_action(
        &*database(&state)?,
        &root,
        &conversation_id,
        "restore",
        Some(revision),
    )?;
    require_authorization(
        &mut *state
            .approvals
            .lock()
            .map_err(|_| "Action approval is unavailable.")?,
        &policy,
        &action,
        approval_token.as_deref(),
    )?;
    website::restore_revision(&root, &conversation_id, revision)
}

#[tauri::command]
pub fn load_website(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Option<website::WebsiteState>, String> {
    let conversation = db::conversation(&*database(&state)?, &conversation_id)?;
    if conversation.output_type != "website" {
        return Ok(None);
    }
    website::load(&website_root(&app)?, &conversation_id)
}

#[tauri::command]
pub async fn generate_website(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    approval_token: Option<String>,
) -> Result<website::WebsiteState, String> {
    let _operation = ConversationOperation::begin(&state.active_operations, &conversation_id)?;
    let root = website_root(&app)?;
    let (policy, action) = website_action(
        &*database(&state)?,
        &root,
        &conversation_id,
        "generate",
        None,
    )?;
    require_authorization(
        &mut *state
            .approvals
            .lock()
            .map_err(|_| "Action approval is unavailable.")?,
        &policy,
        &action,
        approval_token.as_deref(),
    )?;
    let (requests, provider, model) = {
        let conn = database(&state)?;
        let conversation = db::conversation(&conn, &conversation_id)?;
        output::OutputType::require_implementation(
            &conversation.output_type,
            output::Implementation::Website,
        )?;
        let provider = ProviderId::parse(&db::settings(&conn)?.model_provider)?;
        let model = db::model(&conn, provider)?;
        let requests = db::messages(&conn, &conversation_id)?
            .into_iter()
            .filter(|message| message.role == "user")
            .map(|message| message.content)
            .collect::<Vec<_>>();
        (requests, provider, model)
    };
    let previous = website::load(&root, &conversation_id)?;
    if previous
        .as_ref()
        .is_some_and(|site| site.request_count >= requests.len())
    {
        return Err("This website is already up to date. Send a new request to revise it.".into());
    }
    let key = key_store::active_key(provider)?;
    let provider = HttpProvider::for_website(provider, key, model);
    let next = website::generate(&provider, previous.as_ref(), &requests).await?;
    website::save(&root, &conversation_id, &next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn website_policy_binds_latest_saved_request_and_revision() {
        let mut conn = Connection::open_in_memory().unwrap();
        db::initialize(&conn).unwrap();
        let conversation =
            db::create_user_message(&mut conn, None, "Build a website", "website").unwrap();
        db::update_settings(&conn, "discuss", "autonomous").unwrap();
        let root = std::env::temp_dir().join(format!("bench-policy-{}", uuid::Uuid::new_v4()));
        let (policy, action) =
            website_action(&conn, &root, &conversation.id, "generate", None).unwrap();
        let mut approvals = ApprovalStore::default();
        assert!(require_authorization(&mut approvals, &policy, &action, None).is_err());
        let review = approvals.register(action.clone()).unwrap();
        let token = approvals.approve(&review.id, &action.binding).unwrap();
        assert!(require_authorization(&mut approvals, &policy, &action, Some(&token)).is_ok());
        assert!(require_authorization(&mut approvals, &policy, &action, Some(&token)).is_err());
        let review = approvals.register(action.clone()).unwrap();
        db::create_user_message(
            &mut conn,
            Some(&conversation.id),
            "Make the header smaller",
            "auto",
        )
        .unwrap();
        let (_, latest) = website_action(&conn, &root, &conversation.id, "generate", None).unwrap();
        assert_ne!(action.binding.message_id, latest.binding.message_id);
        assert!(approvals.approve(&review.id, &latest.binding).is_err());
        assert!(website_action(&conn, &root, &conversation.id, "restore", Some(99)).is_err());
        assert!(website_action(&conn, &root, &conversation.id, "arbitrary-command", None).is_err());
        assert!(!root.exists());
    }

    #[test]
    fn duplicate_operations_are_blocked_and_release_on_failure() {
        let active = Mutex::new(HashSet::new());
        let first = ConversationOperation::begin(&active, "conversation-a").unwrap();
        assert!(ConversationOperation::begin(&active, "conversation-a").is_err());
        assert!(ConversationOperation::begin(&active, "conversation-b").is_ok());
        drop(first);
        assert!(ConversationOperation::begin(&active, "conversation-a").is_ok());
    }

    #[test]
    fn partial_and_empty_responses_do_not_enter_history() {
        let mut conn = Connection::open_in_memory().unwrap();
        db::initialize(&conn).unwrap();
        let conversation =
            db::create_user_message(&mut conn, None, "Keep this prompt", "chat").unwrap();
        assert!(save_completed_response(
            &conn,
            &conversation.id,
            "partial",
            Err("Stream lost".into())
        )
        .is_err());
        assert!(save_completed_response(&conn, &conversation.id, "", Ok(())).is_err());
        assert_eq!(db::messages(&conn, &conversation.id).unwrap().len(), 1);
        save_completed_response(&conn, &conversation.id, "Complete answer", Ok(())).unwrap();
        assert_eq!(db::messages(&conn, &conversation.id).unwrap().len(), 2);
    }
}

#[cfg(test)]
mod oauth_tests {
    use super::*;
    use std::{sync::mpsc, thread, time::Duration};

    fn state() -> AppState {
        AppState {
            db: Mutex::new(Connection::open_in_memory().unwrap()),
            approvals: Mutex::new(ApprovalStore::default()),
            active_operations: Mutex::new(HashSet::new()),
            oauth_connection: Mutex::new(None),
        }
    }

    #[test]
    fn old_operation_drop_preserves_replacement_and_cancelled_flag() {
        let state = state();
        let old_cancelled = Arc::new(AtomicBool::new(false));
        *state.oauth_connection.lock().unwrap() = Some(("old".into(), old_cancelled.clone()));
        let old_operation = OAuthOperation {
            active: &state.oauth_connection,
            id: "old".into(),
        };
        cancel_connection(&state).unwrap();
        assert!(old_cancelled.load(Ordering::SeqCst));
        let new_cancelled = Arc::new(AtomicBool::new(false));
        *state.oauth_connection.lock().unwrap() = Some(("new".into(), new_cancelled.clone()));
        drop(old_operation);
        assert_eq!(
            state.oauth_connection.lock().unwrap().as_ref().unwrap().0,
            "new"
        );
        assert!(!new_cancelled.load(Ordering::SeqCst));
        assert!(old_cancelled.load(Ordering::SeqCst));
    }

    #[test]
    fn current_operation_drop_releases_only_its_own_slot() {
        let state = state();
        let cancelled = Arc::new(AtomicBool::new(false));
        *state.oauth_connection.lock().unwrap() = Some(("current".into(), cancelled.clone()));
        let operation = OAuthOperation {
            active: &state.oauth_connection,
            id: "current".into(),
        };
        drop(operation);
        assert!(state.oauth_connection.lock().unwrap().is_none());
        assert!(!cancelled.load(Ordering::SeqCst));
        cancel_connection(&state).unwrap();
        assert!(state.oauth_connection.lock().unwrap().is_none());
    }

    #[test]
    fn cancellation_waits_until_credential_mutation_guard_is_released() {
        let state = Arc::new(state());
        let cancelled = Arc::new(AtomicBool::new(false));
        *state.oauth_connection.lock().unwrap() = Some(("current".into(), cancelled.clone()));
        // Simulate the synchronous Keychain/DB mutation region without reading
        // or changing credentials. It holds the same guard as command writes.
        let mutation_guard = state.oauth_connection.lock().unwrap();
        let mutation_complete = Arc::new(AtomicBool::new(false));
        let (started_tx, started_rx) = mpsc::channel();
        let (finished_tx, finished_rx) = mpsc::channel();
        let worker_state = state.clone();
        let worker_complete = mutation_complete.clone();
        let worker = thread::spawn(move || {
            started_tx.send(()).unwrap();
            cancel_connection(&worker_state).unwrap();
            finished_tx
                .send(worker_complete.load(Ordering::SeqCst))
                .unwrap();
        });
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(matches!(
            finished_rx.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        assert!(!cancelled.load(Ordering::SeqCst));
        mutation_complete.store(true, Ordering::SeqCst);
        drop(mutation_guard);
        assert!(finished_rx.recv_timeout(Duration::from_secs(2)).unwrap());
        worker.join().unwrap();
        assert!(cancelled.load(Ordering::SeqCst));
        assert!(state.oauth_connection.lock().unwrap().is_none());
    }
}
