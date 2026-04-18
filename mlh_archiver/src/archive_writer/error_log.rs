//! Error logging for unavailable emails.
//!
//! Appends errors to `{base_output_path}/{list_name}/__errors.csv` in CSV format:
//! `{email_id},{error_message}`

use std::path::{Path, PathBuf};

/// Appends error entries for a mailing list to an error log file.
///
/// # Example
///
/// ```
/// use std::path::Path;
/// use mlh_archiver::archive_writer::ErrorLogger;
///
/// let logger = ErrorLogger::new(Path::new("./output"), "test.list");
/// logger.log("42", "email not available");
/// ```
pub struct ErrorLogger {
    output_path: PathBuf,
}

impl ErrorLogger {
    /// Creates a new error logger for the given list.
    ///
    /// # Arguments
    ///
    /// * `base_path` - Root output directory (e.g., `./output`)
    /// * `list_name` - Mailing list name (becomes subdirectory)
    pub fn new(base_path: &Path, list_name: &str) -> Self {
        Self {
            output_path: base_path.join(list_name).join("__errors.csv"),
        }
    }

    /// Logs an error for an unavailable email.
    ///
    /// Appends `{email_id},{error}` as a new line.
    /// Silently logs a warning if the write fails (non-fatal).
    ///
    /// # Arguments
    ///
    /// * `email_id` - email number that failed
    /// * `error` - Error message
    pub fn log(&self, email_id: &str, error: &str) {
        let line = format!("{email_id},{error}");
        if let Err(e) = crate::file_utils::append_line_to_file(&self.output_path, &line) {
            log::warn!("Failed to append error log for email {email_id}: {e}");
        }
    }
}
