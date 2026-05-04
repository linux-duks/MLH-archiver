use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Attribution {
    pub attribution: String,
    pub identification: String,
}

#[derive(Debug, Clone)]
pub struct ParsedEmail {
    pub headers: HashMap<String, String>,
    pub raw_body: String,
    pub trailers: Vec<Attribution>,
    pub code: Vec<String>,
}
