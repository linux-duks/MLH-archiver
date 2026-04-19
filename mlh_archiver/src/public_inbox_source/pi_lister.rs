use crate::public_inbox_source::{pi_config::PIConfig, pi_utils::find_public_inboxes};
use std::path::PathBuf;

/// retrieve_lists connects to the nntp endpoint, and returns the name of every list available
pub fn retrieve_lists(pi_config: PIConfig) -> crate::Result<Vec<String>> {
    pi_config.validate()?;

    log::debug!(
        "Retrieving the list of PublicInboxes in {}",
        pi_config.inport_directory
    );
    let path = PathBuf::from(pi_config.clone().inport_directory);

    return match find_public_inboxes(&path) {
        Ok(list) => {
            // Filter out incomplete repositories
            let valid_list: Vec<_> = list
                .clone()
                .into_iter()
                .filter(|inbox| !inbox.version.contains("incomplete"))
                .collect();

            log::debug!(
                "Found {} public-inboxes ({} valid)",
                list.len(),
                valid_list.len()
            );

            if valid_list.is_empty() {
                log::warn!(
                    "No valid public-inboxes found in {}",
                    pi_config.clone().inport_directory
                );
            }

            Ok(valid_list
                .iter()
                .map(|l| l.name.clone())
                .collect::<Vec<String>>())
        }
        Err(e) => Err(e),
    };
}
