//! Business cases, message types and the S/PRI Meldungscode table.
//!
//! Sources: AH4S/PRI 2.9.5 §6.3 (code list with texts) and §6.4 (fact sheets: message types
//! and the business cases each code is used with); XSD 4.00.03 `oss-type-enum.xsd`
//! (`GeschaeftsfallMeldungType`, `MeldungstypType`). The table is harmonised with WITA from
//! AH 2.9 on; codes the online list (Anlage 5) adds later are read as [`Meldungscode::Other`].

use core::fmt;

carrier_core::code_enum!(
    /// The business case a message answers. XSD `GeschaeftsfallMeldungType`.
    ///
    /// The eight order cases plus `KUE-LE`, the network owner's own cancellation.
    GeschaeftsfallCode {
        /// `NEU` — Bereitstellung (provisioning).
        Neu = "NEU",
        /// `KUE-AG` — Kündigung durch den Auftraggeber (cancellation by the service provider).
        KueAg = "KUE-AG",
        /// `AEN-LMAE` — Änderung von Leistungsmerkmalen (feature change).
        AenLmae = "AEN-LMAE",
        /// `LAE` — Änderung von Leistungen (service change).
        Lae = "LAE",
        /// `EST` — Entstörung (fault repair).
        Est = "EST",
        /// `PV` — Providerwechsel (provider switch).
        Pv = "PV",
        /// `KUE-LE` — Kündigung durch den Leistungserbringer (cancellation by the network owner).
        KueLe = "KUE-LE",
        /// `GET` — Diagnose GET.
        Get = "GET",
        /// `SET` — Diagnose SET.
        Set = "SET",
    }
);

carrier_core::code_enum!(
    /// The type of a message. XSD `MeldungstypType`; descriptions AH4S/PRI 2.9.5 §6.2.
    Meldungstyp {
        /// `TEQ` — Technische Quittung: the synchronous technical receipt.
        Teq = "TEQ",
        /// `QEB` — Qualifizierte Eingangsbestätigung: the order is commercially admissible.
        Qeb = "QEB",
        /// `ABM` — Auftragsbestätigungsmeldung: the order is confirmed for a date.
        Abm = "ABM",
        /// `ERLM` — Erledigungsmeldung: the service is technically realised.
        Erlm = "ERLM",
        /// `ENTM` — Entgeltmeldung: the billing date.
        Entm = "ENTM",
        /// `ABBM` — Abbruchmeldung: the order is refused and processing stops.
        Abbm = "ABBM",
        /// `VZM` — Verzögerungsmeldung: processing takes longer.
        Vzm = "VZM",
        /// `TAM` — Terminanforderungsmeldung: a new appointment with the end customer is needed.
        Tam = "TAM",
        /// `MTAM` — Mahn-Terminanforderungsmeldung: second and last request for an appointment.
        Mtam = "MTAM",
        /// `KDLE` — Kündigung durch den Leistungserbringer (announcement).
        Kdle = "KDLE",
        /// `ZWM-LE` — Zwischenmeldung of the network owner.
        ZwmLe = "ZWM-LE",
        /// `ZWM-AG` — Zwischenmeldung of the service provider.
        ZwmAg = "ZWM-AG",
        /// `TBK-LE` — Termin beim Kunden, from the network owner.
        TbkLe = "TBK-LE",
        /// `TBK-AG` — Termin beim Kunden, from the service provider.
        TbkAg = "TBK-AG",
        /// `AKM-PV` — Ankündigungsmeldung to the losing provider of a provider switch.
        AkmPv = "AKM-PV",
        /// `RUEM-PV` — Rückmeldung of the losing provider to the AKM-PV.
        RuemPv = "RUEM-PV",
        /// `ABM-PV` — Auftragsbestätigung to the losing provider.
        AbmPv = "ABM-PV",
        /// `ERLM-PV` — Erledigungsmeldung to the losing provider.
        ErlmPv = "ERLM-PV",
        /// `ENTM-PV` — Entgeltmeldung to the losing provider.
        EntmPv = "ENTM-PV",
        /// `ABBM-PV` — Abbruchmeldung to the losing provider: its contract stays as it is.
        AbbmPv = "ABBM-PV",
        /// `ERLM-K` — Erledigungsmeldung Kunde: the service provider reports the service done.
        ErlmK = "ERLM-K",
    }
);

use GeschaeftsfallCode as G;
use Meldungstyp as T;

const ALL: &[G] = G::ALL;

/// One row of the code table.
struct Row {
    code: &'static str,
    text: &'static str,
    typen: &'static [Meldungstyp],
    faelle: &'static [GeschaeftsfallCode],
}

