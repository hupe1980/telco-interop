+++
title = "WBCI validation"
description = "Know before sending whether the peer will refuse a WBCI request or message: 28 cited rules with element path, source and the code the peer answers with."
weight = 3

[extra]
nav = "Validation"
+++

Values are valid by construction: their types check every length, pattern and character
of the schema. Validation covers what types cannot.

{% <rust> %}
```rust
use wbci::*;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
# let xml = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/abbm.xml"))?;
# let Payload::MeldungRequest { meldung: Meldung::Abbm(mut abbm), .. } = Payload::from_xml(xml.as_bytes())? else { unreachable!() };
// 8001 belongs to RUEM-VA, not to ABBM.
abbm.positionen = vec![Position::new(Meldungscode::Zwa).into()];
let issues = Meldung::from(abbm).issues(None);
assert_eq!(issues[0].rule, "code.applicability");
assert_eq!(issues[0].path, "Meldung/Position[1]/Meldungscode");
println!("{}", issues[0]);
// error [code.applicability] Meldung/Position[1]/Meldungscode: 8001 may not answer VA-KUE-MRN in a ABBM (AH4WBCI 2.10 §2, §5.3)
# Ok(())
# }
```
{% </rust> %}

Every `Issue` has a `severity` (error: the peer or certifier refuses it; warning: the
specifications advise against it), a `path`, a stable `rule`, a `message`, its `source` and,
where one exists, the `code` a peer answers with. Issues never contain personal data.

## The rules

Twenty-eight rules; an issue's `rule` field names one of them.

| Rule | Checks | Source |
|---|---|---|
| `schema.count` | repetitions within the schema's bounds | XSD V2.00 |
| `ekp.distinct` | EKPauf and EKPabg differ | Anl. 1 §2.1 |
| `id.issuer` | ids carry the sender's carrier code | Anl. 1 §2.9.1 |
| `block.range` | a number block runs low to high, same length | Anl. 1 §2.3 |
| `rufnummer.duplicate` | a number listed twice (warning) | Anl. 1 §2.3 |
| `zeitfenster.zf3`, `zeitfenster.zf1` | ZF3 only by agreement; ZF1 is for porting without takeover (warnings) | Anl. 1 §3.1 |
| `bezug.ekp` | the repeated original request names the same parties | XSD |
| `str-auf.ekpabg-grund` | the EKPabg cancels only for the listed reasons (warning) | Anl. 1 §2.9.2 |
| `va.lead` | requested date ≥ 20 AT (KUE) or 7 AT (RRNP) ahead (warning; with `issues_at`) | Anl. 1 §2.5 |
| `termin.working-day` | requested or agreed date is a working day (warning) | Anl. 1 §2.3 |
| `absender.party`, `absender.role` | the message comes from the right side | AH §2 |
| `code.unknown`, `code.applicability` | the code exists, belongs to this message kind and answers this case | AH §2, §5.3 |
| `ruem-va.zustimmung` | exactly one 8001 or 8002 | Anl. 1 §2.3 |
| `ruem-va.ada` | ADA codes 8104–8107 carry the matching address part | Anl. 1 §2.3 |
| `ruem-va.zwa`, `ruem-va.nat` | 8001 confirms the requested date; a later date is 8002 | Anl. 1 §2.3 |
| `ruem-va.date` | agreed date ≥ requested date and ≥ 15 AT ahead (KUE; with `issues_at`) | Anl. 1 §2.5 |
| `abbm.rufnummern` | numbers only with 8103 (RNG) | XSD |
| `abbm.son-begruendung` | 8116 always with a reason | Anl. 1 §2.3 |
| `abbm.tvs-termin`, `erlm.tvs-termin` | a refused postponement returns the valid date; an accepted one confirms the requested date | Anl. 1 §2.11 |
| `meldung.ref`, `meldung.answers` | an answer refers to the request's ids, parties and case | AH §2 |
| `akm-tr.leitung` | lines only with takeover; a takeover names its line (warning) | AH §2 |
| `teq.mixed` | a receipt is either `OK` or an error, not both | WBCI 2.0 §3.2 |

`Anfrage::issues()`, `Meldung::issues(answering)` and `Teq::issues()` check the value; `issues_at(…, sent_at,
&calendar)` adds the rules that depend on when it is sent. Ordering rules — a postponement
before an agreement, a duplicate id — are the [process](@/docs/wbci/process.md)'s job.
