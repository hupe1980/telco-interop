//! Lexical facets of the carrier schemas: the permitted character set, whitespace handling and
//! lengths, and the macros that turn them into newtypes and code enumerations.
//!
//! Sources: XSD `wbci-type-simple.xsd` (V2.00, schema version 01.00.02),
//! `ErlaubterZeichenBereichType` and `ErlaubterZeichenBereichMehrzeiligType`; the S/PRI schema
//! 4.00.03 restricts `xsd:string` by length only ([`Charset::Xml`]).

use core::fmt;

/// A value that does not satisfy its schema type.
///
/// The error never contains the rejected text: values may be personal data, and errors end
/// up in logs. It names the type, the facet and, for a character, its position.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{type_name}: {kind}")]
pub struct ValueError {
    /// The schema type, e.g. `Nachname`.
    pub type_name: &'static str,
    /// Which facet was violated.
    pub kind: ValueErrorKind,
}

/// The facet a [`ValueError`] reports.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ValueErrorKind {
    /// Too short or too long, counted in characters as XSD does.
    Length {
        /// Minimum length.
        min: usize,
        /// Maximum length.
        max: usize,
        /// Actual length.
        actual: usize,
    },
    /// The value does not have the required shape.
    Pattern {
        /// The shape, in the XSD's notation.
        expected: &'static str,
    },
    /// A character outside the WBCI character set.
    Char {
        /// Zero-based character position.
        index: usize,
        /// The Unicode scalar value.
        codepoint: u32,
    },
    /// A sequence number too large for nine digits.
    Sequence {
        /// The largest permitted number.
        max: u64,
    },
    /// A character that XML text may not carry (a control character other than tab, line feed
    /// and carriage return, or a non-character).
    XmlChar {
        /// Zero-based character position.
        index: usize,
        /// The Unicode scalar value.
        codepoint: u32,
    },
}

impl fmt::Display for ValueErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { min, max, actual } if min == max => {
                write!(f, "length must be {min}, is {actual}")
            }
            Self::Length { min, max, actual } => {
                write!(f, "length must be {min}..={max}, is {actual}")
            }
            Self::Pattern { expected } => write!(f, "must match {expected}"),
            Self::Char { index, codepoint } => {
                write!(f, "character U+{codepoint:04X} at position {index} is outside the WBCI character set")
            }
            Self::Sequence { max } => write!(f, "sequence number must be at most {max}"),
            Self::XmlChar { index, codepoint } => {
                write!(f, "character U+{codepoint:04X} at position {index} is not permitted in XML text")
            }
        }
    }
}

/// Which characters a type admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Charset {
    /// WBCI `ErlaubterZeichenBereichType`.
    Wbci,
    /// WBCI `ErlaubterZeichenBereichMehrzeiligType`: adds line feed and carriage return.
    WbciMultiline,
    /// `xsd:string` without further restriction: XML characters, no control characters
    /// other than tab, line feed and carriage return.
    Xml,
}

/// The XSD `whiteSpace` facet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ws {
    /// Kept as written.
    Preserve,
    /// Tab, line feed and carriage return become spaces.
    Replace,
    /// `Replace`, then runs of spaces become one and the ends are trimmed.
    Collapse,
}

/// Whether `c` is in WBCI's `ErlaubterZeichenBereichType` (with line breaks if `multiline`).
///
/// The pattern admits U+007F although the schema's prose list does not name it; peers
/// validate against the pattern, so this follows the pattern (an interpretation).
#[must_use]
pub fn in_wbci_charset(c: char, multiline: bool) -> bool {
    matches!(c,
        '\u{09}'
        | '\u{20}'..='\u{5F}'
        | '\u{61}'..='\u{7B}'
        | '\u{7D}' | '\u{7F}' | '\u{A7}'
        | '\u{B0}'..='\u{B3}'
        | '\u{C0}'..='\u{D6}'
        | '\u{D8}'..='\u{F6}'
        | '\u{F8}'..='\u{FF}')
        || (multiline && matches!(c, '\n' | '\r'))
}

/// Whether `c` may appear in an XML 1.0 document at all.
fn is_xml_char(c: char) -> bool {
    matches!(c, '\u{9}' | '\u{A}' | '\u{D}' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..)
}

/// A shape check and its description in the XSD's notation.
pub type Shape = (fn(&str) -> bool, &'static str);

/// The lexical rules of one schema type.
#[derive(Debug, Clone, Copy)]
pub struct TextSpec {
    /// The type's name in errors.
    pub name: &'static str,
    /// Minimum length in characters.
    pub min: usize,
    /// Maximum length in characters.
    pub max: usize,
    /// The permitted characters.
    pub charset: Charset,
    /// The whitespace facet.
    pub ws: Ws,
    /// An extra shape check and its description, for patterned types.
    pub pattern: Option<Shape>,
}

impl TextSpec {
    /// A length-only `xsd:string` restriction: XML characters, whitespace preserved.
    #[must_use]
    pub const fn xml(name: &'static str, min: usize, max: usize) -> Self {
        Self { name, min, max, charset: Charset::Xml, ws: Ws::Preserve, pattern: None }
    }

    /// Normalises whitespace as the facet says, then checks characters, length and shape.
    ///
    /// # Errors
    ///
    /// [`ValueError`] naming the first violated facet.
    pub fn parse(&self, input: &str) -> Result<String, ValueError> {
        let value = match self.ws {
            Ws::Preserve => input.to_owned(),
            Ws::Replace => input.replace(['\t', '\n', '\r'], " "),
            Ws::Collapse => {
                input.split([' ', '\t', '\n', '\r']).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ")
            }
        };
        let err = |kind| ValueError { type_name: self.name, kind };
        for (index, c) in value.chars().enumerate() {
            let codepoint = u32::from(c);
            match self.charset {
                Charset::Wbci | Charset::WbciMultiline => {
                    if !in_wbci_charset(c, self.charset == Charset::WbciMultiline) {
                        return Err(err(ValueErrorKind::Char { index, codepoint }));
                    }
                }
                Charset::Xml => {
                    if !(is_xml_char(c) && !c.is_control() || matches!(c, '\t' | '\n' | '\r')) {
                        return Err(err(ValueErrorKind::XmlChar { index, codepoint }));
                    }
                }
            }
        }
        let len = value.chars().count();
        if len < self.min || len > self.max {
            return Err(err(ValueErrorKind::Length { min: self.min, max: self.max, actual: len }));
        }
        if let Some((check, expected)) = self.pattern
            && !check(&value)
        {
            return Err(err(ValueErrorKind::Pattern { expected }));
        }
        Ok(value)
    }
}

/// `n` to `m` ASCII digits.
#[must_use]
pub fn digits(s: &str, n: usize, m: usize) -> bool {
    (n..=m).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit())
}

