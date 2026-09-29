use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::Path,
};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::provider::{ModelProvider, ProviderMessage};

const UNSAFE_PATH: &str = "Website workspace contains an unsafe path.";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsitePage {
    pub path: String,
    pub html: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteState {
    pub pages: Vec<WebsitePage>,
    pub css: String,
    pub revision: u32,
    pub request_count: usize,
}

#[derive(Deserialize, Serialize)]
struct Manifest {
    revision: u32,
    pages: Vec<String>,
    #[serde(default)]
    request_count: usize,
}

#[derive(Deserialize)]
struct SitePatch {
    files: BTreeMap<String, String>,
    #[serde(default)]
    remove: Vec<String>,
}

fn valid_path(path: &str) -> bool {
    path == "style.css"
        || (path.ends_with(".html")
            && path.len() <= 60
            && path
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "-_ .".contains(character))
            && !path.starts_with('.'))
}

fn site_dir(root: &Path, conversation_id: &str) -> Result<std::path::PathBuf, String> {
    if uuid::Uuid::parse_str(conversation_id).is_err() {
        return Err("Website workspace could not be found.".into());
    }
    check_directory(root)?;
    let dir = root.join(conversation_id);
    check_directory(&dir)?;
    Ok(dir)
}

// Reject local workspace links as well as generated path traversal. Ancestors of
// the application-owned root are trusted; generated files never create links.
fn check_directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(UNSAFE_PATH.into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("Could not read the website workspace.".into()),
    }
}

fn read_limited(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| "Could not read the website revision.".to_owned())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(UNSAFE_PATH.into());
    }
    if metadata.len() > limit as u64 {
        return Err("Website workspace file is too large.".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .and_then(|file| file.take(limit as u64 + 1).read_to_end(&mut bytes))
        .map_err(|_| "Could not read the website revision.".to_owned())?;
    if bytes.len() > limit {
        return Err("Website workspace file is too large.".into());
    }
    Ok(bytes)
}

fn read_text(path: &Path) -> Result<String, String> {
    String::from_utf8(read_limited(path, 100_000)?)
        .map_err(|_| "Website workspace file is unreadable.".into())
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteRevision {
    pub revision: u32,
    pub current: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum Pointer {
    Legacy(u32),
    Current { revision: u32, request_count: usize },
}

fn read_pointer(dir: &Path) -> Result<Option<Pointer>, String> {
    let path = dir.join("current.json");
    match fs::symlink_metadata(&path) {
        Ok(_) => serde_json::from_slice(&read_limited(&path, 1_000)?)
            .map(Some)
            .map_err(|_| "Website workspace metadata is unreadable.".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("Could not read the website workspace.".into()),
    }
}

fn load_revision(dir: &Path, revision: u32) -> Result<WebsiteState, String> {
    let revision_dir = dir.join(format!("revision-{revision}"));
    check_directory(&revision_dir)?;
    let manifest: Manifest =
        serde_json::from_slice(&read_limited(&revision_dir.join("manifest.json"), 4_000)?)
            .map_err(|_| "Website workspace metadata is unreadable.".to_owned())?;
    if manifest.revision != revision
        || manifest.pages.len() > 8
        || manifest
            .pages
            .iter()
            .any(|path| !valid_path(path) || path == "style.css")
    {
        return Err("Website workspace metadata is invalid.".into());
    }
    let pages = manifest
        .pages
        .into_iter()
        .map(|path| {
            let html = read_text(&revision_dir.join(&path))?;
            Ok(WebsitePage { path, html })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let css = read_text(&revision_dir.join("style.css"))?;
    let state = WebsiteState {
        pages,
        css,
        revision,
        request_count: manifest.request_count,
    };
    validate_state(&state)?;
    Ok(state)
}

pub fn load(root: &Path, conversation_id: &str) -> Result<Option<WebsiteState>, String> {
    let dir = site_dir(root, conversation_id)?;
    let mut failure = None;
    match read_pointer(&dir) {
        Ok(Some(pointer)) => {
            let (revision, request_count) = match pointer {
                Pointer::Legacy(revision) => (revision, None),
                Pointer::Current {
                    revision,
                    request_count,
                } => (revision, Some(request_count)),
            };
            match load_revision(&dir, revision) {
                Ok(mut state) => {
                    if let Some(count) = request_count {
                        state.request_count = count;
                    }
                    return Ok(Some(state));
                }
                Err(error) if error == UNSAFE_PATH => return Err(error),
                Err(error) => failure = Some(error),
            }
        }
        Ok(None) => {}
        Err(error) if error == UNSAFE_PATH => return Err(error),
        Err(error) => failure = Some(error),
    }
    // A failed read never rewrites metadata. Restore explicitly activates a
    // healthy version; fallback only exposes safe, fully validated history.
    let revisions = revision_numbers(&dir)?;
    for revision in revisions.iter().rev() {
        if let Ok(state) = load_revision(&dir, *revision) {
            // Use this manifest's count, not the unreadable pointer's count:
            // a lost revision must not consume the user's pending request.
            return Ok(Some(state));
        }
    }
    if let Some(error) = failure {
        return Err(error);
    }
    if !revisions.is_empty() {
        return Err("No readable website version is available.".into());
    }
    Ok(None)
}

fn revision_numbers(dir: &Path) -> Result<Vec<u32>, String> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(_) => return Err("Could not read website history.".into()),
    };
    let mut revisions = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| "Could not read website history.".to_owned())?;
        if let Some(number) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_prefix("revision-"))
            .and_then(|number| number.parse::<u32>().ok())
        {
            revisions.push(number);
        }
    }
    revisions.sort_unstable();
    Ok(revisions)
}

pub fn list_revisions(root: &Path, conversation_id: &str) -> Result<Vec<WebsiteRevision>, String> {
    let dir = site_dir(root, conversation_id)?;
    let current = load(root, conversation_id)?.map(|state| state.revision);
    Ok(revision_numbers(&dir)?
        .into_iter()
        .filter(|revision| load_revision(&dir, *revision).is_ok())
        .map(|revision| WebsiteRevision {
            revision,
            current: current == Some(revision),
        })
        .collect())
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file =
        fs::File::create(path).map_err(|_| "Could not save the website revision.".to_owned())?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| "Could not save the website revision.".to_owned())
}

