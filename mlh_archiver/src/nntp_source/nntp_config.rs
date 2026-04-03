use crate::errors::ConfigError;

/// NNTP-specific configuration
///
/// All NNTP-related settings are nested under this struct.
/// Future source methods (IMAP, local, mbox) will have their own structs.
#[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq, Clone)]
pub struct NntpConfig {
    /// nntp server domain/ip
    pub hostname: String,
    /// nntp server port
    #[serde(default = "default_port")]
    pub port: u16,
    /// List of groups to be read. "ALL" will select all lists available.
    /// Empty value will prompt a selection in the TUI (and save selected values)
    pub group_lists: Option<Vec<String>>,
    /// (optional). Read a specific range of articles from the first list provided.
    /// Comma separated values, or dash separated ranges, like low-high
    pub article_range: Option<String>,
}

impl Default for NntpConfig {
    fn default() -> Self {
        Self {
            hostname: String::new(),
            port: default_port(),
            group_lists: None,
            article_range: None,
        }
    }
}

fn default_port() -> u16 {
    119
}

impl NntpConfig {
    /// Validate that hostname is provided
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.hostname.is_empty() {
            return Err(ConfigError::MissingHostname);
        }
        Ok(())
    }

    /// Get the NNTP server address as a string
    pub fn server_address(&self) -> String {
        format!("{}:{}", self.hostname, self.port)
    }
}
