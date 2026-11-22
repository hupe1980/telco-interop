//! What a payload is, its validation issues and what a code means, as plain text lines — the
//! output of `telco-interop read`, `check` and `code`, for any front end.
//!
//! The command line and the site's playground both print these lines, so they cannot diverge.
//! The functions take bytes and return lines; they read no file, clock or environment (a check
//! judges the date rules at the payload's own control timestamp, the moment it was sent).
//!
//! The lines are **value-free**: interface, kinds, business cases, ids, dates and codes, never a
//! name, address, phone number or free text. Every character taken from the input passes
//! [`sanitize`], so control characters (C0, DEL, C1) arrive as U+FFFD.
//!
//! ```
//! use telco_interop::report;
//!
//! let out = report::code("8103", true, true);
//! assert!(out.ok);
//! assert_eq!(out.lines[0], "WBCI 8103 Rufnummer nicht geschaltet (RNG)");
//!
//! let out = report::read(b"<!DOCTYPE x><x/>");
//! assert!(!out.ok);
//! assert!(out.lines[0].starts_with("unreadable: ") && out.lines[0].contains("TEQ 0995"));
//! ```

use std::fmt;

use crate::{spri, wbci};

/// The envelope namespaces: the root of every payload is in one of them.
const NS_WBCI: &[u8] = b"http://www.vatm.de/wbci/v1/envelope";
const NS_SPRI: &[u8] = b"http://spri.telekom.de/oss/v4/envelope";

/// Replaces control characters (C0, DEL, C1) with U+FFFD, so no input can move a terminal's
/// cursor, clear a screen or hide a line.
#[must_use]
pub fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_control() { '\u{FFFD}' } else { c }).collect()
}

/// The lines of one command and whether it passed (the command line's exit code 0).
///
/// `read` and `check` give the lines for one payload (the command line prints them indented
/// under the file name); `code` gives the lines as printed. Every line is already
/// [sanitized](sanitize).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Output {
    /// The lines, without line breaks.
    pub lines: Vec<String>,
    /// `false`: the payload is unreadable, an issue is an error, or the code is unknown.
    pub ok: bool,
}

impl Output {
    fn push(&mut self, line: impl fmt::Display) {
        self.lines.push(sanitize(&line.to_string()));
    }
}

impl fmt::Display for Output {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, line) in self.lines.iter().enumerate() {
            if i > 0 {
                f.write_str("\n")?;
            }
            f.write_str(line)?;
        }
        Ok(())
    }
}

/// A payload of either interface, its interface detected from the input.
#[derive(Debug, Clone, PartialEq)]
pub enum Payload {
    /// A WBCI payload.
    Wbci(Box<wbci::Payload>),
    /// An S/PRI payload.
    Spri(Box<spri::Payload>),
}

impl Payload {
    /// Reads a WBCI or S/PRI payload, bare or in a SOAP 1.1 envelope: the WBCI reader first,
    /// then the S/PRI reader, each with its default limits.
    ///
    /// # Errors
    ///
    /// Neither reader accepts the input; the error is the one of the interface whose envelope
    /// namespace the input names (WBCI's when it names neither).
    pub fn parse(bytes: &[u8]) -> Result<Self, Unreadable> {
        let wbci_err = match wbci::Payload::from_xml(bytes) {
            Ok(p) => return Ok(Self::Wbci(Box::new(p))),
            Err(e) => e,
        };
        let spri_err = match spri::Payload::from_xml(bytes) {
            Ok(p) => return Ok(Self::Spri(Box::new(p))),
            Err(e) => e,
        };
        let names = |ns: &[u8]| bytes.windows(ns.len()).any(|w| w == ns);
        Err(if names(NS_WBCI) {
            Unreadable { interface: Some("WBCI"), reason: wbci_err.to_string(), teq: wbci_err.teq_code().to_string() }
        } else if names(NS_SPRI) {
            Unreadable { interface: Some("S/PRI"), reason: spri_err.to_string(), teq: spri_err.teq_code().to_string() }
        } else {
            Unreadable { interface: None, reason: wbci_err.to_string(), teq: wbci_err.teq_code().to_string() }
        })
    }

