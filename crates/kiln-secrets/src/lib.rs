//! Tokens for Confluence and Jira: from environment variables
//! (`KILN_CONFLUENCE_TOKEN`, `KILN_JIRA_TOKEN`) for CI, otherwise from the
//! OS credential store through `keyring`, under the suite's names (service
//! `fenix-suite`, account `<kind>:<url>`), so a token stored once serves
//! Flint and Kiln. Prompting for a token without echo for `kiln auth`.
//!
//! A token here is a value that is never formatted: not into a log, a
//! plan, an error message, a file or a command line. The type that holds
//! it exists to make that hard to get wrong.
//!
//! It deliberately doesn't talk to any server; checking a token
//! (`whoami`) belongs to the client that uses it.
//!
//! Arrives in K3; a shell until then.
