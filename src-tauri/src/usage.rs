//! Read-only usage for the user's OpenRouter key, including use by other apps.
//! This is not an account balance or Bench-specific metering. Called manually,
//! never during startup, so the protected key is read only for an explicit action.
use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

const ENDPOINT: &str = "https://openrouter.ai/api/v1/key";
const MAX_BODY: usize = 64 * 1024;
const INVALID_USAGE: &str = "OpenRouter returned unreadable usage data. Retry later.";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenRouterUsage {
    pub usage_daily: f64,
    pub usage_weekly: f64,
    pub usage_monthly: f64,
    pub usage_total: f64,
    pub limit: Option<f64>,
    pub limit_remaining: Option<f64>,
    pub limit_reset: Option<LimitReset>,
    pub byok_usage_monthly: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LimitReset {
    Daily,
    Weekly,
    Monthly,
}

#[derive(Deserialize)]
struct KeyResponse {
    data: KeyUsage,
}

#[derive(Deserialize)]
struct KeyUsage {
    usage_daily: f64,
    usage_weekly: f64,
    usage_monthly: f64,
    usage: f64,
    limit: Option<f64>,
    limit_remaining: Option<f64>,
    limit_reset: Option<LimitReset>,
    byok_usage_monthly: Option<f64>,
}

fn parse_usage(bytes: &[u8]) -> Result<OpenRouterUsage, String> {
    let value: KeyResponse = serde_json::from_slice(bytes).map_err(|_| INVALID_USAGE.to_owned())?;
    let value = value.data;
    if [
        Some(value.usage_daily),
        Some(value.usage_weekly),
        Some(value.usage_monthly),
        Some(value.usage),
        value.limit,
        value.limit_remaining,
        value.byok_usage_monthly,
    ]
    .into_iter()
    .flatten()
    .any(|amount| !amount.is_finite() || amount < 0.0)
    {
        return Err(INVALID_USAGE.into());
    }
    Ok(OpenRouterUsage {
        usage_daily: value.usage_daily,
        usage_weekly: value.usage_weekly,
        usage_monthly: value.usage_monthly,
        usage_total: value.usage,
        limit: value.limit,
        limit_remaining: value.limit_remaining,
        limit_reset: value.limit_reset,
        byok_usage_monthly: value.byok_usage_monthly,
    })
}

fn client(timeout: Duration) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(timeout)
        .connect_timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Usage lookup is unavailable. Retry later.".into())
}

fn request_error(error: reqwest::Error) -> String {
    if error.is_timeout() {
        "OpenRouter usage took too long to load. Retry when ready.".into()
    } else {
        "Could not load OpenRouter usage. Check your connection and retry.".into()
    }
}

async fn fetch(
    client: &reqwest::Client,
    endpoint: &str,
    key: &str,
) -> Result<OpenRouterUsage, String> {
    let response = client
        .get(endpoint)
        .bearer_auth(key)
        .send()
        .await
        .map_err(request_error)?;
    match response.status().as_u16() {
        200..=299 => {}
        401 | 403 => {
            return Err("OpenRouter rejected this API key. Check it in Connections.".into())
        }
        429 => return Err("OpenRouter is rate limiting usage lookups. Retry in a moment.".into()),
        _ => return Err("OpenRouter usage is unavailable. Retry later.".into()),
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_BODY as u64)
    {
        return Err(INVALID_USAGE.into());
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(request_error)?;
        if bytes.len().saturating_add(chunk.len()) > MAX_BODY {
            return Err(INVALID_USAGE.into());
        }
        bytes.extend_from_slice(&chunk);
    }
    parse_usage(&bytes)
}

