//! Tokens for Confluence and Jira: from environment variables
//! (`KILN_CONFLUENCE_TOKEN`, with `KILN_CONFLUENCE_USER` for Cloud), which
//! is what CI uses; otherwise from the OS credential store through
//! `keyring` (Windows Credential Manager; the Secret Service on Linux),
//! under the suite's names: service `fenix-suite`, account
//! `<kind>:<url>` with the URL as configured, trailing slash removed. Flint
//! files its tokens the same way (its F12), so a token stored once serves
//! both. Prompting without echo is for `kiln auth`.
//!
//! A token here is a value that is never formatted: not into a log, a
//! plan, an error message, a file or a command line. [`Token`]'s `Debug`
//! prints `Token(…)` and it has no `Display`; the only way to the text is
//! [`Token::expose`], at the one place a request header is built.
//!
//! It deliberately doesn't talk to any server; checking a token
//! (`whoami`) belongs to the client that uses it.

use std::fmt;

/// The environment variable holding a Confluence token.
pub const CONFLUENCE_TOKEN: &str = "KILN_CONFLUENCE_TOKEN";
/// The environment variable overriding `[confluence] user`.
pub const CONFLUENCE_USER: &str = "KILN_CONFLUENCE_USER";

/// The service every suite tool files its secrets under.
pub const SERVICE: &str = "fenix-suite";

/// The credential store account for `kind` at `url`: `confluence:<url>`.
pub fn account(kind: &str, url: &str) -> String {
    format!("{kind}:{}", url.trim().trim_end_matches('/'))
}

/// Where a token came from, for messages (never the token itself).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Environment,
    CredentialStore,
}

/// A secret. Never formatted.
#[derive(Clone, PartialEq, Eq)]
pub struct Token(String);

impl Token {
    pub fn new(secret: impl Into<String>) -> Self {
        Token(secret.into())
    }

    /// The secret itself, for building an `Authorization` header and
    /// nothing else.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token(…)")
    }
}

/// Reads the environment through `get`, so tests don't touch the
/// process's.
pub fn confluence_from(get: impl Fn(&str) -> Option<String>) -> Option<Token> {
    get(CONFLUENCE_TOKEN)
        .filter(|t| !t.trim().is_empty())
        .map(|t| Token(t.trim().to_owned()))
}

/// `KILN_CONFLUENCE_TOKEN` from the process environment.
pub fn confluence_token() -> Option<Token> {
    confluence_from(|k| std::env::var(k).ok())
}

/// The Confluence token for `url`: the environment first, then the
/// credential store. `Ok(None)` when neither has one; an error when the
/// store can't be read (on a headless Linux machine without a Secret
/// Service, for one).
pub fn confluence_token_for(url: &str) -> Result<Option<(Token, Origin)>, String> {
    if let Some(token) = confluence_token() {
        return Ok(Some((token, Origin::Environment)));
    }
    Ok(get(&account("confluence", url))
        .map_err(|e| format!("{e} (or set {CONFLUENCE_TOKEN})"))?
        .map(|t| (Token(t), Origin::CredentialStore)))
}

fn entry(account: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, account)
        .map_err(|e| format!("the credential store refused `{account}`: {e}"))
}

/// The secret stored as `account`; `None` when there is none.
pub fn get(account: &str) -> Result<Option<String>, String> {
    match entry(account)?.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!(
            "couldn't read `{account}` from the credential store: {e}"
        )),
    }
}

/// Stores `token` as `account`, replacing what was there.
pub fn store(account: &str, token: &Token) -> Result<(), String> {
    entry(account)?
        .set_password(token.expose())
        .map_err(|e| format!("couldn't store `{account}` in the credential store: {e}"))
}

/// Removes `account`'s secret; already gone is fine.
pub fn forget(account: &str) -> Result<(), String> {
    match entry(account)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!(
            "couldn't remove `{account}` from the credential store: {e}"
        )),
    }
}

/// Asks for a secret on the terminal without echoing it. Blank is an
/// error rather than an empty token.
pub fn prompt(text: &str) -> Result<Token, String> {
    let secret = rpassword::prompt_password(text)
        .map_err(|e| format!("couldn't read from the terminal: {e}"))?;
    let secret = secret.trim();
    if secret.is_empty() {
        return Err("no token entered".into());
    }
    Ok(Token(secret.to_owned()))
}

// The store itself isn't unit-tested: a test would write to the
// credential store of whoever runs it. The names are; the round trip is
// checked on a desktop (the pull request's "Needs checking").

/// `KILN_CONFLUENCE_USER` from the process environment, when set.
pub fn confluence_user() -> Option<String> {
    std::env::var(CONFLUENCE_USER)
        .ok()
        .filter(|u| !u.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_never_print() {
        let token = Token::new("s3cret");
        assert_eq!(format!("{token:?}"), "Token(…)");
        assert_eq!(format!("{:?}", Some(token.clone())), "Some(Token(…))");
        assert_eq!(token.expose(), "s3cret");
    }

    #[test]
    fn accounts_use_the_suite_names() {
        assert_eq!(
            account("confluence", "https://c.example.com/conf/"),
            "confluence:https://c.example.com/conf"
        );
        assert_eq!(SERVICE, "fenix-suite");
    }

    #[test]
    fn blank_variables_are_no_token() {
        assert_eq!(confluence_from(|_| Some("  ".into())), None);
        assert_eq!(
            confluence_from(|_| Some(" t ".into())).unwrap().expose(),
            "t"
        );
        assert_eq!(confluence_from(|_| None), None);
    }
}
