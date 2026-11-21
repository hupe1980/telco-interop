+++
title = "Answering as the losing provider"
description = "Decide the answer to a WBCI pre-agreement from your contract record: the Prüflogik of Anlage 1 §2.4, names by sound per the AK S&P word rules, explained."
weight = 6

[extra]
nav = "Losing provider's check"
seo_title = "WBCI Prüflogik: the losing provider's check, in Rust · telco-interop"
+++

Every losing provider (EKPabg) answers each pre-agreement within 3 working days, following
the check table of Anlage 1 V5.4.1 §2.4. `pruefung::check` applies it: you look up the
contract the request names, and get back the answer to send and the reasons.

{% <rust> %}
```rust
use wbci::pruefung::{self, Bestand};
use wbci::*;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
# let xml = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/va-kue-mrn.xml"))?;
# let Payload::AnfrageRequest { anfrage: Anfrage::Vorabstimmung(va), .. } = Payload::from_xml(xml.as_bytes())? else { unreachable!() };
let received: Timestamp = "2026-11-23T08:00:00Z".parse()?;

// Your record of the contract (found by number, name or address in your database).
let mut bestand = Bestand::new(
    Kunde::person(Anrede::Frau, Some(Vorname::new("Erika")?), Nachname::new("Müller")?),
    "D123".parse()?,                                   // your porting id (PKIabg)
);
bestand.rufnummern = vec![OnkzRufnummer { onkz: "30".parse()?, rufnummer: "1234567".parse()? }];
bestand.standort = va.standort().cloned();
bestand.vertragsende = Some("2027-01-29".parse()?);    // later than requested → 8002 (NAT)

let pruefung = pruefung::check(&va, &bestand, received, &Calendar::wbci());
println!("{pruefung}");
// VA-KUE-MRN refused (8112)
//   Rufnummern: exact → match
//   Nachname: phonetic → mismatch (8112)        ← the request says „Mustermann"
//   …
assert!(!pruefung.is_agreed());
# Ok(())
# }
```
{% </rust> %}

`pruefung.antwort` is a ready RUEM-VA or ABBM (it validates clean under the
[validation rules](@/docs/wbci/validation.md)); `pruefung.befunde` lists every field compared with
its method and result and holds no personal data, so it can be logged and shown to the other
provider's clearing team. No record found: `pruefung::not_found(&va)` answers 8117 (KNI).

## Names

There is no prescribed algorithm (Anl. 1 p. 31: „liegt in der Zuständigkeit der einzelnen
Häuser"). The check follows the AK S&P's *Infodokument WBCI Phonetische Prüfung* v1.1 and
compares word by word:

- words are split at spaces and hyphens; punctuation and digits separate words; case is ignored;
- one-letter words are ignored, so „L." never matches;
- stop words are ignored: legal forms (GmbH, KG, AG, e.V., …), titles and salutations (Dr.,
  Dipl., Frau, Herr, …), „Firma", „Inhaber", „z. Hd." and the others of §5.3;
- each stored word is looked up by sound — **Kölner Phonetik**, our choice — in any name field
  of the request, so swapped first and last names still match;
- one part of a double surname is enough, one of several first names is enough (Anl. 1 §2.4);
  merged („AnnaMarie") or split („Heinz Peter" for „Heinzpeter") first names are not.

| Record | Request | Result |
|---|---|---|
| Müller-Lüdenscheidt | Mueller | match |
| Anna Marie | Marie | match |
| Anna Marie | AnnaMarie, A., *(none)* | 8113 AIFVN |
| Schneider | Schmidt | 8112 AIF |
| Beispiel GmbH | Beispiel G.m.b.H. & Co. KG | match |

`pruefung::same_name(stored, given)` gives you the same comparison for your own search.

## Address and numbers

| Field | Compared | On mismatch |
|---|---|---|
| Postleitzahl, Hausnummer | exactly | ADA 8104/8107 if a number identifies the line, else ADF 8108/8111 |
| Ort, Straße | by sound („Str." read as „Straße") | ADA 8105/8106, else ADF 8109/8110 |
| Hausnummernzusatz | not compared | — |
| Numbers, „alle Rufnummern" | exactly | RNG only if none is right; agreed: all your numbers are returned |
| Numbers, without „alle" | exactly | RNG if any is wrong, naming your numbers when one was right and the name matched |
| PBX head and blocks | exactly | RNG; a block with the wrong number of digits adds 8116 with the right block |

The customer is identified by **number and name** (VA-KUE-MRN, VA-RRNP) or **name and either
the address or the optional number** (VA-KUE-ORN). Your record's data — numbers, the corrected
address — goes into the answer only when the customer is identified. Nothing identifying at
all is 8117 (KNI).

## Record facts

| `Bestand` field | Effect |
|---|---|
| `vertragsende` | the switch date moves to the contract end (8002 NAT), then to a working day |
| `alle_inhaber_noetig` + further `inhaber` | every holder must be named, else 8114 (WAI) |
| `andere_vorabstimmung` | 8118 (VAE), with any other reason |
| `neue_willenserklaerung` | 8150 (NWE), VA-KUE only |
| `mehrere_anschluesse` | a VA-KUE-ORN without a number is 8117 (KNI) |
| `abgeschaltet` | a VA-RRNP more than 65 AT after switch-off is 8103 („Nachträgliche Portierung nicht möglich") |

The binding date is the latest of the requested date, 15 working days after receipt
(VA-KUE) and the contract end, moved to the next working day.
