//! The bounded tree parser and the schema-order cursor.

use core::fmt;
use core::str::FromStr;

use jiff::civil::Date;
use jiff::fmt::temporal::{Pieces, PiecesOffset};
use jiff::tz::Offset;
use quick_xml::XmlVersion;
use quick_xml::events::Event;

use super::{Limits, NS_SOAP, NS_XSI, XmlError, XmlErrorKind};
use crate::ValueError;
use crate::envelope::Zeitstempel;

/// One element of the parsed tree.
#[derive(Debug, Default)]
pub struct Node {
    /// The element's namespace, if any.
    pub ns: Option<String>,
    /// The local name.
    pub name: String,
    /// `xsi:type`, resolved: namespace and local name.
    pub xsi_type: Option<(Option<String>, String)>,
    /// The text content (entities and character references resolved).
    pub text: String,
    /// Whether the element holds non-whitespace text.
    pub has_text: bool,
    /// The child elements, in document order.
    pub children: Vec<Node>,
    text_chars: usize,
}

struct Open {
    node: Node,
    scope_len: usize,
    raw_xsi_type: Option<String>,
    text_limit: usize,
    binary: bool,
}

fn malformed(e: impl fmt::Display, offset: u64) -> XmlError {
    XmlError { path: String::new(), offset: Some(offset), kind: XmlErrorKind::Malformed(e.to_string()) }
}

/// Parses `input` into a tree under `limits`. Elements whose local name is in `binary` carry
/// an embedded base64 document and may hold `2 × limits.max_attachment` characters of text.
///
/// # Errors
///
/// [`XmlError`] for oversized, malformed or hostile input.
pub fn parse(input: &[u8], limits: &Limits, binary: &[&str]) -> Result<Node, XmlError> {
    if input.len() > limits.max_bytes {
        return Err(XmlError::at("", XmlErrorKind::TooLarge { limit: limits.max_bytes, actual: input.len() }));
    }
    let text = core::str::from_utf8(input).map_err(|e| XmlError {
        path: String::new(),
        offset: Some(e.valid_up_to() as u64),
        kind: XmlErrorKind::Malformed("not UTF-8".into()),
    })?;
    parse_tree(text, limits, binary)
}

fn resolve<'s>(scope: &'s [(String, String)], prefix: &str) -> Option<&'s str> {
    if prefix == "xml" {
        return Some("http://www.w3.org/XML/1998/namespace");
    }
    scope.iter().rev().find(|(p, _)| p == prefix).map(|(_, uri)| uri.as_str()).filter(|uri| !uri.is_empty())
}

fn split_qname(q: &str) -> (&str, &str) {
    q.split_once(':').unwrap_or(("", q))
}

