use std::path::{Path, PathBuf};

/// A public-inbox email reader using gitoxide (gix).
/// Scans a directory for public-inbox subdirectories and reads the last N emails from each.
use chrono::DateTime;
use clap::Parser;
use gix::bstr::ByteSlice;
use inquire::{Confirm, MultiSelect, Select, Text};

use gix::revision::walk::Info;
use mlh_archiver::public_inbox_source::pi_utils::*;

fn main() {
    let args = Args::parse_from(gix::env::args_os());

    // Initialize logging
    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    match run(args) {
        Ok(()) => {}
        Err(e) => eprintln!("error: {e}"),
    }
}

#[derive(Debug, clap::Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to directory containing public-inbox directories
    #[arg(short, long)]
    inbox_dir: PathBuf,

    /// Number of recent emails to read per inbox (for quick preview)
    #[arg(short, long, default_value = "5")]
    count: usize,

    /// Export configuration to YAML file after browsing
    #[arg(long = "export-config")]
    export_config: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Run a test fetch of a sample article (non-interactive)
    #[arg(long = "test")]
    test: bool,

    /// Specify list name for test fetch (requires --test)
    #[arg(long = "list")]
    list_name: Option<String>,

    /// Article number (position) to fetch (requires --test)
    #[arg(long = "article")]
    article: Option<usize>,
}

fn run(args: Args) -> anyhow::Result<()> {
    let inbox_dir = &args.inbox_dir;
    let count = args.count;

    if !inbox_dir.is_dir() {
        anyhow::bail!(
            "Inbox directory does not exist or is not a directory: {}",
            inbox_dir.display()
        );
    }

    // Find all public-inbox subdirectories
    let inboxes = find_public_inboxes(inbox_dir)?;

    if inboxes.is_empty() {
        println!(
            "No public-inbox directories found in {}",
            inbox_dir.display()
        );
        return Ok(());
    }

    println!("Found {} public-inbox(es)\n", inboxes.len());
    for inbox in &inboxes {
        println!(
            "  - {} ({}): {}",
            inbox.name,
            inbox.version,
            inbox.git_dir.display()
        );
    }

    // Filter out incomplete repositories
    let valid_inboxes: Vec<_> = inboxes
        .into_iter()
        .filter(|inbox| !inbox.version.contains("incomplete"))
        .collect();

    if valid_inboxes.is_empty() {
        println!("No valid public-inboxes found.");
        return Ok(());
    }

    println!("DEBUG: test mode = {}", args.test);
    // Test mode: fetch a sample article and exit
    if args.test {
        return run_test_mode(&valid_inboxes, args.list_name.as_deref(), args.article);
    }

    // Show commit counts for each inbox
    println!("DEBUG: commit counts block");
    println!("📊 Commit counts:");
    for inbox in &valid_inboxes {
        eprintln!("DEBUG: counting commits for {}", inbox.name);
        match count_commits(inbox) {
            Ok(count) => println!("  - {}: {} commits", inbox.name, count),
            Err(e) => println!("  - {}: failed to count commits ({})", inbox.name, e),
        }
    }
    println!();

    println!(
        "{} valid public-inbox(es) available for selection.\n",
        valid_inboxes.len()
    );

    // Interactive selection
    let selected = MultiSelect::new("Select mailing lists:", valid_inboxes)
        .with_help_message("Space to select, Enter to confirm")
        .prompt()
        .unwrap_or_else(|_| std::process::exit(0));

    if selected.is_empty() {
        println!("No lists selected. Exiting.");
        return Ok(());
    }

    println!("\n✅ Selected {} inbox(es):", selected.len());
    for inbox in &selected {
        println!("  - {}", inbox.name);
    }
    println!();

    // Main action loop
    loop {
        let actions = vec![
            "Quick preview (last N emails)",
            "Browse commits interactively",
            "Test fetch a sample article",
            "Generate configuration",
            "Exit",
        ];

        let action = Select::new("What would you like to do?", actions).prompt()?;

        match action {
            "Quick preview (last N emails)" => {
                for inbox in &selected {
                    println!("\nProcessing inbox: {}", inbox.name);
                    println!("  Version: {}", inbox.version);
                    println!("  Git repo: {}", inbox.git_dir.display());

                    match process_inbox(inbox, count) {
                        Ok(email_count) => {
                            println!("  Read {} email(s)\n", email_count);
                        }
                        Err(e) => eprintln!("  Error reading emails: {e}\n"),
                    }
                }
            }
            "Browse commits interactively" => {
                // Let user choose which inbox to browse
                let inbox_names: Vec<String> = selected.iter().map(|i| i.name.clone()).collect();
                let chosen = Select::new("Select inbox to browse:", inbox_names).prompt()?;
                if let Some(inbox) = selected.iter().find(|i| i.name == chosen) {
                    browse_inbox(inbox)?;
                }
            }
            "Test fetch a sample article" => {
                // Let user choose which inbox to test
                let inbox_names: Vec<String> = selected.iter().map(|i| i.name.clone()).collect();
                let chosen = Select::new("Select inbox to test:", inbox_names).prompt()?;
                if let Some(inbox) = selected.iter().find(|i| i.name == chosen) {
                    // Ask for article position (optional)
                    let position_str = Text::new("Article position (optional, default 1):")
                        .with_default("1")
                        .prompt()?;
                    let position = position_str.parse::<usize>().unwrap_or(1);
                    fetch_single_commit(inbox, position)?;
                }
            }
            "Generate configuration" => {
                generate_config_yaml(&selected, inbox_dir)?;
            }
            "Exit" => {
                println!("Goodbye!");
                break;
            }
            _ => unreachable!(),
        }
    }

    Ok(())
}

