//! Writing payloads in the schema's element order.

use core::ops::{Deref, DerefMut};

use carrier_core::xml::{self, WriteError};

use super::{NS_ENVELOPE, NS_MESSAGE, NS_REQUEST, NS_SOAP, NS_XSI, max, types};
use crate::model::{
    Abbm, AbbmTr, AdresseAbweichend, AkmTr, Anfrage, Anlagenkopf, Anschluss, Bezug, Control, Ekp, Erlm, Kunde, Meldung,
    MeldungKopf, OnkzRufnummer, Payload, PortierteRufnummern, Position, ProjektId, RuemVa, Rufnummernblock,
    Rufnummernportierung, Standort, Storno, StornoArt, TechnischeRessource, Teq, Terminverschiebung, UebernahmeLeitung,
    Vorabstimmung, VorabstimmungsFall,
};
use crate::{CarrierCode, Date};

impl Payload {
    /// Writes the payload element alone (`annehmenAnfrageRequest` etc.), without an XML
    /// declaration, ready to go into a SOAP body or a file.
    ///
    /// # Errors
    ///
    /// A repetition outside the schema's bounds (e.g. 100 positions, no position at all).
    pub fn to_xml(&self) -> Result<String, WriteError> {
        let mut w = Writer::new();
        w.payload(self)?;
        Ok(w.0.out)
    }

    /// Writes the payload inside a SOAP 1.1 `Envelope`/`Body`, with an empty `Header` for the
    /// application's WS-Security header, and an XML declaration.
    ///
    /// # Errors
    ///
    /// As [`Payload::to_xml`].
    pub fn to_soap(&self) -> Result<String, WriteError> {
        let mut w = Writer::new();
        w.out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        w.out.push_str("<soapenv:Envelope xmlns:soapenv=\"");
        w.out.push_str(NS_SOAP);
        w.out.push_str("\">\n  <soapenv:Header/>\n  <soapenv:Body>\n");
        w.depth = 2;
        w.payload(self)?;
        w.out.push_str("  </soapenv:Body>\n</soapenv:Envelope>\n");
        Ok(w.0.out)
    }
}

/// The WBCI writer: the shared indenting writer plus the WBCI element structure.
struct Writer(xml::Writer);

impl Deref for Writer {
    type Target = xml::Writer;
    fn deref(&self) -> &xml::Writer {
        &self.0
    }
}

impl DerefMut for Writer {
    fn deref_mut(&mut self) -> &mut xml::Writer {
        &mut self.0
    }
}

impl Writer {
    const fn new() -> Self {
        Self(xml::Writer::new())
    }

    fn payload(&mut self, p: &Payload) -> Result<(), WriteError> {
        let (root, prefix, ns) = match p {
            Payload::AnfrageRequest { .. } => ("env:annehmenAnfrageRequest", "ar", NS_REQUEST),
            Payload::AnfrageResponse { .. } => ("env:annehmenAnfrageResponse", "am", NS_MESSAGE),
            Payload::MeldungRequest { .. } => ("env:annehmenMeldungRequest", "am", NS_MESSAGE),
            Payload::MeldungResponse { .. } => ("env:annehmenMeldungResponse", "am", NS_MESSAGE),
        };
        let xmlns_prefix = format!("xmlns:{prefix}");
        self.open(root, &[("xmlns:env", NS_ENVELOPE), (&xmlns_prefix, ns), ("xmlns:xsi", NS_XSI)]);
        self.control(p.control());
        match p {
            Payload::AnfrageRequest { anfrage, .. } => self.anfrage(anfrage)?,
            Payload::MeldungRequest { meldung, .. } => self.meldung(meldung)?,
            Payload::AnfrageResponse { quittung, .. } | Payload::MeldungResponse { quittung, .. } => {
                self.teq(quittung)?;
            }
        }
        self.close(root);
        Ok(())
    }

    fn control(&mut self, c: &Control) {
        self.open("Control", &[]);
        self.leaf("zeitstempel", &c.zeitstempel.to_string());
        self.leaf("majorRelease", &format!("{:02}", c.release.major));
        self.leaf("minorRelease", &format!("{:02}", c.release.minor));
        self.open("signaturId", &[]);
        self.leaf("issuer", c.signatur.issuer.as_str());
        self.leaf("serial", c.signatur.serial.as_str());
        self.close("signaturId");
        self.close("Control");
    }

