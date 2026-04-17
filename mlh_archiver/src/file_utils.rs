//! File I/O utilities for the MLH Archiver.
//!
//! This module provides helper functions for:
//! - Writing email content to files
//! - Appending to error log files
//! - Reading/writing YAML configuration files
//! - Creating directories as needed

use serde::de::DeserializeOwned;
use serde::ser::{self};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, LineWriter, Write},
    path::Path,
};

/// Writes lines of text to a file, creating parent directories as needed.
///
/// This function is used to store fetched emails as `.eml` files.
///
/// # Arguments
///
/// * `file_path` - Path to the output file
/// * `lines` - Iterator over lines to write (without newlines)
///
/// # Returns
///
/// * `Ok(())` on success
/// * `Err(io::Error)` on failure
///
/// # Side Effects
///
/// - Creates parent directories if they don't exist
/// - Truncates existing file
pub fn write_lines_file<I, L>(file_path: &Path, lines: I) -> io::Result<()>
where
    I: IntoIterator<Item = L>,
    L: AsRef<str>,
{
    // Create or open (truncate) a file for writing
    // check if parent folder need to be created first
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = File::create(file_path)?;
    let mut file = LineWriter::new(file);

    for line in lines {
        write!(file, "{}", line.as_ref())?;
    }

    file.flush()?;

    log::debug!("file written {}", file_path.to_str().unwrap());

    Ok(())
}

/// Appends a single line to a file, creating parent directories as needed.
///
/// This function is used to log unavailable articles to `__errors.csv` files.
///
/// # Arguments
///
/// * `file_path` - Path to the output file
/// * `line` - Line to append (newline added automatically)
///
/// # Returns
///
/// * `Ok(())` on success
/// * `Err(io::Error)` on failure
pub fn append_line_to_file(file_path: &Path, line: &str) -> io::Result<()> {
    // check if parent folder need to be created first
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Open the file in append mode, creating it if it doesn't exist
    let mut file = OpenOptions::new()
        .append(true) // Enable append mode
        .create(true) // Create the file if it doesn't exist
        .open(file_path)?; // Open the file and handle potential errors

    // Write the line to the file
    writeln!(file, "{}", line)?;

    log::debug!(
        "Line appended successfully to {}",
        file_path.to_str().unwrap_or("")
    );

    Ok(())
}

/// Attempts to read a number from a file.
///
/// Reads the file content and parses the first valid number found.
/// Used to read `__progress.yaml` files for progress tracking.
///
/// # Arguments
///
/// * `path` - Path to the file to read
///
/// # Returns
///
/// * `Ok(usize)` - The parsed number
/// * `Err(io::Error)` - File not found, unreadable, or no valid number
pub fn try_read_number(path: &Path) -> Result<usize, io::Error> {
    // Attempt to read the file's content into a string.
    let content = fs::read_to_string(path)?;
    // The file was read successfully

    log::info!("Successfully read file: {}", path.display());
    // Trim whitespace and parse the content into an usize integer.
    // If parsing fails, return a custom error.
    let parts = content.trim().split(" ");
    for part in parts {
        let number = part.trim().parse::<usize>();
        if let Ok(num) = number {
            return Ok(num);
        }
    }
    Err(io::Error::other("failed reading  last status"))
}

/// Writes a serializable value to a YAML file.
///
/// Used for persisting progress tracking data (`__progress.yaml`)
/// and user list selections.
///
/// # Arguments
///
/// * `file_name` - Path to the output file
/// * `value` - Value to serialize (must implement `serde::Serialize`)
///
/// # Returns
///
/// * `Ok(())` on success
/// * `Err(io::Error)` on failure
///
/// # Side Effects
///
/// - Creates parent directories if needed
/// - Appends to file (does not truncate)
pub fn write_yaml<T>(file_name: &str, value: &T) -> io::Result<()>
where
    T: ?Sized + ser::Serialize,
{
    // check if parent folder need to be created first
    let file_path = Path::new(file_name);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(file_path)?;

    serde_yaml::to_writer(f, value).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(())
}

/// Appends a serializable value to a YAML file as a new document.
///
/// Used for streaming-append operations where multiple YAML documents
/// should be stored in a single file (e.g., append-only event logs).
///
/// Each value is preceded by a `---` document separator. If the file
/// is newly created, no leading separator is written (the first document
/// starts directly).
///
/// # Arguments
///
/// * `file_name` - Path to the output file
/// * `value` - Value to serialize (must implement `serde::Serialize`)
///
/// # Returns
///
/// * `Ok(())` on success
/// * `Err(io::Error)` on failure
///
/// # Side Effects
///
/// - Creates parent directories if needed
/// - Appends `---\n` + serialized value to the file
///
/// # Example
///
/// ```rust,no_run
/// use mlh_archiver::file_utils::append_yaml_to_file;
///
/// #[derive(serde::Serialize)]
/// struct Event { id: usize, msg: String }
///
/// append_yaml_to_file("./events.yaml", &Event { id: 1, msg: "first".to_string() }).unwrap();
/// append_yaml_to_file("./events.yaml", &Event { id: 2, msg: "second".to_string() }).unwrap();
/// // File content:
/// // id: 1
/// // msg: first
/// // ---
/// // id: 2
/// // msg: second
/// ```
pub fn append_yaml_to_file<T>(file_name: &str, value: &T) -> io::Result<()>
where
    T: ?Sized + ser::Serialize,
{
    let file_path = Path::new(file_name);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut f = OpenOptions::new()
        .append(true)
        .create(true)
        .open(file_path)?;

    // Check if file is empty — only add separator for subsequent documents
    if f.metadata().map(|m| m.len() > 0).unwrap_or(false) {
        writeln!(f, "---").map_err(io::Error::other)?;
    }

    serde_yaml::to_writer(&mut f, value)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(())
}

