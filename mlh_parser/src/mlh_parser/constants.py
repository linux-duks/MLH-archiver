import polars as pl

PARQUET_COLS_SCHEMA = {
    "from": pl.String,
    "to": pl.List(pl.String),
    "cc": pl.List(pl.String),
    "subject": pl.String,
    # parsed and best-effort-corrected date
    "date": pl.Datetime,
    # date reported by the email client (incorrect in many cases)
    "client-date": pl.List(pl.String),
    "message-id": pl.String,
    "in-reply-to": pl.String,
    "references": pl.List(pl.String),
    "x-mailing-list": pl.String,
    "trailers": pl.List(
        pl.Struct({"attribution": pl.String, "identification": pl.String})
    ),
    "code": pl.List(pl.String),
    "raw_body": pl.String,
}

SIGNED_BLOCK = "trailers"

SINGLE_VALUED_COLS = [
    "from",
    "subject",
    "date",
    "message-id",
    "in-reply-to",
    "x-mailing-list",
    "raw_body",
]

N_PROC = 4

REDO_FAILED_PARSES = (
    False  # Parse only the emails that were unsuccessfully parsed on previous runs.
)

LISTS_TO_PARSE = []

KEYS_MASK = [
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
]