macro_rules! codes {
    ($($(#[$m:meta])* $variant:ident = $code:literal, $text:literal, [$($ty:expr),*], $gf:expr;)+) => {
        /// A message code (Meldungscode) with its official text.
        ///
        /// The 215 codes of AH4S/PRI 2.9.5 §6.3 are variants named after the code (`C1001`);
        /// the technical receipt codes also have names ([`Meldungscode::VERSION_FEHLERHAFT`],
        /// …). A code a peer sends that the list does not contain is kept as
        /// [`Meldungscode::Other`] (the schema admits any 1–10 characters).
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum Meldungscode {
            $($(#[$m])* $variant,)+
            /// A code not in the published list.
            Other(Box<str>),
        }

        impl Meldungscode {
            fn row(&self) -> Option<&'static Row> {
                match self {
                    $(Self::$variant => Some(&Row { code: $code, text: $text, typen: &[$($ty),*], faelle: $gf }),)+
                    Self::Other(_) => None,
                }
            }

            /// Every published code, in the order of AH §6.3.
            pub const ALL: &'static [Meldungscode] = &[$(Self::$variant),+];

            fn from_known(s: &str) -> Option<Self> {
                match s { $($code => Some(Self::$variant),)+ _ => None }
            }
        }
    };
}

codes! {
    /// `OK` — OK.
    Ok = "OK", "OK.", [T::Teq], ALL;
    /// `0000` — Keine Änderung zum Auftrag.
    C0000 = "0000", "Keine Änderung zum Auftrag.", [T::Qeb, T::Abm], &[G::Neu, G::KueAg, G::AenLmae, G::Lae, G::Pv, G::Est];
    /// `0001` — Eine Aktivierung am VLT dieser ABM ist nicht möglich. Es müssen erst weitere Netztechnische Arbeiten ausgeführt werden.
    C0001 = "0001", "Eine Aktivierung am VLT dieser ABM ist nicht möglich. Es müssen erst weitere Netztechnische Arbeiten ausgeführt werden.", [T::Abm], &[G::Neu, G::KueAg, G::AenLmae, G::Lae, G::Pv, G::Est];
    /// `0002` — Der gewünschte Schalttermin liegt außerhalb des betrieblich möglichen Zeitrahmens oder ist kein vertraglicher Arbeitstag.
    C0002 = "0002", "Der gewünschte Schalttermin liegt außerhalb des betrieblich möglichen Zeitrahmens oder ist kein vertraglicher Arbeitstag.", [T::Abm], &[G::Neu, G::KueAg, G::AenLmae, G::Lae, G::Pv];
    /// `0003` — Der Leistungserbringer hat zum gewünschten Schalttermin keine ausreichenden Kapazitäten.
    C0003 = "0003", "Der Leistungserbringer hat zum gewünschten Schalttermin keine ausreichenden Kapazitäten.", [T::Abm], &[G::Neu, G::KueAg, G::AenLmae, G::Lae, G::Pv];
    /// `0005` — Der Ausführungstermin wurde vom Leistungserbringer manuell geändert.
    C0005 = "0005", "Der Ausführungstermin wurde vom Leistungserbringer manuell geändert.", [T::Abm], &[G::Neu, G::KueAg, G::AenLmae, G::Lae, G::Pv];
    /// `0007` — Neuterminierung durch/mit Endkunde.
    C0007 = "0007", "Neuterminierung durch/mit Endkunde.", [T::Abm], &[G::Neu];
    /// `0009` — Der Bau der Endleitung/ Konnektivität wurde hergestellt.
    C0009 = "0009", "Der Bau der Endleitung/ Konnektivität wurde hergestellt.", [T::Abm], &[G::Neu];
    /// `0010` — Auftrag ausgeführt.
    C0010 = "0010", "Auftrag ausgeführt.", [T::Erlm, T::Entm, T::ErlmPv, T::EntmPv], ALL;
    /// `0011` — Montage beim Endkunden erforderlich.
    C0011 = "0011", "Montage beim Endkunden erforderlich.", [T::Abm], &[G::Neu, G::Lae, G::Pv];
    /// `0012` — Stornierung erfolgreich.
    C0012 = "0012", "Stornierung erfolgreich.", [T::Erlm], &[G::Neu, G::KueAg, G::AenLmae, G::Lae, G::Pv, G::Est];
    /// `0013` — Montage beim Endkunden inkl. Bau der Endleitung erforderlich.
    C0013 = "0013", "Montage beim Endkunden inkl. Bau der Endleitung erforderlich.", [T::Abm], &[G::Neu, G::Lae, G::Pv];
    /// `0014` — Montage beim Endkunden mit Ende-zu-Ende-Prüfung erfolgreich ausgeführt.
    C0014 = "0014", "Montage beim Endkunden mit Ende-zu-Ende-Prüfung erfolgreich ausgeführt.", [T::Erlm, T::ErlmPv], &[G::Neu, G::AenLmae, G::Lae, G::Pv, G::Est];
    /// `0015` — Endkunde hat die Bereitstellung der Leistung bestätigt. Anschluss störungsfrei in Betrieb.
    C0015 = "0015", "Endkunde hat die Bereitstellung der Leistung bestätigt. Anschluss störungsfrei in Betrieb.", [T::ErlmK], &[G::Neu, G::Lae, G::Pv];
    /// `0016` — Die vereinbarte Leistung wird nur eingeschränkt bereitgestellt.
    C0016 = "0016", "Die vereinbarte Leistung wird nur eingeschränkt bereitgestellt.", [T::Erlm, T::ErlmPv], &[G::Neu, G::AenLmae, G::Lae, G::Pv, G::Est];
    /// `0020` — Es liegt uns ein Auftrag zum Anbieterwechsel vor. Wir bitten um Antwort. Erhalten wir keine Antwort, wird der Auftrag abgebrochen.
    C0020 = "0020", "Es liegt uns ein Auftrag zum Anbieterwechsel vor. Wir bitten um Antwort. Erhalten wir keine Antwort, wird der Auftrag abgebrochen.", [T::AkmPv], &[G::Pv];
    /// `0021` — Antwort des abgebenden Providers.
    C0021 = "0021", "Antwort des abgebenden Providers.", [T::RuemPv], &[G::Pv];
    /// `0030` — Der Anbieterwechsel erfolgt zum angegebenen Termin.
    C0030 = "0030", "Der Anbieterwechsel erfolgt zum angegebenen Termin.", [T::AbmPv], &[G::Pv];
    /// `0040` — Der Auftrag zum Wechsel wurde zurückgezogen bzw. abgebrochen. Ihr Vertrag bleibt unverändert bestehen.
    C0040 = "0040", "Der Auftrag zum Wechsel wurde zurückgezogen bzw. abgebrochen. Ihr Vertrag bleibt unverändert bestehen.", [T::AbbmPv], &[G::Pv];
    /// `0041` — Der abgebende Provider hat die Kündigung seines Bestandes abgelehnt.
    C0041 = "0041", "Der abgebende Provider hat die Kündigung seines Bestandes abgelehnt.", [T::Abbm], &[G::Pv];
    /// `0042` — Der abgebende Provider hat auf die Ankündigungsmeldung zum Providerwechsel nicht geantwortet.
    C0042 = "0042", "Der abgebende Provider hat auf die Ankündigungsmeldung zum Providerwechsel nicht geantwortet.", [T::Abbm], &[G::Pv];
    /// `0044` — Das beim Vorleister beauftragte Produkt wurde abgelehnt.
    C0044 = "0044", "Das beim Vorleister beauftragte Produkt wurde abgelehnt.", [T::Abbm], &[G::Neu];
    /// `0070` — Der Auftrag wird manuell bearbeitet. Die Bearbeitungszeit kann sich daher verlängern.
    C0070 = "0070", "Der Auftrag wird manuell bearbeitet. Die Bearbeitungszeit kann sich daher verlängern.", [T::Vzm], &[G::Neu, G::KueAg, G::AenLmae, G::Lae, G::Pv, G::Est];
    /// `0073` — Information zum Auftrag
    C0073 = "0073", "Information zum Auftrag", [T::ZwmLe], &[G::Neu, G::Est];
    /// `0074` — Der Auftrag befindet sich weiter in Bearbeitung.
    C0074 = "0074", "Der Auftrag befindet sich weiter in Bearbeitung.", [T::ZwmLe], &[G::Neu, G::Est];
    /// `0075` — Der Leistungserbringer wartet weiter auf den Ausführungs-, Endkundentermin.
    C0075 = "0075", "Der Leistungserbringer wartet weiter auf den Ausführungs-, Endkundentermin.", [T::ZwmLe], &[G::Neu, G::Est];
    /// `0990` — Funktion aktuell nicht unterschützt, Freigabe wird gesondert bekannt gegeben.
    C0990 = "0990", "Funktion aktuell nicht unterschützt, Freigabe wird gesondert bekannt gegeben.", [T::Teq], ALL;
    /// `0991` — Die Schnittstelle ist derzeit überlastet, bitte versuchen Sie es zu einem späteren Zeitpunkt erneut.
    C0991 = "0991", "Die Schnittstelle ist derzeit überlastet, bitte versuchen Sie es zu einem späteren Zeitpunkt erneut.", [T::Teq], ALL;
    /// `0993` — Versionsnummer fehlerhaft.
    C0993 = "0993", "Versionsnummer fehlerhaft.", [T::Teq], ALL;
    /// `0995` — Datenstruktur unzulässig.
    C0995 = "0995", "Datenstruktur unzulässig.", [T::Teq], ALL;
    /// `0999` — Technischer Fehler.
    C0999 = "0999", "Technischer Fehler.", [T::Teq], ALL;
    /// `1001` — Die Anschrift ist nicht bekannt.
    C1001 = "1001", "Die Anschrift ist nicht bekannt.", [T::Abbm], &[G::Neu];
    /// `1003` — Der Kundenwunschtermin liegt außerhalb des zulässigen Zeitrahmens.
    C1003 = "1003", "Der Kundenwunschtermin liegt außerhalb des zulässigen Zeitrahmens.", [T::Abbm], &[G::Neu, G::KueAg, G::AenLmae, G::Lae, G::Pv];
    /// `1006` — Der Auftrag ist derzeit nicht möglich. Es wurden Änderungen am Anschluss beauftragt. Das beauftragte Produkt kann erst nach Erledigung dieses Auftrags erteilt werden.
    C1006 = "1006", "Der Auftrag ist derzeit nicht möglich. Es wurden Änderungen am Anschluss beauftragt. Das beauftragte Produkt kann erst nach Erledigung dieses Auftrags erteilt werden.", [T::Abbm], &[G::Neu, G::KueAg, G::AenLmae, G::Lae];
    /// `1007` — Bei dem vorliegenden Geschäftsfall ist das Änderungskennzeichen für dieses Produkt nicht zulässig.
    C1007 = "1007", "Bei dem vorliegenden Geschäftsfall ist das Änderungskennzeichen für dieses Produkt nicht zulässig.", [T::Abbm], &[G::Neu, G::KueAg, G::AenLmae, G::Lae, G::Pv, G::Est];
    /// `1008` — Die bestellte Konfiguration besteht bereits.
    C1008 = "1008", "Die bestellte Konfiguration besteht bereits.", [T::Abbm], &[G::AenLmae, G::Lae];
    /// `1009` — Das Leitungsmerkmal ist nicht möglich.
    C1009 = "1009", "Das Leitungsmerkmal ist nicht möglich.", [T::Abbm], &[G::Neu, G::AenLmae, G::Lae, G::Pv];
    /// `1010` — Es ist bereits Bestand des Auftraggebers vorhanden.
    C1010 = "1010", "Es ist bereits Bestand des Auftraggebers vorhanden.", [T::Abbm], &[G::Neu, G::AenLmae, G::Lae, G::Pv];
    /// `1011` — Es ist bereits Bestand eines anderen Providers vorhanden.
    C1011 = "1011", "Es ist bereits Bestand eines anderen Providers vorhanden.", [T::Abbm], &[G::Neu];
    /// `1012` — Es liegt bereits ein Auftrag für diesen Anschluss vor. Ihr Auftrag kann erst nach Erledigung des noch offenen Auftrags erteilt werden. Das Erledigungsdatum wird zusätzlich übergeben.
    C1012 = "1012", "Es liegt bereits ein Auftrag für diesen Anschluss vor. Ihr Auftrag kann erst nach Erledigung des noch offenen Auftrags erteilt werden. Das Erledigungsdatum wird zusätzlich übergeben.", [T::Abbm], &[G::Neu, G::KueAg, G::AenLmae, G::Lae, G::Pv, G::Est, G::Get, G::Set];
    /// `1013` — Externe Auftragsnummer ist nicht eindeutig oder nicht korrekt.
    C1013 = "1013", "Externe Auftragsnummer ist nicht eindeutig oder nicht korrekt.", [T::Abbm], &[G::Neu, G::KueAg, G::AenLmae, G::Lae, G::Pv, G::Est, G::Get, G::Set];
    /// `1014` — Die Vertragsnummer ist unbekannt.
    C1014 = "1014", "Die Vertragsnummer ist unbekannt.", [T::Abbm], &[G::KueAg, G::AenLmae, G::Lae, G::Est, G::Get, G::Set];
    /// `1018` — Der Besteller ist nicht berechtigt für den Kunden zu bestellen.
    C1018 = "1018", "Der Besteller ist nicht berechtigt für den Kunden zu bestellen.", [T::Abbm], ALL;
    /// `1020` — Das Zeitfenster ist nicht vereinbar.
    C1020 = "1020", "Das Zeitfenster ist nicht vereinbar.", [T::Abbm], &[G::Neu, G::KueAg, G::AenLmae, G::Lae, G::Pv, G::Est];
    /// `1023` — Ihr Auftrag ist derzeit aus technischen Gründen nicht bearbeitbar. Bitte versuchen Sie es zu einem späteren Zeitpunkt erneut.
    C1023 = "1023", "Ihr Auftrag ist derzeit aus technischen Gründen nicht bearbeitbar. Bitte versuchen Sie es zu einem späteren Zeitpunkt erneut.", [T::Abbm], &[G::Neu, G::KueAg, G::AenLmae, G::Lae, G::Pv, G::Get, G::Set];
    /// `1031` — Die Anlage enthält einen maliziösen Code.
    C1031 = "1031", "Die Anlage enthält einen maliziösen Code.", [T::Abbm], &[G::Neu, G::KueAg, G::AenLmae, G::Lae, G::Pv, G::Est];
    /// `1032` — Zeitfenster passt nicht zum KWT.
    C1032 = "1032", "Zeitfenster passt nicht zum KWT.", [T::Abbm], &[G::Neu, G::KueAg, G::AenLmae, G::Lae, G::Pv];
    /// `1034` — Rufnummer und/oder E-Mailadresse des Endkunden fehlen oder sind falsch angegeben.
    C1034 = "1034", "Rufnummer und/oder E-Mailadresse des Endkunden fehlen oder sind falsch angegeben.", [T::Abbm], &[G::Neu, G::Pv];
    /// `1050` — Bestandsvalidierung fehlgeschlagen.
    C1050 = "1050", "Bestandsvalidierung fehlgeschlagen.", [T::Abbm], &[G::Neu, G::KueAg, G::AenLmae, G::Lae, G::Est, G::Get, G::Set];
    /// `1051` — Der Auftrag ist bei vorhandener Bestandssituation nicht möglich.
    C1051 = "1051", "Der Auftrag ist bei vorhandener Bestandssituation nicht möglich.", [T::Abbm], &[G::AenLmae, G::Lae];
    /// `1085` — Eine Terminverschiebung ist nicht möglich.
    C1085 = "1085", "Eine Terminverschiebung ist nicht möglich.", [T::Abbm], &[G::Neu, G::KueAg, G::Pv];
    /// `1086` — Kundenauftrag ist nicht stornierbar, da Stornierung bereits angestoßen.
    C1086 = "1086", "Kundenauftrag ist nicht stornierbar, da Stornierung bereits angestoßen.", [T::Abbm], &[G::Neu, G::KueAg, G::AenLmae, G::Lae, G::Pv];
    /// `1087` — Eine Stornierung ist nicht möglich.
    C1087 = "1087", "Eine Stornierung ist nicht möglich.", [T::Abbm], &[G::Neu, G::KueAg, G::AenLmae, G::Lae, G::Pv];
    /// `1101` — Produkt an diesem Anschlusstyp nicht möglich.
    C1101 = "1101", "Produkt an diesem Anschlusstyp nicht möglich.", [T::Abbm], &[G::Neu, G::Lae];
    /// `1102` — Produkt oder Geschäftsfall nicht zulässig.
    C1102 = "1102", "Produkt oder Geschäftsfall nicht zulässig.", [T::Abbm], &[G::Neu, G::AenLmae, G::Lae, G::Pv];
    /// `1104` — Das Produkt, auf das sich der Auftrag bezieht, liegt im Bestand nicht vor.
    C1104 = "1104", "Das Produkt, auf das sich der Auftrag bezieht, liegt im Bestand nicht vor.", [T::Abbm], &[G::AenLmae, G::Lae];
    /// `1105` — Das Leistungsmerkmal ist mit dem Produkt nicht kombinierbar.
    C1105 = "1105", "Das Leistungsmerkmal ist mit dem Produkt nicht kombinierbar.", [T::Abbm], &[G::Neu, G::AenLmae, G::Lae, G::Pv];
    /// `1192` — Ein Anbieterwechsel wird der Kündigung vorgezogen.
    C1192 = "1192", "Ein Anbieterwechsel wird der Kündigung vorgezogen.", [T::Abbm], &[G::KueAg];
    /// `1194` — Der Endkunde verweigert die Herstellung.
    C1194 = "1194", "Der Endkunde verweigert die Herstellung.", [T::Abbm], &[G::Neu];
    /// `1198` — Es wurde kein neuer Ausführungstermin übermittelt.
    C1198 = "1198", "Es wurde kein neuer Ausführungstermin übermittelt.", [T::Abbm], &[G::Neu, G::Lae, G::Pv];
    /// `1199` — Die Voraussetzung für die Bereitstellung ist nicht mehr gegeben. Der Auftrag wurde abgebrochen.
    C1199 = "1199", "Die Voraussetzung für die Bereitstellung ist nicht mehr gegeben. Der Auftrag wurde abgebrochen.", [T::Abbm], &[G::Neu];
    /// `1200` — Es wurden nicht alle Aufträge zur Auftragsklammer geliefert.
    C1200 = "1200", "Es wurden nicht alle Aufträge zur Auftragsklammer geliefert.", [T::Abbm], &[G::Neu, G::AenLmae, G::Lae, G::Pv];
    /// `1201` — Auftrag abgewiesen aufgrund der Abweisung eines anderen Auftrages mit der gleichen Auftragsklammer.
    C1201 = "1201", "Auftrag abgewiesen aufgrund der Abweisung eines anderen Auftrages mit der gleichen Auftragsklammer.", [T::Abbm], &[G::Neu, G::AenLmae, G::Lae, G::Pv];
    /// `1300` — Das beauftragte Produkt ist technisch nicht möglich.
    C1300 = "1300", "Das beauftragte Produkt ist technisch nicht möglich.", [T::Abbm], &[G::Neu, G::Lae];
    /// `1301` — Das beauftragte Produkt ist aufgrund der Leitungsdämpfung nicht möglich.
    C1301 = "1301", "Das beauftragte Produkt ist aufgrund der Leitungsdämpfung nicht möglich.", [T::Abbm], &[G::Neu, G::Lae, G::Pv];
    /// `1309` — Das beauftragte Produkt ist aufgrund Portmangel nicht möglich.
    C1309 = "1309", "Das beauftragte Produkt ist aufgrund Portmangel nicht möglich.", [T::Abbm], &[G::Neu];
    /// `1310` — Das beauftrage Produkt ist aufgrund Leitungsmangel nicht möglich.
    C1310 = "1310", "Das beauftrage Produkt ist aufgrund Leitungsmangel nicht möglich.", [T::Abbm], &[G::Neu];
    /// `1311` — Standortangabe nicht eindeutig oder ausreichend.
    C1311 = "1311", "Standortangabe nicht eindeutig oder ausreichend.", [T::Abbm], &[G::Neu];
    /// `1312` — "Freitext„.
    C1312 = "1312", "\"Freitext„.", [T::Abbm], &[G::Neu];
    /// `1320` — HomeID am Standort nicht verfügbar.
    C1320 = "1320", "HomeID am Standort nicht verfügbar.", [T::Abbm], &[G::Neu];
    /// `1500` — Gebietsentscheidung zum Ausbau wurde getroffen. Verbindliche Auftragsannahme.
    C1500 = "1500", "Gebietsentscheidung zum Ausbau wurde getroffen. Verbindliche Auftragsannahme.", [T::Vzm], &[G::Neu];
    /// `1501` — Das Gebiet wird nicht ausgebaut.
    C1501 = "1501", "Das Gebiet wird nicht ausgebaut.", [T::Abbm], &[G::Neu];
    /// `1502` — Die Auskundung hat stattgefunden.
    C1502 = "1502", "Die Auskundung hat stattgefunden.", [T::Vzm], &[G::Neu];
    /// `1503` — Das bestellte Produkt kann mit der vorgesehenen Endleitung nicht bereitgestellt werden.
    C1503 = "1503", "Das bestellte Produkt kann mit der vorgesehenen Endleitung nicht bereitgestellt werden.", [T::Abbm], &[G::Neu];
    /// `1504` — Das bestellte Produkt kann mit dem Ergebnis der Auskundung nicht bereitgestellt werden, da der Standort nicht ausgebaut wird.
    C1504 = "1504", "Das bestellte Produkt kann mit dem Ergebnis der Auskundung nicht bereitgestellt werden, da der Standort nicht ausgebaut wird.", [T::Abbm], &[G::Neu];
    /// `1505` — Der Standort wird aufgrund Eigentümer-Ablehnung nicht ausgebaut.
    C1505 = "1505", "Der Standort wird aufgrund Eigentümer-Ablehnung nicht ausgebaut.", [T::Abbm], &[G::Neu];
    /// `1506` — Der Leistungserbringer vereinbart einen neuen Termin mit den Endkunden.
    C1506 = "1506", "Der Leistungserbringer vereinbart einen neuen Termin mit den Endkunden.", [T::Vzm], &[G::Neu];
    /// `1507` — Die Konnektivität konnte nicht hergestellt werden.
    C1507 = "1507", "Die Konnektivität konnte nicht hergestellt werden.", [T::Abbm], &[G::Neu];
    /// `1508` — Die Konnektivität wurde hergestellt - Bitte Aktivierung des Zielproduktes beauftragen.
    C1508 = "1508", "Die Konnektivität wurde hergestellt - Bitte Aktivierung des Zielproduktes beauftragen.", [T::Vzm], &[G::Neu];
    /// `1509` — Annahme des Auftrages zur Konnektivitäts-Herstellung für den Standort noch nicht möglich.
    C1509 = "1509", "Annahme des Auftrages zur Konnektivitäts-Herstellung für den Standort noch nicht möglich.", [T::Abbm], &[G::Neu];
    /// `1510` — Die Konnektivität ist für den Standort bereits hergestellt.
    C1510 = "1510", "Die Konnektivität ist für den Standort bereits hergestellt.", [T::Abbm], &[G::Neu];
    /// `1511` — Für den Standort ist derzeit kein Ausbau geplant.
    C1511 = "1511", "Für den Standort ist derzeit kein Ausbau geplant.", [T::Abbm], &[G::Neu];
    /// `1512` — Der Eigentümer hat dem Ausbau des Standorts zugestimmt.
    C1512 = "1512", "Der Eigentümer hat dem Ausbau des Standorts zugestimmt.", [T::Vzm], &[G::Neu];
    /// `1513` — Der Auskundungstermin für den Standort wurde festgelegt.
    C1513 = "1513", "Der Auskundungstermin für den Standort wurde festgelegt.", [T::Vzm], &[G::Neu];
    /// `1514` — Die Installation des Hausübergabepunktes für den Standort ist abgeschlossen.
    C1514 = "1514", "Die Installation des Hausübergabepunktes für den Standort ist abgeschlossen.", [T::Vzm], &[G::Neu];
    /// `1515` — Verbindliche Auftragsannahme.
    C1515 = "1515", "Verbindliche Auftragsannahme.", [T::Vzm], &[G::Neu];
    /// `1516` — Der Auskundungstermin für den Standort wird geplant und mit dem Eigentümer abgestimmt.
    C1516 = "1516", "Der Auskundungstermin für den Standort wird geplant und mit dem Eigentümer abgestimmt.", [T::Vzm], &[G::Neu];
    /// `1517` — Der NE3-Installationstermin (Hausübergabepunkt) wird geplant und mit dem Eigentümer abgestimmt.
    C1517 = "1517", "Der NE3-Installationstermin (Hausübergabepunkt) wird geplant und mit dem Eigentümer abgestimmt.", [T::Vzm], &[G::Neu];
    /// `1518` — Der NE3-Installationstermin (Hausübergabepunkt) wurde durch den Eigentümer ausgewählt.
    C1518 = "1518", "Der NE3-Installationstermin (Hausübergabepunkt) wurde durch den Eigentümer ausgewählt.", [T::Vzm], &[G::Neu];
    /// `1519` — Der NE4-Installationstermin (Netzabschluss) wird geplant und mit dem Endkunden abgestimmt.
    C1519 = "1519", "Der NE4-Installationstermin (Netzabschluss) wird geplant und mit dem Endkunden abgestimmt.", [T::Vzm], &[G::Neu];
    /// `1520` — Fehlerhafte oder fehlende Angabe der Gebäudeteil-ID.
    C1520 = "1520", "Fehlerhafte oder fehlende Angabe der Gebäudeteil-ID.", [T::Abbm], &[G::Neu];
    /// `1521` — Falsche Gebäudeteil-ID bei Auftragsausführung erkannt.
    C1521 = "1521", "Falsche Gebäudeteil-ID bei Auftragsausführung erkannt.", [T::Abbm], &[G::Neu];
    /// `1522` — Die Kontaktdaten (Endkunde / Eigentümer) sind nicht vollständig.
    C1522 = "1522", "Die Kontaktdaten (Endkunde / Eigentümer) sind nicht vollständig.", [T::Abbm], &[G::Neu];
    /// `1523` — Zu der Lokation existiert bereits ein weiterer, verhindernder Auftrag zur Herstellung der Konnektivität.
    C1523 = "1523", "Zu der Lokation existiert bereits ein weiterer, verhindernder Auftrag zur Herstellung der Konnektivität.", [T::Abbm], &[G::Neu];
    /// `1524` — Die Eigentümer-Daten sind nicht korrekt, der Auftrag kann nicht prozessiert werden.
    C1524 = "1524", "Die Eigentümer-Daten sind nicht korrekt, der Auftrag kann nicht prozessiert werden.", [T::Abbm], &[G::Neu];
    /// `1525` — Der Endkunde lehnt die Installation ab, Konnektivität konnte nicht hergestellt werden.
    C1525 = "1525", "Der Endkunde lehnt die Installation ab, Konnektivität konnte nicht hergestellt werden.", [T::Abbm], &[G::Neu];
    /// `1526` — Die Eigentümer-Daten wurden nicht angegeben.
    C1526 = "1526", "Die Eigentümer-Daten wurden nicht angegeben.", [T::Abbm], &[G::Neu];
    /// `1530` — Für das bestellte Produkt besteht keine Konnektivität. Es muss die Herstellung der Konnektivität beauftragt werden.
    C1530 = "1530", "Für das bestellte Produkt besteht keine Konnektivität. Es muss die Herstellung der Konnektivität beauftragt werden.", [T::Abbm], &[G::Neu];
    /// `1531` — Der Leistungserbringer informiert den Endkunden über den Ausbautermin.
    C1531 = "1531", "Der Leistungserbringer informiert den Endkunden über den Ausbautermin.", [T::Vzm], &[G::Neu];
    /// `1532` — Der Leistungserbringer informiert den Auftraggeber über den Ausbautermin.
    C1532 = "1532", "Der Leistungserbringer informiert den Auftraggeber über den Ausbautermin.", [T::Vzm], &[G::Neu];
    /// `1533` — Fehlende Gestattung oder fehlende Grundstückseigentümer-Erklärung.
    C1533 = "1533", "Fehlende Gestattung oder fehlende Grundstückseigentümer-Erklärung.", [T::Abbm], &[G::Neu];
    /// `1534` — Der Ausbauzeitraum für den Standort hat sich verschoben.
    C1534 = "1534", "Der Ausbauzeitraum für den Standort hat sich verschoben.", [T::Vzm], &[G::Neu];
    /// `1535` — Daten zum ausführenden Auftragnehmer enthalten.
    C1535 = "1535", "Daten zum ausführenden Auftragnehmer enthalten.", [T::Vzm], &[G::Neu];
    /// `1536` — Interaktion Auftraggeber mit Endkunde notwendig.
    C1536 = "1536", "Interaktion Auftraggeber mit Endkunde notwendig.", [T::Vzm], &[G::Neu, G::Est];
    /// `1537` — Der Ausbau verzögert sich, Grund: Bauverhinderung.
    C1537 = "1537", "Der Ausbau verzögert sich, Grund: Bauverhinderung.", [T::Vzm], &[G::Neu];
    /// `1538` — Der Ausbau verzögert sich, Grund: Höhere Gewalt.
    C1538 = "1538", "Der Ausbau verzögert sich, Grund: Höhere Gewalt.", [T::Vzm], &[G::Neu];
    /// `1539` — Der Ausbau verzögert sich, Grund: Endkunde nicht erreichbar.
    C1539 = "1539", "Der Ausbau verzögert sich, Grund: Endkunde nicht erreichbar.", [T::Vzm], &[G::Neu];
    /// `1540` — Der Ausbau verzögert sich, Grund: Eigentümer nicht erreichbar.
    C1540 = "1540", "Der Ausbau verzögert sich, Grund: Eigentümer nicht erreichbar.", [T::Vzm], &[G::Neu];
    /// `1541` — Der Ausbau verzögert sich, Grund: Verzug durch Endkunde.
    C1541 = "1541", "Der Ausbau verzögert sich, Grund: Verzug durch Endkunde.", [T::Vzm], &[G::Neu];
    /// `1542` — Der Ausbau verzögert sich, Grund: Verzug durch Eigentümer.
    C1542 = "1542", "Der Ausbau verzögert sich, Grund: Verzug durch Eigentümer.", [T::Vzm], &[G::Neu];
    /// `1543` — Der Ausbau verzögert sich.
    C1543 = "1543", "Der Ausbau verzögert sich.", [T::Vzm], &[G::Neu];
    /// `1544` — Der Ausbau wird fortgesetzt.
    C1544 = "1544", "Der Ausbau wird fortgesetzt.", [T::Vzm], &[G::Neu];
    /// `1545` — Der Auskundungstermin wurde storniert.
    C1545 = "1545", "Der Auskundungstermin wurde storniert.", [T::Vzm], &[G::Neu];
    /// `1546` — Der NE3-Installationstermin (Hausübergabepunkt) wurde storniert.
    C1546 = "1546", "Der NE3-Installationstermin (Hausübergabepunkt) wurde storniert.", [T::Vzm], &[G::Neu];
    /// `1547` — Der NE4-Installationstermin (Netzabschluss) wurde aufgrund eines Auftragnehmer-Wechsels storniert.
    C1547 = "1547", "Der NE4-Installationstermin (Netzabschluss) wurde aufgrund eines Auftragnehmer-Wechsels storniert.", [T::Vzm], &[G::Neu];
    /// `1548` — Der NE4-Installationstermin (Netzabschluss) wurde storniert, da die Installation in einem abweichenden Gebäudeteil erforderlich ist.
    C1548 = "1548", "Der NE4-Installationstermin (Netzabschluss) wurde storniert, da die Installation in einem abweichenden Gebäudeteil erforderlich ist.", [T::Vzm], &[G::Neu];
    /// `1549` — Der NE4-Installationstermin (Netzabschluss) wurde durch den Endkunden storniert.
    C1549 = "1549", "Der NE4-Installationstermin (Netzabschluss) wurde durch den Endkunden storniert.", [T::Vzm], &[G::Neu];
    /// `1550` — Der NE4-Installationstermin (Netzabschluss) wurde durch den Auftragnehmer storniert.
    C1550 = "1550", "Der NE4-Installationstermin (Netzabschluss) wurde durch den Auftragnehmer storniert.", [T::Vzm], &[G::Neu];
    /// `1551` — Der NE4-Installationstermin (Netzabschluss) wurde storniert.
    C1551 = "1551", "Der NE4-Installationstermin (Netzabschluss) wurde storniert.", [T::Vzm], &[G::Neu];
    /// `1552` — Der Ausbau verzögert sich, Grund: Warten auf Eigentümerzustimmung.
    C1552 = "1552", "Der Ausbau verzögert sich, Grund: Warten auf Eigentümerzustimmung.", [T::Vzm], &[G::Neu];
    /// `1553` — Konnektivität wurde hergestellt - bitte Aktivierung des Zielproduktes beauftragen.
    C1553 = "1553", "Konnektivität wurde hergestellt - bitte Aktivierung des Zielproduktes beauftragen.", [T::Abbm], &[G::Neu];
    /// `1554` — Die Konnektivität für Ihren Endkunden wird ab jetzt im Rahmen eines Bulk Ausbaus des Gebäudes hergestellt
    C1554 = "1554", "Die Konnektivität für Ihren Endkunden wird ab jetzt im Rahmen eines Bulk Ausbaus des Gebäudes hergestellt", [T::Vzm], &[G::Neu];
    /// `1555` — Es konnte keine Eigentümer-Zustimmung für den Ausbau eingeholt werden.
    C1555 = "1555", "Es konnte keine Eigentümer-Zustimmung für den Ausbau eingeholt werden.", [T::Abbm], &[G::Neu];
    /// `1556` — Die Konnektivität konnte aus Gründen höherer Gewalt nicht hergestellt werden.
    C1556 = "1556", "Die Konnektivität konnte aus Gründen höherer Gewalt nicht hergestellt werden.", [T::Abbm], &[G::Neu];
    /// `1557` — Die Konnektivität konnte nicht hergestellt werden, da der Endkunde nicht erreicht werden konnte.
    C1557 = "1557", "Die Konnektivität konnte nicht hergestellt werden, da der Endkunde nicht erreicht werden konnte.", [T::Abbm], &[G::Neu];
    /// `1558` — Die Konnektivität konnte nicht hergestellt werden, da der Eigentümer nicht erreicht werden konnte.
    C1558 = "1558", "Die Konnektivität konnte nicht hergestellt werden, da der Eigentümer nicht erreicht werden konnte.", [T::Abbm], &[G::Neu];
    /// `1559` — Der Endkunde wird zur Ermittlung der Home-ID kontaktiert - Konnektivität wurde hergestellt.
    C1559 = "1559", "Der Endkunde wird zur Ermittlung der Home-ID kontaktiert - Konnektivität wurde hergestellt.", [T::Vzm], &[G::Neu];
    /// `1560` — Der Hausstich wurde durchgeführt.
    C1560 = "1560", "Der Hausstich wurde durchgeführt.", [T::Vzm], &[G::Neu];
    /// `1561` — Der NE4-Installationstermin (Netzabschluss) wurde storniert, da die Installation im Rahmen eines Bulk-Ausbaus des Gebäudes neu terminiert werden muss.
    C1561 = "1561", "Der NE4-Installationstermin (Netzabschluss) wurde storniert, da die Installation im Rahmen eines Bulk-Ausbaus des Gebäudes neu terminiert werden muss.", [T::Vzm], &[G::Neu];
    /// `1562` — Der Ausbau in dem Gebiet wurde gestartet.
    C1562 = "1562", "Der Ausbau in dem Gebiet wurde gestartet.", [T::Vzm], &[G::Neu];
    /// `1563` — Die Konnektivität für Ihren Endkunden wird ab jetzt im Rahmen eines regulären Auftrags zur Konnektivität hergestellt.
    C1563 = "1563", "Die Konnektivität für Ihren Endkunden wird ab jetzt im Rahmen eines regulären Auftrags zur Konnektivität hergestellt.", [T::Vzm], &[G::Neu];
    /// `1565` — Reminder: Mitwirkung erforderlich! Letztmalige Frist von 14 KT; wenn nach Ablauf keine Abhilfe geschaffen wird, erfolgt der Abbruch des Auftrags. Rechtlich wird dies eine Kündigung des Einzelvertrags bedeuten.
    C1565 = "1565", "Reminder: Mitwirkung erforderlich! Letztmalige Frist von 14 KT; wenn nach Ablauf keine Abhilfe geschaffen wird, erfolgt der Abbruch des Auftrags. Rechtlich wird dies eine Kündigung des Einzelvertrags bedeuten.", [T::Vzm], &[G::Neu];
    /// `1566` — Abbruch des Auftrags aufgrund fehlender Mitwirkung. Rechtlich bedeutet dies eine Kündigung des Einzelvertrags.
    C1566 = "1566", "Abbruch des Auftrags aufgrund fehlender Mitwirkung. Rechtlich bedeutet dies eine Kündigung des Einzelvertrags.", [T::Abbm], &[G::Neu];
    /// `1567` — Der Auftrag ist ab sofort im Glasfaserportal sichtbar.
    C1567 = "1567", "Der Auftrag ist ab sofort im Glasfaserportal sichtbar.", [T::Vzm], &[G::Neu];
    /// `1568` — Vorabinformation: Anstehender Technologiewechsel.
    C1568 = "1568", "Vorabinformation: Anstehender Technologiewechsel.", [T::Vzm], &[G::Neu];
    /// `1569` — Der Glasfaser-Ausbau verzögert sich, Grund: Auftragsstorno wird geprüft.
    C1569 = "1569", "Der Glasfaser-Ausbau verzögert sich, Grund: Auftragsstorno wird geprüft.", [T::Vzm], &[G::Neu];
    /// `1570` — Die Aktivierung des Endkundenanschlusses erfolgt unabhängig zum Ausbau des Netzabschlusses.
    C1570 = "1570", "Die Aktivierung des Endkundenanschlusses erfolgt unabhängig zum Ausbau des Netzabschlusses.", [T::Vzm], &[G::Neu];
    /// `1571` — Die Aktivierung des Endkundenanschlusses kann nicht unmittelbar nach Ausbau des Netzabschlusses erfolgen.
    C1571 = "1571", "Die Aktivierung des Endkundenanschlusses kann nicht unmittelbar nach Ausbau des Netzabschlusses erfolgen.", [T::Vzm], &[G::Neu];
    /// `1572` — Der Ausbau des Netzabschlusses ist erfolgt.
    C1572 = "1572", "Der Ausbau des Netzabschlusses ist erfolgt.", [T::Vzm], &[G::Neu];
    /// `1573` — Die Herstellung der Konnektivität wird bearbeitet sobald die Nachverdichtung beginnt.
    C1573 = "1573", "Die Herstellung der Konnektivität wird bearbeitet sobald die Nachverdichtung beginnt.", [T::Vzm], &[G::Neu];
    /// `1574` — Der Fortschritt der Herstellung der Konnektivität bedingt bei einem Storno durch den Auftraggeber eine Stornogebühr.
    C1574 = "1574", "Der Fortschritt der Herstellung der Konnektivität bedingt bei einem Storno durch den Auftraggeber eine Stornogebühr.", [T::Vzm], &[G::Neu];
    /// `1575` — HomeID Clearing durch Leistungerbringer notwendig.
    C1575 = "1575", "HomeID Clearing durch Leistungerbringer notwendig.", [T::Vzm], &[G::Neu];
    /// `1576` — Für die Aktivierung des Endkundenanschlusses ist ein Endkundentermin notwendig.
    C1576 = "1576", "Für die Aktivierung des Endkundenanschlusses ist ein Endkundentermin notwendig.", [T::Vzm], &[G::Neu];
    /// `1577` — Die HomeID wurde für den Netzabschluss ermittelt.
    C1577 = "1577", "Die HomeID wurde für den Netzabschluss ermittelt.", [T::Vzm], &[G::Neu];
    /// `1578` — Ausbauzustand der Wohnung.
    C1578 = "1578", "Ausbauzustand der Wohnung.", [T::Vzm], &[G::Neu];
    /// `1579` — Der Übergabepunkt des Anschlusses wurde zentral an der Glasfasereinführung im Gebäude hergestellt und nicht in der Nutzungseinheit.
    C1579 = "1579", "Der Übergabepunkt des Anschlusses wurde zentral an der Glasfasereinführung im Gebäude hergestellt und nicht in der Nutzungseinheit.", [T::Vzm], &[G::Neu];
    /// `1580` — Der Termin zur Durchführung des Hausstiches wurde durch den Eigentümer ausgewählt.
    C1580 = "1580", "Der Termin zur Durchführung des Hausstiches wurde durch den Eigentümer ausgewählt.", [T::Vzm], &[G::Neu];
    /// `1581` — Der Termin zur Steigleitungsinstallation wurde durch den Eigentümer ausgewählt.
    C1581 = "1581", "Der Termin zur Steigleitungsinstallation wurde durch den Eigentümer ausgewählt.", [T::Vzm], &[G::Neu];
    /// `1582` — Die Steigleitungsinstallation wurde durchgeführt.
    C1582 = "1582", "Die Steigleitungsinstallation wurde durchgeführt.", [T::Vzm], &[G::Neu];
    /// `1583` — Die Wegesicherung wurde durchgeführt.
    C1583 = "1583", "Die Wegesicherung wurde durchgeführt.", [T::Vzm], &[G::Neu];
    /// `1584` — Die Prüfung auf Kampfmittelfreiheit wurde abgeschlossen.
    C1584 = "1584", "Die Prüfung auf Kampfmittelfreiheit wurde abgeschlossen.", [T::Vzm], &[G::Neu];
    /// `1585` — Der Eigentümer wird zur Einholung der Zustimmung kontaktiert.
    C1585 = "1585", "Der Eigentümer wird zur Einholung der Zustimmung kontaktiert.", [T::Vzm], &[G::Neu];
    /// `1586` — Der Medienkonverter wird vom Leistungserbringer an den Endkunden versandt. The handbook's code list prints this row shifted (it reads 'Der Eigentümer wird zur Einholung der Zustimmung kontaktiert.'); the text here is the fact sheet's (an interpretation).
    C1586 = "1586", "Der Medienkonverter wird vom Leistungserbringer an den Endkunden versandt.", [T::Vzm], &[G::Neu];
    /// `2001` — Die Leistungsnummer ist nicht plausibel oder nicht eindeutig. The handbook's code list prints this row shifted (it reads 'Der Medienkonverter wird vom Leistungserbringer an den Endkunden versandt.'); the text here is the fact sheet's (an interpretation).
    C2001 = "2001", "Die Leistungsnummer ist nicht plausibel oder nicht eindeutig.", [T::Abbm], &[G::Neu, G::KueAg, G::AenLmae, G::Lae, G::Pv, G::Get, G::Set];
    /// `2017` — Produkt aufgrund Vectoring nicht erlaubt.
    C2017 = "2017", "Produkt aufgrund Vectoring nicht erlaubt.", [T::Abbm], &[G::Neu, G::AenLmae, G::Lae, G::Pv];
    /// `5000` — Zahl der in den Planungsabsprachen zum Auftragseingangsdatum abgestimmten Menge an Aufträgen wurde überschritten.
    C5000 = "5000", "Zahl der in den Planungsabsprachen zum Auftragseingangsdatum abgestimmten Menge an Aufträgen wurde überschritten.", [T::Qeb], &[G::Neu];
    /// `5001` — Die Zahl der Aufträge zum geplanten Ausführungstermin liegt über der in Planungsabsprachen festgelegten Menge.
    C5001 = "5001", "Die Zahl der Aufträge zum geplanten Ausführungstermin liegt über der in Planungsabsprachen festgelegten Menge.", [T::Abbm], &[G::Neu, G::Get, G::Set];
    /// `5002` — Die Zahl der Aufträge zum geplanten Ausführungstermin liegt über der in der Planungsabsprache festgelegten Menge. Der Auftrag wurde auf den betrieblich nächstmöglichen Termin verschoben.
    C5002 = "5002", "Die Zahl der Aufträge zum geplanten Ausführungstermin liegt über der in der Planungsabsprache festgelegten Menge. Der Auftrag wurde auf den betrieblich nächstmöglichen Termin verschoben.", [T::Abm], &[G::Neu];
    /// `6000` — Endkunde hat Herstellung zurückgestellt/verweigert.
    C6000 = "6000", "Endkunde hat Herstellung zurückgestellt/verweigert.", [T::Tam, T::Mtam], &[G::Neu, G::Lae, G::Pv, G::Est];
    /// `6001` — Endkunde nicht angetroffen.
    C6001 = "6001", "Endkunde nicht angetroffen.", [T::Tam, T::Mtam], &[G::Neu, G::Lae, G::Pv, G::Est];
    /// `6002` — Nicht ausführbar aus Endkundengrund (z.B. APL nicht zugänglich).
    C6002 = "6002", "Nicht ausführbar aus Endkundengrund (z.B. APL nicht zugänglich).", [T::Tam, T::Mtam], &[G::Neu, G::Est];
    /// `6003` — Nicht ausführbar wegen falscher Angaben des Endkunden.
    C6003 = "6003", "Nicht ausführbar wegen falscher Angaben des Endkunden.", [T::Tam, T::Mtam], &[G::Neu, G::Lae, G::Pv, G::Est];
    /// `6004` — Ausführung wegen lokalem Feiertag nicht möglich
    C6004 = "6004", "Ausführung wegen lokalem Feiertag nicht möglich", [T::Tam, T::Mtam], &[G::Neu, G::Lae, G::Pv, G::Est];
    /// `6005` — Höhere Gewalt.
    C6005 = "6005", "Höhere Gewalt.", [T::Tam, T::Mtam], &[G::Neu, G::Lae, G::Pv, G::Est];
    /// `6006` — Nicht ausführbar aus sonstigem Grund.
    C6006 = "6006", "Nicht ausführbar aus sonstigem Grund.", [T::Tam, T::Mtam], &[G::Neu, G::Lae, G::Pv, G::Est];
    /// `6007` — Endleitung fehlt vollständig.
    C6007 = "6007", "Endleitung fehlt vollständig.", [T::Tam, T::Mtam], &[G::Neu, G::Lae, G::Pv];
    /// `6008` — Endleitung fehlt teilweise.
    C6008 = "6008", "Endleitung fehlt teilweise.", [T::Tam, T::Mtam], &[G::Neu, G::Lae, G::Pv];
    /// `6009` — Endleitung ist defekt.
    C6009 = "6009", "Endleitung ist defekt.", [T::Tam, T::Mtam], &[G::Neu, G::Lae, G::Pv, G::Est];
    /// `6010` — Herstellung der Endleitung nur nach Angebot.
    C6010 = "6010", "Herstellung der Endleitung nur nach Angebot.", [T::Tam, T::Mtam], &[G::Neu, G::Lae, G::Pv];
    /// `6011` — Arbeiten an der Endleitung erforderlich. Endkunde lässt Bau der Abschlusseinrichtung nicht zu.
    C6011 = "6011", "Arbeiten an der Endleitung erforderlich. Endkunde lässt Bau der Abschlusseinrichtung nicht zu.", [T::Tam, T::Mtam], &[G::Neu, G::Lae, G::Pv];
    /// `6012` — Termin am nächsten Werktag (Mo. - Sa.).
    C6012 = "6012", "Termin am nächsten Werktag (Mo. - Sa.).", [T::Abm, T::AbmPv], &[G::Neu, G::KueAg, G::Lae, G::Pv];
    /// `6013` — Nicht ausführbar aus sonstigem technischen Grund.
    C6013 = "6013", "Nicht ausführbar aus sonstigem technischen Grund.", [T::Tam, T::Mtam], &[G::Neu, G::Lae, G::Pv];
    /// `6015` — TAM für Techniker Leistungserbringer
    C6015 = "6015", "TAM für Techniker Leistungserbringer", [T::Tam, T::Mtam], &[G::Neu, G::Lae, G::Pv, G::Est];
    /// `6016` — TAM für Techniker Vorleister
    C6016 = "6016", "TAM für Techniker Vorleister", [T::Tam, T::Mtam], &[G::Neu, G::Lae, G::Pv, G::Est];
    /// `6021` — Technikertermin hat stattgefunden aber Abschluss der Installation durch Techniker ist nicht mehr vor Kundenwunschtermin (KWT) möglich. Bitte mit dem Kunden einen neuen, späteren KWT vereinbaren.
    C6021 = "6021", "Technikertermin hat stattgefunden aber Abschluss der Installation durch Techniker ist nicht mehr vor Kundenwunschtermin (KWT) möglich. Bitte mit dem Kunden einen neuen, späteren KWT vereinbaren.", [T::Tam, T::Mtam], &[G::Neu];
    /// `6022` — Konnektivität hergestellt. Terminierung zur Aktivierung des beauftragten Produktes erforderlich
    C6022 = "6022", "Konnektivität hergestellt. Terminierung zur Aktivierung des beauftragten Produktes erforderlich", [T::Tam, T::Mtam], &[G::Neu];
    /// `6023` — Das Zeitfenster ist nicht vereinbart oder ungültig
    C6023 = "6023", "Das Zeitfenster ist nicht vereinbart oder ungültig", [T::Tam, T::Mtam], &[G::Neu];
    /// `6050` — Keine Terminvereinbarung mit Endkunden möglich.
    C6050 = "6050", "Keine Terminvereinbarung mit Endkunden möglich.", [T::Tam], &[G::Est];
    /// `7000` — Fehler beseitigt.
    C7000 = "7000", "Fehler beseitigt.", [T::Erlm], &[G::Est];
    /// `7003` — Kein Fehler festgestellt.
    C7003 = "7003", "Kein Fehler festgestellt.", [T::Erlm], &[G::Est];
    /// `7005` — Kein Fehler festgestellt (Techniker hat beim Endkunden geprüft).
    C7005 = "7005", "Kein Fehler festgestellt (Techniker hat beim Endkunden geprüft).", [T::Erlm], &[G::Est];
    /// `7006` — Rückfall. Eine Bestandsänderung wird durchgeführt, bzw. eine Leistungsänderung ist notwendig.
    C7006 = "7006", "Rückfall. Eine Bestandsänderung wird durchgeführt, bzw. eine Leistungsänderung ist notwendig.", [T::Erlm], &[G::Est];
    /// `7009` — Kein Fehler festgestellt (Endgerät nicht kompatibel).
    C7009 = "7009", "Kein Fehler festgestellt (Endgerät nicht kompatibel).", [T::Erlm], &[G::Est];
    /// `7010` — Zurückweisung.
    C7010 = "7010", "Zurückweisung.", [T::Erlm], &[G::Est];
    /// `7011` — Referenzauftrag konnte nicht ermittelt werden.
    C7011 = "7011", "Referenzauftrag konnte nicht ermittelt werden.", [T::Abbm], &[G::Est];
    /// `7012` — Reklamation nicht zulässig.
    C7012 = "7012", "Reklamation nicht zulässig.", [T::Abbm], &[G::Est];
    /// `7013` — Entstörauftrag abgewiesen. Fehlende Berechtigung für zu entstörendes Produkt.
    C7013 = "7013", "Entstörauftrag abgewiesen. Fehlende Berechtigung für zu entstörendes Produkt.", [T::Abbm], &[G::Est];
    /// `7014` — Kundenwunschtermin liegt in der Vergangenheit.
    C7014 = "7014", "Kundenwunschtermin liegt in der Vergangenheit.", [T::Abbm], &[G::Est];
    /// `7015` — Es wurde kein Kundentermin übermittelt.
    C7015 = "7015", "Es wurde kein Kundentermin übermittelt.", [T::Abbm], &[G::Est];
    /// `7020` — Terminvereinbarung mit dem Kunden erfolgt.
    C7020 = "7020", "Terminvereinbarung mit dem Kunden erfolgt.", [T::TbkLe], &[G::Est];
    /// `7023` — Endkunde trotz Terminvereinbarung nicht angetroffen.
    C7023 = "7023", "Endkunde trotz Terminvereinbarung nicht angetroffen.", [T::Erlm], &[G::Est];
    /// `7024` — Ihr Anschluss ist ggf. von einer Massenstörung betroffen. The fact sheet names „ZWM" without a side; a notice of a mass outage comes from the network owner, so ZWM-LE (an interpretation).
    C7024 = "7024", "Ihr Anschluss ist ggf. von einer Massenstörung betroffen.", [T::ZwmLe], &[G::Est];
    /// `7030` — Hinderungsgrund für letzten Endkundentermin ist beseitigt.
    C7030 = "7030", "Hinderungsgrund für letzten Endkundentermin ist beseitigt.", [T::TbkAg], &[G::Est];
    /// `7037` — Servicetechnikereinsatz ohne Endkundentermin.
    C7037 = "7037", "Servicetechnikereinsatz ohne Endkundentermin.", [T::TbkLe], &[G::Est];
    /// `7038` — Termin wurde durch Endkunden verschoben.
    C7038 = "7038", "Termin wurde durch Endkunden verschoben.", [T::TbkLe], &[G::Est];
    /// `7039` — Der mit der EST übermittelte Endkundentermin wird voraussichtlich wahrgenommen.
    C7039 = "7039", "Der mit der EST übermittelte Endkundentermin wird voraussichtlich wahrgenommen.", [T::TbkLe], &[G::Est];
    /// `7040` — Ergänzende Information zum Auftrag.
    C7040 = "7040", "Ergänzende Information zum Auftrag.", [T::ZwmAg], &[G::Est];
    /// `7041` — Der Auftrag ist weiter offen.
    C7041 = "7041", "Der Auftrag ist weiter offen.", [T::ZwmAg], &[G::Est];
    /// `7042` — Der Auftraggeber wartet weiter auf den Endkundentermin.
    C7042 = "7042", "Der Auftraggeber wartet weiter auf den Endkundentermin.", [T::ZwmAg], &[G::Est];
    /// `7500` — Die Line ist nicht synchron, eine Diagnose kann nicht durchgeführt werden.
    C7500 = "7500", "Die Line ist nicht synchron, eine Diagnose kann nicht durchgeführt werden.", [T::Abbm], &[G::Get];
    /// `7504` — Zeitüberschreitung beim Warten auf das Diagnoseergebnis. Bitte versuchen Sie es zu einem späteren Zeitpunkt noch einmal.
    C7504 = "7504", "Zeitüberschreitung beim Warten auf das Diagnoseergebnis. Bitte versuchen Sie es zu einem späteren Zeitpunkt noch einmal.", [T::Abbm], &[G::Get, G::Set];
    /// `7508` — Die Anzahl der Diagnoseanfragen auf das betroffene Netzelement ist zu hoch.
    C7508 = "7508", "Die Anzahl der Diagnoseanfragen auf das betroffene Netzelement ist zu hoch.", [T::Abbm], &[G::Get];
    /// `7522` — Es konnte keine Verbindung zum Medienkonverter/Modem aufgebaut werden.
    C7522 = "7522", "Es konnte keine Verbindung zum Medienkonverter/Modem aufgebaut werden.", [T::Abbm], &[G::Neu, G::Est, G::Set];
    /// `7524` — Die Medienkonverter-/Modem-ID ist nicht bekannt.
    C7524 = "7524", "Die Medienkonverter-/Modem-ID ist nicht bekannt.", [T::Abbm], &[G::Neu, G::Est, G::Set];
    /// `8005` — Die Auftragsbestätigung verzögert sich.
    C8005 = "8005", "Die Auftragsbestätigung verzögert sich.", [T::Vzm], &[G::Neu, G::Lae, G::Pv];
    /// `8006` — Der Anschluss muss abgeschaltet werden.
    C8006 = "8006", "Der Anschluss muss abgeschaltet werden.", [T::Kdle], &[G::KueLe];
    /// `8144` — LineID ist nicht bekannt.
    C8144 = "8144", "LineID ist nicht bekannt.", [T::Abbm], &[G::KueAg, G::AenLmae, G::Lae, G::Pv, G::Est, G::Get, G::Set];
    /// `8500` — Die Auftragsbestätigung wurde vom Leistungserbringer nachträglich verändert, der Ausführungstermin bleibt bestehen.
    C8500 = "8500", "Die Auftragsbestätigung wurde vom Leistungserbringer nachträglich verändert, der Ausführungstermin bleibt bestehen.", [T::Abm, T::AbmPv], &[G::Neu, G::KueAg, G::Lae, G::Pv];
    /// `8501` — Der geplante Ausführungstermin wurde durch den Leistungserbringer nachträglich verschoben.
    C8501 = "8501", "Der geplante Ausführungstermin wurde durch den Leistungserbringer nachträglich verschoben.", [T::Abm, T::AbmPv], &[G::Neu, G::KueAg, G::Lae, G::Pv];
    /// `8502` — Zur Ausführung des Auftrags wird zusätzliche Hardware benötigt
    C8502 = "8502", "Zur Ausführung des Auftrags wird zusätzliche Hardware benötigt", [T::Abm], &[G::Neu, G::Pv];
    /// `8510` — Die Übernahme des Vorleistungsproduktes war nicht möglich.
    C8510 = "8510", "Die Übernahme des Vorleistungsproduktes war nicht möglich.", [T::Abbm], &[G::Neu];
    /// `8600` — "Freitext"
    C8600 = "8600", "\"Freitext\"", [T::Abbm], &[G::Neu, G::KueAg, G::Lae, G::Pv, G::Est, G::Get, G::Set];
    /// `8601` — "Freitext"
    C8601 = "8601", "\"Freitext\"", [T::Abbm], &[G::Neu, G::KueAg, G::Lae, G::Pv, G::Est, G::Get, G::Set];
    /// `8610` — Es hat noch kein Technikertermin stattgefunden und es ist kein Technikertermin vor Kundenwunschtermin (KWT) möglich. Bitte mit dem Kunden einen neuen, späteren KWT vereinbaren und einen neuen Auftrag erstellen.
    C8610 = "8610", "Es hat noch kein Technikertermin stattgefunden und es ist kein Technikertermin vor Kundenwunschtermin (KWT) möglich. Bitte mit dem Kunden einen neuen, späteren KWT vereinbaren und einen neuen Auftrag erstellen.", [T::Abbm], &[G::Neu];
    /// `9999` — Reserviert für interne Zwecke.
    C9999 = "9999", "Reserviert für interne Zwecke.", [], &[];
}

impl Meldungscode {
    /// `0990` — the function is not supported yet (TEQ).
    pub const FUNKTION_NICHT_UNTERSTUETZT: Self = Self::C0990;
    /// `0991` — the interface is overloaded; try again later (TEQ).
    pub const UEBERLASTET: Self = Self::C0991;
    /// `0993` — version number wrong: the major release is not supported (TEQ).
    pub const VERSION_FEHLERHAFT: Self = Self::C0993;
    /// `0995` — data structure not admissible: schema or structure check failed (TEQ).
    pub const DATENSTRUKTUR_UNZULAESSIG: Self = Self::C0995;
    /// `0999` — technical error at the receiver (TEQ).
    pub const TECHNISCHER_FEHLER: Self = Self::C0999;

    /// Parses a code as sent on the wire; unknown codes become [`Meldungscode::Other`].
    #[must_use]
    pub fn parse(code: &str) -> Self {
        Self::from_known(code).unwrap_or_else(|| Self::Other(code.into()))
    }

    /// The code on the wire, e.g. `1001`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Other(code) => code,
            known => known.row().map_or("", |r| r.code),
        }
    }

    /// The official text (AH §6.3), or `None` for an unknown code.
    #[must_use]
    pub fn text(&self) -> Option<&'static str> {
        self.row().map(|r| r.text)
    }

    /// The short name the specifications use for the code. The S/PRI handbook names none, so
    /// this is always `None`; it exists for symmetry with WBCI.
    #[must_use]
    pub const fn short(&self) -> Option<&'static str> {
        None
    }

    /// The message types that may carry this code (AH §6.4 „Meldungstyp").
    pub fn meldungstypen(&self) -> impl Iterator<Item = Meldungstyp> + '_ {
        self.row().into_iter().flat_map(|r| r.typen.iter().copied())
    }

    /// The business cases the code is used with (AH §6.4, the cases marked for any product).
    pub fn geschaeftsfaelle(&self) -> impl Iterator<Item = GeschaeftsfallCode> + '_ {
        self.row().into_iter().flat_map(|r| r.faelle.iter().copied())
    }

    /// Whether a message of type `typ` in business case `fall` may carry this code: the fact
    /// sheet names the type and marks the case (AH §6.4). The sheets list types and cases
    /// separately, so every named type counts for every marked case.
    #[must_use]
    pub fn applies(&self, typ: Meldungstyp, fall: GeschaeftsfallCode) -> bool {
        self.row().is_some_and(|r| r.typen.contains(&typ) && r.faelle.contains(&fall))
    }

    /// Whether the code is the positive technical receipt `OK`.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        *self == Self::Ok
    }
}

