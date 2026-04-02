use std::io;
use std::path::Path;
use std::sync::{Arc, atomic::AtomicBool};
use std::{fs, thread, vec};
use testcontainers::{
    GenericBuildableImage, core::WaitFor, runners::SyncBuilder, runners::SyncRunner,
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

// =============================================================================
// default mode Integration Tests
// =============================================================================

#[test]
fn test_read_from_local_nntp_server() {
    println!("loading Containerfile");
    let image = GenericBuildableImage::new("test_nttp_server", "latest")
        .with_dockerfile("./tests/Containerfile")
        .with_file("./tests/test_nttp_server", "./test_nttp_server")
        .build_image()
        .unwrap();

    // Use the built image in containers
    let container = image
        // check log from server
        .with_wait_for(WaitFor::message_on_stdout("Serving on port :8119"))
        .start()
        .unwrap();

    // check if correct port is exmposed
    let host_port = container.get_host_port_ipv4(8119).unwrap();
    let output_dir = "./test_output".to_owned();

    println!("server container running on host port: {}", host_port);
    let mut app_config = AppConfig {
        output_dir: output_dir.clone(),
        nthreads: 1,
        loop_groups: false,
        nntp: Some(NntpConfig {
            hostname: "localhost".to_owned(),
            port: host_port,
            group_lists: Some(vec!["ALL".to_owned()]),
            article_range: None,
        }),
    };

    check_and_delete_folder(output_dir.clone()).unwrap();

    println!("Starting worker");

    // Create shutdown flag for the test
    let shutdown_flag = Arc::new(AtomicBool::new(false));

    let child_handle = thread::spawn(move || {
        println!("Child thread started.");
        let result = start(&mut app_config, shutdown_flag);
        assert!(result.is_ok());

        println!("Child thread stopped.");
    });

    println!("waiting server thread to finish");
    child_handle.join().expect("Child thread panicked");
    container.stop().unwrap();
    container.rm().unwrap();

    println!("Loading list of files");
    let mut found_files = file_list_dir(output_dir.clone());
    // TODO: read file list dynamically from mock db file
    let mut expected_files = vec![
        "./test_output",
        "./test_output/test.groups.foo",
        "./test_output/test.groups.foo/__last_article_number",
        "./test_output/test.groups.foo/1.eml",
        "./test_output/test.groups.foo/2.eml",
        "./test_output/test.groups.bar",
        "./test_output/test.groups.bar/__last_article_number",
        "./test_output/test.groups.bar/1.eml",
        "./test_output/test.groups.bar/2.eml",
        "./test_output/test.groups.empty",
        "./test_output/test.groups.empty/__last_article_number",
        "./test_output/test.groups.synthetic",
        "./test_output/test.groups.synthetic/__last_article_number",
        "./test_output/test.groups.synthetic/1.eml",
        "./test_output/test.groups.synthetic/2.eml",
        "./test_output/test.groups.synthetic/3.eml",
        "./test_output/test.groups.synthetic/4.eml",
        "./test_output/test.groups.synthetic/5.eml",
        "./test_output/test.groups.synthetic/6.eml",
        "./test_output/test.groups.synthetic/7.eml",
        "./test_output/test.groups.synthetic/8.eml",
        "./test_output/test.groups.synthetic/9.eml",
        "./test_output/test.groups.synthetic/10.eml",
        "./test_output/test.groups.synthetic/11.eml",
        "./test_output/test.groups.synthetic/12.eml",
    ];
    found_files.sort();
    expected_files.sort();
    assert_eq!(found_files, expected_files);

    check_and_delete_folder(output_dir).unwrap();
}

// =============================================================================
// Range Variation Integration Tests
// =============================================================================

/// Helper function to run the archiver with a specific article range
/// Returns the list of files created
fn run_archiver_with_range(article_range: Option<String>, test_name: String) -> Vec<String> {
    println!("loading Containerfile for {}", test_name);
    let image = GenericBuildableImage::new("test_nttp_server", "latest")
        .with_dockerfile("./tests/Containerfile")
        .with_file("./tests/test_nttp_server", "./test_nttp_server")
        .build_image()
        .unwrap();

    // Use the built image in containers
    let container = image
        .with_wait_for(WaitFor::message_on_stdout("Serving on port :8119"))
        .start()
        .unwrap();

    let host_port = container.get_host_port_ipv4(8119).unwrap();
    let output_dir = format!("./test_output_{}", test_name);

    println!("server container running on host port: {}", host_port);
    let mut app_config = AppConfig {
        output_dir: output_dir.clone(),
        nthreads: 1,
        loop_groups: false,
        nntp: Some(NntpConfig {
            hostname: "localhost".to_owned(),
            port: host_port,
            group_lists: Some(vec!["ALL".to_owned()]),
            article_range,
        }),
    };

    check_and_delete_folder(output_dir.clone()).unwrap();

    println!("Starting worker for {}", test_name);

    let shutdown_flag = Arc::new(AtomicBool::new(false));

    let test_name_clone = test_name.clone();
    let child_handle = thread::spawn(move || {
        println!("Child thread started for {}.", test_name_clone);
        let result = start(&mut app_config, shutdown_flag);
        assert!(result.is_ok());
        println!("Child thread stopped for {}.", test_name_clone);
    });

    println!("waiting server thread to finish for {}", test_name);
    child_handle.join().expect("Child thread panicked");
    container.stop().unwrap();
    container.rm().unwrap();

    println!("Loading list of files for {}", test_name);
    file_list_dir(output_dir.clone())
}

#[test]
fn test_read_single_article_by_range() {
    let found_files = run_archiver_with_range(Some("5".to_string()), "single".to_string());

    // Only article 5 should be fetched (only exists in synthetic list)
    // Other lists will have __errors files because article 5 doesn't exist
    let mut expected_files = vec![
        "./test_output_single",
        "./test_output_single/test.groups.foo",
        "./test_output_single/test.groups.foo/__errors",
        "./test_output_single/test.groups.bar",
        "./test_output_single/test.groups.bar/__errors",
        "./test_output_single/test.groups.empty",
        "./test_output_single/test.groups.empty/__errors",
        "./test_output_single/test.groups.synthetic",
        "./test_output_single/test.groups.synthetic/__last_article_number",
        "./test_output_single/test.groups.synthetic/5.eml",
    ];

    let mut found_files = found_files;
    found_files.sort();
    expected_files.sort();
    assert_eq!(found_files, expected_files);

    check_and_delete_folder("./test_output_single".to_string()).unwrap();
}

#[test]
fn test_read_article_range() {
    let found_files = run_archiver_with_range(Some("1-3".to_string()), "range".to_string());

    // Articles 1, 2, 3 should be fetched from each list
    // foo has 2 articles (1, 2), bar has 2 (1, 2), synthetic has 3 (1, 2, 3)
    // Lists with unavailable articles will also have __errors files
    let mut expected_files = vec![
        "./test_output_range",
        "./test_output_range/test.groups.foo",
        "./test_output_range/test.groups.foo/__last_article_number",
        "./test_output_range/test.groups.foo/1.eml",
        "./test_output_range/test.groups.foo/2.eml",
        "./test_output_range/test.groups.foo/__errors",
        "./test_output_range/test.groups.bar",
        "./test_output_range/test.groups.bar/__last_article_number",
        "./test_output_range/test.groups.bar/1.eml",
        "./test_output_range/test.groups.bar/2.eml",
        "./test_output_range/test.groups.bar/__errors",
        "./test_output_range/test.groups.empty",
        "./test_output_range/test.groups.empty/__errors",
        "./test_output_range/test.groups.synthetic",
        "./test_output_range/test.groups.synthetic/__last_article_number",
        "./test_output_range/test.groups.synthetic/1.eml",
        "./test_output_range/test.groups.synthetic/2.eml",
        "./test_output_range/test.groups.synthetic/3.eml",
    ];

    let mut found_files = found_files;
    found_files.sort();
    expected_files.sort();
    assert_eq!(found_files, expected_files);

    check_and_delete_folder("./test_output_range".to_string()).unwrap();
}

#[test]
fn test_read_multiple_articles_by_range() {
    let found_files = run_archiver_with_range(Some("1,5,10".to_string()), "multiple".to_string());

    // Articles 1, 5, 10 should be fetched from each list
    // foo has 1 article (1), bar has 1 (1), synthetic has 3 (1, 5, 10)
    // Lists with unavailable articles will also have __errors files
    let mut expected_files = vec![
        "./test_output_multiple",
        "./test_output_multiple/test.groups.foo",
        "./test_output_multiple/test.groups.foo/__last_article_number",
        "./test_output_multiple/test.groups.foo/1.eml",
        "./test_output_multiple/test.groups.foo/__errors",
        "./test_output_multiple/test.groups.bar",
        "./test_output_multiple/test.groups.bar/__last_article_number",
        "./test_output_multiple/test.groups.bar/1.eml",
        "./test_output_multiple/test.groups.bar/__errors",
        "./test_output_multiple/test.groups.empty",
        "./test_output_multiple/test.groups.empty/__errors",
        "./test_output_multiple/test.groups.synthetic",
        "./test_output_multiple/test.groups.synthetic/__last_article_number",
        "./test_output_multiple/test.groups.synthetic/1.eml",
        "./test_output_multiple/test.groups.synthetic/5.eml",
        "./test_output_multiple/test.groups.synthetic/10.eml",
    ];

    let mut found_files = found_files;
    found_files.sort();
    expected_files.sort();
    assert_eq!(found_files, expected_files);

    check_and_delete_folder("./test_output_multiple".to_string()).unwrap();
}

#[test]
fn test_read_mixed_range() {
    let found_files = run_archiver_with_range(Some("1,3-5,10".to_string()), "mixed".to_string());

    // Articles 1, 3, 4, 5, 10 should be fetched from each list
    // foo has 1 article (1), bar has 1 (1), synthetic has 5 (1, 3, 4, 5, 10)
    // Lists with unavailable articles will also have __errors files
    let mut expected_files = vec![
        "./test_output_mixed",
        "./test_output_mixed/test.groups.foo",
        "./test_output_mixed/test.groups.foo/__last_article_number",
        "./test_output_mixed/test.groups.foo/1.eml",
        "./test_output_mixed/test.groups.foo/__errors",
        "./test_output_mixed/test.groups.bar",
        "./test_output_mixed/test.groups.bar/__last_article_number",
        "./test_output_mixed/test.groups.bar/1.eml",
        "./test_output_mixed/test.groups.bar/__errors",
        "./test_output_mixed/test.groups.empty",
        "./test_output_mixed/test.groups.empty/__errors",
        "./test_output_mixed/test.groups.synthetic",
        "./test_output_mixed/test.groups.synthetic/__last_article_number",
        "./test_output_mixed/test.groups.synthetic/1.eml",
        "./test_output_mixed/test.groups.synthetic/3.eml",
        "./test_output_mixed/test.groups.synthetic/4.eml",
        "./test_output_mixed/test.groups.synthetic/5.eml",
        "./test_output_mixed/test.groups.synthetic/10.eml",
    ];

    let mut found_files = found_files;
    found_files.sort();
    expected_files.sort();
    assert_eq!(found_files, expected_files);

    check_and_delete_folder("./test_output_mixed".to_string()).unwrap();
}
