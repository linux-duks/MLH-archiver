//! check_nntp - Interactive NNTP mailing list browser
//!
//! This tool allows you to interactively browse available NNTP mailing lists,
//! preview article ranges, and generate configuration snippets for the MLH Archiver.
//!
//! # Usage
//!
//! ```bash
//! # Interactive mode (prompts for server URL)
//! cargo run --package check_nntp
//!
//! # With CLI arguments
//! cargo run --package check_nntp -- -s nntp://nntp.example.com
//!
//! # With TLS
//! cargo run --package check_nntp -- -s nntps://nntp.example.com
//!
//! # Custom port
//! cargo run --package check_nntp -- -s nntp://nntp.example.com:8119
//!
//! # Export configuration after browsing
//! cargo run --package check_nntp -- -s nntp://nntp.example.com --export-config
//! ```

use clap::Parser;
use inquire::{Confirm, MultiSelect, Select, Text};
use mlh_archiver::nntp_source::{
    connect_to_nntp_server, nntp_utils::server_address, retrieve_lists_with_connection,
};
use std::env;

/// Parsed server configuration from a URL.
struct ServerConfig {
    hostname: String,
    port: Option<u16>,
    use_tls: bool,
}

/// Parses an NNTP server URL into a [`ServerConfig`].
///
/// # Supported formats
///
/// - `nntp://hostname` → port 119, plaintext
/// - `nntps://hostname` → port 563, TLS
/// - `nntp://hostname:port` → custom port, plaintext
/// - `nntps://hostname:port` → custom port, TLS
///
/// # Examples
///
/// ```
/// let cfg = parse_server_url("nntp://example.com").unwrap();
/// assert_eq!(cfg.hostname, "nntp://example.com");
/// assert_eq!(cfg.port, None);
/// assert!(!cfg.use_tls);
///
/// let cfg = parse_server_url("nntps://example.com").unwrap();
/// assert_eq!(cfg.hostname, "nntps://example.com");
/// assert_eq!(cfg.port, None);
/// assert!(cfg.use_tls);
///
/// let cfg = parse_server_url("nntp://example.com:8119").unwrap();
/// assert_eq!(cfg.hostname, "nntp://example.com");
/// assert_eq!(cfg.port, Some(8119));
/// ```
fn parse_server_url(input: &str) -> Result<ServerConfig, String> {
    if input.is_empty() {
        return Err("empty hostname".to_string());
    }

    let input = input.trim();

    // Determine scheme and strip it
    let use_tls = if input.starts_with("nntps://") {
        true
    } else if input.starts_with("nntp://") {
        false
    } else {
        // No recognized scheme — default to plaintext NNTP
        false
    };

    // Only treat the last ':' as a port separator if what follows is purely numeric.
    // This avoids splitting on colons in malformed URLs like "s://hostname".
    let (hostname, port) = if let Some((host, port_str)) = input.rsplit_once(':') {
        if port_str.chars().all(|c| c.is_ascii_digit()) && !port_str.is_empty() {
            let port = port_str
                .parse::<u16>()
                .map_err(|_| format!("invalid port '{}'", port_str))?;
            (host.to_string(), Some(port))
        } else {
            // Not a valid port — treat entire rest as hostname
            (input.to_string(), None)
        }
    } else {
        (input.to_string(), None)
    };

    if hostname.is_empty() {
        return Err("empty hostname".to_string());
    }

    Ok(ServerConfig {
        hostname,
        port,
        use_tls,
    })
}

/// Interactive NNTP mailing list browser and configuration generator
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// NNTP server URL (e.g., nntp://hostname, nntps://hostname, nntp://hostname:port)
    #[arg(short = 's', long = "server")]
    server: Option<String>,

    /// Optional: username
    #[arg(short = 'u', long = "username")]
    username: Option<String>,

    /// Optional: password
    #[arg(short = 'P', long = "password")]
    password: Option<String>,

    /// Export configuration to YAML file after browsing
    #[arg(long = "export-config")]
    export_config: bool,

    /// Enable verbose logging
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,
}

