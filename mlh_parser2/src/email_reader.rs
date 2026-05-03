use mail_parser::{Message, MessageParser};
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

static HEADER_LINE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(From|To|Cc|Subject|Date|Message-ID|In-Reply-To|References|User-Agent|X-Mailer):[ \t]*.*$",
    )
    .unwrap()
});

static EMAIL_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^[\s]*([^<]*?)?\s*<?([a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,})>?\s*$",
    )
    .unwrap()
});

static EMAIL_OBFUSCATED_A_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^[\s]*([^<]*?)?\s*<?([a-zA-Z0-9._%+-]+)\s*\(a\)\s*([a-zA-Z0-9.-]+\.[a-zA-Z]{2,})>?\s*$",
    )
    .unwrap()
});

static EMAIL_OBFUSCATED_AT_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^[\s]*([^<]*?)?\s*<?([a-zA-Z0-9._%+-]+)\s+at\s+([a-zA-Z0-9.-]+\.[a-zA-Z]{2,})>?\s*$",
    )
    .unwrap()
});

// --- Helper functions ---

fn hdr_value_to_string(val: &mail_parser::HeaderValue<'_>) -> String {
    match val {
        mail_parser::HeaderValue::Text(s) => s.to_string(),
        mail_parser::HeaderValue::TextList(v) => v.join(", "),
        mail_parser::HeaderValue::Address(a) => {
            if let Some(first) = a.first() {
                addr_to_string(first)
            } else {
                String::new()
            }
        }
        mail_parser::HeaderValue::DateTime(d) => d.to_rfc3339(),
        mail_parser::HeaderValue::ContentType(ct) => {
            let st = if let Some(ref s) = ct.c_subtype {
                format!("{}/{}", ct.c_type, s)
            } else {
                ct.c_type.to_string()
            };
            if let Some(ref attrs) = ct.attributes {
                let attr_strs: Vec<String> = attrs
                    .iter()
                    .map(|a| format!("{}={}", a.name, a.value))
                    .collect();
                format!("{}; {}", st, attr_strs.join("; "))
            } else {
                st
            }
        }
        other => format!("{:?}", other),
    }
}

fn addr_to_string(addr: &mail_parser::Addr<'_>) -> String {
    let name = addr.name.as_deref().unwrap_or("").to_string();
    let email = addr.address.as_deref().unwrap_or("").to_string();
    if name.is_empty() {
        email
    } else if email.is_empty() {
        name
    } else {
        format!("{} <{}>", name, email)
    }
}

fn score_email_address(value: &str) -> (bool, bool, Option<&'static str>) {
    if let Some(caps) = EMAIL_PATTERN.captures(value) {
        let name = caps.get(1).map_or("", |m| m.as_str()).trim();
        return (!name.is_empty(), true, None);
    }
    if let Some(caps) = EMAIL_OBFUSCATED_A_PATTERN.captures(value) {
        let name = caps.get(1).map_or("", |m| m.as_str()).trim();
        return (!name.is_empty(), false, Some("(a)"));
    }
    if let Some(caps) = EMAIL_OBFUSCATED_AT_PATTERN.captures(value) {
        let name = caps.get(1).map_or("", |m| m.as_str()).trim();
        return (!name.is_empty(), false, Some(" at "));
    }
    (false, false, None)
}

fn normalize_email(value: &str) -> String {
    if let Some(caps) = EMAIL_OBFUSCATED_A_PATTERN.captures(value) {
        let name = caps.get(1).map_or("", |m| m.as_str()).trim();
        let email = format!(
            "{}@{}",
            caps.get(2).unwrap().as_str(),
            caps.get(3).unwrap().as_str()
        );
        if name.is_empty() {
            return email;
        }
        return format!("{} <{}>", name, email);
    }
    if let Some(caps) = EMAIL_OBFUSCATED_AT_PATTERN.captures(value) {
        let name = caps.get(1).map_or("", |m| m.as_str()).trim();
        let email = format!(
            "{}@{}",
            caps.get(2).unwrap().as_str(),
            caps.get(3).unwrap().as_str()
        );
        if name.is_empty() {
            return email;
        }
        return format!("{} <{}>", name, email);
    }
    value.to_string()
}

