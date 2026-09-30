//! Native OpenRouter PKCE. Secrets stay in Rust; the caller persists the key
//! only if this exact connection attempt is still active after completion.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use futures_util::StreamExt;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

const AUTH_URL: &str = "https://openrouter.ai/auth";
const EXCHANGE_URL: &str = "https://openrouter.ai/api/v1/auth/keys";
const MAX_HEADERS: usize = 8192;
const MAX_RESPONSE: usize = 16_384;
const INVALID_EXCHANGE: &str =
    "OpenRouter returned an invalid connection response. Try connecting again.";
const CANCELLED: &str = "OpenRouter connection was cancelled.";

struct Pkce {
    verifier: String,
    challenge: String,
}
impl Pkce {
    fn new() -> Self {
        let verifier = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let challenge = challenge(&verifier);
        Self {
            verifier,
            challenge,
        }
    }
}
fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn authorization_url(callback: &str, challenge: &str) -> String {
    let mut url = reqwest::Url::parse(AUTH_URL).expect("fixed HTTPS authorization URL is valid");
    url.query_pairs_mut()
        .append_pair("callback_url", callback)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("key_label", "Bench");
    url.into()
}

enum Callback {
    Code(String),
    Denied,
}

fn callback(request: &[u8], path: &str, host: &str) -> Option<Callback> {
    let request = std::str::from_utf8(request).ok()?;
    let (head, _) = request.split_once("\r\n\r\n")?;
    let mut lines = head.split("\r\n");
    let mut start = lines.next()?.split(' ');
    if start.next()? != "GET" {
        return None;
    }
    let target = start.next()?;
    if start.next()? != "HTTP/1.1" || start.next().is_some() || !target.starts_with('/') {
        return None;
    }
    let (target_path, query) = target.split_once('?').unwrap_or((target, ""));
    if target_path != path {
        return None;
    }
    let mut host_count = 0;
    for line in lines {
        let (name, value) = line.split_once(':')?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return None;
        }
        if name.eq_ignore_ascii_case("host") {
            host_count += 1;
            if !value.trim().eq_ignore_ascii_case(host) {
                return None;
            }
        }
        if name.eq_ignore_ascii_case("transfer-encoding")
            || (name.eq_ignore_ascii_case("content-length") && value.trim() != "0")
        {
            return None;
        }
    }
    if host_count != 1 {
        return None;
    }
    let url = reqwest::Url::parse(&format!("http://{host}{path}?{query}")).ok()?;
    let mut code = None;
    let mut error = false;
    for (name, value) in url.query_pairs() {
        if name == "code" {
            if code.is_some()
                || value.is_empty()
                || value.len() > 2048
                || !value.bytes().all(|byte| byte.is_ascii_graphic())
            {
                return None;
            }
            code = Some(value.into_owned());
        } else if name == "error" {
            if error {
                return None;
            }
            error = true;
        }
    }
    match (code, error) {
        (Some(code), false) => Some(Callback::Code(code)),
        (None, true) => Some(Callback::Denied),
        _ => None,
    }
}

async fn read_headers(socket: &mut TcpStream) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let count = socket.read(&mut chunk).await.ok()?;
        if count == 0 || bytes.len().saturating_add(count) > MAX_HEADERS {
            return None;
        }
        bytes.extend_from_slice(&chunk[..count]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            return Some(bytes);
        }
    }
}

async fn reply(socket: &mut TcpStream, accepted: bool) {
    let (status, message) = if accepted {
        (
            "200 OK",
            "Authorization received. Return to Bench to finish connecting. You may close this tab.",
        )
    } else {
        (
            "400 Bad Request",
            "This connection callback is invalid. Return to Bench.",
        )
    };
    let response = format!("HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'\r\nConnection: close\r\n\r\n{message}", message.len());
    let _ = tokio::time::timeout(
        Duration::from_secs(1),
        socket.write_all(response.as_bytes()),
    )
    .await;
}

#[derive(Deserialize)]
struct ExchangeResponse {
    key: String,
}

async fn exchange(endpoint: &str, code: &str, verifier: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "OpenRouter connection is unavailable. Try again.")?;
    let response = client.post(endpoint).json(&serde_json::json!({"code":code,"code_verifier":verifier,"code_challenge_method":"S256"}))
        .send().await.map_err(|error| if error.is_timeout() { "OpenRouter connection took too long. Try again." } else { "Could not connect to OpenRouter. Check your connection and try again." })?;
    if !response.status().is_success() {
        return Err("OpenRouter could not authorize this connection. Try connecting again.".into());
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_RESPONSE as u64)
    {
        return Err(INVALID_EXCHANGE.into());
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "OpenRouter connection was interrupted. Try again.")?;
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE {
            return Err(INVALID_EXCHANGE.into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let response: ExchangeResponse =
        serde_json::from_slice(&bytes).map_err(|_| INVALID_EXCHANGE)?;
    if response.key.is_empty()
        || response.key.len() > 4096
        || !response.key.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(INVALID_EXCHANGE.into());
    }
    Ok(response.key)
}

