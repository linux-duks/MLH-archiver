use std::sync::{Arc, atomic::AtomicBool};

use crate::config::{AppConfig, RunMode, RunModeConfig};
use crate::nntp_source::nntp_worker::NNTPWorker;

/// Helper function to check if a shutdown has been requested via the shared flag.
///
/// This is a convenience function for checking the shutdown flag using
/// the correct memory ordering (`Relaxed`).
///
/// # Arguments
///
/// * `shutdown_flag` - Reference to the shared atomic shutdown flag
///
/// # Returns
///
/// `true` if shutdown was requested, `false` otherwise
///
/// # Example
///
/// ```rust,no_run
/// use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
/// use mlh_archiver::worker::is_shutdown_requested;
///
/// let flag = Arc::new(AtomicBool::new(false));
/// if is_shutdown_requested(&flag) {
///     // Clean up and exit
/// }
/// ```
#[inline]
pub fn is_shutdown_requested(shutdown_flag: &Arc<AtomicBool>) -> bool {
    shutdown_flag.load(std::sync::atomic::Ordering::Relaxed)
}

/// Trait representing a worker that can fetch emails from a specific source.
///
/// Workers are the core unit of email fetching. Each worker:
/// - Is created by [`WorkerManager`] with a unique ID
/// - Is moved to its own thread before execution
/// - Receives mailing list names via a crossbeam channel
/// - Checks a shared shutdown flag for graceful termination
/// - **Uses [`ArchiveWriter`](crate::archive_writer::ArchiveWriter) for all file I/O**
///
/// # File I/O Requirement
///
/// **All worker implementations MUST use `ArchiveWriter` for:**
/// - Writing fetched emails to disk (`.eml` files)
/// - Tracking progress (`__progress.yaml` YAML)
/// - Logging errors for unavailable articles (`__errors.csv` CSV)
///
/// This ensures consistent progress tracking and resume support across
/// all source types. Do NOT write files directly.
///
/// # Implementing a New Source
///
/// To implement a new source (e.g., IMAP, ListArchiveX):
///
/// 1. Create a worker struct with required fields:
///    - `shutdown_flag: Arc<AtomicBool>` for graceful shutdown
///    - Source-specific configuration and connection state
///
/// 2. Implement the trait methods:
///    - `consumme_list()` - Main loop that processes mailing lists from channel
///    - `read_email_by_index()` - Fetch a single email (for retry/recovery)
///
/// 3. Check `shutdown_flag` periodically:
///    - At start of each task iteration
///    - During long waits or retries
///    - During email fetching loops
///
/// 4. Use `RefCell` or `Mutex` for mutable connection state
///
/// # Thread Safety
///
/// Workers implement `Send` to allow moving across threads.
/// The `shutdown_flag` is shared via `Arc<AtomicBool>` for lock-free signaling.
///
/// # Lifecycle
///
/// 1. Created by [`WorkerManager::create_workers()`]
/// 2. Moved to a thread via [`Scheduler`](crate::scheduler::Scheduler)
/// 3. Runs until channel closes or shutdown is requested
/// 4. Dropped when thread completes
pub trait Worker: Send {
    /// Processes mailing lists received via channel until completion or shutdown.
    ///
    /// This is the main entry point for email fetching. Implementations should:
    /// 1. Loop indefinitely, receiving list names from the channel
    /// 2. Check shutdown flag at start of each iteration
    /// 3. Create an [`ArchiveWriter`](crate::archive_writer::ArchiveWriter) for the list
    /// 4. Fetch all new emails for the list using the writer for storage
    /// 5. Handle errors gracefully (retry, log, continue)
    ///
    /// # Arguments
    ///
    /// * `self` - Consumed by value (`Box<Self>`) as worker is moved to thread
    /// * `receiver` - Channel receiver for mailing list names
    ///
    /// # Returns
    ///
    /// * `Ok(())` on successful completion or graceful shutdown
    /// * `Err(...)` on unrecoverable errors (e.g., connection failure)
    ///
    /// # Channel Behavior
    ///
    /// - `receiver.recv()` blocks until a message is available
    /// - Returns `RecvError` when channel is closed AND empty
    /// - Multiple workers can share the same receiver (load balancing)
    fn consumme_list(
        self: Box<Self>,
        receiver: crossbeam_channel::Receiver<String>,
    ) -> crate::Result<()>;

    /// Fetches a single email by its index from a mailing list.
    ///
    /// This method is used for:
    /// - Re-fetching emails that previously failed
    /// - Fetching specific articles via `article_range` configuration
    ///
    /// # Arguments
    ///
    /// * `self` - Borrowed reference (allows multiple calls per worker)
    /// * `list_name` - Name of the mailing list
    /// * `email_index` - Index/ID of the email to fetch
    ///
    /// # Returns
    ///
    /// * `Ok(())` if email was fetched and saved successfully
    /// * `Err(...)` on failure (e.g., email not available, connection error)
    fn read_email_by_index(&self, list_name: String, email_index: usize) -> crate::Result<()>;
}