    fn carrier(&mut self, name: &'static str, c: &CarrierCode) {
        self.open(name, &[]);
        self.leaf("CarrierCode", c.as_str());
        self.close(name);
    }

    fn ekp(&mut self, ekp: &Ekp) {
        self.open("Endkundenvertragspartner", &[]);
        self.carrier("EKPauf", &ekp.auf);
        self.carrier("EKPabg", &ekp.abg);
        self.close("Endkundenvertragspartner");
    }

    fn kunde(&mut self, name: &'static str, k: &Kunde) {
        self.open(name, &[]);
        match k {
            Kunde::Person(p) => {
                self.open("person", &[]);
                self.leaf("anrede", p.anrede.as_str());
                self.opt("vorname", p.vorname.as_ref().map(AsRef::as_ref));
                self.leaf("nachname", p.nachname.as_str());
                self.close("person");
            }
            Kunde::Firma(f) => {
                self.open("firma", &[]);
                self.leaf("anrede", f.anrede.as_str());
                self.leaf("firmenname", f.name.as_str());
                self.opt("firmennameZweiterTeil", f.name_zweiter_teil.as_ref().map(AsRef::as_ref));
                self.close("firma");
            }
        }
        self.close(name);
    }

    fn standort(&mut self, s: &Standort) {
        self.open("Standort", &[]);
        self.open("strasse", &[]);
        self.leaf("strassenname", s.strassenname.as_str());
        self.leaf("hausnummer", s.hausnummer.as_str());
        self.opt("hausnummernZusatz", s.hausnummern_zusatz.as_ref().map(AsRef::as_ref));
        self.close("strasse");
        self.leaf("postleitzahl", s.postleitzahl.as_str());
        self.leaf("ort", s.ort.as_str());
        self.close("Standort");
    }

    fn onkz_rufnummer(&mut self, name: &'static str, n: &OnkzRufnummer) {
        self.open(name, &[]);
        self.leaf("ONKZ", n.onkz.as_str());
        self.leaf("rufnummer", n.rufnummer.as_str());
        self.close(name);
    }

    fn anlagenkopf(&mut self, k: &Anlagenkopf) {
        self.open("onkzDurchwahlAbfragestelle", &[]);
        self.leaf("ONKZ", k.onkz.as_str());
        self.leaf("durchwahlnummer", k.durchwahl.as_str());
        self.leaf("abfragestelle", k.abfragestelle.as_str());
        self.close("onkzDurchwahlAbfragestelle");
    }

    fn block_inner(&mut self, b: &Rufnummernblock) {
        self.leaf("rnrBlockVon", b.von.as_str());
        self.leaf("rnrBlockBis", b.bis.as_str());
    }

    fn rufnummernportierung(&mut self, r: &Rufnummernportierung, pki_auf: Option<&str>) -> Result<(), WriteError> {
        self.open("RufnummernPortierung", &[]);
        match &r.anschluss {
            Anschluss::Anlage { kopf, bloecke } => {
                self.open("anlagenanschluss", &[]);
                self.count("zuPortierenderRufnummernblock", bloecke.len(), 1, max::BLOCKS)?;
                self.anlagenkopf(kopf);
                for b in bloecke {
                    self.open("zuPortierenderRufnummernblock", &[]);
                    self.block_inner(b);
                    self.close("zuPortierenderRufnummernblock");
                }
                self.close("anlagenanschluss");
            }
            Anschluss::Einzel { rufnummern, alle_rufnummern } => {
                self.open("einzelanschluss", &[]);
                self.open("rufnummernliste", &[]);
                self.count("zuPortierendeOnkzRnr", rufnummern.len(), 1, max::NUMBERS)?;
                for n in rufnummern {
                    self.onkz_rufnummer("zuPortierendeOnkzRnr", n);
                }
                self.close("rufnummernliste");
                if *alle_rufnummern {
                    self.boolean("alleRufnummern", true);
                }
                self.close("einzelanschluss");
            }
        }
        self.leaf("Portierungszeitfenster", r.zeitfenster.as_str());
        self.opt("PortierungskennungPKIauf", pki_auf);
        self.close("RufnummernPortierung");
        Ok(())
    }

    fn projekt(&mut self, p: &ProjektId) {
        self.open("ProjektID", &[]);
        self.leaf("projektkenner", p.projektkenner.as_str());
        self.opt("kopplungskenner", p.kopplungskenner.as_ref().map(AsRef::as_ref));
        self.close("ProjektID");
    }