/// Run test mode: fetch a specific article from a specific list
fn run_test_mode(
    valid_inboxes: &[PublicInbox],
    list_name: Option<&str>,
    article_pos: Option<usize>,
) -> anyhow::Result<()> {
    if valid_inboxes.is_empty() {
        anyhow::bail!("No valid public inboxes available for testing");
    }

    let inbox = if let Some(name) = list_name {
        valid_inboxes
            .iter()
            .find(|inbox| inbox.name == name)
            .ok_or_else(|| anyhow::anyhow!("List '{}' not found", name))?
    } else {
        &valid_inboxes[0]
    };

    let position = article_pos.unwrap_or(1);

    println!(
        "Testing fetch from list '{}', article position {}",
        inbox.name, position
    );
    fetch_single_commit(inbox, position)
}

/// Fetch a single commit by position (1-indexed from newest)
fn fetch_single_commit(inbox: &PublicInbox, position: usize) -> anyhow::Result<()> {
    // Skip incomplete repositories
    if inbox.version.contains("incomplete") {
        anyhow::bail!("Incomplete repository: {}", inbox.version);
    }

    let mut repo = gix::open(&inbox.git_dir)?;
    repo.object_cache_size(50_000_000);

    // Resolve refs/heads/master to get HEAD commit
    let head_ref = repo
        .refs
        .find("refs/heads/master")
        .map_err(|_| anyhow::anyhow!("refs/heads/master not found"))?;
    let head_id = head_ref
        .target
        .try_id()
        .ok_or_else(|| anyhow::anyhow!("refs/heads/master does not point to an object"))?
        .to_owned();

    // Walk all commits from HEAD (newest first)
    let all_commit_ids: Vec<_> = repo
        .rev_walk([head_id])
        .all()?
        .filter_map(|r| r.ok())
        .collect();

    if position == 0 || position > all_commit_ids.len() {
        anyhow::bail!(
            "Position {} out of range (total commits: {})",
            position,
            all_commit_ids.len()
        );
    }

    let commit_info = &all_commit_ids[position - 1];
    view_commit(&repo, commit_info, position)
}

