//! Messages (`meldung`) and the technical receipt (TEQ).
//!
//! Source: XSD 4.00.03 `oss-message.xsd` (`MeldungType`, the extensions of
//! `MeldungstypAbstractType` and their `meldungsattribute`) and `oss-type-complex.xsd`
//! (`MessageTEQType`, `MeldungspositionOhneAttributeType`, the provider-switch types).

use jiff::civil::Date;

use super::{Anlage, Ansprechpartner, AnsprechpartnerMitRolle, Termin, Wunschtermin};
use crate::{
    A10nsp, AbbmOutcome, AbbmPhase, Aenderungskennzeichen, Antwortcode, Antworttext, Auftraggebernummer, Diagnosename,
    Diagnosewert, Entstoerergebnis, ExterneAuftragsnummer, Freitext, GeschaeftsfallArt, GeschaeftsfallCode, JaNein,
    LineId, Meldungscode, Meldungstext, Meldungstyp, Port, Providername, Sendungsnummer, SeriennummerOnt, StoerungsNr,
    UebergabepunktKunde, Versandunternehmen, Vertragsnummer, Vlan, VorabstimmungsId, WitaMeldungscode,
    WitaMeldungstext, WitaVertragsnummer,
};

/// A message (`MeldungType`): which order it is about and what it says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meldung {
    /// `auftragstyp`.
    pub auftragstyp: Auftragstyp,
    /// `meldungstyp`: the concrete message, written as `xsi:type`.
    pub inhalt: Meldungsinhalt,
}

impl Meldung {
    /// The message type, e.g. `ABM`.
    #[must_use]
    pub const fn typ(&self) -> Meldungstyp {
        self.inhalt.typ()
    }

    /// The codes of all positions, e.g. `0030` in an ABM-PV.
    #[must_use]
    pub fn codes(&self) -> Vec<&Meldungscode> {
        self.inhalt.codes()
    }

