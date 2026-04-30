use std::fs;
use std::path::PathBuf;

use arrow::array::{Array, StringArray};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

use mlh_archiver::archive_writer::{
    ArchiveWriter, EmailData, EmailStore, ParquetEmailStore, WriteMode,
};
use mlh_archiver::config::RunModeConfig;
use mlh_archiver::nntp_source::nntp_config::NntpConfig;

// =============================================================================
// Test helpers
// =============================================================================

/// RAII guard that removes the test directory on creation (clean start) and drop (clean exit).
struct TestDirGuard {
    path: PathBuf,
}

impl TestDirGuard {
    fn new(path: PathBuf) -> Self {
        let _ = fs::remove_dir_all(&path);
        TestDirGuard { path }
    }
}

impl Drop for TestDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn create_dummy_email(id: &str, line_count: usize) -> EmailData {
    let content = (0..line_count)
        .map(|i| format!("Line {} of email {}", i, id))
        .collect::<Vec<_>>()
        .join("");
    EmailData {
        email_id: id.to_string(),
        content,
    }
}

/// Reads a parquet file into a vec of (email_id, content) pairs.
fn read_parquet_file(path: &std::path::Path) -> Vec<(String, String)> {
    let file = std::fs::File::open(path).expect("Parquet file should be readable");
    let builder = ParquetRecordBatchReaderBuilder::try_new(file).unwrap();
    let reader = builder.build().unwrap();

    let mut results = Vec::new();
    for batch_result in reader {
        let batch = batch_result.unwrap();
        let ids = batch
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let contents = batch
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();

        for row in 0..batch.num_rows() {
            let id = ids.value(row).to_string();
            let content = contents.value(row).to_string();
            results.push((id, content));
        }
    }
    results
}

// =============================================================================
// ParquetEmailStore: multi-file creation tests
// =============================================================================

#[test]
fn test_parquet_store_multi_file_creation() {
    let dir = PathBuf::from("./test_parquet_multi_file/");
    let _guard = TestDirGuard::new(dir.clone());
    let base_path = dir.join("emails.parquet");

    let mut store = ParquetEmailStore::new(base_path, 3);

    // Add 10 emails — triggers flush at 3, 6, 9, and close flushes 10th → 4 files
    for i in 0..10 {
        store
            .add_email(create_dummy_email(&format!("email_{}", i), 1))
            .unwrap();
    }
    store.close().unwrap();

    // Verify expected files: emails_000 through emails_003
    for idx in 0..4 {
        let path = dir.join(format!("emails_{:03}.parquet", idx));
        assert!(path.exists(), "Expected emails_{}.parquet to exist", idx);
    }
    assert!(
        !dir.join("emails_4.parquet").exists(),
        "Should not create emails_4.parquet"
    );
}

#[test]
fn test_parquet_store_multi_file_row_counts() {
    let dir = PathBuf::from("./test_parquet_row_counts/");
    let _guard = TestDirGuard::new(dir.clone());
    let base_path = dir.join("rows.parquet");

    let mut store = ParquetEmailStore::new(base_path, 4);

    // Add 10 emails: flushes at 4, 8, then close flushes 2 remaining → 3 files
    for i in 0..10 {
        store
            .add_email(create_dummy_email(&format!("email_{}", i), 1))
            .unwrap();
    }
    store.close().unwrap();

    let f0 = read_parquet_file(&dir.join("rows_000.parquet"));
    assert_eq!(f0.len(), 4, "File 0 should have 4 rows");
    assert_eq!(f0[0].0, "email_0");
    assert_eq!(f0[3].0, "email_3");

    let f1 = read_parquet_file(&dir.join("rows_001.parquet"));
    assert_eq!(f1.len(), 4, "File 1 should have 4 rows");
    assert_eq!(f1[0].0, "email_4");
    assert_eq!(f1[3].0, "email_7");

    let f2 = read_parquet_file(&dir.join("rows_002.parquet"));
    assert_eq!(f2.len(), 2, "File 2 should have 2 rows");
    assert_eq!(f2[0].0, "email_8");
    assert_eq!(f2[1].0, "email_9");

    assert!(!dir.join("rows_003.parquet").exists());
}

