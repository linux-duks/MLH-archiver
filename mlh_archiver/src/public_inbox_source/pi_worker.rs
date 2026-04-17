// use crate::errors;
// use crate::file_utils;
use crate::errors;
use crate::worker::Worker;
use crate::{public_inbox_source::pi_config::PIConfig, worker};
use std::sync::{Arc, atomic::AtomicBool};

use gix::bstr::ByteSlice;
use chrono::DateTime;
// use std::cell::{Cell, RefCell};
// use std::fmt;
// use std::path::Path;
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
                    if let Err(e) = process_inbox(name.as_str()) {
                        log::error!("W{}: Failed to process inbox {}: {}", self.id, name, e);
                    }
                },
                Err(crossbeam_channel::RecvError) => {
                    log::info!("W{}: Channel closed and empty, worker exiting", self.id);
                    return Ok(());
                }
            }
            
        }
    }

    fn read_email_by_index(&self, _list_name: String, _email_index: usize) -> crate::Result<()> {
        Ok(())
    }
}

fn process_inbox(inbox: &str) -> errors::Result<usize> {
    let repo = gix::open(&inbox)?;

    // Enable object cache for better performance
    let mut repo = repo;
    repo.object_cache_size(50_000_000); // 50MB cache

    // Resolve refs/heads/master to get the HEAD commit
    let head_ref = repo
        .refs
        .find("refs/heads/master")?;

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

    // Take the first `count` commits (most recent)
    let commits_to_process: Vec<_> = all_commit_ids.into_iter().collect();

    let mut email_count = 0;

    for info in commits_to_process {
        let commit = repo.find_commit(info.id)?;
        let commit_ref = commit.decode()?;

        let author = commit_ref.author()?;
        let author_time = author.time()?;
        let subject = commit_ref.message.to_str_lossy().to_string();

        // Get the tree and find the "m" entry (message file)
        let tree_id = commit_ref.tree();
        let tree = repo.find_tree(tree_id)?;

        let blob_oid = tree
            .iter()
            .find_map(|e| e.ok())
            .filter(|e| e.filename().as_bytes() == b"m")
            .map(|e| e.object_id());

        if let Some(blob_oid) = blob_oid {
            let blob = repo.find_blob(blob_oid)?;
            let raw_email = String::from_utf8_lossy(&blob.data).to_string();

            // Print immediately
            let timestamp = DateTime::from_timestamp(author_time.seconds, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| format!("timestamp={}", author_time.seconds));

            let preview = if raw_email.len() > 500 {
                format!("{}...", &raw_email[..500])
            } else {
                raw_email
            };

            email_count += 1;
            println!("  --- Email {email_count} ---");
            println!("  Subject: {}", subject.lines().next().unwrap_or(""));
            println!("  Author:  {} <{}>", author.name, author.email);
            println!("  Date:    {timestamp}");
            println!("  Commit:  {}", info.id.to_hex());
            println!("  Raw email:");
            for line in preview.lines() {
                println!("    {line}");
            }
            println!();
            // blob, tree, commit dropped here; memory freed
        }
    }

    // repo dropped here; entire inbox freed from memory
    Ok(email_count)
}


