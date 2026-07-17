use crate::models::update::UpdateCheckResult;

const RELEASES_LATEST_URL: &str =
    "https://api.github.com/repos/andrenalin282/MurmurSSH/releases/latest";
const TIMEOUT_SECS: u64 = 8;

#[derive(serde::Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
}

pub fn strip_v_prefix(tag: &str) -> &str {
    tag.strip_prefix('v')
        .or_else(|| tag.strip_prefix('V'))
        .unwrap_or(tag)
}

/// Parse leading major.minor.patch; ignore any trailing pre-release/build suffix after '-'/'+'.
pub fn parse_semver(s: &str) -> Result<(u64, u64, u64), String> {
    let core = s.split(['-', '+']).next().unwrap_or(s).trim();
    let mut parts = core.split('.');
    let major = parts
        .next()
        .ok_or_else(|| format!("invalid version: {s}"))?
        .parse::<u64>()
        .map_err(|_| format!("invalid version: {s}"))?;
    let minor = parts
        .next()
        .ok_or_else(|| format!("invalid version: {s}"))?
        .parse::<u64>()
        .map_err(|_| format!("invalid version: {s}"))?;
    let patch = parts
        .next()
        .ok_or_else(|| format!("invalid version: {s}"))?
        .parse::<u64>()
        .map_err(|_| format!("invalid version: {s}"))?;
    Ok((major, minor, patch))
}

pub fn is_newer(latest: &str, current: &str) -> Result<bool, String> {
    let l = parse_semver(strip_v_prefix(latest))?;
    let c = parse_semver(strip_v_prefix(current))?;
    Ok(l > c)
}

pub fn check_latest(current_version: &str) -> Result<UpdateCheckResult, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
        .build();

    let resp = agent
        .get(RELEASES_LATEST_URL)
        .set("User-Agent", &format!("MurmurSSH/{current_version}"))
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("update check failed: {e}"))?;

    if !(200..300).contains(&resp.status()) {
        return Err(format!("update check failed: HTTP {}", resp.status()));
    }

    let release: GithubRelease = resp
        .into_json()
        .map_err(|e| format!("update check failed: {e}"))?;

    if release.tag_name.trim().is_empty() {
        return Err("update check failed: empty tag_name".to_string());
    }

    let latest_version = strip_v_prefix(release.tag_name.trim()).to_string();
    let update_available = is_newer(&latest_version, current_version)?;

    if !release.html_url.starts_with("https://") && !release.html_url.starts_with("http://") {
        return Err("update check failed: invalid release URL".to_string());
    }

    Ok(UpdateCheckResult {
        update_available,
        current_version: current_version.to_string(),
        latest_version,
        release_url: release.html_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_v() {
        assert_eq!(strip_v_prefix("v1.7.2"), "1.7.2");
        assert_eq!(strip_v_prefix("1.7.2"), "1.7.2");
    }

    #[test]
    fn compare_newer_equal_older() {
        assert!(is_newer("v1.8.0", "1.7.2").unwrap());
        assert!(!is_newer("1.7.2", "1.7.2").unwrap());
        assert!(!is_newer("1.7.1", "1.7.2").unwrap());
    }

    #[test]
    fn reject_bad_tag() {
        assert!(is_newer("nope", "1.0.0").is_err());
    }
}
