+++
title = "The WBCI process"
description = "Follow one WBCI pre-agreement through its life: state, allowed next steps, open deadlines, refusals with the prescribed codes, and an audit history."
weight = 5

[extra]
nav = "Process"
+++

A `Process` is one pre-agreement as one party sees it. Feed it every request, message and
receipt you send or receive, with its time; it refuses what the protocol forbids and tells
you what comes next. It performs no I/O: store it however you like.

{% <rust> %}
```rust
use wbci::process::{Action, State};
use wbci::*;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
# let xml = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/va-kue-mrn.xml"))?;
# let Payload::AnfrageRequest { anfrage: Anfrage::Vorabstimmung(va), .. } = Payload::from_xml(xml.as_bytes())? else { unreachable!() };
let received: Timestamp = "2026-11-23T08:00:00Z".parse()?;
// As the EKPabg: a pre-agreement arrived.
let mut p = Process::new(Role::EkpAbg, va.clone(), Direction::Received, received)?;
p.apply(Direction::Sent, received, Teq::for_anfrage(&va.clone().into(), Meldungscode::Ok))?;
assert_eq!(p.next_actions(Role::EkpAbg), vec![Action::AnswerVorabstimmung]);

let ruem = RuemVa::zustimmung(&va, va.kundenwunschtermin);
p.apply(Direction::Sent, "2026-11-24T09:00:00Z".parse()?, Meldung::from(ruem))?;
assert_eq!(p.state(), &State::Agreed { wechseltermin: va.kundenwunschtermin });

// The same request again is a duplicate: answer ABBM 8115, nothing changes.
let refusal = p.apply(Direction::Received, "2026-11-24T10:00:00Z".parse()?, Anfrage::from(va)).unwrap_err();
assert_eq!(refusal.code, Meldungscode::IdBereitsVerwendet);

for d in p.deadlines() {
    println!("{d}");   // the AKM-TR reminder and escalation, the last moments to cancel, …
}
# Ok(())
# }
```
{% </rust> %}

## States

```text
Requested ──RUEM-VA──▶ Agreed{date} ──ERLM on STR-AUF──▶ Cancelled
    │                      ├──ERLM on STR-AEN──▶ Changed (submit a new VA)
    ├──ABBM──▶ Rejected     └──ERLM on TVS-VA──▶ Agreed{new date}
    └──ERLM on STR-AUF ──▶ Cancelled          AKM-TR / ABBM-TR: takeover()
```

## What it refuses, and with which code

| Situation | Code |
|---|---|
| a request id already used (also the pre-agreement itself) | 8115 |
| another pre-agreement for the same switch | 8118 (VAE) |
| a reference to another Vorabstimmungs-ID | 8140 |
| a postponement before the agreement, earlier than the agreed date, a second open one, or too late | 8141 |
| a cancellation in the wrong state, a second open one, by the EKPabg before the agreement, or too late | 8142 |
| an AKM-TR announced twice | 8148 |
| an event from the wrong side, another EKP pair, a message out of order | 8116 |

Refusals leave the process unchanged. A negative receipt undoes what it acknowledged, so a
resend with the same id is accepted (WBCI 2.0 §12.3.1); a business answer counts as delivery
(§12.3.3). `history()` lists every accepted event with its time, direction and sender — the
evidence a dispute over a deadline needs. `overdue(now)` lists the deadlines already missed, and
`with_calendar` counts with another calendar (for example `Calendar::mon_fri()`).
