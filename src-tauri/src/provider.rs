use async_trait::async_trait;
use futures_util::{
    future::{select, Either},
    StreamExt,
};
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

const MAX_STREAM_LINE: usize = 256_000;
const MAX_STREAM_EVENT: usize = 512_000;
const MAX_STREAM_BYTES: usize = 4_000_000;
const MAX_RESPONSE_BYTES: usize = 400_000;

fn oversized_response() -> String {
    "The response was too large. Retry with a shorter request.".into()
}

fn closed_consumer() -> String {
    "The response was interrupted. Retry when ready.".into()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderId {
    OpenRouter,
    OpenAI,
    Anthropic,
    Xai,
}

impl ProviderId {
    pub const ALL: [Self; 4] = [Self::OpenRouter, Self::OpenAI, Self::Anthropic, Self::Xai];

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "openrouter" => Ok(Self::OpenRouter),
            "openai" => Ok(Self::OpenAI),
            "anthropic" => Ok(Self::Anthropic),
            "xai" => Ok(Self::Xai),
            _ => Err("Unknown model provider.".into()),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenRouter => "openrouter",
            Self::OpenAI => "openai",
            Self::Anthropic => "anthropic",
            Self::Xai => "xai",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::OpenRouter => "OpenRouter",
            Self::OpenAI => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::Xai => "xAI / Grok",
        }
    }

    pub fn default_model(self) -> &'static str {
        match self {
            Self::OpenRouter => "openrouter/auto",
            Self::OpenAI => "gpt-5.4-mini",
            Self::Anthropic => "claude-sonnet-5-5",
            Self::Xai => "grok-4.7",
        }
    }

    pub fn environment_key(self) -> Option<String> {
        let name = match self {
            Self::OpenRouter => "OPENROUTER_API_KEY",
            Self::OpenAI => "OPENAI_API_KEY",
            Self::Anthropic => "ANTHROPIC_API_KEY",
            Self::Xai => "XAI_API_KEY",
        };
        std::env::var(name)
            .ok()
            .filter(|key| !key.trim().is_empty())
    }

    fn endpoint(self) -> &'static str {
        match self {
            Self::OpenRouter => "https://openrouter.ai/api/v1/chat/completions",
            Self::OpenAI => "https://api.openai.com/v1/chat/completions",
            Self::Anthropic => "https://api.anthropic.com/v1/messages",
            Self::Xai => "https://api.x.ai/v1/chat/completions",
        }
    }
}

#[derive(Clone, Serialize)]
pub struct ProviderMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub id: String,
    pub label: String,
    pub model: String,
    pub key_source: String,
}

#[async_trait]
pub trait ModelProvider: Send + Sync {
    async fn stream_chat(
        &self,
        messages: Vec<ProviderMessage>,
        deltas: UnboundedSender<String>,
    ) -> Result<(), String>;
}

/// Evidence returned by a provider-hosted search tool. The model's prose and
/// usage count are not accepted as evidence of a usable source.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchEvidence {
    pub requests: u64,
    pub source_urls: Vec<String>,
}

impl SearchEvidence {
    pub fn searched(&self) -> bool {
        !self.source_urls.is_empty()
    }
}

pub struct HttpProvider {
    kind: ProviderId,
    client: reqwest::Client,
    endpoint: String,
    api_key: String,
    model: String,
}

impl HttpProvider {
    pub fn new(kind: ProviderId, api_key: String, model: String) -> Self {
        Self::with_timeout(kind, api_key, model, Duration::from_secs(120))
    }

    pub fn for_website(kind: ProviderId, api_key: String, model: String) -> Self {
        Self::with_timeout(kind, api_key, model, Duration::from_secs(240))
    }

