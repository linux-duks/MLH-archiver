use crate::constants::{self, PARQUET_FILE_NAME, SINGLE_VALUED_COLS};
use crate::date_parser;
use crate::email_file_reader;
use crate::errors::ParseError;
use crate::extractors::{self, ParsedEmail};
use arrow::array::*;
use arrow::datatypes::*;
use arrow::record_batch::RecordBatch;
use chrono::{DateTime, FixedOffset};
use parquet::arrow::ArrowWriter;
use parquet::errors::Result;
use parquet::file::properties::WriterProperties;
use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub fn sanitize_surrogate_characters(email_dict: &mut ParsedEmail) {
    let sanitize_str = |s: &mut String| {
        let mut fixed = String::new();
        for c in s.chars() {
            let code = c as u32;
            if (0xD800..=0xDFFF).contains(&code) {
                fixed.push('\u{FFFD}');
            } else {
                fixed.push(c);
            }
        }
        *s = fixed;
    };

    for value in email_dict.headers.values_mut() {
        sanitize_str(value);
    }
    sanitize_str(&mut email_dict.raw_body);
    for attr in &mut email_dict.trailers {
        sanitize_str(&mut attr.attribution);
        sanitize_str(&mut attr.identification);
    }
    for patch in &mut email_dict.code {
        sanitize_str(patch);
    }
}

pub fn post_process_parsed_mail(email_dict: &mut ParsedEmail, now: DateTime<FixedOffset>) {
    let to = email_dict.headers.get("to").cloned().unwrap_or_default();
    if to.is_empty() {
        email_dict.headers.insert("to".to_string(), String::new());
    } else {
        let to_list: Vec<String> = to
            .split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect();
        email_dict
            .headers
            .insert("to".to_string(), to_list.join("||"));
    }

    let cc = email_dict.headers.get("cc").cloned().unwrap_or_default();
    if cc.is_empty() {
        email_dict.headers.insert("cc".to_string(), String::new());
    } else {
        let cc_list: Vec<String> = cc
            .split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect();
        email_dict
            .headers
            .insert("cc".to_string(), cc_list.join("||"));
    }

    let references = email_dict
        .headers
        .get("references")
        .cloned()
        .unwrap_or_default();
    if references.is_empty() {
        email_dict
            .headers
            .insert("references".to_string(), String::new());
    } else {
        let refs: Vec<String> = references
            .split_whitespace()
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect();
        email_dict
            .headers
            .insert("references".to_string(), refs.join("||"));
    }

    for column in SINGLE_VALUED_COLS {
        let has_value = email_dict.headers.contains_key(*column);
        if !has_value {
            if *column == "date" {
                email_dict.headers.insert(column.to_string(), String::new());
            } else {
                email_dict.headers.insert(column.to_string(), String::new());
            }
        }
    }

    let mut date_map = email_dict.headers.clone();
    date_parser::process_date(&mut date_map, now);
    if let Some(processed_date) = date_map.get("date") {
        email_dict
            .headers
            .insert("date".to_string(), processed_date.clone());
    }
    if let Some(client_date) = date_map.get("client-date") {
        email_dict
            .headers
            .insert("client-date".to_string(), client_date.clone());
    }
}

pub fn parse_and_process_email(
    email_data: &[u8],
    now: DateTime<FixedOffset>,
) -> Result<ParsedEmail, ParseError> {
    let mut email = extractors::parse_email_bytes_to_dict(email_data)?;
    post_process_parsed_mail(&mut email, now);
    Ok(email)
}

pub fn get_email_id(email_content: &str) -> Result<String, ParseError> {
    for line in email_content.lines() {
        if line.to_lowercase().starts_with("message-id:") {
            let message_id = line["message-id:".len()..].trim();
            return Ok(message_id.to_string());
        }
    }
    Err(ParseError::NoMessageId)
}

pub fn remove_previous_errors(errors_dir: &Path) -> Result<(), std::io::Error> {
    if errors_dir.is_dir() {
        for entry in fs::read_dir(errors_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                fs::remove_file(entry.path())?;
            }
        }
    }
    Ok(())
}

