//! Validation that predicts the network owner's refusal.
//!
//! Values are valid by construction (their types check the schema's facets); these rules cover
//! what types cannot: which business case allows a Storno or a Terminverschiebung, an
//! Auftragsklammer's size, a Zeitfenster against the weekday, a follow-up order against its
//! original, a code against the message type and business case, the attributes a code needs.
//! Every [`Issue`] names the element path, the rule, its source and, where the sources give one,
//! the code of the ABBM the network owner answers with. Issues never contain personal data.
//!
//! ```
//! use spri::*;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let xml = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/auftrag-aen-lmae.xml"))?;
//! # let Payload::AuftragRequest { mut auftrag, .. } = Payload::from_xml(xml.as_bytes())? else { unreachable!() };
//! // A feature change has no Terminverschiebung (S/PRI 4.4 §5.3.1.3).
//! auftrag.aenderungskennzeichen = Aenderungskennzeichen::Terminverschiebung;
//! let issues = auftrag.issues();
//! assert_eq!(issues[0].rule, "spri.aekz.terminverschiebung");
//! assert_eq!(issues[0].code, Some(Meldungscode::C1007));
//! # Ok(())
//! # }
//! ```
//!
//! ## The rules
//!
//! Severity *error*: the text says the network owner refuses, or uses „must"/„not allowed";
//! *warning*: advised against or likely a problem. Messages are not answered with an ABBM, so
//! their issues carry no code. (I) marks an interpretation.
//!
//! | Rule | Checks | Source | Code |
//! |---|---|---|---|
//! | `spri.aekz.storno` | no Storno in GET and SET (messages: nor KUE-LE) | S/PRI 4.4 §6.2.1.2, §6.2.2.2; Anl. 7 EPK V.13 p.4 | 1007 |
//! | `spri.aekz.terminverschiebung` | Terminverschiebung only in NEU, KUE-AG, LAE, PV | S/PRI 4.4 §5.3.1.3, §6.1.3, §6.2.1.3, §6.2.2.3 | 1007 |
//! | `spri.termin-neu.only-tv` | `terminNeu` only with Terminverschiebung (warning) | XSD 4.00.03 `AuftragType` | |
//! | `spri.geschaeftsfallart.match` | the Geschäftsfallart belongs to the business case; PV also `Bereitstellung` (I) | S/PRI 4.4 §4.1 | |
//! | `spri.auftragsklammer.count` | an Auftragsklammer holds 1 to 20 orders | S/PRI 4.4 §4.3.3; AH 2.9.5 §3.7 | |
//! | `spri.auftragsklammer.case` | only NEU, AEN-LMAE, LAE and PV are bracketed | AH 2.9.5 §3.7; S/PRI 4.4 §4.3.3 | |
//! | `spri.zeitfenster.weekday` | the Zeitfenster fits the weekday of the date; „Werktags" = Monday to Saturday (I) | AH 2.9.5 §3.3; XSD 4.00.03 `ZeitfensterType` | 1032 |
//! | `spri.kue-ag.wita-vertragsnummer` | a KUE-AG with Vorabstimmungs-ID names the WITA contract number (warning, I) | S/PRI 4.4 §5.2.1.4 | |
//! | `spri.line-id.va-id` | the LineID is not the Vorabstimmungs-ID (I: the network owner keeps the two ranges apart, so it answers „LineID ist nicht bekannt") | LineID V1.4 §3.1.3 | 8144 |
//! | `spri.termin.past` | the date is not in the past (`issues_at`); 1003 outside EST (I) | AH 2.9.5 §6.4 (7014, 1003) | 7014, 1003 |
//! | `spri.termin.horizon` | the date is at most 180 days ahead (warning, `issues_at`) | S/PRI 4.4 §2, §5.1.4 | 1003 |
//! | `spri.termin.working-day` | the date is a working day (warning, `issues_at`) | AH 2.9.5 §6.4 (0002) | 0002 |
//! | `spri.bezug.order-number` | a Storno, TV or Reklamation repeats the original's order number | S/PRI 4.4 §5.1.3, §5.1.4, §6.1.5 | |
//! | `spri.bezug.case` | … and its business case | S/PRI 4.4 §5.1.3, §5.1.4 | |
//! | `spri.bezug.original` | … and refers to a standard order | AH 2.9.5 §3.4, §3.5 | |
//! | `spri.bezug.repeated` | a new standard order does not reuse an order number | AH 2.9.5 §3.9 | 1013 |
//! | `spri.storno.identical` | a Storno is identical to the original but for the Änderungskennzeichen (warning) | S/PRI 4.4 §5.1.3, §5.3.1.2, §6.1.2 | |
//! | `spri.tv.identical` | a TV changes only date, `terminNeu`, Anlage, Montageleistung, Seriennummer ONT, Vorabstimmungs-ID (warning) | S/PRI 4.4 §5.1.4 | |
//! | `spri.tv.new-date` | a TV names a date other than the original's | S/PRI 4.4 §5.1.4 | 1085 |
//! | `spri.tv.earlier` | a PV is not moved earlier | AH 2.9.5 §3.4 | 1085 |
//! | `spri.tv.zeitfenster` | a TV keeps the Zeitfenster | AH 2.9.5 §3.4 | 1085 |
//! | `spri.reklamation.identical` | a Reklamation changes only Anlage, contacts, date, Störungsbeschreibung (warning) | S/PRI 4.4 §6.1.5 | |
//! | `spri.code.unknown` | the code is in the published list (warning: the online list grows) | AH 2.9.5 §6.3; S/PRI 4.4 §11.2.4 | |
//! | `spri.code.applicability` | the code belongs to the message type and the business case | AH 2.9.5 §6.4 | |
//! | `spri.code.1312` | `1312` only together with `1310` or `1311` | AH 2.9.5 §6.4 (1312) | |
//! | `spri.code.freitext` | `1312`, `8600`, `8601` carry a text of their own (warning) | AH 2.9.5 §6.4 (1312, 8600, 8601) | |
//! | `spri.vzm.verzoegerungstermin` | a VZM with `1576`, `1580` or `1581` names the Verzögerungstermin | AH 2.9.5 §6.4 | |
//! | `spri.abbm.erledigungstermin` | an ABBM `1006`/`1012` gives the open order's date in `wiedervorlagetermin` (warning, I) | AH 2.9.5 §6.4 (1006, 1012) | |
//! | `spri.abbm.abgebender-provider` | an ABBM `0041`/`0042` names the losing provider | AH 2.9.5 §6.4 (0041, 0042) | |
//! | `spri.ruem-pv.antwortcode-missing` | a refusing RUEM-PV gives a reason | S/PRI 4.4 §5.5.1.1 | |
//! | `spri.ruem-pv.antwortcode-known` | the reason is `LnB`, `KVE`, `TAV`, `VnB` or `Vnv`, ignoring case (I) | S/PRI 4.4 §5.5.1.1; AH 2.9.5 §6.5 | |
//! | `spri.ruem-pv.contradiction` | a consenting RUEM-PV carries no refusal reason (I) | S/PRI 4.4 §5.5.1.1 | |
//! | `spri.teq.code` | a receipt carries only `OK`, `0990`, `0991`, `0993`, `0995`, `0999` | AH 2.9.5 §6.4 | |
//! | `spri.teq.mixed` | a receipt is either `OK` or an error, not both (I) | S/PRI 4.4 §11.2.3.1 | |
//!
//! The rules shared with [`crate::process::Process`] (Storno and Terminverschiebung per
//! business case, a TV with a new date, PV not earlier, the refusal reasons) use the same
//! predicates and codes. Ordering rules — a Storno before the QEB, a second open TV, the 36-hour
//! window — need the order's history and are the process's job.

use core::fmt;

use carrier_core::Calendar;
use carrier_core::count::day_of;
use jiff::civil::{Date, Weekday};
use jiff::{Timestamp, ToSpan};

