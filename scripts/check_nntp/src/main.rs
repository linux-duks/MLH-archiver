//! check_nntp - Interactive NNTP mailing list browser
//!
//! This tool allows you to interactively browse available NNTP mailing lists,
//! preview article ranges, and generate configuration snippets for the MLH Archiver.
//!
//! # Usage
//!
//! ```bash
//! # Interactive mode (prompts for hostname)
//! cargo run --package check_nntp
//!
//! # With CLI arguments
//! cargo run --package check_nntp -- -H nntp.example.com -p 119
//!
//! # Export configuration after browsing
//! cargo run --package check_nntp -- -H nntp.example.com --export-config
//! ```

use clap::Parser;
use inquire::{Confirm, MultiSelect, Select, Text};
use mlh_archiver::nntp_source::{connect_to_nntp_server, retrieve_lists_with_connection};
use std::env;

/// Interactive NNTP mailing list browser and configuration generator
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// NNTP server hostname (accepts "hostname" or "hostname:port")
    #[arg(short = 'H', long = "hostname")]
    hostname: Option<String>,

    /// NNTP server port (overridden if port is specified in --hostname)
    #[arg(short = 'p', long = "port", default_value = "119")]
    port: u16,

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

    // Get hostname and port from CLI, env, or prompt
    // Priority: CLI args > env vars > interactive prompt
    // The prompt accepts "hostname:port" format
    let (hostname, port) = if let Some(host_input) = args.hostname {
        parse_host_port(&host_input, args.port)
    } else if let Ok(env_input) = env::var("NNTP_HOSTNAME") {
        parse_host_port(&env_input, args.port)
    } else {
        prompt_for_server(args.port)
    };

    log::info!("Connecting to NNTP server: {}:{}", hostname, port);

    // Connect and retrieve list of groups
    println!(
        "🔍 Fetching available mailing lists from {}:{}...",
        hostname, port
    );
    let groups = match retrieve_lists_with_connection(
        &hostname,
        port,
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
    let mut select_options = vec!["ALL".to_string()];
    select_options.extend(groups.clone());

    let selected = MultiSelect::new("Select mailing lists to preview:", select_options)
        .with_help_message("Space to select, Enter to confirm")
        .prompt()
        .unwrap_or_else(|_| std::process::exit(0));

    if selected.is_empty() {
        println!("No lists selected. Exiting.");
        return Ok(());
    }

    // Handle "ALL" selection
    let groups_to_preview = if selected.iter().any(|s| s == "ALL") {
        println!("📋 Previewing all {} lists...\n", groups.len());
        groups.clone()
    } else {
        println!("📋 Previewing {} selected lists...\n", selected.len());
        selected.clone()
    };

    // Get group info (article ranges)
    println!("📊 Fetching article ranges...");
    let groups_info = match mlh_archiver::nntp_source::retrieve_groups_info(
        &hostname,
        port,
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
        let config_yaml = generate_config_yaml(&hostname, port, &groups_to_preview);
        println!("📝 Generated configuration:");
        println!("{}", config_yaml);

        // Optionally save to file
        let save = Confirm::new("Save this configuration to archiver_config.yaml?")
            .with_default(false)
            .prompt()
            .unwrap_or(false);

        if save {
            let config_content = generate_full_config_yaml(&hostname, port, &groups_to_preview);
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

                        match connect_to_nntp_server(&hostname, port, args.username, args.password)
                        {
                            Ok(mut stream) => {
                                // Select the group first
                                match stream.group(selection) {
                                    Ok(_) => {
                                        match stream
                                            .raw_article_by_number(test_article_num)
                                        {
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
                                        }
                                    }
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

/// Parses a "hostname:port" string, returning (hostname, port).
/// If no port is specified in the input, returns the default port.
///
/// # Supported formats
///
/// - `"hostname"` → `(hostname, default_port)`
/// - `"hostname:port"` → `(hostname, port)`
/// - `"192.168.1.1:5119"` → `("192.168.1.1", 5119)`
fn parse_host_port(input: &str, default_port: u16) -> (String, u16) {
    let input = input.trim();

    // Try to split on the last colon to handle hostname:port
    if let Some((host, port_str)) = input.rsplit_once(':') {
        if let Ok(port) = port_str.parse::<u16>() {
            return (host.to_string(), port);
        }
    }

    // No valid port found, return input as hostname with default port
    (input.to_string(), default_port)
}

/// Prompt user for NNTP server address (hostname or hostname:port)
fn prompt_for_server(default_port: u16) -> (String, u16) {
    let input = Text::new("Enter NNTP server (hostname or hostname:port):")
        .with_default("nntp.example.com")
        .with_help_message("e.g., nntp.example.com or nntp.example.com:5119")
        .prompt()
        .unwrap_or_else(|_| std::process::exit(0));

    parse_host_port(&input, default_port)
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
fn generate_config_yaml(hostname: &str, port: u16, groups: &[String]) -> String {
    let lists_yaml = groups
        .iter()
        .map(|g| format!("      - {}", g))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"# NNTP Configuration Snippet
# Add this to your archiver_config.yaml

nntp:
  hostname: "{}"
  port: {}
  group_lists:
{}
"#,
        hostname, port, lists_yaml
    )
}

/// Generate full configuration file content
fn generate_full_config_yaml(hostname: &str, port: u16, groups: &[String]) -> String {
    let lists_yaml = groups
        .iter()
        .map(|g| format!("      - {}", g))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"# MLH Archiver Configuration
# Generated by check_nntp

nthreads: 2
output_dir: "./output"
loop_groups: true

nntp:
  hostname: "{}"
  port: {}
  group_lists:
{}
  # article_range: "1-100"  # Optional: fetch specific range
"#,
        hostname, port, lists_yaml
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hostname_only() {
        let (host, port) = parse_host_port("nntp.example.com", 119);
        assert_eq!(host, "nntp.example.com");
        assert_eq!(port, 119);
    }

    #[test]
    fn test_parse_hostname_with_port() {
        let (host, port) = parse_host_port("nntp.example.com:5119", 119);
        assert_eq!(host, "nntp.example.com");
        assert_eq!(port, 5119);
    }

    #[test]
    fn test_parse_ip_with_port() {
        let (host, port) = parse_host_port("192.168.1.1:5119", 119);
        assert_eq!(host, "192.168.1.1");
        assert_eq!(port, 5119);
    }

    #[test]
    fn test_parse_ip_without_port() {
        let (host, port) = parse_host_port("192.168.1.1", 119);
        assert_eq!(host, "192.168.1.1");
        assert_eq!(port, 119);
    }

    #[test]
    fn test_parse_trims_whitespace() {
        let (host, port) = parse_host_port("  nntp.example.com:5119  ", 119);
        assert_eq!(host, "nntp.example.com");
        assert_eq!(port, 5119);
    }

    #[test]
    fn test_parse_invalid_port_falls_back() {
        let (host, port) = parse_host_port("nntp.example.com:abc", 119);
        assert_eq!(host, "nntp.example.com:abc");
        assert_eq!(port, 119);
    }

    #[test]
    fn test_parse_port_out_of_range_falls_back() {
        let (host, port) = parse_host_port("nntp.example.com:70000", 119);
        assert_eq!(host, "nntp.example.com:70000");
        assert_eq!(port, 119);
    }
}