/// What an ABBM tells the service provider to do next, as the EPKs of Anlage 7 (V.13) draw it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbbmOutcome {
    /// „Auftrag korrigieren": correct the order and send it again, as a new order.
    Resend,
    /// „Auftrag abschließen": the order is closed for good.
    Closed,
    /// The Storno or Terminverschiebung failed; the order continues unchanged („Auftrag
    /// unverändert prozessieren", Anl. 7 EPK V.13 p.11, p.12).
    Continues,
    /// The EPK draws no ABBM with this code in this business case and phase.
    Unknown,
}

/// Where in an order's life an ABBM arrives: the EPKs draw a different code set per phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbbmPhase {
    /// After the order's receipt, before the QEB (commercial validation).
    BeforeQeb,
    /// After the QEB, before the ABM (technical validation; in `PV` also the losing
    /// provider's answer).
    AfterQeb,
    /// After the ABM (realisation, including the TAM path).
    AfterAbm,
    /// In answer to a Storno (Anl. 7 EPK V.13 p.11).
    Storno,
    /// In answer to a Terminverschiebung (Anl. 7 EPK V.13 p.12).
    Terminverschiebung,
}

/// The ABBM codes per business case and phase: (resend, closed). Read from the EPKs of Anlage 7
/// V.13: NEU p.1 (Konnektivität p.2 uses the same), KUE-AG p.3, AEN-LMAE p.5, LAE p.6, PV p.7,
/// EST p.8, DIAG p.9.
#[allow(clippy::too_many_lines)] // one table
fn abbm_codes(fall: GeschaeftsfallCode, phase: AbbmPhase) -> (&'static [&'static str], &'static [&'static str]) {
    use AbbmPhase as P;
    const NONE: &[&str] = &[];
    match (fall, phase) {
        (_, P::Storno) if !matches!(fall, G::Get | G::Set | G::KueLe) => (NONE, NONE),
        (_, P::Terminverschiebung) if matches!(fall, G::Neu | G::KueAg | G::Lae | G::Pv) => (NONE, NONE),
        (G::Neu, P::BeforeQeb) => (
            &[
                "1001", "1003", "1006", "1007", "1012", "1013", "1020", "1023", "1031", "1032", "1034", "2001", "2017",
                "5001", "8600",
            ],
            &["1009", "1010", "1011", "1018", "1050", "1101", "1102", "1105", "1530", "8601"],
        ),
        (G::Neu, P::AfterQeb) => (
            &[
                "1001", "1003", "1006", "1007", "1012", "1013", "1020", "1023", "1031", "1032", "1034", "2001", "2017",
                "5001", "8600", "1200", "1201", "1311", "1312", "1320",
            ],
            &[
                "1009", "1010", "1011", "1018", "1050", "1101", "1102", "1105", "1530", "8601", "0044", "1300", "1301",
                "1309", "1310", "8510",
            ],
        ),
        (G::Neu | G::Lae | G::Pv, P::AfterAbm) => (NONE, &["1198", "1199", "2015"]),
        (G::KueAg, P::BeforeQeb) => (
            &["1003", "1006", "1007", "1012", "1013", "1014", "1020", "1023", "1031", "1032", "2001", "8144", "8600"],
            // 1018 is drawn on both branches of p.3; the closed one is taken.
            &["1018", "1050", "1192", "8601"],
        ),
        (G::KueAg, P::AfterQeb) => (&["1023", "1031", "1032", "8600"], NONE),
        (G::AenLmae, P::BeforeQeb) => (
            &[
                "1003", "1006", "1007", "1012", "1013", "1014", "1020", "1023", "1031", "1032", "2001", "2016", "2017",
                "8144",
            ],
            &["1008", "1009", "1010", "1018", "1050", "1051", "1102", "1104", "1105"],
        ),
        (G::AenLmae, P::AfterQeb) => (&["1009", "1023", "1031", "1032", "1200", "1201", "1215", "1216", "1217"], NONE),
        (G::AenLmae, P::AfterAbm) => (NONE, &["2015"]),
        (G::Lae, P::BeforeQeb) => (
            &[
                "1003", "1006", "1007", "1012", "1013", "1014", "1020", "1023", "1031", "1032", "2001", "2017", "8144",
                "8600",
            ],
            &["1008", "1009", "1010", "1018", "1050", "1051", "1101", "1102", "1104", "1105", "1300", "1301", "8601"],
        ),
        (G::Lae, P::AfterQeb) => {
            (&["1009", "1023", "1031", "1032", "1200", "1201", "2017", "8600"], &["1300", "1301", "8601"])
        }
        (G::Pv, P::BeforeQeb) => (
            &[
                "1003", "1007", "1012", "1013", "1014", "1020", "1023", "1031", "1032", "1034", "2001", "2017", "8144",
                "8600",
            ],
            &["1009", "1010", "1018", "1102", "1105", "1301", "8601"],
        ),
        (G::Pv, P::AfterQeb) => (
            &["1003", "1023", "1031", "1032", "1200", "1201", "2017", "8600"],
            &["0041", "0042", "1009", "1010", "1018", "1301", "8601"],
        ),
        (G::Est, P::BeforeQeb) => (
            &["1012", "1013", "1014", "1020", "1031", "7524", "8144", "8600"],
            &["1050", "7011", "7012", "7013", "7014", "7015", "8601"],
        ),
        // EST has no ABM: the loop after the QEB is the realisation; 7015 ends a TAM.
        (G::Est, P::AfterQeb | P::AfterAbm) => {
            (&["1007", "1031", "7522", "7524", "8600"], &["7011", "7012", "7013", "7015", "8601"])
        }
        (G::Get | G::Set, P::BeforeQeb) => (
            &["1012", "1013", "1014", "1023", "2001", "5001", "8144", "8600"],
            &["1018", "1050", "7500", "7504", "7508", "7522", "7524", "8601"],
        ),
        _ => (NONE, NONE),
    }
}

