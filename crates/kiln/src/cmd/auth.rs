//! `kiln auth confluence`: asks for a token without echo, checks it
//! against the server (`whoami`) and stores it in the OS credential store
//! under the suite's names, where Flint finds it too; `--forget` removes it.
//!
//! It deliberately doesn't ask for the Cloud account e-mail: that is
//! `[confluence] user` (or `KILN_CONFLUENCE_USER`), configuration rather
//! than a secret, and storing it beside the token would make two places
//! to change it.

use std::io::IsTerminal;
use std::path::Path;

use kiln_model::ConfluenceFlavour;

use crate::project::load;
use crate::{Fail, EXIT_NETWORK};

pub fn confluence(project: Option<&Path>, forget: bool) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let config = loaded
        .project
        .config
        .confluence
        .as_ref()
        .ok_or_else(|| Fail::usage("kiln.toml has no [confluence] url to store a token for"))?;
    let account = kiln_secrets::account("confluence", &config.url);
    if forget {
        kiln_secrets::forget(&account).map_err(Fail::usage)?;
        println!(
            "removed the token for {} from the credential store",
            config.url
        );
        return Ok(0);
    }
    let user = kiln_secrets::confluence_user().unwrap_or_else(|| config.user.clone());
    if config.resolved_flavour() == ConfluenceFlavour::Cloud && user.trim().is_empty() {
        return Err(Fail::usage(
            "Confluence Cloud signs in with your account e-mail and an API token: set [confluence] user (or KILN_CONFLUENCE_USER) first",
        ));
    }
    if !std::io::stdin().is_terminal() {
        return Err(Fail::usage(
            "kiln auth asks for the token on a terminal; in CI, set KILN_CONFLUENCE_TOKEN instead",
        ));
    }
    let token =
        kiln_secrets::prompt(&format!("Token for {}: ", config.url)).map_err(Fail::usage)?;
    let client = kiln_confluence::connect(config, &token, &user);
    let name = client
        .whoami()
        .map_err(|e| Fail::new(EXIT_NETWORK, format!("the token wasn't stored: {e}")))?;
    kiln_secrets::store(&account, &token).map_err(Fail::usage)?;
    println!(
        "stored the token for {} (signed in as {name}) in the credential store as {}/{account}",
        config.url,
        kiln_secrets::SERVICE
    );
    Ok(0)
}