#[allow(clippy::too_many_lines)]
fn parse_tree(text: &str, limits: &Limits, binary: &[&str]) -> Result<Node, XmlError> {
    let mut reader = quick_xml::Reader::from_str(text);
    let config = reader.config_mut();
    config.check_end_names = true;
    config.expand_empty_elements = false;
    config.trim_text_start = false;
    config.trim_text_end = false;

    let mut scope: Vec<(String, String)> = Vec::new();
    let mut stack: Vec<Open> = Vec::new();
    let mut root: Option<Node> = None;
    let mut elements = 0usize;

    loop {
        let offset = reader.buffer_position();
        let event = reader.read_event().map_err(|e| malformed(e, reader.error_position()))?;
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(e) | Event::Empty(e) => {
                if root.is_some() {
                    return Err(malformed("content after the root element", offset));
                }
                elements += 1;
                if elements > limits.max_elements {
                    return Err(XmlError {
                        path: String::new(),
                        offset: Some(offset),
                        kind: XmlErrorKind::TooManyElements { limit: limits.max_elements },
                    });
                }
                if stack.len() >= limits.max_depth {
                    return Err(XmlError {
                        path: path_of(&stack),
                        offset: Some(offset),
                        kind: XmlErrorKind::TooDeep { limit: limits.max_depth },
                    });
                }
                let qname: String = e.name().as_ref().to_owned();
                let scope_len = scope.len();
                let mut raw_xsi_type = None;
                let mut others: Vec<(String, String)> = Vec::new();
                for attr in e.attributes() {
                    let attr = attr.map_err(|err| malformed(err, offset))?;
                    let key: String = attr.key.as_ref().to_owned();
                    let value = attr
                        .normalized_value(XmlVersion::Implicit1_0)
                        .map_err(|_| XmlError {
                            path: path_of(&stack),
                            offset: Some(offset),
                            kind: XmlErrorKind::Entity,
                        })?
                        .into_owned();
                    if key == "xmlns" {
                        scope.push((String::new(), value));
                    } else if let Some(prefix) = key.strip_prefix("xmlns:") {
                        scope.push((prefix.to_owned(), value));
                    } else {
                        others.push((key, value));
                    }
                }
                for (key, value) in others {
                    let (prefix, local) = split_qname(&key);
                    if local == "type" && !prefix.is_empty() && resolve(&scope, prefix) == Some(NS_XSI) {
                        raw_xsi_type = Some(value);
                    }
                }
                let (prefix, local) = split_qname(&qname);
                let ns = if prefix.is_empty() {
                    resolve(&scope, "").map(str::to_owned)
                } else {
                    Some(
                        resolve(&scope, prefix)
                            .ok_or_else(|| malformed(format!("unbound prefix {prefix}"), offset))?
                            .to_owned(),
                    )
                };
                let is_binary = binary.contains(&local);
                let text_limit = if is_binary {
                    limits.max_attachment.saturating_mul(2).max(limits.max_text)
                } else {
                    limits.max_text
                };
                let node = Node { ns, name: local.to_owned(), ..Node::default() };
                let mut open = Open { node, scope_len, raw_xsi_type, text_limit, binary: is_binary };
                resolve_xsi(&mut open, &scope, &stack)?;
                if empty {
                    scope.truncate(open.scope_len);
                    attach(&mut stack, &mut root, open.node);
                } else {
                    stack.push(open);
                }
            }
            Event::End(_) => {
                let Some(open) = stack.pop() else {
                    return Err(malformed("unmatched end tag", offset));
                };
                scope.truncate(open.scope_len);
                let node = open.node;
                if !node.children.is_empty() && node.has_text {
                    let mut path = path_of(&stack);
                    push_path(&mut path, &node.name);
                    return Err(XmlError { path, offset: Some(offset), kind: XmlErrorKind::MixedContent });
                }
                attach(&mut stack, &mut root, node);
            }
            Event::Text(t) => append_text(&mut stack, &t.xml10_content(), limits, offset)?,
            Event::CData(t) => append_text(&mut stack, &t.xml10_content(), limits, offset)?,
            Event::GeneralRef(r) => {
                let c = if r.is_char_ref() {
                    r.resolve_char_ref().ok().flatten()
                } else {
                    match r.as_ref() {
                        "lt" => Some('<'),
                        "gt" => Some('>'),
                        "amp" => Some('&'),
                        "apos" => Some('\''),
                        "quot" => Some('"'),
                        _ => None,
                    }
                };
                let Some(c) = c else {
                    return Err(XmlError { path: path_of(&stack), offset: Some(offset), kind: XmlErrorKind::Entity });
                };
                let mut buf = [0u8; 4];
                append_text(&mut stack, c.encode_utf8(&mut buf), limits, offset)?;
            }
            Event::DocType(_) => {
                return Err(XmlError { path: String::new(), offset: Some(offset), kind: XmlErrorKind::Doctype });
            }
            Event::Decl(d) => {
                if let Some(enc) = d.encoding() {
                    let enc = enc.map_err(|err| malformed(err, offset))?;
                    if !enc.eq_ignore_ascii_case("utf-8") {
                        return Err(malformed("encoding must be UTF-8", offset));
                    }
                }
            }
            Event::Comment(_) | Event::PI(_) => {}
            Event::Eof => break,
        }
    }
    if !stack.is_empty() {
        return Err(malformed("unexpected end of input", text.len() as u64));
    }
    root.ok_or_else(|| malformed("no root element", 0))
}

fn resolve_xsi(open: &mut Open, scope: &[(String, String)], stack: &[Open]) -> Result<(), XmlError> {
    if let Some(raw) = open.raw_xsi_type.take() {
        let (prefix, local) = split_qname(raw.trim());
        let ns = resolve(scope, prefix).map(str::to_owned);
        if !prefix.is_empty() && ns.is_none() {
            let mut path = path_of(stack);
            push_path(&mut path, &open.node.name);
            return Err(XmlError::at(&path, XmlErrorKind::XsiType(format!("prefix {prefix} is not bound"))));
        }
        open.node.xsi_type = Some((ns, local.to_owned()));
    }
    Ok(())
}

fn attach(stack: &mut [Open], root: &mut Option<Node>, node: Node) {
    match stack.last_mut() {
        Some(parent) => parent.node.children.push(node),
        None => *root = Some(node),
    }
}

