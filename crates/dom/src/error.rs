//! Error types for the SVG DOM crate.

use thiserror::Error;

/// Errors that can occur when working with SVG documents.
#[derive(Debug, Error)]
pub enum DomError {
    /// Failed to parse SVG/XML input.
    #[error("SVG parse error: {0}")]
    ParseError(String),

    /// A referenced node was not found in the document.
    #[error("node not found: {0}")]
    NodeNotFound(String),

    /// An attribute value was invalid.
    #[error("invalid attribute: {0}")]
    InvalidAttribute(String),

    /// The document structure is invalid (e.g. no root element).
    #[error("invalid document: {0}")]
    InvalidDocument(String),

    /// An error from the underlying XML parser.
    #[error("XML error: {0}")]
    XmlError(String),
}

impl From<roxmltree::Error> for DomError {
    fn from(e: roxmltree::Error) -> Self {
        DomError::XmlError(e.to_string())
    }
}
