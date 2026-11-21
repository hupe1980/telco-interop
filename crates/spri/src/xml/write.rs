//! Writing payloads in the schema's element order.

use core::ops::{Deref, DerefMut};

use carrier_core::xml::{self, NS_SOAP, NS_XSI, WriteError};

use super::{NS_ENVELOPE, NS_FTTX, NS_MESSAGE, NS_ORDER, base64, max, meldungstyp_type, types};
use crate::model::{
    Abbm, AbbmPv, Abm, AbmPv, AenderungPosition, AkmPv, Anlage, Ansprechpartner, AnsprechpartnerAuftrag,
    AnsprechpartnerMitRolle, Auftrag, Auftraggeber, Control, Diagnose, Entm, EntmPv, Entstoerung, Erlm, ErlmK,
    FttxAnschluss, Geschaeftsfall, Inhaber, Kdle, Kenner, Kuendigung, Leistungsaenderung, LeistungsmerkmalAenderung,
    Meldung, Meldungsinhalt, Montageleistung, Ort, Payload, Person, PortierungAnschluss, Position, PositionWita,
    Produkt, Providerwechsel, ProviderwechselRueckmeldung, Qeb, RuemPv, StandortA, StandortVersand, Strasse, Tbk, Teq,
    Terminanforderung, Vzm, WitaInfos, Wunschtermin, ZwmAg, ZwmLe,
};
use crate::{Auftraggebernummer, Telefonnummer, VorabstimmungsId};

impl Payload {
    /// Writes the payload element alone (`annehmenAuftragRequest` etc.), without an XML
    /// declaration, ready to go into a SOAP body or a file.
    ///
    /// # Errors
    ///
    /// A repetition outside the schema's bounds (e.g. a message without a position).
    pub fn to_xml(&self) -> Result<String, WriteError> {
        let mut w = Writer::new();
        w.payload(self)?;
        Ok(w.0.out)
    }

    /// Writes the payload inside a SOAP 1.1 `Envelope`/`Body`, with an empty `Header` for the
    /// application's WS-Security header (`policy.xml`), and an XML declaration.
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

/// The S/PRI writer: the shared indenting writer plus the S/PRI element structure.
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

fn s<T: AsRef<str>>(v: Option<&T>) -> Option<&str> {
    v.map(AsRef::as_ref)
}

impl Writer {
    const fn new() -> Self {
        Self(xml::Writer::new())
    }

    fn payload(&mut self, p: &Payload) -> Result<(), WriteError> {
        let root = match p {
            Payload::AuftragRequest { .. } => "env:annehmenAuftragRequest",
            Payload::AuftragResponse { .. } => "env:annehmenAuftragResponse",
            Payload::MeldungRequest { .. } => "env:annehmenMeldungRequest",
            Payload::MeldungResponse { .. } => "env:annehmenMeldungResponse",
        };
        match p {
            Payload::AuftragRequest { .. } => self.open(
                root,
                &[
                    ("xmlns:env", NS_ENVELOPE),
                    ("xmlns:order", NS_ORDER),
                    ("xmlns:fttx", NS_FTTX),
                    ("xmlns:xsi", NS_XSI),
                ],
            ),
            Payload::MeldungRequest { .. } => {
                self.open(root, &[("xmlns:env", NS_ENVELOPE), ("xmlns:message", NS_MESSAGE), ("xmlns:xsi", NS_XSI)]);
            }
            _ => self.open(root, &[("xmlns:env", NS_ENVELOPE)]),
        }
        self.control(p.control());
        match p {
            Payload::AuftragRequest { auftrag, .. } => self.auftrag(auftrag)?,
            Payload::MeldungRequest { meldung, .. } => self.meldung(meldung)?,
            Payload::AuftragResponse { quittung, .. } | Payload::MeldungResponse { quittung, .. } => {
                self.teq(quittung)?;
            }
        }
        self.close(root);
        Ok(())
    }

    fn control(&mut self, c: &Control) {
        self.open("control", &[]);
        self.leaf("zeitstempel", &c.zeitstempel.to_string());
        self.leaf("majorRelease", &format!("{:02}", c.release.major));
        self.leaf("minorRelease", &format!("{:02}", c.release.minor));
        self.open("signaturId", &[]);
        self.leaf("issuer", c.signatur.issuer.as_str());
        self.leaf("serial", c.signatur.serial.as_str());
        self.close("signaturId");
        self.close("control");
    }

    // -- shared complex types --------------------------------------------------------------

    fn auftraggeber(&mut self, name: &'static str, a: &Auftraggeber) {
        self.open(name, &[]);
        self.leaf("auftraggebernummer", a.auftraggebernummer.as_str());
        self.leaf("leistungsnummer", a.leistungsnummer.as_str());
        self.close(name);
    }

    fn person_inner(&mut self, p: &Person) {
        self.leaf("anrede", p.anrede.as_str());
        self.opt("vorname", s(p.vorname.as_ref()));
        self.leaf("nachname", p.nachname.as_str());
    }

