use chrono::{Datelike, DateTime, FixedOffset, NaiveDate, NaiveDateTime, TimeZone, Utc};
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

static DATE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    let rfc2822 = r"(?:(Sun|Mon|Tue|Wed|Thu|Fri|Sat),\s+)?(0[1-9]|[1-2]?[0-9]|3[01])\s+(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+(19[0-9]{2}|[2-9][0-9]{3})\s+(2[0-3]|[0-1][0-9]):([0-5][0-9])(?::(60|[0-5][0-9]))?\s+([-\+][0-9]{2}[0-5][0-9]|(?:UT|GMT|(?:E|C|M|P)(?:ST|DT)|[A-IK-Z]))";
    let rfc1123 = r"\w{3}, \d{2} \w{3} \d{4} \d{2}:\d{2}:\d{2} \w{3}";
    let rfc1036 = r"\w+?, \d{2}-\w{3}-\d{2} \d{2}:\d{2}:\d{2} \w{3}";
    let ctime = r"\w{3} \w{3} \d+? \d{2}:\d{2}:\d{2} \d{4}";
    Regex::new(&format!(
        "(?:{})|(?:{})|(?:{})|(?:{})",
        rfc2822, rfc1123, rfc1036, ctime
    ))
    .unwrap()
});

fn find_date_in_string(text: &str) -> Option<String> {
    DATE_REGEX.find(text).map(|m| m.as_str().to_string())
}

pub fn parse_date_tentative_raw(date: &str) -> Option<DateTime<FixedOffset>> {
    if date.is_empty() {
        return None;
    }

    let found = find_date_in_string(date)?;

    // Handle "(" comments in date strings
    let cleaned = if let Some(pos) = found.find('(') {
        found[..pos].trim().to_string()
    } else {
        found.clone()
    };

    // Try RFC 2822 format first
    if let Ok(dt) = DateTime::parse_from_rfc2822(&cleaned) {
        if has_valid_utc_offset(&dt) {
            return Some(dt);
        }
        return None;
    }

    // Try RFC 3339
    if let Ok(dt) = DateTime::parse_from_rfc3339(&cleaned) {
        if has_valid_utc_offset(&dt) {
            return Some(dt);
        }
        return None;
    }

    last_effort_date_finder(&found)
}

fn has_valid_utc_offset(dt: &DateTime<FixedOffset>) -> bool {
    let offset_secs = dt.offset().local_minus_utc();
    offset_secs > -24 * 3600 && offset_secs < 24 * 3600
}

fn last_effort_date_finder(date_text: &str) -> Option<DateTime<FixedOffset>> {
    let cleaned = if let Some(pos) = date_text.find('(') {
        date_text[..pos].trim().to_string()
    } else {
        date_text.to_string()
    };

    let attempts = vec![
        cleaned.clone(),
        cleaned.replace('.', ":"),
        cleaned
            .chars()
            .take("Fri, 15 Jun 2012 16:52:52".len())
            .collect(),
        cleaned
            .chars()
            .take("Fri, 5 Jun 2012 16:52:52".len())
            .collect(),
    ];

    for attempt in attempts {
        if let Ok(naive) = NaiveDateTime::parse_from_str(&attempt, "%a, %d %b %Y %H:%M:%S") {
            let dt = Utc
                .from_utc_datetime(&naive)
                .with_timezone(&FixedOffset::east_opt(0)?);
            return Some(dt);
        }
    }

    None
}

pub fn is_date_too_old(date_obj: &DateTime<FixedOffset>) -> bool {
    date_obj.year() < 1900
}

pub fn is_date_in_future(date_obj: &DateTime<FixedOffset>, now: DateTime<FixedOffset>) -> bool {
    let max_future = now + chrono::Duration::days(3);
    *date_obj > max_future
}

pub fn check_date_issues(date_obj: &DateTime<FixedOffset>, now: DateTime<FixedOffset>) -> bool {
    is_date_too_old(date_obj) || is_date_in_future(date_obj, now)
}

pub fn fix_millennium_date(
    date_obj: DateTime<FixedOffset>,
    now: DateTime<FixedOffset>,
) -> DateTime<FixedOffset> {
    let year = date_obj.year();
    let max_year = now.year();
    let adjusted = year + 1900;
    if year < 1900 && adjusted <= max_year
        && let Some(new_date) = NaiveDate::from_ymd_opt(adjusted, date_obj.month(), date_obj.day())
        {
            let time = date_obj.time();
            let naive = new_date.and_time(time);
            let utc = Utc.from_utc_datetime(&naive);
            let offset = date_obj.offset();
            if let Some(fixed) = FixedOffset::east_opt(offset.local_minus_utc()) {
                return utc.with_timezone(&fixed);
            }
        }
    date_obj
}

pub fn find_other_date_entries(
    email_dict: &HashMap<String, String>,
) -> Vec<DateTime<FixedOffset>> {
    let mut value_list = Vec::new();
    for header in &["received", "x-received"] {
        if let Some(values_str) = email_dict.get(*header) {
            let res = find_date_in_string(values_str);
            if let Some(date_str) = res
                && let Some(parsed) = parse_date_tentative_raw(&date_str) {
                    value_list.push(parsed);
                }
        }
    }
    value_list
}

pub fn process_date(
    email_dict: &mut HashMap<String, String>,
    now: DateTime<FixedOffset>,
) {
    let raw_date = email_dict
        .get("date")
        .cloned()
        .map(|d| vec![d])
        .unwrap_or_default();

    let client_date: Vec<String> = raw_date
        .iter()
        .filter(|d| !d.is_empty())
        .cloned()
        .collect();
    email_dict.insert("client-date".to_string(), client_date.join("||"));

    let mut date_options: Vec<DateTime<FixedOffset>> = Vec::new();
    for date in &client_date {
        if !date.is_empty() {
            let trimmed = date.trim();
            if let Some(date_str) = find_date_in_string(trimmed)
                && let Some(dt) = parse_date_tentative_raw(&date_str) {
                    date_options.push(dt);
                }
        }
    }

    let mut safe_options: Vec<DateTime<FixedOffset>> = date_options
        .iter()
        .filter(|d| !check_date_issues(d, now))
        .cloned()
        .collect();

    if safe_options.is_empty() {
        safe_options = find_other_date_entries(email_dict);
    }

    if safe_options.is_empty() {
        let millennium_dates: Vec<DateTime<FixedOffset>> = date_options
            .iter()
            .filter(|d| is_date_too_old(d))
            .cloned()
            .collect();
        for d in millennium_dates {
            safe_options.push(fix_millennium_date(d, now));
        }
    }

    if !safe_options.is_empty() {
        safe_options.sort();
        email_dict.insert("date".to_string(), safe_options[0].to_rfc3339());
    } else {
        email_dict.insert("date".to_string(), String::new());
    }
}