    /// The control timestamp: when the payload was sent.
    #[must_use]
    pub fn sent_at(&self) -> wbci::Timestamp {
        match self {
            Self::Wbci(p) => p.control().zeitstempel.timestamp,
            Self::Spri(p) => p.control().zeitstempel.timestamp,
        }
    }
}

/// Why a payload cannot be read, and the receipt (TEQ) code to answer it with.
///
/// The reason is the reader's error message, which names elements and limits, never a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unreadable {
    interface: Option<&'static str>,
    reason: String,
    teq: String,
}

impl Unreadable {
    /// `"WBCI"` or `"S/PRI"`: the interface whose envelope namespace the input names; `None`
    /// when it names neither.
    #[must_use]
    pub const fn interface(&self) -> Option<&'static str> {
        self.interface
    }

    /// The TEQ code to answer with and its text, e.g. `0995 Datenstruktur ungültig`.
    #[must_use]
    pub fn teq(&self) -> &str {
        &self.teq
    }
}

impl fmt::Display for Unreadable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.interface {
            Some(i) => write!(f, "{i}: {} — answer with TEQ {}", self.reason, self.teq),
            None => write!(f, "neither a WBCI nor an S/PRI payload: {} — answer with TEQ {}", self.reason, self.teq),
        }
    }
}

impl std::error::Error for Unreadable {}

/// What the payload is — interface, kind, business case, ids, dates, codes — or why it cannot
/// be read and the TEQ code to answer with (`telco-interop read`).
#[must_use]
pub fn read(bytes: &[u8]) -> Output {
    let mut out = Output { lines: Vec::new(), ok: true };
    match Payload::parse(bytes) {
        Ok(p) => {
            for (label, value) in summary(&p) {
                out.push(format_args!("{label}: {value}"));
            }
        }
        Err(e) => {
            out.ok = false;
            out.push(format_args!("unreadable: {e}"));
        }
    }
    out
}

/// The validation issues of the payload (request, order, message or receipt), `<interface>: no
/// issues`, or why it cannot be read (`telco-interop check`). Fails on an error-level issue.
#[must_use]
pub fn check(bytes: &[u8]) -> Output {
    let mut out = Output { lines: Vec::new(), ok: true };
    let payload = match Payload::parse(bytes) {
        Ok(p) => p,
        Err(e) => {
            out.ok = false;
            out.push(format_args!("unreadable: {e}"));
            return out;
        }
    };
    // Date rules are judged at the payload's own control timestamp: the moment it was sent.
    let sent_at = payload.sent_at();
    let (interface, issues): (&str, Vec<(bool, String)>) = match &payload {
        Payload::Wbci(p) => (
            "WBCI",
            match &**p {
                wbci::Payload::AnfrageRequest { anfrage, .. } => {
                    anfrage.issues_at(sent_at, &wbci::Calendar::wbci()).iter().map(wbci_line).collect()
                }
                wbci::Payload::MeldungRequest { meldung, .. } => meldung.issues(None).iter().map(wbci_line).collect(),
                wbci::Payload::AnfrageResponse { quittung, .. } | wbci::Payload::MeldungResponse { quittung, .. } => {
                    quittung.issues().iter().map(wbci_line).collect()
                }
            },
        ),
        Payload::Spri(p) => (
            "S/PRI",
            match &**p {
                spri::Payload::AuftragRequest { auftrag, .. } => {
                    auftrag.issues_at(sent_at, &spri::Calendar::werktage()).iter().map(spri_line).collect()
                }
                spri::Payload::MeldungRequest { meldung, .. } => meldung.issues().iter().map(spri_line).collect(),
                spri::Payload::AuftragResponse { quittung, .. } | spri::Payload::MeldungResponse { quittung, .. } => {
                    quittung.issues().iter().map(spri_line).collect()
                }
            },
        ),
    };
    if issues.is_empty() {
        out.push(format_args!("{interface}: no issues"));
    }
    for (error, line) in issues {
        if error {
            out.ok = false;
        }
        out.push(line);
    }
    out
}

