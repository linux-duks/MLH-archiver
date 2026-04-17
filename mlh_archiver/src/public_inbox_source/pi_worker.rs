// use crate::errors;
// use crate::file_utils;
use crate::errors;
use crate::worker::Worker;
use crate::{public_inbox_source::pi_config::PIConfig, worker};
use std::sync::{Arc, atomic::AtomicBool};

use crate::archive_writer::ArchiveWriter;
use crate::config::RunModeConfig;
use gix::bstr::ByteSlice;
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

    fn read_email_by_index(&self, _list_name: String, _email_index: usize) -> crate::Result<()> {
        Ok(())
    }
}

impl PIWorker {
    fn process_inbox(&self, list_name: &str) -> errors::Result<usize> {
        // create ArchiveWriter instance for the new list
        let writer = ArchiveWriter::new(
            Path::new(&self.base_output_path),
            &list_name,
            RunModeConfig::PublicInbox(self.pi_config.clone()),
        );

        let repo = gix::open(&list_name)?;

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

        // Take the first `count` commits (most recent)
        let commits_to_process: Vec<_> = all_commit_ids.into_iter().collect();

        for info in commits_to_process {
            let commit = repo.find_commit(info.id)?;
            let commit_ref = commit.decode()?;

            // Get the tree and find the "m" entry (message file)
            let tree_id = commit_ref.tree();
            let tree = repo.find_tree(tree_id)?;

            let blob_oid = tree
                .iter()
                .find_map(|e| e.ok())
                .filter(|e| e.filename().as_bytes() == b"m")
                .map(|e| e.object_id());

            match blob_oid {
                Some(blob_oid) => {
                    let raw_body = read_by_blob_id(&repo, blob_oid)?;
                    writer.archive_email(blob_oid.to_string(), &[raw_body.as_str()])?;
                }
                None => unimplemented!(),
            }
        }

        // repo dropped here; entire inbox freed from memory
        Ok(0)
    }
}

fn read_by_blob_id(repo: &gix::Repository, blob_oid: gix::ObjectId) -> crate::Result<String> {
    let blob = repo.find_blob(blob_oid)?;
    let raw_email = String::from_utf8_lossy(&blob.data).to_string();
    return Ok(raw_email);
}
