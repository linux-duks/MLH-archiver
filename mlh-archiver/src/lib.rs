#![allow(clippy::needless_return)]

pub mod config;
pub mod errors;
pub mod file_utils;
pub mod range_inputs;
pub mod scheduler;
pub mod worker;

pub use errors::Result;

pub fn start(app_config: &mut config::AppConfig) -> crate::errors::Result<()> {
    // Get NNTP config (validates hostname is present)
    let nntp_config = app_config.get_nntp_config();
    nntp_config.validate()?;

    // Connect to NNTP server to get list of groups
    let mut nntp_stream = worker::connect_to_nntp(nntp_config.server_address())?;

    let list_options = nntp_stream.list()?;

    // Clone groups list before dropping nntp_stream
    let mut temp_config = config::AppConfig {
        nntp: Some(nntp_config.clone()),
        ..app_config.clone()
    };
    let groups =
        temp_config.get_group_lists(list_options.iter().map(|an| an.clone().name).collect())?;

    // close initial connection to nntp server
    let _ = nntp_stream.quit();

    log::info!("made a selection of {} {:#?}", groups.len(), groups);
    file_utils::check_or_create_folder(app_config.output_dir.clone())?;

    let mut w = scheduler::Scheduler::new(
        nntp_config,
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