fn append_text(stack: &mut [Open], text: &str, limits: &Limits, offset: u64) -> Result<(), XmlError> {
    let Some(open) = stack.last_mut() else {
        return if text.trim().is_empty() { Ok(()) } else { Err(malformed("text outside the root element", offset)) };
    };
    if !text.trim().is_empty() {
        open.node.has_text = true;
    }
    // Indentation between child elements is not content: it neither counts towards the text
    // limit nor is kept, so a long list does not fail as „text too long".
    if !open.node.children.is_empty() && text.bytes().all(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r')) {
        return Ok(());
    }
    let chars = open.node.text_chars + text.chars().count();
    if chars > open.text_limit {
        let kind = if open.binary {
            XmlErrorKind::AttachmentTooLarge { limit: limits.max_attachment }
        } else {
            XmlErrorKind::TextTooLong { limit: limits.max_text }
        };
        return Err(XmlError { path: path_of(stack), offset: Some(offset), kind });
    }
    open.node.text_chars = chars;
    open.node.text.push_str(text);
    Ok(())
}

fn path_of(stack: &[Open]) -> String {
    stack.iter().map(|o| o.node.name.as_str()).collect::<Vec<_>>().join("/")
}

fn push_path(path: &mut String, name: &str) {
    if !path.is_empty() {
        path.push('/');
    }
    path.push_str(name);
}

/// The payload inside a SOAP 1.1 `Envelope`/`Body`, or `root` itself if it is not an envelope.
///
/// # Errors
///
/// An envelope without a body, or a body without exactly one element.
pub fn unwrap_soap(root: &Node) -> Result<&Node, XmlError> {
    if root.ns.as_deref() != Some(NS_SOAP) {
        return Ok(root);
    }
    if root.name != "Envelope" {
        return Err(XmlError::at(&root.name, XmlErrorKind::Unexpected(root.name.clone())));
    }
    let body = root
        .children
        .iter()
        .find(|c| c.ns.as_deref() == Some(NS_SOAP) && c.name == "Body")
        .ok_or_else(|| XmlError::at("Envelope", XmlErrorKind::Missing("Body")))?;
    match body.children.as_slice() {
        [payload] => Ok(payload),
        [] => Err(XmlError::at("Envelope/Body", XmlErrorKind::Missing("payload"))),
        [_, extra, ..] => Err(XmlError::at("Envelope/Body", XmlErrorKind::Unexpected(extra.name.clone()))),
    }
}

/// Walks the children of one element in schema order.
#[derive(Debug)]
pub struct Seq<'a> {
    node: &'a Node,
    i: usize,
    /// The element's path, for errors.
    pub path: String,
}

impl<'a> Seq<'a> {
    /// A cursor over `node`'s children.
    ///
    /// # Errors
    ///
    /// The element holds text besides elements.
    pub fn new(node: &'a Node, path: String) -> Result<Self, XmlError> {
        if node.has_text {
            return Err(XmlError::at(&path, XmlErrorKind::MixedContent));
        }
        Ok(Self { node, i: 0, path })
    }

