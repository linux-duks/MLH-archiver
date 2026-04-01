use std::io;
use std::path::Path;
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

    let child_handle = thread::spawn(move || {
        println!("Child thread started.");
        let result = start(&mut app_config);
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
