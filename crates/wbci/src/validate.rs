//! Validation that predicts a peer's rejection.
//!
//! Values are valid by construction (their types check the schema's facets); these rules
//! cover what types cannot: repetitions, consistency between fields, codes against message
//! kinds and business cases, a message against the request it answers, and lead times.
//! Every [`Issue`] names the element path, the rule, its source and, where one exists, the
//! code a peer answers with. Issues never contain personal data.

use core::fmt;

use jiff::Timestamp;

use crate::deadline;
use crate::model::{
    AdresseAbweichend, Anfrage, Anschluss, Bezug, Meldung, Rufnummernportierung, StornoArt, Vorabstimmung,
    VorabstimmungsFall,
};
use crate::{Calendar, Geschaeftsfall, Meldungscode, Meldungstyp, Portierungszeitfenster, Role};

/// How serious an issue is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// The specifications advise against it; the peer will likely still accept.
    Warning,
    /// The peer or the certifier will reject it.
    Error,
}

/// One finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// Error or warning.
    pub severity: Severity,
    /// Element path, e.g. `Meldung/Position[1]/Meldungscode`.
    pub path: String,
    /// Stable rule identifier, e.g. `code.applicability`.
    pub rule: &'static str,
    /// What is wrong, in one sentence.
    pub message: String,
    /// Where the rule is written.
    pub source: &'static str,
    /// The code a peer answers with, where one applies.
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

const XSD: &str = "XSD V2.00";
const AH: &str = "AH4WBCI 2.10 §2, §5.3";

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

    fn warn(&mut self, path: impl Into<String>, rule: &'static str, message: impl Into<String>, source: &'static str) {
        self.add(Severity::Warning, path, rule, message, source, None);
    }

    fn count(&mut self, path: &str, element: &str, actual: usize, min: usize, max: usize) {
        if actual < min || actual > max {
            self.error(
                format!("{path}/{element}"),
                "schema.count",
                format!("{element} occurs {actual} times, {min}..={max} allowed"),
                XSD,
                Some(Meldungscode::DatenstrukturUngueltig),
            );
        }
    }
}

impl Anfrage {
    /// Checks the request on its own.
    #[must_use]
    pub fn issues(&self) -> Vec<Issue> {
        let mut r = Report::default();
        let ekp = self.ekp();
        if ekp.auf == ekp.abg {
            r.error(
                "Anfrage/Endkundenvertragspartner",
                "ekp.distinct",
                "EKPauf and EKPabg are the same carrier",
                "Anl. 1 V5.4.1 §2.1",
                None,
            );
        }
        match self {
            Self::Vorabstimmung(va) => vorabstimmung(&mut r, "Anfrage", va),
            Self::Storno(s) => {
                if s.storno_id.carrier() != *self.absender() {
                    r.error(
                        "Anfrage/StornoId",
                        "id.issuer",
                        "the StornoId must carry the sender's carrier code",
                        "Anl. 1 V5.4.1 §2.9.1",
                        None,
                    );
                }
                bezug(&mut r, &s.bezug, &s.ekp);
                if let StornoArt::AufhebungAbg { .. } = s.art {
                    {
                        r.warn("Anfrage/Stornogrund", "str-auf.ekpabg-grund", "an EKPabg may cancel only for the reasons of Anl. 1 §2.9.2 (extraordinary termination, death, cancellation or revocation of the EKPauf contract, RRNP not possible) and must hold proof", "Anl. 1 V5.4.1 §2.9.2");
                    }
                }
            }
            Self::Terminverschiebung(t) => {
                if t.aenderungs_id.carrier() != t.ekp.auf {
                    r.error(
                        "Anfrage/AenderungsId",
                        "id.issuer",
                        "the AenderungsId must carry the EKPauf's carrier code",
                        "Anl. 1 V5.4.1 §2.11",
                        None,
                    );
                }
                bezug(&mut r, &t.bezug, &t.ekp);
            }
        }
        r.0
    }

