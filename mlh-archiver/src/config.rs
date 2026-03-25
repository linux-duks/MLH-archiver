use crate::{errors::ConfigError, file_utils, range_inputs};
use clap::{Args, Parser, ValueHint};
use config::Config;
use glob::glob;
use inquire::MultiSelect;
use std::collections::{HashMap, HashSet};

// TODO: test use confique::Config;

#[derive(Debug, Parser, Default, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct Opts {
    // config file location override
    #[arg(short, long, default_value = "archiver_config*", value_hint = ValueHint::FilePath)]
    config_file: String,

    #[clap(flatten)]
    app_config: Option<AppConfig>,
}

#[derive(Debug, Args, Default, serde::Deserialize, serde::Serialize, PartialEq, Eq, Clone)]
pub struct AppConfig {
    /// nntp server domain/ip
    #[arg(short = 'H', long)]
    pub hostname: Option<String>,
    /// nntp serrver port
    #[arg(short, long, default_value = "119")]
    pub port: u16,
    #[arg(short, long, default_value = "./output", value_hint = ValueHint::DirPath)]
    /// where results will be stored
    pub output_dir: String,
    /// Number of worker threads connecting to different lists
    #[arg(short, long, default_value = "1")]
    pub nthreads: u8,
    /// If true, the app will keep running forever. Otherwise, stop after reading all groups
    #[arg(short, long, default_value = "true")]
    pub loop_groups: bool,

    /// List of groups to be read. "ALL" will select all lists available.
    /// Empty value will prompt a selection in the TUI (and save selected values)
    #[arg(long)]
    pub group_lists: Option<Vec<String>>,
    ///  (optional). Read a specific range of articles from the first list provided. Comma separated values, or dash separated ranges, like low-high
    #[arg(long)]
    pub article_range: Option<String>,
}

pub fn read_config() -> Result<AppConfig, anyhow::Error> {
    let opts = Opts::parse();

    let base_config = opts.app_config.unwrap_or_default();

    let defaults = Config::try_from(&base_config).unwrap();

    // TODO: config layering is not working properly
    let config = Config::builder()
        .set_default("port", 119)
        .unwrap()
        .add_source(defaults)
        // env variable config
        .add_source(
            config::Environment::with_prefix("NNTP")
                .try_parsing(true)
                .separator("_"),
        )
        // TODO:  add xdg_home config
        .add_source(
            glob(&opts.config_file)?
                .map(|path| config::File::from(path.unwrap()))
                .collect::<Vec<_>>(),
        );

    let config = config.build().unwrap();

    let app_config: AppConfig = config.try_deserialize()?;

    Ok(app_config)
}

impl AppConfig {
    /// returns the lists ready to use
    ///
    /// Takes lists from config. If none configured, prompt user for selection.
    /// If list was configured, check if selected lists are available in the server
    /// Return only available lists
    pub fn get_group_lists(
        &mut self,
        list_options: Vec<String>,
    ) -> Result<Vec<String>, ConfigError> {
        let mut answer: Vec<String>;
        if self.group_lists.is_none() {
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
                self.group_lists = None;
                return Err(ConfigError::ListSelectionEmpty);
            } else {
                // save selection to a file
                let mut selected_lists = HashMap::new();
                selected_lists.insert("group_lists", answer.clone());

                match file_utils::write_yaml("archiver_config_selected_lists.yml", &selected_lists) {
                    Ok(_) => Ok(()),
                    Err(e) => Err(ConfigError::Io(e)),
                }?;
            }
        } else {
            let mut group_lists = self.group_lists.clone().unwrap();

            // If "ALL" provided, load all lists
            if group_lists[0] == "ALL" {
                log::info!("Configured to fetch all lists");
                log::debug!("Lists selected: {:#?}", list_options);
                answer = list_options;
            } else {
                // or check if lists provided are valid
                group_lists.dedup();
                let item_set: HashSet<_> = list_options.iter().collect();
                group_lists.retain(|item| item_set.contains(item));
                let (valid, invalid): (Vec<_>, Vec<_>) = group_lists
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

        Ok(answer)
    }

    pub fn get_article_range(&self) -> Option<impl Iterator<Item = usize>> {
        match &self.article_range {
            Some(range_text) => {
                // range and multiple lists
                if self.group_lists.as_ref().is_some_and(|x| x.len() > 1) {
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
