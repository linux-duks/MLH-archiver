pub mod config;
// pub mod constants;
// pub mod date_parser;
pub mod email_reader;
// pub mod extractors;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use std::thread;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// Logic for parsing a single directory
fn parse_mail_at(
    mail_l: &str,
    input_dir: &Path,
    output_dir: &Path,
    fail_on_error: bool,
) -> Result<()> {
    log::debug!("Processing: {}", mail_l);
    // Parsing logic goes here
    Ok(())
}

pub fn start(cfg: &mut crate::config::AppConfig, shutdown_flag: Arc<AtomicBool>) -> Result<()> {
    log::info!("mlh_parser starting — build: {}", env!("CARGO_PKG_VERSION"));

    // 1. Gather the lists (subfolders)
    let mut lists: Vec<String> = if !cfg.lists_to_parse.is_none()() {
        cfg.lists_to_parse.clone()
    } else {
        fs::read_dir(&cfg.input_dir_path)?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                if entry.file_type().ok()?.is_dir() {
                    entry.file_name().into_string().ok()
                } else {
                    None
                }
            })
            .collect()
    };

    if lists.is_empty() {
        log::warn!("No items found to parse.");
        return Ok(());
    }

    // 2. Multiprocessing logic
    if cfg.nthreads <= 1 {
        // Sequential execution
        for item in lists {
            if shutdown_flag.load(Ordering::Relaxed) {
                break;
            }
            parse_mail_at(
                &item,
                &cfg.input_dir_path,
                &cfg.output_dir_path,
                cfg.fail_on_parsing_error,
            )?;
        }
    } else {
        // Manual Thread Pool logic
        // We wrap the list in a Mutex so threads can safely "pop" items from it.
        let work_queue = Arc::new(Mutex::new(lists));
        let mut handles = Vec::with_capacity(cfg.nthreads);

        for i in 0..cfg.nthreads {
            // Clone references for this specific thread
            let queue = Arc::clone(&work_queue);
            let shutdown = Arc::clone(&shutdown_flag);

            // Capture necessary config values (Strings/Paths need to be owned or Arcs)
            let input_path = cfg.input_dir_path.clone();
            let output_path = cfg.output_dir_path.clone();
            let fail_on_err = cfg.fail_on_parsing_error;

            let handle = thread::spawn(move || {
                loop {
                    // Check for shutdown signal
                    if shutdown.load(Ordering::Relaxed) {
                        break;
                    }

                    // Get the next item from the queue
                    let item = {
                        let mut lock = queue.lock().unwrap();
                        lock.pop()
                    }; // Lock is released here immediately after popping

                    match item {
                        Some(mail_l) => {
                            if let Err(e) =
                                parse_mail_at(&mail_l, &input_path, &output_path, fail_on_err)
                            {
                                log::error!("Thread {} error on {}: {}", i, mail_l, e);
                                if fail_on_err {
                                    shutdown.store(true, Ordering::Relaxed);
                                    break;
                                }
                            }
                        }
                        None => break, // No more work left
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to finish
        for handle in handles {
            let _ = handle.join();
        }
    }

    if shutdown_flag.load(Ordering::Relaxed) {
        log::info!("Process exited via shutdown signal.");
    } else {
        log::info!("Process completed successfully.");
    }

    Ok(())
}
