use regex::Regex;

use crate::Attribution;

pub fn extract_attributions(commit_message: &str) -> Vec<Attribution> {
    let mut attributions = Vec::new();

    // Split on signature marker
    let body = commit_message.split("\n-- \n").next().unwrap_or("");

    // Fix common copypaste trailer wrapping
    let re_copypaste = Regex::new(r"(?m)^(\S+:\s+[\da-f]+\s+\([^)]+)\n([^\n]+\))").unwrap();
    let body = re_copypaste.replace_all(body, "$1 $2");

    // Fix line broken signature: Signed-off-by: Long Name\n<email.here@example.com>
    let re_wrapped = Regex::new(r"(?m)^(\S+:\s+[^<]+)\n(<[^>]+>)$").unwrap();
    let body = re_wrapped.replace_all(&body, "$1 $2");

    let pattern = Regex::new(
        r"(?m)^\s*(?P<type>[a-zA-Z\-]+-by):\s*(?P<name>[^<\n]+?)\s*<(?P<email>[^>\n]+)>",
    )
    .unwrap();

    for caps in pattern.captures_iter(&body) {
        let attr_type = caps.name("type").map_or("", |m| m.as_str()).trim();
        let name = caps.name("name").map_or("", |m| m.as_str()).trim();
        let email = caps.name("email").map_or("", |m| m.as_str()).trim();
        attributions.push(Attribution {
            attribution: attr_type.to_string(),
            identification: format!("{} <{}>", name, email),
        });
    }

    attributions
}

/// TODO: improve patch capturing
pub fn extract_patches(email_body: &str) -> Vec<String> {
    let regexes: &[&str] = &[
        r"(^---$[\s\S]*?^--\s*\n^.*$)",
        r"(^---$[\s\S]*?^--[\s=]*$\n^.*$)",
        r"(diff --git[\s\S]*?^--\s*\n^.*$)",
        r"(^---$[\s\S]*?^--*[\S\s=]*$\n^.*$)",
    ];

    let mut patches = Vec::new();
    for pattern in regexes {
        let re = match regex::RegexBuilder::new(pattern).multi_line(true).build() {
            Ok(r) => r,
            Err(_) => continue,
        };
        for m in re.find_iter(email_body) {
            let value = m.as_str().trim().to_string();
            if !value.is_empty() {
                patches.push(value);
            }
        }
        if !patches.is_empty() {
            return patches;
        }
    }

    Vec::new()
}