pub async fn load_openrouter_usage(key: &str) -> Result<OpenRouterUsage, String> {
    fetch(&client(Duration::from_secs(15))?, ENDPOINT, key).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    const DATA: &str = r#"{"data":{"usage_daily":0.12,"usage_weekly":1.25,"usage_monthly":2.5,"usage":9.75,"limit":null,"limit_remaining":null,"limit_reset":null}}"#;

    fn server(response: String, delay: Duration) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/api/v1/key", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = [0_u8; 2048];
            let size = socket.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..size]);
            assert!(request.starts_with("GET /api/v1/key HTTP/1.1"));
            assert!(request
                .to_ascii_lowercase()
                .contains("authorization: bearer fake-test-key"));
            thread::sleep(delay);
            let _ = socket.write_all(response.as_bytes());
        });
        (endpoint, handle)
    }

    fn run(endpoint: &str, timeout: Duration) -> Result<OpenRouterUsage, String> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(fetch(&client(timeout).unwrap(), endpoint, "fake-test-key"))
    }

    #[test]
    fn returns_validated_key_usage_and_preserves_null_caps() {
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{DATA}",
            DATA.len()
        );
        let (endpoint, handle) = server(response, Duration::ZERO);
        let usage = run(&endpoint, Duration::from_secs(3)).unwrap();
        assert_eq!(usage.usage_total, 9.75);
        assert_eq!(usage.usage_monthly, 2.5);
        assert!(usage.limit.is_none());
        let serialized = serde_json::to_value(usage).unwrap();
        assert_eq!(serialized["usageDaily"], 0.12);
        assert!(serialized["limitRemaining"].is_null());
        assert!(serialized.get("label").is_none());
        handle.join().unwrap();
        let reset = DATA
            .replace("\"limit_reset\":null", "\"limit_reset\":\"monthly\"")
            .replace("\"limit\":null", "\"limit\":20");
        assert_eq!(parse_usage(reset.as_bytes()).unwrap().limit, Some(20.0));
    }

    #[test]
    fn rejects_missing_negative_nonfinite_and_malicious_usage_without_echoing_body() {
        for body in [
            "{not-json fake-test-key}".into(),
            r#"{"data":{"usage":1}}"#.into(),
            DATA.replace("\"usage\":9.75", "\"usage\":null"),
            DATA.replace("0.12", "-0.12"),
            DATA.replace("9.75", "1e400"),
            DATA.replace("\"limit\":null", "\"limit\":-1"),
            DATA.replace("\"limit_reset\":null", "\"limit_reset\":\"fake-test-key\""),
        ] {
            assert_eq!(parse_usage(body.as_bytes()).unwrap_err(), INVALID_USAGE);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let (endpoint, handle) = server(response, Duration::ZERO);
            assert_eq!(
                run(&endpoint, Duration::from_secs(3)).unwrap_err(),
                INVALID_USAGE
            );
            handle.join().unwrap();
        }
    }

    #[test]
    fn maps_auth_rate_limit_and_timeout_to_safe_errors() {
        for (status, expected) in [
            ("401 Unauthorized", "API key"),
            ("429 Too Many Requests", "rate limiting"),
        ] {
            let body = "secret fake-test-key";
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let (endpoint, handle) = server(response, Duration::ZERO);
            let error = run(&endpoint, Duration::from_secs(3)).unwrap_err();
            assert!(error.contains(expected));
            assert!(!error.contains("fake-test-key"));
            handle.join().unwrap();
        }
        let (endpoint, handle) = server(String::new(), Duration::from_millis(150));
        assert!(run(&endpoint, Duration::from_millis(30))
            .unwrap_err()
            .contains("too long"));
        handle.join().unwrap();
    }

    #[test]
    fn refuses_redirects_without_contacting_target_or_forwarding_credentials() {
        let destination = TcpListener::bind("127.0.0.1:0").unwrap();
        destination.set_nonblocking(true).unwrap();
        let response = format!("HTTP/1.1 302 Found\r\nLocation: http://{}/steal\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", destination.local_addr().unwrap());
        let (endpoint, handle) = server(response, Duration::ZERO);
        assert!(run(&endpoint, Duration::from_secs(3)).is_err());
        handle.join().unwrap();
        assert_eq!(
            destination.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn bounds_body_with_and_without_content_length() {
        for response in [
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                MAX_BODY + 1
            ),
            format!(
                "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{}",
                "x".repeat(MAX_BODY + 1)
            ),
        ] {
            let (endpoint, handle) = server(response, Duration::ZERO);
            assert_eq!(
                run(&endpoint, Duration::from_secs(3)).unwrap_err(),
                INVALID_USAGE
            );
            handle.join().unwrap();
        }
    }
}