/// Count total commits in a public inbox repository.
fn count_commits(inbox: &PublicInbox) -> anyhow::Result<usize> {
    // Skip incomplete repositories
    if inbox.version.contains("incomplete") {
        return Ok(0);
    }

    let mut repo = gix::open(&inbox.git_dir)?;
    repo.object_cache_size(50_000_000);

    // Resolve refs/heads/master to get HEAD commit
    let head_ref = repo
        .refs
        .find("refs/heads/master")
        .map_err(|_| anyhow::anyhow!("refs/heads/master not found"))?;
    let head_id = head_ref
        .target
        .try_id()
        .ok_or_else(|| anyhow::anyhow!("refs/heads/master does not point to an object"))?
        .to_owned();

    // Walk all commits from HEAD (newest first)
    let all_commit_ids: Vec<_> = repo
        .rev_walk([head_id])
        .all()?
        .filter_map(|r| r.ok())
        .collect();

    Ok(all_commit_ids.len())
}

/// Opens a public-inbox git repo, reads the most recent emails,
/// prints each one immediately, and returns the count of emails processed.
fn process_inbox(inbox: &PublicInbox, count: usize) -> anyhow::Result<usize> {
    // Skip incomplete repositories
    if inbox.version.contains("incomplete") {
        println!("  ⚠️  Skipping incomplete repository: {}", inbox.version);
        return Ok(0);
    }

    let repo = gix::open(&inbox.git_dir)?;

    // Enable object cache for better performance
    let mut repo = repo;
    repo.object_cache_size(50_000_000); // 50MB cache

    // Resolve refs/heads/master to get the HEAD commit
    let head_ref = repo
        .refs
        .find("refs/heads/master")
        .map_err(|_| anyhow::anyhow!("refs/heads/master not found"))?;
    let head_id = head_ref
        .target
        .try_id()
        .ok_or_else(|| anyhow::anyhow!("refs/heads/master does not point to an object"))?
        .to_owned();

    // Walk all commits from HEAD (tip/newest first)
    let all_commit_ids: Vec<_> = repo
        .rev_walk([head_id])
        .all()?
        .filter_map(|r| r.ok())
        .collect();

    if all_commit_ids.is_empty() {
        return Ok(0);
    }

    // Take the first `count` commits (most recent)
    let commits_to_process: Vec<_> = all_commit_ids.into_iter().take(count).collect();

    let mut email_count = 0;

    for info in commits_to_process {
        let commit = repo.find_commit(info.id)?;
        let commit_ref = commit.decode()?;

        let author = commit_ref.author()?;
        let author_time = author.time()?;
        let subject = commit_ref.message.to_str_lossy().to_string();

        // Get the tree and find the "m" entry (message file)
        let tree_id = commit_ref.tree();
        let tree = repo.find_tree(tree_id)?;

        let blob_oid = tree
            .iter()
            .find_map(|e| e.ok())
            .filter(|e| e.filename().as_bytes() == b"m")
            .map(|e| e.object_id());

        if let Some(blob_oid) = blob_oid {
            let blob = repo.find_blob(blob_oid)?;
            let raw_email = String::from_utf8_lossy(&blob.data).to_string();

            // Print immediately
            let timestamp = DateTime::from_timestamp(author_time.seconds, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| format!("timestamp={}", author_time.seconds));

            email_count += 1;
            println!("  --- Email {email_count} ---");
            println!("  Subject: {}", subject.lines().next().unwrap_or(""));
            println!("  Author:  {} <{}>", author.name, author.email);
            println!("  Date:    {timestamp}");
            println!("  Commit:  {}", info.id.to_hex());
            println!("  Raw email:");
            for line in raw_email.lines() {
                println!("    {line}");
            }
            println!();
        }
    }

    // repo dropped here; entire inbox freed from memory
    Ok(email_count)
}

