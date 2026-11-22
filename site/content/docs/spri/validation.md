+++
title = "S/PRI validation"
description = "Know before sending whether the network owner will refuse an S/PRI order or message: 34 cited rules with element path, source and the ABBM code it answers with."
weight = 4

[extra]
nav = "Validation"
seo_title = "S/PRI validation: predict the ABBM before sending · telco-interop"
+++

Values are valid by construction: their types check every length, pattern and value list of
the schema. Validation covers what types cannot — which business case has a Storno or a
Terminverschiebung, an Auftragsklammer's size, a Zeitfenster against the weekday, a follow-up
order against its original, a code against the message type and business case.

{% <rust> %}
```rust
use spri::*;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
# let xml = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/auftrag-pv.xml"))?;
# let Payload::AuftragRequest { auftrag: pv, .. } = Payload::from_xml(xml.as_bytes())? else { unreachable!() };
// The PV fixture asks for 2027-01-15. A Terminverschiebung that moves it earlier:
let mut tv = pv.clone();
tv.aenderungskennzeichen = Aenderungskennzeichen::Terminverschiebung;
tv.termin_neu = Some("2027-01-08".parse()?);

let issues = tv.issues_against(&pv);
assert_eq!(issues[0].rule, "spri.tv.earlier");
assert_eq!(issues[0].code, Some(Meldungscode::C1085));
println!("{}", issues[0]);
// error [spri.tv.earlier] auftrag/terminNeu: a PV cannot move earlier than 2027-01-15 after the positive pre-agreement (AH 2.9.5 §3.4) → 1085

// The date rules need the moment of sending and a calendar.
let late = pv.issues_at("2027-01-16T08:00:00Z".parse()?, &Calendar::werktage());
assert_eq!(late[0].rule, "spri.termin.past");
# Ok(())
# }
```
{% </rust> %}

Every `Issue` has a `severity` (error: the network owner refuses it, or the specification says
it must not be sent so; warning: advised against or likely a problem), a `path` in XML element
names, a stable `rule`, a `message`, its `source` and, where the sources give one, the `code` of
the ABBM the network owner answers with. Messages are not answered with an ABBM, so their issues
carry no code. Issues never contain personal data: no name, address, number or free text.

| Call | Checks |
|---|---|
| `Auftrag::issues()` | the order on its own |
| `Auftrag::issues_at(now, &calendar)` | plus the requested date: of a standard order, or the new date of a Terminverschiebung |
| `Auftrag::issues_against(&original)` | plus a Storno, Terminverschiebung or Reklamation against its order |
| `Meldung::issues()` | a message against the code table |
| `Teq::issues()` | a technical receipt |

## The rules

Thirty-four rules; an issue's `rule` field names one of them. (I) marks an
[interpretation](@/docs/reference/scope.md).

### Orders