    /// The order number the message refers to (`externeAuftragsnummer`, in every message type).
    #[must_use]
    pub const fn externe_auftragsnummer(&self) -> &ExterneAuftragsnummer {
        use Meldungsinhalt as I;
        match &self.inhalt {
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

    /// The Vorabstimmungs-ID the message carries, if its type has one and it is given
    /// (XSD 4.00.03: QEB, ABM, ABBM, ERLM, ENTM, VZM, TAM, MTAM, ZWM-LE, AKM-PV, RUEM-PV, ERLM-K).
    #[must_use]
    pub const fn vorabstimmungs_id(&self) -> Option<&VorabstimmungsId> {
        use Meldungsinhalt as I;
        match &self.inhalt {
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
}

/// The kind of order a message refers to (`AuftragstypType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Auftragstyp {
    /// `geschaeftsfall`.
    pub geschaeftsfall: GeschaeftsfallCode,
    /// `aenderungsKennzeichen`.
    pub aenderungskennzeichen: Aenderungskennzeichen,
    /// `geschaeftsfallart`.
    pub geschaeftsfallart: GeschaeftsfallArt,
}

/// The concrete message (`meldungstyp`, an extension of `MeldungstypAbstractType`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum Meldungsinhalt {
    /// `MeldungstypQEBType`.
    Qeb(Qeb),
    /// `MeldungstypABMType`.
    Abm(Abm),
    /// `MeldungstypABBMType`.
    Abbm(Abbm),
    /// `MeldungstypERLMType`.
    Erlm(Erlm),
    /// `MeldungstypENTMType`.
    Entm(Entm),
    /// `MeldungstypVZMType`.
    Vzm(Vzm),
    /// `MeldungstypTAMType`.
    Tam(Terminanforderung),
    /// `MeldungstypMTAMType`.
    Mtam(Terminanforderung),
    /// `MeldungstypKDLEType`.
    Kdle(Kdle),
    /// `MeldungstypZWM-LEType`.
    ZwmLe(ZwmLe),
    /// `MeldungstypZWM-AGType`.
    ZwmAg(ZwmAg),
    /// `MeldungstypTBK-LEType`.
    TbkLe(Tbk),
    /// `MeldungstypTBK-AGType`.
    TbkAg(Tbk),
    /// `MeldungstypAKM-PVType`.
    AkmPv(AkmPv),
    /// `MeldungstypRUEM-PVType`.
    RuemPv(RuemPv),
    /// `MeldungstypABM-PVType`.
    AbmPv(AbmPv),
    /// `MeldungstypERLM-PVType`.
    ErlmPv(ErlmPv),
    /// `MeldungstypENTM-PVType`.
    EntmPv(EntmPv),
    /// `MeldungstypABBM-PVType`.
    AbbmPv(AbbmPv),
    /// `MeldungstypERLM-KType`.
    ErlmK(ErlmK),
}

impl Meldungsinhalt {
    /// The message type.
    #[must_use]
    pub const fn typ(&self) -> Meldungstyp {
        match self {
            Self::Qeb(_) => Meldungstyp::Qeb,
            Self::Abm(_) => Meldungstyp::Abm,
            Self::Abbm(_) => Meldungstyp::Abbm,
            Self::Erlm(_) => Meldungstyp::Erlm,
            Self::Entm(_) => Meldungstyp::Entm,
            Self::Vzm(_) => Meldungstyp::Vzm,
            Self::Tam(_) => Meldungstyp::Tam,
            Self::Mtam(_) => Meldungstyp::Mtam,
            Self::Kdle(_) => Meldungstyp::Kdle,
            Self::ZwmLe(_) => Meldungstyp::ZwmLe,
            Self::ZwmAg(_) => Meldungstyp::ZwmAg,
            Self::TbkLe(_) => Meldungstyp::TbkLe,
            Self::TbkAg(_) => Meldungstyp::TbkAg,
            Self::AkmPv(_) => Meldungstyp::AkmPv,
            Self::RuemPv(_) => Meldungstyp::RuemPv,
            Self::AbmPv(_) => Meldungstyp::AbmPv,
            Self::ErlmPv(_) => Meldungstyp::ErlmPv,
            Self::EntmPv(_) => Meldungstyp::EntmPv,
            Self::AbbmPv(_) => Meldungstyp::AbbmPv,
            Self::ErlmK(_) => Meldungstyp::ErlmK,
        }
    }

    /// The codes of the message's positions, in order.
    #[must_use]
    pub fn codes(&self) -> Vec<&Meldungscode> {
        fn plain(p: &[Position]) -> Vec<&Meldungscode> {
            p.iter().map(|p| &p.code).collect()
        }
        fn wita(p: &[PositionWita]) -> Vec<&Meldungscode> {
            p.iter().map(|p| &p.position.code).collect()
        }
        match self {
            Self::Qeb(m) => m.positionen.iter().map(|p| &p.position.code).collect(),
            Self::Abm(m) => m.positionen.iter().map(|p| &p.position.position.code).collect(),
            Self::Abbm(m) => wita(&m.positionen),
            Self::Erlm(m) => wita(&m.positionen),
            Self::Vzm(m) => wita(&m.positionen),
            Self::Tam(m) | Self::Mtam(m) => wita(&m.positionen),
            Self::Entm(m) => plain(&m.positionen),
            Self::Kdle(m) => plain(&m.positionen),
            Self::ZwmLe(m) => plain(&m.positionen),
            Self::ZwmAg(m) => plain(&m.positionen),
            Self::TbkLe(m) | Self::TbkAg(m) => plain(&m.positionen),
            Self::AkmPv(m) => plain(&m.positionen),
            Self::RuemPv(m) => plain(&m.positionen),
            Self::AbmPv(m) => plain(&m.positionen),
            Self::ErlmPv(m) => plain(&m.positionen),
            Self::EntmPv(m) => plain(&m.positionen),
            Self::AbbmPv(m) => plain(&m.positionen),
            Self::ErlmK(m) => plain(&m.positionen),
        }
    }
}

/// A message position: a code and its text (`MeldungspositionOhneAttributeType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Position {
    /// `meldungscode`.
    pub code: Meldungscode,
    /// `meldungstext`.
    pub text: Meldungstext,
}

impl Position {
    /// A position with the code's official text (empty for an unknown code).
    #[must_use]
    pub fn new(code: Meldungscode) -> Self {
        let text = Meldungstext::new(code.text().unwrap_or("")).unwrap_or_else(|_| Meldungstext::empty());
        Self { code, text }
    }
}

/// A position with the WITA code and text it maps to
/// (`MeldungspositionOhneAttributeMitWitaErweiterungType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PositionWita {
    /// `meldungscode`, `meldungstext`.
    pub position: Position,
    /// `witaMeldungscode`.
    pub wita_meldungscode: Option<WitaMeldungscode>,
    /// `witaMeldungstext`.
    pub wita_meldungstext: Option<WitaMeldungstext>,
}

