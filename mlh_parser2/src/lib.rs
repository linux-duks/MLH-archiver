pub mod config;
pub mod constants;
pub mod date_parser;
pub mod email_reader;
pub mod email_file_reader;
pub mod errors;
pub mod extractors;
pub mod parser;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// Batch limits to stay well under Arrow's i32 string-offset ceiling (~2.1 GB)
pub const BATCH_MAX_RECORDS: usize = 50_000;
pub const BATCH_MAX_RAW_BYTES: usize = 400 * 1024 * 1024; // 400 MB

fn parse_mail_at(
    mail_l: &str,
    input_dir: &Path,
    output_dir: &Path,
    fail_on_error: bool,
) -> Result<()> {
    log::debug!("Processing: {}", mail_l);
    parser::parse_mail_at(
        mail_l,
        input_dir,
        output_dir,
        fail_on_error,
        BATCH_MAX_RECORDS,
        BATCH_MAX_RAW_BYTES,
    )
    .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))
}

pub fn start(cfg: &mut crate::config::AppConfig, shutdown_flag: Arc<AtomicBool>) -> Result<()> {
    let lists: Vec<String> = if let Some(ref specified_lists) = cfg.lists_to_parse {
        specified_lists.clone()
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

    let input_path = PathBuf::from(&cfg.input_dir_path);
    let output_path = PathBuf::from(&cfg.output_dir_path);

    if cfg.nthreads <= 1 {
        for item in lists {
            if shutdown_flag.load(Ordering::Relaxed) {
                break;
            }
            parse_mail_at(&item, &input_path, &output_path, cfg.fail_on_parsing_error)?;
        }
    } else {
        let work_queue = Arc::new(Mutex::new(lists));
        let mut handles = Vec::with_capacity(cfg.nthreads as usize);

        for i in 0..cfg.nthreads {
            log::debug!("Starting thread {i}");

            let queue = Arc::clone(&work_queue);
            let shutdown = Arc::clone(&shutdown_flag);
            let input = input_path.clone();
            let output = output_path.clone();
            let fail_on_err = cfg.fail_on_parsing_error;

            let handle = thread::spawn(move || {
                loop {
                    if shutdown.load(Ordering::Relaxed) {
                        break;
                    }

                    let item = {
                        let mut lock = queue.lock().unwrap();
                        lock.pop()
                    };

                    match item {
                        Some(mail_l) => {
                            log::debug!("Preparing to read {mail_l}");

                            if let Err(e) = parse_mail_at(&mail_l, &input, &output, fail_on_err) {
                                log::error!("Thread {} error on {}: {}", i, mail_l, e);
                                if fail_on_err {
                                    shutdown.store(true, Ordering::Relaxed);
                                    break;
                                }
                            }
                        }
                        None => break,
                    }
                }
            });
            handles.push(handle);
        }

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
