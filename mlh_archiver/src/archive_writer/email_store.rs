//! Email storage for a mailing list.
//!
//! Writes fetched emails as `.eml` files:
//! `{base_output_path}/{list_name}/{email_id}.eml`

use std::path::{Path, PathBuf};

/// Writes fetched email emails for a mailing list.
///
/// # Example
///
/// ```
/// use std::path::Path;
/// use mlh_archiver::archive_writer::EmailStore;
///
/// let store = EmailStore::new(Path::new("./output"), "test.list");
/// store.write(42, &["From: user@example.com".to_string()]).unwrap();
/// ```
pub struct EmailStore {
    output_path: PathBuf,
}

impl EmailStore {
    /// Creates a new email store for the given list.
    ///
    /// # Arguments
    ///
    /// * `base_path` - Root output directory (e.g., `./output`)
    /// * `list_name` - Mailing list name (becomes subdirectory)
    pub fn new(base_path: &Path, list_name: &str) -> Self {
        Self {
            output_path: base_path.join(list_name),
        }
    }

    /// Writes a fetched email to an `.eml` file.
    ///
    /// # Arguments
    ///
    /// * `email_id` - email number
    /// * `lines` - Raw email lines (written without added newlines)
    pub fn write(&self, email_id: usize, lines: &[String]) -> crate::Result<()> {
        let file_path = self.output_path.join(format!("{email_id}.eml"));
        crate::file_utils::write_lines_file(&file_path, lines).map_err(crate::errors::Error::Io)
    }
}
