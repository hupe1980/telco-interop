//! Arbitrary bytes into the reader; whatever reads is validated, written back and fed to a
//! process. Nothing may panic (wbci-switch/SR-003).
#![no_main]

use libfuzzer_sys::fuzz_target;
use wbci::{Anfrage, Direction, Payload, Process, Role, Timestamp};

fuzz_target!(|data: &[u8]| {
    let Ok(payload) = Payload::from_xml(data) else { return };
    let xml = payload.to_xml();
    // The peer's own timestamp drives the deadlines: extreme values must not panic.
    let at = payload.control().zeitstempel.timestamp;
    match payload {
        Payload::AnfrageRequest { anfrage, .. } => {
            let _ = anfrage.issues_at(at, &wbci::Calendar::wbci());
            if let Anfrage::Vorabstimmung(va) = anfrage {
                if let Ok(mut p) = Process::new(Role::EkpAbg, va.clone(), Direction::Received, at) {
                    let _ = p.apply(Direction::Received, at, Anfrage::Vorabstimmung(va));
                    let _ = p.deadlines();
                    let _ = p.overdue(Timestamp::MAX);
                    let _ = p.next_actions(Role::EkpAuf);
                }
            }
        }
        Payload::MeldungRequest { meldung, .. } => {
            let _ = meldung.issues(None);
        }
        _ => {}
    }
    // The reader enforces the schema's repetitions, so what it accepts is writable.
    assert!(xml.is_ok(), "read but not writable");
});
