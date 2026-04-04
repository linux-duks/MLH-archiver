//! Utility functions for NNTP operations.
//!
//! This module provides reusable NNTP utility functions that can be used
//! by both the archiver and external tools like check_nntp.
//!
//! # Functions
//!
//! - [`connect_to_nntp_server`] - Establish connection to NNTP server
//! - [`get_group_info`] - Retrieve group information (article range)
//! - [`retrieve_lists_with_connection`] - Get all available groups

use crate::errors;
use nntp::NNTPStream;

/// Establishes a connection to an NNTP server and checks capabilities.
///
/// This is a reusable helper function that connects to an NNTP server,
/// logs server capabilities, and returns the connected stream.
///
/// # Arguments
///
/// * `hostname` - Server hostname (e.g., "nntp.example.com")
/// * `port` - Server port (e.g., 119)
///
/// # Returns
///
/// * `Ok(NNTPStream)` - Connected stream ready for commands
/// * `Err(...)` - Connection failure
///
/// # Example
///
/// ```rust,no_run
/// use mlh_archiver::nntp_source::nntp_utils::connect_to_nntp_server;
///
/// let mut stream = connect_to_nntp_server("nntp://nntp.example.com", None, None, None)?;
/// // Use stream for NNTP commands
/// stream.quit()?;
/// # Ok::<(), mlh_archiver::errors::Error>(())
/// ```
pub fn connect_to_nntp_server(
    hostname: &str,
    port: Option<u16>,
    username: Option<String>,
    password: Option<String>,
) -> errors::Result<NNTPStream> {
    // if port is None, let the library assume 119 (PLAINTEXT) or 563 (TLS)
    let address = server_address(hostname, port);

    let mut nntp_stream = NNTPStream::connect(address)?;

    let _ = nntp_stream.set_mode_reader();

    // Log server capabilities
    match nntp_stream.capabilities() {
        Ok(lines) => {
            log::debug!(
                "NNTP server capabilities: {}",
                lines.join(", ").replace('\n', " ")
            );
        }
        Err(e) => log::warn!("Failed checking server capabilities: {}", e),
    }

    if let (Some(username), Some(password)) = (username.clone(), password.clone())
        && let Err(e) = nntp_stream.user_password_authenticate(&username, &password)
    {
        log::warn!("NNTP authentication failed: {}", e);
    }

    Ok(nntp_stream)
}

/// format the server with of without the port
///
/// if port is None, let the library assume 119 (PLAINTEXT) or 563 (TLS)
pub fn server_address(hostname: &str, port: Option<u16>) -> String {
    // TODO: validate if the hostname already has a port in-text ?
    match port {
        Some(port_value) => format!("{}:{}", hostname, port_value),
        None => hostname.to_string(),
    }
}

/// Retrieves information about a newsgroup including article range.
///
/// This function queries the NNTP server for group statistics including
/// the low and high article numbers, which can be used to determine
/// the range of available articles.
///
/// # Arguments
///
/// * `nntp_stream` - Connected NNTP stream
/// * `group_name` - Name of the newsgroup to query
///
/// # Returns
///
/// * `Ok(NewsGroup)` - Group information with article range
/// * `Err(...)` - NNTP protocol error
///
/// # Example
///
/// ```rust,no_run
/// use mlh_archiver::nntp_source::nntp_utils::{connect_to_nntp_server, get_group_info};
///
/// let mut stream = connect_to_nntp_server("nntps://nntp.example.com", None, None, None)?;
/// let group = get_group_info(&mut stream, "dev.example.lists")?;
/// println!("Articles: {} to {}", group.low, group.high);
/// # Ok::<(), mlh_archiver::errors::Error>(())
/// ```
pub fn get_group_info(
    nntp_stream: &mut NNTPStream,
    group_name: &str,
) -> errors::Result<nntp::NewsGroup> {
    let group = nntp_stream.group(group_name)?;
    Ok(group)
}

/// Retrieves all available newsgroups from the NNTP server.
///
/// This is a convenience function that combines connection and list retrieval
/// into a single call. It connects to the server, fetches the list of groups,
/// and cleanly disconnects.
///
/// # Arguments
///
/// * `hostname` - Server hostname
/// * `port` - Server port
///
/// # Returns
///
/// * `Ok(Vec<String>)` - List of group names
/// * `Err(...)` - Connection or protocol error
///
/// # Example
///
/// ```rust,no_run
/// use mlh_archiver::nntp_source::nntp_utils::retrieve_lists_with_connection;
///
/// let groups = retrieve_lists_with_connection("nntp://nntp.example.com", Some(119), None, None)?;
/// println!("Available groups: {}", groups.len());
/// # Ok::<(), mlh_archiver::errors::Error>(())
/// ```
pub fn retrieve_lists_with_connection(
    hostname: &str,
    port: Option<u16>,
    username: Option<String>,
    password: Option<String>,
) -> errors::Result<Vec<String>> {
    let mut nntp_stream = connect_to_nntp_server(hostname, port, username, password)?;

    let list_options = nntp_stream.list()?;
    let groups = list_options.iter().map(|g| g.name.clone()).collect();

    // Clean shutdown
    let _ = nntp_stream.quit();

    Ok(groups)
}

/// Retrieves group information for multiple groups in a single call.
///
/// This function connects to the server, queries info for each group,
/// and returns the results. Useful for previewing article ranges.
///
/// # Arguments
///
/// * `hostname` - Server hostname
/// * `port` - Server port
/// * `groups` - List of group names to query
///
/// # Returns
///
/// * `Ok(Vec<(String, nntp::NewsGroup)>)` - Pair of group name and info
/// * `Err(...)` - Connection or protocol error
pub fn retrieve_groups_info(
    hostname: &str,
    port: Option<u16>,
    groups: &[String],
    username: Option<String>,
    password: Option<String>,
) -> errors::Result<Vec<(String, nntp::NewsGroup)>> {
    let mut nntp_stream = connect_to_nntp_server(hostname, port, username, password)?;

    let mut results = Vec::with_capacity(groups.len());
    for group_name in groups {
        match get_group_info(&mut nntp_stream, group_name) {
            Ok(group_info) => {
                results.push((group_name.clone(), group_info));
            }
            Err(e) => {
                log::warn!("Failed to get info for group '{}': {}", group_name, e);
            }
        }
    }

    let _ = nntp_stream.quit();
    Ok(results)
}
