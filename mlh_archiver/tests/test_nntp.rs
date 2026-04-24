use std::io;
use std::path::Path;
use std::sync::{Arc, atomic::AtomicBool};
use std::{fs, thread, vec};
use testcontainers::{
    Container, GenericBuildableImage, GenericImage, core::WaitFor, runners::SyncBuilder,
    runners::SyncRunner,
};

use mlh_archiver::config::AppConfig;
use mlh_archiver::nntp_source::nntp_config::NntpConfig;
use mlh_archiver::start;
use walkdir::WalkDir;

fn file_list_dir(path: String) -> Vec<String> {
    let mut file_list = vec![];

    for file in WalkDir::new(path).into_iter().filter_map(|file| file.ok()) {
        println!("{}", file.path().display());
        file_list.push(file.path().display().to_string());
    }

    file_list
}

pub fn check_and_delete_folder(folder_path: String) -> io::Result<()> {
    let p = Path::new(&folder_path);
    if p.exists() {
        println!("Clearing outpur dir");
        fs::remove_dir_all(&folder_path).unwrap();
    }
    Ok(())
}

/// Builds the test NNTP server Docker image.
fn build_test_nntp_image() -> GenericImage {
    println!("loading test_nntp_server/Containerfile");
    GenericBuildableImage::new("test_nntp_server", "latest")
        .with_dockerfile("./tests/test_nntp_server/Containerfile")
        .with_file("./tests/test_nntp_server", ".")
        .build_image()
        .unwrap()
}

/// Starts the test NNTP server container.
/// Returns (container, host_port) tuple.
fn start_test_nntp_container(image: GenericImage) -> (Container<GenericImage>, u16) {
    let container = image
        .with_wait_for(WaitFor::message_on_stdout("Serving on port :8119"))
        .start()
        .unwrap();
    let host_port = container.get_host_port_ipv4(8119).unwrap();
    (container, host_port)
}

/// Runs an NNTP archiver test with a custom configuration builder.
/// Returns the list of files created in the output directory.
fn run_nntp_test_with_config<F>(config_builder: F, test_name: &str) -> Vec<String>
where
    F: FnOnce(u16) -> AppConfig,
{
    let image = build_test_nntp_image();
    let (container, host_port) = start_test_nntp_container(image);

    let output_dir = format!("./test_nntp_output_{}", test_name);
    check_and_delete_folder(output_dir.clone()).unwrap();

    let mut app_config = config_builder(host_port);
    app_config.output_dir = output_dir.clone();

    println!("Starting worker for {}", test_name);
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let test_name_owned = test_name.to_string();

    let child_handle = thread::spawn(move || {
        println!("Child thread started for {}.", test_name_owned);
        let result = start(&mut app_config, shutdown_flag);
        assert!(result.is_ok());
        println!("Child thread stopped for {}.", test_name_owned);
    });

    println!("waiting server thread to finish for {}", test_name);
    child_handle.join().expect("Child thread panicked");
    container.stop().unwrap();
    container.rm().unwrap();

    println!("Loading list of files for {}", test_name);
    file_list_dir(output_dir.clone())
}

/// Validates the content of a `__progress.yaml` file.
///
/// Reads the YAML file and verifies:
/// - The file exists and contains `last_email` field
/// - The `last_email` value matches the expected maximum article ID
fn validate_progress_file(path: &str, expected_last_email: usize) {
    let content = fs::read_to_string(path).expect("Progress file should exist");
    assert!(
        content.contains("last_email:"),
        "Progress file should contain 'last_email' field: {}",
        path
    );
    // Parse the YAML content properly using serde_yaml
    let yaml_value: serde_yaml::Value =
        serde_yaml::from_str(&content).expect("Failed to parse YAML content");
    let last_email_str = yaml_value
        .get("last_email")
        .expect("YAML should have last_email field")
        .as_str()
        .expect("last_email should be a string");
    let last_email: usize = last_email_str
        .parse()
        .expect("Failed to parse last_email as usize");
    assert_eq!(
        last_email, expected_last_email,
        "Progress file {} should have last_email={}",
        path, expected_last_email
    );
}

