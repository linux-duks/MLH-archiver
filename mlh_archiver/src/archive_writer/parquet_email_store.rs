use super::email_store::{EmailData, EmailStore};

use arrow::array::{RecordBatch, StringBuilder};
use arrow::datatypes::{DataType, Field, Schema};
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
use parquet::errors::ParquetError;
use parquet::file::properties::{WriterProperties, WriterVersion};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// ParquetEmailStore stores the emails in memory and write batches to parquet files
pub struct ParquetEmailStore {
    output_path: PathBuf,
    schema: Arc<Schema>,
    buffer: Vec<EmailData>,
    batch_size: usize,
    /// file write index keeps track of the number of files written
    writer_index: usize,
    commited_files: Vec<String>,
}

/// email_id: Utf8, content: Utf8
pub fn parquet_email_store_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("email_id", DataType::Utf8, false),
        Field::new("content", DataType::Utf8, false),
    ]))
}

impl ParquetEmailStore {
    /// Initializes the EmailStore with a schema, but delays file creation
    /// until the first flush to ensure empty files aren't created unnecessarily.
    pub fn new(output_path: PathBuf, batch_size: usize) -> Self {
        // Define the Arrow Schema:
        let schema = parquet_email_store_schema();

        Self {
            output_path,
            schema,
            buffer: Vec::with_capacity(batch_size),
            batch_size,
            writer_index: 0,
            commited_files: vec![],
        }
    }

    /// Internal method to lazily initialize the Parquet writer with specific properties
    fn init_writer(&mut self) -> crate::Result<(ArrowWriter<File>, String)> {
        let final_path = self.resolve_path()?;
        let file = File::create(&final_path)?;

        let filename = final_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown.parquet")
            .to_string();

        // Configure storage characteristics according to the plan
        let props = WriterProperties::builder()
            .set_compression(Compression::ZSTD(ZstdLevel::default()))
            .set_writer_version(WriterVersion::PARQUET_2_0)
            .build();

        let writer = ArrowWriter::try_new(file, self.schema.clone(), Some(props))?;
        Ok((writer, filename))
    }

    fn resolve_path(&mut self) -> Result<PathBuf, ParquetError> {
        // Identify the Parent and the File Stem
        let is_parquet = self
            .output_path
            .extension()
            .is_some_and(|ext| ext == "parquet");

        let (parent, stem) = if is_parquet {
            let p = self
                .output_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf();
            let s = self
                .output_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("data");
            (p, s.to_string())
        } else {
            // If the path is a directory or lacks extension, use "data" as stem
            (self.output_path.clone(), "data".to_string())
        };

        // Ensure parent directory exists before listing
        fs::create_dir_all(&parent)
            .map_err(|e| ParquetError::General(format!("Failed to create directories: {}", e)))?;

        // Scan directory to find the next available index
        if let Ok(entries) = fs::read_dir(&parent) {
            let mut max_index = None;

            for entry in entries.flatten() {
                if let Some(file_name) = entry.file_name().to_str() {
                    // Look for files matching "{stem}_{index}.parquet"
                    if file_name.starts_with(&format!("{}_", stem))
                        && file_name.ends_with(".parquet")
                    {
                        // Extract the middle part: {stem}_{HERE}.parquet
                        let index_part = &file_name[stem.len() + 1..file_name.len() - 8];
                        if let Ok(idx) = index_part.parse::<usize>() {
                            max_index = Some(max_index.map_or(idx, |m| std::cmp::max(m, idx)));
                        }
                    }
                }
            }

            // Set writer_index to max + 1 if files exist, otherwise start at 0
            self.writer_index = max_index.map(|m| m + 1).unwrap_or(0);
        }

        // 4. Construct the final path using the forced {stem}_{index}.parquet format
        let final_filename = format!("{}_{:03}.parquet", stem, self.writer_index);
        Ok(parent.join(final_filename))
    }

    /// Converts the row-based buffer into columnar Arrow arrays and writes the batch.
    #[cfg_attr(feature = "otel", tracing::instrument(skip(self)))]
    fn flush(&mut self) -> crate::Result<Vec<String>> {
        let mut synced_items: Vec<String> = vec![];

        if self.buffer.is_empty() {
            return Ok(synced_items);
        }

        let (mut writer, filename) = self.init_writer()?;

        //  Prepare Column Builders
        let mut id_builder = StringBuilder::new();
        let mut content_builder = StringBuilder::new();

        // Drain the buffer and populate columnar builders
        for email in self.buffer.drain(..) {
            synced_items.push(email.email_id.clone());
            id_builder.append_value(email.email_id);
            content_builder.append_value(email.content);
        }

        // Finalize arrays
        let id_array = Arc::new(id_builder.finish());
        let content_array = Arc::new(content_builder.finish());

        // Create the RecordBatch
        let batch = RecordBatch::try_new(self.schema.clone(), vec![id_array, content_array])
            .map_err(|e| ParquetError::General(format!("Arrow error: {}", e)))?;

        // Append the RecordBatch to the Parquet file
        // if let Some(writer) = &mut self.writer {
        writer.write(&batch)?;

        // `close()` writes the metadata footer and safely closes the file.
        // Without this, the parquet file will be corrupted/unreadable.
        writer.close()?;
        self.commited_files.push(filename);

        Ok(synced_items)
    }
}