/// `[0-9A-Z]{n,m}`.
#[must_use]
pub fn upper_alnum(s: &str, n: usize, m: usize) -> bool {
    (n..=m).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit() || b.is_ascii_uppercase())
}

/// Declares a newtype over `String` with a [`TextSpec`].
///
/// `redact` makes `Debug` print only the length: the type may hold personal data. The
/// invoking crate must export `ValueError` at its root.
#[macro_export]
macro_rules! text_type {
    (
        $(#[$meta:meta])*
        $name:ident, $spec:expr, $redact:tt
    ) => {
        $(#[$meta])*
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub(crate) const SPEC: $crate::text::TextSpec = $spec;

            /// Parses a value, applying the schema's whitespace facet first.
            ///
            /// # Errors
            ///
            /// [`ValueError`](crate::ValueError) naming the violated facet.
            pub fn new(value: impl AsRef<str>) -> Result<Self, $crate::ValueError> {
                Self::SPEC.parse(value.as_ref()).map(Self)
            }

            /// The value as written on the wire.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl core::str::FromStr for $name {
            type Err = $crate::ValueError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::new(s)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = $crate::ValueError;
            fn try_from(s: &str) -> Result<Self, Self::Error> {
                Self::new(s)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str(&self.0)
            }
        }

        $crate::text_type!(@debug $name, $redact);
    };
    (@debug $name:ident, redact) => {
        impl core::fmt::Debug for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(f, concat!(stringify!($name), "(<redacted, {} chars>)"), self.0.chars().count())
            }
        }
    };
    (@debug $name:ident, plain) => {
        impl core::fmt::Debug for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(f, concat!(stringify!($name), "({:?})"), self.0)
            }
        }
    };
}

/// Declares a `Copy` enum of schema codes with `as_str`, `from_code` and `Display`.
#[macro_export]
macro_rules! code_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident = $code:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $($(#[$vmeta])* $variant),+
        }

        impl $name {
            /// Every value, in the schema's order.
            pub const ALL: &'static [$name] = &[$(Self::$variant),+];

            /// The code on the wire.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $code),+ }
            }

            /// Parses the code on the wire.
            #[must_use]
            pub fn from_code(s: &str) -> Option<Self> {
                match s { $($code => Some(Self::$variant),)+ _ => None }
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAME: TextSpec =
        TextSpec { name: "Vorname", min: 1, max: 30, charset: Charset::Wbci, ws: Ws::Collapse, pattern: None };

    #[test]
    fn collapse_then_check() {
        assert_eq!(NAME.parse("  Anna \t Marie ").as_deref(), Ok("Anna Marie"));
    }

    #[test]
    fn umlauts_and_symbols_are_in_the_charset() {
        assert!(NAME.parse("Jürgen-Øle § °").is_ok());
    }

    // wbci-switch/SR-004: a control character is refused, naming character and position.
    #[test]
    fn control_character_is_refused_with_position() {
        let err = NAME.parse("An\u{7}na").unwrap_err();
        assert_eq!(err.kind, ValueErrorKind::Char { index: 2, codepoint: 7 });
        assert_eq!(err.to_string(), "Vorname: character U+0007 at position 2 is outside the WBCI character set");
    }

    #[test]
    fn characters_outside_latin1_and_excluded_ascii_are_refused() {
        for bad in ["€", "`", "|", "~", "Ł", "\u{202E}"] {
            assert!(NAME.parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn multiline_admits_line_breaks() {
        let spec = TextSpec { charset: Charset::WbciMultiline, ws: Ws::Preserve, ..NAME };
        assert!(spec.parse("a\r\nb").is_ok());
        assert!(NAME.parse("a\u{0}b").is_err());
    }

    #[test]
    fn length_is_counted_in_characters() {
        let thirty = "ä".repeat(30);
        assert!(NAME.parse(&thirty).is_ok());
        let err = NAME.parse(&"ä".repeat(31)).unwrap_err();
        assert_eq!(err.kind, ValueErrorKind::Length { min: 1, max: 30, actual: 31 });
        assert!(NAME.parse("   ").is_err());
    }

    #[test]
    fn xml_charset_admits_any_text_but_controls() {
        let spec = TextSpec::xml("Freitext", 1, 30);
        assert!(spec.parse("€ | ~ Ł 漢字\n").is_ok());
        let err = spec.parse("a\u{1}").unwrap_err();
        assert_eq!(err.kind, ValueErrorKind::XmlChar { index: 1, codepoint: 1 });
        assert_eq!(err.to_string(), "Freitext: character U+0001 at position 1 is not permitted in XML text");
        assert!(spec.parse("\u{FFFE}").is_err());
    }
}
