use crate::errors::ConfigError;

#[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq, Clone, Default)]
pub struct PIConfig {
    /// (optional) if specified, will use grokmirror to identify the lists available
    pub inport_directory: String,
    // TODO: can we check in the public-inbox metadata ?
    pub origin: String,
    pub grokmirror_manifest: Option<String>,
    // TODO: use public inbox config file if exists to list the available
    // inboxes from config instead of listing the directories
    // Also take the list origin and id from there
    pub public_inbox_config: Option<String>,
    pub group_lists: Option<Vec<String>>,
    /// (optional). Read a specific range of articles from the first list provided.
    /// Comma separated values, or dash separated ranges, like low-high
    pub article_range: Option<String>,
}

impl PIConfig {
    /// Validate configuration.
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