#[test]
fn test_parquet_store_content_integrity_across_files() {
    let dir = PathBuf::from("./test_parquet_content_split/");
    let _guard = TestDirGuard::new(dir.clone());
    let base_path = dir.join("emails.parquet");

    let mut store = ParquetEmailStore::new(base_path, 2);

    // 5 emails → flushes at 2, 4, close flushes 1 → 3 files
    store.add_email(create_dummy_email("alpha", 2)).unwrap();
    store.add_email(create_dummy_email("beta", 3)).unwrap(); // flush → file 0 has alpha, beta
    store.add_email(create_dummy_email("gamma", 1)).unwrap();
    store.add_email(create_dummy_email("delta", 2)).unwrap(); // flush → file 1 has gamma, delta
    store.add_email(create_dummy_email("epsilon", 4)).unwrap();
    store.close().unwrap(); // flush → file 2 has epsilon

    // File 0
    let f0 = read_parquet_file(&dir.join("emails_000.parquet"));
    assert_eq!(f0[0].0, "alpha");
    assert_eq!(f0[0].1, "Line 0 of email alphaLine 1 of email alpha");
    assert_eq!(f0[1].0, "beta");
    assert_eq!(
        f0[1].1,
        "Line 0 of email betaLine 1 of email betaLine 2 of email beta"
    );

    // File 1
    let f1 = read_parquet_file(&dir.join("emails_001.parquet"));
    assert_eq!(f1.len(), 2, "File 1 should have 2 rows");
    assert_eq!(f1[0].0, "gamma");
    assert_eq!(f1[0].1, "Line 0 of email gamma");
    assert_eq!(f1[1].0, "delta");
    assert_eq!(f1[1].1, "Line 0 of email deltaLine 1 of email delta");

    // File 2
    let f2 = read_parquet_file(&dir.join("emails_002.parquet"));
    assert_eq!(f2.len(), 1, "File 2 should have 1 row");
    assert_eq!(f2[0].0, "epsilon");
    assert_eq!(
        f2[0].1,
        "Line 0 of email epsilonLine 1 of email epsilonLine 2 of email epsilonLine 3 of email epsilon"
    );

    assert!(!dir.join("emails_003.parquet").exists());
}

// =============================================================================
// ParquetEmailStore + ArchiveWriter: integration tests
// =============================================================================

#[test]
fn test_archive_writer_parquet_multi_file() {
    let dir = PathBuf::from("./test_aw_parquet_multi/");
    let _guard = TestDirGuard::new(dir.clone());
    let base = dir.join("output");

    {
        let mut writer = ArchiveWriter::new(
            &base,
            "test_list",
            RunModeConfig::NNTP(NntpConfig::default()),
            WriteMode::Parquet { buffer_size: 3 },
        );

        // 7 emails → flushes at 3, 6, close flushes 1 → 3 parquet files
        writer.archive_email("1", ["email one content"]).unwrap();
        writer
            .archive_email("2", ["email two content", "second line"])
            .unwrap();
        writer.archive_email("3", ["email three"]).unwrap(); // flush at 3
        writer.archive_email("4", ["email four content"]).unwrap();
        writer
            .archive_email("5", ["email five", "line two"])
            .unwrap();
        writer.archive_email("6", ["email six"]).unwrap(); // flush at 6
        writer.archive_email("7", ["email seven final"]).unwrap();
        // drop → close → flush remaining "7" → data_002.parquet
    }

    let list_dir = base.join("test_list");

    assert!(list_dir.join("data_000.parquet").exists());
    assert!(list_dir.join("data_001.parquet").exists());
    assert!(list_dir.join("data_002.parquet").exists());
    assert!(!list_dir.join("data_003.parquet").exists());

    // Progress and lineage
    assert!(list_dir.join("__progress.yaml").exists());
    assert!(list_dir.join("__lineage.yaml").exists());

    // Verify file 0: emails 1, 2, 3
    let f0 = read_parquet_file(&list_dir.join("data_000.parquet"));
    assert_eq!(f0.len(), 3);
    assert_eq!(f0[0].0, "1");
    assert_eq!(f0[0].1, "email one content");
    assert_eq!(f0[1].0, "2");
    assert_eq!(f0[1].1, "email two contentsecond line");
    assert_eq!(f0[2].0, "3");

    // Verify file 1: emails 4, 5, 6
    let f1 = read_parquet_file(&list_dir.join("data_001.parquet"));
    assert_eq!(f1.len(), 3);
    assert_eq!(f1[0].0, "4");
    assert_eq!(f1[1].0, "5");
    assert_eq!(f1[2].0, "6");

    // Verify file 2: email 7
    let f2 = read_parquet_file(&list_dir.join("data_002.parquet"));
    assert_eq!(f2.len(), 1);
    assert_eq!(f2[0].0, "7");
    assert_eq!(f2[0].1, "email seven final");

    // Verify progress contains last email
    let progress = fs::read_to_string(list_dir.join("__progress.yaml")).unwrap();
    assert!(
        progress.contains("last_email: '7'") || progress.contains("last_email: 7"),
        "Progress should track last email: {}",
        progress
    );

    // Verify lineage references all emails
    let lineage = fs::read_to_string(list_dir.join("__lineage.yaml")).unwrap();
    for id in 1..=7 {
        assert!(
            lineage.contains(&format!("email_index: {}", id))
                || lineage.contains(&format!("email_index: '{}'", id)),
            "Lineage should reference email {}",
            id
        );
    }
}

