use std::collections::HashMap;

/// A trailer line extracted from an email signature block.
///
/// Common examples: `Signed-off-by`, `Reviewed-by`, `Tested-by`, `Acked-by`.
#[derive(Debug, Clone, PartialEq)]
pub struct Attribution {
    /// The trailer tag (e.g. `Signed-off-by`, `Reviewed-by`)
    pub attribution: String,
    /// The person identifiation in `Name <email>` form
    pub identification: String,
}

/// A fully parsed email with headers, body, trailers, and code patches.
///
/// Produced by [`parse_email`](crate::email_parser::parse_email) and consumed
/// by [`build_record_batch`](crate::dataset_writer::build_record_batch).
#[derive(Debug, Clone)]
pub struct ParsedEmail {
    /// RFC 822 headers plus computed fields (`date`, `client-date`, etc.).
    /// Multi-value headers (to, cc, references) are delimited with `||`.
    pub headers: HashMap<String, String>,
    /// Full email body text, CRLF-normalized to LF.
    pub raw_body: String,
    /// Trailers extracted from the signature block.
    pub trailers: Vec<Attribution>,
    /// Code patches extracted from the email body.
    pub code: Vec<String>,
}