/// Reads and deserializes a value from a YAML file.
///
/// Used for reading progress tracking data (`__progress.yaml`).
///
/// # Arguments
///
/// * `file_name` - Path to the YAML file
///
/// # Returns
///
/// * `Ok(T)` - Deserialized value
/// * `Err(io::Error)` - File not found, unreadable, or invalid YAML
pub fn read_yaml<T>(file_name: &str) -> io::Result<T>
where
    T: DeserializeOwned,
{
    let yaml_content = fs::read_to_string(file_name)?;
    let res: T = serde_yaml::from_str(&yaml_content)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    return Ok(res);
}

/// Ensures a folder exists, creating it if necessary.
///
/// This function is idempotent - calling it multiple times with the
/// same path is safe and has no side effects after the first call.
///
/// # Arguments
///
/// * `folder_path` - Path to the folder to create/check
///
/// # Returns
///
/// * `Ok(())` on success
/// * `Err(io::Error)` on failure
///
/// # Side Effects
///
/// - Creates folder and all parent directories if they don't exist
/// - Logs debug/warn messages about folder existence
pub fn check_or_create_folder(folder_path: String) -> io::Result<()> {
    let p = Path::new(&folder_path);
    if p.exists() {
        log::debug!("Folder '{}' definitely exists.", folder_path);
    } else {
        fs::create_dir_all(&folder_path)?;
        log::warn!(
            "Folder '{}' does not exist (this should not happen after create_dir_all).",
            folder_path
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestDoc {
        id: usize,
        msg: String,
    }

    #[test]
    fn test_append_yaml_creates_file_with_first_doc() {
        let path = "/tmp/test_append_yaml_first.yaml";
        let _ = fs::remove_file(path);

        append_yaml_to_file(
            path,
            &TestDoc {
                id: 1,
                msg: "first".to_string(),
            },
        )
        .unwrap();

        let content = fs::read_to_string(path).unwrap();
        // First document should NOT have a leading ---
        assert!(!content.starts_with("---"));
        assert!(content.contains("id: 1"));
        assert!(content.contains("msg: first"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_append_yaml_adds_separator_for_second_doc() {
        let path = "/tmp/test_append_yaml_second.yaml";
        let _ = fs::remove_file(path);

        append_yaml_to_file(
            path,
            &TestDoc {
                id: 1,
                msg: "first".to_string(),
            },
        )
        .unwrap();
        append_yaml_to_file(
            path,
            &TestDoc {
                id: 2,
                msg: "second".to_string(),
            },
        )
        .unwrap();

        let content = fs::read_to_string(path).unwrap();
        // Should contain a document separator
        assert!(content.contains("\n---\n"));
        assert!(content.contains("id: 1"));
        assert!(content.contains("id: 2"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_append_yaml_creates_parent_dirs() {
        let path = "/tmp/test_append_yaml_nested/deep/dir/doc.yaml";
        let _ = fs::remove_dir_all("/tmp/test_append_yaml_nested");

        append_yaml_to_file(
            path,
            &TestDoc {
                id: 1,
                msg: "test".to_string(),
            },
        )
        .unwrap();
        assert!(Path::new(path).exists());

        let _ = fs::remove_dir_all("/tmp/test_append_yaml_nested");
    }

    #[test]
    fn test_append_yaml_multi_doc_roundtrip() {
        let path = "/tmp/test_append_yaml_roundtrip.yaml";
        let _ = fs::remove_file(path);

        let docs = vec![
            TestDoc {
                id: 1,
                msg: "one".to_string(),
            },
            TestDoc {
                id: 2,
                msg: "two".to_string(),
            },
            TestDoc {
                id: 3,
                msg: "three".to_string(),
            },
        ];
        for doc in &docs {
            append_yaml_to_file(path, doc).unwrap();
        }

        // Read back — split on --- to get individual documents
        let content = fs::read_to_string(path).unwrap();
        let parts: Vec<&str> = content.split("\n---\n").collect();
        assert_eq!(parts.len(), 3);

        let first: TestDoc = serde_yaml::from_str(parts[0]).unwrap();
        assert_eq!(first, docs[0]);

        let second: TestDoc = serde_yaml::from_str(parts[1]).unwrap();
        assert_eq!(second, docs[1]);

        let third: TestDoc = serde_yaml::from_str(parts[2]).unwrap();
        assert_eq!(third, docs[2]);

        // Verify separator count (n-1 separators for n documents)
        assert_eq!(content.matches("\n---\n").count(), 2);

        let _ = fs::remove_file(path);
    }
}