impl From<Position> for PositionWita {
    fn from(position: Position) -> Self {
        Self { position, wita_meldungscode: None, wita_meldungstext: None }
    }
}

/// A QEB position (`MeldungspositionType` with `MeldungspositionsattributeQEBType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PositionQeb {
    /// `meldungscode`, `meldungstext`.
    pub position: Position,
    /// `positionsattribute/ansprechpartnerLeistungserbringer`.
    pub ansprechpartner_leistungserbringer: Option<Ansprechpartner>,
}

impl From<Position> for PositionQeb {
    fn from(position: Position) -> Self {
        Self { position, ansprechpartner_leistungserbringer: None }
    }
}

/// An ABM position (`MeldungspositionMitWitaErweiterungType` with
/// `MeldungspositionsattributeABMType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PositionAbm {
    /// Code, text and WITA mapping.
    pub position: PositionWita,
    /// `positionsattribute/ansprechpartnerLeistungserbringer`.
    pub ansprechpartner_leistungserbringer: Option<Ansprechpartner>,
}

impl From<Position> for PositionAbm {
    fn from(position: Position) -> Self {
        Self { position: position.into(), ansprechpartner_leistungserbringer: None }
    }
}

/// The technical receipt (`MessageTEQType`), the `quittung` of both responses.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Teq {
    /// `meldungsTyp`: always `TEQ` in practice.
    pub meldungstyp: Meldungstyp,
    /// `externeAuftragsnummer`.
    pub externe_auftragsnummer: Option<ExterneAuftragsnummer>,
    /// `auftraggebernummer`.
    pub auftraggebernummer: Option<Auftraggebernummer>,
    /// `meldungspositionen/position`: at least one; `OK` or an error code (AH §6.3).
    pub positionen: Vec<Position>,
}

impl Teq {
    /// A `TEQ` receipt with the single position `code` (with its official text).
    #[must_use]
    pub fn new(
        code: Meldungscode,
        externe_auftragsnummer: Option<ExterneAuftragsnummer>,
        auftraggebernummer: Option<Auftraggebernummer>,
    ) -> Self {
        Self {
            meldungstyp: Meldungstyp::Teq,
            externe_auftragsnummer,
            auftraggebernummer,
            positionen: vec![Position::new(code)],
        }
    }

    /// Whether the receipt is positive (only `OK` positions).
    #[must_use]
    pub fn is_ok(&self) -> bool {
        !self.positionen.is_empty() && self.positionen.iter().all(|p| p.code.is_ok())
    }
}

/// The provider switch as announced to the losing provider (`ProviderwechselAnkuendigungType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderwechselAnkuendigung {
    /// `providername`: the gaining provider.
    pub providername: Providername,
    /// `uebernahmedatumGeplant`.
    pub uebernahmedatum_geplant: Date,
    /// `antwortfrist`: answer by this date.
    pub antwortfrist: Date,
}

/// The provider switch as confirmed to the losing provider (`ProviderwechselBestaetigungType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderwechselBestaetigung {
    /// `providername`: the gaining provider.
    pub providername: Providername,
    /// `uebernahmedatumVerbindlich`.
    pub uebernahmedatum_verbindlich: Date,
}

/// The losing provider's answer (`ProviderwechselRueckmeldungType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderwechselRueckmeldung {
    /// `providername`: the losing provider.
    pub providername: Option<Providername>,
    /// `zustimmungProviderwechsel`.
    pub zustimmung: JaNein,
    /// `antwortcode`.
    pub antwortcode: Option<Antwortcode>,
    /// `antworttext`.
    pub antworttext: Option<Antworttext>,
}

