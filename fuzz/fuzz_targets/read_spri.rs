//! Arbitrary bytes into the S/PRI reader; whatever reads is written back, read again and must be
//! equal. Nothing may panic (spri-model/SR-001).
//!
//! The input may hold several documents separated by NUL bytes: the first order (or KDLE, or
//! AKM-PV) read starts processes for every role that can own it, and every later order, message
//! and receipt is applied to them as sent and as received. Applying never panics, and a refused
//! event leaves the process unchanged (spri-lifecycle/SR-003).
#![no_main]

use libfuzzer_sys::fuzz_target;
use spri::process::{Direction, Message, Process, Role};
use spri::{Meldungstyp, Payload, Timestamp};

const ROLES: [Role; 3] = [Role::Auftraggeber, Role::Leistungserbringer, Role::AbgebenderProvider];

fn start(message: &Message, at: Timestamp) -> Vec<Process> {
    let mut out = Vec::new();
    match message {
        Message::Auftrag(a) => {
            out.extend(Process::new(Role::Auftraggeber, a, Direction::Sent, at));
            out.extend(Process::new(Role::Leistungserbringer, a, Direction::Received, at));
        }
        Message::Meldung(m) if m.typ() == Meldungstyp::Kdle => {
            out.extend(Process::from_kdle(Role::Auftraggeber, m, Direction::Received, at));
            out.extend(Process::from_kdle(Role::Leistungserbringer, m, Direction::Sent, at));
        }
        Message::Meldung(m) => out.extend(Process::from_akm_pv(m, at)),
        Message::Teq(_) => {}
    }
    out
}

fuzz_target!(|data: &[u8]| {
    let base: Timestamp = "2026-11-23T08:00:00Z".parse().expect("a valid instant");
    let mut processes: Vec<Process> = Vec::new();
    for (i, chunk) in data.split(|b| *b == 0).take(32).enumerate() {
        let Ok(payload) = Payload::from_xml(chunk) else { continue };
        let _ = format!("{payload:?}");
        if let Payload::MeldungRequest { meldung, .. } = &payload {
            let _ = meldung.inhalt.codes();
        }
        // The reader enforces the schema's repetitions, so what it accepts is writable and reads
        // back to the same value.
        let xml = payload.to_xml().expect("read but not writable");
        let again = Payload::from_xml(xml.as_bytes()).expect("written but not readable");
        assert_eq!(again, payload, "not stable");

        // Steps of a minute, with an occasional jump of weeks so deadlines pass.
        let minutes = i64::try_from(i).unwrap_or(0) * if chunk.len() % 7 == 0 { 20_000 } else { 1 };
        let at = Timestamp::from_second(base.as_second() + minutes * 60).unwrap_or(base);
        let message: Message = match payload {
            Payload::AuftragRequest { auftrag, .. } => auftrag.into(),
            Payload::MeldungRequest { meldung, .. } => meldung.into(),
            Payload::AuftragResponse { quittung, .. } | Payload::MeldungResponse { quittung, .. } => quittung.into(),
        };
        if processes.is_empty() {
            processes = start(&message, at);
            continue;
        }
        for p in &mut processes {
            for direction in [Direction::Sent, Direction::Received] {
                let before = p.clone();
                if let Err(refusal) = p.apply(direction, at, message.clone()) {
                    assert_eq!(*p, before, "a refusal changed the process");
                    let _ = refusal.to_string();
                }
            }
            for role in ROLES {
                let _ = p.next_actions(role);
            }
            for d in p.deadlines() {
                let _ = d.to_string();
                let _ = d.explanation.to_string();
            }
            let _ = p.overdue(at);
            let _ = p.state().to_string();
            let _ = format!("{p:?}");
        }
    }
});