| Rule | Checks | Source | Code |
|---|---|---|---|
| `spri.aekz.storno` | no Storno in GET and SET (messages: nor KUE-LE) | S/PRI 4.4 §6.2.1.2, §6.2.2.2; Anl. 7 EPK V.13 p.4 | 1007 |
| `spri.aekz.terminverschiebung` | a Terminverschiebung only in NEU, KUE-AG, LAE, PV | S/PRI 4.4 §5.3.1.3, §6.1.3, §6.2.1.3, §6.2.2.3 | 1007 |
| `spri.termin-neu.only-tv` | `terminNeu` only with a Terminverschiebung (warning) | XSD 4.00.03 `AuftragType` | |
| `spri.geschaeftsfallart.match` | the Geschäftsfallart belongs to the business case; PV also `Bereitstellung` (I) | S/PRI 4.4 §4.1 | |
| `spri.auftragsklammer.count` | an Auftragsklammer holds 1 to 20 orders | S/PRI 4.4 §4.3.3; AH 2.9.5 §3.7 | |
| `spri.auftragsklammer.case` | only NEU, AEN-LMAE, LAE and PV are bracketed | AH 2.9.5 §3.7; S/PRI 4.4 §4.3.3 | |
| `spri.zeitfenster.weekday` | the Zeitfenster fits the weekday of the date (I: „Werktags" = Monday to Saturday) | AH 2.9.5 §3.3; XSD 4.00.03 `ZeitfensterType` | 1032 |
| `spri.kue-ag.wita-vertragsnummer` | a KUE-AG with Vorabstimmungs-ID names the WITA contract number (warning, I) | S/PRI 4.4 §5.2.1.4 | |
| `spri.line-id.va-id` | the LineID is not the Vorabstimmungs-ID (I) | LineID V1.4 §3.1.3 | 8144 |
| `spri.termin.past` | the date is not before the day of sending (I: 1003 outside EST) | AH 2.9.5 §6.4 (7014, 1003) | 7014, 1003 |
| `spri.termin.horizon` | the date is at most 180 days ahead (warning) | S/PRI 4.4 §2, §5.1.4 | 1003 |
| `spri.termin.working-day` | the date is a working day of the calendar (warning) | AH 2.9.5 §6.4 (0002) | 0002 |

### Storno, Terminverschiebung, Reklamation

| Rule | Checks | Source | Code |
|---|---|---|---|
| `spri.bezug.order-number` | it repeats the original's order number | S/PRI 4.4 §5.1.3, §5.1.4, §6.1.5 | |
| `spri.bezug.case` | it keeps the business case | S/PRI 4.4 §5.1.3, §5.1.4 | |
| `spri.bezug.original` | it refers to a standard order | AH 2.9.5 §3.4, §3.5 | |
| `spri.bezug.repeated` | a new standard order does not reuse an order number | AH 2.9.5 §3.9 | 1013 |
| `spri.storno.identical` | a Storno is identical to the original but for the Änderungskennzeichen (warning) | S/PRI 4.4 §5.1.3, §5.3.1.2, §6.1.2 | |
| `spri.tv.identical` | a Terminverschiebung changes only the date, `terminNeu`, Anlage, Montageleistung, Seriennummer ONT and Vorabstimmungs-ID (warning) | S/PRI 4.4 §5.1.4 | |
| `spri.tv.new-date` | it names a date other than the original's | S/PRI 4.4 §5.1.4 | 1085 |
| `spri.tv.earlier` | a PV is not moved earlier | AH 2.9.5 §3.4 | 1085 |
| `spri.tv.zeitfenster` | it keeps the Zeitfenster | AH 2.9.5 §3.4 | 1085 |
| `spri.reklamation.identical` | a Reklamation changes only Anlage, contacts, date and Störungsbeschreibung (warning) | S/PRI 4.4 §6.1.5 | |

### Messages and receipts

| Rule | Checks | Source |
|---|---|---|
| `spri.code.unknown` | the code is in the published list (warning: the online list adds codes) | AH 2.9.5 §6.3; S/PRI 4.4 §11.2.4 |
| `spri.code.applicability` | the code belongs to the message type and the business case | AH 2.9.5 §6.4 |
| `spri.code.1312` | `1312` only together with `1310` or `1311` | AH 2.9.5 §6.4 |
| `spri.code.freitext` | `1312`, `8600`, `8601` carry a text of their own, not the placeholder (warning) | AH 2.9.5 §6.4 |
| `spri.vzm.verzoegerungstermin` | a VZM with `1576`, `1580` or `1581` names the appointment | AH 2.9.5 §6.4 |
| `spri.abbm.erledigungstermin` | an ABBM `1006` or `1012` gives the open order's date in `wiedervorlagetermin` (warning, I) | AH 2.9.5 §6.4 |
| `spri.abbm.abgebender-provider` | an ABBM `0041` or `0042` names the losing provider | AH 2.9.5 §6.4 |
| `spri.ruem-pv.antwortcode-missing` | a refusing RUEM-PV gives a reason | S/PRI 4.4 §5.5.1.1 |
| `spri.ruem-pv.antwortcode-known` | the reason is `LnB`, `KVE`, `TAV`, `VnB` or `Vnv` (I: ignoring case) | S/PRI 4.4 §5.5.1.1; AH 2.9.5 §6.5 |
| `spri.ruem-pv.contradiction` | a consenting RUEM-PV carries no refusal reason (I) | S/PRI 4.4 §5.5.1.1 |
| `spri.teq.code` | a receipt carries only `OK`, `0990`, `0991`, `0993`, `0995`, `0999` | AH 2.9.5 §6.4 |
| `spri.teq.mixed` | a receipt is either `OK` or an error, not both (I) | S/PRI 4.4 §11.2.3.1 |

The rules the [lifecycle](@/docs/spri/lifecycle.md) also enforces — Storno and
Terminverschiebung per business case, a new date, a PV not moved earlier, the refusal reasons —
give the same verdict and the same code there. Ordering rules — a Storno before the QEB, a second
open Terminverschiebung, the 36-hour window — need the order's history and are the lifecycle's
job. Not checked: the Home-ID, which the AK S&P leaves to each network operator, and contractual
lead times.
