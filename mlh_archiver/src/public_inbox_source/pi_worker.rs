use crate::worker::Worker;
use crate::{
    public_inbox_source::{pi_config::PIConfig, pi_utils::*},
    worker,
};
use std::sync::{Arc, atomic::AtomicBool};

use crate::archive_writer::ArchiveWriter;
use crate::config::RunModeConfig;
use std::path::Path;

/// A worker that processes public inbox email archives.
///
/// This struct represents a worker that consumes inbox names from a channel and processes
/// the emails contained within those inboxes. It handles both V1 and V2 public inbox
/// formats, supports resuming from a specific email, and can filter emails by article range.
pub struct PIWorker {
    /// Unique identifier for this worker instance
    id: u8,
    /// Configuration for the public inbox source
    pi_config: PIConfig,
    /// Base output path where processed emails will be stored
    base_output_path: String,
    /// Flag used to signal the worker to shut down gracefully
    shutdown_flag: Arc<AtomicBool>,
}

impl PIWorker {
    /// Creates a new PIWorker instance.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this worker
    /// * `pi_config` - Configuration for accessing the public inbox
    /// * `base_output_path` - Directory where processed emails will be written
    /// * `shutdown_flag` - Atomic boolean used to request worker shutdown
    ///
    /// # Returns
    ///
    /// * `PIWorker` - A configured worker instance ready to process inboxes
    pub fn new(
        id: u8,
        pi_config: PIConfig,
        base_output_path: String,
        shutdown_flag: Arc<AtomicBool>,
    ) -> PIWorker {
        return PIWorker {
            id,
            pi_config,
            base_output_path,
            shutdown_flag,
        };
    }
}

impl Worker for PIWorker {
    /// Consumes inbox names from a channel and processes each one.
    ///
    /// This function runs in a loop, receiving inbox names from the provided channel
    /// and processing each inbox by calling `process_inbox`. It continues until
    /// a shutdown is requested or the channel is closed.
    ///
    /// # Arguments
    ///
    /// * `self` - The worker instance (boxed for trait object compatibility)
    /// * `receiver` - Channel that provides inbox names to process
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the worker exits cleanly (shutdown requested or channel closed)
    /// * `Err` - If an error occurs while processing an inbox (logged but doesn't stop the worker)
    fn consumme_list(
        self: Box<Self>,
        receiver: crossbeam_channel::Receiver<String>,
    ) -> crate::Result<()> {
        log::info!("W{}: started consuming tasks", self.id);
        loop {
            if worker::is_shutdown_requested(&self.shutdown_flag) {
                log::info!("W{}: Shutdown requested, exiting...", self.id);
                return Ok(());
            }

            log::info!("W{}: Reading new group from channel", self.id);
            match receiver.recv() {
                Ok(name) => {
                    if let Err(e) = self.process_inbox(name.as_str()) {
                        log::error!("W{}: Failed to process inbox {}: {}", self.id, name, e);
                    }
                }
                Err(crossbeam_channel::RecvError) => {
                    log::info!("W{}: Channel closed and empty, worker exiting", self.id);
                    return Ok(());
                }
            }
        }
    }