    /// Checks the request as sent at `sent_at`: [`Anfrage::issues`] plus lead times.
    #[must_use]
    pub fn issues_at(&self, sent_at: Timestamp, cal: &Calendar) -> Vec<Issue> {
        let mut r = Report(self.issues());
        if let Self::Vorabstimmung(va) = self {
            if let Some(earliest) = deadline::earliest_kundenwunschtermin(va.geschaeftsfall(), sent_at, cal)
                && va.kundenwunschtermin < earliest.date
            {
                r.warn(
                        "Anfrage/Kundenwunschtermin",
                        "va.lead",
                        format!("the requested date should be on or after {} ({} AT after sending); the EKPabg will answer with a later date (8002)", earliest.date, if va.geschaeftsfall() == Geschaeftsfall::VaRrnp { 7 } else { 20 }),
                        deadline::rules::VA_LEAD.source,
                    );
            }
            if let Some(why) = cal.closed(va.kundenwunschtermin) {
                r.warn(
                    "Anfrage/Kundenwunschtermin",
                    "termin.working-day",
                    format!("the requested date is not a working day ({why})"),
                    "Anl. 1 V5.4.1 §2.3",
                );
            }
        }
        if let Self::Terminverschiebung(t) = self
            && let Some(why) = cal.closed(t.neuer_kundenwunschtermin)
        {
            r.warn(
                "Anfrage/NeuerKundenwunschtermin",
                "termin.working-day",
                format!("the new date is not a working day ({why})"),
                "Anl. 1 V5.4.1 §2.11",
            );
        }
        r.0
    }
}

fn vorabstimmung(r: &mut Report, path: &str, va: &Vorabstimmung) {
    if va.id.carrier() != va.ekp.auf {
        r.error(
            format!("{path}/VorabstimmungsId"),
            "id.issuer",
            "the VorabstimmungsId must carry the EKPauf's carrier code",
            "Anl. 1 V5.4.1 §2.9.1",
            None,
        );
    }
    r.count(path, "WeitereAnschlussinhaber", va.weitere_anschlussinhaber.len(), 0, 99);
    match &va.fall {
        VorabstimmungsFall::KueMrn { rufnummernportierung, .. }
        | VorabstimmungsFall::Rrnp { rufnummernportierung, .. } => {
            portierung(r, &format!("{path}/RufnummernPortierung"), rufnummernportierung, va.geschaeftsfall());
        }
        VorabstimmungsFall::KueOrn { .. } => {}
    }
}

fn portierung(r: &mut Report, path: &str, p: &Rufnummernportierung, fall: Geschaeftsfall) {
    match &p.anschluss {
        Anschluss::Einzel { rufnummern, .. } => {
            r.count(
                &format!("{path}/einzelanschluss/rufnummernliste"),
                "zuPortierendeOnkzRnr",
                rufnummern.len(),
                1,
                10,
            );
            for (i, n) in rufnummern.iter().enumerate() {
                if rufnummern.iter().take(i).any(|m| m == n) {
                    r.warn(
                        format!("{path}/einzelanschluss/rufnummernliste/zuPortierendeOnkzRnr[{}]", i + 1),
                        "rufnummer.duplicate",
                        "the number is listed twice",
                        "Anl. 1 V5.4.1 §2.3",
                    );
                }
            }
        }
        Anschluss::Anlage { bloecke, .. } => {
            r.count(&format!("{path}/anlagenanschluss"), "zuPortierenderRufnummernblock", bloecke.len(), 1, 5);
            for (i, b) in bloecke.iter().enumerate() {
                let (von, bis) = (b.von.as_str(), b.bis.as_str());
                if von.len() != bis.len() || von > bis {
                    r.error(
                        format!("{path}/anlagenanschluss/zuPortierenderRufnummernblock[{}]", i + 1),
                        "block.range",
                        "a number block runs from a lower to a higher bound of the same length and is given in full",
                        "Anl. 1 V5.4.1 §2.3",
                        Some(Meldungscode::Rng),
                    );
                }
            }
        }
    }
    match p.zeitfenster {
        Portierungszeitfenster::Zf3 => r.warn(
            format!("{path}/Portierungszeitfenster"),
            "zeitfenster.zf3",
            "ZF3 may only be used if agreed with the other company beforehand",
            "Anl. 1 V5.4.1 §3.1",
        ),
        Portierungszeitfenster::Zf1 if fall == Geschaeftsfall::VaKueMrn => r.warn(
            format!("{path}/Portierungszeitfenster"),
            "zeitfenster.zf1",
            "ZF1 is for porting without line takeover; a switch with takeover uses ZF2",
            "Anl. 1 V5.4.1 §3.1",
        ),
        _ => {}
    }
}

