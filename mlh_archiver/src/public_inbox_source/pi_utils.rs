use crate::errors;
use std::path::{Path, PathBuf};

/// Represents a detected public-inbox directory.
pub struct PublicInbox {
    /// Display name of the inbox
    pub name: String,
    /// V1 or V2
    pub version: String,
    /// Path to the git repository containing the emails
    pub git_dir: PathBuf,
}

/// Scans the base directory for public-inbox subdirectories.
pub fn find_public_inboxes(base_dir: &Path) -> errors::Result<Vec<PublicInbox>> {
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
fn detect_inbox(dir: &Path) -> errors::Result<Option<PublicInbox>> {
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
