use crate::errors::ConfigError;
use clap::{Parser, ValueHint};
use config::Config;
use glob::glob;

#[derive(Debug, Parser, Default)]
pub struct Opts {
    #[arg(short, long, default_value = "parser_config*", value_hint = ValueHint::FilePath)]
    pub config_file: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq, Clone)]
pub struct AppConfig {
    pub nthreads: u8,
    pub input_dir_path: String,
    pub output_dir_path: String,
    pub fail_on_parsing_error: bool,
    pub lists_to_parse: Option<Vec<String>>,
}

pub fn read_config() -> Result<AppConfig, ConfigError> {
    let opts = Opts::parse();

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

    let mut config_builder = Config::builder();

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

    Ok(app_config)
}
