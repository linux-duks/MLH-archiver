#![allow(clippy::needless_return)]

#[cfg(not(feature = "otel"))]
#[cfg(not(feature = "otel"))]
use env_logger::Env;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use mlh_archiver::config;

#[cfg(feature = "otel")]
use mlh_archiver::otel;
use mlh_archiver::start;
use mlh_archiver::Result;

fn main() -> Result<()> {
    #[cfg(feature = "otel")]
    let _guard = otel::init_tracing_subscriber();

    #[cfg(not(feature = "otel"))]
    {
        let env = Env::default().filter_or("RUST_LOG", "info");
        env_logger::init_from_env(env);
    }

    let mut app_config = match config::read_config() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!();
            eprintln!("Configuration options:");
            eprintln!("  - Config file:  archiver_config.yaml (or similar)");
            eprintln!();
            eprintln!("Run with --help for more information.");
            std::process::exit(1);
        }
    };

    // Setup signal handler for Ctrl+C
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let shutdown_flag_signal = Arc::clone(&shutdown_flag);

    ctrlc::set_handler(move || {
        log::info!("Received shutdown signal (Ctrl+C), stopping workers...");
        shutdown_flag_signal.store(true, Ordering::Relaxed);
    })
    .map_err(|e| std::io::Error::other(format!("Failed to set Ctrl+C handler: {}", e)))?;

    start(&mut app_config, shutdown_flag)
}
