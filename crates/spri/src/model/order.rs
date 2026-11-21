//! Orders (`auftrag`): the header and the eight business cases.
//!
//! Source: XSD 4.00.03 `oss-order.xsd` (`AuftragType` and the extensions of
//! `GeschaeftsfallType`), `oss-product-FTTx.xsd` (the FTTx products) and `oss-type-complex.xsd`.

use jiff::civil::Date;

use super::{
    Anlage, Ansprechpartner, AnsprechpartnerMitRolle, Auftraggeber, Kenner, Person, StandortA, StandortVersand,
    Wunschtermin,
};
use crate::{
    Abfragestelle, Aenderungskennzeichen, Aktionscode, Anrede, Diagnosename, Durchwahlnummer, Emailadresse,
    ExterneAuftragsnummer, Freitext, GeschaeftsfallArt, GeschaeftsfallCode, HomeId, LineId, Montagehinweis, Nachname,
    Onkz, Portierungskenner, Produktbezeichner, Rufnummer, RufnummernBlock, SeriennummerOnt, StoerungsNr, Technologie,
    Telefonnummer, Vertragsnummer, VorabstimmungsId, Vorname, WitaVertragsnummer,
};

/// An order (`AuftragType`), sent by the service provider (Auftraggeber) to the network owner
/// (Leistungserbringer) in `annehmenAuftragRequest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Auftrag {
    /// `externeAuftragsnummer`: the service provider's order number. A cancellation or
    /// postponement repeats it (S/PRI 4.4 §5.1.3, §5.1.4).
    pub externe_auftragsnummer: ExterneAuftragsnummer,
    /// `auftraggeber`.
    pub auftraggeber: Auftraggeber,
    /// `besteller`: who orders on behalf of the Auftraggeber, if not the Auftraggeber itself.
    pub besteller: Option<Auftraggeber>,
    /// `geschaeftsfall`: the business case and its data.
    pub geschaeftsfall: Geschaeftsfall,
    /// `geschaeftsfallArt`.
    pub geschaeftsfall_art: GeschaeftsfallArt,
    /// `aenderungskennzeichen`: standard order, cancellation or postponement.
    pub aenderungskennzeichen: Aenderungskennzeichen,
    /// `terminNeu`.
    pub termin_neu: Option<Date>,
    /// `kenner`: project and order grouping.
    pub kenner: Option<Kenner>,
}

/// The business case of an order: the concrete type of the abstract `GeschaeftsfallType`,
/// written as `xsi:type`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum Geschaeftsfall {
    /// `NEU` — `BereitstellungType`: a new line.
    Neu(Bereitstellung),
    /// `KUE-AG` — `KuendigungType`: the service provider cancels a line.
    KueAg(Kuendigung),
    /// `LAE` — `LeistungsaenderungType`: change of the service (product).
    Lae(Leistungsaenderung),
    /// `AEN-LMAE` — `LeistungsmerkmalAenderungType`: change of features.
    AenLmae(LeistungsmerkmalAenderung),
    /// `EST` — `EntstoerungType`: a fault report.
    Est(Entstoerung),
    /// `PV` — `ProviderwechselType`: take over a line from another service provider, after a
    /// WBCI pre-agreement.
    Pv(Providerwechsel),
    /// `GET` — `DiagnoseGetType`: read diagnostic values.
    Get(Diagnose),
    /// `SET` — `DiagnoseSetType`: trigger diagnostic operations.
    Set(Diagnose),
}

impl Geschaeftsfall {
    /// The business case code, e.g. `PV`.
    #[must_use]
    pub const fn code(&self) -> GeschaeftsfallCode {
        match self {
            Self::Neu(_) => GeschaeftsfallCode::Neu,
            Self::KueAg(_) => GeschaeftsfallCode::KueAg,
            Self::Lae(_) => GeschaeftsfallCode::Lae,
            Self::AenLmae(_) => GeschaeftsfallCode::AenLmae,
            Self::Est(_) => GeschaeftsfallCode::Est,
            Self::Pv(_) => GeschaeftsfallCode::Pv,
            Self::Get(_) => GeschaeftsfallCode::Get,
            Self::Set(_) => GeschaeftsfallCode::Set,
        }
    }

