#![allow(clippy::needless_return)]

pub mod config;
pub mod errors;
pub mod file_utils;
pub mod nntp_source;
pub mod range_inputs;
pub mod scheduler;
pub mod worker;

pub use errors::Result;

use config::RunModes;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use worker::WorkerManager;

pub fn start(
    app_config: &mut config::AppConfig,
    shutdown_flag: Arc<AtomicBool>,
) -> crate::errors::Result<()> {
    let run_modes = app_config.get_run_modes();

    // Create worker manager to own all workers
    let mut worker = WorkerManager::new();

    // Create workers for each run mode
    for mode in run_modes {
        match &mode {
            RunModes::NNTP(nntp_config) => {
                // Get available lists in endpoint
                let groups = nntp_source::nntp_lister::retrieve_lists(nntp_config.clone())?;
                // Filter with selected lists by user
                let groups = app_config.get_group_lists(groups, mode.clone())?;

                log::info!("made a selection of {} {:#?}", groups.len(), groups);

                // Create workers for this run mode
                worker.create_workers(mode.clone(), groups, app_config, shutdown_flag.clone());
            }
            RunModes::LocalMbox => {
                unimplemented!()
            }
        }
    }

    file_utils::check_or_create_folder(app_config.output_dir.clone())?;

    let mut scheduler = scheduler::Scheduler::new(
        app_config,
        app_config.output_dir.clone(),
        app_config.nthreads,
        app_config.loop_groups,
        worker.get_groups(),
    );

    match app_config.get_article_range() {
        Some(_range) => unimplemented!(),
        // Some(range) => scheduler.run_range(range),
        None => scheduler.run(),
    }?;

    Ok(())
}