    fn anfrage(&mut self, a: &Anfrage) -> Result<(), WriteError> {
        match a {
            Anfrage::Vorabstimmung(va) => self.vorabstimmung("Anfrage", va),
            Anfrage::Storno(s) => self.storno(s),
            Anfrage::Terminverschiebung(t) => self.tvs(t),
        }
    }

    fn anfrage_head(&mut self, ekp: &Ekp, absender: &CarrierCode) {
        self.carrier("Absender", absender);
        self.ekp(ekp);
    }

    fn vorabstimmung(&mut self, name: &'static str, va: &Vorabstimmung) -> Result<(), WriteError> {
        let ty = match va.fall {
            VorabstimmungsFall::KueMrn { .. } => types::KUE_MRN,
            VorabstimmungsFall::KueOrn { .. } => types::KUE_ORN,
            VorabstimmungsFall::Rrnp { .. } => types::RRNP,
        };
        self.typed(name, "ar", ty);
        self.anfrage_head(&va.ekp, &va.ekp.auf);
        self.leaf("VorabstimmungsId", va.id.as_str());
        self.date("Kundenwunschtermin", va.kundenwunschtermin);
        self.kunde("Endkunde", &va.endkunde);
        self.count("WeitereAnschlussinhaber", va.weitere_anschlussinhaber.len(), 0, max::MANY)?;
        for k in &va.weitere_anschlussinhaber {
            self.kunde("WeitereAnschlussinhaber", k);
        }
        if let Some(p) = &va.projekt {
            self.projekt(p);
        }
        match &va.fall {
            VorabstimmungsFall::KueMrn { standort, rufnummernportierung } => {
                self.standort(standort);
                self.rufnummernportierung(rufnummernportierung, None)?;
            }
            VorabstimmungsFall::KueOrn { standort, anschlussidentifikation } => {
                self.standort(standort);
                if let Some(n) = anschlussidentifikation {
                    self.onkz_rufnummer("Anschlussidentifikation", n);
                }
            }
            VorabstimmungsFall::Rrnp { rufnummernportierung, pki_auf } => {
                self.rufnummernportierung(rufnummernportierung, Some(pki_auf.as_str()))?;
            }
        }
        self.close(name);
        Ok(())
    }

    fn bezug(&mut self, b: &Bezug) -> Result<(), WriteError> {
        self.open("VorabstimmungsDaten", &[]);
        match b {
            Bezug::Id(id) => self.leaf("VorabstimmungsIdRef", id.as_str()),
            Bezug::Originalanfrage(va) => self.vorabstimmung("Originalanfrage", va)?,
        }
        self.close("VorabstimmungsDaten");
        Ok(())
    }

    fn name_standort(&mut self, name: Option<&Kunde>, standort: Option<&Standort>) {
        if let Some(k) = name {
            self.kunde("Name", k);
        }
        if let Some(s) = standort {
            self.standort(s);
        }
    }

    fn storno(&mut self, s: &Storno) -> Result<(), WriteError> {
        let ty = match s.art {
            StornoArt::AufhebungAuf => types::STR_AUF_AUF,
            StornoArt::AufhebungAbg { .. } => types::STR_AUF_ABG,
            StornoArt::AenderungAuf { .. } => types::STR_AEN_AUF,
            StornoArt::AenderungAbg { .. } => types::STR_AEN_ABG,
        };
        self.typed("Anfrage", "ar", ty);
        self.anfrage_head(&s.ekp, s.ekp.of(s.art.sender()));
        self.leaf("StornoId", s.storno_id.as_str());
        self.bezug(&s.bezug)?;
        match &s.art {
            StornoArt::AufhebungAuf => {}
            StornoArt::AenderungAuf { name, standort } => self.name_standort(name.as_ref(), standort.as_ref()),
            StornoArt::AufhebungAbg { name, standort, stornogrund }
            | StornoArt::AenderungAbg { name, standort, stornogrund } => {
                self.name_standort(name.as_ref(), standort.as_ref());
                self.leaf("Stornogrund", stornogrund.as_str());
            }
        }
        self.close("Anfrage");
        Ok(())
    }

    fn tvs(&mut self, t: &Terminverschiebung) -> Result<(), WriteError> {
        self.typed("Anfrage", "ar", types::TVS);
        self.anfrage_head(&t.ekp, &t.ekp.auf);
        self.leaf("AenderungsId", t.aenderungs_id.as_str());
        self.bezug(&t.bezug)?;
        if let Some(k) = &t.name {
            self.kunde("Name", k);
        }
        self.date("NeuerKundenwunschtermin", t.neuer_kundenwunschtermin);
        self.close("Anfrage");
        Ok(())
    }