fn bezug(r: &mut Report, b: &Bezug, ekp: &crate::Ekp) {
    if let Bezug::Originalanfrage(va) = b {
        if va.ekp != *ekp {
            r.error(
                "Anfrage/VorabstimmungsDaten/Originalanfrage",
                "bezug.ekp",
                "the original request names other parties than this request",
                "XSD V2.00 VorabstimmungsdatenType",
                Some(Meldungscode::VorabstimmungsIdUnbekannt),
            );
        }
        vorabstimmung(r, "Anfrage/VorabstimmungsDaten/Originalanfrage", va);
    }
}

impl Meldung {
    /// Checks the message, and against the request it answers if given.
    ///
    /// For AKM-TR and ABBM-TR `answering` is the pre-agreement request.
    #[must_use]
    pub fn issues(&self, answering: Option<&Anfrage>) -> Vec<Issue> {
        let mut r = Report::default();
        let k = self.kopf();
        let typ = self.meldungstyp();
        let sender = k.ekp.role_of(&k.absender);
        if sender.is_none() {
            r.error(
                "Meldung/Absender",
                "absender.party",
                "the Absender is neither EKPauf nor EKPabg",
                "XSD V2.00 AbstractMeldungType",
                None,
            );
        }
        let expected_sender = match typ {
            Meldungstyp::RuemVa | Meldungstyp::AbbmTr => Some(Role::EkpAbg),
            Meldungstyp::AkmTr => Some(Role::EkpAuf),
            _ => answering.map(|a| a.sender().other()),
        };
        if let (Some(expected), Some(actual)) = (expected_sender, sender)
            && expected != actual
        {
            r.error("Meldung/Absender", "absender.role", format!("a {typ} here is sent by the {expected}"), AH, None);
        }
        let (count, positions): (usize, Vec<(usize, &Meldungscode)>) = {
            let codes: Vec<&Meldungscode> = self.codes().collect();
            (codes.len(), codes.into_iter().enumerate().collect())
        };
        r.count("Meldung", "Position", count, 1, 99);
        for (i, code) in &positions {
            let path = format!("Meldung/Position[{}]/Meldungscode", i + 1);
            if code.text().is_none() {
                r.error(
                    &path,
                    "code.unknown",
                    format!("{} is not in the published code list", code.as_str()),
                    AH,
                    None,
                );
            } else if !code.applies(typ, k.geschaeftsfall) {
                r.error(
                    &path,
                    "code.applicability",
                    format!("{} may not answer {} in a {typ}", code.as_str(), k.geschaeftsfall),
                    AH,
                    None,
                );
            }
        }
        match self {
            Self::RuemVa(m) => ruem_va(&mut r, m),
            Self::Abbm(m) => abbm(&mut r, m),
            Self::Erlm(m) => erlm(&mut r, m),
            Self::AkmTr(m) => akm_tr(&mut r, m),
            Self::AbbmTr(_) => {}
        }
        if let Some(a) = answering {
            against(&mut r, self, a);
        }
        r.0
    }

    /// Checks the message as sent at `sent_at`: [`Meldung::issues`] plus the date rules of a
    /// RUEM-VA (Anl. 1 §2.5: at least 15 AT ahead for `VA-KUE-*`, not before the requested date).
    #[must_use]
    pub fn issues_at(&self, answering: Option<&Anfrage>, sent_at: Timestamp, cal: &Calendar) -> Vec<Issue> {
        let mut r = Report(self.issues(answering));
        if let (Self::RuemVa(m), Some(Anfrage::Vorabstimmung(va))) = (self, answering) {
            if va.geschaeftsfall().is_kuendigung() {
                let earliest = deadline::earliest_wechseltermin(sent_at, va.kundenwunschtermin, cal);
                if m.wechseltermin < earliest.date {
                    r.error(
                        "Meldung/Wechseltermin",
                        "ruem-va.date",
                        format!("the switch date must be on or after {}", earliest.date),
                        deadline::rules::WECHSELTERMIN.source,
                        None,
                    );
                }
            }
            if let Some(why) = cal.closed(m.wechseltermin) {
                r.warn(
                    "Meldung/Wechseltermin",
                    "termin.working-day",
                    format!("the switch date is not a working day ({why})"),
                    "Anl. 1 V5.4.1 §2.3",
                );
            }
        }
        r.0
    }
}

