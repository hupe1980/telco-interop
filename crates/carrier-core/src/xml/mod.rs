//! The hardened XML foundation: a bounded tree parser, a cursor that walks an element's
//! children in schema order, and an indenting writer.
//!
//! Input from another operator is untrusted. The parser refuses document type declarations
//! (so no entity can be declared, let alone expanded), resolves only the five predefined
//! entities and character references, accepts UTF-8 only and bounds input size, nesting depth,
//! element count and text length ([`Limits`]). Errors name the element path and byte offset,
//! never element content.
//!
//! Neither the WBCI nor the S/PRI schemas set `elementFormDefault`, so only the global payload
//! elements are namespace-qualified; the cursor matches local elements without a namespace.

mod read;
mod write;

pub use read::{Node, Seq, leaf, parse, parse_date, parse_zeitstempel, unwrap_soap};
pub use write::{WriteError, WriteErrorKind, Writer, escape};

use core::fmt;

use crate::ValueError;

/// The XML Schema instance namespace (`xsi:type`).
pub const NS_XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";
/// The SOAP 1.1 envelope namespace.
pub const NS_SOAP: &str = "http://schemas.xmlsoap.org/soap/envelope/";

/// Bounds on what the reader accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Input size in bytes. Default 1 MiB; a large WBCI payload is a few kilobytes.
    pub max_bytes: usize,
    /// Element nesting depth, SOAP envelope included. Default 32.
    pub max_depth: usize,
    /// Number of elements. Default 10 000.
    pub max_elements: usize,
    /// Characters of text in one element. Default 4 096.
    pub max_text: usize,
    /// Bytes of one embedded document after base64 decoding (S/PRI `anlage`). Default 1 MiB.
    /// Elements that carry such a document may hold `2 × max_attachment` characters of text
    /// (base64 plus line breaks) instead of [`Limits::max_text`].
    pub max_attachment: usize,
}

impl Limits {
    /// The defaults, with room in [`Limits::max_bytes`] for one embedded document of
    /// `max_attachment` bytes in base64 (4 characters per 3 bytes) besides the 1 MiB of the
    /// rest.
    #[must_use]
    pub const fn for_attachments(max_attachment: usize) -> Self {
        let base64 = max_attachment.div_ceil(3).saturating_mul(4);
        Self { max_bytes: (1 << 20) + base64, max_attachment, ..Self::DEFAULT }
    }

    const DEFAULT: Self =
        Self { max_bytes: 1 << 20, max_depth: 32, max_elements: 10_000, max_text: 4096, max_attachment: 1 << 20 };
}

impl Default for Limits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Why input could not be read, before an interface maps it to its own error type.
///
/// The message names the element path and, where the parser knows it, the byte offset; it
/// never repeats element content, which may be personal data.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub struct XmlError {
    /// Element path, e.g. `annehmenAnfrageRequest/Anfrage/Endkunde/person/nachname`.
    pub path: String,
    /// Byte offset in the input, if known.
    pub offset: Option<u64>,
    /// What is wrong.
    pub kind: XmlErrorKind,
}

impl XmlError {
    /// An error at `path` without an offset.
    #[must_use]
    pub fn at(path: &str, kind: XmlErrorKind) -> Self {
        Self { path: path.to_owned(), offset: None, kind }
    }
}

impl fmt::Display for XmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            write!(f, "{}", self.kind)?;
        } else {
            write!(f, "{}: {}", self.path, self.kind)?;
        }
        if let Some(offset) = self.offset {
            write!(f, " (byte {offset})")?;
        }
        Ok(())
    }
}

/// What an [`XmlError`] reports.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum XmlErrorKind {
    /// The input is larger than [`Limits::max_bytes`].
    #[error("input of {actual} bytes exceeds the limit of {limit}")]
    TooLarge {
        /// The limit.
        limit: usize,
        /// The input size.
        actual: usize,
    },
    /// Elements nest deeper than [`Limits::max_depth`].
    #[error("elements nest deeper than {limit}")]
    TooDeep {
        /// The limit.
        limit: usize,
    },
    /// More elements than [`Limits::max_elements`].
    #[error("more than {limit} elements")]
    TooManyElements {
        /// The limit.
        limit: usize,
    },
    /// An element's text is longer than [`Limits::max_text`].
    #[error("text longer than {limit} characters")]
    TextTooLong {
        /// The limit.
        limit: usize,
    },
    /// An embedded document is larger than [`Limits::max_attachment`].
    #[error("embedded document larger than {limit} bytes")]
    AttachmentTooLarge {
        /// The limit in bytes.
        limit: usize,
    },
    /// An element repeats more often than the schema allows.
    #[error("{element} occurs more than {max} times")]
    TooMany {
        /// The element.
        element: &'static str,
        /// The schema's maximum.
        max: usize,
    },
    /// A document type declaration: refused, so no entity can be declared or expanded.
    #[error("DOCTYPE declarations are not accepted")]
    Doctype,
    /// An entity reference other than the five predefined ones.
    #[error("unknown entity reference")]
    Entity,
    /// The document is not well-formed or not UTF-8.
    #[error("malformed XML: {0}")]
    Malformed(String),
    /// A mandatory element is missing.
    #[error("missing element {0}")]
    Missing(&'static str),
    /// An element the schema does not allow here.
    #[error("unexpected element {0}")]
    Unexpected(String),
    /// Text where only elements are allowed, or elements in a text-only element.
    #[error("mixed content is not allowed")]
    MixedContent,
    /// An `xsi:type` that is missing, unknown or in the wrong namespace.
    #[error("xsi:type {0}")]
    XsiType(String),
    /// A value that violates its schema type.
    #[error(transparent)]
    Value(ValueError),
    /// A value of an enumeration, date, boolean or timestamp that does not parse.
    #[error("invalid {0}")]
    Invalid(&'static str),
}
