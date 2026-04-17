use std::path::{Path, PathBuf};

/// A public-inbox email reader using gitoxide (gix).
/// Scans a directory for public-inbox subdirectories and reads the last N emails from each.
use chrono::DateTime;
use clap::Parser;
use gix::bstr::ByteSlice;

use mlh_archiver::public_inbox_source::pi_utils::*;

fn main() {
    let args = Args::parse_from(gix::env::args_os());
    match run(args) {
        Ok(()) => {}
        Err(e) => eprintln!("error: {e}"),
    }
}

#[derive(Debug, clap::Parser)]
#[clap(
    name = "check_git",
    about = "Read recent emails from public-inbox archives using git",
    version = option_env!("GIX_VERSION")
)]
struct Args {
    /// Path to directory containing public-inbox directories
    #[clap(short, long)]
    inbox_dir: PathBuf,

    /// Number of recent emails to read per inbox
    #[clap(short, long, default_value = "5")]
    count: usize,
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
        println!("Processing inbox: {}", inbox.name);
        println!("  Version: {}", inbox.version);
        println!("  Git repo: {}", inbox.git_dir.display());

        match process_inbox(inbox, count) {
            Ok(email_count) => {
                println!("  Read {} email(s)\n", email_count);
            }
            Err(e) => eprintln!("  Error reading emails: {e}\n"),
        }
    }

    Ok(())
}

/// Opens a public-inbox git repo, reads the most recent emails,
/// prints each one immediately, and returns the count of emails processed.
fn process_inbox(inbox: &PublicInbox, count: usize) -> anyhow::Result<usize> {
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

            let preview = if raw_email.len() > 500 {
                format!("{}...", &raw_email[..500])
            } else {
                raw_email
            };

            email_count += 1;
            println!("  --- Email {email_count} ---");
            println!("  Subject: {}", subject.lines().next().unwrap_or(""));
            println!("  Author:  {} <{}>", author.name, author.email);
            println!("  Date:    {timestamp}");
            println!("  Commit:  {}", info.id.to_hex());
            println!("  Raw email:");
            for line in preview.lines() {
                println!("    {line}");
            }
            println!();
            // blob, tree, commit dropped here; memory freed
        }
    }

    // repo dropped here; entire inbox freed from memory
    Ok(email_count)
}
