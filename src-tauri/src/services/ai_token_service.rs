//! AI access tokens: create / list / update / revoke, master switch, and
//! authentication of a presented secret. Only SHA-256 hashes are stored.

use std::io::Read;
use std::path::PathBuf;
use std::sync::Mutex;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};

use crate::models::ai::{AiCreatedToken, AiStore, AiToken, AiTokenInput, AiTokenView};
use crate::services::{ai_guard, fs_secure};

pub const SECRET_PREFIX: &str = "mssh_";
const MAX_TOKENS: usize = 50;
/// Serialises every read-modify-write of the store file.
static LOCK: Mutex<()> = Mutex::new(());

fn config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    PathBuf::from(home).join(".config").join("murmurssh")
}

fn store_path() -> PathBuf {
    config_dir().join("ai_access.json")
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn random_bytes(n: usize) -> Result<Vec<u8>, String> {
    let mut buf = vec![0u8; n];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut buf))
        .map_err(|e| format!("No randomness available: {}", e))?;
    Ok(buf)
}

pub fn random_hex(n: usize) -> Result<String, String> {
    Ok(random_bytes(n)?.iter().map(|b| format!("{:02x}", b)).collect())
}

fn hash_secret(secret: &str) -> String {
    Sha256::digest(secret.as_bytes()).iter().map(|b| format!("{:02x}", b)).collect()
}

fn ct_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// The store is read from disk ONCE per process and afterwards only changed through
/// the GUI commands. A process editing `ai_access.json` behind the app's back (e.g.
/// an AI with shell access) cannot grant itself permissions until MurmurSSH restarts.
static CACHE: Mutex<Option<AiStore>> = Mutex::new(None);

fn load() -> Result<AiStore, String> {
    let mut c = CACHE.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(s) = c.as_ref() {
        return Ok(s.clone());
    }
    let s = load_file()?;
    *c = Some(s.clone());
    Ok(s)
}

fn load_file() -> Result<AiStore, String> {
    let path = store_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).map_err(|e| format!("Failed to parse ai_access.json: {}", e)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AiStore::default()),
        Err(e) => Err(format!("Failed to read ai_access.json: {}", e)),
    }
}

fn save(store: &AiStore) -> Result<(), String> {
    fs_secure::private_dir_all(&config_dir()).map_err(|e| format!("Config dir: {}", e))?;
    let data = serde_json::to_vec_pretty(store).map_err(|e| e.to_string())?;
    fs_secure::write_private(&store_path(), &data)?;
    *CACHE.lock().unwrap_or_else(|p| p.into_inner()) = Some(store.clone());
    Ok(())
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|p| p.into_inner())
}

pub fn is_enabled() -> bool {
    load().map(|s| s.enabled).unwrap_or(false)
}

pub fn set_enabled(enabled: bool) -> Result<(), String> {
    let _g = lock();
    let mut store = load()?;
    store.enabled = enabled;
    save(&store)
}

pub fn list() -> Result<Vec<AiTokenView>, String> {
    Ok(load()?.tokens.iter().map(AiTokenView::from).collect())
}

/// Validated, normalised form of the GUI input.
struct Checked {
    name: String,
    profiles: Vec<String>,
    remote_root: Option<String>,
    local_root: Option<String>,
}

fn check_input(input: &AiTokenInput) -> Result<Checked, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() || name.chars().count() > 64 || name.chars().any(|c| c.is_control()) {
        return Err("Token name must be 1–64 printable characters".to_string());
    }
    if input.profiles.is_empty() {
        return Err("Select at least one profile".to_string());
    }
    let mut profiles = Vec::new();
    for p in &input.profiles {
        fs_secure::check_id(p)?;
        if !profiles.contains(p) {
            profiles.push(p.clone());
        }
    }
    let remote_root = match input.remote_root.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(r) => Some(ai_guard::normalize_remote(r)?),
    };
    let local_root = match input.local_root.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(l) => {
            let c = std::fs::canonicalize(l).map_err(|e| format!("Local root '{}': {}", l, e))?;
            if !c.is_dir() {
                return Err(format!("Local root '{}' is not a directory", l));
            }
            let home = std::env::var("HOME").ok().and_then(|h| std::fs::canonicalize(h).ok());
            if c == std::path::Path::new("/") || home.as_ref().is_some_and(|h| h.starts_with(&c)) {
                return Err("Local folder must be a project folder below your home, not '/' or your home itself".to_string());
            }
            Some(c.to_string_lossy().into_owned())
        }
    };
    if let Some(d) = input.expires_in_days {
        if d == 0 || d > 3650 {
            return Err("Expiry must be 1–3650 days".to_string());
        }
    }
    Ok(Checked { name, profiles, remote_root, local_root })
}

