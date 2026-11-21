+++
title = "S/PRI message codes"
description = "All 215 S/PRI 4.x message codes (Meldungscodes) of the Arbeitshandbuch V2.9.5 with official text, message types and business cases, plus what an ABBM means."
weight = 3

[extra]
nav = "Message codes"
seo_title = "S/PRI Meldungscodes: all 215 message codes · telco-interop"
+++

`spri::Meldungscode` holds the 215 codes of the Arbeitshandbuch S/PRI 4.x V2.9.5: the texts of
§6.3 and, from the fact sheets of §6.4, the message types that carry each code and the
business cases it is used with. `text()` gives the official text, `meldungstypen()` and
`geschaeftsfaelle()` the fact sheet, and `applies(typ, fall)` whether a message may carry it.

{% <rust> %}
```rust
use spri::*;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
assert_eq!(Meldungscode::ALL.len(), 215);

let code: Meldungscode = "0010".parse()?;
assert_eq!(code.text(), Some("Auftrag ausgeführt."));
assert!(code.applies(Meldungstyp::Erlm, GeschaeftsfallCode::Pv));

// A code from the newer online table that the handbook lacks is kept, never rejected.
assert!(matches!(Meldungscode::parse("7999"), Meldungscode::Other(_)));

// What an ABBM with this code means for the order: here, closed for good.
let outcome = Meldungscode::parse("1198").abbm_outcome(GeschaeftsfallCode::Lae, AbbmPhase::AfterAbm);
assert_eq!(outcome, AbbmOutcome::Closed);
# Ok(())
# }
```
{% </rust> %}

## Receipt codes

The technical receipt (TEQ) carries `OK` or one of five error codes, also available as named
constants:

| Code | Constant | Means |
|---|---|---|
| OK | `Meldungscode::Ok` | received and readable |
| 0990 | `FUNKTION_NICHT_UNTERSTUETZT` | function not supported yet |
| 0991 | `UEBERLASTET` | interface overloaded, try again later |
| 0993 | `VERSION_FEHLERHAFT` | major release not supported |
| 0995 | `DATENSTRUKTUR_UNZULAESSIG` | schema or structure check failed |
| 0999 | `TECHNISCHER_FEHLER` | technical error at the receiver |

A read error names the receipt code to answer with: `xml::ReadError::teq_code()`.

## What an ABBM means

`Abbm::outcome(fall, phase)` (or `Meldungscode::abbm_outcome`) reads the code sets the EPKs of
Anlage 7 V.13 draw for each business case and phase (`AbbmPhase::BeforeQeb`, `AfterQeb`,
`AfterAbm`, `Storno`, `Terminverschiebung`):

| `AbbmOutcome` | Means |
|---|---|
| `Resend` | „Auftrag korrigieren": correct the order and send it again, as a new order |
| `Closed` | „Auftrag abschließen": the order is closed for good |
| `Continues` | the Storno or Terminverschiebung failed; the order continues unchanged |
| `Unknown` | the EPK draws no ABBM with this code in this business case and phase |

## All codes