#[test]
fn test_archive_writer_small_batch_exact_boundary() {
    let dir = PathBuf::from("./test_aw_small_batch/");
    let _guard = TestDirGuard::new(dir.clone());
    let base = dir.join("output");

    {
        let mut writer = ArchiveWriter::new(
            &base,
            "boundary_list",
            RunModeConfig::NNTP(NntpConfig::default()),
            WriteMode::Parquet { buffer_size: 2 },
        );

        // Exactly 2 emails → 1 flush on close → 1 file
        writer.archive_email("a", ["content a"]).unwrap();
        writer.archive_email("b", ["content b"]).unwrap();
        // Drop → close → flush both → 1 file
    }

    let list_dir = base.join("boundary_list");
    let f0 = read_parquet_file(&list_dir.join("data_000.parquet"));
    assert_eq!(f0.len(), 2, "Should have exactly 2 rows");
    assert_eq!(f0[0].0, "a");
    assert_eq!(f0[0].1, "content a");
    assert_eq!(f0[1].0, "b");
    assert_eq!(f0[1].1, "content b");
    assert!(!list_dir.join("data_001.parquet").exists());

    // Progress and lineage should exist
    assert!(list_dir.join("__progress.yaml").exists());
    assert!(list_dir.join("__lineage.yaml").exists());

    let progress = fs::read_to_string(list_dir.join("__progress.yaml")).unwrap();
    assert!(
        progress.contains("last_email: 'b'") || progress.contains("last_email: b"),
        "Progress should track last email 'b': {}",
        progress
    );
}

#[test]
fn test_archive_writer_single_email_single_file() {
    let dir = PathBuf::from("./test_aw_single_email/");
    let _guard = TestDirGuard::new(dir.clone());
    let base = dir.join("output");

    {
        let mut writer = ArchiveWriter::new(
            &base,
            "single_list",
            RunModeConfig::NNTP(NntpConfig::default()),
            WriteMode::Parquet { buffer_size: 5 },
        );

        // Only 1 email → flushed on close → 1 file with 1 row
        writer.archive_email("only", ["the only email"]).unwrap();
    }

    let list_dir = base.join("single_list");
    let f0 = read_parquet_file(&list_dir.join("data_000.parquet"));
    assert_eq!(f0.len(), 1);
    assert_eq!(f0[0].0, "only");
    assert_eq!(f0[0].1, "the only email");
    assert!(!list_dir.join("data_001.parquet").exists());

    let progress = fs::read_to_string(list_dir.join("__progress.yaml")).unwrap();
    assert!(
        progress.contains("'only'") || progress.contains("only"),
        "Progress should reference 'only': {}",
        progress
    );
}

#[test]
fn test_archive_writer_empty_no_files() {
    let dir = PathBuf::from("./test_aw_empty/");
    let _guard = TestDirGuard::new(dir.clone());
    let base = dir.join("output");

    {
        let _writer = ArchiveWriter::new(
            &base,
            "empty_list",
            RunModeConfig::NNTP(NntpConfig::default()),
            WriteMode::Parquet { buffer_size: 2 },
        );
        // No emails added → flush on drop is a no-op → no files created
    }

    let list_dir = base.join("empty_list");
    // Directory should exist (created by ProgressTracker::update or fs::create_dir_all)
    // But no parquet files should exist
    assert!(!list_dir.join("data_000.parquet").exists());
    // Progress/lineage should NOT exist (no emails processed)
    assert!(!list_dir.join("__progress.yaml").exists());
}

// =============================================================================
// RawEmail mode comparison: same ArchiveWriter with RawEmails
// =============================================================================

#[test]
fn test_archive_writer_raw_mode_still_works() {
    let dir = PathBuf::from("./test_aw_raw_mode/");
    let _guard = TestDirGuard::new(dir.clone());
    let base = dir.join("output");

    {
        let mut writer = ArchiveWriter::new(
            &base,
            "raw_list",
            RunModeConfig::NNTP(NntpConfig::default()),
            WriteMode::RawEmails,
        );

        writer
            .archive_email("101", ["From: test@example.com", "Subject: hello"])
            .unwrap();
        writer
            .archive_email("102", ["From: other@example.com"])
            .unwrap();
    }

    let list_dir = base.join("raw_list");
    assert!(list_dir.join("101.eml").exists());
    assert!(list_dir.join("102.eml").exists());
    assert!(!list_dir.join("103.eml").exists());

    // Verify content
    let eml101 = fs::read_to_string(list_dir.join("101.eml")).unwrap();
    assert!(eml101.contains("From: test@example.com"));
    assert!(eml101.contains("Subject: hello"));

    let eml102 = fs::read_to_string(list_dir.join("102.eml")).unwrap();
    assert!(eml102.contains("From: other@example.com"));
}
