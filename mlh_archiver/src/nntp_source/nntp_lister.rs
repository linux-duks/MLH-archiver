use crate::nntp_source::{nntp_config::NntpConfig, nntp_utils::retrieve_lists_with_connection};

/// retrieve_lists connects to the nntp endpoint, and returns the name of every list available

#[cfg_attr(feature = "otel", tracing::instrument)]
pub fn retrieve_lists(nntp_config: NntpConfig) -> crate::errors::Result<Vec<String>> {
    // Get NNTP config (validates hostname is present)
    nntp_config.validate()?;

    // Connect to NNTP server to get list of groups
    let groups = retrieve_lists_with_connection(
        &nntp_config.hostname,
        nntp_config.port,
        nntp_config.username,
        nntp_config.password,
    )?;

    Ok(groups)
}
