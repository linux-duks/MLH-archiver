//! Reads `.eml` and `.parquet` files into a unified email iterator.

use arrow::array::{Array, StringArray};
use mlh_archiver::archive_writer::{EmailData, parquet_email_store_schema};
use parquet::arrow::arrow_reader::{
    ArrowReaderOptions, ParquetRecordBatchReader, ParquetRecordBatchReaderBuilder,
};
use std::fs::{self, File};
use std::path::{Path, PathBuf};

/// An iterator over rows in a Parquet file containing archived emails.
///
/// Each row yields an [`EmailData`] with `email_id` (column 0) and `content`
/// (column 1). The iterator transparently reads batches as needed.
pub struct ParquetEmailReader {
    reader: ParquetRecordBatchReader,
    current_batch: Option<arrow::array::RecordBatch>,
    current_row: usize,
}

impl Iterator for ParquetEmailReader {
    type Item = Result<EmailData, Box<dyn std::error::Error>>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(ref batch) = self.current_batch
                && self.current_row < batch.num_rows()
            {
                let id_array = batch
                    .column(0)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .expect("email_id is not Utf8");
                let content_array = batch
                    .column(1)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .expect("content is not Utf8");

                let row = EmailData {
                    email_id: id_array.value(self.current_row).to_string(),
                    content: content_array.value(self.current_row).to_string(),
                };
                self.current_row += 1;
                return Some(Ok(row));
            }

            match self.reader.next() {
                Some(Ok(batch)) => {
                    self.current_batch = Some(batch);
                    self.current_row = 0;
                }
                Some(Err(e)) => return Some(Err(Box::new(e))),
                None => return None,
            }
        }
    }
}

/// Opens a Parquet file and returns a row-by-row [`ParquetEmailReader`].
///
/// Uses [`parquet_email_store_schema`] to set the expected schema on the reader.
pub fn read_parquet_emails(path: &Path) -> Result<ParquetEmailReader, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let schema = parquet_email_store_schema();
    let options = ArrowReaderOptions::new().with_schema(schema);
    let builder = ParquetRecordBatchReaderBuilder::try_new_with_options(file, options)?;
    let reader = builder.build()?;

    Ok(ParquetEmailReader {
        reader,
        current_batch: None,
        current_row: 0,
    })
}

/// Reads a single `.eml` file and returns an [`EmailData`] record.
///
/// The `email_id` is derived from the file stem (name without extension).
pub fn read_eml_email(path: &Path) -> Result<EmailData, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    let email_id = path
        .file_stem()
        .ok_or("missing file stem")?
        .to_string_lossy()
        .to_string();
    let content = String::from_utf8_lossy(&bytes).to_string();
    Ok(EmailData { email_id, content })
}

/// An iterator that chains multiple `.eml` and `.parquet` files together.
///
/// For `.eml` files, each file produces one email. For `.parquet` files,
/// each row in the file produces one email. Unknown extensions are silently
/// skipped.
pub struct MultiFileEmailReader {
    file_paths: std::vec::IntoIter<PathBuf>,
    current_parquet_reader: Option<ParquetEmailReader>,
}

impl Iterator for MultiFileEmailReader {
    type Item = Result<EmailData, Box<dyn std::error::Error>>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // If we're in the middle of a parquet file, continue reading it
            if let Some(ref mut reader) = self.current_parquet_reader {
                let item = reader.next();
                if item.is_some() {
                    return item;
                }
                // Reader exhausted — clear it and move to next file
                self.current_parquet_reader = None;
            }

            // No active reader — try next file
            let path = self.file_paths.next()?;

            let ext = path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();

            match ext.as_str() {
                "eml" => match read_eml_email(&path) {
                    Ok(data) => return Some(Ok(data)),
                    Err(e) => return Some(Err(e)),
                },
                "parquet" => match read_parquet_emails(&path) {
                    Ok(reader) => {
                        self.current_parquet_reader = Some(reader);
                        // Continue loop to get first row from the new reader
                    }
                    Err(e) => return Some(Err(e)),
                },
                _ => {
                    // Unknown extension — skip to next file
                }
            }
        }
    }
}

/// Creates a [`MultiFileEmailReader`] that yields emails from the given file paths.
///
/// Files must have extension `.eml` or `.parquet`. Unknown extensions are
/// silently skipped. File order is preserved from the input vector.
pub fn file_iterator(file_paths: Vec<PathBuf>) -> MultiFileEmailReader {
    MultiFileEmailReader {
        file_paths: file_paths.into_iter(),
        current_parquet_reader: None,
    }
}
