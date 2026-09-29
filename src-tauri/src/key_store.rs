use keyring::{Entry, Error};

use crate::provider::ProviderId;

const SERVICE: &str = "app.bench.desktop.api-key";

fn entry(provider: ProviderId) -> Result<Entry, String> {
    Entry::new(SERVICE, provider.as_str())
        .map_err(|error| format!("Could not access the system credential store: {error}"))
}

fn web_search_entry() -> Result<Entry, String> {
    Entry::new(SERVICE, "brave-search")
        .map_err(|error| format!("Could not access the system credential store: {error}"))
}

pub fn save(provider: ProviderId, key: &str) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() || key.len() > 4096 {
        return Err("Enter a valid API key (1–4096 characters).".into());
    }
    entry(provider)?
        .set_password(key)
        .map_err(|error| format!("Could not save the API key: {error}"))
}

pub fn load(provider: ProviderId) -> Result<Option<String>, String> {
    match entry(provider)?.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(Error::NoEntry) => Ok(None),
        Err(error) => Err(format!("Could not read the API key: {error}")),
    }
}

pub fn source(provider: ProviderId) -> Result<&'static str, String> {
    if load(provider)?.is_some() {
        Ok("keychain")
    } else if provider.environment_key().is_some() {
        Ok("environment")
    } else {
        Ok("none")
    }
}

pub fn exists(provider: ProviderId) -> Result<bool, String> {
    Ok(source(provider)? != "none")
}

pub fn active_key(provider: ProviderId) -> Result<String, String> {
    load(provider)?
        .or_else(|| provider.environment_key())
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| {
            format!(
                "Add a {} API key in Settings to enable chat.",
                provider.label()
            )
        })
}

pub fn remove(provider: ProviderId) -> Result<(), String> {
    match entry(provider)?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(error) => Err(format!("Could not remove the API key: {error}")),
    }
}

pub fn save_web_search_key(key: &str) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() || key.len() > 4096 {
        return Err("Enter a valid search API key (1–4096 characters).".into());
    }
    web_search_entry()?
        .set_password(key)
        .map_err(|error| format!("Could not save the search API key: {error}"))
}

pub fn web_search_key() -> Result<Option<String>, String> {
    match web_search_entry()?.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(Error::NoEntry) => Ok(std::env::var("BRAVE_SEARCH_API_KEY")
            .ok()
            .filter(|key| !key.trim().is_empty())),
        Err(error) => Err(format!("Could not read the search API key: {error}")),
    }
}

pub fn web_search_key_source() -> Result<&'static str, String> {
    match web_search_entry()?.get_password() {
        Ok(_) => Ok("keychain"),
        Err(Error::NoEntry) => Ok(
            if std::env::var("BRAVE_SEARCH_API_KEY")
                .ok()
                .is_some_and(|key| !key.trim().is_empty())
            {
                "environment"
            } else {
                "none"
            },
        ),
        Err(error) => Err(format!("Could not read the search API key: {error}")),
    }
}

pub fn remove_web_search_key() -> Result<(), String> {
    match web_search_entry()?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(error) => Err(format!("Could not remove the search API key: {error}")),
    }
}
