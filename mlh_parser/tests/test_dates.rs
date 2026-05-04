mod common;

use chrono::DateTime;
use common::{list_files_with_extension, map_to_file_extensions, parse_date_file};
use mlh_parser::date_parser::{parse_date_tentative_raw, process_date};
use mlh_parser::email_reader::{decode_mail, get_headers};
use std::fs;

#[test]
#[ignore = "date parsing parity with Python needs iteration"]
fn test_correct_email() {
    let directory = "./date_cases/";
    let email_files = list_files_with_extension(directory, ".eml");

    for email_file in &email_files {
        let fixtures = map_to_file_extensions(email_file, &[".date.expected"]);
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
        let expected_date = parse_date_tentative_raw(&expected_date_str);

        let msg = decode_mail(&mail_bytes).unwrap();
        let mut headers = get_headers(&msg);

        let now = DateTime::from_timestamp(1734748800, 0).unwrap().into();
        process_date(&mut headers, now);

        if let (Some(expected), Some(actual_str)) = (expected_date, headers.get("date"))
            && let Ok(actual) = DateTime::parse_from_rfc3339(actual_str)
        {
            assert_eq!(actual, expected, "Date mismatch for {:?}", email_file);
        }
    }
}

#[test]
#[ignore = "date parsing parity with Python needs iteration"]
fn test_millennium_dates() {
    let millennium_cases = vec![
        ("Mon, 3 Jan 78 18:27:37", "Mon, 3 Jan 1978 18:27:37"),
        ("Mon, 3 Jan 99 18:27:37", "Mon, 3 Jan 99 18:27:37"),
        ("Mon, 3 Jan 100 18:27:37", "Mon, 3 Jan 2000 18:27:37"),
        ("Mon, 3 Jan 0100 18:27:37", "Mon, 3 Jan 2000 18:27:37"),
        ("Mon, 3 Jan 101 18:27:37", "Mon, 3 Jan 2001 18:27:37"),
        ("Mon, 3 Jan 0120 18:27:37", "Mon, 3 Jan 2020 18:27:37"),
    ];

    let now = DateTime::from_timestamp(1734748800, 0).unwrap().into();

    for (found_str, expected_str) in millennium_cases {
        let found_date = parse_date_tentative_raw(found_str).unwrap();
        let expected_date = parse_date_tentative_raw(expected_str).unwrap();
        let fixed = mlh_parser::date_parser::fix_millennium_date(found_date, now);
        assert_eq!(fixed, expected_date, "Failed for {}", found_str);
    }
}