    /// The WBCI Vorabstimmungs-ID the order carries: always in `PV`, optional in `NEU` and
    /// `KUE-AG`, none in the other cases (S/PRI 4.4 §5.5.1.1).
    #[must_use]
    pub const fn vorabstimmungs_id(&self) -> Option<&VorabstimmungsId> {
        match self {
            Self::Neu(n) => n.vorabstimmungs_id.as_ref(),
            Self::KueAg(k) => k.vorabstimmungs_id.as_ref(),
            Self::Pv(p) => Some(&p.vorabstimmungs_id),
            Self::Lae(_) | Self::AenLmae(_) | Self::Est(_) | Self::Get(_) | Self::Set(_) => None,
        }
    }

    /// The line the order is about, if the business case names one (all but `NEU`).
    #[must_use]
    pub const fn line_id(&self) -> Option<&LineId> {
        match self {
            Self::Neu(_) => None,
            Self::KueAg(k) => Some(&k.line_id),
            Self::Lae(l) => Some(&l.line_id),
            Self::AenLmae(l) => Some(&l.line_id),
            Self::Est(e) => Some(&e.line_id),
            Self::Pv(p) => Some(&p.line_id),
            Self::Get(d) | Self::Set(d) => Some(&d.line_id),
        }
    }
}

/// The order's contact persons (`order:AnsprechpartnerType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnsprechpartnerAuftrag {
    /// `auftragsmanagement`: the service provider's order management.
    pub auftragsmanagement: Ansprechpartner,
    /// `ansprechpartner`: further contacts with their role.
    pub weitere: Vec<AnsprechpartnerMitRolle>,
}

/// A product from the product catalogue (`produkt`, FTTx: `ProduktFTTXType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Produkt {
    /// `bezeichner`.
    pub bezeichner: Produktbezeichner,
}

impl From<Produktbezeichner> for Produkt {
    fn from(bezeichner: Produktbezeichner) -> Self {
        Self { bezeichner }
    }
}

/// A change position (`AenderungPositionType`): a product added, changed or removed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AenderungPosition {
    /// `produkt`.
    pub produkt: Produkt,
    /// `aktionscode`.
    pub aktionscode: Aktionscode,
}

/// `NEU` (`BereitstellungType`) with the FTTx product.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bereitstellung {
    /// `vorabstimmungId`: the WBCI pre-agreement, when a provisioning replaces another
    /// provider's line (S/PRI 4.4 §5.1.4).
    pub vorabstimmungs_id: Option<VorabstimmungsId>,
    /// `anlage`.
    pub anlage: Option<Anlage>,
    /// `ansprechpartner`.
    pub ansprechpartner: AnsprechpartnerAuftrag,
    /// `termine/auftraggeberWunschtermin`.
    pub wunschtermin: Wunschtermin,
    /// `auftragsposition/produkt`.
    pub produkt: Produkt,
    /// `auftragsposition/geschaeftsfallProdukt` (`FTTXBereitstellungType`).
    pub fttx: FttxBereitstellung,
    /// `auftragsposition/position/produkt`: further products.
    pub positionen: Vec<Produkt>,
}

/// The FTTx data of a provisioning (`FTTXBereitstellungType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FttxBereitstellung {
    /// `standortA`: where the line goes.
    pub standort_a: StandortA,
    /// `standortVersand`: where hardware is shipped.
    pub standort_versand: Option<StandortVersand>,
    /// `vormieter`: the previous tenant, to find the line.
    pub vormieter: Option<Vormieter>,
    /// `montageleistung`: installation at the customer.
    pub montageleistung: Option<Montageleistung>,
    /// `WitaExtendedInfosGroup`.
    pub wita: WitaExtendedInfos,
}

/// The previous tenant (`FTTxVormieterType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Vormieter {
    /// `person`.
    pub person: Option<VormieterPerson>,
    /// `homeId/homeIdNummer`.
    pub home_id: Option<HomeId>,
    /// `seriennummerONT`.
    pub seriennummer_ont: Option<SeriennummerOnt>,
}

/// The previous tenant's name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VormieterPerson {
    /// `vorname`.
    pub vorname: Option<Vorname>,
    /// `nachname`.
    pub nachname: Nachname,
}

/// Installation at the customer (`MontageleistungType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Montageleistung {
    /// `ansprechpartner`: who lets the installer in.
    pub ansprechpartner: Ansprechpartner,
    /// `montagehinweis`.
    pub montagehinweis: Option<Montagehinweis>,
}

