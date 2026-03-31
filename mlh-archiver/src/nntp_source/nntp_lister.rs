use crate::config;
use crate::nntp_source;

pub(crate) fn retrieve_lists(
    app_config: &mut config::AppConfig,
) -> crate::errors::Result<Vec<String>> {
    // Get NNTP config (validates hostname is present)
    let nntp_config = app_config.get_nntp_config();

    nntp_config.validate()?;

    // Connect to NNTP server to get list of groups
    let mut nntp_stream = nntp_source::connect_to_nntp(nntp_config.server_address())?;

    let list_options = nntp_stream.list()?;

    let groups = list_options.iter().map(|an| an.clone().name).collect();

    // close initial connection to nntp server
    let _ = nntp_stream.quit();

    Ok(groups)
}
