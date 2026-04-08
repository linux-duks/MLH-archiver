use crate::archive_writer::ArchiveWriter;
use crate::config::RunModeConfig;
use crate::errors;
use crate::nntp_source::nntp_config::NntpConfig;
use crate::nntp_source::nntp_utils::connect_to_nntp_server;
use crate::worker::Worker;
use nntp::NNTPStream;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, atomic::AtomicBool};
use std::thread::sleep;
use std::time::Duration;

/// NNTP worker that fetches emails from an NNTP server.
///
/// This worker implements the [`Worker`] trait for NNTP sources.
/// It maintains a persistent connection to an NNTP server and fetches emails
/// for specified mailing lists.
///
/// # Fields
///
/// * `id` - Unique worker identifier for logging
/// * `nntp_config` - NNTP server configuration
/// * `nntp_stream` - NNTP connection wrapped in `RefCell` for interior mutability
/// * `base_output_path` - Root directory for storing fetched emails
/// * `needs_reconnection` - Flag indicating if reconnection is needed after error
/// * `shutdown_flag` - Shared atomic flag for graceful shutdown
///
/// # Thread Safety
///
/// The worker uses `RefCell` for the connection because each worker runs in
/// its own thread with exclusive access. The `shutdown_flag` is shared via
/// `Arc<AtomicBool>` for lock-free signaling.
///
/// # Progress Tracking
///
/// Progress is managed by [`ArchiveWriter`]:
/// - `__progress.yaml` - Last successfully fetched article ID
/// - `__errors.csv` - Log of unavailable articles
pub struct NNTPWorker {
    id: u8,
    nntp_config: NntpConfig,
    nntp_stream: RefCell<NNTPStream>,
    base_output_path: String,
    needs_reconnection: Cell<bool>,
    shutdown_flag: Arc<AtomicBool>,
}

impl NNTPWorker {
    /// Creates a new NNTP worker and establishes connection to the server.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique worker identifier for logging
    /// * `nntp_config` - NNTP server configuration (hostname, port, etc.)
    /// * `base_output_path` - Root directory for storing fetched emails
    /// * `shutdown_flag` - Shared atomic flag for graceful shutdown
    ///
    /// # Panics
    ///
    /// Panics if connection to the NNTP server fails. The caller should
    /// ensure the server is available before creating workers.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use std::sync::{Arc, atomic::AtomicBool};
    /// use mlh_archiver::nntp_source::{nntp_config::NntpConfig, nntp_worker::NNTPWorker};
    ///
    /// let config = NntpConfig {
    ///     hostname: "nntp://nntp.example.com".to_string(),
    ///     port: Some(119),
    ///     ..NntpConfig::default()
    /// };
    /// let shutdown_flag = Arc::new(AtomicBool::new(false));
    /// let worker = NNTPWorker::new(0, config, "./output".to_string(), shutdown_flag);
    /// ```
    pub fn new(
        id: u8,
        nntp_config: NntpConfig,
        base_output_path: String,
        shutdown_flag: Arc<AtomicBool>,
    ) -> NNTPWorker {
        let nntp_stream = connect_to_nntp_server(
            &nntp_config.hostname,
            nntp_config.port,
            nntp_config.username.clone(),
            nntp_config.password.clone(),
        )
        .expect("NNTPWorker should have connected to the server");

        NNTPWorker {
            id,
            nntp_config,
            base_output_path,
            nntp_stream: RefCell::new(nntp_stream),
            needs_reconnection: Cell::new(false),
            shutdown_flag,
        }
    }
}