    fn ansprechpartner_inner(&mut self, a: &Ansprechpartner) {
        self.person_inner(&a.person);
        self.leaf("telefonnummer", a.telefonnummer.as_str());
        self.opt("mobilfunknummer", s(a.mobilfunknummer.as_ref()));
        self.opt("faxnummer", s(a.faxnummer.as_ref()));
        self.opt("emailadresse", s(a.emailadresse.as_ref()));
    }

    fn ansprechpartner(&mut self, name: &'static str, a: &Ansprechpartner) {
        self.open(name, &[]);
        self.ansprechpartner_inner(a);
        self.close(name);
    }

    fn ansprechpartner_mit_rolle(&mut self, name: &'static str, a: &AnsprechpartnerMitRolle) {
        self.open(name, &[]);
        self.ansprechpartner_inner(&a.ansprechpartner);
        self.leaf("rolle", a.rolle.as_str());
        self.close(name);
    }

    fn inhaber(&mut self, i: &Inhaber, phones: Option<(Option<&Telefonnummer>, Option<&Telefonnummer>)>) {
        let name = match i {
            Inhaber::Person(p) => {
                self.open("person", &[]);
                self.person_inner(p);
                "person"
            }
            Inhaber::Firma(f) => {
                self.open("firma", &[]);
                self.leaf("anrede", f.anrede.as_str());
                self.leaf("firmenname", f.firmenname.as_str());
                self.opt("firmennameZweiterTeil", s(f.firmenname_zweiter_teil.as_ref()));
                "firma"
            }
        };
        if let Some((tel, mobil)) = phones {
            self.opt("telefonnummer", s(tel));
            self.opt("mobilfunknummer", s(mobil));
        }
        self.close(name);
    }

    fn strasse(&mut self, st: &Strasse) {
        self.open("strasse", &[]);
        self.leaf("strassenname", st.strassenname.as_str());
        self.leaf("hausnummer", st.hausnummer.as_str());
        self.opt("hausnummernZusatz", s(st.hausnummern_zusatz.as_ref()));
        self.close("strasse");
    }

    fn ort(&mut self, o: &Ort) {
        self.open("ort", &[]);
        self.leaf("ortsname", o.ortsname.as_str());
        self.opt("ortsteil", s(o.ortsteil.as_ref()));
        self.close("ort");
    }

    fn standort_a(&mut self, st: &StandortA) {
        self.open("standortA", &[]);
        self.inhaber(&st.inhaber, Some((st.telefonnummer.as_ref(), st.mobilfunknummer.as_ref())));
        self.strasse(&st.strasse);
        if let Some(g) = &st.gebaeudeteil {
            self.open("gebaeudeteil", &[]);
            self.leaf("gebaeudeteilName", g.name.as_str());
            self.opt("gebaeudeteilZusatz", s(g.zusatz.as_ref()));
            self.close("gebaeudeteil");
        }
        self.opt("land", s(st.land.as_ref()));
        self.leaf("postleitzahl", st.postleitzahl.as_str());
        self.ort(&st.ort);
        self.opt("lageTAE_ONT", s(st.lage_tae_ont.as_ref()));
        self.close("standortA");
    }

    fn standort_versand(&mut self, st: &StandortVersand) {
        self.open("standortVersand", &[]);
        self.inhaber(&st.inhaber, None);
        self.strasse(&st.strasse);
        self.opt("land", s(st.land.as_ref()));
        self.leaf("postleitzahl", st.postleitzahl.as_str());
        self.ort(&st.ort);
        self.close("standortVersand");
    }

    fn wunschtermin(&mut self, name: &'static str, t: Wunschtermin) {
        self.open(name, &[]);
        self.date("datum", t.datum);
        if let Some(z) = t.zeitfenster {
            self.leaf("zeitfenster", &z.to_string());
        }
        self.close(name);
    }

    fn termine(&mut self, t: Wunschtermin) {
        self.open("termine", &[]);
        self.wunschtermin("auftraggeberWunschtermin", t);
        self.close("termine");
    }

    fn anlage(&mut self, a: Option<&Anlage>) {
        let Some(a) = a else { return };
        self.open("anlage", &[]);
        self.leaf("dateiname", a.dateiname.as_str());
        self.leaf("dateityp", a.dateityp.as_str());
        self.opt("beschreibung", s(a.beschreibung.as_ref()));
        self.leaf("inhalt", &base64::encode(&a.inhalt));
        self.leaf("anlagentyp", a.anlagentyp.as_str());
        self.close("anlage");
    }

    fn kenner(&mut self, k: &Kenner) {
        self.open("kenner", &[]);
        if let Some(p) = &k.projekt_id {
            self.open("projektId", &[]);
            self.leaf("projektkenner", p.projektkenner.as_str());
            self.opt("kopplungskenner", s(p.kopplungskenner.as_ref()));
            self.close("projektId");
        }
        if let Some(a) = &k.auftragskenner {
            self.open("auftragskenner", &[]);
            self.leaf("auftragsklammer", &a.auftragsklammer.to_string());
            self.leaf("anzahlAuftraege", &a.anzahl_auftraege.to_string());
            self.close("auftragskenner");
        }
        self.close("kenner");
    }