„all" means every business case, including `KUE-LE`. The texts are those of the handbook
(two misprinted rows, `1586` and `2001`, take the fact sheets' texts; see
[interpretations](@/docs/reference/scope.md#interpretations)).

| Code | Text | Message types | Business cases |
|---|---|---|---|
| OK | OK. | TEQ | all |
| 0000 | Keine Änderung zum Auftrag. | QEB, ABM | NEU, KUE-AG, AEN-LMAE, LAE, PV, EST |
| 0001 | Eine Aktivierung am VLT dieser ABM ist nicht möglich. Es müssen erst weitere Netztechnische Arbeiten ausgeführt werden. | ABM | NEU, KUE-AG, AEN-LMAE, LAE, PV, EST |
| 0002 | Der gewünschte Schalttermin liegt außerhalb des betrieblich möglichen Zeitrahmens oder ist kein vertraglicher Arbeitstag. | ABM | NEU, KUE-AG, AEN-LMAE, LAE, PV |
| 0003 | Der Leistungserbringer hat zum gewünschten Schalttermin keine ausreichenden Kapazitäten. | ABM | NEU, KUE-AG, AEN-LMAE, LAE, PV |
| 0005 | Der Ausführungstermin wurde vom Leistungserbringer manuell geändert. | ABM | NEU, KUE-AG, AEN-LMAE, LAE, PV |
| 0007 | Neuterminierung durch/mit Endkunde. | ABM | NEU |
| 0009 | Der Bau der Endleitung/ Konnektivität wurde hergestellt. | ABM | NEU |
| 0010 | Auftrag ausgeführt. | ERLM, ENTM, ERLM-PV, ENTM-PV | all |
| 0011 | Montage beim Endkunden erforderlich. | ABM | NEU, LAE, PV |
| 0012 | Stornierung erfolgreich. | ERLM | NEU, KUE-AG, AEN-LMAE, LAE, PV, EST |
| 0013 | Montage beim Endkunden inkl. Bau der Endleitung erforderlich. | ABM | NEU, LAE, PV |
| 0014 | Montage beim Endkunden mit Ende-zu-Ende-Prüfung erfolgreich ausgeführt. | ERLM, ERLM-PV | NEU, AEN-LMAE, LAE, PV, EST |
| 0015 | Endkunde hat die Bereitstellung der Leistung bestätigt. Anschluss störungsfrei in Betrieb. | ERLM-K | NEU, LAE, PV |
| 0016 | Die vereinbarte Leistung wird nur eingeschränkt bereitgestellt. | ERLM, ERLM-PV | NEU, AEN-LMAE, LAE, PV, EST |
| 0020 | Es liegt uns ein Auftrag zum Anbieterwechsel vor. Wir bitten um Antwort. Erhalten wir keine Antwort, wird der Auftrag abgebrochen. | AKM-PV | PV |
| 0021 | Antwort des abgebenden Providers. | RUEM-PV | PV |
| 0030 | Der Anbieterwechsel erfolgt zum angegebenen Termin. | ABM-PV | PV |
| 0040 | Der Auftrag zum Wechsel wurde zurückgezogen bzw. abgebrochen. Ihr Vertrag bleibt unverändert bestehen. | ABBM-PV | PV |
| 0041 | Der abgebende Provider hat die Kündigung seines Bestandes abgelehnt. | ABBM | PV |
| 0042 | Der abgebende Provider hat auf die Ankündigungsmeldung zum Providerwechsel nicht geantwortet. | ABBM | PV |
| 0044 | Das beim Vorleister beauftragte Produkt wurde abgelehnt. | ABBM | NEU |
| 0070 | Der Auftrag wird manuell bearbeitet. Die Bearbeitungszeit kann sich daher verlängern. | VZM | NEU, KUE-AG, AEN-LMAE, LAE, PV, EST |
| 0073 | Information zum Auftrag | ZWM-LE | NEU, EST |
| 0074 | Der Auftrag befindet sich weiter in Bearbeitung. | ZWM-LE | NEU, EST |
| 0075 | Der Leistungserbringer wartet weiter auf den Ausführungs-, Endkundentermin. | ZWM-LE | NEU, EST |
| 0990 | Funktion aktuell nicht unterschützt, Freigabe wird gesondert bekannt gegeben. | TEQ | all |
| 0991 | Die Schnittstelle ist derzeit überlastet, bitte versuchen Sie es zu einem späteren Zeitpunkt erneut. | TEQ | all |
| 0993 | Versionsnummer fehlerhaft. | TEQ | all |
| 0995 | Datenstruktur unzulässig. | TEQ | all |
| 0999 | Technischer Fehler. | TEQ | all |
| 1001 | Die Anschrift ist nicht bekannt. | ABBM | NEU |
| 1003 | Der Kundenwunschtermin liegt außerhalb des zulässigen Zeitrahmens. | ABBM | NEU, KUE-AG, AEN-LMAE, LAE, PV |
| 1006 | Der Auftrag ist derzeit nicht möglich. Es wurden Änderungen am Anschluss beauftragt. Das beauftragte Produkt kann erst nach Erledigung dieses Auftrags erteilt werden. | ABBM | NEU, KUE-AG, AEN-LMAE, LAE |
| 1007 | Bei dem vorliegenden Geschäftsfall ist das Änderungskennzeichen für dieses Produkt nicht zulässig. | ABBM | NEU, KUE-AG, AEN-LMAE, LAE, PV, EST |
| 1008 | Die bestellte Konfiguration besteht bereits. | ABBM | AEN-LMAE, LAE |
| 1009 | Das Leitungsmerkmal ist nicht möglich. | ABBM | NEU, AEN-LMAE, LAE, PV |
| 1010 | Es ist bereits Bestand des Auftraggebers vorhanden. | ABBM | NEU, AEN-LMAE, LAE, PV |
| 1011 | Es ist bereits Bestand eines anderen Providers vorhanden. | ABBM | NEU |
| 1012 | Es liegt bereits ein Auftrag für diesen Anschluss vor. Ihr Auftrag kann erst nach Erledigung des noch offenen Auftrags erteilt werden. Das Erledigungsdatum wird zusätzlich übergeben. | ABBM | NEU, KUE-AG, AEN-LMAE, LAE, PV, EST, GET, SET |
| 1013 | Externe Auftragsnummer ist nicht eindeutig oder nicht korrekt. | ABBM | NEU, KUE-AG, AEN-LMAE, LAE, PV, EST, GET, SET |
| 1014 | Die Vertragsnummer ist unbekannt. | ABBM | KUE-AG, AEN-LMAE, LAE, EST, GET, SET |
| 1018 | Der Besteller ist nicht berechtigt für den Kunden zu bestellen. | ABBM | all |
| 1020 | Das Zeitfenster ist nicht vereinbar. | ABBM | NEU, KUE-AG, AEN-LMAE, LAE, PV, EST |
| 1023 | Ihr Auftrag ist derzeit aus technischen Gründen nicht bearbeitbar. Bitte versuchen Sie es zu einem späteren Zeitpunkt erneut. | ABBM | NEU, KUE-AG, AEN-LMAE, LAE, PV, GET, SET |
| 1031 | Die Anlage enthält einen maliziösen Code. | ABBM | NEU, KUE-AG, AEN-LMAE, LAE, PV, EST |
| 1032 | Zeitfenster passt nicht zum KWT. | ABBM | NEU, KUE-AG, AEN-LMAE, LAE, PV |
| 1034 | Rufnummer und/oder E-Mailadresse des Endkunden fehlen oder sind falsch angegeben. | ABBM | NEU, PV |
| 1050 | Bestandsvalidierung fehlgeschlagen. | ABBM | NEU, KUE-AG, AEN-LMAE, LAE, EST, GET, SET |
| 1051 | Der Auftrag ist bei vorhandener Bestandssituation nicht möglich. | ABBM | AEN-LMAE, LAE |
| 1085 | Eine Terminverschiebung ist nicht möglich. | ABBM | NEU, KUE-AG, PV |
| 1086 | Kundenauftrag ist nicht stornierbar, da Stornierung bereits angestoßen. | ABBM | NEU, KUE-AG, AEN-LMAE, LAE, PV |
| 1087 | Eine Stornierung ist nicht möglich. | ABBM | NEU, KUE-AG, AEN-LMAE, LAE, PV |
| 1101 | Produkt an diesem Anschlusstyp nicht möglich. | ABBM | NEU, LAE |
| 1102 | Produkt oder Geschäftsfall nicht zulässig. | ABBM | NEU, AEN-LMAE, LAE, PV |
| 1104 | Das Produkt, auf das sich der Auftrag bezieht, liegt im Bestand nicht vor. | ABBM | AEN-LMAE, LAE |
| 1105 | Das Leistungsmerkmal ist mit dem Produkt nicht kombinierbar. | ABBM | NEU, AEN-LMAE, LAE, PV |
| 1192 | Ein Anbieterwechsel wird der Kündigung vorgezogen. | ABBM | KUE-AG |
| 1194 | Der Endkunde verweigert die Herstellung. | ABBM | NEU |
| 1198 | Es wurde kein neuer Ausführungstermin übermittelt. | ABBM | NEU, LAE, PV |
| 1199 | Die Voraussetzung für die Bereitstellung ist nicht mehr gegeben. Der Auftrag wurde abgebrochen. | ABBM | NEU |
| 1200 | Es wurden nicht alle Aufträge zur Auftragsklammer geliefert. | ABBM | NEU, AEN-LMAE, LAE, PV |
| 1201 | Auftrag abgewiesen aufgrund der Abweisung eines anderen Auftrages mit der gleichen Auftragsklammer. | ABBM | NEU, AEN-LMAE, LAE, PV |
| 1300 | Das beauftragte Produkt ist technisch nicht möglich. | ABBM | NEU, LAE |
| 1301 | Das beauftragte Produkt ist aufgrund der Leitungsdämpfung nicht möglich. | ABBM | NEU, LAE, PV |
| 1309 | Das beauftragte Produkt ist aufgrund Portmangel nicht möglich. | ABBM | NEU |
| 1310 | Das beauftrage Produkt ist aufgrund Leitungsmangel nicht möglich. | ABBM | NEU |
| 1311 | Standortangabe nicht eindeutig oder ausreichend. | ABBM | NEU |
| 1312 | "Freitext„. | ABBM | NEU |
| 1320 | HomeID am Standort nicht verfügbar. | ABBM | NEU |
| 1500 | Gebietsentscheidung zum Ausbau wurde getroffen. Verbindliche Auftragsannahme. | VZM | NEU |
| 1501 | Das Gebiet wird nicht ausgebaut. | ABBM | NEU |
| 1502 | Die Auskundung hat stattgefunden. | VZM | NEU |
| 1503 | Das bestellte Produkt kann mit der vorgesehenen Endleitung nicht bereitgestellt werden. | ABBM | NEU |
| 1504 | Das bestellte Produkt kann mit dem Ergebnis der Auskundung nicht bereitgestellt werden, da der Standort nicht ausgebaut wird. | ABBM | NEU |
| 1505 | Der Standort wird aufgrund Eigentümer-Ablehnung nicht ausgebaut. | ABBM | NEU |
| 1506 | Der Leistungserbringer vereinbart einen neuen Termin mit den Endkunden. | VZM | NEU |
| 1507 | Die Konnektivität konnte nicht hergestellt werden. | ABBM | NEU |
| 1508 | Die Konnektivität wurde hergestellt - Bitte Aktivierung des Zielproduktes beauftragen. | VZM | NEU |
| 1509 | Annahme des Auftrages zur Konnektivitäts-Herstellung für den Standort noch nicht möglich. | ABBM | NEU |
| 1510 | Die Konnektivität ist für den Standort bereits hergestellt. | ABBM | NEU |
| 1511 | Für den Standort ist derzeit kein Ausbau geplant. | ABBM | NEU |
| 1512 | Der Eigentümer hat dem Ausbau des Standorts zugestimmt. | VZM | NEU |
| 1513 | Der Auskundungstermin für den Standort wurde festgelegt. | VZM | NEU |
| 1514 | Die Installation des Hausübergabepunktes für den Standort ist abgeschlossen. | VZM | NEU |
| 1515 | Verbindliche Auftragsannahme. | VZM | NEU |
| 1516 | Der Auskundungstermin für den Standort wird geplant und mit dem Eigentümer abgestimmt. | VZM | NEU |
| 1517 | Der NE3-Installationstermin (Hausübergabepunkt) wird geplant und mit dem Eigentümer abgestimmt. | VZM | NEU |
| 1518 | Der NE3-Installationstermin (Hausübergabepunkt) wurde durch den Eigentümer ausgewählt. | VZM | NEU |
| 1519 | Der NE4-Installationstermin (Netzabschluss) wird geplant und mit dem Endkunden abgestimmt. | VZM | NEU |
| 1520 | Fehlerhafte oder fehlende Angabe der Gebäudeteil-ID. | ABBM | NEU |
| 1521 | Falsche Gebäudeteil-ID bei Auftragsausführung erkannt. | ABBM | NEU |
| 1522 | Die Kontaktdaten (Endkunde / Eigentümer) sind nicht vollständig. | ABBM | NEU |
| 1523 | Zu der Lokation existiert bereits ein weiterer, verhindernder Auftrag zur Herstellung der Konnektivität. | ABBM | NEU |
| 1524 | Die Eigentümer-Daten sind nicht korrekt, der Auftrag kann nicht prozessiert werden. | ABBM | NEU |
| 1525 | Der Endkunde lehnt die Installation ab, Konnektivität konnte nicht hergestellt werden. | ABBM | NEU |
| 1526 | Die Eigentümer-Daten wurden nicht angegeben. | ABBM | NEU |
| 1530 | Für das bestellte Produkt besteht keine Konnektivität. Es muss die Herstellung der Konnektivität beauftragt werden. | ABBM | NEU |
| 1531 | Der Leistungserbringer informiert den Endkunden über den Ausbautermin. | VZM | NEU |
| 1532 | Der Leistungserbringer informiert den Auftraggeber über den Ausbautermin. | VZM | NEU |
| 1533 | Fehlende Gestattung oder fehlende Grundstückseigentümer-Erklärung. | ABBM | NEU |
| 1534 | Der Ausbauzeitraum für den Standort hat sich verschoben. | VZM | NEU |
| 1535 | Daten zum ausführenden Auftragnehmer enthalten. | VZM | NEU |
| 1536 | Interaktion Auftraggeber mit Endkunde notwendig. | VZM | NEU, EST |
| 1537 | Der Ausbau verzögert sich, Grund: Bauverhinderung. | VZM | NEU |
| 1538 | Der Ausbau verzögert sich, Grund: Höhere Gewalt. | VZM | NEU |
| 1539 | Der Ausbau verzögert sich, Grund: Endkunde nicht erreichbar. | VZM | NEU |
| 1540 | Der Ausbau verzögert sich, Grund: Eigentümer nicht erreichbar. | VZM | NEU |
| 1541 | Der Ausbau verzögert sich, Grund: Verzug durch Endkunde. | VZM | NEU |
| 1542 | Der Ausbau verzögert sich, Grund: Verzug durch Eigentümer. | VZM | NEU |
| 1543 | Der Ausbau verzögert sich. | VZM | NEU |
| 1544 | Der Ausbau wird fortgesetzt. | VZM | NEU |
| 1545 | Der Auskundungstermin wurde storniert. | VZM | NEU |
| 1546 | Der NE3-Installationstermin (Hausübergabepunkt) wurde storniert. | VZM | NEU |
| 1547 | Der NE4-Installationstermin (Netzabschluss) wurde aufgrund eines Auftragnehmer-Wechsels storniert. | VZM | NEU |
| 1548 | Der NE4-Installationstermin (Netzabschluss) wurde storniert, da die Installation in einem abweichenden Gebäudeteil erforderlich ist. | VZM | NEU |
| 1549 | Der NE4-Installationstermin (Netzabschluss) wurde durch den Endkunden storniert. | VZM | NEU |
| 1550 | Der NE4-Installationstermin (Netzabschluss) wurde durch den Auftragnehmer storniert. | VZM | NEU |
| 1551 | Der NE4-Installationstermin (Netzabschluss) wurde storniert. | VZM | NEU |
| 1552 | Der Ausbau verzögert sich, Grund: Warten auf Eigentümerzustimmung. | VZM | NEU |
| 1553 | Konnektivität wurde hergestellt - bitte Aktivierung des Zielproduktes beauftragen. | ABBM | NEU |
| 1554 | Die Konnektivität für Ihren Endkunden wird ab jetzt im Rahmen eines Bulk Ausbaus des Gebäudes hergestellt | VZM | NEU |
| 1555 | Es konnte keine Eigentümer-Zustimmung für den Ausbau eingeholt werden. | ABBM | NEU |
| 1556 | Die Konnektivität konnte aus Gründen höherer Gewalt nicht hergestellt werden. | ABBM | NEU |
| 1557 | Die Konnektivität konnte nicht hergestellt werden, da der Endkunde nicht erreicht werden konnte. | ABBM | NEU |
| 1558 | Die Konnektivität konnte nicht hergestellt werden, da der Eigentümer nicht erreicht werden konnte. | ABBM | NEU |
| 1559 | Der Endkunde wird zur Ermittlung der Home-ID kontaktiert - Konnektivität wurde hergestellt. | VZM | NEU |
| 1560 | Der Hausstich wurde durchgeführt. | VZM | NEU |
| 1561 | Der NE4-Installationstermin (Netzabschluss) wurde storniert, da die Installation im Rahmen eines Bulk-Ausbaus des Gebäudes neu terminiert werden muss. | VZM | NEU |
| 1562 | Der Ausbau in dem Gebiet wurde gestartet. | VZM | NEU |
| 1563 | Die Konnektivität für Ihren Endkunden wird ab jetzt im Rahmen eines regulären Auftrags zur Konnektivität hergestellt. | VZM | NEU |
| 1565 | Reminder: Mitwirkung erforderlich! Letztmalige Frist von 14 KT; wenn nach Ablauf keine Abhilfe geschaffen wird, erfolgt der Abbruch des Auftrags. Rechtlich wird dies eine Kündigung des Einzelvertrags bedeuten. | VZM | NEU |
| 1566 | Abbruch des Auftrags aufgrund fehlender Mitwirkung. Rechtlich bedeutet dies eine Kündigung des Einzelvertrags. | ABBM | NEU |
| 1567 | Der Auftrag ist ab sofort im Glasfaserportal sichtbar. | VZM | NEU |
| 1568 | Vorabinformation: Anstehender Technologiewechsel. | VZM | NEU |
| 1569 | Der Glasfaser-Ausbau verzögert sich, Grund: Auftragsstorno wird geprüft. | VZM | NEU |
| 1570 | Die Aktivierung des Endkundenanschlusses erfolgt unabhängig zum Ausbau des Netzabschlusses. | VZM | NEU |
| 1571 | Die Aktivierung des Endkundenanschlusses kann nicht unmittelbar nach Ausbau des Netzabschlusses erfolgen. | VZM | NEU |
| 1572 | Der Ausbau des Netzabschlusses ist erfolgt. | VZM | NEU |
| 1573 | Die Herstellung der Konnektivität wird bearbeitet sobald die Nachverdichtung beginnt. | VZM | NEU |
| 1574 | Der Fortschritt der Herstellung der Konnektivität bedingt bei einem Storno durch den Auftraggeber eine Stornogebühr. | VZM | NEU |
| 1575 | HomeID Clearing durch Leistungerbringer notwendig. | VZM | NEU |
| 1576 | Für die Aktivierung des Endkundenanschlusses ist ein Endkundentermin notwendig. | VZM | NEU |
| 1577 | Die HomeID wurde für den Netzabschluss ermittelt. | VZM | NEU |
| 1578 | Ausbauzustand der Wohnung. | VZM | NEU |
| 1579 | Der Übergabepunkt des Anschlusses wurde zentral an der Glasfasereinführung im Gebäude hergestellt und nicht in der Nutzungseinheit. | VZM | NEU |
| 1580 | Der Termin zur Durchführung des Hausstiches wurde durch den Eigentümer ausgewählt. | VZM | NEU |
| 1581 | Der Termin zur Steigleitungsinstallation wurde durch den Eigentümer ausgewählt. | VZM | NEU |
| 1582 | Die Steigleitungsinstallation wurde durchgeführt. | VZM | NEU |
| 1583 | Die Wegesicherung wurde durchgeführt. | VZM | NEU |
| 1584 | Die Prüfung auf Kampfmittelfreiheit wurde abgeschlossen. | VZM | NEU |
| 1585 | Der Eigentümer wird zur Einholung der Zustimmung kontaktiert. | VZM | NEU |
| 1586 | Der Medienkonverter wird vom Leistungserbringer an den Endkunden versandt. | VZM | NEU |
| 2001 | Die Leistungsnummer ist nicht plausibel oder nicht eindeutig. | ABBM | NEU, KUE-AG, AEN-LMAE, LAE, PV, GET, SET |
| 2017 | Produkt aufgrund Vectoring nicht erlaubt. | ABBM | NEU, AEN-LMAE, LAE, PV |
| 5000 | Zahl der in den Planungsabsprachen zum Auftragseingangsdatum abgestimmten Menge an Aufträgen wurde überschritten. | QEB | NEU |
| 5001 | Die Zahl der Aufträge zum geplanten Ausführungstermin liegt über der in Planungsabsprachen festgelegten Menge. | ABBM | NEU, GET, SET |
| 5002 | Die Zahl der Aufträge zum geplanten Ausführungstermin liegt über der in der Planungsabsprache festgelegten Menge. Der Auftrag wurde auf den betrieblich nächstmöglichen Termin verschoben. | ABM | NEU |
| 6000 | Endkunde hat Herstellung zurückgestellt/verweigert. | TAM, MTAM | NEU, LAE, PV, EST |
| 6001 | Endkunde nicht angetroffen. | TAM, MTAM | NEU, LAE, PV, EST |
| 6002 | Nicht ausführbar aus Endkundengrund (z.B. APL nicht zugänglich). | TAM, MTAM | NEU, EST |
| 6003 | Nicht ausführbar wegen falscher Angaben des Endkunden. | TAM, MTAM | NEU, LAE, PV, EST |
| 6004 | Ausführung wegen lokalem Feiertag nicht möglich | TAM, MTAM | NEU, LAE, PV, EST |
| 6005 | Höhere Gewalt. | TAM, MTAM | NEU, LAE, PV, EST |
| 6006 | Nicht ausführbar aus sonstigem Grund. | TAM, MTAM | NEU, LAE, PV, EST |
| 6007 | Endleitung fehlt vollständig. | TAM, MTAM | NEU, LAE, PV |
| 6008 | Endleitung fehlt teilweise. | TAM, MTAM | NEU, LAE, PV |
| 6009 | Endleitung ist defekt. | TAM, MTAM | NEU, LAE, PV, EST |
| 6010 | Herstellung der Endleitung nur nach Angebot. | TAM, MTAM | NEU, LAE, PV |
| 6011 | Arbeiten an der Endleitung erforderlich. Endkunde lässt Bau der Abschlusseinrichtung nicht zu. | TAM, MTAM | NEU, LAE, PV |
| 6012 | Termin am nächsten Werktag (Mo. - Sa.). | ABM, ABM-PV | NEU, KUE-AG, LAE, PV |
| 6013 | Nicht ausführbar aus sonstigem technischen Grund. | TAM, MTAM | NEU, LAE, PV |
| 6015 | TAM für Techniker Leistungserbringer | TAM, MTAM | NEU, LAE, PV, EST |
| 6016 | TAM für Techniker Vorleister | TAM, MTAM | NEU, LAE, PV, EST |
| 6021 | Technikertermin hat stattgefunden aber Abschluss der Installation durch Techniker ist nicht mehr vor Kundenwunschtermin (KWT) möglich. Bitte mit dem Kunden einen neuen, späteren KWT vereinbaren. | TAM, MTAM | NEU |
| 6022 | Konnektivität hergestellt. Terminierung zur Aktivierung des beauftragten Produktes erforderlich | TAM, MTAM | NEU |
| 6023 | Das Zeitfenster ist nicht vereinbart oder ungültig | TAM, MTAM | NEU |
| 6050 | Keine Terminvereinbarung mit Endkunden möglich. | TAM | EST |
| 7000 | Fehler beseitigt. | ERLM | EST |
| 7003 | Kein Fehler festgestellt. | ERLM | EST |
| 7005 | Kein Fehler festgestellt (Techniker hat beim Endkunden geprüft). | ERLM | EST |
| 7006 | Rückfall. Eine Bestandsänderung wird durchgeführt, bzw. eine Leistungsänderung ist notwendig. | ERLM | EST |
| 7009 | Kein Fehler festgestellt (Endgerät nicht kompatibel). | ERLM | EST |
| 7010 | Zurückweisung. | ERLM | EST |
| 7011 | Referenzauftrag konnte nicht ermittelt werden. | ABBM | EST |
| 7012 | Reklamation nicht zulässig. | ABBM | EST |
| 7013 | Entstörauftrag abgewiesen. Fehlende Berechtigung für zu entstörendes Produkt. | ABBM | EST |
| 7014 | Kundenwunschtermin liegt in der Vergangenheit. | ABBM | EST |
| 7015 | Es wurde kein Kundentermin übermittelt. | ABBM | EST |
| 7020 | Terminvereinbarung mit dem Kunden erfolgt. | TBK-LE | EST |
| 7023 | Endkunde trotz Terminvereinbarung nicht angetroffen. | ERLM | EST |
| 7024 | Ihr Anschluss ist ggf. von einer Massenstörung betroffen. | ZWM-LE | EST |
| 7030 | Hinderungsgrund für letzten Endkundentermin ist beseitigt. | TBK-AG | EST |
| 7037 | Servicetechnikereinsatz ohne Endkundentermin. | TBK-LE | EST |
| 7038 | Termin wurde durch Endkunden verschoben. | TBK-LE | EST |
| 7039 | Der mit der EST übermittelte Endkundentermin wird voraussichtlich wahrgenommen. | TBK-LE | EST |
| 7040 | Ergänzende Information zum Auftrag. | ZWM-AG | EST |
| 7041 | Der Auftrag ist weiter offen. | ZWM-AG | EST |
| 7042 | Der Auftraggeber wartet weiter auf den Endkundentermin. | ZWM-AG | EST |
| 7500 | Die Line ist nicht synchron, eine Diagnose kann nicht durchgeführt werden. | ABBM | GET |
| 7504 | Zeitüberschreitung beim Warten auf das Diagnoseergebnis. Bitte versuchen Sie es zu einem späteren Zeitpunkt noch einmal. | ABBM | GET, SET |
| 7508 | Die Anzahl der Diagnoseanfragen auf das betroffene Netzelement ist zu hoch. | ABBM | GET |
| 7522 | Es konnte keine Verbindung zum Medienkonverter/Modem aufgebaut werden. | ABBM | NEU, EST, SET |
| 7524 | Die Medienkonverter-/Modem-ID ist nicht bekannt. | ABBM | NEU, EST, SET |
| 8005 | Die Auftragsbestätigung verzögert sich. | VZM | NEU, LAE, PV |
| 8006 | Der Anschluss muss abgeschaltet werden. | KDLE | KUE-LE |
| 8144 | LineID ist nicht bekannt. | ABBM | KUE-AG, AEN-LMAE, LAE, PV, EST, GET, SET |
| 8500 | Die Auftragsbestätigung wurde vom Leistungserbringer nachträglich verändert, der Ausführungstermin bleibt bestehen. | ABM, ABM-PV | NEU, KUE-AG, LAE, PV |
| 8501 | Der geplante Ausführungstermin wurde durch den Leistungserbringer nachträglich verschoben. | ABM, ABM-PV | NEU, KUE-AG, LAE, PV |
| 8502 | Zur Ausführung des Auftrags wird zusätzliche Hardware benötigt | ABM | NEU, PV |
| 8510 | Die Übernahme des Vorleistungsproduktes war nicht möglich. | ABBM | NEU |
| 8600 | "Freitext" | ABBM | NEU, KUE-AG, LAE, PV, EST, GET, SET |
| 8601 | "Freitext" | ABBM | NEU, KUE-AG, LAE, PV, EST, GET, SET |
| 8610 | Es hat noch kein Technikertermin stattgefunden und es ist kein Technikertermin vor Kundenwunschtermin (KWT) möglich. Bitte mit dem Kunden einen neuen, späteren KWT vereinbaren und einen neuen Auftrag erstellen. | ABBM | NEU |
| 9999 | Reserviert für interne Zwecke. |  |  |