/// Browse commits in a public inbox with pagination
fn browse_inbox(inbox: &PublicInbox) -> anyhow::Result<()> {
    // Skip incomplete repositories
    if inbox.version.contains("incomplete") {
        println!("⚠️  Skipping incomplete repository: {}", inbox.version);
        return Ok(());
    }

    let mut repo = gix::open(&inbox.git_dir)?;
    repo.object_cache_size(50_000_000); // 50MB cache

    // Resolve refs/heads/master to get HEAD commit
    let head_ref = repo
        .refs
        .find("refs/heads/master")
        .map_err(|_| anyhow::anyhow!("refs/heads/master not found"))?;
    let head_id = head_ref
        .target
        .try_id()
        .ok_or_else(|| anyhow::anyhow!("refs/heads/master does not point to an object"))?
        .to_owned();

    // Collect all commit IDs (newest first)
    let all_commit_ids: Vec<_> = repo
        .rev_walk([head_id])
        .all()?
        .filter_map(|r| r.ok())
        .collect();

    let total_commits = all_commit_ids.len();
    if total_commits == 0 {
        println!("No commits found in this inbox.");
        return Ok(());
    }

    println!("📬 Inbox: {} ({} commits)", inbox.name, total_commits);

    // Pagination variables
    let page_size = 20;
    let mut current_page = 0;
    let total_pages = total_commits.div_ceil(page_size);

    loop {
        let start = current_page * page_size;
        let end = (start + page_size).min(total_commits);
        let page_commits = &all_commit_ids[start..end];

        // Fetch commit details for this page
        let mut commit_details = Vec::new();
        for (i, info) in page_commits.iter().enumerate() {
            let commit = repo.find_commit(info.id)?;
            let commit_ref = commit.decode()?;
            let author = commit_ref.author()?;
            let author_time = author.time()?;
            let subject = commit_ref.message.to_str_lossy().to_string();
            let subject_preview = subject.lines().next().unwrap_or("").to_string();
            let date = DateTime::from_timestamp(author_time.seconds, 0)
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| format!("timestamp={}", author_time.seconds));

            commit_details.push(format!(
                "{:4} | {} | {} | {}",
                start + i + 1, // position (1-indexed)
                date,
                author.name.to_str_lossy(),
                truncate_subject(&subject_preview, 50)
            ));
        }

        // Display page header
        println!(
            "\n📄 Page {} of {} (commits {} to {})",
            current_page + 1,
            total_pages,
            start + 1,
            end
        );
        println!("─────────────────────────────────────────────────────────────");
        for detail in &commit_details {
            println!("{}", detail);
        }
        println!("─────────────────────────────────────────────────────────────");

        // Action selection
        let mut actions = vec![
            "Select commits on this page",
            "View a single commit by number",
        ];
        if current_page > 0 {
            actions.push("Previous page");
        }
        if current_page < total_pages - 1 {
            actions.push("Next page");
        }
        actions.push("Back to inbox selection");

        let action = Select::new("Choose action:", actions).prompt()?;

        match action {
            "Select commits on this page" => {
                let selections =
                    MultiSelect::new("Select commits to view:", commit_details.clone())
                        .with_help_message("Space to select, Enter to confirm")
                        .prompt()?;

                for selected in selections {
                    if let Some(index) = commit_details.iter().position(|c| c == &selected) {
                        let commit_idx = start + index;
                        let commit_info = &all_commit_ids[commit_idx];
                        view_commit(&repo, commit_info, commit_idx + 1)?;
                    }
                }
            }
            "View a single commit by number" => {
                let commit_num = Text::new("Enter commit number (position):")
                    .with_default(&format!("{}", start + 1))
                    .prompt()?;
                if let Ok(num) = commit_num.parse::<usize>() {
                    if num >= 1 && num <= total_commits {
                        let commit_idx = num - 1;
                        let commit_info = &all_commit_ids[commit_idx];
                        view_commit(&repo, commit_info, num)?;
                    } else {
                        println!(
                            "Invalid commit number. Must be between 1 and {}.",
                            total_commits
                        );
                    }
                } else {
                    println!("Invalid number.");
                }
            }
            "Previous page" => {
                current_page -= 1;
            }
            "Next page" => {
                current_page += 1;
            }
            "Back to inbox selection" => {
                break;
            }
            _ => unreachable!(),
        }
    }

    Ok(())
}