async fn cancellation(cancel: Arc<AtomicBool>) {
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    loop {
        interval.tick().await;
        if cancel.load(Ordering::Acquire) {
            return;
        }
    }
}

async fn flow(
    open_browser: impl FnOnce(&str) -> Result<(), String>,
    endpoint: &str,
) -> Result<String, String> {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(|_| "Could not open a local OpenRouter callback. Try connecting again.")?;
    let port = listener
        .local_addr()
        .map_err(|_| "OpenRouter callback is unavailable.")?
        .port();
    let host = format!("localhost:{port}");
    let path = format!("/openrouter/{}", uuid::Uuid::new_v4().simple());
    let pkce = Pkce::new();
    open_browser(&authorization_url(
        &format!("http://{host}{path}"),
        &pkce.challenge,
    ))
    .map_err(|_| "Could not open OpenRouter in your browser. Try connecting again.")?;
    for _ in 0..64 {
        let (mut socket, address) = listener
            .accept()
            .await
            .map_err(|_| "OpenRouter callback was interrupted. Try connecting again.")?;
        if !address.ip().is_loopback() {
            continue;
        }
        let result = tokio::time::timeout(Duration::from_secs(2), read_headers(&mut socket))
            .await
            .ok()
            .flatten()
            .and_then(|bytes| callback(&bytes, &path, &host));
        reply(&mut socket, result.is_some()).await;
        match result {
            Some(Callback::Code(code)) => {
                drop(socket);
                drop(listener);
                return exchange(endpoint, &code, &pkce.verifier).await;
            }
            Some(Callback::Denied) => {
                return Err(
                    "OpenRouter authorization was not completed. Try connecting again.".into(),
                )
            }
            None => {}
        }
    }
    Err("OpenRouter callback was interrupted. Try connecting again.".into())
}

async fn connect_with(
    open_browser: impl FnOnce(&str) -> Result<(), String>,
    cancel: Arc<AtomicBool>,
    endpoint: &str,
    timeout: Duration,
) -> Result<String, String> {
    if cancel.load(Ordering::Acquire) {
        return Err(CANCELLED.into());
    }
    tokio::select! {
        biased;
        _ = cancellation(cancel) => Err(CANCELLED.into()),
        _ = tokio::time::sleep(timeout) => Err("OpenRouter connection expired. Try connecting again.".into()),
        result = flow(open_browser, endpoint) => result,
    }
}

