use crate::ParsedEmail;
use crate::constants;

use arrow::array::*;
use arrow::datatypes::*;
use arrow::record_batch::RecordBatch;
use chrono::DateTime;
use parquet::arrow::ArrowWriter;
use parquet::errors::Result;
use parquet::file::properties::WriterProperties;
use std::fs;
use std::io::BufWriter;
use std::path::Path;
use std::sync::Arc;

pub type DatasetWriter = ArrowWriter<BufWriter<fs::File>>;

pub fn create_writer() {}

pub fn flush_batch(
    mailing_list: &str,
    parquet_path: &Path,
    batch_emails: &mut Vec<(ParsedEmail, String)>,
    batch_raw_body_bytes: &mut usize,
    total_parsed: &mut usize,
    arrow_writer: &mut Option<DatasetWriter>,
) -> Result<(), Box<dyn std::error::Error>> {
    if batch_emails.is_empty() {
        return Ok(());
    }

    let count = batch_emails.len();
    let bytes = *batch_raw_body_bytes;
    log::debug!(
        "parse_mail_at[{mailing_list}]: flushing batch of {count} emails (raw_body_bytes={bytes})",
    );

    let batch = build_record_batch(batch_emails)?;

    if arrow_writer.is_none() {
        let file = fs::File::create(parquet_path)?;
        let writer = BufWriter::new(file);
        let props = WriterProperties::builder().build();
        *arrow_writer = Some(ArrowWriter::try_new(writer, batch.schema(), Some(props))?);
    }

    arrow_writer.as_mut().unwrap().write(&batch)?;

    *total_parsed += count;
    batch_emails.clear();
    *batch_raw_body_bytes = 0;

    Ok(())
}

pub fn build_record_batch(
    emails: &[(ParsedEmail, String)],
) -> Result<RecordBatch, Box<dyn std::error::Error>> {
    let schema = constants::PARQUET_SCHEMA.clone();

    let mut total_raw_body_len: usize = 0;

    let mut from_arr = StringBuilder::new();
    let mut to_arr = ListBuilder::new(StringBuilder::new());
    let mut cc_arr = ListBuilder::new(StringBuilder::new());
    let mut subject_arr = StringBuilder::new();
    let mut date_arr = TimestampMicrosecondBuilder::new();
    let mut client_date_arr = ListBuilder::new(StringBuilder::new());
    let mut message_id_arr = StringBuilder::new();
    let mut in_reply_to_arr = StringBuilder::new();
    let mut references_arr = ListBuilder::new(StringBuilder::new());
    let mut x_mailing_list_arr = StringBuilder::new();

    let trailer_fields = Fields::from(vec![
        Field::new("attribution", DataType::Utf8, false),
        Field::new("identification", DataType::Utf8, false),
    ]);
    let mut trailers_arr = ListBuilder::new(StructBuilder::new(
        trailer_fields,
        vec![
            Box::new(StringBuilder::new()) as Box<dyn ArrayBuilder>,
            Box::new(StringBuilder::new()),
        ],
    ));

    let mut code_arr = ListBuilder::new(StringBuilder::new());
    let mut raw_body_arr = StringBuilder::new();
    let mut file_name_arr = StringBuilder::new();

    for (idx, (email, file_name)) in emails.iter().enumerate() {
        from_arr.append_value(email.headers.get("from").map(|s| s.as_str()).unwrap_or(""));

        // to
        {
            let to_list: Vec<&str> = email
                .headers
                .get("to")
                .map(|s| s.as_str())
                .unwrap_or("")
                .split("||")
                .filter(|x| !x.is_empty())
                .collect();
            for item in &to_list {
                to_arr.values().append_value(*item);
            }
            to_arr.append(!to_list.is_empty());
        }

        // cc
        {
            let cc_list: Vec<&str> = email
                .headers
                .get("cc")
                .map(|s| s.as_str())
                .unwrap_or("")
                .split("||")
                .filter(|x| !x.is_empty())
                .collect();
            for item in &cc_list {
                cc_arr.values().append_value(*item);
            }
            cc_arr.append(!cc_list.is_empty());
        }

        subject_arr.append_value(
            email
                .headers
                .get("subject")
                .map(|s| s.as_str())
                .unwrap_or(""),
        );

        // date
        let date_str = email.headers.get("date").cloned().unwrap_or_default();
        if let Ok(dt) = DateTime::parse_from_rfc3339(&date_str) {
            date_arr.append_value(dt.timestamp_micros());
        } else {
            date_arr.append_null();
        }

        // client-date
        {
            let cds: Vec<&str> = email
                .headers
                .get("client-date")
                .map(|s| s.as_str())
                .unwrap_or("")
                .split("||")
                .filter(|x| !x.is_empty())
                .collect();
            for item in &cds {
                client_date_arr.values().append_value(*item);
            }
            client_date_arr.append(!cds.is_empty());
        }

        message_id_arr.append_value(
            email
                .headers
                .get("message-id")
                .map(|s| s.as_str())
                .unwrap_or(""),
        );
        in_reply_to_arr.append_value(
            email
                .headers
                .get("in-reply-to")
                .map(|s| s.as_str())
                .unwrap_or(""),
        );

        // references
        {
            let refs: Vec<&str> = email
                .headers
                .get("references")
                .map(|s| s.as_str())
                .unwrap_or("")
                .split("||")
                .filter(|x| !x.is_empty())
                .collect();
            for item in &refs {
                references_arr.values().append_value(*item);
            }
            references_arr.append(!refs.is_empty());
        }

        x_mailing_list_arr.append_value(
            email
                .headers
                .get("x-mailing-list")
                .map(|s| s.as_str())
                .unwrap_or(""),
        );

        // trailers - struct list
        {
            let struct_builder = trailers_arr.values();
            for attr in &email.trailers {
                struct_builder
                    .field_builder::<StringBuilder>(0)
                    .unwrap()
                    .append_value(&attr.attribution);
                struct_builder
                    .field_builder::<StringBuilder>(1)
                    .unwrap()
                    .append_value(&attr.identification);
                struct_builder.append(true);
            }
            trailers_arr.append(!email.trailers.is_empty());
        }

        // code
        {
            for patch in &email.code {
                code_arr.values().append_value(patch.as_str());
            }
            code_arr.append(!email.code.is_empty());
        }

        log::debug!(
            "build_record_batch[{idx}] email_id={file_name} raw_body_len={} total_raw_body_len_so_far={total_raw_body_len}",
            email.raw_body.len()
        );
        total_raw_body_len += email.raw_body.len();
        raw_body_arr.append_value(email.raw_body.as_str());
        file_name_arr.append_value(file_name.as_str());
    }

    log::debug!(
        "build_record_batch: finished building {} columns for {} emails, total raw_body bytes: {total_raw_body_len}",
        schema.fields().len(),
        emails.len()
    );

    let batch = RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(from_arr.finish()),
            Arc::new(to_arr.finish()),
            Arc::new(cc_arr.finish()),
            Arc::new(subject_arr.finish()),
            Arc::new(date_arr.finish()),
            Arc::new(client_date_arr.finish()),
            Arc::new(message_id_arr.finish()),
            Arc::new(in_reply_to_arr.finish()),
            Arc::new(references_arr.finish()),
            Arc::new(x_mailing_list_arr.finish()),
            Arc::new(trailers_arr.finish()),
            Arc::new(code_arr.finish()),
            Arc::new(raw_body_arr.finish()),
            Arc::new(file_name_arr.finish()),
        ],
    )?;

    Ok(batch)
}
