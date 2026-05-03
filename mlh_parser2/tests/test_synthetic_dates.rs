mod common;

use chrono::DateTime;
use common::{list_files_with_extension, map_to_file_extensions, parse_date_file};
use mlh_parser2::parser::parse_and_process_email;
use std::fs;

#[test]
fn test_synthetic_date_fallback() {
    let directory = "./synthetic/";
    let email_files = list_files_with_extension(directory, ".eml");

    let now = DateTime::from_timestamp(1734748800, 0)
        .unwrap()
        .into();

    for email_file in &email_files {
        let fixtures = map_to_file_extensions(email_file, &[".date.pytest"]);
        if fixtures.is_empty() {
            continue;
        }
        let date_file = &fixtures[0];

        if !date_file.exists() {
            continue;
        }

        let mail_bytes = fs::read(email_file).unwrap();
        let expected_date_str = parse_date_file(date_file);
        if expected_date_str.is_empty() {
            continue;
        }
        let expected_date =
            mlh_parser2::date_parser::parse_date_tentative_raw(&expected_date_str);

        let result = parse_and_process_email(&mail_bytes, now).unwrap();

        if let Some(actual_str) = result.headers.get("date")
            && let Ok(actual) = DateTime::parse_from_rfc3339(actual_str)
                && let Some(expected) = expected_date {
                    assert_eq!(
                        actual, expected,
                        "Date mismatch for {:?}: expected {}, got {}",
                        email_file, expected_date_str, actual_str
                    );
                }
    }
}