    /// The element walked.
    #[must_use]
    pub const fn node(&self) -> &'a Node {
        self.node
    }

    fn child_path(&self, name: &str) -> String {
        format!("{}/{name}", self.path)
    }

    /// An error at this element.
    #[must_use]
    pub fn error(&self, kind: XmlErrorKind) -> XmlError {
        XmlError::at(&self.path, kind)
    }

    /// Whether the next child is the unqualified element `name`.
    #[must_use]
    pub fn peek(&self, name: &str) -> bool {
        self.node.children.get(self.i).is_some_and(|c| c.name == name && c.ns.is_none())
    }

    /// The next child if it is `name`.
    pub fn opt(&mut self, name: &'static str) -> Option<(&'a Node, String)> {
        if self.peek(name) {
            let node = self.node.children.get(self.i)?;
            self.i += 1;
            Some((node, self.child_path(name)))
        } else {
            None
        }
    }

    /// The next child, which must be `name`.
    ///
    /// # Errors
    ///
    /// [`XmlErrorKind::Missing`].
    pub fn req(&mut self, name: &'static str) -> Result<(&'a Node, String), XmlError> {
        self.opt(name).ok_or_else(|| XmlError::at(&self.path, XmlErrorKind::Missing(name)))
    }

    /// `min..=max` consecutive children `name`.
    ///
    /// # Errors
    ///
    /// Fewer than `min` ([`XmlErrorKind::Missing`]) or more than `max` ([`XmlErrorKind::TooMany`]).
    pub fn many(&mut self, name: &'static str, min: usize, max: usize) -> Result<Vec<(&'a Node, String)>, XmlError> {
        let mut out = Vec::new();
        while let Some(item) = self.opt(name) {
            if out.len() == max {
                return Err(XmlError::at(&self.path, XmlErrorKind::TooMany { element: name, max }));
            }
            out.push(item);
        }
        if out.len() < min {
            return Err(XmlError::at(&self.path, XmlErrorKind::Missing(name)));
        }
        Ok(out)
    }

    /// A cursor over the mandatory child `name`.
    ///
    /// # Errors
    ///
    /// Missing, or holding text.
    pub fn seq(&mut self, name: &'static str) -> Result<Seq<'a>, XmlError> {
        let (node, path) = self.req(name)?;
        Seq::new(node, path)
    }

    /// A cursor over the optional child `name`.
    ///
    /// # Errors
    ///
    /// Holding text.
    pub fn opt_seq(&mut self, name: &'static str) -> Result<Option<Seq<'a>>, XmlError> {
        self.opt(name).map(|(node, path)| Seq::new(node, path)).transpose()
    }

    /// The mandatory text child `name`, parsed.
    ///
    /// # Errors
    ///
    /// Missing, not a leaf, or not a valid value.
    pub fn val<T: FromStr<Err = ValueError>>(&mut self, name: &'static str) -> Result<T, XmlError> {
        let (node, path) = self.req(name)?;
        leaf(node, &path)?.parse().map_err(|e| XmlError::at(&path, XmlErrorKind::Value(e)))
    }

    /// The optional text child `name`, parsed.
    ///
    /// # Errors
    ///
    /// Not a leaf, or not a valid value.
    pub fn opt_val<T: FromStr<Err = ValueError>>(&mut self, name: &'static str) -> Result<Option<T>, XmlError> {
        match self.opt(name) {
            Some((node, path)) => {
                leaf(node, &path)?.parse().map(Some).map_err(|e| XmlError::at(&path, XmlErrorKind::Value(e)))
            }
            None => Ok(None),
        }
    }

    /// The raw text of the mandatory child `name`, and its path.
    ///
    /// # Errors
    ///
    /// Missing or not a leaf.
    pub fn text(&mut self, name: &'static str) -> Result<(&'a str, String), XmlError> {
        let (node, path) = self.req(name)?;
        Ok((leaf(node, &path)?, path))
    }

    /// The `xsd:date` child `name`.
    ///
    /// # Errors
    ///
    /// Missing or not a date.
    pub fn date(&mut self, name: &'static str) -> Result<Date, XmlError> {
        let (text, path) = self.text(name)?;
        parse_date(text, &path)
    }

    /// The optional `xsd:date` child `name`.
    ///
    /// # Errors
    ///
    /// Not a date.
    pub fn opt_date(&mut self, name: &'static str) -> Result<Option<Date>, XmlError> {
        if self.peek(name) { self.date(name).map(Some) } else { Ok(None) }
    }

    /// The `xsd:boolean` child `name`.
    ///
    /// # Errors
    ///
    /// Missing or not a boolean.
    pub fn boolean(&mut self, name: &'static str) -> Result<bool, XmlError> {
        let (text, path) = self.text(name)?;
        match text.trim() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            _ => Err(XmlError::at(&path, XmlErrorKind::Invalid("boolean"))),
        }
    }

    /// The optional `xsd:boolean` child `name`.
    ///
    /// # Errors
    ///
    /// Not a boolean.
    pub fn opt_boolean(&mut self, name: &'static str) -> Result<Option<bool>, XmlError> {
        if self.peek(name) { self.boolean(name).map(Some) } else { Ok(None) }
    }

    /// The enumeration child `name`, parsed by `parse`; `what` names it in the error.
    ///
    /// # Errors
    ///
    /// Missing or not a value of the enumeration.
    pub fn code<T>(
        &mut self,
        name: &'static str,
        what: &'static str,
        parse: fn(&str) -> Option<T>,
    ) -> Result<T, XmlError> {
        let (text, path) = self.text(name)?;
        parse(text.trim()).ok_or_else(|| XmlError::at(&path, XmlErrorKind::Invalid(what)))
    }

    /// The optional enumeration child `name`.
    ///
    /// # Errors
    ///
    /// Not a value of the enumeration.
    pub fn opt_code<T>(
        &mut self,
        name: &'static str,
        what: &'static str,
        parse: fn(&str) -> Option<T>,
    ) -> Result<Option<T>, XmlError> {
        if self.peek(name) { self.code(name, what, parse).map(Some) } else { Ok(None) }
    }

    /// Ends the walk: no child may remain.
    ///
    /// # Errors
    ///
    /// [`XmlErrorKind::Unexpected`] naming the first unread child.
    pub fn done(self) -> Result<(), XmlError> {
        match self.node.children.get(self.i) {
            None => Ok(()),
            Some(extra) => Err(XmlError::at(&self.path, XmlErrorKind::Unexpected(extra.name.clone()))),
        }
    }
}

