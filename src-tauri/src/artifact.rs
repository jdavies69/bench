//! Immutable, validated output metadata. Binary media belongs in a separately
//! validated store; this JSON envelope contains no executable host commands.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const UNSAFE: &str = "Output workspace contains an unsafe path.";
const MAX_BYTES: usize = 500_000;
fn max_bytes(kind: ArtifactKind) -> usize {
    match kind {
        ArtifactKind::Image => 28 * 1024 * 1024,
        ArtifactKind::Voice => 14 * 1024 * 1024,
        _ => MAX_BYTES,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactKind {
    Document,
    Presentation,
    Application,
    Image,
    Voice,
    Agent,
}

impl ArtifactKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::Presentation => "presentation",
            Self::Application => "application",
            Self::Image => "image",
            Self::Voice => "voice",
            Self::Agent => "agent",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactState {
    pub version: u32,
    pub kind: ArtifactKind,
    pub revision: u32,
    pub request_count: usize,
    pub content: Value,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactRevision {
    pub revision: u32,
    pub current: bool,
}

pub type ContentValidator = fn(ArtifactKind, &Value) -> Result<(), String>;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Pointer {
    version: u32,
    revision: u32,
    request_count: usize,
}

fn directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(UNSAFE.into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("Could not read the output workspace.".into()),
    }
}

fn workspace(root: &Path, id: &str, kind: ArtifactKind) -> Result<PathBuf, String> {
    let parsed = uuid::Uuid::parse_str(id).map_err(|_| "Output workspace could not be found.")?;
    if parsed.to_string() != id {
        return Err("Output workspace could not be found.".into());
    }
    directory(root)?;
    let conversation = root.join(id);
    directory(&conversation)?;
    let path = conversation.join(kind.as_str());
    directory(&path)?;
    Ok(path)
}

fn read(path: &Path, max: usize) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "Could not read the output revision.")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(UNSAFE.into());
    }
    if metadata.len() > max as u64 {
        return Err("Output workspace file is too large.".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .and_then(|file| file.take(max as u64 + 1).read_to_end(&mut bytes))
        .map_err(|_| "Could not read the output revision.")?;
    if bytes.len() > max {
        return Err("Output workspace file is too large.".into());
    }
    Ok(bytes)
}

fn numbers(dir: &Path) -> Result<Vec<u32>, String> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(_) => return Err("Could not read output history.".into()),
    };
    let mut values = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| "Could not read output history.")?;
        if let Some(number) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_prefix("revision-"))
            .and_then(|number| number.parse().ok())
        {
            values.push(number);
        }
    }
    values.sort_unstable();
    Ok(values)
}

fn revision(
    dir: &Path,
    kind: ArtifactKind,
    number: u32,
    validate: ContentValidator,
) -> Result<ArtifactState, String> {
    let path = dir.join(format!("revision-{number}"));
    directory(&path)?;
    let state: ArtifactState =
        serde_json::from_slice(&read(&path.join("state.json"), max_bytes(kind))?)
            .map_err(|_| "Output revision is unreadable.")?;
    if state.version != 1 || state.kind != kind || state.revision != number || number == 0 {
        return Err("Output revision metadata is invalid.".into());
    }
    validate(kind, &state.content)?;
    Ok(state)
}

pub fn load(
    root: &Path,
    id: &str,
    kind: ArtifactKind,
    validate: ContentValidator,
) -> Result<Option<ArtifactState>, String> {
    let dir = workspace(root, id, kind)?;
    let pointer_path = dir.join("current.json");
    let mut failure = None;
    match fs::symlink_metadata(&pointer_path) {
        Ok(_) => {
            let pointer = read(&pointer_path, 1000).and_then(|bytes| {
                serde_json::from_slice::<Pointer>(&bytes)
                    .map_err(|_| "Output metadata is unreadable.".into())
            });
            match pointer {
                Ok(pointer) if pointer.version == 1 => {
                    match revision(&dir, kind, pointer.revision, validate) {
                        Ok(mut state) => {
                            state.request_count = pointer.request_count;
                            return Ok(Some(state));
                        }
                        Err(error) if error == UNSAFE => return Err(error),
                        Err(error) => failure = Some(error),
                    }
                }
                Ok(_) => failure = Some("Output metadata is invalid.".into()),
                Err(error) if error == UNSAFE => return Err(error),
                Err(error) => failure = Some(error),
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("Could not read output metadata.".into()),
    }
    let revisions = numbers(&dir)?;
    for number in revisions.iter().rev() {
        if let Ok(state) = revision(&dir, kind, *number, validate) {
            return Ok(Some(state));
        }
    }
    if let Some(error) = failure {
        return Err(error);
    }
    if !revisions.is_empty() {
        return Err("No readable output revision is available.".into());
    }
    Ok(None)
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| "Could not save the output revision.")?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| "Could not save the output revision.".into())
}

