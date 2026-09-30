//! OpenRouter media capabilities, separate from text streaming. Never logs keys
//! or upstream error bodies; only complete validated media leaves this module.
use crate::artifact::{ArtifactKind, ArtifactState};
use crate::provider::{ModelProvider, ProviderMessage};
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine};
use futures_util::StreamExt;
use reqwest::{redirect::Policy, Client, Response};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::mpsc;

const IMAGE_LIMIT: usize = 20 * 1024 * 1024;
const AUDIO_LIMIT: usize = 10 * 1024 * 1024;
const INPUT_LIMIT: usize = 32 * 1024;
// Read-only OpenRouter public catalogs verified September 30, 2026. These are
// independent capabilities; the user's text model is never silently reused.
pub const IMAGE_MODEL: &str = "openai/gpt-image-1-mini";
pub const VOICE_MODEL: &str = "microsoft/mai-voice-2";
pub const VOICE: &str = "en-US-Harper:MAI-Voice-2";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaAsset {
    pub mime_type: String,
    pub data_base64: String,
    pub model: String,
    pub generation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MediaContent {
    pub mime_type: String,
    pub data_base64: String,
    pub model: String,
    pub generation_id: Option<String>,
    pub prompt: String,
    pub voice: Option<String>,
}

impl MediaContent {
    pub fn from_asset(asset: MediaAsset, prompt: String, voice: Option<String>) -> Self {
        Self {
            mime_type: asset.mime_type,
            data_base64: asset.data_base64,
            model: asset.model,
            generation_id: asset.generation_id,
            prompt,
            voice,
        }
    }
}

pub fn validate(kind: ArtifactKind, content: &serde_json::Value) -> Result<(), String> {
    let media: MediaContent = serde_json::from_value(content.clone())
        .map_err(|_| "Media workspace is unreadable.".to_string())?;
    validate_input(&media.model, &media.prompt)?;
    if media.generation_id.as_ref().is_some_and(|id| {
        id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
    }) {
        return Err("Media metadata is invalid.".into());
    }
    let limit = match kind {
        ArtifactKind::Image => IMAGE_LIMIT,
        ArtifactKind::Voice => AUDIO_LIMIT,
        _ => return Err("This output does not contain media.".into()),
    };
    if media.data_base64.len() > (limit.div_ceil(3) * 4) {
        return Err("Generated media exceeds the size limit.".into());
    }
    let bytes = STANDARD
        .decode(&media.data_base64)
        .map_err(|_| "Media workspace contains invalid bytes.".to_string())?;
    if bytes.len() > limit {
        return Err("Generated media exceeds the size limit.".into());
    }
    let valid = match kind {
        ArtifactKind::Image => {
            media.voice.is_none() && image_header_valid(&bytes, &media.mime_type)
        }
        ArtifactKind::Voice => {
            media.mime_type == "audio/mpeg"
                && media.voice.as_ref().is_some_and(|voice| {
                    !voice.is_empty() && voice.len() <= 128 && !voice.chars().any(char::is_control)
                })
                && mp3_header_valid(&bytes)
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err("Media workspace contains invalid bytes.".into())
    }
}

pub fn export_bytes(state: &ArtifactState) -> Result<(String, Vec<u8>), String> {
    validate(state.kind, &state.content)?;
    let media: MediaContent = serde_json::from_value(state.content.clone())
        .map_err(|_| "Media workspace is unreadable.".to_string())?;
    let extension = match media.mime_type.as_str() {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "audio/mpeg" => "mp3",
        _ => return Err("Media export is unavailable.".into()),
    };
    let bytes = STANDARD
        .decode(media.data_base64)
        .map_err(|_| "Media workspace contains invalid bytes.".to_string())?;
    Ok((extension.into(), bytes))
}

pub struct OpenRouterMedia {
    client: Client,
    api_key: String,
    origin: String,
    uploads: Vec<crate::attachments::ImageInput>,
}

#[async_trait]
pub trait MediaProvider: Send + Sync {
    async fn image(
        &self,
        model: &str,
        prompt: &str,
        reference: Option<&MediaContent>,
    ) -> Result<MediaAsset, String>;
    async fn voice(&self, model: &str, script: &str, voice: &str) -> Result<MediaAsset, String>;
}

#[async_trait]
impl MediaProvider for OpenRouterMedia {
    async fn image(
        &self,
        model: &str,
        prompt: &str,
        reference: Option<&MediaContent>,
    ) -> Result<MediaAsset, String> {
        self.generate_image_with_reference(model, prompt, reference)
            .await
    }
    async fn voice(&self, model: &str, script: &str, voice: &str) -> Result<MediaAsset, String> {
        self.generate_voice(model, script, voice).await
    }
}

pub async fn generate(
    provider: &dyn MediaProvider,
    text: &dyn ModelProvider,
    kind: ArtifactKind,
    previous: Option<&ArtifactState>,
    requests: &[String],
) -> Result<ArtifactState, String> {
    if !matches!(kind, ArtifactKind::Image | ArtifactKind::Voice) {
        return Err("This output does not support media generation.".into());
    }
    if requests.is_empty() || requests.iter().any(|request| request.trim().is_empty()) {
        return Err("Save a request before generating media.".into());
    }
    if requests
        .iter()
        .fold(0usize, |size, request| size.saturating_add(request.len()))
        > INPUT_LIMIT
    {
        return Err(
            "Media request history is too large. Start a new conversation with the current brief."
                .into(),
        );
    }
    if let Some(previous) = previous {
        if previous.kind != kind || previous.version != 1 {
            return Err("Media workspace format is incompatible.".into());
        }
        validate(kind, &previous.content)?;
        if previous.request_count >= requests.len() {
            return Err(
                "This output is already up to date. Send a new request to revise it.".into(),
            );
        }
    }
    let previous_content = previous
        .map(|state| serde_json::from_value::<MediaContent>(state.content.clone()))
        .transpose()
        .map_err(|_| "Media workspace is unreadable.".to_string())?;
    let content = if kind == ArtifactKind::Image {
        let prompt = requests.join("\n\nRevision request:\n\n");
        validate_input(IMAGE_MODEL, &prompt)?;
        let asset = provider
            .image(IMAGE_MODEL, &prompt, previous_content.as_ref())
            .await?;
        MediaContent::from_asset(asset, prompt, None)
    } else {
        let messages = vec![
            ProviderMessage { role: "system".into(), content: "Write the finished spoken script requested by the user. Return only the words to speak, without Markdown, directions, commentary, or quotations around the entire script. Preserve text the user requests verbatim. Revise the previous script when present. Never invent business facts, names, hours, prices, or claims; use explicit placeholders. Keep the script under 8000 UTF-8 bytes.".into() },
            ProviderMessage { role: "user".into(), content: serde_json::json!({"requests":requests,"previous_script":previous_content.as_ref().map(|content| &content.prompt)}).to_string() },
        ];
        let (tx, mut rx) = mpsc::unbounded_channel();
        let stream = text.stream_chat(messages, tx);
        let collect = async {
            let mut script = String::new();
            while let Some(delta) = rx.recv().await {
                if script.len().saturating_add(delta.len()) > 8000 {
                    return Err("The generated voice script is too long.".to_string());
                }
                script.push_str(&delta);
            }
            Ok(script)
        };
        let (_, script) = futures_util::future::try_join(stream, collect)
            .await
            .map_err(|_| {
                "Could not create the voice script. Your previous audio is preserved.".to_string()
            })?;
        let script = script.trim().to_owned();
        validate_input(VOICE_MODEL, &script)?;
        let asset = provider.voice(VOICE_MODEL, &script, VOICE).await?;
        MediaContent::from_asset(asset, script, Some(VOICE.into()))
    };
    let content =
        serde_json::to_value(content).map_err(|_| "Could not save generated media.".to_string())?;
    validate(kind, &content)?;
    Ok(ArtifactState {
        version: 1,
        kind,
        revision: previous.map_or(Ok(1), |state| {
            state
                .revision
                .checked_add(1)
                .ok_or("Media history is full.")
        })?,
        request_count: requests.len(),
        content,
    })
}

impl OpenRouterMedia {
    pub fn new(api_key: String) -> Result<Self, String> {
        let client = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(12))
            .build()
            .map_err(|_| "Could not initialize media connection.".to_string())?;
        Ok(Self {
            client,
            api_key,
            origin: "https://openrouter.ai/api/v1".into(),
            uploads: Vec::new(),
        })
    }

    pub fn with_images(mut self, images: Vec<crate::attachments::ImageInput>) -> Self {
        self.uploads = images;
        self
    }

    #[cfg(test)]
    async fn generate_image(&self, model: &str, prompt: &str) -> Result<MediaAsset, String> {
        self.generate_image_with_reference(model, prompt, None)
            .await
    }

    async fn generate_image_with_reference(
        &self,
        model: &str,
        prompt: &str,
        reference: Option<&MediaContent>,
    ) -> Result<MediaAsset, String> {
        validate_input(model, prompt)?;
        let mut body = serde_json::json!({"model":model,"prompt":prompt,"n":1});
        let mut references = self.uploads.iter().map(|image| serde_json::json!({"type":"image_url","image_url":{"url":format!("data:{};base64,{}",image.mime_type,STANDARD.encode(&image.data))}})).collect::<Vec<_>>();
        if let Some(reference) = reference {
            validate(
                ArtifactKind::Image,
                &serde_json::to_value(reference).map_err(|_| "Image reference is unavailable.")?,
            )?;
            references.push(serde_json::json!({"type":"image_url","image_url":{"url":format!("data:{};base64,{}", reference.mime_type, reference.data_base64)}}));
        }
        if !references.is_empty() {
            body["input_references"] = serde_json::Value::Array(references);
        }
        let response = self
            .client
            .post(format!("{}/images", self.origin))
            .bearer_auth(&self.api_key)
            .timeout(Duration::from_secs(240))
            .json(&body)
            .send()
            .await
            .map_err(|_| "Could not reach image generation.".to_string())?;
        let generation_id = generation_id(&response);
        require_success(&response)?;
        require_mime(&response, "application/json")?;
        let bytes = bounded_body(response, IMAGE_LIMIT).await?;
        parse_image(&bytes, model, generation_id)
    }

    pub async fn generate_voice(
        &self,
        model: &str,
        input: &str,
        voice: &str,
    ) -> Result<MediaAsset, String> {
        validate_input(model, input)?;
        if voice.is_empty() || voice.len() > 128 || voice.chars().any(char::is_control) {
            return Err("Choose a supported voice.".into());
        }
        let response = self.client.post(format!("{}/audio/speech", self.origin))
            .bearer_auth(&self.api_key)
            .timeout(Duration::from_secs(120))
            .json(&serde_json::json!({"model":model,"input":input,"voice":voice,"response_format":"mp3"}))
            .send().await.map_err(|_| "Could not reach voice generation.".to_string())?;
        let generation_id = generation_id(&response);
        require_success(&response)?;
        require_mime(&response, "audio/mpeg")?;
        let bytes = bounded_body(response, AUDIO_LIMIT).await?;
        if !mp3_header_valid(&bytes) {
            return Err("Voice generation returned invalid MP3 audio.".into());
        }
        Ok(MediaAsset {
            mime_type: "audio/mpeg".into(),
            data_base64: STANDARD.encode(bytes),
            model: model.into(),
            generation_id,
        })
    }
}

fn validate_input(model: &str, content: &str) -> Result<(), String> {
    if model.is_empty()
        || model.len() > 256
        || !model.contains('/')
        || model.chars().any(char::is_control)
    {
        return Err("Choose a supported media model.".into());
    }
    if content.trim().is_empty() || content.len() > INPUT_LIMIT {
        return Err("Media input must be 1–32768 UTF-8 bytes.".into());
    }
    Ok(())
}

fn require_success(response: &Response) -> Result<(), String> {
    if response.status().is_success() {
        return Ok(());
    }
    match response.status().as_u16() {
        400 | 422 => Err("OpenRouter rejected this media request. Try a different prompt.".into()),
        401 | 403 => Err("Check your OpenRouter connection.".into()),
        402 => Err("OpenRouter could not fund this media request. Check your credits or key spending limit.".into()),
        404 => Err("This media model is currently unavailable on OpenRouter. Try again later.".into()),
        413 => Err("This media request is too large. Shorten the prompt or start a new conversation.".into()),
        500..=599 => Err("OpenRouter media generation is temporarily unavailable. Try again later.".into()),
        429 => Err("OpenRouter is rate limiting media requests. Try again later.".into()),
        _ => Err("OpenRouter could not complete media generation.".into()),
    }
}

fn require_mime(response: &Response, expected: &str) -> Result<(), String> {
    let actual = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .unwrap_or("");
    if actual.trim().eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err("Media generation returned an unexpected content type.".into())
    }
}