/// Generate configuration YAML for selected public inboxes
fn generate_config_yaml(inboxes: &[PublicInbox], inbox_dir: &Path) -> anyhow::Result<()> {
    println!(
        "\n📝 Generating configuration for {} inbox(es)",
        inboxes.len()
    );

    // Ask for origin (default to "public-inbox")
    let origin = Text::new("Enter origin identifier:")
        .with_default("public-inbox")
        .prompt()?;

    // Ask for article range (optional)
    let article_range = Text::new("Article range (optional, e.g., '1-100'):")
        .with_default("")
        .prompt()?;

    let article_range_str = if article_range.trim().is_empty() {
        None
    } else {
        Some(article_range.trim().to_string())
    };

    // Build group_lists from selected inboxes

    // Construct YAML
    let mut yaml = String::new();
    yaml.push_str("# MLH Archiver Configuration - Public Inbox\n");
    yaml.push_str("# Generated by check_git\n\n");
    yaml.push_str("nthreads: 2\n");
    yaml.push_str("output_dir: \"./output\"\n");
    yaml.push_str("loop_groups: false\n\n");
    yaml.push_str("public_inbox:\n");
    yaml.push_str(&format!(
        "  inport_directory: \"{}\"\n",
        inbox_dir.display()
    ));
    yaml.push_str(&format!("  origin: \"{}\"\n", origin));
    yaml.push_str("  group_lists:\n");
    for inbox in inboxes {
        yaml.push_str(&format!("    - \"{}\"\n", inbox.name));
    }
    if let Some(range) = article_range_str {
        yaml.push_str(&format!("  article_range: \"{}\"\n", range));
    }

    println!("\n{}\n", yaml);

    // Offer to save to file
    let save = Confirm::new("Save this configuration to archiver_config.yaml?")
        .with_default(false)
        .prompt()?;

    if save {
        match std::fs::write("archiver_config.yaml", yaml) {
            Ok(_) => println!("✅ Configuration saved to archiver_config.yaml"),
            Err(e) => eprintln!("❌ Failed to save configuration: {}", e),
        }
    }

    Ok(())
}

/// View a single commit in detail (including email content)
fn view_commit(
    repo: &gix::Repository,
    commit_info: &Info<'_>,
    position: usize,
) -> anyhow::Result<()> {
    let commit = repo.find_commit(commit_info.id)?;
    let commit_ref = commit.decode()?;
    let author = commit_ref.author()?;
    let author_time = author.time()?;
    let subject = commit_ref.message.to_str_lossy().to_string();

    let date = DateTime::from_timestamp(author_time.seconds, 0)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| format!("timestamp={}", author_time.seconds));

    println!("\n📧 Commit #{}", position);
    println!("─────────────────────────────────────");
    println!("Subject: {}", subject.lines().next().unwrap_or(""));
    println!("Author:  {} <{}>", author.name, author.email);
    println!("Date:    {}", date);
    println!("Commit:  {}", commit_info.id.to_hex());

    // Get the tree and find the "m" entry (message file)
    let tree_id = commit_ref.tree();
    let tree = repo.find_tree(tree_id)?;

    let blob_oid = tree
        .iter()
        .find_map(|e| e.ok())
        .filter(|e| e.filename().as_bytes() == b"m")
        .map(|e| e.object_id());

    if let Some(blob_oid) = blob_oid {
        let blob = repo.find_blob(blob_oid)?;
        let raw_email = String::from_utf8_lossy(&blob.data);
        println!("─────────────────────────────────────");
        for line in raw_email.lines() {
            println!("{}", line);
        }
    } else {
        println!("No 'm' file found in commit tree");
    }
    println!("─────────────────────────────────────\n");
    Ok(())
}

/// Truncate subject for display
fn truncate_subject(subject: &str, max_len: usize) -> String {
    if subject.len() <= max_len {
        subject.to_string()
    } else {
        format!("{}...", &subject[..max_len - 3])
    }
}