use crate::process::{has_storno, has_tv, is_refusal_code};
use crate::{
    Abbm, Aenderungskennzeichen, Auftrag, Auftragstyp, Geschaeftsfall, GeschaeftsfallArt, GeschaeftsfallCode, JaNein,
    Meldung, Meldungscode, Meldungsinhalt, Meldungstyp, Position, RuemPv, Teq, Vzm, Wunschtermin, Zeitfenster,
};

/// How serious an issue is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Advised against or likely a problem; the peer will probably still accept it.
    Warning,
    /// The peer refuses it, or the specification says it must not be sent so.
    Error,
}

/// One finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// Error or warning.
    pub severity: Severity,
    /// Element path in XML names, e.g. `auftrag/aenderungskennzeichen`.
    pub path: String,
    /// Stable rule identifier, e.g. `spri.aekz.terminverschiebung`.
    pub rule: &'static str,
    /// What is wrong, in one sentence; codes, dates and counts only.
    pub message: String,
    /// Where the rule is written: document, version, clause.
    pub source: &'static str,
    /// The code of the ABBM the network owner answers with, where the sources give one.
    pub code: Option<Meldungscode>,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sev = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        write!(f, "{sev} [{}] {}: {} ({})", self.rule, self.path, self.message, self.source)?;
        if let Some(code) = &self.code {
            write!(f, " → {}", code.as_str())?;
        }
        Ok(())
    }
}

#[derive(Default)]
struct Report(Vec<Issue>);

impl Report {
    fn add(
        &mut self,
        severity: Severity,
        path: impl Into<String>,
        rule: &'static str,
        message: impl Into<String>,
        source: &'static str,
        code: Option<Meldungscode>,
    ) {
        self.0.push(Issue { severity, path: path.into(), rule, message: message.into(), source, code });
    }

    fn error(
        &mut self,
        path: impl Into<String>,
        rule: &'static str,
        message: impl Into<String>,
        source: &'static str,
        code: Option<Meldungscode>,
    ) {
        self.add(Severity::Error, path, rule, message, source, code);
    }

    fn warn(
        &mut self,
        path: impl Into<String>,
        rule: &'static str,
        message: impl Into<String>,
        source: &'static str,
        code: Option<Meldungscode>,
    ) {
        self.add(Severity::Warning, path, rule, message, source, code);
    }
}

const WUNSCH: &str = "auftrag/geschaeftsfall/termine/auftraggeberWunschtermin";
const SRC_STORNO: &str = "S/PRI 4.4 §6.2.1.2, §6.2.2.2; Anl. 7 EPK V.13 p.4";
const SRC_TV: &str = "S/PRI 4.4 §5.3.1.3, §6.1.3, §6.2.1.3, §6.2.2.3";
const SRC_ART: &str = "S/PRI 4.4 §4.1";
const SRC_FACTS: &str = "AH 2.9.5 §6.4";

fn wunschtermin(g: &Geschaeftsfall) -> Option<&Wunschtermin> {
    match g {
        Geschaeftsfall::Neu(x) => Some(&x.wunschtermin),
        Geschaeftsfall::KueAg(x) => Some(&x.wunschtermin),
        Geschaeftsfall::Lae(x) => Some(&x.wunschtermin),
        Geschaeftsfall::AenLmae(x) => Some(&x.wunschtermin),
        Geschaeftsfall::Est(x) => Some(&x.wunschtermin),
        Geschaeftsfall::Pv(x) => Some(&x.wunschtermin),
        Geschaeftsfall::Get(_) | Geschaeftsfall::Set(_) => None,
    }
}

/// The Geschäftsfallarten a business case is filed under (S/PRI 4.4 §4.1). PV also as
/// `Bereitstellung`: the contract treats it as a provisioning (§5.5.1.1, interpretation).
const fn art_fits(fall: GeschaeftsfallCode, art: GeschaeftsfallArt) -> bool {
    use GeschaeftsfallArt as A;
    use GeschaeftsfallCode as G;
    matches!(
        (fall, art),
        (G::Neu, A::Bereitstellung)
            | (G::KueAg | G::KueLe, A::Kuendigung)
            | (G::Lae | G::AenLmae, A::Aenderung)
            | (G::Est, A::Entstoerung)
            | (G::Pv, A::Endkundenanbieterwechsel | A::Bereitstellung)
            | (G::Get | G::Set, A::Diagnose)
    )
}

/// Änderungskennzeichen and Geschäftsfallart against the business case, for orders and messages.
fn auftragstyp(
    r: &mut Report,
    root: &str,
    fall: GeschaeftsfallCode,
    aekz: Aenderungskennzeichen,
    art: GeschaeftsfallArt,
    order: bool,
) {
    let aekz_path = if order { "auftrag/aenderungskennzeichen" } else { "meldung/auftragstyp/aenderungsKennzeichen" };
    let code = |c: Meldungscode| if order { Some(c) } else { None };
    match aekz {
        Aenderungskennzeichen::Storno if !has_storno(fall) => r.error(
            aekz_path,
            "spri.aekz.storno",
            format!("{fall} has no Storno"),
            SRC_STORNO,
            code(Meldungscode::C1007),
        ),
        Aenderungskennzeichen::Terminverschiebung if !has_tv(fall) => r.error(
            aekz_path,
            "spri.aekz.terminverschiebung",
            format!("{fall} has no Terminverschiebung; only NEU, KUE-AG, LAE and PV have one"),
            SRC_TV,
            code(Meldungscode::C1007),
        ),
        _ => {}
    }
    if !art_fits(fall, art) {
        let path =
            if order { format!("{root}/geschaeftsfallArt") } else { format!("{root}/auftragstyp/geschaeftsfallart") };
        r.error(path, "spri.geschaeftsfallart.match", format!("{fall} is not filed under {art}"), SRC_ART, None);
    }
}

/// The weekdays a Zeitfenster may be used on: AH 2.9.5 §3.3 (windows 20–41) and the XSD
/// annotations of windows 1–3; „Werktags" read as Monday to Saturday (interpretation). `None`
/// for windows the documents assign no weekday.
fn weekdays(z: Zeitfenster) -> Option<(&'static [Weekday], &'static str)> {
    use Weekday::{Friday as Fr, Monday as Mo, Saturday as Sa, Thursday as Th, Tuesday as Tu, Wednesday as We};
    const MO_FR: &[Weekday] = &[Mo, Tu, We, Th, Fr];
    const MO_SA: &[Weekday] = &[Mo, Tu, We, Th, Fr, Sa];
    let n: u8 = z.to_string().parse().ok()?;
    Some(match n {
        1 | 2 => (MO_SA, "Monday to Saturday"),
        3 | 24 | 40 | 41 => (&[Sa], "Saturday"),
        22 => (&[Tu], "Tuesday"),
        23 => (&[Fr], "Friday"),
        25 => (&[We], "Wednesday"),
        20 | 21 | 26..=39 => (MO_FR, "Monday to Friday"),
        _ => return None,
    })
}

fn zeitfenster(r: &mut Report, t: Wunschtermin, datum: Date) {
    if let Some(z) = t.zeitfenster
        && let Some((days, name)) = weekdays(z)
        && !days.contains(&datum.weekday())
    {
        r.error(
            format!("{WUNSCH}/zeitfenster"),
            "spri.zeitfenster.weekday",
            format!("Zeitfenster {z} is for {name}; {datum} is a {:?}", datum.weekday()),
            "AH 2.9.5 §3.3; XSD 4.00.03 ZeitfensterType",
            Some(Meldungscode::C1032),
        );
    }
}

/// The date the order asks for: the wish date of a standard order, the new date of a
/// Terminverschiebung (`terminNeu`, else the wish date); none for a Storno or a diagnosis.
fn requested(a: &Auftrag) -> Option<(Date, &'static str)> {
    let t = wunschtermin(&a.geschaeftsfall)?;
    match a.aenderungskennzeichen {
        Aenderungskennzeichen::Standard => {
            Some((t.datum, "auftrag/geschaeftsfall/termine/auftraggeberWunschtermin/datum"))
        }
        Aenderungskennzeichen::Terminverschiebung => Some(match a.termin_neu {
            Some(d) => (d, "auftrag/terminNeu"),
            None => (t.datum, "auftrag/geschaeftsfall/termine/auftraggeberWunschtermin/datum"),
        }),
        Aenderungskennzeichen::Storno => None,
    }
}