fn generation_id(response: &Response) -> Option<String> {
    response
        .headers()
        .get("x-generation-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 128
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
        })
        .map(str::to_owned)
}

async fn bounded_body(response: Response, limit: usize) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|size| size > limit as u64)
    {
        return Err("Generated media exceeds the size limit.".into());
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "Media generation stopped before finishing.".to_string())?;
        if chunk.len() > limit.saturating_sub(bytes.len()) {
            return Err("Generated media exceeds the size limit.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.is_empty() {
        return Err("Media generation returned an empty result.".into());
    }
    Ok(bytes)
}

#[derive(Deserialize)]
struct ImageResponse {
    data: Vec<ImageData>,
}
#[derive(Deserialize)]
struct ImageData {
    b64_json: String,
    media_type: String,
}

fn parse_image(
    body: &[u8],
    model: &str,
    generation_id: Option<String>,
) -> Result<MediaAsset, String> {
    let response: ImageResponse = serde_json::from_slice(body)
        .map_err(|_| "Image generation returned an invalid response.".to_string())?;
    if response.data.len() != 1 {
        return Err("Image generation must return exactly one image.".into());
    }
    let image = &response.data[0];
    if image.b64_json.len() > IMAGE_LIMIT {
        return Err("Generated media exceeds the size limit.".into());
    }
    let bytes = STANDARD
        .decode(&image.b64_json)
        .map_err(|_| "Image generation returned invalid image bytes.".to_string())?;
    if bytes.len() > IMAGE_LIMIT || !image_header_valid(&bytes, &image.media_type) {
        return Err("Image generation returned invalid image bytes.".into());
    }
    Ok(MediaAsset {
        mime_type: image.media_type.clone(),
        data_base64: STANDARD.encode(bytes),
        model: model.into(),
        generation_id,
    })
}

