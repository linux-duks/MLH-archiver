use crate::nntp_source::nntp_config;
use crate::{errors::ConfigError, file_utils};
use clap::{Parser, ValueHint};
use config::Config;
use glob::glob;
use inquire::MultiSelect;
use std::collections::{HashMap, HashSet};

/// Main application configuration
///
/// Global settings (nthreads, output_dir, loop_groups) are at the top level.
/// Source-specific settings are nested, and private (e.g., nntp, imap, local, mbox).
/// Their values should be accessed using the [`RunMode`] ENUM.
///
/// # Example
///
/// ```yaml
/// nthreads: 2
/// output_dir: "./output"
/// loop_groups: true
///
/// nntp:
///   hostname: "nntp.example.com"
///   port: 119
///   group_lists: ["list1", "list2"]
/// ```
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

/// Represents a source type that can be processed by the archiver.
///
/// Each variant corresponds to a different email source implementation.
/// New sources should add a variant here and implement the corresponding
/// configuration and worker logic.
///
/// # Variants
///
/// * `NNTP` - Network News Transfer Protocol source
/// * `LocalMbox` - Local mbox file source (not yet implemented)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    NNTP,
    LocalMbox,
}

/// Configuration for a specific [`RunMode`].
///
/// This enum wraps the source-specific configuration structs.
/// Use [`AppConfig::get_run_mode_config()`] to retrieve the config for a given mode.
///
/// # Variants
///
/// * `NNTP(nntp_config::NntpConfig)` - NNTP server configuration
/// * `LocalMbox` - Local mbox configuration (not yet implemented)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunModeConfig {
    NNTP(nntp_config::NntpConfig),
    LocalMbox,
}

/// Here are implemented the functions for config related to the RunMode and its configs
impl AppConfig {
    /// Retrieves the configuration for a specific run mode.
    ///
    /// # Arguments
    ///
    /// * `run_mode` - The run mode to get configuration for
    ///
    /// # Returns
    ///
    /// * `Some(RunModeConfig)` if the configuration exists for the given mode
    /// * `None` if the configuration is not set (e.g., `nntp` field is `None`)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use mlh_archiver::config::{AppConfig, RunMode};
    ///
    /// let config = AppConfig::default();
    /// if let Some(mode_config) = config.get_run_mode_config(RunMode::NNTP) {
    ///     // Access NNTP configuration
    /// }
    /// ```
    pub fn get_run_mode_config(&self, run_mode: RunMode) -> Option<RunModeConfig> {
        match run_mode {
            RunMode::NNTP => Some(RunModeConfig::NNTP(self.nntp.clone()?)),
            RunMode::LocalMbox => Some(RunModeConfig::LocalMbox),
        }
    }

    /// Retrieves the article range selection text for a specific run mode.
    ///
    /// The range text is a string like `"1,5,10-15"` that specifies which
    /// articles to fetch. It should be parsed by [`crate::range_inputs::parse_sequence()`]
    /// into a lazy iterator.
    ///
    /// # Arguments
    ///
    /// * `run_mode` - The run mode to get range selection for
    ///
    /// # Returns
    ///
    /// * `Some(String)` containing the range text if configured
    /// * `None` if no range is configured or run mode has no config
    pub fn get_range_selection_text(&self, run_mode: RunMode) -> Option<String> {
        match self.get_run_mode_config(run_mode)? {
            RunModeConfig::NNTP(nntp_config) => nntp_config.article_range,
            RunModeConfig::LocalMbox => unimplemented!(),
        }
    }

    /// Returns a list of active run modes based on configuration.
    ///
    /// Checks which source configurations are present (e.g., `nntp.is_some()`)
    /// and returns the corresponding run modes.
    ///
    /// # Returns
    ///
    /// A vector of [`RunMode`] variants that have configuration.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use mlh_archiver::config::AppConfig;
    ///
    /// let config = AppConfig::default();
    /// let modes = config.get_run_modes();
    /// // modes will be empty since nntp is None by default
    /// ```
    pub fn get_run_modes(&self) -> Vec<RunMode> {
        let mut run_modes: Vec<RunMode> = vec![];
        if self.nntp.is_some() {
            run_modes.push(RunMode::NNTP);
        }
        return run_modes;
    }

    /// Other sources should be implemented here too
    fn get_list_selection(&self, run_mode: RunMode) -> Option<Vec<String>> {
        match run_mode {
            RunMode::NNTP => {
                match &self.nntp {
                    Some(nntp_config) => {
                        return nntp_config.group_lists.clone();
                    }
                    None => return None,
                };
            }
            RunMode::LocalMbox => unimplemented!(),
        }
    }
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

/// Command-line options for the archiver.
///
/// Parsed using `clap` from command-line arguments.
#[derive(Debug, Parser, Default)]
pub struct Opts {
    /// config file location override
    #[arg(short, long, default_value = "archiver_config*", value_hint = ValueHint::FilePath)]
    pub config_file: String,
}

/// Reads and validates configuration from files.
///
/// Searches for configuration files matching the glob pattern `archiver_config*`
/// (or custom pattern via `--config-file`). Supports JSON, YAML, and TOML formats.
///
/// Configuration is layered:
/// 1. Default values
/// 2. Config files found by glob pattern (later files override earlier)
///
/// # Returns
///
/// * `Ok(AppConfig)` if configuration is valid
/// * `Err(ConfigError)` if:
///   - Glob pattern is invalid
///   - Config files cannot be read
///   - Deserialization fails
///
/// # Example
///
/// ```rust,no_run
/// use mlh_archiver::config::read_config;
///
/// let config = read_config().expect("Failed to read config");
/// ```
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

    // return Err(ConfigError::MissingHostname);

    Ok(app_config)
}

impl AppConfig {
    /// Retrieves and validates the list of mailing groups to archive.
    ///
    /// This method handles three scenarios:
    /// 1. **No configuration**: Prompts user via TUI to select lists interactively
    /// 2. **"ALL" configured**: Returns all available lists from the server
    /// 3. **Specific lists configured**: Validates against available lists and returns
    ///    only those that exist on the server
    ///
    /// If lists are selected interactively, saves the selection to
    /// `archiver_config_selected_lists.yml` for future runs.
    ///
    /// # Arguments
    ///
    /// * `list_options` - List of available group names from the server
    /// * `run_mode` - The run mode to get list selection for
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<String>)` - Validated list of group names to archive
    /// * `Err(ConfigError::ListSelectionEmpty)` - User selected no lists
    /// * `Err(ConfigError::AllListsUnavailable)` - Configured lists don't exist on server
    /// * `Err(ConfigError::ConfiguredListsNotAvailable)` - Some configured lists invalid
    ///
    /// # Side Effects
    ///
    /// Writes selection to `archiver_config_selected_lists.yml` if user selects interactively.
    pub fn get_group_lists(
        &mut self,
        list_options: Vec<String>,
        run_mode: RunMode,
    ) -> Result<Vec<String>, ConfigError> {
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
                    // group_lists = None;
                    // update the config with the selection
                    // self.set_list_selection(run_mode, group_lists);
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
}
