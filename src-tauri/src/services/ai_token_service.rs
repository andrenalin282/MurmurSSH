//! AI access tokens: create / list / update / revoke, master switch, and
//! authentication of a presented secret. Only SHA-256 hashes are stored.

use std::io::Read;
use std::path::PathBuf;
use std::sync::Mutex;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};

use crate::models::ai::{AiCreatedToken, AiGrant, AiGrantChoice, AiStore, AiToken, AiTokenInput, AiTokenView};
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
    let mut s = load_file()?;
    // A new process = a new app session: drop "until restart" and expired grants.
    let t = now();
    for tok in s.tokens.iter_mut() {
        tok.grants.retain(|g| !g.session_only && grant_live(g, t));
    }
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
    if input.profiles.is_empty() && !input.all_profiles {
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
        all_profiles: input.all_profiles,
        caps: input.caps,
        remote_root: c.remote_root,
        local_root: c.local_root,
        created_at,
        expires_at: input.expires_in_days.map(|d| created_at + d as u64 * 86_400),
        last_used_at: None,
        grants: Vec::new(),
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
    t.all_profiles = input.all_profiles;
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
    save(&store)?;
    // The token's scratch folder goes with it.
    let _ = std::fs::remove_dir_all(staging_dir(id));
    Ok(())
}

/// Per-token scratch folder for transfers: ~/.local/share/murmurssh/ai-staging/<id>.
pub fn staging_dir(token_id: &str) -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    std::path::Path::new(&home).join(".local/share/murmurssh/ai-staging").join(token_id)
}

/// Create (0700) and return the token's scratch folder.
pub fn ensure_staging_dir(token_id: &str) -> Result<std::path::PathBuf, String> {
    let d = staging_dir(token_id);
    fs_secure::private_dir_all(&d).map_err(|e| format!("Staging folder: {}", e))?;
    Ok(d)
}

pub const GRANT_ACTIONS: &[&str] = &["upload", "download", "write", "exec", "delete"];

/// Add a standing approval (from a confirmation dialog). `all` never covers delete.
pub fn add_grant(token_id: &str, profile: &str, action: &str, choice: &AiGrantChoice) -> Result<(), String> {
    let action = match choice.scope.as_str() {
        "all" => "all",
        "action" if GRANT_ACTIONS.contains(&action) => action,
        _ => return Err("Invalid grant scope".to_string()),
    };
    let t = now();
    let (expires_at, session_only) = match choice.duration.as_str() {
        "hour" => (Some(t + 3600), false),
        "session" => (None, true),
        // Delete is never granted without an end (max. 1 h or until restart).
        "forever" if action == "delete" => return Err("Delete can only be allowed for 1 hour or until restart".to_string()),
        "forever" => (None, false),
        _ => return Err("Invalid grant duration".to_string()),
    };
    let _g = lock();
    let mut store = load()?;
    let tok = store.tokens.iter_mut().find(|x| x.id == token_id).ok_or("Token not found")?;
    tok.grants.retain(|g| !(g.profile == profile && g.action == action));
    tok.grants.push(AiGrant {
        id: random_hex(6)?,
        profile: profile.to_string(),
        action: action.to_string(),
        created_at: t,
        expires_at,
        session_only,
    });
    save(&store)
}

pub fn revoke_grant(token_id: &str, grant_id: &str) -> Result<(), String> {
    let _g = lock();
    let mut store = load()?;
    let tok = store.tokens.iter_mut().find(|x| x.id == token_id).ok_or("Token not found")?;
    let n = tok.grants.len();
    tok.grants.retain(|g| g.id != grant_id);
    if tok.grants.len() == n {
        return Err("Approval not found".to_string());
    }
    save(&store)
}

/// Is a stored grant usable at time `t`? Not expired, and never an endless delete
/// grant: those could be stored before delete was limited to 1 h / until restart,
/// so they are rejected on load AND at use time, not only when created.
fn grant_live(g: &AiGrant, t: u64) -> bool {
    let endless_delete = g.action == "delete" && g.expires_at.is_none() && !g.session_only;
    !endless_delete && g.expires_at.is_none_or(|e| t < e)
}

/// Active standing approvals of a token for one profile (fresh from the store).
pub fn active_grants(token_id: &str, profile: &str) -> Vec<AiGrant> {
    let _g = lock();
    let t = now();
    load()
        .ok()
        .and_then(|s| s.tokens.into_iter().find(|x| x.id == token_id))
        .map(|tok| {
            tok.grants
                .into_iter()
                .filter(|g| g.profile == profile && grant_live(g, t))
                .collect()
        })
        .unwrap_or_default()
}

/// Is `action` on `profile` pre-approved? Delete only by its own grant.
pub fn is_granted(token_id: &str, profile: &str, action: &str) -> bool {
    active_grants(token_id, profile)
        .iter()
        .any(|g| g.action == action || (g.action == "all" && action != "delete"))
}

/// Drop a deleted profile from every token so a later profile with the same id
/// does not silently inherit AI access.
pub fn forget_profile(profile_id: &str) {
    let _g = lock();
    if let Ok(mut store) = load() {
        let mut changed = false;
        for t in store.tokens.iter_mut() {
            let n = t.profiles.len() + t.grants.len();
            t.profiles.retain(|p| p != profile_id);
            t.grants.retain(|g| g.profile != profile_id);
            changed |= t.profiles.len() + t.grants.len() != n;
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
    fn loaded_endless_delete_grant_is_dropped() {
        // As stored by the previous version (delete "forever").
        let old: AiGrant = serde_json::from_str(
            r#"{"id":"a1","profile":"p","action":"delete","created_at":1,"expires_at":null,"session_only":false}"#,
        )
        .unwrap();
        assert!(!grant_live(&old, 100));
        let hour = AiGrant { expires_at: Some(200), ..old.clone() };
        assert!(grant_live(&hour, 100));
        assert!(!grant_live(&hour, 200));
        let session = AiGrant { session_only: true, ..old.clone() };
        assert!(grant_live(&session, 100));
        let upload = AiGrant { action: "upload".into(), ..old };
        assert!(grant_live(&upload, 100));
    }

    #[test]
    fn delete_grant_needs_an_end() {
        let forever = AiGrantChoice { scope: "action".into(), duration: "forever".into() };
        // Refused before the store is touched (token id does not need to exist).
        let err = add_grant("no-such-token", "p", "delete", &forever).unwrap_err();
        assert!(err.contains("Delete"), "{err}");
    }

    #[test]
    fn input_validation() {
        let base = AiTokenInput {
            name: "Claude".into(),
            profiles: vec!["web-1".into()],
            all_profiles: false,
            caps: Default::default(),
            remote_root: Some("/var/www/../www/site/".into()),
            local_root: None,
            expires_in_days: Some(7),
        };
        let c = check_input(&base).unwrap();
        assert_eq!(c.remote_root.as_deref(), Some("/var/www/site"));

        let mut all = base.clone();
        all.profiles.clear();
        assert!(check_input(&all).is_err(), "no profile and not all_profiles");
        all.all_profiles = true;
        assert!(check_input(&all).is_ok(), "all_profiles needs no list");

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
