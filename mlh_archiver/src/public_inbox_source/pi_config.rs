use crate::errors::ConfigError;

#[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq, Clone, Default)]
pub struct PIConfig {
    /// (optional) if specified, will use grokmirror to identify the lists available
    pub inport_directory: String,
    // TODO: can we check in the public-inbox metadata ?
    pub origin: String,
    pub grokmirror_manifest: Option<String>,
    pub group_lists: Option<Vec<String>>,
    /// (optional). Read a specific range of articles from the first list provided.
    /// Comma separated values, or dash separated ranges, like low-high
    pub article_range: Option<String>,
}

impl PIConfig {
    /// Validate that hostname is provided
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.inport_directory.is_empty() {
            // TODO: need new errors
            return Err(ConfigError::MissingHostname);
        }
        Ok(())
    }
}
