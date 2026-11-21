+++
title = "One switch across WBCI and S/PRI"
description = "telco_interop::Wechsel: a provider switch agreed over WBCI and ordered over S/PRI as one timeline, with both interfaces' deadlines and six agreement checks."
weight = 1

[extra]
nav = "One switch"
seo_title = "One provider switch across WBCI and S/PRI (Wechsel) · telco-interop"
+++

A provider switch crosses two interfaces. The gaining provider agrees it with the losing
provider over WBCI (VA, RUEM-VA, AKM-TR), then orders the line from the network owner over
S/PRI with a `PV` order that carries the WBCI Vorabstimmungs-ID — the id „wird im gesamten
Prozessablauf als eindeutiges Identifikationsmerkmal verwendet" (S/PRI 4.4 §5.5.1.1). When the
two disagree, the customer finds out on the switch date.

`telco_interop::Wechsel` (both features of `telco-interop`) holds the WBCI process and every
S/PRI process with that id, and recomputes from them on every call:

{% <rust> %}
```rust
use telco_interop::{Partei, Wechsel, wbci};
use wbci::{Anfrage, Direction, Meldung, Payload, Process, Role, RuemVa, Timestamp};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
# let xml = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../wbci/tests/fixtures/va-kue-mrn.xml"))?;
# let Payload::AnfrageRequest { anfrage: Anfrage::Vorabstimmung(va), .. } = Payload::from_xml(xml.as_bytes())? else { unreachable!() };
let sent: Timestamp = "2026-11-23T08:00:00Z".parse()?;
let mut wechsel = Wechsel::new(Process::new(Role::EkpAuf, va.clone(), Direction::Sent, sent)?);
let ruem = RuemVa::zustimmung(&va, va.kundenwunschtermin);
wechsel.wbci_mut().apply(Direction::Received, "2026-11-24T09:00:00Z".parse()?, Meldung::from(ruem))?;

for eintrag in wechsel.timeline() {
    println!("{eintrag}");   // Di 24.11.2026 10:00 WBCI EKPabg → RUEM-VA 8001
}
for befund in wechsel.befunde() {
    println!("{befund}");    // where WBCI and S/PRI disagree
}
for frist in wechsel.deadlines() {
    println!("{frist}");     // both interfaces, sorted
}
let todo = wechsel.next_actions(Partei::EkpAuf);
# let _ = todo;
# Ok(())
# }
```
{% </rust> %}

Attach each S/PRI order with `wechsel.attach(process)`: one with another or no
Vorabstimmungs-ID is refused and handed back in the error. Keep feeding events through
`wbci_mut()` and `spri_mut()` — the switch has no state of its own.

## What it checks

| Rule | Finding | Source |
|---|---|---|
| `wechsel.nicht-abgestimmt` | S/PRI ordered before the WBCI pre-agreement is agreed | S/PRI 4.4 §5.5.1.1 |
| `wechsel.leitung` | the PV's LineID is not a line the AKM-TR takes over | Anl. 1 §2.2; S/PRI 4.4 §5.5.1.1 |
| `wechsel.termin` | the S/PRI requested or binding date differs from the WBCI switch date | Anl. 1 §2.5; S/PRI 4.4 §5.5.1.3 |
| `wechsel.tv-zuerst-wbci` | an S/PRI postponement before the WBCI one (TVS-VA, ERLM 8003) | Anl. 1 §2.11 |
| `wechsel.storno` | WBCI cancelled or changed, the S/PRI order still open | Anl. 1 §2.9/§2.10; S/PRI 4.4 §5.5.1.2 |
| `wechsel.kontinuitaet` | completion more than one working day after the switch date | § 59 Abs. 1 TKG (interpretation) |

Each finding names its rule, source and the party who has to act: `EkpAuf` (gaining provider,
S/PRI Auftraggeber), `EkpAbg` (losing provider, including its `KUE-AG` with the id) or
`Netzbetreiber` (network owner). Timeline, findings and deadlines carry ids, dates, message
types and codes only — never a name, address or number.

{% <note title="Interpretation: continuity"> %}
The continuity rule reads § 59 TKG's „nicht länger als einen Arbeitstag unterbrochen" with the
S/PRI completion date standing in for the end of the outage, which the interfaces cannot see.
{% </note> %}

## The sequence

The [introduction](@/docs/start/introduction.md#one-switch-step-by-step) describes the
messages in order. A complete switch gives this timeline (S/PRI 4.4, the sequence of p. 27):
`VA-KUE-MRN`, `RUEM-VA`, `AKM-TR` over WBCI, then `PV`, `QEB`, `AKM-PV`, `RUEM-PV`, `ABM`,
`ABM-PV`, `ERLM`, `ERLM-PV`, `ENTM`, `ENTM-PV` over S/PRI.
