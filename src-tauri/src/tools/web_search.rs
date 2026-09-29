use std::{
    env,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use reqwest::{Client, Url};
use serde_json::Value;

use super::{SearchHit, Tool, ToolError, ToolResult};

const SEARCH_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_RESPONSE_BYTES: usize = 1_000_000;
const MAX_HITS: usize = 5;

#[derive(Clone)]
enum Backend {
    Searxng(Url),
    Brave(String),
}

#[derive(Clone)]
pub struct WebSearch {
    client: Client,
    backend: Backend,
}

impl WebSearch {
    /// Search configuration is independent of the selected model provider.
    /// The caller supplies the Brave secret from the Rust credential store.
    pub fn from_configuration(
        preference: &str,
        searxng_base: &str,
        brave_key: Option<String>,
    ) -> Option<Self> {
        let brave = || {
            brave_key
                .filter(|key| !key.trim().is_empty())
                .map(Backend::Brave)
        };
        let searxng = || searxng_url(searxng_base).ok().map(Backend::Searxng);
        let backend = match preference {
            "off" => return None,
            "brave" => brave(),
            "searxng" => searxng(),
            "auto" => env::var("BENCH_SEARXNG_URL")
                .ok()
                .and_then(|base| searxng_url(&base).ok())
                .map(Backend::Searxng)
                .or_else(brave),
            _ => None,
        }?;
        Some(Self {
            client: Client::builder()
                .timeout(SEARCH_TIMEOUT)
                .user_agent("Bench/0.0.1")
                .build()
                .ok()?,
            backend,
        })
    }

    pub fn normalize_searxng_url(base: &str) -> Result<String, String> {
        searxng_url(base)
            .map(|url| url.to_string())
            .map_err(|_| "Enter a valid SearXNG http or https URL without credentials.".into())
    }

    #[cfg(test)]
    fn for_searxng(base: &str) -> Self {
        Self {
            client: Client::builder().timeout(SEARCH_TIMEOUT).build().unwrap(),
            backend: Backend::Searxng(searxng_url(base).unwrap()),
        }
    }

    pub fn backend_name(&self) -> &'static str {
        match self.backend {
            Backend::Searxng(_) => "searxng",
            Backend::Brave(_) => "brave",
        }
    }
}

