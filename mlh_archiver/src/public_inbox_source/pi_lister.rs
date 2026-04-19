use crate::public_inbox_source::{pi_config::PIConfig, pi_utils::find_public_inboxes};
use std::path::PathBuf;

/// Retrieve the list of available public inbox lists from the configured import directory.
///
/// This function validates the provided configuration, then scans the import directory for
/// public inbox repositories. It filters out any repositories marked as incomplete (based on
/// version string) and returns the names of the valid inboxes.
///
/// # Arguments
///
/// * `pi_config` - The configuration containing the import directory and other settings.
///
/// # Returns
///
/// * `Ok(Vec<String>)` - A list of names of the valid public inboxes found.
/// * `Err` - If the configuration is invalid or an I/O error occurs while scanning the directory.
///
/// # Example
///
/// ```
/// let config = PIConfig {
///     inport_directory: "/path/to/inboxes".to_string(),
///     origin: "example".to_string(),
///     ..Default::default()
/// };
/// let lists = retrieve_lists(config).expect("Failed to retrieve lists");
/// ```
pub fn retrieve_lists(pi_config: PIConfig) -> crate::Result<Vec<String>> {
    // Validate the configuration before proceeding.
    pi_config.validate()?;

    log::debug!(
        "Retrieving the list of PublicInboxes in {}",
        pi_config.inport_directory
    );
    let path = PathBuf::from(pi_config.clone().inport_directory);

    // Attempt to find public inboxes in the given path.
    match find_public_inboxes(&path) {
        Ok(list) => {
            // Filter out incomplete repositories (those with "incomplete" in the version string).
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

            // Log a warning if no valid inboxes are found.
            if valid_list.is_empty() {
                log::warn!(
                    "No valid public-inboxes found in {}",
                    pi_config.clone().inport_directory
                );
            }

            // Extract and return the names of the valid inboxes.
            Ok(valid_list
                .iter()
                .map(|l| l.name.clone())
                .collect::<Vec<String>>())
        }
        Err(e) => Err(e), // Propagate any errors from `find_public_inboxes`.
    }
}