impl crate::Teq {
    /// Checks a receipt: 1–99 positions, only receipt codes (`OK`, 0993, 0995, 0999), and
    /// `OK` never together with an error code.
    #[must_use]
    pub fn issues(&self) -> Vec<Issue> {
        let mut r = Report::default();
        r.count("Quittung", "Position", self.positionen.len(), 1, 99);
        for (i, p) in self.positionen.iter().enumerate() {
            if !p.code.meldungstypen().any(|t| t == Meldungstyp::Teq) {
                r.error(
                    format!("Quittung/Position[{}]/Meldungscode", i + 1),
                    "code.applicability",
                    format!("{} is not a receipt code (OK, 0993, 0995, 0999)", p.code.as_str()),
                    AH,
                    None,
                );
            }
        }
        if self.positionen.iter().any(|p| p.code.is_ok()) && self.positionen.iter().any(|p| !p.code.is_ok()) {
            r.error(
                "Quittung/Position",
                "teq.mixed",
                "a receipt is either OK or an error, not both",
                "WBCI 2.0 §3.2",
                None,
            );
        }
        r.0
    }
}

fn refs(r: &mut Report, fall: Geschaeftsfall, storno: bool, aenderung: bool) {
    match fall {
        Geschaeftsfall::StrAuf | Geschaeftsfall::StrAen if !storno => {
            r.error(
                "Meldung/StornoIdRef",
                "meldung.ref",
                "an answer to a cancellation refers to its StornoId",
                AH,
                None,
            );
        }
        Geschaeftsfall::TvsVa if !aenderung => {
            r.error(
                "Meldung/AenderungsIdRef",
                "meldung.ref",
                "an answer to a postponement refers to its AenderungsId",
                AH,
                None,
            );
        }
        _ => {}
    }
}

fn against(r: &mut Report, m: &Meldung, a: &Anfrage) {
    let k = m.kopf();
    if k.vorabstimmungs_id != *a.vorabstimmungs_id() {
        r.error(
            "Meldung/VorabstimmungsIdRef",
            "meldung.answers",
            "refers to another Vorabstimmungs-ID than the request",
            AH,
            Some(Meldungscode::VorabstimmungsIdUnbekannt),
        );
    }
    if k.ekp != *a.ekp() {
        r.error(
            "Meldung/Endkundenvertragspartner",
            "meldung.answers",
            "names other parties than the request",
            AH,
            None,
        );
    }
    let answers_request = matches!(m.meldungstyp(), Meldungstyp::RuemVa | Meldungstyp::Abbm | Meldungstyp::Erlm);
    if answers_request && k.geschaeftsfall != a.geschaeftsfall() {
        r.error(
            "Meldung/Geschaeftsfall",
            "meldung.answers",
            format!("answers {} but the request is {}", k.geschaeftsfall, a.geschaeftsfall()),
            AH,
            None,
        );
    }
    if let Anfrage::Storno(s) = a
        && m.storno_id().is_some_and(|id| *id != s.storno_id)
    {
        r.error("Meldung/StornoIdRef", "meldung.answers", "refers to another StornoId", AH, None);
    }
    if let Anfrage::Terminverschiebung(t) = a {
        if m.aenderungs_id().is_some_and(|id| *id != t.aenderungs_id) {
            r.error("Meldung/AenderungsIdRef", "meldung.answers", "refers to another AenderungsId", AH, None);
        }
        if let (Meldung::Erlm(_), Some(termin)) = (m, m.wechseltermin())
            && termin != t.neuer_kundenwunschtermin
        {
            r.error(
                "Meldung/Wechseltermin",
                "erlm.tvs-termin",
                "an accepted postponement confirms the requested new date; agreeing to another date is not allowed",
                "Anl. 1 V5.4.1 §2.11",
                None,
            );
        }
    }
    if let (Meldung::RuemVa(ruem), Anfrage::Vorabstimmung(va)) = (m, a) {
        let requested = va.kundenwunschtermin;
        if ruem.wechseltermin < requested {
            r.error(
                "Meldung/Wechseltermin",
                "ruem-va.date",
                "the switch date is before the requested date",
                "Anl. 1 V5.4.1 §2.5",
                None,
            );
        }
        let zwa = ruem.positionen.iter().any(|p| p.position.code == Meldungscode::Zwa);
        let nat = ruem.positionen.iter().any(|p| p.position.code == Meldungscode::Nat);
        if zwa && ruem.wechseltermin != requested {
            r.error(
                "Meldung/Position",
                "ruem-va.zwa",
                "8001 (ZWA) confirms the requested date; a different date is 8002 (NAT)",
                "Anl. 1 V5.4.1 §2.3",
                None,
            );
        }
        if nat && ruem.wechseltermin == requested {
            r.warn(
                "Meldung/Position",
                "ruem-va.nat",
                "8002 (NAT) announces a later date; the requested date is 8001 (ZWA)",
                "Anl. 1 V5.4.1 §2.3",
            );
        }
    }
}

