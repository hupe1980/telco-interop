//! Property tests: generated values round-trip, and no input makes the reader panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use proptest::prelude::*;
use wbci::*;

fn charset_text(max: usize) -> impl Strategy<Value = String> {
    // Letters, digits, umlauts, punctuation and markup characters from the WBCI set.
    proptest::string::string_regex(&format!("[A-Za-z0-9äöüÄÖÜß<>&\"' .,/-]{{1,{max}}}")).unwrap()
}

proptest! {
    #[test]
    fn names_round_trip(vor in charset_text(30), nach in charset_text(30), ort in charset_text(40)) {
        let (Ok(vorname), Ok(nachname), Ok(ortsname)) = (Vorname::new(&vor), Nachname::new(&nach), Ortsname::new(&ort)) else {
            return Ok(()); // all-whitespace input collapses to empty: rejected, which is correct
        };
        let mut va = common::va_kue_mrn();
        va.endkunde = Kunde::person(Anrede::Keine, Some(vorname), nachname);
        if let VorabstimmungsFall::KueMrn { standort, .. } = &mut va.fall {
            standort.ort = ortsname;
        }
        let payload = Payload::AnfrageRequest { control: common::control("2026-11-23T09:30:00+01:00"), anfrage: va.into() };
        let xml = payload.to_xml().unwrap();
        prop_assert_eq!(Payload::from_xml(xml.as_bytes()).unwrap(), payload);
    }

    #[test]
    fn ids_round_trip(seq in 0u64..=MAX_SEQUENCE, carrier in "[0-9A-Z]{3}\\.[0-9A-Z]{1,6}") {
        let cc: CarrierCode = carrier.parse().unwrap();
        let id = VorabstimmungsId::allocate(&cc, seq).unwrap();
        prop_assert_eq!(id.as_str().parse::<VorabstimmungsId>().unwrap(), id.clone());
        prop_assert_eq!(id.carrier(), cc);
    }

    #[test]
    fn dates_and_timestamps_round_trip(days in 0i32..20_000, secs in 0i64..86_400, offset_min in -720i32..=840) {
        let termin = Date::new(2000, 1, 1).unwrap().checked_add(jiff::Span::new().days(days)).unwrap();
        let offset = jiff::tz::Offset::from_seconds(offset_min * 60).unwrap();
        let ts = Timestamp::from_second(1_700_000_000 + secs).unwrap();
        let mut va = common::va_kue_orn();
        va.kundenwunschtermin = termin;
        let mut control = common::control("2026-11-23T09:30:00Z");
        control.zeitstempel = Zeitstempel { timestamp: ts, offset };
        let payload = Payload::AnfrageRequest { control, anfrage: va.into() };
        let xml = payload.to_xml().unwrap();
        prop_assert_eq!(Payload::from_xml(xml.as_bytes()).unwrap(), payload);
    }

    // wbci-switch/SR-003: arbitrary bytes never panic the reader.
    #[test]
    fn reader_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
        let _ = Payload::from_xml(&bytes);
    }

    // wbci-switch/SR-003: mutated fixtures never panic the reader.
    #[test]
    fn mutated_fixtures_never_panic(i in 0usize..15, pos in any::<prop::sample::Index>(), byte in any::<u8>()) {
        let (_, payload) = common::all_payloads().swap_remove(i);
        let mut xml = payload.to_xml().unwrap().into_bytes();
        let at = pos.index(xml.len());
        xml[at] = byte;
        let _ = Payload::from_xml(&xml);
    }
}
