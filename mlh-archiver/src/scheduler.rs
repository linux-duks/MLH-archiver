use crate::config::AppConfig;
use crate::worker::WorkerGroup;
use crossbeam_channel::bounded;
use std::thread;
use std::time::Duration;

// intervals in seconds
const INTERVAL_BETWEEN_RESCANS: usize = 60 * 60; // 1h

/// Channel capacity per worker group
const CHANNEL_CAPACITY: usize = 10;

pub struct Scheduler<'a> {
    _app_config: &'a AppConfig,
    _base_output_path: String,
    _nthreads: u8,
    loop_groups: bool,
    worker_groups: &'a mut Vec<WorkerGroup>,
}

impl<'a> Scheduler<'a> {
    pub fn new(
        app_config: &'a AppConfig,
        base_output_path: String,
        nthreads: u8,
        loop_groups: bool,
        worker_groups: &'a mut Vec<WorkerGroup>,
    ) -> Scheduler<'a> {
        Scheduler {
            _app_config: app_config,
            _base_output_path: base_output_path,
            _nthreads: nthreads,
            loop_groups,
            worker_groups,
        }
    }

    pub fn run(&mut self) -> crate::Result<()> {
        // Collect thread handles
        let mut handles = Vec::new();

        // Process each worker group - collect tasks first to avoid borrow issues
        let groups: Vec<WorkerGroup> = self.worker_groups.drain(..).collect();

        for group in groups {
            let WorkerGroup { tasks, workers } = group;

            // Create channel - sender goes to producer thread, receivers cloned to workers
            let (sender, receiver): (
                crossbeam_channel::Sender<String>,
                crossbeam_channel::Receiver<String>,
            ) = bounded(CHANNEL_CAPACITY);

            // Spawn worker threads - each worker is moved to its own thread
            let mut worker_handles = Vec::with_capacity(workers.len());
            for worker in workers {
                let receiver_chan = receiver.clone();

                // Spawn worker thread - worker is moved here
                let handle = thread::spawn(move || {
                    // Worker runs until channel is closed or shutdown requested
                    match worker.run(receiver_chan) {
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

            // Drop the original receiver - now only worker receivers exist
            // When all workers drop their receivers, recv() will return RecvError
            drop(receiver);

            // Spawn producer thread that sends tasks
            let producer_handle = Self::spawn_producer_static(sender, tasks, self.loop_groups);
            handles.push(producer_handle);

            // Collect worker handles
            handles.extend(worker_handles);
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

    /// Spawn a producer thread that sends tasks to workers via channel
    fn spawn_producer_static(
        sender: crossbeam_channel::Sender<String>,
        tasks: Vec<String>,
        loop_groups: bool,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            // Looping mode: send tasks, then sleep and repeat
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

    // run_range does not keep track of lists, just run them once for the defined range
    // pub fn run_range(&mut self, range: impl Iterator<Item = usize>) -> crate::Result<()> {
    //     let mut worker = nntp_worker::NNTPWorker::new(
    //         0,
    //         self.app_config.get_nntp_config().unwrap(),
    //         self.base_output_path.clone(),
    //     );
    //
    //     match self.tasklist.first() {
    //         Some(group_name) => {
    //             worker.handle_group_range(group_name.clone(), range)?;
    //             Ok(())
    //         }
    //         None => Err(errors::Error::Unknown),
    //     }?;
    //
    //     return Ok(());
    // }
}