/// Validates the content of a `__lineage.yaml` file.
///
/// Reads the multi-document YAML file and verifies:
/// - The file exists and contains expected number of lineage entries
/// - Each entry has: email_index, list_name, source_type, timestamp, archiver_build_info
/// - The email_index values match the expected article IDs (in order)
fn validate_lineage_file(path: &str, expected_list_name: &str, expected_email_indices: &[usize]) {
    let content = fs::read_to_string(path).expect("Lineage file should exist");

    // Verify source_type contains "NNTP"
    assert!(
        content.contains("source_type:"),
        "Lineage file should contain 'source_type' field: {}",
        path
    );
    assert!(
        content.contains("NNTP"),
        "Lineage file source_type should contain 'NNTP': {}",
        path
    );

    // Verify list_name
    assert!(
        content.contains(expected_list_name),
        "Lineage file should have list_name={}: {}",
        expected_list_name,
        path
    );

    // Verify timestamp exists
    assert!(
        content.contains("timestamp:"),
        "Lineage file should contain 'timestamp' field: {}",
        path
    );

    // Verify archiver_build_info exists and is non-empty
    assert!(
        content.contains("archiver_build_info:"),
        "Lineage file should contain 'archiver_build_info' field: {}",
        path
    );

    // Verify email_index values match expected
    for &email_index in expected_email_indices {
        let unquoted = format!("email_index: {}", email_index);
        let quoted = format!("email_index: '{}'", email_index);
        assert!(
            content.contains(&unquoted) || content.contains(&quoted),
            "Lineage file should contain email_index={}: {}",
            email_index,
            path
        );
    }

    // Verify count of entries
    let entry_count = content.matches("email_index:").count();
    assert_eq!(
        entry_count,
        expected_email_indices.len(),
        "Lineage file should have {} entries, found {}: {}",
        expected_email_indices.len(),
        entry_count,
        path
    );
}

// =============================================================================
// Expected file list helpers
// =============================================================================

/// Returns the root output directory path as a single-element vector.
fn root_dir(dir: &str) -> Vec<String> {
    vec![dir.to_string()]
}

/// Generates all expected file paths for a single mailing list.
///
/// Always includes:
/// - The list directory
///
/// Conditionally includes:
/// - `__progress.yaml` — if `articles` is non-empty (created by `archive_email`)
/// - `__lineage.yaml` — if `articles` is non-empty
/// - `{N}.eml` — for each N in `articles`
/// - `__errors.csv` — if `has_errors` is true
fn list_entry(dir: &str, list_name: &str, articles: &[usize], has_errors: bool) -> Vec<String> {
    let mut files = vec![format!("{}/{}", dir, list_name)];

    // Progress and lineage files only exist when at least one article was fetched
    if !articles.is_empty() {
        files.push(format!("{}/{}/__progress.yaml", dir, list_name));
        files.push(format!("{}/{}/__lineage.yaml", dir, list_name));
    }

    // Article files
    for &n in articles {
        files.push(format!("{}/{}/{}.eml", dir, list_name, n));
    }

    // Errors file
    if has_errors {
        files.push(format!("{}/{}/__errors.csv", dir, list_name));
    }

    files
}

/// Validates both progress and lineage files for a mailing list.
///
/// Checks `__progress.yaml` has the expected `last_email` value,
/// and `__lineage.yaml` contains the expected article indices.
/// Skips all validation for empty article lists (no files created).
fn validate_list(dir: &str, list_name: &str, articles: &[usize]) {
    if articles.is_empty() {
        return;
    }
    let max_article = articles.iter().copied().max().unwrap();
    validate_progress_file(
        &format!("{}/{}/__progress.yaml", dir, list_name),
        max_article,
    );
    validate_lineage_file(
        &format!("{}/{}/__lineage.yaml", dir, list_name),
        list_name,
        articles,
    );
}

// =============================================================================
// default mode Integration Tests
// =============================================================================

#[test]
fn test_read_from_local_nntp_server() {
    let found_files = run_nntp_test_with_config(
        |host_port| {
            AppConfig {
                output_dir: "".to_string(), // will be overwritten by helper
                nthreads: 1,
                loop_groups: false,
                nntp: Some(NntpConfig {
                    hostname: "localhost".to_owned(),
                    port: Some(host_port),
                    group_lists: Some(vec!["*".to_owned()]),
                    ..NntpConfig::default()
                }),
                ..Default::default()
            }
        },
        "default",
    );

    let output_dir = "./test_nntp_output_default";

    let mut found_files = found_files;
    let mut expected_files = [
        root_dir(output_dir),
        list_entry(output_dir, "test.groups.foo", &[1, 2], false),
        list_entry(output_dir, "test.groups.bar", &[1, 2], false),
        list_entry(
            output_dir,
            "test.groups.synthetic",
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
            false,
        ),
    ]
    .concat();
    found_files.sort();
    expected_files.sort();
    assert_eq!(found_files, expected_files);

    // Validate progress and lineage file content
    validate_list(output_dir, "test.groups.foo", &[1, 2]);
    validate_list(output_dir, "test.groups.bar", &[1, 2]);
    validate_list(
        output_dir,
        "test.groups.synthetic",
        &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
    );

    check_and_delete_folder(output_dir.to_string()).unwrap();
}

