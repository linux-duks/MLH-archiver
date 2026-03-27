#![allow(clippy::needless_return)]

use env_logger::Env;

use mlh_archiver::Result;
use mlh_archiver::config;
use mlh_archiver::start;

fn main() -> Result<()> {
    let env = Env::default()
        .filter_or("RUST_LOG", "info")
        .write_style_or("MY_LOG_STYLE", "always");

    env_logger::init_from_env(env);

    let mut app_config = match config::read_config() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!();
            eprintln!("Configuration options:");
            eprintln!("  - Command line: -H HOSTNAME or --hostname HOSTNAME");
            eprintln!("  - Environment:  NNTP_HOSTNAME=...");
            eprintln!("  - Config file:  archiver_config.yaml (or similar)");
            eprintln!();
            eprintln!("Run with --help for more information.");
            std::process::exit(1);
        }
    };

    start(&mut app_config)
}
