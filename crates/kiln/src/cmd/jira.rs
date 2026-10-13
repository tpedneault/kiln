//! The Jira mirror as the commands use it: the cached mirror when it is
//! fresh, else one fetch with the token from `KILN_JIRA_TOKEN` or the
//! credential store, cached in the state folder for the next command
//! (INTEGRATION.md, "Jira"). `kiln auth jira` is here too.

use std::io::IsTerminal;
use std::path::Path;

use kiln_index::Project;
use kiln_jira::Mirror;

use crate::project::{folders, load};
use crate::{Fail, EXIT_NETWORK};

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The mirror for every key the project links. An error is one line
/// naming what's missing (no `[jira] url`, no token) or what Jira said.
pub fn mirror(project: &Project) -> Result<Mirror, String> {
    let url = project
        .config
        .jira_url
        .as_deref()
        .ok_or("kiln.toml has no [jira] url to read tickets from")?;
    mirror_for(url, &kiln_jira::keys(project))
}

/// The mirror of `keys` from the Jira at `url`.
pub fn mirror_for(url: &str, keys: &[String]) -> Result<Mirror, String> {
    let url = url.trim_end_matches('/');
    let keys = keys.to_vec();
    let now = now();
    if keys.is_empty() {
        return Ok(Mirror {
            url: url.to_owned(),
            fetched: now,
            ..Mirror::default()
        });
    }
    let cache = folders().map(|f| f.state.join(kiln_jira::cache_name(url)));
    if let Some(cached) = cache
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| Mirror::parse(&t))
    {
        if cached.covers(url, &keys, now) {
            return Ok(cached);
        }
    }
    if kiln_jira::is_cloud(url) {
        return Err(kiln_jira::cloud_refusal(url));
    }
    let (token, _) = kiln_secrets::jira_token_for(url)?.ok_or_else(|| {
        format!(
            "no Jira token for {url}: set {} or run `kiln auth jira`",
            kiln_secrets::JIRA_TOKEN
        )
    })?;
    let mirror = kiln_jira::fetch(url, token.expose(), &keys, now)?;
    // A cache that can't be written costs a fetch next time, nothing more.
    if let Some(path) = cache {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = fenix_storage::write(&path, mirror.to_text().as_bytes());
    }
    Ok(mirror)
}

/// `kiln auth jira`: asks for a personal access token without echo,
/// checks it with one search and stores it as `fenix-suite`/`jira:<url>`,
/// where Flint finds it too; `--forget` removes it.
pub fn auth(project: Option<&Path>, forget: bool) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let url = loaded
        .project
        .config
        .jira_url
        .clone()
        .ok_or_else(|| Fail::usage("kiln.toml has no [jira] url to store a token for"))?;
    let url = url.trim_end_matches('/').to_owned();
    let account = kiln_secrets::account("jira", &url);
    if forget {
        kiln_secrets::forget(&account).map_err(Fail::usage)?;
        println!("removed the token for {url} from the credential store");
        return Ok(0);
    }
    if kiln_jira::is_cloud(&url) {
        return Err(Fail::usage(kiln_jira::cloud_refusal(&url)));
    }
    if !std::io::stdin().is_terminal() {
        return Err(Fail::usage(format!(
            "kiln auth asks for the token on a terminal; in CI, set {} instead",
            kiln_secrets::JIRA_TOKEN
        )));
    }
    let token =
        kiln_secrets::prompt(&format!("Personal access token for {url}: ")).map_err(Fail::usage)?;
    kiln_jira::sign_in(&url, token.expose())
        .map_err(|e| Fail::new(EXIT_NETWORK, format!("the token wasn't stored: {e}")))?;
    kiln_secrets::store(&account, &token).map_err(Fail::usage)?;
    println!(
        "stored the token for {url} in the credential store as {}/{account}",
        kiln_secrets::SERVICE
    );
    Ok(0)
}