/// The text of a leaf element.
///
/// # Errors
///
/// The element has children.
pub fn leaf<'a>(node: &'a Node, path: &str) -> Result<&'a str, XmlError> {
    if node.children.is_empty() { Ok(&node.text) } else { Err(XmlError::at(path, XmlErrorKind::MixedContent)) }
}

/// `xsd:date`; an optional time-zone suffix is accepted and dropped.
///
/// # Errors
///
/// [`XmlErrorKind::Invalid`].
pub fn parse_date(text: &str, path: &str) -> Result<Date, XmlError> {
    let text = text.trim();
    let core = text.get(..10).unwrap_or(text);
    let rest = text.get(10..).unwrap_or("");
    let tz_ok = rest.is_empty()
        || rest == "Z"
        || (rest.len() == 6 && (rest.starts_with('+') || rest.starts_with('-')) && rest.get(3..4) == Some(":"));
    if core.len() != 10 || !tz_ok {
        return Err(XmlError::at(path, XmlErrorKind::Invalid("date (YYYY-MM-DD)")));
    }
    core.parse().map_err(|_| XmlError::at(path, XmlErrorKind::Invalid("date (YYYY-MM-DD)")))
}

/// `xsd:dateTime` with a time zone, keeping the offset.
///
/// # Errors
///
/// [`XmlErrorKind::Invalid`], also for a timestamp without a time zone.
pub fn parse_zeitstempel(text: &str, path: &str) -> Result<Zeitstempel, XmlError> {
    let invalid = || XmlError::at(path, XmlErrorKind::Invalid("zeitstempel (xsd:dateTime with time zone)"));
    let text = text.trim();
    let pieces = Pieces::parse(text).map_err(|_| invalid())?;
    let offset = match pieces.offset() {
        Some(PiecesOffset::Zulu) => Offset::UTC,
        Some(PiecesOffset::Numeric(n)) => n.offset(),
        _ => return Err(invalid()),
    };
    let timestamp = text.parse().map_err(|_| invalid())?;
    Ok(Zeitstempel { timestamp, offset })
}

impl Node {
    /// The local name of `xsi:type` if it is in namespace `ns`.
    ///
    /// # Errors
    ///
    /// [`XmlErrorKind::XsiType`]: missing or in another namespace.
    pub fn xsi(&self, path: &str, ns: &str) -> Result<&str, XmlError> {
        match &self.xsi_type {
            Some((Some(found), local)) if found == ns => Ok(local),
            Some((_, local)) => {
                Err(XmlError::at(path, XmlErrorKind::XsiType(format!("{local} is not in namespace {ns}"))))
            }
            None => Err(XmlError::at(path, XmlErrorKind::XsiType("missing".into()))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_counted_once_per_chunk() {
        // Many character references in one element: counting stays linear and bounded.
        let body = "&#65;".repeat(5000);
        let xml = format!("<a>{body}</a>");
        let err = parse(xml.as_bytes(), &Limits::default(), &[]).unwrap_err();
        assert_eq!(err.kind, XmlErrorKind::TextTooLong { limit: 4096 });
        let ok = parse(xml.as_bytes(), &Limits::default(), &["a"]).unwrap();
        assert_eq!(ok.text.len(), 5000);
    }

    #[test]
    fn binary_elements_are_bounded_by_the_attachment_limit() {
        let limits = Limits { max_attachment: 3, ..Limits::default() };
        let xml = format!("<a><inhalt>{}</inhalt></a>", "A".repeat(4097));
        let err = parse(xml.as_bytes(), &limits, &["inhalt"]).unwrap_err();
        assert_eq!(err.kind, XmlErrorKind::AttachmentTooLarge { limit: 3 });
        assert_eq!(err.path, "a/inhalt");
    }

    #[test]
    fn attachment_limits_leave_room_for_base64() {
        let l = Limits::for_attachments(3 << 20);
        assert_eq!(l.max_bytes, (1 << 20) + (4 << 20));
        assert_eq!(l.max_attachment, 3 << 20);
        assert_eq!(l.max_text, 4096);
    }
}
