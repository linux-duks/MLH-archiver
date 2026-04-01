use crate::nntp_source::{self, nntp_config::NntpConfig};

pub(crate) fn retrieve_lists(nntp_config: NntpConfig) -> crate::errors::Result<Vec<String>> {
    // Get NNTP config (validates hostname is present)
    nntp_config.validate()?;

    // Connect to NNTP server to get list of groups
    let mut nntp_stream = nntp_source::connect_to_nntp(nntp_config.server_address())?;

    let list_options = nntp_stream.list()?;

    let groups = list_options.iter().map(|an| an.clone().name).collect();

    // close initial connection to nntp server
    let _ = nntp_stream.quit();

    Ok(groups)
}
