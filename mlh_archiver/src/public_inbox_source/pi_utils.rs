use crate::errors;
use std::fmt;
use std::path::{Path, PathBuf};

use gix::bstr::ByteSlice;
use gix::revision::walk::Info;

/// Represents a detected public-inbox directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicInbox {
    /// Display name of the inbox
    pub name: String,
    /// V1 or V2
    pub version: String,
    /// Path to the git repository containing the emails
    pub git_dir: PathBuf,
}

/// Represents a single epoch within a V2 public inbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpochRepo {
    /// Epoch name derived from directory name (e.g., "0", "1", "all")
    pub epoch_name: String,
    /// Path to the epoch's git repository
    pub git_dir: PathBuf,
}

/// Display implementation for RunModeConfig does not need to provide every field
/// It it used in the data-lineage module to save info about how it was used
impl fmt::Display for PublicInbox {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
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

/// Check if a directory is a git repository (has HEAD and objects).
fn is_git_repo(dir: &Path) -> bool {
    dir.join("HEAD").is_file() && dir.join("objects").is_dir()
}

/// Check if a git repository has a master ref (either in refs/heads/master or packed-refs).
fn has_master_ref(dir: &Path) -> bool {
    if dir.join("refs/heads/master").is_file() {
        return true;
    }
    // Check packed-refs for master ref
    if let Ok(content) = std::fs::read_to_string(dir.join("packed-refs")) {
        content.lines().any(|line| {
            let line = line.trim();
            !line.starts_with('#') && line.ends_with(" refs/heads/master")
        })
    } else {
        false
    }
}

/// Finds an epoch repository (git/*.git) that contains the master ref.
/// Returns the path to the epoch repo if found, otherwise None.
fn find_epoch_repo_with_master(git_dir: &Path) -> crate::Result<Option<PathBuf>> {
    for entry in std::fs::read_dir(git_dir)? {
        let entry = entry?;
        let epoch_path = entry.path();
        if epoch_path.is_dir() {
            let epoch_name = epoch_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            // Check if it ends with .git (like 0.git) and is a git repo with master ref
            if epoch_name.ends_with(".git")
                && is_git_repo(&epoch_path)
                && has_master_ref(&epoch_path)
            {
                return Ok(Some(epoch_path));
            }
        }
    }
    Ok(None)
}

/// Check if a git repository has any objects (non-empty objects directory).
fn has_objects(dir: &Path) -> bool {
    let objects_dir = dir.join("objects");
    if !objects_dir.is_dir() {
        return false;
    }
    // Check if objects directory has any files (excluding info/ and pack/)
    match std::fs::read_dir(&objects_dir) {
        Ok(mut entries) => entries.any(|e| e.is_ok()),
        Err(_) => false,
    }
}

/// Detects if a directory is a public-inbox (V1 or V2) and returns its info.
fn detect_inbox(dir: &Path) -> crate::Result<Option<PublicInbox>> {
    let name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Check for V2: has git/ directory with numbered epoch repos (git/0.git, git/1.git, etc.)
    // and optionally an all.git that chains them via git alternates.
    let git_dir = dir.join("git");
    if git_dir.is_dir() {
        // First, try to find an epoch repo with master ref and objects
        if let Some(epoch_repo) = find_epoch_repo_with_master(&git_dir)?
            && has_objects(&epoch_repo)
        {
            return Ok(Some(PublicInbox {
                name,
                version: "V2".to_string(),
                git_dir: epoch_repo,
            }));
        }

        // If no epoch repo with master found, check for all.git with alternates
        let all_git = dir.join("all.git");
        if all_git.is_dir() && is_git_repo(&all_git) {
            // Check if all.git has alternates pointing to git/*.git
            let alternates_path = all_git.join("objects/info/alternates");
            if alternates_path.is_file() {
                // Try to read alternates to find which epoch repo to use
                if let Ok(alternates_content) = std::fs::read_to_string(&alternates_path) {
                    for line in alternates_content.lines() {
                        let alt_path = line.trim();
                        if alt_path.ends_with("/objects") {
                            // Extract the git repo path (remove /objects suffix)
                            if let Some(parent) = Path::new(alt_path).parent()
                                && parent.is_dir()
                                && is_git_repo(parent)
                                && has_master_ref(parent)
                                && has_objects(parent)
                            {
                                return Ok(Some(PublicInbox {
                                    name,
                                    version: "V2 (alternates)".to_string(),
                                    git_dir: parent.to_path_buf(),
                                }));
                            }
                        }
                    }
                }
                // If can't find via alternates, check if all.git itself has objects and refs
                if has_objects(&all_git) && has_master_ref(&all_git) {
                    return Ok(Some(PublicInbox {
                        name,
                        version: "V2 (combined)".to_string(),
                        git_dir: all_git,
                    }));
                } else {
                    // Incomplete: has all.git but missing objects or refs
                    return Ok(Some(PublicInbox {
                        name,
                        version: "V2 (incomplete)".to_string(),
                        git_dir: all_git,
                    }));
                }
            } else {
                // all.git without alternates - treat as V1 style
                if has_objects(&all_git) && has_master_ref(&all_git) {
                    return Ok(Some(PublicInbox {
                        name,
                        version: "V1".to_string(),
                        git_dir: all_git,
                    }));
                } else if has_objects(&all_git) {
                    // Has objects but no master ref - might be empty
                    return Ok(Some(PublicInbox {
                        name,
                        version: "V1 (empty)".to_string(),
                        git_dir: all_git,
                    }));
                }
            }
        }
        // git_dir exists but no valid repo found
        return Ok(None);
    }

    // No git/ directory - check for V1 layouts

    // Check for V1: single bare git repo at the inbox directory itself
    // (or an all.git that IS the main repo, not using alternates)
    let all_git = dir.join("all.git");
    if all_git.is_dir() && is_git_repo(&all_git) && has_master_ref(&all_git) {
        return Ok(Some(PublicInbox {
            name,
            version: "V1".to_string(),
            git_dir: all_git,
        }));
    }

    // Also check if the directory itself is a bare git repo with master ref
    if is_git_repo(dir) && has_master_ref(dir) {
        return Ok(Some(PublicInbox {
            name,
            version: "V1 (bare)".to_string(),
            git_dir: dir.to_path_buf(),
        }));
    }

    // Finally, check for all.git without master ref (might be empty repo)
    if all_git.is_dir() && is_git_repo(&all_git) {
        // Even without master ref, could be a public-inbox (empty)
        return Ok(Some(PublicInbox {
            name,
            version: "V1 (empty)".to_string(),
            git_dir: all_git,
        }));
    }

    Ok(None)
}

/// Get commit at given position (0-indexed from newest).
pub fn get_commit_at_position<'a>(
    repo: &'a gix::Repository,
    position: usize,
) -> crate::Result<Info<'a>> {
    let head_ref = repo.refs.find("refs/heads/master")?;
    let head_id = head_ref
        .target
        .try_id()
        .ok_or_else(|| anyhow::anyhow!("refs/heads/master does not point to an object"))?
        .to_owned();

    let walk = repo.rev_walk([head_id]);
    let iter = walk.all().map_err(|e| anyhow::anyhow!(e))?;
    for (i, info) in iter.enumerate() {
        if i == position {
            return Ok(info.map_err(|e| anyhow::anyhow!(e))?);
        }
    }
    Err(crate::errors::Error::Config(
        crate::errors::ConfigError::MissingHostname,
    ))
}