// =============================================================================
// Range Variation Integration Tests
// =============================================================================

#[test]
fn test_read_single_article_by_range() {
    let found_files = run_nntp_test_with_config(
        |host_port| {
            AppConfig {
                output_dir: "".to_string(), // will be overwritten by helper
                nthreads: 1,
                loop_groups: false,
                nntp: Some(NntpConfig {
                    hostname: "localhost".to_owned(),
                    port: Some(host_port),
                    group_lists: Some(vec!["*".to_owned()]),
                    article_range: Some("5".to_owned()),
                    ..NntpConfig::default()
                }),
                ..Default::default()
            }
        },
        "single",
    );

    // Only article 5 should be fetched (only exists in synthetic list)
    // Other lists will have __errors.csv files because article 5 doesn't exist
    let mut expected_files = [
        root_dir("./test_nntp_output_single"),
        list_entry("./test_nntp_output_single", "test.groups.foo", &[], true),
        list_entry("./test_nntp_output_single", "test.groups.bar", &[], true),
        list_entry("./test_nntp_output_single", "test.groups.empty", &[], true),
        list_entry(
            "./test_nntp_output_single",
            "test.groups.synthetic",
            &[5],
            false,
        ),
    ]
    .concat();
    let mut found_files = found_files;
    found_files.sort();
    expected_files.sort();
    assert_eq!(found_files, expected_files);

    // Validate progress and lineage
    validate_list("./test_nntp_output_single", "test.groups.foo", &[]);
    validate_list("./test_nntp_output_single", "test.groups.bar", &[]);
    validate_list("./test_nntp_output_single", "test.groups.empty", &[]);
    validate_list("./test_nntp_output_single", "test.groups.synthetic", &[5]);

    check_and_delete_folder("./test_nntp_output_single".to_string()).unwrap();
}

#[test]
fn test_read_article_range() {
    let found_files = run_nntp_test_with_config(
        |host_port| {
            AppConfig {
                output_dir: "".to_string(), // will be overwritten by helper
                nthreads: 1,
                loop_groups: false,
                nntp: Some(NntpConfig {
                    hostname: "localhost".to_owned(),
                    port: Some(host_port),
                    group_lists: Some(vec!["*".to_owned()]),
                    article_range: Some("1-3".to_owned()),
                    ..NntpConfig::default()
                }),
                ..Default::default()
            }
        },
        "range",
    );

    // Articles 1, 2, 3 should be fetched from each list
    // foo has 2 articles (1, 2), bar has 2 (1, 2), synthetic has 3 (1, 2, 3)
    // Lists with unavailable articles will also have __errors.csv files
    let mut expected_files = [
        root_dir("./test_nntp_output_range"),
        list_entry("./test_nntp_output_range", "test.groups.foo", &[1, 2], true),
        list_entry("./test_nntp_output_range", "test.groups.bar", &[1, 2], true),
        list_entry("./test_nntp_output_range", "test.groups.empty", &[], true),
        list_entry(
            "./test_nntp_output_range",
            "test.groups.synthetic",
            &[1, 2, 3],
            false,
        ),
    ]
    .concat();
    let mut found_files = found_files;
    found_files.sort();
    expected_files.sort();
    assert_eq!(found_files, expected_files);

    // Validate progress and lineage
    validate_list("./test_nntp_output_range", "test.groups.foo", &[1, 2]);
    validate_list("./test_nntp_output_range", "test.groups.bar", &[1, 2]);
    validate_list("./test_nntp_output_range", "test.groups.empty", &[]);
    validate_list(
        "./test_nntp_output_range",
        "test.groups.synthetic",
        &[1, 2, 3],
    );

    check_and_delete_folder("./test_nntp_output_range".to_string()).unwrap();
}