    fn ansprechpartner_auftrag(&mut self, a: &AnsprechpartnerAuftrag) {
        self.open("ansprechpartner", &[]);
        self.ansprechpartner("auftragsmanagement", &a.auftragsmanagement);
        for w in &a.weitere {
            self.ansprechpartner_mit_rolle("ansprechpartner", w);
        }
        self.close("ansprechpartner");
    }

    fn produkt(&mut self, p: &Produkt) {
        self.typed("produkt", "fttx", types::PRODUKT);
        self.leaf("bezeichner", p.bezeichner.as_str());
        self.close("produkt");
    }

    fn produkt_positionen(&mut self, positionen: &[Produkt]) {
        for p in positionen {
            self.open("position", &[]);
            self.produkt(p);
            self.close("position");
        }
    }

    fn aenderung_positionen(&mut self, positionen: &[AenderungPosition], min: usize) -> Result<(), WriteError> {
        self.count("position", positionen.len(), min, max::UNBOUNDED)?;
        for p in positionen {
            self.open("position", &[]);
            self.produkt(&p.produkt);
            self.leaf("aktionscode", p.aktionscode.as_str());
            self.close("position");
        }
        Ok(())
    }

    fn wita_infos(&mut self, w: &WitaInfos) {
        self.opt("witaVertragsnummer", s(w.wita_vertragsnummer.as_ref()));
        self.opt("technologie", s(w.technologie.as_ref()));
    }

    fn montageleistung(&mut self, m: Option<&Montageleistung>) {
        let Some(m) = m else { return };
        self.open("montageleistung", &[]);
        self.ansprechpartner("ansprechpartner", &m.ansprechpartner);
        self.opt("montagehinweis", s(m.montagehinweis.as_ref()));
        self.close("montageleistung");
    }

    fn va(&mut self, id: Option<&VorabstimmungsId>) {
        self.opt("vorabstimmungId", s(id));
    }

    // -- orders ----------------------------------------------------------------------------

