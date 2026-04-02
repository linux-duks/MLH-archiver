//! NNTP (Network News Transfer Protocol) source implementation.
//!
//! This module provides the NNTP-specific worker implementation for fetching
//! emails from NNTP servers. It includes:
//!
//! - [`nntp_config`] - Configuration struct for NNTP connections
//! - [`nntp_lister`] - Functions to retrieve available mailing lists
//! - [`nntp_worker`] - Worker implementation that fetches emails
//!
//! # Architecture
//!
//! The NNTP worker:
//! 1. Connects to the NNTP server on creation
//! 2. Uses `RefCell` for interior mutability of the connection
//! 3. Checks shutdown flag during long operations
//! 4. Tracks progress via `__last_article_number` files
//! 5. Handles reconnection on network errors

pub mod nntp_config;
pub mod nntp_lister;
pub mod nntp_worker;

use log::{Level, log_enabled};
use nntp::NNTPStream;

/// Establishes a connection to an NNTP server and checks capabilities.
///
/// This is a helper function used by both the lister and worker to create
/// NNTP connections. It logs server capabilities at debug level.
///
/// # Arguments
///
/// * `address` - Server address in format `"hostname:port"`
///
/// # Returns
///
/// * `Ok(NNTPStream)` - Connected stream ready for commands
/// * `Err(...)` - Connection failure
///
/// # Side Effects
///
/// Logs server capabilities if debug logging is enabled.
pub(super) fn connect_to_nntp(address: String) -> nntp::Result<NNTPStream> {
    let mut nntp_stream = match NNTPStream::connect(address) {
        Ok(stream) => stream,
        Err(e) => {
            return Err(e);
        }
    };

    match nntp_stream.capabilities() {
        Ok(lines) => {
            if log_enabled!(Level::Debug) {
                log::debug!(
                    "server capabilities : {}",
                    lines.join(", ").replace("\n", " ")
                );
            }
        }
        Err(e) => log::error!("Failed checking server capabilities: {}", e),
    }
    return Ok(nntp_stream);
}