impl EmailStore for ParquetEmailStore {
    /// Appends data to the buffer. If the batch_size is reached, it triggers a flush.
    fn add_email(&mut self, email: EmailData) -> crate::Result<Option<Vec<String>>> {
        self.buffer.push(email);

        let mut synced_items: Option<Vec<String>> = None;

        if self.buffer.len() >= self.batch_size {
            synced_items = Some(self.flush()?);
        }
        Ok(synced_items)
    }

    /// Flushes any remaining data in the buffer and writes the Parquet footer.
    /// This must be called before the store is dropped to ensure a valid Parquet file.
    fn close(&mut self) -> crate::Result<Option<Vec<String>>> {
        let mut synced_items: Option<Vec<String>> = None;

        let flushed = self.flush()?;
        if !flushed.is_empty() {
            synced_items = Some(flushed)
        }

        Ok(synced_items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::StringArray;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

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

    // Helper function to create dummy email data
    fn create_dummy_email(id: &str, line_count: usize) -> EmailData {
        let content = (0..line_count)
            .map(|i| format!("This is line {} of email {}", i, id))
            .collect::<Vec<_>>()
            .join("");
        EmailData {
            email_id: id.to_string(),
            content,
        }
    }

    #[test]
    fn test_lazy_initialization() {
        let dir = PathBuf::from("./parquet_email_store.test_lazy_initialization/");
        let _guard = TestDirGuard::new(dir.clone());
        let base_path = dir.join("lazy_test.parquet");
        let expected_path = dir.join("lazy_test_000.parquet");

        let mut store = ParquetEmailStore::new(base_path.clone(), 10);

        assert!(
            !expected_path.exists(),
            "File should not be created until data is written"
        );

        store.add_email(create_dummy_email("email_1", 2)).unwrap();
        assert!(
            !expected_path.exists(),
            "File should still not exist, buffer not full"
        );

        store.flush().unwrap();
        assert!(expected_path.exists(), "File should be created after flush");
    }

    #[test]
    fn test_batch_flush_trigger() {
        let dir = PathBuf::from("./parquet_email_store.test_batch_flush_trigger/");
        let _guard = TestDirGuard::new(dir.clone());
        let base_path = dir.join("batch_test.parquet");
        let expected_path = dir.join("batch_test_000.parquet");

        let mut store = ParquetEmailStore::new(base_path.clone(), 2);

        store.add_email(create_dummy_email("email_1", 1)).unwrap();
        assert!(!expected_path.exists());
        assert_eq!(store.buffer.len(), 1);

        store.add_email(create_dummy_email("email_2", 1)).unwrap();
        assert!(expected_path.exists());
        assert_eq!(store.buffer.len(), 0);
    }

    #[test]
    fn test_data_integrity_readback() {
        let dir = PathBuf::from("./parquet_email_store.test_data_integrity_readback/");
        let _guard = TestDirGuard::new(dir.clone());
        let base_path = dir.join("integrity_test.parquet");
        let expected_path = dir.join("integrity_test_000.parquet");

        let mut store = ParquetEmailStore::new(base_path.clone(), 5);
        store.add_email(create_dummy_email("email_1", 2)).unwrap();
        store.add_email(create_dummy_email("email_2", 3)).unwrap();

        store.close().unwrap();

        let file = File::open(&expected_path).unwrap();
        let builder = ParquetRecordBatchReaderBuilder::try_new(file).unwrap();
        let mut reader = builder.build().unwrap();

        let batch = reader.next().unwrap().unwrap();

        assert_eq!(batch.num_rows(), 2);

        let id_array = batch
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        assert_eq!(id_array.value(0), "email_1");
        assert_eq!(id_array.value(1), "email_2");

        let content_array = batch
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();

        assert_eq!(
            content_array.value(0),
            "This is line 0 of email email_1This is line 1 of email email_1"
        );
        assert_eq!(
            content_array.value(1),
            "This is line 0 of email email_2This is line 1 of email email_2This is line 2 of email email_2"
        );

        assert!(reader.next().is_none());
    }

    #[test]
    fn test_path_collision_logic() {
        let dir = PathBuf::from("./parquet_email_store.test_path_collision_logic/");
        let _guard = TestDirGuard::new(dir.clone());
        let base_path = dir.join("collision.parquet");

        {
            let mut store = ParquetEmailStore::new(base_path.clone(), 1);
            store
                .add_email(EmailData {
                    email_id: "1".into(),
                    content: String::new(),
                })
                .unwrap();
            store.close().unwrap();
            assert!(dir.join("collision_000.parquet").exists());
        }

        {
            let mut store = ParquetEmailStore::new(base_path.clone(), 1);
            store
                .add_email(EmailData {
                    email_id: "2".into(),
                    content: String::new(),
                })
                .unwrap();
            store.close().unwrap();
            assert!(dir.join("collision_001.parquet").exists());
        }

        {
            let mut store = ParquetEmailStore::new(base_path.clone(), 1);
            store
                .add_email(EmailData {
                    email_id: "3".into(),
                    content: String::new(),
                })
                .unwrap();
            store.close().unwrap();
            assert!(dir.join("collision_002.parquet").exists());
        }
    }
}
