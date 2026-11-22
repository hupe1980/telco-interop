+++
title = "Sandbox: test without a partner"
description = "Simulated WBCI and S/PRI peers — the losing provider, the network owner, the losing provider in a PV — and 13 named scenarios that play whole switches and orders from both sides."
weight = 2

[extra]
nav = "Sandbox"
seo_title = "WBCI and S/PRI sandbox: simulated peers and scenarios · telco-interop"
+++

The sandbox stands in for the partner a WBCI or S/PRI test usually needs: simulated peers,
each driven by the process state machine of its role, and named scenarios in
`telco_interop::sim` that play a whole switch or order from both sides. Like the rest of the
crates it reads no clock and opens no connection: the same inputs give the same bytes. All data
is synthetic.

## Test your side against a peer

A peer has three calls: `receive` (an inbound payload or its bytes with its time → the receipt
and anything due before), `next_due` (when its next act is due) and `advance(now)` (the acts due
by `now`). Drive your system and the peer in one loop:

{% <rust> %}
```rust
use telco_interop::spri;
use spri::process::State;
use spri::sim::Leistungserbringer;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
# let xml = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../spri/tests/fixtures/auftrag-lae.xml"))?;
let order = spri::Payload::from_xml(&xml)?; // what your system sends
let signatur = spri::SignaturId {
    issuer: spri::SignaturText::new("CN=Sandbox")?,
    serial: spri::SignaturText::new("1")?,
};
let mut network_owner = Leistungserbringer::new(signatur);

let mut now: spri::Timestamp = "2026-11-23T08:00:00Z".parse()?;
let reply = network_owner.receive(&order, now);
assert!(reply.rejection.is_none()); // the TEQ is in reply.outbound, 2 seconds later

// Your loop: deliver every outbound to your system (it acknowledges each with a TEQ, which
// goes back through `receive`), then advance to the next act.
while let Some(due) = network_owner.next_due() {
    for out in network_owner.advance(due) {
        println!("{} → {:?}: {}", out.at, out.to, out.payload.kind()); // QEB, ABM, ERLM, ENTM
    }
    now = due;
}
assert_eq!(*network_owner.process().ok_or("no order")?.state(), State::Closed);
# let _ = now;
# Ok(())
# }
```
{% </rust> %}

What the peers answer:

| Peer | Configure | It sends |
|---|---|---|
| `wbci::sim::Ekpabg` | `Antwort::Pruefung(record)` (the [losing provider's check](@/docs/wbci/pruefung.md)), `Antwort::Zustimmung`, `Antwort::Ablehnung(code)` | TEQ; RUEM-VA (8001/8002 with a permitted date) or ABBM; ERLM 8004 to a Storno, 8003 to a TVS-VA |
| `spri::sim::Leistungserbringer` | `with_terminanforderung`, `with_storno_abgelehnt`, `with_terminverschiebung_abgelehnt`, `with_netzdaten` | TEQ, QEB, ABM, ERLM on the binding date, ENTM — or TAM, MTAM, ABBM 1198; ERLM 0012 or ABBM 1087 to a Storno; a new ABM or ABBM 1085 to a Terminverschiebung; in a PV the AKM-PV, ABM-PV, ERLM-PV, ENTM-PV, or ABBM 0041/0042 and ABBM-PV 0040 |
| `spri::sim::AbgebenderProvider` | `Rueckmeldung::Zustimmung`, `Rueckmeldung::Ablehnung(code)`, `Rueckmeldung::Keine` | TEQ; RUEM-PV, or nothing |

A readable call always gets a positive receipt. What the process forbids is answered with the
ABBM and the code the process names — a Storno too late 8142, a second pre-agreement for the
line 8118, a Storno before the QEB 1087 — and comes back value-free in `reply.rejection`. An
S/PRI order whose validation predicts a refusal code gets that ABBM before the QEB. Bytes that
do not read get no answer; the reading error names the receipt code.

## The scenarios

| Name | What it shows | Source |
|---|---|---|
| `wbci-agreed` | VA-KUE-MRN checked against the losing provider's record: TEQ, RUEM-VA 8001, AKM-TR | Anl. 1 V5.4.1 §2.2–§2.6; WBCI 2.0 §12.3 |
| `wbci-refused-rng` | VA-KUE-MRN, the record has another number: ABBM 8103 (RNG) | Anl. 1 V5.4.1 §2.3, §2.4 |
| `wbci-storno` | VA-KUE-ORN agreed, cancelled by the gaining provider (STR-AUF): ERLM 8004 | Anl. 1 V5.4.1 §2.9.1 |
| `wbci-tvs` | VA-KUE-ORN agreed, postponed by the gaining provider (TVS-VA): ERLM 8003 | Anl. 1 V5.4.1 §2.11 |
| `spri-neu` | NEU: TEQ, QEB, ABM, ERLM on the binding date, ENTM | Anl. 7 EPK V.13 p.1, p.10 |
| `spri-neu-tam` | NEU, the appointment fails: TAM, MTAM, ABBM 1198 | Anl. 7 EPK V.13 p.1; AH 2.9.5 §3.6 |
| `spri-storno` | NEU, Storno after the ABM: ERLM 0012, ENTM | Anl. 7 EPK V.13 p.11; AH 2.9.5 §3.5 |
| `spri-storno-refused` | NEU, Storno refused with ABBM 1087; the order completes | Anl. 7 EPK V.13 p.11 |
| `spri-tv` | NEU, Terminverschiebung after the ABM: a new ABM, ERLM on the new date, ENTM | Anl. 7 EPK V.13 p.12; AH 2.9.5 §3.4 |
| `spri-pv` | PV: AKM-PV, RUEM-PV, ABM with ABM-PV, ERLM with ERLM-PV, ENTM with ENTM-PV | Anl. 7 EPK V.13 p.7; S/PRI 4.4 §5.5.1, §8.2.13 |
| `spri-pv-refused` | PV, the losing provider refuses (KVE): ABBM 0041, ABBM-PV 0040 | Anl. 7 EPK V.13 p.7; S/PRI 4.4 §5.5.1.1, §5.5.1.2 |
| `spri-pv-timeout` | PV, no RUEM-PV by the antwortfrist: ABBM 0042, ABBM-PV 0040 | S/PRI 4.4 §5.5.1.1; Anl. 7 EPK V.13 p.7 |
| `wechsel` | the switch across both: VA, RUEM-VA with the LineID, AKM-TR, PV for the switch date, the PV leg, completion on the switch date | S/PRI 4.4 §5.5.1.1 (p. 27); Anl. 1 V5.4.1 §2.6.2; § 59 TKG |

The gaining provider is scripted: it sends its requests and orders and acknowledges each
message two seconds after it arrives; the other parties are the peers above.

{% <rust> %}
```rust
use telco_interop::sim;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let ablauf = sim::szenario("wechsel").ok_or("no such scenario")?.play(sim::START);
assert!(ablauf.abgelehnt.is_empty());
for nachricht in &ablauf.nachrichten {
    println!("{nachricht}"); // Mo 01.03.2027 09:00 WBCI EKPauf → EKPabg VA-KUE-MRN, …
}
for (name, xml) in ablauf.files()? {
    // 001-wbci-ekpauf-va-kue-mrn.xml, 002-wbci-ekpabg-teq.xml, …
#   let _ = (name, xml);
}
# Ok(())
# }
```
{% </rust> %}

`play` takes the start and derives every date from it through the calendars. For start times
across a whole year the tests prove that every message validates without an issue, meets its
deadline and is accepted by every party's process and by the
[cross-interface checks](@/docs/across-both/one-switch.md); every payload is
[valid against both official schemas](@/docs/reference/conformance.md). The files at
`sim::START` are golden fixtures in the repository.

## On the command line

```sh
telco-interop simulate --list
telco-interop simulate wechsel switch/
telco-interop timeline switch/ --as ekpauf
```

See [the command line](@/docs/start/command-line.md#simulate-a-switch); the
[playground](@/playground.md) shows each scenario's messages in the browser.

## Timing

The documents set the deadlines, not the moment inside them. By default a peer sends a receipt
2 seconds after the call and an answer 1 hour after the event it answers (`Latency`), always
inside the deadline; the ERLM goes at 10:00 German time on the binding date. These moments, the
AKM-PV's antwortfrist and the other sandbox choices are
[interpretations](@/docs/reference/scope.md#interpretations).