impl Auftrag {
    /// Checks the order on its own: Änderungskennzeichen, Geschäftsfallart, Auftragsklammer,
    /// Zeitfenster and identifiers (see the [module rules](crate::validate)).
    #[must_use]
    pub fn issues(&self) -> Vec<Issue> {
        let mut r = Report::default();
        let fall = self.geschaeftsfall.code();
        auftragstyp(&mut r, "auftrag", fall, self.aenderungskennzeichen, self.geschaeftsfall_art, true);
        if self.termin_neu.is_some() && self.aenderungskennzeichen != Aenderungskennzeichen::Terminverschiebung {
            r.warn(
                "auftrag/terminNeu",
                "spri.termin-neu.only-tv",
                format!("terminNeu is given only with a Terminverschiebung, not with {}", self.aenderungskennzeichen),
                "XSD 4.00.03 AuftragType",
                None,
            );
        }
        if let Some(k) = self.kenner.as_ref().and_then(|k| k.auftragskenner.as_ref()) {
            if !(1..=20).contains(&k.anzahl_auftraege) {
                r.error(
                    "auftrag/kenner/auftragskenner/anzahlAuftraege",
                    "spri.auftragsklammer.count",
                    format!("an Auftragsklammer holds 1 to 20 orders, not {}", k.anzahl_auftraege),
                    "S/PRI 4.4 §4.3.3; AH 2.9.5 §3.7",
                    None,
                );
            }
            if !matches!(
                fall,
                GeschaeftsfallCode::Neu
                    | GeschaeftsfallCode::AenLmae
                    | GeschaeftsfallCode::Lae
                    | GeschaeftsfallCode::Pv
            ) {
                r.error(
                    "auftrag/kenner/auftragskenner",
                    "spri.auftragsklammer.case",
                    format!("{fall} orders are not bracketed; only NEU, AEN-LMAE, LAE and PV"),
                    "AH 2.9.5 §3.7; S/PRI 4.4 §4.3.3",
                    None,
                );
            }
        }
        if let (Some(t), Some((datum, _))) = (wunschtermin(&self.geschaeftsfall), requested(self)) {
            zeitfenster(&mut r, *t, datum);
        }
        if let Geschaeftsfall::KueAg(k) = &self.geschaeftsfall
            && k.vorabstimmungs_id.is_some()
            && k.fttx.as_ref().and_then(|w| w.wita_vertragsnummer.as_ref()).is_none()
        {
            r.warn(
                "auftrag/geschaeftsfall/auftragsposition/geschaeftsfallProdukt/witaVertragsnummer",
                "spri.kue-ag.wita-vertragsnummer",
                "a KUE-AG for a provider switch names the Vorabstimmungs-ID and the WITA contract number",
                "S/PRI 4.4 §5.2.1.4",
                None,
            );
        }
        if let (Some(line), Some(va)) = (self.geschaeftsfall.line_id(), self.geschaeftsfall.vorabstimmungs_id())
            && line.as_str() == va.as_str()
        {
            r.error(
                "auftrag/geschaeftsfall/lineId",
                "spri.line-id.va-id",
                "the LineID repeats the Vorabstimmungs-ID; line codes and pre-agreement ids are distinct ranges",
                "LineID V1.4 §3.1.3",
                Some(Meldungscode::C8144),
            );
        }
        r.0
    }

    /// Checks the order as sent at `now`: [`Auftrag::issues`] plus the rules on the requested
    /// date (of a standard order, or the new date of a Terminverschiebung): not in the past,
    /// at most 180 days ahead, on a working day of `calendar`.
    #[must_use]
    pub fn issues_at(&self, now: Timestamp, calendar: &Calendar) -> Vec<Issue> {
        let mut r = Report(self.issues());
        let Some((datum, path)) = requested(self) else { return r.0 };
        let fall = self.geschaeftsfall.code();
        let today = day_of(now);
        if datum < today {
            let (code, source) = if fall == GeschaeftsfallCode::Est {
                (Meldungscode::C7014, "AH 2.9.5 §6.4 (7014)")
            } else {
                (Meldungscode::C1003, "AH 2.9.5 §6.4 (1003)")
            };
            r.error(
                path,
                "spri.termin.past",
                format!("the date {datum} is before the day of sending, {today}"),
                source,
                Some(code),
            );
        }
        if let Ok(limit) = today.checked_add(180.days())
            && datum > limit
        {
            r.warn(
                path,
                "spri.termin.horizon",
                format!("the date {datum} is more than 180 days ahead (after {limit}); only Konnektivität orders with a pre-agreement may"),
                "S/PRI 4.4 §2, §5.1.4",
                Some(Meldungscode::C1003),
            );
        }
        if fall != GeschaeftsfallCode::Est
            && let Some(why) = calendar.closed(datum)
        {
            r.warn(
                path,
                "spri.termin.working-day",
                format!("the date {datum} is not a working day ({why}); the network owner moves it"),
                "AH 2.9.5 §6.4 (0002)",
                Some(Meldungscode::C0002),
            );
        }
        r.0
    }

    /// Checks a follow-up order against the `original` it refers to: [`Auftrag::issues`] plus,
    /// for a Storno, a Terminverschiebung or an EST Reklamation, the same order number and
    /// business case, what may change and the new date; for any other standard order, that it
    /// does not reuse the original's order number.
    #[must_use]
    pub fn issues_against(&self, original: &Self) -> Vec<Issue> {
        let mut r = Report(self.issues());
        let reklamation = matches!(&self.geschaeftsfall, Geschaeftsfall::Est(e) if e.reklamation.is_some());
        let follow_up = self.aenderungskennzeichen != Aenderungskennzeichen::Standard || reklamation;
        let same_number = self.externe_auftragsnummer == original.externe_auftragsnummer;
        if !follow_up {
            if same_number {
                r.error(
                    "auftrag/externeAuftragsnummer",
                    "spri.bezug.repeated",
                    "a new standard order carries an order number of its own",
                    "AH 2.9.5 §3.9",
                    Some(Meldungscode::C1013),
                );
            }
            return r.0;
        }
        if !same_number {
            r.error(
                "auftrag/externeAuftragsnummer",
                "spri.bezug.order-number",
                format!("a {} repeats the order number of the order it refers to", kind(self, reklamation)),
                "S/PRI 4.4 §5.1.3, §5.1.4, §6.1.5",
                None,
            );
        }
        let fall = self.geschaeftsfall.code();
        if fall != original.geschaeftsfall.code() {
            r.error(
                "auftrag/geschaeftsfall",
                "spri.bezug.case",
                format!(
                    "a {} keeps the business case: {fall}, original {}",
                    kind(self, reklamation),
                    original.geschaeftsfall.code()
                ),
                "S/PRI 4.4 §5.1.3, §5.1.4",
                None,
            );
            return r.0;
        }
        if original.aenderungskennzeichen != Aenderungskennzeichen::Standard {
            r.error(
                "auftrag/aenderungskennzeichen",
                "spri.bezug.original",
                format!("the original is a {}, not a standard order", original.aenderungskennzeichen),
                "AH 2.9.5 §3.4, §3.5",
                None,
            );
        }
        match self.aenderungskennzeichen {
            Aenderungskennzeichen::Storno => {
                let mut mine = self.clone();
                mine.aenderungskennzeichen = original.aenderungskennzeichen;
                mine.externe_auftragsnummer.clone_from(&original.externe_auftragsnummer);
                if mine != *original {
                    r.warn(
                        "auftrag",
                        "spri.storno.identical",
                        "a Storno is identical to the original order but for the Änderungskennzeichen",
                        "S/PRI 4.4 §5.1.3, §5.3.1.2, §6.1.2",
                        None,
                    );
                }
            }
            Aenderungskennzeichen::Terminverschiebung => tv(&mut r, self, original),
            Aenderungskennzeichen::Standard => {
                if with_reklamation_fields_of(self, original) != *original {
                    r.warn(
                        "auftrag",
                        "spri.reklamation.identical",
                        "a Reklamation is identical to the repair order but for Anlage, contacts, date, Störungsbeschreibung and the Reklamation",
                        "S/PRI 4.4 §6.1.5",
                        None,
                    );
                }
            }
        }
        r.0
    }
}