fn main() -> mlh_archiver::Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    println!("📬 check_nntp - NNTP Mailing List Browser");
    println!("=========================================\n");

    // Get server config from CLI, env, or prompt
    let server = if let Some(url) = args.server {
        match parse_server_url(&url) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("❌ Invalid server URL: {}", e);
                eprintln!("Expected format: nntp://hostname[:port] or nntps://hostname[:port]");
                std::process::exit(1);
            }
        }
    } else if let Ok(env_input) = env::var("NNTP_SERVER") {
        match parse_server_url(&env_input) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("❌ Invalid NNTP_SERVER env var: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        prompt_for_server()
    };

    let server_url = server_address(&server.hostname, server.port);
    let tls_label = if server.use_tls { " (TLS)" } else { "" };
    log::info!("Connecting to NNTP server: {}{}", server_url, tls_label);

    // Connect and retrieve list of groups
    println!("🔍 Fetching available mailing lists from {}{}...", server_url, tls_label);
    let groups = match retrieve_lists_with_connection(
        &server.hostname,
        server.port,
        args.username.clone(),
        args.password.clone(),
    ) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("❌ Failed to connect to NNTP server: {}", e);
            return Err(e);
        }
    };

    println!("✅ Found {} mailing lists\n", groups.len());

    if groups.is_empty() {
        println!("No mailing lists available on this server.");
        return Ok(());
    }

    // Interactive selection
    let mut select_options = vec!["*".to_string()];
    select_options.extend(groups.clone());

    let selected = MultiSelect::new("Select mailing lists to preview:", select_options)
        .with_help_message("Space to select, Enter to confirm")
        .prompt()
        .unwrap_or_else(|_| std::process::exit(0));

    if selected.is_empty() {
        println!("No lists selected. Exiting.");
        return Ok(());
    }

    // Handle "*" selection
    let groups_to_preview = if selected.iter().any(|s| s == "*") {
        println!("📋 Previewing all {} lists...\n", groups.len());
        groups.clone()
    } else {
        println!("📋 Previewing {} selected lists...\n", selected.len());
        selected.clone()
    };

    // Get group info (article ranges)
    println!("📊 Fetching article ranges...");
    let groups_info = match mlh_archiver::nntp_source::retrieve_groups_info(
        &server.hostname,
        server.port,
        &groups_to_preview,
        args.username.clone(),
        args.password.clone(),
    ) {
        Ok(info) => info,
        Err(e) => {
            eprintln!("⚠️  Warning: Failed to fetch some group info: {}", e);
            Vec::new()
        }
    };

    // Display results
    println!("\n📈 Article Range Preview:");
    println!("─────────────────────────────────────────────────────────────");
    println!("{:<50} {:>12}", "Group", "Articles");
    println!("─────────────────────────────────────────────────────────────");

    for (group_name, group_info) in &groups_info {
        let article_count = group_info.high - group_info.low + 1;
        let range_str = format!("[{}..{}]", group_info.low, group_info.high);
        println!("{:<50} {:>12}", truncate_str(group_name, 49), range_str);
        println!("{:<50} {:>12}", "", format!("({} total)", article_count));
    }

    println!("─────────────────────────────────────────────────────────────\n");

    // Show sample configuration
    if args.export_config {
        let config_yaml = generate_config_yaml(&server, &groups_to_preview);
        println!("📝 Generated configuration:");
        println!("{}", config_yaml);

        // Optionally save to file
        let save = Confirm::new("Save this configuration to archiver_config.yaml?")
            .with_default(false)
            .prompt()
            .unwrap_or(false);

        if save {
            let config_content = generate_full_config_yaml(&server, &groups_to_preview);
            match std::fs::write("archiver_config.yaml", config_content) {
                Ok(_) => println!("✅ Configuration saved to archiver_config.yaml"),
                Err(e) => eprintln!("❌ Failed to save configuration: {}", e),
            }
        }
    } else {
        println!("💡 Tip: Run with --export-config to generate archiver configuration");
    }

    // Offer to test fetch a sample article
    if !groups_info.is_empty() {
        let test_fetch = inquire::Confirm::new("Test fetch a sample article from a selected list?")
            .with_default(false)
            .prompt()
            .unwrap_or(false);

        if test_fetch {
            let list_options: Vec<&String> = groups_info.iter().map(|(name, _)| name).collect();
            if let Ok(selection) = Select::new("Select a list to test:", list_options).prompt() {
                if let Some((_, group_info)) =
                    groups_info.iter().find(|(name, _)| name == selection)
                {
                    println!(
                        "\n📥 Testing fetch from {} (articles {} to {})",
                        selection, group_info.low, group_info.high
                    );

                    if group_info.high >= group_info.low {
                        let test_article_num = group_info.high;
                        println!("Attempting to fetch article #{}...", test_article_num);

                        match connect_to_nntp_server(
                            &server.hostname,
                            server.port,
                            args.username.clone(),
                            args.password.clone(),
                        ) {
                            Ok(mut stream) => {
                                // Select the group first
                                match stream.group(selection) {
                                    Ok(_) => match stream.raw_article_by_number(test_article_num) {
                                        Ok(raw_lines) => {
                                            println!(
                                                "✅ Successfully fetched article #{}",
                                                test_article_num
                                            );
                                            println!("Size: {} lines", raw_lines.len());
                                            println!(
                                                "First few lines: {}",
                                                raw_lines
                                                    .iter()
                                                    .take(3)
                                                    .map(|s| s.as_str())
                                                    .collect::<Vec<_>>()
                                                    .join(", ")
                                            );
                                        }
                                        Err(e) => {
                                            println!("⚠️  Article unavailable: {}", e);
                                        }
                                    },
                                    Err(e) => {
                                        println!("⚠️  Failed to select group: {}", e);
                                    }
                                }
                                let _ = stream.quit();
                            }
                            Err(e) => {
                                println!("⚠️  Failed to connect: {}", e);
                            }
                        }
                    } else {
                        println!("⚠️  Group appears to be empty (low > high)");
                    }
                }
            }
        }
    }

    println!("\n✨ Done!");
    Ok(())
}

