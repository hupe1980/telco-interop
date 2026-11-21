+++
title = "The S/PRI order lifecycle"
description = "spri::Process follows an S/PRI order through the EPKs of Anlage 7 V.13: allowed messages, Storno and TV, the PV leg, explained deadlines, ABBM outcomes."
weight = 2

[extra]
nav = "Order lifecycle"
seo_title = "S/PRI order lifecycle (Anlage 7 EPKs) in Rust · telco-interop"
+++

An order lives for weeks — QEB, ABM (possibly again), appointments, completion, billing, with
cancellations and postponements in between and, for a provider switch, a second exchange with
the losing provider. A `Process` is that life for one party: feed it every order, message and
receipt with its time; it refuses what the flow of the business case does not allow (Anlage 7,
EPKs V.13), keeps the history, and tells each side what comes next and what is due.

{% <rust> %}
```rust
use spri::process::{Action, Direction, Process, Role};
use spri::*;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
# let xml = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/auftrag-pv.xml"))?;
# let Payload::AuftragRequest { auftrag, .. } = Payload::from_xml(xml.as_bytes())? else { unreachable!() };
let sent: Timestamp = "2026-11-23T08:00:00Z".parse()?;
let mut p = Process::new(Role::Auftraggeber, &auftrag, Direction::Sent, sent)?;
let teq = Teq::new(Meldungscode::Ok, Some(auftrag.externe_auftragsnummer.clone()), Some(auftrag.auftraggeber.auftraggebernummer.clone()));
p.apply(Direction::Received, sent, teq)?;
// Now the network owner owes the QEB.
assert!(p.next_actions(Role::Leistungserbringer).contains(&Action::Send(Meldungstyp::Qeb)));
for d in p.deadlines() {
    println!("{d}");
}
# Ok(())
# }
```
{% </rust> %}

## The rules it enforces

| Rule | Source |
|---|---|
| QEB is the first message, before any ABM; further messages only after the ABM | S/PRI 4.4 §8.2.1–§8.2.2 |
| ABM again (6012 next Werktag, 8500–8502 changed) replaces the binding date | Anl. 7 p.1, 3, 6, 7 |
| VZM from receipt, ZWM from QEB, TAM from ABM — each until ERLM | S/PRI 4.4 §8.2.6–§8.2.9 |
| TAM → MTAM after 10 Werktage (Mon–Sat) → ABBM 1198 after another 10 | AH 2.9.5 §3.4 |
| ERLM, then only ENTM; ENTM and ABBM end the order (EST and DIAG may end at ERLM) | S/PRI 4.4 §8.2.3–§8.2.5 |
| One Storno and one TV at a time, after the QEB, until 36 h before the binding date (after a TAM: any time) — none in DIAG/KUE-LE, no TV in AEN-LMAE/EST, none before the first ABM of a build-out order | Anl. 7 p.11–12; AH 2.9.5 §3.4/§3.5 |
| A successful TV comes back as a new ABM; a failed one as ABBM 1085 | Anl. 7 p.12 |
| PV: AKM-PV with an answer date to the losing provider; RUEM-PV positive, negative or late; ABM with ABM-PV; ERLM with ERLM-PV; ENTM with ENTM-PV; ABBM-PV whenever the switch ends after the AKM-PV | Anl. 7 p.7; S/PRI 4.4 §5.5.1 |

## Refusals and ABBM outcomes

A refused event leaves the process unchanged and says why; a refused Storno or TV also names
the ABBM the network owner answers with (`Refusal::answer`: 1007, 1085, 1086, 1087). An ABBM
tells you whether to correct and resend, whether the order continues, or whether it is closed:
`abbm.outcome(fall, phase)` → `AbbmOutcome::{Resend, Continues, Closed, Unknown}`, from the
code sets the EPKs draw for each business case and phase.

The losing provider follows the same switch with `Process::from_akm_pv`; the network owner's
own cancellation starts with `Process::from_kdle`.

## Deadlines

`p.deadlines()` lists what is open, each with the party that must act, the instant and an
explanation of the arithmetic. The rules live in `spri::deadline`:

| Function | Deadline | Source |
|---|---|---|
| `teq_response` | the receiver acknowledges with a TEQ within 30 seconds, 120 with embedded documents | S/PRI 4.4 §11.2.3, §11.4 |
| `storno_latest` | a Storno at least 36 hours before the binding date (VLT); after a TAM also later | AH 2.9.5 §3.5 |
| `terminverschiebung_latest` | a Terminverschiebung at least 36 hours before the binding date (KUE-AG: the termination date); after a TAM also later | AH 2.9.5 §3.4 |
| `mtam_due` | the service provider answers a TAM within 10 Werktage, otherwise the MTAM follows | AH 2.9.5 §3.4, §3.6 |
| `abbm_1198_due` | it answers the MTAM within another 10 Werktage, otherwise ABBM 1198 | AH 2.9.5 §3.4, §3.6 |
| `ruem_pv` | the losing provider answers the AKM-PV by 23:59 of its answer date, otherwise ABBM 0042 and ABBM-PV 0040 | S/PRI 4.4 §5.5.1.1, §8.3.3 |

Werktage are Monday to Saturday without the nationwide public holidays (`Calendar::werktage()`).

{% <note title="Interpretation: 36 hours"> %}
The handbook gives the Storno and TV limit as 36 hours before the binding date; the code fact
sheets say 3 Arbeitstage. The crate counts 36 hours of elapsed time before 0:00 of the binding
date. The full list of the lifecycle's interpretations is in the [`spri::process` API
documentation](https://docs.rs/spri/latest/spri/process/index.html).
{% </note> %}