/// WITA references (`WitaInfosGroup`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct WitaInfos {
    /// `witaVertragsnummer`.
    pub wita_vertragsnummer: Option<WitaVertragsnummer>,
    /// `technologie`.
    pub technologie: Option<Technologie>,
}

/// WITA references, line search and number porting (`WitaExtendedInfosGroup`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct WitaExtendedInfos {
    /// `witaVertragsnummer`, `technologie`.
    pub wita: WitaInfos,
    /// `bestandssuche`: find the existing line by its number.
    pub bestandssuche: Option<OnkzRufnummer>,
    /// `rufnummernPortierung`.
    pub rufnummern_portierung: Option<RufnummernPortierung>,
    /// `abgebenderEKP`.
    pub abgebender_ekp: Option<bool>,
}

/// An area code and a number (`onkz`, `rufnummer`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OnkzRufnummer {
    /// `onkz`.
    pub onkz: Onkz,
    /// `rufnummer`.
    pub rufnummer: Rufnummer,
}

/// A block of PBX numbers (`zuPortierenderRufnummernblock`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Rufnummernblock {
    /// `rnrBlockVon`.
    pub von: RufnummernBlock,
    /// `rnrBlockBis`.
    pub bis: RufnummernBlock,
}

/// The numbers to port (`rufnummernPortierung`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RufnummernPortierung {
    /// `anlagenanschluss` or `einzelanschluss`.
    pub anschluss: PortierungAnschluss,
    /// `portierungskenner`.
    pub portierungskenner: Portierungskenner,
}

/// The numbers of a PBX or a single line.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PortierungAnschluss {
    /// `anlagenanschluss`: a PBX line.
    Anlage {
        /// `onkzDurchwahlAbfragestelle/onkz`.
        onkz: Onkz,
        /// `onkzDurchwahlAbfragestelle/durchwahlnummer`.
        durchwahlnummer: Durchwahlnummer,
        /// `onkzDurchwahlAbfragestelle/abfragestelle`.
        abfragestelle: Abfragestelle,
        /// `zuPortierenderRufnummernblock`: 1 to 5.
        bloecke: Vec<Rufnummernblock>,
    },
    /// `einzelanschluss/rufnummernliste`: 1 to 10 numbers.
    Rufnummern(Vec<OnkzRufnummer>),
    /// `einzelanschluss/alleRufnummern` (`J`): all numbers of the line.
    AlleRufnummern,
}

/// `KUE-AG` (`KuendigungType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kuendigung {
    /// `vertragsnummer`.
    pub vertragsnummer: Vertragsnummer,
    /// `lineId`.
    pub line_id: LineId,
    /// `vorabstimmungId`.
    pub vorabstimmungs_id: Option<VorabstimmungsId>,
    /// `anlage`.
    pub anlage: Option<Anlage>,
    /// `ansprechpartner`.
    pub ansprechpartner: AnsprechpartnerAuftrag,
    /// `termine/auftraggeberWunschtermin`.
    pub wunschtermin: Wunschtermin,
    /// `auftragsposition/geschaeftsfallProdukt` (`FTTXKuendigungType`): WITA references.
    pub fttx: Option<WitaInfos>,
}

/// `LAE` (`LeistungsaenderungType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leistungsaenderung {
    /// `vertragsnummer`.
    pub vertragsnummer: Vertragsnummer,
    /// `lineId`.
    pub line_id: LineId,
    /// `anlage`.
    pub anlage: Option<Anlage>,
    /// `ansprechpartner`.
    pub ansprechpartner: AnsprechpartnerAuftrag,
    /// `termine/auftraggeberWunschtermin`.
    pub wunschtermin: Wunschtermin,
    /// `auftragsposition/produkt`: the new product.
    pub produkt: Produkt,
    /// `auftragsposition/position`: changed positions (any number).
    pub positionen: Vec<AenderungPosition>,
}

/// `AEN-LMAE` (`LeistungsmerkmalAenderungType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeistungsmerkmalAenderung {
    /// `vertragsnummer`.
    pub vertragsnummer: Vertragsnummer,
    /// `lineId`.
    pub line_id: LineId,
    /// `anlage`.
    pub anlage: Option<Anlage>,
    /// `ansprechpartner`.
    pub ansprechpartner: AnsprechpartnerAuftrag,
    /// `termine/auftraggeberWunschtermin`.
    pub wunschtermin: Wunschtermin,
    /// `auftragsposition/produkt`.
    pub produkt: Produkt,
    /// `auftragsposition/position`: the changed features, at least one.
    pub positionen: Vec<AenderungPosition>,
}