    fn position_inner(&mut self, p: &Position) {
        self.leaf("Meldungscode", p.code.as_str());
        self.leaf("Meldungstext", p.text.as_str());
    }

    fn positions<'a>(
        &mut self,
        ty: &str,
        positions: impl ExactSizeIterator<Item = &'a Position>,
    ) -> Result<(), WriteError> {
        self.count("Position", positions.len(), 1, max::MANY)?;
        for p in positions {
            self.typed("Position", "am", ty);
            self.position_inner(p);
            self.close("Position");
        }
        Ok(())
    }

    fn meldung_head(&mut self, ty: &str, k: &MeldungKopf) {
        self.typed("Meldung", "am", ty);
        self.carrier("Absender", &k.absender);
        self.leaf("VorabstimmungsIdRef", k.vorabstimmungs_id.as_str());
    }

    fn meldung_tail(&mut self, k: &MeldungKopf) {
        self.ekp(&k.ekp);
        self.leaf("Geschaeftsfall", k.geschaeftsfall.as_str());
    }

    fn refs(&mut self, termin: Option<Date>, storno: Option<&str>, aenderung: Option<&str>) {
        if let Some(d) = termin {
            self.date("Wechseltermin", d);
        }
        self.opt("StornoIdRef", storno);
        self.opt("AenderungsIdRef", aenderung);
    }

    fn meldung(&mut self, m: &Meldung) -> Result<(), WriteError> {
        match m {
            Meldung::RuemVa(m) => self.ruem_va(m)?,
            Meldung::Abbm(m) => self.abbm(m)?,
            Meldung::Erlm(m) => self.erlm(m)?,
            Meldung::AkmTr(m) => self.akm_tr(m)?,
            Meldung::AbbmTr(m) => self.abbm_tr(m)?,
        }
        self.close("Meldung");
        Ok(())
    }

    fn ruem_va(&mut self, m: &RuemVa) -> Result<(), WriteError> {
        self.meldung_head(types::RUEM_VA, &m.kopf);
        self.count("Position", m.positionen.len(), 1, max::MANY)?;
        for p in &m.positionen {
            self.typed("Position", "am", types::POS_RUEM_VA);
            self.position_inner(&p.position);
            if let Some(a) = &p.adresse_abweichend {
                self.open("AdresseAbweichend", &[]);
                match a {
                    AdresseAbweichend::Strassenname(v) => self.leaf("strassenname", v.as_str()),
                    AdresseAbweichend::Postleitzahl(v) => self.leaf("postleitzahl", v.as_str()),
                    AdresseAbweichend::Ort(v) => self.leaf("ort", v.as_str()),
                    AdresseAbweichend::Hausnummer(v) => self.leaf("hausnummer", v.as_str()),
                }
                self.close("AdresseAbweichend");
            }
            self.close("Position");
        }
        self.meldung_tail(&m.kopf);
        self.count("Ressource", m.ressourcen.len(), 0, max::MANY)?;
        for r in &m.ressourcen {
            self.ressource(r);
        }
        self.opt("Technologie", m.technologie.as_ref().map(AsRef::as_ref));
        self.date("Wechseltermin", m.wechseltermin);
        if let Some(r) = &m.rufnummernportierung {
            self.portierte(r)?;
        }
        Ok(())
    }

    fn ressource(&mut self, r: &TechnischeRessource) {
        self.open("Ressource", &[]);
        self.opt("Vertragsnummer", r.vertragsnummer.as_ref().map(AsRef::as_ref));
        self.opt("LineID", r.line_id.as_ref().map(AsRef::as_ref));
        self.opt("Identifizierer", r.identifizierer.as_ref().map(AsRef::as_ref));
        self.opt("KennungTNBabg", r.kennung_tnb_abg.as_ref().map(AsRef::as_ref));
        self.close("Ressource");
    }

    fn portierte(&mut self, r: &PortierteRufnummern) -> Result<(), WriteError> {
        self.open("RufnummernPortierung", &[]);
        match r {
            PortierteRufnummern::Bloecke { kopf, bloecke } => {
                self.open("portierungRufnummernbloecke", &[]);
                self.count("zuPortierenderRufnummernblock", bloecke.len(), 1, max::BLOCKS)?;
                self.anlagenkopf(kopf);
                for (b, pki) in bloecke {
                    self.open("zuPortierenderRufnummernblock", &[]);
                    self.block_inner(b);
                    self.leaf("PortierungskennungPKIabg", pki.as_str());
                    self.close("zuPortierenderRufnummernblock");
                }
                self.close("portierungRufnummernbloecke");
            }
            PortierteRufnummern::Rufnummern(list) => {
                self.open("portierungRufnummern", &[]);
                self.count("zuPortierendeOnkzRnr", list.len(), 1, max::NUMBERS)?;
                for (n, pki) in list {
                    self.open("zuPortierendeOnkzRnr", &[]);
                    self.leaf("ONKZ", n.onkz.as_str());
                    self.leaf("rufnummer", n.rufnummer.as_str());
                    self.leaf("PortierungskennungPKIabg", pki.as_str());
                    self.close("zuPortierendeOnkzRnr");
                }
                self.close("portierungRufnummern");
            }
        }
        self.close("RufnummernPortierung");
        Ok(())
    }

    fn abbm(&mut self, m: &Abbm) -> Result<(), WriteError> {
        self.meldung_head(types::ABBM, &m.kopf);
        self.count("Position", m.positionen.len(), 1, max::MANY)?;
        for p in &m.positionen {
            self.typed("Position", "am", types::POS_ABBM);
            self.position_inner(&p.position);
            self.count("Rufnummer", p.rufnummern.len(), 0, max::NUMBERS)?;
            for n in &p.rufnummern {
                self.leaf("Rufnummer", n.as_str());
            }
            self.close("Position");
        }
        self.meldung_tail(&m.kopf);
        self.refs(
            m.wechseltermin,
            m.storno_id.as_ref().map(AsRef::as_ref),
            m.aenderungs_id.as_ref().map(AsRef::as_ref),
        );
        self.opt("Begruendung", m.begruendung.as_ref().map(AsRef::as_ref));
        Ok(())
    }

    fn erlm(&mut self, m: &Erlm) -> Result<(), WriteError> {
        self.meldung_head(types::ERLM, &m.kopf);
        self.positions(types::POS_ERLM, m.positionen.iter())?;
        self.meldung_tail(&m.kopf);
        self.refs(
            m.wechseltermin,
            m.storno_id.as_ref().map(AsRef::as_ref),
            m.aenderungs_id.as_ref().map(AsRef::as_ref),
        );
        Ok(())
    }

    fn akm_tr(&mut self, m: &AkmTr) -> Result<(), WriteError> {
        self.meldung_head(types::AKM_TR, &m.kopf);
        self.positions(types::POS_AKM_TR, m.positionen.iter())?;
        self.meldung_tail(&m.kopf);
        self.boolean("Ressourcenuebernahme", m.ressourcenuebernahme);
        self.count("UebernahmeLeitung", m.uebernahme_leitung.len(), 0, max::MANY)?;
        for l in &m.uebernahme_leitung {
            self.open("UebernahmeLeitung", &[]);
            match l {
                UebernahmeLeitung::Vertragsnummer(v) => self.leaf("Vertragsnummer", v.as_str()),
                UebernahmeLeitung::LineId(v) => self.leaf("LineID", v.as_str()),
            }
            self.close("UebernahmeLeitung");
        }
        self.boolean("SichererHafen", m.sicherer_hafen);
        self.opt("PortierungskennungPKIauf", m.pki_auf.as_ref().map(AsRef::as_ref));
        Ok(())
    }

    fn abbm_tr(&mut self, m: &AbbmTr) -> Result<(), WriteError> {
        self.meldung_head(types::ABBM_TR, &m.kopf);
        self.positions(types::POS_ABBM_TR, m.positionen.iter())?;
        self.meldung_tail(&m.kopf);
        Ok(())
    }

    fn teq(&mut self, t: &Teq) -> Result<(), WriteError> {
        self.open("Quittung", &[]);
        self.carrier("Absender", &t.absender);
        self.leaf("VorabstimmungsIdRef", t.vorabstimmungs_id.as_str());
        self.positions(types::POS_TEQ, t.positionen.iter())?;
        self.opt("StornoIdRef", t.storno_id.as_ref().map(AsRef::as_ref));
        self.opt("AenderungsIdRef", t.aenderungs_id.as_ref().map(AsRef::as_ref));
        self.close("Quittung");
        Ok(())
    }
}
