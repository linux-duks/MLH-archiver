#![allow(dead_code)]
// why cant clippy not find these functions being used in other test files ?

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn list_files_with_extension(directory: &str, extension: &str) -> Vec<PathBuf> {
    let ext = if extension.starts_with('.') {
        extension.to_string()
    } else {
        format!(".{}", extension)
    };

    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let dir = base.join(directory.trim_start_matches("./"));

    let dot_ext = ext;
    let ext_without_dot = &dot_ext[1..];

    let mut files: Vec<PathBuf> = match fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|e| e.to_string_lossy() == ext_without_dot)
            })
            .filter(|e| e.file_type().is_ok_and(|ft| ft.is_file()))
            .map(|e| e.path())
            .collect(),
        Err(_) => Vec::new(),
    };
    files.sort();
    files
}

pub fn map_to_file_extensions(email_file_path: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let stem = email_file_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let parent = email_file_path.parent().unwrap_or(Path::new(""));

    extensions
        .iter()
        .map(|ext| parent.join(format!("{}{}", stem, ext)))
        .collect()
}

pub fn parse_date_file(date_file: &Path) -> String {
    match fs::read_to_string(date_file) {
        Ok(content) => content.lines().next().unwrap_or("").trim().to_string(),
        Err(_) => String::new(),
    }
}

pub fn parse_body_file(body_file: &Path) -> String {
    match fs::read_to_string(body_file) {
        Ok(content) => content.replace("\r\n", "\n"),
        Err(_) => String::new(),
    }
}

pub fn parse_headers_file(headers_file: &Path) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    let content = match fs::read_to_string(headers_file) {
        Ok(c) => c.replace("\r\n", "\n"),
        Err(_) => return headers,
    };

    let mut current_header: Option<String> = None;
    let mut current_value = String::new();

    for line in content.lines() {
        let line = line.to_string();

        if line.trim().is_empty() || line.starts_with("--") {
            // End of headers
            if let Some(ref key) = current_header {
                headers.insert(key.clone(), current_value.clone());
            }
            break;
        }

        // Check for continuation line (starts with space/tab)
        if line.starts_with(' ') || line.starts_with('\t') {
            if current_header.is_some() {
                current_value.push(' ');
                current_value.push_str(line.trim());
            }
            continue;
        }

        // Save previous header
        if let Some(ref key) = current_header {
            headers.insert(key.clone(), current_value.clone());
        }

        // Parse new header
        if let Some(colon_pos) = line.find(':') {
            current_header = Some(line[..colon_pos].trim().to_lowercase());
            current_value = line[colon_pos + 1..].trim().to_string();
        } else {
            current_header = None;
            current_value = String::new();
        }
    }

    headers
}