/// The line as confirmed (`FTTxAnschlussType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FttxAnschluss {
    /// `a10nsp`.
    pub a10nsp: A10nsp,
    /// `port`.
    pub port: Port,
    /// `s-vlan`.
    pub s_vlan: Option<Vlan>,
    /// `c-vlan`.
    pub c_vlan: Option<Vlan>,
    /// `lineId`.
    pub line_id: LineId,
    /// `uebergabepunktKunde`.
    pub uebergabepunkt_kunde: UebergabepunktKunde,
    /// `witaVertragsnummer`.
    pub wita_vertragsnummer: Option<WitaVertragsnummer>,
}

/// ONT shipping data (`FTTxVersandinfosONTType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VersandinfosOnt {
    /// `seriennummerONT`.
    pub seriennummer_ont: SeriennummerOnt,
    /// `versand/unternehmen`.
    pub unternehmen: Versandunternehmen,
    /// `versand/sendungsnummer`.
    pub sendungsnummer: Sendungsnummer,
}

/// The result of a repair (`EntstoerungErgebnisType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntstoerErgebnis {
    /// `ergebnis`.
    pub ergebnis: Entstoerergebnis,
    /// `entstoerZeitpunkt`.
    pub entstoer_zeitpunkt: Option<Date>,
    /// `anmerkung`.
    pub anmerkung: Option<Freitext>,
}

/// A diagnostic value (`DiagnoseErgebnisType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiagnoseErgebnis {
    /// `name`.
    pub name: Diagnosename,
    /// `wert`.
    pub wert: Diagnosewert,
}

/// `QEB`: the order is commercially admissible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Qeb {
    /// `externeAuftragsnummer`.
    pub externe_auftragsnummer: ExterneAuftragsnummer,
    /// `auftraggebernummer`.
    pub auftraggebernummer: Auftraggebernummer,
    /// `auftraggebernummerBesteller`.
    pub auftraggebernummer_besteller: Option<Auftraggebernummer>,
    /// `vorabstimmungId`.
    pub vorabstimmungs_id: Option<VorabstimmungsId>,
    /// `stoerungsNrLeistungserbringer`.
    pub stoerungs_nr_leistungserbringer: Option<StoerungsNr>,
    /// `meldungspositionen/position`: at least one.
    pub positionen: Vec<PositionQeb>,
}

/// `ABM`: the order is confirmed for a binding date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abm {
    /// `vertragsnummer`.
    pub vertragsnummer: Vertragsnummer,
    /// `externeAuftragsnummer`.
    pub externe_auftragsnummer: ExterneAuftragsnummer,
    /// `auftraggebernummer`.
    pub auftraggebernummer: Auftraggebernummer,
    /// `auftraggebernummerBesteller`.
    pub auftraggebernummer_besteller: Option<Auftraggebernummer>,
    /// `vorabstimmungId`.
    pub vorabstimmungs_id: Option<VorabstimmungsId>,
    /// `verbindlicherLiefertermin`.
    pub verbindlicher_liefertermin: Wunschtermin,
    /// `abgebenderProvider`.
    pub abgebender_provider: Option<ProviderwechselRueckmeldung>,
    /// `anschluss`.
    pub anschluss: FttxAnschluss,
    /// `meldungspositionen/position`: at least one.
    pub positionen: Vec<PositionAbm>,
}

/// `ABBM`: the order is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abbm {
    /// `vertragsnummer`.
    pub vertragsnummer: Option<Vertragsnummer>,
    /// `externeAuftragsnummer`.
    pub externe_auftragsnummer: ExterneAuftragsnummer,
    /// `auftraggebernummer`.
    pub auftraggebernummer: Auftraggebernummer,
    /// `auftraggebernummerBesteller`.
    pub auftraggebernummer_besteller: Option<Auftraggebernummer>,
    /// `vorabstimmungId`.
    pub vorabstimmungs_id: Option<VorabstimmungsId>,
    /// `wiedervorlagetermin`.
    pub wiedervorlagetermin: Option<Date>,
    /// `abgebenderProvider`.
    pub abgebender_provider: Option<ProviderwechselRueckmeldung>,
    /// `meldungspositionen/position`: at least one.
    pub positionen: Vec<PositionWita>,
}

