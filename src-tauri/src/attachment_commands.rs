use crate::attachments::{self, AttachmentMetadata, AttachmentState, MAX_FILES};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
#[tauri::command]
pub async fn choose_attachments(
    app: AppHandle,
    state: State<'_, AttachmentState>,
) -> Result<Vec<AttachmentMetadata>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter(
            "Documents and images",
            &[
                "txt", "md", "csv", "json", "pdf", "docx", "xlsx", "png", "jpg", "jpeg", "webp",
            ],
        )
        .pick_files(move |paths| {
            let _ = tx.send(paths);
        });
    let Some(paths) = rx.await.map_err(|_| "File selection was interrupted.")? else {
        return Ok(Vec::new());
    };
    if paths.len() > MAX_FILES {
        return Err("Choose at most five files.".into());
    }
    let paths = paths
        .into_iter()
        .map(|path| {
            path.into_path()
                .map_err(|_| "Choose a local file on this Mac.")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let items = tauri::async_runtime::spawn_blocking(move || {
        paths
            .iter()
            .map(|path| attachments::read_selected(path))
            .collect::<Result<Vec<_>, _>>()
    })
    .await
    .map_err(|_| "File extraction was interrupted.")??;
    state.stage(items)
}
#[tauri::command]
pub fn remove_staged_attachment(
    state: State<'_, AttachmentState>,
    attachment_id: String,
) -> Result<(), String> {
    state.remove(&attachment_id)
}
