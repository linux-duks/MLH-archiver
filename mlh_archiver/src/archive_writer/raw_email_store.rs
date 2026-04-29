//! Email storage for a mailing list.
//!
//! Writes fetched emails as `.eml` files:
//! `{base_output_path}/{list_name}/{email_id}.eml`

use std::path::PathBuf;

use crate::archive_writer::{EmailData, EmailStore};

/// Writes fetched email emails for a mailing list.
///
/// # Example
///
/// ```rust
/// use std::path::PathBuf;
/// use mlh_archiver::archive_writer::{RawEmailStore, EmailStore, EmailData};
///
/// let mut store = RawEmailStore::new(PathBuf::from("./output/test.list"));
/// store.add_email(EmailData {
///     email_id: "42".to_string(),
///     content: vec!["From: user@example.com".to_string()],
/// }).unwrap();
/// ```
#[derive(std::fmt::Debug)]
pub struct RawEmailStore {
    output_path: PathBuf,
}

impl RawEmailStore {
    /// Creates a new email store for the given list.
    ///
    /// # Arguments
    ///
    /// * `output_path` - Root output directory (e.g., `./output/list_name`)
    pub fn new(output_path: PathBuf) -> Self {
        Self { output_path }
    }
}

impl EmailStore for RawEmailStore {
    /// Writes a fetched email to an `.eml` file.
    ///
    /// # Arguments
    ///
    /// * `email_id` - email number
    /// * `lines` - Raw email lines (written without added newlines)
    fn add_email(&mut self, email: EmailData) -> crate::Result<Option<Vec<String>>> {
        let file_path = self.output_path.join(format!("{}.eml", email.email_id));
        crate::file_utils::write_lines_file(&file_path, email.content)
            .map_err(crate::errors::Error::Io)?;
        return Ok(Some(vec![email.email_id]));
    }

    fn close(&mut self) -> crate::Result<Option<Vec<String>>> {
        return Ok(None);
    }
}