fn ruem_va(r: &mut Report, m: &crate::RuemVa) {
    let zustimmung =
        m.positionen.iter().filter(|p| matches!(p.position.code, Meldungscode::Zwa | Meldungscode::Nat)).count();
    if zustimmung != 1 {
        r.error(
            "Meldung/Position",
            "ruem-va.zustimmung",
            "a RUEM-VA carries exactly one 8001 (ZWA) or 8002 (NAT)",
            "Anl. 1 V5.4.1 §2.3",
            None,
        );
    }
    for (i, p) in m.positionen.iter().enumerate() {
        let path = format!("Meldung/Position[{}]", i + 1);
        let fits = match (&p.position.code, &p.adresse_abweichend) {
            (Meldungscode::AdaPlz, Some(AdresseAbweichend::Postleitzahl(_)))
            | (Meldungscode::AdaOrt, Some(AdresseAbweichend::Ort(_)))
            | (Meldungscode::AdaStr, Some(AdresseAbweichend::Strassenname(_)))
            | (Meldungscode::AdaHsnr, Some(AdresseAbweichend::Hausnummer(_))) => true,
            (Meldungscode::AdaPlz | Meldungscode::AdaOrt | Meldungscode::AdaStr | Meldungscode::AdaHsnr, _)
            | (_, Some(_)) => false,
            _ => true,
        };
        if !fits {
            r.error(&path, "ruem-va.ada", "an ADA code (8104–8107) carries exactly the matching corrected address part, and only ADA codes carry one", "Anl. 1 V5.4.1 §2.3", None);
        }
    }
    r.count("Meldung", "Ressource", m.ressourcen.len(), 0, 99);
}

fn abbm(r: &mut Report, m: &crate::Abbm) {
    let k = &m.kopf;
    for (i, p) in m.positionen.iter().enumerate() {
        if !p.rufnummern.is_empty() && p.position.code != Meldungscode::Rng {
            r.error(
                format!("Meldung/Position[{}]/Rufnummer", i + 1),
                "abbm.rufnummern",
                "numbers are listed only with 8103 (RNG)",
                "XSD V2.00 MeldungsPositionABBMType",
                None,
            );
        }
        r.count(&format!("Meldung/Position[{}]", i + 1), "Rufnummer", p.rufnummern.len(), 0, 10);
    }
    if m.positionen.iter().any(|p| p.position.code == Meldungscode::Sonstiges) && m.begruendung.is_none() {
        r.error(
            "Meldung/Begruendung",
            "abbm.son-begruendung",
            "8116 (SON) always comes with a reason in Begruendung",
            "Anl. 1 V5.4.1 §2.3",
            None,
        );
    }
    refs(r, k.geschaeftsfall, m.storno_id.is_some(), m.aenderungs_id.is_some());
    if k.geschaeftsfall == Geschaeftsfall::TvsVa && m.wechseltermin.is_none() {
        r.error(
            "Meldung/Wechseltermin",
            "abbm.tvs-termin",
            "a refused postponement returns the date that stays valid",
            "Anl. 1 V5.4.1 §2.11",
            None,
        );
    }
}

fn erlm(r: &mut Report, m: &crate::Erlm) {
    let k = &m.kopf;
    refs(r, k.geschaeftsfall, m.storno_id.is_some(), m.aenderungs_id.is_some());
    if k.geschaeftsfall == Geschaeftsfall::TvsVa && m.wechseltermin.is_none() {
        r.error(
            "Meldung/Wechseltermin",
            "erlm.tvs-termin",
            "an accepted postponement names the new switch date",
            "Anl. 1 V5.4.1 §2.11",
            None,
        );
    }
}

fn akm_tr(r: &mut Report, m: &crate::AkmTr) {
    if !m.ressourcenuebernahme && !m.uebernahme_leitung.is_empty() {
        r.error(
            "Meldung/UebernahmeLeitung",
            "akm-tr.leitung",
            "lines are named only when the resource is taken over",
            AH,
            None,
        );
    }
    if m.ressourcenuebernahme && m.uebernahme_leitung.is_empty() {
        r.warn(
            "Meldung/UebernahmeLeitung",
            "akm-tr.leitung",
            "a takeover names the line (LineID for S/PRI, Vertragsnummer for WITA)",
            AH,
        );
    }
    r.count("Meldung", "UebernahmeLeitung", m.uebernahme_leitung.len(), 0, 99);
}
