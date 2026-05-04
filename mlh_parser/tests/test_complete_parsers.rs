mod common;

use chrono::DateTime;
use common::list_files_with_extension;
use mlh_parser::email_parser::parse_email;
use std::fs;

#[test]
fn test_complete_parser() {
    let directory = "./complete_cases/";
    let email_files = list_files_with_extension(directory, ".eml");
    let now = DateTime::from_timestamp(1734748800, 0).unwrap().into();

    for email_file in &email_files {
        let mail_bytes = match fs::read(email_file) {
            Ok(b) => b,
            Err(_) => continue,
        };

        match parse_email(&mail_bytes, now) {
            Ok(r) => {
                if r.raw_body.is_empty() {
                    eprintln!("Skipping {:?}: empty body", email_file);
                    continue;
                }
                for trailer in &r.trailers {
                    assert!(
                        trailer.attribution.ends_with("-by"),
                        "Invalid attribution: {:?}",
                        trailer.attribution
                    );
                }
            }
            Err(e) => {
                eprintln!("Failed to parse {:?}: {}", email_file, e);
                continue;
            }
        };
    }
}
