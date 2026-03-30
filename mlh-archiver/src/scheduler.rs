use crate::config::NntpConfig;
use crate::errors;
use crate::worker;
use crossbeam_channel::bounded;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

// intervals in seconds
const INTERVAL_BETWEEN_RESCANS: usize = 60 * 60; // 1h

pub struct Scheduler {
    nntp_config: NntpConfig,
    base_output_path: String,
    nthreds: u8,
    loop_groups: bool,
    tasklist: Arc<Vec<String>>,
}

impl Scheduler {
    pub fn new(
        nntp_config: NntpConfig,
        base_output_path: String,
        nthreds: u8,
        loop_groups: bool,
        groups: Vec<String>,
    ) -> Scheduler {
        let mut tasklist: Vec<String> = Vec::with_capacity(groups.len());

        // Schedule all groups for check to the next second
        for group in groups {
            tasklist.push(group.clone());
        }

        Scheduler {
            nntp_config,
            base_output_path,
            nthreds,
            loop_groups,
            tasklist: Arc::new(tasklist),
        }
    }

    pub fn run(&mut self) -> crate::Result<()> {
        // Create channel - sender stays with main thread, receivers go to workers
        let (sender, receiver): (
            crossbeam_channel::Sender<String>,
            crossbeam_channel::Receiver<String>,
        ) = bounded(self.nthreds as usize);

        // Collect thread handles
        let mut handles = Vec::with_capacity(self.nthreds as usize);

        // Clone data needed for workers
        let tasklist = Arc::clone(&self.tasklist);

        // Start worker threads - each gets a clone of the receiver
        for id in 0..self.nthreds {
            log::debug!("Starting worker thread {id}");

            let receiver = receiver.clone();

            let mut worker = worker::Worker::new(
                id,
                self.nntp_config.clone(),
                self.base_output_path.clone(),
                receiver,
            );

            // Spawn worker thread
            let handle = thread::spawn(move || {
                loop {
                    match worker.run() {
                        Ok(_) => {
                            log::info!("Worker {id} finished");
                            break;
                        }
                        Err(err) => {
                            log::warn!("Worker {id} returned an error: {err}");
                            std::thread::sleep(Duration::from_secs(1));
                        }
                    }
                }
            });

            handles.push(handle);

            // Space out thread creation (to prevent multiple connections opening at once)
            std::thread::sleep(Duration::from_secs(2));
        }

        // Drop the original receiver - now only worker receivers exist
        // When all workers drop their receivers, recv() will return RecvError
        drop(receiver);

        // Setup signal handler for Ctrl+C (only needed for loop_groups mode)
        if self.loop_groups {
            let shutdown_flag = Arc::new(AtomicBool::new(false));
            let shutdown_flag_signal = Arc::clone(&shutdown_flag);

            ctrlc::set_handler(move || {
                log::info!("Received shutdown signal (Ctrl+C), stopping workers...");
                shutdown_flag_signal.store(true, Ordering::Relaxed);
            })
            .map_err(|e| {
                errors::Error::Io(std::io::Error::other(format!(
                    "Failed to set Ctrl+C handler: {}",
                    e
                )))
            })?;

            // Main scheduling loop (runs in main thread)
            loop {
                // Check if shutdown was requested
                if shutdown_flag.load(Ordering::Relaxed) {
                    log::info!("Shutdown requested, stopping task dispatch...");
                    break;
                }

                // Send tasks to workers
                for group_name in tasklist.iter() {
                    if shutdown_flag.load(Ordering::Relaxed) {
                        break;
                    }
                    // Send may fail if all receivers are closed
                    if sender.send(group_name.clone()).is_err() {
                        log::warn!("Failed to send task, workers may have stopped");
                        break;
                    }
                }

                // Sleep between checks, but wake up periodically to check shutdown flag
                let sleep_interval = Duration::from_secs(INTERVAL_BETWEEN_RESCANS as u64);
                let check_interval = Duration::from_secs(5);
                let mut elapsed = Duration::ZERO;
                while elapsed < sleep_interval {
                    if shutdown_flag.load(Ordering::Relaxed) {
                        break;
                    }
                    std::thread::sleep(check_interval);
                    elapsed += check_interval;
                }
            }

            // Signal shutdown to workers
            drop(sender);
        } else {
            // Non-looping mode: send all tasks once, then drop sender
            log::info!("Sending {} tasks to workers...", tasklist.len());
            for group_name in tasklist.iter() {
                if sender.send(group_name.clone()).is_err() {
                    log::warn!("Failed to send task, workers may have stopped");
                    break;
                }
            }
            log::info!("All tasks sent. Waiting for workers to complete...");
            // Drop sender to signal workers to exit after draining channel
            drop(sender);
        }

        log::info!("Waiting for {} worker threads to finish...", handles.len());

        // Wait for all worker threads to finish
        for (i, handle) in handles.into_iter().enumerate() {
            log::debug!("Joining worker thread {i}...");
            if let Err(e) = handle.join() {
                log::error!("Failed to join worker thread {i}: {:?}", e);
            }
        }

        log::info!("All worker threads stopped");

        Ok(())
    }

    // run_range does not keep track of lists, just run them once for the defined range
    pub fn run_range(&mut self, range: impl Iterator<Item = usize>) -> crate::Result<()> {
        // Create a channel for single-run mode
        let (_sender, receiver): (
            crossbeam_channel::Sender<String>,
            crossbeam_channel::Receiver<String>,
        ) = bounded(1);

        let mut worker = worker::Worker::new(
            0,
            self.nntp_config.clone(),
            self.base_output_path.clone(),
            receiver,
        );

        match self.tasklist.first() {
            Some(group_name) => {
                worker.handle_group_range(group_name.clone(), range)?;
                Ok(())
            }
            None => Err(errors::Error::Unknown),
        }?;

        return Ok(());
    }
}