pub(crate) fn image_header_valid(bytes: &[u8], mime: &str) -> bool {
    let framing = match mime {
        "image/png" => {
            bytes.len() >= 45
                && bytes.starts_with(b"\x89PNG\r\n\x1a\n")
                && bytes.get(8..16) == Some(b"\0\0\0\rIHDR")
                && bytes.get(bytes.len() - 12..bytes.len() - 4) == Some(b"\0\0\0\0IEND")
        }
        "image/jpeg" => {
            bytes.len() >= 4 && bytes.starts_with(b"\xff\xd8\xff") && bytes.ends_with(b"\xff\xd9")
        }
        "image/webp" => {
            bytes.len() >= 20
                && bytes.starts_with(b"RIFF")
                && bytes.get(8..12) == Some(b"WEBP")
                && matches!(
                    bytes.get(12..16),
                    Some(b"VP8 ") | Some(b"VP8L") | Some(b"VP8X")
                )
                && u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize == bytes.len() - 8
        }
        _ => false,
    };
    if !framing || bytes.len() > IMAGE_LIMIT {
        return false;
    }
    let format = match mime {
        "image/png" => image::ImageFormat::Png,
        "image/jpeg" => image::ImageFormat::Jpeg,
        "image/webp" => image::ImageFormat::WebP,
        _ => return false,
    };
    let reader = || {
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(8192);
        limits.max_image_height = Some(8192);
        // The reader enforces decoded output allocation; codec-internal
        // allocation limits are documented as best-effort by image, not an
        // OS memory sandbox. Strict dimensions and our pixel cap also apply.
        limits.max_alloc = Some(64 * 1024 * 1024);
        let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
        reader.limits(limits);
        reader
    };
    let Ok((width, height)) = reader().into_dimensions() else {
        return false;
    };
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 16_777_216 {
        return false;
    }
    reader().decode().is_ok()
}

