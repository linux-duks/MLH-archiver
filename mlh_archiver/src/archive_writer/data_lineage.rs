//! Data lineage tracking for the archive writer.
//!
//! Every time an article is fetched and stored, a `DataLineage` record is
//! appended to the `__progress.yaml` file. This creates an append-only audit
//! trail that captures:
//!
//! - **What** was fetched (article ID, list name)
//! - **Where** it came from (source type / run mode configuration)
//! - **When** it was fetched (UTC timestamp)
//! - **With which version** of the archiver (build info including commit,
//!   target platform, Rust version, build time)
//!
//! The `__progress.yaml` file is a multi-document YAML stream where each
//! document is a `DataLineage` entry, separated by `---`.
//!
//! # Example file content
//!
//! ```yaml
//! email_index: 1
//! list_name: test.groups.foo
//! source_type: "NNTP h=localhost"
//! timestamp: 2025-01-15T10:30:00Z
//! archiver_build_info: "Archiver v=0.1.0 commit=abc123 ..."
//! ---
//! email_index: 2
//! list_name: test.groups.foo
//! source_type: "NNTP h=localhost"
//! timestamp: 2025-01-15T10:30:05Z
//! archiver_build_info: "Archiver v=0.1.0 commit=abc123 ..."
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use crate::config::{RunModeConfig, built_info};

/// Shared build info string — computed once, cloned cheaply via `Arc`.
static BUILD_INFO: LazyLock<Arc<str>> = LazyLock::new(|| {
    format!(
        "Archiver v='{}' commit='{}' dirty='{}' build_time_utc='{}' target='{}' rustc='{}'",
        built_info::PKG_VERSION,
        built_info::GIT_VERSION.unwrap_or("unknown"),
        match built_info::GIT_DIRTY {
            Some(true) => "true",
            Some(false) => "false",
            None => "unknown",
        },
        built_info::BUILT_TIME_UTC,
        built_info::TARGET,
        built_info::RUSTC_VERSION,
    )
    .into()
});

/// Progress state for a mailing list.
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct DataLineage {
    /// email id/file_name
    pub(crate) email_index: String,
    /// mailing list name
    pub(crate) list_name: String,
    /// name of the RunMode
    pub(crate) source_type: String,
    /// specific for each run mode. Server, directory...
    // pub(crate) source_details: String,
    /// date when the read was performed
    pub(crate) timestamp: DateTime<chrono::Utc>,
    /// build information about the archiver software
    pub(crate) archiver_build_info: String,
}

pub struct DataLineageWriter {
    output_path: PathBuf,
    list_name: String,
    build_info: Arc<str>,
    // save as string, ready to format
    run_mode: String,
}

impl DataLineageWriter {
    /// # Arguments
    ///
    /// * `base_path` - Root output directory (e.g., `./output`)
    /// * `list_name` - Mailing list name (becomes subdirectory)
    pub fn new(base_path: &Path, list_name: &str, run_mode: RunModeConfig) -> Self {
        Self {
            output_path: base_path.join(list_name).join("__lineage.yaml"),
            list_name: list_name.to_string(),
            build_info: BUILD_INFO.clone(),
            run_mode: run_mode.to_string(),
        }
    }

    /// Persists the last successfully processed email ID.
    ///
    /// # Arguments
    ///
    /// * `id` - email ID that was just processed
    pub fn update(&self, id: &str) -> crate::Result<()> {
        crate::file_utils::append_yaml_to_file(
            self.output_path.to_str().unwrap(),
            &DataLineage {
                email_index: id.to_string(),
                list_name: self.list_name.clone(),
                source_type: self.run_mode.clone(),
                archiver_build_info: (*self.build_info).to_string(),
                timestamp: Utc::now(),
            },
        )
        .map_err(crate::errors::Error::Io)
    }
}