impl Abbm {
    /// What this ABBM means for the service provider in business case `fall` and `phase`
    /// (Anl. 7 EPK V.13): the strongest outcome of its positions — closed before continues
    /// before resend; [`AbbmOutcome::Unknown`] when no position's code is drawn there.
    #[must_use]
    pub fn outcome(&self, fall: GeschaeftsfallCode, phase: AbbmPhase) -> AbbmOutcome {
        let all: Vec<AbbmOutcome> = self.positionen.iter().map(|p| p.position.code.abbm_outcome(fall, phase)).collect();
        [AbbmOutcome::Closed, AbbmOutcome::Continues, AbbmOutcome::Resend]
            .into_iter()
            .find(|o| all.contains(o))
            .unwrap_or(AbbmOutcome::Unknown)
    }
}

/// `ERLM`: the service is realised.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Erlm {
    /// `vertragsnummer`.
    pub vertragsnummer: Option<Vertragsnummer>,
    /// `externeAuftragsnummer`.
    pub externe_auftragsnummer: ExterneAuftragsnummer,
    /// `auftraggebernummer`.
    pub auftraggebernummer: Auftraggebernummer,
    /// `auftraggebernummerBesteller`.
    pub auftraggebernummer_besteller: Option<Auftraggebernummer>,
    /// `vorabstimmungId`.
    pub vorabstimmungs_id: Option<VorabstimmungsId>,
    /// `erledigungstermin`.
    pub erledigungstermin: Date,
    /// `entstoerErgebnis`.
    pub entstoer_ergebnis: Option<EntstoerErgebnis>,
    /// `anlage`.
    pub anlage: Option<Anlage>,
    /// `dignoseergebnis` (sic): the diagnostic values.
    pub diagnoseergebnisse: Vec<DiagnoseErgebnis>,
    /// `meldungspositionen/position`: at least one.
    pub positionen: Vec<PositionWita>,
}

/// `ENTM`: the billing date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entm {
    /// `vertragsnummer`.
    pub vertragsnummer: Vertragsnummer,
    /// `externeAuftragsnummer`.
    pub externe_auftragsnummer: ExterneAuftragsnummer,
    /// `auftraggebernummer`.
    pub auftraggebernummer: Auftraggebernummer,
    /// `auftraggebernummerBesteller`.
    pub auftraggebernummer_besteller: Option<Auftraggebernummer>,
    /// `vorabstimmungId`.
    pub vorabstimmungs_id: Option<VorabstimmungsId>,
    /// `entgelttermin`.
    pub entgelttermin: Date,
    /// `anlage`.
    pub anlage: Option<Anlage>,
    /// `meldungspositionen/position`: at least one.
    pub positionen: Vec<Position>,
}

/// `VZM`: processing is delayed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vzm {
    /// `vertragsnummer`.
    pub vertragsnummer: Option<Vertragsnummer>,
    /// `externeAuftragsnummer`.
    pub externe_auftragsnummer: ExterneAuftragsnummer,
    /// `auftraggebernummer`.
    pub auftraggebernummer: Auftraggebernummer,
    /// `auftraggebernummerBesteller`.
    pub auftraggebernummer_besteller: Option<Auftraggebernummer>,
    /// `vorabstimmungId`.
    pub vorabstimmungs_id: Option<VorabstimmungsId>,
    /// `verzoegerungstermin`.
    pub verzoegerungstermin: Option<Date>,
    /// `verzoegerungsgrund`.
    pub verzoegerungsgrund: Option<Freitext>,
    /// `meldungspositionen/position`: at least one.
    pub positionen: Vec<PositionWita>,
}

/// `TAM` and `MTAM`: a new appointment with the end customer is needed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Terminanforderung {
    /// `vertragsnummer`.
    pub vertragsnummer: Vertragsnummer,
    /// `externeAuftragsnummer`.
    pub externe_auftragsnummer: ExterneAuftragsnummer,
    /// `auftraggebernummer`.
    pub auftraggebernummer: Auftraggebernummer,
    /// `auftraggebernummerBesteller`.
    pub auftraggebernummer_besteller: Option<Auftraggebernummer>,
    /// `vorabstimmungId`.
    pub vorabstimmungs_id: Option<VorabstimmungsId>,
    /// `meldungspositionen/position`: at least one.
    pub positionen: Vec<PositionWita>,
}

