+++
title = "WBCI requests and messages"
description = "The six WBCI requests and six messages as Rust types (VA-KUE-MRN, STR-AUF, TVS-VA, RUEM-VA, ABBM, AKM-TR …), and answers built from what they answer."
weight = 1

[extra]
nav = "Requests and messages"
+++

## Requests (`Anfrage`)

| Case | Type | Sent by |
|---|---|---|
| `VA-KUE-MRN` | `Vorabstimmung` with `VorabstimmungsFall::KueMrn` | EKPauf |
| `VA-KUE-ORN` | `VorabstimmungsFall::KueOrn` | EKPauf |
| `VA-RRNP` | `VorabstimmungsFall::Rrnp` | EKPauf |
| `STR-AUF` | `Storno` with `StornoArt::AufhebungAuf` / `AufhebungAbg` | either side |
| `STR-AEN` | `Storno` with `StornoArt::AenderungAuf` / `AenderungAbg` | either side |
| `TVS-VA` | `Terminverschiebung` | EKPauf |

A request has no `Absender` field: its kind fixes the sender, so `Anfrage::absender()` derives
it, the writer fills it in, and the reader refuses a request whose `Absender` contradicts its
type (receipt code 0995). A cancellation by the EKPabg carries a mandatory `stornogrund`.

## Messages (`Meldung`) and the receipt (`Teq`)

| Message | Type | Means |
|---|---|---|
| RUEM-VA | `RuemVa` | agreed: binding `wechseltermin`, 8001 (as asked) or 8002 (later) |
| ABBM | `Abbm` | refused, with codes |
| ERLM | `Erlm` | cancellation (8004) or postponement (8003) done |
| AKM-TR | `AkmTr` | the EKPauf takes over the line (or not), „sicherer Hafen" |
| ABBM-TR | `AbbmTr` | the EKPabg refuses the AKM-TR |
| TEQ | `Teq` | technical receipt for every call: `OK`, 0993, 0995, 0999 |

## Answers from requests

Answers copy every reference (sender, EKP pair, case, ids) from what they answer:

{% <rust> %}
```rust
use wbci::*;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
# let xml = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/va-kue-orn.xml"))?;
# let Payload::AnfrageRequest { anfrage, .. } = Payload::from_xml(xml.as_bytes())? else { unreachable!() };
# let Anfrage::Vorabstimmung(va) = anfrage.clone() else { unreachable!() };
// As the EKPabg: the receipt, then the answer.
let teq = Teq::for_anfrage(&anfrage, Meldungscode::Ok);
let ruem = RuemVa::zustimmung(&va, "2027-01-22".parse()?);   // later than asked → 8002 NAT
assert_eq!(ruem.positionen[0].position.code, Meldungscode::Nat);

// Or refuse, with the reason 8116 always needs:
let abbm = Abbm::ablehnung(&anfrage, Meldungscode::Sonstiges)
    .with_begruendung(Freitext::new("Frist unterschritten")?);
assert!(Meldung::from(abbm).issues(Some(&anfrage)).is_empty());

// As the EKPauf, after the RUEM-VA: take over the line.
let akm = AkmTr::uebernahme(&ruem, vec![UebernahmeLeitung::LineId("DEU.EKP2.1754639821".parse()?)], false);
assert!(akm.ressourcenuebernahme);
# let _ = teq;
# Ok(())
# }
```
{% </rust> %}

Cancellations and postponements are confirmed with `Erlm::storno_ok` and
`Erlm::terminverschiebung_ok`; the AKM-TR is refused with `AbbmTr::ablehnung`.

## Personal data

Names, street, house number, phone numbers and free text print redacted in `Debug`
(`Nachname(<redacted, 10 chars>)`), so logging a request does not log the customer. `Display`
and `as_str()` give the value when you need it.