pub fn create(input: AiTokenInput) -> Result<AiCreatedToken, String> {
    let c = check_input(&input)?;
    let _g = lock();
    let mut store = load()?;
    if store.tokens.len() >= MAX_TOKENS {
        return Err(format!("At most {} tokens", MAX_TOKENS));
    }
    let secret = format!("{}{}", SECRET_PREFIX, URL_SAFE_NO_PAD.encode(random_bytes(32)?));
    let created_at = now();
    let token = AiToken {
        id: random_hex(8)?,
        name: c.name,
        hash: hash_secret(&secret),
        prefix: secret[..SECRET_PREFIX.len() + 6].to_string(),
        profiles: c.profiles,
        caps: input.caps,
        remote_root: c.remote_root,
        local_root: c.local_root,
        created_at,
        expires_at: input.expires_in_days.map(|d| created_at + d as u64 * 86_400),
        last_used_at: None,
    };
    let view = AiTokenView::from(&token);
    store.tokens.push(token);
    save(&store)?;
    Ok(AiCreatedToken { token: view, secret })
}

pub fn update(id: &str, input: AiTokenInput) -> Result<AiTokenView, String> {
    let c = check_input(&input)?;
    let _g = lock();
    let mut store = load()?;
    let t = store.tokens.iter_mut().find(|t| t.id == id).ok_or("Token not found")?;
    t.name = c.name;
    t.profiles = c.profiles;
    t.caps = input.caps;
    t.remote_root = c.remote_root;
    t.local_root = c.local_root;
    t.expires_at = input.expires_in_days.map(|d| now() + d as u64 * 86_400);
    let view = AiTokenView::from(&*t);
    save(&store)?;
    Ok(view)
}

pub fn revoke(id: &str) -> Result<(), String> {
    let _g = lock();
    let mut store = load()?;
    let before = store.tokens.len();
    store.tokens.retain(|t| t.id != id);
    if store.tokens.len() == before {
        return Err("Token not found".to_string());
    }
    save(&store)
}

/// Drop a deleted profile from every token so a later profile with the same id
/// does not silently inherit AI access.
pub fn forget_profile(profile_id: &str) {
    let _g = lock();
    if let Ok(mut store) = load() {
        let mut changed = false;
        for t in store.tokens.iter_mut() {
            let n = t.profiles.len();
            t.profiles.retain(|p| p != profile_id);
            changed |= t.profiles.len() != n;
        }
        if changed {
            let _ = save(&store);
        }
    }
}

/// True while the token still exists, is unexpired and AI access is on — checked
/// again after a human approved, so revoking during an open dialog still wins.
pub fn still_valid(id: &str) -> bool {
    let _g = lock();
    load().is_ok_and(|s| s.enabled && s.tokens.iter().any(|t| t.id == id && t.expires_at.is_none_or(|e| now() < e)))
}

/// Authenticate a presented secret. Fails when the master switch is off, the
/// token is unknown, or it has expired. Records `last_used_at` (at most once a minute).
pub fn authenticate(secret: &str) -> Result<AiToken, String> {
    let _g = lock();
    let mut store = load()?;
    if !store.enabled {
        return Err("AI access is switched off in MurmurSSH".to_string());
    }
    if !secret.starts_with(SECRET_PREFIX) || secret.len() > 128 {
        return Err("Invalid token".to_string());
    }
    let h = hash_secret(secret);
    let now = now();
    let mut found: Option<usize> = None;
    for (i, t) in store.tokens.iter().enumerate() {
        // Compare against every token so timing does not reveal a position.
        if ct_eq(&t.hash, &h) {
            found = Some(i);
        }
    }
    let i = found.ok_or("Invalid token")?;
    if let Some(exp) = store.tokens[i].expires_at {
        if now >= exp {
            return Err("Token has expired".to_string());
        }
    }
    if store.tokens[i].last_used_at.is_none_or(|t| now.saturating_sub(t) >= 60) {
        store.tokens[i].last_used_at = Some(now);
        let _ = save(&store);
    }
    Ok(store.tokens[i].clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_compare() {
        let a = hash_secret("mssh_x");
        assert_eq!(a.len(), 64);
        assert!(ct_eq(&a, &hash_secret("mssh_x")));
        assert!(!ct_eq(&a, &hash_secret("mssh_y")));
        assert!(!ct_eq("ab", "abc"));
    }

    #[test]
    fn input_validation() {
        let base = AiTokenInput {
            name: "Claude".into(),
            profiles: vec!["web-1".into()],
            caps: Default::default(),
            remote_root: Some("/var/www/../www/site/".into()),
            local_root: None,
            expires_in_days: Some(7),
        };
        let c = check_input(&base).unwrap();
        assert_eq!(c.remote_root.as_deref(), Some("/var/www/site"));

        let mut bad = base.clone();
        bad.profiles = vec![];
        assert!(check_input(&bad).is_err());
        let mut bad = base.clone();
        bad.profiles = vec!["../x".into()];
        assert!(check_input(&bad).is_err());
        let mut bad = base.clone();
        bad.name = "  ".into();
        assert!(check_input(&bad).is_err());
        let mut bad = base.clone();
        bad.expires_in_days = Some(0);
        assert!(check_input(&bad).is_err());
        let mut bad = base.clone();
        bad.local_root = Some("/".into());
        assert!(check_input(&bad).is_err());
        if let Ok(home) = std::env::var("HOME") {
            let mut bad = base.clone();
            bad.local_root = Some(home);
            assert!(check_input(&bad).is_err(), "home itself is not a valid local root");
        }
        let mut bad = base;
        bad.remote_root = Some("relative".into());
        assert!(check_input(&bad).is_err());
    }
}