    fn with_timeout(kind: ProviderId, api_key: String, model: String, timeout: Duration) -> Self {
        Self {
            kind,
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(12))
                .timeout(timeout)
                .build()
                .expect("HTTP client configuration is valid"),
            endpoint: kind.endpoint().into(),
            api_key,
            model,
        }
    }

    /// The server tool belongs to the OpenRouter adapter. Other model
    /// providers keep using the provider-independent chat interface.
    pub async fn stream_chat_with_web_search(
        &self,
        messages: Vec<ProviderMessage>,
        deltas: UnboundedSender<String>,
        require_sources: bool,
    ) -> Result<SearchEvidence, String> {
        if self.kind != ProviderId::OpenRouter {
            return Err("Web search is available when OpenRouter is selected in Settings.".into());
        }
        self.stream_chat_inner(messages, deltas, true, require_sources)
            .await
    }

    async fn stream_chat_inner(
        &self,
        messages: Vec<ProviderMessage>,
        deltas: UnboundedSender<String>,
        web_search: bool,
        require_sources: bool,
    ) -> Result<SearchEvidence, String> {
        if deltas.is_closed() {
            return Err(closed_consumer());
        }
        let mut body = if self.kind == ProviderId::Anthropic {
            let system = messages
                .iter()
                .filter(|message| message.role == "system")
                .map(|message| message.content.as_str())
                .collect::<Vec<_>>()
                .join("\n\n");
            let messages = messages
                .into_iter()
                .filter(|message| message.role != "system")
                .collect::<Vec<_>>();
            serde_json::json!({
                "model": self.model,
                "messages": messages,
                "system": system,
                "max_tokens": 4096,
                "stream": true
            })
        } else {
            serde_json::json!({
                "model": self.model,
                "messages": messages,
                "stream": true
            })
        };
        if web_search {
            body["tools"] = serde_json::json!([{
                "type": "openrouter:web_search",
                "parameters": {"max_uses": 3, "max_results": 5, "max_total_results": 12}
            }]);
            // max_uses is ignored by some native search providers. This is the
            // OpenRouter server-tool loop's own hard limit.
            body["max_tool_calls"] = serde_json::json!(3);
            body["stream_options"] = serde_json::json!({"include_usage": true});
        }
        let request = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&body);
        let request = if self.kind == ProviderId::Anthropic {
            request.header("anthropic-version", "2023-06-01")
        } else {
            request
        };
        let response = match select(Box::pin(request.send()), Box::pin(deltas.closed())).await {
            Either::Left((response, _)) => {
                response.map_err(|error| connection_error(self.kind, error))?
            }
            Either::Right(_) => return Err(closed_consumer()),
        };
        if !response.status().is_success() {
            return Err(response_error(self.kind, response.status()));
        }

        let mut stream = response.bytes_stream();
        let mut pending = Vec::<u8>::new();
        let mut event = String::new();
        let mut finished = false;
        let mut stream_bytes = 0_usize;
        let mut response_bytes = 0_usize;
        let mut evidence = SearchEvidence::default();
        let mut buffered = String::new();
        loop {
            let next = match select(Box::pin(stream.next()), Box::pin(deltas.closed())).await {
                Either::Left((next, _)) => next,
                Either::Right(_) => return Err(closed_consumer()),
            };
            let Some(next) = next else {
                break;
            };
            let chunk = next.map_err(|error| connection_error(self.kind, error))?;
            stream_bytes = stream_bytes.saturating_add(chunk.len());
            if stream_bytes > MAX_STREAM_BYTES {
                return Err(oversized_response());
            }
            for segment in chunk.split_inclusive(|byte| *byte == b'\n') {
                if pending.len().saturating_add(segment.len()) > MAX_STREAM_LINE {
                    return Err(oversized_response());
                }
                pending.extend_from_slice(segment);
                if !segment.ends_with(b"\n") {
                    continue;
                }
                let line = std::str::from_utf8(&pending)
                    .map_err(|_| "The provider sent invalid stream text.".to_owned())?;
                let line = line.trim_end_matches(['\r', '\n']);
                if line.is_empty() {
                    if !event.is_empty() {
                        if web_search {
                            if parse_search_event(
                                self.kind,
                                &event,
                                &deltas,
                                &mut response_bytes,
                                &mut evidence,
                                &mut buffered,
                                require_sources,
                            )? {
                                finished = true;
                                break;
                            }
                        } else if parse_event(self.kind, &event, &deltas, &mut response_bytes)? {
                            finished = true;
                            break;
                        }
                        event.clear();
                    }
                } else if let Some(data) = line.strip_prefix("data:") {
                    let data = data.strip_prefix(' ').unwrap_or(data);
                    let separator = usize::from(!event.is_empty());
                    if event
                        .len()
                        .saturating_add(separator)
                        .saturating_add(data.len())
                        > MAX_STREAM_EVENT
                    {
                        return Err(oversized_response());
                    }
                    if separator > 0 {
                        event.push('\n');
                    }
                    event.push_str(data);
                }
                pending.clear();
            }
            if finished {
                break;
            }
        }
        if !finished {
            return Err(format!(
                "{} stopped before finishing. Retry the response.",
                self.kind.label()
            ));
        }
        if web_search {
            if (require_sources || evidence.requests > 0) && !evidence.searched() {
                return Err("Web search did not run, so I can't verify current information. Retry when ready.".into());
            }
            if !buffered.is_empty() {
                deltas.send(buffered).map_err(|_| closed_consumer())?;
            }
        }
        Ok(evidence)
    }
}