/// Prompt user for NNTP server URL
fn prompt_for_server() -> ServerConfig {
    let input = Text::new("Enter NNTP server URL:")
        .with_default("nntp://nntp.example.com")
        .with_help_message(
            "nntp://hostname (port 119), nntps://hostname (port 563), or nntp://hostname:port",
        )
        .prompt()
        .unwrap_or_else(|_| std::process::exit(0));

    match parse_server_url(&input) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("❌ Invalid URL: {}", e);
            std::process::exit(1);
        }
    }
}

/// Truncate string to max length with ellipsis
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}


/// Generate minimal config snippet for selected lists
fn generate_config_yaml(server: &ServerConfig, groups: &[String]) -> String {
    let lists_yaml = groups
        .iter()
        .map(|g| format!("      - {}", g))
        .collect::<Vec<_>>()
        .join("\n");

    let port_line = match server.port {
        Some(p) => format!("  port: {}", p),
        None => "  # port: 119  # optional, defaults to 119".to_string(),
    };

    format!(
        r#"# NNTP Configuration Snippet
# Add this to your archiver_config.yaml

nntp:
  hostname: "{}"
{}
  group_lists:
{}
"#,
        server.hostname, port_line, lists_yaml
    )
}

/// Generate full configuration file content
fn generate_full_config_yaml(server: &ServerConfig, groups: &[String]) -> String {
    let lists_yaml = groups
        .iter()
        .map(|g| format!("      - {}", g))
        .collect::<Vec<_>>()
        .join("\n");

    let port_line = match server.port {
        Some(p) => format!("  port: {}", p),
        None => "  # port: 119  # optional, defaults to 119".to_string(),
    };

    format!(
        r#"# MLH Archiver Configuration
# Generated by check_nntp

nthreads: 2
output_dir: "./output"
loop_groups: true

nntp:
  hostname: "{}"
{}
  group_lists:
{}
  # article_range: "1-100"  # Optional: fetch specific range
"#,
        server.hostname, port_line, lists_yaml
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_nntp_default_port() {
        let cfg = parse_server_url("nntp://example.com").unwrap();
        assert_eq!(cfg.hostname, "nntp://example.com");
        assert_eq!(cfg.port, None);
        assert!(!cfg.use_tls);
    }

    #[test]
    fn test_parse_nntps_default_port() {
        let cfg = parse_server_url("nntps://example.com").unwrap();
        assert_eq!(cfg.hostname, "nntps://example.com");
        assert_eq!(cfg.port, None);
        assert!(cfg.use_tls);
    }

    #[test]
    fn test_parse_nntp_with_port() {
        let cfg = parse_server_url("nntp://example.com:8119").unwrap();
        assert_eq!(cfg.hostname, "nntp://example.com");
        assert_eq!(cfg.port, Some(8119));
        assert!(!cfg.use_tls);
    }

    #[test]
    fn test_parse_nntps_with_port() {
        let cfg = parse_server_url("nntps://example.com:563").unwrap();
        assert_eq!(cfg.hostname, "nntps://example.com");
        assert_eq!(cfg.port, Some(563));
        assert!(cfg.use_tls);
    }

    #[test]
    fn test_parse_ip_with_port() {
        let cfg = parse_server_url("nntp://192.168.1.1:5119").unwrap();
        assert_eq!(cfg.hostname, "nntp://192.168.1.1");
        assert_eq!(cfg.port, Some(5119));
        assert!(!cfg.use_tls);
    }

    #[test]
    fn test_parse_ip_without_port() {
        let cfg = parse_server_url("nntp://192.168.1.1").unwrap();
        assert_eq!(cfg.hostname, "nntp://192.168.1.1");
        assert_eq!(cfg.port, None);
        assert!(!cfg.use_tls);
    }

    #[test]
    fn test_parse_no_scheme_defaults_to_nntp() {
        let cfg = parse_server_url("example.com").unwrap();
        assert_eq!(cfg.hostname, "example.com");
        assert_eq!(cfg.port, None);
        assert!(!cfg.use_tls);
    }

    #[test]
    fn test_parse_no_scheme_with_port() {
        let cfg = parse_server_url("example.com:8119").unwrap();
        assert_eq!(cfg.hostname, "example.com");
        assert_eq!(cfg.port, Some(8119));
        assert!(!cfg.use_tls);
    }

    #[test]
    fn test_parse_trims_whitespace() {
        let cfg = parse_server_url("  nntp://example.com:5119  ").unwrap();
        assert_eq!(cfg.hostname, "nntp://example.com");
        assert_eq!(cfg.port, Some(5119));
    }

    #[test]
    fn test_parse_invalid_port_falls_back_to_hostname() {
        // Non-numeric "port" is treated as part of the hostname
        let cfg = parse_server_url("nntp://example.com:abc").unwrap();
        assert_eq!(cfg.hostname, "nntp://example.com:abc");
        assert_eq!(cfg.port, None);
    }

    #[test]
    fn test_parse_port_out_of_range() {
        let result = parse_server_url("nntp://example.com:70000");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_hostname() {
        // "nntp://" has no hostname after scheme — rsplit_once gives ("nntp", "")
        // "" is not numeric, so falls back to full input as hostname
        let cfg = parse_server_url("nntp://").unwrap();
        assert_eq!(cfg.hostname, "nntp://");
        assert_eq!(cfg.port, None);
    }

    #[test]
    fn test_parse_empty_input() {
        let result = parse_server_url("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_typo_scheme_keeps_full_input() {
        // "nntps://" (double t) is not recognized — full input becomes hostname
        let cfg = parse_server_url("nntps://news.example.com").unwrap();
        assert_eq!(cfg.hostname, "nntps://news.example.com");
        assert_eq!(cfg.port, None);
        assert!(!cfg.use_tls);
    }
}
