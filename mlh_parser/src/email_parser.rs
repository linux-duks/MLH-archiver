//! Top-level email parsing: decodes raw bytes into a [`ParsedEmail`].

use crate::ParsedEmail;
use crate::constants::SINGLE_VALUED_COLS;
use crate::date_parser;
use crate::email_reader;
use crate::errors::ParseError;
use crate::extractors::{self};

use chrono::{DateTime, FixedOffset};
use parquet::errors::Result;

/// Parses a raw RFC 822 email byte slice into a [`ParsedEmail`].
///
/// Extracts headers, body text, trailers, and code patches. Dates are
/// normalized by [`process_date`](crate::date_parser::process_date). Missing
/// single-valued columns are populated with empty strings.
pub fn parse_email(
    email_data: &[u8],
    now: DateTime<FixedOffset>,
) -> Result<ParsedEmail, ParseError> {
    let msg = email_reader::decode_mail(email_data)
        .ok_or_else(|| ParseError::DecodeError("Failed to parse email bytes".to_string()))?;

    let raw_body = email_reader::get_body(&msg);
    let headers = email_reader::get_headers(&msg);

    let trailers = extractors::extract_attributions(&raw_body);
    let code = extractors::extract_patches(&raw_body);

    let mut email = ParsedEmail {
        headers,
        raw_body,
        trailers,
        code,
    };

    post_process_parsed_mail(&mut email, now);
    Ok(email)
}

fn post_process_parsed_mail(email: &mut ParsedEmail, now: DateTime<FixedOffset>) {
    let to = email.headers.get("to").cloned().unwrap_or_default();
    if to.is_empty() {
        email.headers.insert("to".to_string(), String::new());
    } else {
        let to_list: Vec<String> = to
            .split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect();
        email.headers.insert("to".to_string(), to_list.join("||"));
    }

    let cc = email.headers.get("cc").cloned().unwrap_or_default();
    if cc.is_empty() {
        email.headers.insert("cc".to_string(), String::new());
    } else {
        let cc_list: Vec<String> = cc
            .split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect();
        email.headers.insert("cc".to_string(), cc_list.join("||"));
    }

    let references = email.headers.get("references").cloned().unwrap_or_default();
    if references.is_empty() {
        email
            .headers
            .insert("references".to_string(), String::new());
    } else {
        let refs: Vec<String> = references
            .split_whitespace()
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect();
        email
            .headers
            .insert("references".to_string(), refs.join("||"));
    }

    for column in SINGLE_VALUED_COLS {
        let has_value = email.headers.contains_key(*column);
        if !has_value {
            email.headers.insert(column.to_string(), String::new());
        }
    }

    let mut date_map = email.headers.clone();
    date_parser::process_date(&mut date_map, now);
    if let Some(processed_date) = date_map.get("date") {
        email
            .headers
            .insert("date".to_string(), processed_date.clone());
    }
    if let Some(client_date) = date_map.get("client-date") {
        email
            .headers
            .insert("client-date".to_string(), client_date.clone());
    }
}
