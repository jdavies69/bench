//! Signed in-place updates. Credentials and generated work are outside the bundle.
use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

const MAX_PACKAGE: usize = 128 * 1024 * 1024;
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub status: String,
    pub version: Option<String>,
    pub automatic: bool,
    pub message: String,
}
impl Default for UpdateStatus {
    fn default() -> Self {
        Self {
            status: "idle".into(),
            version: None,
            automatic: true,
            message: String::new(),
        }
    }
}
#[derive(Default)]
pub struct UpdateState {
    status: Mutex<UpdateStatus>,
    pending: Mutex<Option<(Update, Vec<u8>)>>,
    busy: AtomicBool,
}
struct Busy<'a>(&'a AtomicBool);
impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
fn lock<T>(value: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, String> {
    value
        .lock()
        .map_err(|_| "App updates are unavailable.".into())
}
fn configured(app: &AppHandle) -> bool {
    let Some(config) = app.config().plugins.0.get("updater") else {
        return false;
    };
    config
        .get("pubkey")
        .and_then(|v| v.as_str())
        .is_some_and(|v| !v.trim().is_empty())
        && config
            .get("endpoints")
            .and_then(|v| v.as_array())
            .is_some_and(|urls| {
                !urls.is_empty()
                    && urls
                        .iter()
                        .all(|url| url.as_str().is_some_and(valid_endpoint))
            })
}
fn valid_endpoint(value: &str) -> bool {
    reqwest::Url::parse(value).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
    })
}
fn automatic(state: &crate::commands::AppState) -> Result<bool, String> {
    let conn = lock(&state.db)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS app_preferences (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
    )
    .map_err(|_| "Update settings are unavailable.")?;
    match conn.query_row(
        "SELECT value FROM app_preferences WHERE key='automatic_updates'",
        [],
        |row| row.get::<_, String>(0),
    ) {
        Ok(value) => Ok(value == "true"),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(true),
        Err(_) => Err("Update settings are unavailable.".into()),
    }
}
#[tauri::command]
pub fn app_update_status(
    app: AppHandle,
    state: State<'_, UpdateState>,
    product: State<'_, crate::commands::AppState>,
) -> Result<UpdateStatus, String> {
    let mut status = lock(&state.status)?.clone();
    status.automatic = automatic(&product)?;
    if !configured(&app) {
        status.status = "unconfigured".into();
        status.message =
            "Automatic updates are being prepared. No release channel is configured yet.".into();
    }
    Ok(status)
}
#[tauri::command]
pub fn set_automatic_updates(
    product: State<'_, crate::commands::AppState>,
    enabled: bool,
) -> Result<(), String> {
    automatic(&product)?;
    lock(&product.db)?.execute("INSERT INTO app_preferences(key,value) VALUES('automatic_updates',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [if enabled { "true" } else { "false" }]).map_err(|_| "Could not save update settings.")?;
    Ok(())
}
#[tauri::command]
pub async fn check_app_update(
    app: AppHandle,
    state: State<'_, UpdateState>,
    product: State<'_, crate::commands::AppState>,
) -> Result<UpdateStatus, String> {
    if !configured(&app) {
        return app_update_status(app, state, product);
    }
    if state.busy.swap(true, Ordering::AcqRel) {
        return Err("An update is already being checked.".into());
    }
    let _busy = Busy(&state.busy);
    if lock(&state.pending)?.is_some() {
        drop(_busy);
        return app_update_status(app, state, product);
    }
    lock(&state.status)?.status = "checking".into();
    let result: Result<(), String> = async {
        let updater = app
            .updater_builder()
            .timeout(Duration::from_secs(30))
            .configure_client(|client| {
                client
                    .https_only(true)
                    .connect_timeout(Duration::from_secs(15))
            })
            .build()
            .map_err(|_| "App updates are unavailable.")?;
        let Some(update) = updater
            .check()
            .await
            .map_err(|_| "Could not check for updates. Try again later.")?
        else {
            let mut status = lock(&state.status)?;
            status.status = "current".into();
            status.version = None;
            status.message = "Bench is up to date.".into();
            return Ok(());
        };
        if !valid_endpoint(update.download_url.as_str()) || update.version.len() > 100 {
            return Err("The update response is invalid.".into());
        }
        {
            let mut status = lock(&state.status)?;
            status.status = "downloading".into();
            status.version = Some(update.version.clone());
            status.message = "Downloading a signed update…".into();
        }
        let public_key = app.config().plugins.0["updater"]["pubkey"]
            .as_str()
            .ok_or("Update verification is unavailable.")?;
        let client = reqwest::Client::builder()
            .https_only(true)
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|_| "Update download is unavailable.")?;
        let bytes = download_verified(
            &client,
            update.download_url.as_str(),
            &update.signature,
            public_key,
            &update.version,
            MAX_PACKAGE,
        )
        .await?;
        *lock(&state.pending)? = Some((update, bytes));
        let mut status = lock(&state.status)?;
        status.status = "ready".into();
        status.message = "Update downloaded and verified. Restart when you’re ready.".into();
        Ok(())
    }
    .await;
    if let Err(error) = result {
        let mut status = lock(&state.status)?;
        status.status = "error".into();
        status.message = error;
    }
    drop(_busy);
    app_update_status(app, state, product)
}
#[tauri::command]
pub fn install_app_update(
    app: AppHandle,
    state: State<'_, UpdateState>,
    product: State<'_, crate::commands::AppState>,
    attachments: State<'_, crate::attachments::AttachmentState>,
) -> Result<(), String> {
    if state.busy.swap(true, Ordering::AcqRel) {
        return Err("An update is already in progress.".into());
    }
    let _busy = Busy(&state.busy);
    if attachments.has_staged()? {
        return Err("Send or remove your selected attachments before restarting.".into());
    }
    let connection = lock(&product.oauth_connection)?;
    if connection.is_some() {
        return Err("Finish or cancel OpenRouter sign-in before restarting.".into());
    }
    let mut operations = lock(&product.active_operations)?;
    if !operations.is_empty() {
        return Err("Wait for the current request to finish before restarting.".into());
    }
    let mut pending = lock(&state.pending)?;
    let (update, bytes) = pending.as_ref().ok_or("Check for an update first.")?;
    update.install(bytes).map_err(|_| {
        "Could not install the update. Try again; your saved work stays on this Mac."
    })?;
    *pending = None;
    operations.insert("__app_update__".into());
    drop(pending);
    drop(operations);
    drop(connection);
    app.restart()
}

