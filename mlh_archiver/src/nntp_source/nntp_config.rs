use crate::errors::ConfigError;
use crate::nntp_source::nntp_utils::server_address;

/// NNTP-specific configuration
///
/// All NNTP-related settings are nested under this struct.
/// Future source methods (IMAP, local, mbox) will have their own structs.
///
/// with the key "NNTP", not in this struct.
#[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq, Clone, Default)]
pub struct NntpConfig {
    /// nntp server domain/ip
    /// can be prefixed by [`nntp://`] or [`nntps://`]
    /// to indicate PLAINTEXT or TLS
    pub hostname: String,
    /// nntp server port
    pub port: Option<u16>,
    /// seconds between each read to the server
    /// defaults to 1
    #[serde(default = "default_interval")]
    pub request_interval: u64,
    /// (optional). Read a specific range of articles from the first list provided.
    /// Comma separated values, or dash separated ranges, like low-high
    pub email_range: Option<String>,

    /// (optional). NNTP server username for authentication
    pub username: Option<String>,
    /// (optional). NNTP server password for authentication
    pub password: Option<String>,
}

// Helper function for Serde
fn default_interval() -> u64 {
    1
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
        server_address(&self.hostname, self.port)
    }
}
