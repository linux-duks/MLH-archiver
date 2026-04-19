use crate::errors::ConfigError;

/// Configuration for the public inbox source.
///
/// This struct holds the configuration needed to connect to and process a public inbox.
/// It includes the import directory, origin, and optional settings for grokmirror,
/// public inbox config, group lists, and article range.
#[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq, Clone, Default)]
pub struct PIConfig {
    /// (optional) if specified, will use grokmirror to identify the lists available
    pub inport_directory: String,
    /// The origin of the public inbox (e.g., the base URL or identifier).
    /// TODO: can we check in the public-inbox metadata ?
    pub origin: String,
    /// Optional path to a grokmirror manifest file for discovering available inboxes.
    pub grokmirror_manifest: Option<String>,
    /// Optional path to a public inbox config file for listing available inboxes.
    /// TODO: use public inbox config file if exists to list the available
    /// inboxes from config instead of listing the directories
    /// Also take the list origin and id from there
    pub public_inbox_config: Option<String>,
    /// Optional list of specific inboxes to process (group lists).
    /// If provided, only these inboxes will be processed.
    pub group_lists: Option<Vec<String>>,
    /// (optional). Read a specific range of articles from the first list provided.
    /// Comma separated values, or dash separated ranges, like low-high
    /// Article numbers are 1-indexed.
    pub article_range: Option<String>,
}

impl PIConfig {
    /// Validate the configuration.
    ///
    /// Checks that the required fields are present and valid.
    /// Currently validates:
    /// - `inport_directory` is not empty and exists as a directory
    /// - `origin` is not empty
    /// - `group_lists` is not empty if provided
    ///
    /// # Returns
    /// - `Ok(())` if the configuration is valid
    /// - `Err(ConfigError)` if the configuration is invalid
    ///
    /// # Errors
    /// - `ConfigError::MissingHostname` if `inport_directory` or `origin` is empty
    /// - `ConfigError::Io` if the import directory does not exist or is not a directory
    /// - `ConfigError::ListSelectionEmpty` if `group_lists` is provided but empty
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.inport_directory.is_empty() {
            // TODO: need new error variant for missing import directory
            return Err(ConfigError::MissingHostname);
        }

        // Check if import directory exists and is a directory
        let path = std::path::Path::new(&self.inport_directory);
        if !path.exists() {
            return Err(ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Import directory does not exist: {}", self.inport_directory),
            )));
        }
        if !path.is_dir() {
            return Err(ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Import directory is not a directory: {}",
                    self.inport_directory
                ),
            )));
        }

        if self.origin.is_empty() {
            // TODO: need new error variant for missing origin
            return Err(ConfigError::MissingHostname);
        }

        // If group_lists is provided, ensure it's not empty
        if let Some(lists) = &self.group_lists
            && lists.is_empty()
        {
            return Err(ConfigError::ListSelectionEmpty);
        }

        // TODO: validate article_range format using parse_sequence
        // For now, just ignore

        Ok(())
    }
}