fn activate(dir: &Path, revision: u32, request_count: usize) -> Result<(), String> {
    let temp = dir.join(format!(".current-{}.json", uuid::Uuid::new_v4()));
    let result = (|| {
        let bytes = serde_json::to_vec(&Pointer::Current {
            revision,
            request_count,
        })
        .map_err(|_| "Could not save website metadata.".to_owned())?;
        write_synced(&temp, &bytes)?;
        fs::rename(&temp, dir.join("current.json"))
            .map_err(|_| "Could not activate the website revision.".to_owned())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

pub fn restore_revision(
    root: &Path,
    conversation_id: &str,
    revision: u32,
) -> Result<WebsiteState, String> {
    let dir = site_dir(root, conversation_id)?;
    let current = load(root, conversation_id)?.ok_or("Website workspace could not be found.")?;
    let mut restored = load_revision(&dir, revision)?;
    restored.request_count = current.request_count;
    activate(&dir, revision, restored.request_count)?;
    Ok(restored)
}

fn apply_patch(previous: Option<&WebsiteState>, patch: SitePatch) -> Result<WebsiteState, String> {
    if patch.files.is_empty() && patch.remove.is_empty() {
        return Err("Website response contained no changes. Retry the request.".into());
    }
    let mut files = BTreeMap::new();
    if let Some(previous) = previous {
        files.insert("style.css".to_owned(), previous.css.clone());
        for page in &previous.pages {
            files.insert(page.path.clone(), page.html.clone());
        }
    }
    for path in patch.remove {
        if !valid_path(&path) || path == "index.html" || path == "style.css" {
            return Err("Website response contains an invalid file change.".into());
        }
        files.remove(&path);
    }
    for (path, content) in patch.files {
        if !valid_path(&path)
            || content.len() > 100_000
            || (path.ends_with(".html") && content.trim().is_empty())
        {
            return Err("Website response contains an invalid file.".into());
        }
        if path == "style.css" {
            let lower = content.to_ascii_lowercase();
            if ["</style", "<script", "@import", "url("]
                .iter()
                .any(|blocked| lower.contains(blocked))
            {
                return Err("Website styles contain unsupported external content.".into());
            }
        }
        if path.ends_with(".html") {
            let lower = content.to_ascii_lowercase();
            if [
                "<script",
                "<iframe",
                "<object",
                "<embed",
                "<base",
                "<meta http-equiv",
            ]
            .iter()
            .any(|blocked| lower.contains(blocked))
            {
                return Err("Website HTML contains unsupported active content.".into());
            }
        }
        files.insert(path, content);
    }
    if !files.contains_key("index.html") || !files.contains_key("style.css") {
        return Err("Website response needs index.html and style.css.".into());
    }
    if files.len() > 9 || files.values().map(String::len).sum::<usize>() > 300_000 {
        return Err("Website response is too large.".into());
    }
    let css = files.remove("style.css").unwrap();
    let pages = files
        .into_iter()
        .map(|(path, html)| WebsitePage { path, html })
        .collect();
    Ok(WebsiteState {
        pages,
        css,
        revision: previous.map_or(Ok(1), |state| {
            state
                .revision
                .checked_add(1)
                .ok_or("Website history is full.")
        })?,
        request_count: previous.map_or(0, |state| state.request_count),
    })
}

fn validate_state(state: &WebsiteState) -> Result<(), String> {
    let mut files = BTreeMap::new();
    files.insert("style.css".to_owned(), state.css.clone());
    for page in &state.pages {
        if files.insert(page.path.clone(), page.html.clone()).is_some() {
            return Err("Website contains duplicate file names.".into());
        }
    }
    apply_patch(
        None,
        SitePatch {
            files,
            remove: vec![],
        },
    )
    .map(|_| ())
}

pub fn save(
    root: &Path,
    conversation_id: &str,
    state: &WebsiteState,
) -> Result<WebsiteState, String> {
    validate_state(state)?;
    let dir = site_dir(root, conversation_id)?;
    fs::create_dir_all(&dir).map_err(|_| "Could not create the website workspace.".to_owned())?;
    let mut saved = state.clone();
    saved.revision = revision_numbers(&dir)?
        .last()
        .copied()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or("Website history is full.")?;
    let staging = dir.join(format!(".revision-{}", uuid::Uuid::new_v4()));
    let final_dir = dir.join(format!("revision-{}", saved.revision));
    fs::create_dir(&staging).map_err(|_| "Could not save the website revision.".to_owned())?;
    let result = (|| {
        for page in &saved.pages {
            write_synced(&staging.join(&page.path), page.html.as_bytes())?;
        }
        write_synced(&staging.join("style.css"), saved.css.as_bytes())?;
        let manifest = Manifest {
            revision: saved.revision,
            pages: saved.pages.iter().map(|page| page.path.clone()).collect(),
            request_count: saved.request_count,
        };
        write_synced(
            &staging.join("manifest.json"),
            &serde_json::to_vec(&manifest)
                .map_err(|_| "Could not save website metadata.".to_owned())?,
        )?;
        fs::rename(&staging, &final_dir)
            .map_err(|_| "Could not save the website revision.".to_owned())?;
        if let Err(error) = activate(&dir, saved.revision, saved.request_count) {
            let _ = fs::remove_dir_all(&final_dir);
            return Err(error);
        }
        Ok(saved)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(staging);
    }
    result
}

pub async fn generate(
    provider: &dyn ModelProvider,
    previous: Option<&WebsiteState>,
    user_requests: &[String],
) -> Result<WebsiteState, String> {
    let previous_files = previous.map(|state| {
        let mut files = BTreeMap::new();
        for page in &state.pages {
            files.insert(page.path.clone(), page.html.clone());
        }
        files.insert("style.css".to_owned(), state.css.clone());
        files
    });
    let system = "You build small, polished static websites for Bench. Return ONLY one JSON object: {\"files\":{\"index.html\":\"...\",\"style.css\":\"...\"},\"remove\":[]}. Use semantic HTML and plain CSS. No JavaScript, external assets, imports, network requests, iframes, forms, or commands. Never invent business facts. If the user has not supplied them, do not claim an address, phone number, prices, opening hours, equipment counts, turnaround time, amenities, testimonials, or specific services as real. Use restrained, clearly labeled placeholders for missing facts and keep generic copy factual. For revisions, return ONLY changed files and preserve the existing site. Add extra top-level .html pages when requested. Link pages with relative hrefs. The preview has no network or script access.";
    let request = serde_json::json!({
        "requests": user_requests,
        "existing_files": previous_files,
        "instruction": if previous.is_some() { "Revise the existing files for the latest request." } else { "Create the first version." }
    });
    let messages = vec![
        ProviderMessage {
            role: "system".into(),
            content: system.into(),
        },
        ProviderMessage {
            role: "user".into(),
            content: request.to_string(),
        },
    ];
    let (tx, mut rx) = mpsc::unbounded_channel();
    let stream = provider.stream_chat(messages, tx);
    let collect = async {
        let mut output = String::new();
        while let Some(delta) = rx.recv().await {
            if output.len().saturating_add(delta.len()) > 400_000 {
                return Err("Website response is too large.".to_owned());
            }
            output.push_str(&delta);
        }
        Ok(output)
    };
    let (_, output) = futures_util::future::try_join(stream, collect).await?;
    let trimmed = output.trim();
    let json = if trimmed.starts_with("```") {
        trimmed
            .trim_start_matches('`')
            .trim_start_matches("json")
            .trim()
            .trim_end_matches('`')
            .trim()
    } else {
        trimmed
    };
    let patch: SitePatch = serde_json::from_str(json)
        .map_err(|_| "Website response was incomplete. Retry the request.".to_owned())?;
    let mut state = apply_patch(previous, patch)?;
    state.request_count = user_requests.len();
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use uuid::Uuid;

    struct FakeProvider(&'static str);

    #[async_trait]
    impl ModelProvider for FakeProvider {
        async fn stream_chat(
            &self,
            _messages: Vec<ProviderMessage>,
            deltas: mpsc::UnboundedSender<String>,
        ) -> Result<(), String> {
            deltas.send(self.0.into()).unwrap();
            Ok(())
        }
    }

    #[test]
    fn prompt_to_files_then_conversational_revision() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let initial = runtime
            .block_on(generate(
                &FakeProvider(r#"{"files":{"index.html":"<h1>Laundros</h1>","style.css":"h1{font-size:4rem}"}}"#),
                None,
                &["Build a simple website for Laundros".into()],
            ))
            .unwrap();
        assert_eq!(initial.request_count, 1);
        let revision = runtime
            .block_on(generate(
                &FakeProvider(r#"{"files":{"style.css":"h1{font-size:2rem}","about.html":"<h1>About Laundros</h1>"}}"#),
                Some(&initial),
                &[
                    "Build a simple website for Laundros".into(),
                    "Make the header smaller and add an About page".into(),
                ],
            ))
            .unwrap();
        assert_eq!(revision.revision, 2);
        assert_eq!(revision.request_count, 2);
        assert_eq!(revision.pages.len(), 2);
        assert!(revision
            .pages
            .iter()
            .any(|page| page.path == "index.html" && page.html.contains("Laundros")));
        assert_eq!(revision.css, "h1{font-size:2rem}");
    }

    #[test]
    fn revisions_keep_unchanged_files_and_reload() {
        let root = std::env::temp_dir().join(format!("bench-web-{}", Uuid::new_v4()));
        let id = Uuid::new_v4().to_string();
        let initial = apply_patch(
            None,
            serde_json::from_str(
                r#"{"files":{"index.html":"<h1>Laundros</h1>","style.css":"body{color:black}"}}"#,
            )
            .unwrap(),
        )
        .unwrap();
        save(&root, &id, &initial).unwrap();
        let revision = apply_patch(
            Some(&initial),
            serde_json::from_str(r#"{"files":{"about.html":"<h1>About</h1>"}}"#).unwrap(),
        )
        .unwrap();
        save(&root, &id, &revision).unwrap();
        let loaded = load(&root, &id).unwrap().unwrap();
        assert_eq!(loaded.revision, 2);
        assert_eq!(loaded.pages.len(), 2);
        assert_eq!(loaded.css, "body{color:black}");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_active_content_and_path_traversal() {
        assert!(!valid_path("../escape.html"));
        assert!(!valid_path("folder/page.html"));
        let result = apply_patch(
            None,
            serde_json::from_str(
                r#"{"files":{"index.html":"<script>alert(1)</script>","style.css":""}}"#,
            )
            .unwrap(),
        );
        assert!(result.is_err());
        let css_escape = apply_patch(
            None,
            serde_json::from_str(
                r#"{"files":{"index.html":"<h1>Safe</h1>","style.css":"</style><script>alert(1)</script>"}}"#,
            )
            .unwrap(),
        );
        assert!(css_escape.is_err());
    }
    fn initial_state() -> WebsiteState {
        apply_patch(
            None,
            serde_json::from_str(
                r#"{"files":{"index.html":"<h1>Laundros</h1>","style.css":"body{color:black}"}}"#,
            )
            .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn rollback_preserves_processed_requests_and_next_revision_does_not_collide() {
        let root = std::env::temp_dir().join(format!("bench-web-{}", Uuid::new_v4()));
        let id = Uuid::new_v4().to_string();
        let mut initial = initial_state();
        initial.request_count = 1;
        let first = save(&root, &id, &initial).unwrap();
        let mut second = apply_patch(
            Some(&first),
            serde_json::from_str(r#"{"files":{"about.html":"<h1>About</h1>"}}"#).unwrap(),
        )
        .unwrap();
        second.request_count = 2;
        save(&root, &id, &second).unwrap();
        let restored = restore_revision(&root, &id, 1).unwrap();
        assert_eq!(restored.pages.len(), 1);
        assert_eq!(restored.request_count, 2);
        let relaunched = load(&root, &id).unwrap().unwrap();
        assert_eq!(relaunched.revision, 1);
        assert_eq!(relaunched.request_count, 2);
        assert_eq!(restore_revision(&root, &id, 2).unwrap().request_count, 2);
        let restored = restore_revision(&root, &id, 1).unwrap();
        let mut revised = apply_patch(
            Some(&restored),
            serde_json::from_str(r#"{"files":{"style.css":"body{color:gray}"}}"#).unwrap(),
        )
        .unwrap();
        revised.request_count = 3;
        let saved = save(&root, &id, &revised).unwrap();
        assert_eq!(saved.revision, 3);
        assert_eq!(load(&root, &id).unwrap().unwrap().css, "body{color:gray}");
        let history = list_revisions(&root, &id).unwrap();
        assert_eq!(
            history.iter().map(|item| item.revision).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert!(history[2].current);
        assert_eq!(
            load_revision(&site_dir(&root, &id).unwrap(), 2)
                .unwrap()
                .pages
                .len(),
            2
        );
        assert!(restore_revision(&root, &id, 99).is_err());
        assert_eq!(load(&root, &id).unwrap().unwrap().revision, 3);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_activation_leaves_no_poisoned_revision_and_can_retry() {
        let root = std::env::temp_dir().join(format!("bench-web-{}", Uuid::new_v4()));
        let id = Uuid::new_v4().to_string();
        let dir = site_dir(&root, &id).unwrap();
        fs::create_dir_all(dir.join("current.json")).unwrap();
        assert!(save(&root, &id, &initial_state()).is_err());
        assert!(!dir.join("revision-1").exists());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir(dir.join("current.json")).unwrap();
        assert_eq!(save(&root, &id, &initial_state()).unwrap().revision, 1);
        let mut invalid = initial_state();
        invalid.pages[0].path = "../escape.html".into();
        assert!(save(&root, &id, &invalid).is_err());
        assert_eq!(load(&root, &id).unwrap().unwrap().revision, 1);
        assert_eq!(list_revisions(&root, &id).unwrap().len(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn malformed_empty_and_oversized_generation_preserve_last_known_good() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let root = std::env::temp_dir().join(format!("bench-web-{}", Uuid::new_v4()));
        let id = Uuid::new_v4().to_string();
        let initial = save(&root, &id, &initial_state()).unwrap();
        for response in [
            "",
            "{broken",
            r#"{"files":{}}"#,
            r#"{"files":{"index.html":"  "}}"#,
        ] {
            assert!(runtime
                .block_on(generate(
                    &FakeProvider(response),
                    Some(&initial),
                    &["Revise".into()]
                ))
                .is_err());
            assert_eq!(
                load(&root, &id).unwrap().unwrap().pages[0].html,
                "<h1>Laundros</h1>"
            );
        }
        struct OversizedProvider;
        #[async_trait]
        impl ModelProvider for OversizedProvider {
            async fn stream_chat(
                &self,
                _: Vec<ProviderMessage>,
                deltas: mpsc::UnboundedSender<String>,
            ) -> Result<(), String> {
                deltas.send("x".repeat(400_001)).unwrap();
                Ok(())
            }
        }
        assert_eq!(
            runtime
                .block_on(generate(
                    &OversizedProvider,
                    Some(&initial),
                    &["Revise".into()]
                ))
                .unwrap_err(),
            "Website response is too large."
        );
        assert_eq!(list_revisions(&root, &id).unwrap().len(), 1);
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn rejects_workspace_directory_and_file_symlinks() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!("bench-web-{}", Uuid::new_v4()));
        let outside = std::env::temp_dir().join(format!("bench-outside-{}", Uuid::new_v4()));
        let id = Uuid::new_v4().to_string();
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let dir = root.join(&id);
        symlink(&outside, &dir).unwrap();
        assert!(save(&root, &id, &initial_state()).is_err());
        assert!(load(&root, &id).is_err());
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
        fs::remove_file(&dir).unwrap();
        save(&root, &id, &initial_state()).unwrap();
        let page = dir.join("revision-1/index.html");
        fs::write(outside.join("secret.html"), "private local content").unwrap();
        fs::remove_file(&page).unwrap();
        symlink(outside.join("secret.html"), &page).unwrap();
        assert!(load(&root, &id).is_err());
        assert!(restore_revision(&root, &id, 1).is_err());
        fs::remove_file(&page).unwrap();
        fs::write(&page, "<h1>Safe</h1>").unwrap();
        fs::remove_file(dir.join("current.json")).unwrap();
        symlink(outside.join("secret.html"), dir.join("current.json")).unwrap();
        assert!(load(&root, &id).is_err());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    #[test]
    fn reload_rejects_oversized_persisted_files_before_parsing() {
        let root = std::env::temp_dir().join(format!("bench-web-{}", Uuid::new_v4()));
        let id = Uuid::new_v4().to_string();
        save(&root, &id, &initial_state()).unwrap();
        let dir = site_dir(&root, &id).unwrap();
        fs::write(dir.join("revision-1/index.html"), "x".repeat(100_001)).unwrap();
        assert_eq!(
            load(&root, &id).unwrap_err(),
            "Website workspace file is too large."
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn corrupt_pointer_recovers_valid_history_without_mutating_or_consuming_requests() {
        let root = std::env::temp_dir().join(format!("bench-web-{}", Uuid::new_v4()));
        let id = Uuid::new_v4().to_string();
        let mut initial = initial_state();
        initial.request_count = 1;
        save(&root, &id, &initial).unwrap();
        let dir = site_dir(&root, &id).unwrap();
        fs::write(dir.join("current.json"), "{broken").unwrap();
        let recovered = load(&root, &id).unwrap().unwrap();
        assert_eq!(recovered.revision, 1);
        assert_eq!(recovered.request_count, 1);
        assert_eq!(
            fs::read_to_string(dir.join("current.json")).unwrap(),
            "{broken"
        );
        assert!(list_revisions(&root, &id).unwrap()[0].current);
        restore_revision(&root, &id, 1).unwrap();
        assert_eq!(load(&root, &id).unwrap().unwrap().revision, 1);
        fs::remove_file(dir.join("current.json")).unwrap();
        assert_eq!(load(&root, &id).unwrap().unwrap().revision, 1);
        assert!(!dir.join("current.json").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_or_corrupt_current_files_recover_last_known_good_history() {
        let root = std::env::temp_dir().join(format!("bench-web-{}", Uuid::new_v4()));
        let id = Uuid::new_v4().to_string();
        let mut initial = initial_state();
        initial.request_count = 1;
        save(&root, &id, &initial).unwrap();
        let mut second = initial.clone();
        second.request_count = 2;
        second.css = "body{color:gray}".into();
        save(&root, &id, &second).unwrap();
        let dir = site_dir(&root, &id).unwrap();
        let css = dir.join("revision-2/style.css");
        fs::remove_file(&css).unwrap();
        let recovered = load(&root, &id).unwrap().unwrap();
        assert_eq!(recovered.revision, 1);
        assert_eq!(recovered.request_count, 1);
        assert_eq!(list_revisions(&root, &id).unwrap().len(), 1);
        fs::write(&css, "<script>bad</script>").unwrap();
        assert_eq!(load(&root, &id).unwrap().unwrap().revision, 1);
        assert!(list_revisions(&root, &id).unwrap()[0].current);
        let restored = restore_revision(&root, &id, 1).unwrap();
        assert_eq!(restored.request_count, 1);
        let next = save(&root, &id, &initial).unwrap();
        assert_eq!(next.revision, 3);
        fs::remove_dir_all(root).unwrap();
    }
}