/// `KDLE`: the network owner announces the cancellation of a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kdle {
    /// `vertragsnummer`.
    pub vertragsnummer: Vertragsnummer,
    /// `externeAuftragsnummer`.
    pub externe_auftragsnummer: ExterneAuftragsnummer,
    /// `auftraggebernummer`.
    pub auftraggebernummer: Auftraggebernummer,
    /// `abschaltungstermin`.
    pub abschaltungstermin: Date,
    /// `abschaltungsgrund`.
    pub abschaltungsgrund: Option<Freitext>,
    /// `meldungspositionen/position`: at least one.
    pub positionen: Vec<Position>,
}

/// `ZWM-LE`: an interim message from the network owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZwmLe {
    /// `vertragsnummer`.
    pub vertragsnummer: Vertragsnummer,
    /// `externeAuftragsnummer`.
    pub externe_auftragsnummer: ExterneAuftragsnummer,
    /// `auftraggebernummer`.
    pub auftraggebernummer: Auftraggebernummer,
    /// `auftraggebernummerBesteller`.
    pub auftraggebernummer_besteller: Option<Auftraggebernummer>,
    /// `vorabstimmungId`.
    pub vorabstimmungs_id: Option<VorabstimmungsId>,
    /// `nachricht`.
    pub nachricht: Freitext,
    /// `ansprechpartner`.
    pub ansprechpartner: Option<AnsprechpartnerMitRolle>,
    /// `anlage`.
    pub anlage: Option<Anlage>,
    /// `versandONT`.
    pub versand_ont: Option<VersandinfosOnt>,
    /// `meldungspositionen/position`: at least one.
    pub positionen: Vec<Position>,
}

/// `ZWM-AG`: an interim message from the service provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZwmAg {
    /// `vertragsnummer`.
    pub vertragsnummer: Vertragsnummer,
    /// `externeAuftragsnummer`.
    pub externe_auftragsnummer: ExterneAuftragsnummer,
    /// `auftraggebernummer`.
    pub auftraggebernummer: Auftraggebernummer,
    /// `auftraggebernummerBesteller`.
    pub auftraggebernummer_besteller: Option<Auftraggebernummer>,
    /// `nachricht`.
    pub nachricht: Freitext,
    /// `ansprechpartner`.
    pub ansprechpartner: Option<AnsprechpartnerMitRolle>,
    /// `anlage`.
    pub anlage: Option<Anlage>,
    /// `meldungspositionen/position`: at least one.
    pub positionen: Vec<Position>,
}

/// `TBK-LE` and `TBK-AG`: an on-site appointment was agreed with the end customer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tbk {
    /// `vertragsnummer`.
    pub vertragsnummer: Vertragsnummer,
    /// `externeAuftragsnummer`.
    pub externe_auftragsnummer: ExterneAuftragsnummer,
    /// `auftraggebernummer`.
    pub auftraggebernummer: Auftraggebernummer,
    /// `auftraggebernummerBesteller`.
    pub auftraggebernummer_besteller: Option<Auftraggebernummer>,
    /// `termin`.
    pub termin: Termin,
    /// `meldungspositionen/position`: at least one.
    pub positionen: Vec<Position>,
}

/// `AKM-PV`: the network owner tells the losing provider about a provider switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AkmPv {
    /// `vertragsnummer`: the losing provider's contract.
    pub vertragsnummer: Vertragsnummer,
    /// `externeAuftragsnummer`.
    pub externe_auftragsnummer: ExterneAuftragsnummer,
    /// `auftraggebernummer`: the losing provider.
    pub auftraggebernummer: Auftraggebernummer,
    /// `vorabstimmungId`.
    pub vorabstimmungs_id: Option<VorabstimmungsId>,
    /// `anschluss/lineId`.
    pub line_id: LineId,
    /// `anlage`.
    pub anlage: Option<Anlage>,
    /// `aufnehmenderProvider`.
    pub aufnehmender_provider: ProviderwechselAnkuendigung,
    /// `meldungspositionen/position`: at least one.
    pub positionen: Vec<Position>,
}

