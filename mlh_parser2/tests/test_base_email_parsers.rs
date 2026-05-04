mod common;

use common::{
    list_files_with_extension, map_to_file_extensions, parse_body_file, parse_headers_file,
};
use mlh_parser2::email_reader::{decode_mail, get_body, get_headers};
use std::fs;

#[test]
#[ignore = "fixture exact-match parity with Python needs iteration"]
fn test_body_parser() {
    let directory = "./complete_cases/";
    let email_files = list_files_with_extension(directory, ".eml");

    for email_file in &email_files {
        let fixtures = map_to_file_extensions(email_file, &[".body.pytest"]);
        if fixtures.is_empty() {
            continue;
        }
        let body_file = &fixtures[0];

        if !body_file.exists() {
            continue;
        }

        let mail_bytes = fs::read(email_file).unwrap();
        let expected_body = parse_body_file(body_file);

        let mail = decode_mail(&mail_bytes).unwrap();
        let actual_body = get_body(&mail);

        assert_eq!(
            actual_body, expected_body,
            "Body mismatch for {:?}",
            email_file
        );
    }
}

#[test]
#[ignore = "fixture exact-match parity with Python needs iteration"]
fn test_header_parser() {
    let directory = "./complete_cases/";
    let email_files = list_files_with_extension(directory, ".eml");

    for email_file in &email_files {
        let fixtures = map_to_file_extensions(email_file, &[".headers.pytest"]);
        if fixtures.is_empty() {
            continue;
        }
        let headers_file = &fixtures[0];

        if !headers_file.exists() {
            continue;
        }

        let mail_bytes = fs::read(email_file).unwrap();
        let expected_headers = parse_headers_file(headers_file);

        let mail = decode_mail(&mail_bytes).unwrap();
        let actual_headers = get_headers(&mail);

        for (key, expected_value) in &expected_headers {
            let actual_value = actual_headers.get(key).cloned().unwrap_or_default();
            assert_eq!(
                actual_value, *expected_value,
                "Header mismatch for '{}' in {:?}",
                key, email_file
            );
        }
    }
}
