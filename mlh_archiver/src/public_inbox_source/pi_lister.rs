use crate::public_inbox_source::{pi_config::PIConfig, pi_utils::find_public_inboxes};
use std::path::PathBuf;

/// retrieve_lists connects to the nntp endpoint, and returns the name of every list available
pub fn retrieve_lists(pi_config: PIConfig) -> crate::errors::Result<Vec<String>> {
    pi_config.validate()?;

    let path = PathBuf::from(pi_config.inport_directory);

    return match find_public_inboxes(&path) {
        Ok(list) => {
            return Ok(list.iter().map(|l| l.name.clone()).collect::<Vec<String>>());
        }
        Err(e) => Err(e),
    };
}