fn connection_error(kind: ProviderId, error: reqwest::Error) -> String {
    if error.is_timeout() {
        format!(
            "{} took too long to respond. Retry when ready.",
            kind.label()
        )
    } else {
        format!(
            "Could not reach {}. Check your connection and retry.",
            kind.label()
        )
    }
}

fn response_error(kind: ProviderId, status: reqwest::StatusCode) -> String {
    match status.as_u16() {
        401 | 403 => format!(
            "{} rejected the API key. Check it in Settings.",
            kind.label()
        ),
        429 => format!(
            "{} is rate limiting requests. Retry in a moment.",
            kind.label()
        ),
        408 | 504 => format!(
            "{} took too long to respond. Retry when ready.",
            kind.label()
        ),
        500..=599 => format!(
            "{} is temporarily unavailable. Retry in a moment.",
            kind.label()
        ),
        400 | 404 | 422 => format!(
            "{} could not process this request. Check the model in Settings or rephrase.",
            kind.label()
        ),
        _ => format!(
            "{} could not complete the request. Retry in a moment.",
            kind.label()
        ),
    }
}

#[async_trait]
impl ModelProvider for HttpProvider {
    async fn stream_chat(
        &self,
        messages: Vec<ProviderMessage>,
        deltas: UnboundedSender<String>,
    ) -> Result<(), String> {
        self.stream_chat_inner(messages, deltas, false, false)
            .await
            .map(|_| ())
    }
}

fn parse_search_event(
    kind: ProviderId,
    data: &str,
    deltas: &UnboundedSender<String>,
    response_bytes: &mut usize,
    evidence: &mut SearchEvidence,
    buffered: &mut String,
    require_sources: bool,
) -> Result<bool, String> {
    if data == "[DONE]" {
        return Ok(true);
    }
    let value: Value = serde_json::from_str(data)
        .map_err(|_| "The provider sent an unreadable stream event.".to_owned())?;
    if value
        .pointer("/error/message")
        .and_then(Value::as_str)
        .is_some()
    {
        return Err(format!(
            "{} stopped the response. Retry in a moment.",
            kind.label()
        ));
    }
    if let Some(count) = value
        .pointer("/usage/server_tool_use/web_search_requests")
        .and_then(Value::as_u64)
    {
        evidence.requests = evidence.requests.max(count);
    }
    // OpenRouter exposes result sources as url_citation annotations. Some
    // routes place them on the streamed delta, others on the final message.
    for pointer in [
        "/choices/0/delta/annotations",
        "/choices/0/message/annotations",
    ] {
        if let Some(annotations) = value.pointer(pointer).and_then(Value::as_array) {
            for annotation in annotations {
                let citation = annotation.get("url_citation").unwrap_or(annotation);
                let url = citation.get("url").and_then(Value::as_str);
                if let Some(url) = url.filter(|url| valid_source_url(url)) {
                    if evidence.source_urls.len() < 25
                        && !evidence.source_urls.iter().any(|existing| existing == url)
                    {
                        evidence.source_urls.push(url.to_owned());
                    }
                }
            }
        }
    }
    if let Some(content) = value
        .pointer("/choices/0/delta/content")
        .and_then(Value::as_str)
    {
        if !content.is_empty() {
            *response_bytes = response_bytes.saturating_add(content.len());
            if *response_bytes > MAX_RESPONSE_BYTES {
                return Err(oversized_response());
            }
            if !require_sources || evidence.searched() {
                if !buffered.is_empty() {
                    deltas
                        .send(std::mem::take(buffered))
                        .map_err(|_| closed_consumer())?;
                }
                deltas
                    .send(content.to_owned())
                    .map_err(|_| closed_consumer())?;
            } else {
                buffered.push_str(content);
            }
        }
    }
    if (!require_sources || evidence.searched()) && !buffered.is_empty() {
        deltas
            .send(std::mem::take(buffered))
            .map_err(|_| closed_consumer())?;
    }
    Ok(false)
}