/// A group of workers that share the same task list and channel.
///
/// Worker groups enable load balancing: multiple workers process tasks
/// from the same mailing lists concurrently. When a task (list name) is
/// sent to the channel, only one worker receives it.
///
/// # Fields
///
/// * `tasks` - List of mailing list names to process
/// * `workers` - Vector of workers that will process the tasks
/// * `run_mode` - The source type (NNTP, IMAP, etc.) for this group
///
/// # Channel Pattern
///
/// ```text
/// Producer ──► Sender ──► Receiver (cloned)
///                           ├─► Worker 1
///                           ├─► Worker 2
///                           └─► Worker N
/// ```
///
/// Each call to `receiver.recv()` delivers the task to exactly one worker.
pub struct WorkerGroup {
    pub tasks: Vec<String>,
    pub workers: Vec<Box<dyn Worker>>,
    pub run_mode: RunMode,
}

/// Manages ownership of all workers for the program lifetime.
///
/// The `WorkerManager` is responsible for:
/// - Creating workers for each configured run mode
/// - Storing workers in [`WorkerGroup`]s by source type
/// - Providing mutable access to groups for the scheduler
///
/// # Ownership Model
///
/// Workers are created once in [`crate::start()`] and owned by the manager
/// until the program exits. They are then moved to individual threads
/// for execution.
///
/// # Thread Safety
///
/// The manager itself is not thread-safe. It is used only during
/// initialization in the main thread before workers are moved to threads.
pub struct WorkerManager {
    groups: Vec<WorkerGroup>,
}

impl WorkerManager {
    /// Creates a new, empty worker manager.
    ///
    /// # Example
    ///
    /// ```rust
    /// use mlh_archiver::worker::WorkerManager;
    ///
    /// let manager = WorkerManager::new();
    /// assert_eq!(manager.num_groups(), 0);
    /// ```
    pub fn new() -> Self {
        WorkerManager { groups: Vec::new() }
    }

    /// Creates workers for a specific run mode and adds them to a group.
    ///
    /// This method:
    /// 1. Determines the number of workers from `app_config.nthreads`
    /// 2. Retrieves source-specific configuration
    /// 3. Creates worker instances with shared shutdown flag
    /// 4. Stores workers in a [`WorkerGroup`] with their task list
    ///
    /// # Arguments
    ///
    /// * `run_mode` - The source type (NNTP, IMAP, etc.) to create workers for
    /// * `tasks` - List of mailing list names to process
    /// * `app_config` - Application configuration (nthreads, output_dir, etc.)
    /// * `shutdown_flag` - Shared atomic flag for graceful shutdown
    ///
    /// # Panics
    ///
    /// Panics if `get_run_mode_config()` returns `None` for a run mode
    /// that was returned by `get_run_modes()`. This should not happen
    /// in normal operation.
    pub fn create_workers(
        &mut self,
        run_mode: RunMode,
        tasks: Vec<String>,
        app_config: &AppConfig,
        shutdown_flag: Arc<AtomicBool>,
    ) {
        match run_mode {
            RunMode::NNTP => {
                let num_workers = app_config.nthreads.max(1) as usize;
                let mut workers: Vec<Box<dyn Worker>> = Vec::with_capacity(num_workers);

                if let Some(RunModeConfig::NNTP(nntp_config)) =
                    app_config.get_run_mode_config(run_mode)
                {
                    for id in 0..num_workers {
                        let worker = NNTPWorker::new(
                            id as u8,
                            nntp_config.clone(),
                            app_config.output_dir.clone(),
                            shutdown_flag.clone(),
                        );
                        workers.push(Box::new(worker));
                    }

                    self.groups.push(WorkerGroup {
                        tasks,
                        workers,
                        run_mode,
                    });
                }
            }
            RunMode::LocalMbox => {
                unimplemented!()
            }
        }
    }

    /// Returns a mutable reference to all worker groups.
    ///
    /// Used by the scheduler to drain groups and move workers to threads.
    /// After calling this, the manager will be empty.
    ///
    /// # Returns
    ///
    /// Mutable reference to the vector of [`WorkerGroup`]s.
    pub fn get_groups(&mut self) -> &mut Vec<WorkerGroup> {
        &mut self.groups
    }

    /// Returns the number of worker groups.
    ///
    /// Each group corresponds to a configured run mode (NNTP, IMAP, etc.).
    ///
    /// # Returns
    ///
    /// Number of groups managed by this manager.
    pub fn num_groups(&self) -> usize {
        self.groups.len()
    }
}

impl Default for WorkerManager {
    fn default() -> Self {
        Self::new()
    }
}