/// Whether the issue is an error, and its line.
fn wbci_line(i: &wbci::Issue) -> (bool, String) {
    (i.severity == wbci::Severity::Error, i.to_string())
}

/// Whether the issue is an error, and its line.
fn spri_line(i: &spri::Issue) -> (bool, String) {
    (i.severity == spri::Severity::Error, i.to_string())
}

/// The meaning of a Meldungscode in WBCI (`in_wbci`) and S/PRI (`in_spri`): its text, and the
/// message types and business cases it applies to (`telco-interop code`). Fails when no chosen
/// interface knows the code.
#[must_use]
pub fn code(code: &str, in_wbci: bool, in_spri: bool) -> Output {
    let mut out = Output { lines: Vec::new(), ok: false };
    if in_wbci {
        let c = wbci::Meldungscode::parse(code);
        if c.text().is_some() {
            out.ok = true;
            out.push(format_args!("WBCI {c}"));
            if let Some(short) = c.short() {
                out.push(format_args!("  short name: {short}"));
            }
            let mut typen: Vec<wbci::Meldungstyp> = Vec::new();
            for t in c.meldungstypen() {
                if !typen.contains(&t) {
                    typen.push(t);
                }
            }
            for t in typen {
                let cases: Vec<&str> =
                    wbci::Geschaeftsfall::ALL.iter().filter(|g| c.applies(t, **g)).map(|g| g.as_str()).collect();
                out.push(format_args!("  {t} in {}", cases.join(", ")));
            }
        } else {
            out.push(format_args!("WBCI: {code} is not a WBCI code"));
        }
    }
    if in_spri {
        let c = spri::Meldungscode::parse(code);
        if c.text().is_some() {
            out.ok = true;
            out.push(format_args!("S/PRI {c}"));
            let typen: Vec<&str> = c.meldungstypen().map(spri::Meldungstyp::as_str).collect();
            if !typen.is_empty() {
                out.push(format_args!("  message types: {}", typen.join(", ")));
            }
            let cases: Vec<&str> = c.geschaeftsfaelle().map(spri::GeschaeftsfallCode::as_str).collect();
            if !cases.is_empty() {
                out.push(format_args!("  business cases: {}", cases.join(", ")));
            }
        } else {
            out.push(format_args!("S/PRI: {code} is not an S/PRI code"));
        }
    }
    out
}

// -- summaries ------------------------------------------------------------------------------

