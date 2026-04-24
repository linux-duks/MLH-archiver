//! Progress tracking for a single mailing list.
//!
//! Tracks the last processed email ID via a YAML file:
//! `{base_output_path}/{list_name}/__progress.yaml`

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Progress state for a mailing list.
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct ReadStatus {
    pub(crate) last_email: String,
}

/// Tracks the last processed email ID for a mailing list.
///
/// # Example
///
/// ```
/// use std::path::Path;
/// use mlh_archiver::archive_writer::ProgressTracker;
///
/// let tracker = ProgressTracker::new(Path::new("./output"), "test.list");
/// let last = tracker.last_processed_id();
/// ```
pub struct ProgressTracker {
    output_path: PathBuf,
}

impl ProgressTracker {
    /// Creates a new progress tracker for the given list.
    ///
    /// # Arguments
    ///
    /// * `base_path` - Root output directory (e.g., `./output`)
    /// * `list_name` - Mailing list name (becomes subdirectory)
    pub fn new(base_path: &Path, list_name: &str) -> Self {
        Self {
            output_path: base_path.join(list_name).join("__progress.yaml"),
        }
    }

    /// Returns the last processed email ID.
    ///
    /// Reads from the YAML file if it exists, otherwise returns `0`.
    /// Also falls back to reading a plain number from the file.
    /// If no file exists, initializes one with `0` to mark the list
    /// as discovered.
    pub fn last_processed_id(&self) -> Option<String> {
        match crate::file_utils::read_yaml::<ReadStatus>(self.output_path.to_str().unwrap()) {
            Ok(status) => Some(status.last_email),
            Err(e) => {
                log::warn!(
                    "Unable to find last_processed_id for {} : {}",
                    self.output_path.to_str().expect("output_path should exist"),
                    e
                );
                None
            }
        }
    }

    /// Persists the last successfully processed email ID.
    ///
    /// # Arguments
    ///
    /// * `id` - email ID that was just processed
    pub fn update(&self, id: &str) -> crate::Result<()> {
        crate::file_utils::write_yaml(
            self.output_path.to_str().unwrap(),
            &ReadStatus {
                last_email: id.to_string(),
            },
        )
        .map_err(crate::errors::Error::Io)
    }
}
