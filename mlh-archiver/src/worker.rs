use std::sync::{Arc, atomic::AtomicBool};

use crate::config::{AppConfig, RunModes};
use crate::nntp_source::nntp_worker::NNTPWorker;

/// Helper to check if shutdown was requested
pub fn is_shutdown_requested(shutdown_flag: &Arc<AtomicBool>) -> bool {
    shutdown_flag.load(std::sync::atomic::Ordering::Relaxed)
}

pub trait Worker: Send {
    fn run(self: Box<Self>, receiver: crossbeam_channel::Receiver<String>) -> crate::Result<()>;
}

/// A group of workers that share the same task list and channel
pub struct WorkerGroup {
    pub tasks: Vec<String>,
    pub workers: Vec<Box<dyn Worker>>,
}

/// Manages ownership of all workers for the program lifetime
pub struct WorkerManager {
    groups: Vec<WorkerGroup>,
}

impl WorkerManager {
    pub fn new() -> Self {
        WorkerManager { groups: Vec::new() }
    }

    /// Create workers for a specific run mode
    pub fn create_workers(
        &mut self,
        run_mode: RunModes,
        tasks: Vec<String>,
        app_config: &AppConfig,
        shutdown_flag: Arc<AtomicBool>,
    ) {
        match run_mode {
            RunModes::NNTP(nntp_config) => {
                let num_workers = app_config.nthreads.max(1) as usize;
                let mut workers: Vec<Box<dyn Worker>> = Vec::with_capacity(num_workers);

                for id in 0..num_workers {
                    let worker = NNTPWorker::new(
                        id as u8,
                        nntp_config.clone(),
                        app_config.output_dir.clone(),
                        shutdown_flag.clone(),
                    );
                    workers.push(Box::new(worker));
                }

                self.groups.push(WorkerGroup { tasks, workers });
            }
            RunModes::LocalMbox => {
                unimplemented!()
            }
        }
    }

    /// Get mutable reference to all worker groups
    pub fn get_groups(&mut self) -> &mut Vec<WorkerGroup> {
        &mut self.groups
    }

    /// Get number of groups
    pub fn num_groups(&self) -> usize {
        self.groups.len()
    }
}

impl Default for WorkerManager {
    fn default() -> Self {
        Self::new()
    }
}
