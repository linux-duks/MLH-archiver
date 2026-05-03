use std::sync::{Arc, LazyLock};

use arrow::datatypes::{DataType, Field, Fields, Schema, TimeUnit};

pub const SIGNED_BLOCK: &str = "trailers";

pub const SINGLE_VALUED_COLS: &[&str] = &[
    "from",
    "subject",
    "date",
    "message-id",
    "in-reply-to",
    "x-mailing-list",
    "raw_body",
];

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

pub const PARQUET_FILE_NAME: &str = "list_data.parquet";