fn collect_email_files(input_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(input_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let ext = path
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();
                if ext == "eml" || ext == "parquet" {
                    files.push(path);
                }
            }
        }
    }
    files.sort();
    files
}

fn build_record_batch(
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

pub fn parse_mail_at(
    mailing_list: &str,
    input_dir: &Path,
    output_dir: &Path,
    fail_on_error: bool,
    max_records_per_batch: usize,
    max_raw_bytes_per_batch: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let list_input_path = input_dir.join(mailing_list);
    let list_output_path = output_dir.join(mailing_list);

    // TODO: reorganize paths
    let parquet_dir_path = output_dir.join("parsed");
    let success_output_path = parquet_dir_path.join(format!("list={}", mailing_list));
    let parquet_path = success_output_path.join(PARQUET_FILE_NAME);
    let error_output_path = list_output_path.join("errors");

    if !list_output_path.is_dir() {
        log::info!("First parse of list '{}'", mailing_list);
        fs::create_dir_all(&parquet_dir_path)?;
        fs::create_dir_all(&list_output_path)?;
        fs::create_dir_all(&success_output_path)?;
        fs::create_dir_all(&error_output_path)?;
    } else {
        remove_previous_errors(&error_output_path)?;
    }

    let files = collect_email_files(&list_input_path);

    log::debug!("Collected a list of {} files. First 5:", files.len());
    if log::log_enabled!(log::Level::Debug) {
        for val in files.iter().take(5) {
            println!(" {}", val.clone().display());
        }
    }

    let now = FixedOffset::east_opt(0)
        .map(|tz| chrono::Utc::now().with_timezone(&tz))
        .unwrap_or_else(|| chrono::Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap()));

    let mut batch_emails: Vec<(ParsedEmail, String)> = Vec::new();
    let mut batch_raw_body_bytes: usize = 0;
    let mut total_parsed: usize = 0;
    let mut arrow_writer: Option<ArrowWriter<BufWriter<fs::File>>> = None;

    let emails = email_file_reader::file_iterator(files);

    for row in emails {
        let row = row?;

        match parse_and_process_email(row.content.as_bytes(), now) {
            Ok(mut email) => {
                sanitize_surrogate_characters(&mut email);
                let raw_len = email.raw_body.len();
                batch_emails.push((email, row.email_id));
                batch_raw_body_bytes += raw_len;
            }
            Err(e) => {
                log::error!("Failed to parse email {}: {}", row.email_id, e);
                if fail_on_error {
                    return Err(Box::new(e));
                }
            }
        }

        let should_flush = batch_emails.len() >= max_records_per_batch
            || batch_raw_body_bytes >= max_raw_bytes_per_batch;

        if should_flush {
            flush_batch(
                mailing_list,
                &parquet_path,
                &mut batch_emails,
                &mut batch_raw_body_bytes,
                &mut total_parsed,
                &mut arrow_writer,
            )?;
        }
    }

    // Flush any remaining emails
    if !batch_emails.is_empty() {
        flush_batch(
            mailing_list,
            &parquet_path,
            &mut batch_emails,
            &mut batch_raw_body_bytes,
            &mut total_parsed,
            &mut arrow_writer,
        )?;
    }

    if let Some(writer) = arrow_writer {
        writer.close()?;
    } else {
        log::warn!("No emails parsed successfully for list '{}'", mailing_list);
        return Ok(());
    }

    log::info!(
        "Saved {} parsed emails for list '{}'",
        total_parsed,
        mailing_list
    );

    Ok(())
}

fn flush_batch(
    mailing_list: &str,
    parquet_path: &Path,
    batch_emails: &mut Vec<(ParsedEmail, String)>,
    batch_raw_body_bytes: &mut usize,
    total_parsed: &mut usize,
    arrow_writer: &mut Option<ArrowWriter<BufWriter<fs::File>>>,
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