    /// Reads a specific email by its 1-indexed position in the inbox.
    ///
    /// This function retrieves a single email from a public inbox based on its
    /// position in the overall email sequence (across all epochs for V2 inboxes).
    /// It's used for testing and random access to specific emails.
    ///
    /// # Arguments
    ///
    /// * `self` - The worker instance
    /// * `list_name` - The name of the public inbox to read from
    /// * `email_index` - The 1-indexed position of the email to retrieve
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the email was successfully retrieved and archived
    /// * `Err` - If the inbox is not found, the index is out of bounds, or an error occurs
    fn read_email_by_index(&self, list_name: String, email_index: usize) -> crate::Result<()> {
        let writer = ArchiveWriter::new(
            Path::new(&self.base_output_path),
            &list_name,
            RunModeConfig::PublicInbox(self.pi_config.clone()),
        );

        let inboxes = find_public_inboxes(std::path::Path::new(&self.pi_config.import_directory))?;
        let inbox = inboxes
            .iter()
            .find(|inbox| inbox.name == list_name)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Public inbox '{}' not found in {}",
                    list_name,
                    self.pi_config.import_directory
                )
            })?;

        let epochs = find_epochs(&inbox.git_dir)?;
        let epochs_to_use = if epochs.is_empty() {
            vec![EpochRepo {
                epoch_name: "1".to_string(),
                git_dir: inbox.git_dir.clone(),
            }]
        } else {
            epochs
        };

        if email_index == 0 {
            return Err(crate::errors::Error::Config(
                crate::errors::ConfigError::MissingHostname,
            ));
        }

        let mut remaining = email_index;
        for epoch in &epochs_to_use {
            let repo = gix::open(&epoch.git_dir)?;
            let mut repo = repo;
            repo.object_cache_size(50_000_000);

            let commit_count = count_commits(&repo)?;
            if remaining <= commit_count {
                let position = remaining - 1;
                let commit_info = get_commit_at_position(&repo, position)?;
                let commit = repo.find_commit(commit_info.id)?;
                let (commit_hash, raw_email) = extract_email_from_commit(&repo, &commit)?;
                let email_id = format_email_id(email_index, &epoch.epoch_name, &commit_hash);
                writer.archive_email(&email_id, [raw_email.as_str()])?;
                log::info!(
                    "W{}: Successfully fetched email {} from {} (epoch {})",
                    self.id,
                    email_index,
                    list_name,
                    epoch.epoch_name
                );
                return Ok(());
            }
            remaining -= commit_count;
        }

        Err(anyhow::anyhow!(
            "Email index {} exceeds total emails in {}",
            email_index,
            list_name
        )
        .into())
    }
}