fn kind(a: &Auftrag, reklamation: bool) -> &'static str {
    match a.aenderungskennzeichen {
        Aenderungskennzeichen::Storno => "Storno",
        Aenderungskennzeichen::Terminverschiebung => "Terminverschiebung",
        Aenderungskennzeichen::Standard if reklamation => "Reklamation",
        Aenderungskennzeichen::Standard => "standard order",
    }
}

fn tv(r: &mut Report, a: &Auftrag, original: &Auftrag) {
    let (Some(new), Some(old)) = (wunschtermin(&a.geschaeftsfall), wunschtermin(&original.geschaeftsfall)) else {
        return;
    };
    let datum = a.termin_neu.unwrap_or(new.datum);
    let path = if a.termin_neu.is_some() {
        "auftrag/terminNeu"
    } else {
        "auftrag/geschaeftsfall/termine/auftraggeberWunschtermin/datum"
    };
    if datum == old.datum {
        r.error(
            path,
            "spri.tv.new-date",
            format!("a Terminverschiebung names a date other than the original {}", old.datum),
            "S/PRI 4.4 §5.1.4",
            Some(Meldungscode::C1085),
        );
    }
    if a.geschaeftsfall.code() == GeschaeftsfallCode::Pv && datum < old.datum {
        r.error(
            path,
            "spri.tv.earlier",
            format!("a PV cannot move earlier than {} after the positive pre-agreement", old.datum),
            "AH 2.9.5 §3.4",
            Some(Meldungscode::C1085),
        );
    }
    if new.zeitfenster != old.zeitfenster {
        r.error(
            format!("{WUNSCH}/zeitfenster"),
            "spri.tv.zeitfenster",
            "a Terminverschiebung keeps the Zeitfenster; another window needs a Storno and a new order",
            "AH 2.9.5 §3.4",
            Some(Meldungscode::C1085),
        );
    }
    if with_tv_fields_of(a, original) != *original {
        r.warn(
            "auftrag",
            "spri.tv.identical",
            "a Terminverschiebung is identical to the original but for date, terminNeu, Anlage, Montageleistung, Seriennummer ONT and Vorabstimmungs-ID",
            "S/PRI 4.4 §5.1.4",
            None,
        );
    }
}

/// `a` with the fields a Terminverschiebung may change taken from `original` (S/PRI 4.4
/// §5.1.4: date, Anlage, Montageleistung; the Seriennummer ONT for the automatic line
/// identification; the Vorabstimmungs-ID of a Konnektivität order). The Zeitfenster is taken
/// too: it has its own rule.
fn with_tv_fields_of(a: &Auftrag, original: &Auftrag) -> Auftrag {
    let mut a = a.clone();
    a.aenderungskennzeichen = original.aenderungskennzeichen;
    a.externe_auftragsnummer.clone_from(&original.externe_auftragsnummer);
    a.termin_neu = original.termin_neu;
    match (&mut a.geschaeftsfall, &original.geschaeftsfall) {
        (Geschaeftsfall::Neu(x), Geschaeftsfall::Neu(o)) => {
            x.wunschtermin = o.wunschtermin;
            x.anlage.clone_from(&o.anlage);
            x.vorabstimmungs_id.clone_from(&o.vorabstimmungs_id);
            x.fttx.montageleistung.clone_from(&o.fttx.montageleistung);
            if let (Some(v), Some(ov)) = (&mut x.fttx.vormieter, &o.fttx.vormieter) {
                v.seriennummer_ont.clone_from(&ov.seriennummer_ont);
            }
        }
        (Geschaeftsfall::KueAg(x), Geschaeftsfall::KueAg(o)) => {
            x.wunschtermin = o.wunschtermin;
            x.anlage.clone_from(&o.anlage);
        }
        (Geschaeftsfall::Lae(x), Geschaeftsfall::Lae(o)) => {
            x.wunschtermin = o.wunschtermin;
            x.anlage.clone_from(&o.anlage);
        }
        (Geschaeftsfall::AenLmae(x), Geschaeftsfall::AenLmae(o)) => {
            x.wunschtermin = o.wunschtermin;
            x.anlage.clone_from(&o.anlage);
        }
        (Geschaeftsfall::Est(x), Geschaeftsfall::Est(o)) => {
            x.wunschtermin = o.wunschtermin;
            x.anlage.clone_from(&o.anlage);
        }
        (Geschaeftsfall::Pv(x), Geschaeftsfall::Pv(o)) => {
            x.wunschtermin = o.wunschtermin;
            x.anlage.clone_from(&o.anlage);
            x.fttx.montageleistung.clone_from(&o.fttx.montageleistung);
        }
        _ => {}
    }
    a
}

/// `a` with the fields a Reklamation may change taken from the repair order `original`
/// (S/PRI 4.4 §6.1.5: Anlagen, Ansprechpartner, Kontakt Endkunde, Termin,
/// Störungsbeschreibung; and the Reklamation itself).
fn with_reklamation_fields_of(a: &Auftrag, original: &Auftrag) -> Auftrag {
    let mut a = a.clone();
    a.externe_auftragsnummer.clone_from(&original.externe_auftragsnummer);
    if let (Geschaeftsfall::Est(x), Geschaeftsfall::Est(o)) = (&mut a.geschaeftsfall, &original.geschaeftsfall) {
        x.anlage.clone_from(&o.anlage);
        x.ansprechpartner.clone_from(&o.ansprechpartner);
        x.kontakt_endkunde.clone_from(&o.kontakt_endkunde);
        x.wunschtermin = o.wunschtermin;
        x.stoerungsbeschreibung.clone_from(&o.stoerungsbeschreibung);
        x.reklamation.clone_from(&o.reklamation);
    }
    a
}

// -- messages --------------------------------------------------------------------------------

/// The codes whose text is a free text of the network owner (AH 2.9.5 §6.4).
const FREITEXT: [&str; 3] = ["1312", "8600", "8601"];

fn position_path(i: usize) -> String {
    format!("meldung/meldungstyp/meldungspositionen/position[{}]", i.saturating_add(1))
}

impl Meldung {
    /// Checks the message: each code against the code table (AH 2.9.5 §6.4) for this message
    /// type and business case, the Änderungskennzeichen and Geschäftsfallart, and the
    /// attributes a code requires (see the [module rules](crate::validate)).
    #[must_use]
    pub fn issues(&self) -> Vec<Issue> {
        let mut r = Report::default();
        let Auftragstyp { geschaeftsfall: fall, aenderungskennzeichen, geschaeftsfallart } = self.auftragstyp;
        auftragstyp(&mut r, "meldung", fall, aenderungskennzeichen, geschaeftsfallart, false);
        let typ = self.typ();
        let positions = positions(&self.inhalt);
        for (i, p) in positions.iter().enumerate() {
            let path = format!("{}/meldungscode", position_path(i));
            let code = &p.code;
            if code.text().is_none() {
                r.warn(
                    path,
                    "spri.code.unknown",
                    "the code is not in the published list (AH 2.9.5 §6.3); later codes come from the online list",
                    "AH 2.9.5 §6.3; S/PRI 4.4 §11.2.4",
                    None,
                );
            } else if !code.applies(typ, fall) {
                r.error(
                    path,
                    "spri.code.applicability",
                    format!("{} is not used in a {typ} of {fall}", code.as_str()),
                    SRC_FACTS,
                    None,
                );
            }
            if FREITEXT.contains(&code.as_str()) {
                let text = p.text.as_str().trim();
                if text.is_empty() || code.text().is_some_and(|official| official == text) {
                    r.warn(
                        format!("{}/meldungstext", position_path(i)),
                        "spri.code.freitext",
                        format!("{} carries the network owner's own text, not the placeholder", code.as_str()),
                        "AH 2.9.5 §6.4 (1312, 8600, 8601)",
                        None,
                    );
                }
            }
        }
        if let Some(i) = positions.iter().position(|p| p.code == Meldungscode::C1312)
            && !positions.iter().any(|p| matches!(p.code, Meldungscode::C1310 | Meldungscode::C1311))
        {
            r.error(
                format!("{}/meldungscode", position_path(i)),
                "spri.code.1312",
                "1312 only adds to 1310 or 1311 in the same message",
                "AH 2.9.5 §6.4 (1312)",
                None,
            );
        }
        match &self.inhalt {
            Meldungsinhalt::Vzm(v) => vzm(&mut r, v),
            Meldungsinhalt::Abbm(a) => abbm(&mut r, a),
            Meldungsinhalt::RuemPv(m) => ruem_pv(&mut r, m),
            _ => {}
        }
        r.0
    }
}

