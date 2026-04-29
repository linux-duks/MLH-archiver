use crate::errors::ConfigError;

/// Configuration for the public inbox source.
///
/// This struct holds the configuration needed to connect to and process a public inbox.
/// It includes the import directory, origin, and optional settings for grokmirror,
/// public inbox config, and email range.
#[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq, Clone, Default)]
pub struct PIConfig {
    /// (optional) if specified, will use grokmirror to identify the lists available
    pub import_directory: String,
    /// The origin of the public inbox (e.g., the base URL or identifier).
    /// TODO: can we check in the public-inbox metadata ?
    pub origin: String,
    /// Optional path to a public inbox config file for listing available inboxes.
    /// TODO: use public inbox config file if exists to list the available
    /// inboxes from config instead of listing the directories
    /// Also take the list origin and id from there
    pub public_inbox_config: Option<String>,
    /// (optional). Read a specific range of articles from the first list provided.
    /// Comma separated values, or dash separated ranges, like low-high
    /// Article numbers are 1-indexed.
    pub email_range: Option<String>,
}

impl PIConfig {
    /// Validate the configuration.
    ///
    /// Checks that the required fields are present and valid.
    /// Currently validates:
    /// - `import_directory` is not empty and exists as a directory
    /// - `origin` is not empty
    ///
    /// # Returns
    /// - `Ok(())` if the configuration is valid
    /// - `Err(ConfigError)` if the configuration is invalid
    ///
    /// # Errors
    /// - `ConfigError::MissingHostname` if `import_directory` or `origin` is empty
    /// - `ConfigError::Io` if the import directory does not exist or is not a directory
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.import_directory.is_empty() {
            return Err(ConfigError::MissingImportDirectory);
        }

        // Check if import directory exists and is a directory
        let path = std::path::Path::new(&self.import_directory);
        if !path.exists() {
            return Err(ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Import directory does not exist: {}", self.import_directory),
            )));
        }
        if !path.is_dir() {
            return Err(ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Import directory is not a directory: {}",
                    self.import_directory
                ),
            )));
        }

        if self.origin.is_empty() {
            return Err(ConfigError::MissingOrigin);
        }

        // TODO: validate email_range format using parse_sequence
        // For now, just ignore

        Ok(())
    }
}
