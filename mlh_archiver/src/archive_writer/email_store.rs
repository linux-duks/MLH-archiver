use std::convert::TryFrom;
use std::fmt;
use std::str::FromStr;

pub struct EmailData {
    pub email_id: String,
    pub content: String,
}

pub trait EmailStore: Send {
    fn add_email(&mut self, email: EmailData) -> crate::Result<Option<Vec<String>>>;
    fn close(&mut self) -> crate::Result<Option<Vec<String>>>;
}

#[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq, Clone, Copy)]
#[serde(try_from = "String")]
pub enum WriteMode {
    RawEmails,
    Parquet { buffer_size: usize },
}

impl Default for WriteMode {
    fn default() -> Self {
        WriteMode::Parquet {
            buffer_size: 10_000,
        }
    }
}

impl fmt::Display for WriteMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WriteMode::RawEmails => write!(f, "raw_email"),
            WriteMode::Parquet { buffer_size } => write!(f, "parquet:{}", buffer_size),
        }
    }
}

impl FromStr for WriteMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let cleaned = s.to_lowercase();

        // 1. Handle RawEmails (Permissive mapping)
        if cleaned == "raw" || cleaned == "raw_email" || cleaned == "rawemail" {
            return Ok(WriteMode::RawEmails);
        }

        // 2. Handle Parquet
        if cleaned.starts_with("parquet") {
            // Find the first digit in the string
            let start_idx = cleaned.find(|c: char| c.is_ascii_digit()).ok_or_else(|| {
                format!(
                    "Parquet mode requires a buffer size (e.g., 'parquet:1000'). Received: {}",
                    s
                )
            })?;

            // Extract the numeric sequence starting from that first digit
            let end_idx = cleaned[start_idx..]
                .find(|c: char| !c.is_ascii_digit())
                .map(|i| i + start_idx)
                .unwrap_or(cleaned.len());

            let buffer_size = cleaned[start_idx..end_idx]
                .parse::<usize>()
                .map_err(|_| format!("Could not parse buffer size from: {}", s))?;

            return Ok(WriteMode::Parquet { buffer_size });
        }

        Err(format!(
            "Unknown WriteMode: '{}'. Valid options are 'raw' or 'parquet:SIZE'",
            s
        ))
    }
}

impl TryFrom<String> for WriteMode {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse() // This calls your FromStr implementation
    }
}

// Also helpful to implement for &str to be thorough
impl TryFrom<&str> for WriteMode {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        s.parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsing_write_mode() {
        let cases = vec![
            ("raw", WriteMode::RawEmails),
            ("raw_email", WriteMode::RawEmails),
            ("RawEmail", WriteMode::RawEmails),
            ("parquet:1000", WriteMode::Parquet { buffer_size: 1000 }),
            (
                "parquet:buffer_size:1000",
                WriteMode::Parquet { buffer_size: 1000 },
            ),
            (
                "parquet{buffer_size=1000}",
                WriteMode::Parquet { buffer_size: 1000 },
            ),
            (
                "Parquet(buffer_size:1000)",
                WriteMode::Parquet { buffer_size: 1000 },
            ),
        ];

        for (input, expected) in cases {
            assert_eq!(WriteMode::from_str(input).unwrap(), expected);
        }
    }

    #[test]
    fn test_serde_yaml_parsing() {
        // A YAML list containing all your permissive formats
        let yaml_input = r#"
            - raw
            - raw_email
            - RawEmail
            - "parquet:1000"
            - "parquet:buffer_size:1000"
            - "parquet{buffer_size=1000}"
            - "Parquet:
                buffer_size: 1000"
        "#;

        let results: Vec<WriteMode> =
            serde_yaml::from_str(yaml_input).expect("Failed to parse YAML");

        // Verify Raw variations
        assert_eq!(results[0], WriteMode::RawEmails);
        assert_eq!(results[1], WriteMode::RawEmails);
        assert_eq!(results[2], WriteMode::RawEmails);

        // Verify Parquet variations
        let expected_parquet = WriteMode::Parquet { buffer_size: 1000 };
        assert_eq!(results[3], expected_parquet);
        assert_eq!(results[4], expected_parquet);
        assert_eq!(results[5], expected_parquet);
        assert_eq!(results[6], expected_parquet);
    }

    #[test]
    fn test_invalid_input() {
        let yaml_input = "unknown_mode";
        let result: Result<WriteMode, _> = serde_yaml::from_str(yaml_input);
        assert!(result.is_err());
    }
}
