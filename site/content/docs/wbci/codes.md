+++
title = "WBCI message codes"
description = "All 36 WBCI message codes (Meldungscodes) with official text, short name, message type and the business cases they answer."
weight = 7

[extra]
nav = "Message codes"
seo_title = "WBCI Meldungscodes: all 36 message codes · telco-interop"
+++

`Meldungscode` holds the codes of the Arbeitshandbuch WBCI 2.10 §5.3. `text()` gives the
official text, `short()` the abbreviation (RNG, ZWA, …), `meldungstypen()` the message kinds
that carry it, and `applies(typ, fall)` whether it may answer a business case (AH §2). A code
a peer sends that is not on the list is kept as `Meldungscode::Other`, and validation flags it.

| Code | Text | Message | Answers |
|---|---|---|---|
| OK | OK. | TEQ | all |
| 0993 | Versionsnummer fehlerhaft | TEQ | all |
| 0995 | Datenstruktur ungültig | TEQ | all |
| 0999 | Technischer Fehler | TEQ | all |
| 8001 | Auftragsbestätigung (ZWA) | RUEM-VA | VA-* |
| 8002 | Auftragsbestätigung (NAT) | RUEM-VA | VA-* |
| 8003 | Terminverschiebung ok | ERLM | TVS-VA |
| 8004 | Storno ok | ERLM | STR-* |
| 8010 | Antwort des aufnehmenden Anbieters | AKM-TR | VA-KUE-* |
| 8103 | Rufnummer nicht geschaltet (RNG) | ABBM | VA-* |
| 8104–8107 | Adresse abweichend PLZ / Ort / Straße / Hausnummer (ADA) | RUEM-VA | VA-KUE-* |
| 8108–8111 | Adresse falsch PLZ / Ort / Straße / Hausnummer (ADF) | ABBM | VA-KUE-* |
| 8112 | Anschlussinhaber falsch (AIF) | ABBM | VA-* |
| 8113 | Anschlussinhaber falsch Vorname (AIFVN) | ABBM | VA-* |
| 8114 | Weitere Anschlussinhaber (WAI) | ABBM | VA-* |
| 8115 | ID wird bereits verwendet | ABBM | all |
| 8116 | Sonstiges (SON) — also: business case not supported | ABBM | all |
| 8117 | Kunde nicht identifizierbar (KNI) | ABBM | VA-* |
| 8118 | Vorabstimmung bereits erfolgt (VAE) | ABBM | VA-* |
| 8140 | Vorabstimmungs ID nicht bekannt | ABBM, ABBM-TR | STR-*; VA-KUE-* |
| 8141 | Terminverschiebung abgelehnt | ABBM | TVS-VA |
| 8142 | Storno abgelehnt | ABBM | STR-* |
| 8143 | WITA Vertragsnummer unbekannt | ABBM-TR | VA-KUE-* |
| 8144 | Line ID unbekannt | ABBM-TR | VA-KUE-* |
| 8145 | WITA Vertragsnummer gehört nicht zur Vorabstimmungs ID | ABBM-TR | VA-KUE-* |
| 8146 | Line ID gehört nicht zur Vorabstimmungs ID | ABBM-TR | VA-KUE-* |
| 8147 | Übernahme der Technischen Ressource nicht möglich | ABBM-TR | VA-KUE-* |
| 8148 | Übernahme der Technischen Ressource wurde bereits bestätigt | ABBM-TR | VA-KUE-* |
| 8149 | Projektkenner unbekannt | ABBM | VA-* |
| 8150 | Neue Willenserklärung des Endkunden (NWE) | ABBM | VA-KUE-* |
