+++
title = "Scope and interpretations"
description = "Which WBCI and S/PRI documents and versions the crates implement, what they leave to your application, and every place they interpret the specifications."
weight = 1

[extra]
nav = "Scope and interpretations"
+++

## Implemented

| Document | Version |
|---|---|
| WBCI Schnittstellenbeschreibung | interface 2.0 (document 2.2, 02.01.2023) |
| XML schema and WSDL (Anlage 4) | V2.00, schema files 01.00.02; namespaces `http://www.vatm.de/wbci/v1/…` |
| Anlage 1 „Spezifikation Vorabstimmung" | V5.4.1 (31.07.2024) |
| Arbeitshandbuch WBCI | 2.10 (April 2024) |
| Infodokument WBCI Phonetische Prüfung | v1.1 (08.07.2021) |
| Anlage Beispiel Prüfbaum | V1.1 (10.04.2019) |
| Providervereinbarung S/PRI | V4.4 (valid from 19.03.2024) |
| S/PRI Anlage 7 „EPKs der Geschäftsfälle" | V.13 (09.10.2026) |
| S/PRI XML schema (Anlage 3) | 4.00.03 (19.12.2014), for all 4.x; namespaces `http://spri.telekom.de/oss/v4/…` |
| Arbeitshandbuch S/PRI 4.x | V2.9.5 (message codes §6.3/§6.4) |
| Struktur und Semantik der LineID | V1.4 |

The specifications are published by the AK S&P (Arbeitskreis Schnittstellen & Prozesse,
hosted by VATM and BUGLAS) at [ak-spri.de](https://ak-spri.de/informationen-zur-wbci/wbci-spezifikationen/).
They are copyrighted; this project carries its own types and its own synthetic test data and
redistributes no part of them. Conformance to both schemas is [proven on every CI run](@/docs/reference/conformance.md)
against the published files, fetched and pinned by SHA-256.

## Left to the application

Transport (SOAP 1.1 over HTTPS), WS-Security signing and verification, TLS and IP
allow-listing, persistence, escalation by spreadsheet and e-mail, conformance certification,
and the search of your customer database (the [check](@/docs/wbci/pruefung.md) compares the
record you found).

## Interpretations

| Question | This crate | Alternative |
|---|---|---|
| What is a working day? | Mon–Fri without nationwide holidays (`Calendar::wbci`) | `Calendar::mon_fri` |
| „24 Stunden (bezogen auf Arbeitstage)" | 24 hours of elapsed time on working days only | — |
| The schema's character pattern admits U+007F, its prose does not | the pattern | — |
| A request's `Absender` contradicts its type | refused when reading (0995) | — |
| Ids from `allocate` | nine decimal digits (`V000000013` for 13) | the schema also admits A–Z |
| A business answer before the receipt | counts as delivery (WBCI 2.0 §12.3.3) | — |
| The phonetic algorithm for names (left to each company) | Kölner Phonetik per word, after the Infodokument's normalisation | Soundex, Double Metaphone |
| „Str." in street names | read as „Straße" | no normalisation (none is specified) |
| RNG with numbers: the table says „für die falschen", §2.3 says all numbers | all your numbers, when one given number was right and the name matched | only the wrong ones |
| Contract end on a non-switching day | the next later working day | the previous one |
| S/PRI's `VorabstimmungIdType` and `LineIdType` are length-only | the stricter WBCI/AK S&P formats (always within S/PRI's lengths) | plain strings |
| S/PRI attachment size (no limit is specified) | 1 MiB decoded by default, configurable | unbounded |
| S/PRI code 7024 („ZWM" without a side) | ZWM-LE: a mass-outage notice comes from the network owner | ZWM-AG |
| S/PRI code list rows `1586` and `2001` (misprinted) | the texts of the fact sheets | the list's texts |
| S/PRI release number | written with two digits (`04`) | `4` |
| S/PRI certificate issuer and serial | the WBCI character set (the values are shared with WBCI) | — |
| S/PRI Storno and Terminverschiebung limit: 36 hours (handbook) or 3 Arbeitstage (fact sheets) | 36 hours of elapsed time before 0:00 of the binding date | 3 Arbeitstage |
| S/PRI Zeitfenster `1`/`2` „Werktags" | Monday to Saturday | Monday to Friday |
| S/PRI wish date in the past outside EST (the fact sheet of 7014 marks EST only) | ABBM 1003 („außerhalb des zulässigen Zeitrahmens") | no code |
| S/PRI Geschäftsfallart of a PV | `Endkundenanbieterwechsel` or `Bereitstellung` (the contract treats a PV as a provisioning) | `Endkundenanbieterwechsel` only |
| S/PRI KUE-AG with a Vorabstimmungs-ID | a provider switch with WITA: without a WITA contract number a warning | no check |
| S/PRI ABBM 1006/1012 „Erledigungstermin offener Auftrag" | carried in `wiedervorlagetermin` | no check |
| S/PRI RUEM-PV antwortcodes (`LnB` in the handbook, `LNB` in the EPK) | compared ignoring case; a consenting RUEM-PV with one is a contradiction | exact spelling |
| S/PRI order whose LineID equals its Vorabstimmungs-ID (LineID V1.4 §3.1.3) | an error, answered with 8144 („LineID ist nicht bekannt"): line codes and pre-agreement ids are distinct ranges | no check |
| S/PRI receipt with `OK` and an error code | an error | allowed |
| § 59 TKG continuity across WBCI and S/PRI | the S/PRI completion date stands in for the end of the outage (`wechsel.kontinuitaet`) | — |

Report a different reading of the specifications as an issue: the crate follows the text, and
corrections to an interpretation are welcome.
