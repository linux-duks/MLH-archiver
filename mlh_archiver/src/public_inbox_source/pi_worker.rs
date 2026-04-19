use crate::worker::Worker;
use crate::{
    public_inbox_source::{pi_config::PIConfig, pi_utils::*},
    worker,
};
use std::sync::{Arc, atomic::AtomicBool};

use crate::archive_writer::ArchiveWriter;
use crate::config::RunModeConfig;
use std::path::Path;

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
        let writer = ArchiveWriter::new(
            Path::new(&self.base_output_path),
            &list_name,
            RunModeConfig::PublicInbox(self.pi_config.clone()),
        );

        let inboxes = find_public_inboxes(std::path::Path::new(&self.pi_config.inport_directory))?;
        let inbox = inboxes
            .iter()
            .find(|inbox| inbox.name == list_name)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Public inbox '{}' not found in {}",
                    list_name,
                    self.pi_config.inport_directory
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
        ).into())
    }
}

impl PIWorker {
    fn process_inbox(&self, list_name: &str) -> crate::Result<usize> {
        let writer = ArchiveWriter::new(
            Path::new(&self.base_output_path),
            list_name,
            RunModeConfig::PublicInbox(self.pi_config.clone()),
        );

        let inboxes = find_public_inboxes(std::path::Path::new(&self.pi_config.inport_directory))?;
        let inbox = inboxes
            .iter()
            .find(|inbox| inbox.name == list_name)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Public inbox '{}' not found in {}",
                    list_name,
                    self.pi_config.inport_directory
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

        let last_email = writer.last_processed_id();
        let resume_info = last_email.and_then(|id| parse_email_id(&id));

        let mut email_count = 0;
        let mut next_email_num = 1;
        let mut skip_until_epoch = None;
        let mut skip_until_sha = None;

        if let Some(ref parsed) = resume_info {
            skip_until_epoch = Some(parsed.epoch_name.clone());
            skip_until_sha = Some(parsed.short_sha.clone());
        }

        let article_range_positions: Option<std::collections::HashSet<usize>> =
            match &self.pi_config.article_range {
                Some(range_str) => {
                    let parsed_range =
                        crate::range_inputs::parse_sequence(range_str).map_err(|_e| {
                            crate::errors::Error::Config(
                                crate::errors::ConfigError::MissingHostname,
                            )
                        })?;
                    Some(
                        parsed_range
                            .map(|article_num| article_num.saturating_sub(1))
                            .collect(),
                    )
                }
                None => None,
            };

        let mut global_position: usize = 0;

        for epoch in &epochs_to_use {
            if let Some(ref target_epoch) = skip_until_epoch {
                if epoch.epoch_name != *target_epoch {
                    let repo = gix::open(&epoch.git_dir)?;
                    let count = count_commits(&repo)?;
                    next_email_num += count;
                    global_position += count;
                    continue;
                }
            }

            let repo = gix::open(&epoch.git_dir)?;
            let mut repo = repo;
            repo.object_cache_size(50_000_000);

            let all_commit_ids = collect_all_commits(&repo)?;
            let commit_count = all_commit_ids.len();

            if all_commit_ids.is_empty() {
                continue;
            }

            let mut found_resume_sha = false;
            let mut in_epoch_skipping = skip_until_sha.is_some();

            for (local_pos, commit_id) in all_commit_ids.into_iter().enumerate() {
                let current_global_position = global_position + local_pos;

                if let Some(ref positions) = article_range_positions {
                    if !positions.contains(&current_global_position) {
                        continue;
                    }
                }

                if in_epoch_skipping {
                    let short = if commit_id.to_string().len() >= 7 {
                        commit_id.to_string()[..7].to_string()
                    } else {
                        commit_id.to_string()
                    };
                    if let Some(ref target_sha) = skip_until_sha {
                        if short == *target_sha {
                            found_resume_sha = true;
                            in_epoch_skipping = false;
                            continue;
                        }
                    }
                    continue;
                }

                if crate::worker::is_shutdown_requested(&self.shutdown_flag) {
                    log::info!(
                        "W{}: Shutdown requested while processing {}, processed {} emails",
                        self.id,
                        list_name,
                        email_count
                    );
                    return Ok(email_count);
                }

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
                        writer.log_error(&commit_id.to_string(), "No 'm' blob found in commit tree");
                        log::warn!("W{}: Commit {} missing 'm' blob", self.id, commit_id);
                        next_email_num += 1;
                    }
                }
            }

            if skip_until_sha.is_some() && !found_resume_sha {
                log::warn!(
                    "W{}: Resume SHA {:?} not found in epoch {}, starting from beginning of this epoch",
                    self.id,
                    skip_until_sha,
                    epoch.epoch_name
                );
            }

            global_position += commit_count;

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