#[test]
fn test_read_multiple_articles_by_range() {
    let found_files = run_nntp_test_with_config(
        |host_port| {
            AppConfig {
                output_dir: "".to_string(), // will be overwritten by helper
                nthreads: 1,
                loop_groups: false,
                nntp: Some(NntpConfig {
                    hostname: "localhost".to_owned(),
                    port: Some(host_port),
                    group_lists: Some(vec!["*".to_owned()]),
                    article_range: Some("1,5,10".to_owned()),
                    ..NntpConfig::default()
                }),
                ..Default::default()
            }
        },
        "multiple",
    );

    // Articles 1, 5, 10 should be fetched from each list
    // foo has 1 article (1), bar has 1 (1), synthetic has 3 (1, 5, 10)
    // Lists with unavailable articles will also have __errors.csv files
    let mut expected_files = [
        root_dir("./test_nntp_output_multiple"),
        list_entry("./test_nntp_output_multiple", "test.groups.foo", &[1], true),
        list_entry("./test_nntp_output_multiple", "test.groups.bar", &[1], true),
        list_entry(
            "./test_nntp_output_multiple",
            "test.groups.empty",
            &[],
            true,
        ),
        list_entry(
            "./test_nntp_output_multiple",
            "test.groups.synthetic",
            &[1, 5, 10],
            false,
        ),
    ]
    .concat();
    let mut found_files = found_files;
    found_files.sort();
    expected_files.sort();
    assert_eq!(found_files, expected_files);

    // Validate progress and lineage
    validate_list("./test_nntp_output_multiple", "test.groups.foo", &[1]);
    validate_list("./test_nntp_output_multiple", "test.groups.bar", &[1]);
    validate_list("./test_nntp_output_multiple", "test.groups.empty", &[]);
    validate_list(
        "./test_nntp_output_multiple",
        "test.groups.synthetic",
        &[1, 5, 10],
    );

    check_and_delete_folder("./test_nntp_output_multiple".to_string()).unwrap();
}

#[test]
fn test_read_mixed_range() {
    let found_files = run_nntp_test_with_config(
        |host_port| {
            AppConfig {
                output_dir: "".to_string(), // will be overwritten by helper
                nthreads: 1,
                loop_groups: false,
                nntp: Some(NntpConfig {
                    hostname: "localhost".to_owned(),
                    port: Some(host_port),
                    group_lists: Some(vec!["*".to_owned()]),
                    article_range: Some("1,3-5,10".to_owned()),
                    ..NntpConfig::default()
                }),
                ..Default::default()
            }
        },
        "mixed",
    );

    // Articles 1, 3, 4, 5, 10 should be fetched from each list
    // foo has 1 article (1), bar has 1 (1), synthetic has 5 (1, 3, 4, 5, 10)
    // Lists with unavailable articles will also have __errors.csv files
    let mut expected_files = [
        root_dir("./test_nntp_output_mixed"),
        list_entry("./test_nntp_output_mixed", "test.groups.foo", &[1], true),
        list_entry("./test_nntp_output_mixed", "test.groups.bar", &[1], true),
        list_entry("./test_nntp_output_mixed", "test.groups.empty", &[], true),
        list_entry(
            "./test_nntp_output_mixed",
            "test.groups.synthetic",
            &[1, 3, 4, 5, 10],
            false,
        ),
    ]
    .concat();
    let mut found_files = found_files;
    found_files.sort();
    expected_files.sort();
    assert_eq!(found_files, expected_files);

    // Validate progress and lineage
    validate_list("./test_nntp_output_mixed", "test.groups.foo", &[1]);
    validate_list("./test_nntp_output_mixed", "test.groups.bar", &[1]);
    validate_list("./test_nntp_output_mixed", "test.groups.empty", &[]);
    validate_list(
        "./test_nntp_output_mixed",
        "test.groups.synthetic",
        &[1, 3, 4, 5, 10],
    );

    check_and_delete_folder("./test_nntp_output_mixed".to_string()).unwrap();
}

// =============================================================================
// Authentication Integration Tests
// =============================================================================

#[test]
fn test_read_from_local_nntp_server_with_auth() {
    let found_files = run_nntp_test_with_config(
        |host_port| {
            AppConfig {
                output_dir: "".to_string(), // will be overwritten by helper
                nthreads: 1,
                loop_groups: false,
                nntp: Some(NntpConfig {
                    hostname: "localhost".to_owned(),
                    port: Some(host_port),
                    group_lists: Some(vec!["*.foo".to_owned()]),
                    username: Some("foo".to_owned()),
                    password: Some("bar".to_owned()),
                    ..NntpConfig::default()
                }),
                ..Default::default()
            }
        },
        "auth",
    );

    let mut found_files = found_files;
    let mut expected_files = [
        root_dir("./test_nntp_output_auth"),
        list_entry("./test_nntp_output_auth", "test.groups.foo", &[1, 2], false),
    ]
    .concat();
    found_files.sort();
    expected_files.sort();
    assert_eq!(found_files, expected_files);

    // Validate progress and lineage
    validate_list("./test_nntp_output_auth", "test.groups.foo", &[1, 2]);

    check_and_delete_folder("./test_nntp_output_auth".to_string()).unwrap();
}
