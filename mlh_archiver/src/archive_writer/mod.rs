//! Archive writer module — reusable facade for storing fetched emails,
//! tracking progress, and logging errors for a single mailing list.
//!
//! # Design
//!
//! `ArchiveWriter` provides a consistent storage interface that **all worker
//! implementations MUST use**. This ensures:
//!
//! 1. **Uniform progress tracking** — `__progress.yaml` YAML files are
//!    created and updated the same way across all sources (NNTP, IMAP, etc.)
//! 2. **Resume support** — workers can resume from the last processed position
//!    regardless of the source type
//! 3. **Consistent file layout** — identical directory structure and file names
//!    for all implementations
//!
//! # Architecture
//!
//! `ArchiveWriter` is a facade over three specialized components:
//!
//! | Component | Purpose |
//! |-----------|---------|
//! | [`ProgressTracker`] | Reads/writes `__progress.yaml` YAML |
//! | [`EmailStore`] | Writes `{id}.eml` files |
//! | [`ErrorLogger`] | Appends `{id},{error}` to `__errors.csv` CSV |
//!
//! # Concurrency
//!
//! Each worker creates its own `ArchiveWriter` instance per list. Since workers
//! write to distinct output paths (one subdirectory per list), **no concurrency
//! control is needed**.
//!
//! # Usage
//!
//! ```rust
//! use std::path::Path;
//! use mlh_archiver::archive_writer::ArchiveWriter;
//!
//! let writer = ArchiveWriter::new(Path::new("./output"), "test.list");
//!
//! // Resume: get last processed email ID
//! let last_id = writer.last_processed_id();
//!
//! // Write a fetched email
//! writer.write_email(42, &["From: user@example.com".to_string()]).unwrap();
//!
//! // Log unavailable emails (non-fatal)
//! writer.log_error(43, "email not available");
//! ```
//!
//! # File Layout
//!
//! ```text
//! output/
//! ├── list.name/
//! │   ├── 1.eml                    # Fetched email
//! │   ├── 2.eml
//! │   ├── __progress.yaml    # YAML: last processed ID
//! │   └── __errors.csv                 # CSV: id,error_message
//! ```

mod email_store;
mod error_log;
mod progress;

pub use email_store::EmailStore;
pub use error_log::ErrorLogger;
pub use progress::ProgressTracker;

use std::path::Path;

/// Facade combining progress tracking, error logging, and email storage
/// for a single mailing list.
///
/// Created once per list by a worker. Safe to share across threads via
/// `&self` since all internal state is file-based.
///
/// # Why a Facade?
///
/// Instead of workers managing their own file I/O, `ArchiveWriter` provides
/// a single interface that all workers use. This ensures consistent behavior
/// across different source implementations (NNTP, IMAP, mbox, etc.).
pub struct ArchiveWriter {
    progress: ProgressTracker,
    error_log: ErrorLogger,
    email_store: EmailStore,
}

impl ArchiveWriter {
    /// Creates a new archive writer for the given list.
    ///
    /// # Arguments
    ///
    /// * `base_output_path` - Root output directory (e.g., `./output`)
    /// * `list_name` - Mailing list name (becomes subdirectory)
    pub fn new(base_output_path: &Path, list_name: &str) -> Self {
        Self {
            progress: ProgressTracker::new(base_output_path, list_name),
            error_log: ErrorLogger::new(base_output_path, list_name),
            email_store: EmailStore::new(base_output_path, list_name),
        }
    }

    /// Returns the last processed email ID from persisted state.
    ///
    /// This is the primary entry point for resume support. Workers should
    /// call this before starting to fetch emails, then start from the
    /// returned ID + 1.
    ///
    /// If no progress file exists, returns `0` and initializes one to mark
    /// the list as discovered.
    pub fn last_processed_id(&self) -> usize {
        self.progress.last_processed_id()
    }

    /// Writes a fetched email to disk.
    ///
    /// # Arguments
    ///
    /// * `email_id` - email number
    /// * `lines` - Raw email lines
    pub fn write_email(&self, email_id: usize, lines: &[String]) -> crate::Result<()> {
        self.email_store.write(email_id, lines)
    }

    /// Persists the last successfully processed email ID.
    ///
    /// Should be called after each successful `write_email` to ensure
    /// progress is saved. This enables the worker to resume from this
    /// point if interrupted.
    pub fn update_progress(&self, id: usize) -> crate::Result<()> {
        self.progress.update(id)
    }

    /// Logs an error for an unavailable email (non-fatal).
    ///
    /// Appends `{email_id},{error}` to the `__errors.csv` file.
    /// Failures to write the error log are logged as warnings but
    /// do not propagate as errors.
    pub fn log_error(&self, email_id: usize, error: &str) {
        self.error_log.log(email_id, error);
    }
}