/// Extract email content from a commit.
/// Returns (commit_hash, raw_email).
pub fn extract_email_from_commit(
    repo: &gix::Repository,
    commit: &gix::Commit,
) -> crate::Result<(String, String)> {
    let commit_ref = commit.decode()?;
    let tree_id = commit_ref.tree();
    let tree = repo.find_tree(tree_id)?;

    let blob_oid = tree
        .iter()
        .find_map(|e| e.ok())
        .filter(|e| e.filename().as_bytes() == b"m")
        .map(|e| e.object_id());

    match blob_oid {
        Some(blob_oid) => {
            let raw_body = read_by_blob_id(repo, blob_oid)?;
            Ok((commit.id().to_string(), raw_body))
        }
        None => Err(crate::errors::Error::Config(
            crate::errors::ConfigError::MissingHostname,
        )),
    }
}

pub fn read_by_blob_id(repo: &gix::Repository, blob_oid: gix::ObjectId) -> crate::Result<String> {
    let blob = repo.find_blob(blob_oid)?;
    let raw_email = String::from_utf8_lossy(&blob.data).to_string();
    return Ok(raw_email);
}

/// Finds all epoch repositories within a V2 public inbox's git/ directory.
/// Returns a sorted Vec<EpochRepo> with numbered epochs first, then "all" last.
pub fn find_epochs(git_dir: &Path) -> crate::Result<Vec<EpochRepo>> {
    let mut epochs = Vec::new();

    if !git_dir.is_dir() {
        return Ok(epochs);
    }

    for entry in std::fs::read_dir(git_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if !name.ends_with(".git") {
            continue;
        }

        if !is_git_repo(&path) || !has_master_ref(&path) {
            continue;
        }

        let epoch_name = name.strip_suffix(".git").unwrap_or(name).to_string();
        epochs.push(EpochRepo {
            epoch_name,
            git_dir: path,
        });
    }

    epochs.sort_by(|a, b| {
        let a_is_all = a.epoch_name == "all";
        let b_is_all = b.epoch_name == "all";
        if a_is_all && !b_is_all {
            return std::cmp::Ordering::Greater;
        }
        if !a_is_all && b_is_all {
            return std::cmp::Ordering::Less;
        }
        a.epoch_name.cmp(&b.epoch_name)
    });

    Ok(epochs)
}