fn select_best_from_header(values: &[String]) -> String {
    if values.is_empty() {
        return String::new();
    }
    if values.len() == 1 {
        return normalize_email(&values[0]);
    }

    let mut scored: Vec<((bool, bool, Option<&str>), &String)> = values
        .iter()
        .map(|v| (score_email_address(v), v))
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    normalize_email(scored[0].1)
}

fn extract_all_from_from_body(raw_email: &[u8]) -> Vec<String> {
    let email_text = String::from_utf8_lossy(raw_email);
    let mut candidates = Vec::new();

    let from_patterns: &[&str] = &[
        r"(?im)^From:\s*([^<\n]*?)?\s*<([a-zA-Z0-9._%+-]+)@([a-zA-Z0-9.-]+\.[a-zA-Z]{2,})>",
        r"(?im)^From:\s*([^<\n]*?)?\s*<([a-zA-Z0-9._%+-]+)\s*\(a\)\s*([a-zA-Z0-9.-]+\.[a-zA-Z]{2,})>",
        r"(?im)^From:\s*([^<\n]*?)?\s*<?([a-zA-Z0-9._%+-]+)\s+at\s+([a-zA-Z0-9.-]+\.[a-zA-Z]{2,})>?",
    ];

    for pat_str in from_patterns {
        let re = Regex::new(pat_str).unwrap();
        for caps in re.captures_iter(&email_text) {
            let name = caps.get(1).map_or("", |m| m.as_str()).trim();
            let email = format!(
                "{}@{}",
                caps.get(2).unwrap().as_str(),
                caps.get(3).unwrap().as_str()
            );
            if name.is_empty() {
                candidates.push(email);
            } else {
                candidates.push(format!("{} <{}>", name, email));
            }
        }
    }

    candidates
}

fn clean_body_leading_headers(body: &str) -> String {
    if body.is_empty() {
        return body.to_string();
    }

    let lines: Vec<&str> = body.lines().collect();
    let mut start_idx = 0;

    for (i, line) in lines.iter().enumerate() {
        let stripped = line.trim();
        if stripped.is_empty() {
            start_idx = i + 1;
            break;
        }
        if HEADER_LINE_PATTERN.is_match(stripped) {
            start_idx = i + 1;
        } else {
            break;
        }
    }

    while start_idx < lines.len() && lines[start_idx].trim().is_empty() {
        start_idx += 1;
    }

    if start_idx == 0 {
        body.to_string()
    } else {
        lines[start_idx..].join("\n")
    }
}

// --- Public API ---

pub fn decode_mail(email_raw: &[u8]) -> Option<Message<'_>> {
    MessageParser::default().parse(email_raw)
}

pub fn get_headers(msg: &Message<'_>, raw_email: &[u8]) -> HashMap<String, String> {
    let mut headers: HashMap<String, String> = HashMap::new();
    let mut from_candidates: Vec<String> = Vec::new();

    for header in msg.headers() {
        let key = header.name().to_lowercase();
        let val_str = hdr_value_to_string(header.value());

        if key == "from" {
            from_candidates.push(val_str);
        } else if key == "date" {
            headers.insert("date".to_string(), val_str);
        } else {
            headers
                .entry(key)
                .and_modify(|existing| {
                    *existing = format!("{}, {}", existing, &val_str);
                })
                .or_insert(val_str);
        }
    }

    if from_candidates.is_empty()
        && let Some(from) = msg.from() {
            for addr in from.iter() {
                from_candidates.push(addr_to_string(addr));
            }
        }

    let body_from = extract_all_from_from_body(raw_email);
    from_candidates.extend(body_from);

    if !from_candidates.is_empty() {
        headers.insert("from".to_string(), select_best_from_header(&from_candidates));
    }

    headers
}

pub fn get_body(msg: &Message<'_>) -> String {
    let mut body_parts: Vec<String> = Vec::new();

    for i in 0.. {
        if let Some(text) = msg.body_text(i) {
            if !text.is_empty() {
                body_parts.push(text.to_string());
            }
        } else {
            break;
        }
    }

    if body_parts.is_empty() {
        for i in 0.. {
            if let Some(html) = msg.body_html(i) {
                if !html.is_empty() {
                    body_parts.push(html.to_string());
                }
            } else {
                break;
            }
        }
    }

    let body = body_parts.join("\n");
    let body = body.replace("\r\n", "\n");
    clean_body_leading_headers(&body)
}
