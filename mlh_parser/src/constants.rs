//! Parquet schema definition, batch limits, and column constants.

use std::sync::{Arc, LazyLock};

use arrow::datatypes::{DataType, Field, Fields, Schema, TimeUnit};

/// Maximum number of emails to accumulate before flushing to a Parquet row group.
///
/// Batch limits keep row-group size well under Arrow's `i32` string-offset
/// ceiling (~2.1 GB). These values can be overridden in tests without needing
/// multi-gigabyte test fixtures.
pub const BATCH_MAX_RECORDS: usize = 50_000;
/// Maximum cumulative raw body bytes before flushing to a Parquet row group (400 MB).
pub const BATCH_MAX_RAW_BYTES: usize = 400 * 1024 * 1024;

/// Internal key used for the signature-trailers block in the parsed email dict.
pub const SIGNED_BLOCK: &str = "trailers";

/// Columns that are guaranteed to hold a single string value (not a list).
///
/// Used by the post-processing step in `email_parser` to ensure missing
/// columns are populated with empty strings.
pub const SINGLE_VALUED_COLS: &[&str] = &[
    "from",
    "subject",
    "date",
    "message-id",
    "in-reply-to",
    "x-mailing-list",
    "raw_body",
];

/// Full set of keys recognized in a parsed email headers map.
pub const KEYS_MASK: &[&str] = &[
    "from",
    "to",
    "cc",
    "subject",
    "date",
    "message-id",
    "in-reply-to",
    "references",
    "x-mailing-list",
    SIGNED_BLOCK,
    "code",
    "raw_body",
    "__file_name",
];

/// The fixed Arrow schema used for all Parquet output.
///
/// Column order: `from, to, cc, subject, date, client-date, message-id,
/// in-reply-to, references, x-mailing-list, trailers, code, raw_body, __file_name`.
pub static PARQUET_SCHEMA: LazyLock<Schema> = LazyLock::new(|| {
    let trailer_fields = Fields::from(vec![
        Field::new("attribution", DataType::Utf8, false),
        Field::new("identification", DataType::Utf8, false),
    ]);

    Schema::new(vec![
        Field::new("from", DataType::Utf8, true),
        Field::new(
            "to",
            DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
            true,
        ),
        Field::new(
            "cc",
            DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
            true,
        ),
        Field::new("subject", DataType::Utf8, true),
        Field::new(
            "date",
            DataType::Timestamp(TimeUnit::Microsecond, None),
            true,
        ),
        Field::new(
            "client-date",
            DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
            true,
        ),
        Field::new("message-id", DataType::Utf8, true),
        Field::new("in-reply-to", DataType::Utf8, true),
        Field::new(
            "references",
            DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
            true,
        ),
        Field::new("x-mailing-list", DataType::Utf8, true),
        Field::new(
            "trailers",
            DataType::List(Arc::new(Field::new(
                "item",
                DataType::Struct(trailer_fields),
                true,
            ))),
            true,
        ),
        Field::new(
            "code",
            DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
            true,
        ),
        Field::new("raw_body", DataType::Utf8, true),
        Field::new("__file_name", DataType::Utf8, true),
    ])
});

/// Output Parquet filename inside each list's Hive partition directory.
pub const PARQUET_FILE_NAME: &str = "list_data.parquet";
