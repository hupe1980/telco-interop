//! The indenting writer.

use core::fmt::Write as _;

use jiff::civil::Date;

/// A value the schema would reject, found while writing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{path}: {kind}")]
pub struct WriteError {
    /// Element path, e.g. `annehmenMeldungRequest/Meldung/Position`.
    pub path: String,
    /// What is wrong.
    pub kind: WriteErrorKind,
}

/// What a [`WriteError`] reports.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum WriteErrorKind {
    /// An element repeats more often than the schema allows.
    #[error("{element} occurs {actual} times, at most {max} allowed")]
    TooMany {
        /// The element.
        element: &'static str,
        /// Schema maximum.
        max: usize,
        /// Count found.
        actual: usize,
    },
    /// A mandatory repeated element is empty.
    #[error("{element} must occur at least once")]
    Missing {
        /// The element.
        element: &'static str,
    },
}

/// Escapes text for element content and attribute values. Carriage returns are written as
/// character references so they survive the parser's line-end normalisation.
pub fn escape(out: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\r' => out.push_str("&#13;"),
            c => out.push(c),
        }
    }
}

/// Writes elements, two spaces per level, one element per line.
#[derive(Debug, Default)]
pub struct Writer {
    /// The output so far.
    pub out: String,
    /// The current indentation level.
    pub depth: usize,
    path: Vec<&'static str>,
}

impl Writer {
    /// An empty writer.
    #[must_use]
    pub const fn new() -> Self {
        Self { out: String::new(), depth: 0, path: Vec::new() }
    }

    fn indent(&mut self) {
        for _ in 0..self.depth {
            self.out.push_str("  ");
        }
    }

    /// Opens `name` with attributes.
    pub fn open(&mut self, name: &'static str, attrs: &[(&str, &str)]) {
        self.indent();
        self.out.push('<');
        self.out.push_str(name);
        for (k, v) in attrs {
            self.out.push(' ');
            self.out.push_str(k);
            self.out.push_str("=\"");
            escape(&mut self.out, v);
            self.out.push('"');
        }
        self.out.push_str(">\n");
        self.depth += 1;
        self.path.push(name);
    }

    /// Closes `name`.
    pub fn close(&mut self, name: &'static str) {
        self.depth = self.depth.saturating_sub(1);
        self.path.pop();
        self.indent();
        self.out.push_str("</");
        self.out.push_str(name);
        self.out.push_str(">\n");
    }

    /// Opens `name` with `xsi:type="prefix:ty"`.
    pub fn typed(&mut self, name: &'static str, prefix: &str, ty: &str) {
        let value = format!("{prefix}:{ty}");
        self.open(name, &[("xsi:type", &value)]);
    }

    /// A text element.
    pub fn leaf(&mut self, name: &'static str, text: &str) {
        self.indent();
        let _ = write!(self.out, "<{name}>");
        escape(&mut self.out, text);
        let _ = writeln!(self.out, "</{name}>");
    }

    /// A text element if `text` is present.
    pub fn opt(&mut self, name: &'static str, text: Option<&str>) {
        if let Some(text) = text {
            self.leaf(name, text);
        }
    }

    /// An `xsd:date` element.
    pub fn date(&mut self, name: &'static str, d: Date) {
        self.leaf(name, &d.to_string());
    }

    /// An optional `xsd:date` element.
    pub fn opt_date(&mut self, name: &'static str, d: Option<Date>) {
        if let Some(d) = d {
            self.date(name, d);
        }
    }

    /// An `xsd:boolean` element.
    pub fn boolean(&mut self, name: &'static str, b: bool) {
        self.leaf(name, if b { "true" } else { "false" });
    }

    /// The path of the open elements, joined by `/`.
    #[must_use]
    pub fn path(&self) -> String {
        self.path.join("/")
    }

    /// Checks that `element` occurs `min..=max` times.
    ///
    /// # Errors
    ///
    /// [`WriteErrorKind::Missing`] or [`WriteErrorKind::TooMany`] at the current path.
    pub fn count(&self, element: &'static str, actual: usize, min: usize, max: usize) -> Result<(), WriteError> {
        if actual < min {
            return Err(WriteError { path: self.path(), kind: WriteErrorKind::Missing { element } });
        }
        if actual > max {
            return Err(WriteError { path: self.path(), kind: WriteErrorKind::TooMany { element, max, actual } });
        }
        Ok(())
    }
}