impl Worker for NNTPWorker {
    fn consumme_list(
        self: Box<Self>,
        receiver: crossbeam_channel::Receiver<String>,
    ) -> crate::Result<()> {
        log::info!("W{}: started consuming tasks", self.id);
        loop {
            // Check shutdown flag at start of each iteration
            if self.shutdown_flag.load(Ordering::Relaxed) {
                log::info!("W{}: Shutdown requested, exiting...", self.id);
                return Ok(());
            }

            // check if reconnection is needed before trying to connect
            if self.needs_reconnection.get() {
                log::debug!("W{}: will attempt a reconnection soon", self.id);
                // wait a minute before trying to reconnect, checking shutdown flag
                let reconnect_wait = Duration::from_secs(60);
                let check_interval = Duration::from_secs(1);
                let mut elapsed = Duration::ZERO;
                while elapsed < reconnect_wait {
                    if self.shutdown_flag.load(Ordering::Relaxed) {
                        log::info!("W{}: Shutdown requested during reconnection wait", self.id);
                        return Ok(());
                    }
                    std::thread::sleep(check_interval);
                    elapsed += check_interval;
                }

                log::info!("W{}: will attempt a reconnection", self.id);
                match self.nntp_stream.borrow_mut().re_connect() {
                    Ok(_) => self.needs_reconnection.set(false),
                    Err(e) => {
                        log::error!(
                            "W{}: attempted reconnection and failed with error {e}",
                            self.id
                        );
                        return Err(errors::Error::NNTP(e));
                    }
                }
            }

            log::info!("W{}: Reading new group from channel", self.id);
            // recv() blocks until a message is available or channel is closed
            // When channel is closed AND empty, returns RecvError
            let list_name = match receiver.recv() {
                Ok(name) => name,
                Err(crossbeam_channel::RecvError) => {
                    log::info!("W{}: Channel closed and empty, worker exiting", self.id);
                    return Ok(());
                }
            };

            // create ArchiveWriter instance for the new list
            let writer = ArchiveWriter::new(
                Path::new(&self.base_output_path),
                &list_name,
                RunModeConfig::NNTP(self.nntp_config.clone()),
            );

            match self.handle_group(list_name.clone(), &writer) {
                Ok(return_status) => {
                    log::info!("W{}: completed a task with: {return_status}", self.id);
                }
                Err(err) => {
                    if nntp::errors::check_network_error(&err) {
                        log::warn!(
                            "W{}: failed with a network error while reading {list_name}. Error {}",
                            self.id,
                            &err
                        );
                        // if connection error was returned, sleep a bit, checking shutdown
                        let sleep_duration = Duration::from_secs(10);
                        let check_interval = Duration::from_secs(1);
                        let mut elapsed = Duration::ZERO;
                        while elapsed < sleep_duration {
                            if self.shutdown_flag.load(Ordering::Relaxed) {
                                log::info!("W{}: Shutdown requested during error wait", self.id);
                                return Ok(());
                            }
                            std::thread::sleep(check_interval);
                            elapsed += check_interval;
                        }
                    } else {
                        log::error!(
                            "W{}: failed while processing {list_name} with error {}",
                            self.id,
                            &err
                        );
                    }

                    // when an error happens, force a reconnection
                    self.needs_reconnection.set(true);
                    // attempt to close connection
                    match self.nntp_stream.borrow_mut().quit() {
                        Ok(_) => {
                            log::debug!("W{}: Connection closed successfully", self.id);
                        }
                        Err(err) => {
                            log::warn!(
                                "W{}: Failed when closing connection with error {err}. Waiting before triggering a reconnection",
                                self.id
                            );
                            std::thread::sleep(Duration::from_secs(5));
                        }
                    }
                }
            };
            // interval between tasks
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    fn read_email_by_index(&self, list_name: String, email_index: usize) -> crate::Result<()> {
        let writer = ArchiveWriter::new(
            Path::new(&self.base_output_path),
            &list_name,
            RunModeConfig::NNTP(self.nntp_config.clone()),
        );

        log::info!("W{}: Checking group : {list_name}", self.id);

        // Verify group exists - borrow dropped immediately after
        self.nntp_stream.borrow_mut().group(&list_name)?;

        log::info!(
            "W{}: Will start collecting mails from range for group {list_name}",
            self.id
        );
        // Borrow is dropped, safe to call read_new_mails
        self.read_new_mails(list_name.clone(), email_index, email_index, &writer)?;
        Ok(())
    }
}

impl NNTPWorker {
    /// Processes a mailing list and fetches all new emails.
    ///
    /// This method:
    /// 1. Reads the last fetched article ID from the archive writer
    /// 2. Queries the NNTP server for the current high water mark
    /// 3. Fetches all articles between last ID and high water mark
    /// 4. Updates progress after each successful fetch via the writer
    ///
    /// # Arguments
    ///
    /// * `list_name` - Name of the mailing list to process
    /// * `writer` - ArchiveWriter for progress tracking and storage
    ///
    /// # Returns
    ///
    /// * `Ok(NNTPWorkerGroupResult::Ok)` - Successfully fetched emails
    /// * `Ok(NNTPWorkerGroupResult::NoNews)` - No new emails available
    /// * `Err(...)` - Connection or protocol error
    ///
    /// # Side Effects
    ///
    /// - Creates/updates `__progress.yaml` YAML file via writer
    /// - Writes fetched emails as `.eml` files via writer
    /// - Logs unavailable articles to `__errors.csv` file via writer
    pub fn handle_group(
        &self,
        list_name: String,
        writer: &ArchiveWriter,
    ) -> nntp::Result<NNTPWorkerGroupResult> {
        let last_article_number = writer.last_processed_id();

        if last_article_number == 0 {
            log::info!("W{}: Reading list {list_name} from mail 0", self.id);
        }

        log::info!(
            "W{}: Checking group : {list_name}. Local max ID: {last_article_number}",
            self.id
        );

        // Get group info - borrow is dropped at end of this scope block
        let should_read_info = {
            let group = self.nntp_stream.borrow_mut().group(&list_name)?;
            log::info!(
                "W{}: Remote max for {} is {}, local is {}",
                self.id,
                list_name,
                group.high,
                last_article_number
            );
            if last_article_number < group.high as usize {
                Some((group.low as usize, group.high as usize))
            } else {
                None
            }
        };

        if let Some((low, high)) = should_read_info {
            log::info!("W{}: Reading emails for group : {list_name}.", self.id);
            // Borrow is already dropped, safe to call read_new_mails
            match self.read_new_mails(
                list_name.clone(),
                last_article_number.max(low),
                high,
                writer,
            ) {
                Ok(num_emails_read) => {
                    return Ok(NNTPWorkerGroupResult::Ok(list_name, num_emails_read));
                }
                Err(e) => {
                    log::error!("W{}: Failed reading new mails: {e}", self.id);
                    return Err(e);
                }
            }
        } else {
            log::info!(
                "W{}: Checking group : {list_name}. Local max ID: {last_article_number}",
                self.id
            );
            return Ok(NNTPWorkerGroupResult::NoNews(list_name));
        }
    }

    /// Fetches emails from a mailing list within an article ID range.
    ///
    /// This is the core email fetching method. It:
    /// 1. Iterates through article IDs from `low` to `high`
    /// 2. Checks shutdown flag before each fetch
    /// 3. Retrieves raw article content with retry logic
    /// 4. Writes emails to `{output_dir}/{list_name}/{id}.eml` via writer
    /// 5. Updates `__progress.yaml` via writer after each success
    ///
    /// # Arguments
    ///
    /// * `list_name` - Name of the mailing list
    /// * `low` - Starting article ID (inclusive)
    /// * `high` - Ending article ID (inclusive)
    /// * `writer` - ArchiveWriter for storage and progress
    ///
    /// # Returns
    ///
    /// * `Ok(usize)` - Number of emails successfully fetched
    /// * `Err(...)` - Connection or protocol error
    ///
    /// # Shutdown Behavior
    ///
    /// If shutdown is requested during fetching, returns the count of
    /// emails fetched so far without error.
    fn read_new_mails(
        &self,
        list_name: String,
        low: usize,
        high: usize,
        writer: &ArchiveWriter,
    ) -> nntp::Result<usize> {
        let mut num_emails_read: usize = 0;
        for current_mail in low..=high {
            // Check shutdown flag during email fetching
            if self.shutdown_flag.load(Ordering::Relaxed) {
                log::info!(
                    "W{}: Shutdown requested while reading {list_name} at {current_mail}/{high}",
                    self.id
                );
                return Ok(num_emails_read);
            }

            match self.get_raw_article_by_number_retryable(current_mail as isize, 3) {
                Ok(raw_article) => {
                    writer
                        .archive_email(current_mail, &raw_article)
                        .map_err(|e| nntp::NNTPError::Io(std::io::Error::other(e)))?;
                    num_emails_read += 1;
                }
                Err(e) => match e {
                    nntp::NNTPError::ArticleUnavailable => {
                        writer.log_error(current_mail, &e.to_string());
                        log::warn!("W{}: Email with number {current_mail} unavailable", self.id);
                    }
                    _ => return Err(e),
                },
            }

            log::info!(
                "W{}: {list_name} {}/{} ({:.2}%)",
                self.id,
                current_mail,
                high,
                (current_mail as f64 / high as f64 * 100.0)
            );
        }
        Ok(num_emails_read)
    }

    fn get_raw_article_by_number_retryable(
        &self,
        mail_num: isize,
        max_retries: usize,
    ) -> nntp::Result<Vec<String>> {
        let mut attempts = 0;
        let retry_delay_ms = 600;
        loop {
            match self
                .nntp_stream
                .borrow_mut()
                .raw_article_by_number(mail_num)
            {
                Ok(raw_article) => {
                    return Ok(raw_article);
                }
                Err(e) => {
                    log::warn!(
                        "W{}: Failed reading article '{}' from '{}'",
                        self.id,
                        mail_num,
                        self.nntp_config.server_address()
                    );
                    attempts += 1;
                    if attempts > max_retries {
                        // Return the last error after max retries
                        return Err(e);
                    }
                    log::warn!(
                        "W{}: Retrying in {}ms...",
                        self.id,
                        (retry_delay_ms * (attempts + 1))
                    );
                    sleep(Duration::from_millis(
                        (retry_delay_ms * (attempts + 1)) as u64,
                    ));
                }
            }
        }
    }
}

/// Result of processing a mailing list.
///
/// Returned by [`NNTPWorker::handle_group()`] to indicate the outcome
/// of list processing.
///
/// # Variants
///
/// * `Ok(String, usize)` - Successfully fetched emails. Contains list name and count.
/// * `NoNews(String)` - No new emails available. Contains list name.
pub enum NNTPWorkerGroupResult {
    Ok(String, usize),
    NoNews(String),
    // Failed(String),
}

impl fmt::Display for NNTPWorkerGroupResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self {
            NNTPWorkerGroupResult::Ok(list_name, num_emails) => {
                write!(f, "Collected {num_emails} new e-mails from {:?}", list_name)
            }
            NNTPWorkerGroupResult::NoNews(list_name) => {
                write!(f, "No New e-mails from {:?}", list_name)
            }
        }
    }
}
