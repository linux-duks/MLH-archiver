use crate::config::AppConfig;
use crate::range_inputs::parse_sequence;
use crate::worker::{Worker, WorkerGroup};
use crossbeam_channel::bounded;
use std::thread::{self, JoinHandle};
use std::time::Duration;

// intervals in seconds
const INTERVAL_BETWEEN_RESCANS: usize = 60 * 60; // 1h

/// Channel capacity per worker group
const CHANNEL_CAPACITY: usize = 10;

pub struct Scheduler<'a> {
    app_config: &'a AppConfig,
    loop_groups: bool,
    worker_groups: &'a mut Vec<WorkerGroup>,
}

impl<'a> Scheduler<'a> {
    pub fn new(
        app_config: &'a AppConfig,
        worker_groups: &'a mut Vec<WorkerGroup>,
    ) -> Scheduler<'a> {
        Scheduler {
            app_config,
            loop_groups: app_config.loop_groups,
            worker_groups,
        }
    }

    pub fn run(&mut self) -> crate::Result<()> {
        // Collect thread handles
        let mut handles = Vec::new();

        // Process each worker group - collect tasks first to avoid borrow issues
        let groups: Vec<WorkerGroup> = self.worker_groups.drain(..).collect();

        for group in groups {
            let WorkerGroup {
                tasks,
                workers,
                run_mode,
            } = group;

            // Create channel - sender goes to producer thread, receivers cloned to workers
            let (sender, receiver): (
                crossbeam_channel::Sender<String>,
                crossbeam_channel::Receiver<String>,
            ) = bounded(CHANNEL_CAPACITY);

            if let Some(range_text) = self.app_config.get_range_selection_text(run_mode) {
                // RUN in range mode
                if tasks.len() > 1 {
                    log::warn!(
                        "Multiple lists selected in Range Mode. This is likely a mistake..."
                    );
                    thread::sleep(Duration::from_secs(2));
                }
                let worker_handles = self.spawn_worker_to_read_email_by_index(workers, receiver);

                handles.extend(worker_handles);

                // Spawn producer thread that sends tasks
                let producer_handle =
                    Self::spawn_producer_for_read_email_by_index(sender, tasks, range_text);
                handles.push(producer_handle);

                // read_email_by_index
            } else {
                // Spawn worker threads - each worker is moved to its own thread
                // receiver is moved to the function, and dropped on return
                // When all workers drop their receivers, recv() will return RecvError
                let worker_handles = self.spawn_workers_to_consumme_list(workers, receiver);
                // Collect worker handles
                handles.extend(worker_handles);

                // Spawn producer thread that sends tasks
                let producer_handle =
                    Self::spawn_producer_for_consume_list(sender, tasks, self.loop_groups);
                handles.push(producer_handle);
            }
        }

        log::info!("Waiting for {} threads to finish...", handles.len());

        // Wait for all threads to finish
        for (i, handle) in handles.into_iter().enumerate() {
            log::debug!("Joining thread {i}...");
            if let Err(e) = handle.join() {
                log::error!("Failed to join thread {i}: {:?}", e);
            }
        }

        log::info!("All threads stopped");

        Ok(())
    }

    /// Wrapper to spawn a thread with the `run` worker trait
    fn spawn_workers_to_consumme_list(
        &self,
        workers: Vec<Box<dyn Worker>>,
        receiver: crossbeam_channel::Receiver<String>,
    ) -> Vec<JoinHandle<()>> {
        let mut worker_handles = Vec::with_capacity(workers.len());
        for worker in workers {
            let receiver_chan = receiver.clone();

            // Spawn worker thread - worker is moved here
            let handle = thread::spawn(move || {
                // Worker runs until channel is closed or shutdown requested
                match worker.consumme_list(receiver_chan) {
                    Ok(_) => {
                        log::debug!("Worker thread finished");
                    }
                    Err(err) => {
                        log::error!("Worker thread finished with error: {err}");
                    }
                }
            });
            worker_handles.push(handle);

            // Space out thread creation (to prevent multiple connections opening at once)
            std::thread::sleep(Duration::from_secs(2));
        }
        return worker_handles;
    }

    /// Wrapper to spawn a thread with the `run` worker trait
    fn spawn_worker_to_read_email_by_index(
        &self,
        workers: Vec<Box<dyn Worker>>,
        receiver: crossbeam_channel::Receiver<String>,
    ) -> Vec<JoinHandle<()>> {
        let mut worker_handles: Vec<JoinHandle<()>> = Vec::with_capacity(workers.len());

        for worker in workers {
            let receiver_chan = receiver.clone();

            // Spawn worker thread - worker is moved here
            let handle = thread::spawn(move || {
                loop {
                    let task = match receiver_chan.recv() {
                        Ok(name) => name,
                        Err(crossbeam_channel::RecvError) => {
                            // log::info!("W{}: Channel closed and empty, worker exiting", self.id);
                            return;
                        }
                    };

                    // Worker runs until channel is closed or shutdown requested
                    let (group_name, index) = task
                        .split_once("_##")
                        .expect("This task message should not fail");

                    match worker.read_email_by_index(
                        group_name.to_string(),
                        index
                            .parse::<usize>()
                            .expect("Task index shoud parse into a usize"),
                    ) {
                        Ok(_) => {
                            log::debug!("Worker thread finished");
                        }
                        Err(err) => {
                            log::error!("Worker thread finished with error: {err}");
                        }
                    }
                }
            });
            worker_handles.push(handle);

            // Space out thread creation (to prevent multiple connections opening at once)
            std::thread::sleep(Duration::from_secs(2));
        }
        return worker_handles;
    }

    /// Spawn a producer thread that sends tasks to workers via channel
    fn spawn_producer_for_consume_list(
        sender: crossbeam_channel::Sender<String>,
        tasks: Vec<String>,
        loop_groups: bool,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            // if in Looping mode: send tasks, then sleep and repeat
            // otherwise, send and drop the sender
            loop {
                // Send all tasks
                for task in &tasks {
                    if sender.send(task.clone()).is_err() {
                        log::warn!("Failed to send task, workers may have stopped");
                        drop(sender);
                        return;
                    }
                }

                // If not in looping mode, close sender and exit
                if !loop_groups {
                    log::info!("All tasks sent. Waiting for workers to complete...");
                    drop(sender);
                    return;
                }

                log::debug!("All tasks sent, waiting for rescan interval...");

                // Sleep between rescans
                std::thread::sleep(Duration::from_secs(INTERVAL_BETWEEN_RESCANS as u64));
            }
        })
    }

    fn spawn_producer_for_read_email_by_index(
        sender: crossbeam_channel::Sender<String>,
        tasks: Vec<String>,
        range_text: String,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            for task in &tasks {
                // Parse range text for each task (memory efficient, lazy evaluation)
                let range = match parse_sequence(&range_text) {
                    Ok(iter) => iter,
                    Err(e) => {
                        log::error!("Failed to parse range '{}': {}", range_text, e);
                        drop(sender);
                        return;
                    }
                };

                for index in range {
                    if sender.send(format!("{}_##{}", task, index)).is_err() {
                        log::warn!("Failed to send task, workers may have stopped");
                        drop(sender);
                        return;
                    }
                }
            }

            // If not in looping mode, close sender and exit
            log::info!("All tasks sent. Waiting for workers to complete...");
            drop(sender);
            return;
        })
    }
}
