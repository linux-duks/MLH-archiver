use crate::nntp_source::nntp_config;
use crate::{errors::ConfigError, file_utils, range_inputs};
use clap::{Parser, ValueHint};
use config::Config;
use glob::glob;
use inquire::MultiSelect;
use std::collections::{HashMap, HashSet};

/// Main application configuration
///
/// Global settings (nthreads, output_dir, loop_groups) are at the top level.
/// Source-specific settings are nested, and private (e.g., nntp, imap, local, mbox).
/// Their values should be accessed using the RunModes ENUM
#[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq, Clone)]
pub struct AppConfig {
    /// Number of worker threads connecting to different lists
    #[serde(default = "default_nthreads")]
    pub nthreads: u8,

    /// Output directory where results will be stored
    #[serde(default = "default_output_dir")]
    pub output_dir: String,

    /// If true, the app will keep running forever. Otherwise, stop after reading all groups
    #[serde(default = "default_loop_groups")]
    pub loop_groups: bool,

    /// NNTP-specific configuration
    pub nntp: Option<nntp_config::NntpConfig>,
}

impl AppConfig {
    // Other sources should be implemented here too
    pub fn get_run_modes(&self) -> Vec<RunModes> {
        let mut run_modes: Vec<RunModes> = vec![];
        if let Some(nntp) = &self.nntp {
            run_modes.push(RunModes::NNTP(nntp.clone()));
        }
        return run_modes;
    }

    // Other sources should be implemented here too
    pub fn get_list_selection(&self, run_mode: RunModes) -> Option<Vec<String>> {
        match run_mode {
            RunModes::NNTP(config) => return config.group_lists,
            RunModes::LocalMbox => unimplemented!(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunModes {
    NNTP(nntp_config::NntpConfig),
    LocalMbox,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            nthreads: default_nthreads(),
            output_dir: default_output_dir(),
            loop_groups: default_loop_groups(),
            nntp: None,
        }
    }
}

fn default_nthreads() -> u8 {
    1
}

fn default_output_dir() -> String {
    "./output".to_string()
}

fn default_loop_groups() -> bool {
    true
}

#[derive(Debug, Parser, Default)]
pub struct Opts {
    /// config file location override
    #[arg(short, long, default_value = "archiver_config*", value_hint = ValueHint::FilePath)]
    pub config_file: String,
}

pub fn read_config() -> Result<AppConfig, ConfigError> {
    let opts = Opts::parse();

    // Collect config files from glob pattern
    let config_files: Vec<_> = glob(&opts.config_file)
        .map_err(|e| {
            ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Invalid config file glob pattern '{}': {}",
                    opts.config_file, e
                ),
            ))
        })?
        .filter_map(|path_result| match path_result {
            Ok(path) => {
                log::debug!("Found config file: {}", path.display());
                Some(config::File::from(path))
            }
            Err(e) => {
                log::warn!("Error reading config file path: {}", e);
                None
            }
        })
        .collect();

    if config_files.is_empty() {
        log::warn!(
            "No config files found matching pattern: {}",
            opts.config_file
        );
    }

    // Build config with layered sources
    let mut config_builder = Config::builder();

    // Add each config file (highest priority)
    for config_file in config_files {
        config_builder = config_builder.add_source(config_file);
    }

    let config = config_builder.build().map_err(|e| {
        ConfigError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Failed to build config: {}", e),
        ))
    })?;

    log::debug!("Config built: {:?}", config);

    let app_config: AppConfig = config.try_deserialize().map_err(|e| {
        ConfigError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Failed to deserialize config: {}", e),
        ))
    })?;

    log::debug!(
        "Deserialized config: hostname={:?}",
        app_config.nntp.as_ref().map(|n| &n.hostname)
    );

    // Validate hostname is provided
    if let Some(ref nntp) = app_config.nntp {
        nntp.validate()?;
    } else {
        return Err(ConfigError::MissingHostname);
    }

    Ok(app_config)
}

impl AppConfig {
    /// Returns the lists ready to use
    ///
    /// Takes lists from config. If none configured, prompt user for selection.
    /// If list was configured, check if selected lists are available in the server
    /// Return only available lists
    pub fn get_group_lists(
        &mut self,
        list_options: Vec<String>,
        run_mode: RunModes,
    ) -> Result<Vec<String>, ConfigError> {
        // TODO: needs generic form
        let mut nntp = self.nntp.clone().unwrap();

        let mut answer: Vec<String>;
        let group_lists = self.get_list_selection(run_mode);
        match group_lists {
            None => {
                log::info!("No group_lists defined");

                // list of options provides, with "ALL" as first
                let mut select_options = vec!["ALL".to_string()];
                select_options.extend(list_options.clone());

                answer = MultiSelect::new("No groups selected. Select them now:", select_options)
                    .prompt()
                    .unwrap_or_else(|_| std::process::exit(0));

                if answer[0] == "ALL" {
                    log::info!("All lists selected");
                    log::debug!("Lists selected: {:#?}", list_options);
                    answer = list_options;
                }

                if answer.is_empty() {
                    log::info!("empty selection");
                    nntp.group_lists = None;
                    return Err(ConfigError::ListSelectionEmpty);
                } else {
                    // save selection to a file
                    let mut selected_lists = HashMap::new();
                    selected_lists.insert("group_lists", answer.clone());

                    match file_utils::write_yaml(
                        "archiver_config_selected_lists.yml",
                        &selected_lists,
                    ) {
                        Ok(_) => Ok(()),
                        Err(e) => Err(ConfigError::Io(e)),
                    }?;
                }
            }
            Some(_) => {
                let mut user_selection = group_lists.expect("is none was validated");
                // If "ALL" provided, load all lists
                if user_selection[0] == "ALL" {
                    log::info!("Configured to fetch all lists");
                    log::debug!("Lists selected: {:#?}", list_options);
                    answer = list_options;
                } else {
                    // or check if lists provided are valid
                    user_selection.dedup();
                    let item_set: HashSet<_> = list_options.iter().collect();
                    user_selection.retain(|item| item_set.contains(item));
                    let (valid, invalid): (Vec<_>, Vec<_>) = user_selection
                        .into_iter()
                        .partition(|item| item_set.contains(item));

                    if valid.is_empty() {
                        return Err(ConfigError::AllListsUnavailable);
                    }
                    if !invalid.is_empty() {
                        log::warn!(
                            "Some lists are unavailable: {}",
                            ConfigError::ConfiguredListsNotAvailable {
                                unavailable_lists: invalid
                            }
                        );
                    }
                    answer = valid;
                }
            }
        }

        Ok(answer)
    }

    pub fn get_article_range(&self) -> Option<impl Iterator<Item = usize>> {
        let nntp = self.nntp.as_ref()?;

        match &nntp.article_range {
            Some(range_text) => {
                // range and multiple lists
                if nntp.group_lists.as_ref().is_some_and(|x| x.len() > 1) {
                    log::warn!(
                        "article_range used with group_lists with more than one list. This is likely an error"
                    );
                }
                return match range_inputs::parse_sequence(range_text) {
                    Ok(range) => Some(range),
                    Err(e) => {
                        log::error!("Invalid article_range input: {e}");
                        None
                    }
                };
            }
            None => {
                return None;
            }
        }
    }
}
