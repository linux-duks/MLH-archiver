// use crate::errors;
// use crate::file_utils;
use crate::errors;
use crate::worker::Worker;
use crate::{public_inbox_source::pi_config::PIConfig, worker};
use std::sync::{Arc, atomic::AtomicBool};

use crate::archive_writer::ArchiveWriter;
use crate::config::RunModeConfig;
use gix::bstr::ByteSlice;
use gix::revision::walk::Info;
// use std::cell::{Cell, RefCell};
// use std::fmt;
use std::path::Path;
// use std::sync::atomic::Ordering;
// use std::thread::sleep;
// use std::time::Duration;

pub struct PIWorker {
    id: u8,
    pi_config: PIConfig,
    base_output_path: String,
    shutdown_flag: Arc<AtomicBool>,
}

impl PIWorker {
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
            // recv() blocks until a message is available or channel is closed
            // When channel is closed AND empty, returns RecvError
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

    fn read_email_by_index(&self, list_name: String, email_index: usize) -> crate::Result<()> {
        // create ArchiveWriter instance for the list
        let writer = ArchiveWriter::new(
            Path::new(&self.base_output_path),
            &list_name,
            RunModeConfig::PublicInbox(self.pi_config.clone()),
        );

        // Find the public inbox directory by name
        let inboxes = crate::public_inbox_source::pi_utils::find_public_inboxes(std::path::Path::new(&self.pi_config.inport_directory))?;
        let inbox = inboxes.iter()
            .find(|inbox| inbox.name == list_name)
            .ok_or_else(|| anyhow::anyhow!("Public inbox '{}' not found in {}", list_name, self.pi_config.inport_directory))?;
        
        let repo = gix::open(&inbox.git_dir)?;
        let mut repo = repo;
        repo.object_cache_size(50_000_000); // 50MB cache

        // Get commit at position (email_index is 1-indexed per NNTP convention)
        if email_index == 0 {
            return Err(crate::errors::Error::Config(crate::errors::ConfigError::MissingHostname));
        }
        let position = email_index - 1;

        let commit_info = self.get_commit_at_position(&repo, position)?;
        let commit = repo.find_commit(commit_info.id)?;
        
        // Extract email from commit
        let (commit_hash, raw_email) = self.extract_email_from_commit(&repo, &commit)?;
        
        // Archive the email
        writer.archive_email(commit_hash, [raw_email.as_str()])?;
        
        log::info!("W{}: Successfully fetched email {} from {}", self.id, email_index, list_name);
        Ok(())
    }
}

impl PIWorker {
    fn process_inbox(&self, list_name: &str) -> errors::Result<usize> {
        // create ArchiveWriter instance for the new list
        let writer = ArchiveWriter::new(
            Path::new(&self.base_output_path),
            list_name,
            RunModeConfig::PublicInbox(self.pi_config.clone()),
        );

        // Find the public inbox directory by name
        let inboxes = crate::public_inbox_source::pi_utils::find_public_inboxes(std::path::Path::new(&self.pi_config.inport_directory))?;
        let inbox = inboxes.iter()
            .find(|inbox| inbox.name == list_name)
            .ok_or_else(|| anyhow::anyhow!("Public inbox '{}' not found in {}", list_name, self.pi_config.inport_directory))?;
        
        let repo = gix::open(&inbox.git_dir)?;

        // Enable object cache for better performance
        let mut repo = repo;
        repo.object_cache_size(50_000_000); // 50MB cache

        // Resolve refs/heads/master to get the HEAD commit
        let head_ref = repo.refs.find("refs/heads/master")?;

        let head_id = head_ref
            .target
            .try_id()
            .ok_or_else(|| anyhow::anyhow!("refs/heads/master does not point to an object"))?
            .to_owned();

        // Walk all commits from HEAD (tip/newest first)
        let all_commit_ids: Vec<_> = repo
            .rev_walk([head_id])
            .all()?
            .filter_map(|r| r.ok())
            .collect();

        if all_commit_ids.is_empty() {
            return Ok(0);
        }

        // Determine which positions to process based on article_range
        let positions_to_process: std::collections::HashSet<usize> = match &self.pi_config.article_range {
            Some(range_str) => {
                // Parse article range (1-indexed) and convert to 0-indexed positions
                let parsed = crate::range_inputs::parse_sequence(range_str)
                    .map_err(|_e| crate::errors::Error::Config(crate::errors::ConfigError::MissingHostname))?;
                parsed.map(|article_num| article_num.saturating_sub(1)).collect()
            }
            None => {
                // Process all commits
                (0..all_commit_ids.len()).collect()
            }
        };

        let mut email_count = 0;

        for (position, info) in all_commit_ids.into_iter().enumerate() {
            // Skip if not in positions_to_process
            if !positions_to_process.contains(&position) {
                continue;
            }

            // Check shutdown flag before processing each commit
            if crate::worker::is_shutdown_requested(&self.shutdown_flag) {
                log::info!("W{}: Shutdown requested while processing {}, processed {} emails", self.id, list_name, email_count);
                return Ok(email_count);
            }

            let commit = repo.find_commit(info.id)?;
            match self.extract_email_from_commit(&repo, &commit) {
                Ok((commit_hash, raw_email)) => {
                    writer.archive_email(commit_hash, [raw_email.as_str()])?;
                    email_count += 1;
                }
                Err(_) => {
                    // Log error for missing message blob
                    writer.log_error(info.id.to_string(), "No 'm' blob found in commit tree");
                    log::warn!("W{}: Commit {} missing 'm' blob", self.id, info.id);
                }
            }
        }

        log::info!("W{}: Processed {} emails from {}", self.id, email_count, list_name);
        Ok(email_count)
    }

    /// Get commit at given position (0-indexed from newest).
    fn get_commit_at_position<'a>(&self, repo: &'a gix::Repository, position: usize) -> crate::Result<Info<'a>> {
        let head_ref = repo.refs.find("refs/heads/master")?;
        let head_id = head_ref
            .target
            .try_id()
            .ok_or_else(|| anyhow::anyhow!("refs/heads/master does not point to an object"))?
            .to_owned();

        let walk = repo.rev_walk([head_id]);
        let iter = walk.all().map_err(|e| anyhow::anyhow!(e))?;
        for (i, info) in iter.enumerate() {
            if i == position {
                return Ok(info.map_err(|e| anyhow::anyhow!(e))?);
            }
        }
        Err(crate::errors::Error::Config(crate::errors::ConfigError::MissingHostname))
    }

    /// Extract email content from a commit.
    /// Returns (commit_hash, raw_email).
    fn extract_email_from_commit(&self, repo: &gix::Repository, commit: &gix::Commit) -> crate::Result<(String, String)> {
        let commit_ref = commit.decode()?;
        let tree_id = commit_ref.tree();
        let tree = repo.find_tree(tree_id)?;

        let blob_oid = tree
            .iter()
            .find_map(|e| e.ok())
            .filter(|e| e.filename().as_bytes() == b"m")
            .map(|e| e.object_id());

        match blob_oid {
            Some(blob_oid) => {
                let raw_body = read_by_blob_id(repo, blob_oid)?;
                Ok((commit.id().to_string(), raw_body))
            }
            None => Err(crate::errors::Error::Config(crate::errors::ConfigError::MissingHostname)),
        }
    }
}

fn read_by_blob_id(repo: &gix::Repository, blob_oid: gix::ObjectId) -> crate::Result<String> {
    let blob = repo.find_blob(blob_oid)?;
    let raw_email = String::from_utf8_lossy(&blob.data).to_string();
    return Ok(raw_email);
}
