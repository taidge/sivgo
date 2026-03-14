use std::fmt;

/// Errors that can occur in math operations.
#[derive(Debug, Clone, PartialEq)]
pub enum MathError {
    /// Failed to parse a string representation.
    ParseError(String),
}

impl fmt::Display for MathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MathError::ParseError(msg) => write!(f, "parse error: {}", msg),
        }
    }
}

impl std::error::Error for MathError {}