    fn auftrag(&mut self, a: &Auftrag) -> Result<(), WriteError> {
        self.open("auftrag", &[]);
        self.leaf("externeAuftragsnummer", a.externe_auftragsnummer.as_str());
        self.auftraggeber("auftraggeber", &a.auftraggeber);
        if let Some(b) = &a.besteller {
            self.auftraggeber("besteller", b);
        }
        self.geschaeftsfall(&a.geschaeftsfall)?;
        self.leaf("geschaeftsfallArt", a.geschaeftsfall_art.as_str());
        self.leaf("aenderungskennzeichen", a.aenderungskennzeichen.as_str());
        self.opt_date("terminNeu", a.termin_neu);
        if let Some(k) = &a.kenner {
            self.kenner(k);
        }
        self.close("auftrag");
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn geschaeftsfall(&mut self, g: &Geschaeftsfall) -> Result<(), WriteError> {
        let ty = match g {
            Geschaeftsfall::Neu(_) => types::NEU,
            Geschaeftsfall::KueAg(_) => types::KUE_AG,
            Geschaeftsfall::Lae(_) => types::LAE,
            Geschaeftsfall::AenLmae(_) => types::AEN_LMAE,
            Geschaeftsfall::Est(_) => types::EST,
            Geschaeftsfall::Pv(_) => types::PV,
            Geschaeftsfall::Get(_) => types::GET,
            Geschaeftsfall::Set(_) => types::SET,
        };
        self.typed("geschaeftsfall", "order", ty);
        match g {
            Geschaeftsfall::Neu(b) => {
                self.va(b.vorabstimmungs_id.as_ref());
                self.anlage(b.anlage.as_ref());
                self.ansprechpartner_auftrag(&b.ansprechpartner);
                self.termine(b.wunschtermin);
                self.open("auftragsposition", &[]);
                self.produkt(&b.produkt);
                self.typed("geschaeftsfallProdukt", "fttx", types::FTTX_NEU);
                let f = &b.fttx;
                self.standort_a(&f.standort_a);
                if let Some(v) = &f.standort_versand {
                    self.standort_versand(v);
                }
                if let Some(v) = &f.vormieter {
                    self.open("vormieter", &[]);
                    if let Some(p) = &v.person {
                        self.open("person", &[]);
                        self.opt("vorname", s(p.vorname.as_ref()));
                        self.leaf("nachname", p.nachname.as_str());
                        self.close("person");
                    }
                    if let Some(h) = &v.home_id {
                        self.open("homeId", &[]);
                        self.leaf("homeIdNummer", h.as_str());
                        self.close("homeId");
                    }
                    self.opt("seriennummerONT", s(v.seriennummer_ont.as_ref()));
                    self.close("vormieter");
                }
                self.montageleistung(f.montageleistung.as_ref());
                self.wita_infos(&f.wita.wita);
                if let Some(b) = &f.wita.bestandssuche {
                    self.open("bestandssuche", &[]);
                    self.leaf("onkz", b.onkz.as_str());
                    self.leaf("rufnummer", b.rufnummer.as_str());
                    self.close("bestandssuche");
                }
                if let Some(r) = &f.wita.rufnummern_portierung {
                    self.open("rufnummernPortierung", &[]);
                    match &r.anschluss {
                        PortierungAnschluss::Anlage { onkz, durchwahlnummer, abfragestelle, bloecke } => {
                            self.open("anlagenanschluss", &[]);
                            self.count("zuPortierenderRufnummernblock", bloecke.len(), 1, max::BLOCKS)?;
                            self.open("onkzDurchwahlAbfragestelle", &[]);
                            self.leaf("onkz", onkz.as_str());
                            self.leaf("durchwahlnummer", durchwahlnummer.as_str());
                            self.leaf("abfragestelle", abfragestelle.as_str());
                            self.close("onkzDurchwahlAbfragestelle");
                            for blk in bloecke {
                                self.open("zuPortierenderRufnummernblock", &[]);
                                self.leaf("rnrBlockVon", blk.von.as_str());
                                self.leaf("rnrBlockBis", blk.bis.as_str());
                                self.close("zuPortierenderRufnummernblock");
                            }
                            self.close("anlagenanschluss");
                        }
                        PortierungAnschluss::Rufnummern(list) => {
                            self.open("einzelanschluss", &[]);
                            self.open("rufnummernliste", &[]);
                            self.count("zuPortierendeOnkzRnr", list.len(), 1, max::NUMBERS)?;
                            for n in list {
                                self.open("zuPortierendeOnkzRnr", &[]);
                                self.leaf("onkz", n.onkz.as_str());
                                self.leaf("rufnummer", n.rufnummer.as_str());
                                self.close("zuPortierendeOnkzRnr");
                            }
                            self.close("rufnummernliste");
                            self.close("einzelanschluss");
                        }
                        PortierungAnschluss::AlleRufnummern => {
                            self.open("einzelanschluss", &[]);
                            self.leaf("alleRufnummern", "J");
                            self.close("einzelanschluss");
                        }
                    }
                    self.leaf("portierungskenner", r.portierungskenner.as_str());
                    self.close("rufnummernPortierung");
                }
                if let Some(e) = f.wita.abgebender_ekp {
                    self.boolean("abgebenderEKP", e);
                }
                self.close("geschaeftsfallProdukt");
                self.produkt_positionen(&b.positionen);
                self.close("auftragsposition");
            }
            Geschaeftsfall::KueAg(k) => self.kuendigung(k),
            Geschaeftsfall::Lae(l) => self.leistungsaenderung(l)?,
            Geschaeftsfall::AenLmae(l) => self.leistungsmerkmal_aenderung(l)?,
            Geschaeftsfall::Est(e) => self.entstoerung(e),
            Geschaeftsfall::Pv(p) => self.providerwechsel(p),
            Geschaeftsfall::Get(d) | Geschaeftsfall::Set(d) => self.diagnose(d)?,
        }
        self.close("geschaeftsfall");
        Ok(())
    }

    fn kuendigung(&mut self, k: &Kuendigung) {
        self.leaf("vertragsnummer", k.vertragsnummer.as_str());
        self.leaf("lineId", k.line_id.as_str());
        self.va(k.vorabstimmungs_id.as_ref());
        self.anlage(k.anlage.as_ref());
        self.ansprechpartner_auftrag(&k.ansprechpartner);
        self.termine(k.wunschtermin);
        if let Some(w) = &k.fttx {
            self.open("auftragsposition", &[]);
            self.typed("geschaeftsfallProdukt", "fttx", types::FTTX_KUE);
            self.wita_infos(w);
            self.close("geschaeftsfallProdukt");
            self.close("auftragsposition");
        }
    }

    fn leistungsaenderung(&mut self, l: &Leistungsaenderung) -> Result<(), WriteError> {
        self.leaf("vertragsnummer", l.vertragsnummer.as_str());
        self.leaf("lineId", l.line_id.as_str());
        self.anlage(l.anlage.as_ref());
        self.ansprechpartner_auftrag(&l.ansprechpartner);
        self.termine(l.wunschtermin);
        self.open("auftragsposition", &[]);
        self.produkt(&l.produkt);
        self.aenderung_positionen(&l.positionen, 0)?;
        self.close("auftragsposition");
        Ok(())
    }

    fn leistungsmerkmal_aenderung(&mut self, l: &LeistungsmerkmalAenderung) -> Result<(), WriteError> {
        self.leaf("vertragsnummer", l.vertragsnummer.as_str());
        self.leaf("lineId", l.line_id.as_str());
        self.anlage(l.anlage.as_ref());
        self.ansprechpartner_auftrag(&l.ansprechpartner);
        self.termine(l.wunschtermin);
        self.open("auftragsposition", &[]);
        self.produkt(&l.produkt);
        self.aenderung_positionen(&l.positionen, 1)?;
        self.close("auftragsposition");
        Ok(())
    }

    fn entstoerung(&mut self, e: &Entstoerung) {
        self.leaf("vertragsnummer", e.vertragsnummer.as_str());
        self.leaf("lineId", e.line_id.as_str());
        self.anlage(e.anlage.as_ref());
        self.ansprechpartner_auftrag(&e.ansprechpartner);
        let k = &e.kontakt_endkunde;
        self.open("kontaktEndkunde", &[]);
        self.leaf("anrede", k.anrede.as_str());
        self.leaf("vorname", k.vorname.as_str());
        self.leaf("nachname", k.nachname.as_str());
        self.opt("telefonnummer", s(k.telefonnummer.as_ref()));
        self.opt("mobilfunknummer", s(k.mobilfunknummer.as_ref()));
        self.opt("faxnummer", s(k.faxnummer.as_ref()));
        self.opt("emailadresse", s(k.emailadresse.as_ref()));
        self.boolean("kontaktaufnahmeErwuenscht", k.kontaktaufnahme_erwuenscht);
        self.close("kontaktEndkunde");
        self.termine(e.wunschtermin);
        self.leaf("stoerungsNrAuftraggeber", e.stoerungs_nr_auftraggeber.as_str());
        if let Some(r) = &e.reklamation {
            self.open("reklamation", &[]);
            self.leaf("stoerungsNrLeistungserbringer", r.stoerungs_nr_leistungserbringer.as_str());
            self.leaf("reklamationsgrund", r.reklamationsgrund.as_str());
            self.close("reklamation");
        }
        self.boolean("vorpruefungErfolgt", e.vorpruefung_erfolgt);
        self.leaf("stoerungsbeschreibung", e.stoerungsbeschreibung.as_str());
        if let Some(x) = e.expressentstoerung {
            self.boolean("expressentstoerung", x);
        }
    }

    fn providerwechsel(&mut self, p: &Providerwechsel) {
        self.leaf("lineId", p.line_id.as_str());
        self.leaf("vorabstimmungId", p.vorabstimmungs_id.as_str());
        self.anlage(p.anlage.as_ref());
        self.ansprechpartner_auftrag(&p.ansprechpartner);
        self.termine(p.wunschtermin);
        self.open("auftragsposition", &[]);
        self.produkt(&p.produkt);
        self.typed("geschaeftsfallProdukt", "fttx", types::FTTX_PV);
        self.standort_a(&p.fttx.standort_a);
        self.montageleistung(p.fttx.montageleistung.as_ref());
        self.close("geschaeftsfallProdukt");
        self.produkt_positionen(&p.positionen);
        self.close("auftragsposition");
    }

    fn diagnose(&mut self, d: &Diagnose) -> Result<(), WriteError> {
        self.leaf("vertragsnummer", d.vertragsnummer.as_str());
        self.leaf("lineId", d.line_id.as_str());
        self.open("auftragsposition", &[]);
        self.open("parameter", &[]);
        self.count("operation", d.operationen.len(), 1, max::UNBOUNDED)?;
        for o in &d.operationen {
            self.leaf("operation", o.as_str());
        }
        self.close("parameter");
        self.close("auftragsposition");
        Ok(())
    }

    // -- messages --------------------------------------------------------------------------

    fn meldung(&mut self, m: &Meldung) -> Result<(), WriteError> {
        self.open("meldung", &[]);
        self.open("auftragstyp", &[]);
        self.leaf("geschaeftsfall", m.auftragstyp.geschaeftsfall.as_str());
        self.leaf("aenderungsKennzeichen", m.auftragstyp.aenderungskennzeichen.as_str());
        self.leaf("geschaeftsfallart", m.auftragstyp.geschaeftsfallart.as_str());
        self.close("auftragstyp");
        let ty = meldungstyp_type(m.typ());
        self.typed("meldungstyp", "message", &ty);
        self.open("meldungsattribute", &[]);
        match &m.inhalt {
            Meldungsinhalt::Qeb(q) => self.qeb(q)?,
            Meldungsinhalt::Abm(a) => self.abm(a)?,
            Meldungsinhalt::Abbm(a) => self.abbm(a)?,
            Meldungsinhalt::Erlm(e) => self.erlm(e)?,
            Meldungsinhalt::Entm(e) => self.entm(e)?,
            Meldungsinhalt::Vzm(v) => self.vzm(v)?,
            Meldungsinhalt::Tam(t) | Meldungsinhalt::Mtam(t) => self.tam(t)?,
            Meldungsinhalt::Kdle(k) => self.kdle(k)?,
            Meldungsinhalt::ZwmLe(z) => self.zwm_le(z)?,
            Meldungsinhalt::ZwmAg(z) => self.zwm_ag(z)?,
            Meldungsinhalt::TbkLe(t) | Meldungsinhalt::TbkAg(t) => self.tbk(t)?,
            Meldungsinhalt::AkmPv(a) => self.akm_pv(a)?,
            Meldungsinhalt::RuemPv(r) => self.ruem_pv(r)?,
            Meldungsinhalt::AbmPv(a) => self.abm_pv(a)?,
            Meldungsinhalt::ErlmPv(e) => {
                self.kopf(Some(&e.vertragsnummer), &e.externe_auftragsnummer, &e.auftraggebernummer);
                self.date("erledigungstermin", e.erledigungstermin);
                self.positions(&e.positionen)?;
            }
            Meldungsinhalt::EntmPv(e) => self.entm_pv(e)?,
            Meldungsinhalt::AbbmPv(a) => self.abbm_pv(a)?,
            Meldungsinhalt::ErlmK(e) => self.erlm_k(e)?,
        }
        self.close("meldungspositionen");
        self.close("meldungstyp");
        self.close("meldung");
        Ok(())
    }

    /// `vertragsnummer`, `externeAuftragsnummer`, `auftraggebernummer`.
    fn kopf(
        &mut self,
        vertragsnummer: Option<&crate::Vertragsnummer>,
        ext: &crate::ExterneAuftragsnummer,
        ag: &Auftraggebernummer,
    ) {
        self.opt("vertragsnummer", s(vertragsnummer));
        self.leaf("externeAuftragsnummer", ext.as_str());
        self.leaf("auftraggebernummer", ag.as_str());
    }

    fn besteller(&mut self, b: Option<&Auftraggebernummer>) {
        self.opt("auftraggebernummerBesteller", s(b));
    }

    /// Closes `meldungsattribute`, opens `meldungspositionen` and checks the count.
    fn start_positions(&mut self, n: usize) -> Result<(), WriteError> {
        self.close("meldungsattribute");
        self.open("meldungspositionen", &[]);
        self.count("position", n, 1, max::UNBOUNDED)
    }

    fn position_inner(&mut self, p: &Position) {
        self.leaf("meldungscode", p.code.as_str());
        self.leaf("meldungstext", p.text.as_str());
    }

    fn positions(&mut self, positionen: &[Position]) -> Result<(), WriteError> {
        self.start_positions(positionen.len())?;
        for p in positionen {
            self.open("position", &[]);
            self.position_inner(p);
            self.close("position");
        }
        Ok(())
    }

    fn wita_position_inner(&mut self, p: &PositionWita) {
        self.position_inner(&p.position);
        self.opt("witaMeldungscode", s(p.wita_meldungscode.as_ref()));
        self.opt("witaMeldungstext", s(p.wita_meldungstext.as_ref()));
    }

    fn wita_positions(&mut self, positionen: &[PositionWita]) -> Result<(), WriteError> {
        self.start_positions(positionen.len())?;
        for p in positionen {
            self.open("position", &[]);
            self.wita_position_inner(p);
            self.close("position");
        }
        Ok(())
    }

    fn positionsattribute(&mut self, ty: &str, a: Option<&Ansprechpartner>) {
        if let Some(a) = a {
            self.typed("positionsattribute", "message", ty);
            self.ansprechpartner("ansprechpartnerLeistungserbringer", a);
            self.close("positionsattribute");
        }
    }

    fn rueckmeldung(&mut self, name: &'static str, r: &ProviderwechselRueckmeldung) {
        self.open(name, &[]);
        self.opt("providername", s(r.providername.as_ref()));
        self.leaf("zustimmungProviderwechsel", r.zustimmung.as_str());
        self.opt("antwortcode", s(r.antwortcode.as_ref()));
        self.opt("antworttext", s(r.antworttext.as_ref()));
        self.close(name);
    }

    fn anschluss(&mut self, a: &FttxAnschluss) {
        self.open("anschluss", &[]);
        self.leaf("a10nsp", a.a10nsp.as_str());
        self.leaf("port", a.port.as_str());
        self.opt("s-vlan", s(a.s_vlan.as_ref()));
        self.opt("c-vlan", s(a.c_vlan.as_ref()));
        self.leaf("lineId", a.line_id.as_str());
        self.leaf("uebergabepunktKunde", a.uebergabepunkt_kunde.as_str());
        self.opt("witaVertragsnummer", s(a.wita_vertragsnummer.as_ref()));
        self.close("anschluss");
    }

    fn line(&mut self, line_id: &crate::LineId) {
        self.open("anschluss", &[]);
        self.leaf("lineId", line_id.as_str());
        self.close("anschluss");
    }

    fn qeb(&mut self, q: &Qeb) -> Result<(), WriteError> {
        self.leaf("externeAuftragsnummer", q.externe_auftragsnummer.as_str());
        self.leaf("auftraggebernummer", q.auftraggebernummer.as_str());
        self.besteller(q.auftraggebernummer_besteller.as_ref());
        self.va(q.vorabstimmungs_id.as_ref());
        self.opt("stoerungsNrLeistungserbringer", s(q.stoerungs_nr_leistungserbringer.as_ref()));
        self.start_positions(q.positionen.len())?;
        for p in &q.positionen {
            self.open("position", &[]);
            self.position_inner(&p.position);
            self.positionsattribute(types::POS_QEB, p.ansprechpartner_leistungserbringer.as_ref());
            self.close("position");
        }
        Ok(())
    }

    fn abm(&mut self, a: &Abm) -> Result<(), WriteError> {
        self.kopf(Some(&a.vertragsnummer), &a.externe_auftragsnummer, &a.auftraggebernummer);
        self.besteller(a.auftraggebernummer_besteller.as_ref());
        self.va(a.vorabstimmungs_id.as_ref());
        self.wunschtermin("verbindlicherLiefertermin", a.verbindlicher_liefertermin);
        if let Some(r) = &a.abgebender_provider {
            self.rueckmeldung("abgebenderProvider", r);
        }
        self.anschluss(&a.anschluss);
        self.start_positions(a.positionen.len())?;
        for p in &a.positionen {
            self.open("position", &[]);
            self.wita_position_inner(&p.position);
            self.positionsattribute(types::POS_ABM, p.ansprechpartner_leistungserbringer.as_ref());
            self.close("position");
        }
        Ok(())
    }

    fn abbm(&mut self, a: &Abbm) -> Result<(), WriteError> {
        self.kopf(a.vertragsnummer.as_ref(), &a.externe_auftragsnummer, &a.auftraggebernummer);
        self.besteller(a.auftraggebernummer_besteller.as_ref());
        self.va(a.vorabstimmungs_id.as_ref());
        self.opt_date("wiedervorlagetermin", a.wiedervorlagetermin);
        if let Some(r) = &a.abgebender_provider {
            self.rueckmeldung("abgebenderProvider", r);
        }
        self.wita_positions(&a.positionen)
    }

    fn erlm(&mut self, e: &Erlm) -> Result<(), WriteError> {
        self.kopf(e.vertragsnummer.as_ref(), &e.externe_auftragsnummer, &e.auftraggebernummer);
        self.besteller(e.auftraggebernummer_besteller.as_ref());
        self.va(e.vorabstimmungs_id.as_ref());
        self.date("erledigungstermin", e.erledigungstermin);
        if let Some(r) = &e.entstoer_ergebnis {
            self.open("entstoerErgebnis", &[]);
            self.leaf("ergebnis", r.ergebnis.as_str());
            self.opt_date("entstoerZeitpunkt", r.entstoer_zeitpunkt);
            self.opt("anmerkung", s(r.anmerkung.as_ref()));
            self.close("entstoerErgebnis");
        }
        self.anlage(e.anlage.as_ref());
        for d in &e.diagnoseergebnisse {
            self.open("dignoseergebnis", &[]);
            self.leaf("name", d.name.as_str());
            self.leaf("wert", d.wert.as_str());
            self.close("dignoseergebnis");
        }
        self.wita_positions(&e.positionen)
    }

    fn entm(&mut self, e: &Entm) -> Result<(), WriteError> {
        self.kopf(Some(&e.vertragsnummer), &e.externe_auftragsnummer, &e.auftraggebernummer);
        self.besteller(e.auftraggebernummer_besteller.as_ref());
        self.va(e.vorabstimmungs_id.as_ref());
        self.date("entgelttermin", e.entgelttermin);
        self.anlage(e.anlage.as_ref());
        self.positions(&e.positionen)
    }

    fn vzm(&mut self, v: &Vzm) -> Result<(), WriteError> {
        self.kopf(v.vertragsnummer.as_ref(), &v.externe_auftragsnummer, &v.auftraggebernummer);
        self.besteller(v.auftraggebernummer_besteller.as_ref());
        self.va(v.vorabstimmungs_id.as_ref());
        self.opt_date("verzoegerungstermin", v.verzoegerungstermin);
        self.opt("verzoegerungsgrund", s(v.verzoegerungsgrund.as_ref()));
        self.wita_positions(&v.positionen)
    }

    fn tam(&mut self, t: &Terminanforderung) -> Result<(), WriteError> {
        self.kopf(Some(&t.vertragsnummer), &t.externe_auftragsnummer, &t.auftraggebernummer);
        self.besteller(t.auftraggebernummer_besteller.as_ref());
        self.va(t.vorabstimmungs_id.as_ref());
        self.wita_positions(&t.positionen)
    }

    fn kdle(&mut self, k: &Kdle) -> Result<(), WriteError> {
        self.kopf(Some(&k.vertragsnummer), &k.externe_auftragsnummer, &k.auftraggebernummer);
        self.date("abschaltungstermin", k.abschaltungstermin);
        self.opt("abschaltungsgrund", s(k.abschaltungsgrund.as_ref()));
        self.positions(&k.positionen)
    }

    fn zwm_le(&mut self, z: &ZwmLe) -> Result<(), WriteError> {
        self.kopf(Some(&z.vertragsnummer), &z.externe_auftragsnummer, &z.auftraggebernummer);
        self.besteller(z.auftraggebernummer_besteller.as_ref());
        self.va(z.vorabstimmungs_id.as_ref());
        self.leaf("nachricht", z.nachricht.as_str());
        if let Some(a) = &z.ansprechpartner {
            self.ansprechpartner_mit_rolle("ansprechpartner", a);
        }
        self.anlage(z.anlage.as_ref());
        if let Some(v) = &z.versand_ont {
            self.open("versandONT", &[]);
            self.leaf("seriennummerONT", v.seriennummer_ont.as_str());
            self.open("versand", &[]);
            self.leaf("unternehmen", v.unternehmen.as_str());
            self.leaf("sendungsnummer", v.sendungsnummer.as_str());
            self.close("versand");
            self.close("versandONT");
        }
        self.positions(&z.positionen)
    }

    fn zwm_ag(&mut self, z: &ZwmAg) -> Result<(), WriteError> {
        self.kopf(Some(&z.vertragsnummer), &z.externe_auftragsnummer, &z.auftraggebernummer);
        self.besteller(z.auftraggebernummer_besteller.as_ref());
        self.leaf("nachricht", z.nachricht.as_str());
        if let Some(a) = &z.ansprechpartner {
            self.ansprechpartner_mit_rolle("ansprechpartner", a);
        }
        self.anlage(z.anlage.as_ref());
        self.positions(&z.positionen)
    }

    fn tbk(&mut self, t: &Tbk) -> Result<(), WriteError> {
        self.kopf(Some(&t.vertragsnummer), &t.externe_auftragsnummer, &t.auftraggebernummer);
        self.besteller(t.auftraggebernummer_besteller.as_ref());
        self.open("termin", &[]);
        self.date("datum", t.termin.datum);
        if let Some(z) = t.termin.zeit {
            self.leaf("zeit", &z.strftime("%H:%M:%S").to_string());
        }
        if let Some(z) = t.termin.zeitfenster {
            self.leaf("zeitfenster", &z.to_string());
        }
        self.close("termin");
        self.positions(&t.positionen)
    }

    fn akm_pv(&mut self, a: &AkmPv) -> Result<(), WriteError> {
        self.kopf(Some(&a.vertragsnummer), &a.externe_auftragsnummer, &a.auftraggebernummer);
        self.va(a.vorabstimmungs_id.as_ref());
        self.line(&a.line_id);
        self.anlage(a.anlage.as_ref());
        self.open("aufnehmenderProvider", &[]);
        self.leaf("providername", a.aufnehmender_provider.providername.as_str());
        self.date("uebernahmedatumGeplant", a.aufnehmender_provider.uebernahmedatum_geplant);
        self.date("antwortfrist", a.aufnehmender_provider.antwortfrist);
        self.close("aufnehmenderProvider");
        self.positions(&a.positionen)
    }

    fn ruem_pv(&mut self, r: &RuemPv) -> Result<(), WriteError> {
        self.kopf(Some(&r.vertragsnummer), &r.externe_auftragsnummer, &r.auftraggebernummer);
        self.va(r.vorabstimmungs_id.as_ref());
        self.line(&r.line_id);
        self.rueckmeldung("abgebenderProvider", &r.abgebender_provider);
        self.positions(&r.positionen)
    }

    fn abm_pv(&mut self, a: &AbmPv) -> Result<(), WriteError> {
        self.kopf(Some(&a.vertragsnummer), &a.externe_auftragsnummer, &a.auftraggebernummer);
        self.line(&a.line_id);
        self.open("aufnehmenderProvider", &[]);
        self.leaf("providername", a.aufnehmender_provider.providername.as_str());
        self.date("uebernahmedatumVerbindlich", a.aufnehmender_provider.uebernahmedatum_verbindlich);
        self.close("aufnehmenderProvider");
        self.positions(&a.positionen)
    }

    fn entm_pv(&mut self, e: &EntmPv) -> Result<(), WriteError> {
        self.kopf(Some(&e.vertragsnummer), &e.externe_auftragsnummer, &e.auftraggebernummer);
        self.date("entgelttermin", e.entgelttermin);
        self.positions(&e.positionen)
    }

    fn abbm_pv(&mut self, a: &AbbmPv) -> Result<(), WriteError> {
        self.kopf(a.vertragsnummer.as_ref(), &a.externe_auftragsnummer, &a.auftraggebernummer);
        self.positions(&a.positionen)
    }

    fn erlm_k(&mut self, e: &ErlmK) -> Result<(), WriteError> {
        self.kopf(Some(&e.vertragsnummer), &e.externe_auftragsnummer, &e.auftraggebernummer);
        self.besteller(e.auftraggebernummer_besteller.as_ref());
        self.va(e.vorabstimmungs_id.as_ref());
        self.positions(&e.positionen)
    }

    fn teq(&mut self, t: &Teq) -> Result<(), WriteError> {
        self.open("quittung", &[]);
        self.leaf("meldungsTyp", t.meldungstyp.as_str());
        self.opt("externeAuftragsnummer", s(t.externe_auftragsnummer.as_ref()));
        self.opt("auftraggebernummer", s(t.auftraggebernummer.as_ref()));
        self.open("meldungspositionen", &[]);
        self.count("position", t.positionen.len(), 1, max::UNBOUNDED)?;
        for p in &t.positionen {
            self.open("position", &[]);
            self.position_inner(p);
            self.close("position");
        }
        self.close("meldungspositionen");
        self.close("quittung");
        Ok(())
    }
}