fn positions(m: &Meldungsinhalt) -> Vec<&Position> {
    fn plain(p: &[Position]) -> Vec<&Position> {
        p.iter().collect()
    }
    fn wita(p: &[crate::PositionWita]) -> Vec<&Position> {
        p.iter().map(|p| &p.position).collect()
    }
    match m {
        Meldungsinhalt::Qeb(x) => x.positionen.iter().map(|p| &p.position).collect(),
        Meldungsinhalt::Abm(x) => x.positionen.iter().map(|p| &p.position.position).collect(),
        Meldungsinhalt::Abbm(x) => wita(&x.positionen),
        Meldungsinhalt::Erlm(x) => wita(&x.positionen),
        Meldungsinhalt::Vzm(x) => wita(&x.positionen),
        Meldungsinhalt::Tam(x) | Meldungsinhalt::Mtam(x) => wita(&x.positionen),
        Meldungsinhalt::Entm(x) => plain(&x.positionen),
        Meldungsinhalt::Kdle(x) => plain(&x.positionen),
        Meldungsinhalt::ZwmLe(x) => plain(&x.positionen),
        Meldungsinhalt::ZwmAg(x) => plain(&x.positionen),
        Meldungsinhalt::TbkLe(x) | Meldungsinhalt::TbkAg(x) => plain(&x.positionen),
        Meldungsinhalt::AkmPv(x) => plain(&x.positionen),
        Meldungsinhalt::RuemPv(x) => plain(&x.positionen),
        Meldungsinhalt::AbmPv(x) => plain(&x.positionen),
        Meldungsinhalt::ErlmPv(x) => plain(&x.positionen),
        Meldungsinhalt::EntmPv(x) => plain(&x.positionen),
        Meldungsinhalt::AbbmPv(x) => plain(&x.positionen),
        Meldungsinhalt::ErlmK(x) => plain(&x.positionen),
    }
}

/// The first of `codes` among the positions, as written (only known codes are passed).
fn has<'a>(p: &'a [crate::PositionWita], codes: &[Meldungscode]) -> Option<&'a str> {
    p.iter().find(|p| codes.contains(&p.position.code)).map(|p| p.position.code.as_str())
}

fn vzm(r: &mut Report, v: &Vzm) {
    if v.verzoegerungstermin.is_none()
        && let Some(code) = has(&v.positionen, &[Meldungscode::C1576, Meldungscode::C1580, Meldungscode::C1581])
    {
        r.error(
            "meldung/meldungstyp/meldungsattribute/verzoegerungstermin",
            "spri.vzm.verzoegerungstermin",
            format!("a VZM with {code} names the appointment in verzoegerungstermin"),
            "AH 2.9.5 §6.4 (1576, 1580, 1581)",
            None,
        );
    }
}

fn abbm(r: &mut Report, a: &Abbm) {
    if a.wiedervorlagetermin.is_none()
        && let Some(code) = has(&a.positionen, &[Meldungscode::C1006, Meldungscode::C1012])
    {
        r.warn(
            "meldung/meldungstyp/meldungsattribute/wiedervorlagetermin",
            "spri.abbm.erledigungstermin",
            format!("an ABBM {code} gives the completion date of the open order (in wiedervorlagetermin)"),
            "AH 2.9.5 §6.4 (1006, 1012)",
            None,
        );
    }
    if a.abgebender_provider.as_ref().and_then(|p| p.providername.as_ref()).is_none()
        && let Some(code) = has(&a.positionen, &[Meldungscode::C0041, Meldungscode::C0042])
    {
        r.error(
            "meldung/meldungstyp/meldungsattribute/abgebenderProvider/providername",
            "spri.abbm.abgebender-provider",
            format!("an ABBM {code} names the losing provider"),
            "AH 2.9.5 §6.4 (0041, 0042)",
            None,
        );
    }
}

fn ruem_pv(r: &mut Report, m: &RuemPv) {
    const PATH: &str = "meldung/meldungstyp/meldungsattribute/abgebenderProvider/antwortcode";
    const SRC: &str = "S/PRI 4.4 §5.5.1.1; AH 2.9.5 §6.5";
    let p = &m.abgebender_provider;
    match (p.zustimmung, &p.antwortcode) {
        (JaNein::Nein, None) => r.error(
            PATH,
            "spri.ruem-pv.antwortcode-missing",
            "a refusing RUEM-PV gives its reason (LnB, KVE, TAV, VnB or Vnv)",
            "S/PRI 4.4 §5.5.1.1",
            None,
        ),
        (JaNein::Nein, Some(c)) if !is_refusal_code(c) => r.error(
            PATH,
            "spri.ruem-pv.antwortcode-known",
            "a RUEM-PV refuses only for LnB, KVE, TAV, VnB or Vnv",
            SRC,
            None,
        ),
        (JaNein::Ja, Some(c)) if is_refusal_code(c) => r.error(
            PATH,
            "spri.ruem-pv.contradiction",
            "a consenting RUEM-PV carries no refusal reason; the network owner reads it as a refusal",
            SRC,
            None,
        ),
        _ => {}
    }
}

impl Teq {
    /// Checks a receipt: only receipt codes (`OK`, `0990`, `0991`, `0993`, `0995`, `0999`), and
    /// `OK` never together with an error code.
    #[must_use]
    pub fn issues(&self) -> Vec<Issue> {
        let mut r = Report::default();
        for (i, p) in self.positionen.iter().enumerate() {
            if !p.code.meldungstypen().any(|t| t == Meldungstyp::Teq) {
                r.error(
                    format!("quittung/meldungspositionen/position[{}]/meldungscode", i.saturating_add(1)),
                    "spri.teq.code",
                    // An unknown code comes from the peer: it is not echoed.
                    match p.code.text() {
                        Some(_) => {
                            format!("{} is not a receipt code (OK, 0990, 0991, 0993, 0995, 0999)", p.code.as_str())
                        }
                        None => "the code is not a receipt code (OK, 0990, 0991, 0993, 0995, 0999)".to_owned(),
                    },
                    SRC_FACTS,
                    None,
                );
            }
        }
        if self.positionen.iter().any(|p| p.code.is_ok()) && self.positionen.iter().any(|p| !p.code.is_ok()) {
            r.error(
                "quittung/meldungspositionen",
                "spri.teq.mixed",
                "a receipt is either OK or an error, not both",
                "S/PRI 4.4 §11.2.3.1",
                None,
            );
        }
        r.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Antwortcode, Kenner, Meldungstext, Payload, Vertragsnummer, Wunschtermin, Zeitfenster};

    fn payload(name: &str) -> Payload {
        let path = format!("{}/tests/fixtures/{name}.xml", env!("CARGO_MANIFEST_DIR"));
        Payload::from_xml(&std::fs::read(path).unwrap()).unwrap()
    }

    fn order(name: &str) -> Auftrag {
        let Payload::AuftragRequest { auftrag, .. } = payload(name) else { panic!("{name}") };
        auftrag
    }

    fn message(name: &str) -> Meldung {
        let Payload::MeldungRequest { meldung, .. } = payload(name) else { panic!("{name}") };
        meldung
    }