/// The lines of a summary: label and value.
type Lines = Vec<(&'static str, String)>;

fn summary(p: &Payload) -> Lines {
    match p {
        Payload::Wbci(p) => wbci_summary(p),
        Payload::Spri(p) => spri_summary(p),
    }
}

fn wbci_summary(p: &wbci::Payload) -> Lines {
    let mut l: Lines = vec![("interface", "WBCI".to_owned())];
    match p {
        wbci::Payload::AnfrageRequest { anfrage, .. } => {
            l.push(("payload", "request (annehmenAnfrageRequest)".to_owned()));
            l.push(("kind", anfrage.geschaeftsfall().to_string()));
            l.push(("sent", p.control().zeitstempel.to_string()));
            l.push(("sender", format!("{} ({})", anfrage.absender(), anfrage.sender())));
            ekp(&mut l, anfrage.ekp());
            l.push(("Vorabstimmungs-ID", anfrage.vorabstimmungs_id().to_string()));
            match anfrage {
                wbci::Anfrage::Vorabstimmung(va) => {
                    l.push(("Kundenwunschtermin", va.kundenwunschtermin.to_string()));
                    if !va.weitere_anschlussinhaber.is_empty() {
                        l.push(("further line holders", va.weitere_anschlussinhaber.len().to_string()));
                    }
                }
                wbci::Anfrage::Storno(s) => l.push(("Storno-ID", s.storno_id.to_string())),
                wbci::Anfrage::Terminverschiebung(t) => {
                    l.push(("Änderungs-ID", t.aenderungs_id.to_string()));
                    l.push(("neuer Kundenwunschtermin", t.neuer_kundenwunschtermin.to_string()));
                }
            }
        }
        wbci::Payload::MeldungRequest { meldung, .. } => {
            let kopf = meldung.kopf();
            l.push(("payload", "message (annehmenMeldungRequest)".to_owned()));
            l.push(("kind", meldung.meldungstyp().to_string()));
            l.push(("business case", kopf.geschaeftsfall.to_string()));
            l.push(("sent", p.control().zeitstempel.to_string()));
            l.push(("sender", sender(&kopf.absender, &kopf.ekp)));
            ekp(&mut l, &kopf.ekp);
            l.push(("Vorabstimmungs-ID", kopf.vorabstimmungs_id.to_string()));
            if let Some(d) = meldung.wechseltermin() {
                l.push(("Wechseltermin", d.to_string()));
            }
            if let Some(id) = meldung.storno_id() {
                l.push(("Storno-ID", id.to_string()));
            }
            if let Some(id) = meldung.aenderungs_id() {
                l.push(("Änderungs-ID", id.to_string()));
            }
            match meldung {
                wbci::Meldung::AkmTr(t) => {
                    l.push(("Ressourcenübernahme", yes_no(t.ressourcenuebernahme)));
                    l.push(("sicherer Hafen", yes_no(t.sicherer_hafen)));
                    for u in &t.uebernahme_leitung {
                        if let wbci::UebernahmeLeitung::LineId(id) = u {
                            l.push(("LineID taken over", id.to_string()));
                        }
                    }
                }
                wbci::Meldung::RuemVa(r) => {
                    for id in r.ressourcen.iter().filter_map(|r| r.line_id.as_ref()) {
                        l.push(("LineID", id.to_string()));
                    }
                }
                _ => {}
            }
            for c in meldung.codes() {
                l.push(("code", c.to_string()));
            }
        }
        wbci::Payload::AnfrageResponse { quittung, .. } | wbci::Payload::MeldungResponse { quittung, .. } => {
            let what = if matches!(p, wbci::Payload::AnfrageResponse { .. }) {
                "receipt for a request (annehmenAnfrageResponse)"
            } else {
                "receipt for a message (annehmenMeldungResponse)"
            };
            l.push(("payload", what.to_owned()));
            l.push(("kind", "TEQ".to_owned()));
            l.push(("sent", p.control().zeitstempel.to_string()));
            l.push(("sender", quittung.absender.to_string()));
            l.push(("Vorabstimmungs-ID", quittung.vorabstimmungs_id.to_string()));
            if let Some(id) = &quittung.storno_id {
                l.push(("Storno-ID", id.to_string()));
            }
            if let Some(id) = &quittung.aenderungs_id {
                l.push(("Änderungs-ID", id.to_string()));
            }
            for pos in &quittung.positionen {
                l.push(("code", pos.code.to_string()));
            }
        }
    }
    l
}

fn ekp(l: &mut Lines, ekp: &wbci::Ekp) {
    l.push(("EKPauf", ekp.auf.to_string()));
    l.push(("EKPabg", ekp.abg.to_string()));
}

fn sender(absender: &wbci::CarrierCode, ekp: &wbci::Ekp) -> String {
    match ekp.role_of(absender) {
        Some(role) => format!("{absender} ({role})"),
        None => format!("{absender} (neither EKP)"),
    }
}

fn yes_no(b: bool) -> String {
    (if b { "yes" } else { "no" }).to_owned()
}

fn spri_summary(p: &spri::Payload) -> Lines {
    let mut l: Lines = vec![("interface", "S/PRI".to_owned())];
    let sent = p.control().zeitstempel.to_string();
    match p {
        spri::Payload::AuftragRequest { auftrag, .. } => {
            l.push(("payload", "order (annehmenAuftragRequest)".to_owned()));
            l.push(("kind", auftrag.geschaeftsfall.code().to_string()));
            l.push(("Änderungskennzeichen", auftrag.aenderungskennzeichen.to_string()));
            l.push(("sent", sent));
            l.push(("externe Auftragsnummer", auftrag.externe_auftragsnummer.to_string()));
            if let Some(id) = auftrag.geschaeftsfall.vorabstimmungs_id() {
                l.push(("Vorabstimmungs-ID", id.to_string()));
            }
            if let Some(id) = auftrag.geschaeftsfall.line_id() {
                l.push(("LineID", id.to_string()));
            }
            if let Some(d) = wunschtermin(&auftrag.geschaeftsfall) {
                l.push(("Wunschtermin", d.to_string()));
            }
            if let Some(d) = auftrag.termin_neu {
                l.push(("Termin neu", d.to_string()));
            }
        }
        spri::Payload::MeldungRequest { meldung, .. } => {
            l.push(("payload", "message (annehmenMeldungRequest)".to_owned()));
            l.push(("kind", meldung.typ().to_string()));
            let t = meldung.auftragstyp;
            l.push(("business case", format!("{} ({})", t.geschaeftsfall, t.aenderungskennzeichen)));
            l.push(("sent", sent));
            l.push(("externe Auftragsnummer", meldung.externe_auftragsnummer().to_string()));
            if let Some(id) = meldung.vorabstimmungs_id() {
                l.push(("Vorabstimmungs-ID", id.to_string()));
            }
            if let Some(id) = meldung_line(meldung) {
                l.push(("LineID", id.to_string()));
            }
            for (label, d) in meldung_dates(meldung) {
                l.push((label, d.to_string()));
            }
            for c in meldung.codes() {
                l.push(("code", c.to_string()));
            }
        }
        spri::Payload::AuftragResponse { quittung, .. } | spri::Payload::MeldungResponse { quittung, .. } => {
            let what = if matches!(p, spri::Payload::AuftragResponse { .. }) {
                "receipt for an order (annehmenAuftragResponse)"
            } else {
                "receipt for a message (annehmenMeldungResponse)"
            };
            l.push(("payload", what.to_owned()));
            l.push(("kind", "TEQ".to_owned()));
            l.push(("sent", sent));
            if let Some(ext) = &quittung.externe_auftragsnummer {
                l.push(("externe Auftragsnummer", ext.to_string()));
            }
            for pos in &quittung.positionen {
                l.push(("code", pos.code.to_string()));
            }
        }
    }
    l
}

/// The requested date of an order.
const fn wunschtermin(g: &spri::Geschaeftsfall) -> Option<spri::Date> {
    match g {
        spri::Geschaeftsfall::Neu(x) => Some(x.wunschtermin.datum),
        spri::Geschaeftsfall::KueAg(x) => Some(x.wunschtermin.datum),
        spri::Geschaeftsfall::Lae(x) => Some(x.wunschtermin.datum),
        spri::Geschaeftsfall::AenLmae(x) => Some(x.wunschtermin.datum),
        spri::Geschaeftsfall::Est(x) => Some(x.wunschtermin.datum),
        spri::Geschaeftsfall::Pv(x) => Some(x.wunschtermin.datum),
        spri::Geschaeftsfall::Get(_) | spri::Geschaeftsfall::Set(_) => None,
    }
}

const fn meldung_line(m: &spri::Meldung) -> Option<&spri::LineId> {
    use spri::Meldungsinhalt as I;
    match &m.inhalt {
        I::Abm(x) => Some(&x.anschluss.line_id),
        I::AkmPv(x) => Some(&x.line_id),
        I::RuemPv(x) => Some(&x.line_id),
        I::AbmPv(x) => Some(&x.line_id),
        _ => None,
    }
}

fn meldung_dates(m: &spri::Meldung) -> Vec<(&'static str, spri::Date)> {
    use spri::Meldungsinhalt as I;
    match &m.inhalt {
        I::Abm(x) => vec![("verbindlicher Liefertermin", x.verbindlicher_liefertermin.datum)],
        I::Abbm(x) => x.wiedervorlagetermin.map(|d| ("Wiedervorlagetermin", d)).into_iter().collect(),
        I::Erlm(x) => vec![("Erledigungstermin", x.erledigungstermin)],
        I::Entm(x) => vec![("Entgelttermin", x.entgelttermin)],
        I::Vzm(x) => x.verzoegerungstermin.map(|d| ("Verzögerungstermin", d)).into_iter().collect(),
        I::Kdle(x) => vec![("Abschaltungstermin", x.abschaltungstermin)],
        I::TbkLe(x) | I::TbkAg(x) => vec![("Termin", x.termin.datum)],
        I::AkmPv(x) => vec![
            ("Übernahmedatum geplant", x.aufnehmender_provider.uebernahmedatum_geplant),
            ("Antwortfrist", x.aufnehmender_provider.antwortfrist),
        ],
        I::AbmPv(x) => vec![("Übernahmedatum verbindlich", x.aufnehmender_provider.uebernahmedatum_verbindlich)],
        I::ErlmPv(x) => vec![("Erledigungstermin", x.erledigungstermin)],
        I::EntmPv(x) => vec![("Entgelttermin", x.entgelttermin)],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(interface: &str, name: &str) -> Vec<u8> {
        let path = format!("{}/../{interface}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read(path).unwrap()
    }

    #[test]
    fn read_summarises_a_wbci_message() {
        let out = read(&fixture("wbci", "abbm.xml"));
        assert!(out.ok);
        for line in [
            "interface: WBCI",
            "kind: ABBM",
            "business case: VA-KUE-MRN",
            "Vorabstimmungs-ID: DEU.EKP1.V000000013",
            "code: 8103 Rufnummer nicht geschaltet (RNG)",
        ] {
            assert!(out.lines.iter().any(|l| l == line), "{line} in {out}");
        }
    }

    #[test]
    fn check_of_a_clean_order() {
        let out = check(&fixture("spri", "auftrag-pv.xml"));
        assert!(out.ok);
        assert_eq!(out.lines, ["S/PRI: no issues"]);
    }

    #[test]
    fn code_in_both_interfaces() {
        let out = code("8103", true, true);
        assert!(out.ok);
        assert_eq!(out.lines[0], "WBCI 8103 Rufnummer nicht geschaltet (RNG)");
        assert!(out.lines.contains(&"S/PRI: 8103 is not an S/PRI code".to_owned()), "{out}");
        assert!(!code("0021", true, false).ok);
    }

    #[test]
    fn a_doctype_is_unreadable_with_teq_0995() {
        let mut xml = b"<!DOCTYPE x [<!ENTITY a \"b\">]>\n".to_vec();
        xml.extend(fixture("wbci", "abbm.xml"));
        for out in [read(&xml), check(&xml)] {
            assert!(!out.ok);
            assert_eq!(out.lines.len(), 1);
            assert!(out.lines[0].starts_with("unreadable: WBCI: DOCTYPE"), "{out}");
            assert!(out.lines[0].contains("answer with TEQ 0995"), "{out}");
        }
        assert!(Payload::parse(&xml).unwrap_err().teq().starts_with("0995 "));
    }

    #[test]
    fn control_characters_are_replaced() {
        // playground/SR-003
        let xml = String::from_utf8(fixture("wbci", "abbm.xml")).unwrap();
        let evil = xml.replace("<Meldungscode>8103</Meldungscode>", "<Meldungscode>\u{1b}[2J</Meldungscode>");
        for out in [read(evil.as_bytes()), check(evil.as_bytes()), code("\u{1b}[2J", true, true)] {
            let text = out.to_string();
            assert!(!text.contains('\u{1b}') && text.contains('\u{FFFD}'), "{text:?}");
        }
        assert_eq!(sanitize("a\u{0}b\u{7f}c\u{9b}"), "a\u{FFFD}b\u{FFFD}c\u{FFFD}");
    }
}