impl PIWorker {
    /// Processes an entire public inbox, archiving all emails.
    ///
    /// This is the main processing function for the PIWorker. It iterates through
    /// all commits in the inbox (across all epochs for V2 inboxes), extracts the
    /// email content from each commit, and archives it using the ArchiveWriter.
    /// It supports resuming from a specific email based on progress tracking
    /// and filtering by article range.
    ///
    /// # Arguments
    ///
    /// * `self` - The worker instance
    /// * `list_name` - The name of the public inbox to process
    ///
    /// # Returns
    ///
    /// * `Ok(usize)` - The number of emails successfully processed
    /// * `Err` - If the inbox is not found or an error occurs during processing
    fn process_inbox(&self, list_name: &str) -> crate::Result<usize> {
        log::info!(
            "W{}: Starting processing emails from {}",
            self.id,
            list_name
        );

        let writer = ArchiveWriter::new(
            Path::new(&self.base_output_path),
            list_name,
            RunModeConfig::PublicInbox(self.pi_config.clone()),
        );

        let inboxes = find_public_inboxes(std::path::Path::new(&self.pi_config.import_directory))?;
        let inbox = inboxes
            .iter()
            .find(|inbox| inbox.name == list_name)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Public inbox '{}' not found in {}",
                    list_name,
                    self.pi_config.import_directory
                )
            })?;

        let epochs = find_epochs(&inbox.git_dir)?;
        let epochs_to_use = if epochs.is_empty() {
            vec![EpochRepo {
                epoch_name: "1".to_string(),
                git_dir: inbox.git_dir.clone(),
            }]
        } else {
            epochs
        };

        // Check for progress to determine where to resume from
        let last_email = writer.last_processed_id();
        let resume_info = last_email.and_then(|id| parse_email_id(&id));

        let mut email_count = 0;
        let mut next_email_num = 1;
        let mut skip_until_epoch = None;
        let mut skip_until_sha = None;

        // If we have resume information, set up skipping to that point
        if let Some(ref parsed) = resume_info {
            skip_until_epoch = Some(parsed.epoch_name.clone());
            skip_until_sha = Some(parsed.short_sha.clone());
        }

        // Parse article range if configured
        let article_range_positions: Option<std::collections::HashSet<usize>> = match &self
            .pi_config
            .article_range
        {
            Some(range_str) => {
                let parsed_range =
                    crate::range_inputs::parse_sequence(range_str).map_err(|_e| {
                        crate::errors::Error::Config(crate::errors::ConfigError::MissingHostname)
                    })?;
                Some(
                    parsed_range
                        .map(|article_num| article_num.saturating_sub(1)) // Convert to 0-indexed
                        .collect(),
                )
            }
            None => None,
        };

        let mut global_position: usize = 0;

        // Process each epoch in order
        for epoch in &epochs_to_use {
            // Skip entire epochs if we're resuming from a later epoch
            if let Some(ref target_epoch) = skip_until_epoch
                && epoch.epoch_name != *target_epoch
            {
                let repo = gix::open(&epoch.git_dir)?;
                let count = count_commits(&repo)?;
                next_email_num += count;
                global_position += count;
                continue;
            }

            let repo = gix::open(&epoch.git_dir)?;
            let mut repo = repo;
            repo.object_cache_size(50_000_000);

            // Collect all commit IDs from newest to oldest
            let all_commit_ids = collect_all_commits(&repo)?;
            let commit_count = all_commit_ids.len();

            if all_commit_ids.is_empty() {
                continue;
            }

            let mut found_resume_sha = false;
            let mut in_epoch_skipping = skip_until_sha.is_some();

            // Process each commit in the epoch
            for (local_pos, commit_id) in all_commit_ids.into_iter().enumerate() {
                let current_global_position = global_position + local_pos;

                // Apply article range filter if configured
                if let Some(ref positions) = article_range_positions
                    && !positions.contains(&current_global_position)
                {
                    continue;
                }

                // If we're skipping to a specific SHA within this epoch, skip until we find it
                if in_epoch_skipping {
                    let short = if commit_id.to_string().len() >= 7 {
                        commit_id.to_string()[..7].to_string()
                    } else {
                        commit_id.to_string()
                    };
                    if let Some(ref target_sha) = skip_until_sha
                        && short == *target_sha
                    {
                        found_resume_sha = true;
                        in_epoch_skipping = false;
                        continue;
                    }
                    continue;
                }

                // Check for shutdown request
                if crate::worker::is_shutdown_requested(&self.shutdown_flag) {
                    log::info!(
                        "W{}: Shutdown requested while processing {}, processed {} emails",
                        self.id,
                        list_name,
                        email_count
                    );
                    return Ok(email_count);
                }

                // Extract and archive the email from this commit
                let commit = repo.find_commit(commit_id)?;
                match extract_email_from_commit(&repo, &commit) {
                    Ok((commit_hash, raw_email)) => {
                        let email_id =
                            format_email_id(next_email_num, &epoch.epoch_name, &commit_hash);
                        writer.archive_email(&email_id, [raw_email.as_str()])?;
                        email_count += 1;
                        next_email_num += 1;
                    }
                    Err(_) => {
                        writer
                            .log_error(&commit_id.to_string(), "No 'm' blob found in commit tree");
                        let subject = match commit.message() {
                            Ok(msg) => msg.summary().to_string(),
                            Err(_) => "<no message>".to_string(),
                        };
                        let tree_str = match commit.tree_id() {
                            Ok(tid) => tid.to_string(),
                            Err(_) => "<unknown>".to_string(),
                        };
                        log::debug!(
                            "W{}: Commit {} missing 'm' blob - subject: '{}', parents: {}, tree: {}",
                            self.id,
                            commit_id,
                            subject,
                            commit.parent_ids().count(),
                            tree_str,
                        );
                        next_email_num += 1;
                    }
                }
            }

            // Warn if we were looking for a resume SHA but didn't find it in this epoch
            if skip_until_sha.is_some() && !found_resume_sha {
                log::warn!(
                    "W{}: Resume SHA {:?} not found in epoch {}, starting from beginning of this epoch",
                    self.id,
                    skip_until_sha,
                    epoch.epoch_name
                );
            }

            global_position += commit_count;

            // Reset skipping flags after processing the target epoch
            skip_until_epoch = None;
            skip_until_sha = None;
        }

        log::info!(
            "W{}: Processed {} emails from {}",
            self.id,
            email_count,
            list_name
        );
        Ok(email_count)
    }
}
