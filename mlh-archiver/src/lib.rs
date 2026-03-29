#![allow(clippy::needless_return)]

pub mod config;
pub mod errors;
pub mod file_utils;
pub mod range_inputs;
pub mod scheduler;
pub mod worker;

pub use errors::Result;

pub fn start(app_config: &mut config::AppConfig) -> crate::errors::Result<()> {
    // Validate hostname before connecting
    let hostname = app_config
        .hostname
        .as_ref()
        .ok_or(crate::errors::ConfigError::MissingHostname)?;

    let mut nntp_stream = worker::connect_to_nntp(format!("{}:{}", hostname, app_config.port))?;

    let list_options = nntp_stream.list()?;
    let groups =
        app_config.get_group_lists(list_options.iter().map(|an| an.clone().name).collect())?;

    // close initial connection to nntp server
    let _ = nntp_stream.quit();

    log::info!("made a selection of {} {:#?}", groups.len(), groups);
    file_utils::check_or_create_folder(app_config.output_dir.clone())?;

    let mut w = scheduler::Scheduler::new(
        app_config.hostname.clone().unwrap(),
        app_config.port,
        app_config.output_dir.clone(),
        app_config.nthreads,
        app_config.loop_groups,
        groups,
    );
    match app_config.get_article_range() {
        Some(range) => w.run_range(range),
        None => w.run(),
    }?;

    Ok(())
}
