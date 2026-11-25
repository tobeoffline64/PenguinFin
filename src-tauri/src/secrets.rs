//! Where the FRED API key is kept: the OS keychain when there is one, otherwise a file in the
//! data folder that only this user can read. The key never goes to the interface.

use std::path::PathBuf;

const SERVICE: &str = "io.github.tobeoffline64.PenguinFin";
const ACCOUNT: &str = "fred_api_key";

pub const IN_KEYCHAIN: &str = "the system keychain";
pub const IN_FILE: &str = "a private file in PenguinFin's data folder";

fn fallback_file() -> PathBuf {
    penguinfin_store::db::data_dir().join("fred-key")
}

#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
fn entry() -> Option<keyring::Entry> {
    // Tests and headless runs can opt out of the keychain.
    if std::env::var_os("PENGUINFIN_NO_KEYCHAIN").is_some() {
        return None;
    }
    keyring::Entry::new(SERVICE, ACCOUNT).ok()
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn entry() -> Option<NoKeychain> {
    None
}

/// Phones get a keychain plugin in phase 5; until then they use the file.
#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
struct NoKeychain;
#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
impl NoKeychain {
    fn get_password(&self) -> Result<String, String> {
        Err(String::new())
    }
    fn set_password(&self, _: &str) -> Result<(), String> {
        Err(String::new())
    }
    fn delete_credential(&self) -> Result<(), String> {
        Ok(())
    }
}

/// The saved key and where it is kept.
pub fn get() -> Option<(String, &'static str)> {
    if let Some(e) = entry()
        && let Ok(k) = e.get_password()
        && !k.trim().is_empty()
    {
        return Some((k, IN_KEYCHAIN));
    }
    std::fs::read_to_string(fallback_file()).ok().map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).map(|k| (k, IN_FILE))
}

/// Save the key, preferring the keychain. Returns where it went.
pub fn set(key: &str) -> Result<&'static str, String> {
    if let Some(e) = entry()
        && e.set_password(key).is_ok()
        && e.get_password().ok().as_deref() == Some(key)
    {
        let _ = std::fs::remove_file(fallback_file());
        return Ok(IN_KEYCHAIN);
    }
    write_private(&fallback_file(), key)?;
    Ok(IN_FILE)
}

pub fn remove() {
    if let Some(e) = entry() {
        let _ = e.delete_credential();
    }
    let _ = std::fs::remove_file(fallback_file());
}

fn write_private(path: &PathBuf, text: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(|e| e.to_string())?;
        f.write_all(text.as_bytes()).map_err(|e| e.to_string())
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, text).map_err(|e| e.to_string())
    }
}
