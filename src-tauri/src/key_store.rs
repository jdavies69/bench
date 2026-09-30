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
    source_from_presence(stored_key_exists(provider.as_str()), || {
        provider.environment_key().is_some()
    })
}

fn source_from_presence(
    stored: Result<bool, String>,
    environment_exists: impl FnOnce() -> bool,
) -> Result<&'static str, String> {
    if stored? {
        Ok("keychain")
    } else if environment_exists() {
        Ok("environment")
    } else {
        Ok("none")
    }
}

#[cfg(target_os = "macos")]
fn stored_key_exists(account: &str) -> Result<bool, String> {
    use security_framework::{
        item::{ItemClass, ItemSearchOptions},
        os::macos::keychain::{SecKeychain, SecPreferencesDomain},
    };

    // Match keyring's default User-domain keychain. Startup needs only status;
    // requesting password data here would trigger the item's access control.
    let keychain = SecKeychain::default_for_domain(SecPreferencesDomain::User)
        .map_err(|error| format!("Could not inspect the API key: {error}"))?;
    let result = ItemSearchOptions::new()
        .keychains(&[keychain])
        .class(ItemClass::generic_password())
        .service(SERVICE)
        .account(account)
        .load_attributes(true)
        .load_data(false)
        .load_refs(false)
        .search();
    match result {
        Ok(items) => Ok(!items.is_empty()),
        Err(error) if error.code() == -25300 => Ok(false), // errSecItemNotFound
        Err(error) => Err(format!("Could not inspect the API key: {error}")),
    }
}

#[cfg(not(target_os = "macos"))]
fn stored_key_exists(account: &str) -> Result<bool, String> {
    let credential = Entry::new(SERVICE, account)
        .map_err(|error| format!("Could not access the system credential store: {error}"))?;
    match credential.get_password() {
        Ok(_) => Ok(true),
        Err(Error::NoEntry) => Ok(false),
        Err(error) => Err(format!("Could not read the API key: {error}")),
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

#[allow(dead_code)] // Legacy Brave credential remains in Keychain, but is not used.
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
    source_from_presence(stored_key_exists("brave-search"), || {
        std::env::var("BRAVE_SEARCH_API_KEY")
            .ok()
            .is_some_and(|key| !key.trim().is_empty())
    })
}

pub fn remove_web_search_key() -> Result<(), String> {
    match web_search_entry()?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(error) => Err(format!("Could not remove the search API key: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_prefers_saved_metadata_without_loading_environment() {
        assert_eq!(
            source_from_presence(Ok(true), || panic!("saved key takes precedence")).unwrap(),
            "keychain"
        );
        assert_eq!(
            source_from_presence(Ok(false), || true).unwrap(),
            "environment"
        );
        assert_eq!(source_from_presence(Ok(false), || false).unwrap(), "none");
    }

    #[test]
    fn metadata_errors_remain_errors_instead_of_hiding_saved_credentials() {
        assert_eq!(
            source_from_presence(Err("Keychain unavailable".into()), || {
                panic!("do not silently switch credentials after a Keychain error")
            })
            .unwrap_err(),
            "Keychain unavailable"
        );
    }
}
