//! Error types for the MLH Archiver.
//!
//! This module defines the error hierarchy used throughout the application.
//! All errors implement `std::error::Error` and can be converted using `?`.

use std::io;
use std::result;
use thiserror::Error;

/// Result type alias using the application's error type.
///
/// # Example
///
/// ```rust,no_run
/// use mlh_archiver::Result;
///
/// fn my_function() -> Result<()> {
///     // ...
///     Ok(())
/// }
/// ```
pub type Result<T> = result::Result<T, Error>;

/// Application-level error type.
///
/// This enum wraps all possible errors that can occur during archiving:
/// - I/O errors (file operations)
/// - NNTP errors (network/protocol)
/// - Configuration errors
///
/// # Example
///
/// ```rust,no_run
/// use mlh_archiver::errors::{Error, Result};
///
/// fn example() -> Result<()> {
///     std::fs::read_to_string("file.txt")?;  // Auto-converts io::Error
///     Ok(())
/// }
/// ```
#[derive(Error, Debug)]
pub enum Error {
    #[error("unknown error")]
    Unknown,
    #[error(transparent)]
    Io(#[from] io::Error),

    #[allow(clippy::upper_case_acronyms)]
    #[error(transparent)]
    NNTP(#[from] nntp::NNTPError),

    #[error("git error: {0}")]
    Git(String),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),

    #[error(transparent)]
    Config(#[from] ConfigError),
}

impl From<gix::Error> for Error {
    fn from(err: gix::Error) -> Self {
        Error::Git(err.to_string())
    }
}

impl From<gix::open::Error> for Error {
    fn from(err: gix::open::Error) -> Self {
        Error::Git(err.to_string())
    }
}

impl From<gix::refs::file::find::existing::Error> for Error {
    fn from(err: gix::refs::file::find::existing::Error) -> Self {
        Error::Git(err.to_string())
    }
}

impl From<gix::objs::decode::Error> for Error {
    fn from(err: gix::objs::decode::Error) -> Self {
        Error::Git(err.to_string())
    }
}

impl From<gix::object::find::existing::with_conversion::Error> for Error {
    fn from(err: gix::object::find::existing::with_conversion::Error) -> Self {
        Error::Git(err.to_string())
    }
}

impl From<gix::object::find::existing::Error> for Error {
    fn from(err: gix::object::find::existing::Error) -> Self {
        Error::Git(err.to_string())
    }
}

impl From<gix::revision::walk::Error> for Error {
    fn from(err: gix::revision::walk::Error) -> Self {
        Error::Git(err.to_string())
    }
}

impl From<gix::date::Error> for Error {
    fn from(err: gix::date::Error) -> Self {
        Error::Git(err.to_string())
    }
}

/// Converts any gix error to our Error type.
#[macro_export]
macro_rules! git_error {
    ($expr:expr) => {
        $expr.map_err(|e: gix::Error| mlh_archiver::Error::Git(e.to_string()))
    };
}

/// Configuration-related errors.
///
/// These errors occur during configuration loading, validation, or
/// list selection.
///
/// # Variants
///
/// * `MissingHostname` - NNTP hostname not configured
/// * `ListSelectionEmpty` - User selected no mailing lists
/// * `RunModeInvalid` - Invalid run mode configuration
/// * `ConfiguredListsNotAvailable` - Configured lists don't exist on server
/// * `AllListsUnavailable` - None of the configured lists are available
/// * `Io(...)` - I/O error during config file operations
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error(
        "missing hostname: provide NNTP server hostname via --hostname/-H, NNTP_HOSTNAME env var, or config file"
    )]
    MissingHostname,
    #[error("invalid list selection. At least one should be configured, or selected in runtime")]
    ListSelectionEmpty,
    #[error("invalid run mode.At least one RunMode should be configured")]
    RunModeInvalid,

    #[error("configured list(s) not available in server. {} Lists with error: {}", unavailable_lists.len(), unavailable_lists.iter().map(|x| x.to_string() + ",").collect::<String>())]
    ConfiguredListsNotAvailable { unavailable_lists: Vec<String> },
    #[error("none of the configured lists are available in server")]
    AllListsUnavailable,

    #[error(transparent)]
    Io(#[from] io::Error),
}