pub async fn connect(
    open_browser: impl FnOnce(&str) -> Result<(), String>,
    cancel: Arc<AtomicBool>,
) -> Result<String, String> {
    connect_with(
        open_browser,
        cancel,
        EXCHANGE_URL,
        Duration::from_secs(5 * 60),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::{TcpListener as StdListener, TcpStream as StdStream},
        thread,
    };

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }
    fn request(target: &str, host: &str) -> Vec<u8> {
        format!("GET {target} HTTP/1.1\r\nHost: {host}\r\n\r\n").into_bytes()
    }
    fn send_callback(callback: &reqwest::Url, target: &str) -> String {
        let mut socket = StdStream::connect(("127.0.0.1", callback.port().unwrap())).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let host = format!("localhost:{}", callback.port().unwrap());
        socket.write_all(&request(target, &host)).unwrap();
        let mut response = String::new();
        socket.read_to_string(&mut response).unwrap();
        response
    }
    fn exchange_server(
        body: &str,
        status: &str,
    ) -> (String, thread::JoinHandle<serde_json::Value>) {
        let listener = StdListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/api/v1/auth/keys", listener.local_addr().unwrap());
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let handle = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0_u8; 1024];
            let header_end = loop {
                let count = socket.read(&mut chunk).unwrap();
                assert!(count > 0);
                bytes.extend_from_slice(&chunk[..count]);
                if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&bytes[..end]);
                    assert!(head.starts_with("POST /api/v1/auth/keys HTTP/1.1"));
                    assert!(!head.to_ascii_lowercase().contains("authorization:"));
                    let length = head
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|value| value.parse::<usize>().ok())
                        })
                        .unwrap();
                    if bytes.len() >= end + 4 + length {
                        break end;
                    }
                }
            };
            let payload = serde_json::from_slice(&bytes[header_end + 4..]).unwrap();
            socket.write_all(response.as_bytes()).unwrap();
            payload
        });
        (endpoint, handle)
    }

    #[test]
    fn pkce_uses_rfc7636_s256_vector_and_unique_strong_verifiers() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        let first = Pkce::new();
        let second = Pkce::new();
        assert_ne!(first.verifier, second.verifier);
        assert_eq!(first.verifier.len(), 64);
        assert_eq!(first.challenge, challenge(&first.verifier));
        assert!(!first.challenge.contains('='));
    }

    #[test]
    fn callback_requires_exact_nonce_path_host_get_and_single_code() {
        let path = "/openrouter/nonce";
        let host = "localhost:12345";
        assert!(
            matches!(callback(&request("/openrouter/nonce?code=valid",host),path,host),Some(Callback::Code(code)) if code=="valid")
        );
        for bytes in [request("/openrouter/wrong?code=secret",host),request("/openrouter/nonce?code=secret","attacker.example"),request("/openrouter/nonce?code=a&code=b",host),request("/openrouter/nonce?code=a&error=denied",host),request("/openrouter/nonce?code=%00",host),request("/openrouter/nonce?code=",host),b"POST /openrouter/nonce?code=a HTTP/1.1\r\nHost: localhost:12345\r\n\r\n".to_vec(),b"GET /openrouter/nonce?code=a HTTP/1.1\r\nHost: localhost:12345\r\nHost: localhost:12345\r\n\r\n".to_vec(),b"GET /openrouter/nonce?code=a HTTP/1.1\r\nHost: localhost:12345\r\nContent-Length: 1\r\n\r\nx".to_vec()] {
            assert!(callback(&bytes,path,host).is_none());
        }
        assert!(matches!(
            callback(
                &request("/openrouter/nonce?error=access_denied", host),
                path,
                host
            ),
            Some(Callback::Denied)
        ));
    }

    #[test]
    fn full_loopback_flow_ignores_wrong_callback_exchanges_once_and_closes_listener() {
        let (endpoint, server) = exchange_server(r#"{"key":"fake-test-key"}"#, "200 OK");
        let (sender, receiver) = std::sync::mpsc::channel();
        let key = runtime()
            .block_on(connect_with(
                move |url| {
                    let url = reqwest::Url::parse(url).unwrap();
                    assert_eq!(url.scheme(), "https");
                    assert_eq!(url.host_str(), Some("openrouter.ai"));
                    let values = url
                        .query_pairs()
                        .collect::<std::collections::HashMap<_, _>>();
                    assert_eq!(values["code_challenge_method"], "S256");
                    assert_eq!(values["key_label"], "Bench");
                    let challenge = values["code_challenge"].to_string();
                    let callback = reqwest::Url::parse(&values["callback_url"]).unwrap();
                    assert_eq!(callback.host_str(), Some("localhost"));
                    sender.send((challenge, callback.port().unwrap())).unwrap();
                    thread::spawn(move || {
                        assert!(send_callback(&callback, "/wrong?code=wrong-secret")
                            .starts_with("HTTP/1.1 400"));
                        let response = send_callback(
                            &callback,
                            &format!("{}?code=authorization-code", callback.path()),
                        );
                        assert!(response.starts_with("HTTP/1.1 200"));
                        assert!(!response.contains("authorization-code"));
                        assert!(!response.contains("fake-test-key"));
                    });
                    Ok(())
                },
                Arc::new(AtomicBool::new(false)),
                &endpoint,
                Duration::from_secs(3),
            ))
            .unwrap();
        assert_eq!(key, "fake-test-key");
        let (expected_challenge, port) = receiver.recv().unwrap();
        let payload = server.join().unwrap();
        assert_eq!(payload["code"], "authorization-code");
        assert_eq!(payload["code_challenge_method"], "S256");
        assert_eq!(
            challenge(payload["code_verifier"].as_str().unwrap()),
            expected_challenge
        );
        assert!(StdStream::connect(("127.0.0.1", port)).is_err());
    }

    #[test]
    fn malformed_and_failed_exchange_never_echo_secrets() {
        for (body, status) in [
            (
                r#"{"error":"authorization-code verifier-secret"}"#,
                "403 Forbidden",
            ),
            (r#"{"key":null}"#, "200 OK"),
            (r#"{"key":""}"#, "200 OK"),
            (r#"{"key":"secret\nvalue"}"#, "200 OK"),
            ("not-json authorization-code", "200 OK"),
        ] {
            let (endpoint, server) = exchange_server(body, status);
            let error = runtime()
                .block_on(exchange(&endpoint, "authorization-code", "verifier-secret"))
                .unwrap_err();
            assert!(!error.contains("authorization-code"));
            assert!(!error.contains("verifier-secret"));
            assert!(!error.contains("secret\\nvalue"));
            server.join().unwrap();
        }
        let huge = format!("{{\"key\":\"{}\"}}", "x".repeat(MAX_RESPONSE));
        let (endpoint, server) = exchange_server(&huge, "200 OK");
        assert_eq!(
            runtime()
                .block_on(exchange(&endpoint, "code", "verifier"))
                .unwrap_err(),
            INVALID_EXCHANGE
        );
        server.join().unwrap();
    }

    #[test]
    fn redirects_do_not_forward_code_or_verifier() {
        let target = StdListener::bind("127.0.0.1:0").unwrap();
        target.set_nonblocking(true).unwrap();
        let source = StdListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/exchange", source.local_addr().unwrap());
        let location = format!("http://{}/steal", target.local_addr().unwrap());
        let server = thread::spawn(move || {
            let (mut socket, _) = source.accept().unwrap();
            let mut bytes = [0; 2048];
            let count = socket.read(&mut bytes).unwrap();
            assert!(count > 0);
            write!(socket,"HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
        });
        assert!(runtime()
            .block_on(exchange(&endpoint, "authorization-code", "verifier-secret"))
            .is_err());
        server.join().unwrap();
        assert_eq!(
            target.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn denied_browser_error_timeout_and_cancellation_are_recoverable_and_redacted() {
        let runtime = runtime();
        let error = runtime
            .block_on(connect_with(
                |_| Err("secret URL".into()),
                Arc::new(AtomicBool::new(false)),
                "http://127.0.0.1:1",
                Duration::from_secs(1),
            ))
            .unwrap_err();
        assert!(!error.contains("secret"));
        let error = runtime
            .block_on(connect_with(
                |_| Ok(()),
                Arc::new(AtomicBool::new(false)),
                "http://127.0.0.1:1",
                Duration::from_millis(20),
            ))
            .unwrap_err();
        assert!(error.contains("expired"));
        runtime.block_on(async {
            let cancelled = Arc::new(AtomicBool::new(false));
            let cancel = cancelled.clone();
            let connection = connect_with(
                |_| Ok(()),
                cancelled,
                "http://127.0.0.1:1",
                Duration::from_secs(5),
            );
            let trigger = async {
                tokio::time::sleep(Duration::from_millis(20)).await;
                cancel.store(true, Ordering::Release);
            };
            let (result, ()) = tokio::join!(connection, trigger);
            assert_eq!(result.unwrap_err(), CANCELLED);
        });
        assert_eq!(
            runtime
                .block_on(connect_with(
                    |_| panic!("cancelled flow must not open browser"),
                    Arc::new(AtomicBool::new(true)),
                    "http://127.0.0.1:1",
                    Duration::from_secs(1)
                ))
                .unwrap_err(),
            CANCELLED
        );
        let error = runtime
            .block_on(connect_with(
                |url| {
                    let url = reqwest::Url::parse(url).unwrap();
                    let values = url
                        .query_pairs()
                        .collect::<std::collections::HashMap<_, _>>();
                    let callback = reqwest::Url::parse(&values["callback_url"]).unwrap();
                    thread::spawn(move || {
                        send_callback(
                            &callback,
                            &format!("{}?error=denied-secret", callback.path()),
                        );
                    });
                    Ok(())
                },
                Arc::new(AtomicBool::new(false)),
                "http://127.0.0.1:1",
                Duration::from_secs(1),
            ))
            .unwrap_err();
        assert!(error.contains("not completed"));
        assert!(!error.contains("denied-secret"));
    }

    #[test]
    fn slow_and_oversized_callback_headers_are_bounded() {
        runtime().block_on(async {
            let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
                .await
                .unwrap();
            let sender = async {
                let mut socket = TcpStream::connect(listener.local_addr().unwrap())
                    .await
                    .unwrap();
                socket
                    .write_all(&vec![b'x'; MAX_HEADERS + 1])
                    .await
                    .unwrap();
            };
            let receiver = async {
                let (mut socket, _) = listener.accept().await.unwrap();
                assert!(read_headers(&mut socket).await.is_none());
            };
            tokio::join!(sender, receiver);
            let slow_sender = async {
                let socket = TcpStream::connect(listener.local_addr().unwrap())
                    .await
                    .unwrap();
                tokio::time::sleep(Duration::from_millis(50)).await;
                drop(socket);
            };
            let bounded_receiver = async {
                let (mut socket, _) = listener.accept().await.unwrap();
                assert!(
                    tokio::time::timeout(Duration::from_millis(20), read_headers(&mut socket))
                        .await
                        .is_err()
                );
            };
            tokio::join!(slow_sender, bounded_receiver);
        });
    }
}