#[async_trait]
impl Tool for WebSearch {
    fn name(&self) -> &'static str {
        "web_search"
    }

    fn category(&self) -> crate::policy::ActionCategory {
        crate::policy::ActionCategory::ReadOnly
    }

    async fn execute(&self, query: &str) -> Result<ToolResult, ToolError> {
        let query = query.trim();
        if query.is_empty() || query.chars().count() > 400 {
            return Err(ToolError::new(
                "invalid_query",
                "Search needs a shorter query.",
            ));
        }
        let start = Instant::now();
        let request = match &self.backend {
            Backend::Searxng(base) => {
                let mut url = base.clone();
                url.query_pairs_mut()
                    .append_pair("q", query)
                    .append_pair("format", "json")
                    .append_pair("safesearch", "1");
                self.client.get(url)
            }
            Backend::Brave(key) => self
                .client
                .get("https://api.search.brave.com/res/v1/web/search")
                .query(&[("q", query), ("count", "5")])
                .header("X-Subscription-Token", key)
                .header("Accept", "application/json"),
        };
        let mut response = request.send().await.map_err(map_network_error)?;
        if !response.status().is_success() {
            return Err(match response.status().as_u16() {
                401 | 403 => ToolError::new("unauthorized", "Web search needs a valid connection."),
                429 => ToolError::new("rate_limited", "Web search is busy. Try again shortly."),
                _ => ToolError::new("http_error", "Web search is unavailable right now."),
            });
        }
        if response
            .content_length()
            .is_some_and(|size| size > MAX_RESPONSE_BYTES as u64)
        {
            return Err(ToolError::new(
                "too_large",
                "Web search returned too much data.",
            ));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(map_network_error)? {
            if chunk.len() > MAX_RESPONSE_BYTES - bytes.len() {
                return Err(ToolError::new(
                    "too_large",
                    "Web search returned too much data.",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        let body: Value = serde_json::from_slice(&bytes).map_err(|_| {
            ToolError::new("malformed", "Web search returned an unreadable result.")
        })?;
        let results = match &self.backend {
            Backend::Searxng(_) => parse_searxng(&body),
            Backend::Brave(_) => parse_brave(&body),
        };
        if results.is_empty() {
            return Err(ToolError::new(
                "empty",
                "Web search found no usable results.",
            ));
        }
        Ok(ToolResult {
            query: query.to_owned(),
            backend: self.backend_name().to_owned(),
            elapsed_ms: start.elapsed().as_millis(),
            summary: "Searched the web".to_owned(),
            results,
        })
    }
}

fn searxng_url(base: &str) -> Result<Url, ()> {
    let mut url = Url::parse(base).map_err(|_| ())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(());
    }
    if !url.path().trim_end_matches('/').ends_with("/search") {
        let path = format!("{}/search", url.path().trim_end_matches('/'));
        url.set_path(&path);
    }
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

fn map_network_error(error: reqwest::Error) -> ToolError {
    if error.is_timeout() {
        ToolError::new("timeout", "Web search timed out. Try again.")
    } else {
        ToolError::new("network", "Web search is unavailable right now.")
    }
}

fn parse_searxng(body: &Value) -> Vec<SearchHit> {
    parse_results(body.get("results"), "content")
}

fn parse_brave(body: &Value) -> Vec<SearchHit> {
    parse_results(
        body.get("web").and_then(|web| web.get("results")),
        "description",
    )
}

fn parse_results(items: Option<&Value>, snippet_key: &str) -> Vec<SearchHit> {
    items
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let title = clean(item.get("title")?.as_str()?, 180);
            let url = item.get("url")?.as_str()?;
            if title.is_empty() || !matches!(Url::parse(url).ok()?.scheme(), "https" | "http") {
                return None;
            }
            Some(SearchHit {
                title,
                url: url.to_owned(),
                snippet: clean(
                    item.get(snippet_key).and_then(Value::as_str).unwrap_or(""),
                    450,
                ),
            })
        })
        .take(MAX_HITS)
        .collect()
}

fn clean(text: &str, max_chars: usize) -> String {
    text.chars()
        .filter(|ch| !ch.is_control() || *ch == ' ')
        .take(max_chars)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Conservative first-pass routing for requests that explicitly need fresh facts.
/// The model must not answer with a made-up current value when search is unavailable.
pub fn requires_current_info(query: &str) -> bool {
    let text = query.to_lowercase();
    let current_words = [
        "current",
        "currently",
        "latest",
        "today",
        "tonight",
        "tomorrow",
        "this week",
        "this month",
        "right now",
        "live",
        "recent",
        "as of",
        "up to date",
        "up-to-date",
    ];
    let volatile_topics = [
        "weather",
        "forecast",
        "stock price",
        "share price",
        "price of",
        "market cap",
        "exchange rate",
        "news",
        "announcement",
        "announced",
        "earnings",
        "score",
        "schedule",
        "who is the",
        "what is the price",
    ];
    let asks_for_facts = [
        "what",
        "who",
        "when",
        "where",
        "how",
        "tell me",
        "show me",
        "give me",
        "check",
        "find",
        "search",
        "weather in",
        "forecast for",
        "stock price",
        "share price",
    ]
    .iter()
    .any(|word| contains_term(&text, word));
    current_words.iter().any(|word| contains_term(&text, word))
        || (asks_for_facts
            && volatile_topics
                .iter()
                .any(|word| contains_term(&text, word)))
}

fn contains_term(text: &str, term: &str) -> bool {
    text.match_indices(term).any(|(start, _)| {
        let before = text[..start].chars().next_back();
        let after = text[start + term.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::mpsc,
        thread,
    };

    #[test]
    fn routes_current_requests() {
        assert!(requires_current_info("weather in NYC this week"));
        assert!(requires_current_info("latest OpenAI announcement"));
        assert!(requires_current_info("current NVIDIA price"));
        assert!(!requires_current_info("Build a website for Laundros"));
        assert!(!requires_current_info("Build a weather app"));
        assert!(!requires_current_info("Deliver the document"));
        assert!(!requires_current_info("Explain how a database index works"));
    }

    #[test]
    fn parses_and_limits_searxng_results() {
        let body = serde_json::json!({"results": [
            {"title": "  A useful source  ", "url": "https://example.com/a", "content": "  A  short\n summary "},
            {"title": "Unsafe", "url": "javascript:alert(1)", "content": "ignore"},
            {"title": "No URL", "content": "ignore"}
        ]});
        assert_eq!(
            parse_searxng(&body),
            vec![SearchHit {
                title: "A useful source".into(),
                url: "https://example.com/a".into(),
                snippet: "A short summary".into(),
            }]
        );
    }

    #[test]
    fn parses_brave_shape() {
        let body = serde_json::json!({"web": {"results": [
            {"title": "Story", "url": "https://example.com/story", "description": "Fresh story"}
        ]}});
        assert_eq!(parse_brave(&body)[0].snippet, "Fresh story");
    }

    #[test]
    fn normalizes_instance_endpoint() {
        assert_eq!(
            searxng_url("https://example.com/").unwrap().as_str(),
            "https://example.com/search"
        );
        assert_eq!(
            searxng_url("http://localhost:8080/search")
                .unwrap()
                .as_str(),
            "http://localhost:8080/search"
        );
        assert!(searxng_url("file:///tmp/search").is_err());
        assert!(searxng_url("https://user:password@example.com/search").is_err());
        assert_eq!(
            WebSearch::for_searxng("https://example.com").name(),
            "web_search"
        );
        assert_eq!(
            WebSearch::for_searxng("https://example.com").category(),
            crate::policy::ActionCategory::ReadOnly
        );
    }

    #[test]
    fn selects_configured_backend_without_model_provider() {
        assert_eq!(
            WebSearch::from_configuration("brave", "", Some("test-key".into()))
                .unwrap()
                .backend_name(),
            "brave"
        );
        assert_eq!(
            WebSearch::from_configuration("searxng", "http://localhost:8080", None)
                .unwrap()
                .backend_name(),
            "searxng"
        );
        assert!(WebSearch::from_configuration("brave", "", None).is_none());
        assert!(WebSearch::from_configuration("off", "", Some("test-key".into())).is_none());
    }

    #[test]
    fn stops_reading_oversized_response_without_content_length() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let (release_tx, release_rx) = mpsc::channel();
        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0_u8; 2048];
            let _ = socket.read(&mut request);
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
            let chunk = vec![b'x'; 65_536];
            for _ in 0..16 {
                socket.write_all(b"10000\r\n").unwrap();
                socket.write_all(&chunk).unwrap();
                socket.write_all(b"\r\n").unwrap();
            }
            socket.write_all(b"1\r\nx\r\n").unwrap();
            // Keep the response open. A reader that buffers the entire body will
            // time out; a bounded reader can reject it before the final chunk.
            let _ = release_rx.recv_timeout(Duration::from_secs(3));
            let _ = socket.write_all(b"0\r\n\r\n");
        });
        let search = WebSearch {
            client: Client::builder()
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap(),
            backend: Backend::Searxng(searxng_url(&base).unwrap()),
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let result = runtime.block_on(search.execute("test"));
        release_tx.send(()).unwrap();
        server.join().unwrap();
        assert_eq!(result.unwrap_err().kind, "too_large");
    }
}
