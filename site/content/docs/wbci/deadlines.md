+++
title = "WBCI deadlines"
description = "Every WBCI deadline of Anlage 1 §2.5–§2.11 computed in German time on working days (AT), with its rule, source and every counted and skipped day."
weight = 4

[extra]
nav = "Deadlines"
+++

## Working days

`Calendar::wbci()` counts Monday to Friday without the nine nationwide public holidays
(Neujahr, Karfreitag, Ostermontag, Tag der Arbeit, Christi Himmelfahrt, Pfingstmontag, Tag der
Deutschen Einheit, 1. and 2. Weihnachtstag; Easter is computed). `with_closed(date)` adds a
regional or company closing day.

{% <note title="Interpretation: which working days?"> %}
Anlage 1 §2.5 says „Arbeitstage (Mo – Fr)", while the interface description makes the
interface available „an bundeseinheitlichen Arbeitstagen". `Calendar::wbci()` reads both
together and skips the nationwide holidays; `Calendar::mon_fri()` is the literal reading.
{% </note> %}

## Counting

Anlage 1 §2.5 gives the rules and three worked examples, which the tests reproduce:

- **Forwards, X+n AT**: ends at 23:59 of the n-th working day after X. A VA acknowledged on
  Monday is answered by Thursday 23:59.
- **Backwards, X−n AT**: before 0:00 of the n-th working day before X. With the switch on
  Friday, a postponement (3 AT) arrives by Monday 23:59; with the switch on Monday, by the
  Tuesday before.
- **24 hours on working days**: elapsed time counted only on working days (an
  interpretation of „24 Stunden (bezogen auf Arbeitstage)"). A cancellation acknowledged Friday 15:00 is answered by Monday 15:00.

{% <rust> %}
```rust
use wbci::{Calendar, Role, Timestamp, deadline};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let teq: Timestamp = "2026-12-23T15:00:00Z".parse()?;   // Wednesday before Christmas
let due = deadline::va_answer(teq, &Calendar::wbci());
println!("{due}");
// EKPabg: by Di 29.12.2026 23:59 — The EKPabg answers a pre-agreement request within 3 AT after the positive TEQ. (Anl. 1 V5.4.1 §2.5, §2.6.1)
println!("{}", due.explanation);
// positive TEQ at Mi 23.12.2026 16:00; forwards: …; counted Do 24.12.2026, Mo 28.12.2026, Di 29.12.2026;
// skipped Fr 25.12.2026 (1. Weihnachtstag), Sa 26.12.2026 (weekend), So 27.12.2026 (weekend)
assert_eq!(due.party, Role::EkpAbg);
assert_eq!(due.explanation.counted.len(), 3);
# Ok(())
# }
```
{% </rust> %}

## The rules

| Function | Deadline | Source |
|---|---|---|
| `earliest_kundenwunschtermin` | requested date ≥ 20 AT (KUE) or 7 AT (RRNP) after sending | Anl. 1 §2.5 |
| `va_answer` | answer 3 AT after the positive TEQ | §2.5, §2.6.1 |
| `earliest_wechseltermin` | agreed KUE date ≥ 15 AT ahead, ≥ requested | §2.5 |
| `teq_escalation` | no receipt 2 hours after sending: escalate | §2.6.1, §2.11.1 |
| `storno_aufhebung_latest` | cancellation 3 AT before the switch | §2.9.1, §2.9.2 |
| `storno_aenderung_latest` | cancellation for change 8 AT before | §2.10 |
| `terminverschiebung_latest` | postponement 3 AT before (24 working hours with a shared network operator) | §2.11 |
| `answer_24h` | cancellation or postponement answered within 24 working hours | §2.9.1, §2.10, §2.11 |
| `akm_tr_reminder`, `akm_tr_escalation` | AKM-TR reminder 10 AT, escalation 4 AT before | §2.6.2 |
| `new_vorabstimmung` | after an agreed change, the new VA by 7 AT before | §2.10 |
| `later_porting` | later porting within 65 AT of switch-off | §2.3 |

All times are `Europe/Berlin`, compiled into the crate; summer time is handled. A `Deadline`
names the party that must act, the instant before which it must act (`due`) and
`is_overdue(now)`.
