//! Bash grammar: enough to tell a finished command from an unfinished one, and
//! to quote a completion the way bash would.
//!
//! Copied from flyline (MIT), which built it on the `flash` lexer. See the
//! headers in the individual files.

// Kept as close to upstream as possible so it can be re-synced: the style lints
// are theirs to fix, and `dead_code` covers the helpers we do not use.
#[allow(dead_code, clippy::collapsible_if)]
pub mod command_acceptance;
#[allow(dead_code, clippy::collapsible_if)]
pub mod dparser;
#[allow(dead_code, clippy::collapsible_if)]
pub mod quoting;
