//! Answers built from the request or message they answer, with every reference copied.

use jiff::civil::Date;

use crate::model::{
    Abbm, AbbmTr, AkmTr, Anfrage, Erlm, Meldung, MeldungKopf, Position, PositionAbbm, PositionRuemVa, RuemVa, Storno,
    Teq, Terminverschiebung, UebernahmeLeitung, Vorabstimmung,
};
use crate::{CarrierCode, Freitext, Meldungscode, Meldungstext};

fn kopf(anfrage: &Anfrage) -> MeldungKopf {
    MeldungKopf {
        absender: anfrage.ekp().of(anfrage.sender().other()).clone(),
        vorabstimmungs_id: anfrage.vorabstimmungs_id().clone(),
        ekp: anfrage.ekp().clone(),
        geschaeftsfall: anfrage.geschaeftsfall(),
    }
}

impl RuemVa {
    /// The EKPabg agrees to `va` with a binding `wechseltermin`: code 8001 (ZWA) if it is the
    /// requested date, 8002 (NAT) otherwise (Anl. 1 V5.4.1 §2.3).
    #[must_use]
    pub fn zustimmung(va: &Vorabstimmung, wechseltermin: Date) -> Self {
        let code = if wechseltermin == va.kundenwunschtermin { Meldungscode::Zwa } else { Meldungscode::Nat };
        Self {
            kopf: kopf(&Anfrage::Vorabstimmung(va.clone())),
            positionen: vec![Position::new(code).into()],
            ressourcen: Vec::new(),
            technologie: None,
            wechseltermin,
            rufnummernportierung: None,
        }
    }

    /// Adds a position, e.g. an ADA hint with the corrected address part.
    #[must_use]
    pub fn with_position(mut self, position: impl Into<PositionRuemVa>) -> Self {
        self.positionen.push(position.into());
        self
    }
}

impl Abbm {
    /// Refuses `anfrage` with `code`; the sender is the side the request went to, and the
    /// Storno-/Änderungs-ID is referred to. For a postponement, set
    /// [`Abbm::wechseltermin`] to the date that stays valid.
    #[must_use]
    pub fn ablehnung(anfrage: &Anfrage, code: Meldungscode) -> Self {
        let (storno_id, aenderungs_id) = match anfrage {
            Anfrage::Storno(s) => (Some(s.storno_id.clone()), None),
            Anfrage::Terminverschiebung(t) => (None, Some(t.aenderungs_id.clone())),
            Anfrage::Vorabstimmung(_) => (None, None),
        };
        Self {
            kopf: kopf(anfrage),
            positionen: vec![Position::new(code).into()],
            wechseltermin: None,
            storno_id,
            aenderungs_id,
            begruendung: None,
        }
    }

    /// Adds a position.
    #[must_use]
    pub fn with_position(mut self, position: impl Into<PositionAbbm>) -> Self {
        self.positionen.push(position.into());
        self
    }

    /// Sets the reason (required with 8116).
    #[must_use]
    pub fn with_begruendung(mut self, begruendung: Freitext) -> Self {
        self.begruendung = Some(begruendung);
        self
    }
}

impl Erlm {
    /// Confirms a cancellation: 8004 (Storno ok).
    #[must_use]
    pub fn storno_ok(storno: &Storno) -> Self {
        Self {
            kopf: kopf(&Anfrage::Storno(storno.clone())),
            positionen: vec![Position::new(Meldungscode::StornoOk)],
            wechseltermin: None,
            storno_id: Some(storno.storno_id.clone()),
            aenderungs_id: None,
        }
    }

    /// Confirms a postponement to the requested date: 8003 (Terminverschiebung ok).
    #[must_use]
    pub fn terminverschiebung_ok(tvs: &Terminverschiebung) -> Self {
        Self {
            kopf: kopf(&Anfrage::Terminverschiebung(tvs.clone())),
            positionen: vec![Position::new(Meldungscode::TerminverschiebungOk)],
            wechseltermin: Some(tvs.neuer_kundenwunschtermin),
            storno_id: None,
            aenderungs_id: Some(tvs.aenderungs_id.clone()),
        }
    }
}

impl AkmTr {
    /// The EKPauf announces, after `ruem_va`, whether it takes over `leitungen` (none: no
    /// takeover) and whether it asks for the „sicherer Hafen": code 8010.
    #[must_use]
    pub fn uebernahme(ruem_va: &RuemVa, leitungen: Vec<UebernahmeLeitung>, sicherer_hafen: bool) -> Self {
        let mut kopf = ruem_va.kopf.clone();
        kopf.absender = kopf.ekp.auf.clone();
        Self {
            kopf,
            positionen: vec![Position::new(Meldungscode::AntwortAufnehmend)],
            ressourcenuebernahme: !leitungen.is_empty(),
            uebernahme_leitung: leitungen,
            sicherer_hafen,
            pki_auf: None,
        }
    }
}

impl AbbmTr {
    /// The EKPabg refuses `akm_tr` with `code` (8140, 8143–8148).
    #[must_use]
    pub fn ablehnung(akm_tr: &AkmTr, code: Meldungscode) -> Self {
        let mut kopf = akm_tr.kopf.clone();
        kopf.absender = kopf.ekp.abg.clone();
        Self { kopf, positionen: vec![Position::new(code)] }
    }
}

impl Teq {
    /// The receipt for `anfrage`, sent by the side it went to.
    #[must_use]
    pub fn for_anfrage(anfrage: &Anfrage, code: Meldungscode) -> Self {
        let (storno_id, aenderungs_id) = match anfrage {
            Anfrage::Storno(s) => (Some(s.storno_id.clone()), None),
            Anfrage::Terminverschiebung(t) => (None, Some(t.aenderungs_id.clone())),
            Anfrage::Vorabstimmung(_) => (None, None),
        };
        Self {
            absender: anfrage.ekp().of(anfrage.sender().other()).clone(),
            vorabstimmungs_id: anfrage.vorabstimmungs_id().clone(),
            positionen: vec![Position::new(code)],
            storno_id,
            aenderungs_id,
        }
    }

    /// The receipt for `meldung`, sent by the side it went to.
    #[must_use]
    pub fn for_meldung(meldung: &Meldung, code: Meldungscode) -> Self {
        let k = meldung.kopf();
        let receiver = k.ekp.role_of(&k.absender).map_or_else(|| k.ekp.auf.clone(), |r| k.ekp.of(r.other()).clone());
        Self {
            absender: receiver,
            vorabstimmungs_id: k.vorabstimmungs_id.clone(),
            positionen: vec![Position::new(code)],
            storno_id: meldung.storno_id().cloned(),
            aenderungs_id: meldung.aenderungs_id().cloned(),
        }
    }

    /// A receipt from `absender` for a call that could not be read, e.g. with
    /// [`ReadError::teq_code`](crate::xml::ReadError::teq_code).
    #[must_use]
    pub fn fehler(
        absender: CarrierCode,
        vorabstimmungs_id: crate::VorabstimmungsId,
        code: Meldungscode,
        text: Option<Meldungstext>,
    ) -> Self {
        let position = match text {
            Some(text) => Position::with_text(code, text),
            None => Position::new(code),
        };
        Self { absender, vorabstimmungs_id, positionen: vec![position], storno_id: None, aenderungs_id: None }
    }
}