fn valid_source_url(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|parsed| {
        matches!(parsed.scheme(), "http" | "https") && parsed.host_str().is_some()
    })
}

fn parse_event(
    kind: ProviderId,
    data: &str,
    deltas: &UnboundedSender<String>,
    response_bytes: &mut usize,
) -> Result<bool, String> {
    if data == "[DONE]" {
        return Ok(true);
    }
    let value: serde_json::Value = serde_json::from_str(data)
        .map_err(|_| "The provider sent an unreadable stream event.".to_owned())?;
    if value
        .pointer("/error/message")
        .and_then(|value| value.as_str())
        .is_some()
    {
        let rate_limited = value
            .pointer("/error/type")
            .and_then(|value| value.as_str())
            .is_some_and(|kind| kind.contains("rate_limit"));
        return Err(if rate_limited {
            format!(
                "{} is rate limiting requests. Retry in a moment.",
                kind.label()
            )
        } else {
            format!("{} stopped the response. Retry in a moment.", kind.label())
        });
    }
    if kind == ProviderId::Anthropic {
        if value.get("type").and_then(|value| value.as_str()) == Some("message_stop") {
            return Ok(true);
        }
        if value
            .pointer("/delta/type")
            .and_then(|value| value.as_str())
            == Some("text_delta")
        {
            if let Some(content) = value
                .pointer("/delta/text")
                .and_then(|value| value.as_str())
            {
                if !content.is_empty() {
                    send_delta(deltas, content, response_bytes)?;
                }
            }
        }
    } else if let Some(content) = value
        .pointer("/choices/0/delta/content")
        .and_then(|value| value.as_str())
    {
        if !content.is_empty() {
            send_delta(deltas, content, response_bytes)?;
        }
    }
    Ok(false)
}