fn activate(dir: &Path, number: u32, count: usize) -> Result<(), String> {
    let temp = dir.join(format!(".current-{}.json", uuid::Uuid::new_v4()));
    let result = (|| {
        let bytes = serde_json::to_vec(&Pointer {
            version: 1,
            revision: number,
            request_count: count,
        })
        .map_err(|_| "Could not save output metadata.")?;
        write(&temp, &bytes)?;
        fs::rename(&temp, dir.join("current.json"))
            .map_err(|_| "Could not activate the output revision.".into())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

pub fn save(
    root: &Path,
    id: &str,
    state: &ArtifactState,
    validate: ContentValidator,
) -> Result<ArtifactState, String> {
    if state.version != 1 {
        return Err("Output format is unsupported.".into());
    }
    validate(state.kind, &state.content)?;
    let dir = workspace(root, id, state.kind)?;
    fs::create_dir_all(&dir).map_err(|_| "Could not create the output workspace.")?;
    let mut saved = state.clone();
    saved.revision = numbers(&dir)?
        .last()
        .copied()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or("Output history is full.")?;
    let bytes = serde_json::to_vec(&saved).map_err(|_| "Could not save output metadata.")?;
    if bytes.len() > max_bytes(state.kind) {
        return Err("Output revision is too large.".into());
    }
    let staging = dir.join(format!(".revision-{}", uuid::Uuid::new_v4()));
    let final_dir = dir.join(format!("revision-{}", saved.revision));
    fs::create_dir(&staging).map_err(|_| "Could not save the output revision.")?;
    let result = (|| {
        write(&staging.join("state.json"), &bytes)?;
        fs::rename(&staging, &final_dir).map_err(|_| "Could not save the output revision.")?;
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

pub fn list_revisions(
    root: &Path,
    id: &str,
    kind: ArtifactKind,
    validate: ContentValidator,
) -> Result<Vec<ArtifactRevision>, String> {
    let dir = workspace(root, id, kind)?;
    let active = load(root, id, kind, validate)?.map(|state| state.revision);
    Ok(numbers(&dir)?
        .into_iter()
        .filter(|number| revision(&dir, kind, *number, validate).is_ok())
        .map(|number| ArtifactRevision {
            revision: number,
            current: active == Some(number),
        })
        .collect())
}

pub fn restore_revision(
    root: &Path,
    id: &str,
    kind: ArtifactKind,
    number: u32,
    validate: ContentValidator,
) -> Result<ArtifactState, String> {
    let dir = workspace(root, id, kind)?;
    let current = load(root, id, kind, validate)?.ok_or("Output workspace could not be found.")?;
    let mut restored = revision(&dir, kind, number, validate)?;
    restored.request_count = current.request_count;
    activate(&dir, number, restored.request_count)?;
    Ok(restored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text_outputs;

    fn root() -> PathBuf {
        std::env::temp_dir().join(format!("bench-artifact-{}", uuid::Uuid::new_v4()))
    }
    fn state(count: usize) -> ArtifactState {
        ArtifactState {
            version: 1,
            kind: ArtifactKind::Document,
            revision: 1,
            request_count: count,
            content: serde_json::json!({"title":"Brief","markdown":format!("Saved request {count}")}),
        }
    }

    #[test]
    fn immutable_revisions_restore_without_replaying_processed_requests() {
        let root = root();
        let id = uuid::Uuid::new_v4().to_string();
        let first = save(&root, &id, &state(1), text_outputs::validate).unwrap();
        let second = save(&root, &id, &state(2), text_outputs::validate).unwrap();
        assert_eq!(second.revision, 2);
        let restored = restore_revision(
            &root,
            &id,
            ArtifactKind::Document,
            1,
            text_outputs::validate,
        )
        .unwrap();
        assert_eq!(restored.content, first.content);
        assert_eq!(restored.request_count, 2);
        let relaunched = load(&root, &id, ArtifactKind::Document, text_outputs::validate)
            .unwrap()
            .unwrap();
        assert_eq!(relaunched.revision, 1);
        assert_eq!(relaunched.request_count, 2);
        let next = save(&root, &id, &state(3), text_outputs::validate).unwrap();
        assert_eq!(next.revision, 3);
        let history =
            list_revisions(&root, &id, ArtifactKind::Document, text_outputs::validate).unwrap();
        assert_eq!(
            history.iter().map(|item| item.revision).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert!(history[2].current);
        assert!(restore_revision(
            &root,
            &id,
            ArtifactKind::Document,
            99,
            text_outputs::validate
        )
        .is_err());
        assert_eq!(
            load(&root, &id, ArtifactKind::Document, text_outputs::validate)
                .unwrap()
                .unwrap()
                .revision,
            3
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrupt_pointer_and_latest_revision_recover_valid_history_without_consuming_requests() {
        let root = root();
        let id = uuid::Uuid::new_v4().to_string();
        save(&root, &id, &state(1), text_outputs::validate).unwrap();
        save(&root, &id, &state(2), text_outputs::validate).unwrap();
        let dir = workspace(&root, &id, ArtifactKind::Document).unwrap();
        fs::write(dir.join("current.json"), "{broken").unwrap();
        assert_eq!(
            load(&root, &id, ArtifactKind::Document, text_outputs::validate)
                .unwrap()
                .unwrap()
                .request_count,
            2
        );
        assert_eq!(
            fs::read_to_string(dir.join("current.json")).unwrap(),
            "{broken"
        );
        fs::write(
            dir.join("revision-2/state.json"),
            serde_json::to_vec(&ArtifactState {
                revision: 2,
                content: serde_json::json!({"title":"Invalid","markdown":"","extra":"invalid"}),
                ..state(2)
            })
            .unwrap(),
        )
        .unwrap();
        let recovered = load(&root, &id, ArtifactKind::Document, text_outputs::validate)
            .unwrap()
            .unwrap();
        assert_eq!(recovered.revision, 1);
        assert_eq!(recovered.request_count, 1);
        assert_eq!(
            list_revisions(&root, &id, ArtifactKind::Document, text_outputs::validate)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            save(&root, &id, &state(3), text_outputs::validate)
                .unwrap()
                .revision,
            3
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_activation_can_retry_without_poisoned_revision() {
        let root = root();
        let id = uuid::Uuid::new_v4().to_string();
        let dir = workspace(&root, &id, ArtifactKind::Document).unwrap();
        fs::create_dir_all(dir.join("current.json")).unwrap();
        assert!(save(&root, &id, &state(1), text_outputs::validate).is_err());
        assert!(!dir.join("revision-1").exists());
        fs::remove_dir(dir.join("current.json")).unwrap();
        assert_eq!(
            save(&root, &id, &state(1), text_outputs::validate)
                .unwrap()
                .revision,
            1
        );
        let mut invalid = state(2);
        invalid.kind = ArtifactKind::Presentation;
        assert!(save(&root, &id, &invalid, text_outputs::validate).is_err());
        assert!(load(
            &root,
            "../escape",
            ArtifactKind::Document,
            text_outputs::validate
        )
        .is_err());
        assert!(load(
            &root,
            &id.replace('-', ""),
            ArtifactKind::Document,
            text_outputs::validate
        )
        .is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn oversized_persisted_revision_is_bounded_before_json_parsing() {
        let root = root();
        let id = uuid::Uuid::new_v4().to_string();
        save(&root, &id, &state(1), text_outputs::validate).unwrap();
        let dir = workspace(&root, &id, ArtifactKind::Document).unwrap();
        fs::write(dir.join("revision-1/state.json"), vec![b'x'; MAX_BYTES + 1]).unwrap();
        assert!(
            load(&root, &id, ArtifactKind::Document, text_outputs::validate)
                .unwrap_err()
                .contains("too large")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_directory_revision_and_pointer_symlinks_without_reading_outside_content() {
        use std::os::unix::fs::symlink;
        let root = root();
        let outside = root.with_extension("outside");
        let id = uuid::Uuid::new_v4().to_string();
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, root.join(&id)).unwrap();
        assert!(save(&root, &id, &state(1), text_outputs::validate).is_err());
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
        fs::remove_file(root.join(&id)).unwrap();
        save(&root, &id, &state(1), text_outputs::validate).unwrap();
        let dir = workspace(&root, &id, ArtifactKind::Document).unwrap();
        fs::write(
            outside.join("private"),
            serde_json::to_vec(&state(1)).unwrap(),
        )
        .unwrap();
        fs::remove_file(dir.join("revision-1/state.json")).unwrap();
        symlink(outside.join("private"), dir.join("revision-1/state.json")).unwrap();
        assert_eq!(
            load(&root, &id, ArtifactKind::Document, text_outputs::validate).unwrap_err(),
            UNSAFE
        );
        assert!(restore_revision(
            &root,
            &id,
            ArtifactKind::Document,
            1,
            text_outputs::validate
        )
        .is_err());
        fs::remove_file(dir.join("current.json")).unwrap();
        symlink(outside.join("private"), dir.join("current.json")).unwrap();
        assert_eq!(
            load(&root, &id, ArtifactKind::Document, text_outputs::validate).unwrap_err(),
            UNSAFE
        );
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }
}
