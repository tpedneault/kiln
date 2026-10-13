//! Tokens for Confluence and Jira. K2: from environment variables
//! (`KILN_CONFLUENCE_TOKEN`, with `KILN_CONFLUENCE_USER` for Cloud), which
//! is what CI uses and what a read-only `--dry-run` needs. K3 adds the OS
//! credential store through `keyring`, under the suite's names (service
//! `fenix-suite`, account `<kind>:<url>`), and prompting without echo for
//! `kiln auth`.
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
    fn blank_variables_are_no_token() {
        assert_eq!(confluence_from(|_| Some("  ".into())), None);
        assert_eq!(
            confluence_from(|_| Some(" t ".into())).unwrap().expose(),
            "t"
        );
        assert_eq!(confluence_from(|_| None), None);
    }
}