/// Counts the total number of commits in a repository from refs/heads/master.
pub fn count_commits(repo: &gix::Repository) -> crate::Result<usize> {
    let head_ref = repo.refs.find("refs/heads/master")?;
    let head_id = head_ref
        .target
        .try_id()
        .ok_or_else(|| anyhow::anyhow!("refs/heads/master does not point to an object"))?
        .to_owned();

    let count = repo
        .rev_walk([head_id])
        .all()?
        .filter_map(|r| r.ok())
        .count();

    Ok(count)
}

/// Formats an email ID from its sequential number, epoch name, and commit SHA.
/// Format: "{padded_id}-e{epoch}-{short_sha}"
/// Example: "0000000001-e1-d3ed66e"
pub fn format_email_id(email_num: usize, epoch_name: &str, commit_sha: &str) -> String {
    let padded = format!("{:010}", email_num);
    let short_sha = if commit_sha.len() >= 7 {
        &commit_sha[..7]
    } else {
        commit_sha
    };
    format!("{}-e{}-{}", padded, epoch_name, short_sha)
}

/// Parsed components of a formatted email ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedEmailId {
    pub email_num: usize,
    pub epoch_name: String,
    pub short_sha: String,
}

/// Parses a formatted email ID back into its components.
/// Format: "{padded_id}-e{epoch}-{short_sha}"
/// Returns None if the format doesn't match.
pub fn parse_email_id(id: &str) -> Option<ParsedEmailId> {
    let parts: Vec<&str> = id.splitn(3, '-').collect();
    if parts.len() != 3 {
        return None;
    }

    let email_num = parts[0].parse::<usize>().ok()?;

    let epoch_and_sha = parts[1];
    if !epoch_and_sha.starts_with('e') {
        return None;
    }
    let epoch_name = epoch_and_sha[1..].to_string();

    let short_sha = parts[2].to_string();

    Some(ParsedEmailId {
        email_num,
        epoch_name,
        short_sha,
    })
}

/// Collects all commit IDs from a repository, ordered from newest to oldest.
pub fn collect_all_commits(repo: &gix::Repository) -> crate::Result<Vec<gix::ObjectId>> {
    let head_ref = repo.refs.find("refs/heads/master")?;
    let head_id = head_ref
        .target
        .try_id()
        .ok_or_else(|| anyhow::anyhow!("refs/heads/master does not point to an object"))?
        .to_owned();

    let commits: Vec<_> = repo
        .rev_walk([head_id])
        .all()?
        .filter_map(|r| r.ok())
        .map(|info| info.id)
        .collect();

    Ok(commits)
}
