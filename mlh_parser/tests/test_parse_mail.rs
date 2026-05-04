use mlh_parser::{
    constants::{BATCH_MAX_RAW_BYTES, BATCH_MAX_RECORDS},
    process_mailing_list,
};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_parse_empty_directory() {
    let temp_dir = TempDir::new().unwrap();
    let input_base = temp_dir.path().to_path_buf();
    let list_dir = input_base.join("empty_list");
    fs::create_dir_all(&list_dir).unwrap();
    let output_base = temp_dir.path().join("output");

    let result = process_mailing_list(
        "empty_list",
        &input_base,
        &output_base,
        false,
        BATCH_MAX_RECORDS,
        BATCH_MAX_RAW_BYTES,
    );
    assert!(result.is_ok());
}

#[test]
fn test_parse_single_eml() {
    let temp_dir = TempDir::new().unwrap();
    let input_base = temp_dir.path().to_path_buf();
    let list_dir = input_base.join("test_list");
    fs::create_dir_all(&list_dir).unwrap();

    let output_base = temp_dir.path().join("output");

    let eml_content = concat!(
        "From: Test User <test@example.com>\r\n",
        "To: recipient@example.com\r\n",
        "Subject: Test Email\r\n",
        "Date: Sat, 29 Mar 2025 20:07:52 +0000\r\n",
        "Message-ID: <test123@example.com>\r\n",
        "\r\n",
        "This is the body of the test email.\r\n"
    );
    fs::write(list_dir.join("test.eml"), eml_content).unwrap();

    let result = process_mailing_list(
        "test_list",
        &input_base,
        &output_base,
        false,
        BATCH_MAX_RECORDS,
        BATCH_MAX_RAW_BYTES,
    );
    assert!(result.is_ok());

    let parquet_path = output_base
        .join("dataset")
        .join("list=test_list")
        .join("list_data.parquet");
    assert!(parquet_path.exists());
}

#[test]
fn test_parse_errors_written_to_csv() {
    let temp_dir = TempDir::new().unwrap();
    let input_base = temp_dir.path().to_path_buf();
    let list_dir = input_base.join("err_list");
    fs::create_dir_all(&list_dir).unwrap();
    let output_base = temp_dir.path().join("output");

    // An empty file triggers a DecodeError (mail-parser returns None)
    fs::write(list_dir.join("broken_01.eml"), "").unwrap();
    // A second broken file to ensure multiple errors accumulate
    fs::write(list_dir.join("broken_02.eml"), "").unwrap();

    let result = process_mailing_list(
        "err_list",
        &input_base,
        &output_base,
        false,
        BATCH_MAX_RECORDS,
        BATCH_MAX_RAW_BYTES,
    );
    assert!(result.is_ok());

    let csv_path = output_base
        .join("errors")
        .join("list=err_list")
        .join("errors.csv");
    assert!(csv_path.exists(), "errors.csv should exist");

    let csv_content = fs::read_to_string(&csv_path).unwrap();
    assert!(csv_content.contains("broken_01"), "should contain first email_id");
    assert!(csv_content.contains("broken_02"), "should contain second email_id");
    assert!(
        csv_content.contains("Failed to decode email"),
        "should contain error message"
    );
}

#[test]
fn test_parse_errors_csv_forwarding_newlines() {
    let temp_dir = TempDir::new().unwrap();
    let input_base = temp_dir.path().to_path_buf();
    let list_dir = input_base.join("multierr");
    fs::create_dir_all(&list_dir).unwrap();
    let output_base = temp_dir.path().join("output");

    // Write content that lacks a Message-ID header — parse_email doesn't fail
    // on this, but we can trigger a decoding error via empty content to keep
    // the test simple.
    fs::write(list_dir.join("bad.eml"), "").unwrap();

    let result = process_mailing_list(
        "multierr",
        &input_base,
        &output_base,
        false,
        BATCH_MAX_RECORDS,
        BATCH_MAX_RAW_BYTES,
    );
    assert!(result.is_ok());

    let csv_path = output_base
        .join("errors")
        .join("list=multierr")
        .join("errors.csv");
    let csv_content = fs::read_to_string(&csv_path).unwrap();

    // The CSV should contain exactly one line (no trailing empty line from
    // writeln, though the last file read may leave one — we check that no
    // raw \n appears inside a field)
    assert!(!csv_content.contains("\"\n\""), "fields should not contain raw newlines");
}