async fn download_verified(
    client: &reqwest::Client,
    url: &str,
    signature: &str,
    public_key: &str,
    version: &str,
    limit: usize,
) -> Result<Vec<u8>, String> {
    use futures_util::StreamExt;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|_| "Could not download the update. Try again later.")?;
    if !response.status().is_success() {
        return Err("The update download is unavailable.".into());
    }
    if response
        .content_length()
        .is_some_and(|size| size > limit as u64)
    {
        return Err("The update package is too large.".into());
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "The update download was interrupted.")?;
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err("The update package is too large.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    verify_package(&bytes, signature, public_key, version)?;
    Ok(bytes)
}
fn verify_package(
    bytes: &[u8],
    signature: &str,
    public_key: &str,
    version: &str,
) -> Result<(), String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let decode = |value: &str| {
        STANDARD
            .decode(value)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .ok_or("The update signature is invalid.")
    };
    let key = minisign_verify::PublicKey::decode(&decode(public_key)?)
        .map_err(|_| "The update signature is invalid.")?;
    let signature = minisign_verify::Signature::decode(&decode(signature)?)
        .map_err(|_| "The update signature is invalid.")?;
    key.verify(bytes, &signature, true)
        .map_err(|_| "The update could not be verified. Your installed app is unchanged.")?;
    let signed = signature
        .trusted_comment()
        .split('\t')
        .find_map(|field| field.strip_prefix("version:"));
    if signed.map(|value| value.trim_start_matches('v')) != Some(version.trim_start_matches('v')) {
        return Err("The signed update version does not match the release.".into());
    }
    Ok(())
}

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(5)).await;
        loop {
            let product = app.state::<crate::commands::AppState>();
            if automatic(&product).unwrap_or(false) && configured(&app) {
                let state = app.state::<UpdateState>();
                let _ = check_app_update(app.clone(), state, product).await;
            }
            tokio::time::sleep(Duration::from_secs(6 * 60 * 60)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_https_endpoints_without_embedded_credentials_are_accepted() {
        assert!(valid_endpoint(
            "https://github.com/owner/releases/latest/download/latest.json"
        ));
        for url in [
            "http://localhost/feed",
            "https://secret@example.com/feed",
            "https://example.com/feed#fragment",
            "file:///tmp/feed",
            "invalid",
        ] {
            assert!(!valid_endpoint(url));
        }
    }
    #[test]
    fn busy_guard_releases_after_failure() {
        let busy = AtomicBool::new(true);
        {
            let _guard = Busy(&busy);
        }
        assert!(!busy.load(Ordering::Acquire));
    }
}

#[cfg(test)]
mod verification_tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine};
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    // These fixtures use a disposable test key whose private part was deleted.
    const PAYLOAD: &[u8] = include_bytes!("../testdata/update-test.payload");
    const KEY: &str = include_str!("../testdata/update-test.pub");
    const SIGNATURE: &str = include_str!("../testdata/update-test.sig");
    const LEGACY_SIGNATURE: &str = include_str!("../testdata/update-test-no-version.sig");

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }
    fn serve(response: Vec<u8>, delay: Option<Duration>) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/fixture", listener.local_addr().unwrap());
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = [0; 4096];
            let _ = stream.read(&mut request);
            if let Some(delay) = delay {
                let split = response
                    .windows(4)
                    .position(|bytes| bytes == b"\r\n\r\n")
                    .unwrap()
                    + 4;
                stream.write_all(&response[..split]).unwrap();
                thread::sleep(delay);
                // The body timeout intentionally closes the client before this write.
                let _ = stream.write_all(&response[split..]);
            } else {
                let _ = stream.write_all(&response);
            }
        });
        (url, worker)
    }
    fn fetch(
        response: Vec<u8>,
        limit: usize,
        delay: Option<Duration>,
        timeout: Duration,
    ) -> Result<Vec<u8>, String> {
        let (url, worker) = serve(response, delay);
        // HTTP is allowed only by this private loopback fixture client.
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let result = runtime().block_on(download_verified(
            &client,
            &url,
            SIGNATURE.trim(),
            KEY.trim(),
            "9.8.7",
            limit,
        ));
        worker.join().unwrap();
        result
    }
    fn response(status: &str, body: &[u8], length: usize) -> Vec<u8> {
        let mut response =
            format!("HTTP/1.1 {status}\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n")
                .into_bytes();
        response.extend_from_slice(body);
        response
    }
    #[test]
    fn signed_payload_requires_correct_bytes_key_and_bound_version() {
        verify_package(PAYLOAD, SIGNATURE.trim(), KEY.trim(), "9.8.7").unwrap();
        verify_package(PAYLOAD, SIGNATURE.trim(), KEY.trim(), "v9.8.7").unwrap();
        let mut changed = PAYLOAD.to_vec();
        changed[0] ^= 1;
        assert!(verify_package(&changed, SIGNATURE.trim(), KEY.trim(), "9.8.7").is_err());
        assert!(verify_package(PAYLOAD, SIGNATURE.trim(), KEY.trim(), "9.8.8").is_err());
        assert!(verify_package(PAYLOAD, LEGACY_SIGNATURE.trim(), KEY.trim(), "9.8.7").is_err());
        // Mutate the valid decoded public key, keeping its structural encoding.
        let key_text = String::from_utf8(STANDARD.decode(KEY.trim()).unwrap()).unwrap();
        let lines: Vec<_> = key_text.lines().collect();
        let mut raw_key = STANDARD.decode(lines[1]).unwrap();
        *raw_key.last_mut().unwrap() ^= 1;
        let wrong_key = STANDARD.encode(format!("{}\n{}\n", lines[0], STANDARD.encode(raw_key)));
        assert!(verify_package(PAYLOAD, SIGNATURE.trim(), &wrong_key, "9.8.7").is_err());
        for bad in ["", "not base64", "c2VjcmV0LWVycm9yLWJvZHk="] {
            let error = verify_package(PAYLOAD, bad, KEY.trim(), "9.8.7").unwrap_err();
            assert_eq!(error, "The update signature is invalid.");
        }
    }
    #[test]
    fn bounded_download_returns_only_verified_payload() {
        let result = fetch(
            response("200 OK", PAYLOAD, PAYLOAD.len()),
            PAYLOAD.len(),
            None,
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(result, PAYLOAD);
        let mut changed = PAYLOAD.to_vec();
        changed[0] ^= 1;
        assert!(fetch(
            response("200 OK", &changed, changed.len()),
            changed.len(),
            None,
            Duration::from_secs(2)
        )
        .unwrap_err()
        .contains("could not be verified"));
    }
    #[test]
    fn declared_and_chunked_bodies_cannot_exceed_limit() {
        assert_eq!(
            fetch(response("200 OK", b"", 9), 8, None, Duration::from_secs(2)).unwrap_err(),
            "The update package is too large."
        );
        let chunked = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n4\r\n1234\r\n5\r\n56789\r\n0\r\n\r\n".to_vec();
        assert_eq!(
            fetch(chunked, 8, None, Duration::from_secs(2)).unwrap_err(),
            "The update package is too large."
        );
    }
    #[test]
    fn transport_failures_hide_upstream_bodies_and_reject_truncation() {
        let secret = b"upstream-private-token-or-stacktrace";
        assert_eq!(
            fetch(
                response("403 Forbidden", secret, secret.len()),
                4096,
                None,
                Duration::from_secs(2)
            )
            .unwrap_err(),
            "The update download is unavailable."
        );
        assert_eq!(
            fetch(
                response("200 OK", secret, secret.len() + 1),
                4096,
                None,
                Duration::from_secs(2)
            )
            .unwrap_err(),
            "The update download was interrupted."
        );
        assert_eq!(
            fetch(
                response("200 OK", PAYLOAD, PAYLOAD.len()),
                4096,
                Some(Duration::from_millis(300)),
                Duration::from_millis(100)
            )
            .unwrap_err(),
            "The update download was interrupted."
        );
    }
}