fn send_delta(
    deltas: &UnboundedSender<String>,
    content: &str,
    response_bytes: &mut usize,
) -> Result<(), String> {
    *response_bytes = response_bytes.saturating_add(content.len());
    if *response_bytes > MAX_RESPONSE_BYTES {
        return Err(oversized_response());
    }
    deltas
        .send(content.to_owned())
        .map_err(|_| closed_consumer())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    fn mock_server(response: String, delay: Duration) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/chat/completions", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0_u8; 2048];
            let _ = socket.read(&mut request);
            thread::sleep(delay);
            let _ = socket.write_all(response.as_bytes());
        });
        (url, handle)
    }

    fn mock_provider(endpoint: String, timeout: Duration) -> HttpProvider {
        let mut provider =
            HttpProvider::new(ProviderId::OpenRouter, "test-key".into(), "test".into());
        provider.endpoint = endpoint;
        provider.client = reqwest::Client::builder().timeout(timeout).build().unwrap();
        provider
    }

    fn run_mock(provider: HttpProvider) -> Result<(), String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
            provider
                .stream_chat(
                    vec![ProviderMessage {
                        role: "user".into(),
                        content: "test".into(),
                    }],
                    tx,
                )
                .await
        })
    }

    #[test]
    fn openrouter_search_event_requires_real_usage_or_citation() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut bytes = 0;
        let mut evidence = SearchEvidence::default();
        let mut buffered = String::new();
        parse_search_event(
            ProviderId::OpenRouter,
            r#"{"choices":[{"delta":{"content":"Tomorrow will be sunny."}}]}"#,
            &tx,
            &mut bytes,
            &mut evidence,
            &mut buffered,
            true,
        )
        .unwrap();
        assert!(!evidence.searched());
        assert!(rx.try_recv().is_err());
        parse_search_event(
            ProviderId::OpenRouter,
            r#"{"choices":[{"delta":{"annotations":[{"type":"url_citation","url_citation":{"url":"https://weather.gov/forecast"}}]}}]}"#,
            &tx, &mut bytes, &mut evidence, &mut buffered, true,
        ).unwrap();
        assert_eq!(evidence.source_urls, vec!["https://weather.gov/forecast"]);
        assert_eq!(rx.try_recv().unwrap(), "Tomorrow will be sunny.");
        parse_search_event(
            ProviderId::OpenRouter,
            r#"{"usage":{"server_tool_use":{"web_search_requests":2}}}"#,
            &tx,
            &mut bytes,
            &mut evidence,
            &mut buffered,
            true,
        )
        .unwrap();
        assert_eq!(evidence.requests, 2);
    }

    #[test]
    fn search_event_rejects_bad_source_url_and_malformed_payload() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut bytes = 0;
        let mut evidence = SearchEvidence::default();
        let mut buffered = String::new();
        parse_search_event(ProviderId::OpenRouter,
            r#"{"choices":[{"delta":{"annotations":[{"url_citation":{"url":"javascript:alert(1)"}}],"content":"Claim"}}]}"#,
            &tx, &mut bytes, &mut evidence, &mut buffered, true).unwrap();
        assert!(!evidence.searched());
        assert!(rx.try_recv().is_err());
        assert!(parse_search_event(
            ProviderId::OpenRouter,
            "not-json",
            &tx,
            &mut bytes,
            &mut evidence,
            &mut buffered,
            true,
        )
        .is_err());
    }

    #[test]
    fn hosted_search_uses_server_tool_and_rejects_unsearched_response() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/chat/completions", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                let size = socket.read(&mut buffer).unwrap();
                request.extend_from_slice(&buffer[..size]);
                let Some(end) = request.windows(4).position(|slice| slice == b"\r\n\r\n") else {
                    continue;
                };
                let head = String::from_utf8_lossy(&request[..end]);
                let length = head
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|value| value.parse::<usize>().ok())
                    })
                    .unwrap();
                if request.len() >= end + 4 + length {
                    break;
                }
            }
            let end = request
                .windows(4)
                .position(|slice| slice == b"\r\n\r\n")
                .unwrap();
            let body: Value = serde_json::from_slice(&request[end + 4..]).unwrap();
            assert_eq!(body["tools"][0]["type"], "openrouter:web_search");
            assert_eq!(body["max_tool_calls"], 3);
            assert_eq!(body["stream_options"]["include_usage"], true);
            let stream = "data: {\"choices\":[{\"delta\":{\"content\":\"Current answer\"}}]}\n\ndata: [DONE]\n\n";
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", stream.len(), stream).unwrap();
        });
        let mut provider = mock_provider(url, Duration::from_secs(3));
        provider.kind = ProviderId::OpenRouter;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let result = provider
                .stream_chat_with_web_search(Vec::new(), tx, true)
                .await;
            assert!(result.unwrap_err().contains("did not run"));
            assert!(rx.try_recv().is_err());
        });
        server.join().unwrap();
    }

    #[test]
    fn hosted_search_streams_verified_answer_and_sources() {
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"Checking \"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"annotations\":[{\"type\":\"url_citation\",\"url_citation\":{\"url\":\"https://example.org/report\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"the report.\"}}]}\n\n",
            "data: {\"usage\":{\"server_tool_use\":{\"web_search_requests\":1}}}\n\n",
            "data: [DONE]\n\n"
        );
        let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        let (endpoint, server) = mock_server(response, Duration::ZERO);
        let provider = mock_provider(endpoint, Duration::from_secs(3));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let evidence = provider
                .stream_chat_with_web_search(Vec::new(), tx, true)
                .await
                .unwrap();
            assert_eq!(evidence.requests, 1);
            assert_eq!(evidence.source_urls, vec!["https://example.org/report"]);
            assert_eq!(rx.recv().await.unwrap(), "Checking ");
            assert_eq!(rx.recv().await.unwrap(), "the report.");
            assert!(rx.recv().await.is_none());
        });
        server.join().unwrap();
    }

    #[test]
    fn optional_search_allows_ordinary_answer_without_tool_use() {
        let body = "data: {\"choices\":[{\"delta\":{\"content\":\"A local explanation.\"}}]}\n\ndata: [DONE]\n\n";
        let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        let (endpoint, server) = mock_server(response, Duration::ZERO);
        let provider = mock_provider(endpoint, Duration::from_secs(3));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let evidence = provider
                .stream_chat_with_web_search(Vec::new(), tx, false)
                .await
                .unwrap();
            assert!(!evidence.searched());
            assert_eq!(evidence.requests, 0);
            assert_eq!(rx.recv().await.unwrap(), "A local explanation.");
            assert!(rx.recv().await.is_none());
        });
        server.join().unwrap();
    }

    #[test]
    fn usage_without_cited_source_cannot_verify_current_answer() {
        let body = "data: {\"choices\":[{\"delta\":{\"content\":\"Unsourced current claim.\"}}]}\n\ndata: {\"usage\":{\"server_tool_use\":{\"web_search_requests\":1}}}\n\ndata: [DONE]\n\n";
        let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        let (endpoint, server) = mock_server(response, Duration::ZERO);
        let provider = mock_provider(endpoint, Duration::from_secs(3));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let error = provider
                .stream_chat_with_web_search(Vec::new(), tx, true)
                .await
                .unwrap_err();
            assert!(error.contains("can't verify"), "{error}");
            assert!(rx.try_recv().is_err());
        });
        server.join().unwrap();
    }

    #[test]
    fn other_providers_cannot_use_openrouter_hosted_search() {
        let provider = HttpProvider::new(ProviderId::OpenAI, "test-key".into(), "test".into());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
            assert!(provider
                .stream_chat_with_web_search(Vec::new(), tx, true)
                .await
                .unwrap_err()
                .contains("OpenRouter"));
        });
    }

    #[test]
    fn parses_openai_compatible_and_anthropic_deltas() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        assert!(!parse_event(
            ProviderId::OpenAI,
            r#"{"choices":[{"delta":{"content":"hello"}}]}"#,
            &tx,
            &mut 0
        )
        .unwrap());
        assert_eq!(rx.try_recv().unwrap(), "hello");
        assert!(!parse_event(
            ProviderId::Anthropic,
            r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"world"}}"#,
            &tx,
            &mut 0
        )
        .unwrap());
        assert_eq!(rx.try_recv().unwrap(), "world");
        assert!(parse_event(
            ProviderId::Anthropic,
            r#"{"type":"message_stop"}"#,
            &tx,
            &mut 0
        )
        .unwrap());
        assert!(parse_event(ProviderId::Xai, "[DONE]", &tx, &mut 0).unwrap());
    }

    #[test]
    fn maps_common_provider_failures_to_clear_messages() {
        assert!(
            response_error(ProviderId::OpenRouter, reqwest::StatusCode::UNAUTHORIZED)
                .contains("API key")
        );
        assert!(response_error(
            ProviderId::OpenRouter,
            reqwest::StatusCode::TOO_MANY_REQUESTS
        )
        .contains("rate limiting"));
        assert!(
            response_error(ProviderId::OpenRouter, reqwest::StatusCode::BAD_GATEWAY)
                .contains("temporarily unavailable")
        );
        assert!(parse_event(
            ProviderId::OpenAI,
            "not-json",
            &tokio::sync::mpsc::unbounded_channel().0,
            &mut 0
        )
        .is_err());
    }

    #[test]
    fn handles_http_auth_and_rate_limit_errors() {
        for (status, expected) in [
            ("401 Unauthorized", "API key"),
            ("429 Too Many Requests", "rate limiting"),
        ] {
            let response =
                format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            let (endpoint, server) = mock_server(response, Duration::ZERO);
            let error = run_mock(mock_provider(endpoint, Duration::from_secs(2))).unwrap_err();
            assert!(error.contains(expected), "{error}");
            server.join().unwrap();
        }
    }

    #[test]
    fn handles_timeout_malformed_event_and_dropped_stream() {
        let (endpoint, server) = mock_server(String::new(), Duration::from_millis(150));
        let timeout = run_mock(mock_provider(endpoint, Duration::from_millis(30))).unwrap_err();
        assert!(timeout.contains("too long"), "{timeout}");
        server.join().unwrap();

        let body = "data: not-json\n\n";
        let malformed = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        let (endpoint, server) = mock_server(malformed, Duration::ZERO);
        let error = run_mock(mock_provider(endpoint, Duration::from_secs(2))).unwrap_err();
        assert!(error.contains("unreadable stream event"), "{error}");
        server.join().unwrap();

        let body = "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n";
        let dropped = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len() + 40);
        let (endpoint, server) = mock_server(dropped, Duration::ZERO);
        assert!(run_mock(mock_provider(endpoint, Duration::from_secs(2))).is_err());
        server.join().unwrap();
    }

    fn chunked_server(chunks: Vec<Vec<u8>>, pause: Duration) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/chat/completions", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0_u8; 2048];
            let _ = socket.read(&mut request);
            let _ = socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n");
            for chunk in chunks {
                if write!(socket, "{:x}\r\n", chunk.len()).is_err()
                    || socket.write_all(&chunk).is_err()
                    || socket.write_all(b"\r\n").is_err()
                {
                    return;
                }
                thread::sleep(pause);
            }
            let _ = socket.write_all(b"0\r\n\r\n");
        });
        (url, handle)
    }

    #[test]
    fn rejects_unbounded_lines_events_and_transport_without_content_length() {
        let cases = [
            vec![vec![b'x'; MAX_STREAM_LINE + 1]],
            vec![format!("data: {}\n", "x".repeat(100_000)).into_bytes(); 6],
            vec![format!(": {}\n", "x".repeat(1_000)).into_bytes(); 4_000],
        ];
        for chunks in cases {
            let (endpoint, server) = chunked_server(chunks, Duration::ZERO);
            let error = run_mock(mock_provider(endpoint, Duration::from_secs(5))).unwrap_err();
            assert!(error.contains("too large"), "{error}");
            server.join().unwrap();
        }
    }

    #[test]
    fn rejects_total_content_larger_than_response_limit() {
        let delta = format!(
            "data: {{\"choices\":[{{\"delta\":{{\"content\":\"{}\"}}}}]}}\n\n",
            "x".repeat(50_000)
        );
        let mut chunks = vec![delta.into_bytes(); 9];
        chunks.push(b"data: [DONE]\n\n".to_vec());
        let (endpoint, server) = chunked_server(chunks, Duration::ZERO);
        let error = run_mock(mock_provider(endpoint, Duration::from_secs(5))).unwrap_err();
        assert!(error.contains("too large"), "{error}");
        server.join().unwrap();
    }

    #[test]
    fn accepts_fragmented_utf8_and_multiline_crlf_events() {
        let body = ": heartbeat\r\ndata: {\"choices\":\r\ndata: [{\"delta\":{\"content\":\"café ☀\"}}]}\r\n\r\ndata: [DONE]\r\n\r\n";
        let chunks = body.as_bytes().chunks(1).map(<[u8]>::to_vec).collect();
        let (endpoint, server) = chunked_server(chunks, Duration::from_millis(1));
        let provider = mock_provider(endpoint, Duration::from_secs(5));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            provider.stream_chat(Vec::new(), tx).await.unwrap();
            assert_eq!(rx.recv().await.unwrap(), "café ☀");
            assert!(rx.recv().await.is_none());
        });
        server.join().unwrap();
    }

    #[test]
    fn stops_when_consumer_closes_during_network_wait() {
        let (endpoint, server) = chunked_server(
            vec![b": waiting\n".to_vec(), b"data: [DONE]\n\n".to_vec()],
            Duration::from_millis(200),
        );
        let provider = mock_provider(endpoint, Duration::from_secs(5));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            let response = provider.stream_chat(Vec::new(), tx);
            let close = async {
                tokio::time::sleep(Duration::from_millis(30)).await;
                drop(rx);
            };
            let (result, ()) = futures_util::future::join(response, close).await;
            assert!(result.unwrap_err().contains("interrupted"));
        });
        server.join().unwrap();
    }

    #[test]
    fn closed_consumer_prevents_network_request_and_delta_delivery() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let provider = mock_provider(
            "http://127.0.0.1:1/unreachable".into(),
            Duration::from_secs(2),
        );
        runtime.block_on(async {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            drop(rx);
            assert!(provider
                .stream_chat(Vec::new(), tx)
                .await
                .unwrap_err()
                .contains("interrupted"));
        });
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        drop(rx);
        assert!(parse_event(
            ProviderId::OpenAI,
            r#"{"choices":[{"delta":{"content":"hello"}}]}"#,
            &tx,
            &mut 0
        )
        .unwrap_err()
        .contains("interrupted"));
    }
}
