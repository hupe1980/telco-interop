//! What a payload is: kinds, ids, dates and codes — never a name, address, phone number or
//! free text.

use telco_interop::{spri, wbci};

use crate::input::{Parsed, spri_control};

/// The lines of a summary: label and value.
pub(crate) type Lines = Vec<(&'static str, String)>;

pub(crate) fn summary(p: &Parsed) -> Lines {
    match p {
        Parsed::Wbci(p) => wbci_summary(p),
        Parsed::Spri(p) => spri_summary(p),
    }
}

// -- WBCI -----------------------------------------------------------------------------------

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

// -- S/PRI ----------------------------------------------------------------------------------

fn spri_summary(p: &spri::Payload) -> Lines {
    let mut l: Lines = vec![("interface", "S/PRI".to_owned())];
    let sent = spri_control(p).zeitstempel.to_string();
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
            l.push(("externe Auftragsnummer", meldung_ext(meldung).to_string()));
            if let Some(id) = meldung_va(meldung) {
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
pub(crate) const fn wunschtermin(g: &spri::Geschaeftsfall) -> Option<spri::Date> {
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

/// The order number a message refers to.
pub(crate) const fn meldung_ext(m: &spri::Meldung) -> &spri::ExterneAuftragsnummer {
    use spri::Meldungsinhalt as I;
    match &m.inhalt {
        I::Qeb(x) => &x.externe_auftragsnummer,
        I::Abm(x) => &x.externe_auftragsnummer,
        I::Abbm(x) => &x.externe_auftragsnummer,
        I::Erlm(x) => &x.externe_auftragsnummer,
        I::Entm(x) => &x.externe_auftragsnummer,
        I::Vzm(x) => &x.externe_auftragsnummer,
        I::Tam(x) | I::Mtam(x) => &x.externe_auftragsnummer,
        I::Kdle(x) => &x.externe_auftragsnummer,
        I::ZwmLe(x) => &x.externe_auftragsnummer,
        I::ZwmAg(x) => &x.externe_auftragsnummer,
        I::TbkLe(x) | I::TbkAg(x) => &x.externe_auftragsnummer,
        I::AkmPv(x) => &x.externe_auftragsnummer,
        I::RuemPv(x) => &x.externe_auftragsnummer,
        I::AbmPv(x) => &x.externe_auftragsnummer,
        I::ErlmPv(x) => &x.externe_auftragsnummer,
        I::EntmPv(x) => &x.externe_auftragsnummer,
        I::AbbmPv(x) => &x.externe_auftragsnummer,
        I::ErlmK(x) => &x.externe_auftragsnummer,
    }
}

/// The Vorabstimmungs-ID a message carries, if its type has one.
pub(crate) const fn meldung_va(m: &spri::Meldung) -> Option<&spri::VorabstimmungsId> {
    use spri::Meldungsinhalt as I;
    match &m.inhalt {
        I::Qeb(x) => x.vorabstimmungs_id.as_ref(),
        I::Abm(x) => x.vorabstimmungs_id.as_ref(),
        I::Abbm(x) => x.vorabstimmungs_id.as_ref(),
        I::Erlm(x) => x.vorabstimmungs_id.as_ref(),
        I::Entm(x) => x.vorabstimmungs_id.as_ref(),
        I::Vzm(x) => x.vorabstimmungs_id.as_ref(),
        I::Tam(x) | I::Mtam(x) => x.vorabstimmungs_id.as_ref(),
        I::ZwmLe(x) => x.vorabstimmungs_id.as_ref(),
        I::AkmPv(x) => x.vorabstimmungs_id.as_ref(),
        I::RuemPv(x) => x.vorabstimmungs_id.as_ref(),
        I::ErlmK(x) => x.vorabstimmungs_id.as_ref(),
        I::Kdle(_)
        | I::ZwmAg(_)
        | I::TbkLe(_)
        | I::TbkAg(_)
        | I::AbmPv(_)
        | I::ErlmPv(_)
        | I::EntmPv(_)
        | I::AbbmPv(_) => None,
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