    fn date(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn at(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

    /// The issues `after` adds to `before`, by rule.
    fn added(before: &[Issue], after: &[Issue]) -> Vec<Issue> {
        after.iter().filter(|i| !before.iter().any(|b| b.rule == i.rule && b.path == i.path)).cloned().collect()
    }

    /// Exactly one new issue: `rule` with `severity` and `code`.
    #[track_caller]
    fn only(before: &[Issue], after: &[Issue], rule: &str, severity: Severity, code: Option<&str>) -> Issue {
        let new = added(before, after);
        assert_eq!(new.len(), 1, "expected only {rule}: {new:#?}");
        let i = new.into_iter().next().unwrap();
        assert_eq!((i.rule, i.severity), (rule, severity), "{i}");
        assert_eq!(i.code.as_ref().map(Meldungscode::as_str), code, "{i}");
        assert!(i.source.contains('§') || i.source.starts_with("XSD"), "source names a clause: {i}");
        i
    }

    fn set_termin(a: &mut Auftrag, t: Wunschtermin) {
        match &mut a.geschaeftsfall {
            Geschaeftsfall::Neu(x) => x.wunschtermin = t,
            Geschaeftsfall::KueAg(x) => x.wunschtermin = t,
            Geschaeftsfall::Lae(x) => x.wunschtermin = t,
            Geschaeftsfall::AenLmae(x) => x.wunschtermin = t,
            Geschaeftsfall::Est(x) => x.wunschtermin = t,
            Geschaeftsfall::Pv(x) => x.wunschtermin = t,
            Geschaeftsfall::Get(_) | Geschaeftsfall::Set(_) => panic!("no date"),
        }
    }

    // -- orders on their own --------------------------------------------------------------

    #[test]
    fn order_fixtures_have_no_error() {
        for name in ["neu", "kue-ag", "lae", "aen-lmae", "est", "pv", "get", "set"] {
            let a = order(&format!("auftrag-{name}"));
            assert!(a.issues().iter().all(|i| i.severity == Severity::Warning), "{name}: {:#?}", a.issues());
        }
    }

    #[test]
    fn storno_only_where_the_case_has_one() {
        for name in ["auftrag-get", "auftrag-set"] {
            let mut a = order(name);
            let before = a.issues();
            a.aenderungskennzeichen = Aenderungskennzeichen::Storno;
            let i = only(&before, &a.issues(), "spri.aekz.storno", Severity::Error, Some("1007"));
            assert_eq!(i.path, "auftrag/aenderungskennzeichen");
        }
        for name in ["auftrag-neu", "auftrag-kue-ag", "auftrag-lae", "auftrag-aen-lmae", "auftrag-pv"] {
            let mut a = order(name);
            let before = a.issues();
            a.aenderungskennzeichen = Aenderungskennzeichen::Storno;
            assert!(added(&before, &a.issues()).iter().all(|i| i.rule != "spri.aekz.storno"), "{name}");
        }
    }

    #[test]
    fn terminverschiebung_only_in_neu_kue_ag_lae_pv() {
        for name in ["auftrag-aen-lmae", "auftrag-est", "auftrag-get", "auftrag-set"] {
            let mut a = order(name);
            let before = a.issues();
            a.aenderungskennzeichen = Aenderungskennzeichen::Terminverschiebung;
            only(&before, &a.issues(), "spri.aekz.terminverschiebung", Severity::Error, Some("1007"));
        }
        let mut a = order("auftrag-lae");
        a.aenderungskennzeichen = Aenderungskennzeichen::Terminverschiebung;
        assert!(a.issues().is_empty(), "{:#?}", a.issues());
    }

    #[test]
    fn termin_neu_only_with_terminverschiebung() {
        let mut a = order("auftrag-lae");
        let before = a.issues();
        a.termin_neu = Some(date("2027-01-20"));
        only(&before, &a.issues(), "spri.termin-neu.only-tv", Severity::Warning, None);
        a.aenderungskennzeichen = Aenderungskennzeichen::Terminverschiebung;
        assert!(a.issues().is_empty());
    }

    #[test]
    fn geschaeftsfallart_belongs_to_the_case() {
        let mut a = order("auftrag-pv");
        let before = a.issues();
        a.geschaeftsfall_art = GeschaeftsfallArt::Diagnose;
        only(&before, &a.issues(), "spri.geschaeftsfallart.match", Severity::Error, None);
        a.geschaeftsfall_art = GeschaeftsfallArt::Bereitstellung;
        assert!(a.issues().is_empty());
    }

    #[test]
    fn auftragsklammer_size_and_cases() {
        let mut a = order("auftrag-neu");
        let before = a.issues();
        for n in [0, 21, -1] {
            if let Some(k) = a.kenner.as_mut().and_then(|k| k.auftragskenner.as_mut()) {
                k.anzahl_auftraege = n;
            }
            only(&before, &a.issues(), "spri.auftragsklammer.count", Severity::Error, None);
        }
        let mut k = order("auftrag-kue-ag");
        let before = k.issues();
        k.kenner = order("auftrag-neu").kenner;
        only(&before, &k.issues(), "spri.auftragsklammer.case", Severity::Error, None);
        k.kenner = Some(Kenner::default());
        assert_eq!(k.issues(), before);
    }

    #[test]
    fn zeitfenster_fits_the_weekday() {
        let mut a = order("auftrag-pv");
        let before = a.issues();
        // 2027-01-15 is a Friday.
        set_termin(&mut a, Wunschtermin { datum: date("2027-01-15"), zeitfenster: Some(Zeitfenster::SAMSTAGS) });
        let i = only(&before, &a.issues(), "spri.zeitfenster.weekday", Severity::Error, Some("1032"));
        assert!(i.path.ends_with("auftraggeberWunschtermin/zeitfenster"), "{i}");
        for (z, d, ok) in [
            ("22", "2027-01-19", true),
            ("22", "2027-01-15", false),
            ("1", "2027-01-16", true),
            ("1", "2027-01-17", false),
            ("30", "2027-01-16", false),
            ("50", "2027-01-17", true),
        ] {
            set_termin(&mut a, Wunschtermin { datum: date(d), zeitfenster: Some(Zeitfenster::new(z).unwrap()) });
            assert_eq!(a.issues().is_empty(), ok, "ZF {z} on {d}");
        }
    }

    #[test]
    fn kue_ag_for_a_switch_names_the_wita_contract() {
        let mut a = order("auftrag-kue-ag");
        let before = a.issues();
        if let Geschaeftsfall::KueAg(k) = &mut a.geschaeftsfall {
            k.fttx = None;
        }
        only(&before, &a.issues(), "spri.kue-ag.wita-vertragsnummer", Severity::Warning, None);
    }

    #[test]
    fn line_id_is_not_the_vorabstimmungs_id() {
        let mut a = order("auftrag-pv");
        let before = a.issues();
        if let Geschaeftsfall::Pv(p) = &mut a.geschaeftsfall {
            p.line_id = p.vorabstimmungs_id.as_str().parse().unwrap();
        }
        only(&before, &a.issues(), "spri.line-id.va-id", Severity::Error, Some("8144"));
    }

    // -- dates ------------------------------------------------------------------------------

    #[test]
    fn date_in_the_past() {
        let cal = Calendar::werktage();
        let a = order("auftrag-pv");
        let before = a.issues_at(at("2027-01-14T12:00:00Z"), &cal);
        assert!(before.is_empty(), "{before:#?}");
        only(
            &before,
            &a.issues_at(at("2027-01-16T12:00:00Z"), &cal),
            "spri.termin.past",
            Severity::Error,
            Some("1003"),
        );
        // The day counts in Berlin: 22:30 UTC is still the 15th, 23:30 UTC already the 16th.
        assert!(a.issues_at(at("2027-01-15T22:30:00Z"), &cal).is_empty());
        assert!(a.issues_at(at("2027-01-15T23:30:00Z"), &cal).iter().any(|i| i.rule == "spri.termin.past"));
        let est = order("auftrag-est");
        let before = est.issues_at(at("2026-11-23T08:00:00Z"), &cal);
        only(
            &before,
            &est.issues_at(at("2026-11-25T08:00:00Z"), &cal),
            "spri.termin.past",
            Severity::Error,
            Some("7014"),
        );
    }

    #[test]
    fn past_date_of_a_terminverschiebung_is_its_new_date() {
        let cal = Calendar::werktage();
        let mut a = order("auftrag-pv");
        a.aenderungskennzeichen = Aenderungskennzeichen::Terminverschiebung;
        a.termin_neu = Some(date("2027-01-20"));
        let now = at("2027-01-18T08:00:00Z");
        assert!(a.issues_at(now, &cal).is_empty(), "{:#?}", a.issues_at(now, &cal));
        a.termin_neu = Some(date("2027-01-16"));
        let i = only(&[], &a.issues_at(now, &cal), "spri.termin.past", Severity::Error, Some("1003"));
        assert_eq!(i.path, "auftrag/terminNeu");
        // A Storno asks for no date.
        a.aenderungskennzeichen = Aenderungskennzeichen::Storno;
        a.termin_neu = None;
        assert!(a.issues_at(at("2028-01-01T08:00:00Z"), &cal).iter().all(|i| !i.rule.starts_with("spri.termin.")));
    }

    #[test]
    fn date_at_most_180_days_ahead() {
        let cal = Calendar::werktage();
        let a = order("auftrag-pv");
        // 2027-01-15 is 181 days after 2026-07-18, 180 after 2026-07-19.
        assert!(a.issues_at(at("2026-07-19T08:00:00Z"), &cal).is_empty());
        only(
            &[],
            &a.issues_at(at("2026-07-18T08:00:00Z"), &cal),
            "spri.termin.horizon",
            Severity::Warning,
            Some("1003"),
        );
    }

    #[test]
    fn date_on_a_working_day() {
        let cal = Calendar::werktage();
        let mut a = order("auftrag-pv");
        set_termin(&mut a, Wunschtermin::from(date("2027-01-17"))); // a Sunday
        let i = only(
            &[],
            &a.issues_at(at("2027-01-04T08:00:00Z"), &cal),
            "spri.termin.working-day",
            Severity::Warning,
            Some("0002"),
        );
        assert!(!i.message.contains("Mustermann"));
    }

    // -- follow-up orders --------------------------------------------------------------------

    fn follow(a: &Auftrag, aekz: Aenderungskennzeichen) -> Auftrag {
        Auftrag { aenderungskennzeichen: aekz, ..a.clone() }
    }

    #[test]
    fn storno_against_its_original() {
        let pv = order("auftrag-pv");
        let s = follow(&pv, Aenderungskennzeichen::Storno);
        assert!(s.issues_against(&pv).is_empty(), "{:#?}", s.issues_against(&pv));
        let mut other = s.clone();
        set_termin(
            &mut other,
            Wunschtermin { datum: date("2027-01-22"), zeitfenster: Some(Zeitfenster::WERKTAGS_VORMITTAG) },
        );
        only(&[], &other.issues_against(&pv), "spri.storno.identical", Severity::Warning, None);
        let mut number = s.clone();
        number.externe_auftragsnummer = crate::ExterneAuftragsnummer::new("A-2026-0002").unwrap();
        only(&[], &number.issues_against(&pv), "spri.bezug.order-number", Severity::Error, None);
        let lae = follow(&order("auftrag-lae"), Aenderungskennzeichen::Storno);
        only(&lae.issues(), &lae.issues_against(&pv), "spri.bezug.case", Severity::Error, None);
        let original_tv = follow(&pv, Aenderungskennzeichen::Terminverschiebung);
        only(&[], &s.issues_against(&original_tv), "spri.bezug.original", Severity::Error, None);
    }

    #[test]
    fn a_new_order_does_not_reuse_an_order_number() {
        let pv = order("auftrag-pv");
        let lae = order("auftrag-lae");
        only(&[], &lae.issues_against(&pv), "spri.bezug.repeated", Severity::Error, Some("1013"));
        let mut fresh = lae;
        fresh.externe_auftragsnummer = crate::ExterneAuftragsnummer::new("A-2026-0002").unwrap();
        assert!(fresh.issues_against(&pv).is_empty());
    }

    #[test]
    fn terminverschiebung_against_its_original() {
        let pv = order("auftrag-pv");
        let mut tv = follow(&pv, Aenderungskennzeichen::Terminverschiebung);
        only(&[], &tv.issues_against(&pv), "spri.tv.new-date", Severity::Error, Some("1085"));
        tv.termin_neu = Some(date("2027-01-22"));
        assert!(tv.issues_against(&pv).is_empty(), "{:#?}", tv.issues_against(&pv));
        tv.termin_neu = Some(date("2027-01-13"));
        only(&[], &tv.issues_against(&pv), "spri.tv.earlier", Severity::Error, Some("1085"));
        // Earlier is allowed outside PV.
        let lae = order("auftrag-lae");
        let mut tv_lae = follow(&lae, Aenderungskennzeichen::Terminverschiebung);
        tv_lae.termin_neu = Some(date("2027-01-13"));
        assert!(tv_lae.issues_against(&lae).is_empty());
        // The date in the wish date instead of terminNeu.
        let mut tv2 = follow(&pv, Aenderungskennzeichen::Terminverschiebung);
        set_termin(
            &mut tv2,
            Wunschtermin { datum: date("2027-01-22"), zeitfenster: Some(Zeitfenster::WERKTAGS_VORMITTAG) },
        );
        assert!(tv2.issues_against(&pv).is_empty());
        set_termin(
            &mut tv2,
            Wunschtermin { datum: date("2027-01-22"), zeitfenster: Some(Zeitfenster::WERKTAGS_NACHMITTAG) },
        );
        only(&[], &tv2.issues_against(&pv), "spri.tv.zeitfenster", Severity::Error, Some("1085"));
    }

    #[test]
    fn terminverschiebung_changes_only_the_allowed_fields() {
        let neu = order("auftrag-neu");
        let mut tv = follow(&neu, Aenderungskennzeichen::Terminverschiebung);
        tv.termin_neu = Some(date("2027-01-27"));
        if let Geschaeftsfall::Neu(n) = &mut tv.geschaeftsfall {
            n.anlage = None;
            n.fttx.montageleistung = None;
            n.vorabstimmungs_id = None;
        }
        let base = tv.issues_against(&neu);
        assert!(base.iter().all(|i| i.rule != "spri.tv.identical"), "{base:#?}");
        if let Geschaeftsfall::Neu(n) = &mut tv.geschaeftsfall {
            n.positionen.clear();
        }
        only(&base, &tv.issues_against(&neu), "spri.tv.identical", Severity::Warning, None);
    }

    #[test]
    fn reklamation_against_its_repair_order() {
        let rek = order("auftrag-est");
        let mut original = rek.clone();
        if let Geschaeftsfall::Est(e) = &mut original.geschaeftsfall {
            e.reklamation = None;
            e.wunschtermin = Wunschtermin::from(date("2026-11-10"));
            e.anlage = None;
        }
        assert!(rek.issues_against(&original).is_empty(), "{:#?}", rek.issues_against(&original));
        let mut other = rek.clone();
        if let Geschaeftsfall::Est(e) = &mut other.geschaeftsfall {
            e.vertragsnummer = Vertragsnummer::new("V999").unwrap();
        }
        only(&[], &other.issues_against(&original), "spri.reklamation.identical", Severity::Warning, None);
    }

    // -- messages -----------------------------------------------------------------------------

    fn with_codes(m: &mut Meldung, codes: &[&str]) {
        let ps: Vec<Position> = codes.iter().map(|c| Position::new(Meldungscode::parse(c))).collect();
        match &mut m.inhalt {
            Meldungsinhalt::Abbm(a) => a.positionen = ps.into_iter().map(Into::into).collect(),
            Meldungsinhalt::Vzm(v) => v.positionen = ps.into_iter().map(Into::into).collect(),
            Meldungsinhalt::Abm(a) => a.positionen = ps.into_iter().map(Into::into).collect(),
            other => panic!("{:?}", other.typ()),
        }
    }

    #[test]
    fn message_fixtures_have_no_issue() {
        for name in [
            "abbm", "abbm-pv", "abm", "abm-pv", "akm-pv", "entm", "entm-pv", "erlm", "erlm-k", "erlm-pv", "kdle",
            "mtam", "qeb", "ruem-pv", "tam", "tbk-ag", "tbk-le", "vzm", "zwm-ag", "zwm-le",
        ] {
            let m = message(&format!("meldung-{name}"));
            assert!(m.issues().is_empty(), "{name}: {:#?}", m.issues());
        }
    }

    #[test]
    fn a_code_belongs_to_message_type_and_case() {
        let mut m = message("meldung-abm");
        with_codes(&mut m, &["0041"]);
        let i = only(&[], &m.issues(), "spri.code.applicability", Severity::Error, None);
        assert_eq!(i.path, "meldung/meldungstyp/meldungspositionen/position[1]/meldungscode");
        assert!(i.message.contains("0041") && i.message.contains("ABM") && i.message.contains("PV"), "{i}");
        let mut m = message("meldung-abbm");
        m.auftragstyp.geschaeftsfall = GeschaeftsfallCode::Pv;
        m.auftragstyp.geschaeftsfallart = GeschaeftsfallArt::Endkundenanbieterwechsel;
        with_codes(&mut m, &["1001"]); // NEU only
        only(&[], &m.issues(), "spri.code.applicability", Severity::Error, None);
    }

    #[test]
    fn an_unknown_code_is_a_warning_and_not_echoed() {
        let mut m = message("meldung-abbm");
        with_codes(&mut m, &["47\u{1b}11"]);
        let i = only(&[], &m.issues(), "spri.code.unknown", Severity::Warning, None);
        assert!(!i.to_string().contains('\u{1b}'), "{i}");
    }

    #[test]
    fn code_1312_only_with_1310_or_1311() {
        let mut m = message("meldung-abbm");
        with_codes(&mut m, &["1312"]);
        if let Meldungsinhalt::Abbm(a) = &mut m.inhalt {
            a.positionen[0].position.text = Meldungstext::new("Hinterhaus").unwrap();
        }
        only(&[], &m.issues(), "spri.code.1312", Severity::Error, None);
        if let Meldungsinhalt::Abbm(a) = &mut m.inhalt {
            a.positionen.insert(0, Position::new(Meldungscode::C1311).into());
        }
        assert!(m.issues().is_empty(), "{:#?}", m.issues());
    }

    #[test]
    fn free_text_codes_carry_a_text() {
        let mut m = message("meldung-abbm");
        with_codes(&mut m, &["8600"]);
        let i = only(&[], &m.issues(), "spri.code.freitext", Severity::Warning, None);
        assert!(i.path.ends_with("position[1]/meldungstext"));
        if let Meldungsinhalt::Abbm(a) = &mut m.inhalt {
            a.positionen[0].position.text = Meldungstext::new("Hausnummer fehlt").unwrap();
        }
        assert!(m.issues().is_empty());
        let mut m = message("meldung-abbm");
        with_codes(&mut m, &["8601"]);
        if let Meldungsinhalt::Abbm(a) = &mut m.inhalt {
            a.positionen[0].position.text = Meldungstext::empty();
        }
        only(&[], &m.issues(), "spri.code.freitext", Severity::Warning, None);
    }

    #[test]
    fn vzm_codes_with_an_appointment_name_it() {
        for code in ["1576", "1580", "1581"] {
            let mut m = message("meldung-vzm");
            with_codes(&mut m, &[code]);
            assert!(m.issues().is_empty(), "{code}");
            if let Meldungsinhalt::Vzm(v) = &mut m.inhalt {
                v.verzoegerungstermin = None;
            }
            only(&[], &m.issues(), "spri.vzm.verzoegerungstermin", Severity::Error, None);
        }
    }

    #[test]
    fn abbm_for_an_open_order_gives_its_date() {
        for code in ["1006", "1012"] {
            let mut m = message("meldung-abbm");
            with_codes(&mut m, &[code]);
            assert!(m.issues().is_empty(), "{code}");
            if let Meldungsinhalt::Abbm(a) = &mut m.inhalt {
                a.wiedervorlagetermin = None;
            }
            only(&[], &m.issues(), "spri.abbm.erledigungstermin", Severity::Warning, None);
        }
    }

    #[test]
    fn abbm_after_the_losing_provider_names_it() {
        let abm = message("meldung-abm");
        let Meldungsinhalt::Abm(a) = &abm.inhalt else { panic!() };
        for code in ["0041", "0042"] {
            let mut m = message("meldung-abbm");
            m.auftragstyp = abm.auftragstyp;
            with_codes(&mut m, &[code]);
            only(&[], &m.issues(), "spri.abbm.abgebender-provider", Severity::Error, None);
            if let Meldungsinhalt::Abbm(b) = &mut m.inhalt {
                b.abgebender_provider.clone_from(&a.abgebender_provider);
            }
            assert!(m.issues().is_empty(), "{code}: {:#?}", m.issues());
        }
    }

    #[test]
    fn ruem_pv_refusal_reasons() {
        let base = message("meldung-ruem-pv");
        let case = |zustimmung: JaNein, code: Option<&str>| {
            let mut m = base.clone();
            if let Meldungsinhalt::RuemPv(r) = &mut m.inhalt {
                r.abgebender_provider.zustimmung = zustimmung;
                r.abgebender_provider.antwortcode = code.map(|c| Antwortcode::new(c).unwrap());
            }
            m.issues()
        };
        assert!(case(JaNein::Ja, None).is_empty());
        assert!(case(JaNein::Nein, Some("KVE")).is_empty());
        assert!(case(JaNein::Nein, Some("LNB")).is_empty());
        only(&[], &case(JaNein::Nein, None), "spri.ruem-pv.antwortcode-missing", Severity::Error, None);
        only(&[], &case(JaNein::Nein, Some("XYZ")), "spri.ruem-pv.antwortcode-known", Severity::Error, None);
        only(&[], &case(JaNein::Ja, Some("tav")), "spri.ruem-pv.contradiction", Severity::Error, None);
    }

    #[test]
    fn message_aekz_and_art_follow_the_case() {
        let mut m = message("meldung-kdle");
        m.auftragstyp.aenderungskennzeichen = Aenderungskennzeichen::Storno;
        let i = only(&[], &m.issues(), "spri.aekz.storno", Severity::Error, None);
        assert_eq!(i.path, "meldung/auftragstyp/aenderungsKennzeichen");
        let mut m = message("meldung-kdle");
        m.auftragstyp.aenderungskennzeichen = Aenderungskennzeichen::Terminverschiebung;
        only(&[], &m.issues(), "spri.aekz.terminverschiebung", Severity::Error, None);
        let mut m = message("meldung-kdle");
        m.auftragstyp.geschaeftsfallart = GeschaeftsfallArt::Bereitstellung;
        only(&[], &m.issues(), "spri.geschaeftsfallart.match", Severity::Error, None);
    }

    // -- receipts -----------------------------------------------------------------------------

    #[test]
    fn receipts() {
        let ok = Teq::new(Meldungscode::Ok, None, None);
        assert!(ok.issues().is_empty());
        assert!(Teq::new(Meldungscode::C0995, None, None).issues().is_empty());
        let wrong = Teq::new(Meldungscode::C1001, None, None);
        let i = only(&[], &wrong.issues(), "spri.teq.code", Severity::Error, None);
        assert_eq!(i.path, "quittung/meldungspositionen/position[1]/meldungscode");
        let mut mixed = ok;
        mixed.positionen.push(Position::new(Meldungscode::C0995));
        only(&[], &mixed.issues(), "spri.teq.mixed", Severity::Error, None);
    }

    #[test]
    fn display_names_rule_path_source_and_code() {
        let mut a = order("auftrag-get");
        a.aenderungskennzeichen = Aenderungskennzeichen::Storno;
        assert_eq!(
            a.issues()[0].to_string(),
            "error [spri.aekz.storno] auftrag/aenderungskennzeichen: GET has no Storno (S/PRI 4.4 §6.2.1.2, §6.2.2.2; Anl. 7 EPK V.13 p.4) → 1007"
        );
    }
}