/// The codes that answer a Storno or a Terminverschiebung negatively: the request failed, the
/// order continues (Anl. 7 EPK V.13 p.11: `1007, 1086, 1087`; p.12: `1007, 1085, 7522-24`).
fn continues(fall: GeschaeftsfallCode, phase: AbbmPhase) -> &'static [&'static str] {
    match phase {
        AbbmPhase::Storno if !matches!(fall, G::Get | G::Set | G::KueLe) => &["1007", "1086", "1087"],
        AbbmPhase::Terminverschiebung if matches!(fall, G::Neu | G::KueAg | G::Lae | G::Pv) => {
            &["1007", "1085", "7522", "7523", "7524"]
        }
        _ => &[],
    }
}

impl Meldungscode {
    /// What an ABBM with this code means in business case `fall` and `phase`: correct and
    /// resend, closed, or (for a Storno or Terminverschiebung) failed with the order continuing.
    /// [`AbbmOutcome::Unknown`] where the EPK (Anl. 7 V.13) draws no ABBM with this code there.
    ///
    /// Codes are compared as sent, so EPK codes missing from the code list (`2015`, `7523`) work.
    #[must_use]
    pub fn abbm_outcome(&self, fall: GeschaeftsfallCode, phase: AbbmPhase) -> AbbmOutcome {
        let code = self.as_str();
        if continues(fall, phase).contains(&code) {
            return AbbmOutcome::Continues;
        }
        let (resend, closed) = abbm_codes(fall, phase);
        if closed.contains(&code) {
            AbbmOutcome::Closed
        } else if resend.contains(&code) {
            AbbmOutcome::Resend
        } else {
            AbbmOutcome::Unknown
        }
    }
}

