+++
title = "Glossary"
description = "Every German WBCI and S/PRI term explained in English: EKPauf, EKPabg, Vorabstimmung, RUEM-VA, ABBM, AKM-TR, QEB, ABM, TAM, PV, AT, WT, LineID and more."
weight = 3

[extra]
seo_title = "Glossary of WBCI and S/PRI terms (EKPauf, RUEM-VA, ABBM, QEB …) · telco-interop"
+++

The crates keep the protocol's German terms in their type names (`Vorabstimmung`, `RuemVa`,
`Leistungserbringer`), so the code reads like the specification. This page translates them.

## Parties and organisations

| Term | Meaning |
|---|---|
| **AK S&P** | *Arbeitskreis Schnittstellen & Prozesse*: the industry working group, hosted by VATM and BUGLAS, that specifies WBCI and S/PRI. |
| **EKP** | *Endkundenvertragspartner*: the provider holding the contract with the end customer. |
| **EKPauf** | *aufnehmender EKP*: the gaining provider. Sends the pre-agreement, the AKM-TR and, in S/PRI, the PV order. `Role::EkpAuf`, `Partei::EkpAuf`. |
| **EKPabg** | *abgebender EKP*: the losing provider. Answers the pre-agreement and, in S/PRI, the AKM-PV. `Role::EkpAbg`, `Partei::EkpAbg`. |
| **Auftraggeber** (AG) | S/PRI: the service provider placing an order; in a provider switch the EKPauf. |
| **Leistungserbringer** (LE, NGA, NGAB) | S/PRI: the network owner that provides the line and executes orders. `Partei::Netzbetreiber` in `Wechsel`. |
| **abgebender Provider** | S/PRI: the losing provider of a `PV`, informed with AKM-PV and answering with RUEM-PV. |
| **Endkunde**, **Anschlussinhaber** | The end customer and the line holder(s); further holders are *weitere Anschlussinhaber*. |
| **Carrier code** | A provider's identifier such as `DEU.EKP1` (`CarrierCode`); it prefixes the ids the provider allocates. |

## WBCI requests

| Term | Meaning |
|---|---|
| **VA**, **Vorabstimmung** | The pre-agreement: the gaining provider's request to agree a switch before any line is touched. |
| **VA-KUE-MRN** | *Kündigung mit Rufnummernportierung*: switch with number porting. |
| **VA-KUE-ORN** | *Kündigung ohne Rufnummernportierung*: switch without number porting. |
| **VA-RRNP** | *reine Rufnummernportierung*: number porting only. |
| **STR-AUF** | *Storno Aufhebung*: cancel the pre-agreement (either side). |
| **STR-AEN** | *Storno Änderung*: cancel in order to send a changed pre-agreement (either side). |
| **TVS-VA** | *Terminverschiebung*: the gaining provider postpones the agreed switch date. |

## WBCI messages

| Term | Meaning |
|---|---|
| **TEQ** | *Technische Quittung*: the synchronous technical receipt of every call: `OK`, or 0993 / 0995 / 0999 (WBCI) and 0990 / 0991 too (S/PRI). |
| **RUEM-VA** | *Rückmeldung Vorabstimmung*: the losing provider agrees, with the binding switch date (8001 as requested, 8002 later). |
| **ABBM** | *Abbruchmeldung*: refusal with one or more codes. Exists in WBCI and S/PRI. |
| **ERLM** | *Erledigungsmeldung*: done. In WBCI it confirms a cancellation (8004) or postponement (8003); in S/PRI it reports the service realised. |
| **AKM-TR** | *Ankündigungsmeldung technische Ressource*: the gaining provider announces whether it takes over the line (8010). |
| **ABBM-TR** | The losing provider's refusal of the AKM-TR (e.g. 8144 LineID unknown). |
| **Sicherer Hafen** | The „safe harbour" procedure the gaining provider can request in the AKM-TR. |

## S/PRI orders (business cases)

| Term | Meaning |
|---|---|
| **NEU** | *Bereitstellung*: provision a new line. |
| **KUE-AG** | *Kündigung durch den Auftraggeber*: the service provider cancels a line, in a switch also the losing provider with the Vorabstimmungs-ID. |
| **KUE-LE** | *Kündigung durch den Leistungserbringer*: the network owner cancels, announced with KDLE. |
| **LAE** | *Änderung von Leistungen*: change the service (e.g. product). |
| **AEN-LMAE** | *Änderung von Leistungsmerkmalen*: change features of the service. |
| **EST** | *Entstörung*: fault report and repair. |
| **PV** | *Providerwechsel*: the provider-switch order after a WBCI pre-agreement, carrying its Vorabstimmungs-ID. |
| **GET**, **SET** (DIAG) | Diagnostics: read or set line parameters. |
| **Storno**, **TV** | Cancellation and *Terminverschiebung* (postponement) of a running order. |

## S/PRI messages