/// `EST` (`EntstoerungType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entstoerung {
    /// `vertragsnummer`.
    pub vertragsnummer: Vertragsnummer,
    /// `lineId`.
    pub line_id: LineId,
    /// `anlage`.
    pub anlage: Option<Anlage>,
    /// `ansprechpartner`.
    pub ansprechpartner: AnsprechpartnerAuftrag,
    /// `kontaktEndkunde`.
    pub kontakt_endkunde: KontaktEndkunde,
    /// `termine/auftraggeberWunschtermin`.
    pub wunschtermin: Wunschtermin,
    /// `stoerungsNrAuftraggeber`: the service provider's ticket.
    pub stoerungs_nr_auftraggeber: StoerungsNr,
    /// `reklamation`: a complaint about an earlier repair.
    pub reklamation: Option<Reklamation>,
    /// `vorpruefungErfolgt`: the service provider checked the line first.
    pub vorpruefung_erfolgt: bool,
    /// `stoerungsbeschreibung`.
    pub stoerungsbeschreibung: Freitext,
    /// `expressentstoerung`.
    pub expressentstoerung: Option<bool>,
}

/// The end customer to contact for a repair (`AnsprechpartnerESTType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KontaktEndkunde {
    /// `anrede`.
    pub anrede: Anrede,
    /// `vorname`.
    pub vorname: Vorname,
    /// `nachname`.
    pub nachname: Nachname,
    /// `telefonnummer`.
    pub telefonnummer: Option<Telefonnummer>,
    /// `mobilfunknummer`.
    pub mobilfunknummer: Option<Telefonnummer>,
    /// `faxnummer`.
    pub faxnummer: Option<Telefonnummer>,
    /// `emailadresse`.
    pub emailadresse: Option<Emailadresse>,
    /// `kontaktaufnahmeErwuenscht`.
    pub kontaktaufnahme_erwuenscht: bool,
}

impl KontaktEndkunde {
    /// The contact as a [`Person`].
    #[must_use]
    pub fn person(&self) -> Person {
        Person { anrede: self.anrede, vorname: Some(self.vorname.clone()), nachname: self.nachname.clone() }
    }
}

/// A complaint about an earlier repair (`EntstoerungsReklamationType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Reklamation {
    /// `stoerungsNrLeistungserbringer`.
    pub stoerungs_nr_leistungserbringer: StoerungsNr,
    /// `reklamationsgrund`.
    pub reklamationsgrund: Freitext,
}

/// `PV` (`ProviderwechselType`): take over a line after a WBCI pre-agreement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Providerwechsel {
    /// `lineId`: the line to take over.
    pub line_id: LineId,
    /// `vorabstimmungId`: the WBCI pre-agreement — the same type as `wbci::VorabstimmungsId`.
    pub vorabstimmungs_id: VorabstimmungsId,
    /// `anlage`.
    pub anlage: Option<Anlage>,
    /// `ansprechpartner`.
    pub ansprechpartner: AnsprechpartnerAuftrag,
    /// `termine/auftraggeberWunschtermin`.
    pub wunschtermin: Wunschtermin,
    /// `auftragsposition/produkt`.
    pub produkt: Produkt,
    /// `auftragsposition/geschaeftsfallProdukt` (`FTTXProviderwechselType`).
    pub fttx: FttxProviderwechsel,
    /// `auftragsposition/position/produkt`: further products.
    pub positionen: Vec<Produkt>,
}

/// The FTTx data of a provider switch (`FTTXProviderwechselType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FttxProviderwechsel {
    /// `standortA`.
    pub standort_a: StandortA,
    /// `montageleistung`.
    pub montageleistung: Option<Montageleistung>,
}

/// `GET` and `SET` (`DiagnoseGetType`, `DiagnoseSetType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Diagnose {
    /// `vertragsnummer`.
    pub vertragsnummer: Vertragsnummer,
    /// `lineId`.
    pub line_id: LineId,
    /// `auftragsposition/parameter/operation`: the diagnostic operations, at least one.
    pub operationen: Vec<Diagnosename>,
}
