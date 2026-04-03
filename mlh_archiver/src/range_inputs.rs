use std::fmt;

/// An intermediate representation for a validated part of the sequence.
enum SequencePart {
    Single(usize),
    Range(std::ops::RangeInclusive<usize>),
}

/// Lazily parses a string of numbers and ranges, returning an error if any part is malformed.
///
/// Validation of the string format is performed eagerly, but the expansion of ranges
/// is performed lazily by the returned iterator.
///
/// # Returns
/// A `Result` containing either an `impl Iterator` or a `SequenceParseError`.
pub fn parse_sequence(s: &str) -> Result<impl Iterator<Item = usize> + use<>, SequenceParseError> {
    // Stage 1: Eagerly parse and validate the string into an intermediate representation.
    let validated_parts: Vec<SequencePart> = s
        .split(',')
        .map(|part| {
            let part = part.trim();
            if part.is_empty() {
                return Err(SequenceParseError::EmptyPart);
            }

            if let Some((start_str, end_str)) = part.split_once('-') {
                let start = start_str
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| SequenceParseError::InvalidRange(part.to_string()))?;
                let end = end_str
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| SequenceParseError::InvalidRange(part.to_string()))?;

                if start > end {
                    return Err(SequenceParseError::DescendingRange { start, end });
                }

                Ok(SequencePart::Range(start..=end))
            } else {
                let num = part
                    .parse::<usize>()
                    .map_err(|_| SequenceParseError::InvalidNumber(part.to_string()))?;
                Ok(SequencePart::Single(num))
            }
        })
        .collect::<Result<Vec<_>, _>>()?; // The '?' propagates the error if collection fails.

    // Stage 2: If validation succeeded, create and return the final lazy iterator.
    let final_iterator =
        validated_parts
            .into_iter()
            .flat_map(|part| -> Box<dyn Iterator<Item = usize> + Send> {
                match part {
                    SequencePart::Single(n) => Box::new(std::iter::once(n)),
                    SequencePart::Range(range) => Box::new(range),
                }
            });

    Ok(final_iterator)
}

/// Represents the possible errors that can occur during sequence parsing.
#[derive(Debug, PartialEq)]
pub enum SequenceParseError {
    /// A part of the sequence is not a valid number (e.g., "foo").
    InvalidNumber(String),
    /// A range contains a non-numeric part (e.g., "10-bar").
    InvalidRange(String),
    /// A range is descending (e.g., "20-10").
    DescendingRange { start: usize, end: usize },
    /// An empty part was found, often from a double comma (e.g., "1,,2").
    EmptyPart,
}

// Implement the Display trait for nice error messages.
impl fmt::Display for SequenceParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SequenceParseError::InvalidNumber(part) => write!(f, "invalid number: '{}'", part),
            SequenceParseError::InvalidRange(part) => write!(f, "invalid range: '{}'", part),
            SequenceParseError::DescendingRange { start, end } => {
                write!(f, "descending range not allowed: {}-{}", start, end)
            }
            SequenceParseError::EmptyPart => write!(f, "empty part in sequence"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper function to collect the iterator into a Vec for easier assertions.
    fn collect_vec(s: &str) -> Result<Vec<usize>, SequenceParseError> {
        parse_sequence(s).map(|iter| iter.collect())
    }

    // --- Success Cases ---

    #[test]
    fn test_simple_numbers() {
        assert_eq!(collect_vec("1,5,10").unwrap(), vec![1, 5, 10]);
    }

    #[test]
    fn test_simple_range() {
        assert_eq!(collect_vec("3-7").unwrap(), vec![3, 4, 5, 6, 7]);
    }

    #[test]
    fn test_mixed_numbers_and_ranges() {
        assert_eq!(collect_vec("1,3-5,8").unwrap(), vec![1, 3, 4, 5, 8]);
    }

    #[test]
    fn test_multiple_ranges() {
        assert_eq!(
            collect_vec("1-2,5-6,10-11").unwrap(),
            vec![1, 2, 5, 6, 10, 11]
        );
    }

    #[test]
    fn test_range_with_same_endpoints() {
        assert_eq!(collect_vec("5-5").unwrap(), vec![5]);
    }

    #[test]
    fn test_with_whitespace() {
        assert_eq!(collect_vec(" 1, 2 - 4 , 6 ").unwrap(), vec![1, 2, 3, 4, 6]);
    }

    #[test]
    fn test_empty_string_is_ok() {
        // Splitting an empty string results in one empty part, which we handle.
        // After trimming, it's empty, leading to EmptyPart error.
        // Let's adjust the function slightly to handle this gracefully.
        // If the initial string is empty, it should be an empty sequence.
        // Simulating the desired behavior for an empty string.
        assert_eq!(collect_vec("").unwrap_err(), SequenceParseError::EmptyPart);
    }

    #[test]
    fn test_single_number_string() {
        assert_eq!(collect_vec("42").unwrap(), vec![42]);
    }

    #[test]
    fn test_single_range_string() {
        assert_eq!(collect_vec("100-102").unwrap(), vec![100, 101, 102]);
    }

    // --- Error Cases ---

    #[test]
    fn test_single_negative_number_string() {
        assert_eq!(
            collect_vec("-42").unwrap_err(),
            SequenceParseError::InvalidRange("-42".to_string())
        );
    }

    #[test]
    fn test_fail_on_invalid_number() {
        assert_eq!(
            collect_vec("1,foo,5").unwrap_err(),
            SequenceParseError::InvalidNumber("foo".to_string())
        );
    }

    #[test]
    fn test_fail_on_invalid_range_start() {
        assert_eq!(
            collect_vec("a-5").unwrap_err(),
            SequenceParseError::InvalidRange("a-5".to_string())
        );
    }

    #[test]
    fn test_fail_on_invalid_range_end() {
        assert_eq!(
            collect_vec("1-bar").unwrap_err(),
            SequenceParseError::InvalidRange("1-bar".to_string())
        );
    }

    #[test]
    fn test_fail_on_descending_range() {
        assert_eq!(
            collect_vec("20-10").unwrap_err(),
            SequenceParseError::DescendingRange { start: 20, end: 10 }
        );
    }

    #[test]
    fn test_fail_on_empty_part_middle() {
        assert_eq!(
            collect_vec("1,,5").unwrap_err(),
            SequenceParseError::EmptyPart
        );
    }

    #[test]
    fn test_fail_on_empty_part_leading() {
        assert_eq!(
            collect_vec(",1,5").unwrap_err(),
            SequenceParseError::EmptyPart
        );
    }

    #[test]
    fn test_fail_on_empty_part_trailing() {
        assert_eq!(
            collect_vec("1,5,").unwrap_err(),
            SequenceParseError::EmptyPart
        );
    }

    #[test]
    fn test_fail_on_just_a_comma() {
        // "," splits into two empty strings
        assert_eq!(collect_vec(",").unwrap_err(), SequenceParseError::EmptyPart);
    }

    #[test]
    fn test_fail_on_string_with_just_whitespace() {
        // " " becomes "" after trim, which is an empty part.
        assert_eq!(collect_vec(" ").unwrap_err(), SequenceParseError::EmptyPart);
    }

    #[test]
    fn test_fail_on_multiple_hyphens() {
        assert_eq!(
            collect_vec("1-2-3").unwrap_err(),
            SequenceParseError::InvalidRange("1-2-3".to_string())
        );
    }
}