fn mp3_header_valid(bytes: &[u8]) -> bool {
    let mut offset = 0;
    if bytes.starts_with(b"ID3") {
        if bytes.len() < 10
            || !(2..=4).contains(&bytes[3])
            || bytes[6..10].iter().any(|byte| byte & 0x80 != 0)
        {
            return false;
        }
        let size = bytes[6..10]
            .iter()
            .fold(0usize, |size, byte| (size << 7) | *byte as usize);
        offset = 10 + size + usize::from(bytes[3] == 4 && bytes[5] & 0x10 != 0) * 10;
    }
    let Some(header) = bytes.get(offset..offset.saturating_add(4)) else {
        return false;
    };
    if header[0] != 0xff || header[1] & 0xe0 != 0xe0 || header[1] & 0x06 != 0x02 {
        return false;
    }
    let version = (header[1] >> 3) & 3;
    let bitrate_index = (header[2] >> 4) as usize;
    let sample_index = ((header[2] >> 2) & 3) as usize;
    if version == 1 || bitrate_index == 0 || bitrate_index == 15 || sample_index == 3 {
        return false;
    }
    let rates = if version == 3 {
        [
            0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
        ]
    } else {
        [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160]
    };
    let sample = [44100, 48000, 32000][sample_index]
        / match version {
            3 => 1,
            2 => 2,
            _ => 4,
        };
    let length = if version == 3 { 144000 } else { 72000 } * rates[bitrate_index] / sample
        + usize::from(header[2] & 2 != 0);
    bytes.len() - offset >= length
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };
    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYGD4DwABBAEAX+XDSwAAAABJRU5ErkJggg==";

    fn server(
        status: &str,
        mime: &str,
        body: Vec<u8>,
        declared: Option<usize>,
    ) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let status = status.to_owned();
        let mime = mime.to_owned();
        let task = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 4096];
            loop {
                let read = stream.read(&mut buffer).unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
                if let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..end]).to_lowercase();
                    let length: usize = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length:").map(str::trim))
                        .unwrap()
                        .parse()
                        .unwrap();
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let length_header = if declared == Some(usize::MAX) {
                "Transfer-Encoding: chunked".into()
            } else {
                format!("Content-Length: {}", declared.unwrap_or(body.len()))
            };
            let response = format!("HTTP/1.1 {status}\r\nContent-Type: {mime}\r\n{length_header}\r\nX-Generation-Id: gen-test\r\nConnection: close\r\n\r\n");
            let _ = stream.write_all(response.as_bytes());
            if declared == Some(usize::MAX) {
                let _ = write!(stream, "{:x}\r\n", body.len());
                let _ = stream.write_all(&body);
                let _ = stream.write_all(b"\r\n0\r\n\r\n");
            } else {
                let _ = stream.write_all(&body);
            }
            String::from_utf8(request).unwrap()
        });
        (origin, task)
    }
    fn client(origin: String) -> OpenRouterMedia {
        let mut client = OpenRouterMedia::new("test-secret".into()).unwrap();
        client.origin = origin;
        client
    }
    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }
    fn mp3() -> Vec<u8> {
        let mut bytes = vec![0; 417];
        bytes[..4].copy_from_slice(&[0xff, 0xfb, 0x90, 0]);
        bytes
    }

    #[test]
    fn image_request_and_raster_response_are_exact() {
        let body = serde_json::json!({"data":[{"b64_json":PNG,"media_type":"image/png"}]})
            .to_string()
            .into_bytes();
        let (origin, task) = server("200 OK", "application/json", body, None);
        let asset = runtime()
            .block_on(client(origin).generate_image("test/image", "draw a tree"))
            .unwrap();
        assert_eq!(asset.data_base64, PNG);
        assert_eq!(asset.generation_id.as_deref(), Some("gen-test"));
        let request = task.join().unwrap();
        assert!(request.starts_with("POST /images HTTP/1.1"));
        assert!(request
            .to_lowercase()
            .contains("authorization: bearer test-secret"));
        let json: serde_json::Value =
            serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"model":"test/image","prompt":"draw a tree","n":1})
        );
    }

    #[test]
    fn speech_request_and_binary_response_are_exact() {
        let audio = mp3();
        let (origin, task) = server("200 OK", "audio/mpeg", audio.clone(), None);
        let asset = runtime()
            .block_on(client(origin).generate_voice("test/speech", "Hello", "sample-voice"))
            .unwrap();
        assert_eq!(STANDARD.decode(asset.data_base64).unwrap(), audio);
        let request = task.join().unwrap();
        assert!(request.starts_with("POST /audio/speech HTTP/1.1"));
        let json: serde_json::Value =
            serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"model":"test/speech","input":"Hello","voice":"sample-voice","response_format":"mp3"})
        );
    }

    #[test]
    fn response_failures_never_surface_upstream_secrets() {
        for (status, mime, body, declared) in [
            (
                "401 Unauthorized",
                "application/json",
                b"test-secret private".to_vec(),
                None,
            ),
            (
                "302 Found\r\nLocation: http://127.0.0.1:9/forbidden",
                "audio/mpeg",
                vec![],
                None,
            ),
            (
                "402 Payment Required",
                "application/json",
                b"test-secret".to_vec(),
                None,
            ),
            (
                "400 Bad Request",
                "application/json",
                b"test-secret".to_vec(),
                None,
            ),
            (
                "404 Not Found",
                "application/json",
                b"test-secret".to_vec(),
                None,
            ),
            (
                "413 Payload Too Large",
                "application/json",
                b"test-secret".to_vec(),
                None,
            ),
            (
                "502 Bad Gateway",
                "application/json",
                b"test-secret".to_vec(),
                None,
            ),
            ("200 OK", "text/html", b"test-secret".to_vec(), None),
            ("200 OK", "audio/mpeg", mp3(), Some(AUDIO_LIMIT + 1)),
            ("200 OK", "audio/mpeg", mp3(), Some(1000)),
            ("200 OK", "audio/mpeg", b"ID3".to_vec(), None),
        ] {
            let (origin, task) = server(status, mime, body, declared);
            let error = runtime()
                .block_on(client(origin).generate_voice("test/speech", "Hello", "voice"))
                .unwrap_err();
            assert!(!error.contains("test-secret"));
            if status.starts_with("302") {
                assert_eq!(error, "OpenRouter could not complete media generation.");
            }
            let expected = match status.split_whitespace().next().unwrap() {
                "402" => Some("OpenRouter could not fund this media request. Check your credits or key spending limit."),
                "400" => Some("OpenRouter rejected this media request. Try a different prompt."),
                "404" => Some("This media model is currently unavailable on OpenRouter. Try again later."),
                "413" => Some("This media request is too large. Shorten the prompt or start a new conversation."),
                "502" => Some("OpenRouter media generation is temporarily unavailable. Try again later."),
                _ => None,
            };
            if let Some(expected) = expected {
                assert_eq!(error, expected);
            }
            task.join().unwrap();
        }
    }

    #[test]
    fn chunked_body_without_advertised_length_is_bounded() {
        let (origin, task) = server("200 OK", "audio/mpeg", b"1234".to_vec(), Some(usize::MAX));
        let result = runtime().block_on(async {
            let response = Client::new()
                .post(&origin)
                .json(&serde_json::json!({}))
                .send()
                .await
                .unwrap();
            assert_eq!(response.content_length(), None);
            bounded_body(response, 3).await
        });
        assert_eq!(
            result.unwrap_err(),
            "Generated media exceeds the size limit."
        );
        task.join().unwrap();
    }

    #[test]
    fn rejects_malformed_empty_svg_and_mime_mismatched_images() {
        for body in [b"not json".to_vec(), b"{\"data\":[]}".to_vec(), serde_json::json!({"data":[{"b64_json":PNG,"media_type":"image/jpeg"}]}).to_string().into_bytes(), serde_json::json!({"data":[{"b64_json":STANDARD.encode(b"<svg/>"),"media_type":"image/svg+xml"}]}).to_string().into_bytes(), serde_json::json!({"data":[{"b64_json":"notbase64!","media_type":"image/png"}]}).to_string().into_bytes()] {
            assert!(parse_image(&body,"test/image",None).is_err());
        }
    }

    #[test]
    fn validates_mp3_tag_and_frame_lengths_not_only_magic() {
        assert!(mp3_header_valid(&mp3()));
        assert!(!mp3_header_valid(&mp3()[..100]));
        let mut tagged = b"ID3\x04\0\0\0\0\0\0".to_vec();
        tagged.extend(mp3());
        assert!(mp3_header_valid(&tagged));
        tagged[6] = 0x80;
        assert!(!mp3_header_valid(&tagged));
        assert!(!mp3_header_valid(b"<html>"));
    }

    #[test]
    fn input_bounds_reject_before_any_transport() {
        assert!(validate_input("test/image", "").is_err());
        assert!(validate_input("test/image", &"a".repeat(INPUT_LIMIT + 1)).is_err());
        assert!(validate_input("secret\nheader", "hello").is_err());
        assert!(validate_input("test/image", &"a".repeat(INPUT_LIMIT)).is_ok());
    }

    struct FakeText;
    #[async_trait]
    impl ModelProvider for FakeText {
        async fn stream_chat(
            &self,
            messages: Vec<ProviderMessage>,
            deltas: mpsc::UnboundedSender<String>,
        ) -> Result<(), String> {
            assert!(messages[1].content.contains("requests"));
            deltas.send("Welcome to Bench.".into()).unwrap();
            Ok(())
        }
    }
    struct FakeMedia {
        fail: bool,
        references: std::sync::Mutex<Vec<bool>>,
    }
    #[async_trait]
    impl MediaProvider for FakeMedia {
        async fn image(
            &self,
            model: &str,
            _prompt: &str,
            reference: Option<&MediaContent>,
        ) -> Result<MediaAsset, String> {
            self.references.lock().unwrap().push(reference.is_some());
            if self.fail {
                return Err("Image generation failed.".into());
            }
            Ok(MediaAsset {
                mime_type: "image/png".into(),
                data_base64: PNG.into(),
                model: model.into(),
                generation_id: Some("gen-fixture".into()),
            })
        }
        async fn voice(
            &self,
            model: &str,
            script: &str,
            voice: &str,
        ) -> Result<MediaAsset, String> {
            assert_eq!(script, "Welcome to Bench.");
            assert_eq!(voice, VOICE);
            if self.fail {
                return Err("Voice generation failed.".into());
            }
            Ok(MediaAsset {
                mime_type: "audio/mpeg".into(),
                data_base64: STANDARD.encode(mp3()),
                model: model.into(),
                generation_id: None,
            })
        }
    }

    #[test]
    fn media_generation_revisions_persist_export_and_recover_without_paid_calls() {
        for kind in [ArtifactKind::Image, ArtifactKind::Voice] {
            let root = std::env::temp_dir().join(format!("bench-media-{}", uuid::Uuid::new_v4()));
            let id = uuid::Uuid::new_v4().to_string();
            let media = FakeMedia {
                fail: false,
                references: std::sync::Mutex::new(vec![]),
            };
            let requests = vec!["Create a welcome output".to_owned()];
            let first = runtime()
                .block_on(generate(&media, &FakeText, kind, None, &requests))
                .unwrap();
            let first = crate::artifact::save(&root, &id, &first, validate).unwrap();
            let mut revised_requests = requests.clone();
            revised_requests.push("Make it warmer".into());
            let revised = runtime()
                .block_on(generate(
                    &media,
                    &FakeText,
                    kind,
                    Some(&first),
                    &revised_requests,
                ))
                .unwrap();
            let revised = crate::artifact::save(&root, &id, &revised, validate).unwrap();
            let loaded = crate::artifact::load(&root, &id, kind, validate)
                .unwrap()
                .unwrap();
            assert_eq!(loaded.revision, 2);
            assert_eq!(loaded.request_count, 2);
            let (extension, bytes) = export_bytes(&loaded).unwrap();
            assert_eq!(
                extension,
                if kind == ArtifactKind::Image {
                    "png"
                } else {
                    "mp3"
                }
            );
            assert_eq!(
                bytes,
                if kind == ArtifactKind::Image {
                    STANDARD.decode(PNG).unwrap()
                } else {
                    mp3()
                }
            );
            assert!(runtime()
                .block_on(generate(
                    &media,
                    &FakeText,
                    kind,
                    Some(&revised),
                    &revised_requests
                ))
                .is_err());
            revised_requests.push("Another change".into());
            let failed = FakeMedia {
                fail: true,
                references: std::sync::Mutex::new(vec![]),
            };
            assert!(runtime()
                .block_on(generate(
                    &failed,
                    &FakeText,
                    kind,
                    Some(&revised),
                    &revised_requests
                ))
                .is_err());
            assert_eq!(
                crate::artifact::load(&root, &id, kind, validate)
                    .unwrap()
                    .unwrap()
                    .revision,
                2
            );
            let restored =
                crate::artifact::restore_revision(&root, &id, kind, 1, validate).unwrap();
            assert_eq!(restored.request_count, 2);
            if kind == ArtifactKind::Image {
                assert_eq!(*media.references.lock().unwrap(), vec![false, true]);
            }
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn image_revision_transmits_only_validated_generated_reference() {
        let body = serde_json::json!({"data":[{"b64_json":PNG,"media_type":"image/png"}]})
            .to_string()
            .into_bytes();
        let (origin, task) = server("200 OK", "application/json", body, None);
        let reference = MediaContent {
            mime_type: "image/png".into(),
            data_base64: PNG.into(),
            model: IMAGE_MODEL.into(),
            generation_id: None,
            prompt: "Original".into(),
            voice: None,
        };
        runtime()
            .block_on(client(origin).generate_image_with_reference(
                IMAGE_MODEL,
                "Make it blue",
                Some(&reference),
            ))
            .unwrap();
        let request = task.join().unwrap();
        let json: serde_json::Value =
            serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(
            json["input_references"][0]["image_url"]["url"],
            format!("data:image/png;base64,{PNG}")
        );
    }

    #[test]
    fn saved_media_validation_rejects_kind_confusion_unknown_fields_and_corrupt_bytes() {
        let mut content = serde_json::to_value(MediaContent {
            mime_type: "image/png".into(),
            data_base64: PNG.into(),
            model: IMAGE_MODEL.into(),
            generation_id: None,
            prompt: "Original".into(),
            voice: None,
        })
        .unwrap();
        assert!(validate(ArtifactKind::Image, &content).is_ok());
        assert!(validate(ArtifactKind::Voice, &content).is_err());
        content["apiKey"] = serde_json::json!("must not be stored");
        assert!(validate(ArtifactKind::Image, &content).is_err());
        content.as_object_mut().unwrap().remove("apiKey");
        content["dataBase64"] = serde_json::json!(STANDARD.encode(b"<script>alert(1)</script>"));
        assert!(validate(ArtifactKind::Image, &content).is_err());
    }

    #[test]
    fn full_raster_decode_rejects_valid_magic_but_missing_or_corrupt_payload() {
        assert!(!image_header_valid(&[0xff, 0xd8, 0xff, 0xd9], "image/jpeg"));
        let mut fake_webp = b"RIFF\x0c\0\0\0WEBPVP8 \0\0\0\0".to_vec();
        assert!(!image_header_valid(&fake_webp, "image/webp"));
        fake_webp[16] = 1;
        assert!(!image_header_valid(&fake_webp, "image/webp"));
        let mut png = STANDARD.decode(PNG).unwrap();
        png[16..20].copy_from_slice(&9000u32.to_be_bytes());
        assert!(!image_header_valid(&png, "image/png"));
        let mut png = STANDARD.decode(PNG).unwrap();
        png[45] ^= 0xff;
        assert!(!image_header_valid(&png, "image/png"));
    }
}
