use std::path::{Path, PathBuf};

/// A public-inbox email reader using gitoxide (gix).
/// Scans a directory for public-inbox subdirectories and reads the last N emails from each.
use chrono::DateTime;
use clap::Parser;
use gix::bstr::ByteSlice;

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
        anyhow::bail!("Inbox directory does not exist or is not a directory: {}", inbox_dir.display());
    }

    // Find all public-inbox subdirectories
    let inboxes = find_public_inboxes(inbox_dir)?;

    if inboxes.is_empty() {
        println!("No public-inbox directories found in {}", inbox_dir.display());
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

/// Represents a detected public-inbox directory.
struct PublicInbox {
    /// Display name of the inbox
    name: String,
    /// V1 or V2
    version: String,
    /// Path to the git repository containing the emails
    git_dir: PathBuf,
}

/// Scans the base directory for public-inbox subdirectories.
fn find_public_inboxes(base_dir: &Path) -> anyhow::Result<Vec<PublicInbox>> {
    let mut inboxes = Vec::new();

    for entry in std::fs::read_dir(base_dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        // Try to detect if this is a public-inbox directory
        if let Some(inbox) = detect_inbox(&path)? {
            inboxes.push(inbox);
        }
    }

    // Sort by name for consistent output
    inboxes.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(inboxes)
}

/// Detects if a directory is a public-inbox (V1 or V2) and returns its info.
fn detect_inbox(dir: &Path) -> anyhow::Result<Option<PublicInbox>> {
    let name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Check for V2: has git/ directory with numbered epoch repos (git/0.git, git/1.git, etc.)
    // and an all.git that chains them via git alternates.
    let git_dir = dir.join("git");
    if git_dir.is_dir() {
        // Look for epoch repos like 0.git, 1.git, etc.
        for entry in std::fs::read_dir(&git_dir)? {
            let entry = entry?;
            let epoch_path = entry.path();
            if epoch_path.is_dir() {
                let epoch_name = epoch_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                // Check if it ends with .git (like 0.git) and has refs/heads/master
                if epoch_name.ends_with(".git")
                    && epoch_path.join("HEAD").is_file()
                    && epoch_path.join("refs/heads/master").is_file()
                {
                    return Ok(Some(PublicInbox {
                        name,
                        version: "V2".to_string(),
                        git_dir: epoch_path,
                    }));
                }
            }
        }
    }

    // Check for V1: single bare git repo at the inbox directory itself
    // (or an all.git that IS the main repo, not using alternates)
    let all_git = dir.join("all.git");
    if all_git.is_dir() && all_git.join("refs/heads/master").is_file() {
        return Ok(Some(PublicInbox {
            name,
            version: "V1".to_string(),
            git_dir: all_git,
        }));
    }

    // Also check if the directory itself is a bare git repo with master ref
    if dir.join("HEAD").is_file()
        && dir.join("objects").is_dir()
        && dir.join("refs/heads/master").is_file()
    {
        return Ok(Some(PublicInbox {
            name,
            version: "V1 (bare)".to_string(),
            git_dir: dir.to_path_buf(),
        }));
    }

    Ok(None)
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
    let commits_to_process: Vec<_> = all_commit_ids
        .into_iter()
        .take(count)
        .collect();

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