| Term | Meaning |
|---|---|
| **QEB** | *Qualifizierte Eingangsbestätigung*: the order is commercially admissible; the first message. |
| **ABM** | *Auftragsbestätigungsmeldung*: the order is confirmed for a binding date (VLT). |
| **VZM** | *Verzögerungsmeldung*: processing takes longer. |
| **TAM**, **MTAM** | *Terminanforderungsmeldung* and its reminder (*Mahn-TAM*): the network owner needs a new appointment with the end customer. |
| **ENTM** | *Entgeltmeldung*: the billing date; the last message of an order. |
| **KDLE** | *Kündigung durch den Leistungserbringer*: the network owner announces a cancellation. |
| **ZWM-LE**, **ZWM-AG** | *Zwischenmeldung*: interim notice of the network owner or the service provider. |
| **TBK-LE**, **TBK-AG** | *Termin beim Kunden*: an appointment at the customer's, from either side. |
| **ERLM-K** | *Erledigungsmeldung Kunde*: the service provider reports the customer-side work done. |
| **AKM-PV** | *Ankündigungsmeldung Providerwechsel*: the network owner announces the switch to the losing provider, with an answer date. |
| **RUEM-PV** | *Rückmeldung Providerwechsel*: the losing provider's answer to the AKM-PV. |
| **ABM-PV**, **ERLM-PV**, **ENTM-PV**, **ABBM-PV** | The ABM, ERLM, ENTM and ABBM of a provider switch, sent to the losing provider. |

## Dates and deadlines

| Term | Meaning |
|---|---|
| **AT** | *Arbeitstag*, working day. WBCI: Monday to Friday without the nationwide public holidays (`Calendar::wbci`). |
| **WT** | *Werktag*: Monday to Saturday without the nationwide public holidays (`Calendar::werktage`), used by S/PRI. |
| **X+n AT**, **X−n AT** | n working days after X (ends 23:59) or before X (ends before 0:00). See [deadlines](@/docs/wbci/deadlines.md). |
| **Kundenwunschtermin** | The date the customer wants to switch, given in the pre-agreement. |
| **Wechseltermin** | The binding switch date the losing provider confirms in the RUEM-VA. |
| **VLT** | *verbindlicher Liefertermin*: the binding date an S/PRI ABM confirms. |
| **Portierungszeitfenster** | The porting window on the switch date: `ZF1` 6–8 o'clock (porting without line takeover), `ZF2` 6–12 o'clock (standard), `ZF3` by bilateral agreement. |

## Identifiers

| Term | Meaning |
|---|---|
| **Vorabstimmungs-ID** | The id of a pre-agreement, e.g. `DEU.EKP1.V000000013`; it travels from WBCI into the S/PRI `PV` order. `VorabstimmungsId`, one type in both crates. |
| **LineID** | The id of a physical line, e.g. `DEU.EKP2.1754639821` (`LineId`); named in the AKM-TR and the PV order. |
| **Portierungskennung** (PKI) | A provider's porting identifier; `PKIauf` of the gaining, `PKIabg` of the losing provider. |
| **ONKZ** | *Ortsnetzkennzahl*: the area code without the leading zero (`30` for Berlin). |
| **Meldungscode** | A message code: four digits with an official text, e.g. 8103 „Rufnummer nicht geschaltet". [WBCI codes](@/docs/wbci/codes.md), [S/PRI codes](@/docs/spri/codes.md). |
| **Externe Auftragsnummer**, **Auftraggebernummer** | S/PRI: the service provider's order number and customer number at the network owner. |
| **WITA-Vertragsnummer** | A Telekom (WITA) contract number, named in an AKM-TR instead of a LineID for Telekom lines. |

## Short names of WBCI codes

| Term | Code | Meaning |
|---|---|---|
| **ZWA** | 8001 | agreed for the requested date |
| **NAT** | 8002 | agreed for a later date |
| **ADA** | 8104–8107 | agreed, but the address differs: the corrected part is returned |
| **ADF** | 8108–8111 | refused: address wrong |
| **RNG** | 8103 | *Rufnummer nicht geschaltet*: number not on this line |
| **AIF**, **AIFVN** | 8112, 8113 | line holder wrong, line holder's first name wrong |
| **WAI** | 8114 | further line holders missing |
| **KNI** | 8117 | customer not identifiable |
| **VAE** | 8118 | another pre-agreement for this switch already exists |
| **NWE** | 8150 | new declaration of intent of the customer |
| **SON** | 8116 | other reason, always with a text |

## In the crates

| Term | Meaning |
|---|---|
| **Bestand** | The losing provider's record of the contract, compared by [`pruefung::check`](@/docs/wbci/pruefung.md). |
| **Befund** | A finding: one compared field (`pruefung`) or one disagreement between WBCI and S/PRI (`Wechsel`). |
| **EPK** | *Ereignisgesteuerte Prozesskette*: the process diagrams of S/PRI Anlage 7, which [`spri::Process`](@/docs/spri/lifecycle.md) follows. |
| **Kölner Phonetik** | A phonetic algorithm for German names; the crate's choice for comparing names by sound. |
