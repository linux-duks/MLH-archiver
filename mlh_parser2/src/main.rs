use env_logger::Env;

use mlh_parser2::Result;
use mlh_parser2::config;
use mlh_parser2::start;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

fn main() -> Result<()> {
    let env = Env::default().filter_or("RUST_LOG", "info");
    env_logger::init_from_env(env);

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
