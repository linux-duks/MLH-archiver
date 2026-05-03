use mlh_parser2::{BATCH_MAX_RAW_BYTES, BATCH_MAX_RECORDS, parser::parse_mail_at};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_parse_empty_directory() {
    let temp_dir = TempDir::new().unwrap();
    let input_base = temp_dir.path().to_path_buf();
    let list_dir = input_base.join("empty_list");
    fs::create_dir_all(&list_dir).unwrap();
    let output_base = temp_dir.path().join("output");

    let result = parse_mail_at("empty_list", &input_base, &output_base, false, BATCH_MAX_RECORDS, BATCH_MAX_RAW_BYTES);
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

    let result = parse_mail_at("test_list", &input_base, &output_base, false,BATCH_MAX_RECORDS, BATCH_MAX_RAW_BYTES);
    assert!(result.is_ok());

    let parquet_path = output_base
        .join("parsed")
        .join("list=test_list")
        .join("list_data.parquet");
    assert!(parquet_path.exists());
}
