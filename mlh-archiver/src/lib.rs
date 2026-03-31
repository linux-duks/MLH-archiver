#![allow(clippy::needless_return)]

pub mod config;
pub mod errors;
pub mod file_utils;
pub mod nntp_source;
pub mod range_inputs;
pub mod scheduler;

pub use errors::Result;

pub fn start(app_config: &mut config::AppConfig) -> crate::errors::Result<()> {
    let groups = nntp_source::nntp_lister::retrieve_lists(app_config)?;

    // Clone groups list before dropping nntp_stream
    let mut temp_config = config::AppConfig {
        nntp: Some(app_config.get_nntp_config()),
        ..app_config.clone()
    };
    let groups = temp_config.get_group_lists(groups)?;

    log::info!("made a selection of {} {:#?}", groups.len(), groups);
    file_utils::check_or_create_folder(app_config.output_dir.clone())?;

    let mut w = scheduler::Scheduler::new(
        app_config.get_nntp_config(),
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