impl core::str::FromStr for Meldungscode {
    type Err = core::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::parse(s))
    }
}

impl fmt::Display for Meldungscode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.text() {
            Some(text) => write!(f, "{} {text}", self.as_str()),
            None => write!(f, "{} (unknown code)", self.as_str()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_complete_and_unique() {
        assert_eq!(Meldungscode::ALL.len(), 215);
        let mut seen = std::collections::HashSet::new();
        for code in Meldungscode::ALL {
            assert!(seen.insert(code.as_str()), "duplicate {}", code.as_str());
            assert_eq!(Meldungscode::parse(code.as_str()), *code);
            let text = code.text().unwrap();
            assert_eq!(crate::Meldungstext::new(text).unwrap().as_str(), text, "{}", code.as_str());
            // Every code but the reserved 9999 has a fact sheet with types and cases.
            if code.as_str() != "9999" {
                assert!(code.meldungstypen().next().is_some(), "{}", code.as_str());
                assert!(code.geschaeftsfaelle().next().is_some(), "{}", code.as_str());
            }
        }
    }

    #[test]
    fn teq_codes_follow_the_handbook() {
        for (c, text) in [
            (Meldungscode::Ok, "OK."),
            (
                Meldungscode::FUNKTION_NICHT_UNTERSTUETZT,
                "Funktion aktuell nicht unterschützt, Freigabe wird gesondert bekannt gegeben.",
            ),
            (
                Meldungscode::UEBERLASTET,
                "Die Schnittstelle ist derzeit überlastet, bitte versuchen Sie es zu einem späteren Zeitpunkt erneut.",
            ),
            (Meldungscode::VERSION_FEHLERHAFT, "Versionsnummer fehlerhaft."),
            (Meldungscode::DATENSTRUKTUR_UNZULAESSIG, "Datenstruktur unzulässig."),
            (Meldungscode::TECHNISCHER_FEHLER, "Technischer Fehler."),
        ] {
            assert_eq!(c.text(), Some(text));
            assert_eq!(c.meldungstypen().collect::<Vec<_>>(), [Meldungstyp::Teq]);
            for fall in GeschaeftsfallCode::ALL {
                assert!(c.applies(Meldungstyp::Teq, *fall));
            }
        }
        let teq: Vec<&str> = Meldungscode::ALL
            .iter()
            .filter(|c| c.meldungstypen().any(|t| t == Meldungstyp::Teq))
            .map(Meldungscode::as_str)
            .collect();
        assert_eq!(teq, ["OK", "0990", "0991", "0993", "0995", "0999"]);
    }

    #[test]
    fn applicability_follows_the_fact_sheets() {
        use Meldungscode as C;
        assert!(C::C0000.applies(T::Qeb, G::Neu));
        assert!(C::C0000.applies(T::Abm, G::Pv));
        assert!(!C::C0000.applies(T::Abm, G::Get));
        assert!(C::C0020.applies(T::AkmPv, G::Pv));
        assert!(!C::C0020.applies(T::AkmPv, G::Neu));
        assert!(C::C0021.applies(T::RuemPv, G::Pv));
        assert!(C::C2001.applies(T::Abbm, G::Set));
        assert!(!C::C9999.applies(T::Abbm, G::Neu));
    }

    #[test]
    fn unknown_codes_are_kept() {
        let c = Meldungscode::parse("4711");
        assert_eq!(c.as_str(), "4711");
        assert_eq!(c.text(), None);
        assert_eq!(c.to_string(), "4711 (unknown code)");
        assert_eq!(Meldungscode::C0993.to_string(), "0993 Versionsnummer fehlerhaft.");
    }

    #[test]
    fn abbm_outcomes_follow_the_epks() {
        use AbbmOutcome as O;
        use AbbmPhase as P;
        let o = |code: &str, fall, phase| Meldungscode::parse(code).abbm_outcome(fall, phase);
        assert_eq!(o("1001", G::Neu, P::BeforeQeb), O::Resend);
        assert_eq!(o("1009", G::Neu, P::BeforeQeb), O::Closed);
        assert_eq!(o("1200", G::Neu, P::BeforeQeb), O::Unknown);
        assert_eq!(o("1200", G::Neu, P::AfterQeb), O::Resend);
        assert_eq!(o("8510", G::Neu, P::AfterQeb), O::Closed);
        assert_eq!(o("2015", G::Neu, P::AfterAbm), O::Closed);
        assert_eq!(o("1198", G::Lae, P::AfterAbm), O::Closed);
        assert_eq!(o("2015", G::KueAg, P::AfterAbm), O::Unknown);
        assert_eq!(o("1018", G::KueAg, P::BeforeQeb), O::Closed);
        assert_eq!(o("1009", G::Lae, P::BeforeQeb), O::Closed);
        assert_eq!(o("1009", G::Lae, P::AfterQeb), O::Resend);
        assert_eq!(o("1215", G::AenLmae, P::AfterQeb), O::Resend);
        assert_eq!(o("0041", G::Pv, P::AfterQeb), O::Closed);
        assert_eq!(o("0042", G::Pv, P::AfterQeb), O::Closed);
        assert_eq!(o("7015", G::Est, P::AfterQeb), O::Closed);
        assert_eq!(o("7524", G::Est, P::BeforeQeb), O::Resend);
        assert_eq!(o("7508", G::Get, P::BeforeQeb), O::Closed);
        assert_eq!(o("5001", G::Set, P::BeforeQeb), O::Resend);
        assert_eq!(o("1086", G::Est, P::Storno), O::Continues);
        assert_eq!(o("1087", G::Neu, P::Storno), O::Continues);
        assert_eq!(o("1085", G::Pv, P::Terminverschiebung), O::Continues);
        assert_eq!(o("7523", G::Neu, P::Terminverschiebung), O::Continues);
        assert_eq!(o("1085", G::AenLmae, P::Terminverschiebung), O::Unknown);
        assert_eq!(o("1086", G::Get, P::Storno), O::Unknown);
        assert_eq!(o("4711", G::Neu, P::BeforeQeb), O::Unknown);
    }
}