/// `RUEM-PV`: the losing provider agrees to or refuses the switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuemPv {
    /// `vertragsnummer`.
    pub vertragsnummer: Vertragsnummer,
    /// `externeAuftragsnummer`.
    pub externe_auftragsnummer: ExterneAuftragsnummer,
    /// `auftraggebernummer`.
    pub auftraggebernummer: Auftraggebernummer,
    /// `vorabstimmungId`.
    pub vorabstimmungs_id: Option<VorabstimmungsId>,
    /// `anschluss/lineId`.
    pub line_id: LineId,
    /// `abgebenderProvider`.
    pub abgebender_provider: ProviderwechselRueckmeldung,
    /// `meldungspositionen/position`: at least one.
    pub positionen: Vec<Position>,
}

/// `ABM-PV`: the switch date is confirmed to the losing provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbmPv {
    /// `vertragsnummer`.
    pub vertragsnummer: Vertragsnummer,
    /// `externeAuftragsnummer`.
    pub externe_auftragsnummer: ExterneAuftragsnummer,
    /// `auftraggebernummer`.
    pub auftraggebernummer: Auftraggebernummer,
    /// `anschluss/lineId`.
    pub line_id: LineId,
    /// `aufnehmenderProvider`.
    pub aufnehmender_provider: ProviderwechselBestaetigung,
    /// `meldungspositionen/position`: at least one.
    pub positionen: Vec<Position>,
}

/// `ERLM-PV`: the losing provider's line was technically taken over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErlmPv {
    /// `vertragsnummer`.
    pub vertragsnummer: Vertragsnummer,
    /// `externeAuftragsnummer`.
    pub externe_auftragsnummer: ExterneAuftragsnummer,
    /// `auftraggebernummer`.
    pub auftraggebernummer: Auftraggebernummer,
    /// `erledigungstermin`.
    pub erledigungstermin: Date,
    /// `meldungspositionen/position`: at least one.
    pub positionen: Vec<Position>,
}

/// `ENTM-PV`: the losing provider's billing ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntmPv {
    /// `vertragsnummer`.
    pub vertragsnummer: Vertragsnummer,
    /// `externeAuftragsnummer`.
    pub externe_auftragsnummer: ExterneAuftragsnummer,
    /// `auftraggebernummer`.
    pub auftraggebernummer: Auftraggebernummer,
    /// `entgelttermin`.
    pub entgelttermin: Date,
    /// `meldungspositionen/position`: at least one.
    pub positionen: Vec<Position>,
}

/// `ABBM-PV`: the switch was aborted; the losing provider's contract stays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbbmPv {
    /// `vertragsnummer`.
    pub vertragsnummer: Option<Vertragsnummer>,
    /// `externeAuftragsnummer`.
    pub externe_auftragsnummer: ExterneAuftragsnummer,
    /// `auftraggebernummer`.
    pub auftraggebernummer: Auftraggebernummer,
    /// `meldungspositionen/position`: at least one.
    pub positionen: Vec<Position>,
}

/// `ERLM-K`: the service provider reports the service done without an appointment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErlmK {
    /// `vertragsnummer`.
    pub vertragsnummer: Vertragsnummer,
    /// `externeAuftragsnummer`.
    pub externe_auftragsnummer: ExterneAuftragsnummer,
    /// `auftraggebernummer`.
    pub auftraggebernummer: Auftraggebernummer,
    /// `auftraggebernummerBesteller`.
    pub auftraggebernummer_besteller: Option<Auftraggebernummer>,
    /// `vorabstimmungId`.
    pub vorabstimmungs_id: Option<VorabstimmungsId>,
    /// `meldungspositionen/position`: at least one.
    pub positionen: Vec<Position>,
}

macro_rules! into_meldungsinhalt {
    ($($ty:ident => $variant:ident),+) => {
        $(impl From<$ty> for Meldungsinhalt {
            fn from(m: $ty) -> Self {
                Self::$variant(m)
            }
        })+
    };
}

into_meldungsinhalt!(
    Qeb => Qeb, Abm => Abm, Abbm => Abbm, Erlm => Erlm, Entm => Entm, Vzm => Vzm, Kdle => Kdle,
    ZwmLe => ZwmLe, ZwmAg => ZwmAg, AkmPv => AkmPv, RuemPv => RuemPv, AbmPv => AbmPv,
    ErlmPv => ErlmPv, EntmPv => EntmPv, AbbmPv => AbbmPv, ErlmK => ErlmK
);
